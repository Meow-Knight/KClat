// Browser dev page (pnpm dev → /dev.html): drive the cat with buttons, no Tauri needed.
import { CatStage } from '../cat/CatStage';
import type { WakeReason } from '../phrases/PhrasePicker';
import { DEFAULT_SETTINGS, type CatSettings, type CharacterId, type KlatLanguage } from '../settings';

const $ = <T extends HTMLElement>(id: string): T => document.getElementById(id) as T;

const stageEl = $<HTMLDivElement>('stage');
const log = $<HTMLDivElement>('log');
const reason = (): WakeReason => $<HTMLSelectElement>('reason').value as WakeReason;

function readSettings(): CatSettings {
  return {
    ...DEFAULT_SETTINGS,
    character: $<HTMLSelectElement>('character').value as CharacterId,
    sfwMode: $<HTMLInputElement>('sfw').checked,
    reduceMotion: $<HTMLInputElement>('reduce').checked,
    klatLanguage: $<HTMLSelectElement>('klatLanguage').value as KlatLanguage,
    yellIntervalMs: Math.max(1000, Number($<HTMLInputElement>('interval').value) || DEFAULT_SETTINGS.yellIntervalMs),
    maxYellsPerWake: Math.max(1, Number($<HTMLInputElement>('max').value) || DEFAULT_SETTINGS.maxYellsPerWake),
  };
}

const stage = new CatStage(stageEl, readSettings(), {
  onDismiss: () => {
    note('dismiss (bubble click or double-click on the awake cat): knockout');
    stage.sleep('knockout');
  },
  onPhase: (phase) => note(`phase: ${phase}`),
  onLayoutChange: () => drawHitboxes(),
  onReadyToggle: (next) => {
    note(`ready ${next ? 'armed, cat out of the lamp' : 'off, cat in the lamp'} (in the app, Rust decides when it fires)`);
    stage.setReady(next);
  },
});
// As the app starts: Ready on, so the cat is out of the lamp.
stage.setReady(true);

// Hitboxes: what the overlay reports to Rust as clickable (everything else is click-through).
const hitLayer = document.createElement('div');
$('desktop').append(hitLayer);
function drawHitboxes(): void {
  hitLayer.replaceChildren();
  if (!$<HTMLInputElement>('showHit').checked) return;
  for (const r of stage.hitboxes()) {
    const box = document.createElement('div');
    box.className = 'hitbox';
    Object.assign(box.style, { left: `${r.x}px`, top: `${r.y}px`, width: `${r.w}px`, height: `${r.h}px` });
    hitLayer.append(box);
  }
}
$('showHit').addEventListener('change', drawHitboxes);

function note(msg: string): void {
  log.textContent = `${new Date().toLocaleTimeString()}  ${msg}`;
}

$('wake').addEventListener('click', () => stage.wake(reason()));
$('sleep').addEventListener('click', () => stage.sleep());
$('knockout').addEventListener('click', () => {
  stage.sleep('knockout');
  note(`knockout: ${stage.controller.knockoutObject ?? 'no object (reduce motion, or already asleep)'}`);
});
$('next').addEventListener('click', () => stage.yellNow(reason()));
$('pet').addEventListener('click', () => stage.pet());
for (const id of ['character', 'sfw', 'reduce', 'klatLanguage', 'interval', 'max']) {
  $(id).addEventListener('change', () => stage.setSettings(readSettings()));
}
$<HTMLInputElement>('dark').addEventListener('change', (e) => {
  $('desktop').classList.toggle('plain', (e.target as HTMLInputElement).checked);
});

// The page mouse stands in for the screen cursor.
const cursor = { x: 0, y: 0 };
window.addEventListener('pointermove', (e) => {
  const r = stageEl.getBoundingClientRect();
  cursor.x = e.clientX - r.left;
  cursor.y = e.clientY - r.top;
  stage.lookAt(cursor);
});
document.documentElement.addEventListener('pointerleave', () => stage.lookAt(null));

Object.assign(window, { stage });
