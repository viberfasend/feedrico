//! The Tauri shell: a thin layer that exposes `feedrico-core` to the UI as commands, runs the
//! background refresh, and serves cached media. No domain logic lives here.

mod machine;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use feedrico_core::feeds::ImportSummary;
use feedrico_core::{AddOutcome, Discovery};
use feedrico_core::{
    ArticleDetail, ArticleRow, Fetcher, Layout, Layouts, Library, ListQuery, OpenOptions, Sidebar,
    Status, opml,
};
use futures::StreamExt;
use serde::Serialize;
use tauri::http::{Response, StatusCode, header};
use tauri::{AppHandle, Emitter, Manager, State};
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
    machine::remember(app, library.dir());
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
/// welcome screen.
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
    let candidates = machine::portable_library()
        .into_iter()
        .chain(machine::recent(&app));
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

/// Opens an http(s) link in the system browser. Nothing else is allowed through.
#[tauri::command]
fn open_external(url: String, app: AppHandle) -> CommandResult<()> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
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
fn navigation_guard<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("navigation-guard")
        .on_navigation(|_, url| {
            matches!(url.scheme(), "tauri")
                || matches!(url.host_str(), Some("tauri.localhost") | Some("localhost"))
        })
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let fetcher = Fetcher::new(MEDIA_PREFIX).expect("HTTP client");
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(navigation_guard())
        .manage(AppState {
            library: Mutex::new(None),
            fetcher,
            refreshing: AtomicBool::new(false),
        })
        .register_uri_scheme_protocol("media", |ctx, request| {
            serve_media(ctx.app_handle(), request.uri().path())
        })
        .invoke_handler(tauri::generate_handler![
            startup,
            recent_libraries,
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
            open_external,
            refresh,
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
