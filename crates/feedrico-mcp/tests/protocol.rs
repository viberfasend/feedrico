mod support;

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

fn run(args: &[&str], input: &str) -> (std::process::ExitStatus, Vec<Value>, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_feedrico-mcp"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let lines = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    (out.status, lines, String::from_utf8(out.stderr).unwrap())
}

#[test]
fn speaks_json_rpc_over_stdio() {
    let root = tempfile::tempdir().unwrap();
    let dir = support::fixture(root.path());
    let before = support::hash_tree(&dir);
    let input = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_starred","arguments":{}}}),
        json!({"jsonrpc":"2.0","id":4,"method":"nope"}),
        json!({"jsonrpc":"2.0","id":5,"method":"ping"}),
    ]
    .iter()
    .map(|v| v.to_string() + "\n")
    .collect::<String>()
        + "not json\n";
    let (status, out, _) = run(&["--library", dir.to_str().unwrap()], &input);
    assert!(status.success());
    // No reply to the notification: 5 requests + 1 parse error.
    assert_eq!(out.len(), 6);
    assert_eq!(out[0]["id"], 1);
    assert_eq!(out[0]["result"]["serverInfo"]["name"], "feedrico");
    assert!(out[0]["result"]["capabilities"]["tools"].is_object());
    assert_eq!(out[0]["result"]["protocolVersion"], "2025-06-18");
    let names: Vec<&str> = out[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "list_folders_and_feeds",
            "list_smart_views",
            "list_articles",
            "search",
            "get_article",
            "list_starred"
        ]
    );
    assert!(out[1]["result"]["tools"][0]["inputSchema"].is_object());
    assert!(
        out[2]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Async Rust news")
    );
    assert_eq!(out[3]["error"]["code"], -32601);
    assert_eq!(out[4]["result"], json!({}));
    assert_eq!(out[5]["error"]["code"], -32700);
    assert_eq!(support::hash_tree(&dir), before);
}

#[test]
fn refuses_to_start_on_a_non_library_and_without_a_path() {
    let root = tempfile::tempdir().unwrap();
    let (status, out, err) = run(&["--library", root.path().to_str().unwrap()], "");
    assert!(!status.success());
    assert!(out.is_empty());
    assert!(err.contains("not a Feedrico Library"), "{err}");
    let (status, _, err) = run(&[], "");
    assert!(!status.success());
    assert!(err.contains("--library"), "{err}");
}
