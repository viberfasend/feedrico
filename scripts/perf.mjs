// Runs every performance benchmark (spec: "Performance budgets") and prints one Markdown report:
//   1. Rust query benchmarks, which also generate the 300 Feed / 50k Article fixture Library
//   2. the UI timing harness: the real app in perf mode against that Library
//
//   npx tauri build --no-bundle && node scripts/perf.mjs [path/to/feedrico]
//
// Writes perf-results.json and appends the report to $GITHUB_STEP_SUMMARY when set. Budgets are
// reported, never gating: shared CI machines are too noisy (spec: "report, not gate").

import { spawn } from 'node:child_process';
import { appendFileSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const exe = process.platform === 'win32' ? '.exe' : '';
const app = process.argv[2] ?? join('target', 'release', `feedrico${exe}`);
const APP_TIMEOUT_MS = 180_000;

/** Runs a command, streams its output, resolves with stdout. */
function run(command, args, env, timeoutMs) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { env: { ...process.env, ...env }, stdio: ['ignore', 'pipe', 'inherit'] });
    let out = '';
    child.stdout.on('data', (chunk) => {
      out += chunk;
      process.stderr.write(chunk);
    });
    const timer = timeoutMs && setTimeout(() => child.kill(), timeoutMs);
    child.on('error', reject);
    child.on('close', (code) => {
      clearTimeout(timer);
      code === 0 ? resolve(out) : reject(new Error(`${command} exited with ${code}`));
    });
  });
}

const fixture = mkdtempSync(join(tmpdir(), 'feedrico-perf-'));
try {
  const cargo = await run(
    'cargo',
    ['test', '-p', 'feedrico-core', '--release', '--test', 'perf', '--', '--ignored', '--nocapture'],
    { FEEDRICO_PERF_OUT: fixture },
  );
  // Everything the benchmark printed, minus the test harness's own lines.
  const queries = cargo
    .split('\n')
    .filter((line) => line.trim() && !/^(running \d+ test|test |test result:)/.test(line.trim()))
    .join('\n');

  let ui;
  try {
    const out = await run(app, [], { FEEDRICO_PERF_LIBRARY: join(fixture, 'Feedrico Library') }, APP_TIMEOUT_MS);
    const line = out.split('\n').find((l) => l.startsWith('FEEDRICO_PERF '));
    ui = line ? JSON.parse(line.slice('FEEDRICO_PERF '.length)) : { error: 'the app printed no report' };
  } catch (e) {
    ui = { error: String(e) };
  }

  writeFileSync('perf-results.json', JSON.stringify({ queries, ui }, null, 2));
  const report = markdown(queries, ui);
  process.stdout.write(report);
  if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, report);
} finally {
  rmSync(fixture, { recursive: true, force: true });
}

function markdown(queries, ui) {
  const lines = ['## Performance', '', '### Queries (Rust core, 300 Feeds / 50k Articles)', '', '```', queries, '```', ''];
  lines.push('### UI (real app, same Library)', '');
  if (ui.error) {
    lines.push(`UI harness failed: ${ui.error}`, '');
    return lines.join('\n');
  }
  lines.push(`Cold start to interactive: **${ui.interactiveMs} ms** (budget 500 ms)`, '');
  lines.push('| Step | n | median | p95 | max | budget |', '|---|---:|---:|---:|---:|---:|');
  for (const [name, s] of Object.entries(ui.switches)) {
    lines.push(`| switch to ${name} | ${s.n} | ${s.p50} ms | ${s.p95} ms | ${s.max} ms | 50 ms |`);
  }
  const n = ui.next;
  lines.push(`| next Article (j) | ${n.n} | ${n.p50} ms | ${n.p95} ms | ${n.max} ms | 16.7 ms |`, '');
  lines.push(ui.over.length ? `Over budget (not gating): ${ui.over.join('; ')}` : 'All UI budgets met.', '');
  return lines.join('\n');
}
