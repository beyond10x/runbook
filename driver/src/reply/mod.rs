//! The internals of `sdc-reply`: what an LLM reply may cite and the JSON schemas of the replies,
//! the checks of the rule approver, the review and the coverage document, the adapters, the
//! records of a case and change.implement as one request. `src/bin/sdc-reply.rs` is the command
//! line over these functions (its module documentation says what each subcommand prints).
//!
//! Key order: `serde_json` sorts the keys of an object, unless a crate in the same build enables
//! its `preserve_order` feature (`runbook` does, and cargo unifies features per workspace). Every
//! function that returns a `Value`, and every write of one to a file, goes through `sorted` or
//! `text`/`pretty`, and every file read through `read_json` and `read_data`, so the bytes are the
//! same in both builds.
use std::fs;
use std::path::Path;

use serde_json::Value;

pub mod adapters;
pub mod citations;
pub mod coverage;
pub mod implement;
pub mod records;
pub mod review;
pub mod review_doc;
pub mod rules;
pub mod schemas;

#[cfg(test)]
mod review_tests;

/// A value with the keys of every object in byte order, whatever order the build keeps.
pub fn sorted(v: Value) -> Value {
    match v {
        Value::Object(m) => {
            let mut e: Vec<(String, Value)> = m.into_iter().collect();
            e.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(e.into_iter().map(|(k, x)| (k, sorted(x))).collect())
        }
        Value::Array(a) => Value::Array(a.into_iter().map(sorted).collect()),
        other => other,
    }
}

/// The compact JSON text of a value, keys in byte order.
pub fn text(v: &Value) -> String {
    sorted(v.clone()).to_string()
}

/// The pretty JSON text of a value, keys in byte order; empty when it cannot be written.
pub fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(&sorted(v.clone())).unwrap_or_default()
}

/// A JSON file as a value with sorted keys; null when it is missing or not JSON.
pub fn read_json(p: &Path) -> Value {
    sorted(fs::read(p).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null))
}

/// A YAML or JSON file as JSON with sorted keys; None when it is neither.
pub fn read_data(p: &Path) -> Option<Value> {
    let text = fs::read_to_string(p).ok()?;
    let v = match p.extension().and_then(|e| e.to_str()) {
        Some("json") => serde_json::from_str(&text).ok(),
        Some("yaml" | "yml") => serde_yaml::from_str::<serde_yaml::Value>(&text).ok().and_then(|v| serde_json::to_value(v).ok()),
        _ => None,
    };
    v.map(sorted)
}

/// The files below `dir` as relative paths, sorted.
pub fn walk(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![std::path::PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let Ok(rd) = fs::read_dir(dir.join(&rel)) else { continue };
        for e in rd.flatten() {
            let r = rel.join(e.file_name());
            if e.path().is_dir() { stack.push(r) } else { out.push(r.to_string_lossy().into_owned()) }
        }
    }
    out.sort();
    out
}
