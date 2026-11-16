# Save Assistant results in the Library, only on request, in their own table

Status: **Proposed** (issue #127, parent epic #110). Draft for the design session that #127
asks for. Amends [ADR 0005 rule 7](0005-assistants-run-the-users-own-cli.md#rules) (ephemeral
results) once accepted. No code yet.

## Context

An Assistant run produces one result per Article: a summary, a translation, an explanation.
The result is untrusted (ADR 0005 rule 6). Today it is shown once and discarded (rule 7).
Readers want to keep the results they found useful and read them again with the Article,
offline and on another machine.

## Decision (proposed)

Add an `assistant_result` table to `library.db` in a new migration V6. This raises the
Library format version from 5 to 6, so it is a Library format change (`feat(library)!`).

| Column | Type | Meaning |
|---|---|---|
| `id` | INTEGER PRIMARY KEY | |
| `article_id` | INTEGER NOT NULL, FK → `article`, ON DELETE CASCADE | The Article the result belongs to |
| `kind` | TEXT NOT NULL | `summary`, `translation`, `explanation` |
| `assistant` | TEXT NOT NULL | The Assistant that wrote it, e.g. `Claude Code` |
| `model` | TEXT | The model, when known |
| `content` | TEXT NOT NULL | The result text, sanitized before storage |
| `created_at` | INTEGER NOT NULL | Unix seconds when the user kept it |

- **Saved only on request.** A "Keep" action on a shown result, never automatically.
- **Never edited.** For a different result the reader runs the Assistant again and keeps the new
  one. An Article can hold several results.
- **Pruning.** Results go with their Article (CASCADE). Starred Articles are never pruned, so
  their results stay.
- **Display.** Kept results render through the sanitizer like all Article content, with the same
  "Written by {Assistant} · may be wrong" label (rule 8).

## Open questions for the design session

- **Search.** Should kept results be searchable? Option: a separate FTS table joined in search,
  instead of changing `article_fts`. Helps the Archivist; costs index size.
- **One per kind?** Keep every result, or replace the earlier result of the same kind?
- **Where in the UI.** Keep button in the Assistant panel; where kept results show in the
  reader (collapsed section under the Article?).
- **Export and backup.** Results are in `library.db`, so backups carry them. Confirm nothing else
  is needed.

## Rejected options

- **Save automatically.** Writes without the reader asking and grows every Library, including
  those of readers who glance at a result once.
- **A separate file in the Library folder.** ADR 0002 keeps structured data in `library.db`; a
  second store adds a second thing to keep consistent when a stick is pulled.
- **Store in the Article row.** Results are derived, not source; a separate table keeps the
  Article as fetched and allows several results.
- **Avoid a format change.** Any table older apps don't know is a format change. Older apps must
  refuse the Library (scenario B), not open it and ignore data.

## Consequences

- Library format version 6. Every Library is migrated when first opened by an app with V6,
  whether or not its reader uses Assistants: the migration only adds an empty table.
- Feedrico 0.4.0 and older refuse a v6 Library with "This Library needs a newer Feedrico".
- Tests: migration from v5 with backup, CASCADE on prune and delete, Starred keeps results,
  sanitizer on store and display.

## Personas

- **Carrier (Jonas):** helped. Kept results travel with the Library and show the same on another
  machine.
- **Offline Reader (Paul):** helped. Kept results read offline; nothing is written unless Paul
  asks. Keeping a result sends nothing anywhere.
- **Host (Ute):** protected by scenario B: her older app refuses the newer Library and writes
  nothing to the stick.
- **Archivist (Henrik):** helped if results are searchable (open question). Results are small
  text: 1,000 results of about 2 KB add about 2 MB.
- **Skimmer (Clara)** and **Newcomer (Mira):** not using Assistants, they see no change except
  the one-time upgrade prompt that every format change brings.

## Version scenario matrix A–E

| | Situation | Expected |
|---|---|---|
| **A** | v5 Library, app with V6 | Asks "Upgrade this Library? Older versions won't open it afterwards.", backs up the database, migrates. Back home, the Carrier's older app refuses with "needs Feedrico X" and changes nothing. |
| **B** | v6 Library, app ≤ 0.4.0 | "This Library needs a newer Feedrico." Nothing written to the stick. |
| **C** | Same version, different OS | Opens identically; the change adds no paths or files. |
| **D** | Stick pulled mid-session | Migration runs in one transaction; at most the last action is lost; stale lock offers *Open anyway*. |
| **E** | Portable Mode from the stick | App and Library travel together; no mismatch. |
