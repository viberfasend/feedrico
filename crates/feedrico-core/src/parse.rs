//! Parsing fetched feed bytes (RSS, Atom, JSON Feed) into sanitized entries, ready to store.
//! Runs off the database lock: it's pure CPU work.

use url::Url;

use crate::sanitize::{self, Sanitized};
use crate::{Error, Result};

#[derive(Debug, Clone)]
pub struct ParsedFeed {
    pub title: Option<String>,
    pub site_url: Option<String>,
    pub entries: Vec<ParsedEntry>,
}

#[derive(Debug, Clone)]
pub struct ParsedEntry {
    /// Stable identity within its Feed (see "Article" in GLOSSARY.md).
    pub identity: String,
    pub url: Option<String>,
    pub title: String,
    pub author: Option<String>,
    pub content_html: String,
    pub content_text: String,
    pub excerpt: String,
    /// Unix seconds, as claimed by the feed. Plausibility is checked when storing.
    pub published_at: Option<i64>,
    pub images: Vec<String>,
    pub enclosures: Vec<ParsedEnclosure>,
}

#[derive(Debug, Clone)]
pub struct ParsedEnclosure {
    pub url: String,
    pub mime_type: Option<String>,
    pub length: Option<i64>,
}

const EXCERPT_CHARS: usize = 220;

pub fn parse(bytes: &[u8], feed_url: &str, media_prefix: &str) -> Result<ParsedFeed> {
    let feed = feed_rs::parser::Builder::new()
        .base_uri(Some(feed_url))
        .build()
        .parse(bytes)
        .map_err(|e| Error::Feed(e.to_string()))?;

    let feed_base = Url::parse(feed_url).ok();
    let site_url = feed
        .links
        .iter()
        .find(|l| l.rel.as_deref().is_none_or(|r| r == "alternate"))
        .and_then(|l| resolve(feed_base.as_ref(), &l.href));

    let entries = feed
        .entries
        .into_iter()
        .map(|entry| {
            let url = entry
                .links
                .iter()
                .find(|l| l.rel.as_deref().is_none_or(|r| r == "alternate"))
                .or(entry.links.first())
                .and_then(|l| resolve(feed_base.as_ref(), &l.href));
            let base = url
                .as_deref()
                .and_then(|u| Url::parse(u).ok())
                .or_else(|| feed_base.clone());

            let raw_html = entry
                .content
                .as_ref()
                .and_then(|c| c.body.clone())
                .filter(|b| !b.trim().is_empty())
                .or_else(|| entry.summary.as_ref().map(text_to_html))
                .unwrap_or_default();
            let Sanitized { html, images } =
                sanitize::sanitize(&raw_html, base.as_ref(), media_prefix);
            let content_text = sanitize::plain_text(&html);

            let title = entry
                .title
                .as_ref()
                .map(title_text)
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| sanitize::excerpt(&content_text, 80));

            let enclosures = entry
                .media
                .iter()
                .flat_map(|m| &m.content)
                .filter_map(|c| {
                    let url = c.url.as_ref()?.to_string();
                    Some(ParsedEnclosure {
                        url,
                        mime_type: c.content_type.as_ref().map(|t| t.to_string()),
                        length: c.size.map(|s| s as i64),
                    })
                })
                // MediaRSS images duplicate inline images; only keep real attachments.
                .filter(|e| {
                    e.mime_type.as_deref().is_none_or(|m| {
                        m.starts_with("audio/")
                            || m.starts_with("video/")
                            || m.starts_with("application/")
                    })
                })
                .collect();

            ParsedEntry {
                identity: entry.id,
                url,
                title,
                author: entry
                    .authors
                    .iter()
                    .find_map(|p| p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()))
                    .map(str::to_owned),
                excerpt: sanitize::excerpt(&content_text, EXCERPT_CHARS),
                content_html: html,
                content_text,
                published_at: entry.published.or(entry.updated).map(|d| d.timestamp()),
                images,
                enclosures,
            }
        })
        .collect();

    Ok(ParsedFeed {
        title: feed
            .title
            .as_ref()
            .map(title_text)
            .filter(|t| !t.is_empty()),
        site_url,
        entries,
    })
}

/// Titles are plain text unless the feed says otherwise; `<World>` in a plain title is text.
fn title_text(text: &feed_rs::model::Text) -> String {
    if text.content_type.to_string().contains("html") {
        sanitize::plain_text(&text.content)
    } else {
        text.content
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn resolve(base: Option<&Url>, href: &str) -> Option<String> {
    let url = match base {
        Some(base) => base.join(href.trim()).ok()?,
        None => Url::parse(href.trim()).ok()?,
    };
    matches!(url.scheme(), "http" | "https").then(|| url.to_string())
}

/// Feed summaries are often plain text with significant line breaks (see feed-rs docs).
fn text_to_html(text: &feed_rs::model::Text) -> String {
    if text.content_type.to_string().contains("html") || text.content.trim_start().starts_with('<')
    {
        return text.content.clone();
    }
    text.content
        .split("\n\n")
        .map(|p| {
            format!(
                "<p>{}</p>",
                p.replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PREFIX: &str = "media://localhost/";

    #[test]
    fn parses_rss_with_relative_links_and_enclosure() {
        let rss = br#"<?xml version="1.0"?>
<rss version="2.0"><channel>
  <title>Example &amp; Co</title><link>https://example.org/</link>
  <item>
    <title>Hello &lt;World&gt;</title>
    <link>/posts/hello</link>
    <guid>post-1</guid>
    <pubDate>Fri, 07 Nov 2025 10:00:00 GMT</pubDate>
    <description><![CDATA[<p>Hi <img src="/a.png"> there</p><script>x()</script>]]></description>
    <enclosure url="https://example.org/ep1.mp3" type="audio/mpeg" length="1234"/>
  </item>
</channel></rss>"#;
        let feed = parse(rss, "https://example.org/feed.xml", PREFIX).unwrap();
        assert_eq!(feed.title.as_deref(), Some("Example & Co"));
        assert_eq!(feed.site_url.as_deref(), Some("https://example.org/"));
        let e = &feed.entries[0];
        assert_eq!(e.identity, "post-1");
        assert_eq!(e.title, "Hello <World>");
        assert_eq!(e.url.as_deref(), Some("https://example.org/posts/hello"));
        assert_eq!(e.published_at, Some(1762509600));
        assert_eq!(e.images, vec!["https://example.org/a.png"]);
        assert!(!e.content_html.contains("script"));
        assert_eq!(e.content_text, "Hi there");
        assert_eq!(e.enclosures.len(), 1);
        assert_eq!(e.enclosures[0].mime_type.as_deref(), Some("audio/mpeg"));
    }

    #[test]
    fn parses_atom_and_json_feed() {
        let atom = br#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Atom Blog</title><id>urn:x</id><updated>2025-11-07T10:00:00Z</updated>
  <entry><title>Entry</title><id>urn:e1</id><updated>2025-11-07T10:00:00Z</updated>
    <link href="https://atom.example/e1"/><content type="html">&lt;b&gt;bold&lt;/b&gt;</content>
    <author><name>Ada</name></author></entry>
</feed>"#;
        let feed = parse(atom, "https://atom.example/feed", PREFIX).unwrap();
        assert_eq!(feed.entries[0].identity, "urn:e1");
        assert_eq!(feed.entries[0].author.as_deref(), Some("Ada"));
        assert_eq!(feed.entries[0].content_text, "bold");

        let json = br#"{"version":"https://jsonfeed.org/version/1.1","title":"J",
            "items":[{"id":"1","content_html":"<p>json</p>","url":"https://j.example/1"}]}"#;
        let feed = parse(json, "https://j.example/feed.json", PREFIX).unwrap();
        assert_eq!(
            feed.entries[0].title, "json",
            "untitled entries fall back to their text"
        );
        assert_eq!(feed.entries[0].published_at, None);
    }

    #[test]
    fn rejects_garbage() {
        assert!(
            parse(
                b"<html><body>not a feed</body></html>",
                "https://x.example",
                PREFIX
            )
            .is_err()
        );
    }
}
