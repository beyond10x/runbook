//! `runbook list`: the protocols under the asset root's `protocol/`.

use serde_json::{json, Value};
use std::path::Path;

/// The runbooks a person can start; every other protocol is a child case of one of them.
pub const ROOT_RUNBOOKS: [&str; 2] = ["sdc", "sdc-bugfix"];

const TITLE_MAX: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The file stem: `sdc`, `sdc-bugfix`, `sdc-story`.
    pub id: String,
    /// The id the document carries (`protocol.id`, for example `sdc.bugfix`).
    pub protocol_id: String,
    pub revision: Option<u64>,
    pub startable: bool,
    pub title: String,
}

pub fn read(protocol_dir: &Path) -> Result<Vec<Entry>, String> {
    let rd = std::fs::read_dir(protocol_dir).map_err(|e| format!("{}: {e}", protocol_dir.display()))?;
    let mut out = Vec::new();
    for ent in rd {
        let ent = ent.map_err(|e| e.to_string())?;
        let name = ent.file_name().to_string_lossy().into_owned();
        let Some(id) = name.strip_suffix(".protocol.yaml") else { continue };
        if !ent.path().is_file() {
            continue;
        }
        let text = std::fs::read_to_string(ent.path()).map_err(|e| format!("{}: {e}", ent.path().display()))?;
        let doc: serde_yaml::Value = serde_yaml::from_str(&text).map_err(|e| format!("{}: {e}", ent.path().display()))?;
        let p = &doc["protocol"];
        out.push(Entry {
            id: id.to_string(),
            protocol_id: p["id"].as_str().unwrap_or(id).to_string(),
            revision: p["revision"].as_u64(),
            startable: ROOT_RUNBOOKS.contains(&id),
            title: title_of(&doc),
        });
    }
    out.sort_by(|a, b| b.startable.cmp(&a.startable).then_with(|| a.id.cmp(&b.id)));
    Ok(out)
}

/// `protocol.title` or `title` when the file carries one, else the first sentence of
/// `protocol.description` (the files carry only that), cut at a word near `TITLE_MAX`.
fn title_of(doc: &serde_yaml::Value) -> String {
    let p = &doc["protocol"];
    if let Some(t) = p["title"].as_str().or_else(|| doc["title"].as_str()) {
        return t.trim().to_string();
    }
    let d = p["description"].as_str().unwrap_or("");
    let line = d.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let first = match line.find(". ") {
        Some(i) => &line[..i],
        None => line.trim_end_matches('.'),
    };
    if first.chars().count() <= TITLE_MAX {
        return first.to_string();
    }
    let cut: String = first.chars().take(TITLE_MAX).collect();
    let cut = cut.rsplit_once(' ').map(|(a, _)| a).unwrap_or(&cut);
    format!("{}...", cut.trim_end_matches([',', ':', ';']))
}

pub fn text(entries: &[Entry]) -> String {
    let w = entries.iter().map(|e| e.id.len()).max().unwrap_or(0);
    let mut s = String::new();
    for e in entries {
        let kind = if e.startable { "runbook" } else { "child" };
        s.push_str(&format!("{:<w$}  {:<7}  {}\n", e.id, kind, e.title));
    }
    s
}

pub fn json(entries: &[Entry]) -> Value {
    Value::Array(
        entries
            .iter()
            .map(|e| {
                json!({
                    "id": e.id,
                    "protocol_id": e.protocol_id,
                    "revision": e.revision,
                    "kind": if e.startable { "runbook" } else { "child" },
                    "startable": e.startable,
                    "title": e.title,
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::testdir::Dir;

    #[test]
    fn title_is_the_first_sentence_and_short() {
        let doc: serde_yaml::Value = serde_yaml::from_str("protocol:\n  id: a\n  description: >-\n    Fix the thing (docs/x.md steps 4 to 6). More text.\n").unwrap();
        assert_eq!(title_of(&doc), "Fix the thing (docs/x.md steps 4 to 6)");
        let long = format!("protocol:\n  description: {}\n", "word ".repeat(60));
        let doc: serde_yaml::Value = serde_yaml::from_str(&long).unwrap();
        let t = title_of(&doc);
        assert!(t.ends_with("...") && t.chars().count() <= TITLE_MAX + 3, "{t}");
    }

    #[test]
    fn a_title_key_wins_and_other_files_are_ignored() {
        let t = Dir::new("list");
        let d = t.dir("protocol");
        std::fs::write(d.join("sdc.protocol.yaml"), "protocol:\n  id: sdc\n  revision: 3\n  title: The cycle\n  description: Long.\n").unwrap();
        std::fs::write(d.join("sdc-story.protocol.yaml"), "protocol:\n  id: sdc.story\n  description: Deliver one story. More.\n").unwrap();
        std::fs::write(d.join("README.md"), "x").unwrap();
        std::fs::create_dir_all(d.join("draft")).unwrap();
        std::fs::write(d.join("draft/orchestrate.protocol.yaml"), "protocol:\n  id: o\n").unwrap();
        let e = read(&d).unwrap();
        assert_eq!(e.len(), 2);
        assert_eq!((e[0].id.as_str(), e[0].startable, e[0].title.as_str(), e[0].revision), ("sdc", true, "The cycle", Some(3)));
        assert_eq!((e[1].id.as_str(), e[1].startable, e[1].title.as_str()), ("sdc-story", false, "Deliver one story"));
    }
}
