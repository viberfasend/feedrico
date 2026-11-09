//! Network access: fetching Feeds and downloading images. The only module that talks HTTP.
//! Polite by default: conditional requests, timeouts, size caps, an honest User-Agent.

use std::time::Duration;

use reqwest::StatusCode;
use reqwest::header::{CONTENT_TYPE, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};

use crate::discover::{self, Candidate};
use crate::feeds::{FeedToFetch, FetchResult};
use crate::{Error, Result, parse};

const USER_AGENT: &str = concat!(
    "Feedrico/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/viberfasend/feedrico)"
);
const MAX_FEED_BYTES: usize = 20 * 1024 * 1024;
/// Spec: "Any single image over 5 MB is skipped and stays a remote link."
pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;

/// Favicons are small; anything bigger is not worth caching.
const MAX_FAVICON_BYTES: usize = 256 * 1024;
/// How many advertised icons one lookup tries, and how much of the page it reads for them.
const MAX_ICON_CANDIDATES: usize = 4;
const MAX_ICON_PAGE_BYTES: usize = 512 * 1024;

/// What adding a Feed by URL found.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Discovery {
    /// Exactly one valid Feed.
    Found {
        url: String,
        title: String,
        site_url: Option<String>,
    },
    /// Several valid Feeds; the user picks.
    Choose { candidates: Vec<Candidate> },
}

/// How many candidate URLs one discovery validates, so a hostile page can't make us crawl.
const MAX_CANDIDATES: usize = 8;

#[derive(Clone)]
pub struct Fetcher {
    client: reqwest::Client,
    media_prefix: String,
}

impl Fetcher {
    /// `media_prefix` is where the shell serves cached images, baked into sanitized HTML.
    pub fn new(media_prefix: impl Into<String>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(8))
            .build()
            .map_err(|e| Error::Network(e.to_string()))?;
        Ok(Self {
            client,
            media_prefix: media_prefix.into(),
        })
    }

    pub fn media_prefix(&self) -> &str {
        &self.media_prefix
    }

    /// Fetches and parses one Feed. Never fails: errors become `FetchResult::Failed` so they can
    /// be recorded on the Feed.
    pub async fn fetch(&self, feed: &FeedToFetch) -> FetchResult {
        match self.try_fetch(feed).await {
            Ok(result) => result,
            Err(e) => FetchResult::Failed(e.to_string()),
        }
    }

    async fn try_fetch(&self, feed: &FeedToFetch) -> Result<FetchResult> {
        let mut request = self.client.get(&feed.url).header(
            reqwest::header::ACCEPT,
            "application/rss+xml, application/atom+xml, application/feed+json, application/xml;q=0.9, */*;q=0.8",
        );
        if let Some(etag) = &feed.etag {
            request = request.header(IF_NONE_MATCH, etag);
        }
        if let Some(modified) = &feed.last_modified {
            request = request.header(IF_MODIFIED_SINCE, modified);
        }
        let response = request.send().await.map_err(network)?;
        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(FetchResult::NotModified);
        }
        if !response.status().is_success() {
            return Err(Error::Network(format!("HTTP {}", response.status())));
        }
        let header = |name| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        let (etag, last_modified) = (header(ETAG), header(LAST_MODIFIED));
        let final_url = response.url().to_string();
        let body = read_capped(response, MAX_FEED_BYTES).await?;

        let prefix = self.media_prefix.clone();
        let parsed = tokio::task::spawn_blocking(move || parse::parse(&body, &final_url, &prefix))
            .await
            .map_err(|e| Error::Feed(e.to_string()))??;
        Ok(FetchResult::Fetched {
            feed: parsed,
            etag,
            last_modified,
        })
    }

    /// Resolves what the user typed (a Feed URL or a website URL) to a validated Feed.
    /// Order: the URL itself, then `<link rel="alternate">` on the page, then common paths.
    pub async fn discover(&self, input: &str) -> Result<Discovery> {
        let start = discover::normalize_input(input)
            .ok_or_else(|| Error::Feed("that doesn't look like a web address".into()))?;
        let (final_url, body) = self.get_body(start.as_str()).await?;
        if let Some(found) = self.validate_bytes(&body, &final_url).await {
            return Ok(found.0);
        }
        let base = url::Url::parse(&final_url).unwrap_or(start);
        let mut candidates = discover::feed_links(&body, &base);
        if candidates.is_empty() {
            candidates = discover::common_candidates(&base);
        }
        let mut valid: Vec<(Candidate, Discovery)> = Vec::new();
        for candidate in candidates.into_iter().take(MAX_CANDIDATES) {
            let Ok((url, body)) = self.get_body(&candidate.url).await else {
                continue;
            };
            if let Some((found, title)) = self.validate_bytes(&body, &url).await {
                if valid
                    .iter()
                    .any(|(_, d)| matches!(d, Discovery::Found { url: u, .. } if *u == url))
                {
                    continue;
                }
                valid.push((
                    Candidate {
                        url,
                        title: candidate.title.or(Some(title)),
                    },
                    found,
                ));
            }
        }
        match valid.len() {
            0 => Err(Error::Feed("no feed found at that address".into())),
            1 => Ok(valid.remove(0).1),
            _ => Ok(Discovery::Choose {
                candidates: valid.into_iter().map(|(c, _)| c).collect(),
            }),
        }
    }

    async fn get_body(&self, url: &str) -> Result<(String, Vec<u8>)> {
        let response = self.client.get(url).send().await.map_err(network)?;
        if !response.status().is_success() {
            return Err(Error::Network(format!("HTTP {}", response.status())));
        }
        let final_url = response.url().to_string();
        Ok((final_url, read_capped(response, MAX_FEED_BYTES).await?))
    }

    /// Parses `body` as a Feed. Returns the discovery plus the title used for display.
    async fn validate_bytes(&self, body: &[u8], url: &str) -> Option<(Discovery, String)> {
        let (body, url, prefix) = (body.to_vec(), url.to_owned(), self.media_prefix.clone());
        let parsed = tokio::task::spawn_blocking(move || {
            parse::parse(&body, &url, &prefix).ok().map(|p| (p, url))
        })
        .await
        .ok()??;
        let (feed, url) = parsed;
        let title = feed
            .title
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| url.clone());
        Some((
            Discovery::Found {
                url,
                title: title.clone(),
                site_url: feed.site_url,
            },
            title,
        ))
    }

    /// Downloads one image. Returns bytes and MIME type; refuses anything that isn't an image
    /// or exceeds `MAX_IMAGE_BYTES`.
    pub async fn download_image(&self, url: &str) -> Result<(Vec<u8>, String)> {
        self.download_capped(url, MAX_IMAGE_BYTES, false).await
    }

    /// Finds and downloads the favicon of the website at `page_url`: the icons its HTML
    /// advertises, then `/favicon.ico`. Returns the icon's URL, bytes and MIME type.
    /// Best effort: `None` when nothing usable is found.
    pub async fn fetch_favicon(&self, page_url: &str) -> Option<(String, Vec<u8>, String)> {
        let start = url::Url::parse(page_url).ok()?;
        let (base, html) = match self.get_body(start.as_str()).await {
            Ok((final_url, body)) => (
                url::Url::parse(&final_url).unwrap_or(start),
                body.into_iter().take(MAX_ICON_PAGE_BYTES).collect(),
            ),
            Err(_) => (start, Vec::new()),
        };
        for icon in discover::icon_links(&html, &base)
            .into_iter()
            .take(MAX_ICON_CANDIDATES)
        {
            if let Ok((bytes, mime)) = self.download_capped(&icon, MAX_FAVICON_BYTES, true).await {
                return Some((icon, bytes, mime));
            }
        }
        None
    }

    /// `lenient_mime`: servers often send `.ico` files as `application/octet-stream`.
    async fn download_capped(
        &self,
        url: &str,
        cap: usize,
        lenient_mime: bool,
    ) -> Result<(Vec<u8>, String)> {
        let response = self.client.get(url).send().await.map_err(network)?;
        if !response.status().is_success() {
            return Err(Error::Network(format!("HTTP {}", response.status())));
        }
        let mime = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.split(';').next().unwrap_or(v).trim().to_ascii_lowercase())
            .unwrap_or_default();
        let mime = if lenient_mime
            && mime == "application/octet-stream"
            && response.url().path().ends_with(".ico")
        {
            "image/x-icon".to_owned()
        } else {
            mime
        };
        if !mime.starts_with("image/") {
            return Err(Error::Network(format!("not an image: {mime}")));
        }
        if response
            .content_length()
            .is_some_and(|len| len as usize > cap)
        {
            return Err(Error::Network("image too large".into()));
        }
        Ok((read_capped(response, cap).await?, mime))
    }
}

async fn read_capped(mut response: reqwest::Response, cap: usize) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(network)? {
        if body.len() + chunk.len() > cap {
            return Err(Error::Network(format!(
                "response larger than {} MB",
                cap / 1024 / 1024
            )));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn network(e: reqwest::Error) -> Error {
    let message = if e.is_timeout() {
        "timed out".to_owned()
    } else if e.is_connect() {
        "could not connect".to_owned()
    } else {
        e.without_url().to_string()
    };
    Error::Network(message)
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::*;

    const RSS: &str = r#"<?xml version="1.0"?><rss version="2.0"><channel><title>Site</title><link>http://x/</link><item><title>One</title><guid>1</guid></item></channel></rss>"#;

    /// Serves `(path, content_type, body)` routes on localhost; unknown paths get 404.
    fn serve(routes: Vec<(&'static str, &'static str, String)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buf = [0u8; 2048];
                let n = stream.read(&mut buf).unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]);
                let path = request.split_whitespace().nth(1).unwrap_or("/");
                let reply = match routes.iter().find(|(p, ..)| *p == path) {
                    Some((_, kind, body)) => format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    ),
                    None => {
                        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            .into()
                    }
                };
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        base
    }

    fn fetcher() -> Fetcher {
        Fetcher::new("media://localhost/").unwrap()
    }

    #[tokio::test]
    async fn a_feed_url_is_validated_as_is() {
        let base = serve(vec![("/f.xml", "application/rss+xml", RSS.into())]);
        let found = fetcher().discover(&format!("{base}/f.xml")).await.unwrap();
        assert_eq!(
            found,
            Discovery::Found {
                url: format!("{base}/f.xml"),
                title: "Site".into(),
                site_url: Some("http://x/".into()),
            }
        );
    }

    #[tokio::test]
    async fn a_website_url_follows_its_alternate_link() {
        let page =
            r#"<head><link rel="alternate" type="application/rss+xml" href="/real.xml"></head>"#;
        let base = serve(vec![
            ("/", "text/html", page.into()),
            ("/real.xml", "application/rss+xml", RSS.into()),
        ]);
        let found = fetcher().discover(&base).await.unwrap();
        assert!(matches!(found, Discovery::Found { url, .. } if url == format!("{base}/real.xml")));
    }

    #[tokio::test]
    async fn several_valid_feeds_become_a_choice() {
        let page = r#"<link rel="alternate" type="application/rss+xml" title="Posts" href="/a.xml">
            <link rel="alternate" type="application/atom+xml" title="Comments" href="/b.xml">
            <link rel="alternate" type="application/rss+xml" href="/broken.xml">"#;
        let base = serve(vec![
            ("/", "text/html", page.into()),
            ("/a.xml", "application/rss+xml", RSS.into()),
            ("/b.xml", "application/rss+xml", RSS.into()),
            ("/broken.xml", "text/html", "<html>nope</html>".into()),
        ]);
        let Discovery::Choose { candidates } = fetcher().discover(&base).await.unwrap() else {
            panic!("expected a choice");
        };
        let titles: Vec<_> = candidates
            .iter()
            .map(|c| c.title.clone().unwrap())
            .collect();
        assert_eq!(titles, ["Posts", "Comments"]);
    }

    #[tokio::test]
    async fn common_paths_are_probed_when_the_page_advertises_nothing() {
        let base = serve(vec![
            ("/", "text/html", "<html></html>".into()),
            ("/atom.xml", "application/atom+xml", RSS.into()),
        ]);
        let found = fetcher().discover(&base).await.unwrap();
        assert!(matches!(found, Discovery::Found { url, .. } if url == format!("{base}/atom.xml")));
    }

    #[tokio::test]
    async fn favicon_comes_from_the_page_link_or_the_default_path() {
        let page = r#"<link rel="icon" href="/big.png"><link rel="icon" href="/small.png">"#;
        let base = serve(vec![
            ("/", "text/html", page.into()),
            ("/big.png", "image/png", "x".repeat(300 * 1024)),
            ("/small.png", "image/png", "PNG".into()),
        ]);
        let (url, bytes, mime) = fetcher().fetch_favicon(&base).await.unwrap();
        assert_eq!(url, format!("{base}/small.png"), "oversized icons are skipped");
        assert_eq!((bytes, mime.as_str()), (b"PNG".to_vec(), "image/png"));

        let base = serve(vec![
            ("/", "text/html", "<html></html>".into()),
            ("/favicon.ico", "application/octet-stream", "ICO".into()),
        ]);
        let (url, _, mime) = fetcher().fetch_favicon(&base).await.unwrap();
        assert_eq!(url, format!("{base}/favicon.ico"));
        assert_eq!(mime, "image/x-icon");

        let base = serve(vec![("/", "text/html", "<html></html>".into())]);
        assert!(fetcher().fetch_favicon(&base).await.is_none());
    }

    #[tokio::test]
    async fn nothing_found_is_an_error() {
        let base = serve(vec![("/", "text/html", "<html></html>".into())]);
        assert!(fetcher().discover(&base).await.is_err());
        assert!(fetcher().discover("not a url at all").await.is_err());
    }
}
