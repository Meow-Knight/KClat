// Phrase selection rules from spec §7.2.
import type { Phrase, PhraseLang } from './phrases';

export type WakeReason = 'done' | 'permission' | 'input';

export interface PickerOptions {
  sfwMode: boolean;
  languages: readonly PhraseLang[];
}

export interface PickRequest {
  /** First yell of this wake: always the wake phrase. */
  first: boolean;
  /** 1-based yell count within the current wake. */
  yellIndex: number;
  reason: WakeReason;
}

/** `ignored` phrases join the pool from this yell on. */
export const IGNORED_FROM_YELL = 3;

const FALLBACK_WAKE: Phrase = { id: 'wake-fallback', text: 'Wake up, human!!!', lang: 'en', tags: ['wake'], explicit: false };

export class PhrasePicker {
  private bag: Phrase[] = [];
  private bagKey = '';
  private last: Phrase | null = null;
  private opts: PickerOptions;

  constructor(
    private readonly phrases: readonly Phrase[],
    opts: PickerOptions,
    private readonly random: () => number = Math.random,
  ) {
    this.opts = { ...opts };
  }

  setOptions(opts: PickerOptions): void {
    this.opts = { ...opts };
    this.bagKey = '';
  }

  pick(req: PickRequest): Phrase {
    if (req.first) return this.remember(this.wakePhrase());

    // The pool depends on the reason and on whether `ignored` is in; a different pool starts a new bag.
    const key = `${req.reason}|${req.yellIndex >= IGNORED_FROM_YELL}`;
    if (key !== this.bagKey) {
      this.bagKey = key;
      this.bag = [];
    }
    if (this.bag.length === 0) this.refill(req);
    return this.remember(this.bag.pop() ?? this.wakePhrase());
  }

  /** The cat's reply when Ready is armed, in one of the chosen languages (English if none match). */
  pickReady(): Phrase {
    const all = this.phrases.filter((p) => p.tags.includes('ready') && this.allowed(p));
    const inLang = all.filter((p) => this.opts.languages.includes(p.lang));
    const pool = inLang.length ? inLang : all;
    return pool[Math.floor(this.random() * pool.length)] ?? FALLBACK_WAKE;
  }

  /** Candidates for a non-first yell, after the SFW and language filters. */
  pool(req: PickRequest): Phrase[] {
    const withIgnored = req.yellIndex >= IGNORED_FROM_YELL;
    return this.phrases.filter(
      (p) =>
        this.allowed(p) &&
        this.opts.languages.includes(p.lang) &&
        (p.tags.includes('general') || p.tags.includes(req.reason) || (withIgnored && p.tags.includes('ignored'))),
    );
  }

  /** Wake phrases skip the language filter: there is no Vietnamese one, and the first yell must always happen. */
  private wakePhrase(): Phrase {
    const wakes = this.phrases.filter((p) => p.tags.includes('wake') && this.allowed(p));
    const preferred = this.opts.sfwMode ? wakes[0] : (wakes.find((p) => p.explicit) ?? wakes[0]);
    return preferred ?? FALLBACK_WAKE;
  }

  private allowed(p: Phrase): boolean {
    return !(this.opts.sfwMode && p.explicit);
  }

  private refill(req: PickRequest): void {
    const bag = this.pool(req);
    for (let i = bag.length - 1; i > 0; i--) {
      const j = Math.floor(this.random() * (i + 1));
      [bag[i], bag[j]] = [bag[j], bag[i]];
    }
    // The bag is drawn from the end; never open a new bag with the phrase just said.
    const end = bag.length - 1;
    if (end > 0 && bag[end] === this.last) [bag[0], bag[end]] = [bag[end], bag[0]];
    this.bag = bag;
  }

  private remember(p: Phrase): Phrase {
    this.last = p;
    return p;
  }
}
