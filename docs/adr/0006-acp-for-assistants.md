# ACP for Assistants: reject as the single protocol for now; keep one headless adapter per CLI

Status: **Proposed** (issue #126, parent epic #110). Builds on ADR 0005 (the per-CLI headless
Assistant adapters). Spike code: `spikes/acp-client/` on branch `research/assistant-acp` (throwaway, own Cargo
workspace, not wired into the app). Raw numbers and commands: the appendix below.

## Question

Could Feedrico speak the Agent Client Protocol (JSON-RPC over stdio) to every Assistant, instead
of one adapter per CLI (ADR 0005)? The spike is a minimal Rust client on the
`agent-client-protocol` crate (5.3.0, protocol v1). It advertises default client capabilities
(no `fs.*`, no `terminal`), opens a session in an empty temp directory, sends
`Summarize: <short text>`, collects `agent_message_chunk` updates and answers every
`session/request_permission` with a reject option (or cancel).

## What we measured (2026-11-14, Linux, vibe 3.1.2, claude 2.1.325, adapter 0.107.0)

The machine was under heavy load (load average 40+, other agents running), so absolute times are
noisy. Read them as orders of magnitude. Real runs per agent stayed small (see the appendix).

| Path | Handshake (spawn to `initialize`) | `session/new` done | First answer text | Done |
|---|---|---|---|---|
| `vibe-acp` (ACP) | 2.1 s, 11.6 s (load) | 3.2 s, 18.4 s | 5.9 s, 22.1 s | 6.1 s, 22.6 s |
| `vibe -p` (headless, epic recipe) | n/a | n/a | n/a | 17 s, 24 s |
| `claude-agent-acp` (ACP, Node adapter + existing `claude`) | 0.4 s | 2.1 s | 5.5 s | 6.1 s |
| `claude -p` (headless, epic recipe, haiku) | n/a | n/a | n/a | 6.3 s, 9.3 s |

- ACP startup is not the problem. It costs about what the headless path costs; under load Vibe
  was equally slow either way, and the Claude adapter adds well under a second to the handshake.
- The Claude adapter defaults to the account's biggest model (it reported `claude-opus-5-5`)
  and loads the full Claude Code prompt and tools: 27,019 tokens of context used for a 30-word
  summary, about 0.15 USD notional, and it moved the plan's 5-hour window (0.61 utilised).
  `claude -p --tools "" --model haiku` is far cheaper. Model and context choice would need
  adapter-specific session options.

## Findings

### Auth: both use the existing plan login, no API key

- Vibe: `initialize` lists one auth method (`browser-auth`), but the session ran fine on the
  stored browser login with no `MISTRAL_API_KEY` set. Vibe read the keyring itself.
- Claude: `auth_methods` is empty, the adapter drove the existing `claude` binary
  (`CLAUDE_CODE_EXECUTABLE`), and the usage updates carried the plan's rate-limit windows. No
  `ANTHROPIC_API_KEY` was set. The adapter does not ship its own credentials or login flow.
- Feedrico still never logs in, never reads credentials, never bundles a CLI. ACP itself does not
  change that rule. One caveat: the standalone adapter archive and the npm package both bundle a
  Claude CLI. We would have to point the adapter at the user's own binary
  (`CLAUDE_CODE_EXECUTABLE`) so we stay inside the policy risk recorded in epic #110.

### Protocol surface versus our no-tools rule: this is the blocker

ACP is a full coding-agent protocol. The client cannot say "no tools"; it can only decline to
offer `fs` and `terminal` capabilities, and those only cover the client-side file and terminal
methods. Agents have their own built-in tools.

- Not advertising `fs`/`terminal` was accepted by both agents; neither crashed or complained.
- **Claude adapter, default mode `auto`: it ran tools without asking.** A prompt asking it to read
  `/etc/hostname` and run `id` made it read the file and execute the command with no
  `session/request_permission` at all. Our "no tools, empty cwd, no user settings" guarantee does
  not hold by default. Getting it back means using adapter-specific `_meta.claudeCode.options`
  or `session/set_mode` to `default` (Manual) plus rejecting every request. We did not verify
  that this closes the hole. That is a per-agent adapter again, just over a different wire.
- **Vibe, default mode `accept-edits`**: the same prompt produced four
  `session/request_permission` calls; we rejected them and the agent reported both actions as
  denied. So "always reject" works for Vibe, but the mode default is not safe for edits, and
  the agent lists the user's own skills and custom agent profiles in `session/new`: user config
  is loaded, unlike `--setting-sources ""` on `claude -p`.
- Both agents persist sessions (`session/list`, `load_session`), and Vibe titled and stored the
  session. Ephemeral results (epic guardrail) would need `session/close`/`delete` handling that
  `claude -p --no-session-persistence` gives us for free.
- Cost of the protocol: sessions, mode state, config options, plans, thought chunks, usage
  updates, permission requests, unstable extensions. Output is still untrusted text that must
  go through the sanitizer.

### Install step for a Newcomer

- Vibe: nothing extra. `vibe-acp` ships with Vibe (`vibe --version` 3.1.2 had it on `PATH`).
- Claude: an extra install. Either `npm install @agentclientprotocol/claude-agent-acp`
  (13 s, 108 packages, 421 MB in `node_modules` because it pulls its own Claude CLI), or a
  218 MB standalone release archive (Linux x64), plus setting `CLAUDE_CODE_EXECUTABLE` to the
  user's `claude`. A Newcomer who has Claude Code installed should not also need Node or a
  second 200 MB download for one summary button. Compared with the headless path, which needs
  only the `claude` they already have.

## Decision (proposed)

**Reject ACP as the single Assistant protocol for now.** Keep the per-CLI headless adapters of
ADR 0005 for Claude Code and Mistral Vibe.

Reasons, in order of weight:

1. **No tools is not expressible.** The headless adapters can say `--tools ""` (Claude) or
   `--enabled-tools none` (Vibe) plus an empty cwd and ignored settings. In ACP the same
   guarantee depends on each agent's mode defaults and non-standard options, and the Claude
   adapter's default ran shell commands unprompted. That breaks an epic guardrail.
2. **Extra install for Claude hurts the Newcomer** (Node or 218 MB, plus an env var), and the
   cheapest Claude path (haiku, no tools, no session) is not what the adapter does by default.
3. **No latency gain.** ACP is no faster than `-p`, and the adapter adds a second process.
4. The benefit (one protocol for many agents) is real but not needed yet: two CLIs, two small
   adapters.

Persona effects:

- **Newcomer (hurt by adopting):** extra adapter install, heavier models, more setup. Rejecting
  keeps setup at "the CLI you already have".
- **Offline Reader (hurt by adopting):** more surface that can phone home or touch files (agent
  tools, MCP servers advertised by the adapter, persisted sessions). Rejecting keeps the
  guarantee: no Assistant set up means no new requests, and a set-up Assistant sees only the
  Article text.
- **Everyone (helped by ACP):** one protocol could bring Codex, Gemini and Copilot with little
  code. Revisit below.

## Revisit when

- An ACP agent can be started with tools disabled through a standard option (not an adapter
  `_meta` key), or ACP gains a "no tools" session capability.
- Claude ships ACP inside the `claude` binary itself, so there is no second install.
- We want an Assistant that only speaks ACP (Codex, Gemini, a local Ollama bridge). Then add
  ACP as one more adapter behind the same Assistant trait, restricted to agents that pass a
  no-tools check (reject-all permissions, no `fs`/`terminal`, empty cwd), not as the replacement.

## Consequences

- No change to the Assistant contract. The ADR 0005 adapters stay.
- `spikes/acp-client/` stays on the research branch as evidence. It is not part of the app
  build or the Cargo workspace.

## Appendix: raw measurements and how to reproduce

Milliseconds from process spawn, load average 40+ (noisy):

- `vibe-acp` run 1: init 2055, session_new 3206, first_chunk 5873, done 6078
- `vibe-acp` runs 2 and 3 (under build load): init 11672 / 11598, session_new 17915 / 18367,
  first_chunk 21212 / 22115, done 22420 / 22647
- `vibe -p`: wall 24.28 s, 17.17 s (also under load)
- `claude-agent-acp` run 1: init 403, session_new 2145, first_chunk 5474, done 6091
- `claude -p` (haiku, no tools): wall 6.27 s, 9.26 s
- Tool-trigger prompt ("read /etc/hostname, run id"):
  - `claude-agent-acp`: done 95059. Ran Read and Bash with no permission request (default mode
    `auto`); the output contained the hostname and the `id` result.
  - `vibe-acp`: four `session/request_permission` calls, all rejected; the agent reported both
    actions as denied. Done 54568.
- Claude adapter usage update: 27,019 tokens of context, model `claude-opus-5-5`, notional cost
  0.148 USD, 5-hour window 0.61 used.

Real runs on the user's plans: Claude 4 (two ACP, two `-p`), Vibe 5 (three ACP, two `-p`). No
`ANTHROPIC_API_KEY` or `MISTRAL_API_KEY` was set.

```sh
cd spikes/acp-client && PATH=/snap/bin:$PATH cargo build --release
B=target/release/acp-client-spike
$B --command "$HOME/.local/bin/vibe-acp"
npm install --prefix "$SCRATCH" @agentclientprotocol/claude-agent-acp      # 13 s, 421 MB
$B --command "env CLAUDE_CODE_EXECUTABLE=$(which claude) $SCRATCH/node_modules/.bin/claude-agent-acp"
```

The client sends default `ClientCapabilities` (no `fs`, no `terminal`), uses an empty temporary
cwd, rejects permission requests by option kind, and otherwise cancels.

Not tested (run budget): whether `session/set_mode` to `default` or
`_meta.claudeCode.options` fully disables the Claude adapter's tools.
