// Release helpers (see docs/releasing.md).
//
//   node scripts/release.mjs version 0.2.0   set the version in every file that carries it
//   node scripts/release.mjs check v0.2.0    fail unless every file matches the tag
//   node scripts/release.mjs notes v0.2.0    print release notes for the tag (Conventional Commits)

import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const VERSION = /^\d+\.\d+\.\d+$/;
const root = fileURLToPath(new URL('..', import.meta.url));

/** The app version lives in all of these; the release workflow checks they agree. */
const JSON_FILES = ['package.json', 'package-lock.json', 'src-tauri/tauri.conf.json'];
const CARGO_FILES = ['src-tauri/Cargo.toml', 'crates/feedrico-core/Cargo.toml'];
/** AppStream metadata for the .deb; its newest `<release>` is the app version too. */
const METAINFO = 'src-tauri/linux/de.andi1984.feedrico.metainfo.xml';
const OWN_CRATES =['feedrico', 'feedrico-core'];

function assertVersion(version) {
  if (!VERSION.test(version)) throw new Error(`${version} is not x.y.z`);
}

/** Sets `version` (and the lockfile's root package) without touching anything else. */
export function setJsonVersion(text, version) {
  assertVersion(version);
  const json = JSON.parse(text);
  json.version = version;
  if (json.packages?.['']) json.packages[''].version = version;
  return JSON.stringify(json, null, 2) + '\n';
}

/** Sets the version in the `[package]` table; dependency versions stay. */
export function setCargoPackageVersion(text, version) {
  assertVersion(version);
  return text.replace(/(\[package\][^[]*?\nversion = ")[^"]*(")/, `$1${version}$2`);
}

/** Sets the version of our own crates in Cargo.lock. */
export function setCargoLockVersion(text, names, version) {
  assertVersion(version);
  return text.replace(/name = "([^"]+)"\nversion = "[^"]*"/g, (entry, name) =>
    names.includes(name) ? `name = "${name}"\nversion = "${version}"` : entry,
  );
}

const RELEASE = /<release version="([^"]*)" date="([^"]*)"\/>/;

/** The newest release listed in the AppStream metainfo (the first one). */
export function latestMetainfoVersion(text) {
  return RELEASE.exec(text)?.[1];
}

/** Lists `version` as the newest AppStream release; a repeat of the latest only gets a new date. */
export function setMetainfoRelease(text, version, date) {
  assertVersion(version);
  const entry = `<release version="${version}" date="${date}"/>`;
  if (latestMetainfoVersion(text) === version) return text.replace(RELEASE, entry);
  return text.replace(/(<releases>\n)(\s*)/, `$1$2${entry}\n$2`);
}

/** Problems with releasing `tag` given the version found in each file; empty when fine. */
export function checkTag(tag, versions) {
  const version = tag.replace(/^v/, '');
  if (!tag.startsWith('v') || !VERSION.test(version)) return [`tag ${tag} is not vX.Y.Z`];
  return Object.entries(versions)
    .filter(([, found]) => found !== version)
    .map(([file, found]) => `${file} has ${found}, the tag says ${version}`);
}

const capitalize = (text) => text.charAt(0).toUpperCase() + text.slice(1);

/** `{ type, breaking, text }` for a Conventional Commit, `null` for anything else. */
export function parseCommit({ subject, body }) {
  const match = /^(\w+)(?:\(([^)]*)\))?(!)?: (.+)$/.exec(subject.trim());
  if (!match) return null;
  const [, type, scope = null, bang, text] = match;
  // The footer runs until a blank line, wrapped at 72 like the rest of the body.
  const footer = /^BREAKING[ -]CHANGE: ([\s\S]+?)(?:\n\s*\n|$(?![\s\S]))/m.exec(body ?? '');
  const footerText = footer?.[1].replace(/\s*\n\s*/g, ' ').trim();
  const breaking = footerText ? capitalize(footerText) : bang ? capitalize(text) : null;
  return { type, scope, breaking, text: capitalize(text) };
}

const SECTIONS = [
  ['New', 'feat'],
  ['Fixed', 'fix'],
  ['Faster', 'perf'],
];

/** Release notes from the commits since the last tag, filled into `template`. */
export function releaseNotes(commits, version, template) {
  // The website (scope `site`) ships separately, not with the app.
  const parsed = commits.map(parseCommit).filter((c) => c && c.scope !== 'site');
  const blocks = [];
  const breaking = parsed.filter((c) => c.breaking).map((c) => c.breaking);
  if (breaking.length) blocks.push(section('Breaking changes', breaking));
  for (const [title, type] of SECTIONS) {
    const items = parsed.filter((c) => c.type === type).map((c) => c.text);
    if (items.length) blocks.push(section(title, items));
  }
  const changes = blocks.length ? blocks.join('\n\n') : 'No user-facing changes.';
  return template
    .replace(/^<!--[\s\S]*?-->\s*/, '')
    .replaceAll('{{version}}', version)
    .replaceAll('{{changes}}', changes);
}

function section(title, items) {
  return `### ${title}\n\n${items.map((item) => `- ${item}`).join('\n')}`;
}

// --- command line -------------------------------------------------------------------------

const read = (file) => readFileSync(new URL(file, `file://${root}`), 'utf8');
const write = (file, text) => writeFileSync(new URL(file, `file://${root}`), text);
const git = (...args) => execFileSync('git', args, { cwd: root, encoding: 'utf8' });

function currentVersions() {
  const versions = {};
  for (const file of JSON_FILES) {
    const json = JSON.parse(read(file));
    versions[file] = json.version;
    if (json.packages?.['']) versions[`${file} (root package)`] = json.packages[''].version;
  }
  for (const file of CARGO_FILES) {
    versions[file] = /\[package\][^[]*?\nversion = "([^"]*)"/.exec(read(file))?.[1];
  }
  versions[`${METAINFO} (newest release)`] = latestMetainfoVersion(read(METAINFO));
  const lock = read('Cargo.lock');
  for (const name of OWN_CRATES) {
    versions[`Cargo.lock ${name}`] = new RegExp(`name = "${name}"\\nversion = "([^"]*)"`).exec(lock)?.[1];
  }
  return versions;
}

function commitsFor(tag) {
  let range = tag;
  try {
    const previous = execFileSync('git', ['describe', '--tags', '--abbrev=0', `${tag}^`], {
      cwd: root,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    });
    range = `${previous.trim()}..${tag}`;
  } catch {
    // First release: everything up to the tag.
  }
  return git('log', '--format=%s%x1f%b%x1e', range)
    .split('\x1e')
    .map((entry) => entry.trim())
    .filter(Boolean)
    .map((entry) => {
      const [subject, body = ''] = entry.split('\x1f');
      return { subject, body };
    });
}

function main([command, arg]) {
  if (command === 'version' && arg) {
    for (const file of JSON_FILES) write(file, setJsonVersion(read(file), arg));
    for (const file of CARGO_FILES) write(file, setCargoPackageVersion(read(file), arg));
    write('Cargo.lock', setCargoLockVersion(read('Cargo.lock'), OWN_CRATES, arg));
    const today = new Date().toISOString().slice(0, 10);
    write(METAINFO, setMetainfoRelease(read(METAINFO), arg, today));
    console.log(`Version set to ${arg}.`);
  } else if (command === 'check' && arg) {
    const errors = checkTag(arg, currentVersions());
    for (const error of errors) console.error(error);
    if (errors.length) process.exit(1);
    console.log(`All files carry ${arg}.`);
  } else if (command === 'notes' && arg) {
    const template = read('.github/release-notes.md');
    process.stdout.write(releaseNotes(commitsFor(arg), arg.replace(/^v/, ''), template));
  } else {
    console.error('usage: node scripts/release.mjs version <x.y.z> | check <vX.Y.Z> | notes <vX.Y.Z>');
    process.exit(2);
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) main(process.argv.slice(2));
