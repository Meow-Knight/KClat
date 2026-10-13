// Overlay window entry: wires Tauri state/settings events to the cat, and reports hitboxes for click-through.
import { bridge, type CatStatePayload } from '../bridge/tauriBridge';
import { CatStage } from '../cat/CatStage';
import type { CatPhase } from '../cat/CatController';
import type { AppSettings } from '../settings';

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

function applyState(s: CatStatePayload): void {
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
    stage.sleep();
  }
  lastAttention = seq;
}

function applySettings(next: AppSettings): void {
  settings = next;
  stage.setSettings(next);
  syncHidden();
}

// Subscribe before reading the current state so no change can slip in between.
await bridge.onState(applyState);
await bridge.onSettings(applySettings);
// Same coordinate space as the stage: the stage fills the window. null = cursor on another monitor.
await bridge.onCursor((p) => stage.lookAt(p));
await bridge.onGrab((mode) => stage.setGrab(mode));
ready = true;
applyState(await bridge.getState());
syncHidden();
void bridge.setHitboxes(stage.hitboxes());

// Dev builds only: lets tooling (WebView2 remote debugging) inspect the cat, as dev.html does.
if (import.meta.env.DEV) Object.assign(window, { stage });
