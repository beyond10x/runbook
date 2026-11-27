/// The principal every daemon of these tests acts as: `runbook serve --principal` and `Config::principal`.
pub const PRINCIPAL: &str = "agent:test-daemon";

/// The caps of the engines of these tests when a test is not about caps: far above anything a run uses here.
pub fn policy() -> runbook::caps::CapPolicy {
    runbook::caps::CapPolicy::fixed(runbook::caps::Caps { max_wall_seconds: 3600, max_llm_calls: 10_000, cap_micro_usd: 1_000_000_000 }).unwrap()
}

mod stub;
mod state;
mod commands;
mod openapi;
mod http;
mod sse;
mod clock;
mod conformance;
mod ui_embed;
mod ui_embed_review;
mod mcp;
mod cli_client;
mod engine;
mod engine_slow;
mod ui_guard;
mod ui_decide;
mod ui_decide_review;
mod principal;
mod runbooks_view;
mod ui_start;
mod security_start;
mod submit_order;
mod answer_order;
mod default_confinement;
mod default_level;
mod delegate;
mod delegate_change;
mod adversary_delegate_change;
mod plugin_session;
mod autonomy;
mod autonomy_guards;
mod gateway;
mod caps;
mod drain;
mod copy_integrity;
mod homes_leave_no_path;
mod decision_artifact;
mod decision_artifact_attack;
mod products;
mod cases;
mod model_routes;
mod model_routes_attack;
mod products_attack;
mod relay;
mod relay_daemon;
mod route_ready;
mod route_wake;
mod run_route;
mod catalog;
mod adversary_a;
mod adversary_b;
mod adversary_wave2;
mod protocol_routes;
mod adversary_wave3;
mod recovered_call;
mod failed_call;
mod run_metrics;
mod adversary_w4;
mod bound_step;
mod adversary_w5;
mod serial_key;
mod context_copy;
mod adversary_context_copy;
mod adversary_w6;
mod adversary_late_call;
mod adversary_wake_booking;
