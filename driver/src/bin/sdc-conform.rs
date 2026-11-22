//! sdc-conform: the conformance observation of one ended run (docs/conformance.md).
//!
//! ```text
//! sdc-conform observe <run dir> [--tables <n>] [--table <table.json>] [--pin <ess-pin.json>] [--tools <dir>]
//! sdc-conform recompute <run dir> [--pin <ess-pin.json>]
//! sdc-conform summary <run dir>
//! sdc-conform check-table --ir <ir.json> --table <table.json> --tree <delivered tree>
//! sdc-conform ess-path [--pin <ess-pin.json>]
//! sdc-conform ess-install [--archive <ess-….tar.gz>] [--pin <ess-pin.json>]
//! sdc-conform prompt [--attempt <n>] [--refused <why>]... [--run-fault <why>]...
//! ```
//!
//! The code is the library's `sdc_tools::conform` (`driver/src/conform.rs`), which `runbook run
//! --conformance observe` calls in process; this program is its command line and nothing else.

use std::process::ExitCode;

fn main() -> ExitCode {
    sdc_tools::conform::cli()
}
