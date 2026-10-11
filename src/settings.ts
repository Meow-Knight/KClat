// Settings file (spec §10). Rust owns load/save; the overlay receives them via the bridge.
import type { PhraseLang } from './phrases/phrases';

/** The part of the settings the cat itself uses. */
export interface CatSettings {
  sfwMode: boolean;
  languages: PhraseLang[];
  yellIntervalMs: number;
  maxYellsPerWake: number;
  reduceMotion: boolean;
}

export interface AppSettings extends CatSettings {
  notificationsEnabled: boolean;
  hideWhileSleeping: boolean;
  staleAfterMs: number;
  startAtLogin: boolean;
  corner: 'bottom-right';
}

export const DEFAULT_SETTINGS: Readonly<AppSettings> = {
  notificationsEnabled: true,
  hideWhileSleeping: false,
  sfwMode: false,
  languages: ['en', 'vi'],
  yellIntervalMs: 30_000,
  maxYellsPerWake: 10,
  staleAfterMs: 1_800_000,
  reduceMotion: false,
  startAtLogin: false,
  corner: 'bottom-right',
};
