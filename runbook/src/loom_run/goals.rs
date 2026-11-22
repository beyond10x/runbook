//! The `goals` port of `runbook run` (`--goals`).

use super::ports::{not_built, Ports, Refusal};

// port: goals
/// Refuses `--goals` until the port is built.
pub fn goals_not_built(ports: &Ports) -> Result<(), Refusal> {
    if ports.goals.is_some() {
        return Err(not_built("--goals"));
    }
    Ok(())
}
