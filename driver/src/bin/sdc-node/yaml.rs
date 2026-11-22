//! The reads the script does with `yq`, done on a parsed document.

use serde_yaml::Value;
use std::fs;
use std::path::Path;

/// The document in the file; `None` when the file is absent or not YAML.
pub fn load(path: &Path) -> Option<Value> {
    serde_yaml::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// `yq -r` of a scalar: a string as it is, `null` for nothing, numbers and booleans as text; a
/// collection as compact JSON.
pub fn text(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => "null".to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => n.to_string(),
        Some(other) => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// The member `key` of a mapping.
pub fn get<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    v.as_mapping()?.get(key)
}

/// `.key // ""` as a string: null and false are the empty string.
pub fn str_or_empty(v: &Value, key: &str) -> String {
    match get(v, key) {
        None | Some(Value::Null) | Some(Value::Bool(false)) => String::new(),
        other => text(other),
    }
}

/// `yq -e '.key'` succeeds: the member is neither absent, `null` nor `false`.
pub fn truthy(v: &Value, key: &str) -> bool {
    !matches!(get(v, key), None | Some(Value::Null) | Some(Value::Bool(false)))
}

/// `.findings[]` of a document: the entries of the list, none when there is none.
pub fn findings(v: &Value) -> Vec<&Value> {
    get(v, "findings").and_then(Value::as_sequence).map(|s| s.iter().collect()).unwrap_or_default()
}

/// `.a | join(" ")` of a list of scalars; empty where yq refuses (not a list).
pub fn join_list(v: Option<&Value>) -> String {
    match v.and_then(Value::as_sequence) {
        Some(s) => s.iter().map(|i| text(Some(i))).collect::<Vec<_>>().join(" "),
        None => String::new(),
    }
}

/// The ids of the findings of `findings.yaml` with the status `resolved`, in file order
/// (`.findings[]? | select(.status == "resolved") | .id`); none when the file cannot be read.
pub fn resolved_ids(path: &Path) -> Vec<String> {
    let Some(doc) = load(path) else { return Vec::new() };
    findings(&doc)
        .into_iter()
        .filter(|f| get(f, "status").and_then(Value::as_str) == Some("resolved"))
        .map(|f| text(get(f, "id")))
        .collect()
}
