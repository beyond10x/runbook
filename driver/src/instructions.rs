//! The model's instruction files, read at run time (docs/driver.md, "Instructions"). One renderer
//! for the Rust binaries that tell a model something (`sdc-reply implement-run`, `sdc-conform`); each includes this file with `#[path = "../instructions.rs"] mod instructions;`.
//!
//! - A path is `instructions/<...>.md`, relative, with no `..` segment: anything else is refused.
//! - The files are found as the shell tools find them: the nearest directory above the running
//!   binary (`driver/target/<profile>/<bin>`) that holds `instructions/` is the repository root.
//! - A placeholder is `{{name}}` (lower case and `_`). Every placeholder is replaced in one pass: a
//!   value is never searched again for placeholders. A placeholder with no value is an error.
//! - The trailing newlines of the file are dropped.

use std::fs;
use std::path::{Path, PathBuf};

/// The repository root: the one `SDC_INSTRUCTIONS_ROOT` names (`sdc-node` sets it from its tools
/// directory), else the nearest directory above the running binary that holds `instructions/`.
pub fn root() -> Result<PathBuf, String> {
    if let Some(r) = std::env::var_os("SDC_INSTRUCTIONS_ROOT").filter(|r| !r.is_empty()) {
        return Ok(PathBuf::from(r));
    }
    let exe = std::env::current_exe().map_err(|e| format!("the instruction files were not found: {e}"))?;
    exe.ancestors()
        .skip(1)
        .find(|d| d.join("instructions").is_dir())
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("no instructions/ directory above {}", exe.display()))
}

/// Whether `rel` is `instructions/<...>.md`, relative, without a `..` segment.
pub fn check_path(rel: &str) -> Result<(), String> {
    if rel.starts_with("instructions/") && rel.ends_with(".md") && !rel.starts_with('/') && !rel.split('/').any(|s| s == "..") {
        Ok(())
    } else {
        Err(format!("the instruction path {rel} is not instructions/<...>.md without a .. segment"))
    }
}

/// The file `rel` under `root` with every `{{name}}` replaced by its value.
pub fn render_in(root: &Path, rel: &str, values: &[(&str, &str)]) -> Result<String, String> {
    check_path(rel)?;
    let text = fs::read_to_string(root.join(rel)).map_err(|_| format!("the instruction file {rel} is missing"))?;
    let text = text.trim_end_matches('\n');
    let placeholder = regex::Regex::new(r"\{\{([a-z_]+)\}\}").map_err(|e| e.to_string())?;
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for m in placeholder.captures_iter(text) {
        let whole = m.get(0).ok_or("no match")?;
        let name = &m[1];
        let (_, value) = values.iter().rev().find(|(k, _)| *k == name).ok_or_else(|| format!("{rel} names {{{{{name}}}}} and no value was given"))?;
        out.push_str(&text[at..whole.start()]);
        out.push_str(value);
        at = whole.end();
    }
    out.push_str(&text[at..]);
    Ok(out)
}

/// The file `rel` of the repository this binary belongs to, filled.
pub fn render(rel: &str, values: &[(&str, &str)]) -> Result<String, String> {
    check_path(rel)?;
    render_in(&root()?, rel, values)
}
