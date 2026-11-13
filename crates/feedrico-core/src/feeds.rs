//! Feeds and Folders in the Library: OPML import, the sidebar tree, refresh bookkeeping.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::lock::now;
use crate::opml::{self, OpmlFeed};
use crate::parse::ParsedFeed;
use crate::{Error, Library, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub added: usize,
    pub skipped: usize,
    pub folders_created: usize,
}

/// Outcome of adding one Feed by URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum AddOutcome {
    Added {
        feed_id: i64,
    },
    /// A Feed with this URL is already subscribed; nothing was changed.
    Exists {
        feed_id: i64,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sidebar {
    pub all_unread: i64,
    pub starred: i64,
    pub today: i64,
    pub folders: Vec<FolderNode>,
    /// Feeds at the top level, not in any Folder.
    pub feeds: Vec<FeedNode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderNode {
    pub id: i64,
    pub name: String,
    pub unread: i64,
    pub feeds: Vec<FeedNode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedNode {
    pub id: i64,
    pub title: String,
    pub site_url: Option<String>,
    pub unread: i64,
    /// Set while the Feed is broken (spec: "Broken Feeds").
    pub last_error: Option<String>,
    pub last_success_at: Option<i64>,
    /// Media key of the cached favicon; `None` until one is cached.
    pub favicon: Option<String>,
}

/// What the fetcher needs to know about a Feed.
#[derive(Debug, Clone)]
pub struct FeedToFetch {
    pub id: i64,
    pub url: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

/// Result of fetching one Feed, produced off the database lock.
#[derive(Debug)]
pub enum FetchResult {
    NotModified,
    Fetched {
        feed: ParsedFeed,
        etag: Option<String>,
        last_modified: Option<String>,
    },
    Failed(String),
}

fn non_empty<'a>(text: &'a str, message: &str) -> Result<&'a str> {
    let text = text.trim();
    if text.is_empty() {
        return Err(Error::Invalid(message.into()));
    }
    Ok(text)
}

fn found(changed: usize) -> Result<()> {
    if changed == 0 {
        Err(Error::NotFound)
    } else {
        Ok(())
    }
}

/// Retry delays for broken Feeds: 30 min, 2 h, then 12 h.
pub fn backoff_secs(error_count: i64) -> i64 {
    match error_count {
        0 => 0,
        1 => 30 * 60,
        2 => 2 * 60 * 60,
        _ => 12 * 60 * 60,
    }
}

impl Library {
    /// Additive import: existing Feeds (same URL) are left untouched, nothing is removed.
    pub fn import_opml(&self, feeds: &[OpmlFeed]) -> Result<ImportSummary> {
        let tx = self.conn().unchecked_transaction()?;
        let mut summary = ImportSummary::default();
        let mut next_position: i64 = tx.query_row(
            "SELECT coalesce(max(position), -1) + 1 FROM feed",
            [],
            |r| r.get(0),
        )?;
        for feed in feeds {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM feed WHERE url = ?1)",
                [&feed.url],
                |r| r.get(0),
            )?;
            if exists {
                summary.skipped += 1;
                continue;
            }
            let folder_id = match &feed.folder {
                None => None,
                Some(name) => {
                    let existing: Option<i64> = tx
                        .query_row("SELECT id FROM folder WHERE name = ?1", [name], |r| {
                            r.get(0)
                        })
                        .optional()?;
                    match existing {
                        Some(id) => Some(id),
                        None => {
                            tx.execute(
                                "INSERT INTO folder (name, position)
                                 VALUES (?1, (SELECT coalesce(max(position), -1) + 1 FROM folder))",
                                [name],
                            )?;
                            summary.folders_created += 1;
                            Some(tx.last_insert_rowid())
                        }
                    }
                }
            };
            tx.execute(
                "INSERT INTO feed (url, title, site_url, folder_id, position) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![feed.url, feed.title, feed.site_url, folder_id, next_position],
            )?;
            next_position += 1;
            summary.added += 1;
        }
        tx.commit()?;
        Ok(summary)
    }

    /// Subscribes to one Feed. A URL that is already subscribed points to the existing Feed
    /// instead of duplicating it. `folder_id: None` means the top level.
    pub fn add_feed(
        &self,
        url: &str,
        title: &str,
        site_url: Option<&str>,
        folder_id: Option<i64>,
    ) -> Result<AddOutcome> {
        let tx = self.conn().unchecked_transaction()?;
        let existing: Option<i64> = tx
            .query_row("SELECT id FROM feed WHERE url = ?1", [url], |r| r.get(0))
            .optional()?;
        if let Some(feed_id) = existing {
            return Ok(AddOutcome::Exists { feed_id });
        }
        tx.execute(
            "INSERT INTO feed (url, title, site_url, folder_id, position)
             VALUES (?1, ?2, ?3, ?4, (SELECT coalesce(max(position), -1) + 1 FROM feed))",
            params![url, title, site_url, folder_id],
        )?;
        let feed_id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(AddOutcome::Added { feed_id })
    }

    pub fn rename_feed(&self, feed_id: i64, title: &str) -> Result<()> {
        let title = non_empty(title, "a Feed needs a name")?;
        let changed = self.conn().execute(
            "UPDATE feed SET title = ?2 WHERE id = ?1",
            params![feed_id, title],
        )?;
        found(changed)
    }

    /// Folder names are unique, because OPML re-import matches Folders by exact name.
    pub fn rename_folder(&self, folder_id: i64, name: &str) -> Result<()> {
        let name = non_empty(name, "a Folder needs a name")?;
        let taken: bool = self.conn().query_row(
            "SELECT EXISTS(SELECT 1 FROM folder WHERE name = ?1 AND id <> ?2)",
            params![name, folder_id],
            |r| r.get(0),
        )?;
        if taken {
            return Err(Error::Invalid(format!(
                "a Folder named \"{name}\" already exists"
            )));
        }
        let changed = self.conn().execute(
            "UPDATE folder SET name = ?2 WHERE id = ?1",
            params![folder_id, name],
        )?;
        found(changed)
    }

    /// Moves a Feed into a Folder, or to the top level with `None`. Folders are one level deep
    /// and a Feed is in at most one Folder, so this is a single column.
    pub fn move_feed(&self, feed_id: i64, folder_id: Option<i64>) -> Result<()> {
        if let Some(id) = folder_id {
            let exists: bool = self.conn().query_row(
                "SELECT EXISTS(SELECT 1 FROM folder WHERE id = ?1)",
                [id],
                |r| r.get(0),
            )?;
            if !exists {
                return Err(Error::NotFound);
            }
        }
        let changed = self.conn().execute(
            "UPDATE feed SET folder_id = ?2 WHERE id = ?1",
            params![feed_id, folder_id],
        )?;
        found(changed)
    }

    /// Deletes a Folder; its Feeds move to the top level.
    pub fn delete_folder(&self, folder_id: i64) -> Result<()> {
        // `feed.folder_id` is ON DELETE SET NULL.
        found(
            self.conn()
                .execute("DELETE FROM folder WHERE id = ?1", [folder_id])?,
        )
    }

    /// Deletes a Folder and unsubscribes from every Feed in it, removing their Articles and the
    /// cached images nothing else refers to.
    pub fn delete_folder_with_feeds(&self, folder_id: i64) -> Result<()> {
        let tx = self.conn().unchecked_transaction()?;
        tx.execute("DELETE FROM feed WHERE folder_id = ?1", [folder_id])?;
        found(tx.execute("DELETE FROM folder WHERE id = ?1", [folder_id])?)?;
        tx.commit()?;
        self.remove_unreferenced_media()
    }

    /// Removes a Feed with its Articles, then deletes cached images nothing refers to anymore.
    pub fn unsubscribe(&self, feed_id: i64) -> Result<()> {
        found(
            self.conn()
                .execute("DELETE FROM feed WHERE id = ?1", [feed_id])?,
        )?;
        self.remove_unreferenced_media()
    }

    /// Unsubscribes from several Feeds in one transaction, cleaning up media once. Unknown ids
    /// are skipped. Returns how many Feeds were removed.
    pub fn unsubscribe_many(&self, feed_ids: &[i64]) -> Result<usize> {
        let tx = self.conn().unchecked_transaction()?;
        let mut removed = 0;
        {
            let mut stmt = tx.prepare("DELETE FROM feed WHERE id = ?1")?;
            for id in feed_ids {
                removed += stmt.execute([id])?;
            }
        }
        tx.commit()?;
        self.remove_unreferenced_media()?;
        Ok(removed)
    }

    pub fn export_opml(&self) -> Result<String> {
        type Row = (String, String, Option<String>, Option<i64>);
        let conn = self.conn();
        let feeds: Vec<Row> = conn
            .prepare("SELECT title, url, site_url, folder_id FROM feed ORDER BY position, title COLLATE NOCASE")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let folders: Vec<(i64, String)> = conn
            .prepare("SELECT id, name FROM folder ORDER BY position, name COLLATE NOCASE")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        fn export(r: &Row) -> opml::ExportFeed<'_> {
            opml::ExportFeed {
                title: &r.0,
                url: &r.1,
                site_url: r.2.as_deref(),
            }
        }
        let folder_exports: Vec<_> = folders
            .iter()
            .map(|(id, name)| opml::ExportFolder {
                name,
                feeds: feeds
                    .iter()
                    .filter(|r| r.3 == Some(*id))
                    .map(export)
                    .collect(),
            })
            .collect();
        let top: Vec<_> = feeds
            .iter()
            .filter(|r| r.3.is_none_or(|id| folders.iter().all(|(f, _)| *f != id)))
            .map(export)
            .collect();
        Ok(opml::export(&folder_exports, &top))
    }

    /// The sidebar tree with unread counts. `today_since` is local midnight in unix seconds.
    pub fn sidebar(&self, today_since: i64) -> Result<Sidebar> {
        let conn = self.conn();
        let (starred, today): (i64, i64) = conn.query_row(
            "SELECT
                (SELECT count(*) FROM article WHERE is_starred = 1),
                (SELECT count(*) FROM article WHERE article_date >= ?1)",
            [today_since],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;

        let mut feeds_stmt = conn.prepare(
            "SELECT f.id, f.title, f.site_url, f.folder_id, f.last_error, f.last_success_at,
                    coalesce(u.unread, 0), m.hash
             FROM feed f
             LEFT JOIN media m ON m.hash = f.favicon_hash
             LEFT JOIN (SELECT feed_id, count(*) AS unread FROM article WHERE is_read = 0 GROUP BY feed_id) u
                    ON u.feed_id = f.id
             ORDER BY f.position, f.title COLLATE NOCASE",
        )?;
        let feeds: Vec<(Option<i64>, FeedNode)> = feeds_stmt
            .query_map([], |r| {
                Ok((
                    r.get(3)?,
                    FeedNode {
                        id: r.get(0)?,
                        title: r.get(1)?,
                        site_url: r.get(2)?,
                        last_error: r.get(4)?,
                        last_success_at: r.get(5)?,
                        unread: r.get(6)?,
                        favicon: r.get(7)?,
                    },
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;

        let mut folders_stmt =
            conn.prepare("SELECT id, name FROM folder ORDER BY position, name COLLATE NOCASE")?;
        let mut folders: Vec<FolderNode> = folders_stmt
            .query_map([], |r| {
                Ok(FolderNode {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    unread: 0,
                    feeds: Vec::new(),
                })
            })?
            .collect::<rusqlite::Result<_>>()?;

        let mut top = Vec::new();
        for (folder_id, node) in feeds {
            match folder_id.and_then(|id| folders.iter_mut().find(|f| f.id == id)) {
                Some(folder) => {
                    folder.unread += node.unread;
                    folder.feeds.push(node);
                }
                None => top.push(node),
            }
        }
        // The per-Feed counts already cover every unread Article; don't scan them twice.
        let all_unread = folders.iter().map(|f| f.unread).sum::<i64>()
            + top.iter().map(|f| f.unread).sum::<i64>();
        Ok(Sidebar {
            all_unread,
            starred,
            today,
            folders,
            feeds: top,
        })
    }

    pub fn feed_count(&self) -> Result<i64> {
        Ok(self
            .conn()
            .query_row("SELECT count(*) FROM feed", [], |r| r.get(0))?)
    }

    /// Feeds to fetch now. Broken Feeds wait out their backoff unless `force` is set.
    pub fn feeds_to_fetch(&self, force: bool) -> Result<Vec<FeedToFetch>> {
        let now = now();
        let mut stmt = self.conn().prepare(
            "SELECT id, url, etag, last_modified, error_count, coalesce(last_fetched_at, 0) FROM feed",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                FeedToFetch {
                    id: r.get(0)?,
                    url: r.get(1)?,
                    etag: r.get(2)?,
                    last_modified: r.get(3)?,
                },
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
            ))
        })?;
        let mut due = Vec::new();
        for row in rows {
            let (feed, errors, last) = row?;
            if force || now - last >= backoff_secs(errors) {
                due.push(feed);
            }
        }
        Ok(due)
    }

    /// One Feed to fetch now, whatever its backoff: *Retry now* on a broken Feed.
    pub fn feed_to_fetch(&self, feed_id: i64) -> Result<FeedToFetch> {
        self.conn()
            .query_row(
                "SELECT id, url, etag, last_modified FROM feed WHERE id = ?1",
                [feed_id],
                |r| {
                    Ok(FeedToFetch {
                        id: r.get(0)?,
                        url: r.get(1)?,
                        etag: r.get(2)?,
                        last_modified: r.get(3)?,
                    })
                },
            )
            .optional()?
            .ok_or(Error::NotFound)
    }

    /// Stores the result of one fetch. Returns the images new Articles need cached.
    pub fn apply_fetch(&self, feed_id: i64, result: FetchResult) -> Result<ApplyOutcome> {
        let now = now();
        let tx = self.conn().unchecked_transaction()?;
        let mut outcome = ApplyOutcome::default();
        match result {
            FetchResult::NotModified => {
                tx.execute(
                    "UPDATE feed SET last_fetched_at = ?2, last_success_at = ?2, last_error = NULL, error_count = 0
                     WHERE id = ?1",
                    params![feed_id, now],
                )?;
            }
            FetchResult::Failed(message) => {
                tx.execute(
                    "UPDATE feed SET last_fetched_at = ?2, last_error = ?3, error_count = error_count + 1
                     WHERE id = ?1",
                    params![feed_id, now, message],
                )?;
                outcome.failed = true;
            }
            FetchResult::Fetched {
                feed,
                etag,
                last_modified,
            } => {
                tx.execute(
                    "UPDATE feed SET last_fetched_at = ?2, last_success_at = ?2, last_error = NULL,
                            error_count = 0, etag = ?3, last_modified = ?4,
                            site_url = coalesce(site_url, ?5)
                     WHERE id = ?1",
                    params![feed_id, now, etag, last_modified, feed.site_url],
                )?;
                for entry in &feed.entries {
                    if let Some(article_id) = crate::articles::upsert(&tx, feed_id, entry, now)? {
                        outcome.new_articles += 1;
                        outcome
                            .images
                            .extend(entry.images.iter().map(|u| (article_id, u.clone())));
                    }
                }
            }
        }
        tx.commit()?;
        Ok(outcome)
    }
}

#[derive(Debug, Default)]
pub struct ApplyOutcome {
    pub new_articles: usize,
    pub failed: bool,
    /// `(article_id, image_url)` pairs for the media cache.
    pub images: Vec<(i64, String)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::DIR_NAME;

    fn opml_feed(url: &str, folder: Option<&str>) -> OpmlFeed {
        OpmlFeed {
            url: url.into(),
            title: url.trim_start_matches("https://").into(),
            site_url: None,
            folder: folder.map(Into::into),
        }
    }

    #[test]
    fn add_feed_subscribes_once_and_honours_the_folder() {
        let root = tempfile::tempdir().unwrap();
        let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
        library
            .import_opml(&[opml_feed("https://a.example", Some("Tech"))])
            .unwrap();
        let folder_id = library.sidebar(0).unwrap().folders[0].id;

        let added = library
            .add_feed("https://b.example/feed", "B", None, Some(folder_id))
            .unwrap();
        assert!(matches!(added, AddOutcome::Added { .. }));
        let again = library
            .add_feed("https://b.example/feed", "B again", None, None)
            .unwrap();
        let AddOutcome::Added { feed_id } = added else {
            unreachable!()
        };
        assert_eq!(again, AddOutcome::Exists { feed_id });

        let sidebar = library.sidebar(0).unwrap();
        assert_eq!(sidebar.folders[0].feeds.len(), 2);
        assert!(sidebar.feeds.is_empty());
        library
            .add_feed("https://c.example", "C", None, None)
            .unwrap();
        assert_eq!(library.sidebar(0).unwrap().feeds.len(), 1);
    }

    #[test]
    fn rename_move_and_delete_folder_keep_the_tree_consistent() {
        let root = tempfile::tempdir().unwrap();
        let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
        library
            .import_opml(&[
                opml_feed("https://a.example", Some("Tech")),
                opml_feed("https://b.example", Some("News")),
                opml_feed("https://c.example", None),
            ])
            .unwrap();
        let sidebar = library.sidebar(0).unwrap();
        let (tech, news) = (sidebar.folders[0].id, sidebar.folders[1].id);
        let a = sidebar.folders[0].feeds[0].id;
        let c = sidebar.feeds[0].id;

        library.rename_feed(a, "  Alpha ").unwrap();
        assert!(library.rename_feed(a, "   ").is_err());
        library.rename_folder(tech, "Technik").unwrap();
        assert!(library.rename_folder(tech, "News").is_err());
        library.rename_folder(tech, "Technik").unwrap(); // same name is not a clash
        assert!(matches!(
            library.rename_feed(999, "x"),
            Err(Error::NotFound)
        ));

        library.move_feed(c, Some(news)).unwrap();
        assert!(matches!(
            library.move_feed(c, Some(999)),
            Err(Error::NotFound)
        ));
        library.move_feed(a, None).unwrap();
        let sidebar = library.sidebar(0).unwrap();
        assert_eq!(sidebar.folders[1].feeds.len(), 2);
        assert_eq!(sidebar.feeds[0].title, "Alpha");

        library.delete_folder(news).unwrap();
        let sidebar = library.sidebar(0).unwrap();
        assert_eq!(sidebar.folders.len(), 1);
        assert_eq!(
            sidebar.feeds.len(),
            3,
            "deleted Folder's Feeds go to the top level"
        );
        assert!(matches!(library.delete_folder(news), Err(Error::NotFound)));
    }

    // Bug (YouTube Folder deleted, its Articles still in All Unread): deleting a Folder could
    // only move its Feeds to the top level, so there was no way to drop a Folder of hundreds of
    // Feeds together with their Articles.
    #[test]
    fn delete_folder_with_feeds_unsubscribes_only_that_folders_feeds() {
        let root = tempfile::tempdir().unwrap();
        let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
        library
            .import_opml(&[
                opml_feed("https://yt1.example", Some("YouTube")),
                opml_feed("https://yt2.example", Some("YouTube")),
                opml_feed("https://news.example", Some("News")),
                opml_feed("https://top.example", None),
            ])
            .unwrap();
        let sidebar = library.sidebar(0).unwrap();
        let news = sidebar.folders.iter().find(|f| f.name == "News").unwrap();
        let youtube = sidebar
            .folders
            .iter()
            .find(|f| f.name == "YouTube")
            .unwrap();
        let kept = [news.feeds[0].id, sidebar.feeds[0].id];
        let gone: Vec<i64> = youtube.feeds.iter().map(|f| f.id).collect();
        let youtube = youtube.id;

        let conn = library.conn();
        for (id, feed) in gone.iter().chain(&kept).enumerate() {
            conn.execute(
                "INSERT INTO article (id, feed_id, identity, first_seen_at, article_date)
                 VALUES (?1, ?2, ?1, 0, 0)",
                params![id as i64 + 1, feed],
            )
            .unwrap();
            let url = if gone.contains(feed) {
                format!("https://img/{feed}.png")
            } else {
                "https://img/kept.png".to_string()
            };
            conn.execute(
                "INSERT INTO article_media (article_id, source_url) VALUES (?1, ?2)",
                params![id as i64 + 1, url],
            )
            .unwrap();
            library.store_media(&url, b"x", "image/png").unwrap();
        }

        library.delete_folder_with_feeds(youtube).unwrap();

        let sidebar = library.sidebar(0).unwrap();
        assert_eq!(sidebar.folders.len(), 1, "only News is left");
        assert_eq!(sidebar.folders[0].feeds.len(), 1);
        assert_eq!(sidebar.feeds.len(), 1, "no Feed moved to the top level");
        assert_eq!(sidebar.all_unread, 2, "only the kept Feeds' Articles count");
        assert_eq!(library.feed_count().unwrap(), 2);
        let key = |u: &str| crate::sanitize::media_key(u);
        for feed in &gone {
            assert!(
                library
                    .media_file(&key(&format!("https://img/{feed}.png")))
                    .unwrap()
                    .is_none()
            );
        }
        assert!(
            library
                .media_file(&key("https://img/kept.png"))
                .unwrap()
                .is_some()
        );
        assert!(matches!(
            library.delete_folder_with_feeds(youtube),
            Err(Error::NotFound)
        ));
    }

    #[test]
    fn unsubscribe_removes_articles_and_unreferenced_images_only() {
        let root = tempfile::tempdir().unwrap();
        let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
        let AddOutcome::Added { feed_id: gone } = library
            .add_feed("https://a.example", "A", None, None)
            .unwrap()
        else {
            unreachable!()
        };
        let AddOutcome::Added { feed_id: kept } = library
            .add_feed("https://b.example", "B", None, None)
            .unwrap()
        else {
            unreachable!()
        };
        let conn = library.conn();
        for (feed, id) in [(gone, 1), (kept, 2)] {
            conn.execute(
                "INSERT INTO article (id, feed_id, identity, first_seen_at, article_date)
                 VALUES (?1, ?2, ?1, 0, 0)",
                params![id, feed],
            )
            .unwrap();
        }
        for (article, url) in [
            (1, "https://img/only-gone.png"),
            (1, "https://img/shared.png"),
            (2, "https://img/shared.png"),
        ] {
            conn.execute(
                "INSERT INTO article_media (article_id, source_url) VALUES (?1, ?2)",
                params![article, url],
            )
            .unwrap();
        }
        library
            .store_media("https://img/only-gone.png", b"1", "image/png")
            .unwrap();
        library
            .store_media("https://img/shared.png", b"2", "image/png")
            .unwrap();
        let key = |u| crate::sanitize::media_key(u);

        library.unsubscribe(gone).unwrap();

        let count: i64 = conn
            .query_row("SELECT count(*) FROM article", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        assert!(
            library
                .media_file(&key("https://img/only-gone.png"))
                .unwrap()
                .is_none()
        );
        assert!(
            !library
                .dir()
                .join("media")
                .join(key("https://img/only-gone.png"))
                .exists()
        );
        let (path, _) = library
            .media_file(&key("https://img/shared.png"))
            .unwrap()
            .unwrap();
        assert!(path.exists());
        assert_eq!(library.feed_count().unwrap(), 1);
    }

    #[test]
    fn import_is_additive_and_builds_the_sidebar() {
        let root = tempfile::tempdir().unwrap();
        let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
        let first = library
            .import_opml(&[
                opml_feed("https://a.example", Some("Tech")),
                opml_feed("https://b.example", Some("Tech")),
                opml_feed("https://c.example", None),
            ])
            .unwrap();
        assert_eq!(
            first,
            ImportSummary {
                added: 3,
                skipped: 0,
                folders_created: 1
            }
        );
        let second = library
            .import_opml(&[
                opml_feed("https://a.example", Some("Elsewhere")),
                opml_feed("https://d.example", Some("Tech")),
            ])
            .unwrap();
        assert_eq!(second.added, 1);
        assert_eq!(second.skipped, 1);
        assert_eq!(second.folders_created, 0);

        let sidebar = library.sidebar(0).unwrap();
        assert_eq!(sidebar.folders.len(), 1, "existing Feed kept its Folder");
        assert_eq!(sidebar.folders[0].feeds.len(), 3);
        assert_eq!(sidebar.feeds.len(), 1);

        let exported = library.export_opml().unwrap();
        let reparsed = opml::parse(&exported).unwrap();
        assert_eq!(reparsed.len(), 4);
        assert!(
            reparsed
                .iter()
                .any(|f| f.url == "https://d.example" && f.folder.as_deref() == Some("Tech"))
        );
        library.close().unwrap();
    }

    #[test]
    fn broken_feeds_back_off() {
        let root = tempfile::tempdir().unwrap();
        let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
        library
            .import_opml(&[opml_feed("https://a.example", None)])
            .unwrap();
        assert_eq!(library.feeds_to_fetch(false).unwrap().len(), 1);
        library
            .apply_fetch(1, FetchResult::Failed("HTTP 404".into()))
            .unwrap();
        assert!(
            library.feeds_to_fetch(false).unwrap().is_empty(),
            "waits 30 min"
        );
        assert_eq!(
            library.feeds_to_fetch(true).unwrap().len(),
            1,
            "manual refresh forces"
        );
        let node = &library.sidebar(0).unwrap().feeds[0];
        assert_eq!(node.last_error.as_deref(), Some("HTTP 404"));
        library.close().unwrap();
    }

    /// Broken Feeds dialog (#66): *Retry now* fetches that one Feed while it is backing off.
    #[test]
    fn one_broken_feed_can_be_fetched_despite_its_backoff() {
        let root = tempfile::tempdir().unwrap();
        let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
        library
            .import_opml(&[
                opml_feed("https://a.example", None),
                opml_feed("https://b.example", None),
            ])
            .unwrap();
        library
            .apply_fetch(1, FetchResult::Failed("HTTP 404".into()))
            .unwrap();
        assert!(
            library
                .feeds_to_fetch(false)
                .unwrap()
                .iter()
                .all(|f| f.id != 1)
        );

        let feed = library.feed_to_fetch(1).unwrap();
        assert_eq!((feed.id, feed.url.as_str()), (1, "https://a.example"));
        assert!(matches!(library.feed_to_fetch(99), Err(Error::NotFound)));
        library.close().unwrap();
    }

    /// Broken Feeds dialog (#66): *Unsubscribe from all listed* removes them in one go.
    #[test]
    fn unsubscribes_from_several_feeds_at_once() {
        let root = tempfile::tempdir().unwrap();
        let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
        library
            .import_opml(&[
                opml_feed("https://a.example", None),
                opml_feed("https://b.example", Some("Tech")),
                opml_feed("https://c.example", None),
            ])
            .unwrap();
        library
            .conn()
            .execute(
                "INSERT INTO article (feed_id, identity, first_seen_at, article_date) VALUES (2, 'x', 0, 0)",
                [],
            )
            .unwrap();

        assert_eq!(library.unsubscribe_many(&[1, 2, 99]).unwrap(), 2);

        let sidebar = library.sidebar(0).unwrap();
        assert_eq!(sidebar.feeds.len(), 1);
        assert_eq!(sidebar.feeds[0].title, "c.example");
        assert!(sidebar.folders[0].feeds.is_empty(), "the Folder stays");
        let articles: i64 = library
            .conn()
            .query_row("SELECT count(*) FROM article", [], |r| r.get(0))
            .unwrap();
        assert_eq!(articles, 0);
        library.close().unwrap();
    }

    #[test]
    fn backoff_steps_are_30_minutes_2_hours_then_12_hours() {
        assert_eq!(backoff_secs(0), 0);
        assert_eq!(backoff_secs(1), 30 * 60);
        assert_eq!(backoff_secs(2), 2 * 3600);
        assert_eq!(backoff_secs(3), 12 * 3600);
        assert_eq!(
            backoff_secs(50),
            12 * 3600,
            "never gives up, never speeds up"
        );
    }

    #[test]
    fn success_clears_the_error_and_keeps_the_last_success_time() {
        let root = tempfile::tempdir().unwrap();
        let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
        library
            .import_opml(&[opml_feed("https://a.example", None)])
            .unwrap();
        for _ in 0..3 {
            library
                .apply_fetch(1, FetchResult::Failed("timed out".into()))
                .unwrap();
        }
        let node = &library.sidebar(0).unwrap().feeds[0];
        assert_eq!(node.last_error.as_deref(), Some("timed out"));
        assert!(node.last_success_at.is_none());
        let errors: i64 = library
            .conn()
            .query_row("SELECT error_count FROM feed WHERE id = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(errors, 3);

        library.apply_fetch(1, FetchResult::NotModified).unwrap();
        let node = &library.sidebar(0).unwrap().feeds[0];
        assert!(node.last_error.is_none(), "badge disappears");
        assert!(node.last_success_at.is_some());
        let errors: i64 = library
            .conn()
            .query_row("SELECT error_count FROM feed WHERE id = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(errors, 0, "backoff resets");
        // Failing again starts over at 30 minutes, and the Feed is still subscribed.
        library
            .apply_fetch(1, FetchResult::Failed("HTTP 500".into()))
            .unwrap();
        assert_eq!(library.feed_count().unwrap(), 1);
        let node = &library.sidebar(0).unwrap().feeds[0];
        assert!(
            node.last_success_at.is_some(),
            "last success is kept for the tooltip"
        );
    }
}

/// Edge cases around the refresh bookkeeping: backoff boundaries and stored conditionals.
#[cfg(test)]
mod edge_tests {
    use super::*;
    use crate::library::DIR_NAME;
    use crate::parse::{ParsedEntry, ParsedFeed};

    fn library_with_feed(site_url: Option<&str>) -> (tempfile::TempDir, Library) {
        let root = tempfile::tempdir().unwrap();
        let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
        library
            .add_feed("https://a.example/feed", "A", site_url, None)
            .unwrap();
        (root, library)
    }

    #[test]
    fn the_backoff_boundary_is_inclusive() {
        let (_root, library) = library_with_feed(None);
        let conn = library.conn();
        // Two failures: the Feed now waits 2 h.
        library
            .apply_fetch(1, FetchResult::Failed("HTTP 500".into()))
            .unwrap();
        library
            .apply_fetch(1, FetchResult::Failed("HTTP 500".into()))
            .unwrap();
        let set_last = |secs_ago: i64| {
            conn.execute(
                "UPDATE feed SET last_fetched_at = ?2 WHERE id = 1",
                params![1, now() - secs_ago],
            )
            .unwrap();
        };

        // Margins keep the test off the wall-clock second boundary.
        set_last(2 * 3600 - 60);
        assert!(
            library.feeds_to_fetch(false).unwrap().is_empty(),
            "before the delay is over: still waiting"
        );
        set_last(2 * 3600 + 60);
        assert_eq!(
            library.feeds_to_fetch(false).unwrap().len(),
            1,
            "once the delay has passed: due again"
        );
        library.close().unwrap();
    }

    #[test]
    fn a_fetched_result_stores_the_conditionals_and_fills_the_site_url() {
        let (_root, library) = library_with_feed(None);
        let entry = ParsedEntry {
            identity: "a".into(),
            url: None,
            title: "A".into(),
            author: None,
            content_html: "<p>A</p>".into(),
            content_text: "A".into(),
            excerpt: "A".into(),
            published_at: None,
            images: vec![],
            lead_image: None,
            enclosures: vec![],

            video: None,
        };
        library
            .apply_fetch(
                1,
                FetchResult::Fetched {
                    feed: ParsedFeed {
                        title: None,
                        site_url: Some("https://a.example/".into()),
                        entries: vec![entry],
                    },
                    etag: Some("\"e1\"".into()),
                    last_modified: Some("Wed, 01 Jan 2025 00:00:00 GMT".into()),
                },
            )
            .unwrap();
        let row: (Option<String>, Option<String>, Option<String>, Option<i64>, i64) = library
            .conn()
            .query_row(
                "SELECT etag, last_modified, site_url, last_success_at, error_count FROM feed WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(row.0.as_deref(), Some("\"e1\""));
        assert_eq!(row.1.as_deref(), Some("Wed, 01 Jan 2025 00:00:00 GMT"));
        assert_eq!(row.2.as_deref(), Some("https://a.example/"));
        assert!(row.3.is_some());
        assert_eq!(row.4, 0);

        // A later fetch without a site_url keeps the one already stored (coalesce).
        library
            .apply_fetch(
                1,
                FetchResult::Fetched {
                    feed: ParsedFeed {
                        title: None,
                        site_url: None,
                        entries: vec![],
                    },
                    etag: Some("\"e2\"".into()),
                    last_modified: None,
                },
            )
            .unwrap();
        let (etag, site_url): (Option<String>, Option<String>) = library
            .conn()
            .query_row("SELECT etag, site_url FROM feed WHERE id = 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(etag.as_deref(), Some("\"e2\""));
        assert_eq!(site_url.as_deref(), Some("https://a.example/"));
        library.close().unwrap();
    }

    #[test]
    fn moving_a_missing_feed_is_not_found_but_moving_to_the_top_level_is_fine() {
        let (_root, library) = library_with_feed(None);
        assert!(matches!(library.move_feed(999, None), Err(Error::NotFound)));
        library.move_feed(1, None).unwrap();
        assert!(library.sidebar(0).unwrap().feeds.iter().any(|f| f.id == 1));
        library.close().unwrap();
    }
}
