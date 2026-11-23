//! The `goals` port of `runbook run` (`--goals`): the requester's goals document is copied, before
//! the tree opens, to `work/goals.yaml` of the run (what the removed driver did for the root
//! case). With `decider.requester` in the settings, the decider answers
//! the decisions of the requester-side classes from that file (`decider/requester.rs`).

use super::ports::{Ports, Refusal, USAGE};
use serde_json::Value;
use std::path::Path;

/// The name of the copy in the workspace.
pub const FILE: &str = "goals.yaml";

// port: goals
/// Refuses `--goals` when it names no file (exit 64, as the old launcher did). The name is the one
/// `refuse_unbuilt` calls; the port is built.
pub fn goals_not_built(ports: &Ports) -> Result<(), Refusal> {
    match &ports.goals {
        Some(path) if !path.is_file() => Err((USAGE, format!("--goals needs a goals file, not '{}'", path.display()))),
        _ => Ok(()),
    }
}

/// Refuses settings that ask for the requester decider without goals: it answers from the goals
/// document and there is none. Checked before anything is written.
pub fn check_settings(ports: &Ports, settings: &Value) -> Result<(), String> {
    let wants = settings.get("decider").and_then(|d| d.get("requester")) == Some(&Value::Bool(true));
    if wants && ports.goals.is_none() {
        return Err("decider.requester: the requester answers from the goals document; give it with --goals".to_owned());
    }
    Ok(())
}

/// Copies the goals document unchanged to `<workspace>/goals.yaml`.
pub fn copy(source: &Path, workspace: &Path) -> Result<(), String> {
    std::fs::copy(source, workspace.join(FILE)).map(|_| ()).map_err(|e| format!("--goals {}: {}", source.display(), e.kind()))
}
