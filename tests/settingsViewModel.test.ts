import { describe, expect, it } from 'vitest';
import {
  NUMBER_FIELDS,
  failureText,
  fromDisplay,
  hooksView,
  isIpcError,
  nextSection,
  sectionFromHash,
  stepped,
  toDisplay,
} from '../src/settings-window/viewModel';
import type { IpcError, Message } from '../src/bridge/generated';
import { tMessage } from '../src/i18n';
import fixture from './fixtures/hooksMessages.json';

const yell = NUMBER_FIELDS.yellIntervalMs;
const settings = { yellIntervalMs: 30_000 };

describe('settings window view model', () => {
  it('shows the yell interval in seconds', () => {
    expect(toDisplay(yell, settings)).toBe(30);
    expect(toDisplay(yell, { yellIntervalMs: 7_400 })).toBe(7);
  });

  it('offers only the yell interval among the numbers', () => {
    expect(Object.keys(NUMBER_FIELDS)).toEqual(['yellIntervalMs']);
  });

  it('turns typed values back into settings values, clamped and whole', () => {
    expect(fromDisplay(yell, '45')).toBe(45_000);
    expect(fromDisplay(yell, ' 7.6 ')).toBe(8_000);
    expect(fromDisplay(yell, '7,4')).toBe(7_000);
    expect(fromDisplay(yell, '1')).toBe(5_000);
    expect(fromDisplay(yell, '9999')).toBe(600_000);
    expect(fromDisplay(yell, 0)).toBe(5_000);
  });

  it('rejects input that is not a number', () => {
    expect(fromDisplay(yell, '')).toBeNull();
    expect(fromDisplay(yell, 'abc')).toBeNull();
    expect(fromDisplay(yell, '  ')).toBeNull();
  });

  it('steps on the grid and stays in range', () => {
    expect(stepped(yell, 30, 1)).toBe(35);
    expect(stepped(yell, 30, -1)).toBe(25);
    expect(stepped(yell, 5, -1)).toBe(5);
    expect(stepped(yell, 600, 1)).toBe(600);
    // An off-grid value from a hand-edited file snaps to the grid.
    expect(stepped(yell, 7, 1)).toBe(10);
    expect(stepped(yell, 7, -1)).toBe(5);
  });

  it('offers install, repair or nothing depending on the hooks status', () => {
    expect(hooksView('installed')).toMatchObject({ tone: 'ok', primary: null, canUninstall: true });
    expect(hooksView('needs-repair').primary).toEqual({ action: 'install', label: 'settings.hooks.repair' });
    expect(hooksView('needs-repair').tone).toBe('warn');
    expect(hooksView('not-installed')).toMatchObject({
      tone: 'warn',
      primary: { action: 'install', label: 'settings.hooks.install' },
      canUninstall: false,
    });
  });

  it('moves between sections with wrap-around', () => {
    expect(nextSection('general', 1)).toBe('language');
    expect(nextSection('advanced', 1)).toBe('general');
    expect(nextSection('general', -1)).toBe('advanced');
  });

  it('opens the section named in the URL hash', () => {
    expect(sectionFromHash('#hooks')).toBe('hooks');
    expect(sectionFromHash('')).toBe('general');
    expect(sectionFromHash('#nope')).toBe('general');
  });
});

// What Install/Uninstall said before Rust stopped sending display text (captured from the Rust texts). Rust's side
// of the same file checks that it still sends these messages (src-tauri/src/ipc/commands/hooks.rs).
describe('hooks messages', () => {
  for (const c of fixture.cases) {
    it(`${c.case} reads as before in English and Vietnamese`, () => {
      const message = c.message as Message;
      if (c.ok) {
        expect(tMessage('en', message)).toBe(c.en);
        expect(tMessage('vi', message)).toBe(c.vi);
      } else {
        const e = { code: c.code, message } as IpcError;
        expect(isIpcError(e)).toBe(true);
        expect(failureText('en', e, () => 'not this')).toBe(c.en);
        expect(failureText('vi', e, () => 'not this')).toBe(c.vi);
      }
    });
  }

  it('anything else that rejects falls back to its reason', () => {
    expect(isIpcError(new Error('boom'))).toBe(false);
    expect(failureText('en', new Error('boom'), (reason) => `failed: ${reason}`)).toBe('failed: Error: boom');
  });
});
