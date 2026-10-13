// Type-checks the workspace for macOS from Windows (or Linux): `pnpm check:mac`.
//
// One dependency (objc2-exception-helper) compiles an Objective-C file with Apple's clang, which other systems
// do not have. A check never links, so scripts/fake-cc stands in for the C compiler and archiver and writes
// empty objects. This proves the Rust code compiles for macOS; it does not replace building on a Mac.
import { execFileSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const targets = process.argv.slice(2).length ? process.argv.slice(2) : ['aarch64-apple-darwin'];
const run = (cmd, args, env = {}) => execFileSync(cmd, args, { cwd: root, stdio: 'inherit', env: { ...process.env, ...env } });

if (process.platform === 'darwin') {
  run('cargo', ['clippy', '--workspace', '--all-targets', '--', '-D', 'warnings']);
  process.exit(0);
}
run('cargo', ['build', '--release', '--quiet', '--manifest-path', join('scripts', 'fake-cc', 'Cargo.toml')]);
const fake = join(root, 'scripts', 'fake-cc', 'target', 'release', process.platform === 'win32' ? 'fake-cc.exe' : 'fake-cc');
for (const target of targets) {
  run('rustup', ['target', 'add', target]);
  const key = target.replaceAll('-', '_');
  console.log(`checking ${target}`);
  run('cargo', ['clippy', '--workspace', '--all-targets', '--target', target, '--', '-D', 'warnings'], {
    [`CC_${key}`]: fake,
    [`AR_${key}`]: fake,
  });
}
