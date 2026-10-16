// Typed wrapper over the Tauri events and commands the overlay and the Settings window use (see
// src-tauri/src/ipc/).
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  CatState,
  DragPermission,
  GrabMode,
  HooksAction,
  HooksInfo,
  HooksResult,
  Point,
  Rect,
  Settings,
  SettingsPatch,
  ZoomChange,
} from './generated';

// Every type that crosses IPC is generated from Rust (./generated.ts); these names are what the windows import.
export type * from './generated';

/** Cursor in the page's CSS px; null = on another monitor (the cat then looks at its bubble). */
export type CursorPayload = Point | null;

export const bridge = {
  getState: (): Promise<CatState> => invoke('get_state'),
  getSettings: (): Promise<Settings> => invoke('get_settings'),
  dismiss: (): Promise<void> => invoke('dismiss'),
  setReady: (on: boolean): Promise<void> => invoke('set_ready', { on }),
  setHitboxes: (rects: Rect[]): Promise<void> => invoke('set_hitboxes', { rects }),
  /** Returns the settings now in effect (out-of-range values clamped). */
  setSettings: (patch: SettingsPatch): Promise<Settings> => invoke('set_settings', { patch }),
  /** Size slider being dragged: shows `scale` (percent) on the overlay without saving it. */
  previewCatScale: (scale: number): Promise<void> => invoke('preview_cat_scale', { scale }),
  /** Overlay only, with the cat hidden: window to the preview size (true) or back to the cat size (false). */
  overlayPreviewZoom: (preview: boolean): Promise<ZoomChange> => invoke('overlay_preview_zoom', { preview }),
  /** Resolves to the new state; rejects with the OS reason, leaving the setting unchanged. */
  setStartAtLogin: (on: boolean): Promise<boolean> => invoke('set_start_at_login', { on }),
  wakeTest: (): Promise<void> => invoke('wake_test'),
  sleep: (): Promise<void> => invoke('sleep'),
  hooksStatus: (): Promise<HooksInfo> => invoke('hooks_status'),
  hooksApply: (action: HooksAction): Promise<HooksResult> => invoke('hooks_apply', { action }),
  dragPermission: (): Promise<DragPermission> => invoke('drag_permission'),
  openDragPermission: (): Promise<void> => invoke('open_drag_permission'),
  onDragPermission: (cb: (p: DragPermission) => void): Promise<UnlistenFn> =>
    listen<DragPermission>('cat://drag-permission', (e) => cb(e.payload)),
  onState: (cb: (s: CatState) => void): Promise<UnlistenFn> => listen<CatState>('cat://state', (e) => cb(e.payload)),
  onSettings: (cb: (s: Settings) => void): Promise<UnlistenFn> => listen<Settings>('cat://settings', (e) => cb(e.payload)),
  /** 30Hz while the cat is awake, only when the cursor moved (see src-tauri/src/tasks/cursor_poller.rs). */
  onCursor: (cb: (p: CursorPayload) => void): Promise<UnlistenFn> => listen<CursorPayload>('cat://cursor', (e) => cb(e.payload)),
  /** Size slider preview: the cat's scale relative to the page, or null when the drag is over. */
  onScalePreview: (cb: (factor: number | null) => void): Promise<UnlistenFn> =>
    listen<number | null>('cat://scale-preview', (e) => cb(e.payload)),
  onGrab: (cb: (mode: GrabMode) => void): Promise<UnlistenFn> => listen<GrabMode>('cat://grab', (e) => cb(e.payload)),
};
