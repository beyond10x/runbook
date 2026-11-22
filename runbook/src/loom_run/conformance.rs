//! The `conformance` port of `runbook run` (`--conformance observe`).

use super::ports::{not_built, Conformance, Ports, Refusal};

// port: conformance
/// Refuses `--conformance observe` until the port is built; `off` is the default and changes nothing.
pub fn conformance_not_built(ports: &Ports) -> Result<(), Refusal> {
    if ports.conformance == Conformance::Observe {
        return Err(not_built("--conformance observe"));
    }
    Ok(())
}
