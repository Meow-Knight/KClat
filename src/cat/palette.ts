// The cat's own colours (spec §5.2). Ink, pupil and mouth are shared by every character: see kit/palette.
import type { CharacterPalette } from '../kit/palette';

export const FUR = 0x9aa6bd;
export const FUR2 = 0x6b7690;
export const CREAM = 0xf6f0e6;
export const PINK = 0xf29bb0;
export const BLUSH = 0xf7a9bc;
export const AMBER = 0xf5b83d;

export const CAT_PALETTE: CharacterPalette = { main: FUR, shade: FUR2, light: CREAM, accent: PINK, iris: AMBER, blush: BLUSH };
