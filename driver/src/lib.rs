#![warn(missing_docs)]
//! The tools of Runbook as a library: the internals of the `sdc-*` programs that other programs
//! (`runbook`) call in process.

/// Answers `<program> --version` when `--version` is the only argument: prints `<program> <version>` (the one
/// version of the repository) and ends the process with status 0. Every program of `driver/` that
/// `scripts/install.sh` installs calls it first, so a check of a release archive starts each one without its inputs.
pub fn answer_version(program: &str) {
    let mut args = std::env::args_os().skip(1);
    if let (Some(a), None) = (args.next(), args.next()) {
        if a == "--version" {
            println!("{program} {}", env!("CARGO_PKG_VERSION"));
            std::process::exit(0);
        }
    }
}

pub mod conform;
pub mod gateway;
#[path = "conformance.rs"]
#[allow(missing_docs, dead_code)]
pub mod conformance;
#[path = "instructions.rs"]
pub mod instructions;
pub mod named;
pub mod reply;
#[path = "run_metrics.rs"]
#[allow(missing_docs, dead_code)]
pub mod run_metrics;
