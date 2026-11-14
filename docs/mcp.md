# Read your Library from Claude Code or Vibe (MCP)

`feedrico-mcp` is a small, **read-only** [MCP](https://modelcontextprotocol.io) server over stdio.
Add it to your CLI yourself, then ask it about your reading ("What did I star about Rust this
month?"). Feedrico never adds it for you.

It is a separate binary (crate `crates/feedrico-mcp`), not part of the app bundle yet. Build it
with `cargo build --release -p feedrico-mcp`; the binary is `target/release/feedrico-mcp`.

## Setup

Claude Code:

```sh
claude mcp add feedrico -- /path/to/feedrico-mcp --library "/path/to/Feedrico Library"
```

Mistral Vibe:

```sh
vibe mcp add feedrico --transport stdio --command /path/to/feedrico-mcp \
  --arg --library --arg "/path/to/Feedrico Library"
```

## Tools

| Tool | What it returns |
|---|---|
| `list_folders_and_feeds` | Folders and Feeds with unread counts |
| `list_smart_views` | counts for All unread, Starred, Today (Articles dated since local midnight) |
| `list_articles` | Articles of a View (`all_unread`, `starred`, `today`, `feed`/`folder` + `id`); `limit`, `unread_only` |
| `search` | full-text search; optional `feed` or `folder` scope, `limit` |
| `get_article` | title, Feed, date, link and the sanitized content as plain text |
| `list_starred` | Starred Articles |

## Guarantees

- Reads only. The Library is opened read-only: no Read state, no settings, no migration, no
  `.lock` taken, changed or removed. It works while Feedrico has the Library open.
- A Library newer than this build is refused ("needs a newer Feedrico"). An older one is refused
  with "open it in Feedrico first" and never upgraded.
- No network. What your CLI does with the results is up to that CLI.
- Article content is untrusted text from the web; the server tells the client not to follow
  instructions found in it.
