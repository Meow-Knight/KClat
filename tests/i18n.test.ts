import { describe, expect, it } from 'vitest';
import { catalogs, t } from '../src/i18n';

const placeholders = (text: string): string[] => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

describe('i18n', () => {
  it('English and Vietnamese have the same keys', () => {
    expect(Object.keys(catalogs.vi).sort()).toEqual(Object.keys(catalogs.en).sort());
  });

  it('no text is empty and placeholders match across languages', () => {
    for (const [key, en] of Object.entries(catalogs.en)) {
      const vi = catalogs.vi[key];
      expect(en.trim(), key).not.toBe('');
      expect(vi.trim(), key).not.toBe('');
      expect(placeholders(vi), key).toEqual(placeholders(en));
    }
  });

  it('looks up by language and fills placeholders', () => {
    expect(t('en', 'tray.quit')).toBe('Quit');
    expect(t('vi', 'tray.quit')).toBe('Thoát');
    expect(t('en', 'tray.tooltip.many', { count: 3 })).toBe('Klat · 3 sessions waiting');
    expect(t('vi', 'tray.tooltip.many', { count: 3 })).toBe('Klat · 3 phiên đang chờ');
  });

  it('leaves an unknown placeholder as it is', () => {
    expect(t('en', 'tray.tooltip.many', {})).toBe('Klat · {count} sessions waiting');
  });
});
