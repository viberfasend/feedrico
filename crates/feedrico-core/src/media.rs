//! The media cache in the Library's `media/` folder (spec: "Image cache").
//! Files are named by `sanitize::media_key` of their source URL.

use std::fs;
use std::path::PathBuf;

use rusqlite::{OptionalExtension, params};

use crate::library::MEDIA_DIR;
use crate::lock::now;
use crate::sanitize::media_key;
use crate::{Library, Result};

/// A key is exactly what `media_key` produces; anything else (e.g. `../`) is rejected.
pub fn is_valid_key(key: &str) -> bool {
    key.len() == 32 && key.bytes().all(|b| b.is_ascii_hexdigit())
}

impl Library {
    pub fn store_media(&self, source_url: &str, bytes: &[u8], mime_type: &str) -> Result<()> {
        let key = media_key(source_url);
        let dir = self.dir().join(MEDIA_DIR);
        let tmp = dir.join(format!("{key}.tmp"));
        fs::write(&tmp, bytes)?;
        fs::rename(&tmp, dir.join(&key))?;
        let tx = self.conn().unchecked_transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO media (hash, mime_type, bytes, stored_at) VALUES (?1, ?2, ?3, ?4)",
            params![key, mime_type, bytes.len() as i64, now()],
        )?;
        tx.execute(
            "UPDATE article_media SET hash = ?1 WHERE source_url = ?2",
            params![key, source_url],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Lead Images referenced by Articles but not cached yet (download failed or happened before
    /// Lead Images existed), newest Articles first.
    pub fn missing_lead_images(&self, limit: usize) -> Result<Vec<String>> {
        // Keys can't be computed in SQL, so match source URLs to Lead Image keys here.
        let mut stmt = self.conn().prepare(
            "SELECT a.lead_image, am.source_url FROM article a
             JOIN article_media am ON am.article_id = a.id
             WHERE a.lead_image IS NOT NULL AND am.hash IS NULL
               AND NOT EXISTS (SELECT 1 FROM media m WHERE m.hash = a.lead_image)
             ORDER BY a.article_date DESC",
        )?;
        let mut seen = std::collections::HashSet::new();
        let mut urls = Vec::new();
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (key, url) = row?;
            if media_key(&url) == key && seen.insert(key) {
                urls.push(url);
                if urls.len() == limit {
                    break;
                }
            }
        }
        Ok(urls)
    }

    /// Deletes cached files that no Article refers to (neither as an image nor as a Lead Image),
    /// and their `media` rows.
    pub fn remove_unreferenced_media(&self) -> Result<()> {
        let mut stmt = self.conn().prepare(
            "SELECT hash FROM media m
             WHERE NOT EXISTS (SELECT 1 FROM article_media am WHERE am.source_url IS NOT NULL AND am.hash = m.hash)
               AND NOT EXISTS (SELECT 1 FROM article a WHERE a.lead_image = m.hash)",
        )?;
        let orphans: Vec<String> = stmt
            .query_map([], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        drop(stmt);
        let dir = self.dir().join(MEDIA_DIR);
        for hash in &orphans {
            if is_valid_key(hash) {
                match fs::remove_file(dir.join(hash)) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.into()),
                }
            }
        }
        let tx = self.conn().unchecked_transaction()?;
        for hash in &orphans {
            tx.execute("DELETE FROM media WHERE hash = ?1", [hash])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Path and MIME type of a cached file, if present.
    pub fn media_file(&self, key: &str) -> Result<Option<(PathBuf, String)>> {
        if !is_valid_key(key) {
            return Ok(None);
        }
        let mime: Option<String> = self
            .conn()
            .query_row("SELECT mime_type FROM media WHERE hash = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(mime.map(|m| (self.dir().join(MEDIA_DIR).join(key), m)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::DIR_NAME;

    #[test]
    fn stores_and_serves_media_but_rejects_traversal() {
        let root = tempfile::tempdir().unwrap();
        let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
        let url = "https://x.example/a.png";
        library.store_media(url, b"\x89PNG", "image/png").unwrap();
        let (path, mime) = library.media_file(&media_key(url)).unwrap().unwrap();
        assert_eq!(fs::read(path).unwrap(), b"\x89PNG");
        assert_eq!(mime, "image/png");
        assert!(library.media_file("../library.db").unwrap().is_none());

        // A Lead Image that failed to download shows up as missing until it's stored.
        library
            .conn()
            .execute_batch(&format!(
                "INSERT INTO feed (url, title) VALUES ('u', 'F');
                 INSERT INTO article (feed_id, identity, first_seen_at, article_date, lead_image)
                   VALUES (1, 'a', 0, 0, '{}');
                 INSERT INTO article_media (article_id, source_url) VALUES (1, 'https://x.example/b.png');
                 INSERT INTO article_media (article_id, source_url) VALUES (1, 'https://x.example/c.png');",
                media_key("https://x.example/b.png")
            ))
            .unwrap();
        assert_eq!(
            library.missing_lead_images(10).unwrap(),
            vec!["https://x.example/b.png"]
        );
        library
            .store_media("https://x.example/b.png", b"GIF8", "image/gif")
            .unwrap();
        assert!(library.missing_lead_images(10).unwrap().is_empty());
        assert!(library.media_file(&"0".repeat(32)).unwrap().is_none());
        library.close().unwrap();
    }
}
