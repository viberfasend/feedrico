//! Run one Assistant [`Task`] (#113).
//!
//! `run` spawns the found Assistant's CLI in a fresh empty temporary directory with its hardened
//! flags, writes the prompt to stdin, streams [`Event`]s to `on_event`, and returns the final
//! text. It stops on `cancel`, killing the process tree, or after `task.timeout`, and maps
//! failures to [`AssistantError`].
//!
//! Each CLI has an adapter (`Adapter`) that owns its exact command line and knows how to read
//! its output. The temporary directory is a `TempDir`, so it is deleted on every exit path.

use std::ffi::OsString;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::time::Instant;

use super::{AssistantError, AssistantKind, Event, FoundAssistant, Task};

/// Cancels a running Task. Clone it and keep it, then call [`Cancel::cancel`] from anywhere.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

type Answer = (String, Option<Value>);

/// How often a running Task looks at its [`Cancel`].
const CANCEL_POLL: Duration = Duration::from_millis(25);
/// The most stderr we keep. It's only used to explain a failure.
const STDERR_CAP: usize = 64 * 1024;
const MESSAGE_CAP: usize = 500;
const SYSTEM_PROMPT_FILE: &str = "system-prompt.txt";
/// Where a `Task::json_schema` is written for CLIs that take it as a file.
const SCHEMA_FILE: &str = "output-schema.json";

/// Run `task` on `assistant`. On success it returns the [`Event::Done`] payload, which has also
/// been sent to `on_event`.
pub async fn run(
    assistant: &FoundAssistant,
    task: &Task,
    cancel: Cancel,
    mut on_event: impl FnMut(Event) + Send,
) -> Result<Answer, AssistantError> {
    if cancel.is_cancelled() {
        return Err(AssistantError::Cancelled);
    }
    if assistant.kind == AssistantKind::Ollama {
        // Not a process: the setup's path is the loopback base URL of the local server.
        let base_url = assistant.path.to_string_lossy();
        return super::ollama::run(&base_url, task, cancel, &mut on_event).await;
    }
    let adapter = adapter_for(assistant.kind);

    // Deleted when it drops, on every exit path.
    let dir = tempfile::Builder::new()
        .prefix("feedrico-assistant-")
        .tempdir()
        .map_err(|e| failed(format!("couldn't create a temporary folder: {e}")))?;
    if let Some(text) = adapter.system_prompt_file(task) {
        std::fs::write(dir.path().join(SYSTEM_PROMPT_FILE), text)
            .map_err(|e| failed(format!("couldn't write the system prompt: {e}")))?;
    }
    if let Some(schema) = &task.json_schema {
        std::fs::write(dir.path().join(SCHEMA_FILE), schema.to_string())
            .map_err(|e| failed(format!("couldn't write the output schema: {e}")))?;
    }

    let mut command = Command::new(&assistant.path);
    command
        .args(adapter.args(task, dir.path()))
        .current_dir(dir.path())
        .envs(adapter.env(dir.path()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flash

    let mut child = spawn_with_retry(&mut command).await?;
    let deadline = Instant::now() + task.timeout;

    // The prompt goes to stdin and never onto the command line.
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let prompt = task.prompt.clone().into_bytes();
    tokio::spawn(async move {
        let _ = stdin.write_all(&prompt).await;
        let _ = stdin.shutdown().await;
    });
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let stderr_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        let _ = (&mut stderr)
            .take(STDERR_CAP as u64)
            .read_to_end(&mut buf)
            .await;
        // Keep draining so the CLI never blocks on a full pipe.
        let _ = tokio::io::copy(&mut stderr, &mut tokio::io::sink()).await;
        String::from_utf8_lossy(&buf).into_owned()
    });
    let mut lines = BufReader::new(child.stdout.take().expect("stdout is piped")).lines();

    let mut parser = adapter.parser(task);
    let mut poll = tokio::time::interval(CANCEL_POLL);
    let stream_result: Result<(), AssistantError> = loop {
        tokio::select! {
            line = lines.next_line() => match line {
                Ok(Some(line)) => {
                    if line.trim().is_empty() {
                        continue;
                    }
                    if let Err(e) = parser.line(&line, &mut on_event) {
                        break Err(e);
                    }
                }
                Ok(None) => break Ok(()),
                Err(e) => break Err(AssistantError::Malformed {
                    message: format!("the output wasn't text: {e}"),
                }),
            },
            _ = poll.tick() => {
                if cancel.is_cancelled() {
                    break Err(AssistantError::Cancelled);
                }
            }
            _ = tokio::time::sleep_until(deadline) => break Err(AssistantError::TimedOut),
        }
    };

    if let Err(e) = stream_result {
        kill_tree(&mut child).await;
        return Err(e);
    }

    // The stream ended. Wait for the exit status, still honouring cancel and the deadline.
    let status = loop {
        tokio::select! {
            status = child.wait() => break status,
            _ = poll.tick() => {
                if cancel.is_cancelled() {
                    kill_tree(&mut child).await;
                    return Err(AssistantError::Cancelled);
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                kill_tree(&mut child).await;
                return Err(AssistantError::TimedOut);
            }
        }
    };
    let code = status.ok().and_then(|s| s.code());
    let stderr_text = tokio::time::timeout(Duration::from_secs(2), stderr_task)
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default();

    let answer = parser.finish(code, &stderr_text)?;
    on_event(Event::Done {
        text: answer.0.clone(),
        structured: answer.1.clone(),
    });
    Ok(answer)
}

/// Spawn, retrying briefly on "text file busy" (ETXTBSY). That happens when the executable was
/// written a moment ago, for example by an installer or an update, while another thread forked.
async fn spawn_with_retry(command: &mut Command) -> Result<Child, AssistantError> {
    let mut attempts = 0;
    loop {
        match command.spawn() {
            Ok(child) => return Ok(child),
            Err(e) if e.raw_os_error() == Some(26) && attempts < 10 => {
                attempts += 1;
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Err(_) => return Err(AssistantError::NotInstalled),
        }
    }
}

fn failed(message: String) -> AssistantError {
    AssistantError::Failed { message }
}

/// Kill the CLI and everything it started, then reap it.
async fn kill_tree(child: &mut Child) {
    if let Some(pid) = child.id() {
        kill_group(pid);
    }
    let _ = child.start_kill();
    let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
}

#[cfg(unix)]
fn kill_group(pid: u32) {
    // The CLI leads its own process group (`process_group(0)`), so `-pid` is the whole tree.
    // SAFETY: `kill` has no memory-safety preconditions; the worst outcome is an error code.
    unsafe {
        libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
    }
}

#[cfg(windows)]
fn kill_group(pid: u32) {
    use std::os::windows::process::CommandExt;
    let _ = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(0x0800_0000)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

// ---- adapters -------------------------------------------------------------------------------

/// What an Assistant adapter does. One per CLI; new CLIs add an implementation.
trait Adapter {
    /// The exact arguments. `dir` is the empty temporary directory; the files named by
    /// [`SYSTEM_PROMPT_FILE`] and [`SCHEMA_FILE`] are written there before the spawn.
    fn args(&self, task: &Task, dir: &Path) -> Vec<OsString>;
    /// Text to put in a file in the temporary directory, for CLIs that take the system prompt
    /// from a file.
    fn system_prompt_file(&self, _task: &Task) -> Option<String> {
        None
    }
    /// Extra environment variables for the CLI, for CLIs that take the system prompt from one.
    fn env(&self, _dir: &Path) -> Vec<(OsString, OsString)> {
        Vec::new()
    }
    fn parser(&self, task: &Task) -> Box<dyn Parser>;
}

/// Reads one CLI's output, a line at a time.
trait Parser: Send {
    fn line(&mut self, line: &str, emit: &mut dyn FnMut(Event)) -> Result<(), AssistantError>;
    /// The answer once the process has exited. `code` is `None` when it was killed by a signal.
    fn finish(&mut self, code: Option<i32>, stderr: &str) -> Result<Answer, AssistantError>;
}

fn adapter_for(kind: AssistantKind) -> Box<dyn Adapter + Send> {
    match kind {
        AssistantKind::ClaudeCode => Box::new(ClaudeCode),
        AssistantKind::MistralVibe => Box::new(MistralVibe),
        AssistantKind::Codex => Box::new(Codex),
        AssistantKind::Gemini => Box::new(Gemini),
        AssistantKind::Ollama => unreachable!("Ollama is an HTTP API, handled in run()"),
    }
}

fn os(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

struct ClaudeCode;

impl Adapter for ClaudeCode {
    /// `--verbose` is required with `--output-format stream-json` in `-p` mode.
    /// `--system-prompt` replaces Claude Code's own coding prompt, so the answer depends only on
    /// our prompt. Never `--bare`: it disables the plan login.
    fn args(&self, task: &Task, _dir: &Path) -> Vec<OsString> {
        let mut args = os(&[
            "-p",
            "--output-format",
            "stream-json",
            "--include-partial-messages",
            "--verbose",
            "--tools",
            "",
            "--strict-mcp-config",
            "--setting-sources",
            "",
            "--no-session-persistence",
        ]);
        if let Some(model) = &task.model {
            args.extend(os(&["--model", model]));
        }
        if let Some(schema) = &task.json_schema {
            args.extend(os(&["--json-schema", &schema.to_string()]));
        }
        args.extend(os(&["--system-prompt", &task.system_prompt]));
        args
    }

    fn parser(&self, _task: &Task) -> Box<dyn Parser> {
        Box::new(ClaudeParser::default())
    }
}

#[derive(Default)]
struct ClaudeParser {
    text: String,
    outcome: Option<Result<Answer, AssistantError>>,
}

impl Parser for ClaudeParser {
    fn line(&mut self, line: &str, emit: &mut dyn FnMut(Event)) -> Result<(), AssistantError> {
        let v = parse_json(line)?;
        match v.get("type").and_then(Value::as_str) {
            Some("stream_event") => {
                let delta = &v["event"]["delta"];
                if v["event"]["type"] == "content_block_delta"
                    && delta["type"] == "text_delta"
                    && let Some(text) = delta["text"].as_str()
                    && !text.is_empty()
                {
                    self.text.push_str(text);
                    emit(Event::Text {
                        delta: text.to_string(),
                    });
                }
            }
            Some("result") => {
                let result = v["result"].as_str();
                self.outcome = Some(if v["is_error"] == true {
                    let status = v["api_error_status"].as_u64();
                    Err(claude_error(result.unwrap_or_default(), status))
                } else {
                    let structured = v.get("structured_output").filter(|s| !s.is_null());
                    Ok((
                        result.map_or_else(|| self.text.clone(), str::to_string),
                        structured.cloned(),
                    ))
                });
            }
            // System, assistant, user and rate-limit events carry nothing we show. Tool-use
            // blocks are never acted on: there are no tools.
            _ => {}
        }
        Ok(())
    }

    fn finish(&mut self, code: Option<i32>, stderr: &str) -> Result<Answer, AssistantError> {
        if let Some(outcome) = self.outcome.take() {
            return outcome;
        }
        match code {
            Some(0) => Err(AssistantError::Malformed {
                message: "the Assistant ended without an answer".into(),
            }),
            _ => Err(from_text_or_failed(stderr, code)),
        }
    }
}

fn claude_error(text: &str, status: Option<u64>) -> AssistantError {
    match status {
        Some(401 | 403) => AssistantError::NotSignedIn,
        Some(429) => AssistantError::UsageLimit,
        _ => from_text_or_failed(text, None),
    }
}

struct MistralVibe;

impl Adapter for MistralVibe {
    fn args(&self, task: &Task, dir: &Path) -> Vec<OsString> {
        let system_prompt_file = dir.join(SYSTEM_PROMPT_FILE);
        let mut args = os(&[
            "-p",
            "--output",
            "streaming",
            "--max-turns",
            "1",
            "--enabled-tools",
            "none",
        ]);
        if let Some(model) = &task.model {
            args.extend(os(&["--model", model]));
        }
        args.extend(os(&[
            "--time-limit",
            &task.timeout.as_secs().max(1).to_string(),
        ]));
        args.push("--system-prompt-file".into());
        args.push(system_prompt_file.into());
        args
    }

    fn system_prompt_file(&self, task: &Task) -> Option<String> {
        Some(task.system_prompt.clone())
    }

    fn parser(&self, _task: &Task) -> Box<dyn Parser> {
        Box::new(VibeParser::default())
    }
}

/// Vibe's `--output streaming` writes one JSON entry per line. An assistant message entry
/// (`type: "message"`, `role: "assistant"`) repeats with the text so far, so we send only what's
/// new for each entry id.
#[derive(Default)]
struct VibeParser {
    /// Entry id and the text of it sent so far, in order of first appearance.
    entries: Vec<(String, String)>,
}

impl Parser for VibeParser {
    fn line(&mut self, line: &str, emit: &mut dyn FnMut(Event)) -> Result<(), AssistantError> {
        let v = parse_json(line)?;
        if v["type"] != "message" || v["role"] != "assistant" {
            return Ok(());
        }
        let text: String = v["content"]
            .as_array()
            .map(|parts| {
                parts
                    .iter()
                    .filter(|p| p["type"] == "text")
                    .filter_map(|p| p["text"].as_str())
                    .collect()
            })
            .unwrap_or_default();
        let id = v["id"].as_str().unwrap_or_default().to_string();
        let index = match self.entries.iter().position(|(i, _)| *i == id) {
            Some(i) => i,
            None => {
                self.entries.push((id, String::new()));
                self.entries.len() - 1
            }
        };
        let seen = &mut self.entries[index].1;
        let delta = match text.strip_prefix(seen.as_str()) {
            Some(rest) => rest.to_string(),
            None => text.clone(), // The entry was rewritten: send it whole.
        };
        *seen = text;
        if !delta.is_empty() {
            emit(Event::Text { delta });
        }
        Ok(())
    }

    fn finish(&mut self, code: Option<i32>, stderr: &str) -> Result<Answer, AssistantError> {
        let stderr = strip_vibe_notices(stderr);
        match code {
            Some(0) => {
                let text: String = self.entries.iter().map(|(_, t)| t.as_str()).collect();
                if text.is_empty() {
                    Err(AssistantError::Malformed {
                        message: "the Assistant ended without an answer".into(),
                    })
                } else {
                    Ok((text, None))
                }
            }
            Some(3) => Err(AssistantError::TimedOut),
            Some(1) if is_not_found(&stderr) => Err(AssistantError::NotInstalled),
            code => Err(from_text_or_failed(&stderr, code)),
        }
    }
}

/// Vibe always prints this harmless notice, which would otherwise look like an auth problem.
fn strip_vibe_notices(stderr: &str) -> String {
    stderr
        .lines()
        .filter(|l| !l.starts_with("Reading API keys from the system keyring"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_not_found(text: &str) -> bool {
    let t = text.to_lowercase();
    t.contains("command not found") || t.contains("no such file")
}

/// OpenAI Codex CLI (`codex exec`; flags checked against `codex exec --help` of codex-cli 0.153.4
/// installed on the dev machine, and against the strings of that binary, on 2026-11-14).
///
/// Codex cannot switch every tool off with one flag, so the hardening is layered:
/// - `-s read-only`: the strictest sandbox. Commands the model asks for can read but never write.
/// - `-c approval_policy="never"`: it never waits for a human, and a refused command just fails.
/// - `--disable shell_tool` (and `unified_exec`): the shell tool is not offered to the model at
///   all (the same effect as `--tools ""` for Claude). Multi-agent, plugins, image viewing and
///   every web-search variant are disabled too; `web_search="disabled"` turns the tool off.
///   Unknown feature names make Codex exit with an error, so only names that exist in 0.153.4
///   are used. Apps, hooks and MCP servers come from the user's and project's config files,
///   which are not loaded (next bullet) and whose project directory is empty anyway.
/// - `--ignore-user-config` and `--ignore-rules`: the user's config.toml (MCP servers, profiles)
///   and execpolicy rules are not loaded. Auth still comes from `CODEX_HOME`, which Feedrico
///   never reads.
/// - `--ephemeral`: no session files are written.
/// - The prompt arrives on stdin (`-`). The system prompt replaces Codex's base instructions
///   through `model_instructions_file`, a file in the temporary directory, and a
///   `json_schema` goes through `--output-schema`, also a file there.
struct Codex;

impl Adapter for Codex {
    fn args(&self, task: &Task, dir: &Path) -> Vec<OsString> {
        let mut args = os(&[
            "exec",
            "--json",
            "--ephemeral",
            "--skip-git-repo-check",
            "--ignore-user-config",
            "--ignore-rules",
            "-s",
            "read-only",
            "-c",
            "approval_policy=\"never\"",
            "-c",
            "web_search=\"disabled\"",
            "--disable",
            "shell_tool",
            "--disable",
            "unified_exec",
            "--disable",
            "multi_agent",
            "--disable",
            "multi_agent_v2",
            "--disable",
            "plugins",
            "--disable",
            "view_image",
            "--disable",
            "web_search_request",
            "--disable",
            "web_search_cached",
            "--disable",
            "standalone_web_search",
        ]);
        if let Some(model) = &task.model {
            args.extend(os(&["-m", model]));
        }
        if task.json_schema.is_some() {
            args.extend(os(&["--output-schema"]));
            args.push(dir.join(SCHEMA_FILE).into_os_string());
        }
        args.push("-c".into());
        let mut instructions = OsString::from("model_instructions_file=\"");
        instructions.push(dir.join(SYSTEM_PROMPT_FILE));
        instructions.push("\"");
        args.push(instructions);
        args.push("-".into());
        args
    }

    fn system_prompt_file(&self, task: &Task) -> Option<String> {
        Some(task.system_prompt.clone())
    }

    fn parser(&self, task: &Task) -> Box<dyn Parser> {
        Box::new(CodexParser {
            has_schema: task.json_schema.is_some(),
            ..CodexParser::default()
        })
    }
}

/// `codex exec --json` writes one event per line (event types `thread.started`, `turn.started`,
/// `item.started`, `item.updated`, `item.completed`, `turn.completed`, `turn.failed`, `error` in
/// codex-cli 0.153.4). It does not stream deltas: an `item.completed` event of type
/// `agent_message` carries a whole message. `turn.failed` carries the error, after an `error`
/// event with the same text.
#[derive(Default)]
struct CodexParser {
    messages: Vec<String>,
    last_error: Option<String>,
    outcome: Option<Result<(), AssistantError>>,
    /// A `--output-schema` was given: the final message must parse as that JSON.
    has_schema: bool,
}

impl Parser for CodexParser {
    fn line(&mut self, line: &str, emit: &mut dyn FnMut(Event)) -> Result<(), AssistantError> {
        let v = parse_json(line)?;
        match v["type"].as_str() {
            Some("item.completed") if v["item"]["type"] == "agent_message" => {
                if let Some(text) = v["item"]["text"].as_str()
                    && !text.is_empty()
                {
                    // Several messages in one turn read as paragraphs. With an output schema the
                    // message is JSON, not text for the reader: buffer it and send only Done.
                    let delta = if self.messages.is_empty() {
                        text.to_string()
                    } else {
                        format!("\n\n{text}")
                    };
                    self.messages.push(text.to_string());
                    if !self.has_schema {
                        emit(Event::Text { delta });
                    }
                }
            }
            Some("error") => self.last_error = v["message"].as_str().map(str::to_string),
            Some("turn.failed") => {
                let message = v["error"]["message"]
                    .as_str()
                    .map(str::to_string)
                    .or_else(|| self.last_error.clone())
                    .unwrap_or_default();
                self.outcome = Some(Err(from_text_or_failed(&message, None)));
            }
            Some("turn.completed") => self.outcome = Some(Ok(())),
            // Reasoning, command and tool items, thread and turn starts, and `error` items
            // (warnings) show nothing. Tool items are never acted on: nothing is offered.
            _ => {}
        }
        Ok(())
    }

    fn finish(&mut self, code: Option<i32>, stderr: &str) -> Result<Answer, AssistantError> {
        match self.outcome.take() {
            Some(Err(e)) => Err(e),
            Some(Ok(())) if !self.messages.is_empty() => {
                let text = self.messages.join("\n\n");
                let structured = if self.has_schema {
                    Some(
                        serde_json::from_str(&text).map_err(|e| AssistantError::Malformed {
                            message: format!("the answer wasn't the JSON that was asked for: {e}"),
                        })?,
                    )
                } else {
                    None
                };
                Ok((text, structured))
            }
            _ => match code {
                Some(0) => Err(AssistantError::Malformed {
                    message: "the Assistant ended without an answer".into(),
                }),
                _ => Err(from_text_or_failed(
                    self.last_error.as_deref().unwrap_or(stderr),
                    code,
                )),
            },
        }
    }
}

/// Google Gemini CLI headless mode (`gemini -p`; flags verified against the official docs at
/// geminicli.com/docs/cli/cli-reference and docs/cli/headless, 2026-11; the binary is not
/// installed on the dev machine, so no flag was run).
///
/// Gemini has no flag that turns every tool off, so it runs in the strictest mode it documents:
/// - `--approval-mode plan` (choices: default, auto_edit, yolo, plan): read-only; the model may
///   not edit files or run commands.
/// - `--extensions none`: the list of extensions to use is only "none", which is never
///   installed, so no extension (and its tools or MCP servers) loads.
/// - It runs in the empty temporary directory, an untrusted folder to Gemini, and Feedrico never
///   passes `--yolo` or `--skip-trust`.
/// - The prompt arrives on stdin; `-p` switches to headless mode and its own text is appended to
///   stdin. The system prompt replaces Gemini's through `GEMINI_SYSTEM_MD`, a file path.
/// - A `json_schema` cannot be passed as a flag: callers must put the JSON shape into the prompt
///   and validate in Rust (as for Vibe).
struct Gemini;

const GEMINI_HEADLESS_PROMPT: &str = "Follow the instructions given on stdin.";

impl Adapter for Gemini {
    fn args(&self, task: &Task, _dir: &Path) -> Vec<OsString> {
        let mut args = os(&[
            "--output-format",
            "stream-json",
            "--approval-mode",
            "plan",
            "--extensions",
            "none",
        ]);
        if let Some(model) = &task.model {
            args.extend(os(&["--model", model]));
        }
        args.extend(os(&["-p", GEMINI_HEADLESS_PROMPT]));
        args
    }

    fn system_prompt_file(&self, task: &Task) -> Option<String> {
        Some(task.system_prompt.clone())
    }

    fn env(&self, dir: &Path) -> Vec<(OsString, OsString)> {
        vec![(
            "GEMINI_SYSTEM_MD".into(),
            dir.join(SYSTEM_PROMPT_FILE).into_os_string(),
        )]
    }

    fn parser(&self, _task: &Task) -> Box<dyn Parser> {
        Box::new(GeminiParser::default())
    }
}

/// `--output-format stream-json`: `init`, `message` (`role` plus a `content` chunk; assistant
/// chunks carry `delta: true` and are incremental, per packages/core/src/output/types.ts),
/// `tool_use`, `tool_result`, `error` (non-fatal warnings) and a final `result` whose `status`
/// is `success` or `error`.
#[derive(Default)]
struct GeminiParser {
    text: String,
    outcome: Option<Result<(), AssistantError>>,
}

impl Parser for GeminiParser {
    fn line(&mut self, line: &str, emit: &mut dyn FnMut(Event)) -> Result<(), AssistantError> {
        let v = parse_json(line)?;
        match v["type"].as_str() {
            Some("message") if v["role"] == "assistant" => {
                if let Some(delta) = v["content"].as_str()
                    && !delta.is_empty()
                {
                    self.text.push_str(delta);
                    emit(Event::Text {
                        delta: delta.to_string(),
                    });
                }
            }
            Some("result") => {
                self.outcome = Some(if v["status"] == "error" {
                    let message = v["error"]["message"].as_str().unwrap_or_default();
                    Err(from_text_or_failed(message, None))
                } else {
                    Ok(())
                });
            }
            // `init`, user messages and warnings carry nothing we show; tool events are never
            // acted on.
            _ => {}
        }
        Ok(())
    }

    fn finish(&mut self, code: Option<i32>, stderr: &str) -> Result<Answer, AssistantError> {
        match self.outcome.take() {
            Some(Err(e)) => Err(e),
            Some(Ok(())) if !self.text.trim().is_empty() => Ok((self.text.clone(), None)),
            _ => match code {
                Some(0) => Err(AssistantError::Malformed {
                    message: "the Assistant ended without an answer".into(),
                }),
                _ => Err(from_text_or_failed(stderr, code)),
            },
        }
    }
}

// ---- shared ---------------------------------------------------------------------------------

fn parse_json(line: &str) -> Result<Value, AssistantError> {
    serde_json::from_str(line).map_err(|e| AssistantError::Malformed {
        message: format!("a line of output wasn't JSON: {e}"),
    })
}

/// Map a CLI's own error text to a typed error. Order matters: sign-in first, then limits,
/// then connection problems.
fn map_error_text(text: &str) -> Option<AssistantError> {
    let t = text.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| t.contains(w));
    if has(&[
        "invalid api key",
        "missing api key",
        "no api key",
        "api key not valid",
        "api key not found",
        "/login",
        "not logged in",
        "not signed in",
        "unauthorized",
        "authentication",
        "sign in again",
        "log in again",
        "refresh token",
        "token_expired",
        "auth method",
        "gemini_api_key",
        "failed to login",
        "401",
    ]) {
        Some(AssistantError::NotSignedIn)
    } else if has(&[
        "usage limit",
        "rate limit",
        "rate_limit",
        "limit reached",
        "hit your limit",
        "quota",
        "too many requests",
        "credit balance",
        "429",
    ]) {
        Some(AssistantError::UsageLimit)
    } else if has(&[
        "unable to connect",
        "connection refused",
        "connection error",
        "connection reset",
        "enotfound",
        "econnrefused",
        "econnreset",
        "network",
        "could not resolve",
        "offline",
        "fetch failed",
    ]) {
        Some(AssistantError::Offline)
    } else {
        None
    }
}

fn from_text_or_failed(text: &str, code: Option<i32>) -> AssistantError {
    if let Some(e) = map_error_text(text) {
        return e;
    }
    let text = text.trim();
    let message = if text.is_empty() {
        match code {
            Some(c) => format!("exit code {c}"),
            None => "the process was stopped".to_string(),
        }
    } else {
        text.chars().take(MESSAGE_CAP).collect()
    };
    AssistantError::Failed { message }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn task() -> Task {
        Task {
            system_prompt: "SYS".into(),
            prompt: "PROMPT".into(),
            json_schema: None,
            model: None,
            timeout: Duration::from_secs(90),
        }
    }

    fn strings(args: Vec<OsString>) -> Vec<String> {
        args.into_iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn claude_command_line_is_exact() {
        let args = strings(ClaudeCode.args(&task(), Path::new("/t")));
        assert_eq!(
            args,
            [
                "-p",
                "--output-format",
                "stream-json",
                "--include-partial-messages",
                "--verbose",
                "--tools",
                "",
                "--strict-mcp-config",
                "--setting-sources",
                "",
                "--no-session-persistence",
                "--system-prompt",
                "SYS",
            ]
        );
    }

    #[test]
    fn claude_command_line_adds_model_and_schema_and_never_bare() {
        let mut t = task();
        t.model = Some("haiku".into());
        t.json_schema = Some(json!({"type": "object"}));
        let args = strings(ClaudeCode.args(&t, Path::new("/t")));
        let at = |flag: &str| args.iter().position(|a| a == flag).unwrap();
        assert_eq!(args[at("--model") + 1], "haiku");
        assert_eq!(args[at("--json-schema") + 1], "{\"type\":\"object\"}");
        assert!(!args.iter().any(|a| a == "--bare" || a == "PROMPT"));
    }

    #[test]
    fn vibe_command_line_is_exact() {
        let args = strings(MistralVibe.args(&task(), Path::new("/t")));
        assert_eq!(
            args,
            [
                "-p",
                "--output",
                "streaming",
                "--max-turns",
                "1",
                "--enabled-tools",
                "none",
                "--time-limit",
                "90",
                "--system-prompt-file",
                "/t/system-prompt.txt",
            ]
        );
        let mut t = task();
        t.model = Some("m".into());
        let args = strings(MistralVibe.args(&t, Path::new("/t")));
        assert_eq!(&args[7..9], ["--model", "m"]);
        assert_eq!(args.last().unwrap(), "/t/system-prompt.txt");
    }

    fn codex_task() -> Task {
        let mut t = task();
        t.model = Some("gpt-5".into());
        t
    }

    #[test]
    fn codex_command_line_is_exact() {
        let args = strings(Codex.args(&codex_task(), Path::new("/t")));
        assert_eq!(
            args,
            [
                "exec",
                "--json",
                "--ephemeral",
                "--skip-git-repo-check",
                "--ignore-user-config",
                "--ignore-rules",
                "-s",
                "read-only",
                "-c",
                "approval_policy=\"never\"",
                "-c",
                "web_search=\"disabled\"",
                "--disable",
                "shell_tool",
                "--disable",
                "unified_exec",
                "--disable",
                "multi_agent",
                "--disable",
                "multi_agent_v2",
                "--disable",
                "plugins",
                "--disable",
                "view_image",
                "--disable",
                "web_search_request",
                "--disable",
                "web_search_cached",
                "--disable",
                "standalone_web_search",
                "-m",
                "gpt-5",
                "-c",
                "model_instructions_file=\"/t/system-prompt.txt\"",
                "-",
            ]
        );
        let args = strings(Codex.args(&task(), Path::new("/t")));
        assert!(!args.iter().any(|a| a == "-m"));
        assert!(!args.iter().any(|a| a == "--output-schema"));
        for banned in [
            "--dangerously-bypass-approvals-and-sandbox",
            "danger-full-access",
            "workspace-write",
            "--search",
            "PROMPT",
        ] {
            assert!(!args.iter().any(|a| a == banned), "{banned}");
        }
    }

    #[test]
    fn codex_adds_output_schema_file_when_asked() {
        let mut t = codex_task();
        t.json_schema = Some(json!({"type": "object"}));
        let args = strings(Codex.args(&t, Path::new("/t")));
        let at = |flag: &str| args.iter().position(|a| a == flag).unwrap();
        assert_eq!(args[at("--output-schema") + 1], "/t/output-schema.json");
    }

    #[test]
    fn codex_parser_collects_agent_messages_and_reads_schema_json() {
        let mut p = CodexParser::default();
        let mut events = Vec::new();
        for line in [
            r#"{"type":"thread.started","thread_id":"t"}"#,
            r#"{"type":"item.completed","item":{"type":"reasoning","text":"thinking"}}"#,
            r#"{"type":"item.completed","item":{"type":"agent_message","text":"Hello"}}"#,
            r#"{"type":"item.completed","item":{"type":"agent_message","text":"world"}}"#,
            r#"{"type":"turn.completed","turn":{"status":"completed"}}"#,
        ] {
            p.line(line, &mut |e| events.push(e)).unwrap();
        }
        let (text, structured) = p.finish(Some(0), "").unwrap();
        assert_eq!(text, "Hello\n\nworld");
        assert_eq!(structured, None);
        assert_eq!(
            events,
            vec![
                Event::Text {
                    delta: "Hello".into()
                },
                Event::Text {
                    delta: "\n\nworld".into()
                },
            ]
        );

        let mut p = CodexParser {
            has_schema: true,
            ..CodexParser::default()
        };
        p.line(
            r#"{"type":"item.completed","item":{"type":"agent_message","text":"{\"answer\":4}"}}"#,
            &mut |_| {},
        )
        .unwrap();
        p.line(r#"{"type":"turn.completed"}"#, &mut |_| {}).unwrap();
        let (text, structured) = p.finish(Some(0), "").unwrap();
        assert_eq!(text, "{\"answer\":4}");
        assert_eq!(structured, Some(json!({"answer": 4})));
    }

    #[test]
    fn codex_parser_turn_failed_and_stderr_map_to_errors() {
        let mut p = CodexParser::default();
        p.line(
            r#"{"type":"error","message":"rate limit reached"}"#,
            &mut |_| {},
        )
        .unwrap();
        p.line(
            r#"{"type":"turn.failed","error":{"message":"You've hit your usage limit"}}"#,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(
            p.finish(Some(1), "").unwrap_err(),
            AssistantError::UsageLimit
        );

        let mut p = CodexParser::default();
        p.line(
            r#"{"type":"turn.failed","error":{"message":"Not logged in"}}"#,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(
            p.finish(Some(1), "").unwrap_err(),
            AssistantError::NotSignedIn
        );
        // No turn event, non-zero exit: stderr explains it.
        let mut p = CodexParser::default();
        assert_eq!(
            p.finish(Some(1), "boom").unwrap_err(),
            AssistantError::Failed {
                message: "boom".into()
            }
        );
        // Clean exit without an answer.
        let mut p = CodexParser::default();
        assert!(matches!(
            p.finish(Some(0), "").unwrap_err(),
            AssistantError::Malformed { .. }
        ));
    }

    #[test]
    fn gemini_parser_streams_assistant_deltas_and_reads_result() {
        let mut p = GeminiParser::default();
        let mut events = Vec::new();
        for line in [
            r#"{"type":"init","session_id":"s","model":"m"}"#,
            r#"{"type":"message","role":"user","content":"PROMPT"}"#,
            r#"{"type":"message","role":"assistant","content":"Feedrico can talk","delta":true}"#,
            r#"{"type":"message","role":"assistant","content":" to you.","delta":true}"#,
            r#"{"type":"tool_use","tool_name":"x"}"#,
            r#"{"type":"error","severity":"warning","message":"w"}"#,
            r#"{"type":"result","status":"success","stats":{}}"#,
        ] {
            p.line(line, &mut |e| events.push(e)).unwrap();
        }
        let (text, structured) = p.finish(Some(0), "").unwrap();
        assert_eq!(text, "Feedrico can talk to you.");
        assert_eq!(structured, None);
        assert_eq!(
            events,
            vec![
                Event::Text {
                    delta: "Feedrico can talk".into()
                },
                Event::Text {
                    delta: " to you.".into()
                },
            ]
        );

        let mut p = GeminiParser::default();
        p.line(
            r#"{"type":"result","status":"error","error":{"type":"auth","message":"API key not valid. Please pass a valid API key."}}"#,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(
            p.finish(Some(1), "").unwrap_err(),
            AssistantError::NotSignedIn
        );
        let mut p = GeminiParser::default();
        assert!(matches!(
            p.finish(Some(0), "").unwrap_err(),
            AssistantError::Malformed { .. }
        ));
    }

    #[test]
    fn gemini_command_line_is_exact() {
        let args = strings(Gemini.args(&codex_task(), Path::new("/t")));
        assert_eq!(
            args,
            [
                "--output-format",
                "stream-json",
                "--approval-mode",
                "plan",
                "--extensions",
                "none",
                "--model",
                "gpt-5",
                "-p",
                "Follow the instructions given on stdin.",
            ]
        );
        for banned in [
            "--yolo",
            "-y",
            "yolo",
            "auto_edit",
            "--skip-trust",
            "PROMPT",
            "SYS",
        ] {
            assert!(!args.iter().any(|a| a == banned), "{banned}");
        }
        assert_eq!(
            Gemini.env(Path::new("/t")),
            vec![(
                OsString::from("GEMINI_SYSTEM_MD"),
                OsString::from("/t/system-prompt.txt")
            )]
        );
    }

    #[test]
    fn expired_sign_in_text_maps_to_not_signed_in() {
        use AssistantError::*;
        for text in [
            "Your access token could not be refreshed because your refresh token has expired. Please log out and sign in again.",
            "Please set an Auth method in your settings",
            "Failed to login. Message: Request contains an invalid argument",
        ] {
            assert_eq!(map_error_text(text), Some(NotSignedIn), "{text}");
        }
    }

    #[test]
    fn error_text_mapping() {
        use AssistantError::*;
        assert_eq!(map_error_text("Invalid API key"), Some(NotSignedIn));
        assert_eq!(map_error_text("Please run /login"), Some(NotSignedIn));
        assert_eq!(map_error_text("Usage limit reached"), Some(UsageLimit));
        assert_eq!(map_error_text("getaddrinfo ENOTFOUND"), Some(Offline));
        assert_eq!(map_error_text("something else"), None);
    }
}
