// Turns the sanitized Article HTML into something calm to read: every image gets one treatment
// from reader-media.ts, emoji images become text, comic title text becomes a caption, images
// that aren't cached become a card that links to the original, pictures open in the viewer, and
// code blocks (pre) get local syntax highlighting plus a copy button (issue #77).
//
// Runs on the DOM after {@html}: the content is already sanitized in the core (no scripts, no
// style attributes, only language-*/lang-* classes on pre/code), so this only adds classes,
// captions, token spans and the viewer hook.
import { hostOf } from '../../format';
import { intendedSize, isEmojiAlt, presentImage } from '../../reader-media';

export interface ViewerImage {
  src: string;
  alt: string;
  caption: string | null;
  /** Where the image links to, if it sits inside a link. */
  link: string | null;
  /** The original URL, kept by the core in data-remote. */
  original: string | null;
}

export interface ContentOptions {
  /** Opens the image viewer at `index` of `images`. */
  onView: (images: ViewerImage[], index: number) => void;
}

const SAFE_URL = /^https?:\/\//;
const CLASSES = ['img-pixel', 'img-inline', 'img-block', 'img-wide', 'img-cap'];

function attrNumber(img: HTMLImageElement, name: 'width' | 'height'): number | null {
  const value = Number.parseInt(img.getAttribute(name) ?? '', 10);
  return Number.isFinite(value) && value > 0 ? value : null;
}

/** Applies the presentation of `img`: before load from its attributes, after load for real. */
export function classify(img: HTMLImageElement, column: number) {
  const loaded = img.complete && img.naturalWidth > 0;
  const natural = loaded ? { width: img.naturalWidth, height: img.naturalHeight } : null;
  const size = intendedSize(attrNumber(img, 'width'), attrNumber(img, 'height'), natural);
  const look = presentImage(size, column);
  img.classList.remove(...CLASSES);
  img.classList.add(`img-${look.kind}`);
  if (look.wide) img.classList.add('img-wide');
  if (look.capHeight) img.classList.add('img-cap');
  // A wide image uses up to its own width, never more: no upscaling past the original.
  if (look.wide && size) img.style.setProperty('--img-width', `${Math.round(size.width)}px`);
  else img.style.removeProperty('--img-width');
  if (look.kind === 'pixel') img.setAttribute('aria-hidden', 'true');
  if (look.kind === 'block' && !img.closest('a')) makeViewable(img);
  else unmakeViewable(img);
}

function makeViewable(img: HTMLImageElement) {
  img.tabIndex = 0;
  img.setAttribute('role', 'button');
  img.setAttribute('aria-label', img.alt ? `View image: ${img.alt}` : 'View image');
}

function unmakeViewable(img: HTMLImageElement) {
  img.removeAttribute('tabindex');
  if (img.getAttribute('role') === 'button') img.removeAttribute('role');
  if (img.getAttribute('aria-label')?.startsWith('View image')) img.removeAttribute('aria-label');
}

/** The caption a reader sees for `img`: its figure's caption, else a title that adds something. */
function captionOf(img: HTMLImageElement): string | null {
  const figcaption = img.closest('figure')?.querySelector('figcaption');
  if (figcaption?.textContent?.trim()) return figcaption.textContent.trim();
  const title = img.title.trim();
  return title && title !== img.alt.trim() ? title : null;
}

/** Comic title text (xkcd and friends) is part of the joke: show it, don't hide it in a tooltip. */
function addTitleCaption(img: HTMLImageElement) {
  const title = img.title.trim();
  if (!title || title === img.alt.trim()) return;
  if (img.closest('figure')?.querySelector('figcaption')) return;
  if (img.nextElementSibling?.classList.contains('img-caption')) return;
  const caption = document.createElement('span');
  caption.className = 'img-caption';
  caption.textContent = title;
  img.after(caption);
}

/** A card in place of an image that isn't cached (over 5 MB, evicted, not fetched yet). */
export function remoteImageCard(img: HTMLImageElement, remote: string): HTMLElement {
  const card = document.createElement('span');
  card.className = 'remote-image';
  const text = document.createElement('span');
  text.className = 'remote-image-text';
  const title = document.createElement('strong');
  title.textContent = 'Image not saved for offline reading';
  text.append(title);
  if (img.alt.trim()) {
    const alt = document.createElement('span');
    alt.textContent = img.alt.trim();
    text.append(alt);
  }
  card.append(text);
  // Inside a link already: that link stays the target, links can't nest.
  if (!img.closest('a')) {
    const link = document.createElement('a');
    link.className = 'remote-image-action';
    link.href = remote;
    link.textContent = `Open on ${hostOf(remote)}`;
    card.append(link);
  }
  return card;
}

/** Svelte action for the content element. Re-run it (via {#key}) for every Article. */
export function enhanceContent(node: HTMLElement, options: ContentOptions) {
  let current = options;
  const column = () => node.clientWidth || 600;

  const images = () =>
    [...node.querySelectorAll<HTMLImageElement>('img.img-block')].filter((img) => !img.classList.contains('missing'));

  for (const img of node.querySelectorAll<HTMLImageElement>('img')) {
    if (isEmojiAlt(img.alt)) {
      const emoji = document.createElement('span');
      emoji.className = 'emoji';
      emoji.textContent = img.alt.trim();
      img.replaceWith(emoji);
      continue;
    }
    classify(img, column());
    addTitleCaption(img);
  }
  prepareCodeBlocks(node);

  const onLoad = (event: Event) => {
    if (event.target instanceof HTMLImageElement) classify(event.target, column());
  };

  // Images load only from the local cache. One that isn't cached "stays a remote link" (spec):
  // the core keeps its original URL in the inert data-remote attribute. Articles stored before
  // that just hide the broken image.
  const onError = (event: Event) => {
    const img = event.target;
    if (!(img instanceof HTMLImageElement)) return;
    const remote = img.dataset.remote;
    if (img.nextElementSibling?.classList.contains('img-caption')) img.nextElementSibling.remove();
    if (remote && SAFE_URL.test(remote)) img.replaceWith(remoteImageCard(img, remote));
    else img.classList.add('missing');
  };

  const open = (img: HTMLImageElement) => {
    const list = images();
    const index = list.indexOf(img);
    if (index < 0) return;
    current.onView(
      list.map((i) => ({
        src: i.currentSrc || i.src,
        alt: i.alt,
        caption: captionOf(i),
        link: i.closest('a')?.getAttribute('href') ?? null,
        original: i.dataset.remote && SAFE_URL.test(i.dataset.remote) ? i.dataset.remote : null,
      })),
      index,
    );
  };

  const viewable = (target: EventTarget | null) =>
    target instanceof HTMLImageElement && target.getAttribute('role') === 'button' ? target : null;

  const onClick = (event: MouseEvent) => {
    const copy = (event.target as HTMLElement).closest('button.code-copy');
    if (copy) {
      event.preventDefault();
      void copyCodeBlock(copy as HTMLButtonElement);
      return;
    }
    const img = viewable(event.target);
    if (img) open(img);
  };
  const onKeydown = (event: KeyboardEvent) => {
    const img = viewable(event.target);
    if (!img || (event.key !== 'Enter' && event.key !== ' ')) return;
    event.preventDefault();
    event.stopPropagation();
    open(img);
  };

  node.addEventListener('load', onLoad, true);
  node.addEventListener('error', onError, true);
  node.addEventListener('click', onClick);
  node.addEventListener('keydown', onKeydown);
  return {
    update: (next: ContentOptions) => (current = next),
    destroy: () => {
      node.removeEventListener('load', onLoad, true);
      node.removeEventListener('error', onError, true);
      node.removeEventListener('click', onClick);
      node.removeEventListener('keydown', onKeydown);
    },
  };
}

// --- Code blocks (issue #77) ---------------------------------------------------------------
// The sanitizer lets only language-*/lang-* classes through on pre/code. A small bundled
// highlighter (compiled into the UI: no CDN, no new network surface, CSP untouched) then colors
// the block and a copy button is added. It runs on the sanitized HTML only, after the Rust
// allow-list.

type TokenType = 'keyword' | 'string' | 'comment' | 'number' | 'function';

interface LangSpec {
  line?: string[];
  block?: [string, string][];
  triple?: string[];
  quotes?: string[];
  keywords?: string[];
  markup?: boolean;
  props?: boolean;
}

const CLike: LangSpec = { line: ['//'], block: [['/*', '*/']], quotes: ['"', "'", '`'] };

const LANGS: Record<string, LangSpec> = {
  rust: { ...CLike, keywords: 'as break const continue crate else enum fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait type unsafe use where while async await dyn true false'.split(' ') },
  javascript: { ...CLike, keywords: 'as async await break case catch class const continue debugger default delete do else export extends finally for from function if import in instanceof let new of return super switch this throw try typeof var void while with yield true false null undefined'.split(' ') },
  typescript: { ...CLike, keywords: 'as async await break case catch class const continue debugger default delete do else enum export extends finally for from function if implements import in instanceof interface let namespace new of private protected public return static super switch this throw try type typeof var void while with yield true false null undefined'.split(' ') },
  go: { ...CLike, keywords: 'break case chan const continue default defer else fallthrough for func go goto if import interface map package range return select struct switch type var true false nil'.split(' ') },
  c: { ...CLike, keywords: 'auto break case char const continue default do double else enum extern float for goto if int long register return short signed sizeof static struct switch typedef union unsigned void volatile while true false NULL'.split(' ') },
  cpp: { ...CLike, keywords: 'alignas auto break case catch class const constexpr continue default delete do double else enum explicit export extern false float for friend goto if inline int long mutable namespace new noexcept nullptr operator private protected public register return short signed sizeof static struct switch template this throw true try typedef typeid typename union unsigned using virtual void volatile while'.split(' ') },
  csharp: { ...CLike, keywords: 'abstract as base bool break case catch class const continue default delegate do double else enum event explicit extern false finally fixed float for foreach goto if implicit in int interface internal is lock long namespace new null object operator out override params private protected public readonly ref return sealed sizeof stackalloc static string struct switch this throw true try typeof uint ulong unchecked unsafe ushort using virtual void volatile while async await var'.split(' ') },
  java: { ...CLike, keywords: 'abstract assert boolean break byte case catch char class const continue default do double else enum extends final finally float for if implements import instanceof int interface long native new package private protected public return short static strictfp super switch synchronized this throw throws transient try void volatile while true false null'.split(' ') },
  kotlin: { ...CLike, keywords: 'as break class continue do else false for fun if in interface is null object package return super this throw true try typealias typeof val var when while'.split(' ') },
  swift: { ...CLike, keywords: 'associatedtype class deinit enum extension fileprivate func import init inout internal let open operator private protocol public rethrows static struct subscript typealias var break case continue default defer do else fallthrough for guard if in repeat return throw switch where while as Any catch false is nil super self Self throws true try'.split(' ') },
  python: { line: ['#'], triple: ['"""', "'''"], quotes: ['"', "'"], keywords: 'and as assert async await break class continue def del elif else except finally for from global if import in is lambda nonlocal not or pass raise return try while with yield True False None'.split(' ') },
  bash: { line: ['#'], quotes: ['"', "'"], keywords: 'if then else elif fi for while until do done case esac function in select time local export readonly return exit source set unset shift trap'.split(' ') },
  json: { quotes: ['"'], keywords: ['true', 'false', 'null'] },
  yaml: { line: ['#'], quotes: ['"', "'"], keywords: ['true', 'false', 'null', 'yes', 'no', 'on', 'off'] },
  toml: { line: ['#'], quotes: ['"', "'"], keywords: ['true', 'false'] },
  ini: { line: ['#', ';'], quotes: ['"', "'"] },
  sql: { line: ['--'], block: [['/*', '*/']], quotes: ['"', "'"], keywords: 'select from where insert into values update set delete create table alter drop join inner left right outer full on group by order having limit offset as and or not null case when then else end distinct union all exists in like between asc desc primary key references'.split(' ') },
  css: { block: [['/*', '*/']], quotes: ['"', "'"], props: true },
  markup: { block: [['<!--', '-->']], quotes: ['"', "'"], markup: true },
  diff: {},
  markdown: {},
};

const ALIASES: Record<string, string> = {
  js: 'javascript', jsx: 'javascript', mjs: 'javascript', cjs: 'javascript',
  ts: 'typescript', tsx: 'typescript', mts: 'typescript', cts: 'typescript',
  py: 'python',
  sh: 'bash', shell: 'bash', zsh: 'bash', console: 'bash',
  yml: 'yaml', md: 'markdown',
  html: 'markup', xml: 'markup', svg: 'markup', vue: 'markup', svelte: 'markup',
  golang: 'go', rs: 'rust', kt: 'kotlin',
  'c++': 'cpp', cxx: 'cpp', hpp: 'cpp',
  'c#': 'csharp', cs: 'csharp',
};

function specFor(lang: string): LangSpec | null {
  const key = ALIASES[lang] ?? lang;
  if (key === 'text' || key === 'plain' || key === 'plaintext') return null;
  return LANGS[key] ?? null;
}

const WORD = /[A-Za-z_][A-Za-z0-9_$]*/y;
const NUM = /0[xX][0-9a-fA-F_]+|\d[\d_]*(?:\.[\d_]+)?(?:[eE][+-]?\d+)?/y;

function scanString(code: string, start: number, quote: string): number {
  let j = start + quote.length;
  while (j < code.length) {
    if (code[j] === '\\') {
      j += 2;
      continue;
    }
    if (code.startsWith(quote, j)) return j + quote.length;
    j++;
  }
  return code.length;
}

/** Splits code into [token type or null, text] pairs. Unknown languages stay plain. */
function tokenize(code: string, lang: string): [TokenType | null, string][] {
  const spec = specFor(lang);
  const out: [TokenType | null, string][] = [];
  let plain = '';
  const flush = () => {
    if (plain) {
      out.push([null, plain]);
      plain = '';
    }
  };
  const push = (type: TokenType | null, text: string) => {
    flush();
    out.push([type, text]);
  };
  let i = 0;
  let afterTagOpen = false;
  while (i < code.length) {
    if (!spec) {
      plain += code[i++];
      continue;
    }
    if (lang === 'diff' || lang === 'markdown') {
      const nl = code.indexOf('\n', i);
      const end = nl === -1 ? code.length : nl;
      const line = code.slice(i, end);
      if (lang === 'diff') {
        if (line.startsWith('+++') || line.startsWith('---')) push('comment', line);
        else if (line.startsWith('@')) push('number', line);
        else if (line.startsWith('+')) push('string', line);
        else if (line.startsWith('-')) push('function', line);
        else plain += line;
      } else if (/^#{1,6}\s/.test(line) || line.startsWith('```')) push('keyword', line);
      else plain += line;
      i = end;
      continue;
    }
    let matched = false;
    for (const start of spec.line ?? []) {
      if (code.startsWith(start, i)) {
        const nl = code.indexOf('\n', i);
        const end = nl === -1 ? code.length : nl;
        push('comment', code.slice(i, end));
        i = end;
        matched = true;
        break;
      }
    }
    if (matched) continue;
    for (const [open, close] of spec.block ?? []) {
      if (code.startsWith(open, i)) {
        const end = code.indexOf(close, i + open.length);
        const stop = end === -1 ? code.length : end + close.length;
        push('comment', code.slice(i, stop));
        i = stop;
        matched = true;
        break;
      }
    }
    if (matched) continue;
    for (const triple of spec.triple ?? []) {
      if (code.startsWith(triple, i)) {
        const stop = scanString(code, i, triple);
        push('string', code.slice(i, stop));
        i = stop;
        matched = true;
        break;
      }
    }
    if (matched) continue;
    const quote = (spec.quotes ?? []).find((q) => code.startsWith(q, i));
    if (quote) {
      const stop = scanString(code, i, quote);
      push('string', code.slice(i, stop));
      i = stop;
      continue;
    }
    WORD.lastIndex = i;
    const word = WORD.exec(code);
    if (word) {
      const text = word[0];
      const after = code.slice(i + text.length);
      let type: TokenType | null = null;
      if (afterTagOpen) type = 'keyword';
      else if (spec.keywords?.includes(text)) type = 'keyword';
      else if (/^\s*\(/.test(after)) type = 'function';
      else if (spec.props && /^\s*:/.test(after)) type = 'function';
      else if (spec.markup && /^\s*=/.test(after)) type = 'function';
      afterTagOpen = false;
      push(type, text);
      i += text.length;
      continue;
    }
    NUM.lastIndex = i;
    const num = NUM.exec(code);
    if (num) {
      push('number', num[0]);
      i = NUM.lastIndex;
      continue;
    }
    if (spec.markup && code[i] === '<') afterTagOpen = true;
    plain += code[i++];
  }
  flush();
  return out;
}

/** The GFM language class the sanitizer kept on pre/code, e.g. "language-rust" → "rust". */
function languageOf(el: Element): string | null {
  for (const cls of el.classList) {
    const m = /^(?:language|lang)-(.+)$/.exec(cls);
    if (m?.[1]) return m[1].toLowerCase();
  }
  return null;
}

function highlight(codeEl: HTMLElement, lang: string) {
  const tokens = tokenize(codeEl.textContent ?? '', lang);
  const frag = document.createDocumentFragment();
  for (const [type, text] of tokens) {
    if (!type) {
      frag.append(text);
    } else {
      const span = document.createElement('span');
      span.className = `tok-${type}`;
      span.textContent = text;
      frag.append(span);
    }
  }
  codeEl.replaceChildren(frag);
}

function prepareCodeBlocks(root: HTMLElement) {
  for (const pre of root.querySelectorAll('pre')) {
    let code = Array.from(pre.children).find((c) => c.tagName === 'CODE') as HTMLElement | undefined;
    if (!code) {
      code = document.createElement('code');
      code.append(...pre.childNodes);
      pre.append(code);
    }
    const lang = languageOf(pre) ?? languageOf(code) ?? '';
    if (lang) highlight(code, lang);
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'code-copy';
    button.setAttribute('aria-label', lang ? `Copy ${lang} code block` : 'Copy code block');
    button.textContent = 'Copy';
    pre.append(button);
  }
}

const copyTimers = new WeakMap<HTMLButtonElement, ReturnType<typeof setTimeout>>();

async function copyCodeBlock(button: HTMLButtonElement) {
  const pre = button.closest('pre');
  const text = pre?.querySelector('code')?.textContent ?? '';
  if (!(await writeClipboard(text))) return;
  button.classList.add('copied');
  button.textContent = 'Copied';
  clearTimeout(copyTimers.get(button));
  copyTimers.set(
    button,
    setTimeout(() => {
      button.classList.remove('copied');
      button.textContent = 'Copy';
    }, 1600),
  );
}

async function writeClipboard(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    // Fall through to the legacy path (some webviews deny the async clipboard API).
  }
  const area = document.createElement('textarea');
  area.value = text;
  area.setAttribute('readonly', '');
  area.style.position = 'fixed';
  area.style.opacity = '0';
  document.body.append(area);
  area.select();
  let ok = false;
  try {
    ok = document.execCommand('copy');
  } catch {
    ok = false;
  }
  area.remove();
  return ok;
}
