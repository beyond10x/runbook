//! The refusals of Loom's tree calls, by name (`loom.tree.TreeRefusalReason`).
//!
//! A refused call carries a reason; a run that ends on one says so by name: `run-stopped.json` holds
//! the reason's snake-case name as `cause`, and `outcome.json` the same as `cause`. [`cause`] matches
//! every reason of the pinned Loom and has no catch-all arm: a reason a later Loom adds stops the
//! build here, and [`index`] with [`REASONS`] stops the test until the list is complete.
//!
//! The refusals this pin added to the embedder interface and what they are here:
//!
//! - `TreeStopped`: `tree hold` is in force; `run_tree` writes nothing and says so. Cause `tree_stopped`.
//! - `ConfinementRefused`: no runner is admitted for the bound programs, or the cgroup root is held by
//!   another run of the process (the one process-wide driver table, keyed by cgroup root, workspace
//!   parent and egress). Cause `confinement_refused`.
//! - `PersonProgramFailed`: the program of a person action ended badly. Cause `person_program_failed`.
//! - `TreeAlreadyStopped`, `TreeNotStopped`, `StopUnusable`: refusals of `hold` and `reopen`, which no
//!   `runbook` command calls; named all the same.
//!
//! `ProgramSignalled`, `Terminated` and the others of Loom's inner enums are not reasons of a tree
//! call: Loom writes the first into the `refusal` of an `ActionFinished` line and wraps the others in
//! `StoreUnreadable` (see `loom_run::refusal`).

use b10x_loom_tree::model::TreeRefusalReason;
use serde_json::Value;

/// Every reason of the pinned Loom, in the order of its definition.
pub const REASONS: [TreeRefusalReason; 29] = [
    TreeRefusalReason::StoreInUse,
    TreeRefusalReason::NotATree,
    TreeRefusalReason::StoreNotEmpty,
    TreeRefusalReason::WorkspaceUnusable,
    TreeRefusalReason::BindingsRefused,
    TreeRefusalReason::AgentNotRunYet,
    TreeRefusalReason::UnknownProtocol,
    TreeRefusalReason::InvalidRevision,
    TreeRefusalReason::InvalidInstant,
    TreeRefusalReason::CaseNotInTree,
    TreeRefusalReason::CaseTerminated,
    TreeRefusalReason::OutcomeNotLegitimate,
    TreeRefusalReason::NotAPersonAction,
    TreeRefusalReason::ConfinementRefused,
    TreeRefusalReason::StoreUnreadable,
    TreeRefusalReason::LogRefused,
    TreeRefusalReason::StoredProtocolChanged,
    TreeRefusalReason::DuplicateProtocol,
    TreeRefusalReason::ProtocolRefused,
    TreeRefusalReason::ProtocolLinkLeavesDirectory,
    TreeRefusalReason::ProtocolNotInDirectory,
    TreeRefusalReason::NoProtocolFiles,
    TreeRefusalReason::UnknownStatusFormat,
    TreeRefusalReason::HandoffCheckFailed,
    TreeRefusalReason::PersonProgramFailed,
    TreeRefusalReason::TreeStopped,
    TreeRefusalReason::TreeAlreadyStopped,
    TreeRefusalReason::TreeNotStopped,
    TreeRefusalReason::StopUnusable,
];

/// The position of `reason` in [`REASONS`]; a match with every reason by name.
pub fn index(reason: TreeRefusalReason) -> usize {
    match reason {
        TreeRefusalReason::StoreInUse => 0,
        TreeRefusalReason::NotATree => 1,
        TreeRefusalReason::StoreNotEmpty => 2,
        TreeRefusalReason::WorkspaceUnusable => 3,
        TreeRefusalReason::BindingsRefused => 4,
        TreeRefusalReason::AgentNotRunYet => 5,
        TreeRefusalReason::UnknownProtocol => 6,
        TreeRefusalReason::InvalidRevision => 7,
        TreeRefusalReason::InvalidInstant => 8,
        TreeRefusalReason::CaseNotInTree => 9,
        TreeRefusalReason::CaseTerminated => 10,
        TreeRefusalReason::OutcomeNotLegitimate => 11,
        TreeRefusalReason::NotAPersonAction => 12,
        TreeRefusalReason::ConfinementRefused => 13,
        TreeRefusalReason::StoreUnreadable => 14,
        TreeRefusalReason::LogRefused => 15,
        TreeRefusalReason::StoredProtocolChanged => 16,
        TreeRefusalReason::DuplicateProtocol => 17,
        TreeRefusalReason::ProtocolRefused => 18,
        TreeRefusalReason::ProtocolLinkLeavesDirectory => 19,
        TreeRefusalReason::ProtocolNotInDirectory => 20,
        TreeRefusalReason::NoProtocolFiles => 21,
        TreeRefusalReason::UnknownStatusFormat => 22,
        TreeRefusalReason::HandoffCheckFailed => 23,
        TreeRefusalReason::PersonProgramFailed => 24,
        TreeRefusalReason::TreeStopped => 25,
        TreeRefusalReason::TreeAlreadyStopped => 26,
        TreeRefusalReason::TreeNotStopped => 27,
        TreeRefusalReason::StopUnusable => 28,
    }
}

/// The cause a run ended on a refusal of `reason` names: Loom's name in snake case.
pub fn cause(reason: TreeRefusalReason) -> &'static str {
    match reason {
        TreeRefusalReason::StoreInUse => "store_in_use",
        TreeRefusalReason::NotATree => "not_a_tree",
        TreeRefusalReason::StoreNotEmpty => "store_not_empty",
        TreeRefusalReason::WorkspaceUnusable => "workspace_unusable",
        TreeRefusalReason::BindingsRefused => "bindings_refused",
        TreeRefusalReason::AgentNotRunYet => "agent_not_run_yet",
        TreeRefusalReason::UnknownProtocol => "unknown_protocol",
        TreeRefusalReason::InvalidRevision => "invalid_revision",
        TreeRefusalReason::InvalidInstant => "invalid_instant",
        TreeRefusalReason::CaseNotInTree => "case_not_in_tree",
        TreeRefusalReason::CaseTerminated => "case_terminated",
        TreeRefusalReason::OutcomeNotLegitimate => "outcome_not_legitimate",
        TreeRefusalReason::NotAPersonAction => "not_a_person_action",
        TreeRefusalReason::ConfinementRefused => "confinement_refused",
        TreeRefusalReason::StoreUnreadable => "store_unreadable",
        TreeRefusalReason::LogRefused => "log_refused",
        TreeRefusalReason::StoredProtocolChanged => "stored_protocol_changed",
        TreeRefusalReason::DuplicateProtocol => "duplicate_protocol",
        TreeRefusalReason::ProtocolRefused => "protocol_refused",
        TreeRefusalReason::ProtocolLinkLeavesDirectory => "protocol_link_leaves_directory",
        TreeRefusalReason::ProtocolNotInDirectory => "protocol_not_in_directory",
        TreeRefusalReason::NoProtocolFiles => "no_protocol_files",
        TreeRefusalReason::UnknownStatusFormat => "unknown_status_format",
        TreeRefusalReason::HandoffCheckFailed => "handoff_check_failed",
        TreeRefusalReason::PersonProgramFailed => "person_program_failed",
        TreeRefusalReason::TreeStopped => "tree_stopped",
        TreeRefusalReason::TreeAlreadyStopped => "tree_already_stopped",
        TreeRefusalReason::TreeNotStopped => "tree_not_stopped",
        TreeRefusalReason::StopUnusable => "stop_unusable",
    }
}

/// The reason a refusal text names: Loom prints a refusal as `<Reason>: ...`. A child process
/// (`b10x-loom`) gives only this text, so its refusals are read by that leading name.
pub fn named(text: &str) -> Option<TreeRefusalReason> {
    let head = text.trim_start().split(':').next()?.trim();
    REASONS
        .into_iter()
        .find(|reason| format!("{reason:?}") == head)
}

/// The refusal a `b10x-loom tree` command printed on standard error: its line
/// `b10x-loom: tree refused: <Reason>: ...`, read by the reason it leads with.
pub fn in_stderr(stderr: &str) -> Option<Refusal> {
    stderr.lines().find_map(|line| {
        let text = line.strip_prefix("b10x-loom: tree refused: ")?;
        Some(Refusal {
            reason: named(text)?,
            text: text.to_owned(),
        })
    })
}

/// The cause of a run that ended `Stuck` because its last action's program was ended by a signal.
pub const PROGRAM_SIGNALLED: &str = "program_signalled";

/// The refusal of the last action of a case's run log, when it is `ProgramSignalled` and nothing
/// followed it but the stop. Loom writes the refusal of a bound program into the `refusal` of an
/// `ActionFinished` line (no `exit`); the action stays undone, so a tree whose only open action is
/// that one ends `Stuck`. Other kinds of line after it (`Evaluated`, another `ActionFinished`) mean
/// the run went on.
pub fn program_signalled_in(log: &str) -> Option<String> {
    let mut last: Option<String> = None;
    for line in log
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
    {
        match line.get("kind").and_then(Value::as_str) {
            Some("ActionFinished") => {
                last = line
                    .get("refusal")
                    .and_then(Value::as_str)
                    .filter(|r| r.starts_with("ProgramSignalled:"))
                    .map(str::to_owned);
            }
            Some("Evaluated") => last = None,
            _ => {}
        }
    }
    last
}

/// [`program_signalled_in`] over every case's run log of the store.
pub fn program_signalled(store: &std::path::Path) -> Option<String> {
    let mut files: Vec<String> = std::fs::read_dir(store)
        .ok()?
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| {
            n.starts_with(b10x_loom_runlog::PREFIX) && n.ends_with(b10x_loom_runlog::EXTENSION)
        })
        .collect();
    files.sort();
    files
        .iter()
        .find_map(|f| program_signalled_in(&std::fs::read_to_string(store.join(f)).ok()?))
}

/// A call into Loom's tree commands that was refused: the reason (typed) and Loom's text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub reason: TreeRefusalReason,
    pub text: String,
}

impl Refusal {
    /// The refusal of a call, as Loom's `Refused` says it.
    pub fn of(refused: &b10x_loom_tree::Refused) -> Refusal {
        Refusal {
            reason: refused.reason(),
            text: refused.to_string(),
        }
    }

    /// The cause name.
    pub fn cause(&self) -> &'static str {
        cause(self.reason)
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(&self.text)
    }
}

impl From<Refusal> for String {
    fn from(refusal: Refusal) -> String {
        refusal.text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_holds_every_reason_once_in_the_order_of_the_match() {
        for (position, reason) in REASONS.iter().enumerate() {
            assert_eq!(index(*reason), position, "{reason:?}");
        }
    }

    #[test]
    fn a_cause_is_the_snake_case_of_the_reason_and_no_two_are_alike() {
        let mut seen = std::collections::BTreeSet::new();
        for reason in REASONS {
            let name = format!("{reason:?}");
            let snake: String = name
                .chars()
                .enumerate()
                .flat_map(|(i, c)| {
                    if c.is_uppercase() && i > 0 {
                        vec!['_', c.to_ascii_lowercase()]
                    } else {
                        vec![c.to_ascii_lowercase()]
                    }
                })
                .collect();
            assert_eq!(cause(reason), snake);
            assert!(seen.insert(cause(reason)), "{reason:?}");
        }
    }

    #[test]
    fn a_text_is_read_by_its_leading_reason() {
        assert_eq!(
            named("TreeStopped: held by process 4: the tree is stopped"),
            Some(TreeRefusalReason::TreeStopped)
        );
        assert_eq!(
            named("ConfinementRefused: the cgroup root is held"),
            Some(TreeRefusalReason::ConfinementRefused)
        );
        assert_eq!(
            named("PersonProgramFailed: case \"x\": the program exited 3"),
            Some(TreeRefusalReason::PersonProgramFailed)
        );
        assert_eq!(
            named("ConfinementRefused: the confinement level local is refused: x"),
            Some(TreeRefusalReason::ConfinementRefused)
        );
        assert_eq!(named("the confinement level local is refused: x"), None);
        assert_eq!(named(""), None);
    }

    #[test]
    fn a_program_ended_by_a_signal_is_read_from_the_last_line_of_the_log_only() {
        let signalled = r#"{"seq":4,"kind":"ActionFinished","action":"note.write","refusal":"ProgramSignalled: action note.write: t.sh was ended by a signal: exit 137"}"#;
        let stop = r#"{"seq":5,"kind":"Stopped","stop":"Stuck","next":[]}"#;
        let opened = r#"{"seq":0,"kind":"CaseOpened","case":"root","protocol":"p@1"}"#;
        let evaluated = r#"{"seq":6,"kind":"Evaluated","case_revision":2}"#;
        let ok = r#"{"seq":7,"kind":"ActionFinished","action":"note.check","exit":0}"#;
        let log = |lines: &[&str]| lines.join("\n");
        assert_eq!(
            program_signalled_in(&log(&[opened, signalled, stop])).as_deref(),
            Some("ProgramSignalled: action note.write: t.sh was ended by a signal: exit 137")
        );
        assert_eq!(
            program_signalled_in(&log(&[opened, signalled, evaluated, ok, stop])),
            None,
            "the run went on after it"
        );
        assert_eq!(program_signalled_in(&log(&[opened, stop])), None);
        let other =
            r#"{"seq":4,"kind":"ActionFinished","action":"a","refusal":"ProgramSpawnFailed: x"}"#;
        assert_eq!(program_signalled_in(&log(&[opened, other, stop])), None);
    }

    #[test]
    fn a_childs_standard_error_gives_the_refusal_it_names() {
        let stderr = "b10x-loom: tree refused: TreeStopped: held by process 4\n{\"reason\":\"TreeStopped\"}\n";
        let refusal = in_stderr(stderr).unwrap();
        assert_eq!(refusal.reason, TreeRefusalReason::TreeStopped);
        assert_eq!(refusal.cause(), "tree_stopped");
        assert_eq!(refusal.text, "TreeStopped: held by process 4");
        assert_eq!(in_stderr("b10x-loom: tree run: Stuck: case c\n"), None);
        assert_eq!(
            in_stderr("b10x-loom: tree refused: something unnamed\n"),
            None
        );
    }
}
