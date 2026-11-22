//! The run settings file (JSON object) and its redacted copy.

use serde_json::Value;
use std::path::Path;

/// The key that names the path of the login file. Its value never reaches the run directory.
pub const LOGIN_KEY: &str = "login_file";

/// Reads the settings file: a JSON object.
pub fn read(_path: &Path) -> Result<Value, String> {
    Err(crate::loom_run::not_built("settings read"))
}

/// The settings as written into the run directory: `login_file` replaced by the text `named`.
pub fn redacted(_settings: &Value) -> Value {
    Value::Null
}
