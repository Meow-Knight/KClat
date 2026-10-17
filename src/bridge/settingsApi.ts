// What the Settings window needs from the app, with two implementations: the real one over Tauri, and a mock
// for `pnpm dev` at /settings.html?mock (design review without the app). Mock options in the URL:
// `lang=vi`, `hooks=needs-repair|not-installed` (Claude Code), `codex=installed|needs-repair|missing` (Codex; not
// installed by default, `missing` = no ~/.codex), `fail=autostart|hooks`, `drag=missing|granted` (macOS).
import {
  bridge,
  type DragPermission,
  type HooksAction,
  type HooksAgent,
  type HooksInfo,
  type HooksStatus,
  type IpcError,
  type Message,
  type SettingsPatch,
} from './tauriBridge';
import { DEFAULT_SETTINGS, type AppSettings, type UiLanguage } from '../settings';

export interface SettingsApi {
  getSettings(): Promise<AppSettings>;
  setSettings(patch: SettingsPatch): Promise<AppSettings>;
  /** Size slider being dragged: show the size on the cat without saving it; `setSettings` ends the preview. */
  previewCatScale(scale: number): Promise<void>;
  setStartAtLogin(on: boolean): Promise<boolean>;
  wakeTest(): Promise<void>;
  sleep(): Promise<void>;
  /** Every agent, in the order the window lists them. */
  hooksStatus(): Promise<HooksInfo[]>;
  /** Installs or removes one agent's hooks. Resolves to what was done; rejects with an IpcError saying why nothing was. */
  hooksApply(agent: HooksAgent, action: HooksAction): Promise<Message>;
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
    previewCatScale: bridge.previewCatScale,
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
  const statusParam = (name: string, fallback: HooksStatus): HooksStatus =>
    (['installed', 'needs-repair', 'not-installed'] as const).find((s) => s === params.get(name)) ?? fallback;
  const hooks: Record<HooksAgent, HooksStatus> = {
    'claude-code': statusParam('hooks', 'installed'),
    codex: statusParam('codex', 'not-installed'),
  };
  const codexFound = params.get('codex') !== 'missing';
  const fail = params.get('fail');
  let drag: DragPermission = (['missing', 'granted'] as const).find((d) => d === params.get('drag')) ?? 'not-needed';
  const dragListeners: ((p: DragPermission) => void)[] = [];
  const listeners: ((s: AppSettings) => void)[] = [];
  const publish = () => listeners.forEach((cb) => cb({ ...settings }));
  const files: Record<HooksAgent, string> = {
    'claude-code': 'C:\\Users\\you\\.claude\\settings.json',
    codex: 'C:\\Users\\you\\.codex\\hooks.json',
  };

  return {
    getSettings: async () => ({ ...settings }),
    setSettings: async (patch) => {
      settings = { ...settings, ...patch };
      publish();
      return { ...settings };
    },
    previewCatScale: async () => {},
    setStartAtLogin: async (on) => {
      await wait(250);
      if (fail === 'autostart') {
        const e: IpcError = {
          code: 'start-at-login',
          message: { key: 'settings.general.startAtLogin.error', vars: { error: 'Access is denied. (os error 5)' } },
        };
        throw e;
      }
      settings = { ...settings, startAtLogin: on };
      publish();
      return on;
    },
    wakeTest: async () => console.info('[mock] wake test'),
    sleep: async () => console.info('[mock] sleep'),
    hooksStatus: async () =>
      (['claude-code', 'codex'] as const).map((agent) => ({
        agent,
        status: hooks[agent],
        filePath: files[agent],
        agentFound: agent === 'claude-code' || codexFound,
      })),
    hooksApply: async (agent, action) => {
      await wait(700);
      if (fail === 'hooks') {
        const e: IpcError = {
          code: 'hooks-broken-json',
          message: { key: 'hooks.error.brokenJson', vars: { detail: 'expected value at line 4 column 3', file: 'settings.json' } },
        };
        throw e;
      }
      hooks[agent] = action === 'install' ? 'installed' : 'not-installed';
      const install = agent === 'codex' ? 'hooks.codex.install.written' : 'hooks.install.written';
      const key = action === 'install' ? install : 'hooks.uninstall.written';
      return { key, vars: { path: files[agent], backup: `${files[agent]}.bak.1760371200` } };
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
