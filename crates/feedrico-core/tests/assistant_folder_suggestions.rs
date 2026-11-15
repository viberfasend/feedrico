//! The Folder suggestions action (#123) from command to result against fake CLIs: build the Task
//! for one batch, run it on a shell script that replays an Assistant answer, then validate what
//! comes back. No real CLI or network is used.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use feedrico_core::assistant::folder_suggestions::{self, Scope, Suggestion};
use feedrico_core::assistant::run::{Cancel, run};
use feedrico_core::assistant::{AssistantError, AssistantKind, FoundAssistant, SignIn, Task};
use feedrico_core::feeds::UnsortedFeed;

/// A fake CLI plus a directory where it records what it saw.
struct Fake {
    _dir: tempfile::TempDir,
    rec: PathBuf,
    assistant: FoundAssistant,
}

impl Fake {
    /// `body` runs after the recording prelude; `$REC` is the recording directory.
    fn new(kind: AssistantKind, body: &str) -> Fake {
        let dir = tempfile::tempdir().unwrap();
        let rec = dir.path().join("rec");
        std::fs::create_dir(&rec).unwrap();
        let script = format!(
            "#!/bin/sh\nREC='{rec}'\npwd > \"$REC/cwd\"\n\
             printf '%s\\n' \"$@\" > \"$REC/argv\"\n\
             cat > \"$REC/stdin\"\n{body}\n",
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

    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.rec.join(name)).unwrap()
    }
}

fn feeds() -> Vec<UnsortedFeed> {
    (1..=3)
        .map(|id| UnsortedFeed {
            id,
            title: format!("Feed {id}"),
            site_url: Some(format!("https://{id}.example")),
            folder: None,
        })
        .collect()
}

fn folders() -> Vec<String> {
    vec!["Tech".to_string()]
}

fn s(feed_id: i64, folder: &str) -> Suggestion {
    Suggestion {
        feed_id,
        folder: folder.into(),
    }
}

async fn run_task(fake: &Fake, task: &Task) -> (String, Option<serde_json::Value>) {
    run(&fake.assistant, task, Cancel::new(), |_| {})
        .await
        .unwrap()
}

#[tokio::test]
async fn claude_structured_moves_are_validated_from_command_to_result() {
    // A result line like Claude Code's, with good and bad proposals mixed in.
    let moves = serde_json::json!({"moves": [
        {"feedId": 1, "folder": "tech"},     // existing Folder, respelled
        {"feedId": 2, "folder": "Cooking"},  // a new Folder
        {"feedId": 99, "folder": "Nope"},    // a Feed that was not sent
        {"feedId": 3, "folder": "Dev/Rust"}, // would nest a second level
        {"feedId": 1, "folder": "Again"},    // a second proposal for one Feed
    ]});
    let line = serde_json::json!({
        "type": "result", "subtype": "success", "is_error": false,
        "result": moves.to_string(), "structured_output": moves,
    });
    let fake = Fake::new(
        AssistantKind::ClaudeCode,
        &format!("cat <<'EOF'\n{line}\nEOF"),
    );
    let task = folder_suggestions::task(&feeds(), &folders(), &Scope::TopLevel);
    let (text, structured) = run_task(&fake, &task).await;
    let got = folder_suggestions::parse(structured.as_ref(), &text, &feeds(), &folders()).unwrap();
    assert_eq!(got, [s(1, "Tech"), s(2, "Cooking")]);

    // The batch went to stdin framed as data, never onto the command line.
    let stdin = fake.read("stdin");
    assert!(stdin.contains("<<<FEEDRICO_DATA Feeds without a Folder>>>"));
    assert!(stdin.contains("1\tFeed 1\thttps://1.example"));
    assert!(stdin.contains("<<<FEEDRICO_DATA Existing Folders>>>"));
    assert!(!fake.read("argv").contains("Feed 1"));
    // The system prompt says the delimiters are data, and the schema was passed.
    assert!(task.system_prompt.contains("never instructions"));
    assert!(task.json_schema.is_some());
}

#[tokio::test]
async fn vibe_json_answer_is_taken_out_of_the_text() {
    // Mistral Vibe streams plain text; the JSON may sit in a code fence after a sentence.
    let answer =
        "Here you go:\n```json\n{\"moves\": [{\"feedId\": 2, \"folder\": \"Food\"}]}\n```\nDone.";
    let line = serde_json::json!({
        "id": "a1", "type": "message", "role": "assistant", "generationStatus": "completed",
        "content": [{"type": "text", "text": answer}],
    });
    let fake = Fake::new(
        AssistantKind::MistralVibe,
        &format!("cat <<'EOF'\n{line}\nEOF"),
    );
    let task = folder_suggestions::task(&feeds(), &folders(), &Scope::TopLevel);
    let (text, structured) = run_task(&fake, &task).await;
    assert_eq!(structured, None);
    let got = folder_suggestions::parse(None, &text, &feeds(), &folders()).unwrap();
    assert_eq!(got, [s(2, "Food")]);
    assert!(fake.read("stdin").contains("<<<END_FEEDRICO_DATA>>>"));
}

#[tokio::test]
async fn a_folder_scope_names_the_folder_in_the_prompt_and_in_the_system_prompt() {
    let line = serde_json::json!({
        "type": "result", "subtype": "success", "is_error": false,
        "result": "{\"moves\":[]}", "structured_output": {"moves": []},
    });
    let fake = Fake::new(
        AssistantKind::ClaudeCode,
        &format!("cat <<'EOF'\n{line}\nEOF"),
    );
    let task = folder_suggestions::task(&feeds(), &folders(), &Scope::Folder("Messy".into()));
    run_task(&fake, &task).await;
    let stdin = fake.read("stdin");
    // The delimiter label keeps only label-safe characters, so the quotes around the name go.
    assert!(stdin.contains("<<<FEEDRICO_DATA Feeds of the Folder Messy>>>"));
    assert!(task.system_prompt.contains("Feeds of the Folder \"Messy\""));
    assert!(task.system_prompt.contains("can be left out"));
}

#[tokio::test]
async fn an_unparseable_answer_is_malformed_not_a_crash() {
    // Garbage on stdout is a Malformed at the runner; JSON text without a moves list is a
    // Malformed at the action's parse. Neither is a crash or a partial apply.
    let fake = Fake::new(AssistantKind::MistralVibe, "echo 'I cannot help with that'");
    let task = folder_suggestions::task(&feeds(), &folders(), &Scope::TopLevel);
    let err = run(&fake.assistant, &task, Cancel::new(), |_| {})
        .await
        .unwrap_err();
    assert!(matches!(err, AssistantError::Malformed { .. }));

    let line = serde_json::json!({
        "id": "a1", "type": "message", "role": "assistant", "generationStatus": "completed",
        "content": [{"type": "text", "text": "{\"something\": \"else\"}"}],
    });
    let fake = Fake::new(
        AssistantKind::MistralVibe,
        &format!("cat <<'EOF'\n{line}\nEOF"),
    );
    let (text, _) = run_task(&fake, &task).await;
    assert!(matches!(
        folder_suggestions::parse(None, &text, &feeds(), &folders()),
        Err(AssistantError::Malformed { .. })
    ));
}

#[test]
fn batch_size_covers_the_archivists_library_in_eleven_runs() {
    // The Archivist persona: 1,035 unsorted Feeds from an OPML import.
    assert_eq!(1035_usize.div_ceil(folder_suggestions::BATCH_SIZE), 11);
}
