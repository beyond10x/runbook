//! The `replay` port of `runbook run` (`--recorded` and the options that go with it).

use super::ports::{not_built, Ports, Refusal, USAGE};

// port: replay
/// Refuses `--recorded` until the port is built; an option of the replay without `--recorded` is
/// refused for that, as the old launcher did.
pub fn replay_not_built(ports: &Ports) -> Result<(), Refusal> {
    let replay = &ports.replay;
    if replay.recorded.is_none() {
        return match replay.first_without_recorded() {
            Some(option) => Err((USAGE, format!("{option} needs --recorded <source>"))),
            None => Ok(()),
        };
    }
    Err(not_built("--recorded"))
}
