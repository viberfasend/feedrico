//! The Tauri shell: a thin layer that exposes `feedrico-core` to the UI as commands, runs the
//! background refresh, and serves cached media. No domain logic lives here.

mod machine;
mod perf;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Instant;

use feedrico_core::assistant::{self, AssistantError};
use feedrico_core::feeds::ImportSummary;
use feedrico_core::{AddOutcome, Discovery, FetchResult};
use feedrico_core::{
    ArticleDetail, ArticleRow, Fetcher, Layout, Layouts, Library, ListQuery, OpenOptions, Settings,
    Sidebar, Status, UpdateStatus, opml,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::http::{Response, StatusCode, header};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_opener::OpenerExt;

/// Where cached images are served. Baked into sanitized HTML by the core.
const MEDIA_PREFIX: &str = if cfg!(windows) {
    "http://media.localhost/"
} else {
    "media://localhost/"
};
const FETCH_CONCURRENCY: usize = 8;
const IMAGE_CONCURRENCY: usize = 6;

struct AppState {
    /// The one open Library (ADR 0001: exactly one at a time).
    library: Mutex<Option<Library>>,
    fetcher: Fetcher,
    refreshing: AtomicBool,
    /// Set only when the app runs its timing harness (`perf.rs`).
    perf: Option<perf::Perf>,
    /// Running Assistant tasks by the run id the UI chose, so `assistant_cancel` can stop one.
    assistant_runs: Mutex<HashMap<String, assistant::run::Cancel>>,
}

/// Errors cross the IPC boundary as a kind the UI can branch on plus a readable message.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandError {
    kind: &'static str,
    message: String,
}

impl CommandError {
    fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
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
            Opml(_) => "invalidOpml",
            NotFound => "notFound",
            Invalid(_) => "invalid",
            InvalidSetting { .. } => "invalidSetting",
            Feed(_) | Network(_) | Manifest(_) | Database(_) | Io(_) => "internal",
        };
        Self::new(kind, error.to_string())
    }
}

impl From<std::io::Error> for CommandError {
    fn from(error: std::io::Error) -> Self {
        Self::new("io", error.to_string())
    }
}

type CommandResult<T> = Result<T, CommandError>;

/// Runs `f` against the open Library.
fn with_library<T>(
    state: &AppState,
    f: impl FnOnce(&Library) -> feedrico_core::Result<T>,
) -> CommandResult<T> {
    let guard = state.library.lock().unwrap_or_else(PoisonError::into_inner);
    let library = guard
        .as_ref()
        .ok_or_else(|| CommandError::new("noLibrary", "no Library is open"))?;
    Ok(f(library)?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LibraryInfo {
    path: PathBuf,
    name: String,
    format_version: u32,
    feed_count: i64,
    portable: bool,
}

fn info(library: &Library) -> CommandResult<LibraryInfo> {
    Ok(LibraryInfo {
        path: library.dir().to_path_buf(),
        name: library
            .dir()
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        format_version: library.format_version()?,
        feed_count: library.feed_count()?,
        portable: machine::portable_library().as_deref() == Some(library.dir()),
    })
}

/// Opens or creates a Library, replacing the current one, and remembers it on this machine.
fn install(app: &AppHandle, state: &AppState, library: Library) -> CommandResult<LibraryInfo> {
    let result = info(&library)?;
    let mut slot = state.library.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(previous) = slot.take() {
        previous.close()?;
    }
    if state.perf.is_none() {
        machine::remember(app, library.dir());
    }
    *slot = Some(library);
    Ok(result)
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Startup {
    Opened {
        library: LibraryInfo,
    },
    /// The candidate Library needs the user's decision (lock, upgrade, too new).
    Attention {
        path: PathBuf,
        status: Status,
    },
    Welcome {
        default_path: PathBuf,
    },
}

/// Spec "Finding a Library on startup": Portable Mode, then the most recent Library, then the
/// welcome screen. An empty Portable Mode folder gets a new Library.
#[tauri::command]
fn startup(app: AppHandle, state: State<'_, AppState>) -> CommandResult<Startup> {
    if let Some(library) = state
        .library
        .lock()
        .expect("library mutex poisoned")
        .as_ref()
    {
        return Ok(Startup::Opened {
            library: info(library)?,
        });
    }
    if let Some(perf) = &state.perf {
        let library = Library::open(&perf.library, OpenOptions::default())?;
        return Ok(Startup::Opened {
            library: install(&app, &state, library)?,
        });
    }
    let portable = machine::portable_library();
    if let Some(path) = portable.as_deref().filter(|p| machine::is_empty_dir(p)) {
        let library = Library::create(path)?;
        return Ok(Startup::Opened {
            library: install(&app, &state, library)?,
        });
    }
    let candidates = portable.into_iter().chain(machine::recent(&app));
    for path in candidates {
        match feedrico_core::library::inspect(&path)? {
            Status::Ready => {
                let library = Library::open(&path, OpenOptions::default())?;
                return Ok(Startup::Opened {
                    library: install(&app, &state, library)?,
                });
            }
            Status::NotALibrary => machine::forget(&app, &path),
            status => return Ok(Startup::Attention { path, status }),
        }
    }
    Ok(Startup::Welcome {
        default_path: machine::default_library_path(&app)?,
    })
}

#[tauri::command]
fn recent_libraries(app: AppHandle) -> Vec<PathBuf> {
    machine::recent(&app)
}

#[tauri::command]
fn assistant_setup_get(app: AppHandle) -> Option<feedrico_core::assistant::AssistantSetup> {
    machine::assistant_setup(&app)
}

/// `None` turns Assistants off and deletes the file. Machine-local, never in the Library.
#[tauri::command]
fn assistant_setup_set(
    setup: Option<feedrico_core::assistant::AssistantSetup>,
    app: AppHandle,
) -> CommandResult<()> {
    machine::set_assistant_setup(&app, setup.as_ref()).map_err(feedrico_core::Error::from)?;
    Ok(())
}

#[tauri::command]
fn library_inspect(path: PathBuf) -> CommandResult<Status> {
    Ok(feedrico_core::library::inspect(&path)?)
}

#[tauri::command]
fn library_create(
    path: PathBuf,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<LibraryInfo> {
    let library = Library::create(&path)?;
    install(&app, &state, library)
}

#[tauri::command]
fn library_open(
    path: PathBuf,
    allow_upgrade: bool,
    take_over_lock: bool,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<LibraryInfo> {
    let library = Library::open(
        &path,
        OpenOptions {
            allow_upgrade,
            take_over_lock,
        },
    )?;
    install(&app, &state, library)
}

#[tauri::command]
fn library_info(state: State<'_, AppState>) -> CommandResult<LibraryInfo> {
    let guard = state.library.lock().unwrap_or_else(PoisonError::into_inner);
    let library = guard
        .as_ref()
        .ok_or_else(|| CommandError::new("noLibrary", "no Library is open"))?;
    info(library)
}

#[tauri::command]
fn library_close(state: State<'_, AppState>) -> CommandResult<()> {
    if let Some(library) = state
        .library
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
    {
        library.close()?;
    }
    Ok(())
}

#[tauri::command]
fn import_opml(path: PathBuf, state: State<'_, AppState>) -> CommandResult<ImportSummary> {
    let bytes = std::fs::read(&path)?;
    let text = String::from_utf8_lossy(&bytes);
    let feeds = opml::parse(&text)?;
    with_library(&state, |l| l.import_opml(&feeds))
}

/// Resolves a Feed URL or website URL to a validated Feed (or a choice of several).
#[tauri::command]
async fn discover_feed(input: String, app: AppHandle) -> CommandResult<Discovery> {
    let fetcher = app.state::<AppState>().fetcher.clone();
    Ok(fetcher.discover(&input).await?)
}

#[tauri::command]
fn add_feed(
    url: String,
    title: String,
    site_url: Option<String>,
    folder_id: Option<i64>,
    state: State<'_, AppState>,
) -> CommandResult<AddOutcome> {
    with_library(&state, |l| {
        l.add_feed(&url, &title, site_url.as_deref(), folder_id)
    })
}

#[tauri::command]
fn rename_feed(id: i64, title: String, state: State<'_, AppState>) -> CommandResult<()> {
    with_library(&state, |l| l.rename_feed(id, &title))
}

#[tauri::command]
fn rename_folder(id: i64, name: String, state: State<'_, AppState>) -> CommandResult<()> {
    with_library(&state, |l| l.rename_folder(id, &name))
}

#[tauri::command]
fn move_feed(id: i64, folder_id: Option<i64>, state: State<'_, AppState>) -> CommandResult<()> {
    with_library(&state, |l| l.move_feed(id, folder_id))
}

#[tauri::command]
fn delete_folder(
    id: i64,
    unsubscribe_feeds: bool,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    with_library(&state, |l| {
        if unsubscribe_feeds {
            l.delete_folder_with_feeds(id)
        } else {
            l.delete_folder(id)
        }
    })
}

#[tauri::command]
fn unsubscribe(id: i64, state: State<'_, AppState>) -> CommandResult<()> {
    with_library(&state, |l| l.unsubscribe(id))
}

#[tauri::command]
fn unsubscribe_feeds(ids: Vec<i64>, state: State<'_, AppState>) -> CommandResult<usize> {
    with_library(&state, |l| l.unsubscribe_many(&ids))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RetryOutcome {
    /// The Feed's error after the attempt; `None` if it worked.
    error: Option<String>,
    new_articles: usize,
}

/// *Retry now* in the Broken Feeds dialog: fetches one Feed at once, ignoring its backoff.
/// New Articles' images are cached in the background, as after a refresh.
#[tauri::command]
async fn retry_feed(id: i64, app: AppHandle) -> CommandResult<RetryOutcome> {
    let state = app.state::<AppState>();
    let (dir, feed) = with_library(&state, |l| {
        Ok((l.dir().to_path_buf(), l.feed_to_fetch(id)?))
    })?;
    let result = state.fetcher.fetch(&feed).await;
    let error = match &result {
        FetchResult::Failed(message) => Some(message.clone()),
        _ => None,
    };
    let outcome = {
        let guard = state.library.lock().unwrap_or_else(PoisonError::into_inner);
        match guard.as_ref() {
            Some(l) if l.dir() == dir => l.apply_fetch(id, result)?,
            _ => return Err(CommandError::new("noLibrary", "the Library was closed")),
        }
    };
    let new_articles = outcome.new_articles;
    if !outcome.images.is_empty() {
        let app = app.clone();
        tauri::async_runtime::spawn(async move { cache_images(&app, &dir, outcome.images).await });
    }
    Ok(RetryOutcome {
        error,
        new_articles,
    })
}

#[tauri::command]
fn export_opml(path: PathBuf, state: State<'_, AppState>) -> CommandResult<()> {
    let xml = with_library(&state, |l| l.export_opml())?;
    std::fs::write(path, xml)?;
    Ok(())
}

/// Copies the open Library to `path` in the background and reports `export-progress`. Only the
/// database snapshot holds the Library; the media copy runs without blocking the UI.
#[tauri::command]
async fn export_library(path: PathBuf, app: AppHandle) -> CommandResult<()> {
    let export = {
        let state = app.state::<AppState>();
        with_library(&state, |l| l.begin_export(&path))?
    };
    let progress_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        export.finish(|p| {
            let _ = progress_app.emit("export-progress", p);
        })
    })
    .await
    .map_err(|e| CommandError::new("internal", e.to_string()))??;
    Ok(())
}

/// Finds the user's Assistants (epic #110). It runs each CLI's own `--version` and status
/// command, so it stays off the UI thread, and the UI calls it only when Settings opens or a
/// first run starts, never at startup.
#[tauri::command]
async fn assistant_detect(
    manual_path: Option<PathBuf>,
) -> CommandResult<Vec<feedrico_core::assistant::FoundAssistant>> {
    tauri::async_runtime::spawn_blocking(move || {
        feedrico_core::assistant::detect::detect(&feedrico_core::assistant::detect::DetectOptions {
            manual_path,
            ..Default::default()
        })
    })
    .await
    .map_err(|e| CommandError::new("internal", e.to_string()))
}

#[tauri::command]
fn sidebar(today_since: i64, state: State<'_, AppState>) -> CommandResult<Sidebar> {
    with_library(&state, |l| l.sidebar(today_since))
}

#[tauri::command]
fn count_articles(query: ListQuery, state: State<'_, AppState>) -> CommandResult<i64> {
    with_library(&state, |l| l.count_articles(&query))
}

#[tauri::command]
fn list_articles(
    query: ListQuery,
    offset: i64,
    limit: i64,
    state: State<'_, AppState>,
) -> CommandResult<Vec<ArticleRow>> {
    with_library(&state, |l| l.list_articles(&query, offset, limit))
}

#[tauri::command]
fn get_article(id: i64, state: State<'_, AppState>) -> CommandResult<ArticleDetail> {
    with_library(&state, |l| l.article(id))
}

#[tauri::command]
fn set_read(id: i64, read: bool, state: State<'_, AppState>) -> CommandResult<()> {
    with_library(&state, |l| l.set_read(id, read))
}

#[tauri::command]
fn set_starred(id: i64, starred: bool, state: State<'_, AppState>) -> CommandResult<()> {
    with_library(&state, |l| l.set_starred(id, starred))
}

#[tauri::command]
fn mark_all_read(query: ListQuery, state: State<'_, AppState>) -> CommandResult<Vec<i64>> {
    with_library(&state, |l| l.mark_all_read(&query))
}

#[tauri::command]
fn mark_unread(ids: Vec<i64>, state: State<'_, AppState>) -> CommandResult<()> {
    with_library(&state, |l| l.mark_unread(&ids))
}

#[tauri::command]
fn layouts(state: State<'_, AppState>) -> CommandResult<Layouts> {
    with_library(&state, |l| l.layouts())
}

/// `view` is a View key (`feed:3`, `allUnread`, …) or `None` for the global default.
#[tauri::command]
fn set_layout(
    view: Option<String>,
    layout: Layout,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    with_library(&state, |l| l.set_layout(view.as_deref(), layout))
}

/// The View selected last (a View key), so the next start continues there.
#[tauri::command]
fn last_view(state: State<'_, AppState>) -> CommandResult<Option<String>> {
    with_library(&state, |l| l.last_view())
}

#[tauri::command]
fn set_last_view(view: String, state: State<'_, AppState>) -> CommandResult<()> {
    with_library(&state, |l| l.set_last_view(&view))
}

/// Every setting that travels with the Library (spec: "What lives in the Library").
#[tauri::command]
fn settings(state: State<'_, AppState>) -> CommandResult<Settings> {
    with_library(&state, |l| l.settings())
}

/// Validates and stores all Library settings. When the image cache cap was lowered, eviction runs
/// in the background so the UI never waits on it.
#[tauri::command]
async fn set_settings(
    new: Settings,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let old_cap = with_library(&state, |l| l.image_cache_cap())?;
    with_library(&state, |l| l.set_settings(&new))?;
    if new.image_cache_cap_bytes < old_cap {
        let cap = new.image_cache_cap_bytes;
        tauri::async_runtime::spawn_blocking(move || {
            let state = app.state::<AppState>();
            let guard = state.library.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(l) = guard.as_ref() {
                let _ = l.evict_media(cap);
                let _ = l.remove_unreferenced_media();
            }
        });
    }
    Ok(())
}

/// Whether a raw key exists in the Library's `setting` table (the one-time theme migration).
#[tauri::command]
fn has_setting(key: String, state: State<'_, AppState>) -> CommandResult<bool> {
    with_library(&state, |l| l.has_setting(&key))
}

/// Recursive size of a folder in bytes, computed off the UI thread.
fn dir_size(dir: &Path) -> std::io::Result<u64> {
    let mut total = 0;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        if meta.is_dir() {
            total += dir_size(&entry.path())?;
        } else {
            total += meta.len();
        }
    }
    Ok(total)
}

/// Total size of the open Library folder, including cached media.
#[tauri::command]
async fn library_size(state: State<'_, AppState>) -> CommandResult<u64> {
    let dir = with_library(&state, |l| Ok(l.dir().to_path_buf()))?;
    tauri::async_runtime::spawn_blocking(move || dir_size(&dir))
        .await
        .map_err(|e| CommandError::new("internal", e.to_string()))?
        .map_err(CommandError::from)
}

/// Size of the Library's `media/` cache alone, shown next to the image cache cap.
#[tauri::command]
async fn media_size(state: State<'_, AppState>) -> CommandResult<u64> {
    let dir = with_library(&state, |l| {
        Ok(l.dir().join(feedrico_core::library::MEDIA_DIR))
    })?;
    tauri::async_runtime::spawn_blocking(move || dir_size(&dir).unwrap_or(0))
        .await
        .map_err(|e| CommandError::new("internal", e.to_string()))
}

/// "Start with OS" is machine-local, so it never lives in the Library. In Portable Mode an
/// autostart entry pointing at a USB stick would break when the stick is gone, so the shell
/// refuses it there too (the UI also disables the toggle).
#[tauri::command]
fn autostart_enabled(app: AppHandle) -> CommandResult<bool> {
    Ok(app.autolaunch().is_enabled().unwrap_or(false))
}

#[tauri::command]
fn set_autostart(enabled: bool, app: AppHandle) -> CommandResult<()> {
    if machine::portable_library().is_some() {
        return Err(CommandError::new(
            "portableMode",
            "start with OS is not available in Portable Mode",
        ));
    }
    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    result.map_err(|e| CommandError::new("io", e.to_string()))
}

/// Spec "Network policy": asks GitHub Releases for the newest version, only when the user
/// clicks "Check for updates". Nothing calls this on startup.
#[tauri::command]
async fn check_for_updates(app: AppHandle) -> CommandResult<UpdateStatus> {
    let fetcher = app.state::<AppState>().fetcher.clone();
    let current = app.package_info().version.to_string();
    Ok(fetcher.check_for_updates(&current).await?)
}

/// Only web links open in the system browser; nothing else is allowed through.
fn is_web_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// Opens an http(s) link in the system browser. Nothing else is allowed through.
#[tauri::command]
fn open_external(url: String, app: AppHandle) -> CommandResult<()> {
    if !is_web_url(&url) {
        return Err(CommandError::new(
            "invalidUrl",
            "only web links can be opened",
        ));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| CommandError::new("io", e.to_string()))
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RefreshProgress {
    done: usize,
    total: usize,
    new_articles: usize,
    failed: usize,
    finished: bool,
}

/// Starts a background refresh. Returns `false` if one is already running.
#[tauri::command]
fn refresh(force: bool, app: AppHandle, state: State<'_, AppState>) -> bool {
    if state.refreshing.swap(true, Ordering::SeqCst) {
        return false;
    }
    tauri::async_runtime::spawn(async move {
        // Clears the flag even if the refresh panics, so refreshing never gets stuck.
        struct Done(AppHandle);
        impl Drop for Done {
            fn drop(&mut self) {
                self.0
                    .state::<AppState>()
                    .refreshing
                    .store(false, Ordering::SeqCst);
            }
        }
        let _done = Done(app.clone());
        run_refresh(&app, force).await;
    });
    true
}

/// Fetching and parsing happen without the database lock; only storing each result takes it,
/// briefly. Results are dropped if the user switched Libraries meanwhile.
async fn run_refresh(app: &AppHandle, force: bool) {
    let state = app.state::<AppState>();
    let started = {
        let guard = state.library.lock().unwrap_or_else(PoisonError::into_inner);
        guard
            .as_ref()
            .and_then(|l| Some((l.dir().to_path_buf(), l.feeds_to_fetch(force).ok()?)))
    };
    let Some((dir, feeds)) = started else { return };

    let mut progress = RefreshProgress {
        done: 0,
        total: feeds.len(),
        new_articles: 0,
        failed: 0,
        finished: false,
    };
    let _ = app.emit("refresh-progress", progress.clone());

    let fetcher = state.fetcher.clone();
    let mut results = futures::stream::iter(feeds)
        .map(|feed| {
            let fetcher = fetcher.clone();
            async move { (feed.id, fetcher.fetch(&feed).await) }
        })
        .buffer_unordered(FETCH_CONCURRENCY);

    let mut images = Vec::new();
    while let Some((feed_id, result)) = results.next().await {
        let applied = {
            let guard = state.library.lock().unwrap_or_else(PoisonError::into_inner);
            match guard.as_ref() {
                Some(l) if l.dir() == dir => l.apply_fetch(feed_id, result).ok(),
                _ => return,
            }
        };
        progress.done += 1;
        if let Some(outcome) = applied {
            progress.new_articles += outcome.new_articles;
            progress.failed += usize::from(outcome.failed);
            images.extend(outcome.images);
        }
        let _ = app.emit("refresh-progress", progress.clone());
    }
    progress.finished = true;
    let _ = app.emit("refresh-progress", progress);

    cache_images(app, &dir, images).await;
    prune(app, &dir).await;
}

/// Articles removed per lock hold, so reading and searching never wait long on Pruning.
const PRUNE_BATCH: usize = 200;

/// Pruning after a refresh. Each batch takes the Library lock on its own; the orphaned images
/// are removed at the end. Tells the UI only if something was removed.
async fn prune(app: &AppHandle, dir: &Path) {
    let state = app.state::<AppState>();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let mut removed = 0;
    loop {
        let n = {
            let guard = state.library.lock().unwrap_or_else(PoisonError::into_inner);
            match guard.as_ref().filter(|l| l.dir() == dir) {
                Some(l) => l.prune_batch(now, PRUNE_BATCH).unwrap_or(0),
                None => return,
            }
        };
        removed += n;
        if n < PRUNE_BATCH {
            break;
        }
    }
    if removed == 0 {
        return;
    }
    {
        let guard = state.library.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(l) = guard.as_ref().filter(|l| l.dir() == dir) {
            let _ = l.remove_unreferenced_media();
        }
    }
    let _ = app.emit("articles-pruned", removed);
}

/// Lead Images that weren't cached when their Article arrived get another chance each refresh.
const LEAD_IMAGE_CATCH_UP: usize = 100;

async fn cache_images(app: &AppHandle, dir: &Path, images: Vec<(i64, String)>) {
    let state = app.state::<AppState>();
    let mut urls: Vec<String> = images.into_iter().map(|(_, url)| url).collect();
    {
        let guard = state.library.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(l) = guard.as_ref().filter(|l| l.dir() == dir) {
            urls.extend(
                l.missing_lead_images(LEAD_IMAGE_CATCH_UP)
                    .unwrap_or_default(),
            );
        }
    }
    urls.sort();
    urls.dedup();
    let fetcher = state.fetcher.clone();
    let mut downloads = futures::stream::iter(urls)
        .map(|url| {
            let fetcher = fetcher.clone();
            async move {
                let result = fetcher.download_image(&url).await;
                (url, result)
            }
        })
        .buffer_unordered(IMAGE_CONCURRENCY);
    while let Some((url, result)) = downloads.next().await {
        let Ok((bytes, mime)) = result else { continue };
        let guard = state.library.lock().unwrap_or_else(PoisonError::into_inner);
        match guard.as_ref() {
            Some(l) if l.dir() == dir => {
                let _ = l.store_media(&url, &bytes, &mime);
            }
            _ => return,
        }
    }
    cache_favicons(app, dir).await;
    // Last, so a fresh download can't push the cache past its cap until the next refresh.
    {
        let guard = state.library.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(l) = guard.as_ref().filter(|l| l.dir() == dir)
            && let Ok(cap) = l.image_cache_cap()
        {
            let _ = l.evict_media(cap);
        }
    }
    let _ = app.emit("media-cached", ());
}

/// Feeds without a favicon get another look each refresh, a few at a time.
const FAVICONS_PER_REFRESH: usize = 20;

async fn cache_favicons(app: &AppHandle, dir: &Path) {
    let state = app.state::<AppState>();
    let pending = {
        let guard = state.library.lock().unwrap_or_else(PoisonError::into_inner);
        guard
            .as_ref()
            .filter(|l| l.dir() == dir)
            .and_then(|l| l.feeds_missing_favicon().ok())
            .unwrap_or_default()
    };
    let fetcher = state.fetcher.clone();
    let mut icons = futures::stream::iter(pending.into_iter().take(FAVICONS_PER_REFRESH))
        .map(|(feed_id, page)| {
            let fetcher = fetcher.clone();
            async move { (feed_id, fetcher.fetch_favicon(&page).await) }
        })
        .buffer_unordered(IMAGE_CONCURRENCY);
    while let Some((feed_id, icon)) = icons.next().await {
        let Some((url, bytes, mime)) = icon else {
            continue;
        };
        let guard = state.library.lock().unwrap_or_else(PoisonError::into_inner);
        match guard.as_ref() {
            Some(l) if l.dir() == dir => {
                let _ = l.set_favicon(feed_id, &url, &bytes, &mime);
            }
            _ => return,
        }
    }
}

#[tauri::command]
fn perf_interactive(state: State<'_, AppState>) -> CommandResult<f64> {
    let perf = state.perf.as_ref().ok_or_else(not_perf_mode)?;
    Ok(perf.elapsed_ms())
}

#[tauri::command]
fn perf_finish(
    report: serde_json::Value,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    state.perf.as_ref().ok_or_else(not_perf_mode)?;
    println!("FEEDRICO_PERF {report}");
    app.exit(0);
    Ok(())
}

/// What the UI asks an Assistant to do. Features add variants.
#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum AssistantAction {
    Test,
    #[serde(rename_all = "camelCase")]
    Summarize {
        article_id: i64,
    },
    #[serde(rename_all = "camelCase")]
    Explain {
        article_id: i64,
    },
    /// "Explain a term" (#118): the selected text and its surrounding paragraph.
    #[serde(rename_all = "camelCase")]
    ExplainTerm {
        article_id: i64,
        term: String,
        context: String,
    },
    #[serde(rename_all = "camelCase")]
    Translate {
        article_id: i64,
    },
    #[serde(rename_all = "camelCase")]
    Ask {
        article_id: i64,
        question: String,
        history: Vec<assistant::ask::Turn>,
    },
    /// One batch of Feeds in `folder_id` (or at the top level when `None`), at most
    /// `BATCH_SIZE` of them. `new_folders` are Folder names the earlier batches of the same
    /// run proposed, so later batches reuse them.
    #[serde(rename_all = "camelCase")]
    FolderSuggestions {
        feed_ids: Vec<i64>,
        new_folders: Vec<String>,
        #[serde(default)]
        folder_id: Option<i64>,
    },
    /// Turn a question into a local search (#122). Only the question and the Library's Folder
    /// and Feed names go to the Assistant, never Article content.
    SearchQuery {
        question: String,
    },
}

impl AssistantAction {
    /// The Article this action works on, if any. It is loaded from the open Library.
    fn article_id(&self) -> Option<i64> {
        match self {
            AssistantAction::Test => None,
            AssistantAction::Summarize { article_id }
            | AssistantAction::Explain { article_id }
            | AssistantAction::ExplainTerm { article_id, .. } => Some(*article_id),
            AssistantAction::Translate { article_id } | AssistantAction::Ask { article_id, .. } => {
                Some(*article_id)
            }
            AssistantAction::FolderSuggestions { .. } | AssistantAction::SearchQuery { .. } => None,
        }
    }
}

/// What a Folder suggestions run needs from the Library: the Task, and the Feeds and Folder
/// names the answer is checked against. `None` when none of the Feeds is in the scope anymore,
/// so there is nothing to ask.
type FolderSuggestionsInput = (
    assistant::Task,
    Vec<feedrico_core::feeds::UnsortedFeed>,
    Vec<String>,
);

fn folder_suggestions_input(
    library: &Library,
    feed_ids: &[i64],
    new_folders: &[String],
    folder_id: Option<i64>,
) -> feedrico_core::Result<Option<FolderSuggestionsInput>> {
    use assistant::folder_suggestions::{BATCH_SIZE, Scope};
    if feed_ids.len() > BATCH_SIZE {
        return Err(feedrico_core::Error::Invalid(format!(
            "ask about at most {BATCH_SIZE} Feeds at a time"
        )));
    }
    let (feeds, scope) = match folder_id {
        None => (library.top_level_feeds(feed_ids)?, Scope::TopLevel),
        Some(id) => {
            let name = library
                .folder_name(id)?
                .ok_or(feedrico_core::Error::NotFound)?;
            (library.folder_feeds(id, feed_ids)?, Scope::Folder(name))
        }
    };
    if feeds.is_empty() {
        return Ok(None);
    }
    let mut folders = library.folder_names()?;
    for name in new_folders {
        if !folders.iter().any(|f| f.eq_ignore_ascii_case(name)) {
            folders.push(name.clone());
        }
    }
    let task = assistant::folder_suggestions::task(&feeds, &folders, &scope);
    Ok(Some((task, feeds, folders)))
}

/// What the panel should say about the input an Article result is based on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArticleNotes {
    /// The Feed gave only a short excerpt, so the answer covers that excerpt.
    excerpt_only: bool,
    /// The Article was cut; the answer is based on the first `words_used` words.
    truncated: bool,
    words_used: usize,
    /// Ask only: how many of the oldest turns were left out to stay inside the caps.
    dropped_turns: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AssistantResult {
    text: String,
    structured: Option<serde_json::Value>,
    /// Set for Article actions only.
    article: Option<ArticleNotes>,
    /// Set for Translate only: the answer already sanitized in the core, images restored.
    html: Option<String>,
}

fn article_notes(
    excerpt_only: bool,
    truncated: bool,
    words_used: usize,
    dropped_turns: Option<usize>,
) -> ArticleNotes {
    ArticleNotes {
        excerpt_only,
        truncated,
        words_used,
        dropped_turns,
    }
}

/// Builds the Task for an action. `article` is the loaded Article for Article actions. The
/// model is the user's choice from the setup, else the fast default for that Assistant.
/// `target_language` is the machine-local translation target from the setup. The third element
/// is the Article's original `<img>` tags for Translate, to restore after the run.
fn build_task(
    action: &AssistantAction,
    article: Option<&ArticleDetail>,
    kind: assistant::AssistantKind,
    setup_model: Option<String>,
    target_language: Option<&str>,
) -> CommandResult<(assistant::Task, Option<ArticleNotes>, Option<Vec<String>>)> {
    use assistant::article::{
        ArticleInput, explain_task, explain_term_task, fast_model, summarize_task,
    };
    use assistant::{ask, translate};
    let input = article.map(ArticleInput::from_article);
    let (mut task, notes, images) = match (action, input.as_ref()) {
        (AssistantAction::Test, _) => (assistant::prompts::test_task(), None, None),
        (AssistantAction::Summarize { .. }, Some(input)) => (
            summarize_task(input),
            Some(article_notes(
                input.excerpt_only,
                input.truncated,
                input.words_used,
                None,
            )),
            None,
        ),
        (AssistantAction::Explain { .. }, Some(input)) => (
            explain_task(input),
            Some(article_notes(
                input.excerpt_only,
                input.truncated,
                input.words_used,
                None,
            )),
            None,
        ),
        (AssistantAction::ExplainTerm { term, context, .. }, Some(input)) => (
            explain_term_task(term, context),
            Some(article_notes(
                input.excerpt_only,
                input.truncated,
                input.words_used,
                None,
            )),
            None,
        ),
        (AssistantAction::Translate { .. }, Some(_)) => {
            let prep = translate::prepare(article.expect("an input means an Article"));
            // The UI defaults the target to the system language; without a setup value, English.
            let target = target_language.unwrap_or("English");
            (
                translate::translate_task(&prep, target),
                Some(article_notes(
                    prep.excerpt_only,
                    prep.truncated,
                    prep.words_used,
                    None,
                )),
                Some(prep.images),
            )
        }
        (
            AssistantAction::Ask {
                question, history, ..
            },
            Some(input),
        ) => {
            let (task, dropped) = ask::ask_task(input, question, history);
            (
                task,
                Some(article_notes(
                    input.excerpt_only,
                    input.truncated,
                    input.words_used,
                    Some(dropped),
                )),
                None,
            )
        }
        (AssistantAction::FolderSuggestions { .. } | AssistantAction::SearchQuery { .. }, _) => {
            return Err(CommandError::new(
                "invalid",
                "This Assistant action doesn't work on an Article.",
            ));
        }
        (_, None) => {
            return Err(CommandError::new(
                "notFound",
                "That Article isn't available.",
            ));
        }
    };
    task.model = setup_model.or_else(|| {
        input
            .as_ref()
            .and_then(|_| fast_model(kind))
            .map(String::from)
    });
    Ok((task, notes, images))
}

impl From<AssistantError> for CommandError {
    fn from(error: AssistantError) -> Self {
        let kind = match error {
            AssistantError::NotInstalled => "notInstalled",
            AssistantError::NotSignedIn => "notSignedIn",
            AssistantError::UsageLimit => "usageLimit",
            AssistantError::Offline => "offline",
            AssistantError::TimedOut => "timedOut",
            AssistantError::Cancelled => "cancelled",
            AssistantError::Malformed { .. } => "malformed",
            AssistantError::Failed { .. } => "failed",
        };
        CommandError::new(kind, error.to_string())
    }
}

/// Which Assistant a run uses: only the machine-local setup, and only once its disclosure is
/// accepted. Without that nothing runs (Assistants are off by default; ADR 0005).
fn run_target(
    setup: Option<assistant::AssistantSetup>,
) -> CommandResult<(assistant::FoundAssistant, Option<String>)> {
    let setup = setup
        .ok_or_else(|| CommandError::new("notSetUp", "Set up an Assistant in Settings first."))?;
    if !setup.disclosure_accepted {
        return Err(CommandError::new(
            "notSetUp",
            "Read and accept the note in Settings, under Assistant, first.",
        ));
    }
    let found = assistant::FoundAssistant {
        kind: setup.kind,
        path: setup.path,
        version: None,
        sign_in: assistant::SignIn::Unknown,
    };
    Ok((found, setup.model))
}

/// The open Library's Folder and Feed names for a SearchQuery run (#122): the prompt gets only
/// these names plus the question, never Article content.
fn search_query_names(state: &AppState) -> CommandResult<feedrico_core::FolderFeedNames> {
    with_library(state, |l| l.folder_feed_names())
}

/// Run `task`, streaming `assistant-event`s for `run_id`; `assistant_cancel` can stop it.
async fn run_task(
    app: &AppHandle,
    state: &State<'_, AppState>,
    assistant: &assistant::FoundAssistant,
    task: &assistant::Task,
    run_id: String,
) -> CommandResult<(String, Option<serde_json::Value>)> {
    let cancel = assistant::run::Cancel::new();
    state
        .assistant_runs
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(run_id.clone(), cancel.clone());
    let event_app = app.clone();
    let event_run_id = run_id.clone();
    let result = assistant::run::run(assistant, task, cancel, move |event| {
        let _ = event_app.emit(
            "assistant-event",
            serde_json::json!({ "runId": event_run_id, "event": event }),
        );
    })
    .await;
    state
        .assistant_runs
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&run_id);
    Ok(result?)
}

/// Run one Assistant task on the user's explicit action, streaming `assistant-event`s.
///
/// Runs only on the machine-local Assistant setup, and only once the user accepted the
/// disclosure: no setup means Assistants are off, so nothing is detected or run behind it.
#[tauri::command]
async fn assistant_run(
    app: AppHandle,
    state: State<'_, AppState>,
    run_id: String,
    action: AssistantAction,
) -> CommandResult<AssistantResult> {
    let setup = machine::assistant_setup(&app);
    let (assistant, model) = run_target(setup.clone())?;

    // Folder suggestions (#123) read Feeds, not an Article, and their answer is checked
    // against exactly the Feeds and Folder names that were sent.
    if let AssistantAction::FolderSuggestions {
        feed_ids,
        new_folders,
        folder_id,
    } = &action
    {
        let input = with_library(&state, |l| {
            folder_suggestions_input(l, feed_ids, new_folders, *folder_id)
        })?;
        let Some((mut task, feeds, folders)) = input else {
            return Ok(AssistantResult {
                text: String::new(),
                structured: Some(serde_json::json!({ "moves": [] })),
                article: None,
                html: None,
            });
        };
        if model.is_some() {
            task.model = model;
        }
        let (text, structured) = run_task(&app, &state, &assistant, &task, run_id).await?;
        let moves =
            assistant::folder_suggestions::parse(structured.as_ref(), &text, &feeds, &folders)?;
        return Ok(AssistantResult {
            text: String::new(),
            structured: Some(serde_json::json!({ "moves": moves })),
            article: None,
            html: None,
        });
    }

    // Natural-language search (#122): the answer is validated against the Library before it
    // crosses to the UI: scope names become ids, unknown ones are dropped, the query is capped.
    if let AssistantAction::SearchQuery { question } = &action {
        let names = search_query_names(&state)?;
        let today = assistant::search::today_ymd();
        let mut task = assistant::search::task(question, &names.folders, &names.feeds, &today);
        if model.is_some() {
            task.model = model;
        }
        let (text, structured) = run_task(&app, &state, &assistant, &task, run_id).await?;
        let parsed = assistant::search::parse_output(
            &text,
            structured.as_ref(),
            &names.folders,
            &names.feeds,
        )?;
        let structured = serde_json::to_value(parsed)
            .map_err(|e| CommandError::new("internal", e.to_string()))?;
        return Ok(AssistantResult {
            text,
            structured: Some(structured),
            article: None,
            html: None,
        });
    }

    let article = match action.article_id() {
        Some(id) => Some(with_library(&state, |l| l.article(id))?),
        None => None,
    };
    let (task, article_notes, images) = build_task(
        &action,
        article.as_ref(),
        assistant.kind,
        model,
        setup.as_ref().and_then(|s| s.target_language.as_deref()),
    )?;

    let (text, structured) = run_task(&app, &state, &assistant, &task, run_id).await?;
    // Translate's answer is HTML: sanitize it in the core like Feed HTML and put the Article's
    // cached-image tags back, so the webview only ever sees sanitized HTML.
    let html = match (&action, images) {
        (AssistantAction::Translate { .. }, Some(images)) => {
            Some(assistant::translate::finish_translation(
                &text,
                &images,
                article.as_ref().and_then(|a| a.url.as_deref()),
                MEDIA_PREFIX,
            ))
        }
        _ => None,
    };
    Ok(AssistantResult {
        text,
        structured,
        article: article_notes,
        html,
    })
}

/// Applies accepted Folder suggestions as one step and returns what it did, which the UI
/// hands back to `undo_folder_moves`. Only Feeds still in `scope_folder_id` (or at the top
/// level when `None`) move.
#[tauri::command]
fn apply_folder_moves(
    moves: Vec<feedrico_core::feeds::FolderMove>,
    scope_folder_id: Option<i64>,
    state: State<'_, AppState>,
) -> CommandResult<feedrico_core::feeds::AppliedMoves> {
    with_library(&state, |l| l.apply_folder_moves(&moves, scope_folder_id))
}

#[tauri::command]
fn undo_folder_moves(
    applied: feedrico_core::feeds::AppliedMoves,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    with_library(&state, |l| l.undo_folder_moves(&applied))
}

/// Stop a running Assistant task. Unknown run ids are ignored (it may have just finished).
#[tauri::command]
fn assistant_cancel(state: State<'_, AppState>, run_id: String) {
    if let Some(cancel) = state
        .assistant_runs
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&run_id)
    {
        cancel.cancel();
    }
}

fn not_perf_mode() -> CommandError {
    CommandError::new("notPerfMode", "only available in perf mode")
}

/// Serves `media://localhost/<key>` from the open Library's cache. Nothing else.
fn serve_media(app: &AppHandle, path: &str) -> Response<Vec<u8>> {
    let key = path.trim_start_matches('/');
    let state = app.state::<AppState>();
    let file = {
        let guard = state.library.lock().unwrap_or_else(PoisonError::into_inner);
        guard
            .as_ref()
            .and_then(|l| l.media_file(key).ok().flatten())
    };
    let found = file.and_then(|(path, mime)| Some((std::fs::read(path).ok()?, mime)));
    match found {
        Some((bytes, mime)) => Response::builder()
            .header(header::CONTENT_TYPE, mime)
            .header(header::CACHE_CONTROL, "max-age=31536000, immutable")
            .body(bytes),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Vec::new()),
    }
    .expect("static response parts are valid")
}

/// Keeps the webview on the app: feed content can never navigate it away.
fn navigation_allowed(url: &tauri::Url) -> bool {
    matches!(url.scheme(), "tauri")
        || matches!(url.host_str(), Some("tauri.localhost") | Some("localhost"))
}

fn navigation_guard<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("navigation-guard")
        .on_navigation(|_, url| navigation_allowed(url))
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let started = Instant::now();
    let fetcher = Fetcher::new(MEDIA_PREFIX).expect("HTTP client");
    let perf = perf::Perf::from_env(started);
    let mut builder = tauri::Builder::default();
    if perf.is_some() {
        builder = builder.plugin(
            tauri::plugin::Builder::<tauri::Wry>::new("perf")
                .js_init_script(perf::INIT_SCRIPT.to_string())
                .build(),
        );
    }
    builder
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        // Window size, position and maximized state: machine-local, restored on start.
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(navigation_guard())
        .manage(AppState {
            library: Mutex::new(None),
            fetcher,
            refreshing: AtomicBool::new(false),
            perf,
            assistant_runs: Mutex::new(HashMap::new()),
        })
        .register_uri_scheme_protocol("media", |ctx, request| {
            serve_media(ctx.app_handle(), request.uri().path())
        })
        .invoke_handler(tauri::generate_handler![
            startup,
            recent_libraries,
            assistant_setup_get,
            assistant_setup_set,
            assistant_run,
            assistant_cancel,
            apply_folder_moves,
            undo_folder_moves,
            library_inspect,
            library_create,
            library_open,
            library_info,
            library_close,
            import_opml,
            export_opml,
            export_library,
            discover_feed,
            add_feed,
            rename_feed,
            rename_folder,
            move_feed,
            delete_folder,
            unsubscribe,
            unsubscribe_feeds,
            retry_feed,
            sidebar,
            count_articles,
            list_articles,
            get_article,
            set_read,
            set_starred,
            mark_all_read,
            mark_unread,
            layouts,
            set_layout,
            last_view,
            set_last_view,
            settings,
            set_settings,
            has_setting,
            library_size,
            media_size,
            autostart_enabled,
            set_autostart,
            check_for_updates,
            open_external,
            refresh,
            perf_interactive,
            perf_finish,
            assistant_detect,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Feedrico")
        .run(|app, event| {
            // Release the lock and checkpoint the database on every way out.
            if let tauri::RunEvent::Exit = event
                && let Some(library) = app
                    .state::<AppState>()
                    .library
                    .lock()
                    .ok()
                    .and_then(|mut s| s.take())
            {
                let _ = library.close();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with(library: Option<Library>) -> AppState {
        AppState {
            library: Mutex::new(library),
            fetcher: Fetcher::new("media://localhost/").unwrap(),
            refreshing: AtomicBool::new(false),
            perf: None,
            assistant_runs: Mutex::new(HashMap::new()),
        }
    }

    /// Every core error crosses the IPC boundary as a stable kind the UI can branch on.
    #[test]
    fn command_errors_map_every_core_error_to_a_stable_kind() {
        use feedrico_core::Error::*;
        let lock = feedrico_core::LockInfo {
            machine: "friend-laptop".into(),
            pid: 4242,
            opened_at: 1762547640,
        };
        let cases: Vec<(feedrico_core::Error, &str)> = vec![
            (NotALibrary("/x".into()), "notALibrary"),
            (TargetNotEmpty("/x".into()), "targetNotEmpty"),
            (
                TooNew {
                    found: 4,
                    supported: 3,
                },
                "tooNew",
            ),
            (
                NeedsUpgrade {
                    found: 1,
                    target: 3,
                },
                "needsUpgrade",
            ),
            (Locked(lock), "locked"),
            (Opml("bad".into()), "invalidOpml"),
            (Feed("bad".into()), "internal"),
            (Network("down".into()), "internal"),
            (Invalid("bad".into()), "invalid"),
            (
                InvalidSetting {
                    key: "theme".into(),
                    message: "unknown".into(),
                },
                "invalidSetting",
            ),
            (NotFound, "notFound"),
            (
                Manifest(serde_json::from_str::<serde_json::Value>("{").unwrap_err()),
                "internal",
            ),
            (Io(std::io::Error::other("disk full")), "internal"),
        ];
        for (error, kind) in cases {
            let command_error = CommandError::from(error);
            assert_eq!(command_error.kind, kind);
            assert!(!command_error.message.is_empty());
        }
        // The message is the human-readable Display of the source error.
        assert_eq!(CommandError::from(NotFound).message, "not found");
        assert_eq!(
            CommandError::from(Locked(feedrico_core::LockInfo {
                machine: "friend-laptop".into(),
                pid: 4242,
                opened_at: 1762547640,
            }))
            .message,
            "this Library is open on friend-laptop since 1762547640"
        );
    }

    #[test]
    fn io_errors_cross_as_io() {
        let error = CommandError::from(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "nope",
        ));
        assert_eq!(error.kind, "io");
        assert!(error.message.contains("nope"));
    }

    #[test]
    fn with_library_runs_against_the_open_library_or_reports_none() {
        let empty = state_with(None);
        let err = with_library(&empty, |_| Ok::<(), feedrico_core::Error>(())).unwrap_err();
        assert_eq!(err.kind, "noLibrary");
        assert_eq!(err.message, "no Library is open");

        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join(feedrico_core::library::DIR_NAME);
        let library = Library::create(&dir).unwrap();
        library
            .add_feed("https://a.example/feed", "A", None, None)
            .unwrap();
        let state = state_with(Some(library));
        assert_eq!(with_library(&state, |l| l.feed_count()).unwrap(), 1);
    }

    #[test]
    fn library_info_reports_path_name_version_and_feed_count() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join(feedrico_core::library::DIR_NAME);
        let library = Library::create(&dir).unwrap();
        library
            .add_feed("https://a.example/feed", "A", None, None)
            .unwrap();
        let info = info(&library).unwrap();
        assert_eq!(info.path, dir);
        assert_eq!(info.name, feedrico_core::library::DIR_NAME);
        assert_eq!(info.format_version, feedrico_core::FORMAT_VERSION);
        assert_eq!(info.feed_count, 1);
        if std::env::var_os("APPIMAGE").is_none() {
            assert!(!info.portable, "tests never run in Portable Mode");
        }
        library.close().unwrap();
    }

    #[test]
    fn dir_size_sums_files_recursively_and_fails_on_missing_dirs() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("a"), [0u8; 10]).unwrap();
        let sub = root.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("b"), [0u8; 5]).unwrap();
        assert_eq!(dir_size(root.path()).unwrap(), 15);
        assert!(dir_size(&root.path().join("missing")).is_err());
    }

    /// Feed content can never navigate the webview away from the app shell.
    #[test]
    fn navigation_stays_on_the_app_shell() {
        for allowed in [
            "tauri://localhost",
            "https://tauri.localhost/",
            "http://localhost:1430/",
            "https://localhost/index.html",
            // The host rule also covers the local media scheme the shell itself registers.
            "media://localhost/abc123",
        ] {
            assert!(
                navigation_allowed(&tauri::Url::parse(allowed).unwrap()),
                "{allowed} must stay"
            );
        }
        for blocked in [
            "https://example.com",
            "http://example.com/feed",
            "media://example.com/abc123",
            "javascript:alert(1)",
            "file:///etc/passwd",
        ] {
            assert!(
                !navigation_allowed(&tauri::Url::parse(blocked).unwrap()),
                "{blocked} must be blocked"
            );
        }
    }

    #[test]
    fn only_http_and_https_links_open_externally() {
        assert!(is_web_url("https://example.com"));
        assert!(is_web_url("http://example.com/x?y=1"));
        for bad in [
            "",
            "example.com",
            "ftp://x",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "HTTPS://example.com",
            " https://example.com",
        ] {
            assert!(!is_web_url(bad), "{bad:?} must not open in a browser");
        }
    }

    #[test]
    fn assistant_errors_cross_ipc_with_their_kind() {
        let e = CommandError::from(AssistantError::UsageLimit);
        assert_eq!(e.kind, "usageLimit");
        let e = CommandError::from(AssistantError::Malformed {
            message: "x".into(),
        });
        assert_eq!(e.kind, "malformed");
    }

    fn detail(html: &str) -> ArticleDetail {
        ArticleDetail {
            id: 7,
            feed_id: 1,
            feed_title: "Blog".into(),
            feed_site_url: None,
            title: "Title".into(),
            url: None,
            author: None,
            content_html: html.into(),
            article_date: 0,
            is_read: false,
            is_starred: false,
            favicon: None,
            enclosures: vec![],
            video: None,
        }
    }

    #[test]
    fn article_actions_deserialize_from_the_ui_shape() {
        let a: AssistantAction =
            serde_json::from_str(r#"{"kind":"summarize","articleId":7}"#).unwrap();
        assert_eq!(a.article_id(), Some(7));
        let a: AssistantAction =
            serde_json::from_str(r#"{"kind":"explain","articleId":8}"#).unwrap();
        assert_eq!(a.article_id(), Some(8));
        let a: AssistantAction = serde_json::from_str(
            r#"{"kind":"explainTerm","articleId":9,"term":"quota","context":"The quota resets daily."}"#,
        )
        .unwrap();
        assert_eq!(a.article_id(), Some(9));
        let a: AssistantAction = serde_json::from_str(r#"{"kind":"test"}"#).unwrap();
        assert_eq!(a.article_id(), None);
        let a: AssistantAction =
            serde_json::from_str(r#"{"kind":"translate","articleId":9}"#).unwrap();
        assert_eq!(a.article_id(), Some(9));
        let a: AssistantAction = serde_json::from_str(
            r#"{"kind":"ask","articleId":10,"question":"Why?","history":[{"question":"Q","answer":"A"}]}"#,
        )
        .unwrap();
        assert_eq!(a.article_id(), Some(10));
        match a {
            AssistantAction::Ask {
                question, history, ..
            } => {
                assert_eq!(question, "Why?");
                assert_eq!(
                    history,
                    vec![assistant::ask::Turn {
                        question: "Q".into(),
                        answer: "A".into(),
                    }]
                );
            }
            _ => panic!("expected ask"),
        }
    }

    #[test]
    fn actions_build_their_task_with_the_fast_model_unless_the_setup_names_one() {
        use assistant::AssistantKind::{ClaudeCode, MistralVibe};
        let article = detail("<p>Hello body</p>");
        let summarize = AssistantAction::Summarize { article_id: 7 };
        let (task, notes, images) =
            build_task(&summarize, Some(&article), ClaudeCode, None, None).unwrap();
        assert_eq!(task.model.as_deref(), Some("haiku"));
        assert!(task.system_prompt.contains("3 to 5"));
        assert!(task.prompt.contains("<<<FEEDRICO_DATA Article>>>"));
        assert!(task.prompt.contains("Hello body"));
        let notes = notes.unwrap();
        assert!(notes.excerpt_only && !notes.truncated && notes.dropped_turns.is_none());
        assert!(images.is_none());

        let explain = AssistantAction::Explain { article_id: 7 };
        let (task, _, _) = build_task(
            &explain,
            Some(&article),
            ClaudeCode,
            Some("sonnet".into()),
            None,
        )
        .unwrap();
        assert_eq!(task.model.as_deref(), Some("sonnet"));
        assert!(task.system_prompt.contains("plain language"));

        let (task, _, _) = build_task(&summarize, Some(&article), MistralVibe, None, None).unwrap();
        assert_eq!(task.model, None);
    }

    #[test]
    fn the_test_action_has_no_article_and_an_article_action_needs_one() {
        let (task, notes, images) = build_task(
            &AssistantAction::Test,
            None,
            assistant::AssistantKind::ClaudeCode,
            None,
            None,
        )
        .unwrap();
        assert!(notes.is_none() && images.is_none());
        assert_eq!(task.model, None);
        let err = build_task(
            &AssistantAction::Summarize { article_id: 1 },
            None,
            assistant::AssistantKind::ClaudeCode,
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(err.kind, "notFound");
    }

    #[test]
    fn translate_builds_a_task_with_the_target_language_and_the_images_to_restore() {
        let body = format!(
            "<p>{}</p>",
            "Bonjour le monde, voici un article long. ".repeat(15)
        );
        let article = detail(&format!(
            r#"{body}<img src="media://localhost/abc" alt="Une image">"#
        ));
        let action = AssistantAction::Translate { article_id: 7 };
        let (task, notes, images) = build_task(
            &action,
            Some(&article),
            assistant::AssistantKind::ClaudeCode,
            None,
            Some("de-DE"),
        )
        .unwrap();
        assert!(task.system_prompt.contains("into de-DE"));
        assert!(task.system_prompt.contains("[[IMAGE_n]]"));
        assert!(task.prompt.contains("[[IMAGE_1]]"));
        assert!(!task.prompt.contains("<img"));
        assert_eq!(task.model.as_deref(), Some("haiku"));
        let notes = notes.unwrap();
        assert!(!notes.excerpt_only && !notes.truncated && notes.dropped_turns.is_none());
        let images = images.unwrap();
        assert_eq!(images.len(), 1);
        assert!(images[0].contains("media://localhost/abc"));
        // Without a target language the UI default (the system language) is the shell's fallback.
        let (task, _, _) = build_task(
            &action,
            Some(&article),
            assistant::AssistantKind::ClaudeCode,
            None,
            None,
        )
        .unwrap();
        assert!(task.system_prompt.contains("into English"));
    }

    #[test]
    fn ask_builds_a_task_with_the_question_history_and_the_dropped_count() {
        let article =
            detail("<p>The buses become electric until 2030 and that is the whole story here.</p>");
        let history: Vec<assistant::ask::Turn> = (0..10)
            .map(|i| assistant::ask::Turn {
                question: format!("Q{i}"),
                answer: format!("A{i}"),
            })
            .collect();
        let action = AssistantAction::Ask {
            article_id: 7,
            question: "Until when?".into(),
            history,
        };
        let (task, notes, images) = build_task(
            &action,
            Some(&article),
            assistant::AssistantKind::ClaudeCode,
            None,
            None,
        )
        .unwrap();
        assert!(images.is_none());
        assert!(task.prompt.contains("<<<FEEDRICO_DATA Article>>>"));
        assert!(
            task.prompt
                .contains("<<<FEEDRICO_DATA Conversation so far>>>")
        );
        assert!(task.prompt.contains("Q: Q9"));
        assert!(!task.prompt.contains("Q: Q3"));
        assert!(
            task.prompt
                .contains("<<<FEEDRICO_DATA Question>>>\nUntil when?")
        );
        assert!(task.system_prompt.contains("The Article doesn't say"));
        assert_eq!(notes.unwrap().dropped_turns, Some(4));
    }

    #[test]
    fn explain_term_builds_a_task_from_the_selection_and_its_paragraph_only() {
        use assistant::AssistantKind::ClaudeCode;
        use assistant::article::{MAX_CONTEXT_CHARS, MAX_TERM_CHARS};
        let article = detail("<p>The quota is a limit on requests. It resets daily.</p>");
        let action = AssistantAction::ExplainTerm {
            article_id: 7,
            term: "quota".into(),
            context: "The quota is a limit on requests. It resets daily.".into(),
        };
        let (task, notes, _) = build_task(&action, Some(&article), ClaudeCode, None, None).unwrap();
        assert_eq!(task.model.as_deref(), Some("haiku"));
        assert!(task.system_prompt.contains("plain language"));
        assert!(
            task.prompt
                .contains("<<<FEEDRICO_DATA Selected term>>>\nquota\n<<<END_FEEDRICO_DATA>>>")
        );
        assert!(task.prompt.contains(
            "<<<FEEDRICO_DATA Paragraph around the selection>>>\n\
                 The quota is a limit on requests. It resets daily.\n<<<END_FEEDRICO_DATA>>>"
        ));
        // The Article body itself is not part of the prompt.
        assert!(!task.prompt.contains("FEEDRICO_DATA Article"));
        assert!(notes.is_some());

        // Term and context are capped before they reach the CLI.
        let long = AssistantAction::ExplainTerm {
            article_id: 7,
            term: "x".repeat(MAX_TERM_CHARS + 100),
            context: "y".repeat(MAX_CONTEXT_CHARS + 100),
        };
        let (task, _, _) = build_task(&long, Some(&article), ClaudeCode, None, None).unwrap();
        let term = task
            .prompt
            .split("<<<FEEDRICO_DATA Selected term>>>\n")
            .nth(1)
            .unwrap()
            .split("\n<<<END_FEEDRICO_DATA>>>")
            .next()
            .unwrap();
        assert_eq!(term.chars().count(), MAX_TERM_CHARS);
        let context = task
            .prompt
            .split("<<<FEEDRICO_DATA Paragraph around the selection>>>\n")
            .nth(1)
            .unwrap()
            .split("\n<<<END_FEEDRICO_DATA>>>")
            .next()
            .unwrap();
        assert_eq!(context.chars().count(), MAX_CONTEXT_CHARS);

        // Without the Article there is nothing to anchor the selection to.
        let err = build_task(&action, None, ClaudeCode, None, None).unwrap_err();
        assert_eq!(err.kind, "notFound");
    }

    #[test]
    fn a_search_query_task_carries_only_the_question_and_the_folder_feed_names() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join(feedrico_core::library::DIR_NAME);
        let library = Library::create(&dir).unwrap();
        library
            .add_feed("https://a.example/feed", "Rust Blog", None, None)
            .unwrap();
        let state = state_with(Some(library));
        let names = search_query_names(&state).unwrap();
        assert!(names.folders.is_empty());
        assert_eq!(names.feeds, vec![(1, "Rust Blog".to_string())]);
        let task = assistant::search::task(
            "wal mode last spring",
            &names.folders,
            &names.feeds,
            "2026-11-14",
        );
        assert!(
            task.prompt
                .contains("<<<FEEDRICO_DATA Question>>>\nwal mode last spring")
        );
        assert!(task.prompt.contains("Rust Blog"));
        assert!(task.json_schema.is_some());

        let empty = state_with(None);
        assert_eq!(search_query_names(&empty).unwrap_err().kind, "noLibrary");
    }

    #[test]
    fn runs_need_a_setup_with_an_accepted_disclosure() {
        let setup = |accepted| assistant::AssistantSetup {
            kind: assistant::AssistantKind::ClaudeCode,
            path: "/usr/bin/claude".into(),
            model: Some("haiku".into()),
            disclosure_accepted: accepted,
            target_language: Some("de".into()),
        };
        assert_eq!(run_target(None).unwrap_err().kind, "notSetUp");
        assert_eq!(run_target(Some(setup(false))).unwrap_err().kind, "notSetUp");
        let (found, model) = run_target(Some(setup(true))).unwrap();
        assert_eq!(found.kind, assistant::AssistantKind::ClaudeCode);
        assert_eq!(found.path, std::path::PathBuf::from("/usr/bin/claude"));
        assert_eq!(model.as_deref(), Some("haiku"));
    }

    #[test]
    fn folder_suggestions_action_deserializes_camel_case_with_an_optional_folder() {
        let a: AssistantAction = serde_json::from_value(serde_json::json!({
            "kind": "folderSuggestions", "feedIds": [1, 2], "newFolders": ["Cooking"]
        }))
        .unwrap();
        let AssistantAction::FolderSuggestions {
            feed_ids,
            new_folders,
            folder_id,
        } = a
        else {
            panic!("expected FolderSuggestions")
        };
        assert_eq!(feed_ids, [1, 2]);
        assert_eq!(new_folders, ["Cooking"]);
        assert_eq!(folder_id, None, "folderId may be omitted");

        let a: AssistantAction = serde_json::from_value(serde_json::json!({
            "kind": "folderSuggestions", "feedIds": [], "newFolders": [], "folderId": 7
        }))
        .unwrap();
        let AssistantAction::FolderSuggestions { folder_id, .. } = a else {
            panic!("expected FolderSuggestions")
        };
        assert_eq!(folder_id, Some(7));

        let t: AssistantAction =
            serde_json::from_value(serde_json::json!({ "kind": "test" })).unwrap();
        assert!(matches!(t, AssistantAction::Test));
    }

    /// A Library with one sorted Feed in "Tech", one top-level Feed, and the Folder id.
    fn suggestions_library() -> (tempfile::TempDir, Library, i64, i64, i64) {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join(feedrico_core::library::DIR_NAME);
        let library = Library::create(&dir).unwrap();
        library
            .add_feed("https://a.example/feed", "A", None, None)
            .unwrap();
        library
            .add_feed("https://b.example/feed", "B", None, None)
            .unwrap();
        library
            .import_opml(&[feedrico_core::opml::OpmlFeed {
                url: "https://c.example/feed".into(),
                title: "C".into(),
                site_url: None,
                folder: Some("Tech".into()),
            }])
            .unwrap();
        let tech_id = library.sidebar(0).unwrap().folders[0].id;
        let sorted = library.sidebar(0).unwrap().feeds[0].id;
        library.move_feed(sorted, Some(tech_id)).unwrap();
        let free = library.sidebar(0).unwrap().feeds[0].id;
        (root, library, tech_id, sorted, free)
    }

    #[test]
    fn folder_suggestions_input_caps_the_batch_and_builds_the_task() {
        use assistant::folder_suggestions::BATCH_SIZE;
        let (_root, library, tech_id, sorted, free) = suggestions_library();

        // More than one batch per run is refused; the UI batches.
        let too_many: Vec<i64> = (0..(BATCH_SIZE as i64 + 1)).collect();
        assert!(matches!(
            folder_suggestions_input(&library, &too_many, &[], None),
            Err(feedrico_core::Error::Invalid(_))
        ));

        // Feeds outside the scope: nothing to ask.
        assert!(
            folder_suggestions_input(&library, &[sorted], &[], None)
                .unwrap()
                .is_none()
        );

        // Top-level scope: the task carries the Feeds, the existing Folders plus the new ones
        // earlier batches proposed.
        let (task, feeds, folders) = folder_suggestions_input(
            &library,
            &[sorted, free, 9999],
            &["Cooking".to_string()],
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].id, free);
        assert_eq!(folders, ["Tech", "Cooking"]);
        assert!(task.prompt.contains("Feeds without a Folder"));

        // Folder scope: only that Folder's Feeds are sent, and the prompt names the Folder.
        let (task, feeds, _) =
            folder_suggestions_input(&library, &[sorted, free], &[], Some(tech_id))
                .unwrap()
                .unwrap();
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].id, sorted);
        assert!(task.system_prompt.contains("Feeds of the Folder \"Tech\""));

        // An unknown Folder is a not-found, not an empty run.
        assert!(matches!(
            folder_suggestions_input(&library, &[free], &[], Some(9999)),
            Err(feedrico_core::Error::NotFound)
        ));
    }

    #[test]
    fn apply_and_undo_folder_moves_work_as_one_step_over_ipc() {
        // The commands are thin `with_library` wrappers; this exercises what they call.
        let (_root, library, _tech_id, _sorted, free) = suggestions_library();
        let state = state_with(Some(library));
        let moves = vec![feedrico_core::feeds::FolderMove {
            feed_id: free,
            folder: "Cooking".into(),
        }];
        let applied = with_library(&state, |l| l.apply_folder_moves(&moves, None)).unwrap();
        assert_eq!(applied.moved.len(), 1);
        assert_eq!(applied.created_folder_ids.len(), 1);
        with_library(&state, |l| {
            assert_eq!(l.folder_names().unwrap(), ["Tech", "Cooking"]);
            assert!(l.top_level_feeds(&[free]).unwrap().is_empty());
            Ok(())
        })
        .unwrap();

        with_library(&state, |l| l.undo_folder_moves(&applied)).unwrap();
        with_library(&state, |l| {
            assert_eq!(
                l.folder_names().unwrap(),
                ["Tech"],
                "the empty Folder is gone again"
            );
            assert_eq!(l.top_level_feeds(&[free]).unwrap().len(), 1);
            Ok(())
        })
        .unwrap();
    }
}
