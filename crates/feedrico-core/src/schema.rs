//! Database migrations. The Library format version equals the number of migrations applied,
//! stored both in `PRAGMA user_version` and in `library.json`.
//!
//! Never edit a released migration: Libraries in the wild were built from it. Append a new one.

use rusqlite::Connection;

use crate::Result;

pub(crate) const MIGRATIONS: &[&str] = &[V1, V2, V3];

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
