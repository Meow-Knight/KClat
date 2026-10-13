// Browser dev page (pnpm dev → /dev.html): drive the cat with buttons, no Tauri needed.
import { CatStage } from '../cat/CatStage';
import type { WakeReason } from '../phrases/PhrasePicker';
import { DEFAULT_SETTINGS, type CatSettings, type KlatLanguage } from '../settings';

const $ = <T extends HTMLElement>(id: string): T => document.getElementById(id) as T;

const stageEl = $<HTMLDivElement>('stage');
const log = $<HTMLDivElement>('log');
const reason = (): WakeReason => $<HTMLSelectElement>('reason').value as WakeReason;

function readSettings(): CatSettings {
  const en = $<HTMLInputElement>('en').checked;
  const vi = $<HTMLInputElement>('vi').checked;
  const klatLanguage: KlatLanguage = en === vi ? 'auto' : en ? 'en' : 'vi';
  return {
    ...DEFAULT_SETTINGS,
    sfwMode: $<HTMLInputElement>('sfw').checked,
    reduceMotion: $<HTMLInputElement>('reduce').checked,
    klatLanguage,
    yellIntervalMs: Math.max(1000, Number($<HTMLInputElement>('interval').value) || DEFAULT_SETTINGS.yellIntervalMs),
    maxYellsPerWake: Math.max(1, Number($<HTMLInputElement>('max').value) || DEFAULT_SETTINGS.maxYellsPerWake),
  };
}

const stage = new CatStage(stageEl, readSettings(), {
  onDismiss: () => {
    note('dismiss (bubble click or double-click on the awake cat)');
    stage.sleep();
  },
  onPhase: (phase) => note(`phase: ${phase}`),
  onLayoutChange: () => drawHitboxes(),
  onReadyToggle: (next) => {
    note(`ready ${next ? 'armed' : 'off'} (in the app, Rust decides when it fires)`);
    stage.setReady(next);
  },
});

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
$('next').addEventListener('click', () => stage.yellNow(reason()));
$('pet').addEventListener('click', () => stage.pet());
for (const id of ['sfw', 'reduce', 'en', 'vi', 'interval', 'max']) {
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
