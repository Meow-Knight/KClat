// Overlay window entry: wires Tauri state/settings events to the cat, and reports hitboxes for click-through.
import { bridge, type CatState, type ZoomChange } from '../bridge/tauriBridge';
import { CatStage } from '../cat/CatStage';
import type { CatPhase } from '../cat/CatController';
import { CAT_SCALE, type AppSettings } from '../settings';

const stageEl = document.querySelector<HTMLDivElement>('#stage')!;

let settings: AppSettings = await bridge.getSettings();
let phase: CatPhase = 'asleep';
let ready = false;

const stage = new CatStage(stageEl, settings, {
  onDismiss: () => void bridge.dismiss(),
  onPhase: (p) => {
    phase = p;
    syncHidden();
  },
  onLayoutChange: () => {
    if (ready) void bridge.setHitboxes(stage.hitboxes());
  },
  // Rust owns Ready; its state comes back through cat://state and applyState().
  onReadyToggle: (next) => void bridge.setReady(next),
});

/** hideWhileSleeping: the cat disappears once the sleep sequence has finished. */
function syncHidden(): void {
  stage.setHidden(settings.hideWhileSleeping && phase === 'asleep');
}

let lastAttention = 0;

function applyState(s: CatState): void {
  stage.setReady(s.ready);
  // Name the project only when more than one session is waiting (spec §7.2 rule 5).
  stage.setSubtitle(s.sessions.length > 1 ? s.attention?.projectName || undefined : undefined);
  const seq = s.attention?.seq ?? lastAttention;
  if (s.awake) {
    stage.setHidden(false); // show before wake() (spec §9)
    if (stage.controller.current !== 'active') {
      stage.wake(s.reason ?? 'done');
    } else if (s.attention && seq !== lastAttention) {
      // Already awake and another session (or a new reason) needs you: yell now, don't re-run wake (§8.2).
      stage.yellNow(s.attention.reason);
    }
  } else {
    // Something you did (answered Claude, dismissed) gets the cat knocked out; an expiry or Pause, a yawn.
    stage.sleep(s.sleepCause === 'user' ? 'knockout' : 'yawn');
  }
  lastAttention = seq;
}

/** The window's zoom while the size slider is dragged (src-tauri/src/ui/overlay.rs PREVIEW_ZOOM). */
const PREVIEW_ZOOM = CAT_SCALE.max / 100;

function applySettings(next: AppSettings): void {
  const rescaled = next.catScale !== settings.catScale;
  settings = next;
  stage.setSettings(next);
  if (rescaled && !previewing) stage.setZoom(next.catScale / 100);
  stage.setUiLanguage(next.uiLanguage);
  syncHidden();
}

/**
 * Size slider (src-tauri/src/ui/overlay.rs): while it is dragged the window sits at the largest size and the cat is
 * scaled with a transform, which the compositor does every frame. Switching the window between that and the
 * real size takes a few frames to land (size, position and zoom separately), so the cat is hidden meanwhile.
 */
let previewing = false;
let switchToken = 0;
/** Give up waiting for a switch after this long and show the cat anyway. */
const SWITCH_TIMEOUT_MS = 600;

async function switchWindow(preview: boolean): Promise<void> {
  const token = ++switchToken;
  const dpr = window.devicePixelRatio;
  stageEl.style.visibility = 'hidden';
  let change: ZoomChange;
  try {
    change = await bridge.overlayPreviewZoom(preview);
  } catch {
    stageEl.style.visibility = '';
    return;
  }
  const dprAfter = (dpr * change.to) / change.from;
  const deadline = performance.now() + SWITCH_TIMEOUT_MS;
  const landed = () =>
    Math.abs(window.devicePixelRatio - dprAfter) < 0.01 &&
    Math.abs(window.innerWidth - change.width) < 2 &&
    Math.abs(window.innerHeight - change.height) < 2;
  // One more frame once it has landed, for the stage to draw at the new size.
  const check = (settledFrames: number) => {
    if (token !== switchToken) return;
    if (settledFrames >= 2 || performance.now() > deadline) {
      stageEl.style.visibility = '';
      return;
    }
    requestAnimationFrame(() => check(landed() ? settledFrames + 1 : 0));
  };
  check(0);
}

function applyScalePreview(factor: number | null): void {
  if (factor !== null) {
    stageEl.style.transform = `scale(${factor})`;
    stage.setZoom(PREVIEW_ZOOM * factor);
    if (!previewing) {
      previewing = true;
      void switchWindow(true);
    }
    return;
  }
  // The drag is over. Asked even if this page missed the start, so the window always ends at the saved size.
  const wasPreviewing = previewing;
  previewing = false;
  if (!wasPreviewing) {
    void bridge.overlayPreviewZoom(false);
    return;
  }
  stageEl.style.visibility = 'hidden';
  stageEl.style.transform = '';
  stage.setZoom(settings.catScale / 100);
  void switchWindow(false);
}

// Subscribe before reading the current state so no change can slip in between.
await bridge.onState(applyState);
await bridge.onSettings(applySettings);
// Same coordinate space as the stage: the stage fills the window. null = cursor on another monitor.
await bridge.onCursor((p) => stage.lookAt(p));
await bridge.onGrab((mode) => stage.setGrab(mode));
await bridge.onScalePreview(applyScalePreview);
ready = true;
stage.setUiLanguage(settings.uiLanguage);
stage.setZoom(settings.catScale / 100);
applyState(await bridge.getState());
syncHidden();
void bridge.setHitboxes(stage.hitboxes());

// Dev builds only: lets tooling (WebView2 remote debugging) inspect the cat, as dev.html does.
if (import.meta.env.DEV) Object.assign(window, { stage });
