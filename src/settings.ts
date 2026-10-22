// Settings file (spec §10, docs/specs/settings-i18n-macos.md §2). Rust owns load/save and validation; the
// overlay and the Settings window receive them via the bridge.
// The types are generated from Rust (bridge/generated.ts, pure types: any layer may import them).
import type { Character, KlatLanguage, Settings } from './bridge/generated';
import { PHRASE_LANGS, type PhraseLang } from './phrases/phrases';

export type { KlatLanguage, UiLanguage } from './bridge/generated';
/** Who sits in the corner (src/characters). */
export type CharacterId = Character;
/** Every character once; a character added in Rust and missing here fails to compile. */
const CHARACTER_SET: Record<CharacterId, true> = { cat: true, snake: true, dragon: true, capybara: true };
export const CHARACTERS = Object.keys(CHARACTER_SET) as readonly CharacterId[];

/** The settings file, as Rust sends it (`Settings` in crates/klat-core/src/settings.rs). */
export type AppSettings = Settings;

/** The part of the settings the cat itself uses. */
export type CatSettings = Pick<
  AppSettings,
  'character' | 'sfwMode' | 'klatLanguage' | 'yellIntervalMs' | 'maxYellsPerWake' | 'reduceMotion'
>;

export const DEFAULT_SETTINGS: Readonly<AppSettings> = {
  character: 'cat',
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

/** Cat size slider: percent range and step (Rust clamps to the same range). Step 1 so a drag moves smoothly. */
export const CAT_SCALE = { min: 50, max: 200, step: 1 } as const;

/** The phrase languages the cat may use for a Klat Language: Auto mixes them all. */
export function phraseLanguages(klat: KlatLanguage): PhraseLang[] {
  return klat === 'auto' ? [...PHRASE_LANGS] : [klat];
}
