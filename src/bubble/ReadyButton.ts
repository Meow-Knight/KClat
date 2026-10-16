// The small Ready button next to the sleeping cat: "sleep now, wake me when Claude is done".
// Icon: a cat's collar bell (pink collar, jingle bell) - ring the bell and the cat will ring you back.
import { t } from '../i18n';
import type { UiLanguage } from '../settings';

const ICON = `<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
  <path class="collar" d="M3.5 5.2 Q12 9.4 20.5 5.2" fill="none" stroke-width="2.6" stroke-linecap="round"/>
  <circle class="ring" cx="12" cy="8.6" r="1.5" fill="none" stroke-width="1.5"/>
  <circle class="bell" cx="12" cy="15" r="6" stroke-width="1.8"/>
  <path class="band" d="M6.4 13.6 Q12 15.4 17.6 13.6" fill="none" stroke-width="1.5"/>
  <circle class="slot" cx="12" cy="17.4" r="1.25"/>
  <path class="slot-line" d="M12 18.6v2" stroke-width="1.5" stroke-linecap="round"/>
  <circle class="shine" cx="9.6" cy="12" r="1.1"/>
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
