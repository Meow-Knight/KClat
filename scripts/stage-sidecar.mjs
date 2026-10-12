// Copies the release build of klat-hook to src-tauri/binaries/ under the target-triple name that
// Tauri's bundle.externalBin expects. The installer then places it next to klat.exe as
// klat-hook.exe, which is where the hooks installer points Claude Code.
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const host = /^host: (.+)$/m.exec(execFileSync('rustc', ['-vV'], { encoding: 'utf8' }))?.[1];
if (!host) throw new Error('could not read the host triple from `rustc -vV`');
const ext = host.includes('windows') ? '.exe' : '';
const from = join(root, 'target', 'release', `klat-hook${ext}`);
const to = join(root, 'src-tauri', 'binaries', `klat-hook-${host}${ext}`);
mkdirSync(dirname(to), { recursive: true });
copyFileSync(from, to);
console.log(`staged ${from} -> ${to}`);
