// Builds klat-hook in release mode and copies it to src-tauri/binaries/ under the target-triple name that
// Tauri's bundle.externalBin expects. The bundle then places it next to the app's binary (klat-hook.exe next
// to klat.exe on Windows, Klat.app/Contents/MacOS/klat-hook on macOS), which is where the hooks installer
// points Claude Code.
//
//   node scripts/stage-sidecar.mjs                                   # the target being built, else this machine
//   node scripts/stage-sidecar.mjs --target universal-apple-darwin   # Apple Silicon + Intel, joined with lipo
//   node scripts/stage-sidecar.mjs --target universal-apple-darwin --dry-run   # print the steps only
//
// Without --target it uses TAURI_ENV_TARGET_TRIPLE, which `tauri build` sets for its beforeBuildCommand.
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const UNIVERSAL = 'universal-apple-darwin';
const UNIVERSAL_PARTS = ['aarch64-apple-darwin', 'x86_64-apple-darwin'];

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const dryRun = args.includes('--dry-run');
const targetArg = args.includes('--target') ? args[args.indexOf('--target') + 1] : undefined;

const host = /^host: (.+)$/m.exec(execFileSync('rustc', ['-vV'], { encoding: 'utf8' }))?.[1];
if (!host) throw new Error('could not read the host triple from `rustc -vV`');
const target = targetArg || process.env.TAURI_ENV_TARGET_TRIPLE || host;

const ext = target.includes('windows') ? '.exe' : '';
const to = join(root, 'src-tauri', 'binaries', `klat-hook-${target}${ext}`);
const show = (p) => relative(root, p);

function run(cmd, cmdArgs) {
  console.log(`> ${cmd} ${cmdArgs.join(' ')}`);
  if (!dryRun) execFileSync(cmd, cmdArgs, { cwd: root, stdio: 'inherit' });
}

/** Release build of klat-hook for `triple`; returns the path of the binary. */
function build(triple) {
  // The host build goes to target/release, as it always has (the Windows installer depends on nothing else).
  if (triple === host) {
    run('cargo', ['build', '-p', 'klat-hook', '--release']);
    return join(root, 'target', 'release', `klat-hook${ext}`);
  }
  run('rustup', ['target', 'add', triple]);
  run('cargo', ['build', '-p', 'klat-hook', '--release', '--target', triple]);
  return join(root, 'target', triple, 'release', `klat-hook${ext}`);
}

if (!dryRun) mkdirSync(dirname(to), { recursive: true });
if (target === UNIVERSAL) {
  if (process.platform !== 'darwin' && !dryRun) {
    throw new Error(`${UNIVERSAL} needs a Mac (Apple's linker and lipo). Use --dry-run to see the steps.`);
  }
  const parts = UNIVERSAL_PARTS.map(build);
  run('lipo', ['-create', '-output', to, ...parts]);
} else {
  const from = build(target);
  console.log(`> copy ${show(from)} -> ${show(to)}`);
  if (!dryRun) copyFileSync(from, to);
}
console.log(`${dryRun ? 'would stage' : 'staged'} ${show(to)}`);
