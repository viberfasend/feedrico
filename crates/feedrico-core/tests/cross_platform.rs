//! A Library created on Linux must open unchanged on Windows and macOS (issue #5). The fixture
//! under `tests/fixtures/` is committed; CI opens it on all three platforms.
//!
//! Regenerate after a format change (and commit the result):
//! `cargo test -p feedrico-core --test cross_platform -- --ignored regenerate_fixture`

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

    assert!(matches!(
        feedrico_core::library::inspect(&dir).unwrap(),
        Status::Ready
    ));
    let library = Library::open(&dir, OpenOptions::default()).unwrap();
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

#[test]
#[ignore = "regenerates the committed fixture"]
fn regenerate_fixture() {
    let dir = fixture_dir();
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    let library = Library::create(&dir).unwrap();
    let conn = library.conn();
    conn.execute("INSERT INTO folder (name) VALUES ('Tech')", [])
        .unwrap();
    conn.execute(
        "INSERT INTO feed (url, title, folder_id) VALUES ('https://example.org/feed.xml', 'Ünïcode Feed 日本語', 1)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO article (feed_id, identity, title, first_seen_at, article_date, is_starred)
         VALUES (1, 'a1', 'Hello from Linux', 1700000000, 1700000000, 1)",
        [],
    )
    .unwrap();
    library.close().unwrap();
    // Git doesn't track empty folders; an empty media/ is recreated on demand.
    fs::write(dir.join("media/.gitkeep"), "").unwrap();
}
