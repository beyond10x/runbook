//! The `bugfix` port of `runbook run` (`--bugfix`, `--bugfix-ticket`).

use super::ports::{not_built, Ports, Refusal};

// port: bugfix
/// Refuses `--bugfix` and `--bugfix-ticket` until the port is built.
pub fn bugfix_not_built(ports: &Ports) -> Result<(), Refusal> {
    if ports.bugfix.seed.is_some() {
        return Err(not_built("--bugfix"));
    }
    if ports.bugfix.ticket.is_some() {
        return Err(not_built("--bugfix-ticket"));
    }
    Ok(())
}
