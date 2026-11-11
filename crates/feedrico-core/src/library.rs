//! The Library: the self-contained folder holding all of a user's state (ADR 0001, ADR 0002).
//!
//! ```text
//! Feedrico Library/
//! ├── library.json   manifest: format version
//! ├── library.db     SQLite: Feeds, Folders, Articles, settings, search index
//! ├── media/         cached images and favicons, by hash of their source URL
//! └── .lock          present while open
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, MAIN_DB};
use serde::Serialize;

use crate::lock::{self, LockGuard, LockInfo};
use crate::manifest::{self, Manifest};
use crate::schema::{self, MIGRATIONS};
use crate::{Error, Result};

/// Default folder name for a new Library, and the name Portable Mode looks for next to the
/// executable.
pub const DIR_NAME: &str = "Feedrico Library";
pub const DB_FILE: &str = "library.db";
pub const MEDIA_DIR: &str = "media";

/// What opening a Library would run into, determined without changing anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Status {
    Ready,
    /// Opening requires a migration; older app versions can't read the Library afterwards.
    NeedsUpgrade {
        found: u32,
        target: u32,
    },
    /// Written by a newer app; this build must not touch it.
    TooNew {
        found: u32,
        supported: u32,
    },
    /// Open elsewhere, or left behind by a crash or a yanked stick.
    Locked(LockInfo),
    NotALibrary,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OpenOptions {
    /// The user agreed to migrate an older Library. A backup of the database is made first.
    pub allow_upgrade: bool,
    /// The user agreed to take over a lock they believe is stale.
    pub take_over_lock: bool,
}

#[derive(Debug)]
pub struct Library {
    dir: PathBuf,
    conn: Option<Connection>,
    _lock: LockGuard,
}

pub fn inspect(dir: &Path) -> Result<Status> {
    inspect_with(dir, MIGRATIONS)
}

fn inspect_with(dir: &Path, migrations: &[&str]) -> Result<Status> {
    let supported = migrations.len() as u32;
    if !dir.join(manifest::FILE_NAME).is_file() || !dir.join(DB_FILE).is_file() {
        return Ok(Status::NotALibrary);
    }
    let found = Manifest::read(dir)?.format_version;
    if found > supported {
        return Ok(Status::TooNew { found, supported });
    }
    if let Some(info) = lock::read(dir)? {
        return Ok(Status::Locked(info));
    }
    if found < supported {
        return Ok(Status::NeedsUpgrade {
            found,
            target: supported,
        });
    }
    Ok(Status::Ready)
}

impl Library {
    /// Creates a new Library at `dir`, which must not exist or be an empty directory.
    pub fn create(dir: &Path) -> Result<Self> {
        Self::create_with(dir, MIGRATIONS)
    }

    /// Opens an existing Library, refusing anything that could lose or corrupt data unless the
    /// user explicitly agreed through `options`.
    pub fn open(dir: &Path, options: OpenOptions) -> Result<Self> {
        Self::open_with(dir, options, MIGRATIONS)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn format_version(&self) -> Result<u32> {
        schema::version(self.conn())
    }

    pub fn conn(&self) -> &Connection {
        self.conn.as_ref().expect("connection lives until close")
    }

    /// Closes the Library and leaves the folder self-contained (no WAL side files), so it can be
    /// copied or unplugged right away.
    pub fn close(mut self) -> Result<()> {
        self.shutdown()
    }

    fn shutdown(&mut self) -> Result<()> {
        let Some(conn) = self.conn.take() else {
            return Ok(());
        };
        // Always close the connection, even if tidying up the journal fails.
        let tidy = conn
            .pragma_update(None, "wal_checkpoint", "TRUNCATE")
            .and_then(|()| conn.pragma_update(None, "journal_mode", "DELETE"));
        let closed = conn.close().map_err(|(_, e)| e);
        tidy?;
        closed?;
        Ok(())
    }

    fn create_with(dir: &Path, migrations: &[&str]) -> Result<Self> {
        if dir.exists() && fs::read_dir(dir)?.next().is_some() {
            return Err(Error::TargetNotEmpty(dir.to_path_buf()));
        }
        fs::create_dir_all(dir.join(MEDIA_DIR))?;
        let lock = LockGuard::acquire(dir, false)?;
        let mut conn = connect(dir)?;
        configure(&conn)?;
        let format_version = schema::migrate(&mut conn, migrations)?;
        Manifest {
            format_version,
            created_with: env!("CARGO_PKG_VERSION").to_owned(),
            created_at: lock::now(),
        }
        .write(dir)?;
        Ok(Self {
            dir: dir.to_path_buf(),
            conn: Some(conn),
            _lock: lock,
        })
    }

    fn open_with(dir: &Path, options: OpenOptions, migrations: &[&str]) -> Result<Self> {
        let supported = migrations.len() as u32;
        if !dir.join(manifest::FILE_NAME).is_file() || !dir.join(DB_FILE).is_file() {
            return Err(Error::NotALibrary(dir.to_path_buf()));
        }
        let mut manifest = Manifest::read(dir)?;
        let found = manifest.format_version;
        if found > supported {
            return Err(Error::TooNew { found, supported });
        }
        if found < supported && !options.allow_upgrade {
            return Err(Error::NeedsUpgrade {
                found,
                target: supported,
            });
        }

        let lock = LockGuard::acquire(dir, options.take_over_lock)?;
        let mut conn = connect(dir)?;

        // The database is the truth for migrations; the manifest may lag if a previous upgrade
        // was interrupted between the two writes.
        let db_version = schema::version(&conn)?;
        if db_version > supported {
            return Err(Error::TooNew {
                found: db_version,
                supported,
            });
        }
        if db_version < supported {
            if !options.allow_upgrade {
                return Err(Error::NeedsUpgrade {
                    found: db_version,
                    target: supported,
                });
            }
            let backup = dir.join(format!("{DB_FILE}.backup-v{db_version}"));
            conn.backup(MAIN_DB, &backup, None::<fn(rusqlite::backup::Progress)>)?;
        }
        // Only now change anything in the file: journal mode is persisted in the header.
        configure(&conn)?;
        if db_version < supported {
            schema::migrate(&mut conn, migrations)?;
        }
        if manifest.format_version != supported {
            manifest.format_version = supported;
            manifest.write(dir)?;
        }

        Ok(Self {
            dir: dir.to_path_buf(),
            conn: Some(conn),
            _lock: lock,
        })
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn connect(dir: &Path) -> Result<Connection> {
    let conn = Connection::open(dir.join(DB_FILE))?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}

fn configure(conn: &Connection) -> Result<()> {
    // WAL lets the UI read while a refresh writes. `close` checkpoints and switches back to
    // DELETE so the folder never depends on side files once closed.
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn create_close_reopen() {
        let root = tmp();
        let dir = root.path().join(DIR_NAME);
        let library = Library::create(&dir).unwrap();
        assert_eq!(library.format_version().unwrap(), crate::FORMAT_VERSION);
        assert!(dir.join(lock::FILE_NAME).exists());
        library.close().unwrap();

        assert!(!dir.join(lock::FILE_NAME).exists());
        assert!(
            !dir.join("library.db-wal").exists(),
            "closed Library must be self-contained"
        );
        assert_eq!(inspect(&dir).unwrap(), Status::Ready);

        let library = Library::open(&dir, OpenOptions::default()).unwrap();
        library.close().unwrap();
    }

    #[test]
    fn create_refuses_non_empty_dir() {
        let root = tmp();
        fs::write(root.path().join("something"), b"x").unwrap();
        assert!(matches!(
            Library::create(root.path()),
            Err(Error::TargetNotEmpty(_))
        ));
    }

    #[test]
    fn plain_folder_is_not_a_library() {
        let root = tmp();
        assert_eq!(inspect(root.path()).unwrap(), Status::NotALibrary);
        assert!(matches!(
            Library::open(root.path(), OpenOptions::default()),
            Err(Error::NotALibrary(_))
        ));
    }

    #[test]
    fn second_open_is_refused_until_lock_taken_over() {
        let root = tmp();
        let dir = root.path().join(DIR_NAME);
        let first = Library::create(&dir).unwrap();

        assert!(matches!(inspect(&dir).unwrap(), Status::Locked(_)));
        match Library::open(&dir, OpenOptions::default()) {
            Err(Error::Locked(info)) => assert_eq!(info.pid, std::process::id()),
            other => panic!("expected Locked, got {other:?}"),
        }

        first.close().unwrap();

        // A crash or yanked stick on another machine leaves its lock behind.
        fs::write(
            dir.join(lock::FILE_NAME),
            br#"{"machine":"friend-laptop","pid":4242,"openedAt":1762547640}"#,
        )
        .unwrap();
        match inspect(&dir).unwrap() {
            Status::Locked(info) => assert_eq!(info.machine, "friend-laptop"),
            other => panic!("expected Locked, got {other:?}"),
        }
        let taken = Library::open(
            &dir,
            OpenOptions {
                take_over_lock: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            lock::read(&dir).unwrap().unwrap().pid,
            std::process::id(),
            "taking over rewrites the lock"
        );
        taken.close().unwrap();
        assert!(!dir.join(lock::FILE_NAME).exists());
    }

    const V2_EXTRA: &str = "CREATE TABLE extra (x INTEGER);";

    #[test]
    fn older_library_needs_consent_and_gets_a_backup() {
        let root = tmp();
        let dir = root.path().join(DIR_NAME);
        Library::create_with(&dir, &MIGRATIONS[..1])
            .unwrap()
            .close()
            .unwrap();
        let newer: Vec<&str> = MIGRATIONS[..1].iter().copied().chain([V2_EXTRA]).collect();

        assert_eq!(
            inspect_with(&dir, &newer).unwrap(),
            Status::NeedsUpgrade {
                found: 1,
                target: 2
            }
        );
        assert!(matches!(
            Library::open_with(&dir, OpenOptions::default(), &newer),
            Err(Error::NeedsUpgrade {
                found: 1,
                target: 2
            })
        ));
        assert!(
            !dir.join(lock::FILE_NAME).exists(),
            "refusal must not leave a lock"
        );

        let upgraded = Library::open_with(
            &dir,
            OpenOptions {
                allow_upgrade: true,
                ..Default::default()
            },
            &newer,
        )
        .unwrap();
        assert_eq!(upgraded.format_version().unwrap(), 2);
        upgraded.close().unwrap();
        assert!(dir.join("library.db.backup-v1").is_file());
        assert_eq!(Manifest::read(&dir).unwrap().format_version, 2);
    }

    #[test]
    fn v1_libraries_upgrade_and_backfill_lead_images() {
        let root = tmp();
        let dir = root.path().join(DIR_NAME);
        let v1 = Library::create_with(&dir, &MIGRATIONS[..1]).unwrap();
        v1.conn()
            .execute_batch(
                "INSERT INTO feed (url, title) VALUES ('u', 'F');
                 INSERT INTO article (feed_id, identity, first_seen_at, article_date) VALUES (1, 'a', 0, 0);
                 INSERT INTO article (feed_id, identity, first_seen_at, article_date) VALUES (1, 'b', 0, 0);
                 INSERT INTO article_media (article_id, source_url) VALUES (1, 'https://x/first.png');
                 INSERT INTO article_media (article_id, source_url) VALUES (1, 'https://x/second.png');",
            )
            .unwrap();
        v1.close().unwrap();

        let upgraded = Library::open(
            &dir,
            OpenOptions {
                allow_upgrade: true,
                ..Default::default()
            },
        )
        .unwrap();
        let lead: Vec<Option<String>> = upgraded
            .conn()
            .prepare("SELECT lead_image FROM article ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            lead,
            vec![
                Some(crate::sanitize::media_key("https://x/first.png")),
                None
            ]
        );
        upgraded.close().unwrap();
    }

    #[test]
    fn v2_libraries_upgrade_to_an_unread_index_the_sidebar_uses() {
        let root = tmp();
        let dir = root.path().join(DIR_NAME);
        Library::create_with(&dir, &MIGRATIONS[..2])
            .unwrap()
            .close()
            .unwrap();
        assert_eq!(
            inspect(&dir).unwrap(),
            Status::NeedsUpgrade {
                found: 2,
                target: 3
            }
        );

        let upgraded = Library::open(
            &dir,
            OpenOptions {
                allow_upgrade: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(dir.join("library.db.backup-v2").is_file());
        let plan: String = upgraded
            .conn()
            .query_row(
                "EXPLAIN QUERY PLAN SELECT feed_id, count(*) FROM article WHERE is_read = 0 GROUP BY feed_id",
                [],
                |r| r.get(3),
            )
            .unwrap();
        assert!(plan.contains("article_feed_unread"), "plan was: {plan}");
        upgraded.close().unwrap();
    }

    #[test]
    fn newer_library_is_never_touched() {
        let root = tmp();
        let dir = root.path().join(DIR_NAME);
        let newer: Vec<&str> = MIGRATIONS[..1].iter().copied().chain([V2_EXTRA]).collect();
        Library::create_with(&dir, &newer).unwrap().close().unwrap();
        let before = fs::read(dir.join(DB_FILE)).unwrap();

        assert_eq!(
            inspect_with(&dir, &MIGRATIONS[..1]).unwrap(),
            Status::TooNew {
                found: 2,
                supported: 1
            }
        );
        assert!(matches!(
            Library::open_with(
                &dir,
                OpenOptions {
                    allow_upgrade: true,
                    take_over_lock: true
                },
                &MIGRATIONS[..1]
            ),
            Err(Error::TooNew {
                found: 2,
                supported: 1
            })
        ));
        assert_eq!(fs::read(dir.join(DB_FILE)).unwrap(), before);
        assert!(!dir.join(lock::FILE_NAME).exists());
    }

    #[test]
    fn library_folder_is_relocatable() {
        let root = tmp();
        let home = root.path().join("home").join(DIR_NAME);
        let library = Library::create(&home).unwrap();
        library
            .conn()
            .execute(
                "INSERT INTO feed (url, title) VALUES ('https://example.org/feed.xml', 'Example')",
                [],
            )
            .unwrap();
        library.close().unwrap();

        // Carry it to the "friend's machine": a plain folder move, like copying to a stick.
        let stick = root.path().join("stick").join(DIR_NAME);
        fs::create_dir_all(stick.parent().unwrap()).unwrap();
        fs::rename(&home, &stick).unwrap();

        let library = Library::open(&stick, OpenOptions::default()).unwrap();
        let title: String = library
            .conn()
            .query_row("SELECT title FROM feed", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "Example");
        library.close().unwrap();
    }

    #[test]
    fn full_text_index_follows_articles() {
        let root = tmp();
        let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
        let conn = library.conn();
        conn.execute("INSERT INTO feed (url, title) VALUES ('u', 'F')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO article (feed_id, identity, title, content_text, first_seen_at, article_date)
             VALUES (1, 'a', 'Altweibersommer', 'Spinnfäden im Wind', 0, 0)",
            [],
        )
        .unwrap();
        let hits = |q: &str| -> i64 {
            conn.query_row(
                "SELECT count(*) FROM article_fts WHERE article_fts MATCH ?1",
                [q],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(hits("spinnfaden"), 1, "diacritics are folded");
        conn.execute("DELETE FROM article", []).unwrap();
        assert_eq!(hits("spinnfaden"), 0);
        library.close().unwrap();
    }
}
