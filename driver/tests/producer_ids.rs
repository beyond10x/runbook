//! No producer id of a binding, a protocol, an example or a view names the deleted runtime: the
//! runner is `runbook run`, so its evidence has the producer `runbook` (or `runbook.<x>`). A
//! producer id stands as the value of `producer` and as a key of `independence` (`$independence`
//! in the JSON files). The text is read from the tree; no program is started. The scan covers the
//! `json`, `yaml`, `yml` and `jsonl` files of five directories (see the test); a shell script is
//! not scanned, and a covered file that does not parse fails the test.

use serde::Deserialize as _;
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

/// The producer ids that name `OLD` in a parsed text: as a `producer` value or as a key of
/// `independence` / `$independence`. `json` is one document, `yaml` and `yml` hold any number of
/// documents (a later one is named `#<n>` in the path), `jsonl` is one document per non-empty
/// line (named `:<line>`). A text that does not parse is an error; any other extension is not read.
fn old_ids_in_structure(text: &str, ext: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    match ext {
        "json" => walk(&serde_json::from_str(text).map_err(|e| e.to_string())?, "", &mut out),
        "yaml" | "yml" => {
            for (i, doc) in serde_yaml::Deserializer::from_str(text).enumerate() {
                let v = Value::deserialize(doc).map_err(|e| e.to_string())?;
                walk(&v, &if i == 0 { String::new() } else { format!("#{}", i + 1) }, &mut out);
            }
        }
        "jsonl" => {
            for (i, l) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
                let v: Value = serde_json::from_str(l).map_err(|e| format!("line {}: {e}", i + 1))?;
                walk(&v, &format!(":{}", i + 1), &mut out);
            }
        }
        _ => {}
    }
    Ok(out)
}

/// What the scan finds in one file: its old ids, each named with the file, or the one line that
/// says the file does not parse.
fn scan_file(rel: &str, text: &str, ext: &str) -> Vec<String> {
    match old_ids_in_structure(text, ext) {
        Ok(ids) => ids.into_iter().map(|s| format!("{rel}{s}")).collect(),
        Err(e) => vec![format!("{rel}: does not parse as {ext}: {e}")],
    }
}

/// Scans the files of `bindings`, `protocol`, `driver/examples`, `views` and `website/data` with
/// the extension `json`, `yaml`, `yml` or `jsonl` (every document of a YAML file, every line of a
/// `jsonl` file). A scanned file that does not parse fails the test and is named. Files of any
/// other type are not scanned, shell scripts (`bindings/tools/*.sh`) among them.
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
        found.extend(scan_file(&rel, &text, ext));
    }
    assert!(all.len() > 20, "the scan reached {} files, expected the whole tree", all.len());
    assert!(found.is_empty(), "producer ids that name `{OLD}`:\n{}", found.join("\n"));
}

#[test]
fn the_scan_sees_a_producer_id_that_stands_as_a_key() {
    let json = r#"{"$independence": {"sdc-drive": ["different_principal"]}, "a": {"producer": "x"}}"#;
    assert_eq!(ids(json, "json"), ["/$independence: key sdc-drive"]);
    let yaml = "independence:\n  sdc-drive.merge: [different_principal]\nactions: []\n";
    assert_eq!(ids(yaml, "yaml"), ["/independence: key sdc-drive.merge"]);
    let clean = r#"{"$independence": {"llm-review": []}, "a": [{"producer": "runbook"}]}"#;
    assert!(ids(clean, "json").is_empty());
    assert!(old_ids_in_structure("{not json", "json").is_err());
}

fn ids(text: &str, ext: &str) -> Vec<String> {
    old_ids_in_structure(text, ext).unwrap()
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
    assert!(ids(&json, "json").is_empty() && ids(&yaml, "yaml").is_empty());

    // 1. an `$independence` key in one JSON file
    let mut v: Value = serde_json::from_str(&json).unwrap();
    v["$independence"].as_object_mut().expect("the file has an $independence object").insert(format!("{OLD}.merge"), Value::Array(vec![]));
    let got = ids(&serde_json::to_string_pretty(&v).unwrap(), "json");
    assert_eq!(got, [format!("/$independence: key {OLD}.merge")]);

    // 2. an `independence` key in one YAML file
    let mut v: Value = serde_yaml::from_str(&yaml).unwrap();
    v["independence"].as_object_mut().expect("the file has an independence object").insert(format!("{OLD}.merge"), Value::Array(vec![]));
    let got = ids(&serde_yaml::to_string(&v).unwrap(), "yaml");
    assert_eq!(got, [format!("/independence: key {OLD}.merge")]);

    // 3. a `producer:` value in one YAML file, written as a line of the file's own text
    let mutated = yaml.replacen("producer: ess", &format!("producer: {OLD}"), 1);
    assert_ne!(mutated, yaml, "the file has a line `producer: ess`");
    let got = ids(&mutated, "yaml");
    assert_eq!(got.len(), 1, "{got:?}");
    assert!(got[0].ends_with(&format!("/producer: \"{OLD}\"")), "{got:?}");

    // 4. a `producer` value split across two lines (a YAML double-quoted scalar, its line break escaped)
    let (head, tail) = OLD.split_at(4);
    let split = format!("{yaml}\nzz_mutant:\n  producer: \"{head}\\\n    {tail}\"\n");
    assert!(split.contains(&format!("{head}\\\n")) && !split.contains(OLD), "the id stands on two lines");
    let got = ids(&split, "yaml");
    assert_eq!(got, [format!("/zz_mutant/producer: \"{OLD}\"")]);
}

/// Three more mutants, made on real files: an unparsable YAML file, a second YAML document that
/// holds the old id, and a `jsonl` line that holds it. Each is caught, and the first is named.
#[test]
fn the_scan_catches_an_unparsable_file_a_second_document_and_a_jsonl_line() {
    let yaml_rel = "bindings/sdc-story.yaml";
    let yaml = read_tree(yaml_rel);
    assert!(scan_file(yaml_rel, &yaml, "yaml").is_empty());

    // 1. an unparsable YAML file: an unclosed flow sequence after the real text
    let broken = format!("{yaml}\nzz_mutant: [unclosed\n");
    let got = scan_file(yaml_rel, &broken, "yaml");
    assert_eq!(got.len(), 1, "{got:?}");
    assert!(got[0].starts_with(&format!("{yaml_rel}: does not parse as yaml")), "{got:?}");

    // 2. a second YAML document with the old id
    let second = format!("{yaml}\n---\nactions:\n  - producer: {OLD}\n");
    let got = scan_file(yaml_rel, &second, "yaml");
    assert_eq!(got, [format!("{yaml_rel}#2/actions/0/producer: \"{OLD}\"")]);

    // 3. a `jsonl` line with the old id, after a line of a real jsonl file
    let (rel, text) = find_jsonl(&root().join("driver/examples")).expect("driver/examples holds a jsonl file");
    assert!(scan_file(&rel, &text, "jsonl").is_empty(), "{rel} has an old id already");
    let mutated = format!("{text}\n{{\"event\": {{\"producer\": \"{OLD}\"}}}}\n");
    let line = mutated.lines().count();
    let got = scan_file(&rel, &mutated, "jsonl");
    assert_eq!(got, [format!("{rel}:{line}/event/producer: \"{OLD}\"")]);
    let broken = format!("{text}\n{{not json\n");
    assert!(scan_file(&rel, &broken, "jsonl")[0].contains("does not parse as jsonl: line"), "a bad line is named");
}

/// The first `jsonl` file under `dir` (sorted), with its path from the root and its text.
fn find_jsonl(dir: &Path) -> Option<(String, String)> {
    let mut all = Vec::new();
    files(dir, &mut all);
    all.sort();
    let p = all.into_iter().find(|p| p.extension().is_some_and(|e| e == "jsonl"))?;
    let text = fs::read_to_string(&p).ok()?;
    Some((p.strip_prefix(root()).unwrap().display().to_string(), text))
}
