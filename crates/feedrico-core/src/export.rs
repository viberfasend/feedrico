//! "Export / back up Library": a consistent copy of the open Library in another folder.
//!
//! The database is copied first through SQLite's online backup API, so the snapshot is
//! consistent even while the app writes. Media follows (files are immutable and named by content
//! hash, so a copy is always whole), and `library.json` is written last: until it exists the
//! target is not a Library, so a half-finished export can never be opened by mistake. No `.lock`
//! is copied; the result opens like any closed Library.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, MAIN_DB};
use serde::Serialize;

use crate::library::{DB_FILE, Library, MEDIA_DIR};
use crate::manifest::{self, Manifest};
use crate::{Error, Result};

/// Reported after the database snapshot and after each media file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProgress {
    /// Media files copied so far.
    pub done: usize,
    /// Media files to copy.
    pub total: usize,
}

/// An export whose database snapshot is already taken. It holds no reference to the open
/// Library, so the slow media copy can run without blocking it.
#[derive(Debug)]
pub struct Export {
    source: PathBuf,
    target: PathBuf,
    manifest: Manifest,
}

impl Library {
    /// Snapshots the database into `target` (which must not exist or be empty) and returns the
    /// rest of the work.
    pub fn begin_export(&self, target: &Path) -> Result<Export> {
        if target.exists() && fs::read_dir(target)?.next().is_some() {
            return Err(Error::TargetNotEmpty(target.to_path_buf()));
        }
        if is_inside(target, self.dir()) {
            return Err(Error::Invalid(
                "the export folder can't be inside the Library itself".into(),
            ));
        }
        let mut manifest = Manifest::read(self.dir())?;
        manifest.format_version = self.format_version()?;

        fs::create_dir_all(target.join(MEDIA_DIR))?;
        let snapshot = snapshot_db(self.conn(), &target.join(DB_FILE));
        if let Err(e) = snapshot {
            cleanup(target);
            return Err(e);
        }
        Ok(Export {
            source: self.dir().to_path_buf(),
            target: target.to_path_buf(),
            manifest,
        })
    }

    /// Exports in one go, for callers that don't mind holding the Library meanwhile.
    pub fn export_to(&self, target: &Path, progress: impl FnMut(ExportProgress)) -> Result<()> {
        self.begin_export(target)?.finish(progress)
    }
}

impl Export {
    /// Copies the media and writes `library.json`. On failure the partial export is removed.
    pub fn finish(self, mut progress: impl FnMut(ExportProgress)) -> Result<()> {
        let result = self.copy_media(&mut progress).and_then(|()| {
            self.manifest.write(&self.target)?;
            Ok(())
        });
        if result.is_err() {
            cleanup(&self.target);
        }
        result
    }

    fn copy_media(&self, progress: &mut impl FnMut(ExportProgress)) -> Result<()> {
        let mut files = Vec::new();
        for entry in fs::read_dir(self.source.join(MEDIA_DIR))? {
            let entry = entry?;
            let name = entry.file_name();
            // Skip downloads in flight.
            if entry.file_type()?.is_file() && !name.to_string_lossy().ends_with(".tmp") {
                files.push(name);
            }
        }
        let total = files.len();
        progress(ExportProgress { done: 0, total });
        for (i, name) in files.iter().enumerate() {
            let from = self.source.join(MEDIA_DIR).join(name);
            match fs::copy(&from, self.target.join(MEDIA_DIR).join(name)) {
                Ok(_) => {}
                // Evicted since we listed the folder; nothing references it any more.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
            progress(ExportProgress { done: i + 1, total });
        }
        Ok(())
    }
}

fn snapshot_db(conn: &Connection, to: &Path) -> Result<()> {
    conn.backup(MAIN_DB, to, None::<fn(rusqlite::backup::Progress)>)?;
    // Leave the copy self-contained, like a closed Library.
    let copy = Connection::open(to)?;
    copy.pragma_update(None, "journal_mode", "DELETE")?;
    copy.close().map_err(|(_, e)| e)?;
    Ok(())
}

fn cleanup(target: &Path) {
    let _ = fs::remove_file(target.join(manifest::FILE_NAME));
    let _ = fs::remove_file(target.join(DB_FILE));
    let _ = fs::remove_dir_all(target.join(MEDIA_DIR));
}

fn is_inside(target: &Path, dir: &Path) -> bool {
    let abs = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    // The target may not exist yet: resolve its nearest existing ancestor.
    let mut existing = target;
    let mut rest = Vec::new();
    while !existing.exists() {
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name);
                existing = parent;
            }
            _ => break,
        }
    }
    let mut resolved = abs(existing);
    resolved.extend(rest.iter().rev());
    resolved.starts_with(abs(dir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{DIR_NAME, OpenOptions};
    use crate::lock;

    fn open_with_content(root: &Path) -> Library {
        let library = Library::create(&root.join(DIR_NAME)).unwrap();
        library
            .add_feed("https://example.com/feed.xml", "Example", None, None)
            .unwrap();
        fs::write(library.dir().join(MEDIA_DIR).join("aaa"), b"one").unwrap();
        fs::write(library.dir().join(MEDIA_DIR).join("bbb"), b"two").unwrap();
        library
    }

    #[test]
    fn snapshot_opens_as_a_normal_library_with_the_same_content() {
        let root = tempfile::tempdir().unwrap();
        let library = open_with_content(root.path());
        let target = root.path().join("backup");
        library.export_to(&target, |_| {}).unwrap();

        // Still open, still usable.
        assert_eq!(library.feed_count().unwrap(), 1);
        assert!(!target.join(lock::FILE_NAME).exists(), "no lock copied");
        assert!(!target.join("library.db-wal").exists());
        assert_eq!(
            crate::library::inspect(&target).unwrap(),
            crate::Status::Ready
        );

        let copy = Library::open(&target, OpenOptions::default()).unwrap();
        assert_eq!(copy.feed_count().unwrap(), 1);
        assert_eq!(copy.format_version().unwrap(), crate::FORMAT_VERSION);
        assert_eq!(
            fs::read(target.join(MEDIA_DIR).join("aaa")).unwrap(),
            b"one"
        );
        assert_eq!(
            fs::read(target.join(MEDIA_DIR).join("bbb")).unwrap(),
            b"two"
        );
    }

    #[test]
    fn accepts_an_empty_existing_folder() {
        let root = tempfile::tempdir().unwrap();
        let library = open_with_content(root.path());
        let target = root.path().join("empty");
        fs::create_dir(&target).unwrap();
        library.export_to(&target, |_| {}).unwrap();
        assert!(target.join(manifest::FILE_NAME).is_file());
    }

    #[test]
    fn refuses_a_non_empty_target_and_leaves_it_alone() {
        let root = tempfile::tempdir().unwrap();
        let library = open_with_content(root.path());
        let target = root.path().join("full");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("mine.txt"), b"keep").unwrap();
        assert!(matches!(
            library.export_to(&target, |_| {}),
            Err(Error::TargetNotEmpty(_))
        ));
        assert_eq!(fs::read(target.join("mine.txt")).unwrap(), b"keep");
        assert!(!target.join(DB_FILE).exists());
    }

    #[test]
    fn refuses_a_target_inside_the_library() {
        let root = tempfile::tempdir().unwrap();
        let library = open_with_content(root.path());
        let target = library.dir().join("nested").join("backup");
        assert!(matches!(
            library.export_to(&target, |_| {}),
            Err(Error::Invalid(_))
        ));
    }

    #[test]
    fn reports_progress_per_media_file() {
        let root = tempfile::tempdir().unwrap();
        let library = open_with_content(root.path());
        fs::write(library.dir().join(MEDIA_DIR).join("ccc.tmp"), b"half").unwrap();
        let mut seen = Vec::new();
        library
            .export_to(&root.path().join("backup"), |p| seen.push(p))
            .unwrap();
        assert_eq!(seen.first(), Some(&ExportProgress { done: 0, total: 2 }));
        assert_eq!(seen.last(), Some(&ExportProgress { done: 2, total: 2 }));
        assert_eq!(seen.len(), 3);
        assert!(!root.path().join("backup/media/ccc.tmp").exists());
    }

    #[test]
    fn manifest_appears_only_when_the_export_is_complete() {
        let root = tempfile::tempdir().unwrap();
        let library = open_with_content(root.path());
        let target = root.path().join("backup");
        let export = library.begin_export(&target).unwrap();
        assert_eq!(
            crate::library::inspect(&target).unwrap(),
            crate::Status::NotALibrary
        );
        export.finish(|_| {}).unwrap();
        assert_eq!(
            crate::library::inspect(&target).unwrap(),
            crate::Status::Ready
        );
    }
}
