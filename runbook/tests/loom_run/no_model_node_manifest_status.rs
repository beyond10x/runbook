//! The manifest check of a run without a model node before a `tree status`: a manifest rewritten while
//! Loom answers a `tree run` ends the run by its own cause and not by an error of Loom. A fake `b10x-loom`.

use super::core::{code, stderr, World};
use super::fake::tree_run_json;
use super::no_model_node_levels_and_caps::{argvs, zero, NO_MODEL};

/// The row "a `tree status`" of the table of the page ("a changed manifest ends the run by this cause
/// and not by an error of Loom"). The cases on the real Loom that held it were removed, and the
/// mutant "manifest check skipped before a tree status only" passes every committed case. Here the
/// manifest is rewritten while Loom answers a `tree run`; the next path of the runner is the `tree
/// status` of that step. It is refused there: the reason names `tree status`, and the status of the
/// step is not asked of Loom (the one `tree status` after the rewrite is the closing one).
#[test]
fn a_manifest_rewritten_during_a_step_is_refused_before_the_status_of_that_step() {
    let w = World::new("zc-manifest-status");
    std::fs::write(w.root.join("bindings/sdc.yaml"), NO_MODEL).unwrap();
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.fake.hook("tree_run", "1", "printf '{\"bindings\":\"%s/bindings/none.yaml\"}' \"$PWD\" > \"$store/tree.json\"\n");
    let o = w.run(&zero(&w, "substrate"));
    let outcome = w.outcome("r1");
    let calls: Vec<String> = argvs(&w).iter().map(|a| a.iter().take(2).cloned().collect::<Vec<_>>().join(" ")).collect();
    println!("exit {}, calls of Loom: {calls:?}, reason: {}", code(&o), outcome["reason"]);
    assert_eq!(code(&o), 13, "{}", stderr(&o));
    assert_eq!(outcome["cause"], "model_call_without_model_node", "{outcome}");
    let reason = outcome["reason"].as_str().unwrap_or("");
    assert!(reason.contains("tree manifest") && reason.contains("tree status"), "the run was not refused at the status of the step: {outcome}");
    let run_at = calls.iter().position(|c| c == "tree run").expect("no tree run");
    let statuses = calls[run_at + 1..].iter().filter(|c| *c == "tree status").count();
    assert!(statuses <= 1, "the status of the step was asked of Loom after the manifest was rewritten: {calls:?}");
    assert_eq!(calls.iter().filter(|c| *c == "tree run").count(), 1, "{calls:?}");
}
