//! The kind of a run directory, shared by `sdc-finding-kpis` and `sdc-run-timeline`
//! (docs/loom-run-directory.md, "The two evaluation tools").
//!
//! Two kinds hold the same case store layout (`store/<root>/events.jsonl`, `observed.jsonl`,
//! `children/`) and the same append-only records; they differ in who wrote them:
//!
//! - `Loom`: `runbook loom-run` (`runbook/src/loom_run/`). Its loop writes `settings.json` first, so
//!   that file is the mark. The case store under `store/<root>/` is a projection of Loom's run logs.
//! - `Driver`: the removed driver runtime, whose runs stay on disk. No `settings.json`;
//!   the case store was the driver's own.
//!
//! A directory with neither mark is refused. The mark is looked for in this order: Loom's first,
//! because a Loom directory also holds the driver's layout.

use std::path::{Path, PathBuf};

use crate::run_metrics as rm;

/// The file that marks a Loom run directory.
pub const LOOM_MARK: &str = "settings.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunKind {
    Driver,
    Loom,
}

/// The kind of `run`, or the refusal line that names the two files looked for.
pub fn detect(run: &Path) -> Result<RunKind, String> {
    if run.join(LOOM_MARK).is_file() {
        return Ok(RunKind::Loom);
    }
    if rm::root_case_dir(run).join("events.jsonl").is_file() {
        return Ok(RunKind::Driver);
    }
    Err(format!(
        "{} is not a run directory: it holds neither {LOOM_MARK} (runbook loom-run) nor store/<case>/events.jsonl (a run of the removed driver)",
        run.display()
    ))
}

/// The workspace directory of the case tree's files `questions.yaml`, `findings.yaml` and
/// `context.yaml`: the root case's `work/` of a driver run (its own copy), the run's `work/` (the
/// product tree) of a Loom run, whose case store keeps only `findings.yaml`.
pub fn work_dir(run: &Path, root: &Path, kind: RunKind) -> PathBuf {
    match kind {
        RunKind::Driver => root.join("work"),
        RunKind::Loom => run.join("work"),
    }
}
