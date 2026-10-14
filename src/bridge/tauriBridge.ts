// Typed wrapper over the Tauri events and commands the overlay and the Settings window use (see
// src-tauri/src/commands.rs, state.rs).
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { WakeReason } from '../phrases/PhrasePicker';
import type { AppSettings } from '../settings';

export interface SessionView {
  projectName: string;
  reason: WakeReason;
}

/** The latest "a session needs you" moment; `seq` changes each time (see src-tauri/src/state.rs). */
export interface Attention {
  seq: number;
  projectName: string;
  reason: WakeReason;
}

export interface CatStatePayload {
  awake: boolean;
  reason: WakeReason | null;
  /** Sessions currently calling for you, oldest first. */
  sessions: SessionView[];
  attention: Attention | null;
  /** Ready is armed: finished turns wake the cat until you turn it off. */
  ready: boolean;
  /** Why the latest change happened: 'user' (you answered or dismissed) or 'auto' (expiry, Pause). */
  sleepCause: 'user' | 'auto';
}

/** Cursor in logical px relative to the overlay's top-left corner; null = on another monitor. */
export type CursorPayload = { x: number; y: number } | null;

/** Moving the cat with Space (see src-tauri/src/drag.rs): Space held over the cat, then the mouse pressed. */
export type GrabMode = 'idle' | 'armed' | 'dragging';

/** Logical px relative to the overlay's top-left corner. */
export interface HitRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Claude Code hooks, as src-tauri/src/hooks_installer.rs reports them. */
export type HooksStatus = 'installed' | 'needs-repair' | 'not-installed';

export interface HooksInfo {
  status: HooksStatus;
  /** `~/.claude/settings.json`; null when the home folder is unknown. */
  settingsPath: string | null;
}

export interface HooksResult {
  ok: boolean;
  /** Text to show the user, success or error. */
  message: string;
}

/** Whether dragging the cat needs a permission first (macOS: Accessibility). See src-tauri/src/drag.rs. */
export type DragPermission = 'not-needed' | 'granted' | 'missing';

/** What the Settings window may change (src-tauri/src/settings.rs `PATCHABLE`); Rust validates it again. */
export type SettingsPatch = Partial<
  Pick<
    AppSettings,
    | 'notificationsEnabled'
    | 'hideWhileSleeping'
    | 'sfwMode'
    | 'uiLanguage'
    | 'klatLanguage'
    | 'yellIntervalMs'
    | 'maxYellsPerWake'
    | 'staleAfterMs'
    | 'reduceMotion'
    | 'catScale'
  >
>;

export const bridge = {
  getState: (): Promise<CatStatePayload> => invoke('get_state'),
  getSettings: (): Promise<AppSettings> => invoke('get_settings'),
  dismiss: (): Promise<void> => invoke('dismiss'),
  setReady: (on: boolean): Promise<void> => invoke('set_ready', { on }),
  setHitboxes: (rects: HitRect[]): Promise<void> => invoke('set_hitboxes', { rects }),
  /** Returns the settings now in effect (out-of-range values clamped). */
  setSettings: (patch: SettingsPatch): Promise<AppSettings> => invoke('set_settings', { patch }),
  /** Resolves to the new state; rejects with the OS reason, leaving the setting unchanged. */
  setStartAtLogin: (on: boolean): Promise<boolean> => invoke('set_start_at_login', { on }),
  wakeTest: (): Promise<void> => invoke('wake_test'),
  sleep: (): Promise<void> => invoke('sleep'),
  hooksStatus: (): Promise<HooksInfo> => invoke('hooks_status'),
  hooksApply: (action: 'install' | 'uninstall'): Promise<HooksResult> => invoke('hooks_apply', { action }),
  dragPermission: (): Promise<DragPermission> => invoke('drag_permission'),
  openDragPermission: (): Promise<void> => invoke('open_drag_permission'),
  onDragPermission: (cb: (p: DragPermission) => void): Promise<UnlistenFn> =>
    listen<DragPermission>('cat://drag-permission', (e) => cb(e.payload)),
  onState: (cb: (s: CatStatePayload) => void): Promise<UnlistenFn> => listen<CatStatePayload>('cat://state', (e) => cb(e.payload)),
  onSettings: (cb: (s: AppSettings) => void): Promise<UnlistenFn> => listen<AppSettings>('cat://settings', (e) => cb(e.payload)),
  /** 30Hz while the cat is awake, only when the cursor moved (see src-tauri/src/cursor.rs). */
  onCursor: (cb: (p: CursorPayload) => void): Promise<UnlistenFn> => listen<CursorPayload>('cat://cursor', (e) => cb(e.payload)),
  onGrab: (cb: (mode: GrabMode) => void): Promise<UnlistenFn> => listen<GrabMode>('cat://grab', (e) => cb(e.payload)),
};
