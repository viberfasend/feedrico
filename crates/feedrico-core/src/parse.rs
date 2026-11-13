//! Parsing fetched feed bytes (RSS, Atom, JSON Feed) into sanitized entries, ready to store.
//! Runs off the database lock: it's pure CPU work.

use std::cell::RefCell;
use std::collections::HashMap;

use lol_html::{RewriteStrSettings, element, rewrite_str};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::sanitize::{self, Sanitized, VideoHost, video_identity};
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
    /// The video this Article is about or embeds, if any (video Feeds and video embeds).
    pub video: Option<ParsedVideo>,
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

/// A video identity plus the metadata the Feed itself carries. Stored as JSON on the Article
/// (schema V4) and rendered as a card in the reader; never a live player (spec v1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedVideo {
    pub host: VideoHost,
    pub id: String,
    /// Seconds, from `media:content/@duration`.
    #[serde(default)]
    pub duration: Option<u64>,
    /// From `media:community/media:statistics/@views` (triage signal for the Archivist).
    #[serde(default)]
    pub views: Option<u64>,
    /// Source URL of the Feed's own thumbnail (`media:thumbnail`); cached like any image and
    /// the Lead Image of a video Feed's Articles.
    #[serde(default)]
    pub thumbnail: Option<String>,
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
            let video = entry_video(&entry, base.as_ref(), &raw_html, thumbnail.as_deref());
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
                video,
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

/// The video of an entry, if any: the `yt:video:` id prefix first, then watch/youtu.be links,
/// then a video embed iframe in the content. Duration and view count come from `entry.media`.
fn entry_video(
    entry: &feed_rs::model::Entry,
    base: Option<&Url>,
    raw_html: &str,
    thumbnail: Option<&str>,
) -> Option<ParsedVideo> {
    let (host, id) = sanitize::yt_video_id(&entry.id)
        .map(|id| (VideoHost::YouTube, id.to_owned()))
        .or_else(|| {
            entry.links.iter().find_map(|l| {
                resolve(base, &l.href)
                    .and_then(|s| Url::parse(&s).ok())
                    .and_then(|u| video_identity(&u))
            })
        })
        .or_else(|| first_iframe_video(raw_html, base))?;
    let duration = entry.media.iter().find_map(|m| {
        m.duration.map(|d| d.as_secs()).or_else(|| {
            m.content
                .iter()
                .find_map(|c| c.duration.map(|d| d.as_secs()))
        })
    });
    let views = entry
        .media
        .iter()
        .find_map(|m| m.community.as_ref().and_then(|c| c.stats_views));
    Some(ParsedVideo {
        host,
        id,
        duration,
        views,
        thumbnail: thumbnail.map(str::to_owned),
    })
}

/// The first video embed iframe in the raw content HTML (pass 1 of the sanitizer replaces it
/// with a placeholder later).
fn first_iframe_video(html: &str, base: Option<&Url>) -> Option<(VideoHost, String)> {
    let found: RefCell<Option<(VideoHost, String)>> = RefCell::new(None);
    let _ = rewrite_str(
        html,
        RewriteStrSettings::new().append_element_content_handler(element!("iframe", |el| {
            let mut found = found.borrow_mut();
            if found.is_none()
                && let Some(src) = el.get_attribute("src")
            {
                *found = resolve(base, &src)
                    .and_then(|s| Url::parse(&s).ok())
                    .and_then(|u| video_identity(&u));
            }
            Ok(())
        })),
    );
    found.into_inner()
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

    #[test]
    fn extracts_video_identity_from_youtube_channel_feed() {
        // The feed-rs YouTube fixture shape (atom_mediarss_youtube_1.xml), plus a duration.
        let atom = br#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns:yt="http://www.youtube.com/xml/schemas/2015" xmlns:media="http://search.yahoo.com/mrss/" xmlns="http://www.w3.org/2005/Atom">
    <id>yt:channel:UC7_gcs09iThXybpVgjHZ_7g</id>
    <title>PBS Space Time</title>
    <updated>2020-12-25T23:12:12+00:00</updated>
    <entry>
        <id>yt:video:0A1ouV7iD8o</id>
        <yt:videoId>0A1ouV7iD8o</yt:videoId>
        <title>Navigating with Quantum Entanglement</title>
        <link rel="alternate" href="https://www.youtube.com/watch?v=0A1ouV7iD8o"/>
        <published>2020-12-22T19:15:01+00:00</published>
        <updated>2020-12-25T23:12:12+00:00</updated>
        <media:group>
            <media:title>Navigating with Quantum Entanglement</media:title>
            <media:content url="https://www.youtube.com/v/0A1ouV7iD8o?version=3" type="application/x-shockwave-flash" width="640" height="390" duration="754"/>
            <media:thumbnail url="https://i1.ytimg.com/vi/0A1ouV7iD8o/hqdefault.jpg" width="480" height="360"/>
            <media:description>Check Out Weathered</media:description>
            <media:community>
                <media:starRating count="15020" average="4.95" min="1" max="5"/>
                <media:statistics views="304321" favorites="42"/>
            </media:community>
        </media:group>
    </entry>
</feed>"#;
        let feed = parse(
            atom,
            "https://www.youtube.com/feeds/videos.xml?channel_id=UC7_gcs09iThXybpVgjHZ_7g",
            PREFIX,
        )
        .unwrap();
        let e = &feed.entries[0];
        let video = e.video.as_ref().expect("a yt: video Feed entry is a video");
        assert_eq!(video.host, crate::sanitize::VideoHost::YouTube);
        assert_eq!(video.id, "0A1ouV7iD8o", "the id prefix wins over the link");
        assert_eq!(video.duration, Some(754), "media:content/@duration");
        assert_eq!(video.views, Some(304_321), "media:community statistics");
        assert_eq!(
            video.thumbnail.as_deref(),
            Some("https://i1.ytimg.com/vi/0A1ouV7iD8o/hqdefault.jpg")
        );
        assert_eq!(
            e.lead_image.as_deref(),
            Some("https://i1.ytimg.com/vi/0A1ouV7iD8o/hqdefault.jpg"),
            "media:thumbnail stays the Lead Image and gets cached"
        );
    }

    #[test]
    fn extracts_video_identity_from_watch_and_short_links() {
        let rss = br#"<?xml version="1.0"?>
<rss version="2.0"><channel><title>T</title>
  <item><title>watch</title><guid>w1</guid>
    <link>https://www.youtube.com/watch?v=dQw4w9WgXcQ&amp;t=42</link><description>x</description></item>
  <item><title>short</title><guid>w2</guid>
    <link>https://youtu.be/dQw4w9WgXcQ</link><description>x</description></item>
  <item><title>plain</title><guid>w3</guid>
    <link>https://blog.example/post</link><description>x</description></item>
</channel></rss>"#;
        let feed = parse(rss, "https://blog.example/feed", PREFIX).unwrap();
        let video = |i: usize| {
            feed.entries[i]
                .video
                .as_ref()
                .map(|v| (v.host, v.id.clone()))
        };
        assert_eq!(
            video(0),
            Some((
                crate::sanitize::VideoHost::YouTube,
                "dQw4w9WgXcQ".to_owned()
            ))
        );
        assert_eq!(
            video(1),
            Some((
                crate::sanitize::VideoHost::YouTube,
                "dQw4w9WgXcQ".to_owned()
            ))
        );
        assert_eq!(feed.entries[2].video, None);
    }

    #[test]
    fn extracts_video_identity_from_embed_iframes_in_content() {
        let rss = br#"<?xml version="1.0"?>
<rss version="2.0"><channel><title>T</title>
  <item><title>yt</title><guid>e1</guid><link>https://blog.example/e1</link>
    <description><![CDATA[<p>Watch:</p><iframe src="https://www.youtube.com/embed/0A1ouV7iD8o" width="560" height="315"></iframe>]]></description></item>
  <item><title>vimeo</title><guid>e2</guid><link>https://blog.example/e2</link>
    <description><![CDATA[<iframe src="https://player.vimeo.com/video/76979871"></iframe>]]></description></item>
  <item><title>other</title><guid>e3</guid><link>https://blog.example/e3</link>
    <description><![CDATA[<iframe src="https://widgets.example.com/player"></iframe>]]></description></item>
</channel></rss>"#;
        let feed = parse(rss, "https://blog.example/feed", PREFIX).unwrap();
        let yt = feed.entries[0].video.as_ref().unwrap();
        assert_eq!(
            (yt.host, yt.id.as_str()),
            (crate::sanitize::VideoHost::YouTube, "0A1ouV7iD8o")
        );
        assert_eq!(yt.duration, None, "no media: carried on a blog embed");
        let vimeo = feed.entries[1].video.as_ref().unwrap();
        assert_eq!(
            (vimeo.host, vimeo.id.as_str()),
            (crate::sanitize::VideoHost::Vimeo, "76979871")
        );
        assert!(feed.entries[2].video.is_none());
        // The iframe itself is replaced by a placeholder in the sanitized content.
        assert!(
            feed.entries[0].content_html.contains("embed-card"),
            "{}",
            feed.entries[0].content_html
        );
        assert!(!feed.entries[0].content_html.contains("<iframe"));
    }

    #[test]
    fn yt_video_id_prefix_wins_over_a_content_iframe() {
        let rss = br#"<?xml version="1.0"?>
<rss version="2.0"><channel><title>T</title>
  <item><title>both</title><guid>yt:video:AAAAAAAAAAA</guid><link>https://blog.example/both</link>
    <description><![CDATA[<iframe src="https://www.youtube.com/embed/BBBBBBBBBBB"></iframe>]]></description></item>
</channel></rss>"#;
        let feed = parse(rss, "https://blog.example/feed", PREFIX).unwrap();
        assert_eq!(feed.entries[0].video.as_ref().unwrap().id, "AAAAAAAAAAA");
    }
}

/// Content-safety edges: feed input is untrusted, so non-web links and plain text must not
/// turn into markup or URLs.
#[cfg(test)]
mod edge_tests {
    use super::*;

    const PREFIX: &str = "media://localhost/";

    #[test]
    fn non_web_links_are_dropped() {
        let rss = br#"<?xml version="1.0"?>
<rss version="2.0"><channel><title>T</title><link>javascript:alert(1)</link>
  <item><title>Evil</title><guid>e1</guid><link>javascript:alert(1)</link>
    <description>text</description></item>
</channel></rss>"#;
        let feed = parse(rss, "https://x.example/feed.xml", PREFIX).unwrap();
        assert_eq!(feed.site_url, None, "a javascript: channel link is dropped");
        assert_eq!(
            feed.entries[0].url, None,
            "a javascript: item link is dropped"
        );
    }

    #[test]
    fn plain_text_summaries_become_escaped_html() {
        let atom = br#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom"><title>T</title><id>urn:t</id>
  <entry><title>S</title><id>urn:e1</id>
    <summary>Fish &amp; Chips &lt;3

Second paragraph</summary></entry>
</feed>"#;
        let feed = parse(atom, "https://x.example/feed", PREFIX).unwrap();
        let e = &feed.entries[0];
        assert!(
            e.content_html.contains("&lt;3"),
            "the < is escaped in the HTML: {}",
            e.content_html
        );
        assert!(
            e.content_text.contains("Fish & Chips <3"),
            "the reader text is unescaped: {}",
            e.content_text
        );
        assert!(
            e.content_html.contains("<p>"),
            "paragraphs are kept: {}",
            e.content_html
        );
    }
}
