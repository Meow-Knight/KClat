// Fixed poses for pictures of a character: the dev sheet (characters.html) and the Settings previews.
// Each one puts every part in a known state, at a fixed time, so a picture is the same on every run.
import { CatController, type CatPose } from '../cat/CatController';

/** Seconds: the time every picture is drawn at (breathing, sway, aura flicker). */
export const PICTURE_T = 1.234;

let rest: CatPose | undefined;

/** The controller's resting pose (asleep) with `p` on top. */
export function poseWith(p: Partial<CatPose>): CatPose {
  if (!rest) {
    const c = new CatController({ say() {}, hush() {} }, { yellIntervalMs: 1e9, maxYellsPerWake: 1, reduceMotion: false });
    rest = { ...c.pose };
    c.dispose();
  }
  return { ...rest, ...p };
}

const AWAKE: Partial<CatPose> = { sleepy: 0, eye: 0.62, grumpy: 1, ear: 1, zzz: false, auraFlicker: true };

export const ASLEEP_POSE: Partial<CatPose> = { zzz: true };
/** Awake and grumpy, without the aura: what the Settings previews show for the picked character. */
export const GRUMPY_POSE: Partial<CatPose> = { ...AWAKE, lx: -0.15, tailUp: 0.4 };

/** The dev sheet, in order: asleep, awake, yelling, petted, yawning, poked asleep, then the four knockouts. */
export const SHEET_POSES: readonly Partial<CatPose>[] = [
  ASLEEP_POSE,
  { ...AWAKE, lx: 0.3, ly: -0.1, tapping: true, tailUp: 0.4, aura: 1 },
  { ...AWAKE, eye: 1.25, mouth: 1, grumpy: 0, puff: 0.5, aura: 1.3, jumpY: 0.3, squash: -0.1, earTw: 0.5 },
  { ...AWAKE, happy: 1, purring: true },
  { sleepy: 0.3, yawn: 1, mouth: 0.85, eye: 0.1 },
  { zzz: true, annoyed: 1, swish: 1, auraFlicker: true },
  { ...AWAKE, bonkKind: 0, bonkT: 0.1 },
  { ...AWAKE, bonkKind: 1, bonkT: 0.25 },
  { sleepy: 0.5, dizzy: 1, bump: 1, bonkKind: 2, bonkT: 0.45, auraFlicker: true },
  { sleepy: 0.8, dizzy: 1, bump: 1, bonkKind: 3, bonkT: 0.6, auraFlicker: true },
];
