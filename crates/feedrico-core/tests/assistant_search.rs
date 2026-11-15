//! Natural-language search end to end (#122): build the SearchQuery task from a Library's
//! Folder/Feed names, run it against a fake CLI (a shell script standing in for `claude`/`vibe`,
//! no real CLI or network), validate the structured answer, and run the resulting local search
//! against a real Library.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::Duration;

use feedrico_core::assistant::run::{Cancel, run};
use feedrico_core::assistant::{AssistantKind, FoundAssistant, SignIn, search};
use feedrico_core::feeds::FetchResult;
use feedrico_core::library::DIR_NAME;
use feedrico_core::opml::OpmlFeed;
use feedrico_core::parse::{ParsedEntry, ParsedFeed};
use feedrico_core::{Library, View};
use serde_json::json;

/// A fake CLI plus a directory where it records what it saw.
struct Fake {
    _dir: tempfile::TempDir,
    rec: PathBuf,
    assistant: FoundAssistant,
}

impl Fake {
    /// `body` runs after the recording prelude; the prompt arrives on stdin.
    fn new(kind: AssistantKind, body: &str) -> Fake {
        let dir = tempfile::tempdir().unwrap();
        let rec = dir.path().join("rec");
        std::fs::create_dir(&rec).unwrap();
        let script = format!(
            "#!/bin/sh\nREC='{rec}'\nprintf '%s\\n' \"$@\" > \"$REC/argv\"\ncat > \"$REC/stdin\"\n{body}\n",
            rec = rec.display()
        );
        let path = dir.path().join(kind.binary_name());
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        Fake {
            _dir: dir,
            rec,
            assistant: FoundAssistant {
                kind,
                path,
                version: None,
                sign_in: SignIn::Plan,
                models: Vec::new(),
            },
        }
    }

    /// A fake Claude Code that answers with `structured_output` in a `result` line.
    fn claude(structured_output: serde_json::Value) -> Fake {
        let result = serde_json::to_string(&structured_output).unwrap();
        Fake::new(
            AssistantKind::ClaudeCode,
            &format!(
                "cat <<'EOF'\n{{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\
                 \"result\":{result:?},\"structured_output\":{structured_output}}}\nEOF\nexit 0"
            ),
        )
    }

    /// A fake Mistral Vibe whose answer is plain JSON text in one assistant message.
    fn vibe(answer: &str) -> Fake {
        let text = serde_json::to_string(answer).unwrap();
        Fake::new(
            AssistantKind::MistralVibe,
            &format!(
                "cat <<'EOF'\n{{\"id\":\"a1\",\"type\":\"message\",\"role\":\"assistant\",\
                 \"content\":[{{\"type\":\"text\",\"text\":{text}}}]}}\nEOF\nexit 0"
            ),
        )
    }

    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.rec.join(name)).unwrap()
    }
}

fn entry(identity: &str, title: &str, published_at: i64, content_text: &str) -> ParsedEntry {
    ParsedEntry {
        identity: identity.into(),
        url: Some(format!("https://x.example/{identity}")),
        title: title.into(),
        author: None,
        content_html: format!("<p>{content_text}</p>"),
        content_text: content_text.into(),
        excerpt: content_text.into(),
        published_at: Some(published_at),
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

/// A Library with a Folder "Tech" holding "Rust Blog" (one Article from spring 2025) and a
/// top-level "Hacker News" (one Article from January 2026).
fn library() -> (tempfile::TempDir, Library) {
    let root = tempfile::tempdir().unwrap();
    let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
    library
        .import_opml(&[
            OpmlFeed {
                url: "https://rust.example/feed".into(),
                title: "Rust Blog".into(),
                site_url: None,
                folder: Some("Tech".into()),
            },
            OpmlFeed {
                url: "https://hn.example/feed".into(),
                title: "Hacker News".into(),
                site_url: None,
                folder: None,
            },
        ])
        .unwrap();
    library
        .apply_fetch(
            1,
            fetched(vec![entry(
                "wal",
                "SQLite WAL mode explained",
                1_743_552_000, // 2025-04-02
                "write ahead log and checkpoints",
            )]),
        )
        .unwrap();
    library
        .apply_fetch(
            2,
            fetched(vec![entry(
                "hn",
                "Show HN: a thing",
                1_768_003_200, // 2026-01-10
                "some project",
            )]),
        )
        .unwrap();
    (root, library)
}

const QUESTION: &str = "that post about SQLite WAL mode from last spring";

#[tokio::test]
async fn claude_search_query_runs_end_to_end_against_the_library() {
    let (_root, library) = library();
    let names = library.folder_feed_names().unwrap();
    let task = search::task(QUESTION, &names.folders, &names.feeds, "2026-11-14");
    let fake = Fake::claude(json!({
        "query": "title:\"wal\"* AND sqlite",
        "since": "2025-03-01",
        "until": "2025-05-31",
        "feed": "Rust Blog",
        "folder": null
    }));
    let (text, structured) = run(&fake.assistant, &task, Cancel::new(), |_| {})
        .await
        .unwrap();
    let out =
        search::parse_output(&text, structured.as_ref(), &names.folders, &names.feeds).unwrap();
    assert_eq!(out.feed, Some(1), "the Feed name resolved to its id");
    assert_eq!(out.since, search::parse_ymd("2025-03-01"));
    assert_eq!(
        out.until,
        search::parse_ymd("2025-05-31").map(|d| d + 86_399)
    );
    assert!(out.dropped.is_empty());

    // The structured output maps to the search call, and the search runs locally.
    let q = out.to_list_query();
    assert!(matches!(q.view, View::Search { fts: true, .. }));
    assert_eq!(library.count_articles(&q).unwrap(), 1);
    let rows = library.list_articles(&q, 0, 10).unwrap();
    assert_eq!(rows[0].title, "SQLite WAL mode explained");

    // Only the question and the Folder/Feed names went to the CLI, never Article content.
    let stdin = fake.read("stdin");
    assert!(stdin.contains("<<<FEEDRICO_DATA Question>>>"));
    assert!(stdin.contains(QUESTION));
    assert!(stdin.contains("<<<FEEDRICO_DATA Folders in this Library>>>\nTech"));
    assert!(stdin.contains("<<<FEEDRICO_DATA Feeds in this Library>>>\nRust Blog\nHacker News"));
    assert!(
        !stdin.contains("write ahead log"),
        "Article content must not be sent"
    );
    assert!(
        !stdin.contains("SQLite WAL mode explained"),
        "the Article title is content too"
    );
    // The JSON schema went along for Claude Code.
    let argv = fake.read("argv");
    assert!(argv.contains("--json-schema"));
    assert!(argv.contains("\"query\""));
    library.close().unwrap();
}

#[tokio::test]
async fn vibe_text_only_answer_is_validated_the_same_way() {
    let (_root, library) = library();
    let names = library.folder_feed_names().unwrap();
    let task = search::task(QUESTION, &names.folders, &names.feeds, "2026-11-14");
    let fake = Fake::vibe("{\"query\": \"\\\"wal\\\"*\", \"feed\": \"Rust Blog\"}");
    let (text, structured) = run(&fake.assistant, &task, Cancel::new(), |_| {})
        .await
        .unwrap();
    assert!(
        structured.is_none(),
        "Vibe has no structured output channel"
    );
    let out =
        search::parse_output(&text, structured.as_ref(), &names.folders, &names.feeds).unwrap();
    assert_eq!(out.query, "\"wal\"*");
    assert_eq!(out.feed, Some(1));
    assert_eq!(library.count_articles(&out.to_list_query()).unwrap(), 1);
    library.close().unwrap();
}

#[tokio::test]
async fn an_invalid_generated_query_and_an_unknown_scope_still_search_locally() {
    let (_root, library) = library();
    let names = library.folder_feed_names().unwrap();
    let task = search::task(QUESTION, &names.folders, &names.feeds, "2026-11-14");
    // Unbalanced quote: not valid FTS5. Unknown Feed name: dropped.
    let fake = Fake::claude(json!({
        "query": "\"wal",
        "since": null,
        "until": null,
        "feed": "No Such Feed",
        "folder": null
    }));
    let (text, structured) = run(&fake.assistant, &task, Cancel::new(), |_| {})
        .await
        .unwrap();
    let out =
        search::parse_output(&text, structured.as_ref(), &names.folders, &names.feeds).unwrap();
    assert_eq!(out.feed, None);
    assert_eq!(out.dropped, vec!["No Such Feed".to_string()]);
    // Invalid FTS falls back to the escaped plain-text query, which still finds the Article.
    let q = out.to_list_query();
    assert_eq!(
        library.count_articles(&q).unwrap(),
        1,
        "fallback to plain text"
    );
    library.close().unwrap();
}

#[tokio::test]
async fn a_timeout_on_the_search_task_maps_to_timed_out() {
    let (_root, library) = library();
    let names = library.folder_feed_names().unwrap();
    let mut task = search::task(QUESTION, &names.folders, &names.feeds, "2026-11-14");
    task.timeout = Duration::from_millis(300);
    let fake = Fake::new(AssistantKind::ClaudeCode, "sleep 30");
    let err = run(&fake.assistant, &task, Cancel::new(), |_| {})
        .await
        .unwrap_err();
    assert_eq!(
        err,
        feedrico_core::assistant::AssistantError::TimedOut,
        "a slow Assistant must not hang the search"
    );
    library.close().unwrap();
}
