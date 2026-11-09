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
