//! The calls into Loom's library that both runtimes make: the daemon's engine (`engine::Engine`)
//! and `runbook run --in-process` (`loom_run::library`). This is the one place that names
//! `open_tree`, `run_tree`, `approve`, `close`, `tree_status` and `verify_tree`; neither caller
//! has a second copy of what is done with them.

use super::decision::Parsed;
use crate::runtime::Confinement;
use b10x_loom_tree::model::{ApprovalDecision, DecisionKind, NextDecision, TreeStop};
use b10x_loom_tree::{
    approve, close, open_tree, read_protocols_directory, run_tree, status_json, stored_catalog, tree_status, verify_json, verify_tree, ApproveRequest, CloseRequest,
    ConfinementAdapter, EngineConfinement, EngineRunner, OpenRequest, Reasons, RunSettings, TestRunner, TreeTime, UNCONFINED_FLAG_REASON,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Whether the process sits in a cgroup whose subtree it may write: the delegation the `linux`
/// level needs. A check that fails closed; Substrate's own probe still decides at the first
/// program.
pub fn linux_delegated() -> bool {
    let Ok(text) = std::fs::read_to_string("/proc/self/cgroup") else { return false };
    let Some(path) = text.lines().find_map(|l| l.strip_prefix("0::")) else { return false };
    let dir = Path::new("/sys/fs/cgroup").join(path.trim_start_matches('/'));
    std::fs::OpenOptions::new().write(true).open(dir.join("cgroup.subtree_control")).is_ok()
}

/// The levels this host can give: `local`, and `linux` where the process is in a delegated cgroup.
pub fn host_levels() -> Vec<Confinement> {
    let mut levels = vec![Confinement::Local];
    if linux_delegated() {
        levels.push(Confinement::Linux);
    }
    levels
}

/// The runner every bound program of a tree starts through, at `level`; the refusal names the level.
pub fn runner(level: Confinement) -> Result<Arc<dyn TestRunner>, String> {
    let confinement = match level {
        Confinement::Local => EngineConfinement { adapter: ConfinementAdapter::Local, local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()), cgroup_root: None },
        Confinement::Linux => EngineConfinement { adapter: ConfinementAdapter::Linux, local_reason: None, cgroup_root: None },
    };
    EngineRunner::open(&confinement).map(|r| Arc::new(r) as Arc<dyn TestRunner>).map_err(|e| format!("ConfinementRefused: the confinement level {} is refused: {e}", level.as_str()))
}

/// The `--confinement` value of `tree run` a level is.
pub fn confinement_flag(level: Confinement) -> &'static str {
    match level {
        Confinement::Local => "local",
        Confinement::Linux => "substrate",
    }
}

/// How a run goes: at most `max_steps` steps, no wall bound (the callers bound the wall themselves).
pub fn run_settings(runner: Arc<dyn TestRunner>, level: Confinement, max_steps: Option<i64>) -> RunSettings {
    RunSettings { max_steps, max_wall_seconds: None, time: TreeTime::Host, runner, confinement: Some(confinement_flag(level).to_owned()), cgroup_root: None }
}

/// Which protocol of the directory is the root.
pub enum Root<'a> {
    /// The protocol with this id at this revision.
    Protocol { id: &'a str, revision: i64 },
    /// The registration `<id with dots as dashes>@<revision>`.
    Registration(&'a str),
}

/// A tree to open.
pub struct Opening<'a> {
    pub store: &'a Path,
    pub workspace: &'a Path,
    /// The directory of the protocol files; every `*.protocol.yaml` in it is kept in the store.
    pub protocols: &'a Path,
    pub root: Root<'a>,
    /// The root's bindings file.
    pub bindings: &'a Path,
    /// The id of the root case.
    pub case: &'a str,
}

/// Reads the protocol directory, finds the root protocol (a refusal before anything is written),
/// runs `before_open` (the caller's own writes, such as the idea file) and opens the tree.
pub fn open(o: &Opening<'_>, before_open: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
    let read = read_protocols_directory(o.protocols).map_err(|e| e.to_string())?;
    let found = read.kept.iter().find(|k| match &o.root {
        Root::Protocol { id, revision } => k.protocol.protocol == *id && k.protocol.revision == *revision,
        Root::Registration(name) => k.protocol.registration == *name,
    });
    let registration = match (found, &o.root) {
        (Some(k), _) => k.protocol.registration.clone(),
        (None, Root::Protocol { id, revision }) => return Err(format!("the protocol directory registers no {id}@{revision}")),
        (None, Root::Registration(name)) => return Err(format!("the protocol directory registers no {name}")),
    };
    before_open()?;
    open_tree(&OpenRequest {
        store: o.store.to_path_buf(),
        workspace: o.workspace.to_path_buf(),
        protocol: registration,
        bindings: o.bindings.to_path_buf(),
        revisions: Vec::new(),
        case: o.case.to_owned(),
        time: TreeTime::Host,
        catalog: &read.catalog,
        protocols: read.kept,
    })
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Loom's reader of a protocol directory, as [`open`] reads it, and the root protocol `id`@`revision` in it: the loader's
/// refusal, before anything is written.
pub fn check_protocols(protocols: &Path, id: &str, revision: i64) -> Result<(), String> {
    let read = read_protocols_directory(protocols).map_err(|e| e.to_string())?;
    if read.kept.iter().any(|k| k.protocol.protocol == id && k.protocol.revision == revision) {
        Ok(())
    } else {
        Err(format!("the protocol directory registers no {id}@{revision}"))
    }
}

/// The catalog the tree in `store` keeps.
pub fn catalog(store: &Path) -> Result<loom_protocols::ProtocolCatalog, String> {
    stored_catalog(store).map_err(|e| e.to_string())?.ok_or_else(|| "the tree keeps no protocols: it was not opened by the library".to_owned())
}

/// What one `run_tree` call ended with.
pub struct Ran {
    pub stop: TreeStop,
    pub case: String,
    pub steps: i64,
    pub detail: Option<String>,
    /// The run as the `tree run` command prints it (`b10x_loom_tree::run_json`).
    pub document: Value,
    pub decisions: Vec<NextDecision>,
}

/// `run_tree` once, with the observer the caller gives.
pub fn run(
    store: &Path,
    catalog: &loom_protocols::ProtocolCatalog,
    settings: &RunSettings,
    observer: Option<&mut dyn b10x_loom_tree::RunObserver>,
    cancel: Option<&AtomicBool>,
) -> Result<Ran, super::refusal::Refusal> {
    let ran = run_tree(store, catalog, settings, observer, cancel).map_err(|e| super::refusal::Refusal::of(&e))?;
    let document = b10x_loom_tree::run_json(&ran);
    Ok(Ran { stop: ran.stop, case: ran.case, steps: ran.steps, detail: ran.detail, document, decisions: ran.decisions })
}

/// The exit status `b10x-loom tree run` ends with for each way a run stops.
pub fn stop_status(stop: TreeStop) -> i32 {
    match stop {
        TreeStop::Terminal => 0,
        TreeStop::AwaitingPerson | TreeStop::AwaitingAuthority => 3,
        TreeStop::Stuck => 4,
        TreeStop::BudgetSpent | TreeStop::Cancelled => 5,
    }
}

/// `tree status` of the store.
pub fn status(store: &Path, catalog: &loom_protocols::ProtocolCatalog) -> Result<b10x_loom_tree::model::TreeStatus, String> {
    tree_status(store, catalog).map_err(|e| e.to_string())
}

/// `tree status` of the store, as the document the command prints.
pub fn status_document(store: &Path, catalog: &loom_protocols::ProtocolCatalog) -> Result<Value, String> {
    status(store, catalog).map(|s| status_json(&s))
}

/// `tree verify` of the store: the document the command prints and whether every log verified.
pub fn verify_document(store: &Path, catalog: &loom_protocols::ProtocolCatalog) -> Result<(Value, bool), String> {
    let verified = verify_tree(store, catalog).map_err(|e| e.to_string())?;
    Ok((verify_json(&verified), verified.failure.is_none()))
}

/// The decision a `tree run` document names, read back from its structured members (the printed
/// `command` is carried and never read).
pub fn decision_from_json(v: &Value) -> Result<NextDecision, String> {
    let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
    let kind = match v.get("kind").and_then(Value::as_str) {
        Some("Approve") => DecisionKind::Approve,
        Some("ApproveAction") => DecisionKind::ApproveAction,
        Some("Close") => DecisionKind::Close,
        Some("Handoff") => DecisionKind::Handoff,
        other => return Err(format!("a decision of kind {other:?}: the library answers Approve, ApproveAction, Close and Handoff")),
    };
    Ok(NextDecision {
        case: text("case").ok_or("a decision names no case")?,
        kind,
        action: text("action"),
        capability: text("capability"),
        principal: text("principal"),
        decides: text("decides").unwrap_or_default(),
        subject: text("subject"),
        revision: text("revision"),
        path: text("path"),
        outcome: text("outcome"),
        command: v.get("command").and_then(Value::as_array).map(|w| w.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default(),
    })
}

/// The library call an answer is, chosen by the decision's kind and structured members.
pub fn apply(store: &Path, catalog: &loom_protocols::ProtocolCatalog, d: &NextDecision, parsed: &Parsed, principal: &str, runner: &Arc<dyn TestRunner>) -> Result<(), String> {
    match d.kind {
        DecisionKind::Approve | DecisionKind::ApproveAction => {
            let action = if d.kind == DecisionKind::ApproveAction { d.action.as_deref() } else { None };
            let reasons = match (&parsed.reasons, &parsed.reasons_file) {
                (Some(text), _) => Some(Reasons::Text(text)),
                (None, Some(file)) => Some(Reasons::File(file)),
                (None, None) => None,
            };
            approve(&ApproveRequest {
                store,
                case: &d.case,
                capability: d.capability.as_deref(),
                principal,
                decision: if parsed.deny { ApprovalDecision::Deny } else { ApprovalDecision::Allow },
                action,
                result: parsed.result.as_deref(),
                reasons,
                time: TreeTime::Host,
                catalog,
                runner: Some(Arc::clone(runner)),
            })
            .map(|_| ())
            .map_err(|e| e.to_string())
        }
        DecisionKind::Close => {
            if parsed.result.is_some() || parsed.reasons.is_some() || parsed.reasons_file.is_some() {
                return Err("a close takes approve or deny, not a result".to_string());
            }
            if parsed.deny {
                return Ok(());
            }
            let outcome = d.outcome.as_deref().ok_or("the decision names no outcome")?;
            close(&CloseRequest { store, case: &d.case, outcome, time: TreeTime::Host, catalog }).map(|_| ()).map_err(|e| e.to_string())
        }
        DecisionKind::Handoff => Ok(()),
    }
}

/// A path as the library takes it: absolute.
pub fn absolute(path: &Path) -> Result<PathBuf, String> {
    std::path::absolute(path).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_printed_principal_is_carried_and_an_absent_one_stays_absent() {
        let with = json!({"case": ".", "kind": "ApproveAction", "action": "scope.answer", "capability": "scope.answer", "principal": "person:owner", "decides": "d"});
        assert_eq!(decision_from_json(&with).unwrap().principal.as_deref(), Some("person:owner"));
        let without = json!({"case": ".", "kind": "Close", "outcome": "operating", "decides": "d"});
        assert_eq!(decision_from_json(&without).unwrap().principal, None);
    }
}
