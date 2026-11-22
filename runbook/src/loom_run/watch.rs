//! The `watch` port of `runbook run` (`--watch`).

use super::ports::{not_built, Ports, Refusal};

// port: watch
/// Refuses `--watch` until the port is built.
pub fn watch_not_built(ports: &Ports) -> Result<(), Refusal> {
    if ports.watch {
        return Err(not_built("--watch"));
    }
    Ok(())
}
