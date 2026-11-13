// Application state. One instance, shared by all components.
//
// The Article list is virtualized: only `total` and the pages around the viewport are loaded,
// so a view with 50,000 Articles costs the same as one with 50.

import { open, save } from '@tauri-apps/plugin-dialog';
import {
  api,
  isCommandError,
  LAYOUTS,
  todaySince,
  type ArticleDetail,
  type ArticleRow,
  type Layout,
  type Layouts,
  type LibraryInfo,
  type ListQuery,
  type RefreshProgress,
  type Settings,
  type Sidebar,
  type Sort,
  type Status,
  type UpdateStatus,
  type View,
} from './api';
import { applyDensity, applyTheme, type Theme } from './tokens';

export const PAGE_SIZE = 100;
/** Only read once, to migrate a pre-Library theme into the open Library; then removed. */
const THEME_KEY = 'feedrico.theme';

/** What the user picked in the sidebar. `today` resolves to a concrete `since` per query. */
export type Selection =
  | { kind: 'allUnread' }
  | { kind: 'starred' }
  | { kind: 'today' }
  | { kind: 'feed'; id: number }
  | { kind: 'folder'; id: number };

/** A Selection as a View key (`allUnread`, `feed:3`), the form the Library stores. */
export function selectionKey(s: Selection): string {
  return s.kind === 'feed' || s.kind === 'folder' ? `${s.kind}:${s.id}` : s.kind;
}

/** The Selection for a stored View key, or null if the key is unknown. */
export function parseSelection(key: string | null | undefined): Selection | null {
  if (key === 'allUnread' || key === 'starred' || key === 'today') return { kind: key };
  const match = /^(feed|folder):(\d+)$/.exec(key ?? '');
  return match ? { kind: match[1] as 'feed' | 'folder', id: Number(match[2]) } : null;
}

export type Phase = 'loading' | 'welcome' | 'attention' | 'ready';

export interface Toast {
  id: number;
  message: string;
  action?: { label: string; run: () => void };
}

export class AppState {
  phase = $state<Phase>('loading');
  /** The Add Feed dialog. */
  addFeedOpen = $state(false);
  /** The Broken Feeds dialog. */
  brokenFeedsOpen = $state(false);
  /** Broken Feeds with the name of their Folder, in sidebar order (spec: "Broken Feeds"). */
  brokenFeeds = $derived.by(() => {
    const sidebar = this.sidebar;
    if (!sidebar) return [];
    return [
      ...sidebar.folders.flatMap((folder) => folder.feeds.map((feed) => ({ feed, folder: folder.name as string | null }))),
      ...sidebar.feeds.map((feed) => ({ feed, folder: null })),
    ].filter(({ feed }) => feed.lastError !== null);
  });
  library = $state<LibraryInfo | null>(null);
  defaultPath = $state('');
  recent = $state<string[]>([]);
  /** A Library that needs a decision. `takeOverLock` remembers an earlier "Open anyway". */
  attention = $state<{ path: string; status: Status; takeOverLock: boolean } | null>(null);

  sidebar = $state<Sidebar | null>(null);
  selection = $state<Selection>({ kind: 'allUnread' });
  sort = $state<Sort>('newestFirst');
  search = $state('');
  /** False once the user widened a search from the selected Folder or Feed to everything. */
  searchScoped = $state(true);

  /** Number of Articles in the current list. */
  total = $state(0);
  /** Bumped whenever the list is reset; components key their scroll position on it. */
  listEpoch = $state(0);
  /** Bumped when cached pages change, so rows re-render. */
  pagesVersion = $state(0);
  private pages = new Map<number, ArticleRow[]>();
  private loading = new Map<number, Promise<void>>();

  selectedIndex = $state<number | null>(null);
  article = $state<ArticleDetail | null>(null);
  /** Refresh brought new Articles while the user was reading; offer to show them. */
  hasNewArticles = $state(false);

  layouts = $state<Layouts>({ default: 'list', views: {} });
  /** Grid Layouts show the reader as a slide-over panel; List always shows it. */
  readerOpen = $state(false);
  /** Columns of the current grid, reported by the grid so arrow keys can move spatially. */
  gridColumns = $state(1);
  /** Gallery only: Articles in the View that have no Lead Image and are therefore hidden. */
  hiddenWithoutImage = $state(0);

  refresh = $state<RefreshProgress | null>(null);
  toasts = $state<Toast[]>([]);
  /** Set while a Library backup is running. */
  exportProgress = $state<string | null>(null);
  /** Perf mode (see `perf.ts`): a generated Library, so never touch the network. */
  perf = false;
  /** Every setting that travels with the Library; null until one is open. */
  settings = $state<Settings | null>(null);
  /** Numbers are replaced by dots when the Library says so (calm mode for unread counts). */
  get hideCounts() {
    return this.settings?.appearance.unreadCounts === 'hide';
  }
  /** The Settings dialog. */
  settingsOpen = $state(false);
  /** Total size of the Library folder and of its media cache, for the Settings dialog. */
  librarySize = $state<number | null>(null);
  mediaSize = $state<number | null>(null);
  /** The installed version of the app, for Settings > About; null until read. */
  appVersion = $state<string | null>(null);
  /** The result of the last manual update check this session; null until one ran. */
  updateStatus = $state<UpdateStatus | null>(null);
  updateChecking = $state(false);
  /** "Start with OS" (machine-local); null until read. */
  autostart = $state<boolean | null>(null);
  /** The active theme; stored in the Library (spec: "What lives in the Library"). */
  theme = $state<Theme>('system');
  private toastId = 0;
  private timer: ReturnType<typeof setInterval> | null = null;

  // ---- Startup and Libraries ------------------------------------------------------------

  async start() {
    applyTheme(this.theme);
    await api.onRefreshProgress((p) => this.onProgress(p));
    await api.onMediaCached(() => this.onMediaCached());
    await api.onArticlesPruned(() => this.onPruned());
    try {
      const startup = await api.startup();
      if (startup.kind === 'opened') await this.enter(startup.library);
      else if (startup.kind === 'attention') this.needsAttention(startup.path, startup.status);
      else await this.showWelcome(startup.defaultPath);
    } catch (e) {
      this.notify(errorMessage(e));
      await this.showWelcome('');
    }
  }

  async showWelcome(defaultPath?: string) {
    if (defaultPath !== undefined) this.defaultPath = defaultPath;
    this.recent = await api.recentLibraries().catch(() => []);
    this.phase = 'welcome';
  }

  needsAttention(path: string, status: Status, takeOverLock = false) {
    this.attention = { path, status, takeOverLock };
    this.phase = 'attention';
  }

  async create(path: string) {
    try {
      await this.enter(await api.create(path));
    } catch (e) {
      this.notify(errorMessage(e));
    }
  }

  /** Opens a Library, routing lock/upgrade/version problems to the attention screen. */
  async open(path: string, allowUpgrade = false, takeOverLock = false) {
    try {
      await this.enter(await api.open(path, allowUpgrade, takeOverLock));
    } catch (e) {
      // One problem can hide another: a stale lock *and* an older format. After the user takes
      // over the lock, ask about the upgrade next instead of re-reporting the lock.
      if (isCommandError(e) && e.kind === 'needsUpgrade') {
        this.needsAttention(path, { kind: 'needsUpgrade', found: 0, target: 0 }, takeOverLock);
      } else if (isCommandError(e) && ['locked', 'tooNew'].includes(e.kind)) {
        this.needsAttention(path, await api.inspect(path), takeOverLock);
      } else {
        this.notify(errorMessage(e));
      }
    }
  }

  private async enter(library: LibraryInfo) {
    this.library = library;
    this.attention = null;
    this.selection = { kind: 'allUnread' };
    this.search = '';
    this.phase = 'ready';
    this.layouts = await api.layouts();
    this.settings = await api.settings();
    this.theme = this.settings.appearance.theme;
    this.applySettings();
    await this.migrateStoredTheme();
    await this.loadSidebar();
    this.selection = await this.restoredSelection();
    await this.resetList();
    this.scheduleRefresh();
  }

  /**
   * The View that was open last time, if it still exists; otherwise All Unread. A Feed or Folder
   * that was removed since (on this or another machine) must not leave an empty, nameless View.
   */
  private async restoredSelection(): Promise<Selection> {
    const fallback: Selection = { kind: 'allUnread' };
    if (this.perf) return fallback;
    let last: Selection | null = null;
    try {
      last = parseSelection(await api.lastView());
    } catch {
      return fallback;
    }
    if (!last) return fallback;
    const folders = this.sidebar?.folders ?? [];
    if (last.kind === 'folder') return folders.some((f) => f.id === last.id) ? last : fallback;
    if (last.kind === 'feed') {
      const feeds = [...(this.sidebar?.feeds ?? []), ...folders.flatMap((f) => f.feeds)];
      return feeds.some((f) => f.id === last.id) ? last : fallback;
    }
    return last;
  }

  /** Applies theme, density and reader typography from the Library settings to the root. */
  applySettings() {
    const settings = this.settings;
    if (!settings) return;
    this.theme = settings.appearance.theme;
    applyTheme(settings.appearance.theme);
    applyDensity(settings.appearance.density);
    document.documentElement.dataset.unreadCounts = settings.appearance.unreadCounts ?? 'show';
    const root = document.documentElement;
    root.style.setProperty('--reader-measure', `${settings.reader.measure}ch`);
    root.style.setProperty('--reader-font-size', `${settings.reader.fontSize}px`);
    root.style.setProperty('--reader-line-height', String(settings.reader.lineHeight));
    root.style.setProperty(
      '--reader-font-family',
      settings.reader.fontFamily === 'sans' ? 'var(--font-family-ui)' : 'var(--font-family-reading)',
    );
  }

  /**
   * One-time migration: a theme stored per machine in localStorage before themes lived in the
   * Library moves into the Library, then the key is removed. Runs once per Library open.
   */
  async migrateStoredTheme() {
    const stored = localStorage.getItem(THEME_KEY);
    if (!stored) return;
    localStorage.removeItem(THEME_KEY);
    if (!this.settings || !this.library) return;
    if (stored !== 'system' && stored !== 'light' && stored !== 'dark') return;
    if (await api.hasSetting('appearance.theme')) return;
    await this.updateSettings({
      ...this.settings,
      appearance: { ...this.settings.appearance, theme: stored },
    });
  }

  /** Writes settings to the Library, applies them locally and reschedules the refresh timer. */
  async updateSettings(settings: Settings) {
    const previous = this.settings;
    this.settings = settings;
    this.applySettings();
    try {
      await api.setSettings(settings);
    } catch (e) {
      // Every save writes all settings: keep the screen in step with what the Library holds, or
      // one rejected value would make every later change fail too.
      this.settings = previous;
      this.applySettings();
      this.notify(errorMessage(e));
      return;
    }
    if (settings.refresh.intervalMinutes !== previous?.refresh.intervalMinutes) this.scheduleRefresh();
  }

  /** (Re)starts the auto-refresh timer with the interval from the Library settings. */
  private scheduleRefresh() {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
    if (this.perf || !this.library) return;
    if (this.library.feedCount > 0) void this.startRefresh(false);
    const minutes = this.settings?.refresh.intervalMinutes ?? 30;
    this.timer = setInterval(() => void this.startRefresh(false), minutes * 60 * 1000);
  }

  /** Opens the Settings dialog with fresh settings and Library facts. */
  async openSettings() {
    try {
      const [settings, librarySize, mediaSize, autostart, appVersion] = await Promise.all([
        api.settings(),
        api.librarySize(),
        api.mediaSize(),
        api.autostartEnabled(),
        api.appVersion(),
      ]);
      this.settings = settings;
      this.applySettings();
      this.librarySize = librarySize;
      this.mediaSize = mediaSize;
      this.autostart = autostart;
      this.appVersion = appVersion;
      this.settingsOpen = true;
    } catch (e) {
      this.notify(errorMessage(e));
    }
  }

  async setAutostart(enabled: boolean) {
    try {
      await api.setAutostart(enabled);
      this.autostart = enabled;
    } catch (e) {
      this.notify(errorMessage(e));
    }
  }

  async importOpml(path: string) {
    try {
      const s = await api.importOpml(path);
      this.library = await api.info();
      await this.loadSidebar();
      const parts = [`${s.added} Feed${s.added === 1 ? '' : 's'} added`];
      if (s.skipped) parts.push(`${s.skipped} already present`);
      this.notify(parts.join(', '));
      if (s.added) void this.startRefresh(false);
    } catch (e) {
      this.notify(errorMessage(e));
    }
  }

  /** Adds a validated Feed and selects it; an existing URL selects the existing Feed instead. */
  async addFeed(url: string, title: string, siteUrl: string | null, folderId: number | null) {
    try {
      const outcome = await api.addFeed(url, title, siteUrl, folderId);
      this.library = await api.info();
      await this.loadSidebar();
      this.select({ kind: 'feed', id: outcome.feedId });
      this.notify(outcome.kind === 'exists' ? 'Already subscribed — showing that Feed' : `Added ${title}`);
      if (outcome.kind === 'added') void this.startRefresh(false);
      return true;
    } catch (e) {
      this.notify(errorMessage(e));
      return false;
    }
  }

  // ---- Managing Feeds and Folders ---------------------------------------------------------

  /** Runs a library change, refreshes the sidebar, and reports errors as a toast. */
  private async manage(change: () => Promise<void>, after?: () => Promise<void> | void) {
    try {
      await change();
      await this.loadSidebar();
      await after?.();
      return true;
    } catch (e) {
      this.notify(errorMessage(e));
      return false;
    }
  }

  renameFeed(id: number, title: string) {
    return this.manage(() => api.renameFeed(id, title));
  }

  renameFolder(id: number, name: string) {
    return this.manage(() => api.renameFolder(id, name));
  }

  moveFeed(id: number, folderId: number | null) {
    // A Folder's list changes when one of its Feeds leaves or arrives.
    return this.manage(
      () => api.moveFeed(id, folderId),
      () => (this.selection.kind === 'folder' ? this.resetList() : undefined),
    );
  }

  /** Deletes a Folder. Its Feeds move to the top level, or are unsubscribed with their Articles. */
  deleteFolder(id: number, unsubscribeFeeds: boolean) {
    return this.manage(
      async () => {
        await api.deleteFolder(id, unsubscribeFeeds);
        if (unsubscribeFeeds) this.library = await api.info();
      },
      () => {
        if (this.selection.kind === 'folder' && this.selection.id === id) return this.select({ kind: 'allUnread' });
        // Unsubscribed Feeds' Articles vanish from every other View too.
        return unsubscribeFeeds ? this.resetList() : undefined;
      },
    );
  }

  unsubscribe(id: number) {
    return this.manage(
      async () => {
        await api.unsubscribe(id);
        this.library = await api.info();
      },
      () => (this.selection.kind === 'feed' && this.selection.id === id ? this.select({ kind: 'allUnread' }) : this.resetList()),
    );
  }

  /** Unsubscribes from several Feeds at once (Broken Feeds dialog). */
  unsubscribeFeeds(ids: number[]) {
    return this.manage(
      async () => {
        await api.unsubscribeFeeds(ids);
        this.library = await api.info();
      },
      () =>
        this.selection.kind === 'feed' && ids.includes(this.selection.id)
          ? this.select({ kind: 'allUnread' })
          : this.resetList(),
    );
  }

  /**
   * Fetches one broken Feed now. The sidebar reloads, so the Feed's row shows the result.
   * Resolves to the error after the attempt, or null if the Feed works again.
   */
  async retryFeed(id: number): Promise<string | null> {
    try {
      const outcome = await api.retryFeed(id);
      await this.loadSidebar();
      if (outcome.newArticles > 0) {
        if (this.selectedIndex === null) void this.resetList();
        else this.hasNewArticles = true;
      }
      return outcome.error;
    } catch (e) {
      const message = errorMessage(e);
      this.notify(message);
      return message;
    }
  }

  async exportOpml(path: string) {
    try {
      await api.exportOpml(path);
      this.notify('OPML exported');
    } catch (e) {
      this.notify(errorMessage(e));
    }
  }

  /** Asks for an OPML file and imports it (sidebar button and palette share this). */
  async chooseImportOpml() {
    const file = await open({
      title: 'Import OPML',
      filters: [{ name: 'OPML', extensions: ['opml', 'xml'] }],
    });
    if (typeof file === 'string') await this.importOpml(file);
  }

  /** Asks where to write the OPML export (sidebar button and palette share this). */
  async chooseExportOpml() {
    const file = await save({
      title: 'Export OPML',
      defaultPath: 'feedrico.opml',
      filters: [{ name: 'OPML', extensions: ['opml'] }],
    });
    if (file) await this.exportOpml(file);
  }

  /** The theme lives in the Library now; the footer button and palette action write through. */
  setTheme(theme: Theme) {
    if (!this.settings) {
      // No Library open (welcome screen): follow the OS.
      this.theme = 'system';
      applyTheme('system');
      return;
    }
    void this.updateSettings({
      ...this.settings,
      appearance: { ...this.settings.appearance, theme },
    });
  }

  cycleTheme() {
    const themes: Theme[] = ['system', 'light', 'dark'];
    this.setTheme(themes[(themes.indexOf(this.theme) + 1) % themes.length] ?? 'system');
  }

  /** Copies the open Library to an empty folder; the core reports progress per media file. */
  async exportLibrary(path: string) {
    const unlisten = await api.onExportProgress((p) => {
      this.exportProgress = p.total > 0 ? `Backing up Library… ${p.done} / ${p.total}` : null;
    });
    try {
      await api.exportLibrary(path);
      this.notify('Library backed up');
    } catch (e) {
      this.notify(errorMessage(e));
    } finally {
      unlisten();
      this.exportProgress = null;
    }
  }

  // ---- Sidebar and views ------------------------------------------------------------------

  async loadSidebar() {
    this.sidebar = await api.sidebar(todaySince());
  }

  private sidebarTimer: ReturnType<typeof setTimeout> | null = null;
  /** Unread counts change on every keypress; coalesce the reloads. */
  scheduleSidebar() {
    if (this.sidebarTimer) return;
    this.sidebarTimer = setTimeout(() => {
      this.sidebarTimer = null;
      void this.loadSidebar();
    }, 250);
  }

  query(): ListQuery {
    const withLeadImage = this.layout === 'gallery';
    const search = this.search.trim();
    if (search) {
      const scope = this.searchScope;
      const view: View =
        scope?.kind === 'feed'
          ? { kind: 'search', feed: scope.id }
          : scope?.kind === 'folder'
            ? { kind: 'search', folder: scope.id }
            : { kind: 'search' };
      return { view, sort: this.sort, search, withLeadImage };
    }
    const s = this.selection;
    const view: View = s.kind === 'today' ? { kind: 'today', since: todaySince() } : s;
    return { view, sort: this.sort, withLeadImage };
  }

  /** The key a View's Layout is stored under; search results use the default. */
  get viewKey(): string | null {
    if (this.search.trim()) return null;
    const s = this.selection;
    return 'id' in s ? `${s.kind}:${s.id}` : s.kind;
  }

  get layout(): Layout {
    const key = this.viewKey;
    return (key && this.layouts.views[key]) || this.layouts.default;
  }

  get isGrid(): boolean {
    return this.layout !== 'list';
  }

  async setLayout(layout: Layout) {
    const key = this.viewKey;
    const before = this.layout;
    if (key) this.layouts.views[key] = layout;
    else this.layouts.default = layout;
    // An open Article stays open across Layouts: the pane in a list is the slide-over in a grid.
    this.readerOpen = this.article !== null;
    await api.setLayout(key, layout);
    // Gallery filters the list; switching into or out of it needs a fresh list.
    if ((before === 'gallery') !== (layout === 'gallery')) await this.resetList({ keepArticle: true });
  }

  cycleLayout() {
    const next = LAYOUTS[(LAYOUTS.indexOf(this.layout) + 1) % LAYOUTS.length] ?? 'list';
    void this.setLayout(next);
  }

  /** Resolves once the new View's first page is loaded. */
  select(selection: Selection) {
    this.selection = selection;
    this.search = '';
    this.searchScoped = true;
    if (!this.perf) {
      // Best effort: a failed save only means the next start opens All Unread.
      void Promise.resolve()
        .then(() => api.setLastView(selectionKey(selection)))
        .catch(() => {});
    }
    return this.resetList();
  }

  /**
   * Spec "Search": searching with a Folder or Feed selected searches inside it, until the user
   * removes the scope. Smart Views always search everything.
   */
  get searchScope(): { kind: 'feed' | 'folder'; id: number } | null {
    const s = this.selection;
    return this.searchScoped && (s.kind === 'feed' || s.kind === 'folder') ? s : null;
  }

  /** Drops the Folder or Feed scope for this search; selecting a View brings it back. */
  searchEverywhere() {
    this.searchScoped = false;
    return this.search.trim() ? this.resetList() : Promise.resolve();
  }

  setSearch(search: string) {
    this.search = search;
    return this.resetList();
  }

  toggleSort() {
    this.sort = this.sort === 'newestFirst' ? 'oldestFirst' : 'newestFirst';
    void this.resetList();
  }

  isSelected(selection: Selection): boolean {
    if (this.search.trim()) return false;
    const s = this.selection;
    return (
      s.kind === selection.kind &&
      (!('id' in s) || !('id' in selection) || s.id === selection.id)
    );
  }

  // ---- The Article list -------------------------------------------------------------------

  async resetList(options: { keepArticle?: boolean } = {}) {
    // Bump the epoch first: anything still loading for the previous list is discarded, and
    // components that ask for pages while we count already load the new list.
    const query = this.query();
    const epoch = ++this.listEpoch;
    this.pages = new Map();
    this.loading = new Map();
    this.selectedIndex = null;
    this.hasNewArticles = false;
    if (!options.keepArticle) {
      this.article = null;
      this.readerOpen = false;
    }
    const total = await api.count(query);
    const hidden = query.withLeadImage
      ? (await api.count({ ...query, withLeadImage: false })) - total
      : 0;
    if (epoch !== this.listEpoch) return;
    this.total = total;
    this.hiddenWithoutImage = hidden;
    this.pagesVersion++;
    await this.ensureRange(0, PAGE_SIZE);
  }

  /** Reactive: reading a row subscribes to page loads and row patches. */
  rowAt(index: number): ArticleRow | undefined {
    void this.pagesVersion;
    return this.pages.get(Math.floor(index / PAGE_SIZE))?.[index % PAGE_SIZE];
  }

  /** Loads the pages covering `[start, end)`. */
  async ensureRange(start: number, end: number) {
    const first = Math.max(0, Math.floor(start / PAGE_SIZE));
    const last = Math.floor(Math.max(start, Math.min(end, this.total) - 1) / PAGE_SIZE);
    const epoch = this.listEpoch;
    const jobs: Promise<void>[] = [];
    for (let page = first; page <= last; page++) {
      if (this.pages.has(page)) continue;
      let job = this.loading.get(page);
      if (!job) {
        const loading = this.loading;
        const request: Promise<void> = api
          .list(this.query(), page * PAGE_SIZE, PAGE_SIZE)
          .then((rows) => {
            if (epoch !== this.listEpoch) return;
            this.pages.set(page, rows);
            this.pagesVersion++;
          })
          .finally(() => {
            // Always clear the marker, or a discarded request would block the page forever.
            if (loading.get(page) === request) loading.delete(page);
          });
        loading.set(page, request);
        job = request;
      }
      jobs.push(job);
    }
    await Promise.all(jobs);
  }

  private patchRow(id: number, patch: Partial<ArticleRow>) {
    for (const rows of this.pages.values()) {
      const row = rows.find((r) => r.id === id);
      if (row) Object.assign(row, patch);
    }
    this.pagesVersion++;
  }

  /** Opens the Article at `index` in the reader, which marks it Read. */
  async selectIndex(index: number) {
    if (index < 0 || index >= this.total) return;
    await this.ensureRange(index, index + 1);
    const row = this.rowAt(index);
    if (!row) return;
    this.selectedIndex = index;
    if (this.isGrid) this.readerOpen = true;
    const detail = await api.article(row.id);
    if (this.selectedIndex !== index) return;
    this.article = detail;
    if (!detail.isRead) await this.setRead(true);
  }

  /** Moves the highlight without opening: in grids, seeing an Article doesn't make it Read. */
  focusIndex(index: number) {
    if (index < 0 || index >= this.total) return;
    this.selectedIndex = index;
    void this.ensureRange(index, index + 1);
  }

  async move(delta: number) {
    const next = this.selectedIndex === null ? 0 : this.selectedIndex + delta;
    const index = Math.min(Math.max(next, 0), this.total - 1);
    if (this.isGrid && !this.readerOpen) this.focusIndex(index);
    else await this.selectIndex(index);
  }

  /** Arrow keys: spatial in Cards and Gallery, sequential elsewhere. */
  moveSpatial(direction: 'up' | 'down' | 'left' | 'right') {
    const vertical = this.layout === 'cards' || this.layout === 'gallery' ? this.gridColumns : 1;
    const step = { up: -vertical, down: vertical, left: -1, right: 1 }[direction];
    if (this.layout === 'list' && (direction === 'left' || direction === 'right')) return;
    this.move(step);
  }

  openReader() {
    if (this.selectedIndex === null) this.focusIndex(0);
    if (this.selectedIndex !== null) void this.selectIndex(this.selectedIndex);
  }

  closeReader() {
    this.readerOpen = false;
  }

  /** Images and favicons were cached in the background: show them where they are on screen. */
  async onMediaCached() {
    await Promise.all([this.reloadArticle(), this.loadSidebar()]);
    const sidebar = this.sidebar;
    if (!sidebar) return;
    const icons = new Map<number, string | null>();
    for (const feed of [...sidebar.feeds, ...sidebar.folders.flatMap((f) => f.feeds)]) {
      icons.set(feed.id, feed.favicon);
    }
    for (const rows of this.pages.values()) {
      for (const row of rows) row.favicon = icons.get(row.feedId) ?? row.favicon;
    }
    this.pagesVersion++;
  }

  private async reloadArticle() {
    if (this.article) this.article = await api.article(this.article.id);
  }

  async setRead(read: boolean) {
    const a = this.article;
    if (!a) return;
    a.isRead = read;
    this.patchRow(a.id, { isRead: read });
    await api.setRead(a.id, read);
    this.scheduleSidebar();
  }

  async toggleRead() {
    if (this.article) await this.setRead(!this.article.isRead);
  }

  async toggleStar() {
    const a = this.article;
    if (!a) return;
    a.isStarred = !a.isStarred;
    this.patchRow(a.id, { isStarred: a.isStarred });
    await api.setStarred(a.id, a.isStarred);
    this.scheduleSidebar();
  }

  async markAllRead() {
    const ids = await api.markAllRead(this.query());
    if (!ids.length) return;
    for (const id of ids) this.patchRow(id, { isRead: true });
    if (this.article && ids.includes(this.article.id)) this.article.isRead = true;
    await this.loadSidebar();
    this.notify(`${ids.length} marked as read`, {
      label: 'Undo',
      run: async () => {
        await api.markUnread(ids);
        for (const id of ids) this.patchRow(id, { isRead: false });
        await this.loadSidebar();
      },
    });
  }

  openInBrowser() {
    const url = this.article?.url;
    if (url) this.openExternal(url);
  }

  openExternal(url: string) {
    api.openExternal(url).catch((e) => this.notify(errorMessage(e)));
  }

  // ---- Updates ----------------------------------------------------------------------------

  private updateCheckRunning = false;

  /** One manual check against GitHub Releases. Network policy: only ever run from a click. */
  async checkForUpdates() {
    if (this.updateCheckRunning) return;
    this.updateCheckRunning = true;
    this.updateChecking = true;
    try {
      const status = await api.checkForUpdates();
      this.updateStatus = status;
      if (status.kind === 'available') {
        this.notify(`Feedrico ${status.latest} is available`, {
          label: 'Open',
          run: () => this.openExternal(status.url),
        });
      } else if (status.kind === 'noReleases') {
        this.notify('No release published yet');
      } else {
        this.notify(`You're up to date (${status.current})`);
      }
    } catch {
      this.notify("Couldn't reach GitHub. Check your connection and try again.");
    } finally {
      this.updateCheckRunning = false;
      this.updateChecking = false;
    }
  }

  // ---- Refresh ----------------------------------------------------------------------------

  async startRefresh(force: boolean) {
    if (!this.library) return;
    await api.refresh(force);
  }

  private onProgress(p: RefreshProgress) {
    this.refresh = p;
    if (p.done % 10 === 0 || p.finished) this.scheduleSidebar();
    if (p.finished) {
      if (p.newArticles > 0) {
        // Don't yank the list away from someone reading; offer the new Articles instead.
        if (this.selectedIndex === null) void this.resetList();
        else this.hasNewArticles = true;
      }
      setTimeout(() => {
        if (this.refresh?.finished) this.refresh = null;
      }, 2500);
    }
  }

  /** Pruning removed Articles: refresh the counts, and the list unless someone is reading it. */
  private onPruned() {
    void this.loadSidebar();
    if (this.selectedIndex === null) void this.resetList();
    else this.hasNewArticles = true;
  }

  // ---- Toasts -----------------------------------------------------------------------------

  notify(message: string, action?: Toast['action']) {
    const toast = { id: ++this.toastId, message, action };
    this.toasts = [...this.toasts, toast];
    setTimeout(() => this.dismiss(toast.id), action ? 8000 : 4000);
  }

  dismiss(id: number) {
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }
}

export function errorMessage(e: unknown): string {
  if (isCommandError(e)) return e.message;
  return e instanceof Error ? e.message : String(e);
}

export const app = new AppState();
