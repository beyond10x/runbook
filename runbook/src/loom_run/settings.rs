//! The run settings file (JSON object) and its redacted copy.

use serde_json::Value;
use std::path::Path;

/// The key that names the path of the login file. Its value never reaches the run directory.
pub const LOGIN_KEY: &str = "login_file";

/// Reads the settings file: a JSON object.
pub fn read(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("--settings {}: {e}", path.display()))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("--settings {}: not JSON: {e}", path.display()))?;
    if !v.is_object() {
        return Err(format!("--settings {}: not a JSON object", path.display()));
    }
    Ok(v)
}

/// The redacted settings with what the command was given added, as `RunStarted` carries them (the
/// members `sdc-experiment check --runs-dir` compares): `loom` (`sha256`, `confinement`,
/// `cap_micro_usd`), `max_llm_calls`, `max_wall_seconds`, `case`, `root_protocol`. The options
/// win over a member of the same name in the file. The path of the binary is not in them.
pub fn with_run(redacted: &Value, args: &crate::loom_run::Args, loom_sha256: &str) -> Value {
    let mut v = redacted.clone();
    if let Some(o) = v.as_object_mut() {
        o.insert("loom".to_owned(), serde_json::json!({"sha256": loom_sha256, "confinement": args.confinement.as_arg(), "cap_micro_usd": args.cap_micro_usd}));
        o.insert("max_llm_calls".to_owned(), Value::from(args.max_llm_calls));
        o.insert("max_wall_seconds".to_owned(), Value::from(args.max_wall_seconds));
        o.insert("case".to_owned(), Value::from(args.case.clone()));
        o.insert("root_protocol".to_owned(), Value::from(args.root_protocol.clone()));
    }
    v
}

/// The settings as written into the run directory: `login_file` replaced by the text `named`.
pub fn redacted(settings: &Value) -> Value {
    let mut v = settings.clone();
    if let Some(o) = v.as_object_mut() {
        if o.contains_key(LOGIN_KEY) {
            o.insert(LOGIN_KEY.to_owned(), Value::String("named".to_owned()));
        }
    }
    v
}
