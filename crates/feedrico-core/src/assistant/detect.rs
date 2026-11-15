//! Find the Assistants installed on this machine (#112).
//!
//! Contract: `detect` looks on `PATH`, then in the usual install directories (a desktop launch
//! gets a minimal `PATH`), with a manual path taking precedence. It runs each CLI's own
//! `--version` and sign-in status command with a short timeout, and never reads credential
//! files or keyrings. It's called lazily, never at startup.

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::{AssistantKind, FoundAssistant, SignIn};

/// How long one probe (`--version`, `auth status`) may run before it is killed.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Inputs for detection, injectable so tests can use fake CLIs and an empty `PATH`.
#[derive(Debug, Clone, Default)]
pub struct DetectOptions {
    /// A path the user set by hand in Settings. It wins over everything else.
    pub manual_path: Option<PathBuf>,
    /// The `PATH` to search. `None` means the process environment.
    pub path_env: Option<std::ffi::OsString>,
    /// Extra directories to search after `PATH`. `None` means the built-in list for this OS.
    pub fallback_dirs: Option<Vec<PathBuf>>,
    /// Don't look for an Ollama server. Tests set this so a real server on the machine can't
    /// change their results.
    pub skip_ollama: bool,
}

/// Every Assistant found, in [`super::AssistantKind::ALL`] order. A binary that is found but
/// doesn't answer its probes is still reported, with no version and [`SignIn::Unknown`].
pub fn detect(options: &DetectOptions) -> Vec<FoundAssistant> {
    let path_env = options
        .path_env
        .clone()
        .or_else(|| std::env::var_os("PATH"))
        .unwrap_or_default();
    let fallback = options
        .fallback_dirs
        .clone()
        .unwrap_or_else(default_fallback_dirs);
    AssistantKind::ALL
        .into_iter()
        .filter_map(|kind| {
            if kind == AssistantKind::Ollama {
                return detect_ollama(options);
            }
            let path = locate(kind, options.manual_path.as_deref(), &path_env, &fallback)?;
            Some(probe(kind, path))
        })
        .collect()
}

/// Ollama has no binary to find: ask the server on this computer, at the manual URL if one was
/// given, else at the default address. A server that doesn't answer is simply not listed.
fn detect_ollama(options: &DetectOptions) -> Option<FoundAssistant> {
    if options.skip_ollama {
        return None;
    }
    let manual = options
        .manual_path
        .as_ref()
        .and_then(|p| p.to_str())
        .filter(|p| p.starts_with("http"));
    let base = super::ollama::normalized(manual.unwrap_or(super::ollama::DEFAULT_BASE_URL))?;
    let probe = super::ollama::probe(&base)?;
    Some(FoundAssistant {
        kind: AssistantKind::Ollama,
        path: PathBuf::from(base),
        version: probe.version,
        sign_in: SignIn::NotApplicable,
        models: probe.models,
    })
}

/// A manual path applies to the Assistant its file name starts with (`claude*`, `vibe*`, case-
/// insensitive, without `.exe`/`.cmd`), and any other name is ignored and never run. It wins over
/// `PATH`, which wins over the fallback directories.
fn locate(
    kind: AssistantKind,
    manual: Option<&Path>,
    path_env: &OsString,
    fallback: &[PathBuf],
) -> Option<PathBuf> {
    if let Some(manual) = manual {
        let name = manual
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let name = name
            .strip_suffix(".exe")
            .or_else(|| name.strip_suffix(".cmd"))
            .unwrap_or(&name);
        if name.starts_with(kind.binary_name()) && is_executable(manual) {
            return Some(manual.to_path_buf());
        }
    }
    std::env::split_paths(path_env)
        .chain(fallback.iter().cloned())
        .filter(|dir| !dir.as_os_str().is_empty())
        .find_map(|dir| find_in(&dir, kind.binary_name()))
}

fn find_in(dir: &Path, name: &str) -> Option<PathBuf> {
    let candidates: &[&str] = if cfg!(windows) {
        &["exe", "cmd", ""]
    } else {
        &[""]
    };
    candidates.iter().find_map(|ext| {
        let file = if ext.is_empty() {
            name.to_string()
        } else {
            format!("{name}.{ext}")
        };
        let path = dir.join(file);
        is_executable(&path).then_some(path)
    })
}

fn is_executable(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// The usual install directories, for an app launched without the user's shell `PATH`.
fn default_fallback_dirs() -> Vec<PathBuf> {
    let var = |name: &str| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let mut dirs = Vec::new();
    if cfg!(windows) {
        if let Some(appdata) = var("APPDATA") {
            dirs.push(appdata.join("npm"));
        }
        if let Some(local) = var("LOCALAPPDATA") {
            dirs.push(local.join("Programs"));
        }
        return dirs;
    }
    let home = var("HOME");
    if let Some(home) = &home {
        dirs.push(home.join(".local/bin"));
        dirs.push(home.join(".claude/local"));
    }
    dirs.push(PathBuf::from("/opt/homebrew/bin"));
    dirs.push(PathBuf::from("/usr/local/bin"));
    if let Some(home) = &home {
        dirs.push(home.join(".npm-global/bin"));
    }
    dirs
}

fn probe(kind: AssistantKind, path: PathBuf) -> FoundAssistant {
    let version = run_probe(&path, &["--version"], false).and_then(|out| parse_version(&out));
    let sign_in = match kind {
        // No answer to `--version` means a broken or hung binary: don't wait on a second probe.
        _ if version.is_none() => SignIn::Unknown,
        AssistantKind::ClaudeCode => claude_sign_in(&path),
        // Vibe (checked with `vibe --help` and `vibe config --help`) has no status command.
        // Its only sign-in path is `--setup`, which is interactive and must never run on its
        // own, and the key lives in a credential file we must not read. So: Unknown.
        AssistantKind::MistralVibe => SignIn::Unknown,
        AssistantKind::Codex => codex_sign_in(&path),
        // Gemini CLI has no status command. Its sign-in lives in credential files (and the
        // environment) we must not read. So: Unknown.
        AssistantKind::Gemini => SignIn::Unknown,
        AssistantKind::Ollama => SignIn::NotApplicable,
    };
    FoundAssistant {
        kind,
        path,
        version,
        sign_in,
        models: Vec::new(),
    }
}

/// The first word that starts with a digit ("2.1.0 (Claude Code)", "vibe 1.4.2"), else the
/// whole first line.
fn parse_version(out: &str) -> Option<String> {
    let line = out.lines().map(str::trim).find(|l| !l.is_empty())?;
    let word = line
        .split_whitespace()
        .find(|w| w.starts_with(|c: char| c.is_ascii_digit()));
    Some(word.unwrap_or(line).to_string())
}

/// `claude auth status --json`: `loggedIn`, and `authMethod` "claude.ai" for a plan.
fn claude_sign_in(path: &Path) -> SignIn {
    let Some(out) = run_probe(path, &["auth", "status", "--json"], false) else {
        return SignIn::Unknown;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(out.trim()) else {
        return SignIn::Unknown;
    };
    match json.get("loggedIn").and_then(|v| v.as_bool()) {
        Some(false) => SignIn::NotSignedIn,
        Some(true) => match json.get("authMethod").and_then(|v| v.as_str()) {
            Some("claude.ai") => SignIn::Plan,
            _ => SignIn::ApiKey,
        },
        None => SignIn::Unknown,
    }
}

/// `codex login status` reads no credentials itself and answers on stderr: "Logged in using
/// ChatGPT", "Logged in using an API key", or "Not logged in". A signed-in answer can still be
/// a session that has expired; the run then reports it as not signed in.
fn codex_sign_in(path: &Path) -> SignIn {
    let Some(out) = run_probe(path, &["login", "status"], true) else {
        return SignIn::Unknown;
    };
    let out = out.to_lowercase();
    if out.contains("not logged in") {
        SignIn::NotSignedIn
    } else if out.contains("chatgpt") {
        SignIn::Plan
    } else if out.contains("api key") {
        SignIn::ApiKey
    } else {
        SignIn::Unknown
    }
}

/// Run `path args` with no stdin, and return stdout (plus stderr when `with_stderr`, for CLIs
/// that answer there) if it exits successfully within [`PROBE_TIMEOUT`]. On expiry the process is killed. `claude auth status` exits non-zero when
/// signed out but still prints JSON, so non-zero exits keep their output too.
fn run_probe(path: &Path, args: &[&str], with_stderr: bool) -> Option<String> {
    let mut command = Command::new(path);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(if with_stderr {
            Stdio::piped()
        } else {
            Stdio::null()
        });
    let mut child = spawn_with_retry(&mut command)?;
    let mut stdout = child.stdout.take()?;
    // Read on threads, and never join them: a killed script's grandchild could keep the pipe
    // open, and detection must not wait for it.
    let (tx, rx) = mpsc::channel();
    if let Some(mut stderr) = child.stderr.take() {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut buf = String::new();
            let _ = stderr.read_to_string(&mut buf);
            let _ = tx.send(buf);
        });
    }
    std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = stdout.read_to_string(&mut buf);
        let _ = tx.send(buf);
    });
    let expected = if with_stderr { 2 } else { 1 };
    let deadline = Instant::now() + PROBE_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut out = String::new();
                for _ in 0..expected {
                    out.push_str(&rx.recv_timeout(Duration::from_secs(1)).ok()?);
                }
                // A non-zero exit with no output (a broken binary) tells us nothing.
                return (status.success() || !out.trim().is_empty()).then_some(out);
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// Spawn, retrying briefly on "text file busy" (ETXTBSY), like the runner does. It happens when
/// the executable was written a moment ago (an update, or a test's fake CLI) while another thread
/// forked; without the retry the Assistant would look missing or its sign-in unknown.
fn spawn_with_retry(command: &mut Command) -> Option<std::process::Child> {
    let mut attempts = 0;
    loop {
        match command.spawn() {
            Ok(child) => return Some(child),
            Err(e) if e.raw_os_error() == Some(26) && attempts < 10 => {
                attempts += 1;
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return None,
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::assistant::{AssistantKind, SignIn};
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    /// Write an executable sh script named `name` into `dir`.
    fn fake(dir: &Path, name: &str, body: &str) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn fake_claude(dir: &Path, status: &str) -> PathBuf {
        fake(
            dir,
            "claude",
            &format!(
                "case \"$1\" in\n--version) echo '2.1.0 (Claude Code)';;\nauth) echo '{status}';;\nesac"
            ),
        )
    }

    fn fake_vibe(dir: &Path) -> PathBuf {
        fake(
            dir,
            "vibe",
            "case \"$1\" in\n--version) echo 'vibe 1.4.2';;\n*) echo 'vibe must not be asked anything else' >&2; exit 9;;\nesac",
        )
    }

    /// `codex login status` answers on stderr ("Logged in using ChatGPT", "... API key",
    /// "Not logged in"), verified against codex-cli 0.153.4 on 2026-11-14.
    fn fake_codex(dir: &Path, status: &str) -> PathBuf {
        fake(
            dir,
            "codex",
            &format!(
                "case \"$1 $2\" in\n--version*) echo 'codex-cli 0.153.4';;\n\"login status\") echo '{status}' >&2; exit 0;;\nesac"
            ),
        )
    }

    /// Gemini CLI has no status command, so detection must only ever run `--version`.
    fn fake_gemini(dir: &Path) -> PathBuf {
        fake(
            dir,
            "gemini",
            "case \"$1\" in\n--version) echo '0.20.0';;\n*) echo 'gemini must not be asked anything else' >&2; exit 9;;\nesac",
        )
    }

    fn opts(path: &Path) -> DetectOptions {
        DetectOptions {
            manual_path: None,
            path_env: Some(path.as_os_str().to_owned()),
            fallback_dirs: Some(vec![]),
            skip_ollama: true,
        }
    }

    fn find(found: &[FoundAssistant], kind: AssistantKind) -> &FoundAssistant {
        found.iter().find(|f| f.kind == kind).expect("found")
    }

    #[test]
    fn finds_both_on_path_with_version_and_sign_in() {
        let dir = tempfile::tempdir().unwrap();
        let claude = fake_claude(dir.path(), r#"{"loggedIn":true,"authMethod":"claude.ai"}"#);
        let vibe = fake_vibe(dir.path());
        let found = detect(&opts(dir.path()));
        assert_eq!(found.len(), 2);
        let c = find(&found, AssistantKind::ClaudeCode);
        assert_eq!(c.path, claude);
        assert_eq!(c.version.as_deref(), Some("2.1.0"));
        assert_eq!(c.sign_in, SignIn::Plan);
        let v = find(&found, AssistantKind::MistralVibe);
        assert_eq!(v.path, vibe);
        assert_eq!(v.version.as_deref(), Some("1.4.2"));
        // Vibe has no status command that avoids credential files.
        assert_eq!(v.sign_in, SignIn::Unknown);
    }

    #[test]
    fn finds_nothing_when_nothing_is_installed() {
        let empty = tempfile::tempdir().unwrap();
        assert!(detect(&opts(empty.path())).is_empty());
    }

    #[test]
    fn finds_in_fallback_dir_with_empty_path() {
        let empty = tempfile::tempdir().unwrap();
        let fb = tempfile::tempdir().unwrap();
        fake_claude(fb.path(), r#"{"loggedIn":true,"authMethod":"claude.ai"}"#);
        let o = DetectOptions {
            fallback_dirs: Some(vec![fb.path().to_path_buf()]),
            ..opts(empty.path())
        };
        let found = detect(&o);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, fb.path().join("claude"));
    }

    #[test]
    fn path_wins_over_fallback_dirs() {
        let on_path = tempfile::tempdir().unwrap();
        let fb = tempfile::tempdir().unwrap();
        fake_claude(on_path.path(), r#"{"loggedIn":false}"#);
        fake_claude(fb.path(), r#"{"loggedIn":false}"#);
        let o = DetectOptions {
            fallback_dirs: Some(vec![fb.path().to_path_buf()]),
            ..opts(on_path.path())
        };
        assert_eq!(detect(&o)[0].path, on_path.path().join("claude"));
    }

    #[test]
    fn manual_path_wins_over_path() {
        let on_path = tempfile::tempdir().unwrap();
        let manual_dir = tempfile::tempdir().unwrap();
        fake_claude(on_path.path(), r#"{"loggedIn":false}"#);
        let manual = fake_claude(
            manual_dir.path(),
            r#"{"loggedIn":true,"authMethod":"claude.ai"}"#,
        );
        let o = DetectOptions {
            manual_path: Some(manual.clone()),
            ..opts(on_path.path())
        };
        let found = detect(&o);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, manual);
        assert_eq!(found[0].sign_in, SignIn::Plan);
    }

    #[test]
    fn api_key_sign_in() {
        for method in ["console", "apiKey", "api_key"] {
            let dir = tempfile::tempdir().unwrap();
            fake_claude(
                dir.path(),
                &format!(r#"{{"loggedIn":true,"authMethod":"{method}"}}"#),
            );
            assert_eq!(
                detect(&opts(dir.path()))[0].sign_in,
                SignIn::ApiKey,
                "{method}"
            );
        }
    }

    #[test]
    fn not_signed_in() {
        let dir = tempfile::tempdir().unwrap();
        fake_claude(dir.path(), r#"{"loggedIn":false}"#);
        assert_eq!(detect(&opts(dir.path()))[0].sign_in, SignIn::NotSignedIn);
    }

    #[test]
    fn unreadable_status_is_unknown() {
        let dir = tempfile::tempdir().unwrap();
        fake_claude(dir.path(), "not json");
        let found = detect(&opts(dir.path()));
        assert_eq!(found[0].sign_in, SignIn::Unknown);
        assert_eq!(found[0].version.as_deref(), Some("2.1.0"));
    }

    #[test]
    fn broken_binary_is_reported_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        fake(dir.path(), "claude", "exit 127");
        let found = detect(&opts(dir.path()));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].version, None);
        assert_eq!(found[0].sign_in, SignIn::Unknown);
    }

    #[test]
    fn hanging_binary_times_out() {
        let dir = tempfile::tempdir().unwrap();
        fake(dir.path(), "claude", "exec sleep 30");
        let start = std::time::Instant::now();
        let found = detect(&opts(dir.path()));
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "{:?}",
            start.elapsed()
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].version, None);
        assert_eq!(found[0].sign_in, SignIn::Unknown);
    }

    #[test]
    fn non_executable_file_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("claude"), "#!/bin/sh\n").unwrap();
        assert!(detect(&opts(dir.path())).is_empty());
    }

    #[test]
    fn manual_path_kind_is_decided_by_file_name_prefix() {
        let empty = tempfile::tempdir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        for (name, kind) in [
            ("claude-wrapper", AssistantKind::ClaudeCode),
            ("Claude.exe", AssistantKind::ClaudeCode),
            ("vibe-3.1", AssistantKind::MistralVibe),
        ] {
            let manual = fake_vibe(dir.path());
            let named = dir.path().join(name);
            std::fs::rename(&manual, &named).unwrap();
            let o = DetectOptions {
                manual_path: Some(named.clone()),
                ..opts(empty.path())
            };
            let found = detect(&o);
            assert_eq!(found.len(), 1, "{name}");
            assert_eq!(found[0].kind, kind, "{name}");
            assert_eq!(found[0].path, named, "{name}");
        }
    }

    #[test]
    fn finds_codex_with_login_status_from_stderr() {
        let dir = tempfile::tempdir().unwrap();
        fake_codex(dir.path(), "Logged in using ChatGPT");
        let found = detect(&opts(dir.path()));
        let c = find(&found, AssistantKind::Codex);
        assert_eq!(c.version.as_deref(), Some("0.153.4"));
        assert_eq!(c.sign_in, SignIn::Plan);
    }

    #[test]
    fn codex_login_status_variants() {
        for (status, expected) in [
            ("Logged in using an API key", SignIn::ApiKey),
            ("Not logged in", SignIn::NotSignedIn),
            ("something unreadable", SignIn::Unknown),
        ] {
            let dir = tempfile::tempdir().unwrap();
            fake_codex(dir.path(), status);
            assert_eq!(
                find(&detect(&opts(dir.path())), AssistantKind::Codex).sign_in,
                expected,
                "{status}"
            );
        }
    }

    #[test]
    fn finds_gemini_with_unknown_sign_in_and_only_version_probed() {
        let dir = tempfile::tempdir().unwrap();
        fake_gemini(dir.path());
        let found = detect(&opts(dir.path()));
        let g = find(&found, AssistantKind::Gemini);
        assert_eq!(g.version.as_deref(), Some("0.20.0"));
        assert_eq!(g.sign_in, SignIn::Unknown);
    }

    #[test]
    fn ollama_is_skipped_when_tests_ask_for_it() {
        let dir = tempfile::tempdir().unwrap();
        fake_claude(dir.path(), r#"{"loggedIn":true,"authMethod":"claude.ai"}"#);
        // Even a manual URL that would be a valid loopback base is ignored with skip_ollama.
        let o = DetectOptions {
            manual_path: Some(PathBuf::from("http://127.0.0.1:11434")),
            ..opts(dir.path())
        };
        let found = detect(&o);
        assert!(found.iter().all(|f| f.kind != AssistantKind::Ollama));
    }

    #[test]
    fn manual_path_with_another_name_is_ignored_and_never_run() {
        let empty = tempfile::tempdir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("ran");
        let evil = fake(dir.path(), "rm-rf", &format!("touch {}", marker.display()));
        let o = DetectOptions {
            manual_path: Some(evil),
            ..opts(empty.path())
        };
        assert!(detect(&o).is_empty());
        assert!(!marker.exists());
    }
}
