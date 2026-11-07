//! Network access: fetching Feeds and downloading images. The only module that talks HTTP.
//! Polite by default: conditional requests, timeouts, size caps, an honest User-Agent.

use std::time::Duration;

use reqwest::StatusCode;
use reqwest::header::{CONTENT_TYPE, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};

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

    /// Downloads one image. Returns bytes and MIME type; refuses anything that isn't an image
    /// or exceeds `MAX_IMAGE_BYTES`.
    pub async fn download_image(&self, url: &str) -> Result<(Vec<u8>, String)> {
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
        if !mime.starts_with("image/") {
            return Err(Error::Network(format!("not an image: {mime}")));
        }
        if response
            .content_length()
            .is_some_and(|len| len as usize > MAX_IMAGE_BYTES)
        {
            return Err(Error::Network("image too large".into()));
        }
        Ok((read_capped(response, MAX_IMAGE_BYTES).await?, mime))
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
