//! The rule kinds of the decider: `approve` and `reject` answer without a model, and the dry adapter
//! falls back to the `approve` rule. The reply has the members `decider.sh` printed, in its order.

use serde_json::{json, Value};

const APPROVE_WHY: &str = "rule decider: approve every pending approval, accept every question's default, close at the legitimate outcome";
const REJECT_WHY: &str = "rule decider: refuse every decision";
const DRY_WHY: &str = "dry run: no LLM was called; the approve rule decided";

/// The reply of `decision` (approve or reject) by rule: a question or a finding that is approved
/// takes its own default as the answer. `kind` is the decider kind the reply names.
pub(super) fn rule(kind: &str, decision: &str, why: &str, dry: bool, req: &Value) -> Value {
    let answer = match (req["type"].as_str(), decision) {
        (Some("question"), "approve") => req["question"]["default"].clone(),
        (Some("finding"), "approve") => req["finding"]["default"].clone(),
        _ => Value::Null,
    };
    json!({"decision": decision, "answer": answer, "best": null, "reasons": [why], "cites": [], "kind": kind, "model": null,
        "adapter": "rule", "usage": null, "cost_usd": 0, "valid": true, "dry": dry})
}

/// The reply of the kind `approve` or `reject`.
pub(super) fn rule_reply(kind: &str, req: &Value) -> Value {
    match kind {
        "reject" => rule(kind, "reject", REJECT_WHY, false, req),
        _ => rule(kind, "approve", APPROVE_WHY, false, req),
    }
}

/// The reply of a dry call: the approve rule, marked dry, with what the adapter printed.
pub(super) fn dry_reply(rec: &std::path::Path, response: &Value, req: &Value) -> Value {
    let mut reply = rule("model", "approve", DRY_WHY, true, req);
    if let Some(map) = reply.as_object_mut() {
        map.insert("kind".to_owned(), json!("model"));
        map.insert("model".to_owned(), response["model"].clone());
        map.insert("effort".to_owned(), response["effort"].clone());
        map.insert("adapter".to_owned(), json!("dry"));
        map.insert("exit".to_owned(), response["exit"].clone());
        map.insert("record".to_owned(), json!(rec.display().to_string()));
        map.insert("dry".to_owned(), json!(true));
    }
    reply
}
