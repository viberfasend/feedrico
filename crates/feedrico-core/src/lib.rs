//! Feedrico's domain core.
//!
//! Everything that touches user data lives here, independent of the UI shell, so it can be
//! tested headless. Vocabulary follows `GLOSSARY.md` at the repository root.

pub mod articles;
pub mod discover;
mod error;
pub mod export;
pub mod feeds;
pub mod fetch;
pub mod library;
pub mod lock;
pub mod manifest;
pub mod media;
pub mod opml;
pub mod parse;
pub mod prune;
pub mod sanitize;
mod schema;
pub mod settings;
pub mod update;

pub use articles::{ArticleDetail, ArticleRow, ListQuery, Sort, View};
pub use error::{Error, Result};
pub use export::{Export, ExportProgress};
pub use feeds::{AddOutcome, FeedToFetch, FetchResult, ImportSummary, Sidebar};
pub use fetch::{Discovery, Fetcher};
pub use library::{Library, OpenOptions, Status};
pub use lock::LockInfo;
pub use manifest::Manifest;
pub use settings::{
    AppearanceSettings, Density, Layout, Layouts, ReaderFontFamily, ReaderSettings,
    RefreshSettings, Settings, Theme, UnreadCounts,
};
pub use update::UpdateStatus;

/// The newest Library format this build can read and write.
pub const FORMAT_VERSION: u32 = schema::MIGRATIONS.len() as u32;

#[cfg(test)]
mod tests {
    use super::*;

    /// The format version is a compatibility contract (ADR 0002): it must always equal the
    /// number of migrations, or `library.json` and `PRAGMA user_version` would disagree.
    #[test]
    fn format_version_is_the_number_of_migrations() {
        assert_eq!(FORMAT_VERSION, schema::MIGRATIONS.len() as u32);
    }
}
