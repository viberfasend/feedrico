//! Parsing fetched feed bytes (RSS, Atom, JSON Feed) into sanitized entries, ready to store.
//! Runs off the database lock: it's pure CPU work.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
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
    /// Source URL of the Lead Image (see GLOSSARY.md), if any. Also listed in `images`.
    pub lead_image: Option<String>,
    pub enclosures: Vec<ParsedEnclosure>,
}

/// Readable kind of an enclosure. The MediaRSS `medium` attribute is dropped by feed-rs 3.0,
/// so the kind is derived from the MIME type, else the file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EnclosureKind {
    Audio,
    Video,
    Document,
}

impl EnclosureKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EnclosureKind::Audio => "audio",
            EnclosureKind::Video => "video",
            EnclosureKind::Document => "document",
        }
    }

    /// Reads the value stored in the Library; unknown values are documents.
    pub fn from_db(s: &str) -> Self {
        match s {
            "audio" => EnclosureKind::Audio,
            "video" => EnclosureKind::Video,
            _ => EnclosureKind::Document,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParsedEnclosure {
    /// Direct media URL — or the player page when only `media:player` supplied a URL
    /// (feed-rs 3.0 folds `media:player` into `MediaContent.url`).
    pub url: String,
    /// Player page (`media:player`) when the URL is a page, not a media file.
    pub player_url: Option<String>,
    pub mime_type: Option<String>,
    pub length: Option<i64>,
    /// Play time in seconds: MediaRSS `media:content duration` / JSON Feed `duration_in_seconds`.
    pub duration_secs: Option<i64>,
    pub kind: EnclosureKind,
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

    // feed-rs 3.0 maps JSON Feed attachments[] to entry.links and drops duration_in_seconds;
    // recover the durations from the raw bytes so they survive.
    let json_durations = json_attachment_durations(bytes);

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
            let Sanitized { html, mut images } =
                sanitize::sanitize(&raw_html, base.as_ref(), media_prefix);

            // Lead Image: the feed's own thumbnail first, then the first content image.
            let thumbnail = entry
                .media
                .iter()
                .flat_map(|m| m.thumbnails.iter().map(|t| t.image.uri.clone()))
                .chain(
                    entry
                        .media
                        .iter()
                        .flat_map(|m| &m.content)
                        .filter(|c| is_image(c))
                        .filter_map(|c| c.url.as_ref().map(|u| u.to_string())),
                )
                .find_map(|u| resolve(base.as_ref(), &u));
            if let Some(thumb) = &thumbnail
                && !images.contains(thumb)
            {
                images.push(thumb.clone());
            }
            let lead_image = thumbnail.or_else(|| images.first().cloned());
            let content_text = sanitize::plain_text(&html);

            let title = entry
                .title
                .as_ref()
                .map(title_text)
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| sanitize::excerpt(&content_text, 80));

            let mut enclosures: Vec<ParsedEnclosure> = entry
                .media
                .iter()
                .flat_map(|m| &m.content)
                .filter_map(enclosure_from_media_content)
                .collect();

            // JSON Feed 1.1 attachments[] land in entry.links carrying media_type/length
            // (feed-rs parser/json/mod.rs handle_attachment); Atom rel="enclosure" links have
            // the same shape. Real links (alternate etc.) never carry a media type.
            for link in &entry.links {
                let is_attachment = link.media_type.is_some()
                    && link.rel.as_deref().is_none_or(|r| r == "enclosure");
                if !is_attachment {
                    continue;
                }
                let Some(url) = resolve(base.as_ref(), &link.href) else {
                    continue;
                };
                // Don't duplicate a media:content that carries the same URL.
                if enclosures.iter().any(|e| e.url == url) {
                    continue;
                }
                if let Some(enc) = enclosure_from_link(link, &url) {
                    let duration_secs = json_durations
                        .get(&entry.id)
                        .and_then(|by_url| by_url.get(&url))
                        .copied();
                    let mut enc = enc;
                    enc.duration_secs = enc.duration_secs.or(duration_secs);
                    enclosures.push(enc);
                }
            }

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
                lead_image,
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

/// MediaRSS images often come without a MIME type; fall back to the file extension.
fn is_image(content: &feed_rs::model::MediaContent) -> bool {
    match &content.content_type {
        Some(t) => t.to_string().starts_with("image/"),
        None => content.url.as_ref().is_some_and(|u| {
            let path = u.path().to_ascii_lowercase();
            [".jpg", ".jpeg", ".png", ".gif", ".webp", ".avif"]
                .iter()
                .any(|ext| path.ends_with(ext))
        }),
    }
}

/// Plugin-era dead types (YouTube's Flash `media:content`) must not become clickable
/// enclosures, even though they sit under `application/`.
fn is_dead_type(mime: Option<&str>, url: &str) -> bool {
    const DEAD: &[&str] = &[
        "application/x-shockwave-flash",
        "application/futuresplash",
        "application/x-director",
    ];
    if let Some(m) = mime {
        let base = m.trim().to_ascii_lowercase();
        let base = base.split(';').next().unwrap_or("").trim();
        if DEAD.contains(&base) {
            return true;
        }
    }
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .to_ascii_lowercase();
    path.ends_with(".swf") || path.ends_with(".spl")
}

fn file_extension(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let name = path.rsplit('/').next()?;
    let ext = name.rsplit('.').next()?;
    (ext.len() < name.len()).then(|| ext.to_ascii_lowercase())
}

const AUDIO_EXTS: &[&str] = &[
    "mp3", "m4a", "aac", "ogg", "oga", "opus", "wav", "weba", "flac",
];
const VIDEO_EXTS: &[&str] = &[
    "mp4", "m4v", "mkv", "webm", "mov", "avi", "mpg", "mpeg", "ogv", "ts",
];
const DOCUMENT_EXTS: &[&str] = &[
    "pdf", "epub", "zip", "gz", "tar", "txt", "md", "csv", "json", "xml", "doc", "docx", "odt",
    "xls", "xlsx", "ppt", "pptx",
];

/// The readable kind: from the MIME type, else from the file extension.
fn enclosure_kind(mime: Option<&str>, url: &str) -> EnclosureKind {
    if let Some(m) = mime {
        let m = m.trim().to_ascii_lowercase();
        if m.starts_with("audio/") {
            return EnclosureKind::Audio;
        }
        if m.starts_with("video/") {
            return EnclosureKind::Video;
        }
        // A generic binary type says nothing; the extension decides.
        if !matches!(
            m.as_str(),
            "application/octet-stream" | "binary/octet-stream"
        ) {
            return EnclosureKind::Document;
        }
    }
    match file_extension(url).as_deref() {
        Some(ext) if AUDIO_EXTS.contains(&ext) => EnclosureKind::Audio,
        Some(ext) if VIDEO_EXTS.contains(&ext) => EnclosureKind::Video,
        _ => EnclosureKind::Document,
    }
}

/// feed-rs folds `media:player` into the media URL; a player page is HTML, not a media file.
fn is_player_page(mime: Option<&str>, url: &str) -> bool {
    let ext = file_extension(url);
    let known_file = ext.as_deref().is_some_and(|e| {
        AUDIO_EXTS.contains(&e) || VIDEO_EXTS.contains(&e) || DOCUMENT_EXTS.contains(&e)
    });
    if known_file {
        return false;
    }
    match mime {
        Some(m) => m.trim().to_ascii_lowercase().starts_with("text/html"),
        None => true,
    }
}

/// Only real attachments: audio, video, documents — and extension-typed files without a MIME.
fn is_enclosure_mime(mime: Option<&str>) -> bool {
    mime.is_none_or(|m| {
        let m = m.trim().to_ascii_lowercase();
        m.starts_with("audio/") || m.starts_with("video/") || m.starts_with("application/")
    })
}

fn enclosure_from_media_content(c: &feed_rs::model::MediaContent) -> Option<ParsedEnclosure> {
    // MediaRSS images duplicate inline images; only keep real attachments.
    if is_image(c) {
        return None;
    }
    let url = c.url.as_ref()?.to_string();
    let mime_type = c.content_type.as_ref().map(|t| t.to_string());
    if is_dead_type(mime_type.as_deref(), &url) || !is_enclosure_mime(mime_type.as_deref()) {
        return None;
    }
    let player_url = is_player_page(mime_type.as_deref(), &url).then(|| url.clone());
    Some(ParsedEnclosure {
        kind: enclosure_kind(mime_type.as_deref(), &url),
        url,
        player_url,
        mime_type,
        length: c.size.map(|s| s as i64),
        duration_secs: c.duration.map(|d| d.as_secs() as i64),
    })
}

/// A JSON Feed attachment (or Atom rel="enclosure" link) after feed-rs flattened it into links.
fn enclosure_from_link(link: &feed_rs::model::Link, url: &str) -> Option<ParsedEnclosure> {
    let mime = link.media_type.clone()?;
    if mime.starts_with("image/")
        || is_dead_type(Some(&mime), url)
        || !is_enclosure_mime(Some(&mime))
    {
        return None;
    }
    let player_url = is_player_page(Some(&mime), url).then(|| url.to_string());
    Some(ParsedEnclosure {
        url: url.to_string(),
        player_url,
        mime_type: Some(mime.clone()),
        length: link.length.map(|l| l as i64),
        duration_secs: None,
        kind: enclosure_kind(Some(&mime), url),
    })
}

/// JSON Feed `attachments[].duration_in_seconds` per entry id and URL, recovered from the raw
/// bytes because feed-rs 3.0 drops the field (parser/json/mod.rs JsonAttachment).
fn json_attachment_durations(bytes: &[u8]) -> HashMap<String, HashMap<String, i64>> {
    let mut durations = HashMap::new();
    let Ok(text) = std::str::from_utf8(bytes) else {
        return durations;
    };
    if !text.trim_start().starts_with('{') {
        return durations;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return durations;
    };
    let Some(items) = value.get("items").and_then(|i| i.as_array()) else {
        return durations;
    };
    for item in items {
        let Some(id) = item.get("id").and_then(|i| i.as_str()) else {
            continue;
        };
        let mut by_url = HashMap::new();
        if let Some(attachments) = item.get("attachments").and_then(|a| a.as_array()) {
            for att in attachments {
                let url = att.get("url").and_then(|u| u.as_str());
                let secs = att
                    .get("duration_in_seconds")
                    .and_then(|d| d.as_f64())
                    .map(|d| d.round() as i64);
                if let (Some(url), Some(secs)) = (url, secs) {
                    by_url.insert(url.to_string(), secs);
                }
            }
        }
        durations.insert(id.to_string(), by_url);
    }
    durations
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
        assert_eq!(e.lead_image.as_deref(), Some("https://example.org/a.png"));
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
    fn feed_thumbnail_wins_as_lead_image() {
        let rss = br#"<?xml version="1.0"?>
<rss version="2.0" xmlns:media="http://search.yahoo.com/mrss/"><channel><title>T</title>
  <item><title>Photo</title><guid>p1</guid><link>https://p.example/1</link>
    <media:content url="https://p.example/thumb.jpg" medium="image"/>
    <description><![CDATA[<p><img src="https://p.example/inline.jpg"></p>]]></description>
  </item>
  <item><title>Text only</title><guid>p2</guid><description>words</description></item>
</channel></rss>"#;
        let feed = parse(rss, "https://p.example/feed", PREFIX).unwrap();
        let photo = &feed.entries[0];
        assert_eq!(
            photo.lead_image.as_deref(),
            Some("https://p.example/thumb.jpg")
        );
        assert!(
            photo
                .images
                .contains(&"https://p.example/thumb.jpg".to_owned()),
            "thumbnail gets cached"
        );
        assert_eq!(feed.entries[1].lead_image, None);
    }

    #[test]
    fn rss2_enclosure_carries_kind() {
        let rss = br#"<?xml version="1.0"?>
<rss version="2.0"><channel><title>T</title><link>https://example.org/</link>
  <item><title>Ep</title><guid>e1</guid><link>https://example.org/e1</link>
    <enclosure url="https://example.org/ep1.mp3" type="audio/mpeg" length="1234"/>
  </item>
</channel></rss>"#;
        let feed = parse(rss, "https://example.org/feed.xml", PREFIX).unwrap();
        let enc = &feed.entries[0].enclosures[0];
        assert_eq!(enc.url, "https://example.org/ep1.mp3");
        assert_eq!(enc.mime_type.as_deref(), Some("audio/mpeg"));
        assert_eq!(enc.length, Some(1234));
        assert_eq!(enc.kind, EnclosureKind::Audio);
        assert_eq!(enc.duration_secs, None);
        assert_eq!(enc.player_url, None);
    }

    #[test]
    fn mediarss_group_gives_duration_and_kind() {
        // The Archivist (Henrik) wants episode length at a glance. feed-rs 3.0 drops the
        // medium attribute, so the kind comes from the MIME type, else the file extension.
        let rss = br#"<?xml version="1.0"?>
<rss version="2.0" xmlns:media="http://search.yahoo.com/mrss/"><channel><title>T</title>
  <item><title>Video</title><guid>v1</guid><link>https://v.example/1</link>
    <media:group>
      <media:content url="https://v.example/vid.mp4" type="video/mp4" medium="video" duration="612" fileSize="9876543"/>
      <media:content url="https://v.example/trailer.mp3"/>
      <media:content url="https://v.example/thumb.jpg" medium="image"/>
    </media:group>
  </item>
</channel></rss>"#;
        let feed = parse(rss, "https://v.example/feed", PREFIX).unwrap();
        let encs = &feed.entries[0].enclosures;
        assert_eq!(encs.len(), 2, "the image stays out of the enclosures");
        assert_eq!(encs[0].url, "https://v.example/vid.mp4");
        assert_eq!(encs[0].kind, EnclosureKind::Video);
        assert_eq!(encs[0].duration_secs, Some(612));
        assert_eq!(encs[0].length, Some(9876543));
        assert_eq!(encs[1].url, "https://v.example/trailer.mp3");
        assert_eq!(encs[1].mime_type, None);
        assert_eq!(
            encs[1].kind,
            EnclosureKind::Audio,
            "no MIME type: kind from the file extension"
        );
    }

    #[test]
    fn json_feed_attachments_become_enclosures() {
        // feed-rs maps attachments[] to entry.links (media_type/length); the item's own link
        // must not become an enclosure. duration_in_seconds is dropped by feed-rs and
        // recovered from the raw JSON.
        let json = br#"{"version":"https://jsonfeed.org/version/1.1","title":"J",
            "items":[{"id":"1","url":"https://j.example/1","content_html":"<p>hi</p>",
              "attachments":[
                {"url":"https://j.example/ep.mp3","mime_type":"audio/mpeg","size_in_bytes":4096,"duration_in_seconds":754},
                {"url":"https://j.example/notes.pdf","mime_type":"application/pdf","size_in_bytes":2048}
              ]}]}"#;
        let feed = parse(json, "https://j.example/feed.json", PREFIX).unwrap();
        let e = &feed.entries[0];
        assert_eq!(e.url.as_deref(), Some("https://j.example/1"));
        assert_eq!(
            e.enclosures.len(),
            2,
            "the item's own link is not an attachment"
        );
        assert_eq!(e.enclosures[0].url, "https://j.example/ep.mp3");
        assert_eq!(e.enclosures[0].kind, EnclosureKind::Audio);
        assert_eq!(e.enclosures[0].length, Some(4096));
        assert_eq!(e.enclosures[0].duration_secs, Some(754));
        assert_eq!(e.enclosures[1].url, "https://j.example/notes.pdf");
        assert_eq!(e.enclosures[1].kind, EnclosureKind::Document);
    }

    #[test]
    fn youtube_flash_media_content_is_not_an_enclosure() {
        let atom = br#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:media="http://search.yahoo.com/mrss/" xmlns:yt="http://www.youtube.com/xml/schemas/2015">
  <title>YT</title><id>yt:channel:UCx</id><updated>2025-11-07T10:00:00Z</updated>
  <entry>
    <id>yt:video:abc123</id><yt:videoId>abc123</yt:videoId><yt:channelId>UCx</yt:channelId>
    <title>Video</title><updated>2025-11-07T10:00:00Z</updated>
    <link rel="alternate" href="https://www.youtube.com/watch?v=abc123"/>
    <media:group>
      <media:title>Video</media:title>
      <media:thumbnail url="https://i.ytimg.com/vi/abc123/hqdefault.jpg" width="480" height="360"/>
      <media:content url="https://www.youtube.com/v/abc123?version=3" type="application/x-shockwave-flash" medium="video"/>
    </media:group>
  </entry>
</feed>"#;
        let feed = parse(
            atom,
            "https://www.youtube.com/feeds/videos.xml?channel_id=UCx",
            PREFIX,
        )
        .unwrap();
        assert!(
            feed.entries[0].enclosures.is_empty(),
            "the dead Flash-era URL must not become a clickable enclosure"
        );
        assert_eq!(
            feed.entries[0].lead_image.as_deref(),
            Some("https://i.ytimg.com/vi/abc123/hqdefault.jpg")
        );
    }

    #[test]
    fn media_player_url_is_the_open_target() {
        // media:content without a direct URL: feed-rs folds media:player into the URL.
        let rss = br#"<?xml version="1.0"?>
<rss version="2.0" xmlns:media="http://search.yahoo.com/mrss/"><channel><title>T</title>
  <item><title>Embed</title><guid>p1</guid><link>https://e.example/1</link>
    <media:content medium="video">
      <media:player url="https://player.example/watch?v=xyz"/>
    </media:content>
  </item>
</channel></rss>"#;
        let feed = parse(rss, "https://e.example/feed", PREFIX).unwrap();
        let enc = &feed.entries[0].enclosures[0];
        assert_eq!(enc.url, "https://player.example/watch?v=xyz");
        assert_eq!(
            enc.player_url.as_deref(),
            Some("https://player.example/watch?v=xyz")
        );
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
