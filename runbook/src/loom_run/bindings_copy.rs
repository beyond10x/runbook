//! R2 fills this file: the copy of the bindings with the run's principals, models, budgets and
//! login file from the settings. The committed `bindings/*.yaml` are never edited.

use serde_json::Value;
use std::path::{Path, PathBuf};

/// Copies the bindings directory `src` into `dst` (which does not exist yet) and applies
/// `settings` to the copy; returns the root bindings file `dst/<root_protocol_name>.yaml` that
/// `tree open --bindings` is given. R1's version copies the directory unchanged. The copy must
/// not hold the login file's content or its path under a home directory (a relative path to a
/// link inside `dst` is allowed). `settings` carries the login file's real path.
pub fn materialise(src: &Path, dst: &Path, root_protocol_name: &str, _settings: &Value) -> Result<PathBuf, String> {
    copy_dir(src, dst)?;
    Ok(dst.join(format!("{root_protocol_name}.yaml")))
}

fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    let entries = std::fs::read_dir(src).map_err(|e| format!("{}: {e}", src.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", src.display()))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let kind = entry.file_type().map_err(|e| format!("{}: {e}", from.display()))?;
        if kind.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
        }
    }
    Ok(())
}
