//! R2 fills this file: the real deciders (a rule where the settings say so, a model call through
//! `bindings/tools/decider.sh`) and the choice of the decider from the settings.

use crate::loom_run::decide::{Decider, RefusingDecider};
use serde_json::Value;

/// Chooses the decider from the run settings. Absent `decider` key: the refusing default. R1's
/// version knows no kind; R2 adds the real ones and keeps `"scripted"` (`ScriptedDecider::from_value`).
pub fn from_settings(settings: &Value) -> Result<Box<dyn Decider>, String> {
    match settings.get("decider") {
        None => Ok(Box::new(RefusingDecider)),
        Some(_) => Err(crate::loom_run::not_built("decider from settings")),
    }
}
