//! Machine-local state that never travels with a Library (spec: "What stays on the machine"):
//! the recent-Libraries list and Portable Mode detection.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use feedrico_core::assistant::AssistantSetup;
use feedrico_core::library::DIR_NAME;
use tauri::{AppHandle, Manager};

const RECENT_FILE: &str = "recent-libraries.json";
const ASSISTANT_FILE: &str = "assistant.json";
const MAX_RECENT: usize = 10;

/// Portable Mode: a Library folder next to the executable (or next to the AppImage file, since
/// an AppImage runs from a temporary mount). macOS is excluded: Gatekeeper quarantines apps run
/// from removable media.
pub fn portable_library() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        return None;
    }
    let candidate = portable_dir(std::env::var_os("APPIMAGE"), std::env::current_exe().ok())?;
    candidate.is_dir().then_some(candidate)
}

fn portable_dir(appimage: Option<OsString>, exe: Option<PathBuf>) -> Option<PathBuf> {
    let base = appimage.map(PathBuf::from).or(exe)?;
    Some(base.parent()?.join(DIR_NAME))
}

/// An empty Portable Mode folder is where the user wants a new Library: the first start creates
/// it there. Anything with files in it is left alone.
pub fn is_empty_dir(dir: &Path) -> bool {
    fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_none())
}

pub fn default_library_path(app: &AppHandle) -> std::io::Result<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(dir.join(DIR_NAME))
}

fn recent_file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join(RECENT_FILE))
}

pub fn recent(app: &AppHandle) -> Vec<PathBuf> {
    recent_file(app)
        .and_then(|f| fs::read(f).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save(app: &AppHandle, list: &[PathBuf]) {
    let Some(file) = recent_file(app) else { return };
    if let Some(parent) = file.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_vec_pretty(list) {
        let _ = fs::write(file, json);
    }
}

/// Moves `path` to the front of the recent list.
pub fn remember(app: &AppHandle, path: &Path) {
    let mut list = recent(app);
    list.retain(|p| p != path);
    list.insert(0, path.to_path_buf());
    list.truncate(MAX_RECENT);
    save(app, &list);
}

pub fn forget(app: &AppHandle, path: &Path) {
    let mut list = recent(app);
    list.retain(|p| p != path);
    save(app, &list);
}

fn assistant_file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join(ASSISTANT_FILE))
}

/// The Assistant set up on this machine; `None` means off (also for an unreadable file).
pub fn assistant_setup(app: &AppHandle) -> Option<AssistantSetup> {
    assistant_file(app).and_then(|f| read_assistant(&f))
}

/// Saves the setup, or turns Assistants off with `None`. Machine-local: never in a Library.
pub fn set_assistant_setup(app: &AppHandle, setup: Option<&AssistantSetup>) -> std::io::Result<()> {
    let file = assistant_file(app).ok_or_else(|| std::io::Error::other("no app data dir"))?;
    write_assistant(&file, setup)
}

fn read_assistant(file: &Path) -> Option<AssistantSetup> {
    serde_json::from_slice(&fs::read(file).ok()?).ok()
}

fn write_assistant(file: &Path, setup: Option<&AssistantSetup>) -> std::io::Result<()> {
    match setup {
        Some(setup) => {
            if let Some(parent) = file.parent() {
                fs::create_dir_all(parent)?;
            }
            let json = serde_json::to_vec_pretty(setup).map_err(std::io::Error::other)?;
            fs::write(file, json)
        }
        None => match fs::remove_file(file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_mode_looks_next_to_the_appimage_file_first() {
        let dir = portable_dir(
            Some("/media/stick/Feedrico.AppImage".into()),
            Some("/tmp/.mount_Feedri/usr/bin/feedrico".into()),
        );
        assert_eq!(dir, Some(PathBuf::from("/media/stick").join(DIR_NAME)));
    }

    #[test]
    fn portable_mode_looks_next_to_the_executable_otherwise() {
        let dir = portable_dir(None, Some("E:/Feedrico/feedrico.exe".into()));
        assert_eq!(dir, Some(PathBuf::from("E:/Feedrico").join(DIR_NAME)));
        assert_eq!(portable_dir(None, None), None);
    }

    // Portable Mode had no way to start: an empty `Feedrico Library` folder next to the app
    // read as "not a Library" and was skipped. An empty folder now gets a new Library.
    #[test]
    fn only_an_empty_folder_starts_a_new_portable_library() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join(DIR_NAME);
        assert!(!is_empty_dir(&dir), "missing");
        fs::create_dir(&dir).unwrap();
        assert!(is_empty_dir(&dir), "empty");
        fs::write(dir.join("notes.txt"), "mine").unwrap();
        assert!(!is_empty_dir(&dir), "has files");
    }

    fn setup() -> AssistantSetup {
        AssistantSetup {
            kind: feedrico_core::assistant::AssistantKind::ClaudeCode,
            path: "/usr/bin/claude".into(),
            model: Some("haiku".into()),
            disclosure_accepted: true,
            target_language: Some("de".into()),
            interests: "Rust, local-first software".into(),
        }
    }

    // The setup lives in the app data dir only: a Library folder next to it stays byte-identical.
    #[test]
    fn the_assistant_setup_round_trips_without_touching_a_library() {
        let root = tempfile::tempdir().unwrap();
        let library = root.path().join(DIR_NAME);
        fs::create_dir(&library).unwrap();
        fs::write(library.join("library.json"), "{\"settings\":1}").unwrap();
        let snapshot = |d: &Path| {
            let mut v: Vec<_> = fs::read_dir(d)
                .unwrap()
                .map(|e| {
                    let p = e.unwrap().path();
                    (p.clone(), fs::read(&p).unwrap())
                })
                .collect();
            v.sort();
            v
        };
        let before = snapshot(&library);

        let file = root.path().join("appdata").join(ASSISTANT_FILE);
        assert_eq!(read_assistant(&file), None, "off by default");
        write_assistant(&file, Some(&setup())).unwrap();
        assert_eq!(read_assistant(&file), Some(setup()));
        let json = fs::read_to_string(&file).unwrap();
        assert!(json.contains("\"disclosureAccepted\": true"), "{json}");
        assert!(json.contains("\"claudeCode\""), "{json}");
        assert!(json.contains("\"targetLanguage\": \"de\""), "{json}");
        assert!(
            json.contains("\"interests\": \"Rust, local-first software\""),
            "{json}"
        );
        write_assistant(&file, None).unwrap();
        assert_eq!(read_assistant(&file), None, "turning off deletes the file");
        assert!(!file.exists());
        write_assistant(&file, None).unwrap();

        assert_eq!(snapshot(&library), before, "the Library is untouched");
    }

    // The interests field is serde-defaulted, so a setup file from before #121 still loads.
    #[test]
    fn an_older_setup_file_without_interests_still_loads() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join(ASSISTANT_FILE);
        std::fs::write(
            &file,
            r#"{"kind":"claudeCode","path":"/usr/bin/claude","model":"haiku","disclosureAccepted":true}"#,
        )
        .unwrap();
        let setup = read_assistant(&file).unwrap();
        assert_eq!(setup.interests, "");
        assert!(setup.disclosure_accepted);
    }

    #[test]
    fn an_unreadable_assistant_file_reads_as_off() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join(ASSISTANT_FILE);
        fs::write(&file, "not json").unwrap();
        assert_eq!(read_assistant(&file), None);
    }

    // The target language was added later: a setup file from before it still loads, with None.
    #[test]
    fn an_older_setup_file_without_the_target_language_still_loads() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join(ASSISTANT_FILE);
        fs::write(
            &file,
            r#"{"kind":"claudeCode","path":"/usr/bin/claude","model":"haiku","disclosureAccepted":true}"#,
        )
        .unwrap();
        let setup = read_assistant(&file).unwrap();
        assert_eq!(setup.target_language, None);
        assert!(setup.disclosure_accepted);
    }
}
