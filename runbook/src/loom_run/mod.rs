//! `runbook loom-run`: one run of a case tree through the `b10x-loom` binary as a child process.
//!
//! The runner loops `tree run --max-steps 1`, asks a [`decide::Decider`] for every person or
//! authority decision Loom prints, keeps the spend of the model calls in a ledger shared between
//! runs, and writes a run directory. Files by owner (so that three units can work at once):
//!
//! - this unit (R1): `mod.rs`, `settings.rs`, `ledger.rs`, `child.rs`, `cost.rs`, `budget.rs`,
//!   `events.rs`, `decide.rs`, `run_files.rs`, `run_loop.rs`;
//! - R2 (decider): `decider.rs`, `bindings_copy.rs`;
//! - R3 (run directory writer): `run_dir.rs`.

pub mod bindings_copy;
pub mod budget;
pub mod child;
pub mod cost;
pub mod decide;
pub mod decider;
pub mod events;
pub mod ledger;
pub mod run_dir;
pub mod run_files;
pub mod run_loop;
pub mod settings;

use clap::{Args as ClapArgs, ValueEnum};
use std::path::PathBuf;
use std::process::ExitCode;

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

/// The options of `runbook loom-run` (all required; none has a default).
#[derive(ClapArgs, Clone, Debug)]
pub struct Args {
    /// Name of the run; the run directory is `<runs-dir>/<name>` and must not exist.
    #[arg(long, value_name = "NAME")]
    pub run: String,
    /// Directory that holds the run directories.
    #[arg(long, value_name = "DIR")]
    pub runs_dir: PathBuf,
    /// The `b10x-loom` binary.
    #[arg(long, value_name = "FILE")]
    pub loom: PathBuf,
    /// The sha256 (hex) the `--loom` file must have; a different one is refused before anything is written.
    #[arg(long, value_name = "HEX")]
    pub loom_sha256: String,
    /// The `bindings/` directory of a checkout (its yaml files are copied, never edited).
    #[arg(long, value_name = "DIR")]
    pub bindings: PathBuf,
    /// The directory of the protocol files (`*.protocol.yaml`) to register.
    #[arg(long, value_name = "DIR")]
    pub protocols: PathBuf,
    /// The root protocol, `name@revision` (for example `sdc@8`).
    #[arg(long, value_name = "NAME@REVISION")]
    pub root_protocol: String,
    /// The id of the root case.
    #[arg(long, value_name = "ID")]
    pub case: String,
    /// The idea file placed in the workspace.
    #[arg(long, value_name = "FILE")]
    pub idea: PathBuf,
    /// The confinement of every bound program: `local` or `substrate`. No default.
    #[arg(long, value_enum, value_name = "LEVEL")]
    pub confinement: Confinement,
    /// The run settings (JSON): principals, models, budgets, the decider, the login file's path.
    #[arg(long, value_name = "FILE")]
    pub settings: PathBuf,
    /// The spend ledger shared by every run that counts against the same cap.
    #[arg(long, value_name = "FILE")]
    pub spend_ledger: PathBuf,
    /// The spend cap in micro USD over all runs that share the ledger.
    #[arg(long, value_name = "N")]
    pub cap_micro_usd: u64,
    /// The most model calls of this run (steps and decisions together).
    #[arg(long, value_name = "N")]
    pub max_llm_calls: u64,
    /// The most wall seconds of this run.
    #[arg(long, value_name = "N")]
    pub max_wall_seconds: u64,
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

/// How a run ended; the `outcome` of `outcome.json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The root case was terminated; the tree's own outcome name when Loom printed one.
    Ended { tree_outcome: Option<String> },
    /// `tree run` exit 4.
    Stuck,
    /// The spend cap was reached before the next call.
    StoppedSpendCap,
    /// The wall-time cap was reached.
    StoppedWallTime,
    /// The model-call cap was reached.
    StoppedLlmCalls,
    /// SIGTERM or SIGINT.
    StoppedSignal,
    /// The decider answered "cannot decide".
    StoppedDecider { reason: String },
    /// Anything that failed closed.
    Error { reason: String },
}

impl Outcome {
    /// The text written as `outcome` in `outcome.json`.
    pub fn label(&self) -> &'static str {
        match self {
            Outcome::Ended { .. } => "ended",
            Outcome::Stuck => "stuck",
            Outcome::StoppedSpendCap => "stopped: spend cap",
            Outcome::StoppedWallTime => "stopped: wall time",
            Outcome::StoppedLlmCalls => "stopped: llm calls",
            Outcome::StoppedSignal => "stopped: signal",
            Outcome::StoppedDecider { .. } => "stopped: decider",
            Outcome::Error { .. } => "error",
        }
    }

    /// The reason text of the outcome, when it has one.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Outcome::StoppedDecider { reason } | Outcome::Error { reason } => Some(reason),
            Outcome::Ended { tree_outcome } => tree_outcome.as_deref(),
            _ => None,
        }
    }

    /// The exit code of the runner.
    pub fn exit_code(&self) -> u8 {
        match self {
            Outcome::Ended { .. } => EXIT_ENDED,
            Outcome::Stuck => EXIT_STUCK,
            Outcome::StoppedSpendCap => EXIT_SPEND_CAP,
            Outcome::StoppedWallTime => EXIT_WALL_TIME,
            Outcome::StoppedLlmCalls => EXIT_LLM_CALLS,
            Outcome::StoppedSignal => EXIT_SIGNAL,
            Outcome::StoppedDecider { .. } => EXIT_DECIDER,
            Outcome::Error { .. } => EXIT_ERROR,
        }
    }
}

/// The text of the error of a part that is declared and not built yet.
pub fn not_built(what: &str) -> String {
    format!("not built: {what}")
}

/// `runbook loom-run`.
pub fn command(args: Args) -> ExitCode {
    match run_loop::run(&args) {
        Ok(outcome) => {
            if let Some(reason) = outcome.reason() {
                eprintln!("runbook loom-run: {}: {reason}", outcome.label());
            } else {
                eprintln!("runbook loom-run: {}", outcome.label());
            }
            ExitCode::from(outcome.exit_code())
        }
        Err(refusal) => {
            eprintln!("runbook loom-run: refused: {refusal}");
            ExitCode::from(EXIT_REFUSED)
        }
    }
}
