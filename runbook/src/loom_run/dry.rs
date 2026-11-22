//! The `dry` port of `runbook run` (`--dry`).

use super::ports::{not_built, Ports, Refusal};

// port: dry
/// Refuses `--dry` until the port is built.
pub fn dry_not_built(ports: &Ports) -> Result<(), Refusal> {
    if ports.dry {
        return Err(not_built("--dry"));
    }
    Ok(())
}
