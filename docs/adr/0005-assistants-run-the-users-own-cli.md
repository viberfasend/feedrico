# Assistants run the user's own signed-in AI CLI, headless, one shot, with no tools

Issue #111 (epic #110). Some reading work (summaries, translations, digests, triage, search) is
a good fit for an AI model, but Feedrico is a local-only app with no account and no backend. The
decision: an optional **Assistant** is an AI command-line tool the user already installed and
signed in to themselves (Claude Code `claude`, Mistral Vibe `vibe`). Feedrico launches that
binary headless, one shot: the prompt goes in on stdin, the answer comes back as JSON, with every
tool disabled and an empty temporary working directory. The user's own CLI does the work under
the user's own plan. A small `Assistant` trait in the Rust core has one adapter per CLI.

## Rejected options

- **An API key in Feedrico.** That is an account and a secret stored inside a local-only app, and
  it is exactly what the user does not want.
- **The Claude Agent SDK.** A Node/Python library meant for developer API keys. It does not fit
  the Rust core, and it would pull a runtime into a portable app.
- **Agent Client Protocol (ACP), for now.** Many agents speak it (Vibe ships `vibe-acp`), but
  Claude needs a separate `claude-agent-acp` adapter, and ACP is a full agent protocol
  (sessions, tool calls, permissions, file system) for what is text in, text out. Evaluated
  later in its own issue (#126).

## Rules

These apply to every Assistant feature and may not be relaxed silently.

1. **Credentials.** Feedrico never offers a login, never reads, copies or stores credentials
   (no credential files, no keyrings), and never bundles or modifies a CLI. It only runs the
   binary the user installed and signed in to, and that CLI's own status command
   (`claude auth status --json`).
2. **Off by default.** Nothing appears in the UI until the user has set up an Assistant. With
   none set up, Feedrico makes exactly the requests it makes today.
3. **Machine-local setup.** The Assistant setup lives in the app data directory next to the
   recent-Libraries list, never in the Library. A Library carried to another machine never
   triggers that machine's CLI.
4. **Explicit action only.** Every run starts from a click, key or command. No background runs,
   no runs at startup, none on refresh.
5. **No tools, empty cwd.** The CLI runs with every tool disabled, in a fresh empty temporary
   directory, ignoring the user's project and user settings and MCP servers, so Article content
   cannot make it touch files, the shell or the web.
6. **Untrusted input and output.** Article text is data: prompts frame it as data, never as
   instructions. The output is untrusted too and goes through the Rust sanitizer before the
   webview shows it.
7. **Ephemeral results.** Results are not written to the Library. Saving them is a separate,
   later change with a Library format change (#127), which must walk the version matrix A-E.
8. **Honest labels and budgets.** Every result says which Assistant wrote it and that it may be
   wrong. Detection is lazy and runs never happen on the UI thread.

## Verified headless commands

Verified on a Linux dev machine on 2026-11-14 with no `ANTHROPIC_API_KEY` or `MISTRAL_API_KEY`
set:

```sh
# Claude Code 2.1.x, signed in with a claude.ai plan (`claude auth status --json` -> "authMethod": "claude.ai")
echo "<prompt>" | claude -p --tools "" --strict-mcp-config --setting-sources "" \
  --no-session-persistence --model haiku --output-format json \
  --json-schema '{"type":"object","properties":{"summary":{"type":"string"}},"required":["summary"]}'
# -> {"type":"result","subtype":"success","structured_output":{"summary":"..."}}, 1.7 s

# Mistral Vibe 3.1.x, signed in through the browser (keyring)
echo "<prompt>" | vibe -p --output json --max-turns 1 --enabled-tools none
# -> JSON array of session entries; the last "message" entry holds the answer
```

**Never use `claude --bare`.** In bare mode, authentication is strictly `ANTHROPIC_API_KEY`;
the plan login is never read, so the Assistant would silently stop working for plan users (or
tempt us to ask for a key, which rule 1 forbids).

Vibe exit codes: 0 finished, 1 could not start, 2 API or runtime failure, 3 stopped early.
Without `--auto-approve`, tools that need approval are denied in headless mode. A machine
without an OS keyring cannot keep the browser login and needs `api_key_env`.

## Policy risk: re-check before each release that touches Assistants

Anthropic's terms are the reason for rule 1. Quotes recorded on 2026-11-14:

- Support article "Use the Claude Agent SDK with your Claude plan" (updates of 2026-06-15 and
  2026-10-07): "Claude Agent SDK, `claude -p`, and third-party app usage still draw from your
  subscription limits."
  <https://support.claude.com/en/articles/15036540-use-the-claude-agent-sdk-with-your-claude-plan>
- Legal and compliance: the restrictions do not "prevent an end user from signing in to the
  unmodified Claude Code binary with their own Claude subscription."
  <https://code.claude.com/docs/en/legal-and-compliance>
- But Anthropic "does not permit third-party developers to offer Claude.ai login into their own
  applications, or to route requests through Free, Pro, Max, or Ultra plan credentials on behalf
  of their users", and the Agent SDK overview says the same "unless previously approved".
  <https://code.claude.com/docs/en/agent-sdk/overview>

Our reading: launching the unmodified binary the user signed in to, on the user's own click, is
the end user using Claude Code. Offering a login, touching credentials or routing requests
ourselves would not be. If these pages change, this ADR must be revisited, and the Claude adapter
may have to go. Mistral's side: <https://docs.mistral.ai/vibe/code/cli/work-with-cli>.

## Personas

- **The Offline Reader (Paul): hurt and protected.** Article text leaves the machine for
  Anthropic or Mistral when he runs an Assistant. Opt-in, off by default, per-action runs and a
  disclosure saying where the text goes keep this his choice; with no Assistant set up his
  traffic is unchanged. A fully local option (Ollama) is planned (#125).
- **The Host (Ute): protected.** Jonas's Library opened on her machine never runs her CLI or
  uses her plan, because the setup is machine-local.
- **The Carrier (Jonas): unaffected.** The Library does not change, so there is nothing to walk
  in the version matrix A-E until results are saved in it (#127).
- **The Archivist (Henrik)** gains triage and natural-language search at 10k unread, with no
  cost to cold start or scrolling. **The Skimmer (Clara)** gets summaries and a briefing of
  Today. **The Newcomer (Mira)** sees nothing new until she sets it up.
- **Accessibility spectrum:** "explain in plain language" and translation help limited attention
  and reading outside one's language; the result panel must be keyboard and screen-reader
  friendly.

## Consequences

- The spec's network policy gains one carve-out (the user's own CLI, on a run) and an
  "Assistants" section; Feedrico itself still talks only to feed and website hosts and GitHub
  Releases.
- Every adapter is a thin process wrapper tested with fake CLIs; tests never need a real `claude`,
  `vibe` or the network.
- Changing any rule above, or adding a way for Feedrico to hold credentials, needs a new ADR that
  supersedes this one.
