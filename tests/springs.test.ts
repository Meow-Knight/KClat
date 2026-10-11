import { describe, expect, it } from 'vitest';
import { Ballistic, MAX_DT, Spring, SpringSet } from '../src/cat/springs';
import { SPRINGS } from '../src/cat/CatController';

const run = (s: { step(dt: number): void }, seconds: number, dt = 1 / 60): void => {
  for (let t = 0; t < seconds; t += dt) s.step(dt);
};

describe('Spring', () => {
  it.each(Object.entries(SPRINGS))('%s converges to its target', (_name, [init, k, d]) => {
    const s = new Spring(init, k, d);
    s.t = init + 1;
    run(s, 4);
    expect(s.v).toBeCloseTo(init + 1, 3);
    expect(Math.abs(s.vel)).toBeLessThan(1e-3);
  });

  it('an impulse moves the value and then settles back', () => {
    const s = new Spring(0, 220, 10);
    s.vel += 7;
    s.step(1 / 60);
    expect(s.v).toBeGreaterThan(0);
    run(s, 4);
    expect(s.v).toBeCloseTo(0, 3);
  });

  it('critically damped never overshoots', () => {
    const s = new Spring(0, 260, 12);
    s.setCriticallyDamped(true);
    s.t = 1;
    let max = 0;
    for (let i = 0; i < 600; i++) {
      s.step(1 / 60);
      max = Math.max(max, s.v);
    }
    expect(max).toBeLessThanOrEqual(1 + 1e-9);
    expect(s.v).toBeCloseTo(1, 3);
  });

  it('turning critical damping off restores the original damping', () => {
    const s = new Spring(0, 260, 12);
    s.setCriticallyDamped(true);
    s.setCriticallyDamped(false);
    expect(s.d).toBe(12);
  });
});

describe('SpringSet', () => {
  it('clamps large steps to MAX_DT', () => {
    const set = new SpringSet({ a: [0, 100, 10] });
    const ref = new Spring(0, 100, 10);
    set.p.a.t = 1;
    ref.t = 1;
    set.step(1);
    ref.step(MAX_DT);
    expect(set.p.a.v).toBe(ref.v);
  });
});

describe('Ballistic', () => {
  it('rises, falls and reports the landing speed once', () => {
    const b = new Ballistic(18);
    b.launch(4.4);
    let peak = 0;
    const impacts: number[] = [];
    for (let i = 0; i < 120; i++) {
      const hit = b.step(1 / 60);
      peak = Math.max(peak, b.y);
      if (hit > 0) impacts.push(hit);
    }
    expect(peak).toBeCloseTo((4.4 * 4.4) / (2 * 18), 1);
    expect(b.y).toBe(0);
    expect(impacts.length).toBeGreaterThanOrEqual(1);
    expect(impacts[0]).toBeGreaterThan(4);
    // Resting on the ground produces only tiny gravity "impacts", below the squash threshold.
    expect(impacts.slice(1).every((v) => v < 1.2)).toBe(true);
  });
});
