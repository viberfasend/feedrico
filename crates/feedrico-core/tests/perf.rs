//! Query benchmarks against a generated Library of 300 Feeds and 50,000 Articles
//! (spec: "Performance budgets"). Not part of the normal run:
//!
//! ```sh
//! cargo test -p feedrico-core --release --test perf -- --ignored --nocapture
//! ```
//!
//! `FEEDRICO_PERF_OUT=/some/dir` additionally keeps the generated Library for the UI harness.
//! Budgets are reported; they only fail the run when `FEEDRICO_PERF_STRICT=1`, because shared CI
//! machines are noisy ("report, not gate").

use std::time::{Duration, Instant};

use feedrico_core::library::DIR_NAME;
use feedrico_core::{Library, ListQuery, Sort, View};

const FEEDS: i64 = 300;
const ARTICLES: i64 = 50_000;
const FOLDERS: i64 = 20;
const SWITCH_BUDGET: Duration = Duration::from_millis(50);

/// Deterministic xorshift, so every run measures the same Library.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

const WORDS: &[&str] = &[
    "rust",
    "tauri",
    "reader",
    "feed",
    "library",
    "design",
    "browser",
    "database",
    "privacy",
    "weekend",
    "garden",
    "kernel",
    "typography",
    "release",
    "benchmark",
    "summer",
    "coffee",
    "network",
    "archive",
    "silk",
];

fn generate(library: &Library) {
    let conn = library.conn();
    let tx = conn.unchecked_transaction().unwrap();
    for f in 1..=FOLDERS {
        tx.execute(
            "INSERT INTO folder (id, name, position) VALUES (?1, ?2, ?1)",
            rusqlite::params![f, format!("Folder {f}")],
        )
        .unwrap();
    }
    for f in 1..=FEEDS {
        let folder =
            (f % (FOLDERS + 5) <= FOLDERS && f % (FOLDERS + 5) > 0).then_some(f % (FOLDERS + 5));
        tx.execute(
            "INSERT INTO feed (id, url, title, folder_id, position) VALUES (?1, ?2, ?3, ?4, ?1)",
            rusqlite::params![
                f,
                format!("https://feed{f}.example/rss"),
                format!("Feed {f}"),
                folder
            ],
        )
        .unwrap();
    }
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let now = 1_800_000_000_i64;
    let mut insert = tx
        .prepare(
            "INSERT INTO article (feed_id, identity, url, title, content_html, content_text, excerpt,
                                  published_at, first_seen_at, article_date, is_read, is_starred, lead_image)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, ?7, ?7, ?7, ?8, ?9, ?10)",
        )
        .unwrap();
    for i in 0..ARTICLES {
        let feed = 1 + (rng.next() % FEEDS as u64) as i64;
        let mut words = || -> String {
            (0..60)
                .map(|_| WORDS[(rng.next() % WORDS.len() as u64) as usize])
                .collect::<Vec<_>>()
                .join(" ")
        };
        let mut body = words();
        if (i as u64).is_multiple_of(100) {
            body.push_str(" zephyrine"); // a rare term for realistic search
        }
        let date = now - (i * 600);
        let read = rng.next() % 10 < 7; // ~30% unread
        let starred = rng.next() % 100 < 2;
        let lead = rng
            .next()
            .is_multiple_of(3)
            .then(|| format!("{:032x}", rng.next()));
        insert
            .execute(rusqlite::params![
                feed,
                format!("id-{i}"),
                format!("https://example.org/{i}"),
                body.split(' ').take(6).collect::<Vec<_>>().join(" "),
                body,
                body.chars().take(220).collect::<String>(),
                date,
                read,
                starred,
                lead,
            ])
            .unwrap();
    }
    drop(insert);
    tx.commit().unwrap();
}

fn time<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let value = f();
    (value, start.elapsed())
}

#[test]
#[ignore = "benchmark: run explicitly, see the module docs"]
fn query_budgets() {
    let root = tempfile::tempdir().unwrap();
    let dir = match std::env::var_os("FEEDRICO_PERF_OUT") {
        Some(out) => std::path::PathBuf::from(out).join(DIR_NAME),
        None => root.path().join(DIR_NAME),
    };
    let library = Library::create(&dir).unwrap();
    let (_, generated) = time(|| generate(&library));
    println!("generated {FEEDS} Feeds / {ARTICLES} Articles in {generated:?}");

    let q = |view, search: Option<&str>, lead| ListQuery {
        view,
        sort: Sort::NewestFirst,
        search: search.map(Into::into),
        with_lead_image: lead,
    };
    let cases: Vec<(&str, ListQuery)> = vec![
        ("All Unread", q(View::AllUnread, None, false)),
        ("Starred", q(View::Starred, None, false)),
        (
            "Today",
            q(
                View::Today {
                    since: 1_800_000_000 - 86_400,
                },
                None,
                false,
            ),
        ),
        ("Feed", q(View::Feed { id: 42 }, None, false)),
        ("Folder", q(View::Folder { id: 3 }, None, false)),
        (
            "Gallery (All Unread, Lead Image)",
            q(View::AllUnread, None, true),
        ),
        (
            "Search, rare term",
            q(View::Search, Some("zephyrine"), false),
        ),
        // Worst case: matches ~90% of the Library, so counting alone touches every row.
        (
            "Search, common words",
            q(View::Search, Some("rust benchmark"), false),
        ),
    ];

    let mut failures = Vec::new();
    println!(
        "{:<36} {:>10} {:>12}",
        "switch to view (count + page 1)", "rows", "time"
    );
    for (name, query) in &cases {
        // Best of three: the first run warms SQLite's page cache like a real session would.
        let mut best = Duration::MAX;
        let mut total = 0;
        for _ in 0..3 {
            let ((count, _), elapsed) = time(|| {
                let count = library.count_articles(query).unwrap();
                let page = library.list_articles(query, 0, 100).unwrap();
                (count, page)
            });
            total = count;
            best = best.min(elapsed);
        }
        println!("{name:<36} {total:>10} {best:>12.2?}");
        if best > SWITCH_BUDGET {
            failures.push(format!("{name}: {best:?}"));
        }
    }

    let (_, sidebar) = time(|| library.sidebar(1_800_000_000 - 86_400).unwrap());
    println!(
        "{:<36} {:>10} {sidebar:>12.2?}",
        "sidebar (counts for every Feed)", "-"
    );
    if sidebar > SWITCH_BUDGET {
        failures.push(format!("sidebar: {sidebar:?}"));
    }

    let (_, deep) = time(|| {
        library
            .list_articles(&cases[0].1, ARTICLES / 3, 100)
            .unwrap()
    });
    println!(
        "{:<36} {:>10} {deep:>12.2?}",
        "deep page (offset ~16k)", "-"
    );
    if deep > SWITCH_BUDGET {
        failures.push(format!("deep page: {deep:?}"));
    }

    let (_, detail) = time(|| library.article(1234).unwrap());
    println!("{:<36} {:>10} {detail:>12.2?}", "open one Article", "-");

    let (_, mark) = time(|| library.set_read(1234, true).unwrap());
    println!("{:<36} {:>10} {mark:>12.2?}", "mark one Article read", "-");

    if std::env::var("FEEDRICO_PERF_STRICT").as_deref() == Ok("1") {
        assert!(failures.is_empty(), "over the 50 ms budget: {failures:?}");
    } else if !failures.is_empty() {
        println!("OVER BUDGET (not gating): {failures:?}");
    }
    library.close().unwrap();
}
