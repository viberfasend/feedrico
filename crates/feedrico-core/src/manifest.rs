//! `library.json`: the small, human-readable file that says "this folder is a Library" and which
//! format it uses. Readable without opening the database, so version checks are cheap and safe.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Result;

pub const FILE_NAME: &str = "library.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// Compatibility contract between app versions (ADR 0002).
    pub format_version: u32,
    /// Feedrico version that created the Library, for diagnostics only.
    pub created_with: String,
    /// Unix seconds.
    pub created_at: i64,
}

impl Manifest {
    pub fn read(library_dir: &Path) -> Result<Self> {
        let bytes = fs::read(library_dir.join(FILE_NAME))?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Writes via a temporary file and a rename, so a yanked stick never leaves half a manifest.
    pub fn write(&self, library_dir: &Path) -> Result<()> {
        let tmp = library_dir.join(format!("{FILE_NAME}.tmp"));
        let mut json = serde_json::to_vec_pretty(self)?;
        json.push(b'\n');
        fs::write(&tmp, json)?;
        fs::rename(&tmp, library_dir.join(FILE_NAME))?;
        Ok(())
    }
}
