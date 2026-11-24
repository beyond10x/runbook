//! The pages that say what a refused run writes are held against `src/engine/refusal.rs`: a cause
//! the code can write and the page does not list is a failure here. Reads files of the checkout only.

use runbook::engine::refusal::{cause, PROGRAM_REFUSALS, REASONS};
use std::collections::BTreeSet;
use std::path::Path;

/// Every cause the code can write: one per reason, and one per refusal of a bound program.
fn real_causes() -> BTreeSet<String> {
    let mut v: BTreeSet<String> = REASONS.iter().map(|r| cause(*r).to_owned()).collect();
    v.extend(PROGRAM_REFUSALS.iter().map(|(_, c)| (*c).to_owned()));
    v
}

fn page(rel: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

#[test]
fn the_run_directory_page_says_a_refused_run_writes_one_of_the_reason_causes() {
    let text = page("docs/loom-run-directory.md");
    let row = text.lines().find(|l| l.starts_with("| `run-stopped.json`")).expect("the row of run-stopped.json");
    assert!(row.contains("refused"), "the cause column does not name the refused outcome: {row}");
    for (_, program_cause) in PROGRAM_REFUSALS {
        assert!(row.contains(program_cause), "the cause column does not name {program_cause}: {row}");
    }
    assert!(row.contains("docs/loom-run.md"), "the cause column does not say where the reason causes are listed: {row}");
}

#[test]
fn the_loom_run_page_lists_every_cause_a_refusal_writes() {
    let text = page("docs/loom-run.md");
    let mut missing: Vec<&str> = REASONS.iter().map(|r| cause(*r)).filter(|c| !text.contains(&format!("`{c}`"))).collect();
    missing.extend(PROGRAM_REFUSALS.iter().map(|(_, c)| *c).filter(|c| !text.contains(&format!("`{c}`"))));
    assert_eq!(missing, Vec::<&str>::new(), "docs/loom-run.md does not list these causes of refusal.rs");
}

#[test]
fn the_loom_run_page_lists_cause_among_the_members_of_outcome_json() {
    let text = page("docs/loom-run.md");
    let at = text.find("runbook.loom-run-outcome/1: outcome, reason").expect("the members line of outcome.json");
    let end = text[at..].find("tree-status.json").map(|p| at + p).expect("the line after the members");
    assert!(text[at..end].contains("cause"), "{}", &text[at..end]);
}

#[test]
fn the_loom_run_page_states_the_text_of_a_level_the_library_cannot_give() {
    let text = page("docs/loom-run.md");
    // the whole text, as `check_level` words it (`library.rs` asserts the same literal)
    let text_of_the_refusal = "--confinement substrate is not a level the library can give here: it offers local (the linux level needs a cgroup this process may delegate)";
    assert!(text.contains(text_of_the_refusal), "the page does not state the text of the refusal of check_level (library.rs)");
}

/// The other direction: a cause the page names and the code does not have fails, so a cause that is
/// renamed or dropped in `refusal.rs` cannot stay on the page.
#[test]
fn the_loom_run_page_names_no_cause_the_code_does_not_have() {
    let text = page("docs/loom-run.md");
    let from = text.find("#### A refusal of Loom by a named reason").expect("the section");
    let to = text[from..].find("The run log has two new kinds").map(|p| from + p).expect("the end of the section");
    let have = real_causes();
    let mut ghosts = BTreeSet::new();
    // a backticked word of snake case that is not a name of the code or of `outcome.json` is taken for a cause
    let words = ["run_tree", "check_level", "stderr_tail", "loom_exit_code", "exit_code"];
    for part in text[from..to].split('`').skip(1).step_by(2) {
        let snake = !part.is_empty() && part.contains('_') && part.chars().all(|c| c.is_ascii_lowercase() || c == '_');
        if snake && !words.contains(&part) && !have.contains(part) {
            ghosts.insert(part.to_owned());
        }
    }
    assert_eq!(ghosts, BTreeSet::new(), "docs/loom-run.md names causes that refusal.rs does not have");
}

/// The number of reasons is computed from `refusal.rs`; the pages say it where they count.
#[test]
fn the_pages_count_the_reasons_as_the_code_does() {
    let n = REASONS.len();
    let run = page("docs/loom-run.md");
    assert!(run.contains(&format!("The {n} causes")), "docs/loom-run.md does not say 'The {n} causes'");
    assert!(run.contains(&format!("{n} of them")), "docs/loom-run.md does not say '{n} of them'");
    let dir = page("docs/loom-run-directory.md");
    assert!(dir.contains(&format!("one of {n} (")), "docs/loom-run-directory.md does not say 'one of {n}'");
    let log = page("CHANGELOG.md");
    assert!(log.contains(&format!("({n}, one `cause` each")), "CHANGELOG.md does not count {n} reasons");
}

#[test]
fn the_loom_run_page_says_which_refusals_are_exit_12_and_which_stay_exit_1() {
    let text = page("docs/loom-run.md");
    for needed in [
        "every refusal of `tree open` that leads with a reason name",
        "`tree status` and `tree verify`",
        "before a run directory exists",
    ] {
        assert!(text.contains(needed), "docs/loom-run.md lacks: {needed}");
    }
}

/// The page says what `reason` a `stuck` run carries: the refusal's lead when a refusal stopped the tree.
#[test]
fn the_loom_run_page_says_a_stuck_run_names_the_refusal_that_stopped_its_tree() {
    let text = page("docs/loom-run.md");
    for needed in ["no run ends `stuck` with\n`reason: null` when a refusal stopped its tree", "`RunsSpent: action <name>`", "a tree with no move and no refusal ends `stuck` with\n`reason: null`"] {
        assert!(text.contains(needed), "docs/loom-run.md lacks: {needed}");
    }
}

/// A refusal is read by the name it leads with: a text of another refusal that holds a program refusal's lead is none.
#[test]
fn a_refusal_text_is_read_by_its_leading_name_only() {
    use runbook::engine::refusal::program_refused_in;
    let line = |refusal: &str| format!(r#"{{"seq":4,"kind":"ActionFinished","action":"a","refusal":"{refusal}"}}"#);
    let stop = r#"{"seq":5,"kind":"Stopped","stop":"Stuck","next":[]}"#;
    for text in ["ResultNotInTable: action a: the report said ProgramError: x", "RunsSpent: action a: x ProgramSignalled: y"] {
        assert_eq!(program_refused_in(&format!("{}\n{stop}", line(text))), None, "{text}");
    }
}

/// A refusal followed by the case's evaluation and nothing else is no longer what the tree is stuck on.
#[test]
fn a_refusal_the_case_was_evaluated_after_is_not_the_reason_of_the_stop() {
    use runbook::engine::refusal::program_refused_in;
    let refused = r#"{"seq":4,"kind":"ActionFinished","action":"a","refusal":"ProgramError: action a: p ended exit status 69"}"#;
    let evaluated = r#"{"seq":5,"kind":"Evaluated","case_revision":2}"#;
    let stop = r#"{"seq":6,"kind":"Stopped","stop":"Stuck","next":[]}"#;
    assert_eq!(program_refused_in(&format!("{refused}\n{evaluated}\n{stop}")), None);
}
