//! The Tauri shell: a thin layer that exposes `feedrico-core` to the UI as commands.
//! No domain logic lives here.

use std::path::PathBuf;
use std::sync::Mutex;

use feedrico_core::{Library, OpenOptions, Status};
use serde::Serialize;
use tauri::State;

/// The one open Library (ADR 0001: exactly one at a time).
#[derive(Default)]
struct OpenLibrary(Mutex<Option<Library>>);

/// Errors cross the IPC boundary as a kind the UI can branch on plus a readable message.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandError {
    kind: &'static str,
    message: String,
}

impl From<feedrico_core::Error> for CommandError {
    fn from(error: feedrico_core::Error) -> Self {
        use feedrico_core::Error::*;
        let kind = match &error {
            NotALibrary(_) => "notALibrary",
            TargetNotEmpty(_) => "targetNotEmpty",
            TooNew { .. } => "tooNew",
            NeedsUpgrade { .. } => "needsUpgrade",
            Locked(_) => "locked",
            Manifest(_) | Database(_) | Io(_) => "internal",
        };
        Self {
            kind,
            message: error.to_string(),
        }
    }
}

type CommandResult<T> = Result<T, CommandError>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LibraryInfo {
    path: PathBuf,
    format_version: u32,
}

fn info(library: &Library) -> CommandResult<LibraryInfo> {
    Ok(LibraryInfo {
        path: library.dir().to_path_buf(),
        format_version: library.format_version()?,
    })
}

#[tauri::command]
fn library_inspect(path: PathBuf) -> CommandResult<Status> {
    Ok(feedrico_core::library::inspect(&path)?)
}

#[tauri::command]
fn library_create(path: PathBuf, state: State<'_, OpenLibrary>) -> CommandResult<LibraryInfo> {
    let mut slot = state.0.lock().expect("library mutex poisoned");
    if let Some(previous) = slot.take() {
        previous.close()?;
    }
    let library = Library::create(&path)?;
    let result = info(&library);
    *slot = Some(library);
    result
}

#[tauri::command]
fn library_open(
    path: PathBuf,
    allow_upgrade: bool,
    take_over_lock: bool,
    state: State<'_, OpenLibrary>,
) -> CommandResult<LibraryInfo> {
    let mut slot = state.0.lock().expect("library mutex poisoned");
    if let Some(previous) = slot.take() {
        previous.close()?;
    }
    let library = Library::open(
        &path,
        OpenOptions {
            allow_upgrade,
            take_over_lock,
        },
    )?;
    let result = info(&library);
    *slot = Some(library);
    result
}

#[tauri::command]
fn library_close(state: State<'_, OpenLibrary>) -> CommandResult<()> {
    if let Some(library) = state.0.lock().expect("library mutex poisoned").take() {
        library.close()?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(OpenLibrary::default())
        .invoke_handler(tauri::generate_handler![
            library_inspect,
            library_create,
            library_open,
            library_close
        ])
        .build(tauri::generate_context!())
        .expect("error while building Feedrico")
        .run(|app, event| {
            // Release the lock and checkpoint the database on every way out.
            if let tauri::RunEvent::Exit = event {
                use tauri::Manager;
                if let Some(library) = app
                    .state::<OpenLibrary>()
                    .0
                    .lock()
                    .ok()
                    .and_then(|mut s| s.take())
                {
                    let _ = library.close();
                }
            }
        });
}
