// rAF loop with an FPS cap (spec §9.1: 60fps awake, 20fps asleep) that stops while the page is hidden.
// Between frames it sleeps on a timer instead of skipping vsyncs: on a 165Hz display a 20fps cap would
// otherwise wake the page 165 times a second just to do nothing.

export type Tick = (dt: number, nowMs: number) => void;

export interface LoopEnv {
  requestFrame(cb: (now: number) => void): number;
  cancelFrame(id: number): void;
  setTimer(cb: () => void, ms: number): number;
  clearTimer(id: number): void;
  isHidden(): boolean;
  onVisibilityChange(cb: () => void): () => void;
}

export const browserEnv = (): LoopEnv => ({
  requestFrame: (cb) => requestAnimationFrame(cb),
  cancelFrame: (id) => cancelAnimationFrame(id),
  setTimer: (cb, ms) => window.setTimeout(cb, ms),
  clearTimer: (id) => window.clearTimeout(id),
  isHidden: () => document.hidden,
  onVisibilityChange: (cb) => {
    document.addEventListener('visibilitychange', cb);
    return () => document.removeEventListener('visibilitychange', cb);
  },
});

/** Wake this long before the next frame is due and let rAF align it to vsync. */
const WAKE_EARLY_MS = 4;
/**
 * A frame may arrive this early and still count. Equal to WAKE_EARLY_MS so the vsync right after a timer
 * wake-up is always accepted; the anchor still advances by whole intervals, so the average rate holds.
 */
const SLACK_MS = WAKE_EARLY_MS;

export class RenderLoop {
  private maxFps = 60;
  private frameId: number | null = null;
  private timerId: number | null = null;
  private running = false;
  /** Pacing anchor: advances in whole intervals so the cap averages out on high-refresh displays. */
  private anchor = -1;
  private lastTick = -1;
  private readonly unlisten: () => void;

  constructor(
    private readonly tick: Tick,
    private readonly env: LoopEnv = browserEnv(),
  ) {
    this.unlisten = env.onVisibilityChange(() => this.sync());
  }

  setMaxFps(fps: number): void {
    this.maxFps = fps;
  }

  start(): void {
    this.running = true;
    this.sync();
  }

  stop(): void {
    this.running = false;
    this.sync();
  }

  dispose(): void {
    this.stop();
    this.unlisten();
  }

  get active(): boolean {
    return this.frameId !== null || this.timerId !== null;
  }

  private sync(): void {
    const want = this.running && !this.env.isHidden();
    if (want && !this.active) {
      this.anchor = -1;
      this.lastTick = -1;
      this.schedule(0);
    } else if (!want) {
      if (this.frameId !== null) this.env.cancelFrame(this.frameId);
      if (this.timerId !== null) this.env.clearTimer(this.timerId);
      this.frameId = null;
      this.timerId = null;
    }
  }

  /** Request a frame after roughly `delayMs`; short delays go straight to rAF. */
  private schedule(delayMs: number): void {
    if (delayMs - WAKE_EARLY_MS > 1) {
      this.timerId = this.env.setTimer(this.wake, delayMs - WAKE_EARLY_MS);
    } else {
      this.frameId = this.env.requestFrame(this.frame);
    }
  }

  private readonly wake = (): void => {
    this.timerId = null;
    this.frameId = this.env.requestFrame(this.frame);
  };

  private readonly frame = (now: number): void => {
    this.frameId = null;
    const interval = 1000 / this.maxFps;
    if (this.anchor >= 0) {
      const elapsed = now - this.anchor;
      if (elapsed < interval - SLACK_MS) {
        this.schedule(interval - elapsed);
        return;
      }
      // Drop whole missed intervals instead of bursting to catch up.
      this.anchor = elapsed > interval * 2 ? now : this.anchor + interval;
    } else {
      this.anchor = now;
    }
    const dt = this.lastTick < 0 ? 0 : (now - this.lastTick) / 1000;
    this.lastTick = now;
    this.tick(dt, now);
    // The tick may have stopped the loop, or stopped and restarted it (which already scheduled a frame).
    if (this.running && !this.env.isHidden() && !this.active) this.schedule(this.anchor + interval - now);
  };
}
