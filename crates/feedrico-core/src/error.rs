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

    #[error("invalid setting {key}: {message}")]
    InvalidSetting { key: String, message: String },

    #[error("not found")]
    NotFound,

    #[error("invalid manifest: {0}")]
    Manifest(#[from] serde_json::Error),

    #[error(transparent)]
    Database(#[from] rusqlite::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_names_the_problem_in_plain_words() {
        assert_eq!(
            Error::NotALibrary(PathBuf::from("/x")).to_string(),
            "/x is not a Feedrico Library"
        );
        assert_eq!(
            Error::TargetNotEmpty(PathBuf::from("/x")).to_string(),
            "/x already exists and is not empty"
        );
        assert_eq!(
            Error::TooNew {
                found: 4,
                supported: 3
            }
            .to_string(),
            "this Library needs a newer Feedrico (format 4, this build supports up to 3)"
        );
        assert_eq!(
            Error::NeedsUpgrade {
                found: 1,
                target: 3
            }
            .to_string(),
            "this Library uses format 1 and must be upgraded to 3 before opening"
        );
        assert_eq!(
            Error::Opml("no <opml> element".into()).to_string(),
            "not a valid OPML file: no <opml> element"
        );
        assert_eq!(
            Error::Feed("nope".into()).to_string(),
            "not a valid feed: nope"
        );
        assert_eq!(
            Error::Network("timed out".into()).to_string(),
            "network error: timed out"
        );
        assert_eq!(Error::Invalid("bad".into()).to_string(), "bad");
        assert_eq!(
            Error::InvalidSetting {
                key: "theme".into(),
                message: "unknown".into()
            }
            .to_string(),
            "invalid setting theme: unknown"
        );
        assert_eq!(Error::NotFound.to_string(), "not found");
    }

    /// Version scenario D (docs/personas.md): a stick pulled mid-session leaves a lock whose
    /// message names the machine and the time it was opened.
    #[test]
    fn a_lock_error_names_the_machine_and_the_time() {
        let info = LockInfo {
            machine: "friend-laptop".into(),
            pid: 4242,
            opened_at: 1762547640,
        };
        assert_eq!(
            Error::Locked(info).to_string(),
            "this Library is open on friend-laptop since 1762547640"
        );
    }

    #[test]
    fn conversions_wrap_the_source_error() {
        let io: Error = std::io::Error::new(std::io::ErrorKind::NotFound, "gone").into();
        assert!(matches!(io, Error::Io(_)));
        let json = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        assert!(matches!(Error::from(json), Error::Manifest(_)));
        assert!(matches!(
            Error::from(rusqlite::Error::InvalidQuery),
            Error::Database(_)
        ));

        fn identity(x: Result<i32>) -> Result<i32> {
            x
        }
        assert_eq!(identity(Ok(1)).unwrap(), 1);
        assert!(identity(Err(Error::NotFound)).is_err());
    }
}
