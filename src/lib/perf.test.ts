import { describe, expect, it } from 'vitest';
import { BUDGETS, runPerf, summarize, type PerfTarget } from './perf';
import type { Selection } from './state.svelte';

describe('summarize', () => {
  it('reports count, median, 95th percentile and maximum', () => {
    expect(summarize([5, 1, 3, 2, 4])).toEqual({ n: 5, p50: 3, p95: 5, max: 5 });
    expect(summarize([])).toEqual({ n: 0, p50: 0, p95: 0, max: 0 });
  });

  it('rounds to a tenth of a millisecond', () => {
    expect(summarize([1.234, 1.256]).max).toBe(1.3);
  });
});

/** A fake app on a fake clock: every action advances time by what it "costs". */
function fakeApp(costs: { switchMs: (s: Selection, search: string) => number; nextMs: number }) {
  let clock = 0;
  const calls: string[] = [];
  let articleId = 0;
  const target: PerfTarget = {
    sidebar: { folders: [{ id: 3, feeds: [{ id: 42 }] }], feeds: [{ id: 7 }] },
    get article() {
      return articleId ? { id: articleId } : null;
    },
    async select(s) {
      calls.push(`select:${s.kind}${'id' in s ? `:${s.id}` : ''}`);
      clock += costs.switchMs(s, '');
    },
    async setSearch(q) {
      calls.push(`search:${q}`);
      clock += costs.switchMs({ kind: 'allUnread' }, q);
    },
    async move(delta) {
      calls.push(`move:${delta}`);
      articleId += 1;
      clock += costs.nextMs;
    },
  };
  return { target, calls, now: () => clock, advance: (ms: number) => (clock += ms) };
}

describe('runPerf', () => {
  it('times every View switch three times and 50 next-Article steps', async () => {
    const { target, calls, now } = fakeApp({
      switchMs: (_s, search) => (search ? 80 : 10),
      nextMs: 5,
    });

    const report = await runPerf(target, { now, flush: async () => {}, interactive: async () => 321 });

    expect(report.interactiveMs).toBe(321);
    expect(Object.keys(report.switches)).toEqual(['All Unread', 'Starred', 'Today', 'Folder', 'Feed', 'Search']);
    expect(report.switches['Starred']).toEqual({ n: 3, p50: 10, p95: 10, max: 10 });
    expect(report.switches['Search']?.p50).toBe(80);
    expect(report.next).toEqual({ n: 50, p50: 5, p95: 5, max: 5 });
    expect(calls).toContain('select:folder:3');
    expect(calls).toContain('select:feed:7');
    expect(calls).toContain('search:zephyrine');
    // j/k is measured from the top of All Unread, like a reading session.
    const firstMove = calls.indexOf('move:1');
    expect(calls[firstMove - 1]).toBe('select:allUnread');
  });

  it('lists what is over budget, by median', async () => {
    const { target, now } = fakeApp({ switchMs: (_s, search) => (search ? 80 : 10), nextMs: 20 });

    const report = await runPerf(target, { now, flush: async () => {}, interactive: async () => 900 });

    expect(report.over).toEqual([
      `start: 900 ms > ${BUDGETS.startMs} ms`,
      `switch to Search: 80 ms > ${BUDGETS.switchMs} ms`,
      `next Article: 20 ms > ${BUDGETS.nextMs} ms`,
    ]);
  });

  it('counts a step only once its result is on screen', async () => {
    const { target, now, advance } = fakeApp({ switchMs: () => 1, nextMs: 1 });

    // Rendering costs 2 ms on top of the state change.
    const report = await runPerf(target, { now, flush: async () => void advance(2), interactive: async () => 1 });

    expect(report.next.p50).toBe(3);
    expect(report.switches['Feed']?.p50).toBe(3);
  });
});
