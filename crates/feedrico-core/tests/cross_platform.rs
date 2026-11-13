//! A Library created on Linux must open unchanged on Windows and macOS (issue #5). The fixture
//! under `tests/fixtures/` is committed; CI opens it on all three platforms.
//!
//! The fixture is frozen at format 2 (created by build 0.1.0 on Linux) on purpose: every build
//! must upgrade an older Library on every OS. Don't regenerate it; add a new fixture when a
//! format version needs the same cross-platform proof.

use std::fs;
use std::path::{Path, PathBuf};

use feedrico_core::library::DIR_NAME;
use feedrico_core::{FORMAT_VERSION, Library, OpenOptions, Status};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/library")
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn fixture_library_from_linux_opens_here() {
    // Work on a copy so the committed fixture is never modified (opening writes the lock).
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join(DIR_NAME);
    copy_dir(&fixture_dir(), &dir);

    // The fixture is format 2 on purpose: every build must upgrade an older Library on every OS.
    assert!(matches!(
        feedrico_core::library::inspect(&dir).unwrap(),
        Status::NeedsUpgrade { found: 2, .. }
    ));
    assert!(matches!(
        Library::open(&dir, OpenOptions::default()),
        Err(feedrico_core::Error::NeedsUpgrade { .. })
    ));
    let library = Library::open(
        &dir,
        OpenOptions {
            allow_upgrade: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(library.format_version().unwrap(), FORMAT_VERSION);
    let (title, starred): (String, bool) = library
        .conn()
        .query_row(
            "SELECT a.title, a.is_starred FROM article a JOIN feed f ON f.id = a.feed_id
             WHERE f.title = 'Ünïcode Feed 日本語'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(title, "Hello from Linux");
    assert!(starred);
    library.close().unwrap();
}

/// Scenario C (docs/personas.md): no absolute paths anywhere inside a Library, so the same
/// folder opens identically wherever it lands (Linux Library on a Windows Host, a stick, …).
#[test]
fn the_library_contains_no_absolute_paths() {
    let manifest = fs::read_to_string(fixture_dir().join("library.json")).unwrap();
    let db = fs::read(fixture_dir().join("library.db")).unwrap();
    for needle in ["/home/", "/tmp/", "/Users/", "/private/"] {
        assert!(!manifest.contains(needle), "manifest mentions {needle}");
        assert!(
            !db.windows(needle.len()).any(|w| w == needle.as_bytes()),
            "database mentions {needle}"
        );
    }
}

/// After the one-time upgrade, the fixture is an ordinary same-version Library: it reopens
/// without another prompt and leaves no lock behind (scenarios A/C on the upgraded copy).
#[test]
fn the_upgraded_fixture_reopens_without_another_upgrade() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join(DIR_NAME);
    copy_dir(&fixture_dir(), &dir);
    let library = Library::open(
        &dir,
        OpenOptions {
            allow_upgrade: true,
            ..Default::default()
        },
    )
    .unwrap();
    library.close().unwrap();

    assert_eq!(
        feedrico_core::library::inspect(&dir).unwrap(),
        Status::Ready
    );
    assert!(!dir.join(".lock").exists(), "closing removes the lock");
    let library = Library::open(&dir, OpenOptions::default()).unwrap();
    assert_eq!(library.format_version().unwrap(), FORMAT_VERSION);
    library.close().unwrap();
}

/// Scenario D (docs/personas.md): a stick pulled mid-session leaves a lock that names the
/// machine and the time; "Open anyway" takes it over, and without consent nothing is written.
#[test]
fn a_stale_foreign_lock_is_reported_with_machine_and_time() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join(DIR_NAME);
    copy_dir(&fixture_dir(), &dir);
    let library = Library::open(
        &dir,
        OpenOptions {
            allow_upgrade: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(matches!(
        feedrico_core::library::inspect(&dir).unwrap(),
        Status::Locked(_)
    ));
    library.close().unwrap();

    fs::write(
        dir.join(".lock"),
        br#"{"machine":"friend-laptop","pid":4242,"openedAt":1762547640}"#,
    )
    .unwrap();
    match feedrico_core::library::inspect(&dir).unwrap() {
        Status::Locked(info) => {
            assert_eq!(info.machine, "friend-laptop");
            assert_eq!(info.opened_at, 1762547640);
        }
        other => panic!("expected Locked, got {other:?}"),
    }
    assert!(matches!(
        Library::open(&dir, OpenOptions::default()),
        Err(feedrico_core::Error::Locked(_))
    ));
    let library = Library::open(
        &dir,
        OpenOptions {
            take_over_lock: true,
            ..Default::default()
        },
    )
    .unwrap();
    library.close().unwrap();
    assert!(!dir.join(".lock").exists());
}
