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
        assert!(library.media_file(&"0".repeat(32)).unwrap().is_none());
        library.close().unwrap();
    }
}
