import { defineConfig } from 'vitest/config';

// Tauri expects a fixed dev port and must not have Vite clear its Rust output.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    watch: { ignored: ['**/src-tauri/**'] },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    target: 'chrome105',
    // The overlay (index.html) and the Settings window; dev.html is served by `pnpm dev` only.
    rolldownOptions: {
      input: { overlay: 'index.html', settings: 'settings.html' },
    },
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  test: {
    include: ['tests/**/*.test.ts'],
    environment: 'node',
  },
});
