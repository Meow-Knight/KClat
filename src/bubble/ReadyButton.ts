// The small Ready button next to the sleeping cat: an Aladdin lamp. Ready on: the cat is out, sleeping, and
// wakes you when Claude is done. Ready off: the cat is sucked into the lamp (CatStage) and only the lamp is left.
// The spout points left, at the cat: that is where it goes in and comes out.
import { t } from '../i18n';
import type { UiLanguage } from '../settings';

const ICON = `<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
  <path class="smoke" d="M2.4 8.2 Q0.8 6.2 3 4.6 Q5 3 3.8 1.2" fill="none" stroke-width="1.5" stroke-linecap="round"/>
  <path class="handle" d="M18.6 14 Q23 12 21.6 16.2 Q20.8 18 17.8 17.2" fill="none" stroke-width="1.6" stroke-linecap="round"/>
  <path class="foot" d="M10.4 18.8 L9.6 21 H16.4 L15.6 18.8 Z" stroke-width="1.4" stroke-linejoin="round"/>
  <path class="lamp" d="M2 9.6 Q3.4 15 7.6 15.2 Q8.8 19.4 12.8 19.2 Q17.4 19 19 13.4 H8.2 Q6.4 11.4 2 9.6 Z" stroke-width="1.6" stroke-linejoin="round"/>
  <path class="lamp" d="M9.4 13.4 Q12.8 8.6 16.2 13.4 Z" stroke-width="1.5" stroke-linejoin="round"/>
  <circle class="knob" cx="12.8" cy="10" r="1"/>
  <circle class="shine" cx="15.8" cy="15.6" r="0.9"/>
</svg>`;

/**
 * The cat size zooms the whole page, button included. Up to this zoom the button grows with the cat; past it
 * the button is scaled back so it stays at this size and does not crowd the cat.
 */
const MAX_ZOOM = 1.2;

export class ReadyButton {
  readonly el: HTMLButtonElement;
  private on = false;
  private lang: UiLanguage = 'en';
  /** Counter-scale for the page zoom (1 = none); see MAX_ZOOM. */
  private shrink = 1;

  constructor(
    parent: HTMLElement,
    private readonly onToggle: (next: boolean) => void,
  ) {
    this.el = document.createElement('button');
    this.el.type = 'button';
    this.el.className = 'cat-ready';
    this.el.innerHTML = ICON;
    this.render();
    this.el.addEventListener('click', (e) => {
      e.stopPropagation();
      this.onToggle(!this.on);
    });
    parent.append(this.el);
  }

  get pressed(): boolean {
    return this.on;
  }

  get visible(): boolean {
    return !this.el.hidden;
  }

  set(on: boolean): void {
    if (on === this.on) return;
    this.on = on;
    this.render();
  }

  /** Platform Language for the tooltip. */
  setLanguage(lang: UiLanguage): void {
    if (lang === this.lang) return;
    this.lang = lang;
    this.render();
  }

  setVisible(visible: boolean): void {
    this.el.hidden = !visible;
  }

  /** The cat's on-screen scale (page zoom times any preview transform): caps the button's size at MAX_ZOOM. */
  setZoom(zoom: number): void {
    this.shrink = Math.min(1, MAX_ZOOM / zoom);
    this.el.style.setProperty('--ready-shrink', String(this.shrink));
  }

  /** The button's clickable box in stage CSS px: its layout box, shrunk about its centre like the drawing. */
  hitbox(): { x: number; y: number; w: number; h: number } {
    const w = this.el.offsetWidth * this.shrink;
    const h = this.el.offsetHeight * this.shrink;
    return {
      x: this.el.offsetLeft + (this.el.offsetWidth - w) / 2,
      y: this.el.offsetTop + (this.el.offsetHeight - h) / 2,
      w,
      h,
    };
  }

  /** Centre the button on a stage point (CSS px). */
  placeAt(x: number, y: number): void {
    this.el.style.left = `${Math.round(x - this.el.offsetWidth / 2)}px`;
    this.el.style.top = `${Math.round(y - this.el.offsetHeight / 2)}px`;
  }

  private render(): void {
    this.el.setAttribute('aria-pressed', String(this.on));
    this.el.title = t(this.lang, this.on ? 'ready.on' : 'ready.off');
    this.el.setAttribute('aria-label', this.el.title);
  }
}
