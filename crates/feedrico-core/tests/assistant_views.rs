//! Briefing and picks against fake CLIs and a real temporary Library (#120, #121). The fake
//! scripts stand in for `claude`/`vibe` (see `tests/assistant_run.rs`); no real CLI or network
//! is used. Besides the end-to-end result, these tests prove that a run never changes Read or
//! Starred in the Library.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use feedrico_core::assistant::run::{Cancel, run};
use feedrico_core::assistant::{AssistantKind, FoundAssistant, SignIn, briefing, picks};
use feedrico_core::feeds::FetchResult;
use feedrico_core::library::DIR_NAME;
use feedrico_core::opml::OpmlFeed;
use feedrico_core::parse::{ParsedEntry, ParsedFeed};
use feedrico_core::{Library, ListQuery, Sort, View};

/// A fake CLI plus the directory where it records what it saw (stdin, argv).
struct Fake {
    _dir: tempfile::TempDir,
    rec: PathBuf,
    assistant: FoundAssistant,
}

impl Fake {
    /// `body` runs after `cat > "$REC/stdin"`.
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
            },
        }
    }

    fn stdin(&self) -> String {
        std::fs::read_to_string(self.rec.join("stdin")).unwrap()
    }

    fn argv(&self) -> String {
        std::fs::read_to_string(self.rec.join("argv")).unwrap()
    }
}

/// A fake Claude Code that answers with `structured_output`.
fn fake_claude(structured: serde_json::Value) -> Fake {
    let line = serde_json::json!({
        "type": "result", "subtype": "success", "is_error": false,
        "result": structured.to_string(), "structured_output": structured,
    });
    Fake::new(
        AssistantKind::ClaudeCode,
        &format!("cat <<'EOF'\n{line}\nEOF\nexit 0"),
    )
}

/// A fake Mistral Vibe whose last message entry holds `text`.
fn fake_vibe(text: &str) -> Fake {
    let entry = serde_json::json!({
        "id": "a1", "type": "message", "role": "assistant", "generationStatus": "completed",
        "content": [{ "type": "text", "text": text }],
    });
    Fake::new(
        AssistantKind::MistralVibe,
        &format!("cat <<'EOF'\n{entry}\nEOF\nexit 0"),
    )
}

fn entry(identity: &str, title: &str, published_at: i64) -> ParsedEntry {
    ParsedEntry {
        identity: identity.into(),
        url: Some(format!("https://x.example/{identity}")),
        title: title.into(),
        author: None,
        content_html: format!("<p>{title} full body, never sent</p>"),
        content_text: format!("{title} full body, never sent"),
        excerpt: format!("{title} excerpt"),
        published_at: Some(published_at),
        images: vec![],
        lead_image: None,
        enclosures: vec![],
        video: None,
    }
}

/// A Library with one Feed (id 1, in Folder F) and `n` Articles, ids 1..=n, newest last.
fn library_with(n: usize) -> (tempfile::TempDir, Library) {
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
    let entries = (0..n)
        .map(|i| {
            entry(
                &format!("e{i}"),
                &format!("Article {i}"),
                1_700_000_000 + i as i64,
            )
        })
        .collect();
    library
        .apply_fetch(
            1,
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
    (root, library)
}

fn feed_query(sort: Sort) -> ListQuery {
    ListQuery {
        view: View::Feed { id: 1 },
        sort,
        search: None,
        with_lead_image: false,
    }
}

fn unread_query() -> ListQuery {
    ListQuery {
        view: View::AllUnread,
        sort: Sort::NewestFirst,
        search: None,
        with_lead_image: false,
    }
}

fn starred_query() -> ListQuery {
    ListQuery {
        view: View::Starred,
        sort: Sort::NewestFirst,
        search: None,
        with_lead_image: false,
    }
}

/// Read/Starred fingerprint of the whole Library, to prove a run changed nothing.
fn state_fingerprint(library: &Library) -> (i64, i64, Vec<(i64, bool, bool)>) {
    let conn = library.conn();
    let rows: Vec<(i64, bool, bool)> = conn
        .prepare("SELECT id, is_read, is_starred FROM article ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    (
        library.count_articles(&unread_query()).unwrap(),
        library.count_articles(&starred_query()).unwrap(),
        rows,
    )
}

#[tokio::test]
async fn briefing_end_to_end_with_fake_claude() {
    let (_root, library) = library_with(3);
    library.set_read(1, true).unwrap();
    library.set_starred(2, true).unwrap();
    let before = state_fingerprint(&library);

    let plan = briefing::plan(&library, &feed_query(Sort::NewestFirst)).unwrap();
    assert_eq!(plan.considered, 3);

    // The Assistant names one real id, one unknown id, and repeats one.
    let structured = serde_json::json!({
        "topics": [
            { "topic": "Rust", "sentence": "Three posts about Rust.", "articleIds": [3, 999_999, 2] },
            { "topic": "Ghost", "sentence": "Only unknown ids.", "articleIds": [42_424_242] }
        ]
    });
    let fake = fake_claude(structured.clone());
    let (text, got) = run(&fake.assistant, &plan.task, Cancel::new(), |_| {})
        .await
        .unwrap();
    assert_eq!(got.as_ref(), Some(&structured));

    // The prompt carried the wrapped, labelled input on stdin — never on argv — and never the
    // full content.
    let stdin = fake.stdin();
    assert!(stdin.contains("based from all 3 Articles"));
    assert!(stdin.contains("<<<FEEDRICO_DATA Articles of the View>>>"));
    assert!(stdin.contains("[3] X — Article 2\nArticle 2 excerpt"));
    assert!(!stdin.contains("full body, never sent"));
    assert!(!fake.argv().contains("Article 2"));

    let briefing = briefing::validate(&library, &plan, &text, got.as_ref()).unwrap();
    assert_eq!(briefing.considered, 3);
    assert_eq!(briefing.topics.len(), 1, "the all-unknown topic is dropped");
    assert_eq!(briefing.topics[0].article_ids, vec![3, 2]);
    assert_eq!(briefing.articles.len(), 2);
    assert_eq!(briefing.articles[0].title, "Article 2");
    assert_eq!(briefing.articles[0].feed_title, "X");

    // The run never marked anything Read or Starred.
    assert_eq!(state_fingerprint(&library), before);
    library.close().unwrap();
}

#[tokio::test]
async fn briefing_end_to_end_with_fake_vibe_reads_json_from_the_text() {
    let (_root, library) = library_with(2);
    let before = state_fingerprint(&library);
    let plan = briefing::plan(&library, &feed_query(Sort::NewestFirst)).unwrap();
    let answer =
        r#"{"topics": [{"topic": "All", "sentence": "Two posts.", "articleIds": [2, 1]}]}"#;
    let fake = fake_vibe(&format!("Here is the briefing:\n{answer}\nDone."));
    let (text, structured) = run(&fake.assistant, &plan.task, Cancel::new(), |_| {})
        .await
        .unwrap();
    assert!(
        structured.is_none(),
        "Vibe has no structured_output channel"
    );
    let briefing = briefing::validate(&library, &plan, &text, structured.as_ref()).unwrap();
    assert_eq!(briefing.topics.len(), 1);
    assert_eq!(briefing.topics[0].article_ids, vec![2, 1]);
    assert_eq!(state_fingerprint(&library), before);
    library.close().unwrap();
}

#[tokio::test]
async fn picks_end_to_end_with_fake_vibe_in_batches() {
    let (_root, library) = library_with(60);
    library.set_read(1, true).unwrap();
    library.set_starred(5, true).unwrap();
    let before = state_fingerprint(&library);

    let plan = picks::plan(&library, &unread_query(), "Rust; not crypto").unwrap();
    assert_eq!(plan.considered, 59);
    assert_eq!(plan.tasks.len(), 3, "59 unread at 25 per batch");

    // Every batch gets the same answer: ids 1 (read, so not in the input but still in the
    // Library), 2 and an unknown one. Duplicates across batches collapse in the merge.
    let answer = r#"{"picks": [{"id": 1, "score": 0.9, "reason": "right on topic"}, {"id": 2, "score": 0.4, "reason": "ok"}, {"id": 999999, "score": 1.0, "reason": "unknown"}]}"#;
    let fake = fake_vibe(answer);

    let mut batches = Vec::new();
    for task in &plan.tasks {
        let (text, structured) = run(&fake.assistant, task, Cancel::new(), |_| {})
            .await
            .unwrap();
        let stdin = fake.stdin();
        assert!(stdin.contains("The user's interests:\nRust; not crypto"));
        assert!(stdin.contains("<<<FEEDRICO_DATA Articles of the View>>>"));
        assert!(stdin.contains("based from all 59 Articles"));
        assert!(!stdin.contains("[1] "), "the read Article is not ranked");
        assert!(!stdin.contains("full body, never sent"));
        assert!(!fake.argv().contains("Article"));
        let batch = picks::validate_batch(&library, &text, structured.as_ref()).unwrap();
        batches.push(batch);
    }
    let picks = picks::merge(&library, &plan, batches).unwrap();
    assert_eq!(
        picks.considered, 59,
        "the result says how many were considered"
    );
    assert!(!picks.capped);
    assert_eq!(
        picks.picks.len(),
        2,
        "unknown id dropped, duplicate collapsed"
    );
    assert_eq!(picks.picks[0].id, 1);
    assert_eq!(picks.picks[0].score, 0.9);
    assert_eq!(picks.picks[0].title, "Article 0");
    assert_eq!(picks.picks[1].id, 2);

    // Picks never change Read or Starred, and the chronological sort is untouched (the query
    // is only read).
    assert_eq!(state_fingerprint(&library), before);
    let rows = library.list_articles(&unread_query(), 0, 5).unwrap();
    assert_eq!(rows[0].title, "Article 59", "still newest first");
    library.close().unwrap();
}

#[tokio::test]
async fn picks_cap_the_newest_100_and_say_so() {
    let (_root, library) = library_with(150);
    let before = state_fingerprint(&library);
    let plan = picks::plan(&library, &unread_query(), "Rust").unwrap();
    assert_eq!(plan.considered, 100);
    assert!(plan.capped);
    assert_eq!(plan.tasks.len(), 4);
    assert!(
        plan.tasks[0]
            .prompt
            .contains("from the newest 100 Articles")
    );
    assert!(plan.tasks[0].prompt.contains("— Article 149\n"));
    assert!(!plan.tasks[0].prompt.contains("— Article 49\n"));

    let fake = fake_vibe(r#"{"picks": []}"#);
    for task in &plan.tasks {
        let (text, structured) = run(&fake.assistant, task, Cancel::new(), |_| {})
            .await
            .unwrap();
        let batch = picks::validate_batch(&library, &text, structured.as_ref()).unwrap();
        assert!(batch.is_empty());
    }
    assert_eq!(state_fingerprint(&library), before);
    library.close().unwrap();
}

#[tokio::test]
async fn a_malformed_answer_is_reported_not_acted_on() {
    let (_root, library) = library_with(2);
    let before = state_fingerprint(&library);
    let plan = briefing::plan(&library, &feed_query(Sort::NewestFirst)).unwrap();
    let fake = fake_vibe("I cannot do that, sorry.");
    let (text, structured) = run(&fake.assistant, &plan.task, Cancel::new(), |_| {})
        .await
        .unwrap();
    let err = briefing::validate(&library, &plan, &text, structured.as_ref()).unwrap_err();
    assert!(matches!(
        err,
        feedrico_core::assistant::AssistantError::Malformed { .. }
    ));
    assert_eq!(state_fingerprint(&library), before);
    library.close().unwrap();
}
