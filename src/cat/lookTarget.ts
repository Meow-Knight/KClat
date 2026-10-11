// Cursor offset → head look angles (spec §6.4).

export interface Look {
  lx: number;
  ly: number;
}

/** Where the cat looks without cursor data: towards the bubble on its left. */
export const DEFAULT_LOOK: Readonly<Look> = { lx: -0.35, ly: -0.08 };

const clamp1 = (v: number): number => (v < -1 ? -1 : v > 1 ? 1 : v);

/** dx, dy: logical px from the cat's head on screen to the cursor. Writes into `out` (called at 30Hz). */
export function lookFromOffset(dx: number, dy: number, out: Look): Look {
  out.lx = clamp1(dx / 260) * 0.75;
  out.ly = clamp1(dy / 200) * 0.45;
  return out;
}
