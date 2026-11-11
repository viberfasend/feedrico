import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { fileURLToPath } from 'node:url';

// Tauri expects a fixed dev port and serves the built files from dist/.
const host = process.env.TAURI_DEV_HOST;

const slashes = (path: string) => path.replaceAll('\\', '/');
/** Agent worktrees (`.claude/worktrees/`) are full checkouts inside this one. */
const agentCheckouts = slashes(fileURLToPath(new URL('./.claude/', import.meta.url)));
/**
 * True for files of the agent checkouts inside this one. Anchored to this checkout, so a dev
 * server started inside a worktree still watches its own files (a glob for any `.claude` folder
 * would ignore them all).
 */
export const isAgentCheckout = (path: string) => slashes(path).startsWith(agentCheckouts);

export default defineConfig(({ command }) => ({
  // Dev server: CSS ships inside each component's JS. As a separate virtual module it could be
  // requested before its component was compiled, and Vite then cached the raw .svelte source as
  // CSS (see src/lib/dev-server.test.ts). Builds still emit one CSS file.
  plugins: [svelte({ emitCss: command === 'build' })],
  // Tests need Svelte's client runtime, or $effect never runs. Rune tests also opt into the
  // happy-dom environment so the Svelte plugin compiles them for the client.
  resolve: process.env.VITEST ? { conditions: ['browser'] } : undefined,
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    watch: { ignored: ['**/src-tauri/**', '**/crates/**', '**/target/**', isAgentCheckout] },
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
}));
