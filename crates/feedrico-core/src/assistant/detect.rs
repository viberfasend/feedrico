//! Find the Assistants installed on this machine (#112).
//!
//! Contract: `detect` looks on `PATH`, then in the usual install directories (a desktop launch
//! gets a minimal `PATH`), with a manual path taking precedence. It runs each CLI's own
//! `--version` and sign-in status command with a short timeout, and never reads credential
//! files or keyrings. It's called lazily, never at startup.

use std::path::PathBuf;

use super::FoundAssistant;

/// Inputs for detection, injectable so tests can use fake CLIs and an empty `PATH`.
#[derive(Debug, Clone, Default)]
pub struct DetectOptions {
    /// A path the user set by hand in Settings. It wins over everything else.
    pub manual_path: Option<PathBuf>,
    /// The `PATH` to search. `None` means the process environment.
    pub path_env: Option<std::ffi::OsString>,
    /// Extra directories to search after `PATH`. `None` means the built-in list for this OS.
    pub fallback_dirs: Option<Vec<PathBuf>>,
}

/// Every Assistant found, in [`super::AssistantKind::ALL`] order.
pub fn detect(_options: &DetectOptions) -> Vec<FoundAssistant> {
    Vec::new() // #112 implements this.
}
