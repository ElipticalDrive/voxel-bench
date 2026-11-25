//! Pausing a timing run from outside. `MR_PAUSE_FILE` names a file; while it exists the run
//! holds between passes (no worker alive, so nothing on the GPU), and a pass under way when it
//! appears is dropped and run again on resume, so no measurement spans a pause. While holding,
//! the run writes `<file>.held` (its pid), so whoever paused it knows it is held. Without the
//! variable nothing is checked. `.agent/bin/idle` sets it for its jobs (`idle pause`/`resume`).

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// The pass was dropped because a pause was asked for (`run_pass`'s error).
#[derive(Debug)]
pub struct Paused;

impl std::fmt::Display for Paused {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("paused")
    }
}

impl std::error::Error for Paused {}

fn file() -> Option<&'static PathBuf> {
    static FILE: OnceLock<Option<PathBuf>> = OnceLock::new();
    FILE.get_or_init(|| {
        std::env::var_os("MR_PAUSE_FILE")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    })
    .as_ref()
}

/// Whether a pause is asked for now.
pub fn requested() -> bool {
    file().is_some_and(|f| f.exists())
}

/// Whether `e` is a pass dropped by a pause.
pub fn is(e: &anyhow::Error) -> bool {
    e.downcast_ref::<Paused>().is_some()
}

/// Waits while a pause is asked for.
pub fn hold() {
    let Some(f) = file().filter(|f| f.exists()) else {
        return;
    };
    let held = PathBuf::from(format!("{}.held", f.display()));
    let why = std::fs::read_to_string(f).unwrap_or_default();
    tracing::info!("paused ({}: {}); waiting", f.display(), why.trim());
    let _ = std::fs::write(&held, format!("{}\n", std::process::id()));
    let t = Instant::now();
    while f.exists() {
        std::thread::sleep(Duration::from_millis(500));
    }
    let _ = std::fs::remove_file(&held);
    tracing::info!("resumed after {:.0} s", t.elapsed().as_secs_f64());
}
