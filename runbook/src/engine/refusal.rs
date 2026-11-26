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
//! `ProgramSignalled`, `ProgramError`, `Terminated` and the others of Loom's inner enums are not
//! reasons of a tree call: Loom writes the first two into the `refusal` of an `ActionFinished` line
//! ([`PROGRAM_REFUSALS`]) and wraps the others in `StoreUnreadable` (see `loom_run::refusal`).

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

/// The cause of a run that ended `Stuck` because its last action's program ended with an exit that
/// the binding's `exits.error` lists (`ProgramError`).
pub const PROGRAM_ERROR: &str = "program_error";

/// The refusals of a bound program that Loom writes into the `refusal` of an `ActionFinished` line
/// and that end a run `refused` here: the text each leads with and its cause. The other members of
/// Loom's `ProgramRefusalReason` (`ResultNotInTable`, `RunsSpent`, ...) are not in the table: a
/// tree they leave without a move still ends `stuck`.
pub const PROGRAM_REFUSALS: [(&str, &str); 2] = [
    ("ProgramSignalled:", PROGRAM_SIGNALLED),
    ("ProgramError:", PROGRAM_ERROR),
];

/// A refusal of a bound program that left the tree without a move: its cause and Loom's text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgramRefused {
    pub cause: &'static str,
    pub text: String,
}

/// The refusal of the last action of a case's run log, when it is one of [`PROGRAM_REFUSALS`] and
/// nothing followed it but the stop. Loom writes the refusal of a bound program into the `refusal`
/// of an `ActionFinished` line (no `exit`); the action stays undone, so a tree whose only open
/// action is that one ends `Stuck`. Other kinds of line after it (`Evaluated`, another
/// `ActionFinished`) mean the run went on.
pub fn program_refused_in(log: &str) -> Option<ProgramRefused> {
    let text = last_refusal_in(log)?;
    PROGRAM_REFUSALS
        .iter()
        .find(|(lead, _)| text.starts_with(lead))
        .map(|(_, cause)| ProgramRefused { cause, text })
}

/// The `refusal` of the last action of a case's run log, whatever it is, when nothing followed it but
/// the stop (an `Evaluated` line after it means the run went on).
pub fn last_refusal_in(log: &str) -> Option<String> {
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
                    .map(str::to_owned);
            }
            Some("Evaluated") => last = None,
            _ => {}
        }
    }
    last
}

/// The lead of a refusal text of a bound program: its reason word and its action, `<Reason>: action
/// <name>`. Loom writes the rest of the text after a second colon and it may hold a path or a value
/// of the run, so a reason that leaves the run names the lead only.
pub fn lead_of(text: &str) -> String {
    let mut parts = text.splitn(3, ':');
    let reason = parts.next().unwrap_or_default().trim();
    match parts.next() {
        Some(action) => format!("{reason}: {}", action.trim()),
        None => reason.to_owned(),
    }
}

fn run_log_files(store: &std::path::Path) -> Vec<String> {
    let mut files: Vec<String> = std::fs::read_dir(store)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| {
            n.starts_with(b10x_loom_runlog::PREFIX) && n.ends_with(b10x_loom_runlog::EXTENSION)
        })
        .collect();
    files.sort();
    files
}

/// [`program_refused_in`] over every case's run log of the store.
pub fn program_refused(store: &std::path::Path) -> Option<ProgramRefused> {
    run_log_files(store)
        .iter()
        .find_map(|f| program_refused_in(&std::fs::read_to_string(store.join(f)).ok()?))
}

/// The lead ([`lead_of`]) of the refusal that stopped the tree: the first case's log whose last
/// action was refused, a refusal of a bound program or any other (`ResultNotInTable`, `RunsSpent`,
/// ...). `None` for a tree that has no move and no refusal.
pub fn stuck_on(store: &std::path::Path) -> Option<String> {
    run_log_files(store)
        .iter()
        .find_map(|f| last_refusal_in(&std::fs::read_to_string(store.join(f)).ok()?))
        .map(|text| lead_of(&text))
}

/// A step whose program failed: its action and the exit its program ended with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailedStep {
    pub action: String,
    pub exit: i64,
}

/// The last program of a case's run log, when it failed: the last `ActionFinished` line that ended a program has an
/// `exit` other than 0. A program that ended with a refusal of [`PROGRAM_REFUSALS`] (an exit `exits.error` lists, a
/// signal) is no failed step here: [`program_refused`] names it. A refusal of another kind (`RunsSpent` of an action
/// whose program failed every run it had, `ResultNotInTable`, ...) ended no program and is passed over. `None` when
/// the last program ended 0, ended with such a refusal, or none ran.
pub fn failed_step_in(log: &str) -> Option<FailedStep> {
    let mut last: Option<FailedStep> = None;
    for line in log.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()) {
        if line.get("kind").and_then(Value::as_str) != Some("ActionFinished") {
            continue;
        }
        match (line.get("exit").and_then(Value::as_i64), line.get("refusal").and_then(Value::as_str)) {
            (Some(exit), _) => {
                let action = line.get("action").and_then(Value::as_str).unwrap_or_default().to_owned();
                last = Some(FailedStep { action, exit });
            }
            (None, Some(text)) if PROGRAM_REFUSALS.iter().any(|(lead, _)| text.starts_with(lead)) => last = None,
            _ => {}
        }
    }
    last.filter(|s| s.exit != 0)
}

/// [`failed_step_in`] of the log of the case `case`, else of the first case's log that has one.
pub fn failed_step(store: &std::path::Path, case: &str) -> Option<FailedStep> {
    let own = b10x_loom_runlog::log_file_name(case);
    let read = |f: &str| std::fs::read_to_string(store.join(f)).ok().and_then(|t| failed_step_in(&t));
    read(&own).or_else(|| run_log_files(store).iter().filter(|f| **f != own).find_map(|f| read(f)))
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
    fn a_refused_program_is_read_from_the_last_line_of_the_log_only() {
        let signalled = r#"{"seq":4,"kind":"ActionFinished","action":"note.write","refusal":"ProgramSignalled: action note.write: t.sh was ended by a signal: exit 137"}"#;
        let errored = r#"{"seq":4,"kind":"ActionFinished","action":"repro.run","refusal":"ProgramError: action repro.run: t.sh ended exit status 69, which exits.error lists; nothing was recorded"}"#;
        let stop = r#"{"seq":5,"kind":"Stopped","stop":"Stuck","next":[]}"#;
        let opened = r#"{"seq":0,"kind":"CaseOpened","case":"root","protocol":"p@1"}"#;
        let evaluated = r#"{"seq":6,"kind":"Evaluated","case_revision":2}"#;
        let ok = r#"{"seq":7,"kind":"ActionFinished","action":"note.check","exit":0}"#;
        let log = |lines: &[&str]| lines.join("\n");
        let refused = |lines: &[&str]| program_refused_in(&log(lines));
        let said = |cause: &'static str, text: &str| {
            Some(ProgramRefused {
                cause,
                text: text.to_owned(),
            })
        };
        assert_eq!(
            refused(&[opened, signalled, stop]),
            said(
                PROGRAM_SIGNALLED,
                "ProgramSignalled: action note.write: t.sh was ended by a signal: exit 137"
            )
        );
        assert_eq!(
            refused(&[opened, errored, stop]),
            said(
                PROGRAM_ERROR,
                "ProgramError: action repro.run: t.sh ended exit status 69, which exits.error lists; nothing was recorded"
            )
        );
        for (name, line) in [("signalled", signalled), ("errored", errored)] {
            assert_eq!(
                refused(&[opened, line, evaluated, ok, stop]),
                None,
                "{name}: the run went on after it"
            );
        }
        assert_eq!(refused(&[opened, stop]), None);
        let other =
            r#"{"seq":4,"kind":"ActionFinished","action":"a","refusal":"ProgramSpawnFailed: x"}"#;
        assert_eq!(refused(&[opened, other, stop]), None);
        // The members of Loom's `ProgramRefusalReason` that a tree can end stuck on and that this table
        // leaves out end `stuck` still.
        for reason in [
            "ResultNotInTable",
            "ReportUnreadable",
            "ArgumentsUnreadable",
            "RunsSpent",
            "CaseOfAnotherProtocol",
            "ArtifactUnreadable",
            "Cancelled",
        ] {
            let line = format!(
                r#"{{"seq":4,"kind":"ActionFinished","action":"a","refusal":"{reason}: action a: x"}}"#
            );
            assert_eq!(refused(&[opened, &line, stop]), None, "{reason}");
        }
    }

    #[test]
    fn a_tree_stuck_on_any_refusal_names_its_lead_and_one_stuck_on_none_names_nothing() {
        let line = |refusal: &str| {
            format!(r#"{{"seq":4,"kind":"ActionFinished","action":"a","refusal":"{refusal}"}}"#)
        };
        let stop = r#"{"seq":5,"kind":"Stopped","stop":"Stuck","next":[]}"#;
        let evaluated = r#"{"seq":6,"kind":"Evaluated","case_revision":2}"#;
        let last = |lines: &[&str]| last_refusal_in(&lines.join("\n"));
        let table =
            line("ResultNotInTable: action draft.check: exit 124 is in no result of /w/x");
        assert_eq!(
            last(&[&table, stop]).map(|t| lead_of(&t)),
            Some("ResultNotInTable: action draft.check".to_owned())
        );
        assert_eq!(last(&[&table, evaluated, stop]), None);
        assert_eq!(last(&[stop]), None);
        assert_eq!(lead_of("RunsSpent"), "RunsSpent");
        assert_eq!(
            lead_of("ProgramError: action repro.run: t.sh ended exit status 69"),
            "ProgramError: action repro.run"
        );
    }

    #[test]
    fn the_table_of_refused_programs_has_one_cause_each_and_the_causes_are_no_reason() {
        let mut seen = std::collections::BTreeSet::new();
        for (lead, cause_name) in PROGRAM_REFUSALS {
            assert!(lead.ends_with(':'), "{lead}");
            assert!(seen.insert(cause_name), "{cause_name}");
            assert!(!REASONS.iter().any(|r| cause(*r) == cause_name), "{cause_name} is a reason's cause");
            assert!(named(lead).is_none(), "{lead} is the name of a reason of a tree call");
        }
    }

    #[test]
    fn the_failed_step_is_the_last_program_that_ended_with_an_exit_other_than_0() {
        let failed = r#"{"seq":3,"kind":"ActionFinished","action":"delegation.free","exit":1}"#;
        let again = r#"{"seq":5,"kind":"ActionFinished","action":"delegation.free","exit":2}"#;
        let spent = r#"{"seq":7,"kind":"ActionFinished","action":"delegation.free","refusal":"RunsSpent: action delegation.free: 2 of 2 runs"}"#;
        let evaluated = r#"{"seq":4,"kind":"Evaluated"}"#;
        let log = [failed, evaluated, again, evaluated, spent].join("\n");
        assert_eq!(failed_step_in(&log), Some(FailedStep { action: "delegation.free".into(), exit: 2 }), "the last exit, and a RunsSpent ends no program");
        let ok = r#"{"seq":9,"kind":"ActionFinished","action":"note.check","exit":0}"#;
        assert_eq!(failed_step_in(&[failed, ok].join("\n")), None, "the last program ended 0");
        let errored = r#"{"seq":9,"kind":"ActionFinished","action":"repro.run","refusal":"ProgramError: action repro.run: t.sh ended exit status 69, which exits.error lists"}"#;
        assert_eq!(failed_step_in(&[failed, errored].join("\n")), None, "a refused program is named by program_refused");
        assert_eq!(failed_step_in(""), None);
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
