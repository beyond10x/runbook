//! The daemon of Runbook: `runbook serve`. The binary (`main.rs`) calls it; the tests start it in
//! process. The runtime behind it is a trait (`runtime`); `stub` is the runtime of the tests.

pub mod admission;
pub mod api;
pub mod budget;
pub mod caps;
pub mod client;
pub mod clock;
pub mod commands;
pub mod cost;
pub mod delegation;
pub mod engine;
pub mod ledger;
pub mod mcp;
pub mod record_write;
pub mod relay;
pub mod routes;
pub mod runbooks;
pub mod runtime;
pub mod serve;
pub mod state;
pub mod stub;
pub mod test_suite;
pub mod ui_embed;
