//! Articles: storing fetched entries, listing them per view, Read and Starred state, Pruning.

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::parse::{EnclosureKind, ParsedEntry};
use crate::{Error, Library, Result};

/// Anything before this is treated as a missing or bogus date (1995-01-01).
const EARLIEST_PLAUSIBLE: i64 = 788_918_400;

/// See "Article Date" in GLOSSARY.md.
pub fn article_date(published_at: Option<i64>, first_seen_at: i64) -> i64 {
    match published_at {
        Some(t) if (EARLIEST_PLAUSIBLE..=first_seen_at).contains(&t) => t,
        _ => first_seen_at,
    }
}

/// Inserts a new Article or updates an edited one. Returns the id if the Article is new.
pub(crate) fn upsert(
    conn: &Connection,
    feed_id: i64,
    e: &ParsedEntry,
    now: i64,
) -> Result<Option<i64>> {
    let existing: Option<(i64, String, String)> = conn
        .query_row(
            "SELECT id, title, content_html FROM article WHERE feed_id = ?1 AND identity = ?2",
            params![feed_id, e.identity],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;

    match existing {
        // Edited upstream: refresh the content, keep Read/Starred and the Article Date.
        Some((id, title, html)) => {
            if title != e.title || html != e.content_html {
                conn.execute(
                    "UPDATE article SET title = ?2, content_html = ?3, content_text = ?4, excerpt = ?5,
                            url = ?6, author = ?7, lead_image = ?8
                     WHERE id = ?1",
                    params![
                        id,
                        e.title,
                        e.content_html,
                        e.content_text,
                        e.excerpt,
                        e.url,
                        e.author,
                        lead_key(e)
                    ],
                )?;
            }
            Ok(None)
        }
        None => {
            conn.execute(
                "INSERT INTO article (feed_id, identity, url, title, author, content_html, content_text,
                                      excerpt, published_at, first_seen_at, article_date, lead_image)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    feed_id,
                    e.identity,
                    e.url,
                    e.title,
                    e.author,
                    e.content_html,
                    e.content_text,
                    e.excerpt,
                    e.published_at,
                    now,
                    article_date(e.published_at, now),
                    lead_key(e),
                ],
            )?;
            let id = conn.last_insert_rowid();
            for enc in &e.enclosures {
                conn.execute(
                    "INSERT INTO enclosure (article_id, url, mime_type, length, duration_secs, kind, player_url)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        id,
                        enc.url,
                        enc.mime_type,
                        enc.length,
                        enc.duration_secs,
                        enc.kind.as_str(),
                        enc.player_url
                    ],
                )?;
            }
            for url in &e.images {
                conn.execute(
                    "INSERT OR IGNORE INTO article_media (article_id, source_url) VALUES (?1, ?2)",
                    params![id, url],
                )?;
            }
            Ok(Some(id))
        }
    }
}

fn lead_key(e: &ParsedEntry) -> Option<String> {
    e.lead_image.as_deref().map(crate::sanitize::media_key)
}

/// Which Articles a list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum View {
    AllUnread,
    Starred,
    /// `since` is local midnight in unix seconds; the UI knows the user's time zone.
    Today {
        since: i64,
    },
    Feed {
        id: i64,
    },
    Folder {
        id: i64,
    },
    /// Full-text search; the query lives in `ListQuery::search`. Scoped to one Feed or one
    /// Folder when set, otherwise the whole Library.
    Search {
        #[serde(default)]
        feed: Option<i64>,
        #[serde(default)]
        folder: Option<i64>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Sort {
    #[default]
    NewestFirst,
    OldestFirst,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    pub view: View,
    #[serde(default)]
    pub sort: Sort,
    #[serde(default)]
    pub search: Option<String>,
    /// Only Articles with a Lead Image (the Gallery Layout).
    #[serde(default)]
    pub with_lead_image: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleRow {
    pub id: i64,
    pub feed_id: i64,
    pub feed_title: String,
    pub title: String,
    pub excerpt: String,
    pub article_date: i64,
    pub is_read: bool,
    pub is_starred: bool,
    /// Media key of the Lead Image, if any.
    pub lead_image: Option<String>,
    /// Media key of the Feed's cached favicon, if any.
    pub favicon: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleDetail {
    pub id: i64,
    pub feed_id: i64,
    pub feed_title: String,
    pub feed_site_url: Option<String>,
    pub title: String,
    pub url: Option<String>,
    pub author: Option<String>,
    pub content_html: String,
    pub article_date: i64,
    pub is_read: bool,
    pub is_starred: bool,
    /// Media key of the Feed's cached favicon, if any.
    pub favicon: Option<String>,
    pub enclosures: Vec<Enclosure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Enclosure {
    pub url: String,
    pub mime_type: Option<String>,
    pub length: Option<i64>,
    /// Play time in seconds (MediaRSS `duration` / JSON Feed `duration_in_seconds`).
    pub duration_secs: Option<i64>,
    pub kind: EnclosureKind,
    /// Player page (`media:player`) when the URL is not a direct media file.
    pub player_url: Option<String>,
}

/// SQL `WHERE` clause and parameters for a query. Columns refer to `article a`.
fn filter(q: &ListQuery) -> (String, Vec<rusqlite::types::Value>) {
    let (clause, args) = view_filter(q);
    if q.with_lead_image {
        (format!("({clause}) AND a.lead_image IS NOT NULL"), args)
    } else {
        (clause, args)
    }
}

fn view_filter(q: &ListQuery) -> (String, Vec<rusqlite::types::Value>) {
    use rusqlite::types::Value;
    match q.view {
        View::AllUnread => ("a.is_read = 0".into(), vec![]),
        View::Starred => ("a.is_starred = 1".into(), vec![]),
        View::Today { since } => ("a.article_date >= ?".into(), vec![Value::Integer(since)]),
        View::Feed { id } => ("a.feed_id = ?".into(), vec![Value::Integer(id)]),
        View::Folder { id } => (
            "a.feed_id IN (SELECT id FROM feed WHERE folder_id = ?)".into(),
            vec![Value::Integer(id)],
        ),
        View::Search { feed, folder } => {
            let query = fts_query(q.search.as_deref().unwrap_or(""));
            let mut clause =
                String::from("a.id IN (SELECT rowid FROM article_fts WHERE article_fts MATCH ?)");
            let mut args = vec![Value::Text(query)];
            if let Some(id) = feed {
                clause.push_str(" AND a.feed_id = ?");
                args.push(Value::Integer(id));
            } else if let Some(id) = folder {
                clause.push_str(" AND a.feed_id IN (SELECT id FROM feed WHERE folder_id = ?)");
                args.push(Value::Integer(id));
            }
            (clause, args)
        }
    }
}

/// Turns user input into a safe FTS5 prefix query: every word must match, as a prefix.
fn fts_query(input: &str) -> String {
    let terms: Vec<String> = input
        .split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
        })
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{w}\"*"))
        .collect();
    if terms.is_empty() {
        "\"\"".into()
    } else {
        terms.join(" ")
    }
}

impl Library {
    pub fn count_articles(&self, q: &ListQuery) -> Result<i64> {
        let (clause, args) = filter(q);
        Ok(self.conn().query_row(
            &format!("SELECT count(*) FROM article a WHERE {clause}"),
            rusqlite::params_from_iter(args),
            |r| r.get(0),
        )?)
    }

    pub fn list_articles(&self, q: &ListQuery, offset: i64, limit: i64) -> Result<Vec<ArticleRow>> {
        let (clause, mut args) = filter(q);
        let order = match q.sort {
            Sort::NewestFirst => "a.article_date DESC, a.id DESC",
            Sort::OldestFirst => "a.article_date ASC, a.id ASC",
        };
        args.push(limit.into());
        args.push(offset.into());
        let mut stmt = self.conn().prepare_cached(&format!(
            "SELECT a.id, a.feed_id, f.title, a.title, a.excerpt, a.article_date, a.is_read, a.is_starred,
                    a.lead_image, m.hash
             FROM article a JOIN feed f ON f.id = a.feed_id
             LEFT JOIN media m ON m.hash = f.favicon_hash
             WHERE {clause} ORDER BY {order} LIMIT ? OFFSET ?"
        ))?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(args), |r| {
                Ok(ArticleRow {
                    id: r.get(0)?,
                    feed_id: r.get(1)?,
                    feed_title: r.get(2)?,
                    title: r.get(3)?,
                    excerpt: r.get(4)?,
                    article_date: r.get(5)?,
                    is_read: r.get(6)?,
                    is_starred: r.get(7)?,
                    lead_image: r.get(8)?,
                    favicon: r.get(9)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    pub fn article(&self, id: i64) -> Result<ArticleDetail> {
        let conn = self.conn();
        let mut detail = conn
            .query_row(
                "SELECT a.id, a.feed_id, f.title, f.site_url, a.title, a.url, a.author, a.content_html,
                        a.article_date, a.is_read, a.is_starred, m.hash
                 FROM article a JOIN feed f ON f.id = a.feed_id
                 LEFT JOIN media m ON m.hash = f.favicon_hash
                 WHERE a.id = ?1",
                [id],
                |r| {
                    Ok(ArticleDetail {
                        id: r.get(0)?,
                        feed_id: r.get(1)?,
                        feed_title: r.get(2)?,
                        feed_site_url: r.get(3)?,
                        title: r.get(4)?,
                        url: r.get(5)?,
                        author: r.get(6)?,
                        content_html: r.get(7)?,
                        article_date: r.get(8)?,
                        is_read: r.get(9)?,
                        is_starred: r.get(10)?,
                        favicon: r.get(11)?,
                        enclosures: Vec::new(),
                    })
                },
            )
            .optional()?
            .ok_or(Error::NotFound)?;
        let mut stmt = conn.prepare_cached(
            "SELECT url, mime_type, length, duration_secs, kind, player_url FROM enclosure WHERE article_id = ?1 ORDER BY id",
        )?;
        detail.enclosures = stmt
            .query_map([id], |r| {
                let kind: String = r.get(4)?;
                Ok(Enclosure {
                    url: r.get(0)?,
                    mime_type: r.get(1)?,
                    length: r.get(2)?,
                    duration_secs: r.get(3)?,
                    kind: EnclosureKind::from_db(&kind),
                    player_url: r.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(detail)
    }

    pub fn set_read(&self, id: i64, read: bool) -> Result<()> {
        self.conn().execute(
            "UPDATE article SET is_read = ?2 WHERE id = ?1",
            params![id, read],
        )?;
        Ok(())
    }

    pub fn set_starred(&self, id: i64, starred: bool) -> Result<()> {
        self.conn().execute(
            "UPDATE article SET is_starred = ?2 WHERE id = ?1",
            params![id, starred],
        )?;
        Ok(())
    }

    /// Marks every Article in the view as Read. Returns their ids so the action can be undone.
    pub fn mark_all_read(&self, q: &ListQuery) -> Result<Vec<i64>> {
        let (clause, args) = filter(q);
        let tx = self.conn().unchecked_transaction()?;
        let ids: Vec<i64> = tx
            .prepare(&format!(
                "SELECT a.id FROM article a WHERE a.is_read = 0 AND {clause}"
            ))?
            .query_map(rusqlite::params_from_iter(args), |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        {
            let mut stmt = tx.prepare("UPDATE article SET is_read = 1 WHERE id = ?1")?;
            for id in &ids {
                stmt.execute([id])?;
            }
        }
        tx.commit()?;
        Ok(ids)
    }

    /// Undo for `mark_all_read`.
    pub fn mark_unread(&self, ids: &[i64]) -> Result<()> {
        let tx = self.conn().unchecked_transaction()?;
        {
            let mut stmt = tx.prepare("UPDATE article SET is_read = 0 WHERE id = ?1")?;
            for id in ids {
                stmt.execute([id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feeds::FetchResult;
    use crate::library::DIR_NAME;
    use crate::opml::OpmlFeed;
    use crate::parse::{EnclosureKind, ParsedEnclosure, ParsedEntry, ParsedFeed};

    fn entry(identity: &str, title: &str, published_at: Option<i64>) -> ParsedEntry {
        ParsedEntry {
            identity: identity.into(),
            url: Some(format!("https://x.example/{identity}")),
            title: title.into(),
            author: None,
            content_html: format!("<p>{title} body</p>"),
            content_text: format!("{title} body"),
            excerpt: format!("{title} body"),
            published_at,
            images: vec![],
            lead_image: None,
            enclosures: vec![],
            video: None,
        }
    }

    fn fetched(entries: Vec<ParsedEntry>) -> FetchResult {
        FetchResult::Fetched {
            feed: ParsedFeed {
                title: None,
                site_url: None,
                entries,
            },
            etag: None,
            last_modified: None,
        }
    }

    fn library_with_feed() -> (tempfile::TempDir, Library) {
        let root = tempfile::tempdir().unwrap();
        let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
        library
            .import_opml(&[OpmlFeed {
                url: "https://x.example/feed".into(),
                title: "X".into(),
                site_url: None,
                folder: Some("F".into()),
            }])
            .unwrap();
        (root, library)
    }

    #[test]
    fn article_date_rules() {
        let now = 1_800_000_000;
        assert_eq!(article_date(Some(1_700_000_000), now), 1_700_000_000);
        assert_eq!(article_date(None, now), now, "missing → first seen");
        assert_eq!(article_date(Some(now + 3600), now), now, "future → clamped");
        assert_eq!(article_date(Some(0), now), now, "epoch zero is bogus");
    }

    #[test]
    fn edits_keep_state_and_views_filter() {
        let (_root, library) = library_with_feed();
        let outcome = library
            .apply_fetch(
                1,
                fetched(vec![
                    entry("a", "Alpha", Some(1_700_000_000)),
                    entry("b", "Beta", Some(1_700_000_100)),
                ]),
            )
            .unwrap();
        assert_eq!(outcome.new_articles, 2);

        let all = ListQuery {
            view: View::AllUnread,
            sort: Sort::NewestFirst,
            search: None,
            with_lead_image: false,
        };
        let rows = library.list_articles(&all, 0, 50).unwrap();
        assert_eq!(
            rows.iter().map(|r| r.title.as_str()).collect::<Vec<_>>(),
            ["Beta", "Alpha"]
        );

        library.set_read(rows[1].id, true).unwrap();
        library.set_starred(rows[1].id, true).unwrap();

        // Re-fetch with an edited title: no new Article, state kept.
        let outcome = library
            .apply_fetch(
                1,
                fetched(vec![entry("a", "Alpha (updated)", Some(1_700_000_000))]),
            )
            .unwrap();
        assert_eq!(outcome.new_articles, 0);
        let detail = library.article(rows[1].id).unwrap();
        assert_eq!(detail.title, "Alpha (updated)");
        assert!(detail.is_read && detail.is_starred);

        assert_eq!(library.count_articles(&all).unwrap(), 1);
        let starred = ListQuery {
            view: View::Starred,
            ..all.clone()
        };
        assert_eq!(library.count_articles(&starred).unwrap(), 1);
        let folder = ListQuery {
            view: View::Folder { id: 1 },
            ..all.clone()
        };
        assert_eq!(library.count_articles(&folder).unwrap(), 2);
        let oldest = ListQuery {
            sort: Sort::OldestFirst,
            ..folder
        };
        assert_eq!(
            library.list_articles(&oldest, 0, 1).unwrap()[0].title,
            "Alpha (updated)"
        );

        let sidebar = library.sidebar(0).unwrap();
        assert_eq!((sidebar.all_unread, sidebar.starred), (1, 1));
        assert_eq!(sidebar.folders[0].unread, 1);
        library.close().unwrap();
    }

    #[test]
    fn enclosures_round_trip_with_metadata() {
        // The Archivist (Henrik) checks episode length at a glance; the Offline Reader (Paul)
        // relies on the download staying a plain local link.
        let (_root, library) = library_with_feed();
        let mut e = entry("a", "Alpha", None);
        e.enclosures = vec![
            ParsedEnclosure {
                url: "https://x.example/ep12.mp3".into(),
                player_url: None,
                mime_type: Some("audio/mpeg".into()),
                length: Some(12_000_000),
                duration_secs: Some(754),
                kind: EnclosureKind::Audio,
            },
            ParsedEnclosure {
                url: "https://x.example/watch?v=1".into(),
                player_url: Some("https://x.example/watch?v=1".into()),
                mime_type: None,
                length: None,
                duration_secs: None,
                kind: EnclosureKind::Document,
            },
        ];
        library.apply_fetch(1, fetched(vec![e])).unwrap();
        let rows = library
            .list_articles(
                &ListQuery {
                    view: View::AllUnread,
                    sort: Sort::NewestFirst,
                    search: None,
                    with_lead_image: false,
                },
                0,
                10,
            )
            .unwrap();
        let detail = library.article(rows[0].id).unwrap();
        assert_eq!(detail.enclosures.len(), 2);
        assert_eq!(
            detail.enclosures[0],
            Enclosure {
                url: "https://x.example/ep12.mp3".into(),
                mime_type: Some("audio/mpeg".into()),
                length: Some(12_000_000),
                duration_secs: Some(754),
                kind: EnclosureKind::Audio,
                player_url: None,
            }
        );
        assert_eq!(detail.enclosures[1].kind, EnclosureKind::Document);
        assert_eq!(
            detail.enclosures[1].player_url.as_deref(),
            Some("https://x.example/watch?v=1")
        );
        library.close().unwrap();
    }

    #[test]
    fn mark_all_read_is_undoable_and_search_works() {
        let (_root, library) = library_with_feed();
        library
            .apply_fetch(
                1,
                fetched(vec![
                    entry("a", "Spinnfäden", None),
                    entry("b", "Honey", None),
                ]),
            )
            .unwrap();
        let all = ListQuery {
            view: View::AllUnread,
            sort: Sort::NewestFirst,
            search: None,
            with_lead_image: false,
        };
        let ids = library.mark_all_read(&all).unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(library.count_articles(&all).unwrap(), 0);
        library.mark_unread(&ids).unwrap();
        assert_eq!(library.count_articles(&all).unwrap(), 2);

        let search = |s: &str| ListQuery {
            view: View::Search {
                feed: None,
                folder: None,
            },
            sort: Sort::NewestFirst,
            search: Some(s.into()),
            with_lead_image: false,
        };
        assert_eq!(
            library.count_articles(&search("spinn")).unwrap(),
            1,
            "prefix + diacritics"
        );
        assert_eq!(
            library.count_articles(&search("hon bod")).unwrap(),
            1,
            "all terms"
        );
        assert_eq!(
            library.count_articles(&search("\"); DROP")).unwrap(),
            0,
            "input is escaped"
        );

        // Spec "Search": scoped to everything, a Folder or a Feed.
        let feed: i64 = library
            .conn()
            .query_row("SELECT feed_id FROM article LIMIT 1", [], |r| r.get(0))
            .unwrap();
        let scoped = |feed: Option<i64>, folder: Option<i64>| ListQuery {
            view: View::Search { feed, folder },
            ..search("spinn")
        };
        assert_eq!(
            library.count_articles(&scoped(Some(feed), None)).unwrap(),
            1
        );
        assert_eq!(
            library
                .count_articles(&scoped(Some(feed + 99), None))
                .unwrap(),
            0,
            "another Feed"
        );
        assert_eq!(
            library.count_articles(&scoped(None, Some(99))).unwrap(),
            0,
            "a Folder without that Feed"
        );
        let wire: View = serde_json::from_str(r#"{"kind":"search"}"#).unwrap();
        assert!(
            matches!(
                wire,
                View::Search {
                    feed: None,
                    folder: None
                }
            ),
            "unscoped on the wire"
        );
        library.close().unwrap();
    }
}

/// Edge cases around the Article Date rules, FTS query building and view filters.
#[cfg(test)]
mod edge_tests {
    use super::*;
    use crate::feeds::FetchResult;
    use crate::library::DIR_NAME;
    use crate::opml::OpmlFeed;
    use crate::parse::{ParsedEnclosure, ParsedEntry, ParsedFeed};

    fn entry(identity: &str, title: &str, published_at: Option<i64>) -> ParsedEntry {
        ParsedEntry {
            identity: identity.into(),
            url: Some(format!("https://x.example/{identity}")),
            title: title.into(),
            author: None,
            content_html: format!("<p>{title}</p>"),
            content_text: title.into(),
            excerpt: title.into(),
            published_at,
            images: vec![],
            lead_image: None,
            enclosures: vec![],
        }
    }

    fn fetched(entries: Vec<ParsedEntry>) -> FetchResult {
        FetchResult::Fetched {
            feed: ParsedFeed {
                title: None,
                site_url: None,
                entries,
            },
            etag: None,
            last_modified: None,
        }
    }

    fn library_with_feed() -> (tempfile::TempDir, Library) {
        let root = tempfile::tempdir().unwrap();
        let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
        library
            .import_opml(&[OpmlFeed {
                url: "https://x.example/feed".into(),
                title: "X".into(),
                site_url: None,
                folder: None,
            }])
            .unwrap();
        (root, library)
    }

    #[test]
    fn article_date_boundaries() {
        let now = 1_800_000_000;
        assert_eq!(
            article_date(Some(EARLIEST_PLAUSIBLE), now),
            EARLIEST_PLAUSIBLE,
            "the boundary itself is plausible"
        );
        assert_eq!(
            article_date(Some(EARLIEST_PLAUSIBLE - 1), now),
            now,
            "one second before 1995 is bogus"
        );
        assert_eq!(
            article_date(Some(now), now),
            now,
            "exactly first-seen is allowed"
        );
        assert_eq!(article_date(Some(now + 1), now), now, "future is clamped");
    }

    #[test]
    fn fts_query_builds_a_safe_prefix_query() {
        assert_eq!(fts_query(""), "\"\"");
        assert_eq!(fts_query("   "), "\"\"");
        assert_eq!(fts_query("hello"), "\"hello\"*");
        assert_eq!(fts_query("hello  world"), "\"hello\"* \"world\"*");
        assert_eq!(
            fts_query("he\"llo; DROP TABLE x--"),
            "\"hello\"* \"DROP\"* \"TABLE\"* \"x\"*",
            "quotes, punctuation and SQL keywords become plain terms"
        );
        assert_eq!(fts_query("café"), "\"café\"*", "unicode letters are kept");
    }

    #[test]
    fn today_view_and_the_lead_image_filter() {
        let (_root, library) = library_with_feed();
        let now = crate::lock::now();
        let mut with_image = entry("img", "With image", Some(now - 100));
        with_image.lead_image = Some("https://x.example/p.png".into());
        with_image.images = vec!["https://x.example/p.png".into()];
        library
            .apply_fetch(
                1,
                fetched(vec![
                    with_image,
                    entry("old", "Old", Some(1_000_000_000)),
                    entry("fresh", "Fresh", None),
                ]),
            )
            .unwrap();

        let today = ListQuery {
            view: View::Today { since: now - 200 },
            sort: Sort::NewestFirst,
            search: None,
            with_lead_image: false,
        };
        let titles: Vec<_> = library
            .list_articles(&today, 0, 50)
            .unwrap()
            .into_iter()
            .map(|r| r.title)
            .collect();
        assert_eq!(titles, ["Fresh", "With image"], "only today's Articles");

        let gallery = ListQuery {
            with_lead_image: true,
            ..today
        };
        let rows = library.list_articles(&gallery, 0, 50).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "With image");
        assert!(rows[0].lead_image.is_some());
        library.close().unwrap();
    }

    #[test]
    fn article_detail_carries_enclosures_and_a_missing_id_is_not_found() {
        let (_root, library) = library_with_feed();
        let mut e = entry("pod", "Podcast", None);
        e.enclosures = vec![ParsedEnclosure {
            url: "https://x.example/ep.mp3".into(),
            mime_type: Some("audio/mpeg".into()),
            length: Some(42),
        }];
        library.apply_fetch(1, fetched(vec![e])).unwrap();

        let detail = library.article(1).unwrap();
        assert_eq!(detail.enclosures.len(), 1);
        assert_eq!(detail.enclosures[0].url, "https://x.example/ep.mp3");
        assert_eq!(detail.enclosures[0].length, Some(42));

        assert!(matches!(library.article(999), Err(Error::NotFound)));
        library.close().unwrap();
    }

    #[test]
    fn mark_all_read_on_an_already_read_view_returns_no_ids() {
        let (_root, library) = library_with_feed();
        library
            .apply_fetch(1, fetched(vec![entry("a", "A", None)]))
            .unwrap();
        let all = ListQuery {
            view: View::AllUnread,
            sort: Sort::NewestFirst,
            search: None,
            with_lead_image: false,
        };
        library.set_read(1, true).unwrap();
        assert!(library.mark_all_read(&all).unwrap().is_empty());
        library.close().unwrap();
    }

    #[test]
    fn search_prefers_the_feed_scope_when_both_are_set() {
        let (_root, library) = library_with_feed();
        library
            .apply_fetch(1, fetched(vec![entry("a", "Spinnfäden", None)]))
            .unwrap();
        let q = ListQuery {
            view: View::Search {
                feed: Some(1),
                folder: Some(999),
            },
            sort: Sort::NewestFirst,
            search: Some("spinn".into()),
            with_lead_image: false,
        };
        assert_eq!(
            library.count_articles(&q).unwrap(),
            1,
            "the Feed scope wins over the Folder scope"
        );
        library.close().unwrap();
    }
}
