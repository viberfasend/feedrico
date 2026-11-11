//! Machine-local state that never travels with a Library (spec: "What stays on the machine"):
//! the recent-Libraries list and Portable Mode detection.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use feedrico_core::library::DIR_NAME;
use tauri::{AppHandle, Manager};

const RECENT_FILE: &str = "recent-libraries.json";
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
}
