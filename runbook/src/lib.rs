//! The daemon of Runbook: `runbook serve`. The binary (`main.rs`) calls it; the tests start it in
//! process. The runtime behind it is a trait (`runtime`); `stub` is the runtime of the tests.

pub mod admission;
pub mod api;
pub mod budget;
pub mod caps;
pub mod client;
pub mod clock;
pub mod commands;
pub mod context_copy;
pub mod cost;
pub mod delegation;
pub mod engine;
pub mod handback;
pub mod ledger;
pub mod mcp;
pub mod pod_time;
pub mod record_write;
pub mod relay;
pub mod route_day;
pub mod routes;
pub mod run_metrics;
pub mod runbooks;
pub mod runtime;
pub mod serve;
pub mod state;
pub mod stub;
pub mod test_suite;
pub mod ui_embed;
