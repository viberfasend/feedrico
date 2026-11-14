mod support;

use feedrico_core::Library;
use feedrico_mcp::{Server, open_library};
use serde_json::{Value, json};
use support::*;

fn call(server: &Server, name: &str, args: Value) -> (bool, String) {
    let req = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":args}});
    let resp: Value = serde_json::from_str(&server.handle_line(&req.to_string()).unwrap()).unwrap();
    let r = &resp["result"];
    (
        r["isError"].as_bool().unwrap_or(false),
        r["content"][0]["text"].as_str().unwrap().to_owned(),
    )
}

fn server(dir: &std::path::Path) -> Server {
    Server::new(open_library(dir).unwrap())
}

#[test]
fn folders_and_feeds_and_smart_views() {
    let root = tempfile::tempdir().unwrap();
    let s = server(&fixture(root.path()));
    let (err, text) = call(&s, "list_folders_and_feeds", json!({}));
    assert!(!err);
    let v: Value = serde_json::from_str(&text).unwrap();
    let titles: Vec<&str> = v["feeds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"Rust Blog") && titles.contains(&"Cooking"));
    assert_eq!(v["feeds"][0]["unread"], 2);

    let (_, text) = call(&s, "list_smart_views", json!({}));
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["allUnread"], 2);
    assert_eq!(v["starred"], 1);
}

#[test]
fn list_articles_respects_view_limit_and_unread_only() {
    let root = tempfile::tempdir().unwrap();
    let s = server(&fixture(root.path()));
    let (_, text) = call(&s, "list_articles", json!({"view":"today","limit":10}));
    let v: Value = serde_json::from_str(&text).unwrap();
    assert!(v["articles"].as_array().is_some());

    let (_, text) = call(
        &s,
        "list_articles",
        json!({"view":"feed","id":2,"limit":10}),
    );
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["articles"].as_array().unwrap().len(), 1);
    assert_eq!(v["articles"][0]["title"], "Sourdough");
    assert_eq!(v["articles"][0]["read"], true);
    let (_, text) = call(
        &s,
        "list_articles",
        json!({"view":"feed","id":2,"unread_only":true}),
    );
    let v: Value = serde_json::from_str(&text).unwrap();
    assert!(v["articles"].as_array().unwrap().is_empty());

    let (_, text) = call(&s, "list_articles", json!({"view":"feed","id":1,"limit":1}));
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["articles"].as_array().unwrap().len(), 1);
    assert_eq!(v["articles"][0]["title"], "Async Rust news");
    assert_eq!(v["articles"][0]["date"], "2023-11-14T22:13:20Z");

    let (err, text) = call(&s, "list_articles", json!({"view":"nonsense"}));
    assert!(err, "{text}");
}

#[test]
fn search_and_list_starred() {
    let root = tempfile::tempdir().unwrap();
    let s = server(&fixture(root.path()));
    let (_, text) = call(&s, "search", json!({"query":"rust"}));
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["articles"].as_array().unwrap().len(), 2);
    let (_, text) = call(&s, "search", json!({"query":"rust","feed":2}));
    let v: Value = serde_json::from_str(&text).unwrap();
    assert!(v["articles"].as_array().unwrap().is_empty());
    let (_, text) = call(&s, "list_starred", json!({}));
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["articles"].as_array().unwrap().len(), 1);
    assert_eq!(v["articles"][0]["title"], "Async Rust news");
}

#[test]
fn get_article_returns_plain_text_and_metadata() {
    let root = tempfile::tempdir().unwrap();
    let s = server(&fixture(root.path()));
    let (_, text) = call(&s, "list_starred", json!({}));
    let v: Value = serde_json::from_str(&text).unwrap();
    let id = v["articles"][0]["id"].as_i64().unwrap();
    let (err, text) = call(&s, "get_article", json!({"id": id}));
    assert!(!err);
    assert!(text.contains("# Async Rust news"));
    assert!(text.contains("Feed: Rust Blog"));
    assert!(text.contains("https://example.org/r1"));
    assert!(text.contains("Borrow checker & async."));
    assert!(!text.contains("<p>") && !text.contains("x()"));

    let (err, _) = call(&s, "get_article", json!({"id": 9999}));
    assert!(err);
}

#[test]
fn a_session_changes_no_byte_and_works_while_locked() {
    let root = tempfile::tempdir().unwrap();
    let dir = fixture(root.path());
    std::fs::write(
        dir.join(".lock"),
        br#"{"machine":"friend-laptop","pid":4242,"openedAt":1762547640}"#,
    )
    .unwrap();
    let before = hash_tree(&dir);
    {
        let s = server(&dir);
        for (n, a) in [
            ("list_folders_and_feeds", json!({})),
            ("list_smart_views", json!({})),
            ("list_articles", json!({"view":"all_unread"})),
            ("search", json!({"query":"rust"})),
            ("list_starred", json!({})),
            ("get_article", json!({"id":1})),
        ] {
            assert!(!call(&s, n, a).0, "{n}");
        }
    }
    assert_eq!(hash_tree(&dir), before);
}

#[test]
fn works_while_the_app_has_the_library_open() {
    let root = tempfile::tempdir().unwrap();
    let dir = fixture(root.path());
    let app = Library::open(&dir, Default::default()).unwrap();
    let s = server(&dir);
    let (err, text) = call(&s, "list_smart_views", json!({}));
    assert!(!err, "{text}");
    drop(s);
    assert!(dir.join(".lock").exists());
    app.close().unwrap();
}

#[test]
fn too_new_and_needs_upgrade_are_refused() {
    let root = tempfile::tempdir().unwrap();
    let dir = fixture(root.path());
    let manifest = std::fs::read_to_string(dir.join("library.json")).unwrap();
    let v: Value = serde_json::from_str(&manifest).unwrap();
    let current = v["formatVersion"].as_u64().unwrap();

    std::fs::write(
        dir.join("library.json"),
        manifest
            .replace(
                &format!("\"formatVersion\": {current}"),
                "\"formatVersion\": 999",
            )
            .replace(
                &format!("\"formatVersion\":{current}"),
                "\"formatVersion\":999",
            ),
    )
    .unwrap();
    let err = open_library(&dir).err().unwrap();
    assert!(err.contains("needs a newer Feedrico"), "{err}");

    // Older: the database says an earlier version.
    std::fs::write(dir.join("library.json"), &manifest).unwrap();
    rusqlite::Connection::open(dir.join("library.db"))
        .unwrap()
        .pragma_update(None, "user_version", current as u32 - 1)
        .unwrap();
    let err = open_library(&dir).err().unwrap();
    assert!(err.contains("open it in Feedrico first"), "{err}");
}
