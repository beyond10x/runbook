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
