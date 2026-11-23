//! The pages that say what a refused run writes are held against `src/engine/refusal.rs`: a cause
//! the code can write and the page does not list is a failure here. Reads files of the checkout only.

use runbook::engine::refusal::{cause, PROGRAM_SIGNALLED, REASONS};
use std::path::Path;

fn page(rel: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

#[test]
fn the_run_directory_page_says_a_refused_run_writes_one_of_the_reason_causes() {
    let text = page("docs/loom-run-directory.md");
    let row = text.lines().find(|l| l.starts_with("| `run-stopped.json`")).expect("the row of run-stopped.json");
    assert!(row.contains("refused"), "the cause column does not name the refused outcome: {row}");
    assert!(row.contains(PROGRAM_SIGNALLED), "the cause column does not name {PROGRAM_SIGNALLED}: {row}");
    assert!(row.contains("docs/loom-run.md"), "the cause column does not say where the reason causes are listed: {row}");
}

#[test]
fn the_loom_run_page_lists_every_cause_a_refusal_writes() {
    let text = page("docs/loom-run.md");
    let mut missing: Vec<&str> = REASONS.iter().map(|r| cause(*r)).filter(|c| !text.contains(&format!("`{c}`"))).collect();
    if !text.contains(&format!("`{PROGRAM_SIGNALLED}`")) {
        missing.push(PROGRAM_SIGNALLED);
    }
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
    assert!(text.contains("--confinement substrate is not a level the library can give here"), "the page does not state the text of the refusal of check_level (library.rs)");
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
