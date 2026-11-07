# Feedrico

A fast, local-only desktop RSS reader. Import your OPML file and read. No account, no backend, no sync service, no telemetry.

All your Feeds, Articles, Read and Starred state, cached content and settings live in one **Library** folder. Copy it to a USB stick, open it on a friend's machine, and Feedrico shows exactly the same data there.

> Status: early design. Nothing to install yet.

## Docs

- [v1 specification](docs/spec/v1.md)
- [Glossary](GLOSSARY.md): the words we use and the ones we avoid
- [Architecture decisions](docs/adr/)

## Stack

Tauri 2 with a Rust core, Svelte 5 + TypeScript UI, SQLite, and W3C design tokens built with Style Dictionary.

## Development

Prerequisites: Rust (stable), Node 24, and on Linux the WebKitGTK packages Tauri needs:

```sh
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libxdo-dev
```

```sh
npm install
npm run tauri dev     # run the app
npm run check         # tokens + type-check
npm test              # tokens + UI tests
cargo test --workspace
```

Layout:

- `crates/feedrico-core`: all domain logic (Library, Feeds, Articles). No Tauri dependency, tested headless.
- `src-tauri`: thin Tauri shell exposing the core as commands.
- `src`: Svelte 5 UI.
- `tokens`: design tokens (W3C DTCG JSON). `npm run tokens` builds CSS variables and a typed accessor, and fails if any colour pair misses WCAG 2.2 AA.

## Licence

[GPL-3.0](LICENSE)
