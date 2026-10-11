// Composes renderer, rig, controller, bubble and render loop inside one container element.
// Shared by the overlay window and the browser dev page.
import * as THREE from 'three';
import { Bubble } from '../bubble/Bubble';
import { PHRASES } from '../phrases/phrases';
import { PhrasePicker, type WakeReason } from '../phrases/PhrasePicker';
import type { CatSettings } from '../settings';
import { CatController, type CatPhase } from './CatController';
import { CatRig, HIT_BOUNDS } from './CatRig';
import { RenderLoop } from './RenderLoop';
import './stage.css';

const AWAKE_FPS = 60;
const ASLEEP_FPS = 20;

/** Framing for a ~360×280 stage: cat on the right, room for the bubble on the left. */
const CAMERA_POS = new THREE.Vector3(0.5, 2.3, 11.5);
const CAMERA_TARGET = new THREE.Vector3(0.5, 2.0, 0);

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
}

export class CatStage {
  readonly controller: CatController;
  readonly bubble: Bubble;

  private readonly renderer: THREE.WebGLRenderer;
  private readonly scene = new THREE.Scene();
  private readonly camera = new THREE.PerspectiveCamera(30, 1, 0.1, 100);
  private readonly rig = new CatRig();
  private readonly picker: PhrasePicker;
  private readonly loop: RenderLoop;
  private readonly resizeObserver: ResizeObserver;
  private readonly head = { x: 0, y: 0 };
  private readonly probe = new THREE.Vector3();
  private width = 1;
  private height = 1;
  private subtitle: string | undefined;
  private settings: CatSettings;
  private phase: CatPhase = 'asleep';
  private hidden = false;

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
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    this.renderer.setClearColor(0x000000, 0);

    this.camera.position.copy(CAMERA_POS);
    this.camera.lookAt(CAMERA_TARGET);
    this.scene.add(new THREE.AmbientLight(0xffffff, 0.95));
    const sun = new THREE.DirectionalLight(0xffffff, 2.3);
    sun.position.set(-3, 5, 6);
    this.scene.add(sun, this.rig.object);


    this.bubble = new Bubble(container, () => callbacks.onDismiss(), settings.reduceMotion);
    this.picker = new PhrasePicker(PHRASES, settings);

    this.controller = new CatController(
      {
        say: (req) => {
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
        this.loop.setMaxFps(phase === 'asleep' ? ASLEEP_FPS : AWAKE_FPS);
        callbacks.onPhase?.(phase);
        callbacks.onLayoutChange?.();
      },
    );

    canvas.addEventListener('click', () => this.controller.pet());

    this.loop = new RenderLoop((dt, now) => this.frame(dt, now));
    this.loop.setMaxFps(ASLEEP_FPS);
    this.setReduceMotionClass();

    // The observer also fires once right after observe(), which reports the first layout.
    this.resizeObserver = new ResizeObserver(() => this.resize(true));
    this.resizeObserver.observe(container);
    this.resize(false);
    this.loop.start();
  }

  wake(reason?: WakeReason): void {
    this.controller.wake(reason);
  }

  sleep(): void {
    this.controller.sleep();
  }

  yellNow(reason?: WakeReason): void {
    this.controller.yellNow(reason);
  }

  pet(): void {
    this.controller.pet();
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
    this.callbacks.onLayoutChange?.();
  }

  get isHidden(): boolean {
    return this.hidden;
  }

  /** Clickable regions: the cat (a generous box for the current pose) and the bubble while it shows. */
  hitboxes(): StageRect[] {
    if (this.hidden) return [];
    const rects = [this.projectBox(HIT_BOUNDS[this.phase === 'awake' ? 'awake' : 'asleep'])];
    if (this.bubble.visible) {
      const el = this.bubble.el;
      // Layout box, not the bounding rect: the pop-in transform starts at scale(0).
      rects.push({ x: el.offsetLeft, y: el.offsetTop, w: el.offsetWidth, h: el.offsetHeight });
    }
    return rects;
  }

  setSettings(settings: CatSettings): void {
    this.settings = { ...settings };
    this.controller.setOptions(settings);
    this.picker.setOptions(settings);
    this.bubble.setReduceMotion(settings.reduceMotion);
    this.setReduceMotionClass();
  }

  dispose(): void {
    this.loop.dispose();
    this.resizeObserver.disconnect();
    this.controller.dispose();
    this.rig.dispose();
    this.renderer.dispose();
    this.container.replaceChildren();
    this.container.classList.remove('cat-stage', 'asleep', 'reduce-motion');
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
    this.camera.updateProjectionMatrix();

    if (notify) this.callbacks.onLayoutChange?.();
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
