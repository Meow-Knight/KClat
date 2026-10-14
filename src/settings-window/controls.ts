// Small DOM building blocks for the Settings window: rows, cards, switches, segmented controls, steppers, sliders,
// buttons.
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
}

export function row({ title, desc, control, below }: RowParts): HTMLElement {
  const titleId = uid('row-title');
  const descId = uid('row-desc');
  if (control) {
    const target = control.matches('button, input, [role]') ? control : control.querySelector<HTMLElement>('[role], input');
    target?.setAttribute('aria-labelledby', titleId);
    if (desc) target?.setAttribute('aria-describedby', descId);
  }
  return h(
    'div',
    { class: 'row' },
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
  el.append(...buttons);
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
  /** While dragging (at most every SLIDER_LIVE_MS) and once more on release; arrow keys send each step. */
  onChange(value: number): void;
}

/** How often a dragged slider sends its value, so the result follows the hand without flooding Rust. */
const SLIDER_LIVE_MS = 50;

/** A horizontal range slider with its value beside it. Native keyboard: arrows, Page Up/Down, Home/End. */
export function slider(p: SliderParts): Control<number> {
  const input = h('input', {
    class: 'slider-input',
    attrs: { type: 'range', min: String(p.min), max: String(p.max), step: String(p.step), 'data-focus-key': p.focusKey },
  });
  const label = h('span', { class: 'slider-value', attrs: { 'aria-hidden': 'true' } });
  let value = p.value;
  let sent = p.value;
  let dragging = false;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const render = (v: number) => {
    input.value = String(v);
    input.setAttribute('aria-valuetext', p.format(v));
    input.style.setProperty('--fill', `${((v - p.min) / (p.max - p.min)) * 100}%`);
    label.textContent = p.format(v);
  };
  const send = () => {
    clearTimeout(timer);
    timer = undefined;
    const v = Number(input.value);
    if (v !== sent) {
      sent = v;
      p.onChange(v);
    }
  };

  input.addEventListener('pointerdown', () => (dragging = true));
  input.addEventListener('input', () => {
    render(Number(input.value));
    if (!dragging) send(); // keyboard: every step counts
    else timer ??= setTimeout(send, SLIDER_LIVE_MS);
  });
  const release = () => {
    if (!dragging) return;
    dragging = false;
    send(); // Rust's answer to the final value comes back through set()
  };
  input.addEventListener('change', release);
  input.addEventListener('pointerup', release);
  input.addEventListener('pointercancel', release);
  render(value);

  return {
    el: h('div', { class: 'slider' }, input, label),
    set(next) {
      value = next;
      sent = next;
      // Replies to earlier values arrive while dragging; moving the thumb under the hand would fight it.
      if (!dragging) render(next);
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
