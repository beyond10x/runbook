//! The reply of the implement node: its schema and the check by program.
use std::collections::BTreeSet;

use serde_json::{json, Map, Value};

use super::tree::confine;
use crate::reply::schemas::reply_schema;
use crate::reply::sorted;

#[derive(Debug, Default, PartialEq)]
/// A checked reply of the implement node.
pub struct Reply {
    /// The files written: confined path and full content.
    pub files: Vec<(String, String)>,
    /// The reply's note.
    pub note: String,
    /// The findings the reply raised.
    pub findings: Vec<Value>,
    /// The paths the reply left for a continuation.
    pub remaining: Vec<String>,
}

const FINDING_TYPES: [&str; 3] = ["ambiguity", "contradiction", "missing_decision"];
const PATH_PATTERN: &str = "^[^/.\\\\][^\\\\]*$";

/// The reply schema: `files` (path and full content each), `note`, `findings`, `remaining`.
/// The envelope is schema-checked; a file's `content` is free text (docs/driver.md).
pub fn schema() -> Value {
    let path = json!({"type": "string", "minLength": 1, "pattern": PATH_PATTERN});
    let mut files = json!({"type": "array", "items": reply_schema(vec![("path", path.clone()), ("content", json!({"type": "string"}))])});
    files["minItems"] = json!(1);
    let mut s = reply_schema(vec![
        ("files", files),
        ("note", json!({"type": "string"})),
        ("findings", json!({"type": "array", "items": reply_schema(vec![
            ("type", json!({"type": "string", "enum": FINDING_TYPES})),
            ("cites", json!({"type": "array", "minItems": 1, "items": {"type": "string"}})),
            ("says", json!({"type": "string", "minLength": 1})),
            // 2 to 4: checked by program (check_reply); the schema keeps to the keywords the
            // review and decider schemas already use with claude -p --json-schema.
            ("options", json!({"type": "array", "minItems": 1, "items": {"type": "string"}})),
            ("default", json!({"type": "string"})),
            ("scope_changing", json!({"type": "boolean"})),
        ])})),
        ("remaining", json!({"type": "array", "items": path})),
    ]);
    s["description"] = json!("change.implement's reply: the complete text of every file written or replaced");
    sorted(s)
}

/// The keys of an object against the expected set: missing and unknown keys, named.
fn keys(v: &Map<String, Value>, want: &[&str], at: &str, why: &mut Vec<String>) {
    for k in want {
        if !v.contains_key(*k) {
            why.push(format!("{at}`{k}` is missing"));
        }
    }
    for k in v.keys() {
        if !want.contains(&k.as_str()) {
            why.push(format!("{at}unknown key `{k}`"));
        }
    }
}

fn string_list(v: &Value) -> Option<Vec<String>> {
    v.as_array()?.iter().map(|x| x.as_str().map(String::from)).collect()
}

/// The reply checked by program against `schema` and the path rule. `asked`: on a continuation,
/// the paths asked for; each must be in `files` or again in `remaining`.
pub fn check_reply(v: &Value, asked: Option<&[String]>) -> Result<Reply, Vec<String>> {
    let Some(o) = v.as_object() else { return Err(vec!["the reply is not a JSON object".into()]) };
    let mut why = Vec::new();
    keys(o, &["files", "note", "findings", "remaining"], "", &mut why);
    let mut r = Reply::default();
    match o.get("files").map(|f| f.as_array()) {
        Some(Some(fs_)) if fs_.is_empty() => why.push("`files` is empty: write at least one file".into()),
        Some(Some(fs_)) => {
            let mut seen = BTreeSet::new();
            for (i, f) in fs_.iter().enumerate() {
                let at = format!("files[{i}]: ");
                let Some(fo) = f.as_object() else { why.push(format!("{at}not an object")); continue };
                keys(fo, &["path", "content"], &at, &mut why);
                let content = fo.get("content").and_then(|c| c.as_str());
                if content.is_none() {
                    why.push(format!("{at}`content` is not a string"));
                }
                match fo.get("path").and_then(|p| p.as_str()) {
                    None => why.push(format!("{at}`path` is not a string")),
                    Some(p) => match confine(p) {
                        Err(e) => why.push(format!("{at}{e}")),
                        Ok(p) if !seen.insert(p.clone()) => why.push(format!("{at}{p:?} twice")),
                        Ok(p) => r.files.push((p, content.unwrap_or_default().to_string())),
                    },
                }
            }
        }
        Some(None) => why.push("`files` is not a list".into()),
        None => {}
    }
    match o.get("note") {
        Some(Value::String(s)) => r.note = s.clone(),
        Some(_) => why.push("`note` is not a string".into()),
        None => {}
    }
    match o.get("findings").map(|f| f.as_array()) {
        Some(Some(list)) => {
            for (i, f) in list.iter().enumerate() {
                let at = format!("findings[{i}]: ");
                let Some(fo) = f.as_object() else { why.push(format!("{at}not an object")); continue };
                keys(fo, &["type", "cites", "says", "options", "default", "scope_changing"], &at, &mut why);
                if !fo.get("type").and_then(|t| t.as_str()).is_some_and(|t| FINDING_TYPES.contains(&t)) {
                    why.push(format!("{at}`type` is not one of {}", FINDING_TYPES.join(", ")));
                }
                if !fo.get("cites").and_then(string_list).is_some_and(|c| !c.is_empty()) {
                    why.push(format!("{at}`cites` is not a non-empty list of ids"));
                }
                if !fo.get("says").and_then(|s| s.as_str()).is_some_and(|s| !s.trim().is_empty()) {
                    why.push(format!("{at}`says` is empty"));
                }
                let opts = fo.get("options").and_then(string_list).filter(|o| (2..=4).contains(&o.len()));
                if opts.is_none() {
                    why.push(format!("{at}`options` is not a list of 2 to 4 answers"));
                }
                if !fo.get("default").and_then(|d| d.as_str()).is_some_and(|d| opts.as_ref().is_some_and(|o| o.iter().any(|x| x == d))) {
                    why.push(format!("{at}`default` is not one of its options"));
                }
                if !fo.get("scope_changing").is_some_and(|s| s.is_boolean()) {
                    why.push(format!("{at}`scope_changing` is not true or false"));
                }
                r.findings.push(f.clone());
            }
        }
        Some(None) => why.push("`findings` is not a list".into()),
        None => {}
    }
    match o.get("remaining").map(string_list) {
        Some(Some(list)) => {
            for p in list {
                match confine(&p) {
                    Err(e) => why.push(format!("remaining: {e}")),
                    Ok(p) if r.files.iter().any(|(f, _)| *f == p) => why.push(format!("remaining: {p} is both written and remaining")),
                    Ok(p) => r.remaining.push(p),
                }
            }
        }
        Some(None) => why.push("`remaining` is not a list of paths".into()),
        None => {}
    }
    for p in asked.unwrap_or(&[]) {
        if !r.files.iter().any(|(f, _)| f == p) && !r.remaining.contains(p) {
            why.push(format!("{p} was asked for: write it, or list it in `remaining` again"));
        }
    }
    if why.is_empty() { Ok(r) } else { Err(why) }
}
