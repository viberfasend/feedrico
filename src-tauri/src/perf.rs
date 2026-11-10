//! Perf mode (spec: "Performance budgets"). With `FEEDRICO_PERF_LIBRARY=<Library folder>` the app
//! opens that Library (without touching the recent list), the UI runs its timing harness
//! (`src/lib/perf.ts`), and the shell prints the report as one `FEEDRICO_PERF {json}` line and
//! exits. Without the variable none of this exists: no script is injected and both commands refuse.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Instant;

pub const ENV: &str = "FEEDRICO_PERF_LIBRARY";
/// Tells the UI to run the harness. Injected before any page script runs.
pub const INIT_SCRIPT: &str = "window.__FEEDRICO_PERF__ = true;";

pub struct Perf {
    pub library: PathBuf,
    started: Instant,
}

impl Perf {
    pub fn from_env(started: Instant) -> Option<Self> {
        Self::from_var(std::env::var_os(ENV), started)
    }

    fn from_var(value: Option<OsString>, started: Instant) -> Option<Self> {
        let library = PathBuf::from(value.filter(|v| !v.is_empty())?);
        Some(Self { library, started })
    }

    /// Milliseconds since the process started.
    pub fn elapsed_ms(&self) -> f64 {
        self.started.elapsed().as_secs_f64() * 1000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perf_mode_needs_a_library_path() {
        let now = Instant::now();
        assert!(Perf::from_var(None, now).is_none());
        assert!(Perf::from_var(Some(OsString::new()), now).is_none());
        let perf = Perf::from_var(Some("/tmp/Feedrico Library".into()), now).unwrap();
        assert_eq!(perf.library, PathBuf::from("/tmp/Feedrico Library"));
        assert!(perf.elapsed_ms() >= 0.0);
    }
}
