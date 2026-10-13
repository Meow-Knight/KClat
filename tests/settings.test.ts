import { describe, expect, it } from 'vitest';
import { DEFAULT_SETTINGS, phraseLanguages } from '../src/settings';

describe('settings', () => {
  it('defaults to English UI and a cat that mixes both languages', () => {
    expect(DEFAULT_SETTINGS.uiLanguage).toBe('en');
    expect(DEFAULT_SETTINGS.klatLanguage).toBe('auto');
  });

  it('maps Klat Language to the phrase languages the cat may use', () => {
    expect(phraseLanguages('auto')).toEqual(['en', 'vi']);
    expect(phraseLanguages('en')).toEqual(['en']);
    expect(phraseLanguages('vi')).toEqual(['vi']);
  });
});
