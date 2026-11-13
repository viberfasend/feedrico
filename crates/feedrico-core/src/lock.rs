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

    #[test]
    fn staleness_rules_for_pids() {
        let here = LockInfo::current().machine;
        let dead_local = LockInfo {
            machine: here.clone(),
            pid: 4000000000,
            opened_at: 1,
        };
        assert!(is_stale_here(&dead_local), "dead pid on this machine");
        let own = LockInfo {
            machine: here,
            pid: std::process::id(),
            opened_at: 1,
        };
        assert!(!is_stale_here(&own), "our own live pid is not stale");
        let foreign = LockInfo {
            machine: "friend-laptop".into(),
            pid: 4000000000,
            opened_at: 1,
        };
        assert!(!is_stale_here(&foreign), "other machines always ask");
    }
}

/// Lock behaviour that holds on every OS (the Linux-only staleness rules live in `tests` above).
#[cfg(test)]
mod portable_tests {
    use super::*;

    #[test]
    fn a_missing_lock_reads_as_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read(dir.path()).unwrap(), None);
    }

    /// A lock whose JSON is unreadable (truncated write on a yanked stick) still counts as a
    /// lock, attributed to an unknown machine, so the user is asked instead of overwritten.
    #[test]
    fn a_garbage_lock_counts_as_a_lock_from_an_unknown_machine() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(FILE_NAME), b"{\"machine\":\"fri").unwrap();
        assert_eq!(read(dir.path()).unwrap(), Some(LockInfo::unknown()));
        assert!(matches!(
            LockGuard::acquire(dir.path(), false),
            Err(Error::Locked(_))
        ));
    }

    #[test]
    fn acquire_writes_machine_pid_and_time_and_refuses_a_second_instance() {
        let dir = tempfile::tempdir().unwrap();
        let guard = LockGuard::acquire(dir.path(), false).unwrap();
        let info = read(dir.path()).unwrap().unwrap();
        assert_eq!(info.machine, LockInfo::current().machine);
        assert_eq!(info.pid, std::process::id());
        assert!(info.opened_at > 0);

        // Our own live pid is not stale, so a second open is refused with the holder's info.
        match LockGuard::acquire(dir.path(), false) {
            Err(Error::Locked(held)) => assert_eq!(held.pid, std::process::id()),
            other => panic!("expected Locked, got {other:?}"),
        }
        drop(guard);
        assert_eq!(read(dir.path()).unwrap(), None, "closing removes the lock");
    }

    /// Version scenario D (docs/personas.md): the Carrier pulled the stick mid-session; the Host
    /// sees the foreign lock and can take it over deliberately with "Open anyway".
    #[test]
    fn take_over_rewrites_a_foreign_lock() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(FILE_NAME),
            br#"{"machine":"friend-laptop","pid":4242,"openedAt":1762547640}"#,
        )
        .unwrap();
        assert!(matches!(
            LockGuard::acquire(dir.path(), false),
            Err(Error::Locked(_))
        ));

        let guard = LockGuard::acquire(dir.path(), true).unwrap();
        let info = read(dir.path()).unwrap().unwrap();
        assert_eq!(info.machine, LockInfo::current().machine);
        assert_eq!(info.pid, std::process::id());
        drop(guard);
        assert!(!dir.path().join(FILE_NAME).exists());
    }

    #[test]
    fn lock_json_is_camel_case_on_the_wire() {
        let info = LockInfo {
            machine: "m".into(),
            pid: 7,
            opened_at: 42,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert_eq!(json, r#"{"machine":"m","pid":7,"openedAt":42}"#);
        assert_eq!(serde_json::from_str::<LockInfo>(&json).unwrap(), info);
    }

    #[test]
    fn acquiring_inside_a_missing_directory_is_an_io_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            LockGuard::acquire(&dir.path().join("nope"), false),
            Err(Error::Io(_))
        ));
    }
}
