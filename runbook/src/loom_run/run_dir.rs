//! R3 fills this file: the writer of the files `sdc-experiment report` and
//! `runbook parity summarize` read, from the events of a run.

use crate::loom_run::events::{Event, EventSink};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Writes the run directory's report files as events arrive. R1's version writes nothing.
pub struct RunDirWriter {
    #[allow(dead_code)]
    run_dir: PathBuf,
}

impl RunDirWriter {
    /// Opens the writer for `run_dir` (it exists; R1's files are in it). `settings` has the login
    /// path replaced by `named`.
    pub fn open(run_dir: &Path, _settings: &Value) -> Result<RunDirWriter, String> {
        Ok(RunDirWriter { run_dir: run_dir.to_path_buf() })
    }
}

impl EventSink for RunDirWriter {
    fn event(&mut self, _at_secs: u64, _event: &Event) -> Result<(), String> {
        Ok(())
    }
}
