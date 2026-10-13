// Settings window entry (docs/specs/settings-i18n-macos.md §4). Every change goes to Rust at once (no Save
// button); Rust validates it and broadcasts `cat://settings`, which this window, the overlay and the tray follow.
import type { HooksStatus, SettingsPatch } from '../bridge/tauriBridge';
import { t as translate, type UiKey } from '../i18n';
import type { AppSettings } from '../settings';
import { mockApi, tauriApi } from './api';
import type { Ctx, SectionView } from './context';
import { h, svg } from './controls';
import { advancedSection } from './sections/advanced';
import { generalSection } from './sections/general';
import { hooksSection } from './sections/hooks';
import { languageSection } from './sections/language';
import './settings.css';
import { SECTIONS, nextSection, sectionFromHash, type SectionId } from './viewModel';

const CAT = `<svg class="brand-cat" viewBox="0 0 40 40" aria-hidden="true">
  <path d="M8.5 17 7 5.5l9 6.2M31.5 17 33 5.5l-9 6.2" fill="#9aa6bd" stroke="#2b2d42" stroke-width="1.6" stroke-linejoin="round"/>
  <path d="M9.6 13.2 9 8.6l3.6 2.6M30.4 13.2 31 8.6l-3.6 2.6" fill="#f29bb0"/>
  <ellipse cx="20" cy="22.5" rx="13.2" ry="11" fill="#9aa6bd" stroke="#2b2d42" stroke-width="1.6"/>
  <path d="M12.4 21.4q2.6 2.1 5.2 0M22.4 21.4q2.6 2.1 5.2 0" fill="none" stroke="#2b2d42" stroke-width="1.5" stroke-linecap="round"/>
  <path d="M12.6 17.6l4.4 1.4M27.4 17.6 23 19" stroke="#2b2d42" stroke-width="1.5" stroke-linecap="round"/>
  <path d="M18.6 25h2.8l-1.4 1.6z" fill="#f29bb0" stroke="#2b2d42" stroke-width="0.9" stroke-linejoin="round"/>
  <ellipse cx="12" cy="26" rx="2" ry="1.2" fill="#f7a9bc" opacity=".7"/>
  <ellipse cx="28" cy="26" rx="2" ry="1.2" fill="#f7a9bc" opacity=".7"/>
</svg>`;

const ICONS: Record<SectionId, string> = {
  general: '<path d="M3 5h6M13 5h4M3 15h2M9 15h8M9 3v4M5 13v4" />',
  language: '<circle cx="10" cy="10" r="7"/><path d="M3 10h14M10 3c2.2 2 2.2 12 0 14M10 3c-2.2 2-2.2 12 0 14"/>',
  hooks: '<path d="M8.5 11.5l3-3M7 9.5 5.3 11.2a2.6 2.6 0 0 0 3.7 3.7l1.7-1.7M13 10.5l1.7-1.7A2.6 2.6 0 0 0 11 5.1L9.3 6.8"/>',
  advanced: '<circle cx="10" cy="10" r="2.6"/><path d="M10 2.8v2M10 15.2v2M2.8 10h2M15.2 10h2M4.9 4.9l1.4 1.4M13.7 13.7l1.4 1.4M4.9 15.1l1.4-1.4M13.7 6.3l1.4-1.4"/>',
};

const NAV_KEY: Record<SectionId, UiKey> = {
  general: 'settings.nav.general',
  language: 'settings.nav.language',
  hooks: 'settings.nav.hooks',
  advanced: 'settings.nav.advanced',
};
const TITLE_KEY: Record<SectionId, [UiKey, UiKey]> = {
  general: ['settings.general.title', 'settings.general.lead'],
  language: ['settings.language.title', 'settings.language.lead'],
  hooks: ['settings.hooks.title', 'settings.hooks.lead'],
  advanced: ['settings.advanced.title', 'settings.advanced.lead'],
};
const BUILD: Record<SectionId, (ctx: Ctx) => SectionView> = {
  general: generalSection,
  language: languageSection,
  hooks: hooksSection,
  advanced: advancedSection,
};

const params = new URLSearchParams(location.search);
const api = params.has('mock') || !('__TAURI_INTERNALS__' in window) ? mockApi(params) : tauriApi();
const root = document.getElementById('app')!;

let settings: AppSettings = await api.getSettings();
let version = '';
let active: SectionId = sectionFromHash(location.hash);
let hooksStatus: HooksStatus | null = null;
let view: SectionView | null = null;
let navButtons: Partial<Record<SectionId, HTMLButtonElement>> = {};
let main: HTMLElement;

const t = (key: UiKey, vars?: Record<string, string | number>) => translate(settings.uiLanguage, key, vars);

function ctx(): Ctx {
  return {
    api,
    lang: settings.uiLanguage,
    settings,
    t,
    patch: (p: SettingsPatch) => void api.setSettings(p).then(apply),
    onHooksStatus: (status) => {
      hooksStatus = status;
      paintHooksDot();
    },
  };
}

function paintHooksDot(): void {
  const dot = navButtons.hooks?.querySelector('.nav-dot');
  if (dot) dot.className = `nav-dot nav-dot-${hooksStatus ?? 'unknown'}`;
}

function renderSection(focusNav = false): void {
  const [title, lead] = TITLE_KEY[active];
  view = BUILD[active](ctx());
  main.replaceChildren(
    h(
      'div',
      { class: 'content' },
      h('header', { class: 'content-head' }, h('h1', { text: t(title), attrs: { tabindex: '-1' } }), h('p', { class: 'lead', text: t(lead) })),
      view.el,
    ),
  );
  main.scrollTop = 0;
  for (const id of SECTIONS) {
    const b = navButtons[id]!;
    b.setAttribute('aria-current', String(id === active));
    b.tabIndex = id === active ? 0 : -1;
  }
  if (focusNav) navButtons[active]!.focus();
  history.replaceState(null, '', `${location.search}#${active}`);
}

function select(id: SectionId, focusNav = false): void {
  if (id === active && view) return;
  active = id;
  renderSection(focusNav);
}

/** Builds the whole window. Used at start and when the Platform Language changes. */
function renderAll(): void {
  const focusKey = (document.activeElement as HTMLElement | null)?.dataset?.focusKey;
  document.documentElement.lang = settings.uiLanguage;
  document.title = t('settings.windowTitle');
  document.body.classList.toggle('reduce-motion', settings.reduceMotion);

  navButtons = {};
  const nav = h('nav', { class: 'nav', attrs: { 'aria-label': t('settings.nav.label') } });
  for (const id of SECTIONS) {
    const b = h(
      'button',
      // aria-label: the visible label is hidden in narrow windows.
      { class: 'nav-item', attrs: { type: 'button', 'data-focus-key': `nav:${id}`, 'aria-label': t(NAV_KEY[id]) } },
      svg(`<svg class="nav-icon" viewBox="0 0 20 20" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">${ICONS[id]}</svg>`),
      h('span', { class: 'nav-label', text: t(NAV_KEY[id]) }),
      id === 'hooks' ? h('span', { class: 'nav-dot', attrs: { 'aria-hidden': 'true' } }) : null,
    );
    b.addEventListener('click', () => select(id));
    b.addEventListener('keydown', (e) => {
      const dir = e.key === 'ArrowDown' ? 1 : e.key === 'ArrowUp' ? -1 : 0;
      if (dir) {
        e.preventDefault();
        select(nextSection(active, dir), true);
      }
    });
    navButtons[id] = b;
    nav.append(b);
  }

  const sidebar = h(
    'aside',
    { class: 'sidebar' },
    h(
      'div',
      { class: 'brand' },
      h('div', { class: 'brand-badge' }, svg(CAT)),
      h('div', {}, h('div', { class: 'brand-name', text: 'Klat' }), h('div', { class: 'brand-version', text: version ? t('settings.version', { version }) : '' })),
    ),
    nav,
  );
  main = h('main', { class: 'main' });
  root.replaceChildren(sidebar, main);
  paintHooksDot();
  renderSection();
  if (focusKey) document.querySelector<HTMLElement>(`[data-focus-key="${CSS.escape(focusKey)}"]`)?.focus();
}

function apply(next: AppSettings): void {
  const relabel = next.uiLanguage !== settings.uiLanguage;
  settings = next;
  if (relabel) renderAll();
  else {
    document.body.classList.toggle('reduce-motion', next.reduceMotion);
    view?.update(next);
  }
}

await api.onSettings(apply);
renderAll();
void api.version().then((v) => {
  version = v;
  const el = document.querySelector('.brand-version');
  if (el) el.textContent = t('settings.version', { version });
});
