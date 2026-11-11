# Running Feedrico locally

How to get the desktop app running from a fresh clone, where it keeps its data, and how to
run the checks. For debugging a running app, see the `debugging-feedrico` skill in
`.claude/skills/`.

## Prerequisites

| Tool | Version | Check |
|---|---|---|
| Rust | stable, **1.85 or newer** (the workspace uses edition 2024) | `rustc --version` |
| Node.js | 24 (what CI uses) | `node --version` |
| Platform libraries | see below | |

If `rustc --version` reports something older than 1.85 although you installed a current
toolchain, an old `rustc` comes first on your `PATH`. Put the right one first, for example
`PATH=/snap/bin:$PATH` for a snap install or `PATH=$HOME/.cargo/bin:$PATH` for rustup.

### Linux (Debian, Ubuntu)

Tauri renders the UI with WebKitGTK:

```sh
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libxdo-dev
```

Other distributions: see the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

### macOS

Install the Xcode Command Line Tools: `xcode-select --install`.

### Windows

Install the Microsoft C++ Build Tools ("Desktop development with C++"). WebView2 ships with
Windows 10 (1803+) and 11.

## First run

```sh
git clone <repo-url> feedrico
cd feedrico
npm install          # also points git at .githooks (commit-msg check)
npm run tauri dev
```

`npm run tauri dev` does three things:

1. builds the design tokens (`npm run tokens`),
2. starts the Vite dev server for the Svelte UI on `http://localhost:1420`,
3. compiles `src-tauri` and the Rust core, then opens the Feedrico window.

The first build compiles all Rust dependencies and takes a few minutes. Later starts are fast.

The app is ready when the output shows ``Running `target/debug/feedrico` ``.

### While it runs

- **Svelte changes** (`src/`) hot-reload in the window.
- **Rust changes** (`crates/`, `src-tauri/`) rebuild and restart the app. If the rebuild
  fails, the output shows `error[…]` and the **old** binary keeps running.
- `console.log` / `console.warn` from the UI and `eprintln!` from Rust both appear in the
  terminal running `tauri dev`.
- Stop with `Ctrl+C`.

The UI also opens in a plain browser at `http://localhost:1420`, but without the Tauri shell
nothing loads: every call into the Rust core fails. Use the desktop window.

## Your Library

On first start Feedrico shows the welcome screen:

- **Create new Library** creates a `Feedrico Library` folder in the app data directory (the
  path is shown under the button).
- **Create in another place…** and **Open existing Library…** let you pick a folder.

Then use **Import OPML…** in the sidebar to add Feeds.

Where things live:

| What | Linux | macOS | Windows |
|---|---|---|---|
| Default Library | `~/.local/share/de.andi1984.feedrico/Feedrico Library` | `~/Library/Application Support/de.andi1984.feedrico/Feedrico Library` | `%APPDATA%\de.andi1984.feedrico\Feedrico Library` |
| Recent Libraries list | `~/.config/de.andi1984.feedrico/recent-libraries.json` | `~/Library/Application Support/de.andi1984.feedrico/recent-libraries.json` | `%APPDATA%\de.andi1984.feedrico\recent-libraries.json` |

The dev build uses the same locations as an installed Feedrico. To keep test data apart from
your real reading, create a separate Library for development with **Create in another
place…**.

**Portable Mode:** if a `Feedrico Library` folder sits next to the executable, Feedrico opens
it directly. For a dev build that is `target/debug/Feedrico Library`. Not on macOS.

Never delete a Library's `.lock` file by hand. A lock left by a crashed Feedrico on the same
machine is taken over automatically.

## Checks

Run these before committing (CI runs the same):

```sh
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run check     # tokens + svelte-check
npm test          # tokens + Vitest
```

Live network tests are skipped by default:

```sh
cargo test -p feedrico-core -- --ignored
```

## Release build

```sh
npm run tauri build
```

The bundles (e.g. `.deb`, `.AppImage`, `.dmg`, `.msi`, depending on the platform) land in
`target/release/bundle/`.

## Troubleshooting

| Symptom | Fix |
|---|---|
| ``failed to run 'cargo metadata'`` … `` `resolver` setting `3` is not valid`` or ``feature `edition2024` is required`` | Cargo is older than 1.85. Run `which -a cargo` and `cargo --version`; update the toolchain (`rustup update stable`) or put the newer one first on your `PATH` (see Prerequisites). |
| `Port 1420 is already in use` | Another `tauri dev` or Vite is still running. Stop it. |
| `pkg-config` / `webkit2gtk-4.1` not found | Install the Linux packages above. |
| Window opens blank | Check the terminal for Vite or `error[` output; the Rust or UI build failed. |
| Changes don't show up | A failed Rust rebuild leaves the old binary running. Check the output for `error[`. |
| "This Library is still marked as open" | Another machine (or a Feedrico that didn't close cleanly) holds the lock. If it isn't open anywhere else, choose **Open anyway**. |
| "This Library needs a newer Feedrico" | The Library was written by a newer build. Check out a newer commit or open another Library. |
