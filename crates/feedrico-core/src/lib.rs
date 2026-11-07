//! Feedrico's domain core.
//!
//! Everything that touches user data lives here, independent of the UI shell, so it can be
//! tested headless. Vocabulary follows `GLOSSARY.md` at the repository root.

mod error;
pub mod library;
pub mod lock;
pub mod manifest;
mod schema;

pub use error::{Error, Result};
pub use library::{Library, OpenOptions, Status};
pub use lock::LockInfo;
pub use manifest::Manifest;

/// The newest Library format this build can read and write.
pub const FORMAT_VERSION: u32 = schema::MIGRATIONS.len() as u32;
