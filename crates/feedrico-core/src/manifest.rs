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

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Manifest {
        Manifest {
            format_version: 3,
            created_with: "0.1.0".into(),
            created_at: 1762547640,
        }
    }

    #[test]
    fn write_then_read_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        manifest().write(dir.path()).unwrap();
        assert_eq!(Manifest::read(dir.path()).unwrap(), manifest());
        assert!(
            !dir.path().join(format!("{FILE_NAME}.tmp")).exists(),
            "the atomic write leaves no temporary file behind"
        );
    }

    #[test]
    fn the_json_on_disk_is_camel_case_and_human_readable() {
        let dir = tempfile::tempdir().unwrap();
        manifest().write(dir.path()).unwrap();
        let text = fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        assert!(text.contains("\"formatVersion\": 3"));
        assert!(text.contains("\"createdWith\": \"0.1.0\""));
        assert!(text.contains("\"createdAt\": 1762547640"));
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn an_existing_manifest_is_replaced_not_appended() {
        let dir = tempfile::tempdir().unwrap();
        manifest().write(dir.path()).unwrap();
        let newer = Manifest {
            format_version: 4,
            ..manifest()
        };
        newer.write(dir.path()).unwrap();
        assert_eq!(Manifest::read(dir.path()).unwrap().format_version, 4);
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            1,
            "only library.json remains"
        );
    }

    #[test]
    fn a_missing_or_broken_manifest_fails_clearly() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            Manifest::read(dir.path()).unwrap_err(),
            crate::Error::Io(_)
        ));
        fs::write(dir.path().join(FILE_NAME), b"{not json").unwrap();
        assert!(matches!(
            Manifest::read(dir.path()).unwrap_err(),
            crate::Error::Manifest(_)
        ));
    }
}
