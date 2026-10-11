import { describe, expect, it } from 'vitest';
import { RenderLoop, type LoopEnv } from '../src/cat/RenderLoop';

/**
 * A fake display with a virtual clock: `run(hz, seconds)` steps vsync by vsync. Timers fire when due;
 * a frame requested before a vsync runs at that vsync, like requestAnimationFrame.
 */
function fakeEnv() {
  let now = 0;
  let nextId = 1;
  const frames = new Map<number, (now: number) => void>();
  const timers = new Map<number, { due: number; cb: () => void }>();
  let hidden = false;
  let onVis: () => void = () => {};
  let rafCallbacks = 0;
  const env: LoopEnv = {
    requestFrame: (cb) => {
      frames.set(nextId, cb);
      return nextId++;
    },
    cancelFrame: (id) => void frames.delete(id),
    setTimer: (cb, ms) => {
      timers.set(nextId, { due: now + ms, cb });
      return nextId++;
    },
    clearTimer: (id) => void timers.delete(id),
    isHidden: () => hidden,
    onVisibilityChange: (cb) => {
      onVis = cb;
      return () => {};
    },
  };
  const fireTimers = (until: number): void => {
    for (;;) {
      const due = [...timers].filter(([, t]) => t.due <= until).sort((a, b) => a[1].due - b[1].due)[0];
      if (!due) return;
      timers.delete(due[0]);
      now = Math.max(now, due[1].due);
      due[1].cb();
    }
  };
  return {
    env,
    run(hz: number, seconds: number) {
      const step = 1000 / hz;
      const end = now + seconds * 1000;
      let vsync = Math.ceil(now / step) * step;
      while (vsync < end) {
        vsync += step;
        fireTimers(vsync);
        now = vsync;
        const due = [...frames];
        frames.clear();
        for (const [, cb] of due) {
          rafCallbacks++;
          cb(now);
        }
      }
    },
    setHidden(h: boolean) {
      hidden = h;
      onVis();
    },
    get pending() {
      return frames.size + timers.size;
    },
    get rafCallbacks() {
      return rafCallbacks;
    },
  };
}

function measure(displayHz: number, cap: number) {
  const d = fakeEnv();
  let ticks = 0;
  const loop = new RenderLoop(() => ticks++, d.env);
  loop.setMaxFps(cap);
  loop.start();
  d.run(displayHz, 2);
  return { fps: ticks / 2, wakeups: d.rafCallbacks / 2 };
}

describe('RenderLoop', () => {
  it('runs at the display rate when the cap is higher', () => {
    expect(measure(60, 60).fps).toBeCloseTo(60, -1);
  });

  it.each([
    [60, 20],
    [144, 60],
    [144, 20],
    [165, 20],
    [165, 60],
    [120, 60],
  ])('caps a %iHz display to %ifps', (hz, cap) => {
    const { fps } = measure(hz, cap);
    expect(fps).toBeGreaterThanOrEqual(cap * 0.9);
    expect(fps).toBeLessThanOrEqual(cap * 1.05);
  });

  it.each([
    [165, 20],
    [144, 60],
  ])('on %iHz capped to %ifps it does not wake on every vsync', (hz, cap) => {
    const { wakeups } = measure(hz, cap);
    expect(wakeups).toBeLessThan(cap * 1.25);
  });

  it('passes real elapsed seconds as dt', () => {
    const d = fakeEnv();
    const dts: number[] = [];
    const loop = new RenderLoop((dt) => dts.push(dt), d.env);
    loop.setMaxFps(20);
    loop.start();
    d.run(60, 1);
    expect(dts[0]).toBe(0);
    for (const dt of dts.slice(1)) expect(dt).toBeCloseTo(0.05, 2);
  });

  it('stops while hidden and resumes when visible', () => {
    const d = fakeEnv();
    let ticks = 0;
    const loop = new RenderLoop(() => ticks++, d.env);
    loop.start();
    d.run(60, 0.5);
    d.setHidden(true);
    expect(loop.active).toBe(false);
    expect(d.pending).toBe(0);
    const before = ticks;
    d.run(60, 1);
    expect(ticks).toBe(before);
    d.setHidden(false);
    expect(loop.active).toBe(true);
    d.run(60, 0.5);
    expect(ticks).toBeGreaterThan(before);
  });

  it('stop() cancels whatever is pending, frame or timer', () => {
    const d = fakeEnv();
    const loop = new RenderLoop(() => {}, d.env);
    loop.setMaxFps(20);
    loop.start();
    expect(d.pending).toBe(1);
    d.run(60, 0.2); // now waiting on a timer between frames
    loop.stop();
    expect(d.pending).toBe(0);
    expect(loop.active).toBe(false);
  });

  it('a tick that stops the loop is not rescheduled', () => {
    const d = fakeEnv();
    let ticks = 0;
    const loop = new RenderLoop(() => {
      ticks++;
      if (ticks === 3) loop.stop();
    }, d.env);
    loop.start();
    d.run(60, 1);
    expect(ticks).toBe(3);
    expect(d.pending).toBe(0);
  });

  it('a tick that stops and restarts the loop leaves exactly one pending request', () => {
    const d = fakeEnv();
    let ticks = 0;
    const loop = new RenderLoop(() => {
      ticks++;
      if (ticks === 2) {
        loop.stop();
        loop.start();
      }
    }, d.env);
    loop.start();
    d.run(60, 0.1);
    expect(d.pending).toBe(1);
  });
});
