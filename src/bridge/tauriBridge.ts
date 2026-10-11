// Typed wrapper over the Tauri events and commands the overlay uses (see src-tauri/src/commands.rs, state.rs).
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
}

/** Cursor in logical px relative to the overlay's top-left corner; null = on another monitor. */
export type CursorPayload = { x: number; y: number } | null;

/** Logical px relative to the overlay's top-left corner. */
export interface HitRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export const bridge = {
  getState: (): Promise<CatStatePayload> => invoke('get_state'),
  getSettings: (): Promise<AppSettings> => invoke('get_settings'),
  dismiss: (): Promise<void> => invoke('dismiss'),
  setHitboxes: (rects: HitRect[]): Promise<void> => invoke('set_hitboxes', { rects }),
  onState: (cb: (s: CatStatePayload) => void): Promise<UnlistenFn> => listen<CatStatePayload>('cat://state', (e) => cb(e.payload)),
  onSettings: (cb: (s: AppSettings) => void): Promise<UnlistenFn> => listen<AppSettings>('cat://settings', (e) => cb(e.payload)),
  /** 30Hz while the cat is awake, only when the cursor moved (see src-tauri/src/cursor.rs). */
  onCursor: (cb: (p: CursorPayload) => void): Promise<UnlistenFn> => listen<CursorPayload>('cat://cursor', (e) => cb(e.payload)),
};
