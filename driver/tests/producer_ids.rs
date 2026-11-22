//! No producer id of a binding, a protocol, an example or a view names the deleted runtime: the
//! runner is `runbook run`, so its evidence has the producer `runbook` (or `runbook.<x>`). A
//! producer id stands as the value of `producer` and as a key of `independence` (`$independence`
//! in the JSON files). The text is read from the tree; no program is started.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const OLD: &str = "sdc-drive";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn names_old_producer(line: &str) -> bool {
    let Some(at) = line.find("producer") else { return false };
    line[at..].contains(OLD)
}

fn walk(v: &Value, path: &str, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                let here = format!("{path}/{k}");
                if matches!(k.as_str(), "independence" | "$independence") {
                    if let Value::Object(ids) = x {
                        out.extend(ids.keys().filter(|id| id.contains(OLD)).map(|id| format!("{here}: key {id}")));
                    }
                }
                if k == "producer" && x.as_str().is_some_and(|s| s.contains(OLD)) {
                    out.push(format!("{here}: {x}"));
                }
                walk(x, &here, out);
            }
        }
        Value::Array(a) => a.iter().enumerate().for_each(|(i, x)| walk(x, &format!("{path}/{i}"), out)),
        _ => {}
    }
}

/// The producer ids that name `OLD` in a parsed JSON or YAML text: as a `producer` value or as a
/// key of `independence` / `$independence`. A text that does not parse has none.
fn old_ids_in_structure(text: &str, ext: &str) -> Vec<String> {
    let doc: Option<Value> = match ext {
        "json" => serde_json::from_str(text).ok(),
        "yaml" | "yml" => serde_yaml::from_str(text).ok(),
        _ => None,
    };
    let mut out = Vec::new();
    if let Some(d) = doc {
        walk(&d, "", &mut out);
    }
    out
}

#[test]
fn no_producer_id_names_the_deleted_runtime() {
    let r = root();
    let mut all = Vec::new();
    for d in ["bindings", "protocol", "driver/examples", "views", "website/data"] {
        files(&r.join(d), &mut all);
    }
    all.sort();
    let mut found = Vec::new();
    for p in &all {
        let Ok(text) = fs::read_to_string(p) else { continue };
        let rel = p.strip_prefix(&r).unwrap().display().to_string();
        for (i, l) in text.lines().enumerate() {
            if names_old_producer(l) {
                found.push(format!("{rel}:{}: {}", i + 1, l.trim()));
            }
        }
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        found.extend(old_ids_in_structure(&text, ext).into_iter().map(|s| format!("{rel}{s}")));
    }
    assert!(all.len() > 20, "the scan reached {} files, expected the whole tree", all.len());
    assert!(found.is_empty(), "producer ids that name `{OLD}`:\n{}", found.join("\n"));
}

#[test]
fn the_scan_sees_a_producer_line() {
    assert!(names_old_producer("    \"producer\": \"sdc-drive.merge\","));
    assert!(names_old_producer("        producer: sdc-drive"));
    assert!(!names_old_producer("        producer: runbook.merge"));
    assert!(!names_old_producer("  \"note\": \"the old sdc-drive\""));
}

#[test]
fn the_scan_sees_a_producer_id_that_stands_as_a_key() {
    let json = r#"{"$independence": {"sdc-drive": ["different_principal"]}, "a": {"producer": "x"}}"#;
    assert_eq!(old_ids_in_structure(json, "json"), ["/$independence: key sdc-drive"]);
    let yaml = "independence:\n  sdc-drive.merge: [different_principal]\nactions: []\n";
    assert_eq!(old_ids_in_structure(yaml, "yaml"), ["/independence: key sdc-drive.merge"]);
    let clean = r#"{"$independence": {"llm-review": []}, "a": [{"producer": "runbook"}]}"#;
    assert!(old_ids_in_structure(clean, "json").is_empty());
    assert!(old_ids_in_structure("{not json", "json").is_empty());
}
