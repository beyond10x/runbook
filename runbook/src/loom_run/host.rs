//! What the run loop needs from Loom, as one trait with two implementations: the `b10x-loom`
//! binary as a child process (`child_host.rs`) and Loom as a library in this process
//! (`library.rs`). The loop (`run_loop.rs`), the decider, the budget, the spend ledger, the
//! settings, the bindings copy, the private directory and the run directory files exist once and
//! serve both.
//!
//! A host is given decisions as the JSON object Loom printed (`decisions[]` of a `tree run`
//! document) and an [`Answer`] the loop worked out; how an answer reaches Loom is the host's:
//! the child runs the command Loom printed with the answer added, the library calls Loom with the
//! structured members of the decision and never reads the printed command.

use crate::loom_run::Outcome;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// What one call of Loom left: an exit status in the language of `b10x-loom` (the library gives
/// the status the command would have ended with), what the call printed, what it refused with.
#[derive(Debug, Default)]
pub struct Call {
    /// The exit status; `None` when a child was ended by a signal.
    pub code: Option<i32>,
    /// What it printed on standard output (a JSON document).
    pub stdout: Vec<u8>,
    /// What it printed on standard error.
    pub stderr: Vec<u8>,
}

/// A way out of the loop that is not an error.
pub enum Halt {
    /// The run ended with this outcome.
    Done(Outcome),
    /// The run failed closed.
    Failed(String),
    /// A `b10x-loom` call failed: the exit code and the tail of standard error go to the outcome.
    Loom { reason: String, exit: Option<i32>, stderr_tail: String },
}

impl From<String> for Halt {
    fn from(reason: String) -> Halt {
        Halt::Failed(reason)
    }
}

/// The last 1500 characters of what a call printed on standard error.
pub fn tail_of(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let text = text.trim_end();
    let skip = text.chars().count().saturating_sub(1500);
    text.chars().skip(skip).collect()
}

/// Which Loom a run used, as the run records it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Identity {
    /// The binary of a child process, by the sha256 of its bytes.
    Binary { sha256: String },
    /// The library, by the commit the manifest pins (`RUNBOOK_LOOM_COMMIT`, read from `Cargo.lock`
    /// at build time).
    Library { commit: &'static str },
}

/// The tree to open.
pub struct Opening<'a> {
    /// The protocol files to register: registration name and file.
    pub protocols: &'a [(String, PathBuf)],
    /// The run's copy of the protocol files (a directory of `*.protocol.yaml`).
    pub protocol_dir: &'a Path,
    /// The root protocol, `name@revision`.
    pub root_protocol: &'a str,
    /// The root file of the bindings copy.
    pub root_file: &'a Path,
    /// The id of the root case.
    pub case: &'a str,
}

/// The answer to one decision, as the loop works it out from the decider's verdict and the
/// bindings copy.
pub struct Answer<'a> {
    /// The decision, as Loom printed it.
    pub decision: &'a Value,
    /// Its `kind`.
    pub kind: &'a str,
    /// `false` is a denial or a rejection.
    pub approve: bool,
    /// The principal the decider named.
    pub principal: Option<&'a str>,
    /// The result a person action is taken with (`performed`, `rejected`), when it declares one.
    pub result: Option<&'static str>,
    /// The file of the workspace that holds the reasons of a rejection.
    pub reasons_file: Option<&'a Path>,
    /// A denial of a capability (a rejection of a person action names `result` instead).
    pub deny: bool,
}

/// Loom, as the run loop calls it.
pub trait LoomHost {
    /// Whether a refusal has an exit code and a standard error of its own (a child has; the
    /// library has neither, and `outcome.json` then holds `null` for both).
    fn has_exit_codes(&self) -> bool;

    /// Registers the protocols and opens the tree.
    fn open(&mut self, opening: &Opening<'_>) -> Result<(), Halt>;

    /// One step: `tree run --max-steps 1` at the run's confinement, ended by a signal or by the
    /// wall-time deadline when the host can end it.
    fn step(&mut self) -> Result<Call, Halt>;

    /// `tree status` (the document the command prints).
    fn status(&mut self) -> Result<Call, Halt>;

    /// Whether the host can answer `decision` at all; asked before the decider is.
    fn admit(&self, decision: &Value) -> Result<(), String>;

    /// The principal the answer is given as, for the event of the run ("" when there is none): the
    /// decider's, or the one the printed command names.
    fn principal(&self, answer: &Answer<'_>) -> String;

    /// Gives Loom the answer.
    fn answer(&mut self, answer: &Answer<'_>) -> Result<(), Halt>;

    /// Whether answering a handoff runs a call that may start model programs (the child runs the
    /// `tree run` it printed; the library goes on with the next step).
    fn runs_handoff(&self) -> bool;

    /// Whether the host's runner writes the `action_started` event of each bound program itself, just before the program
    /// starts (the library does: it is the one point before a program that Loom gives). A host that cannot (a child
    /// runs whole steps) has the loop write one event before each step it calls.
    fn announces_starts(&self) -> bool {
        false
    }

    /// The call a handoff is; only asked when [`LoomHost::runs_handoff`].
    fn handoff(&mut self, answer: &Answer<'_>) -> Result<Call, Halt>;

    /// `tree status` and `tree verify` at the end of the run, not ended by a signal.
    fn conclude(&mut self) -> (Result<Call, String>, Result<Call, String>);
}
