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
fn setup_of(args: &Args, run_dir: &Path) -> Setup {
    let lookup = crate::assets::Lookup::from_process();
    let assets = lookup.locate().ok();
    let instructions = assets.as_ref().map(|a| a.root.clone()).or_else(|| args.bindings.parent().map(Path::to_path_buf));
    Setup {
        work: Some("work".into()),
        ess: Some(EssTool { tool: args.bindings.join("tools/sdc-tool.sh"), cwd: run_dir.join("conformance/ess-cwd") }),
        tools: Some(args.bindings.join("tools")),
        instructions,
        adapter: lookup.helper("sdc-conform-adapter", assets.as_ref()),
    }
}

/// Observes the product of a run that ended, when `--conformance observe` was given. The line it
/// prints says what was written or why nothing was; a failure is the line or `oracle.json`'s
/// `unknown` with its cause, never a change of the run's outcome.
pub fn after_close(args: &Args, run_dir: &Path, outcome: &Outcome) {
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
    let setup = setup_of(args, run_dir);
    let pin = Path::new("driver/ess-pin.json");
    let seen = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| conform::observe(run_dir, None, tables, pin, &setup)));
    match seen {
        Ok(Ok(doc)) => eprintln!("runbook run: conformance: {}", doc["conformance"]["summary"].as_str().unwrap_or("")),
        Ok(Err(e)) => eprintln!("runbook run: conformance: not observed ({e})"),
        Err(_) => eprintln!("runbook run: conformance: not observed (the observation failed unexpectedly)"),
    }
}
