// Every quality gate in one command: `pnpm check`. The repository has no CI, so this is the bar each change
// (and each refactor phase, docs/ARCHITECTURE.md §12) must pass on the developer's machine.
//
//   pnpm check                     # all gates; output of a failed gate is printed under the table
//   pnpm check --fail-fast         # stop at the first failed gate
//   pnpm check --verbose           # stream every gate's output as it runs
//   pnpm check --mac               # also type-check the workspace for macOS (scripts/check-macos.mjs)
//   pnpm check --skip audit,clippy # leave gates out (e.g. `audit` when offline)
//
// Exits 1 when a gate fails. A gate whose tool is not installed (cargo-audit) is reported as SKIP, not FAIL.
import { spawnSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const failFast = args.includes('--fail-fast');
const verbose = args.includes('--verbose');
const skip = new Set(args.includes('--skip') ? (args[args.indexOf('--skip') + 1] ?? '').split(',') : []);

/** In order. `needs`: a command that must succeed first, or the gate is skipped (tool not installed). */
const GATES = [
  { name: 'fmt', cmd: 'cargo fmt --all --check' },
  { name: 'clippy', cmd: 'cargo clippy --workspace --all-targets -- -D warnings' },
  { name: 'test:rust', cmd: 'cargo test --workspace' },
  { name: 'lint', cmd: 'pnpm lint' },
  { name: 'typecheck', cmd: 'pnpm typecheck' },
  { name: 'test', cmd: 'pnpm test' },
  { name: 'audit', cmd: 'cargo audit', needs: 'cargo audit --version' },
  ...(args.includes('--mac') ? [{ name: 'mac', cmd: 'node scripts/check-macos.mjs' }] : []),
];

const unknown = [...skip].filter((s) => s && !GATES.some((g) => g.name === s));
if (unknown.length) {
  console.error(`unknown gate(s) for --skip: ${unknown.join(', ')} (gates: ${GATES.map((g) => g.name).join(', ')})`);
  process.exit(2);
}

// Commands are fixed strings above, so running them through the shell is safe; it lets `pnpm` resolve to
// pnpm.cmd on Windows.
const run = (cmd, stream) =>
  spawnSync(cmd, {
    cwd: root,
    shell: true,
    encoding: 'utf8',
    stdio: stream ? 'inherit' : 'pipe',
    maxBuffer: 64 * 1024 * 1024,
  });

const results = [];
for (const gate of GATES) {
  if (skip.has(gate.name)) {
    results.push({ ...gate, status: 'SKIP', note: '--skip' });
    continue;
  }
  if (gate.needs && run(gate.needs, false).status !== 0) {
    results.push({ ...gate, status: 'SKIP', note: `not installed (${gate.needs})` });
    continue;
  }
  process.stdout.write(`> ${gate.cmd}\n`);
  const started = Date.now();
  const r = run(gate.cmd, verbose);
  const ok = r.status === 0;
  results.push({ ...gate, status: ok ? 'OK' : 'FAIL', ms: Date.now() - started, output: `${r.stdout ?? ''}${r.stderr ?? ''}` });
  if (!ok && failFast) break;
}

const failed = results.filter((r) => r.status === 'FAIL');
if (!verbose) {
  for (const r of failed) {
    console.log(`\n──── ${r.name} failed: ${r.cmd} ────\n${r.output.trimEnd()}`);
  }
}

const secs = (ms) => (ms === undefined ? '' : `${(ms / 1000).toFixed(1)}s`);
const width = Math.max(...results.map((r) => r.name.length));
console.log(`\n ${'gate'.padEnd(width)}  result    time  note`);
for (const r of results) {
  console.log(` ${r.name.padEnd(width)}  ${r.status.padEnd(6)}  ${secs(r.ms).padStart(6)}  ${r.note ?? ''}`);
}
const total = results.reduce((sum, r) => sum + (r.ms ?? 0), 0);
console.log(`\n${failed.length ? `FAILED: ${failed.map((r) => r.name).join(', ')}` : 'all gates passed'} (${secs(total)})`);
process.exit(failed.length ? 1 : 0);
