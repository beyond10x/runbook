//! The check of the model's reply (`out/decision.json`): a program, not the model, decides whether
//! it is a decision. An invalid or missing reply is `escalate`, with the reason; the caller asks again.

use super::prompt::or;
use sdc_tools::reply::citations::{ctx, resolve};
use sdc_tools::reply::records::Facts;
use serde_json::{json, Map, Value};
use std::path::Path;

const KNOWN: [&str; 5] = ["answer", "best", "cites", "decision", "reasons"];

/// What the call is checked against.
pub(super) struct Check<'a> {
    pub(super) req: &'a Value,
    pub(super) cwd: &'a Path,
    pub(super) case_dir: &'a Path,
    pub(super) rec: &'a Path,
    pub(super) final_escalate: bool,
}

/// What the call says of itself, from the adapter's response.
fn meta(response: &Value, rec: &Path, exit: &Value) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("kind".to_owned(), json!("model"));
    for key in ["model", "effort", "adapter", "usage", "cost_usd", "duration_ms"] {
        m.insert(key.to_owned(), response[key].clone());
    }
    m.insert("exit".to_owned(), exit.clone());
    m.insert("record".to_owned(), json!(rec.display().to_string()));
    m.insert("schema_constrained".to_owned(), or(&response["schema_constrained"], &json!(false)).clone());
    m.insert("schema_note".to_owned(), or(&response["schema_note"], &Value::Null).clone());
    m
}

fn is_text(v: &Value) -> bool {
    v.as_str().is_some_and(|s| !s.is_empty())
}

fn in_options(v: &Value, options: &[Value]) -> bool {
    options.contains(v)
}

/// Why `out` is not a decision of this request, or an empty text when it is. `resolved` holds the
/// resolution of each cite.
fn why(out: &Value, req: &Value, resolved: &[Value], final_escalate: bool) -> String {
    let kind = req["type"].as_str().unwrap_or("");
    let options: Vec<Value> = or(&req["finding"]["options"], &json!([])).as_array().cloned().unwrap_or_default();
    let rule = !req["finding"]["sides"].is_null() || !req["finding"]["trace"].is_null();
    let Some(map) = out.as_object() else { return "not a JSON object".to_owned() };
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();
    let unknown: Vec<&&String> = keys.iter().filter(|k| !KNOWN.contains(&k.as_str())).collect();
    let (d, b, a, r) = (&out["decision"], &out["best"], &out["answer"], &out["reasons"]);
    let empty = json!([]);
    let cites = or(&out["cites"], &empty);
    let is_best = |s: &str| b.as_str() == Some(s);
    if !unknown.is_empty() {
        return format!("unknown keys {}", Value::from(unknown.iter().map(|k| k.as_str()).collect::<Vec<_>>()));
    }
    if !matches!(d.as_str(), Some("approve" | "reject" | "escalate")) {
        return "decision is not approve, reject or escalate".to_owned();
    }
    let decision = d.as_str().unwrap_or("");
    if kind == "finding" && decision == "reject" {
        return "a finding is decided with approve and an answer, or escalate".to_owned();
    }
    if kind == "question" && decision == "reject" {
        return "a question is decided with approve and an answer, or escalate".to_owned();
    }
    if !r.as_array().is_some_and(|l| !l.is_empty() && l.iter().all(is_text)) {
        return "reasons is not a non-empty list of sentences".to_owned();
    }
    if !cites.as_array().is_some_and(|l| l.iter().all(Value::is_string)) {
        return "cites is not a list of ids".to_owned();
    }
    let bad: Vec<String> = resolved.iter().filter(|x| x["ok"] != true).map(|x| format!("{} ({})", raw_text(&x["cite"]), raw_text(&x["why"]))).collect();
    if !bad.is_empty() {
        return format!("cites what does not resolve: {}", bad.join("; "));
    }
    if !b.is_null() && decision != "escalate" {
        return "best is only for an escalate".to_owned();
    }
    if kind == "finding" && !b.is_null() && !is_text(b) {
        return "best is not an answer".to_owned();
    }
    if kind == "finding" && rule && !b.is_null() && !in_options(b, &options) {
        return "best is not one of the options of a rule finding".to_owned();
    }
    if kind != "finding" && !b.is_null() && !is_best("approve") && !is_best("reject") {
        return "best is not approve or reject".to_owned();
    }
    if final_escalate && decision == "escalate" && b.is_null() && kind != "question" && kind != "finding" {
        return "an escalate needs best: approve or reject (SDC_DECIDER_ESCALATE=final)".to_owned();
    }
    if kind == "question" && (decision == "approve" || is_best("approve")) && !is_text(a) {
        return "an approved question needs an answer".to_owned();
    }
    if kind == "finding" && decision == "approve" && !is_text(a) {
        return "an approved finding needs an answer".to_owned();
    }
    if kind == "finding" && decision == "approve" && rule && !in_options(a, &options) {
        return "the answer to a rule finding is one of its options".to_owned();
    }
    if kind != "question" && kind != "finding" && !a.is_null() {
        return "answer is only for a question or a finding".to_owned();
    }
    String::new()
}

fn raw_text(v: &Value) -> String {
    super::prompt::raw(v)
}

/// The reply of the decider for the model's output: its decision with the call's meta, or an
/// `escalate` that names why the output is not one.
pub(super) fn validate(c: &Check<'_>, response: &Value) -> Value {
    let out_file = c.cwd.join("out").join("decision.json");
    let bytes = std::fs::read(&out_file).unwrap_or_default();
    let parsed: Option<Value> = serde_json::from_slice::<Value>(&bytes).ok().filter(|v| !v.is_null() && v != &json!(false));
    let exit = or(&response["exit"], &json!(0)).clone();
    let m = meta(response, c.rec, &exit);
    let mut resolved: Vec<Value> = Vec::new();
    if let Some(out) = &parsed {
        let cites: Vec<&str> = out["cites"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        if !cites.is_empty() {
            let context = ctx(c.cwd, &Facts::driver(c.case_dir));
            resolved = cites.iter().map(|cite| resolve(&context, cite)).collect();
        }
    }
    let problem = if bytes.is_empty() {
        "out/decision.json is missing".to_owned()
    } else {
        match &parsed {
            None => "out/decision.json is not JSON".to_owned(),
            Some(out) => why(out, c.req, &resolved, c.final_escalate),
        }
    };
    let mut reply = Map::new();
    match (&parsed, problem.is_empty()) {
        (Some(out), true) => {
            reply.insert("decision".to_owned(), out["decision"].clone());
            reply.insert("answer".to_owned(), or(&out["answer"], &Value::Null).clone());
            reply.insert("best".to_owned(), or(&out["best"], &Value::Null).clone());
            reply.insert("reasons".to_owned(), out["reasons"].clone());
            reply.insert("cites".to_owned(), or(&out["cites"], &json!([])).clone());
            reply.insert("citations".to_owned(), Value::Array(resolved.iter().map(|x| json!({"cite": x["cite"], "form": x["form"], "value": x["value"]})).collect()));
            reply.insert("valid".to_owned(), json!(true));
        }
        _ => {
            reply.insert("decision".to_owned(), json!("escalate"));
            reply.insert("answer".to_owned(), Value::Null);
            reply.insert("best".to_owned(), Value::Null);
            reply.insert("reasons".to_owned(), json!([format!("invalid decider reply: {problem}")]));
            reply.insert("cites".to_owned(), json!([]));
            reply.insert("valid".to_owned(), json!(false));
            reply.insert("invalid_reason".to_owned(), json!(problem));
        }
    }
    reply.extend(m);
    Value::Object(reply)
}
