// Numbers the architecture refactor (docs/ARCHITECTURE.md §11, beads epic ClauCat-dv6) is checked against:
// `node scripts/measure.mjs`. Prints a Markdown table to paste into docs/verification.md.
//
//   node scripts/measure.mjs            # everything (builds klat-hook in release mode first)
//   node scripts/measure.mjs --runs 100 # hook runs per case (default 50)
//
// - klat-hook: binary size, and wall time per run (spawn included) in three cases: the app is not running (no
//   runtime.json), runtime.json points at a closed port (the connect timeout), and a stand-in app answers 204.
//   The hook reads its data folder from APPDATA / HOME / XDG_DATA_HOME, which point at a temp folder here, so the
//   real Klat is never touched.
// - Tests: how many Rust and TypeScript tests pass.
// - Edit-test loop: after touching the session logic, how long `cargo test -p <crate>` takes (rebuild + run).
//   Measured on whichever crate holds that logic today (src-tauri until Phase 2, klat-core after it).
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const RUNS = args.includes('--runs') ? Number(args[args.indexOf('--runs') + 1]) : 50;
const LOOP_RUNS = 3;
const TOKEN = 'a'.repeat(64);
const STDIN = JSON.stringify({
  session_id: 'measure-0000-0000-0000-000000000000',
  hook_event_name: 'Stop',
  cwd: join(root),
  transcript_path: join(root, 'transcript.jsonl'),
  stop_hook_active: false,
});

const sh = (cmd) => spawnSync(cmd, { cwd: root, shell: true, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
const ms = (ns) => Number(ns) / 1e6;
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];
const p95 = (xs) => [...xs].sort((a, b) => a - b)[Math.min(xs.length - 1, Math.ceil(xs.length * 0.95) - 1)];
const fmt = (x) => `${x.toFixed(1)} ms`;

function must(cmd) {
  console.error(`> ${cmd}`);
  const r = sh(cmd);
  if (r.status !== 0) {
    console.error(`${r.stdout}${r.stderr}`);
    throw new Error(`failed: ${cmd}`);
  }
  return `${r.stdout}${r.stderr}`;
}

// ── klat-hook ───────────────────────────────────────────────────────────────────────────────────────────────
must('cargo build --release -p klat-hook');
const hook = join(root, 'target', 'release', process.platform === 'win32' ? 'klat-hook.exe' : 'klat-hook');
const hookBytes = statSync(hook).size;

// The env the hook reads its data folder from (crates/klat-hook data_dir), pointed at a temp folder.
const base = mkdtempSync(join(tmpdir(), 'klat-measure-'));
const env = { ...process.env };
let dataDir;
if (process.platform === 'win32') {
  env.APPDATA = base;
  dataDir = join(base, 'Klat');
} else if (process.platform === 'darwin') {
  env.HOME = base;
  dataDir = join(base, 'Library', 'Application Support', 'Klat');
} else {
  env.XDG_DATA_HOME = base;
  dataDir = join(base, 'Klat');
}
const runtime = join(dataDir, 'runtime.json');
const writeRuntime = (port) => {
  mkdirSync(dataDir, { recursive: true });
  writeFileSync(runtime, JSON.stringify({ port, token: TOKEN, pid: process.pid }));
};

/** One hook run: wall time in ms, spawn to exit. Async, so a stand-in server in this process can answer. */
function runHook() {
  return new Promise((resolve, reject) => {
    const started = process.hrtime.bigint();
    const child = spawn(hook, ['Stop'], { env, stdio: ['pipe', 'ignore', 'ignore'] });
    child.on('error', reject);
    child.on('exit', () => resolve(ms(process.hrtime.bigint() - started)));
    child.stdin.end(STDIN);
  });
}

async function timeHook(label) {
  await runHook(); // warm the file cache
  const times = [];
  for (let i = 0; i < RUNS; i++) times.push(await runHook());
  console.error(`  ${label}: median ${fmt(median(times))}`);
  return times;
}

const listen = (handler) =>
  new Promise((resolve) => {
    const server = createServer(handler);
    server.listen(0, '127.0.0.1', () => resolve(server));
  });

console.error(`timing klat-hook, ${RUNS} runs per case`);
const noApp = await timeHook('no runtime.json');

const closed = await listen(() => {});
const closedPort = closed.address().port;
await new Promise((r) => closed.close(r));
writeRuntime(closedPort);
const closedPortTimes = await timeHook('closed port');

let received = 0;
const app = await listen((socket) => {
  socket.once('data', () => {
    received += 1;
    socket.end('HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n');
  });
  socket.on('error', () => {});
});
writeRuntime(app.address().port);
const answered = await timeHook('stand-in app answers 204');
await new Promise((r) => app.close(r));
rmSync(base, { recursive: true, force: true });
if (received < RUNS + 1) throw new Error(`the stand-in app got ${received} of ${RUNS + 1} requests`);

// ── Tests ───────────────────────────────────────────────────────────────────────────────────────────────────
console.error('counting tests');
const rustOut = must('cargo test --workspace');
const sum = (re) => [...rustOut.matchAll(re)].reduce((n, m) => n + Number(m[1]), 0);
const rustPassed = sum(/test result: ok\. (\d+) passed/g);
const rustIgnored = sum(/(\d+) ignored/g);
const tsOut = must('pnpm test');
const tsPassed = Number(/Tests\s+(\d+) passed/.exec(tsOut)?.[1] ?? NaN);

// ── Edit-test loop ──────────────────────────────────────────────────────────────────────────────────────────
const core = join(root, 'crates', 'klat-core', 'src', 'session.rs');
const [loopFile, loopCrate] = existsSync(core)
  ? [core, 'klat-core']
  : [join(root, 'src-tauri', 'src', 'session_store.rs'), 'klat'];
const loopCmd = `cargo test -p ${loopCrate}`;
must(loopCmd); // up to date before timing
console.error(`edit-test loop: touch ${loopFile.slice(root.length + 1)}, ${loopCmd} (${LOOP_RUNS} runs)`);
const loops = [];
for (let i = 0; i < LOOP_RUNS; i++) {
  const now = new Date();
  utimesSync(loopFile, now, now);
  const started = process.hrtime.bigint();
  must(loopCmd);
  loops.push(ms(process.hrtime.bigint() - started));
}

// ── Report ──────────────────────────────────────────────────────────────────────────────────────────────────
const rustc = /rustc (\S+)/.exec(sh('rustc --version').stdout)?.[1] ?? '?';
const commit = sh('git rev-parse --short HEAD').stdout.trim();
const dirty = sh('git status --porcelain').stdout.trim() ? ' + uncommitted changes' : '';
const row = (name, times) => `| klat-hook, ${name} | ${fmt(median(times))} | ${fmt(p95(times))} |`;
console.log(`
Measured ${new Date().toISOString().slice(0, 10)} at \`${commit}\`${dirty}, ${process.platform}-${process.arch}, rustc ${rustc}, ${RUNS} hook runs per case (wall time including process start).

| What | Median | p95 |
|---|---|---|
${row('app not running (no runtime.json)', noApp)}
${row('runtime.json points at a closed port', closedPortTimes)}
${row('app answers 204', answered)}
| Edit-test loop: touch \`${loopFile.slice(root.length + 1).replaceAll('\\', '/')}\`, \`${loopCmd}\` | ${fmt(median(loops))} | (${LOOP_RUNS} runs) |

| What | Value |
|---|---|
| klat-hook release binary | ${(hookBytes / 1024).toFixed(1)} KB (${hookBytes} bytes) |
| Rust tests passed (\`cargo test --workspace\`) | ${rustPassed} (${rustIgnored} ignored) |
| TypeScript tests passed (\`pnpm test\`) | ${tsPassed} |
`);
