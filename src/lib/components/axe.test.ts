// @vitest-environment happy-dom
// Automated WCAG check (issue #32): axe-core runs over the mounted app in every Layout, with the
// slide-over reader open, the Settings dialog open and the command palette open. Text contrast is
// disabled (no real layout in a DOM emulator; the token build covers contrast in both themes) —
// everything else must be clean.
import axeModule from 'axe-core';

// axe-core is CJS: the default export is the axe object; the async API is axe.run(context, options).
interface AxeResults {
  violations: { id: string; impact: string; nodes: { target: string[] }[] }[];
}
const axe = axeModule as unknown as {
  run: (context: Element, options: object) => Promise<AxeResults>;
};
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ArticleDetail, ArticleRow, Layout, ListQuery, Settings, Sidebar } from '../api';

const defaultSettings = (): Settings => ({
  appearance: { theme: 'system', density: 'comfortable' },
  reader: { fontFamily: 'serif', fontSize: 18, measure: 66, lineHeight: 1.65 },
  refresh: { intervalMinutes: 30 },
  imageCacheCapBytes: 1024 ** 3,
  pruneReadDays: 90,
  pruneUnreadDays: 365,
});

function row(i: number): ArticleRow {
  return {
    id: i + 1,
    feedId: 1,
    feedTitle: 'Rust Blog',
    title: `Article ${i + 1}`,
    excerpt: 'An excerpt',
    articleDate: 1_700_000_000 - i,
    isRead: i % 2 === 1,
    isStarred: i === 1,
    leadImage: null,
    favicon: null,
  };
}

const sidebar: Sidebar = {
  allUnread: 3,
  starred: 1,
  today: 1,
  folders: [
    {
      id: 2,
      name: 'Tech',
      unread: 2,
      feeds: [{ id: 5, title: 'Rust Blog', siteUrl: 'https://blog.rust-lang.org', unread: 2, lastError: 'HTTP 404', lastSuccessAt: 1_700_000_000, favicon: null }],
    },
  ],
  feeds: [{ id: 7, title: 'Hacker News', siteUrl: 'https://news.yorker.com', unread: 1, lastError: null, lastSuccessAt: null, favicon: null }],
};

const mockApi = {
  startup: vi.fn(async () => ({ kind: 'welcome', defaultPath: '' })),
  recentLibraries: vi.fn(async () => [] as string[]),
  onRefreshProgress: vi.fn(async () => () => {}),
  onMediaCached: vi.fn(async () => () => {}),
  onArticlesPruned: vi.fn(async () => () => {}),
  count: vi.fn(async (_q: ListQuery) => 4),
  list: vi.fn(async (_q: ListQuery, offset: number, limit: number) =>
    Array.from({ length: Math.max(0, Math.min(limit, 4 - offset)) }, (_, i) => row(offset + i)),
  ),
  refresh: vi.fn(async () => true),
  sidebar: vi.fn(async (): Promise<Sidebar> => sidebar),
  info: vi.fn(async () => ({ path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 2, portable: false })),
  layouts: vi.fn(async () => ({ default: 'list', views: {} })),
  setLayout: vi.fn(async () => {}),
  article: vi.fn(
    async (id: number): Promise<ArticleDetail> => ({
      ...row(id - 1),
      feedSiteUrl: null,
      url: 'https://example.com/a',
      author: null,
      contentHtml: '<p>Hello</p>',
      enclosures: [],
    }),
  ),
  setRead: vi.fn(async () => {}),
  articleIndex: vi.fn(async (_q: ListQuery, id: number) => (id <= 4 ? id - 1 : null)),
  deleteFolder: vi.fn(async () => {}),
  openExternal: vi.fn(async () => {}),
  appVersion: vi.fn(async () => '0.1.0'),
  checkForUpdates: vi.fn(),
  settings: vi.fn(async () => defaultSettings()),
  setSettings: vi.fn(async () => {}),
  hasSetting: vi.fn(async () => false),
  librarySize: vi.fn(async () => 1000),
  mediaSize: vi.fn(async () => 500),
  autostartEnabled: vi.fn(async () => false),
  setAutostart: vi.fn(async () => {}),
};

vi.mock('../api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api')>()),
  api: mockApi,
}));

let assistantSetup: unknown = null;
interface FakeTask {
  resolve: (r: { text: string; structured: unknown }) => void;
  reject: (e: unknown) => void;
}
const assistantTasks: FakeTask[] = [];
vi.mock('../assistant', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../assistant')>()),
  getAssistantSetup: async () => assistantSetup,
  setAssistantSetup: async () => {},
  detectAssistants: async () => [
    { kind: 'claudeCode', path: '/usr/bin/claude', version: '2.1.0', signIn: 'apiKey' },
    { kind: 'mistralVibe', path: '/usr/bin/vibe', version: '1.0', signIn: 'notSignedIn' },
  ],
  runAssistant: vi.fn(() => {
    let resolve!: FakeTask['resolve'];
    let reject!: FakeTask['reject'];
    const result = new Promise<{ text: string; structured: unknown }>((res, rej) => ((resolve = res), (reject = rej)));
    result.catch(() => {});
    assistantTasks.push({ resolve, reject });
    return { runId: `r${assistantTasks.length}`, result, cancel: async () => reject({ kind: 'cancelled', message: 'x' }) };
  }),
}));

// happy-dom's Animation has no cancel(); zero-duration transitions skip it entirely.
vi.mock('svelte/transition', () => ({
  fade: () => ({ duration: 0 }),
  fly: () => ({ duration: 0 }),
}));

const { app } = await import('../state.svelte');
const { folderSuggestions } = await import('../folder-suggestions.svelte');
const { assist } = await import('../assistant-state.svelte');
const { default: App } = await import('../../App.svelte');

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

let mounted: ReturnType<typeof mount> | null = null;

async function mountReady(layout: Layout) {
  mounted = mount(App, { target: document.body });
  await settle();
  app.phase = 'ready';
  app.library = { path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 2, portable: false };
  app.settings = defaultSettings();
  app.sidebar = sidebar;
  app.layouts = { default: layout, views: {} };
  flushSync();
  await app.resetList();
  await settle();
  flushSync();
}

async function expectNoViolations(label: string) {
  const results = await axe.run(document.body, {
    runOnly: { type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'] },
    rules: { 'color-contrast': { enabled: false } },
  });
  const violations = results.violations.map(
    (v) => `${v.id} (${v.impact}): ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`,
  );
  // Guard against a vacuous run: axe must have evaluated a real rule set.
  expect((results as unknown as { passes: unknown[] }).passes.length, `${label}: axe ran rules`).toBeGreaterThan(10);
  expect(violations, `${label} must have no axe violations`).toEqual([]);
}

beforeEach(() => {
  vi.clearAllMocks();
  assistantTasks.length = 0;
  localStorage.clear();
  assistantSetup = null;
  assist.reset();
  app.settingsOpen = false;
  app.brokenFeedsOpen = false;
  app.addFeedOpen = false;
  app.selectedIndex = null;
  app.article = null;
  app.readerOpen = false;
});

afterEach(() => {
  if (mounted) void unmount(mounted);
  mounted = null;
  document.body.innerHTML = '';
});

describe('axe over every screen state', () => {
  for (const layout of ['list', 'cards', 'magazine', 'gallery'] as Layout[]) {
    it(`has no violations in the ${layout} Layout`, async () => {
      await mountReady(layout);
      await expectNoViolations(layout);
    });
  }

  it('has no violations with the slide-over reader open', async () => {
    await mountReady('cards');
    await app.selectIndex(0);
    await settle();
    flushSync();
    expect(app.readerOpen).toBe(true);
    await expectNoViolations('slide-over reader');
  });

  it('has no violations with the Assistant ask panel open in the reader (#119)', async () => {
    assistantSetup = {
      kind: 'claudeCode',
      path: '/usr/bin/claude',
      model: null,
      disclosureAccepted: true,
      targetLanguage: 'en',
    };
    const { assist } = await import('../assistant-state.svelte');
    await mountReady('cards');
    await app.selectIndex(0);
    await settle();
    flushSync();
    assist.start(assist.entries.find((e) => e.id === 'ask')!);
    flushSync();
    await settle();
    flushSync();
    expect(document.querySelector('#ask-question')).not.toBeNull();
    await expectNoViolations('assistant ask panel');
    assist.reset();
    assistantSetup = null;
  });

  it('has no violations with the Settings dialog open', async () => {
    await mountReady('list');
    await app.openSettings();
    await settle();
    flushSync();
    expect(document.querySelector('.settings-dialog')).not.toBeNull();
    await expectNoViolations('settings dialog');
  });

  for (const state of ['off', 'listed', 'set up'] as const) {
    it(`has no violations in the Assistant section (${state})`, async () => {
      assistantSetup =
        state === 'set up'
          ? { kind: 'claudeCode', path: '/usr/bin/claude', model: 'haiku', disclosureAccepted: false }
          : null;
      await mountReady('list');
      await app.openSettings();
      await settle();
      flushSync();
      const tab = [...document.querySelectorAll<HTMLButtonElement>('[role="tab"]')].find(
        (el) => el.textContent?.trim() === 'Assistant',
      )!;
      tab.click();
      await settle();
      flushSync();
      if (state === 'listed') {
        [...document.querySelectorAll<HTMLButtonElement>('button')]
          .find((b) => b.textContent?.trim() === 'Set up')!
          .click();
        await settle();
        flushSync();
        expect(document.querySelector('[role="radiogroup"]')).not.toBeNull();
      }
      await expectNoViolations(`assistant section (${state})`);
    });
  }

  it('has no violations with the Broken Feeds dialog open, and while confirming', async () => {
    await mountReady('list');
    app.brokenFeedsOpen = true;
    await settle();
    flushSync();
    expect(document.querySelector('.broken-feeds-dialog')).not.toBeNull();
    await expectNoViolations('broken feeds dialog');
    document.querySelector<HTMLButtonElement>('[aria-label="Unsubscribe from Rust Blog"]')!.click();
    await settle();
    flushSync();
    await expectNoViolations('broken feeds confirmation');
    app.brokenFeedsOpen = false;
  });

  it('has no violations with the Assistant View panel open while running, done and failed', async () => {
    assistantSetup = { kind: 'claudeCode', path: '/usr/bin/claude', model: 'haiku', disclosureAccepted: true, interests: 'Rust' };
    await mountReady('list');
    const entry = assist.viewEntries.find((e) => e.id === 'briefing')!;
    assist.start(entry);
    flushSync();
    await settle();
    expect(document.querySelector('.vpanel')).not.toBeNull();
    await expectNoViolations('assistant view panel (running)');
    assistantTasks.at(-1)!.resolve({
      text: '',
      structured: {
        considered: 4,
        capped: false,
        topics: [{ topic: 'Rust', sentence: 'Four posts about Rust.', articleIds: [1, 2] }],
        articles: [
          { id: 1, title: 'Article 1', feedTitle: 'Rust Blog' },
          { id: 2, title: 'Article 2', feedTitle: 'Rust Blog' },
        ],
      },
    });
    await settle();
    flushSync();
    await settle();
    flushSync();
    expect(document.querySelector('.vpanel .topics')).not.toBeNull();
    await expectNoViolations('assistant view panel (done)');
    assist.start(assist.viewEntries.find((e) => e.id === 'picks')!);
    flushSync();
    await settle();
    assistantTasks.at(-1)!.reject({ kind: 'noInterests', message: 'x' });
    await settle();
    flushSync();
    expect(document.querySelector('.vpanel .error')).not.toBeNull();
    await expectNoViolations('assistant view panel (error)');
    assist.discardView();
  });

  it('has no violations with the command palette open', async () => {
    await mountReady('list');
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', ctrlKey: true, bubbles: true, cancelable: true }));
    await settle();
    flushSync();
    expect(document.querySelector('[role="combobox"]')).not.toBeNull();
    await expectNoViolations('command palette');
  });

  it('has no violations with the Folder suggestions dialog open, and the Assistant palette entry (#123)', async () => {
    assistantSetup = { kind: 'claudeCode', path: '/usr/bin/claude', model: 'haiku', disclosureAccepted: true };
    await mountReady('list');
    await settle();
    flushSync();
    // While a batch runs: progress line, Stop, live region.
    folderSuggestions.open = true;
    folderSuggestions.phase = 'running';
    folderSuggestions.scope = { kind: 'topLevel' };
    folderSuggestions.totalFeeds = 1035;
    folderSuggestions.doneFeeds = 100;
    folderSuggestions.batch = 2;
    folderSuggestions.batches = 11;
    folderSuggestions.announcement = 'Group 1 of 11 done';
    flushSync();
    expect(document.querySelector('.folder-suggestions-dialog')).not.toBeNull();
    await expectNoViolations('folder suggestions, running');
    // With proposals: checkboxes, accept/reject all, Apply.
    folderSuggestions.phase = 'done';
    folderSuggestions.proposals = [
      { feedId: 7, feedTitle: 'Hacker News', folder: 'Tech', isNewFolder: false, accepted: true },
      { feedId: 8, feedTitle: 'A rather long Feed title that must ellipsize', folder: 'Cooking', isNewFolder: true, accepted: false },
    ];
    folderSuggestions.announcement = 'Suggestions ready';
    flushSync();
    await expectNoViolations('folder suggestions, proposals');
    folderSuggestions.reset();
    // The palette lists the Assistant entry only with the setup accepted.
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', ctrlKey: true, bubbles: true, cancelable: true }));
    await settle();
    flushSync();
    const labels = [...document.querySelectorAll('[role="option"]')].map((o) => o.textContent ?? '');
    expect(labels.some((l) => l.includes('Suggest Folders'))).toBe(true);
    await expectNoViolations('command palette with Assistant entry');
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', ctrlKey: true, bubbles: true, cancelable: true }));
    folderSuggestions.reset();
  });

  // #122: the Assistant search trigger next to the search field, idle and busy.
  it('has no violations with the Assistant search trigger shown and busy', async () => {
    assistantSetup = { kind: 'claudeCode', path: '/usr/bin/claude', model: 'haiku', disclosureAccepted: true };
    await mountReady('list');
    await settle();
    flushSync();
    const trigger = document.querySelector<HTMLButtonElement>('#assistant-search');
    expect(trigger, 'the trigger is shown once an Assistant is set up').not.toBeNull();
    expect(trigger!.getAttribute('aria-label')).toBe('Ask the Assistant to search');
    await expectNoViolations('assistant search trigger');
    assist.searchRun = 'running';
    flushSync();
    expect(trigger!.getAttribute('aria-busy')).toBe('true');
    await expectNoViolations('assistant search trigger busy');
  });
});
