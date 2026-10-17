// Language: Platform Language (this UI) and Klat Language (the cat), with a sample of what the cat would say.
import { PHRASE_LANGS, PHRASES, type PhraseLang } from '../../phrases/phrases';
import { PhrasePicker, type WakeReason } from '../../phrases/PhrasePicker';
import { phraseLanguages, type AppSettings, type KlatLanguage, type UiLanguage } from '../../settings';
import { card, h, row, segmented, svg } from '../controls';
import type { Ctx, SectionView } from '../context';

const REFRESH_ICON = `<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M13 8a5 5 0 1 1-1.5-3.6M13 2.5v2.8h-2.8" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
const REASONS: readonly WakeReason[] = ['done', 'permission', 'input'];
/** Each language in its own name, as language pickers do. */
const LANGUAGE_NAMES: Record<PhraseLang, string> = { en: 'English', vi: 'Tiếng Việt', zh: '中文', ja: '日本語', ko: '한국어' };

export function languageSection(ctx: Ctx): SectionView {
  const { t, settings: s } = ctx;

  const platform = segmented<UiLanguage>(
    'uiLanguage',
    [
      { value: 'en', label: LANGUAGE_NAMES.en, lang: 'en' },
      { value: 'vi', label: LANGUAGE_NAMES.vi, lang: 'vi' },
    ],
    s.uiLanguage,
    (v) => ctx.patch({ uiLanguage: v }),
  );
  const klat = segmented<KlatLanguage>(
    'klatLanguage',
    [
      ...PHRASE_LANGS.map((lang) => ({ value: lang, label: LANGUAGE_NAMES[lang], lang })),
      { value: 'auto', label: t('settings.language.auto') },
    ],
    s.klatLanguage,
    (v) => ctx.patch({ klatLanguage: v }),
  );

  // The preview: the first line is the wake yell, then any line the cat could say later.
  const bubble = h('p', { class: 'preview-bubble', attrs: { 'aria-live': 'polite' } });
  const picker = new PhrasePicker(PHRASES, { sfwMode: s.sfwMode, languages: phraseLanguages(s.klatLanguage) });
  let shown = 0;
  let key = '';
  const show = (first: boolean) => {
    const reason = REASONS[shown % REASONS.length];
    const phrase = picker.pick({ first, yellIndex: first ? 1 : 3 + (shown % 3), reason });
    shown++;
    bubble.textContent = phrase.text;
    bubble.lang = phrase.lang;
  };
  const reset = (next: AppSettings) => {
    const k = `${next.klatLanguage}|${next.sfwMode}`;
    if (k === key) return;
    key = k;
    picker.setOptions({ sfwMode: next.sfwMode, languages: phraseLanguages(next.klatLanguage) });
    shown = 0;
    show(true);
  };
  const another = h('button', { class: 'btn btn-ghost', attrs: { type: 'button', 'data-focus-key': 'anotherLine' } }, svg(REFRESH_ICON), t('settings.language.another'));
  another.addEventListener('click', () => show(false));
  reset(s);

  const preview = h(
    'div',
    { class: 'preview' },
    h('div', { class: 'preview-label', text: t('settings.language.preview') }),
    h('div', { class: 'preview-stage' }, bubble, another),
  );

  const el = h(
    'div',
    {},
    card(null, row({ title: t('settings.language.platform'), desc: t('settings.language.platform.desc'), control: platform.el })),
    card(null, row({ title: t('settings.language.klat'), desc: t('settings.language.klat.desc'), control: klat.el, below: preview, stacked: true })),
  );

  return {
    el,
    update(next) {
      platform.set(next.uiLanguage);
      klat.set(next.klatLanguage);
      reset(next);
    },
  };
}
