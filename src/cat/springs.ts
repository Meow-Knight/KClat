// Spring physics for every animated parameter (spec §6.1). No keyframes: retargeting mid-motion never pops.

/** Frame steps are clamped so a stalled tab does not explode the integration. */
export const MAX_DT = 1 / 30;

export class Spring {
  vel = 0;
  t: number;
  d: number;
  private readonly baseD: number;

  constructor(
    public v: number,
    readonly k: number,
    d: number,
  ) {
    this.t = v;
    this.d = d;
    this.baseD = d;
  }

  /** Semi-implicit Euler. */
  step(dt: number): void {
    this.vel += (this.k * (this.t - this.v) - this.d * this.vel) * dt;
    this.v += this.vel * dt;
  }

  /** reduceMotion: no overshoot (d = 2·√k). */
  setCriticallyDamped(on: boolean): void {
    this.d = on ? 2 * Math.sqrt(this.k) : this.baseD;
  }
}

export type SpringSpec = readonly [init: number, k: number, d: number];

export class SpringSet<K extends string> {
  readonly p: Readonly<Record<K, Spring>>;
  private readonly list: Spring[];

  constructor(specs: Readonly<Record<K, SpringSpec>>) {
    const p = {} as Record<K, Spring>;
    for (const name of Object.keys(specs) as K[]) {
      const [init, k, d] = specs[name];
      p[name] = new Spring(init, k, d);
    }
    this.p = p;
    this.list = Object.values(p);
  }

  step(dt: number): void {
    const h = Math.min(dt, MAX_DT);
    for (let i = 0; i < this.list.length; i++) this.list[i].step(h);
  }

  setCriticallyDamped(on: boolean): void {
    for (let i = 0; i < this.list.length; i++) this.list[i].setCriticallyDamped(on);
  }
}

/** Vertical jump with gravity; y = 0 is the ground. */
export class Ballistic {
  y = 0;
  v = 0;

  constructor(readonly gravity = 18) {}

  launch(v: number): void {
    this.v = v;
  }

  /** Advances one step. Returns the downward speed if the body hit the ground this step, else 0. */
  step(dt: number): number {
    const h = Math.min(dt, MAX_DT);
    this.v -= this.gravity * h;
    this.y += this.v * h;
    if (this.y > 0) return 0;
    const impact = -this.v;
    this.y = 0;
    this.v = 0;
    return impact > 0 ? impact : 0;
  }
}
