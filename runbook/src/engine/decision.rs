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

use crate::runtime::{DecisionRequest, RequestArtifact};
use b10x_loom_tree::model::{DecisionKind, NextDecision};
use serde_json::{json, Value};
use std::path::Path;

/// A decision of one round, with what the engine can do about it.
#[derive(Clone, Debug)]
pub struct Planned {
    pub request: DecisionRequest,
    pub decision: NextDecision,
    /// Why no library call can answer it from its structured members; the run then waits for a
    /// cancel.
    pub unanswerable: Option<String>,
    /// What was read from the case for it: a request asked again is built from this, never from the
    /// file read again.
    pub found: Found,
}

/// The file of the case that holds a decision request in the contract's shape, in the directory of the
/// decision's subject (`docs/engine.md`, "A request read from the case").
pub const ARTIFACT_FILE: &str = "decision-request.json";
/// The file beside the bindings files that names the question member of a person action.
pub const QUESTIONS_FILE: &str = "person-questions.json";
/// The format line of [`QUESTIONS_FILE`].
pub const QUESTIONS_FORMAT: &str = "runbook.person-questions/1";
/// The name the cause of a run starts with when its artifact fails the check.
pub const REFUSED: &str = "DecisionArtifactRefused";
/// The kind of the run event that names why a question member gave no question.
pub const QUESTION_UNREAD: &str = "runbook.question-field-unread";
/// The most bytes of an artifact, a subject or a cited file the engine reads.
const READ_MAX: u64 = 1 << 20;

/// What the engine read for a decision from its case and from the run's bindings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Found {
    /// The case's [`ARTIFACT_FILE`]: its question, its options and the members beside them.
    pub contract: Option<Contract>,
    /// The text of the question member the bindings name for the action.
    pub question: Option<String>,
    /// The results the action declares, where the bindings name a question member for it.
    pub declared: Option<Vec<String>>,
    /// Why the named question member gave no question (recorded as [`QUESTION_UNREAD`]).
    pub unread: Option<String>,
}

/// A decision request in the contract's shape, as the case holds it and the check passed it.
#[derive(Clone, Debug, PartialEq)]
pub struct Contract {
    pub question: String,
    pub options: Vec<Value>,
    pub artifact: RequestArtifact,
}

/// What the case of `decision` holds for its request: the artifact in the directory of the decision's
/// `path` inside `workspace`, where it is meant for this decision (its `unblocks` names the decision's
/// action, or its outcome), and, where `bindings` (the run's bindings directory) names a question
/// member for the person action, that member of the subject and the results the action declares. A
/// decision with no `path` inside the workspace reads no artifact. An artifact that fails the check is
/// an error, the cause of the run; it names no path but the workspace's relative ones.
pub fn read_case(decision: &NextDecision, workspace: &Path, bindings: Option<&Path>) -> Result<Found, String> {
    let subject = subject_of(decision, workspace);
    let mut found = Found::default();
    let mut options_read: Option<usize> = None;
    if let Some(dir) = subject.as_deref().map(|s| s.parent().unwrap_or(Path::new(""))) {
        let shown = dir.join(ARTIFACT_FILE).display().to_string();
        let refused = |why: String| format!("{REFUSED}: {shown}: {why}");
        // The subject's directory is opened below the workspace without following a link: a directory reached through
        // one is not the workspace's, and no artifact is read from it.
        if let Ok(held) = crate::record_write::Held::open_beneath(workspace, dir) {
            if held.holds(ARTIFACT_FILE) {
                let bytes = held.read_within(ARTIFACT_FILE, READ_MAX).map_err(|_| refused("not a regular file of one name of at most 1 MiB".into()))?;
                let doc: Value = serde_json::from_slice(&bytes).map_err(|e| refused(format!("not JSON: {e}")))?;
                if meant_for(&doc, decision) {
                    let c = contract(&doc, decision, workspace).map_err(refused)?;
                    options_read = Some(c.options.len());
                    found.contract = Some(c);
                }
            }
        }
    }
    let action = decision.action.as_deref().filter(|_| decision.kind == DecisionKind::ApproveAction);
    if let (Some(action), Some(bindings)) = (action, bindings) {
        match question_member(bindings, action)? {
            None => {}
            Some(Err(why)) => found.unread = Some(why),
            Some(Ok(member)) => match super::shape::declared_results(bindings, action) {
                Ok(Some(results)) if !results.is_empty() => {
                    found.declared = Some(results);
                    match subject_text(subject.as_deref(), workspace, &member) {
                        Ok(text) => found.question = Some(text),
                        Err(why) => found.unread = Some(why),
                    }
                }
                Ok(_) => found.unread = Some(format!("the action {action} declares no result to offer as an option; the request of before is asked")),
                Err(e) => found.unread = Some(format!("the declared results of {action} cannot be read ({e}); the request of before is asked")),
            },
        }
    }
    // The contract opens no request of fewer than two options (`sdc.decision.TooFewOptions`); an artifact of none is
    // taken only for its question, where the declared results give the options.
    if let Some(n) = options_read.filter(|n| *n < 2 && !(*n == 0 && found.declared.is_some())) {
        let shown = subject.as_deref().map(|s| s.parent().unwrap_or(Path::new("")).join(ARTIFACT_FILE).display().to_string()).unwrap_or_default();
        return Err(format!("{REFUSED}: {shown}: it offers {n} option{}, and a request offers at least two (too-few-options)", if n == 1 { "" } else { "s" }));
    }
    Ok(found)
}

/// The decision's subject file, relative to `workspace`, where it lies inside it.
fn subject_of(decision: &NextDecision, workspace: &Path) -> Option<std::path::PathBuf> {
    let path = Path::new(decision.path.as_deref()?);
    let inside = if path.is_absolute() { path.strip_prefix(workspace).ok()? } else { path };
    (inside.components().count() > 0 && inside.components().all(|c| matches!(c, std::path::Component::Normal(_)))).then(|| inside.to_path_buf())
}

/// Whether the artifact `doc` is the request of `decision`: its `unblocks.action` is the decision's
/// action, or, for an outcome, its `unblocks.outcome` the decision's outcome.
fn meant_for(doc: &Value, decision: &NextDecision) -> bool {
    let unblocks = &doc["unblocks"];
    let action = decision.action.as_deref().is_some_and(|a| unblocks["action"].as_str() == Some(a));
    let outcome = decision.kind == DecisionKind::Close && unblocks["kind"] == "outcome" && decision.outcome.as_deref().is_some_and(|o| unblocks["outcome"].as_str() == Some(o));
    action || outcome
}

/// The checked request of the artifact `doc`: its question, its options (each with a letter of its own,
/// `does`, `cost` and `consequence`), the revision it was made for, its recommendation and its grounds,
/// each `path_line` citation resolved in `workspace`.
fn contract(doc: &Value, decision: &NextDecision, workspace: &Path) -> Result<Contract, String> {
    let question = doc["question"].as_str().filter(|q| !q.trim().is_empty()).ok_or("it has no question")?;
    let options = doc["options"].as_array().ok_or("it has no list of options")?;
    let mut letters: Vec<&str> = Vec::new();
    for (n, o) in options.iter().enumerate() {
        let letter = o["letter"].as_str().filter(|l| !l.is_empty()).ok_or(format!("option {} has no letter", n + 1))?;
        if letters.contains(&letter) {
            return Err(format!("it names the letter {letter} twice"));
        }
        if !(o["does"].is_string() && o["cost"].is_string() && o["consequence"].is_object()) {
            return Err(format!("option {letter} lacks does, cost or consequence"));
        }
        // An option of an action's approval is taken with its `consequence.answer` as the result, else as `performed`:
        // one that blocks the action names the result it records, or the action would be performed by it.
        if let (DecisionKind::ApproveAction, Some(action)) = (decision.kind, decision.action.as_deref()) {
            let blocks = o["consequence"]["blocks"].as_array().is_some_and(|b| b.iter().any(|x| x.as_str() == Some(action)));
            if blocks && !o["consequence"]["answer"].as_str().is_some_and(|a| !a.trim().is_empty()) {
                return Err(format!("option {letter} blocks {action} and names no answer, so taking it would perform {action}"));
            }
        }
        letters.push(letter);
    }
    let revision = doc["subject_revision"].as_str().ok_or("it has no subject_revision")?;
    match decision.revision.as_deref() {
        Some(r) if r == revision => {}
        Some(r) => return Err(format!("its subject_revision {revision} is not the revision under decision {r}")),
        None => return Err(format!("its subject_revision {revision} cannot be held against the decision, which names no revision")),
    }
    let recommended_option = doc["recommended_option"].as_str().map(str::to_owned);
    if let Some(r) = recommended_option.as_deref().filter(|r| !letters.contains(r)) {
        return Err(format!("it recommends {r}, which is no option"));
    }
    let strength = doc["recommendation_strength"].as_str().unwrap_or("none");
    if !["none", "weak", "moderate", "strong"].contains(&strength) {
        return Err(format!("its recommendation_strength {strength} is not none, weak, moderate or strong"));
    }
    let grounds = match &doc["grounds"] {
        Value::Null => Vec::new(),
        Value::Array(g) => g.clone(),
        _ => return Err("its grounds are not a list".into()),
    };
    for (n, g) in grounds.iter().enumerate() {
        if g["citation"]["form"] == "path_line" {
            path_line(&g["citation"]["value"], workspace).map_err(|why| format!("ground {}: {why}", n + 1))?;
        }
    }
    let artifact = RequestArtifact {
        recommended_option,
        recommended_answer: doc["recommended_answer"].as_str().map(str::to_owned),
        recommendation_strength: strength.to_owned(),
        grounds,
        free_answer: doc["unblocks"]["kind"] == "question",
    };
    Ok(Contract { question: question.to_owned(), options: options.clone(), artifact })
}

/// A `path_line` citation resolves: a file below `workspace`, the lines inside it, and the digest of the
/// lines where it names one (`sha256:<hex>` of the lines as `sed -n <line>,<end>p` prints them, as
/// `tools/decision-grounds.sh` takes it).
fn path_line(v: &Value, workspace: &Path) -> Result<(), String> {
    use sha2::Digest;
    let path = v["path"].as_str().ok_or("a path_line citation without a path")?;
    let rel = Path::new(path);
    if rel.is_absolute() || !rel.components().all(|c| matches!(c, std::path::Component::Normal(_) | std::path::Component::CurDir)) {
        return Err(format!("the cited file {path} leaves the workspace"));
    }
    let line = v["line"].as_i64().ok_or(format!("the citation of {path} has no line"))?;
    let end = v["line_end"].as_i64().unwrap_or(line);
    // Every component is opened below the workspace without following a link: a citation below a link the program
    // left resolves nowhere, whatever the link names.
    let bytes = crate::record_write::read_regular_beneath(workspace, rel, READ_MAX).map_err(|_| format!("no file {path} below the workspace (a missing name, a link or not a regular file of one name)"))?;
    let lines: Vec<&[u8]> = bytes.split_inclusive(|b| *b == b'\n').collect();
    let n = bytes.iter().filter(|b| **b == b'\n').count() as i64;
    if !(line >= 1 && end >= line && end <= n) {
        return Err(format!("lines {line}-{end} outside {path} ({n} lines)"));
    }
    if let Some(want) = v["digest"].as_str() {
        let cited = lines[(line - 1) as usize..end as usize].concat();
        if format!("sha256:{:x}", sha2::Sha256::digest(&cited)) != want {
            return Err(format!("changed: lines {line}-{end} of {path} differ from the cited digest"));
        }
    }
    Ok(())
}

/// The member of the subject that [`QUESTIONS_FILE`] in `bindings` names as the question of `action`;
/// none when the file is absent or names no member for it, and why no member can be read where it names
/// one that is not a text or an empty text (recorded like a member the subject lacks).
fn question_member(bindings: &Path, action: &str) -> Result<Option<Result<String, String>>, String> {
    let refused = |why: String| format!("{REFUSED}: bindings/{QUESTIONS_FILE}: {why}");
    let held = crate::record_write::Held::open(bindings).map_err(|_| refused("the bindings directory cannot be read".into()))?;
    if !held.exists(QUESTIONS_FILE) {
        return Ok(None);
    }
    let text = held.read_text(QUESTIONS_FILE).map_err(|_| refused("not a regular file of one name".into()))?;
    let doc: Value = serde_json::from_str(&text).map_err(|e| refused(format!("not JSON: {e}")))?;
    if doc["format"] != QUESTIONS_FORMAT {
        return Err(refused(format!("its format is not {QUESTIONS_FORMAT}")));
    }
    match &doc["actions"][action] {
        Value::Null => Ok(None),
        Value::String(m) if !m.trim().is_empty() => Ok(Some(Ok(m.clone()))),
        _ => Ok(Some(Err(format!("bindings/{QUESTIONS_FILE} names for {action} a member that is not a text of a name")))),
    }
}

/// The text of the top-level `member` of the subject (JSON or YAML), or why there is none.
fn subject_text(subject: Option<&Path>, workspace: &Path, member: &str) -> Result<String, String> {
    let subject = subject.ok_or(format!("the decision names no subject file inside the workspace to read the member {member} of"))?;
    let shown = subject.display();
    let bytes = crate::record_write::read_regular_beneath(workspace, subject, READ_MAX).map_err(|_| format!("the subject {shown} cannot be read below the workspace"))?;
    let doc: serde_yaml::Value = serde_yaml::from_slice(&bytes).map_err(|_| format!("the subject {shown} is not a JSON or YAML document"))?;
    let text = doc.get(member).and_then(serde_yaml::Value::as_str).ok_or(format!("the subject {shown} has no member {member} that is text"))?;
    if text.trim().is_empty() {
        return Err(format!("the member {member} of {shown} is empty"));
    }
    Ok(text.to_owned())
}

/// The options of the results an action declares: each result is an option with the result as its letter.
fn declared_options(results: &[String]) -> Vec<Value> {
    results
        .iter()
        .map(|r| {
            let does = if r == "free" {
                "take the action with the result free; this option takes text: answer free: and the text, which goes with it as the reasons".to_owned()
            } else {
                format!("take the action with the result {r}")
            };
            json!({"letter": r, "does": does, "cost": "", "consequence": {"admits": [], "blocks": [], "answer": r}})
        })
        .collect()
}

/// The effect of `answer` on a request read from its case: the option it names (its letter alone, or
/// its letter, a colon and text that goes with it as the reasons), by the decision's kind and the
/// option's consequence; else the free answer, where the request takes one. A close and a handoff take
/// no reasons.
pub fn parse_option(request: &DecisionRequest, kind: DecisionKind, answer: &str) -> Result<Parsed, String> {
    let Some((option, text)) = crate::runtime::named_option(&request.options, answer) else {
        if request.artifact.as_ref().is_some_and(|a| a.free_answer) {
            return Ok(Parsed { reasons: Some(answer.to_owned()), ..Parsed::default() });
        }
        let letters: Vec<&str> = request.options.iter().filter_map(|o| o["letter"].as_str()).collect();
        return Err(format!("{answer:?} names no option of the request; the options are {}", letters.join(", ")));
    };
    let c = &option["consequence"];
    let reasons = text.map(str::to_owned);
    Ok(match kind {
        DecisionKind::Approve => Parsed { deny: c["grant"] != "granted", reasons, ..Parsed::default() },
        DecisionKind::ApproveAction => Parsed { result: c["answer"].as_str().map(str::to_owned), reasons, ..Parsed::default() },
        DecisionKind::Close => Parsed { deny: c["produces"] != "canon_decisions", ..Parsed::default() },
        DecisionKind::Handoff => Parsed::default(),
    })
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
#[cfg(test)]
pub fn plan(request_id: String, decision: &NextDecision, note: Option<&str>) -> Planned {
    plan_with(request_id, decision, note, &Found::default())
}

/// [`plan`], with what was read from the case: the request takes the artifact's question, options and
/// members where there is one, else the question member and the declared results where the bindings
/// name them, else it is the request [`plan`] builds.
pub fn plan_with(request_id: String, decision: &NextDecision, note: Option<&str>, found: &Found) -> Planned {
    let kind = decision.kind;
    let missing = match kind {
        DecisionKind::Approve if decision.capability.is_none() => Some("capability"),
        DecisionKind::ApproveAction if decision.action.is_none() => Some("action"),
        DecisionKind::Close if decision.outcome.is_none() => Some("outcome"),
        _ => None,
    };
    let unanswerable = missing.map(|m| format!("a {} decision without the member {m}: no library call can be chosen from its structured members (Loom 5af3f9c2, generated/rust/loom/src/tree.rs:78 and :102)", kind_name(kind)));
    let mut generated = decision.decides.clone();
    for (label, value) in [("subject", &decision.subject), ("revision", &decision.revision), ("path", &decision.path)] {
        if let Some(v) = value {
            generated.push_str(&format!("\n{label}: {v}"));
        }
    }
    let mut notes = String::new();
    if let Some(n) = note {
        notes.push_str(&format!("\nThe earlier answer was refused: {n}"));
    }
    if let Some(u) = &unanswerable {
        notes.push_str(&format!("\nThis request cannot be answered: {u}."));
    }
    let read: Option<(String, Vec<Value>, RequestArtifact)> = match (&found.contract, &found.declared) {
        (Some(c), Some(_)) if !c.options.is_empty() => Some((c.question.clone(), c.options.clone(), c.artifact.clone())),
        (Some(c), None) => Some((c.question.clone(), c.options.clone(), c.artifact.clone())),
        (contract, Some(results)) => {
            let question = found.question.clone().or_else(|| contract.as_ref().map(|c| c.question.clone())).unwrap_or_else(|| generated.clone());
            Some((question, declared_options(results), RequestArtifact { recommendation_strength: "none".into(), ..RequestArtifact::default() }))
        }
        (None, None) => None,
    };
    if let Some((question, options, artifact)) = read {
        let request = DecisionRequest {
            request_id,
            case_id: decision.case.clone(),
            action: decision.action.clone().unwrap_or_else(|| match kind {
                DecisionKind::Close => "close".to_string(),
                _ => "handoff".to_string(),
            }),
            capability: decision.capability.clone().unwrap_or_default(),
            question: question + &notes,
            options,
            artifact: Some(artifact),
        };
        return Planned { request, decision: decision.clone(), unanswerable, found: found.clone() };
    }
    let question = generated + &notes;
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
        artifact: None,
    };
    Planned { request, decision: decision.clone(), unanswerable, found: found.clone() }
}

/// An answer read.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Parsed {
    pub deny: bool,
    pub result: Option<String>,
    pub reasons: Option<String>,
    /// The reasons as a file of the workspace (the run loop writes a rejection's text there).
    pub reasons_file: Option<std::path::PathBuf>,
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
            principal: None,
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

    // ---- a request read from the case (story decision-options-come-from-a-case-artifact) ----

    use sha2::Digest;
    use std::path::PathBuf;

    /// A directory of the test's own, below the test binary's directory.
    fn dir(tag: &str) -> PathBuf {
        let d = std::env::current_exe().unwrap().parent().unwrap().join(format!("decision-unit-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A decision request in the contract's shape: three options, a recommendation and two grounds.
    fn contract(revision: &str) -> Value {
        json!({
            "request_id": "DR-c-a-1", "case_id": "c", "question": "Which draft goes out?",
            "subject_artifact": "draft", "subject_revision": revision,
            "unblocks": {"kind": "approval", "action": "a"},
            "options": [
                {"letter": "A", "does": "ship the draft", "cost": "none", "consequence": {"admits": ["a"], "blocks": []}},
                {"letter": "B", "does": "send it back", "cost": "one more round", "consequence": {"admits": [], "blocks": ["a"], "answer": "rejected"}},
                {"letter": "C", "does": "ship a short form", "cost": "a day", "consequence": {"admits": ["a"], "blocks": [], "answer": "short"}}
            ],
            "recommended_option": "B", "recommendation_strength": "moderate",
            "grounds": [
                {"source": "idea", "citation": {"form": "path_line", "value": {"path": "idea.yaml", "line": 2}}, "supports": "B", "how": "the idea asks for tea"},
                {"source": "policy", "citation": {"form": "record_id", "value": {"store": "sdc_decisions", "record_id": "DR-0"}}, "supports": "B", "how": "the last round sent it back"}
            ]
        })
    }

    /// A workspace with the subject `draft.txt`, a two-line `idea.yaml` and `artifact` as the case's artifact.
    fn workspace(tag: &str, artifact: &Value) -> PathBuf {
        let w = dir(tag);
        std::fs::write(w.join("idea.yaml"), "idea:\n  text: tea\n").unwrap();
        std::fs::write(w.join("draft.txt"), "draft\n").unwrap();
        std::fs::write(w.join(ARTIFACT_FILE), artifact.to_string()).unwrap();
        w
    }

    /// The person action's decision, its subject the file `name` of `w`.
    fn on(w: &Path, name: &str) -> NextDecision {
        let mut d = decision(DecisionKind::ApproveAction);
        d.path = Some(w.join(name).display().to_string());
        d
    }

    #[test]
    fn a_request_with_nothing_read_from_its_case_is_the_one_of_before_byte_for_byte() {
        let bytes = |kind| serde_json::to_string(&plan("r".into(), &decision(kind), None).request.options).unwrap();
        assert_eq!(bytes(DecisionKind::Approve), r#"[{"answer":"approve","effect":"allow the capability cap for case c"},{"answer":"deny","effect":"record a denial of that capability"}]"#);
        assert_eq!(
            bytes(DecisionKind::ApproveAction),
            r#"[{"answer":"approve","effect":"take the action with the result performed, as the principal who answers"},{"answer":"{\"result\":\"<result>\",\"reasons\":\"<text>\"}","effect":"take the action with a result its binding names and the reasons (the result alone may be given as the answer)"}]"#
        );
        assert_eq!(bytes(DecisionKind::Close), r#"[{"answer":"approve","effect":"close case c through done"},{"answer":"deny","effect":"decline this outcome"}]"#);
        assert_eq!(bytes(DecisionKind::Handoff), r#"[{"answer":"approve","effect":"the handoff text was done: run the tree again"}]"#);
        let p = plan("r".into(), &decision(DecisionKind::ApproveAction), None);
        assert_eq!(p.request.question, "a of case c\nsubject: draft\nrevision: r1");
        assert!(p.request.artifact.is_none());
        // A case with no artifact, under bindings that name no question member, reads nothing.
        let w = dir("nothing");
        std::fs::write(w.join("draft.txt"), "draft\n").unwrap();
        let found = read_case(&on(&w, "draft.txt"), &w, Some(&w)).unwrap();
        assert_eq!(found, Found::default());
        assert_eq!(plan_with("r".into(), &on(&w, "draft.txt"), None, &found).request, plan("r".into(), &on(&w, "draft.txt"), None).request);
    }

    #[test]
    fn a_case_artifact_gives_the_question_the_options_the_recommendation_and_the_grounds_and_none_of_the_fixed_lists() {
        let a = contract("r1");
        let w = workspace("contract", &a);
        let d = on(&w, "draft.txt");
        let found = read_case(&d, &w, None).unwrap();
        let p = plan_with("r".into(), &d, None, &found);
        assert_eq!(p.request.question, "Which draft goes out?");
        assert_eq!(p.request.options, a["options"].as_array().unwrap().clone());
        let art = p.request.artifact.clone().expect("the request carries the artifact's members");
        assert_eq!((art.recommended_option.as_deref(), art.recommended_answer.as_deref(), art.recommendation_strength.as_str(), art.free_answer), (Some("B"), None, "moderate", false));
        assert_eq!(art.grounds, a["grounds"].as_array().unwrap().clone());
        let text = serde_json::to_string(&p.request.options).unwrap();
        assert!(!text.contains("effect") && !text.contains("\"answer\":\"approve\""), "an option of the engine's own: {text}");
        // Asked again after a refusal: the same options and members, from what was read, not from the file.
        std::fs::remove_file(w.join(ARTIFACT_FILE)).unwrap();
        let again = plan_with("r2".into(), &d, Some("refused"), &p.found);
        assert_eq!((again.request.options.clone(), again.request.artifact.clone()), (p.request.options.clone(), p.request.artifact.clone()));
        assert!(again.request.question.starts_with("Which draft goes out?") && again.request.question.contains("refused"));
    }

    #[test]
    fn an_artifact_that_fails_the_check_is_refused_by_name() {
        let w = workspace("stale", &contract("r0"));
        let e = read_case(&on(&w, "draft.txt"), &w, None).unwrap_err();
        assert!(e.starts_with(REFUSED) && e.contains("subject_revision r0") && e.contains("r1"), "{e}");
        for (tag, citation, why) in [
            ("range", json!({"path": "idea.yaml", "line": 9}), "lines 9-9 outside idea.yaml (2 lines)"),
            ("nofile", json!({"path": "gone.yaml", "line": 1}), "no file gone.yaml"),
            ("leaves", json!({"path": "../idea.yaml", "line": 1}), "leaves the workspace"),
            ("digest", json!({"path": "idea.yaml", "line": 1, "digest": "sha256:00"}), "differ from the cited digest"),
        ] {
            let mut a = contract("r1");
            a["grounds"][0]["citation"]["value"] = citation;
            let w = workspace(tag, &a);
            let e = read_case(&on(&w, "draft.txt"), &w, None).unwrap_err();
            assert!(e.starts_with(REFUSED) && e.contains(why), "{tag}: {e}");
        }
        let mut a = contract("r1");
        let digest = format!("sha256:{:x}", sha2::Sha256::digest(b"  text: tea\n"));
        a["grounds"][0]["citation"]["value"] = json!({"path": "idea.yaml", "line": 2, "digest": digest});
        let w = workspace("digest-holds", &a);
        assert!(read_case(&on(&w, "draft.txt"), &w, None).is_ok(), "a digest of the cited lines that still holds passes");
        let mut a = contract("r1");
        a["options"][2]["letter"] = json!("A");
        let w = workspace("twice", &a);
        assert!(read_case(&on(&w, "draft.txt"), &w, None).unwrap_err().contains("letter A twice"));
        let w = workspace("not-json", &json!(null));
        std::fs::write(w.join(ARTIFACT_FILE), "{").unwrap();
        assert!(read_case(&on(&w, "draft.txt"), &w, None).unwrap_err().starts_with(REFUSED));
        let w = workspace("link", &json!(null));
        std::fs::remove_file(w.join(ARTIFACT_FILE)).unwrap();
        std::fs::write(w.join("elsewhere.json"), contract("r1").to_string()).unwrap();
        std::os::unix::fs::symlink(w.join("elsewhere.json"), w.join(ARTIFACT_FILE)).unwrap();
        assert!(read_case(&on(&w, "draft.txt"), &w, None).unwrap_err().starts_with(REFUSED), "a link at the artifact is not followed");
    }

    #[test]
    fn a_letter_takes_the_effect_of_its_option_by_the_kind_and_text_after_it_is_the_reasons() {
        let w = workspace("letters", &contract("r1"));
        let d = on(&w, "draft.txt");
        let r = plan_with("r".into(), &d, None, &read_case(&d, &w, None).unwrap()).request;
        let k = DecisionKind::ApproveAction;
        assert_eq!(parse_option(&r, k, "A").unwrap(), Parsed::default());
        assert_eq!(parse_option(&r, k, "B").unwrap(), Parsed { result: Some("rejected".into()), ..Parsed::default() });
        assert_eq!(parse_option(&r, k, "C").unwrap().result.as_deref(), Some("short"));
        assert_eq!(parse_option(&r, k, "B: add the tea").unwrap(), Parsed { result: Some("rejected".into()), reasons: Some("add the tea".into()), ..Parsed::default() });
        let refused = parse_option(&r, k, "approve").unwrap_err();
        assert!(refused.contains("A, B, C"), "{refused}");
        assert!(parse_option(&r, k, "use green tea").is_err());
        let opt = |letter: &str, consequence: Value| json!({"letter": letter, "does": "d", "cost": "c", "consequence": consequence});
        let mut q = r.clone();
        q.options = vec![opt("A", json!({"admits": [], "blocks": [], "grant": "granted"})), opt("B", json!({"admits": [], "blocks": [], "grant": "denied"})), opt("C", json!({"admits": [], "blocks": []}))];
        assert_eq!([parse_option(&q, DecisionKind::Approve, "A").unwrap().deny, parse_option(&q, DecisionKind::Approve, "B").unwrap().deny, parse_option(&q, DecisionKind::Approve, "C").unwrap().deny], [false, true, true]);
        q.options = vec![opt("A", json!({"admits": [], "blocks": [], "produces": "canon_decisions"})), opt("B", json!({"admits": [], "blocks": []}))];
        assert_eq!(parse_option(&q, DecisionKind::Close, "A: yes").unwrap(), Parsed::default(), "a close takes no reasons");
        assert!(parse_option(&q, DecisionKind::Close, "B").unwrap().deny);
        assert_eq!(parse_option(&q, DecisionKind::Handoff, "B").unwrap(), Parsed::default());
        q.artifact.as_mut().unwrap().free_answer = true;
        assert_eq!(parse_option(&q, k, "use green tea").unwrap(), Parsed { reasons: Some("use green tea".into()), ..Parsed::default() });
    }

    /// A bindings directory whose person action `a` declares the results A, B and `free`, with `questions`
    /// as its question file when given.
    fn bindings(tag: &str, questions: Option<&str>) -> PathBuf {
        let b = dir(tag);
        let text = "actions:\n  - action: a\n    executor: {kind: person, principal: x}\n    evidence:\n      - {kind: k, subject: ask, from: effect, results: {A: approved, B: rejected, free: approved}}\n";
        std::fs::write(b.join("a.yaml"), text).unwrap();
        if let Some(q) = questions {
            std::fs::write(b.join(QUESTIONS_FILE), q).unwrap();
        }
        b
    }

    #[test]
    fn where_the_bindings_name_a_question_member_the_question_is_its_text_and_the_options_are_the_declared_results() {
        let w = dir("declared-w");
        std::fs::write(w.join("ask.yaml"), "question: Which tea goes in?\nnote: x\n").unwrap();
        let d = on(&w, "ask.yaml");
        let named = format!(r#"{{"format": "{QUESTIONS_FORMAT}", "actions": {{"a": "question"}}}}"#);
        let found = read_case(&d, &w, Some(&bindings("declared-b", Some(&named)))).unwrap();
        let p = plan_with("r".into(), &d, None, &found);
        assert_eq!(p.request.question, "Which tea goes in?");
        let letters: Vec<&str> = p.request.options.iter().map(|o| o["letter"].as_str().unwrap()).collect();
        assert_eq!(letters, ["A", "B", "free"]);
        assert_eq!(p.request.options[1]["consequence"], json!({"admits": [], "blocks": [], "answer": "B"}));
        assert!(p.request.options[2]["does"].as_str().unwrap().contains("takes text"), "{}", p.request.options[2]);
        let art = p.request.artifact.clone().unwrap();
        assert_eq!((art.recommendation_strength.as_str(), art.grounds.len(), art.free_answer), ("none", 0, false));
        let r = &p.request;
        assert_eq!(parse_option(r, DecisionKind::ApproveAction, "B").unwrap(), Parsed { result: Some("B".into()), ..Parsed::default() });
        assert_eq!(parse_option(r, DecisionKind::ApproveAction, "free: green tea").unwrap(), Parsed { result: Some("free".into()), reasons: Some("green tea".into()), ..Parsed::default() });
        assert!(parse_option(r, DecisionKind::ApproveAction, "Z").is_err());
        // A contract artifact with options wins over the declared results; the two are never mixed.
        std::fs::write(w.join(ARTIFACT_FILE), contract("r1").to_string()).unwrap();
        std::fs::write(w.join("idea.yaml"), "idea:\n  text: tea\n").unwrap();
        let found = read_case(&d, &w, Some(&bindings("declared-b2", Some(&named)))).unwrap();
        let p = plan_with("r".into(), &d, None, &found);
        let letters: Vec<&str> = p.request.options.iter().map(|o| o["letter"].as_str().unwrap()).collect();
        assert_eq!((letters, p.request.question.as_str()), (vec!["A", "B", "C"], "Which draft goes out?"));
    }

    #[test]
    fn a_question_member_that_gives_no_text_leaves_the_question_of_before_and_says_why() {
        let w = dir("unread-w");
        std::fs::write(w.join("ask.yaml"), "question: \"\"\n").unwrap();
        let named = format!(r#"{{"format": "{QUESTIONS_FORMAT}", "actions": {{"a": "question"}}}}"#);
        let b = bindings("unread-b", Some(&named));
        for (subject, why) in [("ask.yaml", "is empty"), ("gone.yaml", "gone.yaml")] {
            let d = on(&w, subject);
            let found = read_case(&d, &w, Some(&b)).unwrap();
            assert!(found.unread.as_deref().is_some_and(|u| u.contains(why)), "{subject}: {found:?}");
            let p = plan_with("r".into(), &d, None, &found);
            assert!(p.request.question.starts_with("a of case c\n"), "{}", p.request.question);
            assert_eq!(p.request.options.len(), 3, "the declared results stay the options");
        }
        // Bindings that name no member for the action read nothing of the subject.
        let other = format!(r#"{{"format": "{QUESTIONS_FORMAT}", "actions": {{"b": "question"}}}}"#);
        assert_eq!(read_case(&on(&w, "ask.yaml"), &w, Some(&bindings("unread-b2", Some(&other)))).unwrap(), Found::default());
        // A member name that is empty or not a text is recorded the same way, and the run goes on.
        for (tag, member) in [("unread-b3", "\"\""), ("unread-b4", "3"), ("unread-b5", "\"  \"")] {
            let q = format!(r#"{{"format": "{QUESTIONS_FORMAT}", "actions": {{"a": {member}}}}}"#);
            let found = read_case(&on(&w, "ask.yaml"), &w, Some(&bindings(tag, Some(&q)))).unwrap();
            assert!(found.unread.as_deref().is_some_and(|u| u.contains("not a text of a name")) && found.declared.is_none(), "{member}: {found:?}");
            assert!(plan_with("r".into(), &on(&w, "ask.yaml"), None, &found).request.artifact.is_none(), "{member}: the request of before");
        }
    }

    /// Adversary pass on f26206e3: no request is opened that no answer can take, no citation resolves through a
    /// link, an option never performs an action it blocks, and `free` takes text only.
    #[test]
    fn no_request_opens_that_no_answer_can_take_and_no_option_performs_what_it_blocks() {
        // Fewer than two options: refused by the contract's cause; none is taken for the question alone where the
        // declared results give the options.
        for (tag, n) in [("few-0", 0), ("few-1", 1)] {
            let mut a = contract("r1");
            let cut = a["options"].as_array().unwrap()[..n].to_vec();
            a["options"] = json!(cut);
            a.as_object_mut().unwrap().remove("recommended_option");
            let w = workspace(tag, &a);
            let e = read_case(&on(&w, "draft.txt"), &w, None).unwrap_err();
            assert!(e.starts_with(REFUSED) && e.contains("too-few-options"), "{tag}: {e}");
        }
        let mut a = contract("r1");
        a["options"] = json!([]);
        a.as_object_mut().unwrap().remove("recommended_option");
        let w = workspace("few-declared", &a);
        std::fs::write(w.join("draft.txt"), "question: Which tea goes in?\n").unwrap();
        let mut d = on(&w, "draft.txt");
        d.revision = Some("r1".into());
        let named = format!(r#"{{"format": "{QUESTIONS_FORMAT}", "actions": {{"a": "question"}}}}"#);
        let found = read_case(&d, &w, Some(&bindings("few-declared-b", Some(&named)))).unwrap();
        assert_eq!(plan_with("r".into(), &d, None, &found).request.options.len(), 3, "the declared results give the options");
        // An action that declares no result: the request of before, and the cause recorded.
        let b = dir("no-result-b");
        std::fs::write(b.join("a.yaml"), "actions:\n  - action: a\n    executor: {kind: person, principal: x}\n").unwrap();
        std::fs::write(b.join(QUESTIONS_FILE), &named).unwrap();
        let w = dir("no-result-w");
        std::fs::write(w.join("ask.yaml"), "question: Which tea goes in?\n").unwrap();
        let found = read_case(&on(&w, "ask.yaml"), &w, Some(&b)).unwrap();
        assert!(found.declared.is_none() && found.unread.as_deref().is_some_and(|u| u.contains("declares no result")), "{found:?}");
        assert!(plan_with("r".into(), &on(&w, "ask.yaml"), None, &found).request.artifact.is_none());
        // A citation below a directory link resolves nowhere, whatever the link names.
        let outside = dir("link-outside");
        std::fs::write(outside.join("secret.txt"), "secret\n").unwrap();
        let mut a = contract("r1");
        a["grounds"][0]["citation"]["value"] = json!({"path": "out/secret.txt", "line": 1});
        let w = workspace("dir-link", &a);
        std::os::unix::fs::symlink(&outside, w.join("out")).unwrap();
        let e = read_case(&on(&w, "draft.txt"), &w, None).unwrap_err();
        assert!(e.starts_with(REFUSED) && e.contains("no file out/secret.txt below the workspace"), "{e}");
        // A subject reached through a directory link reads no artifact from there.
        let elsewhere = dir("link-subject");
        std::fs::write(elsewhere.join("draft.txt"), "draft\n").unwrap();
        std::fs::write(elsewhere.join(ARTIFACT_FILE), contract("r1").to_string()).unwrap();
        let w = dir("link-subject-w");
        std::os::unix::fs::symlink(&elsewhere, w.join("in")).unwrap();
        assert_eq!(read_case(&on(&w, "in/draft.txt"), &w, None).unwrap(), Found::default());
        // An option of an action's approval that blocks the action and names no answer would perform it.
        let mut a = contract("r1");
        a["options"][1]["consequence"] = json!({"admits": [], "blocks": ["a"]});
        let w = workspace("blocks", &a);
        let e = read_case(&on(&w, "draft.txt"), &w, None).unwrap_err();
        assert!(e.starts_with(REFUSED) && e.contains("option B blocks a and names no answer"), "{e}");
        // `free` takes text: its letter alone, or with nothing after the colon, names no option.
        let opts = declared_options(&["A".into(), "free".into()]);
        assert!(crate::runtime::named_option(&opts, "free").is_none() && crate::runtime::named_option(&opts, "free:  ").is_none());
        assert_eq!(crate::runtime::named_option(&opts, "free: green tea").map(|(o, t)| (o["letter"].clone(), t)), Some((json!("free"), Some("green tea"))));
        assert!(crate::runtime::named_option(&opts, "A").is_some());
    }
}
