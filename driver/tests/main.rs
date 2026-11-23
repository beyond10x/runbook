//! The one integration-test binary of `sdc-tools`: one module per file of this directory. A new
//! test file is added here as a `mod` line; `autotests = false` in `Cargo.toml` keeps cargo from
//! building each file as a binary of its own.

mod answers;
mod bugfix_languages;
mod bugfix_prompt;
mod check_bindings_exits;
mod check_bindings_exits_inputs;
mod check_bindings_exits_links_and_shapes;
mod check_bindings_exits_roots;
mod check_bindings_exits_unwritable_yaml_json_copies_and_protocol_ids;
mod check_bindings_exits_value_table;
mod check_bindings_exits_values_and_registrations;
mod conformance;
mod decision_contract_mutants;
mod decision_grounds;
mod decision_request;
mod capture_stream;
mod ess_pin_guard;
mod experiment_loom;
mod finding_kpis;
mod impl_gate;
mod not_called_writes;
mod plugin;
mod producer_ids;
mod progress;
mod questions_check;
mod reply_bytes;
mod capture_buffer_bound;
mod bounds_and_digest;
mod capture_fate;
mod run_compare;
mod run_metrics;
mod run_timeline;
mod runtime_block;
mod sdc_node;
mod settings_record_order;
mod settings_record_text;
mod unreferenced;
mod workspace_empty_loom_case;
mod workspace_layout;
