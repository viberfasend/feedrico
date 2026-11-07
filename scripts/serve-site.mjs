// Serves site-dist/ under /feedrico/, the path GitHub Pages will use, so relative URLs are
// tested the way they will be deployed. No dependencies.
//
//   npm run site:preview            http://localhost:4173/feedrico/
//   PORT=8080 npm run site:preview

import { createServer } from 'node:http';
import { readFile, stat } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';

const ROOT = 'site-dist';
const BASE = '/feedrico/';
const PORT = Number(process.env.PORT ?? 4173);

const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.ico': 'image/x-icon',
  '.woff2': 'font/woff2',
  '.webmanifest': 'application/manifest+json',
  '.txt': 'text/plain; charset=utf-8',
};

createServer(async (req, res) => {
  const url = new URL(req.url ?? '/', 'http://localhost');
  if (!url.pathname.startsWith(BASE)) {
    res.writeHead(302, { location: BASE + url.search });
    return res.end();
  }
  let path = normalize(decodeURIComponent(url.pathname.slice(BASE.length)));
  if (path.startsWith('..')) {
    res.writeHead(400);
    return res.end();
  }
  let file = join(ROOT, path);
  try {
    if ((await stat(file)).isDirectory()) file = join(file, 'index.html');
    const body = await readFile(file);
    res.writeHead(200, { 'content-type': TYPES[extname(file)] ?? 'application/octet-stream' });
    res.end(body);
  } catch {
    res.writeHead(404, { 'content-type': 'text/plain' });
    res.end('Not found');
  }
}).listen(PORT, () => console.log(`Feedrico site: http://localhost:${PORT}${BASE}`));
