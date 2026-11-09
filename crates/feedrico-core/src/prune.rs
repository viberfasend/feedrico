//! Pruning (spec: "Pruning"): removes old Articles by Article Date, then the cached images
//! nothing refers to any more. Starred Articles are never touched.

use crate::{Library, Result};

const DAY: i64 = 24 * 60 * 60;

/// Articles deleted per database transaction in `prune`.
const BATCH: usize = 500;

impl Library {
    /// Deletes up to `limit` Articles that are due for Pruning at time `now` (unix seconds):
    /// Read, unstarred ones older than the Read threshold, unread, unstarred ones older than the
    /// unread threshold. Returns how many were removed; call again until it returns less than
    /// `limit`, so the caller can release the Library between batches. Does not touch images:
    /// finish with `remove_unreferenced_media`, or use `prune`.
    pub fn prune_batch(&self, now: i64, limit: usize) -> Result<usize> {
        let read_before = now - i64::from(self.prune_read_days()?) * DAY;
        let unread_before = now - i64::from(self.prune_unread_days()?) * DAY;
        let removed = self.conn().execute(
            "DELETE FROM article WHERE id IN (
                 SELECT id FROM article
                 WHERE is_starred = 0
                   AND article_date < CASE is_read WHEN 1 THEN ?1 ELSE ?2 END
                 LIMIT ?3)",
            rusqlite::params![read_before, unread_before, limit as i64],
        )?;
        Ok(removed)
    }

    /// Runs all of Pruning at once. Returns the number of Articles removed.
    pub fn prune(&self, now: i64) -> Result<usize> {
        let mut total = 0;
        loop {
            let n = self.prune_batch(now, BATCH)?;
            total += n;
            if n < BATCH {
                break;
            }
        }
        if total > 0 {
            self.remove_unreferenced_media()?;
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::DIR_NAME;
    use crate::sanitize::media_key;
    use rusqlite::params;

    const NOW: i64 = 1_000_000_000;

    fn library() -> (tempfile::TempDir, Library) {
        let root = tempfile::tempdir().unwrap();
        let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
        library
            .conn()
            .execute("INSERT INTO feed (url, title) VALUES ('u', 'F')", [])
            .unwrap();
        (root, library)
    }

    /// An Article `age_days` old, with one cached image `https://x.example/<id>.png`.
    fn add(library: &Library, id: i64, read: bool, starred: bool, age_days: i64) {
        add_at(library, id, read, starred, NOW - age_days * DAY);
    }

    fn add_at(library: &Library, id: i64, read: bool, starred: bool, date: i64) {
        library
            .conn()
            .execute(
                "INSERT INTO article (id, feed_id, identity, title, content_text, first_seen_at, article_date, is_read, is_starred)
                 VALUES (?1, 1, ?1, 'needle', 'needle', 0, ?2, ?3, ?4)",
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
        library.store_media(&url, b"img", "image/png").unwrap();
    }

    fn ids(library: &Library) -> Vec<i64> {
        let mut stmt = library
            .conn()
            .prepare("SELECT id FROM article ORDER BY id")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(std::result::Result::unwrap)
            .collect()
    }

    fn cached(library: &Library, id: i64) -> bool {
        library
            .media_file(&media_key(&format!("https://x.example/{id}.png")))
            .unwrap()
            .is_some_and(|(p, _)| p.exists())
    }

    #[test]
    fn defaults_are_90_days_read_and_a_year_unread() {
        let (_root, library) = library();
        assert_eq!(library.prune_read_days().unwrap(), 90);
        assert_eq!(library.prune_unread_days().unwrap(), 365);
        library.close().unwrap();
    }

    #[test]
    fn starred_articles_are_kept_forever() {
        let (_root, library) = library();
        add(&library, 1, true, true, 5000);
        add(&library, 2, false, true, 5000);
        assert_eq!(library.prune(NOW).unwrap(), 0);
        assert_eq!(ids(&library), vec![1, 2]);
        assert!(cached(&library, 1) && cached(&library, 2));
        library.close().unwrap();
    }

    #[test]
    fn read_articles_go_after_90_days_and_unread_after_a_year() {
        let (_root, library) = library();
        add(&library, 1, true, false, 91); // read, past
        add(&library, 2, true, false, 89); // read, within
        add(&library, 3, false, false, 91); // unread at 91 days: kept
        add(&library, 4, false, false, 366); // unread, past
        add(&library, 5, false, false, 364); // unread, within
        assert_eq!(library.prune(NOW).unwrap(), 2);
        assert_eq!(ids(&library), vec![2, 3, 5]);
        library.close().unwrap();
    }

    #[test]
    fn exactly_on_the_threshold_is_kept() {
        let (_root, library) = library();
        add(&library, 1, true, false, 90);
        add(&library, 2, false, false, 365);
        add_at(&library, 3, true, false, NOW - 90 * DAY - 1);
        assert_eq!(library.prune(NOW).unwrap(), 1);
        assert_eq!(ids(&library), vec![1, 2]);
        library.close().unwrap();
    }

    #[test]
    fn thresholds_are_configurable() {
        let (_root, library) = library();
        add(&library, 1, true, false, 10);
        add(&library, 2, false, false, 10);
        add(&library, 3, true, false, 40);
        library.set_prune_read_days(30).unwrap();
        library.set_prune_unread_days(5).unwrap();
        assert_eq!(library.prune_read_days().unwrap(), 30);
        assert_eq!(library.prune(NOW).unwrap(), 2);
        assert_eq!(ids(&library), vec![1]);
        library.close().unwrap();
    }

    #[test]
    fn images_go_with_their_articles_unless_another_article_shows_them() {
        let (_root, library) = library();
        add(&library, 1, true, false, 200); // pruned
        add(&library, 2, true, false, 1); // kept
        add(&library, 3, true, false, 200); // pruned, but Article 2 shows its image too
        library
            .conn()
            .execute(
                "INSERT INTO article_media (article_id, source_url, hash) VALUES (2, 'https://x.example/3.png', ?1)",
                [media_key("https://x.example/3.png")],
            )
            .unwrap();
        // A pruned Article's Lead Image goes too.
        library
            .conn()
            .execute(
                "UPDATE article SET lead_image = ?1 WHERE id = 1",
                [media_key("https://x.example/1.png")],
            )
            .unwrap();
        library.prune(NOW).unwrap();
        assert_eq!(ids(&library), vec![2]);
        assert!(!cached(&library, 1), "only the pruned Article had it");
        assert!(cached(&library, 2));
        assert!(cached(&library, 3), "Article 2 still shows it");
        library.close().unwrap();
    }

    #[test]
    fn favicons_survive_pruning() {
        let (_root, library) = library();
        add(&library, 1, true, false, 200);
        library
            .set_favicon(1, "https://x.example/favicon.ico", b"ICO", "image/x-icon")
            .unwrap();
        library.prune(NOW).unwrap();
        let key = media_key("https://x.example/favicon.ico");
        assert!(library.media_file(&key).unwrap().is_some());
        assert!(!cached(&library, 1));
        library.close().unwrap();
    }

    #[test]
    fn search_index_stays_consistent() {
        let (_root, library) = library();
        add(&library, 1, true, false, 200);
        add(&library, 2, true, false, 1);
        let hits = |library: &Library| -> i64 {
            library
                .conn()
                .query_row(
                    "SELECT count(*) FROM article_fts WHERE article_fts MATCH 'needle'",
                    [],
                    |r| r.get(0),
                )
                .unwrap()
        };
        assert_eq!(hits(&library), 2);
        library.prune(NOW).unwrap();
        assert_eq!(hits(&library), 1);
        library
            .conn()
            .execute(
                "INSERT INTO article_fts(article_fts) VALUES ('integrity-check')",
                [],
            )
            .unwrap();
        library.close().unwrap();
    }

    #[test]
    fn batches_respect_the_limit() {
        let (_root, library) = library();
        for id in 1..=5 {
            add(&library, id, true, false, 200);
        }
        assert_eq!(library.prune_batch(NOW, 2).unwrap(), 2);
        assert_eq!(library.prune_batch(NOW, 2).unwrap(), 2);
        assert_eq!(library.prune_batch(NOW, 2).unwrap(), 1);
        assert!(ids(&library).is_empty());
        library.close().unwrap();
    }
}
