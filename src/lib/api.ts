// Typed bridge to the Rust shell. Every command lives here; components never call invoke().

import { getVersion } from '@tauri-apps/api/app';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { Density, Theme } from './tokens';

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
  /** Media key of the cached favicon; null until one is cached. See mediaUrl(). */
  favicon: string | null;
}

export interface RetryOutcome {
  /** The Feed's error after the attempt; null if it worked. */
  error: string | null;
  newArticles: number;
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
  | { kind: 'search'; feed?: number; folder?: number };

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

// ---- Settings (they travel with the Library; "start with OS" stays on the machine) --------

export type ReaderFontFamily = 'serif' | 'sans';

export interface AppearanceSettings {
  theme: Theme;
  density: Density;
  /** Unread numbers in the sidebar and list header; `hide` shows a dot instead. */
  unreadCounts?: 'show' | 'hide';
}

export interface ReaderSettings {
  fontFamily: ReaderFontFamily;
  /** Font size in px (14–24). */
  fontSize: number;
  /** Line width in ch (50–90). */
  measure: number;
  /** Line height (1.3–2.0). */
  lineHeight: number;
}

export interface RefreshSettings {
  intervalMinutes: number;
}

export interface Settings {
  appearance: AppearanceSettings;
  reader: ReaderSettings;
  refresh: RefreshSettings;
  imageCacheCapBytes: number;
  pruneReadDays: number;
  pruneUnreadDays: number;
}

/** The refresh intervals the Settings dialog offers. */
export const REFRESH_INTERVALS = [15, 30, 60, 120, 240] as const;

/** Image cache cap options in bytes. */
export const IMAGE_CACHE_CAPS = [
  250 * 1024 * 1024,
  500 * 1024 * 1024,
  1024 * 1024 * 1024,
  2 * 1024 * 1024 * 1024,
  5 * 1024 * 1024 * 1024,
] as const;

export const PRUNE_READ_OPTIONS = [30, 90, 180, 365] as const;
export const PRUNE_UNREAD_OPTIONS = [90, 180, 365, 730] as const;

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
  /** Media key of the Feed's cached favicon; null shows a generated tile. */
  favicon: string | null;
}

export interface Enclosure {
  url: string;
  mimeType: string | null;
  length: number | null;
  /** Play time in seconds (MediaRSS duration / JSON Feed duration_in_seconds). */
  durationSecs: number | null;
  /** Readable kind derived from the MediaRSS medium / MIME type / file extension. */
  kind: 'audio' | 'video' | 'document';
  /** media:player page; the "open" target when there is no direct media URL. */
  playerUrl: string | null;
}

export type VideoHost = 'youtube' | 'vimeo';

/** The video an Article is about or embeds (#75), stored on the Article in the Library. */
export interface VideoMeta {
  host: VideoHost;
  /** The video id on its platform. */
  id: string;
  /** Seconds, from media:content/@duration. */
  duration: number | null;
  /** From media:community/media:statistics/@views. */
  views: number | null;
  /** Source URL of the Feed's own thumbnail (media:thumbnail), cached at fetch time. */
  thumbnail: string | null;
}

export interface ArticleDetail extends ArticleRow {
  feedSiteUrl: string | null;
  url: string | null;
  author: string | null;
  contentHtml: string;
  enclosures: Enclosure[];
  /** The video identity + metadata; null for plain Articles (older Libraries may lack it). */
  video?: VideoMeta | null;
}

export interface ImportSummary {
  added: number;
  skipped: number;
  foldersCreated: number;
}

export interface FeedCandidate {
  url: string;
  title: string | null;
}

export type Discovery =
  | { kind: 'found'; url: string; title: string; siteUrl: string | null }
  | { kind: 'choose'; candidates: FeedCandidate[] };

export type AddOutcome = { kind: 'added'; feedId: number } | { kind: 'exists'; feedId: number };

export interface ExportProgress {
  done: number;
  total: number;
}

export interface RefreshProgress {
  done: number;
  total: number;
  newArticles: number;
  failed: number;
  finished: boolean;
}

/** Result of one manual check against GitHub Releases (only ever requested on a click). */
export type UpdateStatus =
  | { kind: 'upToDate'; current: string }
  | { kind: 'available'; current: string; latest: string; url: string }
  | { kind: 'noReleases'; current: string };

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
  discoverFeed: (input: string) => invoke<Discovery>('discover_feed', { input }),
  addFeed: (url: string, title: string, siteUrl: string | null, folderId: number | null) =>
    invoke<AddOutcome>('add_feed', { url, title, siteUrl, folderId }),
  renameFeed: (id: number, title: string) => invoke<void>('rename_feed', { id, title }),
  renameFolder: (id: number, name: string) => invoke<void>('rename_folder', { id, name }),
  moveFeed: (id: number, folderId: number | null) => invoke<void>('move_feed', { id, folderId }),
  deleteFolder: (id: number, unsubscribeFeeds: boolean) =>
    invoke<void>('delete_folder', { id, unsubscribeFeeds }),
  unsubscribe: (id: number) => invoke<void>('unsubscribe', { id }),
  /** Unsubscribes from several Feeds at once; resolves to how many were removed. */
  unsubscribeFeeds: (ids: number[]) => invoke<number>('unsubscribe_feeds', { ids }),
  /** Fetches one Feed now, ignoring its backoff. */
  retryFeed: (id: number) => invoke<RetryOutcome>('retry_feed', { id }),
  exportOpml: (path: string) => invoke<void>('export_opml', { path }),
  exportLibrary: (path: string) => invoke<void>('export_library', { path }),
  onExportProgress: (cb: (p: ExportProgress) => void): Promise<UnlistenFn> =>
    listen<ExportProgress>('export-progress', (e) => cb(e.payload)),
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
  lastView: () => invoke<string | null>('last_view'),
  setLastView: (view: string) => invoke<void>('set_last_view', { view }),
  settings: () => invoke<Settings>('settings'),
  setSettings: (settings: Settings) => invoke<void>('set_settings', { new: settings }),
  hasSetting: (key: string) => invoke<boolean>('has_setting', { key }),
  librarySize: () => invoke<number>('library_size'),
  mediaSize: () => invoke<number>('media_size'),
  autostartEnabled: () => invoke<boolean>('autostart_enabled'),
  setAutostart: (enabled: boolean) => invoke<void>('set_autostart', { enabled }),
  openExternal: (url: string) => invoke<void>('open_external', { url }),
  appVersion: () => getVersion(),
  checkForUpdates: () => invoke<UpdateStatus>('check_for_updates'),
  refresh: (force: boolean) => invoke<boolean>('refresh', { force }),
  onRefreshProgress: (cb: (p: RefreshProgress) => void): Promise<UnlistenFn> =>
    listen<RefreshProgress>('refresh-progress', (e) => cb(e.payload)),
  onMediaCached: (cb: () => void): Promise<UnlistenFn> => listen('media-cached', () => cb()),
  onArticlesPruned: (cb: () => void): Promise<UnlistenFn> => listen('articles-pruned', () => cb()),
  /** Perf mode only: milliseconds since the process started. */
  perfInteractive: () => invoke<number>('perf_interactive'),
  /** Perf mode only: prints the report and exits the app. */
  perfFinish: (report: unknown) => invoke<void>('perf_finish', { report }),
};

/** URL of a cached image, served from the open Library by the shell's media protocol. */
export function mediaUrl(key: string): string {
  return convertFileSrc(key, 'media');
}

/** The canonical watch page of a video, for the video card's "Open in browser" link. */
export function videoWatchUrl(video: VideoMeta): string {
  const id = encodeURIComponent(video.id);
  return video.host === 'vimeo' ? `https://vimeo.com/${id}` : `https://www.youtube.com/watch?v=${id}`;
}

/** Local midnight in unix seconds: the start of the Today Smart View. */
export function todaySince(now = new Date()): number {
  const midnight = new Date(now);
  midnight.setHours(0, 0, 0, 0);
  return Math.floor(midnight.getTime() / 1000);
}
