//! Cases of the review of the unit `caps-are-optional` (part 1, the runner) that hold the decision "no cap
//! named is no limit and no refusal; a cap that is named stops the run" to its contract (`docs/loom-run.md`,
//! "The money options"): what a run that names no cap books, the printed command of a handoff under a named
//! cap, and the guards that stay. Two cases are `#[ignore]`d measurements of the next unit,
//! `spend-cap-with-nothing-left-starts-nothing`. A fake `b10x-loom` and stand-ins for the model: no model call,
//! no network request, no login file read (the login file of the world is an invented text).

use super::core::{code, model_call_hook, spent, stderr, World};
use super::fake::tree_run_json;
use serde_json::json;

/// A model node whose argv names no per-call budget.
const MODEL_NO_BUDGET: &str = "format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: node.write\n    executor:\n      kind: program\n      argv:\n        - sdc-node\n        - node.write\n";

/// A model node with a per-call budget of 1 USD, block style (what the runner's lowering writes back).
const MODEL_BUDGET: &str = "format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: node.write\n    executor:\n      kind: program\n      argv:\n        - sdc-node\n        - node.write\n        - --max-budget-usd\n        - \"1\"\n";

const NO_MODEL: &str = "format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: check.run\n    executor:\n      kind: program\n      argv: [tools/sdc-tool.sh, check-idea]\n";

fn without(a: &mut Vec<String>, flag: &str) {
    let i = a.iter().position(|x| x == flag).unwrap();
    a.drain(i..=i + 1);
}

/// The options of run `name` with none of the three money options.
fn bare(w: &World, name: &str) -> Vec<String> {
    let mut a = w.args(name, 0);
    for flag in ["--spend-ledger", "--cap-micro-usd", "--max-llm-calls"] {
        without(&mut a, flag);
    }
    a
}

fn bindings(w: &World, text: &str) {
    std::fs::write(w.root.join("bindings/sdc.yaml"), text).unwrap();
}

/// A shell snippet for a `tree_run` hook: a model call that ended with exit 1 and wrote no cost.
fn unknown_cost_hook() -> String {
    "w=work/.sdc/llm; mkdir -p \"$w\"\n\
i=$(( $(find \"$w\" -mindepth 1 -maxdepth 1 -type d | wc -l) + 1 ))\n\
d=$(printf '%03d-node.write' $i); mkdir -p \"$w/$d\"\n\
printf '{\"action\":\"node.write\",\"record\":\"%s/%s\",\"adapter\":\"claude\",\"exit\":1}\\n' \"$PWD/$w\" \"$d\" >> \"$w/calls.jsonl\"\n"
        .to_owned()
}

/// A hook: a model call whose record states a cost of 10 trillion USD.
fn huge_cost_hook() -> String {
    "w=work/.sdc/llm; mkdir -p \"$w\"\n\
i=$(( $(find \"$w\" -mindepth 1 -maxdepth 1 -type d | wc -l) + 1 ))\n\
d=$(printf '%03d-node.write' $i); mkdir -p \"$w/$d\"\n\
printf '{\"action\":\"node.write\",\"record\":\"%s/%s\",\"cost_usd\":10000000000000}\\n' \"$PWD/$w\" \"$d\" >> \"$w/calls.jsonl\"\n"
        .to_owned()
}

fn close_after(w: &World, first: u32, last: u32) {
    for n in first..last {
        w.fake.answer("tree_run", &n.to_string(), &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    }
    w.fake.answer("tree_run", &last.to_string(), &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
}

// ---- A named cap or none, and the arithmetic of what is unnamed ----

/// A run that names a ledger and no cap holds no reservation: a model call that ended with no cost books
/// nothing into the ledger every other run shares, and is recorded as a call of unknown cost (the line of
/// `llm-calls.jsonl` has `cost_unknown` true and `cost_usd` null, not 0 and not the largest number).
#[test]
fn a_call_of_unknown_cost_in_a_run_with_a_ledger_and_no_cap_books_nothing_like_the_largest_number() {
    let w = World::new("capsrev-poison");
    bindings(&w, MODEL_NO_BUDGET);
    close_after(&w, 1, 2);
    w.fake.hook("tree_run", "1", &unknown_cost_hook());
    let mut a = bare(&w, "r1");
    a.extend(["--spend-ledger".to_owned(), w.ledger().display().to_string()]);
    let o = w.run(&a);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let booked = spent(&w);
    assert_eq!(booked, 0, "the ledger of every run that shares the file holds {booked} micro USD after one failed call of a run that named no cap");
    let outcome = w.outcome("r1");
    assert_eq!((outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some(1), Some(0)), "{outcome}");
    let text = std::fs::read_to_string(w.run_dir("r1").join("llm-calls.jsonl")).unwrap();
    let lines: Vec<serde_json::Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines.len(), 1, "{text}");
    assert_eq!((lines[0]["cost_unknown"].as_bool(), lines[0]["cost_usd"].is_null(), lines[0].get("cost_booked").is_none()), (Some(true), true, true), "{text}");
}

/// The same call in a run that names a cap books its reservation, as before: the member `cost_usd` is that
/// number and `cost_unknown` is true.
#[test]
fn a_call_of_unknown_cost_in_a_run_with_a_cap_books_its_reservation_and_says_so() {
    let w = World::new("capsrev-unknown-capped");
    bindings(&w, MODEL_BUDGET);
    close_after(&w, 1, 2);
    w.fake.hook("tree_run", "1", &unknown_cost_hook());
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(spent(&w), 1_000_000, "the reservation of the call, the budget of its bindings");
    let text = std::fs::read_to_string(w.run_dir("r1").join("llm-calls.jsonl")).unwrap();
    let line: serde_json::Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!((line["cost_unknown"].as_bool(), line["cost_usd"].as_f64()), (Some(true), Some(1.0)), "{text}");
}

/// The consequence: a run with a cap that shares the ledger with the run above is not starved by it.
#[test]
fn a_run_with_a_cap_is_not_starved_by_a_run_without_one_that_shares_its_ledger() {
    let w = World::new("capsrev-starve");
    bindings(&w, MODEL_NO_BUDGET);
    close_after(&w, 1, 2);
    w.fake.hook("tree_run", "1", &unknown_cost_hook());
    let mut a = bare(&w, "r1");
    a.extend(["--spend-ledger".to_owned(), w.ledger().display().to_string()]);
    let o = w.run(&a);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    // a second run, a cap of 1 USD, one call of 0.1 USD
    w.fake.answer("tree_run", "3", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "3", &model_call_hook("0.1"));
    w.fake.answer("tree_run", "4", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let o = w.run(&w.args("r2", 1_000_000));
    assert_eq!(code(&o), 0, "the second run, cap 1 USD, 0.1 USD to spend, ledger at {}: {}", spent(&w), stderr(&o));
}

/// Two such calls in a run with no cap and no ledger: the total of the run stays 0, two calls are counted.
#[test]
fn two_calls_of_unknown_cost_in_a_run_without_a_cap_do_not_overflow_the_total_of_the_run() {
    let w = World::new("capsrev-overflow");
    bindings(&w, MODEL_NO_BUDGET);
    close_after(&w, 1, 3);
    w.fake.hook("tree_run", "1", &unknown_cost_hook());
    w.fake.hook("tree_run", "2", &unknown_cost_hook());
    let o = w.run(&bare(&w, "r1"));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(!stderr(&o).contains("overflow"), "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!((outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some(2), Some(0)), "{outcome}");
}

/// The total of the run saturates: two calls of 10 trillion USD each (more than a u64 holds in micro USD
/// together) end the run with the largest number, not with an overflow.
#[test]
fn the_total_of_a_run_saturates_and_never_overflows() {
    let w = World::new("capsrev-saturate");
    bindings(&w, MODEL_NO_BUDGET);
    close_after(&w, 1, 3);
    w.fake.hook("tree_run", "1", &huge_cost_hook());
    w.fake.hook("tree_run", "2", &huge_cost_hook());
    let o = w.run(&bare(&w, "r1"));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!((outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some(2), Some(u64::MAX)), "{outcome}");
}

/// A cap of 0 spends nothing: the model node of the run names no per-call budget, so the amount the run
/// wants for a call is the cap itself, 0, and 0 wanted is never refused.
#[test]
#[ignore = "measurement: a cap of 0 with a model node that names no budget is the story spend-cap-with-nothing-left-starts-nothing"]
fn a_cap_of_0_starts_no_call_of_a_model_node_that_names_no_budget() {
    let w = World::new("capsrev-zero-nobudget");
    bindings(&w, MODEL_NO_BUDGET);
    for n in 1..=3u32 {
        w.fake.answer("tree_run", &n.to_string(), &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
        w.fake.hook("tree_run", &n.to_string(), &model_call_hook("0.6"));
    }
    w.fake.answer("tree_run", "4", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let o = w.run(&w.args("r1", 0));
    let outcome = w.outcome("r1");
    assert_eq!(code(&o), 4, "a cap of 0 let {} model call(s) and {} micro USD through: {outcome}", outcome["llm_calls"], outcome["spent_micro_usd"]);
    assert_eq!((outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some(0), Some(0)), "{outcome}");
}

/// The run was opened without a model node and a cap of 0; the writer of its first step rewrites the run's
/// copy of the bindings to name the model tool (no budget). A model call it would make is stopped by the cap.
#[test]
#[ignore = "measurement: a cap of 0 and a model tool a program wrote into the copy is the story spend-cap-with-nothing-left-starts-nothing"]
fn a_cap_of_0_stops_the_model_tool_that_a_program_wrote_into_the_copy_of_a_run_without_a_model_node() {
    let w = World::new("capsrev-zero-rewrite");
    bindings(&w, NO_MODEL);
    let rewrite = format!("printf '%s' '{}' > bindings/sdc.yaml\n", MODEL_NO_BUDGET.replace('\'', ""));
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "1", &rewrite);
    w.fake.answer("tree_run", "2", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "2", &model_call_hook("0.5"));
    w.fake.answer("tree_run", "3", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let mut a = bare(&w, "r1");
    a.extend(["--cap-micro-usd".to_owned(), "0".to_owned()]);
    let o = w.run(&a);
    let outcome = w.outcome("r1");
    assert_eq!(outcome["has_model_node"], false, "{outcome}");
    assert_eq!((outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some(0), Some(0)), "cap 0, exit {}: a model call was made: {outcome}", code(&o));
}

// ---- The handoff ----

/// A named cap is looked at between two calls of Loom. The printed `tree run` of a handoff carries no bound
/// of its own and the runner runs it as printed: every step it takes spends from the one reservation.
#[test]
#[ignore = "measurement: a run opened with a model node runs the printed command of a handoff as printed under a named cap; the bound is the story handoff-under-a-named-cap-runs-one-step-at-a-time"]
fn the_printed_tree_run_of_a_handoff_is_bounded_to_one_step_when_a_cap_is_named() {
    let w = World::new("capsrev-handoff");
    bindings(&w, MODEL_BUDGET);
    let handoff = "{\"action\":\"amend.upstream\",\"case\":\"root\",\"command\":[\"b10x-loom\",\"tree\",\"run\",\"--store\",\"@STORE@\",\"--confinement\",\"local\"],\"decides\":\"Add a second stop, then run again.\",\"kind\":\"Handoff\"}";
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 2, None, &format!("[{handoff}]")), 3);
    w.fake.answer("tree_run", "2", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.answer("tree_run", "3", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.settings(&json!({"decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [{"approve": true, "cost_micro_usd": 0}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let runs = w.fake.calls_of(&["tree", "run"]);
    assert!(runs.len() >= 2, "the printed command was not run: {}", runs.len());
    let unbounded: Vec<&Vec<String>> = runs.iter().map(|c| &c.argv).filter(|a| !a.windows(2).any(|p| p[0] == "--max-steps" && p[1] == "1")).collect();
    assert!(unbounded.is_empty(), "a tree run call with a named cap is not bounded to one step: {unbounded:?}");
}

// ---- Guards that hold ----

/// What a bound program of a run opened without a model node gets once it has rewritten the copy of the
/// bindings to name the model tool: no private directory, no link to the login file, no path of the login
/// file anywhere in the run directory, the call it makes is booked and counted against the named cap.
#[test]
fn guard_a_copy_rewritten_to_name_the_model_tool_gets_no_login_and_its_call_is_booked() {
    let w = World::new("capsrev-guard-login");
    bindings(&w, NO_MODEL);
    let rewrite = format!("printf '%s' '{}' > bindings/sdc.yaml\n", MODEL_BUDGET.replace('\'', ""));
    let xdg = w.root.join("xdg").display().to_string();
    let look = format!("[ -e \"{xdg}/runbook\" ] && [ -n \"$(ls -A \"{xdg}/runbook\")\" ] && touch \"$D/had-private\"\n");
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "1", &rewrite);
    w.fake.answer("tree_run", "2", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "2", &format!("{look}{}", model_call_hook("0.3")));
    w.fake.answer("tree_run", "3", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let mut a = w.args("r1", 5_000_000);
    without(&mut a, "--spend-ledger");
    let o = w.run(&a);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!((outcome["has_model_node"].as_bool(), outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some(false), Some(1), Some(300_000)), "{outcome}");
    assert!(!w.fake.dir.join("had-private").exists(), "a private directory of the run existed while a rewritten model tool could run");
    assert!(!w.root.join("xdg/runbook/r1").exists());
    let mut hits = Vec::new();
    for needle in ["LOGIN-FILE-MARKER-4711", "credentials.json"] {
        super::core::grep_tree(&w.run_dir("r1"), needle, &mut hits);
    }
    assert!(hits.is_empty(), "the login file or its path is in the run directory: {hits:?}");
    for call in w.fake.calls() {
        assert!(!call.env.iter().any(|(k, v)| v.contains("credentials.json") || v.contains("LOGIN-FILE-MARKER") || k == "ANTHROPIC_API_KEY"), "the login or a key reached Loom: {:?}", call.argv);
    }
}

/// A named cap still stops a run whose copy was rewritten to name a model tool with a budget.
#[test]
fn guard_a_named_cap_stops_a_run_without_a_model_node_whose_copy_was_rewritten_to_name_a_budgeted_model_tool() {
    let w = World::new("capsrev-guard-cap");
    bindings(&w, NO_MODEL);
    let rewrite = format!("printf '%s' '{}' > bindings/sdc.yaml\n", MODEL_BUDGET.replace('\'', ""));
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "1", &rewrite);
    for n in 2..=4u32 {
        w.fake.answer("tree_run", &n.to_string(), &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
        w.fake.hook("tree_run", &n.to_string(), &model_call_hook("0.6"));
    }
    w.fake.answer("tree_run", "5", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let mut a = bare(&w, "r1");
    a.extend(["--cap-micro-usd".to_owned(), "500000".to_owned()]);
    let o = w.run(&a);
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!((outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some(1), Some(500_000)), "{outcome}");
}

/// A cap named twice must agree, a 0 from the settings file is a cap, and a member that is no whole number is refused.
#[test]
fn guard_a_cap_named_twice_must_agree_and_a_zero_from_the_settings_is_a_cap() {
    let w = World::new("capsrev-guard-twice");
    close_after(&w, 1, 1);
    w.settings(&json!({"cap_micro_usd": 0}));
    let o = w.run(&bare(&w, "r1"));
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    // a member of 1 beside an option of 0 is refused before a run directory exists
    w.settings(&json!({"cap_micro_usd": 1}));
    let mut a = bare(&w, "r2");
    a.extend(["--cap-micro-usd".to_owned(), "0".to_owned()]);
    let o = w.run(&a);
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(!w.run_dir("r2").exists());
    // a member that is not a whole number is refused, not read as "none"
    for bad in [json!(-1), json!(1.5), json!("5")] {
        w.settings(&json!({ "cap_micro_usd": bad.clone() }));
        let o = w.run(&bare(&w, "r3"));
        assert_eq!(code(&o), 2, "{bad}: {}", stderr(&o));
        assert!(!w.run_dir("r3").exists(), "{bad}");
    }
}
