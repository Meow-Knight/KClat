import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CatController, type CatPhase, type ControllerOptions, type YellRequest } from '../src/cat/CatController';
import { DEFAULT_LOOK } from '../src/cat/lookTarget';

const OPTS: ControllerOptions = { yellIntervalMs: 30_000, maxYellsPerWake: 10, reduceMotion: false };

function setup(opts: Partial<ControllerOptions> = {}) {
  const says: YellRequest[] = [];
  const hush = vi.fn();
  const phases: CatPhase[] = [];
  const c = new CatController({ say: (r) => says.push(r), hush }, { ...OPTS, ...opts }, (p) => phases.push(p), () => 0.5);
  /** Advance timers and physics together, like the render loop would. */
  const advance = (ms: number): void => {
    for (let t = 0; t < ms; t += 16) {
      vi.advanceTimersByTime(16);
      c.update(0.016);
    }
  };
  return { c, says, hush, phases, advance };
}

describe('CatController', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('starts asleep', () => {
    const { c } = setup();
    expect(c.current).toBe('sleep');
    expect(c.pose.sleepy).toBe(1);
  });

  it('wake() runs the sequence: first yell at 240ms is the wake yell', () => {
    const { c, says, phases, advance } = setup();
    c.wake('permission');
    expect(c.current).toBe('active');
    expect(phases).toEqual(['awake']);
    advance(200);
    expect(says).toHaveLength(0);
    advance(100);
    expect(says).toEqual([{ first: true, yellIndex: 1, reason: 'permission' }]);
    expect(c.spring('mouth').t).toBe(1);
    advance(1500);
    expect(c.spring('mouth').t).toBe(0);
    expect(c.spring('grumpy').t).toBe(1);
    expect(c.pose.tapping).toBe(true);
    expect(c.pose.sleepy).toBeLessThan(0.05);
  });

  it('wake() jumps (jumpY > 0) unless reduceMotion', () => {
    const a = setup();
    a.c.wake();
    let peak = 0;
    for (let i = 0; i < 40; i++) {
      a.advance(16);
      peak = Math.max(peak, a.c.pose.jumpY);
    }
    expect(peak).toBeGreaterThan(0.3);

    const b = setup({ reduceMotion: true });
    b.c.wake();
    let peakB = 0;
    let squashB = 0;
    for (let i = 0; i < 100; i++) {
      b.advance(16);
      peakB = Math.max(peakB, b.c.pose.jumpY);
      squashB = Math.max(squashB, Math.abs(b.c.pose.squash), b.c.pose.puff);
    }
    expect(peakB).toBe(0);
    expect(squashB).toBe(0);
  });

  it('wake() while ACTIVE does nothing', () => {
    const { c, says, phases, advance } = setup();
    c.wake();
    advance(500);
    c.wake();
    advance(500);
    expect(phases).toEqual(['awake']);
    expect(says).toHaveLength(1);
  });

  it('yells again every yellIntervalMs and stops at maxYellsPerWake', () => {
    const { c, says } = setup({ yellIntervalMs: 1000, maxYellsPerWake: 4 });
    c.wake();
    vi.advanceTimersByTime(60_000);
    expect(says.map((s) => s.yellIndex)).toEqual([1, 2, 3, 4]);
    expect(says.slice(1).every((s) => !s.first)).toBe(true);
    expect(vi.getTimerCount()).toBe(2); // only blink and ear twitch remain
  });

  it('sleep() cancels every pending timer and hushes the bubble', () => {
    const { c, says, hush } = setup({ yellIntervalMs: 1000 });
    c.wake();
    vi.advanceTimersByTime(100); // before the first yell
    c.sleep();
    expect(hush).toHaveBeenCalledOnce();
    vi.advanceTimersByTime(60_000);
    expect(says).toHaveLength(0);
    expect(c.current).toBe('sleep');
    expect(vi.getTimerCount()).toBe(2);
  });

  it('sleep() yawns, then curls up and reports asleep after 1s', () => {
    const { c, phases, advance } = setup();
    c.wake();
    advance(2000);
    c.sleep();
    expect(c.spring('yawn').t).toBe(1);
    expect(c.spring('mouth').t).toBe(0.85);
    advance(900);
    expect(phases).toEqual(['awake']);
    advance(200);
    expect(phases).toEqual(['awake', 'asleep']);
    expect(c.spring('sleepy').t).toBe(1);
    expect(c.spring('mouth').t).toBe(0);
    expect(c.pose.tapping).toBe(false);
  });

  it('Zzz show only while settled asleep, and never with reduceMotion', () => {
    const { c, advance } = setup();
    c.update(0.016);
    expect(c.pose.zzz).toBe(true);
    c.wake();
    advance(32);
    expect(c.pose.zzz).toBe(false);
    advance(2000);
    c.sleep();
    advance(500); // yawning
    expect(c.pose.zzz).toBe(false);
    advance(700);
    expect(c.pose.zzz).toBe(true);
    c.setOptions({ ...OPTS, reduceMotion: true });
    advance(32);
    expect(c.pose.zzz).toBe(false);
  });

  it('sleep() while asleep does nothing', () => {
    const { c, hush, phases } = setup();
    c.sleep();
    expect(hush).not.toHaveBeenCalled();
    expect(phases).toEqual([]);
  });

  it('sleep() right after a pet does not leave the happy face on', () => {
    const { c, advance } = setup();
    c.wake();
    advance(2000);
    c.pet();
    c.sleep();
    advance(3000);
    expect(c.spring('happy').t).toBe(0);
  });

  it('yellNow() yells immediately and restarts the ignore timer', () => {
    const { c, says } = setup({ yellIntervalMs: 1000 });
    c.wake('done');
    vi.advanceTimersByTime(900);
    expect(says).toHaveLength(1);
    c.yellNow('input');
    expect(says[1]).toEqual({ first: false, yellIndex: 2, reason: 'input' });
    vi.advanceTimersByTime(500); // the old 1000ms tick would have fired here
    expect(says).toHaveLength(2);
    vi.advanceTimersByTime(600);
    expect(says).toHaveLength(3);
  });

  it('yellNow() while asleep wakes the cat', () => {
    const { c, says } = setup();
    c.yellNow('permission');
    expect(c.current).toBe('active');
    vi.advanceTimersByTime(300);
    expect(says[0]).toMatchObject({ first: true, reason: 'permission' });
  });

  it('pet() while ACTIVE shows the happy face for 1.3s and purrs', () => {
    const { c, advance } = setup();
    c.wake();
    advance(2000);
    c.pet();
    advance(100);
    expect(c.spring('happy').t).toBe(1);
    expect(c.pose.purring).toBe(true);
    advance(1300);
    expect(c.spring('happy').t).toBe(0);
    expect(c.pose.purring).toBe(false);
  });

  it('pet() while asleep: eyes stay shut, angry mark and tail swish, then it calms down', () => {
    const { c, advance } = setup();
    c.pet();
    advance(300);
    expect(c.spring('eye').t).toBe(0.02);
    expect(c.pose.eye).toBeLessThan(0.18); // drawn closed
    expect(c.pose.annoyed).toBeGreaterThan(0.5);
    expect(c.pose.swish).toBeGreaterThan(0.3);
    expect(c.spring('grumpy').t).toBe(1);
    advance(800);
    c.pet(); // poking again keeps it annoyed for another full stretch
    advance(1000);
    expect(c.spring('annoyed').t).toBe(1);
    advance(600);
    expect(c.spring('annoyed').t).toBe(0);
    expect(c.spring('swish').t).toBe(0);
    expect(c.spring('grumpy').t).toBe(0.3);
    expect(c.current).toBe('sleep');
  });

  it('pet() while asleep under reduceMotion shows the mark without the swish', () => {
    const { c, advance } = setup({ reduceMotion: true });
    c.pet();
    advance(300);
    expect(c.pose.annoyed).toBeGreaterThan(0.5);
    expect(c.spring('swish').t).toBe(0);
  });

  it('waking clears the angry mark', () => {
    const { c, advance } = setup();
    c.pet();
    advance(100);
    c.wake();
    expect(c.spring('annoyed').t).toBe(0);
    expect(c.spring('swish').t).toBe(0);
  });

  it('setLook maps cursor offsets; lookDefault faces the bubble', () => {
    const { c } = setup();
    c.setLook(260, -400);
    expect(c.spring('lx').t).toBeCloseTo(0.75);
    expect(c.spring('ly').t).toBeCloseTo(-0.45);
    c.lookDefault();
    expect(c.spring('lx').t).toBe(DEFAULT_LOOK.lx);
    expect(c.spring('ly').t).toBe(DEFAULT_LOOK.ly);
  });

  it('the aura burns while awake, flares on each yell, and goes out on sleep', () => {
    const { c, advance } = setup({ yellIntervalMs: 1000 });
    expect(c.spring('aura').t).toBe(0);
    c.wake();
    expect(c.spring('aura').t).toBe(1);
    advance(2000);
    const steady = c.pose.aura;
    expect(steady).toBeGreaterThan(0.8);
    c.yellNow();
    advance(120);
    expect(c.pose.aura).toBeGreaterThan(steady + 0.05); // flare
    c.sleep();
    expect(c.spring('aura').t).toBe(0);
    advance(3000);
    expect(c.pose.aura).toBeLessThan(0.02);
  });

  it('reduceMotion keeps the aura but stops its flicker and flares', () => {
    const { c, advance } = setup({ reduceMotion: true });
    c.wake();
    advance(2000);
    expect(c.pose.auraFlicker).toBe(false);
    let peak = 0;
    c.yellNow();
    for (let i = 0; i < 30; i++) {
      advance(16);
      peak = Math.max(peak, c.pose.aura);
    }
    expect(peak).toBeLessThanOrEqual(1.0001);
    expect(c.pose.aura).toBeGreaterThan(0.9);
  });

  it('Ready while asleep: a quick peek, one ear up until Ready ends', () => {
    const { c, advance } = setup();
    c.setReady(true);
    expect(c.isReady).toBe(true);
    expect(c.spring('eye').t).toBe(0.4);
    expect(c.spring('ear').t).toBeCloseTo(0.62);
    advance(800);
    expect(c.spring('eye').t).toBe(0.02); // eyes closed again, ear still up
    expect(c.spring('ear').t).toBeCloseTo(0.62);
    c.setReady(false);
    expect(c.spring('ear').t).toBeCloseTo(0.3);
  });

  it('Ready survives a wake and keeps the ear up after the next sleep', () => {
    const { c, advance } = setup();
    c.setReady(true);
    c.wake();
    expect(c.spring('ear').t).toBe(1);
    advance(2000);
    c.sleep();
    advance(1200);
    expect(c.spring('ear').t).toBeCloseTo(0.62);
  });

  it('dispose() clears every timer', () => {
    const { c } = setup();
    c.wake();
    c.dispose();
    expect(vi.getTimerCount()).toBe(0);
  });
});
