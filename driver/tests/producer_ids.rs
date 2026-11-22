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
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        found.extend(old_ids_in_structure(&text, ext).into_iter().map(|s| format!("{rel}{s}")));
    }
    assert!(all.len() > 20, "the scan reached {} files, expected the whole tree", all.len());
    assert!(found.is_empty(), "producer ids that name `{OLD}`:\n{}", found.join("\n"));
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

fn read_tree(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap()
}

/// The four mutants, each made on a real file of the tree: the unmutated file has no old id, the
/// mutated text has exactly the one the mutation put there.
#[test]
fn the_scan_catches_four_mutants_of_real_files() {
    let json_rel = "driver/examples/sdc/sdc-story.bindings.json";
    let yaml_rel = "bindings/sdc-story.yaml";
    let (json, yaml) = (read_tree(json_rel), read_tree(yaml_rel));
    assert!(old_ids_in_structure(&json, "json").is_empty() && old_ids_in_structure(&yaml, "yaml").is_empty());

    // 1. an `$independence` key in one JSON file
    let mut v: Value = serde_json::from_str(&json).unwrap();
    v["$independence"].as_object_mut().expect("the file has an $independence object").insert(format!("{OLD}.merge"), Value::Array(vec![]));
    let got = old_ids_in_structure(&serde_json::to_string_pretty(&v).unwrap(), "json");
    assert_eq!(got, [format!("/$independence: key {OLD}.merge")]);

    // 2. an `independence` key in one YAML file
    let mut v: Value = serde_yaml::from_str(&yaml).unwrap();
    v["independence"].as_object_mut().expect("the file has an independence object").insert(format!("{OLD}.merge"), Value::Array(vec![]));
    let got = old_ids_in_structure(&serde_yaml::to_string(&v).unwrap(), "yaml");
    assert_eq!(got, [format!("/independence: key {OLD}.merge")]);

    // 3. a `producer:` value in one YAML file, written as a line of the file's own text
    let mutated = yaml.replacen("producer: ess", &format!("producer: {OLD}"), 1);
    assert_ne!(mutated, yaml, "the file has a line `producer: ess`");
    let got = old_ids_in_structure(&mutated, "yaml");
    assert_eq!(got.len(), 1, "{got:?}");
    assert!(got[0].ends_with(&format!("/producer: \"{OLD}\"")), "{got:?}");

    // 4. a `producer` value split across two lines (a YAML double-quoted scalar, its line break escaped)
    let (head, tail) = OLD.split_at(4);
    let split = format!("{yaml}\nzz_mutant:\n  producer: \"{head}\\\n    {tail}\"\n");
    assert!(split.contains(&format!("{head}\\\n")) && !split.contains(OLD), "the id stands on two lines");
    let got = old_ids_in_structure(&split, "yaml");
    assert_eq!(got, [format!("/zz_mutant/producer: \"{OLD}\"")]);
}
