// Small DOM building blocks for the Settings window: rows, cards, switches, segmented controls, picture choices,
// steppers, sliders, buttons.
// Each control returns its element plus a `set` that reflects a value coming back from Rust without firing.

type Child = Node | string | null | undefined | false;

export function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: { class?: string; text?: string; attrs?: Record<string, string> } = {},
  ...children: Child[]
): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  if (props.class) el.className = props.class;
  if (props.text !== undefined) el.textContent = props.text;
  for (const [k, v] of Object.entries(props.attrs ?? {})) el.setAttribute(k, v);
  for (const c of children) if (c) el.append(c);
  return el;
}

export function svg(markup: string): SVGElement {
  const tpl = document.createElement('template');
  tpl.innerHTML = markup.trim();
  return tpl.content.firstElementChild as SVGElement;
}

let ids = 0;
const uid = (prefix: string) => `${prefix}-${++ids}`;

/** A titled group of rows. */
export function card(title: string | null, ...rows: Child[]): HTMLElement {
  const id = uid('card');
  return h(
    'section',
    { class: 'card-group', attrs: title ? { 'aria-labelledby': id } : {} },
    title ? h('h2', { class: 'card-title', text: title, attrs: { id } }) : null,
    h('div', { class: 'card' }, ...rows),
  );
}

export interface RowParts {
  title: string;
  desc?: string;
  /** Labelled by the title and described by the description. */
  control?: HTMLElement;
  /** Extra content under the text (errors, previews). */
  below?: HTMLElement;
  /** The control goes under the text instead of beside it (a control too wide to share the line). */
  stacked?: boolean;
}

export function row({ title, desc, control, below, stacked }: RowParts): HTMLElement {
  const titleId = uid('row-title');
  const descId = uid('row-desc');
  if (control) {
    const target = control.matches('button, input, [role]') ? control : control.querySelector<HTMLElement>('[role], input');
    target?.setAttribute('aria-labelledby', titleId);
    if (desc) target?.setAttribute('aria-describedby', descId);
  }
  return h(
    'div',
    { class: stacked ? 'row row-stacked' : 'row' },
    h(
      'div',
      { class: 'row-text' },
      h('div', { class: 'row-title', text: title, attrs: { id: titleId } }),
      desc ? h('div', { class: 'row-desc', text: desc, attrs: { id: descId } }) : null,
    ),
    control ? h('div', { class: 'row-control' }, control) : null,
    below ? h('div', { class: 'row-below' }, below) : null,
  );
}

export interface Control<T> {
  el: HTMLElement;
  set(value: T): void;
}

export function toggle(focusKey: string, checked: boolean, onChange: (next: boolean) => void): Control<boolean> & {
  setBusy(busy: boolean): void;
} {
  const el = h('button', {
    class: 'switch',
    attrs: { type: 'button', role: 'switch', 'data-focus-key': focusKey },
  });
  el.append(h('span', { class: 'switch-knob', attrs: { 'aria-hidden': 'true' } }));
  let value = checked;
  const render = () => el.setAttribute('aria-checked', String(value));
  render();
  el.addEventListener('click', () => onChange(!value));
  return {
    el,
    set(next) {
      value = next;
      render();
    },
    setBusy(busy) {
      el.disabled = busy;
      el.classList.toggle('busy', busy);
    },
  };
}

export interface Option<T extends string> {
  value: T;
  label: string;
  /** BCP 47 tag when the label is in another language than the page (screen readers switch voice). */
  lang?: string;
  /** One line under the label (picture cards only); screen readers read it as the option's description. */
  desc?: string;
}

/** A radio group drawn as a segmented control. Arrow keys move and select, as in native radio groups. */
export function segmented<T extends string>(
  focusKey: string,
  options: readonly Option<T>[],
  value: T,
  onChange: (next: T) => void,
): Control<T> {
  const el = h('div', { class: 'segmented', attrs: { role: 'radiogroup' } });
  const buttons = options.map((o) =>
    h('button', {
      class: 'segment',
      text: o.label,
      attrs: { type: 'button', role: 'radio', 'data-focus-key': `${focusKey}:${o.value}`, ...(o.lang ? { lang: o.lang } : {}) },
    }),
  );
  el.append(...buttons);
  return radioGroup(el, buttons, options, value, onChange);
}

/** What the picture choice shows for one option: the picked one awake, the others asleep (awake on hover). */
export interface Pictures {
  awake: string;
  asleep: string;
}

/**
 * A radio group of picture cards, one per option. Pictures can arrive later (setPictures); until then the
 * cards show their labels only.
 */
export function pictureChoice<T extends string>(
  focusKey: string,
  label: string,
  options: readonly Option<T>[],
  value: T,
  onChange: (next: T) => void,
): Control<T> & { setPictures(pictures: Readonly<Record<T, Pictures>>): void } {
  const el = h('div', { class: 'picks', attrs: { role: 'radiogroup', 'aria-label': label } });
  const art = new Map<T, [HTMLImageElement, HTMLImageElement]>();
  const buttons = options.map((o) => {
    // Decorative: the label names the option.
    const asleep = h('img', { class: 'pick-asleep', attrs: { alt: '', draggable: 'false' } });
    const awake = h('img', { class: 'pick-awake', attrs: { alt: '', draggable: 'false' } });
    art.set(o.value, [asleep, awake]);
    const descId = `${focusKey}-${o.value}-desc`;
    return h(
      'button',
      {
        class: 'pick',
        attrs: {
          type: 'button',
          role: 'radio',
          'data-focus-key': `${focusKey}:${o.value}`,
          ...(o.desc ? { 'aria-describedby': descId } : {}),
        },
      },
      h('span', { class: 'pick-art', attrs: { 'aria-hidden': 'true' } }, asleep, awake),
      // The label alone names the option; the description is linked by aria-describedby.
      h('span', { class: 'pick-label', text: o.label }),
      ...(o.desc ? [h('span', { class: 'pick-desc', text: o.desc, attrs: { id: descId, 'aria-hidden': 'true' } })] : []),
    );
  });
  el.append(...buttons);
  return {
    ...radioGroup(el, buttons, options, value, onChange),
    setPictures(pictures) {
      for (const [v, [asleep, awake]] of art) {
        asleep.src = pictures[v].asleep;
        awake.src = pictures[v].awake;
      }
      el.classList.add('has-pictures');
    },
  };
}

/** Radio behaviour for a group of buttons: one tab stop, click to pick, arrow keys move and pick. */
function radioGroup<T extends string>(
  el: HTMLElement,
  buttons: HTMLElement[],
  options: readonly Option<T>[],
  value: T,
  onChange: (next: T) => void,
): Control<T> {
  let current = value;
  const render = () =>
    buttons.forEach((b, i) => {
      const on = options[i].value === current;
      b.setAttribute('aria-checked', String(on));
      b.tabIndex = on ? 0 : -1;
    });
  buttons.forEach((b, i) => {
    b.addEventListener('click', () => {
      if (options[i].value !== current) onChange(options[i].value);
    });
    b.addEventListener('keydown', (e) => {
      const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[e.key];
      if (!step) return;
      e.preventDefault();
      const j = (i + step + options.length) % options.length;
      buttons[j].focus();
      onChange(options[j].value);
    });
  });
  render();
  return {
    el,
    set(next) {
      current = next;
      render();
    },
  };
}

export interface StepperParts {
  focusKey: string;
  value: number;
  unit: string;
  decreaseLabel: string;
  increaseLabel: string;
  min: number;
  max: number;
  /** Typed text (null = not a number) or a step direction. */
  onCommit(input: string | { step: 1 | -1 }): void;
}

export function stepper(p: StepperParts): Control<number> {
  const input = h('input', {
    class: 'stepper-input',
    attrs: { type: 'text', inputmode: 'numeric', autocomplete: 'off', spellcheck: 'false', 'data-focus-key': p.focusKey },
  });
  const minus = h('button', { class: 'stepper-btn', text: '−', attrs: { type: 'button', 'aria-label': p.decreaseLabel, tabindex: '-1' } });
  const plus = h('button', { class: 'stepper-btn', text: '+', attrs: { type: 'button', 'aria-label': p.increaseLabel, tabindex: '-1' } });
  let value = p.value;
  const render = () => {
    input.value = String(value);
    input.setAttribute('aria-valuenow', String(value));
    minus.disabled = value <= p.min;
    plus.disabled = value >= p.max;
  };
  input.setAttribute('role', 'spinbutton');
  input.setAttribute('aria-valuemin', String(p.min));
  input.setAttribute('aria-valuemax', String(p.max));
  const commitTyped = () => {
    if (input.value.trim() !== String(value)) p.onCommit(input.value);
    render(); // snaps back if Rust keeps the old value
  };
  input.addEventListener('change', commitTyped);
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') commitTyped();
    else if (e.key === 'Escape') render();
    else if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
      e.preventDefault();
      p.onCommit({ step: e.key === 'ArrowUp' ? 1 : -1 });
    }
  });
  minus.addEventListener('click', () => p.onCommit({ step: -1 }));
  plus.addEventListener('click', () => p.onCommit({ step: 1 }));
  render();
  return {
    el: h('div', { class: 'stepper' }, minus, input, h('span', { class: 'stepper-unit', text: p.unit, attrs: { 'aria-hidden': 'true' } }), plus),
    set(next) {
      value = next;
      if (document.activeElement !== input) render();
      else {
        input.setAttribute('aria-valuenow', String(value));
        minus.disabled = value <= p.min;
        plus.disabled = value >= p.max;
      }
    },
  };
}

export interface SliderParts {
  focusKey: string;
  value: number;
  min: number;
  max: number;
  step: number;
  /** The value as shown next to the track and read by screen readers ("120%"). */
  format(value: number): string;
  /**
   * Every move, at most once per frame: show the value without saving it. Optional; without it the value is
   * only sent through onChange.
   */
  onPreview?(value: number): void;
  /** The value to keep: on release after a drag, or once the keys have rested for SLIDER_KEYS_REST_MS. */
  onChange(value: number): void;
}

/** Arrow keys preview each step and save once they have been left alone this long (a held key repeats). */
const SLIDER_KEYS_REST_MS = 400;

/** A horizontal range slider with its value beside it. Native keyboard: arrows, Page Up/Down, Home/End. */
export function slider(p: SliderParts): Control<number> {
  const input = h('input', {
    class: 'slider-input',
    attrs: { type: 'range', min: String(p.min), max: String(p.max), step: String(p.step), 'data-focus-key': p.focusKey },
  });
  const label = h('span', { class: 'slider-value', attrs: { 'aria-hidden': 'true' } });
  let sent = p.value;
  let dragging = false;
  let frame: number | undefined;
  let rest: ReturnType<typeof setTimeout> | undefined;

  const render = (v: number) => {
    input.value = String(v);
    input.setAttribute('aria-valuetext', p.format(v));
    input.style.setProperty('--fill', `${((v - p.min) / (p.max - p.min)) * 100}%`);
    label.textContent = p.format(v);
  };
  const preview = () => {
    frame = undefined;
    p.onPreview?.(Number(input.value));
  };
  /** Saves the value; a preview still waiting for its frame is dropped, so none can follow the save. */
  const commit = () => {
    if (frame !== undefined) cancelAnimationFrame(frame);
    frame = undefined;
    clearTimeout(rest);
    rest = undefined;
    const v = Number(input.value);
    if (v !== sent || p.onPreview) {
      sent = v;
      p.onChange(v);
    }
  };

  input.addEventListener('pointerdown', () => (dragging = true));
  input.addEventListener('input', () => {
    render(Number(input.value));
    if (!p.onPreview) {
      if (!dragging) commit();
      return;
    }
    frame ??= requestAnimationFrame(preview);
    if (!dragging) {
      clearTimeout(rest);
      rest = setTimeout(commit, SLIDER_KEYS_REST_MS);
    }
  });
  const release = () => {
    if (!dragging) return;
    dragging = false;
    commit(); // Rust's answer to the final value comes back through set()
  };
  input.addEventListener('change', release);
  input.addEventListener('pointerup', release);
  input.addEventListener('pointercancel', release);
  // Leaving the window in the middle of a drag or before the keys have rested still keeps the value.
  window.addEventListener('blur', () => {
    if (dragging) release();
    else if (rest !== undefined) commit();
  });
  render(p.value);

  return {
    el: h('div', { class: 'slider' }, input, label),
    set(next) {
      sent = next;
      // Replies to earlier values arrive while dragging; moving the thumb under the hand would fight it.
      if (!dragging && rest === undefined) render(next);
    },
  };
}

export function button(
  label: string,
  variant: 'primary' | 'secondary' | 'danger',
  focusKey: string,
  onClick: () => void,
): HTMLButtonElement {
  const el = h('button', { class: `btn btn-${variant}`, text: label, attrs: { type: 'button', 'data-focus-key': focusKey } });
  el.addEventListener('click', onClick);
  return el;
}

/** An inline result message. Errors are announced at once, successes politely. */
export function banner(tone: 'ok' | 'error', text: string): HTMLElement {
  return h(
    'div',
    { class: `banner banner-${tone}`, attrs: { role: tone === 'error' ? 'alert' : 'status' } },
    svg(
      tone === 'ok'
        ? '<svg class="banner-icon" viewBox="0 0 16 16" aria-hidden="true"><path d="M3.5 8.5l3 3 6-7" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>'
        : '<svg class="banner-icon" viewBox="0 0 16 16" aria-hidden="true"><path d="M8 4.5v4.2M8 11.3v.2" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/><circle cx="8" cy="8" r="6.3" fill="none" stroke="currentColor" stroke-width="1.4"/></svg>',
    ),
    h('div', { class: 'banner-text', text }),
  );
}
