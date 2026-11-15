//! Saving Articles to Ferrico, the local-first bookmark manager, through its loopback API:
//! `POST http://127.0.0.1:59432/bookmarks` with a Bearer token read from Ferrico's own
//! `settings.json`. Ferrico binds the port to 127.0.0.1 only (see its SECURITY.md), so this
//! never leaves the machine — the Offline Reader (Paul) is unaffected — and it runs only when
//! the user explicitly saves an Article (spec: network policy). The Archivist (Henrik) gets
//! the `b` shortcut in the UI.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Ferrico's extension bridge address. Loopback only, never configurable to anything else.
pub const API_BASE: &str = "http://127.0.0.1:59432";

/// What the UI sends for the open Article. Field names cross IPC as camelCase; the wire body
/// below uses Ferrico's snake_case field names.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBookmarkInput {
    pub url: String,
    pub title: String,
    pub description: Option<String>,
    pub feed_url: Option<String>,
}

/// Ferrico's `POST /bookmarks` answer. On the wire the field is `created_at`; across IPC to the
/// UI it crosses as `createdAt`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedBookmark {
    pub id: String,
    #[serde(rename(serialize = "createdAt", deserialize = "created_at"))]
    pub created_at: i64,
}

/// Why a save failed, with messages written for the toast the UI shows.
#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("only Articles with a web link can be saved to Ferrico")]
    InvalidUrl,
    #[error(
        "Ferrico doesn't seem to be installed here (no settings file with an API token was found)"
    )]
    NotInstalled,
    #[error("Ferrico isn't running — start the Ferrico app and try again")]
    NotRunning,
    #[error("Ferrico rejected the API token — check the token in Ferrico's settings")]
    Unauthorized,
    #[error("Ferrico answered with an error: {0}")]
    Rejected(String),
}

/// Where Ferrico keeps its database and `settings.json` on desktop (its `resolve_data_dir`:
/// `dirs::data_dir()/ferrico`).
pub fn data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("ferrico"))
}

/// Reads the API token from a Ferrico `settings.json`.
pub fn read_token(settings_path: &Path) -> Result<String, SaveError> {
    let raw = std::fs::read_to_string(settings_path).map_err(|_| SaveError::NotInstalled)?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|_| SaveError::NotInstalled)?;
    value
        .get("api_token")
        .and_then(|t| t.as_str())
        .map(str::to_owned)
        .ok_or(SaveError::NotInstalled)
}

/// The token from Ferrico's settings file in the platform data dir.
pub fn load_api_token() -> Result<String, SaveError> {
    let dir = data_dir().ok_or(SaveError::NotInstalled)?;
    read_token(&dir.join("settings.json"))
}

/// POSTs one bookmark to Ferrico's loopback API. `base` is a parameter so tests can point at a
/// mock server; the app always goes through [`API_BASE`].
pub async fn save_bookmark(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    input: &SaveBookmarkInput,
) -> Result<SavedBookmark, SaveError> {
    if !(input.url.starts_with("https://") || input.url.starts_with("http://")) {
        return Err(SaveError::InvalidUrl);
    }
    let body = serde_json::json!({
        "url": input.url,
        "title": input.title,
        "description": input.description,
        "feed_url": input.feed_url,
    });
    let response = client
        .post(format!("{base}/bookmarks"))
        .bearer_auth(token)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                SaveError::NotRunning
            } else {
                SaveError::Rejected(e.to_string())
            }
        })?;
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(SaveError::Unauthorized);
    }
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(SaveError::Rejected(format!(
            "HTTP {status}: {}",
            text.trim()
        )));
    }
    let text = response
        .text()
        .await
        .map_err(|e| SaveError::Rejected(e.to_string()))?;
    serde_json::from_str(&text).map_err(|e| SaveError::Rejected(e.to_string()))
}

/// The app entry point: load the token from Ferrico's settings, then POST to the loopback API.
pub async fn save(
    client: &reqwest::Client,
    input: &SaveBookmarkInput,
) -> Result<SavedBookmark, SaveError> {
    let token = load_api_token()?;
    save_bookmark(client, API_BASE, &token, input).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn input() -> SaveBookmarkInput {
        SaveBookmarkInput {
            url: "https://example.com/post".to_string(),
            title: "A post".to_string(),
            description: Some("excerpt".to_string()),
            feed_url: Some("https://example.com".to_string()),
        }
    }

    /// True once the buffer holds full headers plus the Content-Length worth of body.
    fn request_complete(buf: &[u8]) -> bool {
        let Some(head_end) = buf.windows(4).position(|w| w == b"\r\n\r\n") else {
            return false;
        };
        let head = String::from_utf8_lossy(&buf[..head_end]).to_lowercase();
        let length = head
            .lines()
            .find_map(|l| l.strip_prefix("content-length: "))
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(0);
        buf.len() >= head_end + 4 + length
    }

    /// A one-shot mock of Ferrico's bridge: answers one request with `response`, returns the raw
    /// request it received.
    async fn mock_server(response: &'static str) -> (String, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = socket.read(&mut chunk).await.unwrap();
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
                if request_complete(&buf) {
                    break;
                }
            }
            socket.write_all(response.as_bytes()).await.unwrap();
            socket.shutdown().await.ok();
            String::from_utf8_lossy(&buf).into_owned()
        });
        (format!("http://{addr}"), handle)
    }

    fn ok_response() -> &'static str {
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 36\r\nconnection: close\r\n\r\n{\"id\":\"abc\",\"created_at\":1700000000}"
    }

    #[tokio::test]
    async fn posts_snake_case_body_with_bearer_token_and_parses_the_answer() {
        let (base, server) = mock_server(ok_response()).await;
        let saved = save_bookmark(&reqwest::Client::new(), &base, "sekrit", &input())
            .await
            .unwrap();
        assert_eq!(
            saved,
            SavedBookmark {
                id: "abc".to_string(),
                created_at: 1_700_000_000
            }
        );
        let request = server.await.unwrap();
        let head = request.split("\r\n\r\n").next().unwrap().to_lowercase();
        assert!(request.starts_with("POST /bookmarks HTTP/1.1"));
        assert!(head.contains("authorization: bearer sekrit"));
        let body = request.split("\r\n\r\n").nth(1).unwrap();
        let body: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(
            body,
            serde_json::json!({
                "url": "https://example.com/post",
                "title": "A post",
                "description": "excerpt",
                "feed_url": "https://example.com",
            })
        );
    }

    #[tokio::test]
    async fn unauthorized_maps_to_the_token_error() {
        let (base, server) = mock_server(
            "HTTP/1.1 401 Unauthorized\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
        )
        .await;
        let err = save_bookmark(&reqwest::Client::new(), &base, "wrong", &input())
            .await
            .unwrap_err();
        assert!(matches!(err, SaveError::Unauthorized));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn other_statuses_map_to_rejected_with_the_body() {
        let (base, server) = mock_server(
            "HTTP/1.1 500 Internal Server Error\r\ncontent-length: 7\r\nconnection: close\r\n\r\nboom!!!",
        )
        .await;
        let err = save_bookmark(&reqwest::Client::new(), &base, "tok", &input())
            .await
            .unwrap_err();
        match err {
            SaveError::Rejected(m) => {
                assert!(m.contains("500"), "message should name the status: {m}");
                assert!(m.contains("boom!!!"), "message should carry the body: {m}");
            }
            other => panic!("expected Rejected, got {other:?}"),
        }
        server.await.unwrap();
    }

    #[tokio::test]
    async fn connection_refused_maps_to_not_running() {
        // Bind and immediately drop, so the port is closed on loopback.
        let addr = TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap()
            .local_addr()
            .unwrap();
        let err = save_bookmark(
            &reqwest::Client::new(),
            &format!("http://{addr}"),
            "tok",
            &input(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, SaveError::NotRunning));
    }

    #[tokio::test]
    async fn non_web_urls_are_rejected_before_any_request() {
        let mut bad = input();
        bad.url = "ftp://example.com/x".to_string();
        let err = save_bookmark(&reqwest::Client::new(), "http://127.0.0.1:1", "tok", &bad)
            .await
            .unwrap_err();
        assert!(matches!(err, SaveError::InvalidUrl));
    }

    #[test]
    fn read_token_finds_the_api_token_and_reports_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let settings = dir.path().join("settings.json");
        assert!(matches!(
            read_token(&settings).unwrap_err(),
            SaveError::NotInstalled
        ));
        std::fs::write(&settings, r#"{ "api_token": "abc123" }"#).unwrap();
        assert_eq!(read_token(&settings).unwrap(), "abc123");
        std::fs::write(&settings, r#"{ "backup": {} }"#).unwrap();
        assert!(matches!(
            read_token(&settings).unwrap_err(),
            SaveError::NotInstalled
        ));
    }
}
