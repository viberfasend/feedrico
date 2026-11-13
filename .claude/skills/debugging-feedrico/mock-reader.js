// Reader scenario mock: like mock-ipc.js, but each Article is a real-world reader scenario:
// WordPress post (featured image, emoji image, figure, share icons, tracking pixel), photo set
// (portrait/landscape), webcomic (tall image + title text, wide strip), summary-only news with a
// Lead Image, podcast (audio + cover art enclosures), technical post (huge screenshot,
// transparent diagram, inline icon, code, table), images missing from the cache, embed card +
// linked image, a very long German title without URL, a title-only Article and a photo
// enclosure. Use it for every change to the reader (both themes, List and a grid Layout).
//
// Run it like mock-ipc.js. Set `window.__mockLayout = 'list'` before running for the List
// Layout (default: Cards). It imports the App module URL the page first loaded, so reload the
// page after every source change: otherwise it remounts the stale module.
const now = Math.floor(Date.now() / 1000);

// Synthetic images as SVG data URIs with real intrinsic sizes.
const photo = (w, h, hue, label = '') =>
  'data:image/svg+xml,' + encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="hsl(${hue} 60% 55%)"/><stop offset="1" stop-color="hsl(${hue + 40} 50% 25%)"/></linearGradient></defs><rect width="${w}" height="${h}" fill="url(#g)"/><circle cx="${w * 0.7}" cy="${h * 0.3}" r="${Math.min(w, h) * 0.12}" fill="hsl(45 90% 75%)"/><path d="M0 ${h} L${w * 0.35} ${h * 0.55} L${w * 0.6} ${h * 0.8} L${w * 0.8} ${h * 0.6} L${w} ${h} Z" fill="hsl(${hue + 80} 30% 18%)"/><text x="16" y="36" font-family="sans-serif" font-size="24" fill="white">${label} ${w}×${h}</text></svg>`,
  );
const diagram = (w, h) =>
  'data:image/svg+xml,' + encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}"><g fill="none" stroke="black" stroke-width="3"><rect x="20" y="40" width="160" height="80"/><rect x="${w - 180}" y="40" width="160" height="80"/><path d="M180 80 H${w - 180}"/></g><text x="40" y="88" font-family="sans-serif" font-size="22" fill="black">Client</text><text x="${w - 160}" y="88" font-family="sans-serif" font-size="22" fill="black">Server</text><text x="20" y="${h - 20}" font-family="sans-serif" font-size="16" fill="black">transparent PNG-style diagram (black on transparent)</text></svg>`,
  );
const icon = (w, h, hue) =>
  'data:image/svg+xml,' + encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 16 16"><rect width="16" height="16" rx="3" fill="hsl(${hue} 70% 45%)"/><circle cx="8" cy="8" r="4" fill="white"/></svg>`,
  );
const emoji = icon(72, 72, 45);
const pixel = icon(1, 1, 0);

const IMG = {
  wpHero: photo(1200, 675, 20, 'featured'),
  wpFig: photo(800, 533, 200, 'figure'),
  share1: icon(24, 24, 220), share2: icon(24, 24, 10),
  p1: photo(1600, 1067, 30, 'p1'), p2: photo(1067, 1600, 90, 'p2 portrait'), p3: photo(1600, 1067, 150, 'p3'),
  p4: photo(1200, 1200, 210, 'p4 square'), p5: photo(1067, 1600, 270, 'p5 portrait'), p6: photo(1600, 900, 330, 'p6'),
  comicTall: photo(740, 2400, 50, 'tall comic'), comicStrip: photo(1800, 520, 0, 'strip'),
  newsThumb: photo(1280, 720, 340, 'media:thumbnail'),
  podcastArt: photo(1400, 1400, 280, 'cover art'),
  shot: photo(2400, 1500, 180, 'screenshot'),
  diag: diagram(900, 300),
  smallPhoto: photo(320, 200, 120, 'small'),
  linked: photo(1000, 600, 60, 'linked'),
  flickr: photo(2048, 1365, 100, 'enclosure'),
};
const img = (k, attrs = '') => `<img src="${IMG[k]}" ${attrs}>`;
const remote = (url, attrs = '') => `<img src="media://localhost/0000missing" data-remote="${url}" ${attrs}>`;
const lorem = (n = 1) => Array.from({ length: n }, () =>
  '<p>Late summer light lies low across the fields; the threads of the gossamer drift between the stalks and catch on everything. The morning is cool, the afternoon warm, and the evenings come earlier each week than you expect.</p>').join('');

const FEEDS = [
  ['WordPress Garden Blog', 'https://garden.example'],
  ['Lens & Light Photo', 'https://lens.example'],
  ['Inkwell Comics', 'https://inkwell.example'],
  ['The Daily Ledger — World News and Analysis From Our Correspondents Everywhere', 'https://ledger.example'],
  ['Night Shift Podcast', 'https://nightshift.example'],
  ['Systems Notes', 'https://systems.example'],
];
const feeds = FEEDS.map(([title, siteUrl], i) => ({ id: i + 1, title, siteUrl, unread: 2, lastError: null, lastSuccessAt: now, favicon: i % 2 ? null : `ico${i}` }));

const A = [
  { feed: 1, title: 'Planting the autumn bed: a step-by-step guide', author: 'Ruth Gärtner', lead: 'wpHero', url: 'https://garden.example/autumn-bed',
    html: `<p>${img('wpHero', 'width="1200" height="675" alt="Raised bed in morning light"')}</p>${lorem(2)}<p>We were thrilled ${img.call(null, 'share1').replace(IMG.share1, emoji).replace('<img', '<img class="emoji" alt="😀" width="72" height="72"')} with how it turned out.</p><figure>${img('wpFig', 'width="800" height="533" alt="Bulbs laid out"')}<figcaption>Tulip and crocus bulbs, sorted by planting depth. Photo: Ruth Gärtner</figcaption></figure>${lorem(2)}<p>Share this: <a href="https://facebook.com/share">${img('share1', 'width="24" height="24" alt="Facebook"')}</a> <a href="https://twitter.com/share">${img('share2', 'width="24" height="24" alt="Twitter"')}</a></p><p>The post <a href="https://garden.example/autumn-bed">Planting the autumn bed</a> appeared first on <a href="https://garden.example">WordPress Garden Blog</a>.</p>${img.call(null, 'share1').replace(IMG.share1, pixel).replace('<img', '<img width="1" height="1" alt=""')}` },
  { feed: 2, title: 'Six frames from the Altweibersommer', author: null, lead: 'p1', url: 'https://lens.example/six',
    html: `${img('p1')}${img('p2')}${img('p3')}${img('p4')}${img('p5')}${img('p6')}` },
  { feed: 3, title: 'Gossamer #412', author: null, lead: 'comicTall', url: 'https://inkwell.example/412',
    html: `<p>${img('comicTall', 'title="The spider insists the web is load-bearing. The engineer is not convinced." alt="Gossamer #412"')}</p><p>${img('comicStrip', 'title="Bonus strip: three panels, one joke." alt="Bonus strip"')}</p>` },
  { feed: 4, title: 'Harvest festivals return to the valley after a three-year pause as communities rebuild', author: 'Ledger Staff', lead: 'newsThumb', url: 'https://ledger.example/harvest',
    html: `<p>Towns across the valley are reviving their autumn festivals this weekend. Read the full story on our website.</p>` },
  { feed: 5, title: 'Episode 88: Listening to the dark', author: 'Mara & Tom', lead: 'podcastArt', url: 'https://nightshift.example/88',
    html: `<p>This week we talk to a sound recordist who spends her nights in forests.</p><ul><li>00:00 Intro</li><li>04:12 Field recording kit</li><li>31:40 Listener mail</li></ul>`,
    enclosures: [{ url: 'https://cdn.nightshift.example/audio/ep88-listening-to-the-dark.mp3?ref=rss&utm=feed', mimeType: 'audio/mpeg', length: 58_400_000 }, { url: 'https://nightshift.example/art/88.jpg', mimeType: 'image/jpeg', length: 412_000 }] },
  { feed: 6, title: 'How request coalescing works', author: null, lead: 'shot', url: 'https://systems.example/coalescing',
    html: `${lorem(1)}<p>Here is the dashboard: ${img('shot', 'width="2400" height="1500" alt="Dashboard screenshot"')}</p>${lorem(1)}<p>The flow looks like this:</p><p>${img('diag', 'alt="Client/server diagram"')}</p><p>Look for the ${img('share1', 'width="16" height="16" alt="gear"')} icon in the toolbar, then press <code>Ctrl+R</code>.</p><pre><code>fn coalesce(reqs: &amp;[Req]) -&gt; Vec&lt;Batch&gt; { todo!() }</code></pre><table><tr><th>p50</th><th>p99</th></tr><tr><td>4 ms</td><td>38 ms</td></tr></table><p>${img('smallPhoto', 'width="320" height="200" alt="Small photo"')}</p>${lorem(1)}` },
  { feed: 2, title: 'Offline: images that never made it into the cache', author: null, lead: null, url: 'https://lens.example/missing',
    html: `${lorem(1)}${remote('https://lens.example/huge-panorama.jpg', 'alt="Panorama over the valley"')}${lorem(1)}${remote('https://lens.example/other.jpg')}<p><a href="https://lens.example/gallery">${remote('https://lens.example/linked.jpg', 'alt="Linked image"')}</a></p>` },
  { feed: 6, title: 'Talk recording and slides', author: 'Systems Team', lead: 'linked', url: 'https://systems.example/talk',
    html: `<p class="embed-link"><a href="https://www.youtube.com/embed/abc">Embedded content on youtube.com: open in browser</a></p>${lorem(1)}<p><a href="https://systems.example/slides.pdf">${img('linked', 'alt="First slide, links to the PDF"')}</a></p>` },
  { feed: 4, title: 'Ein außerordentlich langer Titel, der zeigt, wie sich Überschriften mit sehr langen zusammengesetzten Wörtern wie Donaudampfschifffahrtsgesellschaftskapitän verhalten', author: null, lead: null, url: null,
    html: `<p>No URL, no author, no image. Just text.</p>` },
  { feed: 1, title: 'Title only', author: null, lead: null, url: 'https://garden.example/title-only', html: '' },
  { feed: 2, title: 'Photo of the day: valley fog', author: 'Flickr user', lead: 'flickr', url: 'https://lens.example/fog',
    html: '', enclosures: [{ url: 'https://live.staticflickr.example/65535/fog_b.jpg', mimeType: 'image/jpeg', length: 1_840_000 }] },
];

const rowOf = (a, i) => {
  const feed = feeds[a.feed - 1];
  return { id: i + 1, feedId: feed.id, feedTitle: feed.title, title: a.title, excerpt: a.html.replace(/<[^>]+>/g, ' ').slice(0, 140) || '', articleDate: now - i * 3600, isRead: false, isStarred: i === 2, leadImage: a.lead ? `img:${a.lead}` : null, favicon: feed.favicon };
};
const rows = (offset, limit) => A.slice(offset, offset + limit).map((a, j) => rowOf(a, offset + j));
const iconSvg = (key) => icon(16, 16, key.length * 40);
const library = { path: '/mock', name: 'Mock Library', formatVersion: 3, feedCount: feeds.length, portable: false };

const handlers = {
  startup: () => ({ kind: 'opened', library }),
  library_info: () => library,
  layouts: () => ({ default: window.__mockLayout || 'cards', views: {} }),
  set_layout: () => null,
  settings: () => ({
    appearance: { theme: window.__mockTheme || 'light', density: 'comfortable', unreadCounts: 'show' },
    reader: { fontFamily: 'serif', fontSize: 18, measure: 66, lineHeight: 1.65 },
    refresh: { intervalMinutes: 30 }, imageCacheCapBytes: 1024 ** 3, pruneReadDays: 90, pruneUnreadDays: 365,
  }),
  set_settings: () => null,
  has_setting: () => true,
  last_view: () => null,
  set_last_view: () => null,
  sidebar: () => ({ allUnread: A.length, starred: 1, today: 3, folders: [{ id: 1, name: 'Reading', unread: 6, feeds: feeds.slice(0, 3) }], feeds: feeds.slice(3) }),
  count_articles: () => A.length,
  list_articles: ({ offset, limit }) => rows(offset, limit),
  get_article: ({ id }) => {
    const a = A[id - 1];
    const feed = feeds[a.feed - 1];
    return { ...rowOf(a, id - 1), feedSiteUrl: feed.siteUrl, url: a.url, author: a.author, contentHtml: a.html, enclosures: a.enclosures || [] };
  },
  set_read: () => null, set_starred: () => null,
  refresh: () => false,
  'plugin:event|listen': () => 1,
};

window.__TAURI_INTERNALS__ = {
  invoke: async (cmd, args) => {
    const handler = handlers[cmd];
    if (!handler) { console.warn('[mock-ipc] unhandled', cmd, args); return null; }
    return handler(args || {});
  },
  transformCallback: (cb) => { const id = (Math.random() * 1e9) | 0; window[`_${id}`] = cb; return id; },
  convertFileSrc: (path, protocol) => (path.startsWith('img:') ? IMG[path.slice(4)] : protocol === 'media' ? iconSvg(path) : `${protocol}://localhost/${path}`),
  metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
};
window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };

const loaded = performance.getEntriesByType('resource').map((e) => e.name);
const svelteUrl = loaded.find((u) => /\.vite\/deps\/svelte\.js/.test(u));
const appUrl = loaded.find((u) => /\/src\/App\.svelte/.test(u));
const mainUrl = loaded.find((u) => /\/src\/main\.ts/.test(u)) ?? '/src/main.ts';
const { mount, unmount } = await import(svelteUrl);
const { default: App } = await import(appUrl);
const main = await import(mainUrl);
if (window.__mockApp) await unmount(window.__mockApp); else await unmount(main.default);
window.__mockApp = mount(App, { target: document.getElementById('app') });
'mock-reader: mounted';
