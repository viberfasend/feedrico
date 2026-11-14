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

    let mut command = Command::new(&assistant.path);
    command
        .args(adapter.args(task, &dir.path().join(SYSTEM_PROMPT_FILE)))
        .current_dir(dir.path())
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

    let mut parser = adapter.parser();
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
    /// The exact arguments. `system_prompt_file` is where `system_prompt_file()` text is written.
    fn args(&self, task: &Task, system_prompt_file: &Path) -> Vec<OsString>;
    /// Text to put in a file in the temporary directory, for CLIs that take the system prompt
    /// from a file.
    fn system_prompt_file(&self, _task: &Task) -> Option<String> {
        None
    }
    fn parser(&self) -> Box<dyn Parser>;
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
    fn args(&self, task: &Task, _system_prompt_file: &Path) -> Vec<OsString> {
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

    fn parser(&self) -> Box<dyn Parser> {
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
    fn args(&self, task: &Task, system_prompt_file: &Path) -> Vec<OsString> {
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

    fn parser(&self) -> Box<dyn Parser> {
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
        "/login",
        "not logged in",
        "not signed in",
        "unauthorized",
        "authentication",
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
        let args = strings(ClaudeCode.args(&task(), Path::new("unused")));
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
        let args = strings(ClaudeCode.args(&t, Path::new("unused")));
        let at = |flag: &str| args.iter().position(|a| a == flag).unwrap();
        assert_eq!(args[at("--model") + 1], "haiku");
        assert_eq!(args[at("--json-schema") + 1], "{\"type\":\"object\"}");
        assert!(!args.iter().any(|a| a == "--bare" || a == "PROMPT"));
    }

    #[test]
    fn vibe_command_line_is_exact() {
        let args = strings(MistralVibe.args(&task(), Path::new("/t/system-prompt.txt")));
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
        let args = strings(MistralVibe.args(&t, Path::new("f")));
        assert_eq!(&args[7..9], ["--model", "m"]);
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
