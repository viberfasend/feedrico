// Application state. One instance, shared by all components.
//
// The Article list is virtualized: only `total` and the pages around the viewport are loaded,
// so a view with 50,000 Articles costs the same as one with 50.

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
  type Sidebar,
  type Sort,
  type Status,
  type View,
} from './api';

export const PAGE_SIZE = 100;
const AUTO_REFRESH_MS = 30 * 60 * 1000;

/** What the user picked in the sidebar. `today` resolves to a concrete `since` per query. */
export type Selection =
  | { kind: 'allUnread' }
  | { kind: 'starred' }
  | { kind: 'today' }
  | { kind: 'feed'; id: number }
  | { kind: 'folder'; id: number };

export type Phase = 'loading' | 'welcome' | 'attention' | 'ready';

export interface Toast {
  id: number;
  message: string;
  action?: { label: string; run: () => void };
}

class AppState {
  phase = $state<Phase>('loading');
  library = $state<LibraryInfo | null>(null);
  defaultPath = $state('');
  recent = $state<string[]>([]);
  attention = $state<{ path: string; status: Status } | null>(null);

  sidebar = $state<Sidebar | null>(null);
  selection = $state<Selection>({ kind: 'allUnread' });
  sort = $state<Sort>('newestFirst');
  search = $state('');

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
  private toastId = 0;
  private timer: ReturnType<typeof setInterval> | null = null;

  // ---- Startup and Libraries ------------------------------------------------------------

  async start() {
    await api.onRefreshProgress((p) => this.onProgress(p));
    await api.onMediaCached(() => this.reloadArticle());
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

  needsAttention(path: string, status: Status) {
    this.attention = { path, status };
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
      if (isCommandError(e) && ['locked', 'needsUpgrade', 'tooNew'].includes(e.kind)) {
        this.needsAttention(path, await api.inspect(path));
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
    await Promise.all([this.loadSidebar(), this.resetList()]);
    if (library.feedCount > 0) void this.startRefresh(false);
    if (this.timer) clearInterval(this.timer);
    this.timer = setInterval(() => void this.startRefresh(false), AUTO_REFRESH_MS);
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

  async exportOpml(path: string) {
    try {
      await api.exportOpml(path);
      this.notify('OPML exported');
    } catch (e) {
      this.notify(errorMessage(e));
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
    if (search) return { view: { kind: 'search' }, sort: this.sort, search, withLeadImage };
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
    this.readerOpen = false;
    await api.setLayout(key, layout);
    // Gallery filters the list; switching into or out of it needs a fresh list.
    if ((before === 'gallery') !== (layout === 'gallery')) await this.resetList();
  }

  cycleLayout() {
    const next = LAYOUTS[(LAYOUTS.indexOf(this.layout) + 1) % LAYOUTS.length] ?? 'list';
    void this.setLayout(next);
  }

  select(selection: Selection) {
    this.selection = selection;
    this.search = '';
    void this.resetList();
  }

  setSearch(search: string) {
    this.search = search;
    void this.resetList();
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

  async resetList() {
    const query = this.query();
    this.pages = new Map();
    this.loading = new Map();
    this.selectedIndex = null;
    this.article = null;
    this.hasNewArticles = false;
    this.readerOpen = false;
    this.total = await api.count(query);
    this.hiddenWithoutImage = query.withLeadImage
      ? (await api.count({ ...query, withLeadImage: false })) - this.total
      : 0;
    this.listEpoch++;
    this.pagesVersion++;
    await this.ensureRange(0, PAGE_SIZE);
  }

  rowAt(index: number): ArticleRow | undefined {
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
        job = api.list(this.query(), page * PAGE_SIZE, PAGE_SIZE).then((rows) => {
          if (epoch !== this.listEpoch) return;
          this.pages.set(page, rows);
          this.loading.delete(page);
          this.pagesVersion++;
        });
        this.loading.set(page, job);
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

  move(delta: number) {
    const next = this.selectedIndex === null ? 0 : this.selectedIndex + delta;
    const index = Math.min(Math.max(next, 0), this.total - 1);
    if (this.isGrid && !this.readerOpen) this.focusIndex(index);
    else void this.selectIndex(index);
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
