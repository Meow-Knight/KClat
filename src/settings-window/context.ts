// What every section of the Settings window gets: the API, the current settings, and translated text.
import type { HooksStatus, SettingsPatch } from '../bridge/tauriBridge';
import type { UiKey } from '../i18n';
import type { AppSettings, UiLanguage } from '../settings';
import type { SettingsApi } from '../bridge/settingsApi';

export interface Ctx {
  api: SettingsApi;
  lang: UiLanguage;
  settings: AppSettings;
  t(key: UiKey, vars?: Record<string, string | number>): string;
  /** Sends a change to Rust; the result comes back through `update`. */
  patch(p: SettingsPatch): void;
  /** Tells the sidebar the hooks status changed. */
  onHooksStatus(status: HooksStatus): void;
}

export interface SectionView {
  el: HTMLElement;
  /** Settings changed (here or elsewhere): reflect them without rebuilding. */
  update(s: AppSettings): void;
}
