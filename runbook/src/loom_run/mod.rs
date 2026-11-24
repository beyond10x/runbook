//! `runbook run` (in this process, `here.rs`): one run of a case tree through Loom, as the
//! `b10x-loom` binary in a child process (`--loom`, `child_host.rs`) or as a library in this
//! process (no `--loom`, `library.rs`); the loop reaches either through the trait of `host.rs`.
//!
//! The runner loops one step at a time (`tree run --max-steps 1`), asks a [`decide::Decider`] for
//! every person or authority decision Loom gives, counts the model calls and their spend, and
//! writes a run directory. The money options (`--spend-ledger`, `--cap-micro-usd`,
//! `--max-llm-calls`) are each optional: none named is no limit and no ledger; a cap that is named
//! stops the run when it is reached, a ledger that is named records what every run sharing it
//! spent. Files by owner (so that three units can work at once):
//!
//! - this unit (R1): `mod.rs`, `settings.rs`, `ledger.rs`, `child.rs`, `cost.rs`, `budget.rs`,
//!   `events.rs`, `decide.rs`, `run_files.rs`, `run_loop.rs`, `host.rs`, `child_host.rs`,
//!   `library.rs`;
//! - R2 (decider): `decider.rs`, `bindings_copy.rs`;
//! - R3 (run directory writer): `run_dir.rs`.

pub mod answers;
pub mod arbiter;
pub mod bindings_copy;
pub use runbook::budget;
pub mod bugfix;
pub mod child;
pub mod child_host;
pub mod conformance;
pub use runbook::cost;
pub mod decide;
pub mod decider;
pub mod dry;
pub mod events;
pub mod gateway;
pub mod goals;
pub mod here;
pub mod host;
pub mod in_flight;
pub use runbook::ledger;
pub mod library;
pub mod model_node;
pub mod ports;
pub mod private;
pub use runbook::record_write;
pub mod replay;
pub mod run_dir;
pub mod run_files;
pub mod run_loop;
pub mod seeded;
pub mod settings;
pub mod watch;

use clap::ValueEnum;
use std::path::PathBuf;

/// The confinement every bound program of the tree starts under; passed to every `tree run` as
/// given. There is no default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Confinement {
    /// Unconfined, declared insecure, through Substrate's local adapter.
    Local,
    /// Substrate's linux adapter.
    Substrate,
}

impl Confinement {
    /// The value `b10x-loom tree run --confinement` takes.
    pub fn as_arg(self) -> &'static str {
        match self {
            Confinement::Local => "local",
            Confinement::Substrate => "substrate",
        }
    }
}

/// What a run needs, resolved from the options of `runbook run` (`here.rs`).
#[derive(Clone, Debug)]
pub struct Args {
    /// Name of the run; the run directory is `<runs-dir>/<name>` and must not exist.
    pub run: String,
    /// Directory that holds the run directories.
    pub runs_dir: PathBuf,
    /// The `b10x-loom` binary, run as a child process (with `loom_sha256`); neither: Loom is a
    /// library of this process.
    pub loom: Option<PathBuf>,
    /// The sha256 (hex) the `loom` file must have; a different one is refused before anything is written.
    pub loom_sha256: Option<String>,
    /// The `bindings/` directory (its yaml files are copied, never edited).
    pub bindings: PathBuf,
    /// The directory of the protocol files (`*.protocol.yaml`) to register.
    pub protocols: PathBuf,
    /// The root protocol, `name@revision` (for example `sdc@8`).
    pub root_protocol: String,
    /// The id of the root case.
    pub case: String,
    /// The text of the idea; the runner writes the idea file the tree reads from it.
    pub intake: String,
    /// The confinement of every bound program.
    pub confinement: Confinement,
    /// The run settings file (TOML, `docs/loom-run-settings.md`) the caller named, if any: its
    /// members win over `defaults`.
    pub settings: Option<PathBuf>,
    /// The run settings members of `runbook.toml` (`config.rs`): the defaults under `settings`.
    pub defaults: serde_json::Map<String, serde_json::Value>,
    /// The spend ledger shared by every run that counts against the same cap. `None`: no ledger is
    /// opened; a cap that is named is then counted in the run itself.
    pub spend_ledger: Option<PathBuf>,
    /// The spend cap in micro USD over all runs that share the ledger. `None`: no spend cap. A
    /// named 0 means spend nothing.
    pub cap_micro_usd: Option<u64>,
    /// The most model calls of this run (steps and decisions together). `None`: no limit. A named
    /// 0 means no call.
    pub max_llm_calls: Option<u64>,
    /// The most wall seconds of this run.
    pub max_wall_seconds: u64,
    /// The options of the six ports of the old driver's launcher (`ports.rs`).
    pub ports: ports::Ports,
    /// The `runbook.toml` the values were taken from, by name and origin (never the path): the member
    /// `config` of the run's start record. `None`: no file, or a file that stated nothing.
    pub config: Option<String>,
}

/// How the run reaches Loom, from the options: one of the two ways.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Way<'a> {
    /// `--loom <bin> --loom-sha256 <hex>`.
    Child { loom: &'a std::path::Path, sha256: &'a str },
    /// Neither option: Loom as a library of this process.
    Library,
}

impl Args {
    /// Whether Loom is a library of this process (no `--loom`, no `--loom-sha256`).
    pub fn in_process(&self) -> bool {
        self.loom.is_none() && self.loom_sha256.is_none()
    }

    /// The way the options name, or the usage error (a refusal, exit 2, before anything is written).
    pub fn way(&self) -> Result<Way<'_>, String> {
        const USAGE: &str = "usage: give --loom <bin> with --loom-sha256 <hex> to run the binary as a child, or neither for Loom as a library";
        match (self.loom.as_deref(), self.loom_sha256.as_deref()) {
            (None, None) => Ok(Way::Library),
            (Some(loom), Some(sha256)) => Ok(Way::Child { loom, sha256 }),
            _ => Err(format!("{USAGE}: --loom and --loom-sha256 go together")),
        }
    }
}

/// Exit code: the tree ended through its one legitimate outcome.
pub const EXIT_ENDED: u8 = 0;
/// Exit code: the run failed closed (see the outcome file's reason).
pub const EXIT_ERROR: u8 = 1;
/// Exit code: refused before anything was written.
pub const EXIT_REFUSED: u8 = 2;
/// Exit code: the tree is stuck (nothing useful, no outcome, nothing awaiting a decision).
pub const EXIT_STUCK: u8 = 3;
/// Exit code: stopped by the spend cap.
pub const EXIT_SPEND_CAP: u8 = 4;
/// Exit code: stopped by the wall-time cap.
pub const EXIT_WALL_TIME: u8 = 5;
/// Exit code: stopped by the model-call cap.
pub const EXIT_LLM_CALLS: u8 = 6;
/// Exit code: stopped by a signal.
pub const EXIT_SIGNAL: u8 = 7;
/// Exit code: the decider could not decide.
pub const EXIT_DECIDER: u8 = 8;
/// Exit code: the decider denied a decision and the tree stopped on it again.
pub const EXIT_DENIED: u8 = 9;
/// Exit code: the decider rejected a decision and nothing records a rejection (the old driver's exit 6,
/// cause `rejected_without_rework_path`; 6 is the model-call cap here).
pub const EXIT_REJECTED: u8 = 10;
/// Exit code: a re-drive of a recording (`--recorded`) reached a call the recording has no reply for.
pub const EXIT_DIVERGED: u8 = 11;
/// Exit code: Loom refused a call of the run by a named reason (`TreeStopped`, `ConfinementRefused`,
/// `PersonProgramFailed`, ...): the cause in `outcome.json` and `run-stopped.json` is the reason's name.
/// Also a tree left without a move by a bound program that Loom refused (`ProgramSignalled`,
/// `ProgramError`): the cause is `program_signalled` or `program_error`.
pub const EXIT_LOOM_REFUSED: u8 = 12;

/// How a run ended; the `outcome` of `outcome.json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The root case was terminated; the tree's own outcome name when Loom printed one.
    Ended { tree_outcome: Option<String> },
    /// `tree run` exit 4: no move and, when a bound program was refused for something other than an exit or a signal, the lead of that refusal (`<Reason>: action <name>`).
    Stuck { reason: Option<String> },
    /// The spend cap was reached before the next call.
    StoppedSpendCap,
    /// The wall-time cap was reached.
    StoppedWallTime,
    /// The model-call cap was reached.
    StoppedLlmCalls,
    /// SIGTERM or SIGINT.
    StoppedSignal,
    /// The decider rejected a decision and the action declares no result that records a rejection:
    /// there is no rework path (cause `rejected_without_rework_path`).
    StoppedRejected { reason: String },
    /// The decider answered "cannot decide".
    StoppedDecider { reason: String },
    /// Anything that failed closed.
    Error { reason: String },
    /// A `b10x-loom` call failed (an exit code the runner does not know, or a printed command that
    /// failed): outcome `error`, with the exit code and the tail of standard error.
    LoomFailed { reason: String, exit: Option<i32>, stderr_tail: String },
    /// The decider denied a decision and the tree stopped on the same decision again.
    Denied { action: String },
    /// A re-drive of a recording reached a call the recording has no reply for (policy `stop`, a spent live cap).
    Diverged { reason: String },
    /// Loom refused a call of the run by a named reason: `cause` is the reason's snake-case name
    /// (`engine::refusal::cause`), `reason` Loom's text. A child's refusal also has the exit code and
    /// the tail of standard error.
    Refused { cause: &'static str, reason: String, loom_exit: Option<i32>, stderr_tail: Option<String> },
}

impl Outcome {
    /// The named cause of the outcome, when it has one beyond its label.
    pub fn cause(&self) -> Option<&'static str> {
        match self {
            Outcome::Refused { cause, .. } => Some(cause),
            _ => None,
        }
    }

    /// The text written as `outcome` in `outcome.json`.
    pub fn label(&self) -> &'static str {
        match self {
            Outcome::Ended { .. } => "ended",
            Outcome::Stuck { .. } => "stuck",
            Outcome::StoppedSpendCap => "stopped: spend cap",
            Outcome::StoppedWallTime => "stopped: wall time",
            Outcome::StoppedLlmCalls => "stopped: llm calls",
            Outcome::StoppedSignal => "stopped: signal",
            Outcome::StoppedDecider { .. } => "stopped: decider",
            Outcome::StoppedRejected { .. } => "stopped: rejected",
            Outcome::Error { .. } => "error",
            Outcome::LoomFailed { .. } => "error",
            Outcome::Denied { .. } => "denied",
            Outcome::Diverged { .. } => "stopped: recording diverged",
            Outcome::Refused { .. } => "refused",
        }
    }

    /// The reason text of the outcome, when it has one.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Outcome::StoppedDecider { reason } | Outcome::StoppedRejected { reason } | Outcome::Error { reason } | Outcome::LoomFailed { reason, .. } | Outcome::Diverged { reason } | Outcome::Refused { reason, .. } => Some(reason),
            Outcome::Denied { action } => Some(action),
            Outcome::Ended { tree_outcome } => tree_outcome.as_deref(),
            Outcome::Stuck { reason } => reason.as_deref(),
            _ => None,
        }
    }

    /// The exit code of the runner.
    pub fn exit_code(&self) -> u8 {
        match self {
            Outcome::Ended { .. } => EXIT_ENDED,
            Outcome::Stuck { .. } => EXIT_STUCK,
            Outcome::StoppedSpendCap => EXIT_SPEND_CAP,
            Outcome::StoppedWallTime => EXIT_WALL_TIME,
            Outcome::StoppedLlmCalls => EXIT_LLM_CALLS,
            Outcome::StoppedSignal => EXIT_SIGNAL,
            Outcome::StoppedDecider { .. } => EXIT_DECIDER,
            Outcome::StoppedRejected { .. } => EXIT_REJECTED,
            Outcome::Error { .. } | Outcome::LoomFailed { .. } => EXIT_ERROR,
            Outcome::Denied { .. } => EXIT_DENIED,
            Outcome::Diverged { .. } => EXIT_DIVERGED,
            Outcome::Refused { .. } => EXIT_LOOM_REFUSED,
        }
    }
}

#[cfg(test)]
mod outcome_tests {
    use super::*;
    use b10x_loom_tree::model::TreeRefusalReason;
    use runbook::engine::refusal::{cause, REASONS};

    /// A run that Loom refused by a named reason ends `refused`, exit 12, and the reason's own name
    /// is its cause: every reason of the pinned Loom, none through a catch-all.
    #[test]
    fn a_refusal_of_loom_ends_the_run_refused_with_its_reason_as_the_cause() {
        let mut causes = std::collections::BTreeSet::new();
        for reason in REASONS {
            let outcome = Outcome::Refused { cause: cause(reason), reason: format!("{reason:?}: x"), loom_exit: None, stderr_tail: None };
            assert_eq!((outcome.label(), outcome.exit_code(), outcome.cause()), ("refused", EXIT_LOOM_REFUSED, Some(cause(reason))), "{reason:?}");
            assert!(outcome.reason().is_some_and(|r| r.starts_with(&format!("{reason:?}"))));
            assert!(causes.insert(cause(reason)));
        }
        for named in [TreeRefusalReason::TreeStopped, TreeRefusalReason::ConfinementRefused, TreeRefusalReason::PersonProgramFailed] {
            assert!(causes.contains(cause(named)), "{named:?}");
        }
        assert_eq!(Outcome::Error { reason: "x".into() }.cause(), None);
    }

    #[test]
    fn the_exit_codes_are_all_different() {
        let outcomes = [
            Outcome::Ended { tree_outcome: None },
            Outcome::Error { reason: String::new() },
            Outcome::Stuck { reason: None },
            Outcome::StoppedSpendCap,
            Outcome::StoppedWallTime,
            Outcome::StoppedLlmCalls,
            Outcome::StoppedSignal,
            Outcome::StoppedDecider { reason: String::new() },
            Outcome::Denied { action: String::new() },
            Outcome::StoppedRejected { reason: String::new() },
            Outcome::Diverged { reason: String::new() },
            Outcome::Refused { cause: "tree_stopped", reason: String::new(), loom_exit: None, stderr_tail: None },
        ];
        let codes: std::collections::BTreeSet<u8> = outcomes.iter().map(Outcome::exit_code).collect();
        assert_eq!(codes.len(), outcomes.len(), "{codes:?}");
    }
}
