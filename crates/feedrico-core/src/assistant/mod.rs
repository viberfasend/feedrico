//! Assistants: the user's own locally installed and signed-in AI command-line tool (Claude Code,
//! Mistral Vibe) that Feedrico may ask to work on Articles when the user says so. Epic #110.
//!
//! This module is the shared contract the Assistant issues build on. The ground rules apply to
//! all of them:
//!
//! - Feedrico never offers a login, and never reads, copies or stores credentials. It only runs
//!   the user's own CLI binary and that CLI's own status commands.
//! - Runs start only from an explicit user action, never at startup or on refresh.
//! - The CLI runs with every tool disabled, in a fresh empty temporary directory, with the
//!   user's and project's settings and MCP servers ignored.
//! - Article text is untrusted input and is framed as data. Output is untrusted too: it goes
//!   through the sanitizer before the webview shows it.
//! - The Assistant setup is machine-local (it lives in the app data dir, never in the Library).
//!   Results are ephemeral: nothing an Assistant produces is written to the Library.
//!
//! Layout:
//! - [`detect`]: find installed Assistants, their version and sign-in state (#112).
//! - [`article`]: Article input and the reading-aid prompts (#116, #118).
//! - [`run`]: run one [`Task`], stream [`Event`]s, cancel, map errors (#113).
//! - [`ask`]: ask a question about the Article, resending the capped conversation (#119).
//! - [`translate`]: translate the Article, structure and image placeholders kept (#117).
//! - [`search`]: turn a question into a local search query (#122).
//! - [`view_input`]: the newest N Articles of a View as prompt input (#120, #121).
//! - [`briefing`]: a briefing of a View, grouped topics with Article ids (#120).
//! - [`picks`]: rank a View's unread Articles against the user's interests (#121).

pub mod article;
pub mod ask;
pub mod briefing;
pub mod detect;
pub mod folder_suggestions;
pub mod picks;
pub mod prompts;
pub mod run;
pub mod search;
pub mod translate;
pub mod view_input;

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// The Assistant the user set up on this machine. Stored as `assistant.json` in the app data
/// dir (never in the Library). Its absence means Assistants are off.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantSetup {
    pub kind: AssistantKind,
    pub path: PathBuf,
    pub model: Option<String>,
    /// The user has seen the disclosure that Article text goes to the provider.
    pub disclosure_accepted: bool,
    /// The language Translate renders Articles into (a BCP 47 tag or name), machine-local.
    /// `None` means the UI's default, the system language. Never stored in the Library.
    #[serde(default)]
    pub target_language: Option<String>,
    /// Free text: what the user likes to read, used only by the picks run (#121). Machine-local,
    /// so it never travels with a Library. Empty means not set.
    #[serde(default)]
    pub interests: String,
}

/// The Assistants Feedrico knows how to drive. New CLIs (Codex, Gemini, Ollama: #125) add a
/// variant and an adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AssistantKind {
    ClaudeCode,
    MistralVibe,
}

impl AssistantKind {
    pub const ALL: [AssistantKind; 2] = [AssistantKind::ClaudeCode, AssistantKind::MistralVibe];

    /// The executable name without extension.
    pub fn binary_name(self) -> &'static str {
        match self {
            AssistantKind::ClaudeCode => "claude",
            AssistantKind::MistralVibe => "vibe",
        }
    }

    /// The name shown in the UI, for example "Written by Claude Code".
    pub fn display_name(self) -> &'static str {
        match self {
            AssistantKind::ClaudeCode => "Claude Code",
            AssistantKind::MistralVibe => "Mistral Vibe",
        }
    }

    /// The company the CLI sends content to, used in the disclosure.
    pub fn provider(self) -> &'static str {
        match self {
            AssistantKind::ClaudeCode => "Anthropic",
            AssistantKind::MistralVibe => "Mistral",
        }
    }
}

/// How the CLI is signed in, as far as its own status command tells us.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SignIn {
    /// Signed in with a subscription plan: runs draw from the plan's limits.
    Plan,
    /// Signed in with an API key: runs are billed per use.
    ApiKey,
    NotSignedIn,
    /// The CLI gives no cheap way to tell, or the check failed.
    Unknown,
}

/// An Assistant found on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundAssistant {
    pub kind: AssistantKind,
    pub path: PathBuf,
    pub version: Option<String>,
    pub sign_in: SignIn,
}

/// One unit of work for an Assistant. Features (#116 and later) build a `Task` from a prompt
/// template plus Article content. The runner never sees Library types.
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    /// Instructions, including the rule that delimited content is data, never instructions.
    pub system_prompt: String,
    /// The user turn, sent on stdin (never on the command line).
    pub prompt: String,
    /// Ask for validated structured output (Claude Code: `--json-schema`).
    pub json_schema: Option<serde_json::Value>,
    /// A model alias such as "haiku". `None` uses the CLI's default.
    pub model: Option<String>,
    pub timeout: Duration,
}

/// What a running Task reports, in order: any number of `Text` deltas, then `Done` once.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Event {
    Text {
        delta: String,
    },
    Done {
        text: String,
        structured: Option<serde_json::Value>,
    },
}

/// Why a Task did not finish. Each variant gets a plain message with a next step in the UI.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AssistantError {
    #[error("the Assistant's command-line tool isn't installed or can't be started")]
    NotInstalled,
    #[error("the Assistant isn't signed in")]
    NotSignedIn,
    #[error("the Assistant's usage limit is reached")]
    UsageLimit,
    #[error("the Assistant can't reach its provider")]
    Offline,
    #[error("the Assistant took too long")]
    TimedOut,
    #[error("stopped")]
    Cancelled,
    #[error("the Assistant's answer couldn't be read: {message}")]
    Malformed { message: String },
    #[error("the Assistant failed: {message}")]
    Failed { message: String },
}
