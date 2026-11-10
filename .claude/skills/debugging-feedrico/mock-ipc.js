// Renders the real Svelte UI from the running dev server (http://localhost:1420) in a normal
// browser, with the Tauri IPC replaced by canned data. Lets an agent *see* layout and styling
// (screenshots, DOM, computed styles) without the WebKitGTK window, which agents can't capture
// on GNOME Wayland.
//
// Use: open http://localhost:1420/ in the browser (the UI first fails to start: no Tauri), then
// run this whole file as one script in the page (e.g. Claude in Chrome's javascript tool).
// It installs the mock and remounts <App/>. Edit `handlers` to shape the data you need.
//
// Limits: Chrome is not WebKitGTK. A glitch that only shows in the app may be engine-specific
// (see "Known traps") or dev-server state (see "UI glitch that a restart fixes").

const now = Math.floor(Date.now() / 1000);
const feeds = Array.from({ length: 6 }, (_, i) => ({
  id: i + 1, title: `Feed ${i + 1}`, siteUrl: null, unread: 5, lastError: null, lastSuccessAt: now,
}));
const TOTAL = 40;
const rows = (offset, limit) =>
  Array.from({ length: Math.max(0, Math.min(limit, TOTAL - offset)) }, (_, i) => ({
    id: offset + i + 1, feedId: 1, feedTitle: 'Feed 1',
    title: `Article ${offset + i + 1} with a reasonably long headline`,
    excerpt: 'Lorem ipsum dolor sit amet', articleDate: now - i * 600,
    isRead: false, isStarred: false, leadImage: null,
  }));
const library = { path: '/mock', name: 'Mock Library', formatVersion: 3, feedCount: feeds.length, portable: false };

const handlers = {
  startup: () => ({ kind: 'opened', library }),
  library_info: () => library,
  layouts: () => ({ default: 'list', views: {} }),
  set_layout: () => null,
  sidebar: () => ({
    allUnread: 30, starred: 0, today: 3,
    folders: [{ id: 1, name: 'Tech', unread: 15, feeds: feeds.slice(0, 3) }],
    feeds: feeds.slice(3),
  }),
  count_articles: () => TOTAL,
  list_articles: ({ offset, limit }) => rows(offset, limit),
  refresh: () => false,
  'plugin:event|listen': () => 1,
};

window.__TAURI_INTERNALS__ = {
  invoke: async (cmd, args) => {
    const handler = handlers[cmd];
    if (!handler) {
      console.warn('[mock-ipc] unhandled', cmd, args);
      return null;
    }
    return handler(args || {});
  },
  transformCallback: (cb) => {
    const id = (Math.random() * 1e9) | 0;
    window[`_${id}`] = cb;
    return id;
  },
  convertFileSrc: (path, protocol) => `${protocol}://localhost/${path}`,
  metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
};
window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };

// Reuse the exact module URLs the page loaded, so there is one Svelte runtime and one app state.
const loaded = performance.getEntriesByType('resource').map((e) => e.name);
const svelteUrl = loaded.find((u) => /\.vite\/deps\/svelte\.js/.test(u));
const appUrl = loaded.find((u) => /\/src\/App\.svelte/.test(u));
const mainUrl = loaded.find((u) => /\/src\/main\.ts/.test(u)) ?? '/src/main.ts';
const { mount, unmount } = await import(svelteUrl);
const { default: App } = await import(appUrl);
const main = await import(mainUrl);
await unmount(main.default);
window.__mockApp = mount(App, { target: document.getElementById('app') });
'mock-ipc: mounted';
