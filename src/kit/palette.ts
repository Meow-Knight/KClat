// Colours every Klat character shares (spec §5.2). A character's own colours are a CharacterPalette.

/** Outlines, brows, whiskers, closed eyes, the "ω" mouth. */
export const INK = 0x2b2d42;
export const PUPIL = 0x1b1b2f;
/** Inside of the open mouth. */
export const MOUTH = 0x5a2a3a;
/** Eye highlights. */
export const HIGHLIGHT = 0xffffff;
/** Stars and the impact burst. */
export const GOLD = 0xf5b83d;
export const ANGER_RED = '#e5383b';
export const SHADOW_OPACITY = 0.13;

/** A character's colours, by role (.claude/skills/klat-character/DNA.md §2.1). */
export interface CharacterPalette {
  /** Main colour: head, body; also the bump after a knockout. */
  main: number;
  /** Darker shade of main: markings, the Zzz letters. */
  shade: number;
  /** Light parts: muzzle, belly, paws. */
  light: number;
  /** Nose, inner ears, tongue. */
  accent: number;
  iris: number;
  /** Cheek blush (drawn without an outline). */
  blush: number;
}
