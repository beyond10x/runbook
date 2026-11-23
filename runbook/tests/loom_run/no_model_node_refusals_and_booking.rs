//! The two refusal kinds of a run (exit 64 for a missing option of a run with a model node, exit 2
//! for the other refusals) and what the caps book under `local` (`docs/loom-run.md`, "A run without
//! a model node" and "What the spend cap does not promise"). A fake `b10x-loom`, no model call, no
//! network request. Three cases are guards of the default step. Two are measurements, `#[ignore]`d:
//! they assert what the backlog story `spend-cap-with-nothing-left-starts-nothing` is to build and
//! fail until it is built.

use super::core::{code, stderr, World};
use super::fake::{decision_json, tree_run_json};
use super::no_model_node_levels_and_caps::{last_line, set, without, NO_MODEL};
use serde_json::{json, Value};
use std::process::Output;

/// A model tool the bindings declare without a budget option of its own.
const MODEL_TOOL_WITHOUT_A_BUDGET: &str = "format: loom.bindings/1
protocol: sdc@8
actions:
  - action: node.write
    executor:
      kind: program
      argv: [sdc-node, node.write]
";

/// A hook that records `count` calls of `cost` USD each, whatever budget the copy names.
fn records(cost: &str, count: u32) -> String {
    let mut s = String::from("w=work/.sdc/llm; mkdir -p \"$w\"\n");
    for _ in 0..count {
        s.push_str("i=$(( $(find \"$w\" -mindepth 1 -maxdepth 1 -type d | wc -l) + 1 ))\n");
        s.push_str("d=$(printf '%03d-node.write' $i); mkdir -p \"$w/$d\"\n");
        s.push_str(&format!("printf '{{\"action\":\"node.write\",\"record\":\"%s/%s\",\"cost_usd\":{cost}}}\\n' \"$PWD/$w\" \"$d\" >> \"$w/calls.jsonl\"\n"));
    }
    s
}

fn booked(w: &World) -> Option<u64> {
    std::fs::read_to_string(w.ledger()).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v["spent_micro_usd"].as_u64())
}

fn tree_runs(w: &World) -> usize {
    w.fake.calls_of(&["tree", "run"]).len()
}

/// What was written by a run that was to be refused before anything is written.
fn written(w: &World) -> Vec<&'static str> {
    let mut made = Vec::new();
    for (name, path) in [("a run directory", w.run_dir("r1")), ("a ledger file", w.ledger()), ("a private directory", w.root.join("xdg/runbook"))] {
        if path.exists() {
            made.push(name);
        }
    }
    made
}

fn said(what: &str, o: &Output) {
    println!("{what}: exit {}, {}", code(o), last_line(o));
}

/// With a model node, a value that is present and unusable ends as the code of `main`
/// ends it (the paths are `main`'s: the loop's call cap, the reservation, the ledger opened at the
/// opening, the option parser): a call cap of 0 is exit 6 and a spend cap of 0 is exit 4, both
/// before a first `tree run`; a ledger that cannot be one fails the run closed (exit 1) before a
/// first `tree run`; a value the parser refuses is exit 64 with nothing written.
#[test]
fn with_a_model_node_a_value_that_is_present_and_unusable_ends_as_on_main() {
    type Change = Box<dyn Fn(&World, &mut Vec<String>)>;
    // (what, the change, the exit, refused before anything is written)
    let rows: Vec<(&str, Change, i32, bool)> = vec![
        ("--cap-micro-usd 0", Box::new(|_: &World, a: &mut Vec<String>| set(a, "--cap-micro-usd", "0")), 4, false),
        ("--max-llm-calls 0", Box::new(|_: &World, a: &mut Vec<String>| set(a, "--max-llm-calls", "0")), 6, false),
        ("--spend-ledger that names a directory", Box::new(|w: &World, a: &mut Vec<String>| set(a, "--spend-ledger", &w.root.join("bindings").display().to_string())), 1, false),
        (
            "--spend-ledger that holds no ledger",
            Box::new(|w: &World, a: &mut Vec<String>| {
                std::fs::write(w.root.join("not-a-ledger.json"), "{{ not json").unwrap();
                set(a, "--spend-ledger", &w.root.join("not-a-ledger.json").display().to_string());
            }),
            1,
            false,
        ),
        ("--spend-ledger with an empty value", Box::new(|_: &World, a: &mut Vec<String>| set(a, "--spend-ledger", "")), 64, true),
        ("--cap-micro-usd -1", Box::new(|_: &World, a: &mut Vec<String>| set(a, "--cap-micro-usd", "-1")), 64, true),
        ("--max-llm-calls that is a word", Box::new(|_: &World, a: &mut Vec<String>| set(a, "--max-llm-calls", "many")), 64, true),
    ];
    let mut wrong = Vec::new();
    for (n, (what, change, exit, refused)) in rows.iter().enumerate() {
        let w = World::new(&format!("nmrb-unusable-{n}"));
        w.fake.answer("tree_run", "default", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
        let mut a = w.args("r1", 1_000_000);
        change(&w, &mut a);
        let o = w.run(&a);
        said(what, &o);
        if code(&o) != *exit {
            wrong.push(format!("{what}: exit {}, {exit} expected ({})", code(&o), last_line(&o)));
        }
        if tree_runs(&w) != 0 {
            wrong.push(format!("{what}: {} calls of tree run", tree_runs(&w)));
        }
        if *refused && (w.run_dir("r1").exists() || w.root.join("xdg/runbook").exists() || w.fake.calls_so_far() != 0) {
            wrong.push(format!("{what}: something was written or started before the refusal"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The usage refusal of a run with a model node (no `--spend-ledger`) stands before the
/// run directory, the ledger, the login link and the private directory also when another refusal
/// of the run waits behind it: a run directory that exists already is left as it was, and a
/// runtime directory that is not there is not asked for. A refusal that stands before it (settings
/// that are no JSON) wins with its own exit, and nothing is written either.
#[test]
fn the_usage_refusal_of_a_run_with_a_model_node_writes_nothing_beside_another_refusal() {
    let mut wrong = Vec::new();
    // a run directory that exists
    let w = World::new("nmrb-usage-rundir");
    std::fs::create_dir_all(w.run_dir("r1")).unwrap();
    std::fs::write(w.run_dir("r1").join("kept.txt"), "kept").unwrap();
    let mut a = w.args("r1", 1_000_000);
    without(&mut a, "--spend-ledger");
    let o = w.run(&a);
    said("no ledger, the run directory exists", &o);
    let names: Vec<String> = std::fs::read_dir(w.run_dir("r1")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    if code(&o) != 64 || !stderr(&o).contains("--spend-ledger is required") {
        wrong.push(format!("the run directory exists: exit {}, {}", code(&o), last_line(&o)));
    }
    if names != ["kept.txt"] || w.ledger().exists() || w.root.join("xdg/runbook").exists() || w.fake.calls_so_far() != 0 {
        wrong.push(format!("the run directory exists: something was written or started: {names:?}"));
    }
    // no runtime directory for the private directory
    let w = World::new("nmrb-usage-xdg");
    let mut a = w.args("r1", 1_000_000);
    without(&mut a, "--spend-ledger");
    let mut c = w.command(&a);
    c.env_remove("XDG_RUNTIME_DIR");
    let o = c.output().unwrap();
    said("no ledger, no XDG_RUNTIME_DIR", &o);
    if code(&o) != 64 || !stderr(&o).contains("--spend-ledger is required") {
        wrong.push(format!("no XDG_RUNTIME_DIR: exit {}, {}", code(&o), last_line(&o)));
    }
    if !written(&w).is_empty() || w.fake.calls_so_far() != 0 {
        wrong.push(format!("no XDG_RUNTIME_DIR: written {:?}", written(&w)));
    }
    // settings that are no JSON: the refusal of the settings stands first
    let w = World::new("nmrb-usage-settings");
    std::fs::write(w.root.join("settings.json"), "{{ not json").unwrap();
    let mut a = w.args("r1", 1_000_000);
    without(&mut a, "--spend-ledger");
    let o = w.run(&a);
    said("no ledger, settings that are no JSON", &o);
    if code(&o) == 0 || !written(&w).is_empty() || w.fake.calls_so_far() != 0 {
        wrong.push(format!("settings that are no JSON: exit {}, written {:?}", code(&o), written(&w)));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Under `local` with caps, what one step records is booked whole, whatever its amount
/// and however many calls it holds: nothing bounds the amount a program writes into its record.
/// What holds is the stop: once the booked amount has reached the cap, no further `tree run` is
/// made, and the run ends `stopped: spend cap`, exit 4. The same with a model node (the tool that
/// does not keep to its lowered budget).
#[test]
fn under_local_what_one_step_records_is_booked_whole_and_no_tree_run_follows_the_cap() {
    let mut wrong = Vec::new();
    // (what, the cost of a call in USD, calls in one step, booked)
    for (n, (what, cost, count, want)) in [("one call of 1000 USD", "1000", 1u32, 1_000_000_000u64), ("three calls of 0.1 USD in one step", "0.1", 3, 300_000)].iter().enumerate() {
        let mut seen = Vec::new();
        for with_node in [true, false] {
            let w = World::new(&format!("nmrb-whole-{n}-{with_node}"));
            if !with_node {
                std::fs::write(w.root.join("bindings/sdc.yaml"), NO_MODEL).unwrap();
            }
            w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
            w.fake.hook("tree_run", "default", &records(cost, *count));
            let o = w.run(&w.args("r1", 150_000));
            let outcome = w.outcome("r1");
            println!("{what}, cap 150000, model node {with_node}: exit {}, outcome {}, llm_calls {}, booked {:?}, tree runs {}", code(&o), outcome["outcome"], outcome["llm_calls"], booked(&w), tree_runs(&w));
            if outcome["has_model_node"] != with_node {
                wrong.push(format!("{what}: has_model_node is {} in the run built with_node={with_node}", outcome["has_model_node"]));
            }
            if code(&o) != 4 || tree_runs(&w) != 1 {
                wrong.push(format!("{what}, model node {with_node}: exit {}, {} calls of tree run; exit 4 after one expected", code(&o), tree_runs(&w)));
            }
            if booked(&w) != Some(*want) || outcome["llm_calls"].as_u64() != Some(u64::from(*count)) {
                wrong.push(format!("{what}, model node {with_node}: booked {:?} with {} calls; {want} with {count} expected", booked(&w), outcome["llm_calls"]));
            }
            seen.push((code(&o), booked(&w), outcome["llm_calls"].clone(), tree_runs(&w)));
        }
        if seen[0] != seen[1] {
            wrong.push(format!("{what}: with a model node {:?}, without one {:?}", seen[0], seen[1]));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// A measurement, not a guard of this unit: what the runner does with a decision the
/// step that passed the cap printed. The case asserts what "after the cap is passed no further
/// bound program starts" would hold: no `tree approve` is made (it runs the program a person
/// action binds) once the booked amount is at or above the cap.
#[test]
#[ignore = "backlog story spend-cap-with-nothing-left-starts-nothing; measurement: fails until that story is built (a decision printed by the step that passed the cap is still answered by tree approve)"]
fn measured_after_the_cap_is_passed_no_tree_approve_is_made() {
    let mut wrong = Vec::new();
    for with_node in [true, false] {
        let w = World::new(&format!("nmrb-approve-{with_node}"));
        if !with_node {
            std::fs::write(w.root.join("bindings/sdc.yaml"), NO_MODEL).unwrap();
        }
        let d = decision_json("owner.approve", "ApproveAction", "person:owner");
        w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 1, None, &format!("[{d}]")), 3);
        w.fake.hook("tree_run", "1", &records("0.2", 1));
        w.fake.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
        w.fake.answer("tree_run", "default", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
        w.settings(&json!({"decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [{"approve": true, "principal": "person:owner", "cost_micro_usd": 0, "model_calls": 0}]}}));
        let o = w.run(&w.args("r1", 150_000));
        let approves = w.fake.calls_of(&["tree", "approve"]).len();
        println!("cap 150000, one call of 0.2 USD and a decision in the first step, model node {with_node}: exit {}, {}, booked {:?}, tree runs {}, tree approves {approves}", code(&o), last_line(&o), booked(&w), tree_runs(&w));
        if code(&o) != 4 || tree_runs(&w) != 1 || booked(&w) != Some(200_000) {
            wrong.push(format!("model node {with_node}: exit {}, {} calls of tree run, booked {:?}", code(&o), tree_runs(&w), booked(&w)));
        }
        if approves != 0 {
            wrong.push(format!("model node {with_node}: {approves} calls of tree approve after the booked amount passed the cap"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// A measurement, on a path this unit did not change: a model tool the bindings declare without
/// `--max-budget-usd`, run with a spend cap of 0. The page says the stop before the first `tree run`
/// holds only when the bindings name `--max-budget-usd` ("What the spend cap does not promise"); the
/// case asserts the stop the backlog story is to give in this case too, and shows the "otherwise" of
/// the page: the step starts and the call cap ends the run.
#[test]
#[ignore = "backlog story spend-cap-with-nothing-left-starts-nothing; measurement: fails until that story is built (a spend cap of 0 and a declared model tool that names no budget option)"]
fn measured_a_spend_cap_of_0_stops_a_run_whose_model_tool_names_no_budget_before_the_first_tree_run() {
    let w = World::new("nmrb-cap0-nobudget");
    std::fs::write(w.root.join("bindings/sdc.yaml"), MODEL_TOOL_WITHOUT_A_BUDGET).unwrap();
    w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "default", &records("0.1", 1));
    let mut a = w.args("r1", 0);
    set(&mut a, "--max-llm-calls", "3");
    let o = w.run(&a);
    let outcome = w.outcome("r1");
    println!("cap 0, calls 3, a model tool without a budget option: exit {}, {}, has_model_node {}, llm_calls {}, booked {:?}, tree runs {}", code(&o), last_line(&o), outcome["has_model_node"], outcome["llm_calls"], booked(&w), tree_runs(&w));
    assert_eq!(outcome["has_model_node"], true, "{outcome}");
    assert_eq!((code(&o), tree_runs(&w), booked(&w).unwrap_or(0)), (4, 0, 0), "a spend cap of 0 did not stop the run before its first tree run");
}
