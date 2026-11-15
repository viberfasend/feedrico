//! Translate (#117) through the runner: a fake CLI records what it was sent and answers with a
//! hostile "translation". The prompt must carry image placeholders, never image data, and the
//! answer must come back sanitized, with links and the cached image src intact.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use feedrico_core::articles::ArticleDetail;
use feedrico_core::assistant::run::{Cancel, run};
use feedrico_core::assistant::translate::{finish_translation, prepare, translate_task};
use feedrico_core::assistant::{AssistantKind, FoundAssistant, SignIn};

fn article(html: &str) -> ArticleDetail {
    ArticleDetail {
        id: 1,
        feed_id: 1,
        feed_title: "Fixture Feed".into(),
        feed_site_url: None,
        title: "Un Article".into(),
        url: Some("https://example.com/post".into()),
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

/// A fake CLI that records argv and stdin, then replays a fixture.
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
async fn translate_sends_placeholders_not_image_data_and_returns_sanitized_html() {
    let html = r#"<p>Bonjour <a href="https://example.com/orig">le monde</a></p><img src="media://localhost/abc123" alt="Une image" data-remote="https://example.com/i.png"><p>Encore du texte.</p>"#;
    let detail = article(html);
    let prep = prepare(&detail);
    assert_eq!(prep.images.len(), 1);
    assert!(prep.html.contains("[[IMAGE_1]]"), "{}", prep.html);
    assert!(!prep.html.contains("<img"), "{}", prep.html);

    let (_dir, rec, found) = fake(AssistantKind::ClaudeCode, "claude-translation.jsonl");
    let mut task = translate_task(&prep, "French");
    task.model = Some("haiku".into());
    let (text, _) = run(&found, &task, Cancel::new(), |_| {}).await.unwrap();

    // The Assistant got the structure with a placeholder, never the image tag or its src.
    let stdin = std::fs::read_to_string(rec.join("stdin")).unwrap();
    assert!(stdin.contains("<<<FEEDRICO_DATA Article>>>"));
    assert!(stdin.contains("Bonjour"));
    assert!(stdin.contains("[[IMAGE_1]]"));
    assert!(!stdin.contains("<img"), "{stdin}");
    assert!(!stdin.contains("media://localhost/abc123"), "{stdin}");
    assert!(!stdin.contains("data-remote"), "{stdin}");
    // The target language is an instruction, on the command line only for Claude, never in stdin
    // as Article data.
    let argv = std::fs::read_to_string(rec.join("argv")).unwrap();
    assert!(argv.contains("French"), "{argv}");
    assert!(!stdin.contains("French"), "{stdin}");

    // The hostile answer comes back clean, links and the cached image survive.
    let out = finish_translation(
        &text,
        &prep.images,
        detail.url.as_deref(),
        "media://localhost/",
    );
    assert!(!out.contains("<script"), "{out}");
    assert!(!out.contains("onerror"), "{out}");
    assert!(!out.contains("javascript:"), "{out}");
    assert!(out.contains(r#"href="https://example.com/ok""#), "{out}");
    assert!(out.contains(r#"src="media://localhost/abc123""#), "{out}");
    assert!(out.contains(r#"alt="Une image""#), "{out}");
    assert!(out.contains("Bonjour le monde"), "{out}");
    assert!(out.contains("<h3>Un titre</h3>"), "{out}");
}

#[tokio::test]
async fn translate_runs_on_vibe_too_and_leaves_no_temp_files() {
    let detail = article("<p>Hola mundo</p><img src=\"media://localhost/zzz\" alt=\"x\">");
    let prep = prepare(&detail);
    let (_dir, rec, found) = fake(AssistantKind::MistralVibe, "vibe-text.jsonl");
    let task = translate_task(&prep, "Spanish");
    let (text, _) = run(&found, &task, Cancel::new(), |_| {}).await.unwrap();
    assert_eq!(text, "Feedrico can talk to you.");
    let stdin = std::fs::read_to_string(rec.join("stdin")).unwrap();
    assert!(stdin.contains("[[IMAGE_1]]"));
    assert!(!stdin.contains("<img"));
    let cwd = std::fs::read_to_string(rec.join("cwd")).unwrap();
    assert!(!Path::new(cwd.trim()).exists(), "the temp dir is removed");
}
