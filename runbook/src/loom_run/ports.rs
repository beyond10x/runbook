//! The options of the six ports of the old driver's launcher (`scripts/run-trial.sh`) as `runbook
//! run` takes them, parsed into [`Ports`]. Each port has its own file with one function that
//! refuses the port's options until the port is built: `dry.rs`, `replay.rs`, `bugfix.rs`,
//! `goals.rs`, `watch.rs`, `conformance.rs`. Building a port replaces the body of its function and
//! reads its field of [`Ports`]; no other port's file is touched.

use clap::ValueEnum;
use std::path::PathBuf;

/// A refusal of the options: the exit code and the line.
pub type Refusal = (u8, String);

/// The exit code of a usage error.
pub const USAGE: u8 = 64;

/// What to do with a call that has no recorded reply (`--on-divergence`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum OnDivergence {
    /// Stop the run (exit 9 of the old driver).
    Stop,
    /// Take the nearest recorded reply.
    Nearest,
    /// Go live, up to `--live-cap` calls.
    Live,
}

/// Whether the delivered product is held to the specification's suite (`--conformance`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Conformance {
    /// Not observed.
    #[default]
    Off,
    /// Run the suite after a close and write `oracle.json`; it decides nothing.
    Observe,
}

/// `--recorded` and the options that go with it.
#[derive(Clone, Debug, Default)]
pub struct Replay {
    /// The recorded run or recording to re-drive.
    pub recorded: Option<PathBuf>,
    /// What to do with a call that has no recorded reply.
    pub on_divergence: Option<OnDivergence>,
    /// The adapter of a live call: `dry`, `metaharness`, `claude-cli` or a path.
    pub live_adapter: Option<String>,
    /// The most live calls.
    pub live_cap: Option<u64>,
    /// Faults to inject, one per option.
    pub inject: Vec<String>,
    /// Conditions, `KEY=VALUE`, one per option (the shared `--set`).
    pub set: Vec<String>,
    /// The actions whose recorded results are taken as they are.
    pub take_recorded: Option<String>,
    /// The commit of the recorded run's source tree.
    pub source_tree: Option<String>,
}

impl Replay {
    /// The first replay option given, named; `--recorded` itself is not one.
    pub fn first_without_recorded(&self) -> Option<&'static str> {
        if self.on_divergence.is_some() {
            Some("--on-divergence")
        } else if self.live_adapter.is_some() {
            Some("--live-adapter")
        } else if self.live_cap.is_some() {
            Some("--live-cap")
        } else if !self.inject.is_empty() {
            Some("--inject")
        } else if !self.set.is_empty() {
            Some("--set")
        } else if self.take_recorded.is_some() {
            Some("--take-recorded")
        } else if self.source_tree.is_some() {
            Some("--source-tree")
        } else {
            None
        }
    }
}

/// `--bugfix` and `--bugfix-ticket`.
#[derive(Clone, Debug, Default)]
pub struct Bugfix {
    /// The seed directory.
    pub seed: Option<PathBuf>,
    /// The ticket file (`sdc.ticket/1`).
    pub ticket: Option<PathBuf>,
}

/// The options of the six ports.
#[derive(Clone, Debug, Default)]
pub struct Ports {
    /// `--dry`: the stub cycle, every model command printed and none run (built: `dry.rs`).
    pub dry: bool,
    /// The replay options.
    pub replay: Replay,
    /// The bug-fix options.
    pub bugfix: Bugfix,
    /// `--goals`: the requester's goals document.
    pub goals: Option<PathBuf>,
    /// `--watch`: record the file changes of the run.
    pub watch: bool,
    /// `--conformance`.
    pub conformance: Conformance,
}

/// The first port option given whose port is not built: the exit code and the line.
pub fn refuse_unbuilt(ports: &Ports) -> Result<(), Refusal> {
    super::dry::dry_not_built(ports)?;
    super::replay::replay_not_built(ports)?;
    super::bugfix::bugfix_not_built(ports)?;
    super::goals::goals_not_built(ports)?;
    super::watch::watch_not_built(ports)?;
    super::conformance::conformance_not_built(ports)
}
