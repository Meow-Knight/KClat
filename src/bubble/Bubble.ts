// Speech bubble: pops in, shakes its text, and dismisses on click.

export class Bubble {
  readonly el: HTMLDivElement;
  private readonly text: HTMLSpanElement;
  private readonly sub: HTMLSpanElement;
  private shown = false;

  constructor(
    parent: HTMLElement,
    private readonly onDismiss: () => void,
    private reduceMotion = false,
  ) {
    this.el = document.createElement('div');
    this.el.className = 'cat-bubble';
    this.el.setAttribute('role', 'status');
    this.text = document.createElement('span');
    this.text.className = 'cat-bubble-text';
    this.sub = document.createElement('span');
    this.sub.className = 'cat-bubble-sub';
    this.el.append(this.text, this.sub);
    this.el.addEventListener('click', (e) => {
      e.stopPropagation();
      if (this.shown) this.onDismiss();
    });
    parent.append(this.el);
  }

  get visible(): boolean {
    return this.shown;
  }

  /** `subtitle`: project name, shown when more than one session is waiting. */
  show(text: string, subtitle?: string): void {
    this.text.textContent = text;
    this.sub.textContent = subtitle ?? '';
    this.sub.hidden = !subtitle;
    this.shown = true;
    this.el.classList.add('show');
    this.el.classList.remove('shake');
    if (!this.reduceMotion) {
      void this.el.offsetWidth; // restart the CSS animation
      this.el.classList.add('shake');
    }
  }

  hide(): void {
    this.shown = false;
    this.el.classList.remove('show', 'shake');
  }

  setReduceMotion(on: boolean): void {
    this.reduceMotion = on;
    if (on) this.el.classList.remove('shake');
  }
}
