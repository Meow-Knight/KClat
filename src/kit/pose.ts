// What a rig is given every frame (the contract between the brain, cat/CatController.ts, and every character's
// body). Every rig draws from these names; the controller drives each number with a spring (SPRINGS there must
// cover exactly POSE_PARAMS).

/** The pose numbers, each one eased by a spring in the controller. */
export const POSE_PARAMS = [
  'sleepy',
  'eye',
  'ear',
  'earTw',
  'mouth',
  'puff',
  'squash',
  'lx',
  'ly',
  'yawn',
  'happy',
  'grumpy',
  'tailUp',
  /** Super Saiyan aura: 0 = off, 1 = burning; impulses flare it past 1 on each yell. */
  'aura',
  /** Poked while asleep: the angry mark (0 = hidden, 1 = shown; impulses make it pop). */
  'annoyed',
  /** Poked while asleep: extra tail swish on top of the sleepy sway. */
  'swish',
  /** Knocked out: X eyes and stars circling the head (0 = off, 1 = seeing stars). */
  'dizzy',
  /** Knocked out: the bump on the head (impulse makes it pop). */
  'bump',
] as const;
export type PoseParam = (typeof POSE_PARAMS)[number];

/** Everything the rig needs for one frame. Mutated in place by the controller's `update()`. */
export type CatPose = Record<PoseParam, number> & {
  jumpY: number;
  tapping: boolean;
  purring: boolean;
  /** Show the floating Zzz: asleep with the sleep sequence finished, and motion allowed. */
  zzz: boolean;
  /** Animate the aura flames (off under reduceMotion: a steady glow instead). */
  auraFlicker: boolean;
  /** The object thrown at the cat (index into KNOCKOUT_OBJECTS), or -1 when nothing is flying. */
  bonkKind: number;
  /** Seconds since the throw started; the rig draws the flight and the bounce from it. */
  bonkT: number;
};

/** What can fly at the cat's head, in the order the rig builds them. */
export const KNOCKOUT_OBJECTS = ['stick', 'slipper', 'durian', 'pan'] as const;
export type KnockoutObject = (typeof KNOCKOUT_OBJECTS)[number];

/** Knockout timeline, in ms from the throw. The rig uses KNOCKOUT_IMPACT_S for the hit point. */
export const KNOCKOUT_IMPACT_MS = 380;
export const KNOCKOUT_IMPACT_S = KNOCKOUT_IMPACT_MS / 1000;
/** After the hit the object bounces away; it is hidden once it has left the frame. */
export const KNOCKOUT_FLIGHT_END_S = KNOCKOUT_IMPACT_S + 1.1;
