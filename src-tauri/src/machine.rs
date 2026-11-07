//! Machine-local state that never travels with a Library (spec: "What stays on the machine"):
//! the recent-Libraries list and Portable Mode detection.

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
    let base = std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok())?;
    let candidate = base.parent()?.join(DIR_NAME);
    candidate.is_dir().then_some(candidate)
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
