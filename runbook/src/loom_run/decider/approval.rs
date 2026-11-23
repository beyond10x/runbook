//! What a decision entry asks for and the files around one call: the approval read from Loom's
//! entry, the case directory the decider reads (`case.json` and a link `work`), the numbering of
//! the call directories and the request the decider is given.

use super::*;

/// The approval a decision entry asks for, read with an explicit path for every absent member.
pub(super) struct Approval {
    pub(super) case: String,
    pub(super) action: String,
    pub(super) capability: Option<String>,
    pub(super) subject: String,
    pub(super) revision: String,
    pub(super) path: Option<String>,
    /// The outcome of a `Close` entry; the decision is then a close, not an approval.
    pub(super) close_outcome: Option<String>,
}

pub(super) fn text_member<'a>(decision: &'a Value, name: &str) -> Option<&'a str> {
    decision.get(name).and_then(Value::as_str).filter(|s| !s.is_empty())
}

pub(super) fn plain_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains('/') && !name.contains('\0')
}

pub(super) fn read_approval(request: &DecisionRequest<'_>, bindings: Option<&Path>) -> Result<Approval, String> {
    let decision = request.decision;
    let kind = text_member(decision, "kind").ok_or("the decision has no kind")?;
    if !APPROVAL_KINDS.contains(&kind) {
        return Err(match kind {
            "Handoff" => "kind Handoff: the decider has no reply for a writer's handoff; the printed `tree run` changes nothing until the work the handoff names is done".to_owned(),
            other => format!("kind {other} not known"),
        });
    }
    let case = text_member(decision, "case").ok_or("the decision names no case")?.to_owned();
    if !plain_name(&case) {
        return Err("the case name is not a plain name".to_owned());
    }
    if kind == "Close" {
        let outcome = text_member(decision, "outcome").ok_or("the Close decision names no outcome (member `outcome` absent)")?.to_owned();
        return Ok(Approval { case, action: "close".to_owned(), capability: None, subject: String::new(), revision: String::new(), path: None, close_outcome: Some(outcome) });
    }
    let action = text_member(decision, "action").ok_or("the decision names no action (member `action` absent)")?.to_owned();
    if bindings.is_some_and(|dir| answers::is_answer_step_in(dir, &action)) {
        return Err(format!(
            "{action} is a step whose outcome is a person's text, not a recorded result: Loom's approval of it carries no answer, and the answers file of the bindings ({}) declares nothing for it",
            answers::FILE
        ));
    }
    let subject = text_member(decision, "subject").ok_or("the decision names no subject (member `subject` absent): the decider cannot say what is decided")?.to_owned();
    let revision = text_member(decision, "revision").ok_or("the decision names no revision (member `revision` absent): the decider cannot say which revision is decided")?.to_owned();
    let path = match text_member(decision, "path") {
        None => None,
        Some(path) => {
            let relative = Path::new(path)
                .strip_prefix(request.workspace)
                .map_err(|_| "the decision's path is not inside the workspace".to_owned())?;
            if relative.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
                return Err("the decision's path has a segment that is not a plain name".to_owned());
            }
            Some(relative.to_string_lossy().into_owned())
        }
    };
    Ok(Approval { case, action, capability: text_member(decision, "capability").map(str::to_owned), subject, revision, path, close_outcome: None })
}

pub(super) fn hex(text: &str) -> String {
    text.bytes().map(|b| format!("{b:02x}")).collect()
}

/// The artifacts of a case with their revisions, from Loom's case file (`case_file` of
/// `tree status`: `<store>/case-<hex of the case id>.json`, store named by `--store` of the
/// decision's `command`). Empty when the file cannot be read.
pub(super) fn case_artifacts(decision: &Value, case: &str) -> Vec<(String, String)> {
    let words: Vec<&str> = decision.get("command").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let store = match words.iter().position(|w| *w == "--store").and_then(|i| words.get(i + 1)) {
        Some(store) => PathBuf::from(store),
        None => return Vec::new(),
    };
    let file = store.join(format!("case-{}.json", hex(case)));
    let document: Value = match std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()) {
        Some(document) => document,
        None => return Vec::new(),
    };
    document
        .get("artifacts")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| Some((item.get("artifact")?.as_str()?.to_owned(), item.get("revision")?.as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default()
}

/// The directory the decider takes as `case_dir`: `case.json` (`id` from the decision's `case`,
/// `artifacts` from Loom's case file and the decision's own subject) and `work` (a link to the
/// workspace). Under `<run dir>/decider-cases/<case>/`.
pub(super) fn make_case_dir(request: &DecisionRequest<'_>, approval: &Approval) -> Result<PathBuf, String> {
    let dir = request.run_dir.join("decider-cases").join(&approval.case);
    std::fs::create_dir_all(&dir).map_err(|e| format!("decider case directory: {e}"))?;
    let mut artifacts = Map::new();
    for (name, revision) in case_artifacts(request.decision, &approval.case) {
        artifacts.insert(name, json!({ "revision": revision }));
    }
    if !approval.subject.is_empty() {
        artifacts.insert(approval.subject.clone(), json!({ "revision": approval.revision }));
    }
    let case = json!({"id": approval.case, "artifacts": artifacts});
    std::fs::write(dir.join("case.json"), format!("{case}\n")).map_err(|e| format!("decider case file: {e}"))?;
    // The file the reply's address cites (`<artifact>@<revision>#<entry>`) resolve through: the
    // decision's subject and its path are the one artifact the runner knows a file of.
    let bindings = dir.join("bindings.json");
    match (approval.subject.is_empty(), &approval.path) {
        (false, Some(path)) => std::fs::write(bindings, format!("{}\n", json!({"$artifacts": {approval.subject.clone(): path}}))).map_err(|e| format!("decider case file: {e}"))?,
        _ => {
            let _ = std::fs::remove_file(bindings);
        }
    }
    let work = dir.join("work");
    if std::fs::symlink_metadata(&work).is_ok() {
        std::fs::remove_file(&work).map_err(|e| format!("decider case directory: {e}"))?;
    }
    std::os::unix::fs::symlink(request.workspace, &work).map_err(|e| format!("decider case directory: {e}"))?;
    Ok(dir)
}

pub(super) fn record_dirs(base: &Path) -> u64 {
    std::fs::read_dir(base)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false) && e.file_name() != "rule")
                .count() as u64
        })
        .unwrap_or(0)
}

pub(super) fn safe_part(text: &str) -> String {
    text.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' }).collect()
}

pub(super) fn usd_text(micro: u64) -> String {
    format!("{}.{:06}", micro / 1_000_000, micro % 1_000_000)
}

pub(super) fn cost_micro(reply: &Value) -> Option<u64> {
    let usd = reply.get("cost_usd")?.as_f64()?;
    (usd.is_finite() && usd >= 0.0).then(|| (usd * 1_000_000.0).round() as u64)
}

/// The request the decider reads, built from the decision entry: `type` approval, the case directory, the subject, and
/// an empty `position` (Loom's output holds no claim values).
pub(super) fn decide_request(approval: &Approval, case_dir: &Path, record_dir: &Path, retry: Option<(u64, String)>) -> Value {
    let mut request = json!({
        "type": "approval",
        "case_dir": case_dir.display().to_string(),
        "action": approval.action,
        "capability": approval.capability,
        "subject": approval.subject,
        "path": approval.path,
        "revision": approval.revision,
        "position": {"claims": {}, "open_obligations": [], "legitimate_outcomes": []},
        "record_dir": record_dir.display().to_string(),
    });
    if let Some(outcome) = &approval.close_outcome {
        request["type"] = json!("close");
        request["outcome"] = json!(outcome);
        for member in ["action", "capability", "subject", "path", "revision"] {
            request[member] = Value::Null;
        }
    }
    if let Some((attempt, why)) = retry {
        request["retry"] = json!({"attempt": attempt, "invalid_reason": why});
    }
    request
}

/// The decider's reasons of a reply: the `reasons` text members joined by line breaks.
pub(super) fn reply_reasons(reply: &Value) -> Option<String> {
    let items = reply.get("reasons").and_then(Value::as_array)?;
    let text = items.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n");
    (!text.trim().is_empty()).then_some(text)
}
