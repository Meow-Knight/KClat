// What the Settings window needs from the app, with two implementations: the real one over Tauri, and a mock
// for `pnpm dev` at /settings.html?mock (design review without the app). Mock options in the URL:
// `lang=vi`, `hooks=needs-repair|not-installed`, `fail=autostart|hooks`, `drag=missing|granted` (macOS).
import {
  bridge,
  type DragPermission,
  type HooksInfo,
  type HooksResult,
  type HooksStatus,
  type SettingsPatch,
} from '../bridge/tauriBridge';
import { t } from '../i18n';
import { DEFAULT_SETTINGS, type AppSettings, type UiLanguage } from '../settings';

export interface SettingsApi {
  getSettings(): Promise<AppSettings>;
  setSettings(patch: SettingsPatch): Promise<AppSettings>;
  setStartAtLogin(on: boolean): Promise<boolean>;
  wakeTest(): Promise<void>;
  sleep(): Promise<void>;
  hooksStatus(): Promise<HooksInfo>;
  hooksApply(action: 'install' | 'uninstall'): Promise<HooksResult>;
  dragPermission(): Promise<DragPermission>;
  openDragPermission(): Promise<void>;
  onDragPermission(cb: (p: DragPermission) => void): Promise<unknown>;
  /** Settings changed anywhere (tray, overlay, this window). */
  onSettings(cb: (s: AppSettings) => void): Promise<unknown>;
  version(): Promise<string>;
}

export function tauriApi(): SettingsApi {
  return {
    getSettings: bridge.getSettings,
    setSettings: bridge.setSettings,
    setStartAtLogin: bridge.setStartAtLogin,
    wakeTest: bridge.wakeTest,
    sleep: bridge.sleep,
    hooksStatus: bridge.hooksStatus,
    hooksApply: bridge.hooksApply,
    dragPermission: bridge.dragPermission,
    openDragPermission: bridge.openDragPermission,
    onDragPermission: bridge.onDragPermission,
    onSettings: bridge.onSettings,
    version: async () => (await import('@tauri-apps/api/app')).getVersion(),
  };
}

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export function mockApi(params: URLSearchParams): SettingsApi {
  const lang: UiLanguage = params.get('lang') === 'vi' ? 'vi' : 'en';
  let settings: AppSettings = { ...DEFAULT_SETTINGS, uiLanguage: lang };
  let hooks: HooksStatus = (['installed', 'needs-repair', 'not-installed'] as const).find((s) => s === params.get('hooks')) ?? 'installed';
  const fail = params.get('fail');
  let drag: DragPermission = (['missing', 'granted'] as const).find((d) => d === params.get('drag')) ?? 'not-needed';
  const dragListeners: ((p: DragPermission) => void)[] = [];
  const listeners: ((s: AppSettings) => void)[] = [];
  const publish = () => listeners.forEach((cb) => cb({ ...settings }));
  const settingsPath = 'C:\\Users\\you\\.claude\\settings.json';

  return {
    getSettings: async () => ({ ...settings }),
    setSettings: async (patch) => {
      settings = { ...settings, ...patch };
      publish();
      return { ...settings };
    },
    setStartAtLogin: async (on) => {
      await wait(250);
      if (fail === 'autostart') throw new Error('Access is denied. (os error 5)');
      settings = { ...settings, startAtLogin: on };
      publish();
      return on;
    },
    wakeTest: async () => console.info('[mock] wake test'),
    sleep: async () => console.info('[mock] sleep'),
    hooksStatus: async () => ({ status: hooks, settingsPath }),
    hooksApply: async (action) => {
      await wait(700);
      const lang = settings.uiLanguage;
      if (fail === 'hooks') {
        return { ok: false, message: t(lang, 'hooks.error.brokenJson', { detail: 'expected value at line 4 column 3' }) };
      }
      hooks = action === 'install' ? 'installed' : 'not-installed';
      const key = action === 'install' ? 'hooks.install.written' : 'hooks.uninstall.written';
      return { ok: true, message: t(lang, key, { path: settingsPath, backup: `${settingsPath}.bak.1760371200` }) };
    },
    dragPermission: async () => drag,
    openDragPermission: async () => {
      // The user grants it in System Settings a moment later.
      await wait(1200);
      drag = 'granted';
      dragListeners.forEach((cb) => cb(drag));
    },
    onDragPermission: async (cb) => dragListeners.push(cb),
    onSettings: async (cb) => listeners.push(cb),
    version: async () => '0.1.0',
  };
}
