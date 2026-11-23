//! The shape an answer of a decider takes for Loom, in one place for `runbook run` and the daemon's
//! engine: the results a person action declares, the result a decider's verdict names, and the text of
//! a rejection.
//!
//! `loom_run::run_loop` calls these for the run it makes in its own process; the binary's arbiter
//! (`loom_run::arbiter`) calls the same ones for a run of the daemon.

use serde_json::Value;
use std::path::{Path, PathBuf};

/// Why a verdict cannot take the shape Loom needs.
#[derive(Debug, PartialEq, Eq)]
pub enum Unshaped {
    /// A rejection nothing can record: the bindings declare no result for it and no step reworks the
    /// rejected revision. The words are the reason of the stopped run.
    Rejected(String),
    /// The verdict names a result the bindings do not declare for the action: Loom would refuse it.
    Failed(String),
}

/// What a verdict becomes: the result to name, whether it is a denial, and the rejection's reasons.
#[derive(Debug, PartialEq, Eq, Default)]
pub struct Shaped {
    /// The result of a person action: `performed` or `rejected`.
    pub result: Option<&'static str>,
    /// A decision with no action (the approval of a capability) takes no result: its rejection is a denial.
    pub deny: bool,
    /// The text of a rejection, as the decider gave it (not yet cleaned, see [`clean_reasons`]).
    pub reasons: Option<String>,
}

/// The results a person action declares, from the bindings copy in `bindings_dir`: the keys of
/// `results` that every `from: effect` entry of the action lists. `None`: the copy names no such action.
pub fn declared_results(bindings_dir: &Path, action: &str) -> Result<Option<Vec<String>>, String> {
    let mut declared: Option<Vec<String>> = None;
    let entries = std::fs::read_dir(bindings_dir).map_err(|e| format!("{}: {e}", bindings_dir.display()))?;
    let mut files: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "yaml")).collect();
    files.sort();
    for file in files {
        let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let doc: serde_yaml::Value = serde_yaml::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
        let Some(actions) = doc.get("actions").and_then(serde_yaml::Value::as_sequence) else { continue };
        for entry in actions.iter().filter(|a| a.get("action").and_then(serde_yaml::Value::as_str) == Some(action)) {
            if entry.get("executor").and_then(|e| e.get("kind")).and_then(serde_yaml::Value::as_str) != Some("person") {
                continue;
            }
            let evidence = entry.get("evidence").and_then(serde_yaml::Value::as_sequence).cloned().unwrap_or_default();
            for item in evidence.iter().filter(|i| i.get("from").and_then(serde_yaml::Value::as_str) == Some("effect")) {
                let keys: Vec<String> = item
                    .get("results")
                    .and_then(serde_yaml::Value::as_mapping)
                    .map(|m| m.keys().filter_map(|k| k.as_str().map(str::to_owned)).collect())
                    .unwrap_or_default();
                declared = Some(match declared {
                    None => keys,
                    Some(have) => have.into_iter().filter(|k| keys.contains(k)).collect(),
                });
            }
            declared.get_or_insert_with(Vec::new);
        }
    }
    Ok(declared)
}

/// Names the result of a person action: `performed` for an approval, `rejected` for a rejection, where the
/// action declares results. A result the action does not declare is refused before Loom is called. A decision
/// with no action (an approval of a capability) takes no result; its rejection is a denial.
///
/// `kind` is the decision's kind as Loom prints it, `who` the principal named in the refusal text.
pub fn person_result(bindings_dir: &Path, decision: &Value, kind: &str, approve: bool, who: Option<&str>, reasons: Option<&str>) -> Result<Shaped, Unshaped> {
    let action = decision.get("action").and_then(Value::as_str).filter(|a| !a.is_empty());
    let Some(action) = action.filter(|_| kind == "ApproveAction") else {
        return Ok(Shaped { result: None, deny: !approve, reasons: None });
    };
    let declared = declared_results(bindings_dir, action).map_err(Unshaped::Failed)?.unwrap_or_default();
    let result = if approve { "performed" } else { "rejected" };
    if !declared.iter().any(|k| k == result) {
        if approve && declared.is_empty() {
            return Ok(Shaped::default());
        }
        let listed = if declared.is_empty() { "none".to_owned() } else { declared.join(", ") };
        if !approve {
            // A rejection nothing can record: no rework path.
            let case = decision.get("case").and_then(Value::as_str).unwrap_or("");
            let who = who.unwrap_or("the decider");
            let subject = decision.get("subject").and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or("it");
            let revision = decision.get("revision").and_then(Value::as_str).unwrap_or("");
            let said = reasons.map(|r| r.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|r| !r.is_empty()).unwrap_or_else(|| "no reasons given".to_owned());
            let said: String = said.chars().take(400).collect();
            return Err(Unshaped::Rejected(format!(
                "{action} in {case}: {who} rejected {subject} {revision} ({said}); rejected_without_rework_path: the bindings declare the results {listed} for the action, so nothing records a rejection and no step reworks the rejected revision; it waits on a protocol path for a rejected revision"
            )));
        }
        return Err(Unshaped::Failed(format!("the decider answered {result} for action {action}, but the bindings of the run declare the results {listed} for it: Loom would refuse the result")));
    }
    let wants_reasons = !(approve && !reasons.is_some_and(|r| !r.trim().is_empty()));
    Ok(Shaped { result: Some(result), deny: false, reasons: wants_reasons.then(|| reasons.unwrap_or("").to_owned()) })
}

/// The reasons of a rejection as Loom may be given them (a regular file below the workspace is read: at most
/// 60 lines and 65536 bytes): the login file's path, the run directory and the home directory taken out, the
/// lines and the bytes cut, an empty text replaced.
pub fn clean_reasons(reasons: Option<&str>, login: Option<&str>, run_dir: &Path, home: Option<&str>) -> String {
    let mut text = reasons.unwrap_or("").to_owned();
    if let Some(login) = login.filter(|l| !l.is_empty()) {
        text = text.replace(login, "<login file>");
    }
    text = text.replace(&run_dir.display().to_string(), "<run>");
    if let Some(home) = home.filter(|h| h.len() > 1) {
        text = text.replace(home, "~");
    }
    let mut kept: Vec<&str> = text.lines().take(60).collect();
    while kept.last().is_some_and(|l| l.trim().is_empty()) {
        kept.pop();
    }
    let mut text = kept.join("\n");
    if text.len() > 65_000 {
        let mut end = 65_000;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    if text.trim().is_empty() {
        text = "The decider rejected this and gave no reasons.".to_owned();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bindings(tag: &str, results: &str) -> PathBuf {
        let dir = std::env::current_exe().unwrap().parent().unwrap().join(format!("shape-unit-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let text = format!("actions:\n  - action: review\n    executor: {{kind: person, principal: x}}\n    evidence:\n      - {{kind: k, from: effect, results: {results}}}\n");
        std::fs::write(dir.join("a.yaml"), text).unwrap();
        dir
    }

    fn decision(kind: &str) -> Value {
        json!({"case": "c", "kind": kind, "action": "review", "subject": "doc", "revision": "r1"})
    }

    #[test]
    fn a_person_action_takes_the_result_its_bindings_declare() {
        let dir = bindings("both", "{performed: ok, rejected: no}");
        assert_eq!(person_result(&dir, &decision("ApproveAction"), "ApproveAction", true, None, None), Ok(Shaped { result: Some("performed"), deny: false, reasons: None }));
        let rejected = person_result(&dir, &decision("ApproveAction"), "ApproveAction", false, None, Some("too long")).unwrap();
        assert_eq!((rejected.result, rejected.reasons.as_deref()), (Some("rejected"), Some("too long")));
    }

    #[test]
    fn a_capability_approval_takes_no_result_and_its_rejection_is_a_denial() {
        let dir = bindings("cap", "{performed: ok}");
        assert_eq!(person_result(&dir, &decision("Approve"), "Approve", true, None, None), Ok(Shaped::default()));
        assert_eq!(person_result(&dir, &decision("Approve"), "Approve", false, None, None), Ok(Shaped { result: None, deny: true, reasons: None }));
    }

    #[test]
    fn a_rejection_nothing_records_names_why_and_a_result_nobody_declares_is_refused() {
        let dir = bindings("one", "{performed: ok}");
        let refused = person_result(&dir, &decision("ApproveAction"), "ApproveAction", false, Some("decider:x"), Some("no")).unwrap_err();
        assert!(matches!(&refused, Unshaped::Rejected(t) if t.contains("rejected_without_rework_path") && t.contains("decider:x rejected doc r1")), "{refused:?}");
        let dir = bindings("other", "{done: ok}");
        let refused = person_result(&dir, &decision("ApproveAction"), "ApproveAction", true, None, None).unwrap_err();
        assert!(matches!(&refused, Unshaped::Failed(t) if t.contains("declare the results done")), "{refused:?}");
    }

    #[test]
    fn reasons_lose_the_paths_that_are_not_for_the_record_and_are_never_empty() {
        let run = Path::new("/state/run-1");
        let cleaned = clean_reasons(Some("see /state/run-1/x and /secret/login and /srv/user/file"), Some("/secret/login"), run, Some("/srv/user"));
        assert_eq!(cleaned, "see <run>/x and <login file> and ~/file");
        assert_eq!(clean_reasons(None, None, run, None), "The decider rejected this and gave no reasons.");
        assert_eq!(clean_reasons(Some(&"x\n".repeat(100)), None, run, None).lines().count(), 60);
    }
}
