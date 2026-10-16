// Behaviour checks for the architecture refactor (docs/ARCHITECTURE.md §11, beads epic ClauCat-dv6): starts the
// real app (debug build), feeds it hook events through the real klat-hook, and reads what the app decided over
// its own commands. Windows only (it drives the overlay page over WebView2 remote debugging).
//
//   pnpm regress
//
// Isolation: the app runs with APPDATA pointed at a temp folder, so your settings.json, log and runtime.json are
// not touched, and the Claude Code hooks you have installed (which read the real runtime.json) cannot reach it.
// It never presses Install/Uninstall hooks or Start at login. Quit an installed Klat first (tray > Quit): a
// second Klat hands over to the running one and exits.
//
// What it cannot see (tray, Space-drag, DPI changes, the animations) is the owner's part of the regression
// script in docs/verification.md.
import { execFileSync, spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { connect } from 'node:net';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const CDP_PORT = 9333;
const VITE = 'http://localhost:1420';

if (process.platform !== 'win32') {
  console.error('pnpm regress drives WebView2 and runs on Windows only.');
  process.exit(2);
}
if (!globalThis.WebSocket) {
  // Node 20 has WebSocket behind a flag (on by default from Node 22).
  const r = spawnSync(process.execPath, ['--experimental-websocket', ...process.argv.slice(1)], { stdio: 'inherit' });
  process.exit(r.status ?? 1);
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const running = (image) =>
  spawnSync('tasklist', ['/FI', `IMAGENAME eq ${image}`, '/NH'], { encoding: 'utf8' }).stdout.includes(image);

if (running('klat.exe')) {
  console.error('Klat is running. Quit it first (tray > Quit): a second Klat would hand over to it and exit.');
  process.exit(2);
}

// ── Start: build, Vite, the app ─────────────────────────────────────────────────────────────────────────────
console.error('> cargo build -p klat -p klat-hook');
execFileSync('cargo', ['build', '-p', 'klat', '-p', 'klat-hook'], { cwd: root, stdio: ['ignore', 'ignore', 'inherit'] });
const appExe = join(root, 'target', 'debug', 'klat.exe');
const hookExe = join(root, 'target', 'debug', 'klat-hook.exe');

const appData = mkdtempSync(join(tmpdir(), 'klat-regress-'));
const dataDir = join(appData, 'Klat');
const children = [];
const stop = () => {
  for (const c of children) spawnSync('taskkill', ['/PID', String(c.pid), '/T', '/F'], { stdio: 'ignore' });
};
process.on('exit', () => {
  stop();
  // The killed app may still hold its log open for a moment.
  rmSync(appData, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
});
process.on('SIGINT', () => process.exit(130));
// Whatever hangs (a page that never loads, a command that never answers), the app is stopped and the run fails.
setTimeout(() => {
  console.error('pnpm regress: gave up after 5 minutes');
  process.exit(1);
}, 5 * 60_000).unref();

async function until(what, fn, ms = 60_000) {
  const end = Date.now() + ms;
  for (;;) {
    try {
      const v = await fn();
      if (v) return v;
    } catch {
      // not up yet
    }
    if (Date.now() > end) throw new Error(`timed out waiting for ${what}`);
    await sleep(200);
  }
}

const viteUp = () => fetch(VITE).then((r) => r.ok);
if (!(await viteUp().catch(() => false))) {
  console.error('> pnpm dev (Vite)');
  children.push(spawn('pnpm dev', { cwd: root, shell: true, stdio: 'ignore' }));
  await until('Vite on :1420', viteUp);
}
console.error('> klat.exe (debug, APPDATA in a temp folder, remote debugging on)');
children.push(
  spawn(appExe, [], {
    cwd: root,
    stdio: 'ignore',
    env: { ...process.env, APPDATA: appData, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${CDP_PORT}` },
  }),
);
const runtime = await until('runtime.json', () => {
  const p = join(dataDir, 'runtime.json');
  return existsSync(p) && JSON.parse(readFileSync(p, 'utf8'));
});
const target = await until('the overlay page over CDP', async () => {
  const pages = await (await fetch(`http://127.0.0.1:${CDP_PORT}/json`)).json();
  return pages.find((p) => p.type === 'page' && !p.url.includes('settings'));
});

// ── Drivers ─────────────────────────────────────────────────────────────────────────────────────────────────
const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  ws.onopen = resolve;
  ws.onerror = reject;
});
let nextId = 0;
const pending = new Map();
ws.onmessage = (m) => {
  const msg = JSON.parse(m.data);
  pending.get(msg.id)?.(msg);
  pending.delete(msg.id);
};
async function evaluate(expression) {
  const id = ++nextId;
  const msg = await new Promise((resolve) => {
    pending.set(id, resolve);
    ws.send(JSON.stringify({ id, method: 'Runtime.evaluate', params: { expression, awaitPromise: true, returnByValue: true } }));
  });
  const r = msg.result;
  if (msg.error || r.exceptionDetails) throw new Error(`${expression}: ${JSON.stringify(msg.error ?? r.exceptionDetails)}`);
  return r.result.value;
}
await until('Tauri IPC in the page', () => evaluate('!!window.__TAURI_INTERNALS__'));
const invoke = (cmd, args = {}) => evaluate(`window.__TAURI_INTERNALS__.invoke(${JSON.stringify(cmd)}, ${JSON.stringify(args)})`);
const state = () => invoke('get_state');

/** Runs the real klat-hook with Claude Code's stdin shape, against this app. */
function hook(name, sessionId, extra = {}) {
  const input = JSON.stringify({
    session_id: sessionId,
    hook_event_name: name,
    cwd: `C:\\work\\project-${sessionId}`,
    transcript_path: join(appData, `${sessionId}.jsonl`),
    ...extra,
  });
  spawnSync(hookExe, [name], { input, env: { ...process.env, APPDATA: appData }, stdio: ['pipe', 'ignore', 'ignore'] });
}

/** Raw request to the event server; returns the status code (0 if the connection gave none). */
function raw(request) {
  return new Promise((resolve) => {
    const s = connect(runtime.port, '127.0.0.1', () => s.end(request));
    let data = '';
    s.on('data', (d) => (data += d));
    s.on('close', () => resolve(Number(/^HTTP\/1\.1 (\d{3})/.exec(data)?.[1] ?? 0)));
    s.on('error', () => resolve(0));
  });
}
const post = (headers, body) =>
  raw(`POST /event HTTP/1.1\r\nHost: 127.0.0.1\r\n${headers}Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`);

// ── Checks ──────────────────────────────────────────────────────────────────────────────────────────────────
const results = [];
async function check(name, fn) {
  try {
    const detail = await fn();
    results.push({ name, ok: true, detail: detail ?? '' });
  } catch (e) {
    results.push({ name, ok: false, detail: e.message });
  }
}
function expect(cond, msg) {
  if (!cond) throw new Error(msg);
}
const summary = (s) => `awake=${s.awake} reason=${s.reason} sessions=${s.sessions.length} seq=${s.attention?.seq ?? '-'} ready=${s.ready} cause=${s.sleepCause}`;

const STATE_KEYS = ['attention', 'awake', 'ready', 'reason', 'sessions', 'sleepCause'];
const SETTINGS_KEYS = [
  'catScale', 'character', 'corner', 'hideWhileSleeping', 'klatLanguage', 'maxYellsPerWake', 'notificationsEnabled',
  'position', 'reduceMotion', 'sfwMode', 'staleAfterMs', 'startAtLogin', 'uiLanguage', 'yellIntervalMs',
];

await check('At launch: asleep, Ready armed', async () => {
  const s = await state();
  expect(!s.awake && s.ready && s.sessions.length === 0, summary(s));
  return summary(s);
});
await check('Wire format: cat://state and settings keys unchanged', async () => {
  const s = Object.keys(await state()).sort();
  const t = Object.keys(await invoke('get_settings')).sort();
  expect(JSON.stringify(s) === JSON.stringify(STATE_KEYS), `state keys ${s}`);
  expect(JSON.stringify(t) === JSON.stringify(SETTINGS_KEYS), `settings keys ${t}`);
  return `${s.length} state keys, ${t.length} settings keys`;
});
await check('Stop with Ready armed wakes (done), project name from cwd', async () => {
  hook('Stop', 'sa');
  const s = await state();
  expect(s.awake && s.reason === 'done' && s.attention?.seq === 1, summary(s));
  expect(s.attention.projectName === 'project-sa' && s.sessions[0]?.projectName === 'project-sa', JSON.stringify(s.attention));
  return summary(s);
});
await check('Same Stop again within 2s is a duplicate (no new yell)', async () => {
  hook('Stop', 'sa');
  const s = await state();
  expect(s.awake && s.attention?.seq === 1, summary(s));
  return summary(s);
});
await check('UserPromptSubmit clears it: asleep, cause user', async () => {
  hook('UserPromptSubmit', 'sa');
  const s = await state();
  expect(!s.awake && s.sessions.length === 0 && s.sleepCause === 'user', summary(s));
  return summary(s);
});
await check('Ready off: a finished turn does not wake', async () => {
  await invoke('set_ready', { on: false });
  hook('Stop', 'sb');
  const s = await state();
  expect(!s.awake && !s.ready, summary(s));
  return summary(s);
});
await check('PermissionRequest always wakes (permission)', async () => {
  hook('PermissionRequest', 'sc', { tool_name: 'Bash', tool_input: { command: 'echo secret' } });
  const s = await state();
  expect(s.awake && s.reason === 'permission', summary(s));
  return summary(s);
});
await check('Notification permission_prompt on a waiting session is ignored', async () => {
  const before = (await state()).attention?.seq;
  hook('Notification', 'sc', { notification_type: 'permission_prompt', message: 'x' });
  const s = await state();
  expect(s.awake && s.attention?.seq === before, `${summary(s)} (seq before ${before})`);
  return summary(s);
});
await check('PostToolUse of that tool clears the permission wait', async () => {
  hook('PostToolUse', 'sc', { tool_name: 'Bash', tool_input: { command: 'echo' }, tool_response: {} });
  const s = await state();
  expect(!s.awake, summary(s));
  return summary(s);
});
await check('Notification idle_prompt wakes (input); SessionEnd clears', async () => {
  hook('Notification', 'sd', { notification_type: 'idle_prompt', message: 'x' });
  const a = await state();
  expect(a.awake && a.reason === 'input', summary(a));
  hook('SessionEnd', 'sd', { reason: 'exit' });
  const b = await state();
  expect(!b.awake && b.sessions.length === 0, summary(b));
  return `${summary(a)} → ${summary(b)}`;
});
await check('Pause holds a permission wait; resume wakes (cause auto); Dismiss sleeps (cause user)', async () => {
  await invoke('set_settings', { patch: { notificationsEnabled: false } });
  hook('PermissionRequest', 'se', { tool_name: 'Edit' });
  const paused = await state();
  expect(!paused.awake, `paused: ${summary(paused)}`);
  await invoke('set_settings', { patch: { notificationsEnabled: true } });
  const resumed = await state();
  expect(resumed.awake && resumed.reason === 'permission' && resumed.sleepCause === 'auto', `resumed: ${summary(resumed)}`);
  await invoke('dismiss');
  const dismissed = await state();
  expect(!dismissed.awake && dismissed.sleepCause === 'user', `dismissed: ${summary(dismissed)}`);
  return `${summary(paused)} → ${summary(resumed)} → ${summary(dismissed)}`;
});
await check('Wake test wakes; Sleep puts it back', async () => {
  await invoke('wake_test');
  const a = await state();
  expect(a.awake && a.reason === 'done', summary(a));
  await invoke('sleep');
  const b = await state();
  expect(!b.awake, summary(b));
  return `${summary(a)} → ${summary(b)}`;
});
await check('Settings: patch saved to settings.json; non-patchable keys ignored', async () => {
  const before = await invoke('get_settings');
  const after = await invoke('set_settings', { patch: { uiLanguage: 'vi', position: { x: 1, y: 2 }, startAtLogin: !before.startAtLogin } });
  const file = JSON.parse(readFileSync(join(dataDir, 'settings.json'), 'utf8'));
  expect(after.uiLanguage === 'vi' && file.uiLanguage === 'vi', `uiLanguage ${after.uiLanguage} / file ${file.uiLanguage}`);
  expect(JSON.stringify(after.position) === JSON.stringify(before.position), `position changed to ${JSON.stringify(after.position)}`);
  expect(after.startAtLogin === before.startAtLogin, 'startAtLogin changed through set_settings');
  await invoke('set_settings', { patch: { uiLanguage: before.uiLanguage } });
  return 'uiLanguage vi saved, position and startAtLogin untouched';
});
await check('Cat size 150%: the overlay zooms by 1.5, back at 100%', async () => {
  const dpr0 = await evaluate('window.devicePixelRatio');
  await invoke('set_settings', { patch: { catScale: 150 } });
  const dpr1 = await until('zoom', async () => {
    const d = await evaluate('window.devicePixelRatio');
    return Math.abs(d / dpr0 - 1.5) < 0.02 && d;
  }, 5000);
  await invoke('set_settings', { patch: { catScale: 100 } });
  const dpr2 = await until('zoom back', async () => {
    const d = await evaluate('window.devicePixelRatio');
    return Math.abs(d - dpr0) < 0.02 && d;
  }, 5000);
  return `devicePixelRatio ${dpr0} → ${dpr1} → ${dpr2}`;
});
await check('Event server rejects: wrong token 401, Origin 403, GET 405, unknown hook 422', async () => {
  const body = JSON.stringify({ eventId: '1', hookEventName: 'Stop', sessionId: 'x', occurredAt: 1 });
  const auth = `Authorization: Bearer ${runtime.token}\r\n`;
  const got = {
    wrongToken: await post(`Authorization: Bearer ${'0'.repeat(64)}\r\n`, body),
    origin: await post(`${auth}Origin: https://evil.example\r\n`, body),
    get: await raw(`GET /event HTTP/1.1\r\n${auth}\r\n`),
    unknownHook: await post(auth, body.replace('Stop', 'PreToolUse')),
  };
  expect(got.wrongToken === 401 && got.origin === 403 && got.get === 405 && got.unknownHook === 422, JSON.stringify(got));
  const s = await state();
  expect(!s.awake, `a rejected event changed the state: ${summary(s)}`);
  return JSON.stringify(got);
});
await check('Log: hook lines carry names only, rejections are logged', async () => {
  const log = readFileSync(join(dataDir, 'klat.log'), 'utf8');
  expect(log.includes('hook PermissionRequest session sc'), 'no hook line for session sc');
  expect(log.includes('rejected a request to the event endpoint: HTTP 401'), 'no rejection line');
  expect(!log.includes('echo secret'), 'tool input reached the log');
  return `${log.split('\n').filter(Boolean).length} lines`;
});

ws.close();

// ── Report ──────────────────────────────────────────────────────────────────────────────────────────────────
const commit = spawnSync('git', ['rev-parse', '--short', 'HEAD'], { cwd: root, encoding: 'utf8' }).stdout.trim();
const dirty = spawnSync('git', ['status', '--porcelain'], { cwd: root, encoding: 'utf8' }).stdout.trim() ? ' + uncommitted changes' : '';
console.log(`\nRegression run ${new Date().toISOString().slice(0, 16).replace('T', ' ')} UTC at \`${commit}\`${dirty}, debug build.\n`);
console.log('| Check | Result | Detail |\n|---|---|---|');
for (const r of results) console.log(`| ${r.name} | ${r.ok ? '✅' : '❌'} | ${r.detail.replaceAll('|', '\\|')} |`);
const failed = results.filter((r) => !r.ok).length;
console.log(`\n${failed ? `${failed} of ${results.length} checks FAILED` : `all ${results.length} checks passed`}`);
// Explicit: the app and Vite keep the event loop alive, and the exit handler is what stops them.
process.exit(failed ? 1 : 0);
