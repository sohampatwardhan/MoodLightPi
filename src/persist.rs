use crate::state::State;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const DEFAULT_PATH: &str = "/var/lib/moodlightpi/state.json";

/// Load state; any error (missing/corrupt) yields the safe default.
pub fn load(path: &Path) -> State {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            tracing::warn!("corrupt state file, using default: {e}");
            State::default()
        }),
        Err(_) => State::default(),
    }
}

/// Atomic write: temp file in the same dir -> fsync -> rename over target.
pub fn save_atomic(path: &Path, state: &State) -> anyhow::Result<()> {
    let tmp: PathBuf = path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(state)?;
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(&json)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Rate-limited persister: skips writes when unchanged and enforces a
/// minimum interval between disk writes. Call `flush` on shutdown.
pub struct Persister {
    path: PathBuf,
    min_interval: std::time::Duration,
    last_written: Option<std::time::Instant>,
    last_state: Option<State>,
    pending: Option<State>,
}

impl Persister {
    pub fn new(path: PathBuf, min_interval: std::time::Duration) -> Self {
        let existing = load(&path);
        Self {
            path,
            min_interval,
            last_written: None,
            last_state: Some(existing),
            pending: None,
        }
    }

    /// Record a new state; writes to disk only if changed and enough time
    /// has elapsed, otherwise stashes it as pending for the next `maybe_flush`.
    pub fn record(&mut self, state: &State) {
        if self.last_state.as_ref() == Some(state) {
            return;
        }
        let due = self
            .last_written
            .map(|t| t.elapsed() >= self.min_interval)
            .unwrap_or(true);
        if due {
            self.write(state);
        } else {
            self.pending = Some(state.clone());
        }
    }

    /// Flush any pending state ignoring the rate limit (call on shutdown).
    pub fn flush(&mut self) {
        if let Some(state) = self.pending.take() {
            self.write(&state);
        }
    }

    fn write(&mut self, state: &State) {
        if let Err(e) = save_atomic(&self.path, state) {
            tracing::error!("failed to persist state: {e}");
            return;
        }
        self.last_written = Some(std::time::Instant::now());
        self.last_state = Some(state.clone());
        self.pending = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::State;

    fn tmp_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("mlp-test-{}-{}.json", name, std::process::id()));
        p
    }

    #[test]
    fn load_missing_returns_default() {
        let p = tmp_path("missing");
        let _ = std::fs::remove_file(&p);
        assert_eq!(load(&p), State::default());
    }

    #[test]
    fn save_then_load_roundtrips() {
        let p = tmp_path("roundtrip");
        let s = State { brightness: 123, ..State::default() };
        save_atomic(&p, &s).unwrap();
        assert_eq!(load(&p), s);
        assert!(!p.with_extension("json.tmp").exists(), "temp file must be renamed away");
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn load_corrupt_returns_default() {
        let p = tmp_path("corrupt");
        std::fs::write(&p, b"{ not json").unwrap();
        assert_eq!(load(&p), State::default());
        let _ = std::fs::remove_file(&p);
    }
}
