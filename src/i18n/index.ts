// Platform Language (docs/specs/settings-i18n-macos.md §3.1). The same catalogs are compiled into the Rust side
// (crates/klat-core/src/i18n.rs). A key missing in Vietnamese falls back to English; `{name}` is replaced by vars[name].
import type { UiLanguage } from '../settings';
import en from './en.json';
import vi from './vi.json';

export type UiKey = keyof typeof en;

const CATALOGS: Record<UiLanguage, Partial<Record<UiKey, string>>> = { en, vi };

export function t(lang: UiLanguage, key: UiKey, vars?: Record<string, string | number>): string {
  const text = CATALOGS[lang][key] ?? en[key] ?? key;
  if (!vars) return text;
  return text.replace(/\{(\w+)\}/g, (match, name: string) => (name in vars ? String(vars[name]) : match));
}

/** For tests: both catalogs as plain records. */
export const catalogs: Readonly<Record<UiLanguage, Readonly<Record<string, string>>>> = { en, vi };
