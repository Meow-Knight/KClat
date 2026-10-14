// Composes renderer, rig, controller, bubble and render loop inside one container element.
// Shared by the overlay window and the browser dev page.
import * as THREE from 'three';
import { Bubble } from '../bubble/Bubble';
import { ReadyButton } from '../bubble/ReadyButton';
import { PHRASES } from '../phrases/phrases';
import { PhrasePicker, type PickerOptions, type WakeReason } from '../phrases/PhrasePicker';
import { phraseLanguages, type CatSettings, type UiLanguage } from '../settings';
import { CatController, type CatPhase, type SleepStyle } from './CatController';
import { createRig } from '../characters';
import type { CharacterRig } from '../kit/CharacterRig';
import { RenderLoop } from './RenderLoop';
import '../assets/fonts/fonts.css';
import './stage.css';

const AWAKE_FPS = 60;
const ASLEEP_FPS = 20;

/**
 * Framing for the bottom 280px of the stage: cat on the right, room for the bubble on the left. A taller
 * stage keeps the same scale and adds headroom above (for the aura's spikes) instead of zooming in.
 */
const CAMERA_POS = new THREE.Vector3(0.5, 2.3, 11.5);
const CAMERA_TARGET = new THREE.Vector3(0.5, 2.0, 0);
const BASE_HEIGHT = 280;
const BASE_FOV = 30;
const TAN_HALF_FOV = Math.tan(THREE.MathUtils.degToRad(BASE_FOV / 2));
/** World units per CSS px at the cat, for the base framing. */
const WORLD_PER_PX = (2 * CAMERA_POS.distanceTo(CAMERA_TARGET) * TAN_HALF_FOV) / BASE_HEIGHT;
/** Where the Ready button sits: on the ground to the right of the sleeping cat (world space). */
const READY_ANCHOR = new THREE.Vector3(3.2, 0.35, 0.9);
/** Canvas resolution: the display's pixel ratio (which includes the cat-size zoom), capped. */
const pixelRatio = () => Math.min(window.devicePixelRatio, 3);
/** How long the cat's "fine, I'll wake you" reply stays up. */
const READY_REPLY_MS = 2200;

/** Logical px relative to the stage's top-left corner. */
export interface StageRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface StageCallbacks {
  /** The bubble was clicked. */
  onDismiss(): void;
  onPhase?(phase: CatPhase): void;
  /** hitboxes() changed: the bubble appeared or went away, the pose changed, or the stage resized. */
  onLayoutChange?(): void;
  /** The Ready button was clicked; `next` is the state asked for. The owner applies it with setReady(). */
  onReadyToggle?(next: boolean): void;
}

export class CatStage {
  readonly controller: CatController;
  readonly bubble: Bubble;
  readonly readyButton: ReadyButton;

  private readonly renderer: THREE.WebGLRenderer;
  private readonly scene = new THREE.Scene();
  private readonly camera = new THREE.PerspectiveCamera(30, 1, 0.1, 100);
  private rig: CharacterRig;
  private readonly picker: PhrasePicker;
  private readonly loop: RenderLoop;
  private readonly resizeObserver: ResizeObserver;
  /** Matches the current devicePixelRatio; fires when it changes (cat size, or a monitor with another DPI). */
  private dprQuery: MediaQueryList | undefined;
  private readonly head = { x: 0, y: 0 };
  private readonly probe = new THREE.Vector3();
  private readonly lookTarget = new THREE.Vector3();
  private width = 1;
  private height = 1;
  private subtitle: string | undefined;
  private settings: CatSettings;
  private phase: CatPhase = 'asleep';
  private hidden = false;
  private readyReplyTimer: ReturnType<typeof setTimeout> | undefined;
  private grab: 'idle' | 'armed' | 'dragging' = 'idle';
  /** The mouse went down while Space was held: that press moves the cat, so its click must not pet or dismiss. */
  private pressInGrab = false;

  constructor(
    private readonly container: HTMLElement,
    settings: CatSettings,
    private readonly callbacks: StageCallbacks,
  ) {
    this.settings = { ...settings };
    container.classList.add('cat-stage', 'asleep');

    const canvas = document.createElement('canvas');
    container.append(canvas);
    this.renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, premultipliedAlpha: true });
    this.renderer.setPixelRatio(pixelRatio());
    this.renderer.setClearColor(0x000000, 0);

    this.camera.position.copy(CAMERA_POS);
    this.camera.lookAt(CAMERA_TARGET);
    this.scene.add(new THREE.AmbientLight(0xffffff, 0.95));
    const sun = new THREE.DirectionalLight(0xffffff, 2.3);
    sun.position.set(-3, 5, 6);
    this.rig = createRig(settings.character);
    this.scene.add(sun, this.rig.object);


    this.bubble = new Bubble(container, () => callbacks.onDismiss(), settings.reduceMotion);
    this.picker = new PhrasePicker(PHRASES, pickerOptions(settings));
    this.readyButton = new ReadyButton(container, (next) => {
      if (next) this.replyReady();
      callbacks.onReadyToggle?.(next);
    });

    this.controller = new CatController(
      {
        say: (req) => {
          clearTimeout(this.readyReplyTimer);
          const wasVisible = this.bubble.visible;
          this.bubble.show(this.picker.pick(req).text, this.subtitle);
          if (!wasVisible) callbacks.onLayoutChange?.();
        },
        hush: () => {
          this.bubble.hide();
          callbacks.onLayoutChange?.();
        },
      },
      settings,
      (phase) => {
        this.phase = phase;
        container.classList.toggle('asleep', phase === 'asleep');
        this.syncReadyButton();
        this.loop.setMaxFps(phase === 'asleep' ? ASLEEP_FPS : AWAKE_FPS);
        callbacks.onPhase?.(phase);
        callbacks.onLayoutChange?.();
      },
    );

    // Capture phase, so a press that moved the cat never reaches the canvas, the bubble or the Ready button.
    container.addEventListener('pointerdown', () => (this.pressInGrab = this.grab !== 'idle'), true);
    for (const type of ['click', 'dblclick']) {
      container.addEventListener(
        type,
        (e) => {
          if (!this.pressInGrab) return;
          e.stopPropagation();
          e.preventDefault();
          if (type === 'click') this.pressInGrab = this.grab !== 'idle';
        },
        true,
      );
    }

    canvas.addEventListener('click', () => this.controller.pet());
    // Double-click the awake cat: "not now" - it goes back to sleep, like clicking the bubble.
    canvas.addEventListener('dblclick', () => {
      if (this.controller.current === 'active') callbacks.onDismiss();
    });

    this.loop = new RenderLoop((dt, now) => this.frame(dt, now));
    this.loop.setMaxFps(ASLEEP_FPS);
    this.setReduceMotionClass();

    // The observer also fires once right after observe(), which reports the first layout.
    this.resizeObserver = new ResizeObserver(() => this.resize(true));
    this.resizeObserver.observe(container);
    this.resize(false);
    this.watchPixelRatio();
    this.loop.start();
  }

  wake(reason?: WakeReason): void {
    this.controller.wake(reason);
  }

  /** 'knockout' when you did something (answered, dismissed); 'yawn' when nobody did. */
  sleep(style: SleepStyle = 'yawn'): void {
    this.controller.sleep(style);
  }

  yellNow(reason?: WakeReason): void {
    this.controller.yellNow(reason);
  }

  pet(): void {
    this.controller.pet();
  }

  /** Platform Language for the overlay's own text (the Ready button tooltip). */
  setUiLanguage(lang: UiLanguage): void {
    this.readyButton.setLanguage(lang);
  }

  /** Reflects the Ready state (from Rust, or the dev page) on the button and the cat's ears. */
  setReady(on: boolean): void {
    this.readyButton.set(on);
    this.controller.setReady(on);
  }

  /** Space held over the cat ('armed'), then the mouse pressed ('dragging'): shows a hand cursor. */
  setGrab(mode: 'idle' | 'armed' | 'dragging'): void {
    this.grab = mode;
    this.container.classList.toggle('grab-armed', mode === 'armed');
    this.container.classList.toggle('grab-dragging', mode === 'dragging');
  }

  /** Project name under the phrase (only set when more than one session is waiting). */
  setSubtitle(subtitle: string | undefined): void {
    this.subtitle = subtitle;
  }

  /** Cursor position in CSS px relative to the stage's top-left corner; null = no cursor data. */
  lookAt(point: { x: number; y: number } | null): void {
    if (point) this.controller.setLook(point.x - this.head.x, point.y - this.head.y);
    else this.controller.lookDefault();
  }

  /**
   * Hide the cat without hiding the window (hideWhileSleeping). Showing a hidden Tauri window can take focus
   * on Windows, so the overlay stays up, renders nothing, stops its loop and reports no hitboxes.
   */
  setHidden(hidden: boolean): void {
    if (hidden === this.hidden) return;
    this.hidden = hidden;
    this.container.style.visibility = hidden ? 'hidden' : '';
    if (hidden) this.loop.stop();
    else this.loop.start();
    this.syncReadyButton();
    this.callbacks.onLayoutChange?.();
  }

  get isHidden(): boolean {
    return this.hidden;
  }

  /** Clickable regions: the cat (a generous box for the current pose), the bubble and the Ready button. */
  hitboxes(): StageRect[] {
    if (this.hidden) return [];
    const rects = [this.projectBox(this.rig.hitBounds[this.phase === 'awake' ? 'awake' : 'asleep'])];
    // Layout boxes, not bounding rects: the bubble's pop-in transform starts at scale(0).
    for (const el of [this.bubble.visible ? this.bubble.el : null, this.readyButton.visible ? this.readyButton.el : null]) {
      if (el) rects.push({ x: el.offsetLeft, y: el.offsetTop, w: el.offsetWidth, h: el.offsetHeight });
    }
    return rects;
  }

  setSettings(settings: CatSettings): void {
    const switched = settings.character !== this.settings.character;
    this.settings = { ...settings };
    if (switched) this.swapRig();
    this.controller.setOptions(settings);
    this.picker.setOptions(pickerOptions(settings));
    this.bubble.setReduceMotion(settings.reduceMotion);
    this.setReduceMotionClass();
  }

  dispose(): void {
    clearTimeout(this.readyReplyTimer);
    this.loop.dispose();
    this.resizeObserver.disconnect();
    this.dprQuery?.removeEventListener('change', this.onPixelRatioChange);
    this.controller.dispose();
    this.rig.dispose();
    this.renderer.dispose();
    this.container.replaceChildren();
    this.container.classList.remove('cat-stage', 'asleep', 'reduce-motion', 'grab-armed', 'grab-dragging');
  }

  /** Another character picked in Settings: same brain and pose, new body. Its hitbox may differ. */
  private swapRig(): void {
    this.scene.remove(this.rig.object);
    this.rig.dispose();
    this.rig = createRig(this.settings.character);
    this.scene.add(this.rig.object);
    this.callbacks.onLayoutChange?.();
  }

  private frame(dt: number, now: number): void {
    this.controller.update(dt);
    this.rig.applyPose(this.controller.pose, now / 1000);
    this.renderer.render(this.scene, this.camera);
    this.rig.headScreen(this.camera, this.width, this.height, this.head);
  }

  private resize(notify: boolean): void {
    this.width = Math.max(1, this.container.clientWidth);
    this.height = Math.max(1, this.container.clientHeight);
    this.renderer.setSize(this.width, this.height, false);
    this.camera.aspect = this.width / this.height;
    // Same px-per-world scale as the base framing; extra height becomes headroom above the cat.
    this.camera.fov = THREE.MathUtils.radToDeg(2 * Math.atan((TAN_HALF_FOV * this.height) / BASE_HEIGHT));
    const lift = ((this.height - BASE_HEIGHT) / 2) * WORLD_PER_PX;
    this.camera.position.set(CAMERA_POS.x, CAMERA_POS.y + lift, CAMERA_POS.z);
    this.camera.lookAt(this.lookTarget.set(CAMERA_TARGET.x, CAMERA_TARGET.y + lift, CAMERA_TARGET.z));
    this.camera.updateProjectionMatrix();

    this.probe.copy(READY_ANCHOR).project(this.camera);
    this.readyButton.placeAt(((this.probe.x + 1) / 2) * this.width, ((1 - this.probe.y) / 2) * this.height);
    if (notify) this.callbacks.onLayoutChange?.();
  }

  /**
   * A zoom (the cat size) or a DPI change leaves the stage's CSS size alone, so the ResizeObserver stays quiet:
   * follow devicePixelRatio instead, or the canvas would be stretched and blurry.
   */
  private watchPixelRatio(): void {
    this.dprQuery?.removeEventListener('change', this.onPixelRatioChange);
    this.dprQuery = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
    this.dprQuery.addEventListener('change', this.onPixelRatioChange);
  }

  private readonly onPixelRatioChange = (): void => {
    this.renderer.setPixelRatio(pixelRatio());
    this.watchPixelRatio();
  };

  /** The button is only offered while the cat is asleep and shown; Ready is meaningless once it is awake. */
  private syncReadyButton(): void {
    this.readyButton.setVisible(this.phase === 'asleep' && !this.hidden);
  }

  /** The cat's short reply when you arm Ready, then the bubble goes away again. */
  private replyReady(): void {
    if (this.controller.current !== 'sleep') return;
    const wasVisible = this.bubble.visible;
    this.bubble.show(this.picker.pickReady().text);
    if (!wasVisible) this.callbacks.onLayoutChange?.();
    clearTimeout(this.readyReplyTimer);
    this.readyReplyTimer = setTimeout(() => {
      if (this.controller.current !== 'sleep') return;
      this.bubble.hide();
      this.callbacks.onLayoutChange?.();
    }, READY_REPLY_MS);
  }

  /** Screen-space bounding rect of a world box, clamped to the stage. */
  private projectBox(box: THREE.Box3): StageRect {
    let x0 = Infinity;
    let y0 = Infinity;
    let x1 = -Infinity;
    let y1 = -Infinity;
    for (let i = 0; i < 8; i++) {
      this.probe
        .set(i & 1 ? box.max.x : box.min.x, i & 2 ? box.max.y : box.min.y, i & 4 ? box.max.z : box.min.z)
        .project(this.camera);
      const sx = ((this.probe.x + 1) / 2) * this.width;
      const sy = ((1 - this.probe.y) / 2) * this.height;
      x0 = Math.min(x0, sx);
      y0 = Math.min(y0, sy);
      x1 = Math.max(x1, sx);
      y1 = Math.max(y1, sy);
    }
    x0 = Math.max(0, x0);
    y0 = Math.max(0, y0);
    x1 = Math.min(this.width, x1);
    y1 = Math.min(this.height, y1);
    return { x: x0, y: y0, w: Math.max(0, x1 - x0), h: Math.max(0, y1 - y0) };
  }

  private setReduceMotionClass(): void {
    this.container.classList.toggle('reduce-motion', this.settings.reduceMotion);
  }
}

function pickerOptions(s: CatSettings): PickerOptions {
  return { sfwMode: s.sfwMode, languages: phraseLanguages(s.klatLanguage) };
}
