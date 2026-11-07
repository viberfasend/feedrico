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

/// A lock left by a process on this machine that no longer runs (crash, kill, dev restart) is
/// provably stale and can be taken over without asking. Locks from other machines always ask.
pub fn is_stale_here(info: &LockInfo) -> bool {
    info.machine == LockInfo::current().machine
        && info.pid != std::process::id()
        && !process_alive(info.pid)
}

#[cfg(target_os = "linux")]
fn process_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// Without a cheap, dependency-free check, assume it's alive and let the user decide.
#[cfg(not(target_os = "linux"))]
fn process_alive(_pid: u32) -> bool {
    true
}

/// Reads the lock of a Library, if any. An unreadable lock still counts as a lock.
/// Provably stale locks from this machine are ignored (see `is_stale_here`).
pub fn read(library_dir: &Path) -> Result<Option<LockInfo>> {
    Ok(read_raw(library_dir)?.filter(|info| !is_stale_here(info)))
}

fn read_raw(library_dir: &Path) -> Result<Option<LockInfo>> {
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
        let take_over = take_over || (path.exists() && read(library_dir)?.is_none());
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

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn dead_local_locks_are_stale_but_foreign_ones_are_not() {
        let dir = tempfile::tempdir().unwrap();
        let here = LockInfo::current().machine;
        // PIDs this high don't exist on Linux (pid_max is at most 2^22).
        let dead = format!(r#"{{"machine":"{here}","pid":4000000000,"openedAt":1}}"#);
        fs::write(dir.path().join(FILE_NAME), dead).unwrap();
        assert!(read(dir.path()).unwrap().is_none());
        let guard = LockGuard::acquire(dir.path(), false).unwrap();
        drop(guard);

        let foreign = r#"{"machine":"friend-laptop","pid":4000000000,"openedAt":1}"#;
        fs::write(dir.path().join(FILE_NAME), foreign).unwrap();
        assert!(read(dir.path()).unwrap().is_some());
        assert!(matches!(
            LockGuard::acquire(dir.path(), false),
            Err(Error::Locked(_))
        ));
    }
}
