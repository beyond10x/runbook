//! The one integration-test binary of `sdc-drive`: one module per file of this directory. A new
//! test file is added here as a `mod` line; `autotests = false` in `Cargo.toml` keeps cargo from
//! building each file as a binary of its own.

mod answers;
mod bugfix_languages;
mod bugfix_prompt;
mod conformance;
mod decision_request;
mod experiment_loom;
mod impl_gate;
mod node_parity;
mod plugin;
mod progress;
mod questions_check;
mod reply_bytes;
mod run_compare;
mod run_metrics;
mod run_timeline;
mod unreferenced;
