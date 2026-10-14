// Settings file (spec §10, docs/specs/settings-i18n-macos.md §2). Rust owns load/save and validation; the
// overlay and the Settings window receive them via the bridge.
import type { PhraseLang } from './phrases/phrases';

/** Platform Language: the tray, the Settings window and dialogs. */
export type UiLanguage = 'en' | 'vi';
/** Klat Language: what the cat says. Auto mixes both languages. */
export type KlatLanguage = PhraseLang | 'auto';

/** The part of the settings the cat itself uses. */
export interface CatSettings {
  sfwMode: boolean;
  klatLanguage: KlatLanguage;
  yellIntervalMs: number;
  maxYellsPerWake: number;
  reduceMotion: boolean;
}

export interface AppSettings extends CatSettings {
  notificationsEnabled: boolean;
  hideWhileSleeping: boolean;
  uiLanguage: UiLanguage;
  staleAfterMs: number;
  /** Cat size in percent (CAT_SCALE range). Rust sizes and zooms the overlay window; the page does not see it. */
  catScale: number;
  startAtLogin: boolean;
  corner: 'bottom-right';
  /** Where the cat was dragged (window top-left, physical px); null = the corner. */
  position: { x: number; y: number } | null;
}

export const DEFAULT_SETTINGS: Readonly<AppSettings> = {
  notificationsEnabled: true,
  hideWhileSleeping: false,
  sfwMode: false,
  uiLanguage: 'en',
  klatLanguage: 'auto',
  yellIntervalMs: 30_000,
  maxYellsPerWake: 10,
  staleAfterMs: 1_800_000,
  reduceMotion: false,
  catScale: 100,
  startAtLogin: false,
  corner: 'bottom-right',
  position: null,
};

/** Cat size slider: percent range and step (Rust clamps to the same range). */
export const CAT_SCALE = { min: 50, max: 200, step: 5 } as const;

/** The phrase languages the cat may use for a Klat Language. */
export function phraseLanguages(klat: KlatLanguage): PhraseLang[] {
  return klat === 'auto' ? ['en', 'vi'] : [klat];
}
