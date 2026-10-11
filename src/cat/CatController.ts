// Animation state machine (spec §6.2): turns wake/sleep/pet/look into spring targets and timed sequences.
// Pure logic: no Three.js, no DOM. The rig reads `pose` every frame.
import type { WakeReason } from '../phrases/PhrasePicker';
import { Ballistic, SpringSet, type SpringSpec } from './springs';
import { DEFAULT_LOOK, lookFromOffset, type Look } from './lookTarget';

export type CatState = 'sleep' | 'active';

export const SPRINGS = {
  sleepy: [1, 30, 9],
  eye: [0.02, 420, 26],
  ear: [0.3, 200, 12],
  earTw: [0, 220, 10],
  mouth: [0, 260, 20],
  puff: [0, 180, 9],
  squash: [0, 260, 12],
  lx: [0, 70, 11],
  ly: [0, 70, 11],
  yawn: [0, 60, 10],
  happy: [0, 140, 14],
  grumpy: [0.3, 120, 14],
  tailUp: [0, 60, 8],
} as const satisfies Record<string, SpringSpec>;

export type ParamName = keyof typeof SPRINGS;

/** Everything the rig needs for one frame. Mutated in place by `update()`. */
export type CatPose = Record<ParamName, number> & {
  jumpY: number;
  tapping: boolean;
  purring: boolean;
  /** Show the floating Zzz: asleep with the sleep sequence finished, and motion allowed. */
  zzz: boolean;
};

export interface YellRequest {
  first: boolean;
  /** 1-based count within the current wake. */
  yellIndex: number;
  reason: WakeReason;
}

/** Side effects the controller asks for; the stage maps them onto the bubble. */
export interface CatVoice {
  say(req: YellRequest): void;
  hush(): void;
}

export interface ControllerOptions {
  yellIntervalMs: number;
  maxYellsPerWake: number;
  reduceMotion: boolean;
}

/** 'awake' fires when wake() starts; 'asleep' fires when the sleep sequence has finished. */
export type CatPhase = 'awake' | 'asleep';

const EYE_WIDE = 1.25;
const EYE_GRUMPY = 0.62;
const EYE_ASLEEP = 0.02;
const GRAVITY = 18;
const LANDING_SQUASH_MIN = 1.2;

export class CatController {
  readonly pose: CatPose;
  private readonly springs = new SpringSet(SPRINGS);
  private readonly jump = new Ballistic(GRAVITY);
  private readonly look: Look = { lx: 0, ly: 0 };

  private state: CatState = 'sleep';
  private reason: WakeReason = 'done';
  private eyeBase = EYE_ASLEEP;
  private yellCount = 0;
  private tapping = false;
  private purrLeft = 0;
  /** Asleep and the sleep sequence has finished (true at start: the cat boots asleep). */
  private settledAsleep = true;

  private seqTimers: ReturnType<typeof setTimeout>[] = [];
  private yellTimer: ReturnType<typeof setTimeout> | undefined;
  private settleTimer: ReturnType<typeof setTimeout> | undefined;
  private blinkTimer: ReturnType<typeof setTimeout> | undefined;
  private twitchTimer: ReturnType<typeof setTimeout> | undefined;
  private opts: ControllerOptions;

  constructor(
    private readonly voice: CatVoice,
    opts: ControllerOptions,
    private readonly onPhase: (phase: CatPhase) => void = () => {},
    private readonly random: () => number = Math.random,
  ) {
    this.opts = { ...opts };
    this.springs.setCriticallyDamped(opts.reduceMotion);
    const p = this.springs.p;
    this.pose = {
      sleepy: p.sleepy.v, eye: p.eye.v, ear: p.ear.v, earTw: p.earTw.v, mouth: p.mouth.v, puff: p.puff.v,
      squash: p.squash.v, lx: p.lx.v, ly: p.ly.v, yawn: p.yawn.v, happy: p.happy.v, grumpy: p.grumpy.v,
      tailUp: p.tailUp.v, jumpY: 0, tapping: false, purring: false, zzz: !opts.reduceMotion,
    };
    this.scheduleBlink();
    this.scheduleTwitch();
  }

  get current(): CatState {
    return this.state;
  }

  /** Read-only view of a spring, for tests and debugging. */
  spring(name: ParamName): { readonly v: number; readonly t: number } {
    return this.springs.p[name];
  }

  setOptions(opts: ControllerOptions): void {
    this.opts = { ...opts };
    this.springs.setCriticallyDamped(opts.reduceMotion);
    if (opts.reduceMotion) this.purrLeft = 0;
  }

  wake(reason: WakeReason = 'done'): void {
    if (this.state === 'active') return;
    this.clearSequence();
    this.state = 'active';
    this.settledAsleep = false;
    this.reason = reason;
    this.yellCount = 0;
    this.onPhase('awake');

    const p = this.springs.p;
    const motion = !this.opts.reduceMotion;
    p.sleepy.t = 0;
    p.yawn.t = 0;
    p.ear.t = 1;
    p.ear.vel += 9;
    this.eyeBase = EYE_WIDE;
    p.eye.t = EYE_WIDE;
    p.eye.vel += 14;
    this.setLookTarget(DEFAULT_LOOK.lx, DEFAULT_LOOK.ly);
    if (motion) p.squash.t = 0.22; // crouch before the jump

    this.later(150, () => {
      p.squash.t = 0;
      if (!motion) return;
      p.squash.vel -= 4;
      this.jump.launch(4.4);
      p.puff.t = 1;
      p.puff.vel += 7;
    });
    this.later(240, () => this.yell(true));
    this.later(750, () => (p.puff.t = 0));
    this.later(1500, () => (this.tapping = true));
    this.scheduleNextYell();
  }

  sleep(): void {
    if (this.state !== 'active') return;
    this.clearSequence();
    this.state = 'sleep';
    this.tapping = false;
    this.purrLeft = 0;
    this.voice.hush();

    const p = this.springs.p;
    p.happy.t = 0;
    p.puff.t = 0;
    p.mouth.t = 0.85; // yawn
    p.yawn.t = 1;
    this.eyeBase = 0.1;
    p.eye.t = 0.1;
    p.tailUp.t = 0;

    this.later(1000, () => {
      p.mouth.t = 0;
      p.yawn.t = 0;
      p.sleepy.t = 1;
      p.ear.t = 0.3;
      this.eyeBase = EYE_ASLEEP;
      p.eye.t = EYE_ASLEEP;
      p.grumpy.t = 0.3;
      this.settledAsleep = true;
      this.onPhase('asleep');
    });
  }

  /**
   * Yell right now (another session became pending, or "Next phrase" in the dev page).
   * Wakes the cat if it is asleep. Restarts the ignore timer so yells never come back to back.
   */
  yellNow(reason?: WakeReason): void {
    if (this.state !== 'active') {
      this.wake(reason);
      return;
    }
    if (reason) this.reason = reason;
    this.yell(false);
    this.scheduleNextYell();
  }

  pet(): void {
    const p = this.springs.p;
    if (this.state === 'active') {
      p.happy.t = 1;
      if (!this.opts.reduceMotion) this.purrLeft = 1.3;
      this.later(1300, () => (p.happy.t = 0));
    } else {
      p.eye.t = 0.4; // peek
      p.earTw.vel += 9;
      this.later(900, () => {
        if (this.state === 'sleep') p.eye.t = this.eyeBase;
      });
    }
  }

  /** dx, dy: logical px from the head on screen to the cursor. */
  setLook(dx: number, dy: number): void {
    lookFromOffset(dx, dy, this.look);
    this.setLookTarget(this.look.lx, this.look.ly);
  }

  /** No cursor data (cursor on another screen): look at the bubble. */
  lookDefault(): void {
    this.setLookTarget(DEFAULT_LOOK.lx, DEFAULT_LOOK.ly);
  }

  /** Advance physics by dt seconds. Allocation-free: runs every frame. */
  update(dt: number): void {
    const p = this.springs.p;
    this.springs.step(dt);
    const impact = this.jump.step(dt);
    if (impact > LANDING_SQUASH_MIN && !this.opts.reduceMotion) p.squash.vel += impact * 1.3;
    if (this.purrLeft > 0) this.purrLeft = Math.max(0, this.purrLeft - dt);

    const o = this.pose;
    o.sleepy = p.sleepy.v;
    o.eye = p.eye.v;
    o.ear = p.ear.v;
    o.earTw = p.earTw.v;
    o.mouth = p.mouth.v;
    o.puff = p.puff.v;
    o.squash = p.squash.v;
    o.lx = p.lx.v;
    o.ly = p.ly.v;
    o.yawn = p.yawn.v;
    o.happy = p.happy.v;
    o.grumpy = p.grumpy.v;
    o.tailUp = p.tailUp.v;
    o.jumpY = this.jump.y;
    o.tapping = this.tapping;
    o.purring = this.purrLeft > 0;
    o.zzz = this.settledAsleep && !this.opts.reduceMotion;
  }

  dispose(): void {
    this.clearSequence();
    clearTimeout(this.blinkTimer);
    clearTimeout(this.twitchTimer);
  }

  private yell(first: boolean): void {
    if (this.state !== 'active') return;
    this.yellCount++;
    this.voice.say({ first, yellIndex: this.yellCount, reason: this.reason });

    const p = this.springs.p;
    p.mouth.t = 1;
    p.grumpy.t = 0;
    p.eye.t = EYE_WIDE;
    p.eye.vel += 6;
    p.ear.vel += 6;
    p.tailUp.t = 1;
    if (!first && !this.opts.reduceMotion) {
      this.jump.launch(2.6);
      p.squash.vel += 2;
    }

    // Settle into the grumpy squint. A newer yell replaces this so it cannot shut the new yell's mouth early.
    clearTimeout(this.settleTimer);
    this.settleTimer = setTimeout(() => {
      if (this.state !== 'active') return;
      p.mouth.t = 0;
      p.grumpy.t = 1;
      this.eyeBase = EYE_GRUMPY;
      p.eye.t = EYE_GRUMPY;
      p.tailUp.t = 0.4;
    }, 1100);
  }

  /** The ignore timer: yell again every yellIntervalMs until maxYellsPerWake, then sit grumpy and silent. */
  private scheduleNextYell(): void {
    clearTimeout(this.yellTimer);
    this.yellTimer = setTimeout(() => {
      if (this.state !== 'active' || this.yellCount >= this.opts.maxYellsPerWake) return;
      this.yell(false);
      this.scheduleNextYell();
    }, this.opts.yellIntervalMs);
  }

  private scheduleBlink(): void {
    this.blinkTimer = setTimeout(
      () => {
        const p = this.springs.p;
        if (this.state === 'active' && p.happy.t < 0.5 && p.mouth.t < 0.5) {
          p.eye.t = 0.03;
          setTimeout(() => {
            if (this.state === 'active' && p.eye.t === 0.03) p.eye.t = this.eyeBase;
          }, 110);
        }
        this.scheduleBlink();
      },
      1600 + this.random() * 3400,
    );
  }

  private scheduleTwitch(): void {
    this.twitchTimer = setTimeout(
      () => {
        this.springs.p.earTw.vel += (this.random() < 0.5 ? -1 : 1) * 7;
        this.scheduleTwitch();
      },
      2000 + this.random() * 4500,
    );
  }

  private setLookTarget(lx: number, ly: number): void {
    this.springs.p.lx.t = lx;
    this.springs.p.ly.t = ly;
  }

  private later(ms: number, fn: () => void): void {
    this.seqTimers.push(setTimeout(fn, ms));
  }

  private clearSequence(): void {
    for (const t of this.seqTimers) clearTimeout(t);
    this.seqTimers = [];
    clearTimeout(this.yellTimer);
    clearTimeout(this.settleTimer);
    this.yellTimer = undefined;
    this.settleTimer = undefined;
  }
}
