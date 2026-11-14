//! Shared fixture: a small Library built through the core API.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use feedrico_core::Library;
use feedrico_core::feeds::FetchResult;
use feedrico_core::parse::{ParsedEntry, ParsedFeed};

pub fn entry(id: &str, title: &str, html: &str, published_at: i64) -> ParsedEntry {
    ParsedEntry {
        identity: id.into(),
        url: Some(format!("https://example.org/{id}")),
        title: title.into(),
        author: Some("Ada".into()),
        content_html: html.into(),
        content_text: feedrico_core::sanitize::plain_text(html),
        excerpt: feedrico_core::sanitize::plain_text(html),
        published_at: Some(published_at),
        images: vec![],
        lead_image: None,
        enclosures: vec![],
        video: None,
    }
}

/// Two Feeds (one in a Folder), three Articles, one starred, one read. Closed, unlocked.
pub fn fixture(root: &Path) -> PathBuf {
    let dir = root.join("Feedrico Library");
    let lib = Library::create(&dir).unwrap();
    let a = lib
        .add_feed("https://rust.example/feed", "Rust Blog", None, None)
        .unwrap();
    let b = lib
        .add_feed("https://cooking.example/feed", "Cooking", None, None)
        .unwrap();
    let (a, b) = match (a, b) {
        (
            feedrico_core::AddOutcome::Added { feed_id: a },
            feedrico_core::AddOutcome::Added { feed_id: b },
        ) => (a, b),
        _ => panic!("expected new feeds"),
    };
    let now = 1_700_000_000;
    for (feed, entries) in [
        (
            a,
            vec![
                entry(
                    "r1",
                    "Async Rust news",
                    "<p>Borrow <b>checker</b> &amp; async.</p><script>x()</script>",
                    now,
                ),
                entry("r2", "Rust 2030", "<p>Editions arrive.</p>", now - 100),
            ],
        ),
        (
            b,
            vec![entry(
                "c1",
                "Sourdough",
                "<p>Flour and water.</p>",
                now - 200,
            )],
        ),
    ] {
        lib.apply_fetch(
            feed,
            FetchResult::Fetched {
                feed: ParsedFeed {
                    title: None,
                    site_url: None,
                    entries,
                },
                etag: None,
                last_modified: None,
            },
        )
        .unwrap();
    }
    let rows = |q: &str| -> Vec<i64> {
        lib.list_articles(
            &feedrico_core::ListQuery {
                view: feedrico_core::View::Search {
                    feed: None,
                    folder: None,
                },
                sort: Default::default(),
                search: Some(q.into()),
                with_lead_image: false,
            },
            0,
            10,
        )
        .unwrap()
        .iter()
        .map(|r| r.id)
        .collect()
    };
    lib.set_starred(rows("async")[0], true).unwrap();
    lib.set_read(rows("sourdough")[0], true).unwrap();
    lib.close().unwrap();
    dir
}

pub fn hash_tree(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push((
                    p.strip_prefix(dir).unwrap().to_string_lossy().into_owned(),
                    fs::read(&p).unwrap(),
                ));
            }
        }
    }
    out.sort();
    out
}
