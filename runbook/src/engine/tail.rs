//! The run logs of the tree's store, reported to the event sink line by line.
//!
//! Loom's `run_tree` gives its observer each line it writes. The lines `tree open`, `tree approve`
//! and `tree close` write do not pass an observer, so after each of those calls the engine reads
//! the logs and reports what it has not reported yet. A line is reported as Loom encodes it
//! (`b10x_loom_runlog::encode`), once, in the order of its log.

use crate::runtime::EventSink;
use b10x_loom_runlog::{encode, EXTENSION, PREFIX};
use b10x_loom_tree::runlog::RunLogLine;
use b10x_loom_tree::{log_file_name, read_log, RunObserver};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct LogTail<'a> {
    store: PathBuf,
    sink: &'a dyn EventSink,
    /// Per log file: the `seq` the next line to report has.
    next: HashMap<String, i64>,
    /// The first line that could not be read or reported; the loop looks at it after each call.
    pub failure: Option<String>,
}

impl<'a> LogTail<'a> {
    pub fn new(store: &Path, sink: &'a dyn EventSink) -> LogTail<'a> {
        LogTail { store: store.to_path_buf(), sink, next: HashMap::new(), failure: None }
    }

    fn files(&self) -> Result<Vec<String>, String> {
        let rd = std::fs::read_dir(&self.store).map_err(|e| format!("{}: {e}", self.store.display()))?;
        let mut names: Vec<String> =
            rd.flatten().filter_map(|e| e.file_name().into_string().ok()).filter(|n| n.starts_with(PREFIX) && n.ends_with(EXTENSION)).collect();
        names.sort();
        Ok(names)
    }

    fn read(&self, file: &str) -> Result<Vec<RunLogLine>, String> {
        let bytes = std::fs::read(self.store.join(file)).map_err(|e| format!("{file}: {e}"))?;
        read_log(&bytes).map_err(|r| format!("{file}: the run log is refused {:?} at line {}: {}", r.refusal, r.line, r.detail))
    }

    fn report(&mut self, file: &str, line: &RunLogLine) {
        let text = encode(line);
        match serde_json::from_str::<Value>(&text) {
            Ok(record) => {
                let kind = record.get("kind").and_then(Value::as_str).unwrap_or("").to_string();
                self.sink.record(&kind, record);
                self.next.insert(file.to_string(), line.seq + 1);
            }
            Err(e) => {
                self.failure.get_or_insert(format!("{file}: a line that Loom encoded does not read as JSON: {e}"));
            }
        }
    }

    /// Report the lines of one log not reported yet.
    pub fn sync_file(&mut self, file: &str) -> Result<(), String> {
        let from = self.next.get(file).copied().unwrap_or(0);
        for line in self.read(file)? {
            if line.seq >= from {
                self.report(file, &line);
            }
        }
        Ok(())
    }

    /// Report the lines of every log not reported yet.
    pub fn sync_all(&mut self) -> Result<(), String> {
        for file in self.files()? {
            self.sync_file(&file)?;
        }
        Ok(())
    }

    /// Take every line now in the logs as reported, without reporting it (a run found on disk: its
    /// earlier lines reached the daemon before).
    pub fn mark_all_seen(&mut self) -> Result<(), String> {
        for file in self.files()? {
            let next = self.read(&file)?.last().map_or(0, |l| l.seq + 1);
            self.next.insert(file, next);
        }
        Ok(())
    }

    /// What `run_tree` calls for each line it writes.
    pub fn observer(&mut self) -> Observer<'_, 'a> {
        Observer { tail: self }
    }
}

pub struct Observer<'t, 'a> {
    tail: &'t mut LogTail<'a>,
}

impl RunObserver for Observer<'_, '_> {
    fn line(&mut self, case: &str, line: &RunLogLine) {
        let file = log_file_name(case);
        let next = self.tail.next.get(&file).copied().unwrap_or(0);
        if line.seq == next {
            self.tail.report(&file, line);
        } else if line.seq > next {
            if let Err(e) = self.tail.sync_file(&file) {
                self.tail.failure.get_or_insert(e);
            }
        }
    }
}
