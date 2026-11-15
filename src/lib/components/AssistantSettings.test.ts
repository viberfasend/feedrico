// @vitest-environment happy-dom
// Settings > Assistant (#114): off by default, set up by detection or a path, a model, the
// disclosure before the first test, a Test with streamed text and elapsed time, plain errors,
// and Turn off. The setup is machine-local; the Library is never involved.
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AssistantEvent, AssistantSetup, FoundAssistant } from '../assistant';

const claude = (signIn: FoundAssistant['signIn']): FoundAssistant => ({
  kind: 'claudeCode',
  path: '/usr/bin/claude',
  version: '2.1.0',
  signIn,
});
const vibe: FoundAssistant = { kind: 'mistralVibe', path: '/usr/bin/vibe', version: '1.0', signIn: 'notSignedIn' };

let stored: AssistantSetup | null = null;
let events: AssistantEvent[] = [];
let runResult: Promise<{ text: string; structured: null }>;

const mockAssistant = {
  getAssistantSetup: vi.fn(async () => stored),
  setAssistantSetup: vi.fn(async (s: AssistantSetup | null) => {
    stored = s;
  }),
  detectAssistants: vi.fn(async (_p: string | null = null) => [] as FoundAssistant[]),
  runAssistant: vi.fn((_a: unknown, onEvent: (e: AssistantEvent) => void) => {
    for (const e of events) onEvent(e);
    return { runId: 'r', result: runResult, cancel: async () => {} };
  }),
};

vi.mock('../assistant', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../assistant')>()),
  ...mockAssistant,
}));

const { default: AssistantSettings } = await import('./AssistantSettings.svelte');

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
let mounted: ReturnType<typeof mount> | null = null;

async function open() {
  mounted = mount(AssistantSettings, { target: document.body });
  await settle();
  flushSync();
}
const text = () => document.body.textContent?.replace(/\s+/g, ' ') ?? '';
const button = (name: string) =>
  [...document.querySelectorAll<HTMLButtonElement>('button')].find((b) => b.textContent?.trim() === name)!;
async function click(el: Element) {
  (el as HTMLElement).click();
  await settle();
  flushSync();
  await settle();
  flushSync();
}

beforeEach(() => {
  vi.clearAllMocks();
  stored = null;
  events = [];
  runResult = Promise.resolve({ text: 'Hello', structured: null });
  mockAssistant.detectAssistants.mockImplementation(async () => []);
});
afterEach(() => {
  if (mounted) void unmount(mounted);
  mounted = null;
  document.body.innerHTML = '';
});

describe('Assistant settings', () => {
  it('is off by default: one sentence and a Set up button, nothing detected or run', async () => {
    await open();
    expect(text()).toContain('An Assistant is an AI tool');
    expect(button('Set up')).toBeTruthy();
    expect(mockAssistant.detectAssistants).not.toHaveBeenCalled();
    expect(mockAssistant.runAssistant).not.toHaveBeenCalled();
    expect(document.querySelector('[role="radiogroup"]')).toBeNull();
  });

  it('says so while looking, then when nothing is found', async () => {
    let release!: (v: FoundAssistant[]) => void;
    mockAssistant.detectAssistants.mockImplementation(() => new Promise((r) => (release = r)));
    await open();
    await click(button('Set up'));
    expect(text()).toContain('Looking for Assistants');
    release([]);
    await settle();
    flushSync();
    expect(text()).toContain('No Assistant found');
    expect(document.querySelector('input[placeholder*="claude"]')).not.toBeNull();
  });

  it('lists each sign-in state in plain words', async () => {
    mockAssistant.detectAssistants.mockImplementation(async () => [claude('plan'), vibe]);
    await open();
    await click(button('Set up'));
    expect(text()).toContain('Claude Code 2.1.0');
    expect(text()).toContain('signed in with your Claude plan');
    expect(text()).toContain('Mistral Vibe 1.0');
    expect(text()).toContain('not signed in: run vibe --setup');

    mockAssistant.detectAssistants.mockImplementation(async () => [claude('apiKey')]);
    await click(button('Look again'));
    expect(text()).toContain('signed in with an API key: runs are billed per use');

    mockAssistant.detectAssistants.mockImplementation(async () => [claude('notSignedIn')]);
    await click(button('Look again'));
    expect(text()).toContain('not signed in: open a terminal and run claude to sign in');
  });

  it('looks again with a manual path', async () => {
    await open();
    await click(button('Set up'));
    const input = document.querySelector<HTMLInputElement>('input[placeholder*="claude"]')!;
    input.value = '/opt/claude';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    await click(button('Look again'));
    expect(mockAssistant.detectAssistants).toHaveBeenLastCalledWith('/opt/claude');
  });

  it('picking an Assistant saves it with haiku, no accepted disclosure and the system language', async () => {
    mockAssistant.detectAssistants.mockImplementation(async () => [claude('plan')]);
    await open();
    await click(button('Set up'));
    await click(document.querySelector('input[type="radio"]')!);
    expect(stored).toEqual({
      kind: 'claudeCode',
      path: '/usr/bin/claude',
      model: 'haiku',
      disclosureAccepted: false,
      targetLanguage: navigator.language,
      interests: '',
    });
    expect(text()).toContain('gives the Article\'s text to Claude Code on this computer, which sends it to Anthropic under your Claude plan');
    expect(button('Test').disabled, 'Test waits for the disclosure').toBe(true);
  });

  it('has a translation language field that defaults to the system language and saves a change', async () => {
    stored = { kind: 'claudeCode', path: '/usr/bin/claude', model: 'haiku', disclosureAccepted: true };
    await open();
    const language = document.querySelector<HTMLInputElement>('input[id$="-language"]')!;
    expect(language).not.toBeNull();
    expect(language.value).toBe('');
    expect(language.placeholder).toBe(navigator.language);
    expect(text()).toContain('Translation language');
    language.value = 'de';
    language.dispatchEvent(new Event('change', { bubbles: true }));
    await settle();
    expect(stored?.targetLanguage).toBe('de');
    // Empty means the system language again.
    language.value = '';
    language.dispatchEvent(new Event('change', { bubbles: true }));
    await settle();
    expect(stored?.targetLanguage).toBeNull();
  });

  it('names Mistral for Vibe and has a free-text model, empty by default', async () => {
    stored = { kind: 'mistralVibe', path: '/usr/bin/vibe', model: null, disclosureAccepted: false, interests: '' };
    await open();
    expect(text()).toContain('sends it to Mistral. Nothing is sent unless you ask.');
    expect(text()).not.toContain('Claude plan');
    const model = document.querySelector<HTMLInputElement>('input[placeholder="Default"]')!;
    expect(model.value).toBe('');
    model.value = 'devstral';
    model.dispatchEvent(new Event('change', { bubbles: true }));
    await settle();
    expect(stored?.model).toBe('devstral');
  });

  it('offers default, haiku, sonnet and opus for Claude, and saves the choice', async () => {
    stored = { kind: 'claudeCode', path: '/usr/bin/claude', model: 'haiku', disclosureAccepted: false, interests: '' };
    await open();
    const select = document.querySelector<HTMLSelectElement>('select[aria-label="Model"]')!;
    expect([...select.options].map((o) => o.value)).toEqual(['', 'haiku', 'sonnet', 'opus']);
    expect(select.value).toBe('haiku');
    select.value = 'opus';
    select.dispatchEvent(new Event('change', { bubbles: true }));
    await settle();
    expect(stored?.model).toBe('opus');
    select.value = '';
    select.dispatchEvent(new Event('change', { bubbles: true }));
    await settle();
    expect(stored?.model).toBeNull();
  });

  it('accepting the disclosure enables Test; Test streams the answer and shows the time', async () => {
    stored = { kind: 'claudeCode', path: '/usr/bin/claude', model: 'haiku', disclosureAccepted: false, interests: '' };
    await open();
    await click(document.querySelector('input[type="checkbox"]')!);
    expect(stored?.disclosureAccepted).toBe(true);
    events = [{ kind: 'text', delta: 'Hel' }];
    runResult = Promise.resolve({ text: 'Hello there', structured: null });
    await click(button('Test'));
    expect(mockAssistant.runAssistant).toHaveBeenCalledWith({ kind: 'test' }, expect.any(Function));
    const live = document.querySelector('[aria-live="polite"][aria-label="Test result"]')!;
    expect(live.textContent).toContain('Hello there');
    expect(live.textContent).toMatch(/Answered in \d+\.\d s/);
  });

  it('shows the streamed text while testing', async () => {
    stored = { kind: 'claudeCode', path: '/usr/bin/claude', model: 'haiku', disclosureAccepted: true, interests: '' };
    events = [{ kind: 'text', delta: 'Hel' }];
    runResult = new Promise(() => {});
    await open();
    await click(button('Testing…') ?? button('Test'));
    expect(document.querySelector('[aria-label="Test result"]')?.textContent).toContain('Hel');
    expect(button('Testing…').disabled).toBe(true);
  });

  it('turns plain errors into words with a next step', async () => {
    stored = { kind: 'claudeCode', path: '/usr/bin/claude', model: 'haiku', disclosureAccepted: true, interests: '' };
    await open();
    runResult = Promise.reject({ kind: 'notSignedIn', message: 'x' });
    await click(button('Test'));
    expect(document.querySelector('[aria-label="Test result"]')?.textContent).toContain(
      'Claude Code is not signed in. Sign in yourself in a terminal',
    );
    runResult = Promise.reject({ kind: 'offline', message: 'x' });
    await click(button('Test'));
    expect(document.querySelector('[aria-label="Test result"]')?.textContent).toContain('could not reach its provider');
  });

  it('Turn off deletes the setup and returns to the off state', async () => {
    stored = { kind: 'claudeCode', path: '/usr/bin/claude', model: 'haiku', disclosureAccepted: true, interests: '' };
    await open();
    await click(button('Turn off'));
    expect(mockAssistant.setAssistantSetup).toHaveBeenLastCalledWith(null);
    expect(stored).toBeNull();
    expect(button('Set up')).toBeTruthy();
    expect(text()).not.toContain('gives the Article');
  });

  it('keeps the set-up Assistant in the list, selected, when it is no longer found', async () => {
    stored = { kind: 'claudeCode', path: '/gone/claude', model: 'haiku', disclosureAccepted: true, interests: '' };
    mockAssistant.detectAssistants.mockImplementation(async () => [vibe]);
    await open();
    await click(button('Change'));
    const radios = [...document.querySelectorAll<HTMLInputElement>('input[type="radio"]')];
    expect(radios).toHaveLength(2);
    expect(radios.filter((r) => r.checked)).toHaveLength(1);
    expect(text()).toContain('Not found at /gone/claude \u2014 check the path or Look again');
  });

  it('saves the interests as free text, and keeps them when the Assistant changes', async () => {
    stored = { kind: 'claudeCode', path: '/usr/bin/claude', model: 'haiku', disclosureAccepted: true, interests: '' };
    await open();
    const area = document.querySelector<HTMLTextAreaElement>('textarea')!;
    expect(area.placeholder).toContain('Rust');
    area.value = 'Rust, local-first software; not crypto';
    area.dispatchEvent(new Event('change', { bubbles: true }));
    await settle();
    flushSync();
    expect(stored?.interests).toBe('Rust, local-first software; not crypto');
    expect(text()).toContain('never in the Library');
    // Changing the Assistant keeps the interests.
    mockAssistant.detectAssistants.mockImplementation(async () => [claude('plan'), vibe]);
    await click(button('Change'));
    await click([...document.querySelectorAll<HTMLInputElement>('input[type="radio"]')].at(-1)!);
    expect(stored?.kind).toBe('mistralVibe');
    expect(stored?.interests).toBe('Rust, local-first software; not crypto');
  });

  it('lists the set-up Assistant even when nothing is found', async () => {
    stored = { kind: 'claudeCode', path: '/gone/claude', model: 'haiku', disclosureAccepted: true, interests: '' };
    await open();
    await click(button('Change'));
    expect(text()).toContain('Not found at /gone/claude');
  });
});
