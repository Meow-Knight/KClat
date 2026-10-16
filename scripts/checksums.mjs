// Writes SHA256SUMS.txt next to each release bundle (the NSIS installer, the .dmg), so whoever downloads Klat can
// check the file is the one that was built. Klat is not code-signed yet, so this is the only integrity check
// users have: publish SHA256SUMS.txt alongside the files.
//
//   node scripts/checksums.mjs        # after `pnpm release` or `pnpm release:mac`
//
// Check a download:  Windows  Get-FileHash Klat_<version>_x64-setup.exe -Algorithm SHA256
//                    macOS    shasum -a 256 Klat_<version>_universal.dmg
import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
// Only this product's files: the bundle folders keep older builds (e.g. from before the rename to Klat).
const { productName } = JSON.parse(readFileSync(join(root, 'src-tauri', 'tauri.conf.json'), 'utf8'));
const bundles = [
  { dir: join(root, 'target', 'release', 'bundle', 'nsis'), ext: '.exe' },
  { dir: join(root, 'target', 'universal-apple-darwin', 'release', 'bundle', 'dmg'), ext: '.dmg' },
  { dir: join(root, 'target', 'release', 'bundle', 'dmg'), ext: '.dmg' },
];

let found = 0;
for (const { dir, ext } of bundles) {
  if (!existsSync(dir)) continue;
  const files = readdirSync(dir)
    .filter((f) => f.startsWith(`${productName}_`) && f.endsWith(ext))
    .sort();
  if (files.length === 0) continue;
  const lines = files.map((f) => `${createHash('sha256').update(readFileSync(join(dir, f))).digest('hex')}  ${f}`);
  const out = join(dir, 'SHA256SUMS.txt');
  writeFileSync(out, lines.join('\n') + '\n');
  console.log(`${relative(root, out)}\n  ${lines.join('\n  ')}`);
  found += files.length;
}
if (found === 0) {
  console.error('no release bundles found: run `pnpm release` (or `pnpm release:mac`) first');
  process.exit(1);
}
