//! The `.lock` file that keeps two app instances from opening one Library (ADR 0001).
//!
//! A lock is advisory: it records who opened the Library and when, so a stale lock left by a
//! crash or a yanked stick can be shown to the user and taken over deliberately.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

pub const FILE_NAME: &str = ".lock";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockInfo {
    pub machine: String,
    pub pid: u32,
    /// Unix seconds.
    pub opened_at: i64,
}

impl LockInfo {
    fn current() -> Self {
        Self {
            machine: gethostname::gethostname().to_string_lossy().into_owned(),
            pid: std::process::id(),
            opened_at: now(),
        }
    }

    fn unknown() -> Self {
        Self {
            machine: "unknown".into(),
            pid: 0,
            opened_at: 0,
        }
    }
}

/// Reads the lock of a Library, if any. An unreadable lock still counts as a lock.
pub fn read(library_dir: &Path) -> Result<Option<LockInfo>> {
    match fs::read(library_dir.join(FILE_NAME)) {
        Ok(bytes) => Ok(Some(
            serde_json::from_slice(&bytes).unwrap_or_else(|_| LockInfo::unknown()),
        )),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Holds the lock for as long as it lives; dropping it releases the lock.
#[derive(Debug)]
pub struct LockGuard {
    path: PathBuf,
}

impl LockGuard {
    pub fn acquire(library_dir: &Path, take_over: bool) -> Result<Self> {
        let path = library_dir.join(FILE_NAME);
        let json = serde_json::to_vec(&LockInfo::current())?;
        let mut options = OpenOptions::new();
        options.write(true);
        if take_over {
            options.create(true).truncate(true);
        } else {
            options.create_new(true);
        }
        match options.open(&path) {
            Ok(mut file) => {
                file.write_all(&json)?;
                file.sync_all()?;
                Ok(Self { path })
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                // The holder may release it between our open and read; report it as unknown.
                Err(Error::Locked(
                    read(library_dir)?.unwrap_or_else(LockInfo::unknown),
                ))
            }
            Err(e) => Err(e.into()),
        }
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
