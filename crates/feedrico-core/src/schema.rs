//! Database migrations. The Library format version equals the number of migrations applied,
//! stored both in `PRAGMA user_version` and in `library.json`.
//!
//! Never edit a released migration: Libraries in the wild were built from it. Append a new one.

use rusqlite::Connection;

use crate::Result;

pub(crate) const MIGRATIONS: &[&str] = &[V1, V2, V3, V4, V5];

const V1: &str = r#"
CREATE TABLE folder (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL,
    position    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE feed (
    id                INTEGER PRIMARY KEY,
    url               TEXT    NOT NULL UNIQUE,
    title             TEXT    NOT NULL,
    site_url          TEXT,
    folder_id         INTEGER REFERENCES folder(id) ON DELETE SET NULL,
    position          INTEGER NOT NULL DEFAULT 0,
    favicon_hash      TEXT,
    etag              TEXT,
    last_modified     TEXT,
    last_fetched_at   INTEGER,
    last_success_at   INTEGER,
    last_error        TEXT,
    error_count       INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX feed_folder ON feed(folder_id);

-- article_date: see "Article Date" in GLOSSARY.md. Times are unix seconds.
CREATE TABLE article (
    id              INTEGER PRIMARY KEY,
    feed_id         INTEGER NOT NULL REFERENCES feed(id) ON DELETE CASCADE,
    identity        TEXT    NOT NULL,
    url             TEXT,
    title           TEXT    NOT NULL DEFAULT '',
    author          TEXT,
    content_html    TEXT    NOT NULL DEFAULT '',
    content_text    TEXT    NOT NULL DEFAULT '',
    excerpt         TEXT    NOT NULL DEFAULT '',
    published_at    INTEGER,
    first_seen_at   INTEGER NOT NULL,
    article_date    INTEGER NOT NULL,
    is_read         INTEGER NOT NULL DEFAULT 0,
    is_starred      INTEGER NOT NULL DEFAULT 0,
    UNIQUE (feed_id, identity)
);
CREATE INDEX article_feed_date    ON article(feed_id, article_date DESC);
CREATE INDEX article_date_all     ON article(article_date DESC);
CREATE INDEX article_unread_date  ON article(article_date DESC) WHERE is_read = 0;
CREATE INDEX article_starred_date ON article(article_date DESC) WHERE is_starred = 1;

CREATE TABLE enclosure (
    id          INTEGER PRIMARY KEY,
    article_id  INTEGER NOT NULL REFERENCES article(id) ON DELETE CASCADE,
    url         TEXT    NOT NULL,
    mime_type   TEXT,
    length      INTEGER
);
CREATE INDEX enclosure_article ON enclosure(article_id);

-- Files in media/, addressed by content hash. Paths are always relative to the Library.
CREATE TABLE media (
    hash        TEXT    PRIMARY KEY,
    mime_type   TEXT    NOT NULL,
    bytes       INTEGER NOT NULL,
    stored_at   INTEGER NOT NULL
);

CREATE TABLE article_media (
    article_id  INTEGER NOT NULL REFERENCES article(id) ON DELETE CASCADE,
    source_url  TEXT    NOT NULL,
    hash        TEXT    REFERENCES media(hash) ON DELETE SET NULL,
    PRIMARY KEY (article_id, source_url)
);
CREATE INDEX article_media_hash ON article_media(hash);

-- Library settings (they travel with the Library). Values are JSON.
CREATE TABLE setting (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL
);

CREATE VIRTUAL TABLE article_fts USING fts5(
    title,
    content_text,
    content = 'article',
    content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2'
);
CREATE TRIGGER article_fts_insert AFTER INSERT ON article BEGIN
    INSERT INTO article_fts(rowid, title, content_text) VALUES (new.id, new.title, new.content_text);
END;
CREATE TRIGGER article_fts_delete AFTER DELETE ON article BEGIN
    INSERT INTO article_fts(article_fts, rowid, title, content_text)
        VALUES ('delete', old.id, old.title, old.content_text);
END;
CREATE TRIGGER article_fts_update AFTER UPDATE OF title, content_text ON article BEGIN
    INSERT INTO article_fts(article_fts, rowid, title, content_text)
        VALUES ('delete', old.id, old.title, old.content_text);
    INSERT INTO article_fts(rowid, title, content_text) VALUES (new.id, new.title, new.content_text);
END;
"#;

/// Lead Image per Article (see GLOSSARY.md), stored as its media key.
const V2: &str = r#"
ALTER TABLE article ADD COLUMN lead_image TEXT;
CREATE INDEX article_lead_date ON article(article_date DESC) WHERE lead_image IS NOT NULL;
"#;

/// Unread counts per Feed (the sidebar) read this index instead of scanning every Article.
const V3: &str = r#"
CREATE INDEX article_feed_unread ON article(feed_id) WHERE is_read = 0;
"#;

/// Enclosure metadata: play time in seconds, readable kind (audio|video|document), and the
/// media:player page when the URL is not a direct media file.
const V4: &str = r#"
ALTER TABLE enclosure ADD COLUMN duration_secs INTEGER;
ALTER TABLE enclosure ADD COLUMN kind TEXT NOT NULL DEFAULT 'document';
ALTER TABLE enclosure ADD COLUMN player_url TEXT;
"#;

/// Video metadata per Article (JSON, see `parse::ParsedVideo`): platform, video id, duration,
/// view count, thumbnail source. Null when the Article is not about a video. Existing video
/// Feed Articles (`yt:video:` entry ids) get their identity backfilled.
const V5: &str = r#"
ALTER TABLE article ADD COLUMN video TEXT;
UPDATE article SET video = json_object('host', 'youtube', 'id', substr(identity, 10))
 WHERE substr(identity, 1, 9) = 'yt:video:';
"#;

/// Existing Articles get the first image of their content as Lead Image.
fn backfill_lead_images(conn: &Connection) -> Result<()> {
    let firsts: Vec<(i64, String)> = conn
        .prepare(
            "SELECT article_id, source_url FROM article_media
             WHERE rowid IN (SELECT min(rowid) FROM article_media GROUP BY article_id)",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut update = conn.prepare("UPDATE article SET lead_image = ?2 WHERE id = ?1")?;
    for (article_id, url) in firsts {
        update.execute(rusqlite::params![
            article_id,
            crate::sanitize::media_key(&url)
        ])?;
    }
    Ok(())
}

pub(crate) fn version(conn: &Connection) -> Result<u32> {
    Ok(conn.pragma_query_value(None, "user_version", |row| row.get(0))?)
}

/// Applies every migration past the database's current version, each in its own transaction.
pub(crate) fn migrate(conn: &mut Connection, migrations: &[&str]) -> Result<u32> {
    let current = version(conn)? as usize;
    for (index, sql) in migrations.iter().enumerate().skip(current) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        if *sql == V2 {
            backfill_lead_images(&tx)?;
        }
        tx.pragma_update(None, "user_version", (index + 1) as u32)?;
        tx.commit()?;
    }
    Ok(migrations.len() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    fn columns(conn: &Connection, table: &str) -> Vec<String> {
        conn.prepare(&format!("PRAGMA table_info({table})"))
            .unwrap()
            .query_map([], |r| r.get(1))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    fn object_names(conn: &Connection) -> Vec<String> {
        conn.prepare("SELECT name FROM sqlite_master WHERE type IN ('table', 'index')")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    fn a_fresh_database_is_at_version_zero() {
        assert_eq!(version(&conn()).unwrap(), 0);
    }

    #[test]
    fn migrating_applies_every_step_and_sets_user_version() {
        let mut c = conn();
        let applied = migrate(&mut c, MIGRATIONS).unwrap();
        assert_eq!(applied, MIGRATIONS.len() as u32);
        assert_eq!(version(&c).unwrap(), MIGRATIONS.len() as u32);

        let objects = object_names(&c);
        for expected in [
            "folder",
            "feed",
            "article",
            "enclosure",
            "media",
            "article_media",
            "setting",
            "article_fts",
            "article_feed_unread",
        ] {
            assert!(objects.iter().any(|o| o == expected), "missing {expected}");
        }
        // V2 added the Lead Image column; V1 alone must not have it.
        assert!(columns(&c, "article").iter().any(|c| c == "lead_image"));
    }

    #[test]
    fn migrations_are_idempotent() {
        let mut c = conn();
        migrate(&mut c, MIGRATIONS).unwrap();
        assert_eq!(
            migrate(&mut c, MIGRATIONS).unwrap(),
            MIGRATIONS.len() as u32
        );
        assert_eq!(version(&c).unwrap(), MIGRATIONS.len() as u32);
    }

    #[test]
    fn a_partial_migration_stops_at_the_requested_version() {
        let mut c = conn();
        migrate(&mut c, &MIGRATIONS[..1]).unwrap();
        assert_eq!(version(&c).unwrap(), 1);
        assert!(!columns(&c, "article").iter().any(|c| c == "lead_image"));
        assert!(!object_names(&c).iter().any(|o| o == "article_feed_unread"));

        // Continuing with the full list picks up exactly where it stopped.
        migrate(&mut c, MIGRATIONS).unwrap();
        assert_eq!(version(&c).unwrap(), MIGRATIONS.len() as u32);
        assert!(columns(&c, "article").iter().any(|c| c == "lead_image"));
    }

    #[test]
    fn a_failing_migration_leaves_the_completed_steps_intact() {
        let mut c = conn();
        let broken = [MIGRATIONS[0], "THIS IS NOT SQL"];
        assert!(migrate(&mut c, &broken).is_err());
        assert_eq!(
            version(&c).unwrap(),
            1,
            "the completed step stays committed, the failed one rolls back"
        );
    }

    /// The V2 backfill: existing Articles get the first cached image as Lead Image. The "first"
    /// is the lowest rowid, i.e. insertion order, not URL order.
    #[test]
    fn v2_backfills_the_lead_image_from_the_first_inserted_image() {
        let mut c = conn();
        migrate(&mut c, &MIGRATIONS[..1]).unwrap();
        c.execute_batch(
            "INSERT INTO feed (url, title) VALUES ('u', 'F');
             INSERT INTO article (feed_id, identity, first_seen_at, article_date) VALUES (1, 'a', 0, 0);
             INSERT INTO article (feed_id, identity, first_seen_at, article_date) VALUES (1, 'b', 0, 0);
             INSERT INTO article_media (article_id, source_url) VALUES (1, 'https://x/zzz.png');
             INSERT INTO article_media (article_id, source_url) VALUES (1, 'https://x/aaa.png');",
        )
        .unwrap();

        migrate(&mut c, MIGRATIONS).unwrap();
        let leads: Vec<Option<String>> = c
            .prepare("SELECT lead_image FROM article ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            leads,
            vec![Some(crate::sanitize::media_key("https://x/zzz.png")), None]
        );

        /// The V5 backfill: existing `yt:video:` Articles get their video identity backfilled.
        #[test]
        fn v5_adds_the_video_column_and_backfills_yt_video_identities() {
            let mut c = conn();
            migrate(&mut c, &MIGRATIONS[..1]).unwrap();
            c.execute_batch(
            "INSERT INTO feed (url, title) VALUES ('https://www.youtube.com/feeds/videos.xml?channel_id=UCx', 'YT');
             INSERT INTO article (feed_id, identity, title, first_seen_at, article_date)
               VALUES (1, 'yt:video:0A1ouV7iD8o', 'A video', 0, 0);
             INSERT INTO article (feed_id, identity, title, first_seen_at, article_date)
               VALUES (1, 'post-1', 'A post', 0, 0);",
        )
        .unwrap();

            assert_eq!(
                migrate(&mut c, MIGRATIONS).unwrap(),
                MIGRATIONS.len() as u32
            );

            let video: Option<String> = c
                .query_row(
                    "SELECT video FROM article WHERE identity = 'yt:video:0A1ouV7iD8o'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&video.unwrap()).unwrap();
            assert_eq!(parsed["host"], "youtube");
            assert_eq!(parsed["id"], "0A1ouV7iD8o");
            let none: Option<String> = c
                .query_row(
                    "SELECT video FROM article WHERE identity = 'post-1'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(none.is_none(), "plain Articles stay without video metadata");
        }
    }
}
