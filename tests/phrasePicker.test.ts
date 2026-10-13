import { describe, expect, it } from 'vitest';
import { PHRASES, type Phrase } from '../src/phrases/phrases';
import { PhrasePicker, type PickRequest, type PickerOptions } from '../src/phrases/PhrasePicker';

const ALL: PickerOptions = { sfwMode: false, languages: ['en', 'vi'] };
const EN: PickerOptions = { sfwMode: false, languages: ['en'] };
const VI: PickerOptions = { sfwMode: false, languages: ['vi'] };
const FIRST: PickRequest = { first: true, yellIndex: 1, reason: 'done' };
const VI_WAKE = 'Dậy dậy dậy, dầu nhớt đây!!!';

/** Deterministic PRNG (mulberry32) so failures reproduce. */
function seeded(seed: number): () => number {
  let a = seed;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const later = (yellIndex: number, reason: PickRequest['reason'] = 'done'): PickRequest => ({ first: false, yellIndex, reason });

describe('PhrasePicker', () => {
  it('in English the first yell is always the English wake phrase', () => {
    const picker = new PhrasePicker(PHRASES, EN, seeded(1));
    for (let i = 0; i < 5; i++) {
      expect(picker.pick({ first: true, yellIndex: 1, reason: 'permission' }).text).toBe('Wake up, B**CH!!!');
      picker.pick(later(2));
    }
    const sfw = new PhrasePicker(PHRASES, { ...EN, sfwMode: true });
    expect(sfw.pick(FIRST).text).toBe('Wake up, human!!!');
  });

  it('in Vietnamese the first yell is the Vietnamese wake phrase, SFW or not', () => {
    for (const sfwMode of [false, true]) {
      const picker = new PhrasePicker(PHRASES, { ...VI, sfwMode }, seeded(2));
      for (let i = 0; i < 5; i++) expect(picker.pick(FIRST).text).toBe(VI_WAKE);
    }
  });

  it('with both languages (Auto) the first yell is one of the two wake phrases', () => {
    for (const [sfwMode, english] of [
      [false, 'Wake up, B**CH!!!'],
      [true, 'Wake up, human!!!'],
    ] as const) {
      const seen = new Set<string>();
      for (let seed = 1; seed <= 40; seed++) {
        seen.add(new PhrasePicker(PHRASES, { ...ALL, sfwMode }, seeded(seed)).pick(FIRST).text);
      }
      expect([...seen].sort()).toEqual([english, VI_WAKE].sort());
    }
  });

  it('the first yell still happens when no language is chosen', () => {
    const picker = new PhrasePicker(PHRASES, { sfwMode: false, languages: [] });
    expect(picker.pick(FIRST).tags).toContain('wake');
  });

  it('does not repeat a phrase until the bag is empty', () => {
    const picker = new PhrasePicker(PHRASES, ALL, seeded(7));
    const req = later(5, 'permission');
    const size = picker.pool(req).length;
    const drawn = Array.from({ length: size }, () => picker.pick(req).id);
    expect(new Set(drawn).size).toBe(size);
  });

  it('never repeats the last phrase when a new bag starts', () => {
    for (let seed = 1; seed <= 200; seed++) {
      const picker = new PhrasePicker(PHRASES, ALL, seeded(seed));
      const req = later(5, 'input');
      const size = picker.pool(req).length;
      let prev: Phrase | null = null;
      for (let i = 0; i < size * 3; i++) {
        const p = picker.pick(req);
        expect(p).not.toBe(prev);
        prev = p;
      }
    }
  });

  it('pool = general + matching reason, without wake phrases', () => {
    const picker = new PhrasePicker(PHRASES, ALL);
    const pool = picker.pool(later(2, 'permission'));
    expect(pool.length).toBeGreaterThan(0);
    for (const p of pool) {
      expect(p.tags.some((t) => t === 'general' || t === 'permission')).toBe(true);
    }
    expect(pool.some((p) => p.tags.includes('permission'))).toBe(true);
    expect(pool.some((p) => p.tags.includes('done'))).toBe(false);
  });

  it('ignored phrases appear only from the 3rd yell on', () => {
    const picker = new PhrasePicker(PHRASES, ALL, seeded(3));
    for (const idx of [2]) {
      expect(picker.pool(later(idx)).some((p) => p.tags.includes('ignored'))).toBe(false);
      for (let i = 0; i < 100; i++) expect(picker.pick(later(idx)).tags).not.toContain('ignored');
    }
    expect(picker.pool(later(3)).some((p) => p.tags.includes('ignored'))).toBe(true);
    const seen = new Set(Array.from({ length: 60 }, () => picker.pick(later(3)).tags[0]));
    expect(seen.has('ignored')).toBe(true);
  });

  it('SFW mode drops explicit phrases', () => {
    const picker = new PhrasePicker(PHRASES, { ...ALL, sfwMode: true }, seeded(9));
    for (const reason of ['done', 'permission', 'input'] as const) {
      expect(picker.pool(later(5, reason)).every((p) => !p.explicit)).toBe(true);
      for (let i = 0; i < 80; i++) expect(picker.pick(later(5, reason)).explicit).toBe(false);
    }
  });

  it('filters by language', () => {
    const vi = new PhrasePicker(PHRASES, VI, seeded(4));
    expect(vi.pick(FIRST).lang).toBe('vi');
    for (let i = 0; i < 50; i++) expect(vi.pick(later(4)).lang).toBe('vi');
    const en = new PhrasePicker(PHRASES, EN, seeded(4));
    expect(en.pick(FIRST).lang).toBe('en');
    for (let i = 0; i < 50; i++) expect(en.pick(later(4)).lang).toBe('en');
  });

  it('Auto mixes both languages in later yells', () => {
    const picker = new PhrasePicker(PHRASES, ALL, seeded(6));
    const langs = new Set(Array.from({ length: 60 }, () => picker.pick(later(4)).lang));
    expect([...langs].sort()).toEqual(['en', 'vi']);
  });

  it('setOptions takes effect on the next pick', () => {
    const picker = new PhrasePicker(PHRASES, ALL, seeded(5));
    picker.pick(later(4));
    picker.setOptions({ sfwMode: true, languages: ['en'] });
    for (let i = 0; i < 40; i++) {
      const p = picker.pick(later(4));
      expect(p.explicit).toBe(false);
      expect(p.lang).toBe('en');
    }
  });

  it('falls back to the wake phrase when filters leave nothing', () => {
    const picker = new PhrasePicker(PHRASES, { sfwMode: false, languages: [] });
    expect(picker.pick(later(4)).tags).toContain('wake');
  });

  it('the Ready reply follows the language filter and never joins the yell pool', () => {
    const vi = new PhrasePicker(PHRASES, { sfwMode: true, languages: ['vi'] }, seeded(2));
    for (let i = 0; i < 20; i++) {
      const p = vi.pickReady();
      expect(p.tags).toContain('ready');
      expect(p.lang).toBe('vi');
    }
    const picker = new PhrasePicker(PHRASES, ALL, seeded(2));
    for (const reason of ['done', 'permission', 'input'] as const) {
      expect(picker.pool(later(5, reason)).some((p) => p.tags.includes('ready'))).toBe(false);
    }
  });

  it('every phrase in the data has a unique id', () => {
    expect(new Set(PHRASES.map((p) => p.id)).size).toBe(PHRASES.length);
  });
});
