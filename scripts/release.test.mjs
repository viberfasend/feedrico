import { describe, expect, it } from 'vitest';
import {
  checkTag,
  parseCommit,
  releaseNotes,
  setCargoLockVersion,
  setCargoPackageVersion,
  setJsonVersion,
} from './release.mjs';

const cargoToml = `[package]
name = "feedrico"
version = "0.1.0"
edition = "2024"

[dependencies]
tauri = { version = "2", features = [] }
`;

const cargoLock = `[[package]]
name = "feedrico"
version = "0.1.0"
dependencies = []

[[package]]
name = "feedrico-core"
version = "0.1.0"

[[package]]
name = "feed-rs"
version = "0.1.0"
`;

describe('setting the version', () => {
  it('changes only the version in package.json, tauri.conf.json and the lockfile root', () => {
    const lock = JSON.stringify({ name: 'feedrico', version: '0.1.0', packages: { '': { version: '0.1.0' }, 'node_modules/x': { version: '0.1.0' } } }, null, 2) + '\n';
    const out = JSON.parse(setJsonVersion(lock, '0.2.0'));
    expect(out.version).toBe('0.2.0');
    expect(out.packages[''].version).toBe('0.2.0');
    expect(out.packages['node_modules/x'].version).toBe('0.1.0');
    expect(setJsonVersion('{\n  "version": "0.1.0"\n}\n', '0.2.0')).toBe('{\n  "version": "0.2.0"\n}\n');
  });

  it('changes the [package] version of a Cargo.toml, not dependency versions', () => {
    const out = setCargoPackageVersion(cargoToml, '0.2.0');
    expect(out).toContain('version = "0.2.0"\nedition');
    expect(out).toContain('tauri = { version = "2"');
  });

  it('changes our own crates in Cargo.lock only', () => {
    const out = setCargoLockVersion(cargoLock, ['feedrico', 'feedrico-core'], '0.2.0');
    expect(out.match(/version = "0\.2\.0"/g)).toHaveLength(2);
    expect(out).toContain('name = "feed-rs"\nversion = "0.1.0"');
  });

  it('rejects anything that is not x.y.z', () => {
    expect(() => setJsonVersion('{"version":"0.1.0"}', 'v0.2.0')).toThrow();
    expect(() => setCargoPackageVersion(cargoToml, '0.2')).toThrow();
  });
});

describe('checking a release tag', () => {
  it('passes when every file carries the tag version', () => {
    expect(checkTag('v0.2.0', { 'package.json': '0.2.0', 'Cargo.lock feedrico': '0.2.0' })).toEqual([]);
  });

  it('names each file that disagrees', () => {
    const errors = checkTag('v0.2.0', { 'package.json': '0.2.0', 'src-tauri/tauri.conf.json': '0.1.0' });
    expect(errors).toEqual(['src-tauri/tauri.conf.json has 0.1.0, the tag says 0.2.0']);
  });

  it('only accepts tags like v1.2.3', () => {
    expect(checkTag('0.2.0', {})).toEqual(['tag 0.2.0 is not vX.Y.Z']);
  });
});

describe('release notes', () => {
  it('parses Conventional Commit subjects and breaking changes', () => {
    expect(parseCommit({ subject: 'feat(ui): add the Settings dialog', body: '' })).toEqual({
      type: 'feat', scope: 'ui', breaking: null, text: 'Add the Settings dialog',
    });
    expect(parseCommit({ subject: 'feat(library)!: add an unread index', body: 'Why.\n\nBREAKING CHANGE: older apps can\'t open it' }).breaking)
      .toBe("Older apps can't open it");
    expect(parseCommit({ subject: 'fix!: drop old setting', body: '' }).breaking).toBe('Drop old setting');
    expect(parseCommit({ subject: 'Merge branch x', body: '' })).toBeNull();
  });

  it('reads a breaking change footer that wraps over several lines', () => {
    const body = 'Why.\n\nBREAKING CHANGE: Library format version 3. Older Feedrico versions\nrefuse upgraded Libraries.\n\nCo-Authored-By: Someone <a@b.c>';
    expect(parseCommit({ subject: 'feat(library)!: add an index', body }).breaking).toBe(
      'Library format version 3. Older Feedrico versions refuse upgraded Libraries.',
    );
  });

  it('leaves out changes to the website', () => {
    const notes = releaseNotes([{ subject: 'feat(site): add the Feedrico website', body: '' }], '0.2.0', '{{changes}}');
    expect(notes).toBe('No user-facing changes.');
  });

  it('groups user-facing changes and leaves out the rest', () => {
    const notes = releaseNotes(
      [
        { subject: 'feat(ui): add the Settings dialog', body: '' },
        { subject: 'fix(ui): link to the original when an image isn\'t cached', body: 'Closes #18' },
        { subject: 'perf(core): count unread in 1 ms', body: '' },
        { subject: 'feat(library)!: add an unread index', body: 'BREAKING CHANGE: older apps can\'t open upgraded Libraries' },
        { subject: 'docs: record the audit', body: '' },
        { subject: 'chore(release): 0.2.0', body: '' },
        { subject: 'test(ui): run axe', body: '' },
      ],
      '0.2.0',
      '# Feedrico {{version}}\n\n{{changes}}\n',
    );
    expect(notes).toBe(`# Feedrico 0.2.0

### Breaking changes

- Older apps can't open upgraded Libraries

### New

- Add the Settings dialog
- Add an unread index

### Fixed

- Link to the original when an image isn't cached

### Faster

- Count unread in 1 ms
`);
  });

  it('drops the leading comment of the template', () => {
    expect(releaseNotes([], '0.2.0', '<!-- about {{version}} -->\n\n# {{version}}')).toBe('# 0.2.0');
  });

  it('says so when nothing user-facing changed', () => {
    expect(releaseNotes([{ subject: 'ci: cache npm', body: '' }], '0.2.1', '{{changes}}')).toBe('No user-facing changes.');
  });
});
