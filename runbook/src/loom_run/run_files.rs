//! The files of this unit in the run directory: `settings.json`, `events.jsonl` (one JSON object
//! per event), `outcome.json`, `tree-status.json` and `tree-verify.json`.

use crate::loom_run::events::{Event, EventSink};
use std::path::{Path, PathBuf};

/// Writes R1's files as events arrive.
pub struct BasicFiles {
    #[allow(dead_code)]
    run_dir: PathBuf,
}

impl BasicFiles {
    /// Opens the writer for `run_dir`.
    pub fn open(run_dir: &Path) -> Result<BasicFiles, String> {
        Ok(BasicFiles { run_dir: run_dir.to_path_buf() })
    }
}

impl EventSink for BasicFiles {
    fn event(&mut self, _at_secs: u64, _event: &Event) -> Result<(), String> {
        Err(crate::loom_run::not_built("run files"))
    }
}
