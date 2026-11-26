//! sdc-answers: the two programs `bindings/tools/answers.yaml` names for `runbook run`, which
//! put the decider's answer where the removed driver put it: a scope-changing
//! question (`answer_questions`, lines 1122 to 1148) and a finding (`decide_findings`, lines 1150
//! to 1227). They run in the workspace of the case, as Loom starts a bound program.
//!
//!   sdc-answers requests questions|findings
//!   sdc-answers apply question|finding <id> <answer> <principal>
//!
//! `requests` prints one JSON object per line for the decider (`type`, `id`, the object itself,
//! `subject`, `path`, `capability`, and for a finding the `refusal` of a decision the program check
//! refused). `apply` writes one answer as a person's is written: a question gets `answer`,
//! `status` (`accepted` for its default, else `changed`) and `answered_by`; a finding gets
//! `decision` and `decided_by`, and loses its `refused_decision`. Exit 0; 1 with the reason on
//! standard error when a file cannot be read or written, an id is not in it, or a finding was
//! raised again after the decision on it was applied (the driver stops there: `finding_reraised`).

use std::fs;
use std::process::ExitCode;

use serde_json::{json, Value as Json};
use serde_yaml::Value;

const QUESTIONS: &str = "questions.yaml";
const FINDINGS: &str = "findings.yaml";

fn load(file: &str) -> Result<Value, String> {
    let text = fs::read_to_string(file).map_err(|e| format!("{file}: {}", e.kind()))?;
    serde_yaml::from_str(&text).map_err(|e| format!("{file}: {e}"))
}

fn save(file: &str, doc: &Value) -> Result<(), String> {
    let text = serde_yaml::to_string(doc).map_err(|e| format!("{file}: {e}"))?;
    let tmp = format!("{file}.tmp");
    fs::write(&tmp, text).map_err(|e| format!("{tmp}: {}", e.kind()))?;
    fs::rename(&tmp, file).map_err(|e| format!("{file}: {}", e.kind()))
}

fn list<'a>(doc: &'a Value, key: &str, file: &str) -> Result<&'a Vec<Value>, String> {
    doc.get(key).and_then(Value::as_sequence).ok_or_else(|| format!("{file}: no list `{key}`"))
}

fn text<'a>(item: &'a Value, key: &str) -> &'a str {
    item.get(key).and_then(Value::as_str).unwrap_or("")
}

fn json(value: &Value) -> Result<Json, String> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

/// An open scope-changing question, or one answered with no principal that has the right to give
/// the answer (`answered_by` empty or `non-interactive`).
fn question_waits(q: &Value) -> bool {
    q.get("scope_changing").and_then(Value::as_bool) == Some(true) && (text(q, "status") == "open" || matches!(text(q, "answered_by"), "" | "non-interactive"))
}

fn requests(what: &str) -> Result<Vec<Json>, String> {
    let mut out = Vec::new();
    match what {
        "questions" => {
            let doc = load(QUESTIONS)?;
            for q in list(&doc, "questions", QUESTIONS)?.iter().filter(|q| question_waits(q)) {
                out.push(json!({"type": "question", "id": text(q, "id"), "question": json(q)?, "subject": "questions", "path": QUESTIONS, "capability": "scope.answer"}));
            }
        }
        "findings" => {
            let doc = load(FINDINGS)?;
            for f in list(&doc, "findings", FINDINGS)?.iter().filter(|f| text(f, "status") == "needs_person" && text(f, "decision").is_empty()) {
                let id = text(f, "id");
                if let Some(raised) = f.get("reraised").filter(|r| !r.is_null()) {
                    return Err(format!(
                        "{id} raised again after {}'s decision \"{}\" was applied ({}): the apply of that decision did not take effect",
                        text(raised, "decided_by"),
                        text(raised, "decision"),
                        text(raised, "says")
                    ));
                }
                let mut finding = f.clone();
                let mut line = json!({"type": "finding", "id": id, "subject": "findings", "path": FINDINGS, "capability": null});
                if let Some(refused) = f.get("refused_decision").filter(|r| !r.is_null()) {
                    // Only a decider's refused decision is put to the decider again; a person's waits for the person.
                    if !text(refused, "decided_by").starts_with("decider:") {
                        continue;
                    }
                    line["refusal"] = json(refused)?;
                    if let Some(map) = finding.as_mapping_mut() {
                        map.remove("refused_decision");
                    }
                }
                line["finding"] = json(&finding)?;
                out.push(line);
            }
        }
        other => return Err(format!("requests: questions or findings, not {other}")),
    }
    Ok(out)
}

fn put(item: &mut Value, key: &str, value: &str) {
    if let Some(map) = item.as_mapping_mut() {
        map.insert(Value::String(key.to_owned()), Value::String(value.to_owned()));
    }
}

fn apply(what: &str, id: &str, answer: &str, principal: &str) -> Result<(), String> {
    let (file, key) = match what {
        "question" => (QUESTIONS, "questions"),
        "finding" => (FINDINGS, "findings"),
        other => return Err(format!("apply: question or finding, not {other}")),
    };
    let mut doc = load(file)?;
    let items = doc.get_mut(key).and_then(Value::as_sequence_mut).ok_or_else(|| format!("{file}: no list `{key}`"))?;
    let item = items.iter_mut().find(|i| text(i, "id") == id).ok_or_else(|| format!("{file}: no {what} {id}"))?;
    if what == "question" {
        let status = if item.get("default").and_then(Value::as_str) == Some(answer) { "accepted" } else { "changed" };
        put(item, "answer", answer);
        put(item, "status", status);
        put(item, "answered_by", principal);
    } else {
        put(item, "decision", answer);
        put(item, "decided_by", principal);
        if let Some(map) = item.as_mapping_mut() {
            map.remove("refused_decision");
        }
    }
    save(file, &doc)
}

fn run(args: &[String]) -> Result<(), String> {
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["requests", what] => {
            for line in requests(what)? {
                println!("{line}");
            }
            Ok(())
        }
        ["apply", what, id, answer, principal] => apply(what, id, answer, principal),
        _ => Err("usage: sdc-answers requests questions|findings | sdc-answers apply question|finding <id> <answer> <principal>".to_owned()),
    }
}

fn main() -> ExitCode {
    sdc_tools::answer_version("sdc-answers");
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("sdc-answers: {why}");
            ExitCode::from(1)
        }
    }
}
