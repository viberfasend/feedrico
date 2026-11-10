// UI timing harness (spec: "Performance budgets"). In perf mode the shell opens a generated
// Library and this script drives the real app: View switches and next-Article steps, each timed
// until its result is in the DOM. Never runs for users: see `src-tauri/src/perf.rs`.

import type { Selection } from './state.svelte';

export const BUDGETS = {
  startMs: 500,
  switchMs: 50,
  /** `j`/`k` to the next Article rendered: one frame at 60 Hz. */
  nextMs: 16.7,
};

const ROUNDS = 3;
const NEXT_STEPS = 50;
/** A rare term in the generated Library, like a real search. */
const SEARCH = 'zephyrine';

export interface Stats {
  n: number;
  p50: number;
  p95: number;
  max: number;
}

export interface PerfReport {
  interactiveMs: number;
  switches: Record<string, Stats>;
  next: Stats;
  /** Budgets missed, judged by the median. Reported, never gating. */
  over: string[];
}

/** The slice of the app state the harness drives. */
export interface PerfTarget {
  readonly sidebar: { folders: { id: number; feeds: { id: number }[] }[]; feeds: { id: number }[] } | null;
  readonly article: { id: number } | null;
  select(selection: Selection): Promise<void>;
  setSearch(search: string): Promise<void>;
  move(delta: number): Promise<void>;
}

export interface PerfDeps {
  now: () => number;
  /** Resolves once pending state changes are in the DOM. */
  flush: () => Promise<void>;
  /** Milliseconds from process start to now, measured by the shell. */
  interactive: () => Promise<number>;
}

const round = (ms: number) => Math.round(ms * 10) / 10;

export function summarize(samples: number[]): Stats {
  if (samples.length === 0) return { n: 0, p50: 0, p95: 0, max: 0 };
  const sorted = [...samples].sort((a, b) => a - b);
  const at = (q: number) => sorted[Math.min(sorted.length - 1, Math.ceil(q * sorted.length) - 1)] ?? 0;
  return { n: sorted.length, p50: round(at(0.5)), p95: round(at(0.95)), max: round(sorted.at(-1) ?? 0) };
}

export async function runPerf(app: PerfTarget, deps: PerfDeps): Promise<PerfReport> {
  const interactiveMs = Math.round(await deps.interactive());
  const timed = async (step: () => Promise<void>) => {
    const start = deps.now();
    await step();
    await deps.flush();
    return deps.now() - start;
  };

  const folder = app.sidebar?.folders.find((f) => f.feeds.length > 0);
  const feed = app.sidebar?.feeds[0] ?? folder?.feeds[0];
  const views: [string, () => Promise<void>][] = [
    ['All Unread', () => app.select({ kind: 'allUnread' })],
    ['Starred', () => app.select({ kind: 'starred' })],
    ['Today', () => app.select({ kind: 'today' })],
    ...(folder ? [['Folder', () => app.select({ kind: 'folder', id: folder.id })] as [string, () => Promise<void>]] : []),
    ...(feed ? [['Feed', () => app.select({ kind: 'feed', id: feed.id })] as [string, () => Promise<void>]] : []),
    ['Search', () => app.setSearch(SEARCH)],
  ];
  const samples = new Map<string, number[]>(views.map(([name]) => [name, []]));
  for (let round = 0; round < ROUNDS; round++) {
    for (const [name, step] of views) samples.get(name)?.push(await timed(step));
  }
  const switches = Object.fromEntries([...samples].map(([name, s]) => [name, summarize(s)]));

  await app.select({ kind: 'allUnread' });
  await deps.flush();
  const steps: number[] = [];
  for (let i = 0; i < NEXT_STEPS; i++) steps.push(await timed(() => app.move(1)));
  const next = summarize(steps);

  const over: string[] = [];
  if (interactiveMs > BUDGETS.startMs) over.push(`start: ${interactiveMs} ms > ${BUDGETS.startMs} ms`);
  for (const [name, stats] of Object.entries(switches)) {
    if (stats.p50 > BUDGETS.switchMs) over.push(`switch to ${name}: ${stats.p50} ms > ${BUDGETS.switchMs} ms`);
  }
  if (next.p50 > BUDGETS.nextMs) over.push(`next Article: ${next.p50} ms > ${BUDGETS.nextMs} ms`);
  return { interactiveMs, switches, next, over };
}

/** Runs the harness in the real app, hands the report to the shell, which prints it and exits. */
export async function perfSession(app: PerfTarget) {
  const { tick } = await import('svelte');
  const { api } = await import('./api');
  const frame = () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
  try {
    const report = await runPerf(app, {
      now: () => performance.now(),
      flush: () => tick(),
      interactive: async () => {
        await tick();
        await frame();
        return api.perfInteractive();
      },
    });
    await api.perfFinish(report);
  } catch (e) {
    await api.perfFinish({ error: String(e) });
  }
}
