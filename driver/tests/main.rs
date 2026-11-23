//! The one integration-test binary of `sdc-tools`: one module per file of this directory. A new
//! test file is added here as a `mod` line; `autotests = false` in `Cargo.toml` keeps cargo from
//! building each file as a binary of its own.

mod answers;
mod bugfix_languages;
mod bugfix_prompt;
mod check_bindings_exits;
mod conformance;
mod decision_contract_mutants;
mod decision_grounds;
mod decision_request;
mod experiment_loom;
mod finding_kpis;
mod impl_gate;
mod not_called_writes;
mod plugin;
mod producer_ids;
mod progress;
mod questions_check;
mod reply_bytes;
mod run_compare;
mod run_metrics;
mod run_timeline;
mod runtime_block;
mod sdc_node;
mod unreferenced;
mod workspace_empty_loom_case;
mod workspace_layout;
