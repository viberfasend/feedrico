# Releasing Feedrico

A release is a `vX.Y.Z` tag. Pushing it runs `.github/workflows/release.yml`, which builds every
package and attaches it to a **draft** GitHub Release. Nothing is public until you publish the
draft, and "Check for updates" in the app only sees published releases.

## Packages

| Platform | Package | Portable Mode |
|---|---|---|
| Linux | AppImage (built on Ubuntu 22.04 for an older glibc) | yes |
| Linux | `.deb` | no |
| Windows | NSIS installer (`-setup.exe`) | no |
| Windows | portable `.exe` (the plain executable) | yes |
| macOS 11+ | universal `.dmg` (Apple silicon and Intel) | no, see the spec |

Portable Mode: a `Feedrico Library` folder next to the AppImage or portable `.exe` is opened on
start; an empty one gets a new Library. See "Finding a Library on startup" in
[`spec/v1.md`](spec/v1.md).

The builds are not code-signed yet. The release notes tell users how to get past SmartScreen and
Gatekeeper.

## Steps

1. Pick the version from the commits since the last tag (Conventional Commits): a `feat` means
   a minor bump, only `fix`/`perf` a patch, a breaking change a major bump (minor while we are
   on 0.x).
2. Set it everywhere and commit on a branch, then merge it through a PR like any change:

   ```sh
   node scripts/release.mjs version 0.2.0
   git commit -am "chore(release): 0.2.0"
   ```

   It updates `package.json`, `package-lock.json`, `src-tauri/tauri.conf.json` (the version the
   update check compares), both `Cargo.toml` files and `Cargo.lock`.
3. Tag the merged commit on `main` and push the tag:

   ```sh
   git switch main && git pull
   git tag v0.2.0 && git push origin v0.2.0
   ```

4. The workflow fails early if any file disagrees with the tag. Otherwise it drafts the release
   with notes from `.github/release-notes.md`, filled by `node scripts/release.mjs notes v0.2.0`
   (breaking changes, then `feat`, `fix` and `perf` subjects; website commits are left out).
5. When all three builds are attached, check the draft:
   - five files: AppImage, `.deb`, `-setup.exe`, `-portable.exe`, `.dmg`
   - rewrite the generated notes where a subject doesn't make sense to a reader
   - smoke-test at least the package for your own system, and Portable Mode with an empty
     `Feedrico Library` folder next to the AppImage or portable `.exe`
6. Publish the draft.

If a build fails, fix it on `main`, delete the draft and the tag (`git push --delete origin
v0.2.0`), and tag again. Re-running a failed job uploads into the same draft (`--clobber`).

## Library format changes

A release that bumps the Library format version is breaking for older apps (`feat(library)!`).
Walk the version scenario matrix A–E in [`personas.md`](personas.md) before tagging, and keep
the breaking change text readable: it becomes the first section of the release notes.
