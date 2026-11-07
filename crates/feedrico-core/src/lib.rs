//! Feedrico's domain core.
//!
//! Everything that touches user data lives here, independent of the UI shell, so it can be
//! tested headless. Vocabulary follows `GLOSSARY.md` at the repository root.

pub mod articles;
mod error;
pub mod feeds;
pub mod fetch;
pub mod library;
pub mod lock;
pub mod manifest;
pub mod media;
pub mod opml;
pub mod parse;
pub mod sanitize;
mod schema;

pub use articles::{ArticleDetail, ArticleRow, ListQuery, Sort, View};
pub use error::{Error, Result};
pub use feeds::{FeedToFetch, FetchResult, ImportSummary, Sidebar};
pub use fetch::Fetcher;
pub use library::{Library, OpenOptions, Status};
pub use lock::LockInfo;
pub use manifest::Manifest;

/// The newest Library format this build can read and write.
pub const FORMAT_VERSION: u32 = schema::MIGRATIONS.len() as u32;
