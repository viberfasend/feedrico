// Typed bridge to the Rust shell. Every command lives here; components never call invoke().

import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export interface LockInfo {
  machine: string;
  pid: number;
  openedAt: number;
}

export type Status =
  | { kind: 'ready' }
  | { kind: 'needsUpgrade'; found: number; target: number }
  | { kind: 'tooNew'; found: number; supported: number }
  | ({ kind: 'locked' } & LockInfo)
  | { kind: 'notALibrary' };

export interface LibraryInfo {
  path: string;
  name: string;
  formatVersion: number;
  feedCount: number;
  portable: boolean;
}

export type Startup =
  | { kind: 'opened'; library: LibraryInfo }
  | { kind: 'attention'; path: string; status: Status }
  | { kind: 'welcome'; defaultPath: string };

export interface CommandError {
  kind: string;
  message: string;
}

export function isCommandError(e: unknown): e is CommandError {
  return typeof e === 'object' && e !== null && 'kind' in e && 'message' in e;
}

export interface FeedNode {
  id: number;
  title: string;
  siteUrl: string | null;
  unread: number;
  lastError: string | null;
  lastSuccessAt: number | null;
}

export interface FolderNode {
  id: number;
  name: string;
  unread: number;
  feeds: FeedNode[];
}

export interface Sidebar {
  allUnread: number;
  starred: number;
  today: number;
  folders: FolderNode[];
  feeds: FeedNode[];
}

export type View =
  | { kind: 'allUnread' }
  | { kind: 'starred' }
  | { kind: 'today'; since: number }
  | { kind: 'feed'; id: number }
  | { kind: 'folder'; id: number }
  | { kind: 'search' };

export type Sort = 'newestFirst' | 'oldestFirst';

export interface ListQuery {
  view: View;
  sort: Sort;
  search?: string | null;
  /** Only Articles with a Lead Image (Gallery). */
  withLeadImage?: boolean;
}

export type Layout = 'list' | 'cards' | 'magazine' | 'gallery';
export const LAYOUTS: Layout[] = ['list', 'cards', 'magazine', 'gallery'];

export interface Layouts {
  default: Layout;
  views: Record<string, Layout>;
}

export interface ArticleRow {
  id: number;
  feedId: number;
  feedTitle: string;
  title: string;
  excerpt: string;
  articleDate: number;
  isRead: boolean;
  isStarred: boolean;
  /** Media key of the Lead Image; see mediaUrl(). */
  leadImage: string | null;
}

export interface Enclosure {
  url: string;
  mimeType: string | null;
  length: number | null;
}

export interface ArticleDetail extends ArticleRow {
  feedSiteUrl: string | null;
  url: string | null;
  author: string | null;
  contentHtml: string;
  enclosures: Enclosure[];
}

export interface ImportSummary {
  added: number;
  skipped: number;
  foldersCreated: number;
}

export interface RefreshProgress {
  done: number;
  total: number;
  newArticles: number;
  failed: number;
  finished: boolean;
}

export const api = {
  startup: () => invoke<Startup>('startup'),
  recentLibraries: () => invoke<string[]>('recent_libraries'),
  inspect: (path: string) => invoke<Status>('library_inspect', { path }),
  create: (path: string) => invoke<LibraryInfo>('library_create', { path }),
  open: (path: string, allowUpgrade = false, takeOverLock = false) =>
    invoke<LibraryInfo>('library_open', { path, allowUpgrade, takeOverLock }),
  info: () => invoke<LibraryInfo>('library_info'),
  close: () => invoke<void>('library_close'),
  importOpml: (path: string) => invoke<ImportSummary>('import_opml', { path }),
  exportOpml: (path: string) => invoke<void>('export_opml', { path }),
  sidebar: (todaySince: number) => invoke<Sidebar>('sidebar', { todaySince }),
  count: (query: ListQuery) => invoke<number>('count_articles', { query }),
  list: (query: ListQuery, offset: number, limit: number) =>
    invoke<ArticleRow[]>('list_articles', { query, offset, limit }),
  article: (id: number) => invoke<ArticleDetail>('get_article', { id }),
  setRead: (id: number, read: boolean) => invoke<void>('set_read', { id, read }),
  setStarred: (id: number, starred: boolean) => invoke<void>('set_starred', { id, starred }),
  markAllRead: (query: ListQuery) => invoke<number[]>('mark_all_read', { query }),
  markUnread: (ids: number[]) => invoke<void>('mark_unread', { ids }),
  layouts: () => invoke<Layouts>('layouts'),
  setLayout: (view: string | null, layout: Layout) => invoke<void>('set_layout', { view, layout }),
  openExternal: (url: string) => invoke<void>('open_external', { url }),
  refresh: (force: boolean) => invoke<boolean>('refresh', { force }),
  onRefreshProgress: (cb: (p: RefreshProgress) => void): Promise<UnlistenFn> =>
    listen<RefreshProgress>('refresh-progress', (e) => cb(e.payload)),
  onMediaCached: (cb: () => void): Promise<UnlistenFn> => listen('media-cached', () => cb()),
};

/** URL of a cached image, served from the open Library by the shell's media protocol. */
export function mediaUrl(key: string): string {
  return convertFileSrc(key, 'media');
}

/** Local midnight in unix seconds: the start of the Today Smart View. */
export function todaySince(now = new Date()): number {
  const midnight = new Date(now);
  midnight.setHours(0, 0, 0, 0);
  return Math.floor(midnight.getTime() / 1000);
}
