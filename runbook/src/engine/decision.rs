//! What a decision Loom names becomes, and what an answer becomes.
//!
//! A decision is read from its structured members (`case`, `kind`, `action`, `capability`,
//! `outcome`, `decides`, `subject`, `revision`, `path`) and never from its printed `command`: the
//! engine neither parses nor runs that line. An answer is a library call chosen by the kind:
//!
//! | kind | answer `approve` | answer `deny` | any other word, or a JSON object with `result` |
//! |---|---|---|---|
//! | `Approve` | `approve` of the capability | `approve` with the decision deny | refused by Loom |
//! | `ApproveAction` | `approve` with the action, the result `performed` | refused by Loom (deny takes no action) | `approve` with the action and that result |
//! | `Close` | `close` through the decision's outcome | declines this outcome | refused (no result for a close) |
//! | `Handoff` | the text was done, run the tree again | the same | the same |

use crate::runtime::DecisionRequest;
use b10x_loom_tree::model::{DecisionKind, NextDecision};
use serde_json::{json, Value};

/// A decision of one round, with what the engine can do about it.
#[derive(Clone, Debug)]
pub struct Planned {
    pub request: DecisionRequest,
    pub decision: NextDecision,
    /// Why no library call can answer it from its structured members; the run then waits for a
    /// cancel.
    pub unanswerable: Option<String>,
}

/// The kind as Loom names it.
pub fn kind_name(kind: DecisionKind) -> &'static str {
    match kind {
        DecisionKind::Approve => "Approve",
        DecisionKind::ApproveAction => "ApproveAction",
        DecisionKind::Close => "Close",
        DecisionKind::Handoff => "Handoff",
    }
}

/// The request for `decision`, under the id `request_id`; `note` (the refusal of an earlier answer)
/// is added to the question.
pub fn plan(request_id: String, decision: &NextDecision, note: Option<&str>) -> Planned {
    let kind = decision.kind;
    let missing = match kind {
        DecisionKind::Approve if decision.capability.is_none() => Some("capability"),
        DecisionKind::ApproveAction if decision.action.is_none() => Some("action"),
        DecisionKind::Close if decision.outcome.is_none() => Some("outcome"),
        _ => None,
    };
    let unanswerable = missing.map(|m| format!("a {} decision without the member {m}: no library call can be chosen from its structured members (Loom 5af3f9c2, generated/rust/loom/src/tree.rs:78 and :102)", kind_name(kind)));
    let mut question = decision.decides.clone();
    for (label, value) in [("subject", &decision.subject), ("revision", &decision.revision), ("path", &decision.path)] {
        if let Some(v) = value {
            question.push_str(&format!("\n{label}: {v}"));
        }
    }
    if let Some(n) = note {
        question.push_str(&format!("\nThe earlier answer was refused: {n}"));
    }
    if let Some(u) = &unanswerable {
        question.push_str(&format!("\nThis request cannot be answered: {u}."));
    }
    let case = &decision.case;
    let options = match kind {
        DecisionKind::Approve => vec![
            json!({"answer": "approve", "effect": format!("allow the capability {} for case {case}", decision.capability.as_deref().unwrap_or(""))}),
            json!({"answer": "deny", "effect": "record a denial of that capability"}),
        ],
        DecisionKind::ApproveAction => vec![
            json!({"answer": "approve", "effect": "take the action with the result performed, as the principal who answers"}),
            json!({"answer": "{\"result\":\"<result>\",\"reasons\":\"<text>\"}", "effect": "take the action with a result its binding names and the reasons (the result alone may be given as the answer)"}),
        ],
        DecisionKind::Close => vec![
            json!({"answer": "approve", "effect": format!("close case {case} through {}", decision.outcome.as_deref().unwrap_or(""))}),
            json!({"answer": "deny", "effect": "decline this outcome"}),
        ],
        DecisionKind::Handoff => vec![json!({"answer": "approve", "effect": "the handoff text was done: run the tree again"})],
    };
    let request = DecisionRequest {
        request_id,
        case_id: decision.case.clone(),
        action: decision.action.clone().unwrap_or_else(|| match kind {
            DecisionKind::Close => "close".to_string(),
            _ => "handoff".to_string(),
        }),
        capability: decision.capability.clone().unwrap_or_default(),
        question,
        options,
    };
    Planned { request, decision: decision.clone(), unanswerable }
}

/// An answer read.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Parsed {
    pub deny: bool,
    pub result: Option<String>,
    pub reasons: Option<String>,
}

/// `approve`, `deny`, a bare result name, or a JSON object with `decision` (`approve` or `deny`),
/// `result` and `reasons`; anything else is refused with what is accepted.
pub fn parse_answer(text: &str) -> Result<Parsed, String> {
    let text = text.trim();
    const HELP: &str = "an answer is approve, deny, a result name, or a JSON object with decision, result and reasons";
    if text.starts_with('{') {
        let v: Value = serde_json::from_str(text).map_err(|e| format!("the answer is not JSON: {e}; {HELP}"))?;
        let obj = v.as_object().ok_or_else(|| HELP.to_string())?;
        let mut out = Parsed::default();
        for (k, val) in obj {
            let s = val.as_str().ok_or_else(|| format!("{k} is not text; {HELP}"))?;
            match k.as_str() {
                "decision" => match s {
                    "approve" => {}
                    "deny" => out.deny = true,
                    other => return Err(format!("decision {other:?} is not approve or deny")),
                },
                "result" => out.result = Some(s.to_string()),
                "reasons" => out.reasons = Some(s.to_string()),
                other => return Err(format!("{other} is not a member of an answer; {HELP}")),
            }
        }
        return Ok(out);
    }
    match text {
        "" => Err(format!("the answer is empty; {HELP}")),
        "approve" => Ok(Parsed::default()),
        "deny" => Ok(Parsed { deny: true, ..Parsed::default() }),
        word if !word.contains(char::is_whitespace) => Ok(Parsed { result: Some(word.to_string()), ..Parsed::default() }),
        _ => Err(HELP.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_are_read() {
        assert_eq!(parse_answer("approve").unwrap(), Parsed::default());
        assert!(parse_answer(" deny ").unwrap().deny);
        assert_eq!(parse_answer("rejected").unwrap().result.as_deref(), Some("rejected"));
        let p = parse_answer(r#"{"result":"rejected","reasons":"too long"}"#).unwrap();
        assert_eq!((p.result.as_deref(), p.reasons.as_deref(), p.deny), (Some("rejected"), Some("too long"), false));
        assert!(parse_answer("").is_err());
        assert!(parse_answer("two words").is_err());
        assert!(parse_answer(r#"{"result":1}"#).is_err());
        assert!(parse_answer(r#"{"colour":"red"}"#).is_err());
        assert!(parse_answer(r#"{"decision":"maybe"}"#).is_err());
    }

    fn decision(kind: DecisionKind) -> NextDecision {
        NextDecision {
            case: "c".into(),
            kind,
            action: Some("a".into()),
            capability: Some("cap".into()),
            decides: "a of case c".into(),
            subject: Some("draft".into()),
            revision: Some("r1".into()),
            path: None,
            outcome: Some("done".into()),
            command: vec!["b10x-loom".into(), "never".into(), "read".into()],
        }
    }

    #[test]
    fn a_decision_is_read_from_its_members_and_never_from_its_command() {
        let p = plan("r1".into(), &decision(DecisionKind::ApproveAction), None);
        assert_eq!((p.request.case_id.as_str(), p.request.action.as_str(), p.request.capability.as_str()), ("c", "a", "cap"));
        assert!(p.request.question.contains("subject: draft") && !p.request.question.contains("never"));
        assert!(p.unanswerable.is_none());
    }

    #[test]
    fn a_decision_that_lacks_the_member_its_call_needs_is_unanswerable() {
        let mut d = decision(DecisionKind::Close);
        d.outcome = None;
        assert!(plan("r".into(), &d, None).unanswerable.unwrap().contains("outcome"));
        let mut d = decision(DecisionKind::Approve);
        d.capability = None;
        assert!(plan("r".into(), &d, None).unanswerable.unwrap().contains("capability"));
    }
}
