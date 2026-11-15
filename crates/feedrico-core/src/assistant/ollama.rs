//! Ollama: models running on this computer, over its local HTTP API (#125).
//!
//! Unlike the CLI Assistants this is not a process Feedrico starts. The server is already
//! running (or not), and Feedrico talks to it with `GET /api/tags` (detection) and a streaming
//! `POST /api/chat` (a run). The `path` of the setup is the server's base URL.
//!
//! The one hard rule: **only loopback addresses**. Article text must never leave the computer,
//! so [`loopback_base`] rejects every other host, userinfo and scheme, and both detection and
//! runs go through it. Proxies are off and redirects are not followed, so a loopback server
//! cannot bounce a request somewhere else either.

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::time::Instant;
use url::{Host, Url};

use super::run::Cancel;
use super::{AssistantError, Event, Task};

/// Where Ollama listens unless the user says otherwise.
pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:11434";

/// How long the detection probe may take, per request.
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const CANCEL_POLL: Duration = Duration::from_millis(25);
const MESSAGE_CAP: usize = 500;

/// Accept `http://<loopback>[:port]` and nothing else. `localhost` becomes `127.0.0.1`, so no
/// DNS lookup can ever send us elsewhere.
pub fn loopback_base(input: &str) -> Option<Url> {
    let mut url = Url::parse(input.trim()).ok()?;
    if url.scheme() != "http" || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    let ok = match url.host()? {
        Host::Ipv4(ip) => ip.is_loopback(),
        Host::Ipv6(ip) => ip.is_loopback(),
        Host::Domain(d) => d.eq_ignore_ascii_case("localhost"),
    };
    if !ok {
        return None;
    }
    if matches!(url.host()?, Host::Domain(_)) {
        url.set_host(Some("127.0.0.1")).ok()?;
    }
    // Only the origin matters: the API paths are ours.
    url.set_path("");
    url.set_query(None);
    url.set_fragment(None);
    Some(url)
}

/// The loopback base URL as stored in the setup (no trailing slash), or `None` if it isn't one.
pub fn normalized(input: &str) -> Option<String> {
    loopback_base(input).map(|u| u.as_str().trim_end_matches('/').to_string())
}

fn endpoint(base: &Url, path: &str) -> String {
    format!("{}{path}", base.as_str().trim_end_matches('/'))
}

/// What detection learned from a running server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    pub version: Option<String>,
    pub models: Vec<String>,
}

/// Ask the server at `base_url` which models are installed (`GET /api/tags`), and its version.
/// `None` when the URL isn't loopback, nothing answers within a short timeout, or the answer
/// isn't Ollama's.
pub fn probe(base_url: &str) -> Option<Probe> {
    let base = loopback_base(base_url)?;
    let tags = get_json(&base, "/api/tags")?;
    let mut models: Vec<String> = tags["models"]
        .as_array()?
        .iter()
        .filter_map(|m| m["name"].as_str().or_else(|| m["model"].as_str()))
        .map(str::to_string)
        .collect();
    models.sort();
    let version =
        get_json(&base, "/api/version").and_then(|v| v["version"].as_str().map(str::to_string));
    Some(Probe { version, models })
}

/// A tiny HTTP/1.0 GET over a loopback socket. HTTP/1.0 keeps the answer free of chunking, and
/// it avoids an HTTP client (and its proxy settings) for a two-line request.
fn get_json(base: &Url, path: &str) -> Option<Value> {
    let port = base.port_or_known_default()?;
    let ip: IpAddr = match base.host()? {
        Host::Ipv4(ip) => ip.into(),
        Host::Ipv6(ip) => ip.into(),
        Host::Domain(_) => Ipv4Addr::LOCALHOST.into(),
    };
    let mut stream = TcpStream::connect_timeout(&SocketAddr::new(ip, port), PROBE_TIMEOUT).ok()?;
    stream.set_read_timeout(Some(PROBE_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(PROBE_TIMEOUT)).ok()?;
    write!(
        stream,
        "GET {path} HTTP/1.0\r\nHost: {ip}:{port}\r\nAccept: application/json\r\n\r\n"
    )
    .ok()?;
    let mut raw = Vec::new();
    // Cap what we accept: a model list is small.
    stream.take(4 * 1024 * 1024).read_to_end(&mut raw).ok()?;
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = String::from_utf8_lossy(&raw[..split]);
    let status: u16 = head
        .lines()
        .next()?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    if status != 200 {
        return None;
    }
    serde_json::from_slice(&raw[split + 4..]).ok()
}

/// Run `task` on the local model named by `task.model`, streaming `Text` events.
pub async fn run(
    base_url: &str,
    task: &Task,
    cancel: Cancel,
    on_event: &mut (dyn FnMut(Event) + Send),
) -> Result<(String, Option<Value>), AssistantError> {
    let base = loopback_base(base_url).ok_or_else(|| AssistantError::Failed {
        message: "Ollama must be on this computer (an address like http://127.0.0.1:11434)".into(),
    })?;
    let model = task
        .model
        .as_deref()
        .filter(|m| !m.trim().is_empty())
        .ok_or_else(|| AssistantError::Failed {
            message: "choose an Ollama model in Settings first".into(),
        })?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| AssistantError::Failed {
            message: e.to_string(),
        })?;
    let mut body = json!({
        "model": model,
        "stream": true,
        "messages": [
            {"role": "system", "content": task.system_prompt},
            {"role": "user", "content": task.prompt},
        ],
    });
    if let Some(schema) = &task.json_schema {
        body["format"] = schema.clone();
    }
    let deadline = Instant::now() + task.timeout;

    let send = client
        .post(endpoint(&base, "/api/chat"))
        .header("content-type", "application/json")
        .body(body.to_string())
        .send();
    let mut response = interruptible(send, &cancel, deadline).await?.map_err(|e| {
        // Only a refused connection means "not running"; a connection that drops
        // mid-request must not be reported as a missing tool.
        if e.is_connect() {
            AssistantError::NotInstalled
        } else {
            AssistantError::Failed {
                message: format!("the Ollama server dropped the request: {e}"),
            }
        }
    })?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let text = interruptible(response.text(), &cancel, deadline)
            .await?
            .unwrap_or_default();
        return Err(error_from_body(&text, Some(status), model));
    }

    let mut buf: Vec<u8> = Vec::new();
    let mut text = String::new();
    let mut done = false;
    while !done {
        let Some(chunk) = interruptible(response.chunk(), &cancel, deadline)
            .await?
            .map_err(|_| AssistantError::Offline)?
        else {
            break;
        };
        buf.extend_from_slice(&chunk);
        while let Some(end) = buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = buf.drain(..=end).collect();
            let line = String::from_utf8_lossy(&line);
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(line).map_err(|e| AssistantError::Malformed {
                message: format!("a line of output wasn't JSON: {e}"),
            })?;
            if let Some(err) = v["error"].as_str() {
                return Err(error_from_body(err, None, model));
            }
            if let Some(delta) = v["message"]["content"].as_str()
                && !delta.is_empty()
            {
                text.push_str(delta);
                // With a `format` schema the chunks are JSON, not text for the reader: buffer
                // them and send only Done.
                if task.json_schema.is_none() {
                    on_event(Event::Text {
                        delta: delta.to_string(),
                    });
                }
            }
            if v["done"] == true {
                done = true;
                break;
            }
        }
    }
    if !done {
        return Err(AssistantError::Malformed {
            message: "Ollama stopped before the answer was complete".into(),
        });
    }
    if text.trim().is_empty() {
        return Err(AssistantError::Malformed {
            message: "the Assistant ended without an answer".into(),
        });
    }
    let structured = match &task.json_schema {
        Some(_) => Some(serde_json::from_str::<Value>(text.trim()).map_err(|e| {
            AssistantError::Malformed {
                message: format!("the answer wasn't the JSON that was asked for: {e}"),
            }
        })?),
        None => None,
    };
    on_event(Event::Done {
        text: text.clone(),
        structured: structured.clone(),
    });
    Ok((text, structured))
}

/// Await `fut`, but give up on cancel or at the deadline.
async fn interruptible<T>(
    fut: impl std::future::Future<Output = T>,
    cancel: &Cancel,
    deadline: Instant,
) -> Result<T, AssistantError> {
    tokio::pin!(fut);
    let mut poll = tokio::time::interval(CANCEL_POLL);
    loop {
        tokio::select! {
            out = &mut fut => return Ok(out),
            _ = poll.tick() => {
                if cancel.is_cancelled() {
                    return Err(AssistantError::Cancelled);
                }
            }
            _ = tokio::time::sleep_until(deadline) => return Err(AssistantError::TimedOut),
        }
    }
}

/// Ollama answers errors as `{"error":"..."}`, with 404 for a model that isn't installed.
fn error_from_body(body: &str, status: Option<u16>, model: &str) -> AssistantError {
    let message = serde_json::from_str::<Value>(body.trim())
        .ok()
        .and_then(|v| v["error"].as_str().map(str::to_string))
        .unwrap_or_else(|| body.trim().to_string());
    let lower = message.to_lowercase();
    if status == Some(404) || lower.contains("not found") {
        return AssistantError::Failed {
            message: format!("the model \"{model}\" isn't installed (run `ollama pull {model}`)"),
        };
    }
    AssistantError::Failed {
        message: if message.is_empty() {
            format!("Ollama answered with an error ({})", status.unwrap_or(0))
        } else {
            message.chars().take(MESSAGE_CAP).collect()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_loopback_hosts_are_allowed() {
        for ok in [
            "http://127.0.0.1:11434",
            "http://127.0.0.1",
            "http://127.1.2.3:9",
            "http://[::1]:11434",
            "http://localhost:11434",
            "http://LOCALHOST:11434/",
        ] {
            assert!(loopback_base(ok).is_some(), "{ok}");
        }
        for bad in [
            "http://example.com:11434",
            "http://192.168.1.10:11434",
            "http://0.0.0.0:11434",
            "http://10.0.0.1",
            "http://127.0.0.1.evil.com",
            "http://localhost.evil.com",
            "http://user:pw@127.0.0.1:11434",
            "http://127.0.0.1@evil.com",
            "https://127.0.0.1:11434",
            "ftp://127.0.0.1",
            "http://[::ffff:8.8.8.8]",
            "/usr/bin/ollama",
            "",
        ] {
            assert!(loopback_base(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn localhost_is_pinned_to_the_ipv4_loopback_address() {
        let url = loopback_base("http://localhost:11434/api/x?y=1").unwrap();
        assert_eq!(url.as_str(), "http://127.0.0.1:11434/");
    }

    #[test]
    fn probe_refuses_other_hosts_without_connecting() {
        assert_eq!(probe("http://example.com"), None);
        assert_eq!(probe("http://192.0.2.1:11434"), None);
    }
}
