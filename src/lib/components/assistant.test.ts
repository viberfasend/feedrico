// @vitest-environment happy-dom
// The Assistant panel in the reader (#115): hidden without a setup, streams, renders the done
// text from sanitized Markdown, shows every error with a next step, Esc stops, j/k discards.
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import axeModule from 'axe-core';
import type { ArticleDetail } from '../api';
import type { AssistantErrorKind, AssistantEvent, AssistantResult, AssistantSetup } from '../assistant';

const mockApi = { openExternal: vi.fn(async (_url: string) => {}), article: vi.fn(), list: vi.fn(async () => []) };

interface FakeTask {
  emit: (e: AssistantEvent) => void;
  resolve: (r: AssistantResult) => void;
  reject: (e: unknown) => void;
  cancel: ReturnType<typeof vi.fn>;
}
const tasks: FakeTask[] = [];
/** The action each started run was given, in order. */
const actions: unknown[] = [];
let setup: AssistantSetup | null = null;

vi.mock('../api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api')>()),
  api: mockApi,
  mediaUrl: (key: string) => `media://localhost/${key}`,
}));
vi.mock('../assistant', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../assistant')>()),
  getAssistantSetup: vi.fn(async () => setup),
  runAssistant: vi.fn((action: unknown, onEvent: (e: AssistantEvent) => void) => {
    actions.push(action);
    let resolve!: FakeTask['resolve'];
    let reject!: FakeTask['reject'];
    const result = new Promise<AssistantResult>((res, rej) => ((resolve = res), (reject = rej)));
    result.catch(() => {});
    const cancel = vi.fn(async () => reject({ kind: 'cancelled', message: 'Cancelled' }));
    tasks.push({ emit: onEvent, resolve, reject, cancel });
    return { runId: `r${tasks.length}`, result, cancel };
  }),
}));

const { app } = await import('../state.svelte');
const { assist } = await import('../assistant-state.svelte');
const { default: Reader } = await import('./Reader.svelte');

const article = (id = 1): ArticleDetail => ({
  id, feedId: 1, feedTitle: 'Blog', title: `Post ${id}`, excerpt: '', articleDate: 1_700_000_000,
  isRead: true, isStarred: false, leadImage: null, favicon: null, feedSiteUrl: null, url: null,
  author: null, contentHtml: '<p>Body</p>', enclosures: [],
});

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
let mounted: ReturnType<typeof mount> | null = null;
const q = (sel: string) => document.querySelector<HTMLElement>(sel);
const press = (key: string, init: KeyboardEventInit = {}) => {
  window.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init }));
  flushSync();
};

async function render(s: AssistantSetup | null = { kind: 'claudeCode', path: '/bin/claude', model: null, disclosureAccepted: true }) {
  setup = s;
  app.phase = 'ready';
  app.article = article();
  mounted = mount(Reader, { target: document.body });
  await settle();
  flushSync();
}

async function start() {
  assist.start(assist.entries[0]!);
  flushSync();
  await settle();
  return tasks.at(-1)!;
}

beforeEach(() => {
  tasks.length = 0;
  actions.length = 0;
  mockApi.openExternal.mockClear();
  Object.defineProperty(navigator, 'clipboard', { value: { writeText: vi.fn(async () => {}) }, configurable: true });
});

afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  assist.reset();
  app.article = null;
  document.body.innerHTML = '';
});

describe('visibility', () => {
  it('is hidden completely without a setup: no panel, no toolbar button, no live region', async () => {
    await render(null);
    expect(q('[aria-label="Assistant"]')).toBeNull();
    expect(document.body.textContent).not.toContain('Assistant');
    assist.start(assist.entries[0]!);
    flushSync();
    expect(tasks).toHaveLength(0);
    press('i');
    expect(assist.menuOpen).toBe(false);
  });

  it('stays hidden while the disclosure is not accepted, and runs nothing', async () => {
    await render({ kind: 'claudeCode', path: '/bin/claude', model: null, disclosureAccepted: false });
    expect(q('[aria-label="Assistant"]')).toBeNull();
    assist.start(assist.entries[0]!);
    flushSync();
    expect(tasks).toHaveLength(0);
    press('i');
    expect(assist.menuOpen).toBe(false);
  });

  it('shows the toolbar button when a setup exists, and the shortcut opens its menu', async () => {
    await render();
    expect(q('.assistant-trigger')).not.toBeNull();
    press('i');
    expect(assist.menuOpen).toBe(true);
  });

  it('ignores the shortcut while typing', async () => {
    await render();
    const input = document.createElement('input');
    document.body.append(input);
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'i', bubbles: true }));
    flushSync();
    expect(assist.menuOpen).toBe(false);
  });
});

describe('a run', () => {
  it('streams text, labels the writer, and does not announce tokens', async () => {
    await render();
    const task = await start();
    task.emit({ kind: 'text', delta: 'Hello ' });
    task.emit({ kind: 'text', delta: 'wor' });
    flushSync();
    expect(q('.stream')!.textContent).toBe('Hello wor');
    expect(q('.panel')!.textContent).toContain('Written by Claude Code · may be wrong');
    expect(q('[aria-live="polite"]')!.textContent).toBe('');
  });

  it('labels Mistral Vibe by name', async () => {
    await render({ kind: 'mistralVibe', path: '/bin/vibe', model: null, disclosureAccepted: true });
    await start();
    expect(q('.panel')!.textContent).toContain('Written by Mistral Vibe · may be wrong');
  });

  it('renders the done text as Markdown and announces completion once', async () => {
    await render();
    const task = await start();
    task.emit({ kind: 'text', delta: 'partial' });
    task.resolve({ text: 'All **good**.\n\n- one\n- two', structured: null });
    await settle();
    flushSync();
    expect(q('.rendered strong')!.textContent).toBe('good');
    expect(q('.rendered ul li')).not.toBeNull();
    expect(q('.stream')).toBeNull();
    expect(q('[aria-live="polite"]')!.textContent).toBe('Answer ready');
  });

  it('removes <script>, onerror= and javascript: links from the output', async () => {
    await render();
    const task = await start();
    task.resolve({
      text: '<script>window.x=1</script>\n\n<img src=x onerror="window.x=2">\n\n[a](javascript:alert(1)) and [b](https://example.com/ok)',
      structured: null,
    });
    await settle();
    flushSync();
    const box = q('.rendered')!;
    expect(box.querySelector('script, img')).toBeNull();
    expect(box.querySelector('[onerror]')).toBeNull();
    expect(box.querySelector('a[href^="javascript"]')).toBeNull();
    expect(box.textContent).toContain('a and');
    expect([...box.querySelectorAll('a')].map((a) => a.getAttribute('href'))).toEqual(['https://example.com/ok']);
  });

  it('opens links through the system browser', async () => {
    await render();
    const task = await start();
    task.resolve({ text: '[b](https://example.com/ok)', structured: null });
    await settle();
    flushSync();
    q('.rendered a')!.click();
    expect(mockApi.openExternal).toHaveBeenCalledWith('https://example.com/ok');
  });

  it('never moves focus into the panel', async () => {
    await render();
    const before = document.activeElement;
    const task = await start();
    task.resolve({ text: 'x', structured: null });
    await settle();
    flushSync();
    expect(document.activeElement).toBe(before);
  });

  it('copies the plain text', async () => {
    await render();
    const task = await start();
    task.resolve({ text: 'Copy **me**', structured: null });
    await settle();
    flushSync();
    q('[data-action="copy"]')!.click();
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith('Copy **me**');
  });

  it('tries again with a fresh run', async () => {
    await render();
    const task = await start();
    task.resolve({ text: 'one', structured: null });
    await settle();
    flushSync();
    q('[data-action="retry"]')!.click();
    flushSync();
    expect(tasks).toHaveLength(2);
    expect(q('.stream')).not.toBeNull();
  });

  it('closes and discards the result', async () => {
    await render();
    const task = await start();
    task.resolve({ text: 'one', structured: null });
    await settle();
    flushSync();
    q('[data-action="close"]')!.click();
    flushSync();
    expect(q('.panel')).toBeNull();
    expect(assist.run).toBeNull();
  });
});

describe('Esc and navigation', () => {
  it('Esc stops the run, keeps what arrived, and does not reach other Esc handlers', async () => {
    await render();
    const task = await start();
    task.emit({ kind: 'text', delta: 'half an ans' });
    const other = vi.fn();
    window.addEventListener('keydown', other);
    press('Escape');
    window.removeEventListener('keydown', other);
    await settle();
    flushSync();
    expect(task.cancel).toHaveBeenCalledTimes(1);
    expect(other).not.toHaveBeenCalled();
    expect(assist.run!.phase).toBe('stopped');
    expect(q('.panel')!.textContent).toContain('half an ans');
    expect(q('.panel')!.textContent).toContain('Stopped');
  });

  it('Esc is left alone when nothing is running', async () => {
    await render();
    const other = vi.fn();
    window.addEventListener('keydown', other);
    press('Escape');
    window.removeEventListener('keydown', other);
    expect(other).toHaveBeenCalled();
  });

  it('moving to another Article (j/k) cancels the run and discards the result', async () => {
    await render();
    const task = await start();
    task.emit({ kind: 'text', delta: 'abc' });
    app.article = article(2);
    await settle();
    flushSync();
    expect(task.cancel).toHaveBeenCalledTimes(1);
    expect(assist.run).toBeNull();
    expect(q('.panel')).toBeNull();
    // A late result from the old run must not come back.
    task.resolve({ text: 'late', structured: null });
    await settle();
    flushSync();
    expect(assist.run).toBeNull();
  });

  it('closing the reader cancels the run', async () => {
    await render();
    const task = await start();
    unmount(mounted!);
    mounted = null;
    expect(task.cancel).toHaveBeenCalledTimes(1);
    expect(assist.run).toBeNull();
  });
});

describe('errors', () => {
  const cases: [AssistantErrorKind, RegExp, RegExp][] = [
    ['notInstalled', /can't be found/, /Settings/],
    ['notSignedIn', /isn't signed in/, /run `claude`/],
    ['usageLimit', /usage limit/, /try again/i],
    ['offline', /reach Anthropic/, /internet connection/],
    ['timedOut', /too long/, /try again/i],
    ['malformed', /couldn't read/, /try again/i],
    ['failed', /couldn't finish/, /try again/i],
    ['notSetUp', /No Assistant is set up/, /Settings/],
  ];
  for (const [kind, what, next] of cases) {
    it(`explains ${kind} in plain words with a next step`, async () => {
      await render();
      const task = await start();
      task.reject({ kind, message: 'raw detail' });
      await settle();
      flushSync();
      const error = q('.error')!;
      expect(error.textContent).toMatch(what);
      expect(q('.next')!.textContent).toMatch(next);
      expect(q('[aria-live="polite"]')!.textContent).toMatch(what);
      expect(q('[data-action="retry"]')).not.toBeNull();
    });
  }

  it('treats a cancelled error as a stop, not a failure', async () => {
    await render();
    const task = await start();
    task.reject({ kind: 'cancelled', message: 'Cancelled' });
    await settle();
    flushSync();
    expect(q('.error')).toBeNull();
    expect(assist.run!.phase).toBe('stopped');
  });

  it('falls back to "failed" for an unknown error', async () => {
    await render();
    const task = await start();
    task.reject(new Error('boom'));
    await settle();
    flushSync();
    expect(q('.error')!.textContent).toMatch(/couldn't finish/);
  });
});

describe('command palette', () => {
  async function paletteLabels() {
    const { default: CommandPalette } = await import('./CommandPalette.svelte');
    const palette = mount(CommandPalette, { target: document.body, props: { open: true } });
    await settle();
    flushSync();
    const labels = [...document.querySelectorAll('[role="option"]')].map((o) => o.textContent ?? '');
    unmount(palette);
    return labels.join('|');
  }

  it('lists the Summarize and Explain entries only with a setup and an Article', async () => {
    await render();
    const labels = await paletteLabels();
    expect(labels).toContain('Assistant: Summarize this Article');
    expect(labels).toContain('Assistant: Explain this Article in plain language');
    expect(labels).not.toContain('Check it works');
  });

  it('lists no Assistant entry without a setup', async () => {
    await render(null);
    expect(await paletteLabels()).not.toContain('Assistant');
  });
});

describe('entries and the Summarize shortcut (#116)', () => {
  it('the reader menu lists Summarize and Explain simply', async () => {
    await render();
    press('i');
    expect(assist.menuOpen).toBe(true);
    const labels = [...document.querySelectorAll('.menu-item')].map((el) => el.textContent?.trim());
    expect(labels).toContain('Summarize (u)');
    expect(labels).toContain('Explain simply');
  });

  it('u starts Summarize on the open Article', async () => {
    await render();
    press('u');
    flushSync();
    expect(tasks).toHaveLength(1);
    expect(actions[0]).toEqual({ kind: 'summarize', articleId: 1 });
    expect(q('.panel')!.getAttribute('aria-label')).toBe('Assistant: Summary');
  });

  it('u does nothing without a setup or while typing', async () => {
    await render(null);
    press('u');
    expect(tasks).toHaveLength(0);
    await render();
    const input = document.createElement('input');
    document.body.append(input);
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'u', bubbles: true }));
    flushSync();
    expect(tasks).toHaveLength(0);
  });

  it('the menu and palette start Explain simply with the open Article', async () => {
    await render();
    assist.startEntry('explain');
    flushSync();
    expect(actions[0]).toEqual({ kind: 'explain', articleId: 1 });
  });
});

describe('panel notes about the input (#116)', () => {
  it('shows the excerpt-only and truncation notes from the result', async () => {
    await render();
    const task = await start();
    task.resolve({
      text: 'A short answer.',
      structured: null,
      article: { excerptOnly: true, truncated: true, wordsUsed: 1234 },
    });
    await settle();
    flushSync();
    const notes = [...document.querySelectorAll('.panel .note')].map((n) => n.textContent?.trim());
    expect(notes).toContain('This Feed only gives an excerpt; the summary covers that excerpt.');
    expect(notes).toContain('This Article is long; the answer covers the first 1234 words.');
  });

  it('shows no notes when the whole Article was sent', async () => {
    await render();
    const task = await start();
    task.resolve({ text: 'A short answer.', structured: null, article: { excerptOnly: false, truncated: false, wordsUsed: 80 } });
    await settle();
    flushSync();
    expect(document.querySelectorAll('.panel .note')).toHaveLength(0);
  });

  it('shows no notes while the run has not finished', async () => {
    await render();
    await start();
    flushSync();
    expect(document.querySelectorAll('.panel .note')).toHaveLength(0);
  });
});

describe('explain the selected term (#118)', () => {
  const richArticle = (): ArticleDetail => ({
    ...article(),
    contentHtml: '<p>First para.</p><p>The <b>quota</b> is a limit on requests. It resets daily.</p>',
  });

  function selectTerm() {
    const b = document.querySelector('#pane-reader .content b');
    expect(b).not.toBeNull();
    const range = document.createRange();
    range.selectNodeContents(b!);
    const sel = window.getSelection()!;
    sel.removeAllRanges();
    sel.addRange(range);
    document.dispatchEvent(new Event('selectionchange'));
    flushSync();
  }

  async function paletteOptions() {
    const { default: CommandPalette } = await import('./CommandPalette.svelte');
    const palette = mount(CommandPalette, { target: document.body, props: { open: true } });
    await settle();
    flushSync();
    const options = [...document.querySelectorAll('[role="option"]')].map((o) => ({
      label: o.textContent ?? '',
      el: o as HTMLElement,
    }));
    return { palette, options };
  }

  it('the palette lists "Explain the selected term" only with a selection, and runs it with term and paragraph', async () => {
    app.phase = 'ready';
    app.article = richArticle();
    setup = { kind: 'claudeCode', path: '/bin/claude', model: null, disclosureAccepted: true };
    mounted = mount(Reader, { target: document.body });
    await settle();
    flushSync();

    // No selection: the entry is not offered.
    let { palette, options } = await paletteOptions();
    expect(options.map((o) => o.label).join('|')).not.toContain('Explain the selected term');
    unmount(palette);

    // Select the term, open the palette again (it captures the selection), pick the entry.
    selectTerm();
    ({ palette, options } = await paletteOptions());
    const entry = options.find((o) => o.label.includes('Explain the selected term'));
    expect(entry).toBeDefined();
    entry!.el.click();
    await settle();
    flushSync();
    unmount(palette);
    expect(actions.at(-1)).toEqual({
      kind: 'explainTerm',
      articleId: 1,
      term: 'quota',
      context: 'The quota is a limit on requests. It resets daily.',
    });
  });

  it('the reader menu hides the entry without a selection and shows it with one', async () => {
    await render();
    press('i');
    let labels = [...document.querySelectorAll('.menu-item')].map((el) => el.textContent?.trim());
    expect(labels).not.toContain('Explain selection');
    assist.menuOpen = false;
    flushSync();

    app.article = richArticle();
    await settle();
    flushSync();
    selectTerm();
    press('i');
    labels = [...document.querySelectorAll('.menu-item')].map((el) => el.textContent?.trim());
    expect(labels).toContain('Explain selection');
  });

  it('a selection outside the reader does not offer the entry', async () => {
    await render();
    const outside = document.createElement('p');
    outside.textContent = 'somewhere else';
    document.body.append(outside);
    const range = document.createRange();
    range.selectNodeContents(outside);
    const sel = window.getSelection()!;
    sel.removeAllRanges();
    sel.addRange(range);
    document.dispatchEvent(new Event('selectionchange'));
    flushSync();
    press('i');
    const labels = [...document.querySelectorAll('.menu-item')].map((el) => el.textContent?.trim());
    expect(labels).not.toContain('Explain selection');
  });
});

describe('accessibility', () => {
  const axe = axeModule as unknown as {
    run: (ctx: unknown, opts: unknown) => Promise<{ violations: { id: string; nodes: { target: string[] }[] }[] }>;
  };
  async function expectNoViolations() {
    const results = await axe.run(document.body, {
      runOnly: { type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'] },
      rules: { 'color-contrast': { enabled: false } },
    });
    expect(results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' '))}`)).toEqual([]);
  }

  it('has no violations while running, done with input notes, and failed', async () => {
    await render();
    const task = await start();
    task.emit({ kind: 'text', delta: 'Streaming' });
    flushSync();
    await expectNoViolations();
    task.resolve({
      text: 'Done with [a link](https://example.com).\n\n- one\n- two',
      structured: null,
      article: { excerptOnly: true, truncated: true, wordsUsed: 4321 },
    });
    await settle();
    flushSync();
    expect(q('.panel')!.textContent).toContain('excerpt');
    await expectNoViolations();
    q('[data-action="retry"]')!.click();
    flushSync();
    tasks.at(-1)!.reject({ kind: 'notSignedIn', message: 'x' });
    await settle();
    flushSync();
    expect(q('.error')).not.toBeNull();
    await expectNoViolations();
  });
});
