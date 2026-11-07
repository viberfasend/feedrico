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

## Licence

[GPL-3.0](LICENSE)
