use std::path::PathBuf;

use crate::lock::LockInfo;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0} is not a Feedrico Library")]
    NotALibrary(PathBuf),

    #[error("{0} already exists and is not empty")]
    TargetNotEmpty(PathBuf),

    #[error(
        "this Library needs a newer Feedrico (format {found}, this build supports up to {supported})"
    )]
    TooNew { found: u32, supported: u32 },

    #[error("this Library uses format {found} and must be upgraded to {target} before opening")]
    NeedsUpgrade { found: u32, target: u32 },

    #[error("this Library is open on {} since {}", .0.machine, .0.opened_at)]
    Locked(LockInfo),

    #[error("not a valid OPML file: {0}")]
    Opml(String),

    #[error("not a valid feed: {0}")]
    Feed(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("{0}")]
    Invalid(String),

    #[error("not found")]
    NotFound,

    #[error("invalid manifest: {0}")]
    Manifest(#[from] serde_json::Error),

    #[error(transparent)]
    Database(#[from] rusqlite::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
