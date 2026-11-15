// Takes the website's screenshots of the real UI, filled with the demo data in demo-ipc.js.
//
//   npm run dev                         (in another terminal; serves http://localhost:1420)
//   npm run site:shots                  (writes site/img/*.webp)
//
// Drives a headless Chromium over the DevTools protocol; no extra dependency. Set CHROMIUM to
// the browser binary if `chromium` is not on the PATH. Chromium is not WebKitGTK, so look at the
// pictures before committing them.
import { spawn } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const OUT = join(HERE, '../../site/img');
const APP = process.env.APP_URL ?? 'http://localhost:1420/';
const PORT = 9333;
const WIDTH = 1280;
const HEIGHT = 800;

// name, Layout, theme, and whether to open the first Article.
const SHOTS = [
  ['list-light', 'list', 'light', true],
  ['cards-light', 'cards', 'light', false],
  ['magazine-light', 'magazine', 'light', false],
  ['gallery-dark', 'gallery', 'dark', false],
  ['list-dark', 'list', 'dark', true],
];
const only = process.argv.slice(2);

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const demo = await readFile(join(HERE, 'demo-ipc.js'), 'utf8');

const browser = spawn(process.env.CHROMIUM ?? 'chromium', [
  '--headless=new', `--remote-debugging-port=${PORT}`, '--hide-scrollbars', '--no-first-run', '--lang=en-GB',
  `--window-size=${WIDTH},${HEIGHT}`, 'about:blank',
], { stdio: 'ignore' });

try {
  let target;
  for (let i = 0; i < 50 && !target; i++) {
    await sleep(200);
    try {
      const pages = await (await fetch(`http://127.0.0.1:${PORT}/json`)).json();
      target = pages.find((p) => p.type === 'page');
    } catch { /* not up yet */ }
  }
  if (!target) throw new Error('Chromium did not start');

  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
  let seq = 0;
  const waiting = new Map();
  ws.onmessage = (event) => {
    const message = JSON.parse(event.data);
    const done = waiting.get(message.id);
    if (!done) return;
    waiting.delete(message.id);
    if (message.error) done.reject(new Error(message.error.message));
    else done.resolve(message.result);
  };
  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      waiting.set(++seq, { resolve, reject });
      ws.send(JSON.stringify({ id: seq, method, params }));
    });
  const run = async (expression) => {
    const { result, exceptionDetails } = await send('Runtime.evaluate', {
      expression, awaitPromise: true, returnByValue: true,
    });
    if (exceptionDetails) throw new Error(exceptionDetails.exception?.description ?? exceptionDetails.text);
    return result.value;
  };

  await send('Page.enable');
  await send('Emulation.setDeviceMetricsOverride', { width: WIDTH, height: HEIGHT, deviceScaleFactor: 2, mobile: false });
  await mkdir(OUT, { recursive: true });

  for (const [name, layout, theme, open] of SHOTS) {
    if (only.length && !only.includes(name)) continue;
    await send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: theme }] });
    await send('Page.navigate', { url: APP });
    await sleep(2500);
    await run(`window.__demo = ${JSON.stringify({ layout, theme })};
      import(URL.createObjectURL(new Blob([${JSON.stringify(demo)}], { type: 'text/javascript' }))).then(() => 'ok')`);
    await sleep(1200);
    if (open) {
      await run(`document.querySelector(${JSON.stringify(process.env.ROW ?? '[data-article-id], [role="option"]')})?.click()`);
      await sleep(900);
    }
    const { data } = await send('Page.captureScreenshot', { format: 'webp', quality: 82 });
    await writeFile(join(OUT, `${name}.webp`), Buffer.from(data, 'base64'));
    console.log(`site/img/${name}.webp`);
  }
  ws.close();
} finally {
  browser.kill();
}
