//! What a run without a model node leaves of the login file: nothing. The settings name a login file
//! (a stand-in with invented text); no private directory is made, no file of the run directory holds
//! the file's path or text, and no call of Loom carries either in its argv or environment. A fake
//! `b10x-loom`, under `substrate` with none of the money options: no model call, no network request.

use super::core::{code, grep_tree, stderr, World};
use super::fake::tree_run_json;

fn set(a: &mut [String], flag: &str, value: &str) {
    let i = a.iter().position(|x| x == flag).unwrap();
    a[i + 1] = value.to_owned();
}

fn without(a: &mut Vec<String>, flag: &str) {
    let i = a.iter().position(|x| x == flag).unwrap();
    a.drain(i..=i + 1);
}

fn bare(w: &World) -> Vec<String> {
    let mut a = w.args("r1", 0);
    for flag in ["--spend-ledger", "--cap-micro-usd", "--max-llm-calls"] {
        without(&mut a, flag);
    }
    set(&mut a, "--confinement", "substrate");
    a
}

const NO_MODEL: &str = "format: loom.bindings/1
protocol: sdc@8
actions:
  - action: check.run
    executor:
      kind: program
      argv: [tools/sdc-tool.sh, check-idea]
  - action: scope.approve
    executor: {kind: person, principal: person:owner}
";

/// A run without a model node, settings that name a login file: no private directory, no link,
/// and no file of the run directory holds the path of the login file or its text; no call of
/// Loom carries either in its argv or its environment.
#[test]
fn a_run_without_a_model_node_leaves_no_trace_of_the_login_file() {
    let w = World::new("login-trace");
    w.fake.record_env();
    std::fs::write(w.root.join("bindings/sdc.yaml"), NO_MODEL).unwrap();
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let o = w.run(&bare(&w));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(w.outcome("r1")["has_model_node"], false);
    assert!(!w.root.join("xdg/runbook").exists(), "a private directory was made");
    let login = w.login_path().display().to_string();
    for needle in [login.as_str(), "LOGIN-FILE-MARKER-4711", "loginhome"] {
        let mut found = Vec::new();
        grep_tree(&w.run_dir("r1"), needle, &mut found);
        assert!(found.is_empty(), "files of the run directory hold {needle}: {found:?}");
        for call in w.fake.calls() {
            assert!(!call.argv.iter().any(|a| a.contains(needle)), "{needle} is in the argv of a call of Loom: {:?}", call.argv);
            assert!(!call.env().iter().any(|(k, v)| k.contains(needle) || v.contains(needle)), "{needle} is in the environment of a call of Loom");
        }
        assert!(!String::from_utf8_lossy(&o.stdout).contains(needle) && !stderr(&o).contains(needle), "{needle} is on a stream of the runner");
    }
}
