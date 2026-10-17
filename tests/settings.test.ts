import { describe, expect, it } from 'vitest';
import { DEFAULT_SETTINGS, phraseLanguages } from '../src/settings';

describe('settings', () => {
  it('defaults to English UI and a cat that mixes every language', () => {
    expect(DEFAULT_SETTINGS.uiLanguage).toBe('en');
    expect(DEFAULT_SETTINGS.klatLanguage).toBe('auto');
  });

  it('maps Klat Language to the phrase languages the cat may use', () => {
    expect(phraseLanguages('auto')).toEqual(['en', 'vi', 'zh', 'ja', 'ko']);
    expect(phraseLanguages('en')).toEqual(['en']);
    expect(phraseLanguages('vi')).toEqual(['vi']);
    expect(phraseLanguages('ko')).toEqual(['ko']);
  });
});
