//! The daemon of Runbook: `runbook serve`. The binary (`main.rs`) calls it; the tests start it in
//! process. The runtime behind it is a trait (`runtime`); `stub` is the runtime of the tests.

pub mod runtime;
pub mod state;
pub mod stub;
