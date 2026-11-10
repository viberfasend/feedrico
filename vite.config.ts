import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Tauri expects a fixed dev port and serves the built files from dist/.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [svelte()],
  // Tests need Svelte's client runtime, or $effect never runs. Rune tests also opt into the
  // happy-dom environment so the Svelte plugin compiles them for the client.
  resolve: process.env.VITEST ? { conditions: ['browser'] } : undefined,
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    // Agent worktrees (.claude/worktrees/) are full checkouts inside the root: never watch them.
    watch: { ignored: ['**/src-tauri/**', '**/crates/**', '**/target/**', '**/.claude/**'] },
  },
  // Vite would otherwise scan every **/*.html under the root, including other checkouts' code.
  optimizeDeps: { entries: ['index.html'] },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    // Tauri webviews: WebView2 (Chromium) on Windows, WebKit on macOS and Linux.
    target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome120' : 'safari16',
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  test: {
    include: ['scripts/**/*.test.mjs', 'src/**/*.test.ts'],
  },
});
