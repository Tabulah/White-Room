import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';

// Tauri expects a fixed dev-server port and must be able to see Rust errors.
// See https://v2.tauri.app/start/frontend/vite/
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [svelte(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    watch: {
      // The Rust backend is rebuilt by the Tauri CLI, not Vite.
      ignored: ['**/src-tauri/**'],
    },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: {
    // Tauri uses WebKit (Safari) on macOS.
    target: 'safari15',
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  test: {
    include: ['src/**/*.test.ts'],
    environment: 'node',
    // Vitest blanks CSS imports by default; the design-token tests read this
    // file's source (`tokens.css?raw`).
    css: { include: [/\/src\/lib\/design\/tokens\.css/] },
  },
});
