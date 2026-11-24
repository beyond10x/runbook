//! Where a runbook's text is, under the runbook home: `protocol/<name>.protocol.yaml` and
//! `bindings/<name>.yaml` (the layout `runbook check` and `runbook doctor` know).

use crate::runtime::RunbookInfo;
use std::path::{Path, PathBuf};

/// A runbook found under the home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// `<home>/protocol`: every protocol of the runbook, the root's and its children's.
    pub protocol_dir: PathBuf,
    /// `<home>/bindings/<name>.yaml`: the root's bindings file, which names the children's.
    pub bindings: PathBuf,
    /// The id the root protocol declares (`sdc`, `sdc.bugfix`).
    pub protocol_id: String,
    pub revision: i64,
}

/// A name that is one path segment: no separator, no leading dot.
pub fn plain(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// The runbook `name` (at `version`, when one is asked for: the root protocol's revision) under
/// `home`, or why it is not there.
pub fn resolve(home: &Path, name: &str, version: Option<&str>) -> Result<Resolved, String> {
    if !plain(name) {
        return Err(format!("{name:?} is not a runbook name"));
    }
    let protocol_dir = home.join("protocol");
    let file = protocol_dir.join(format!("{name}.protocol.yaml"));
    let bindings = home.join("bindings").join(format!("{name}.yaml"));
    if !file.is_file() {
        return Err(format!("the runbook home holds no protocol/{name}.protocol.yaml"));
    }
    if !bindings.is_file() {
        return Err(format!("the runbook home holds no bindings/{name}.yaml"));
    }
    let text = std::fs::read_to_string(&file).map_err(|e| format!("protocol/{name}.protocol.yaml: {e}"))?;
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).map_err(|e| format!("protocol/{name}.protocol.yaml: {e}"))?;
    let protocol_id = doc["protocol"]["id"].as_str().ok_or_else(|| format!("protocol/{name}.protocol.yaml: no protocol.id"))?.to_string();
    let revision = doc["protocol"]["revision"].as_i64().ok_or_else(|| format!("protocol/{name}.protocol.yaml: no protocol.revision"))?;
    if let Some(v) = version {
        if v != revision.to_string() {
            return Err(format!("the runbook {name} is at revision {revision}, not {v}"));
        }
    }
    Ok(Resolved { protocol_dir, bindings, protocol_id, revision })
}

/// The runbook `name` in the first of `homes` that has its protocol file (the order of
/// `runbooks::load`: the configured directories, then the built-in home), or why the last one has not.
pub fn resolve_in(homes: &[PathBuf], name: &str, version: Option<&str>) -> Result<Resolved, String> {
    if !plain(name) {
        return Err(format!("{name:?} is not a runbook name"));
    }
    let home = crate::runbooks::locate(homes, name).or_else(|| homes.last().cloned()).ok_or_else(|| "no runbook home".to_owned())?;
    resolve(&home, name, version)
}

/// The runbooks of every home, each name once, the first home's row winning.
pub fn list_in(homes: &[PathBuf]) -> Vec<RunbookInfo> {
    let mut seen = std::collections::BTreeSet::new();
    homes.iter().flat_map(|h| list(h)).filter(|r| seen.insert(r.name.clone())).collect()
}

/// The runbooks under `home`: every `protocol/<name>.protocol.yaml` that `resolve` accepts, with the
/// revision of its root protocol as the version. A file that does not resolve is not listed.
pub fn list(home: &Path) -> Vec<RunbookInfo> {
    let Ok(entries) = std::fs::read_dir(home.join("protocol")) else { return Vec::new() };
    entries
        .flatten()
        .filter_map(|e| {
            let file = e.file_name().into_string().ok()?;
            let name = file.strip_suffix(".protocol.yaml")?;
            let found = resolve(home, name, None).ok()?;
            Some(RunbookInfo { name: name.to_string(), version: Some(found.revision.to_string()) })
        })
        .collect()
}

/// The path, in the workspace, the root bindings place the artifact `idea` at. A failure names the
/// file as `bindings/<file>`, never the path it was read at.
pub fn idea_path(bindings: &Path) -> Result<Option<String>, String> {
    let named = format!("bindings/{}", bindings.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()));
    let text = std::fs::read_to_string(bindings).map_err(|e| format!("{named}: {e}"))?;
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).map_err(|e| format!("{named}: {e}"))?;
    Ok(doc["artifacts"]["idea"].as_str().map(str::to_string))
}

/// The idea document the intake text becomes: a mapping that already has `text` is kept as given,
/// any other text becomes `text: <the text>`.
pub fn idea_document(intake: &str) -> String {
    if let Ok(serde_yaml::Value::Mapping(m)) = serde_yaml::from_str::<serde_yaml::Value>(intake) {
        if m.contains_key("text") {
            return if intake.ends_with('\n') { intake.to_string() } else { format!("{intake}\n") };
        }
    }
    let mut m = serde_yaml::Mapping::new();
    m.insert("text".into(), intake.into());
    serde_yaml::to_string(&serde_yaml::Value::Mapping(m)).unwrap_or_else(|_| format!("text: {intake:?}\n"))
}
