//! The events of a run, as they happen, and the trait that receives them.
//!
//! The loop gives every event to every [`EventSink`]: the files of this unit
//! (`run_files::BasicFiles`: settings, outcome, event log) and the run directory writer of R3
//! (`run_dir::RunDirWriter`: the files `sdc-experiment report` reads).

use serde_json::Value;

/// Which kind of call a cost belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CostSource {
    /// A model call made by a bound program during a step.
    Step,
    /// A model call made by the decider.
    Decider,
}

/// Where the start of an action was seen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartSource {
    /// The library: the runner every bound program starts through, just before the program starts.
    ProgramStart,
    /// The child: the runner started a `tree run` call; the program that call starts is not seen.
    StepCall,
}

/// Which cap was reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cap {
    /// `--cap-micro-usd` over the shared ledger.
    Spend,
    /// `--max-llm-calls`.
    LlmCalls,
    /// `--max-wall-seconds`.
    WallTime,
}

/// One thing that happened in a run.
#[derive(Clone, Debug)]
pub enum Event {
    /// The run directory exists and the settings are written. `settings` has the login file's
    /// path replaced by the text `named`.
    RunStarted { run: String, settings: Value },
    /// One `tree run --max-steps 1` ended. `stop` is Loom's `stop` field, `steps` its `steps`,
    /// `exit` the exit code of `b10x-loom`.
    StepMade { index: u64, exit: i32, stop: String, steps: i64 },
    /// Loom printed a decision (one entry of `decisions[]`, unchanged).
    DecisionAsked { decision: Value },
    /// The decider answered; `approve` false is a denial. The answer was then recorded by
    /// `tree approve`.
    DecisionAnswered { decision: Value, approve: bool, principal: String, cost_micro_usd: u64, model_calls: u64 },
    /// The decider answered, or declined to answer, one request of the answers file. `line` is the
    /// line of `decisions.jsonl` (`answers::decision_line`).
    AnswerRecorded { line: Value },
    /// A decision whose action the answers file declares was taken after its requests were
    /// answered; its answers are the `AnswerRecorded` events before it.
    DeclaredTaken { decision: Value, principal: String },
    /// A cost was added to the ledger. `record` is the cost record as read (a line of
    /// `.sdc/llm/calls.jsonl`) or a note of the decider.
    CostedCall { source: CostSource, cost_micro_usd: u64, record: Value },
    /// A step ended before it started any adapter: its call directory `call` is no model call, it
    /// costs nothing and its reservation is released. `exit` is the exit of its `status.json`.
    NoModelCall { call: String, exit: i64 },
    /// A program of an action is about to start, or (under a child, which the runner cannot see into) a `tree run` call starts. `case`, `action`, `role` and `try_number` are `None` when the runner does not know them.
    ActionStarted { source: StartSource, role: Option<&'static str>, case: Option<String>, action: Option<String>, try_number: Option<u64> },
    /// A cap stopped the run before the next call.
    CapReached { cap: Cap, detail: String },
    /// `tree status` and `tree verify` were stored (exit codes of both).
    TreeStored { status_exit: i32, verify_exit: i32 },
    /// The run ended: `outcome` is the outcome label, `exit_code` the runner's exit code. `spent_micro_usd` and
    /// `llm_calls` count what the run started; `recorded_micro_usd` and `recorded_llm_calls` what a re-drive took from its recording.
    Ended { outcome: String, reason: Option<String>, cause: Option<String>, loom_exit: Option<i32>, stderr_tail: Option<String>, exit_code: u8, steps: u64, spent_micro_usd: u64, llm_calls: u64, recorded_micro_usd: u64, recorded_llm_calls: u64 },
}

/// Receives the events of a run in order. An error fails the run closed (outcome `error`).
pub trait EventSink {
    /// One event; `at_secs` is the runner's clock in seconds since the Unix epoch.
    fn event(&mut self, at_secs: u64, event: &Event) -> Result<(), String>;
}
