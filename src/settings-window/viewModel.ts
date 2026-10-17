// Pure logic of the Settings window: units, ranges, and what the Hooks section offers. No DOM, so it is tested
// directly (tests/settingsViewModel.test.ts).
import type { HooksStatus, IpcError } from '../bridge/tauriBridge';
import { tMessage, type UiKey } from '../i18n';
import type { AppSettings, UiLanguage } from '../settings';

export type SectionId = 'general' | 'language' | 'hooks' | 'advanced';
export const SECTIONS: readonly SectionId[] = ['general', 'language', 'hooks', 'advanced'];

/**
 * A number setting shown in a friendlier unit. Ranges match src-tauri/src/settings.rs. maxYellsPerWake and
 * staleAfterMs stay out of the window (owner's call, 2026-10-13); they can still be set in settings.json.
 */
export interface NumberField {
  key: 'yellIntervalMs';
  /** Settings value per displayed unit. */
  scale: number;
  min: number;
  max: number;
  step: number;
  unit: UiKey;
}

export const NUMBER_FIELDS: Readonly<Record<NumberField['key'], NumberField>> = {
  yellIntervalMs: { key: 'yellIntervalMs', scale: 1000, min: 5, max: 600, step: 5, unit: 'settings.unit.seconds' },
};

/** The value to display for a setting, in the field's unit. */
export function toDisplay(field: NumberField, settings: Pick<AppSettings, NumberField['key']>): number {
  return Math.round(settings[field.key] / field.scale);
}

/**
 * What to store for a typed or stepped display value: clamped into range and rounded to a whole unit.
 * Returns null for input that is not a number, so the field can snap back to the current value.
 */
export function fromDisplay(field: NumberField, input: string | number): number | null {
  const n = typeof input === 'number' ? input : Number(String(input).trim().replace(',', '.'));
  if (String(input).trim() === '' || !Number.isFinite(n)) return null;
  const clamped = Math.min(field.max, Math.max(field.min, Math.round(n)));
  return clamped * field.scale;
}

/** One step up or down from the displayed value, kept in range. */
export function stepped(field: NumberField, display: number, direction: 1 | -1): number {
  const next = display + direction * field.step;
  // Off-grid values (hand-edited files) snap to the grid on the first step.
  const snapped = direction > 0 ? Math.floor(next / field.step) * field.step : Math.ceil(next / field.step) * field.step;
  return Math.min(field.max, Math.max(field.min, snapped));
}

export interface HooksView {
  label: UiKey;
  description: UiKey;
  /** Not installed is a warning too: without hooks the cat never wakes. */
  tone: 'ok' | 'warn';
  /** The main button (install or repair); null when everything is installed. */
  primary: { action: 'install'; label: UiKey } | null;
  canUninstall: boolean;
}

export function hooksView(status: HooksStatus): HooksView {
  switch (status) {
    case 'installed':
      return {
        label: 'settings.hooks.installed',
        description: 'settings.hooks.installed.desc',
        tone: 'ok',
        primary: null,
        canUninstall: true,
      };
    case 'needs-repair':
      return {
        label: 'settings.hooks.needsRepair',
        description: 'settings.hooks.needsRepair.desc',
        tone: 'warn',
        primary: { action: 'install', label: 'settings.hooks.repair' },
        canUninstall: true,
      };
    case 'not-installed':
      return {
        label: 'settings.hooks.notInstalled',
        description: 'settings.hooks.notInstalled.desc',
        tone: 'warn',
        primary: { action: 'install', label: 'settings.hooks.install' },
        canUninstall: false,
      };
  }
}

/**
 * The sidebar dot for all agents at once. A repair anywhere comes first; otherwise one installed agent is enough,
 * since the user may want the hooks for only one of them.
 */
export function hooksSummary(statuses: readonly HooksStatus[]): HooksStatus {
  if (statuses.includes('needs-repair')) return 'needs-repair';
  return statuses.includes('installed') ? 'installed' : 'not-installed';
}

/** The folder an agent keeps its hooks file in (`~/.codex` for `~/.codex/hooks.json`), on either OS. */
export function agentFolder(filePath: string): string {
  const cut = Math.max(filePath.lastIndexOf('/'), filePath.lastIndexOf('\\'));
  return cut > 0 ? filePath.slice(0, cut) : filePath;
}

/** The section after (or before) `current`, wrapping around: arrow keys in the sidebar. */
export function nextSection(current: SectionId, direction: 1 | -1): SectionId {
  const i = SECTIONS.indexOf(current);
  return SECTIONS[(i + direction + SECTIONS.length) % SECTIONS.length];
}

/** `#hooks` in the URL opens that section; anything else opens General. */
export function sectionFromHash(hash: string): SectionId {
  const id = hash.replace(/^#/, '');
  return (SECTIONS as readonly string[]).includes(id) ? (id as SectionId) : 'general';
}

/** Whether a rejected command came from Rust's `IpcError` (anything else is a bridge or runtime failure). */
export function isIpcError(e: unknown): e is IpcError {
  if (typeof e !== 'object' || e === null || !('code' in e) || !('message' in e)) return false;
  const m = (e as { message: unknown }).message;
  return typeof m === 'object' && m !== null && 'key' in m;
}

/** What to show for a command that rejected: Rust's message for an `IpcError`, otherwise `fallback` of the reason. */
export function failureText(lang: UiLanguage, e: unknown, fallback: (reason: string) => string): string {
  return isIpcError(e) ? tMessage(lang, e.message) : fallback(String(e));
}
