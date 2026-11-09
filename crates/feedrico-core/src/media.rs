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
    /// Read, unstarred Articles are skipped: their images may have been evicted on purpose.
    pub fn missing_lead_images(&self, limit: usize) -> Result<Vec<String>> {
        // Keys can't be computed in SQL, so match source URLs to Lead Image keys here.
        let mut stmt = self.conn().prepare(
            "SELECT a.lead_image, am.source_url FROM article a
             JOIN article_media am ON am.article_id = a.id
             WHERE a.lead_image IS NOT NULL AND am.hash IS NULL
               AND (a.is_read = 0 OR a.is_starred = 1)
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

    /// Deletes cached files that no Article refers to (neither as an image nor as a Lead Image)
    /// and no Feed uses as its favicon,
    /// and their `media` rows.
    pub fn remove_unreferenced_media(&self) -> Result<()> {
        let mut stmt = self.conn().prepare(
            "SELECT hash FROM media m
             WHERE NOT EXISTS (SELECT 1 FROM article_media am WHERE am.source_url IS NOT NULL AND am.hash = m.hash)
               AND NOT EXISTS (SELECT 1 FROM article a WHERE a.lead_image = m.hash)
               AND NOT EXISTS (SELECT 1 FROM feed f WHERE f.favicon_hash = m.hash)",
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

    /// Brings the cached images under `cap` bytes (spec: "Image cache"). Evicts images whose
    /// Articles are all Read and unstarred, oldest Article first. Favicons, and images of
    /// unread or Starred Articles, are never touched, so the cap is soft when only those remain.
    /// Evicted images stay remote links: the file and `media` row go, `article_media.hash`
    /// becomes NULL. Returns the number of bytes freed.
    pub fn evict_media(&self, cap: u64) -> Result<u64> {
        let total: i64 = self
            .conn()
            .query_row("SELECT coalesce(sum(bytes), 0) FROM media", [], |r| r.get(0))?;
        let mut over = (total as u64).saturating_sub(cap);
        if over == 0 {
            return Ok(0);
        }
        let mut stmt = self.conn().prepare(
            "SELECT m.hash, m.bytes
             FROM media m JOIN article_media am ON am.hash = m.hash
                          JOIN article a ON a.id = am.article_id
             WHERE NOT EXISTS (SELECT 1 FROM feed f WHERE f.favicon_hash = m.hash)
             GROUP BY m.hash
             HAVING min(a.is_read = 1 AND a.is_starred = 0) = 1
             ORDER BY max(a.article_date), m.hash",
        )?;
        let candidates: Vec<(String, i64)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?;
        drop(stmt);
        let dir = self.dir().join(MEDIA_DIR);
        let (mut freed, mut evicted) = (0u64, Vec::new());
        for (hash, bytes) in candidates {
            if over == 0 {
                break;
            }
            if is_valid_key(&hash) {
                match fs::remove_file(dir.join(&hash)) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.into()),
                }
            }
            over = over.saturating_sub(bytes as u64);
            freed += bytes as u64;
            evicted.push(hash);
        }
        let tx = self.conn().unchecked_transaction()?;
        for hash in &evicted {
            tx.execute(
                "UPDATE article_media SET hash = NULL WHERE hash = ?1",
                [hash],
            )?;
            tx.execute("DELETE FROM media WHERE hash = ?1", [hash])?;
        }
        tx.commit()?;
        Ok(freed)
    }

    /// Caches a Feed's favicon (always cached, never evicted). `source_url` is where it came from.
    pub fn set_favicon(
        &self,
        feed_id: i64,
        source_url: &str,
        bytes: &[u8],
        mime_type: &str,
    ) -> Result<()> {
        self.store_media(source_url, bytes, mime_type)?;
        self.conn().execute(
            "UPDATE feed SET favicon_hash = ?2 WHERE id = ?1",
            params![feed_id, media_key(source_url)],
        )?;
        Ok(())
    }

    /// Feeds without a cached favicon, as `(feed id, page URL to look for an icon)`:
    /// the Feed's website, else the Feed URL itself.
    pub fn feeds_missing_favicon(&self) -> Result<Vec<(i64, String)>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, coalesce(site_url, url) FROM feed
             WHERE favicon_hash IS NULL OR favicon_hash NOT IN (SELECT hash FROM media)",
        )?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
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

    /// Feed 1 with Articles `(is_read, is_starred, article_date, image bytes)`; each Article has
    /// one image of the given size.
    fn library_with(articles: &[(bool, bool, i64, usize)]) -> (tempfile::TempDir, Library) {
        let root = tempfile::tempdir().unwrap();
        let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
        library
            .conn()
            .execute("INSERT INTO feed (url, title) VALUES ('u', 'F')", [])
            .unwrap();
        for (i, (read, starred, date, size)) in articles.iter().enumerate() {
            let id = i as i64 + 1;
            library
                .conn()
                .execute(
                    "INSERT INTO article (id, feed_id, identity, first_seen_at, article_date, is_read, is_starred)
                     VALUES (?1, 1, ?1, 0, ?2, ?3, ?4)",
                    params![id, date, read, starred],
                )
                .unwrap();
            let url = format!("https://x.example/{id}.png");
            library
                .conn()
                .execute(
                    "INSERT INTO article_media (article_id, source_url) VALUES (?1, ?2)",
                    params![id, url],
                )
                .unwrap();
            library
                .store_media(&url, &vec![0u8; *size], "image/png")
                .unwrap();
        }
        (root, library)
    }

    fn cached(library: &Library, id: i64) -> bool {
        let key = media_key(&format!("https://x.example/{id}.png"));
        library
            .media_file(&key)
            .unwrap()
            .is_some_and(|(p, _)| p.exists())
    }

    #[test]
    fn eviction_takes_read_unstarred_oldest_first_and_stops_at_the_cap() {
        // 1: read, old. 2: read, newer. 3: unread. 4: read but starred. 5: read, newest.
        let (_root, library) = library_with(&[
            (true, false, 10, 100),
            (true, false, 20, 100),
            (false, false, 5, 100),
            (true, true, 1, 100),
            (true, false, 30, 100),
        ]);
        assert_eq!(library.evict_media(1000).unwrap(), 0, "under the cap");
        assert_eq!(library.evict_media(300).unwrap(), 200);
        assert!(!cached(&library, 1) && !cached(&library, 2));
        assert!(cached(&library, 3) && cached(&library, 4) && cached(&library, 5));
        // Evicted images stay as remote links.
        let nulls: i64 = library
            .conn()
            .query_row(
                "SELECT count(*) FROM article_media WHERE hash IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(nulls, 2);
        // Soft cap: unread and Starred images are never evicted, even when still over.
        library.evict_media(0).unwrap();
        assert!(!cached(&library, 5));
        assert!(cached(&library, 3) && cached(&library, 4));
        library.close().unwrap();
    }

    #[test]
    fn a_shared_image_is_kept_while_any_article_needs_it() {
        let (_root, library) = library_with(&[(true, false, 1, 100), (false, false, 2, 100)]);
        library
            .conn()
            .execute(
                "INSERT INTO article_media (article_id, source_url) VALUES (2, 'https://x.example/1.png')",
                [],
            )
            .unwrap();
        library
            .store_media("https://x.example/1.png", &[0u8; 100], "image/png")
            .unwrap();
        library.evict_media(0).unwrap();
        assert!(
            cached(&library, 1),
            "Article 2 is unread and still shows it"
        );
        library.close().unwrap();
    }

    #[test]
    fn evicted_images_of_read_articles_are_not_downloaded_again() {
        let (_root, library) = library_with(&[(true, false, 1, 100)]);
        let key = media_key("https://x.example/1.png");
        library
            .conn()
            .execute("UPDATE article SET lead_image = ?1", [&key])
            .unwrap();
        library.evict_media(0).unwrap();
        assert!(library.missing_lead_images(10).unwrap().is_empty());
        library.close().unwrap();
    }

    #[test]
    fn favicons_are_cached_and_never_evicted() {
        let (_root, library) = library_with(&[(true, false, 1, 100)]);
        assert_eq!(
            library.feeds_missing_favicon().unwrap(),
            vec![(1, "u".to_owned())]
        );
        library
            .set_favicon(1, "https://x.example/favicon.ico", b"ICO", "image/x-icon")
            .unwrap();
        assert!(library.feeds_missing_favicon().unwrap().is_empty());
        library.evict_media(0).unwrap();
        let key = media_key("https://x.example/favicon.ico");
        assert!(
            library
                .media_file(&key)
                .unwrap()
                .is_some_and(|(p, _)| p.exists())
        );
        library.remove_unreferenced_media().unwrap();
        assert!(
            library.media_file(&key).unwrap().is_some(),
            "still referenced by the Feed"
        );
        library.close().unwrap();
    }

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
