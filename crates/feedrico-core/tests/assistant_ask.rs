//! Ask about the Article (#119) through the runner: a fake CLI records stdin, so the test can
//! check that each question resends the Article plus the conversation so far, inside the caps,
//! with the oldest turns dropped first. No CLI session is kept and no files are left behind.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use feedrico_core::articles::ArticleDetail;
use feedrico_core::assistant::article::ArticleInput;
use feedrico_core::assistant::ask::{MAX_TURNS, Turn, ask_task};
use feedrico_core::assistant::run::{Cancel, run};
use feedrico_core::assistant::{AssistantKind, FoundAssistant, SignIn};

fn article(html: &str) -> ArticleDetail {
    ArticleDetail {
        id: 1,
        feed_id: 1,
        feed_title: "Fixture Feed".into(),
        feed_site_url: None,
        title: "City buses".into(),
        url: None,
        author: None,
        content_html: html.into(),
        article_date: 0,
        is_read: false,
        is_starred: false,
        favicon: None,
        enclosures: vec![],
        video: None,
    }
}

fn fake(kind: AssistantKind, fixture: &str) -> (tempfile::TempDir, PathBuf, FoundAssistant) {
    let dir = tempfile::tempdir().unwrap();
    let rec = dir.path().join("rec");
    std::fs::create_dir(&rec).unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assistant");
    let script = format!(
        "#!/bin/sh\nREC='{rec}'\npwd > \"$REC/cwd\"\nprintf '%s\\n' \"$@\" > \"$REC/argv\"\ncat > \"$REC/stdin\"\ncat '{f}'\n",
        rec = rec.display(),
        f = fixtures.join(fixture).display()
    );
    let path = dir.path().join(kind.binary_name());
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let found = FoundAssistant {
        kind,
        path,
        version: None,
        sign_in: SignIn::Plan,
    };
    (dir, rec, found)
}

#[tokio::test]
async fn each_question_resends_the_article_and_the_capped_conversation() {
    let detail = article("<p>SECRET-ASK-BODY The city builds electric buses until 2030.</p>");
    let input = ArticleInput::from_article(&detail);
    let history: Vec<Turn> = (0..10)
        .map(|i| Turn {
            question: format!("Q{i}"),
            answer: format!("A{i}"),
        })
        .collect();
    let (task, dropped) = ask_task(&input, "What about buses? QNEW", &history);
    assert_eq!(dropped, 10 - MAX_TURNS);

    let (_dir, rec, found) = fake(AssistantKind::ClaudeCode, "claude-text.jsonl");
    run(&found, &task, Cancel::new(), |_| {}).await.unwrap();

    let stdin = std::fs::read_to_string(rec.join("stdin")).unwrap();
    // The Article is resent, wrapped as data.
    assert!(stdin.contains("<<<FEEDRICO_DATA Article>>>"));
    assert!(stdin.contains("SECRET-ASK-BODY"));
    // The conversation so far is resent, wrapped as data, newest turns only.
    assert!(stdin.contains("<<<FEEDRICO_DATA Conversation so far>>>"));
    assert!(stdin.contains("Q9"), "{stdin}");
    assert!(stdin.contains("Q4"), "{stdin}");
    assert!(
        !stdin.contains("Q3"),
        "the oldest turns drop first: {stdin}"
    );
    assert!(stdin.contains("A9") && !stdin.contains("A0\n"));
    // The new question is the last data block.
    assert!(stdin.contains("<<<FEEDRICO_DATA Question>>>"));
    assert!(stdin.contains("What about buses? QNEW"));
    // The Article and the conversation stay off the command line.
    let argv = std::fs::read_to_string(rec.join("argv")).unwrap();
    assert!(!argv.contains("SECRET-ASK-BODY") && !argv.contains("Q9"));
    // No CLI session is kept.
    assert!(argv.contains("--no-session-persistence"));
    // The temporary working directory is gone afterwards.
    let cwd = std::fs::read_to_string(rec.join("cwd")).unwrap();
    assert!(!Path::new(cwd.trim()).exists());
}

#[tokio::test]
async fn the_first_question_has_no_conversation_block() {
    let input = ArticleInput::from_article(&article("<p>SECRET-FIRST Only body.</p>"));
    let (task, dropped) = ask_task(&input, "First question?", &[]);
    assert_eq!(dropped, 0);
    let (_dir, rec, found) = fake(AssistantKind::MistralVibe, "vibe-text.jsonl");
    run(&found, &task, Cancel::new(), |_| {}).await.unwrap();
    let stdin = std::fs::read_to_string(rec.join("stdin")).unwrap();
    assert!(stdin.contains("SECRET-FIRST"));
    assert!(!stdin.contains("Conversation so far"), "{stdin}");
    assert!(stdin.contains("<<<FEEDRICO_DATA Question>>>\nFirst question?"));
}
