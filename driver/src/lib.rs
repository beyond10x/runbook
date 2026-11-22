#![warn(missing_docs)]
//! The tools of Runbook as a library: the internals of the `sdc-*` programs that other programs
//! (`runbook`) call in process.

#[path = "instructions.rs"]
pub mod instructions;
pub mod reply;
