import js from '@eslint/js';
import globals from 'globals';
import tseslint from 'typescript-eslint';

// Frontend layers (docs/ARCHITECTURE.md §4.3), inner to outer. A folder imports only from its own layer or an inner
// one. bridge/generated.ts holds only types generated from Rust, so every layer may import it.
const LAYERS = [
  ['settings', 'i18n', 'phrases'],
  ['kit'],
  ['characters'],
  ['cat', 'bubble'],
  ['bridge'],
  ['overlay', 'settings-window'],
];
const layerOf = (name) => LAYERS.findIndex((names) => names.includes(name));
/** A top-level folder (or settings.ts) as an import path names it, after the `../`s that lead back to src/. */
const target = (name) => (name === 'bridge' ? 'bridge/(?!generated(\\.ts)?$)' : `${name}(/|\\.ts$|$)`);

/** no-restricted-imports for the files of `dir` that sit `depth` folders below src/ (0: src/settings.ts). */
function layerRule(dir, depth) {
  const layer = layerOf(dir);
  const up = depth === 0 ? '\\./' : '(\\.\\./)'.repeat(depth);
  const patterns = LAYERS.flat()
    .filter((name) => layerOf(name) > layer)
    .map((name) => ({
      regex: `^${up}${target(name)}`,
      message: `${dir} is layer ${layer}; ${name} is layer ${layerOf(name)} (docs/ARCHITECTURE.md §4.3).`,
    }));
  if (dir === 'cat' || dir === 'bubble') {
    patterns.push({
      regex: `^${up}characters/[^/]+/`,
      message: 'Get a rig through the registry (characters/index.ts), not a character file (docs/ARCHITECTURE.md §4.3).',
    });
  }
  if (dir !== 'bridge') {
    patterns.push({ regex: '^@tauri-apps/', message: 'Only src/bridge/ talks to Tauri (docs/ARCHITECTURE.md §4.3).' });
  }
  if (layer === 0) {
    patterns.push({ regex: '^three$', message: 'Layer 0 is plain data and functions: no Three.js (docs/ARCHITECTURE.md §4.3).' });
  }
  const files =
    depth === 0 ? [`src/${dir}.ts`] : [`src/${dir}/${'*/'.repeat(depth - 1)}*.ts`];
  return { files, rules: { 'no-restricted-imports': ['error', { patterns }] } };
}

const layerRules = LAYERS.flat().flatMap((dir) => (dir === 'settings' ? [layerRule(dir, 0)] : [1, 2, 3].map((d) => layerRule(dir, d))));

export default tseslint.config(
  { ignores: ['dist', 'target', 'src-tauri/target', 'src-tauri/gen', 'docs/prototype', 'scripts/fake-cc/target'] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    languageOptions: { globals: { ...globals.browser } },
  },
  {
    files: ['*.config.{js,ts}', 'scripts/**/*.mjs'],
    languageOptions: { globals: { ...globals.node } },
  },
  ...layerRules,
);
