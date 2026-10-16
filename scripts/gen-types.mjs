// Rewrites src/bridge/generated.ts from the Rust types (src-tauri/src/ipc/bindings.rs): `pnpm gen:types`.
// `pnpm check` runs the same test without KLAT_WRITE_BINDINGS and fails while the file is out of date.
import { execFileSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
execFileSync('cargo', ['test', '-p', 'klat', '--quiet', 'ipc::bindings'], {
  cwd: root,
  stdio: 'inherit',
  env: { ...process.env, KLAT_WRITE_BINDINGS: '1' },
});
console.log('wrote src/bridge/generated.ts');
