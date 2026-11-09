//! Feed autodiscovery: find the Feed URLs behind a website URL.
//!
//! Pure functions only; the network side lives in `fetch.rs`. Spec: "by website URL with
//! autodiscovery (`<link rel="alternate">` and common paths)".

use lol_html::{RewriteStrSettings, element, rewrite_str};
use serde::Serialize;
use url::Url;

/// Paths probed when a page advertises no Feed, in order.
pub const COMMON_PATHS: &[&str] = &[
    "/feed",
    "/feed/",
    "/feed.xml",
    "/rss",
    "/rss.xml",
    "/atom.xml",
    "/index.xml",
    "/feed.json",
];

const FEED_TYPES: &[&str] = &[
    "application/rss+xml",
    "application/atom+xml",
    "application/feed+json",
    "application/json",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub url: String,
    pub title: Option<String>,
}

/// Turns what the user typed into an absolute http(s) URL. A bare `example.com` gets `https://`.
pub fn normalize_input(input: &str) -> Option<Url> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    };
    let url = Url::parse(&with_scheme).ok()?;
    (matches!(url.scheme(), "http" | "https") && url.host_str().is_some()).then_some(url)
}

/// Feed links advertised by an HTML page via `<link rel="alternate" type="...">`.
/// Duplicates are dropped; relative hrefs resolve against `base`.
pub fn feed_links(html: &[u8], base: &Url) -> Vec<Candidate> {
    let mut found: Vec<Candidate> = Vec::new();
    let handler = element!("link[rel][href]", |el| {
        let rel = el.get_attribute("rel").unwrap_or_default();
        let kind = el.get_attribute("type").unwrap_or_default();
        let is_alternate = rel
            .split_ascii_whitespace()
            .any(|r| r.eq_ignore_ascii_case("alternate"));
        let is_feed = FEED_TYPES
            .iter()
            .any(|t| kind.trim().eq_ignore_ascii_case(t));
        if is_alternate
            && is_feed
            && let Some(href) = el.get_attribute("href")
            && let Ok(url) = base.join(href.trim())
        {
            let url = url.to_string();
            if !found.iter().any(|c| c.url == url) {
                let title = el
                    .get_attribute("title")
                    .map(|t| t.trim().to_owned())
                    .filter(|t| !t.is_empty());
                found.push(Candidate { url, title });
            }
        }
        Ok(())
    });
    // Malformed HTML is common; whatever was found before an error still counts.
    let text = String::from_utf8_lossy(html);
    let _ = rewrite_str(
        &text,
        RewriteStrSettings::new().append_element_content_handler(handler),
    );
    found
}

/// Common Feed locations on the site root of `base`.
pub fn common_candidates(base: &Url) -> Vec<Candidate> {
    COMMON_PATHS
        .iter()
        .filter_map(|path| base.join(path).ok())
        .map(|url| Candidate {
            url: url.to_string(),
            title: None,
        })
        .collect()
}

/// Icon URLs advertised by an HTML page via `<link rel="icon">` (also `shortcut icon`), in
/// document order, followed by `/favicon.ico` on the site root unless already listed.
pub fn icon_links(html: &[u8], base: &Url) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let handler = element!("link[rel][href]", |el| {
        let rel = el.get_attribute("rel").unwrap_or_default();
        if rel
            .split_ascii_whitespace()
            .any(|r| r.eq_ignore_ascii_case("icon"))
            && let Some(href) = el.get_attribute("href")
            && let Ok(url) = base.join(href.trim())
            && matches!(url.scheme(), "http" | "https")
        {
            let url = url.to_string();
            if !found.contains(&url) {
                found.push(url);
            }
        }
        Ok(())
    });
    let text = String::from_utf8_lossy(html);
    let _ = rewrite_str(
        &text,
        RewriteStrSettings::new().append_element_content_handler(handler),
    );
    if let Ok(default) = base.join("/favicon.ico") {
        let default = default.to_string();
        if !found.contains(&default) {
            found.push(default);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_links_resolve_and_end_with_the_default_path() {
        let html = br#"<link rel="shortcut icon" href="/i.png">
            <link rel="apple-touch-icon" href="/apple.png">
            <link rel="icon" href="//cdn.example/x.svg"><link rel="icon" href="javascript:x">"#;
        assert_eq!(
            icon_links(html, &base()),
            [
                "https://example.com/i.png",
                "https://cdn.example/x.svg",
                "https://example.com/favicon.ico"
            ]
        );
        assert_eq!(
            icon_links(b"<html></html>", &base()),
            ["https://example.com/favicon.ico"]
        );
    }

    fn base() -> Url {
        Url::parse("https://example.com/blog/").unwrap()
    }

    #[test]
    fn normalizes_bare_hosts_and_rejects_junk() {
        assert_eq!(
            normalize_input("  example.com/blog ").unwrap().as_str(),
            "https://example.com/blog"
        );
        assert_eq!(
            normalize_input("http://example.com").unwrap().scheme(),
            "http"
        );
        assert!(normalize_input("").is_none());
        assert!(normalize_input("ftp://example.com").is_none());
        assert!(normalize_input("javascript:alert(1)").is_none());
    }

    #[test]
    fn finds_alternate_feed_links_and_resolves_relative_hrefs() {
        let html = br#"<html><head>
            <link rel="stylesheet" href="/a.css">
            <link rel="alternate" type="application/rss+xml" title="Posts" href="/feed.xml">
            <link rel="ALTERNATE" type="application/atom+xml" href="atom.xml">
            <link rel="alternate" type="application/rss+xml" href="/feed.xml">
            <link rel="alternate" type="text/html" href="/de/">
            <link rel="alternate feed" type="application/feed+json" href="https://other.example/f.json">
        </head></html>"#;
        let found = feed_links(html, &base());
        assert_eq!(
            found,
            vec![
                Candidate {
                    url: "https://example.com/feed.xml".into(),
                    title: Some("Posts".into())
                },
                Candidate {
                    url: "https://example.com/blog/atom.xml".into(),
                    title: None
                },
                Candidate {
                    url: "https://other.example/f.json".into(),
                    title: None
                },
            ]
        );
    }

    #[test]
    fn survives_malformed_html() {
        let html = b"<html><head><link rel=alternate type=application/rss+xml href=/f><p><<<";
        assert_eq!(feed_links(html, &base()).len(), 1);
    }

    #[test]
    fn common_candidates_hang_off_the_site_root() {
        let urls: Vec<_> = common_candidates(&Url::parse("https://example.com/blog/post").unwrap())
            .into_iter()
            .map(|c| c.url)
            .collect();
        assert_eq!(urls[0], "https://example.com/feed");
        assert!(urls.contains(&"https://example.com/atom.xml".to_owned()));
    }
}
