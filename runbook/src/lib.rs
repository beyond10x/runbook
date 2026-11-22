//! The daemon of Runbook: `runbook serve`. The binary (`main.rs`) calls it; the tests start it in
//! process. The runtime behind it is a trait (`runtime`); `stub` is the runtime of the tests.

pub mod api;
pub mod client;
pub mod clock;
pub mod commands;
pub mod engine;
pub mod mcp;
pub mod runtime;
pub mod serve;
pub mod state;
pub mod stub;
