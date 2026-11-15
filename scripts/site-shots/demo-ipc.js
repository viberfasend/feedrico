// Demo data for the website's screenshots: the real Svelte UI with the Tauri IPC replaced by
// made-up Feeds and Articles (no real publisher, no real photo). Injected by shoot.mjs into the
// dev server's page; `window.__demo = { layout, theme }` picks the Layout and the theme.
const demo = window.__demo ?? { layout: 'list', theme: 'light' };
const now = Math.floor(Date.now() / 1000);
const today9 = Math.floor(new Date().setHours(9, 14, 0, 0) / 1000);

const FEEDS = [
  ['The Garden Letter', 150], ['Slow Kitchen', 28], ['Night Sky Notes', 232], ['Trail Journal', 95],
  ['The Bike Shed', 200], ['Letters from Lisbon', 12], ['Old Maps Quarterly', 45],
].map(([title, hue], i) => ({
  id: i + 1, title, hue, siteUrl: null, unread: [6, 3, 3, 2, 1, 0, 1][i], lastError: null, lastSuccessAt: now,
  favicon: `fav-${hue}`,
}));

// [feed, title, excerpt, scene or null]
const ARTICLES = [
  [1, 'What to plant before the first frost', 'Garlic, broad beans and a handful of bulbs: a short list for a slow autumn weekend in the garden.', 'field'],
  [2, 'A plum cake that forgives everything', 'Too ripe, too tart, a little bruised. This one-bowl cake takes whatever the tree gives you.', 'plum'],
  [3, 'Five things to spot in the sky this week', 'Saturn is back in the evening, the Pleiades are rising, and there is a thin crescent moon on Thursday.', 'night'],
  [4, 'The ridge walk, one season later', 'Same path, different light. The bracken has turned copper and the huts are quiet again.', 'ridge'],
  [5, 'How to true a wheel with a zip tie', 'No stand, no gauge: a cheap trick that gets a wobbly wheel straight enough for the ride home.', null],
  [6, 'The tram that goes nowhere in particular', 'Line 28 on a weekday morning, before the queues: a letter about taking the long way round on purpose.', 'city'],
  [2, 'Stock from scraps, step by step', 'Onion skins, carrot tops and a parmesan rind. An hour on the stove, and the freezer does the rest.', 'ember'],
  [1, 'Saving seeds from this year’s tomatoes', 'Pick the best fruit, find a jar of water, and give it three days of patience.', 'moss'],
  [4, 'A weekend pack that weighs almost nothing', 'What stays home matters more than what comes along. A list, with the weights.', 'dawn'],
  [5, 'Winter tyres: are they worth it?', 'A season of commuting on both kinds, and an honest answer at the end of it.', 'frost'],
  [7, 'The coastline nobody could agree on', 'Three surveys, three shapes. How a stretch of shore kept moving on paper for two hundred years.', 'sea'],
  [3, 'A beginner’s guide to the winter triangle', 'Three bright stars, one easy shape, and the best hour to look for it.', 'dusk'],
  [6, 'Sunday, with pastries', 'On queueing for the good ones, and the bench where you eat them.', null],
  [1, 'Leaf mould: the lazy gardener’s gold', 'A wire cage, a pile of leaves, and a year of doing nothing at all.', 'leaf'],
  [2, 'Bread for people without time', 'Stir it at night, bake it in the morning. No kneading, no fuss.', 'bread'],
  [4, 'Notes from a foggy summit', 'There was no view. It was one of the best days of the year anyway.', 'fog'],
];

// Flat illustrated scenes: a sky, a light, two ranges of hills. Drawn here, so nothing is borrowed.
const SCENES = {
  field: ['#f6d9a0', '#f2b65e', '#fff2cf', '#7d8a4e', '#56633a'], plum: ['#f3c9c2', '#b9627a', '#ffe6d6', '#6b2d4e', '#44203a'],
  night: ['#1e1a3c', '#3b3470', '#f5e9c4', '#17142d', '#0e0c1f'], ridge: ['#f4c58a', '#d9793f', '#fff0d0', '#8a4b2b', '#5a3020'],
  city: ['#f7d7b0', '#e79a6a', '#fff3da', '#a9604a', '#6e3d3a'], ember: ['#f0a561', '#c9542f', '#ffe2b0', '#7a2d1e', '#4a1c16'],
  moss: ['#dfe6c2', '#a9bd7a', '#fbf7df', '#5d7a44', '#3c5430'], dawn: ['#fbe3c7', '#f2a98a', '#ffffff', '#8c7aa6', '#5c4f7d'],
  frost: ['#e6eef2', '#b5c9d6', '#ffffff', '#7f97a8', '#566b7c'], sea: ['#cfe6ea', '#7fb7c4', '#fff8e0', '#2f7186', '#1e4d60'],
  dusk: ['#3a2f66', '#b86a7a', '#ffd9a0', '#2a2250', '#1a1535'], leaf: ['#f3d9a6', '#d99a3e', '#fff1cc', '#9a5a22', '#6a3c18'],
  bread: ['#f5e2c0', '#dcae6e', '#fff6e2', '#a8743c', '#7a5228'], fog: ['#e9e6e0', '#c9c4bc', '#f8f6f1', '#a39d94', '#7d776f'],
};
const scene = (name) => {
  const [a, b, sun, near, far] = SCENES[name];
  // The light sits somewhere else in every scene, so a grid of them doesn't look stamped.
  const x = 380 + ((name.charCodeAt(0) * 7 + name.length * 131) % 840);
  const y = 300 + ((name.charCodeAt(1) * 13) % 200);
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="1000" viewBox="0 0 1600 1000">
<defs><linearGradient id="s" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${a}"/><stop offset="1" stop-color="${b}"/></linearGradient>
<radialGradient id="h"><stop offset="0" stop-color="${sun}" stop-opacity=".7"/><stop offset="1" stop-color="${sun}" stop-opacity="0"/></radialGradient></defs>
<rect width="1600" height="1000" fill="url(#s)"/><circle cx="${x}" cy="${y}" r="360" fill="url(#h)"/><circle cx="${x}" cy="${y}" r="78" fill="${sun}"/>
<path d="M0 700C260 560 420 640 640 600S1040 480 1240 580s260 60 360 20V1000H0z" fill="${near}" opacity=".75"/>
<path d="M0 820C200 740 380 800 620 770s420-90 640-40 240 30 340 10V1000H0z" fill="${far}"/></svg>`;
  return 'data:image/svg+xml,' + encodeURIComponent(svg);
};
const favicon = (hue) =>
  'data:image/svg+xml,' + encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><rect width="16" height="16" rx="4" fill="hsl(${hue} 45% 42%)"/><circle cx="10.5" cy="5.5" r="2.2" fill="hsl(${hue} 80% 85%)"/></svg>`,
  );
const media = (key) => (key.startsWith('fav-') ? favicon(key.slice(4)) : scene(key.slice(4)));

const row = (i) => {
  const [feedId, title, excerpt, image] = ARTICLES[i];
  const feed = FEEDS[feedId - 1];
  return {
    id: i + 1, feedId, feedTitle: feed.title, title, excerpt,
    articleDate: i < 5 ? today9 - i * 1380 : today9 - 86400 - i * 2400,
    isRead: i > 4 && i !== 8, isStarred: i === 3, leadImage: image && `img-${image}`, favicon: feed.favicon,
  };
};
const BODY = `<p>The beds look finished by now, but the soil is still warm, and a few things would much rather go in today than wait for spring. Garlic first: split a bulb, push the cloves in a thumb deep, and forget about them until June.</p>
<p>Broad beans are next. Sown now, they sit small and stubborn through the winter and are a month ahead of everything else when the light comes back. Cover them if the pigeons are watching.</p>
<h2>And then the bulbs</h2>
<p>A handful of crocuses under the apple tree costs ten minutes, and in February you will be glad you spent them. Tulips can wait until the first cold week; they like it that way.</p>
<p>That is the whole list. The rest of the weekend belongs to tea, and to looking at the garden without doing anything to it.</p>`;
const library = { path: '/demo', name: 'Feedrico Library', formatVersion: 3, feedCount: FEEDS.length, portable: false };

const handlers = {
  startup: () => ({ kind: 'opened', library }),
  library_info: () => library,
  layouts: () => ({ default: demo.layout, views: {} }),
  set_layout: () => null,
  settings: () => ({
    appearance: { theme: demo.theme, density: 'comfortable', unreadCounts: 'show' },
    reader: { fontFamily: 'serif', fontSize: 18, measure: 66, lineHeight: 1.65 },
    refresh: { intervalMinutes: 30 },
    imageCacheCapBytes: 1024 ** 3, pruneReadDays: 90, pruneUnreadDays: 365,
  }),
  set_settings: () => null,
  has_setting: () => true,
  last_view: () => null,
  set_last_view: () => null,
  sidebar: () => ({
    allUnread: 16, starred: 1, today: 5,
    folders: [
      { id: 1, name: 'Home', unread: 9, feeds: FEEDS.slice(0, 2) },
      { id: 2, name: 'Outdoors', unread: 6, feeds: FEEDS.slice(2, 5) },
    ],
    feeds: FEEDS.slice(5),
  }),
  count_articles: () => ARTICLES.length,
  list_articles: ({ offset, limit }) =>
    ARTICLES.slice(offset, offset + limit).map((_, i) => row(offset + i)),
  get_article: ({ id }) => ({
    ...row(id - 1), feedSiteUrl: 'https://example.com', url: 'https://example.com/article',
    author: 'Marta Lindqvist', contentHtml: BODY, enclosures: [],
  }),
  set_read: () => null,
  refresh: () => false,
  'plugin:event|listen': () => 1,
  'plugin:app|version': () => '0.4.0',
  check_for_updates: () => ({ kind: 'upToDate', current: '0.4.0' }),
  library_size: () => 1234567,
  media_size: () => 65432,
  autostart_enabled: () => false,
  assistant_setup_get: () => null,
  assistant_detect: () => [],
};

window.__TAURI_INTERNALS__ = {
  invoke: async (cmd, args) => {
    const handler = handlers[cmd];
    if (!handler) {
      console.warn('[demo-ipc] unhandled', cmd);
      return null;
    }
    return handler(args || {});
  },
  transformCallback: (cb) => {
    const id = (Math.random() * 1e9) | 0;
    window[`_${id}`] = cb;
    return id;
  },
  convertFileSrc: (path, protocol) => (protocol === 'media' ? media(path) : `${protocol}://localhost/${path}`),
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
window.__demoApp = mount(App, { target: document.getElementById('app') });
