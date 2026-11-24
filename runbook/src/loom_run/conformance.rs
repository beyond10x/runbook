//! The `conformance` port of `runbook run` (`--conformance observe`): after the run ended closed,
//! the delivered product in the run's workspace is held to the conformance suite of the run's own
//! specification and `oracle.json` is written into the run directory (`docs/conformance.md`). The
//! observation decides nothing: whatever it does, the run's outcome and exit are as they were.
//!
//! The observation is `sdc_tools::conform::observe` (the library the `sdc-conform` program is the
//! command line of). It runs ess through `bindings/tools/sdc-tool.sh ess`, which pins the release,
//! never an `ess` of its own; the workspace is `<run>/work`, whatever the root case is called.

use super::ports::{Conformance, Ports, Refusal};
use super::{Args, Outcome};
use sdc_tools::conform::{self, EssTool, Setup};
use std::path::Path;

/// What `--conformance` leaves to refuse: nothing. Both modes are built; the function stays because
/// `ports::refuse_unbuilt` calls one function per port.
pub fn conformance_not_built(_ports: &Ports) -> Result<(), Refusal> {
    Ok(())
}

/// Where the observation finds its parts, from the options of the run and the asset root.
/// `budget_micro_usd` is the most its node sessions may spend (the reservation) when a spend cap is named.
fn setup_of(args: &Args, run_dir: &Path, budget_micro_usd: Option<u64>) -> Setup {
    let lookup = crate::assets::Lookup::from_process();
    let assets = lookup.locate().ok();
    let instructions = assets.as_ref().map(|a| a.root.clone()).or_else(|| args.bindings.parent().map(Path::to_path_buf));
    Setup {
        work: Some("work".into()),
        ess: Some(EssTool { tool: args.bindings.join("tools/sdc-tool.sh"), cwd: run_dir.join("conformance/ess-cwd") }),
        tools: Some(args.bindings.join("tools")),
        instructions,
        adapter: lookup.helper("sdc-conform-adapter", assets.as_ref()),
        budget_usd: budget_micro_usd.map(|micro| micro as f64 / 1e6),
    }
}

/// What the observation cost, as far as its own record says.
pub enum Spent {
    /// No table slot was reached: no model could have been called.
    Nothing,
    /// The cost its record names, micro USD.
    Cost(u64),
    /// Not known (a failure, a record that names no cost, or a model session that ended without a
    /// cost record): the most it could have spent.
    Unknown,
}

/// What stands between the observation and a named spend cap (`run_loop/observation.rs`): `start`
/// refuses with the line to print when nothing is left, `finish` books what the observation cost.
pub trait Gate {
    /// `Err(line)`: the observation is not started. `Ok(budget)`: it is; `budget` is the most its
    /// model sessions may spend together, micro USD, when a spend cap is named (what the reservation
    /// holds), none when no cap is named.
    fn start(&mut self) -> Result<Option<u64>, String>;
    /// Called once after a started observation.
    fn finish(&mut self, spent: Spent);
}

/// What an observation's record says it cost. A model session that ran and left no cost record
/// (its adapter was killed or crashed) is a session of unknown cost, so the observation is.
fn spent_of(doc: &serde_json::Value) -> Spent {
    let record = &doc["conformance"];
    if record.get("cost_unknown_sessions").and_then(serde_json::Value::as_u64).is_some_and(|n| n > 0) {
        return Spent::Unknown;
    }
    match record.get("table_cost_usd").filter(|v| !v.is_null()) {
        Some(cost) => crate::loom_run::cost::usd_to_micro(cost).map_or(Spent::Unknown, Spent::Cost),
        None if record.get("tables_written").is_some() => Spent::Unknown,
        None => Spent::Nothing,
    }
}

/// Observes the product of a run that ended, when `--conformance observe` was given. The line it
/// prints says what was written or why nothing was; a failure is the line or `oracle.json`'s
/// `unknown` with its cause, never a change of the run's outcome (so a run that closed ends with
/// the exit it had, also when the observation is not started for a spend cap that has nothing left).
pub fn after_close(args: &Args, run_dir: &Path, outcome: &Outcome, gate: &mut dyn Gate) {
    if args.ports.conformance != Conformance::Observe {
        return;
    }
    if !matches!(outcome, Outcome::Ended { .. }) {
        eprintln!("runbook run: conformance: not observed (the run did not close: {})", outcome.label());
        return;
    }
    let tables = match conform::tables_from_env() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("runbook run: conformance: not observed ({e})");
            return;
        }
    };
    let budget = match gate.start() {
        Ok(budget) => budget,
        Err(line) => {
            eprintln!("runbook run: conformance: {line}");
            return;
        }
    };
    let setup = setup_of(args, run_dir, budget);
    let pin = Path::new("driver/ess-pin.json");
    let seen = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| conform::observe(run_dir, None, tables, pin, &setup)));
    match seen {
        Ok(Ok(doc)) => {
            gate.finish(spent_of(&doc));
            eprintln!("runbook run: conformance: {}", doc["conformance"]["summary"].as_str().unwrap_or(""));
        }
        Ok(Err(e)) => {
            gate.finish(Spent::Unknown);
            eprintln!("runbook run: conformance: not observed ({e})");
        }
        Err(_) => {
            gate.finish(Spent::Unknown);
            eprintln!("runbook run: conformance: not observed (the observation failed unexpectedly)");
        }
    }
}
