//! Schemas: the JSON schemas of the replies of the decider, the review and the coverage document.
use std::path::Path;

use serde_json::{json, Map, Value};

use super::citations::{citable_set, Ctx};
use super::{read_json, sorted};

/// A JSON schema of one object reply: every property required, nothing else allowed.
pub fn reply_schema(props: Vec<(&str, Value)>) -> Value {
    let required: Vec<&str> = props.iter().map(|(k, _)| *k).collect();
    let properties: Map<String, Value> = props.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    sorted(json!({"type": "object", "additionalProperties": false, "required": required, "properties": properties}))
}

/// The reply schema of the decider for an approval, question, finding or close.
pub fn decider_schema(c: &Ctx, kind: &str, fin: bool, options: &[String], rule: bool) -> Value {
    let set = citable_set(c);
    let mut cites: Vec<Value> = set["ids"].as_array().cloned().unwrap_or_default();
    cites.extend(set["addresses"].as_array().cloned().unwrap_or_default());
    let decisions: Vec<&str> = if kind == "finding" || kind == "question" { vec!["approve", "escalate"] } else { vec!["approve", "reject", "escalate"] };
    let answer = match kind {
        "finding" if rule => json!({"anyOf": [{"type": "string", "enum": options}, {"type": "null"}]}),
        "question" | "finding" => json!({"type": ["string", "null"]}),
        _ => json!({"type": "null"}),
    };
    let best = match (kind, fin) {
        ("finding", true) if rule => json!({"anyOf": [{"type": "string", "enum": options}, {"type": "null"}]}),
        ("finding", true) => json!({"type": ["string", "null"]}),
        ("question", _) | (_, false) => json!({"type": "null"}),
        _ => json!({"anyOf": [{"type": "string", "enum": ["approve", "reject"]}, {"type": "null"}]}),
    };
    reply_schema(vec![
        ("decision", json!({"type": "string", "enum": decisions})),
        ("answer", answer),
        ("reasons", json!({"type": "array", "minItems": 1, "items": {"type": "string"}})),
        ("cites", json!({"type": "array", "items": {"type": "string", "enum": cites}})),
        ("best", best),
    ])
}

/// What a review finding may name as violated (`violation`: the story's requirements, acceptance
/// lines and covered commands) and as outside the story (`outside`: requirements no story covers).
pub fn review_targets(in_dir: &Path) -> (Vec<String>, Vec<String>) {
    let scope = read_json(&in_dir.join("scope.json"));
    let ids = |v: &Value, k: &str| v.as_array().into_iter().flatten().filter_map(|e| e[k].as_str().map(String::from)).collect::<Vec<_>>();
    let mut inside = ids(&scope["requirements"], "id");
    inside.extend(ids(&scope["acceptance"], "cite"));
    inside.extend(scope["covers"].as_array().into_iter().flatten().filter_map(|c| c.as_str().map(String::from)));
    (inside, ids(&read_json(&in_dir.join("uncovered.json"))["requirements"], "id"))
}

/// The reply schema of change.review: a decision, a summary, and findings, each a place in the
/// change tree (`impl/<path>:<line>[-<line>]`) and what it violates. A facts review (its inputs
/// have `listing.json`) may also cite a listed or changed file by path alone.
pub fn review_schema(in_dir: &Path) -> Value {
    let (inside, outside) = review_targets(in_dir);
    let mut all = inside;
    all.extend(outside);
    let at = if in_dir.join("listing.json").is_file() { "^impl/[^:]+(:[0-9]+(-[0-9]+)?)?$" } else { "^impl/[^:]+:[0-9]+(-[0-9]+)?$" };
    reply_schema(vec![
        ("decision", json!({"type": "string", "enum": ["approved", "rejected"]})),
        ("summary", json!({"type": "string"})),
        ("findings", json!({"type": "array", "items": reply_schema(vec![
            ("kind", json!({"type": "string", "enum": ["violation", "outside"]})),
            ("at", json!({"type": "string", "pattern": at})),
            ("violates", json!({"type": "string", "enum": all})),
            ("says", json!({"type": "string", "minLength": 1})),
        ])})),
    ])
}

/// The reply schema of change.implement's coverage document: one entry per requirement of the
/// story, `built` with at least one place and a test, or `not_built` with a reason.
pub fn coverage_schema(ids: &[String]) -> Value {
    let id = if ids.is_empty() { json!({"type": "string"}) } else { json!({"type": "string", "enum": ids}) };
    let built = reply_schema(vec![
        ("id", id.clone()),
        ("status", json!({"type": "string", "enum": ["built"]})),
        ("at", json!({"type": "array", "minItems": 1, "items": reply_schema(vec![
            ("file", json!({"type": "string", "minLength": 1})),
            ("line", json!({"type": "integer", "minimum": 1})),
        ])})),
        ("test", json!({"type": "string", "minLength": 1})),
    ]);
    let not_built = reply_schema(vec![
        ("id", id),
        ("status", json!({"type": "string", "enum": ["not_built"]})),
        ("reason", json!({"type": "string", "minLength": 1})),
    ]);
    let mut list = json!({"type": "array", "items": {"anyOf": [built, not_built]}});
    if ids.is_empty() {
        list["maxItems"] = json!(0);
    }
    reply_schema(vec![("requirements", list)])
}
