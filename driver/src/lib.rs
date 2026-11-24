#![warn(missing_docs)]
//! The tools of Runbook as a library: the internals of the `sdc-*` programs that other programs
//! (`runbook`) call in process.

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
