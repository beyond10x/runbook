//! The levels and the caps of a run without a model node: the spellings of a level that start nothing,
//! the forms of "no caps" under `local` that start nothing, and the caps that end a run under `local` as
//! they end a run with a model node. A fake `b10x-loom`, no model call,
//! no network request.

use super::core::{code, model_call_hook, stderr, World};
use super::fake::tree_run_json;
use serde_json::{json, Value};
use std::process::Output;

pub(super) const NO_MODEL: &str = "format: loom.bindings/1
protocol: sdc@8
actions:
  - action: check.run
    executor:
      kind: program
      argv: [tools/sdc-tool.sh, check-idea]
  - action: scope.approve
    executor: {kind: person, principal: person:owner}
";

pub(super) fn set(a: &mut [String], flag: &str, value: &str) {
    let i = a.iter().position(|x| x == flag).unwrap();
    a[i + 1] = value.to_owned();
}

pub(super) fn without(a: &mut Vec<String>, flag: &str) {
    let i = a.iter().position(|x| x == flag).unwrap();
    a.drain(i..=i + 1);
}

/// Run `r1` with every cap at 0 and no ledger, under `level`.
pub(super) fn zero(w: &World, level: &str) -> Vec<String> {
    let mut a = w.args("r1", 0);
    set(&mut a, "--max-llm-calls", "0");
    without(&mut a, "--spend-ledger");
    set(&mut a, "--confinement", level);
    a
}

pub(super) fn last_line(o: &Output) -> String {
    stderr(o).lines().last().unwrap_or("").to_owned()
}

pub(super) fn argvs(w: &World) -> Vec<Vec<String>> {
    w.fake.calls().into_iter().map(|c| c.argv).collect()
}

/// No spelling of a level but `substrate` itself starts a run without a model node and without caps:
/// each is refused before anything is written and before any call of Loom.
#[test]
fn no_other_spelling_of_a_level_starts_a_run_without_caps() {
    let levels = ["Substrate", "SUBSTRATE", "substrate ", " substrate", "substrate\n", "sub", "s", "linux", "strict", "none", "sandbox", "substrate,local", "local,substrate", "substrate=local", ""];
    let wrong: Vec<String> = std::thread::scope(|s| {
        let handles: Vec<_> = levels
            .iter()
            .enumerate()
            .map(|(n, level)| {
                s.spawn(move || {
                    let w = World::new(&format!("zc-level-{n}"));
                    std::fs::write(w.root.join("bindings/sdc.yaml"), NO_MODEL).unwrap();
                    let o = w.run(&zero(&w, level));
                    println!("--confinement {level:?}: exit {}, {}", code(&o), last_line(&o));
                    nothing_started(&w, &o, &format!("--confinement {level:?}"), true)
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// What is wrong when a run was to be refused: exit 0, a call of Loom, and (with `written`) a run
/// directory, a ledger file or a private directory.
fn nothing_started(w: &World, o: &Output, what: &str, written: bool) -> Vec<String> {
    let mut wrong = Vec::new();
    if code(o) == 0 {
        wrong.push(format!("{what}: exit 0"));
    }
    if w.fake.calls_so_far() != 0 {
        wrong.push(format!("{what}: {} calls of Loom: {:?}", w.fake.calls_so_far(), argvs(w)));
    }
    if written {
        for (name, path) in [("a run directory", w.run_dir("r1")), ("a ledger file", w.ledger()), ("a private directory", w.root.join("xdg/runbook"))] {
            if path.exists() {
                wrong.push(format!("{what}: {name} was made (exit {}, {})", code(o), last_line(o)));
            }
        }
    }
    wrong
}

/// The level named twice, the level in the settings file against the option, and the level in the
/// environment: none makes a run under `local` start without caps, and a run that does start names
/// one level in every call of Loom.
#[test]
fn a_level_from_the_settings_the_environment_or_a_second_option_does_not_replace_the_option() {
    let mut wrong = Vec::new();
    // the settings name substrate, the option local
    let w = World::new("zc-level-settings-a");
    std::fs::write(w.root.join("bindings/sdc.yaml"), NO_MODEL).unwrap();
    w.settings(&json!({"confinement": "substrate"}));
    let o = w.run(&zero(&w, "local"));
    println!("settings substrate, option local: exit {}, {}", code(&o), last_line(&o));
    wrong.extend(nothing_started(&w, &o, "settings substrate, option local", true));
    // the settings name the level in another letter case
    let w = World::new("zc-level-settings-b");
    std::fs::write(w.root.join("bindings/sdc.yaml"), NO_MODEL).unwrap();
    w.settings(&json!({"confinement": "Substrate"}));
    let o = w.run(&zero(&w, "local"));
    println!("settings Substrate, option local: exit {}, {}", code(&o), last_line(&o));
    wrong.extend(nothing_started(&w, &o, "settings Substrate, option local", true));
    // the environment names a level
    let w = World::new("zc-level-env");
    std::fs::write(w.root.join("bindings/sdc.yaml"), NO_MODEL).unwrap();
    let mut c = w.command(&zero(&w, "local"));
    for name in ["RUNBOOK_CONFINEMENT", "CONFINEMENT", "LOOM_CONFINEMENT", "RUNBOOK_RUN_CONFINEMENT"] {
        c.env(name, "substrate");
    }
    let o = c.output().unwrap();
    println!("environment substrate, option local: exit {}, {}", code(&o), last_line(&o));
    wrong.extend(nothing_started(&w, &o, "environment substrate, option local", true));
    // the option without a value in the list, the environment naming one
    let w = World::new("zc-level-env-only");
    std::fs::write(w.root.join("bindings/sdc.yaml"), NO_MODEL).unwrap();
    let mut a = zero(&w, "local");
    without(&mut a, "--confinement");
    let mut c = w.command(&a);
    for name in ["RUNBOOK_CONFINEMENT", "CONFINEMENT", "LOOM_CONFINEMENT"] {
        c.env(name, "substrate");
    }
    let o = c.output().unwrap();
    println!("no option, environment substrate: exit {}, {}", code(&o), last_line(&o));
    wrong.extend(nothing_started(&w, &o, "no option, environment substrate", true));
    // the option twice
    for (n, order) in [["local", "substrate"], ["substrate", "local"]].iter().enumerate() {
        let w = World::new(&format!("zc-level-twice-{n}"));
        std::fs::write(w.root.join("bindings/sdc.yaml"), NO_MODEL).unwrap();
        w.fake.answer("tree_run", "1", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
        let mut a = zero(&w, order[0]);
        a.extend(["--confinement".to_owned(), order[1].to_owned()]);
        let o = w.run(&a);
        println!("--confinement {} --confinement {}: exit {}, {}", order[0], order[1], code(&o), last_line(&o));
        for argv in argvs(&w) {
            let levels: Vec<&String> = argv.iter().enumerate().filter(|(i, _)| *i > 0 && argv[i - 1] == "--confinement").map(|(_, x)| x).collect();
            if levels.iter().any(|l| l.as_str() != "substrate") {
                wrong.push(format!("the option twice ({order:?}): a call of Loom under another level than substrate: {argv:?}"));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Under `local`, a run without a model node starts no call of Loom (so no bound program) in any
/// form of "no caps" the four committed forms do not hold: an option that is absent, a value that
/// is 0 in another spelling, a member of the settings that disagrees with the option, a ledger that
/// cannot be one. `written` forms must also leave no run directory, ledger file or private directory.
#[test]
fn no_form_of_no_caps_starts_a_bound_program_under_local() {
    type Change = Box<dyn Fn(&World, &mut Vec<String>) + Sync>;
    let forms: Vec<(&str, bool, Change)> = vec![
        ("--cap-micro-usd absent", true, Box::new(|_: &World, a: &mut Vec<String>| without(a, "--cap-micro-usd"))),
        ("--max-llm-calls absent", true, Box::new(|_: &World, a: &mut Vec<String>| without(a, "--max-llm-calls"))),
        ("--cap-micro-usd 00", true, Box::new(|_: &World, a: &mut Vec<String>| set(a, "--cap-micro-usd", "00"))),
        ("--max-llm-calls +0", true, Box::new(|_: &World, a: &mut Vec<String>| set(a, "--max-llm-calls", "+0"))),
        ("--cap-micro-usd -1", true, Box::new(|_: &World, a: &mut Vec<String>| set(a, "--cap-micro-usd", "-1"))),
        ("--cap-micro-usd without a value", true, Box::new(|_: &World, a: &mut Vec<String>| set(a, "--cap-micro-usd", ""))),
        ("--cap-micro-usd 0 and cap_micro_usd 5000000 in the settings", true, Box::new(|w: &World, a: &mut Vec<String>| {
            w.settings(&json!({"cap_micro_usd": 5_000_000}));
            set(a, "--cap-micro-usd", "0");
        })),
        ("--max-llm-calls 0 and max_llm_calls 100 in the settings", true, Box::new(|w: &World, a: &mut Vec<String>| {
            w.settings(&json!({"max_llm_calls": 100}));
            set(a, "--max-llm-calls", "0");
        })),
        ("both caps 0 in the options and in the settings, a ledger named", true, Box::new(|w: &World, a: &mut Vec<String>| {
            w.settings(&json!({"cap_micro_usd": 0, "max_llm_calls": 0}));
            set(a, "--cap-micro-usd", "0");
            set(a, "--max-llm-calls", "0");
        })),
        ("no --spend-ledger and a spend_ledger member in the settings", true, Box::new(|w: &World, a: &mut Vec<String>| {
            w.settings(&json!({"spend_ledger": w.ledger().display().to_string()}));
            without(a, "--spend-ledger");
        })),
        ("--spend-ledger with an empty value", false, Box::new(|_: &World, a: &mut Vec<String>| set(a, "--spend-ledger", ""))),
        ("--spend-ledger that names a directory", false, Box::new(|w: &World, a: &mut Vec<String>| set(a, "--spend-ledger", &w.root.join("bindings").display().to_string()))),
        ("--spend-ledger that holds no ledger", false, Box::new(|w: &World, a: &mut Vec<String>| {
            std::fs::write(w.root.join("not-a-ledger.json"), "{{ not json").unwrap();
            set(a, "--spend-ledger", &w.root.join("not-a-ledger.json").display().to_string());
        })),
    ];
    let wrong: Vec<String> = std::thread::scope(|s| {
        let handles: Vec<_> = forms
            .iter()
            .enumerate()
            .map(|(n, (what, written, change))| {
                s.spawn(move || {
                    let w = World::new(&format!("zc-nocaps-{n}"));
                    std::fs::write(w.root.join("bindings/sdc.yaml"), NO_MODEL).unwrap();
                    w.fake.answer("tree_run", "default", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
                    let mut a = w.args("r1", 5_000_000);
                    change(&w, &mut a);
                    let o = w.run(&a);
                    println!("{what}: exit {}, run directory {}, {}", code(&o), w.run_dir("r1").exists(), last_line(&o));
                    let mut wrong = nothing_started(&w, &o, what, *written);
                    if !*written && !w.fake.calls_of(&["tree", "run"]).is_empty() {
                        wrong.push(format!("{what}: a tree run was made"));
                    }
                    wrong
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Under `local` with caps, the caps end a run without a model node at the same line as a run with
/// one: the same options and the same recorded calls give the same exit, the same outcome and the
/// same count of calls. The amount booked is the same while the caps are not reached (100000 for one
/// recorded call of 0.1 USD, 200000 for two). At a cap it can differ: the runner lowers the
/// `--max-budget-usd` of a declared model tool to what the cap has left, and an undeclared program has
/// no such option. What one step records is booked whole, and the runner bounds neither the amount nor
/// the count of what one step records. Here each step records one call, so the figures differ by one
/// call (cap 150000: 150000 with a model node, 200000 without; cap 50000: 50000 and 100000). The case
/// `under_local_what_one_step_records_is_booked_whole_and_no_tree_run_follows_the_cap` of
/// `no_model_node_refusals_and_booking.rs` holds the whole booking. `docs/loom-run.md` says this.
#[test]
fn under_local_the_caps_end_a_run_without_a_model_node_as_they_end_a_run_with_one() {
    let mut wrong = Vec::new();
    // (cap, calls, booked by the run with a model node, booked by the run without one)
    for (n, (cap, calls, booked_with, booked_without)) in [(10_000_000u64, 1u64, 100_000u64, 100_000u64), (10_000_000, 2, 200_000, 200_000), (150_000, 100, 150_000, 200_000), (50_000, 100, 50_000, 100_000)].iter().enumerate() {
        let mut seen = Vec::new();
        for with_node in [true, false] {
            let w = World::new(&format!("zc-parity-{n}-{with_node}"));
            if !with_node {
                std::fs::write(w.root.join("bindings/sdc.yaml"), NO_MODEL).unwrap();
            }
            w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
            w.fake.hook("tree_run", "default", &model_call_hook("0.1"));
            let mut a = w.args("r1", *cap);
            set(&mut a, "--max-llm-calls", &calls.to_string());
            let o = w.run(&a);
            let outcome = w.outcome("r1");
            let booked = std::fs::read_to_string(w.ledger()).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v["spent_micro_usd"].as_u64());
            println!(
                "cap {cap}, calls {calls}, model node {with_node}: exit {}, outcome {}, llm_calls {}, spent {}, booked {booked:?}, tree runs {}",
                code(&o),
                outcome["outcome"],
                outcome["llm_calls"],
                outcome["spent_micro_usd"],
                w.fake.calls_of(&["tree", "run"]).len()
            );
            if outcome["has_model_node"] != with_node {
                wrong.push(format!("cap {cap}, calls {calls}: has_model_node is {} in the run built {}", outcome["has_model_node"], if with_node { "with a model node" } else { "without one" }));
            }
            if outcome["spent_micro_usd"].as_u64() != booked {
                wrong.push(format!("cap {cap}, calls {calls}, model node {with_node}: the outcome states {} and the ledger holds {booked:?}", outcome["spent_micro_usd"]));
            }
            seen.push((code(&o), outcome["outcome"].clone(), outcome["llm_calls"].clone(), w.fake.calls_of(&["tree", "run"]).len()));
            let want = if with_node { *booked_with } else { *booked_without };
            if booked != Some(want) {
                wrong.push(format!("cap {cap}, calls {calls}, model node {with_node}: the ledger holds {booked:?}, {want} expected"));
            }
        }
        if seen[0] != seen[1] {
            wrong.push(format!("cap {cap}, calls {calls}: with a model node {:?}, without one {:?}", seen[0], seen[1]));
        }
        if seen[1].0 == 0 {
            wrong.push(format!("cap {cap}, calls {calls}: the run without a model node ended with exit 0 past its caps"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
