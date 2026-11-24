//! The money caps of the runner (`caps-are-optional` and `spend-cap-with-nothing-left-starts-nothing`): a cap that is named and cannot be read never becomes "no limit",
//! and a named cap with nothing left starts nothing that may spend, at every place that starts a program or asks a model.
//! Each case holds one guard; a case marked ignored states a known gap. A fake `b10x-loom` and stand-ins for the model: no model call, no network request,
//! no login file read (the login file of the world is an invented text).

use super::core::{code, model_call_hook, spent, stderr, World};
use super::decider::{fixture_settings, write_settings};
use super::fake::{decision_json, tree_run_json};
use serde_json::{json, Value};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const MODEL_NO_BUDGET: &str = "format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: node.write\n    executor:\n      kind: program\n      argv:\n        - sdc-node\n        - node.write\n";

const MODEL_BUDGET: &str = "format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: node.write\n    executor:\n      kind: program\n      argv:\n        - sdc-node\n        - node.write\n        - --max-budget-usd\n        - \"1\"\n";

const NO_MODEL: &str = "format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: check.run\n    executor:\n      kind: program\n      argv: [tools/sdc-tool.sh, check-idea]\n";

fn bindings(w: &World, text: &str) {
    std::fs::write(w.root.join("bindings/sdc.yaml"), text).unwrap();
}

fn without(a: &mut Vec<String>, flag: &str) {
    let i = a.iter().position(|x| x == flag).unwrap();
    a.drain(i..=i + 1);
}

fn set_value(a: &mut [String], flag: &str, value: &str) {
    let i = a.iter().position(|x| x == flag).unwrap();
    a[i + 1] = value.to_owned();
}

fn tree_runs(w: &World) -> usize {
    w.fake.calls_of(&["tree", "run"]).len()
}

fn steps_then_end(w: &World, last: u32) {
    for n in 1..last {
        w.fake.answer("tree_run", &n.to_string(), &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    }
    w.fake.answer("tree_run", &last.to_string(), &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.fake.answer("tree_run", "default", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
}

// ---- A: a value that is named and cannot be read ----

/// Every spelling of a cap that is no whole number is refused by the option parser before a run directory
/// exists, for both caps; none of them starts a run that has no limit. `+0` is a whole number: a cap of 0.
#[test]
fn an_option_value_that_is_no_whole_number_is_refused_and_never_means_no_limit() {
    let w = World::new("capg-optvalues");
    bindings(&w, MODEL_BUDGET);
    steps_then_end(&w, 3);
    let bad = [
        "", "-0", "-1", "1.5", "5.0", "abc", "10usd", "1e3", " 5", "5 ", "NaN", "inf", "-inf", "1e400", "18446744073709551616", "99999999999999999999999", "0x10", "1_000", "٣", "５", "5,0", "null", "true",
    ];
    let mut wrong = Vec::new();
    let mut n = 0;
    for flag in ["--cap-micro-usd", "--max-llm-calls"] {
        for value in bad {
            n += 1;
            let name = format!("o{n}");
            let mut a = w.args(&name, 1_000_000);
            set_value(&mut a, flag, value);
            let o = w.run(&a);
            if code(&o) == 0 || w.run_dir(&name).exists() {
                wrong.push(format!("{flag} {value:?}: exit {}, run directory {}", code(&o), w.run_dir(&name).exists()));
            }
        }
        // named twice
        n += 1;
        let name = format!("o{n}");
        let mut a = w.args(&name, 1_000_000);
        a.extend([flag.to_owned(), "0".to_owned()]);
        let o = w.run(&a);
        if code(&o) == 0 {
            wrong.push(format!("{flag} named twice: exit 0"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert_eq!(tree_runs(&w), 0, "a run was started with a value that cannot be read");
    // "+0" is a whole number: a named 0
    let mut a = w.args("plus0", 1_000_000);
    set_value(&mut a, "--cap-micro-usd", "+0");
    let o = w.run(&a);
    assert_eq!((code(&o), tree_runs(&w)), (4, 0), "{}", stderr(&o));
}

/// The same for the settings file: a member that names a cap and is not a whole number is refused before a run
/// directory exists; `-0` and `+0` are a 0; a member named twice is not TOML.
#[test]
fn a_settings_member_that_is_no_whole_number_is_refused_and_never_means_no_limit() {
    let w = World::new("capg-memvalues");
    bindings(&w, MODEL_BUDGET);
    steps_then_end(&w, 3);
    let base = std::fs::read_to_string(w.root.join("settings.toml")).unwrap();
    let bad = ["\"5\"", "\"\"", "\"0\"", "5.0", "1e3", "-1", "9223372036854775808", "18446744073709551615", "inf", "nan", "-inf", "[5]", "[]", "true", "{ a = 1 }", "1979-05-27"];
    let mut wrong = Vec::new();
    let mut n = 0;
    for (member, flag) in [("cap_micro_usd", "--cap-micro-usd"), ("max_llm_calls", "--max-llm-calls")] {
        for literal in bad {
            n += 1;
            let name = format!("m{n}");
            std::fs::write(w.root.join("settings.toml"), format!("{base}{member} = {literal}\n")).unwrap();
            let mut a = w.args(&name, 1_000_000);
            without(&mut a, flag);
            let o = w.run(&a);
            if code(&o) != 2 || w.run_dir(&name).exists() {
                wrong.push(format!("{member} = {literal}: exit {}, run directory {}: {}", code(&o), w.run_dir(&name).exists(), stderr(&o).lines().last().unwrap_or("")));
            }
        }
        n += 1;
        let name = format!("m{n}");
        std::fs::write(w.root.join("settings.toml"), format!("{base}{member} = 5\n{member} = 6\n")).unwrap();
        let mut a = w.args(&name, 1_000_000);
        without(&mut a, flag);
        let o = w.run(&a);
        if code(&o) != 2 || w.run_dir(&name).exists() {
            wrong.push(format!("{member} named twice: exit {}", code(&o)));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert_eq!(tree_runs(&w), 0);
    for literal in ["-0", "+0", "0x0"] {
        std::fs::write(w.root.join("settings.toml"), format!("{base}cap_micro_usd = {literal}\n")).unwrap();
        let mut a = w.args(&format!("z{}", ["a", "b", "c"][["-0", "+0", "0x0"].iter().position(|x| *x == literal).unwrap()]), 1_000_000);
        without(&mut a, "--cap-micro-usd");
        let o = w.run(&a);
        assert_eq!(code(&o), 4, "cap_micro_usd = {literal}: {}", stderr(&o));
    }
    assert_eq!(tree_runs(&w), 0, "a 0 from the settings let a tree run start");
}

/// A cap of the largest number a u64 holds is a cap: a call of unknown cost books its whole reservation and the
/// next call finds nothing left. `left == u64::MAX` is never read as "no cap".
#[test]
fn a_cap_of_the_largest_number_is_a_cap_that_a_failed_call_spends_whole() {
    let w = World::new("capg-capmax");
    bindings(&w, MODEL_NO_BUDGET);
    steps_then_end(&w, 3);
    let unknown = "w=work/.sdc/llm; mkdir -p \"$w\"\n\
i=$(( $(find \"$w\" -mindepth 1 -maxdepth 1 -type d | wc -l) + 1 ))\n\
d=$(printf '%03d-node.write' $i); mkdir -p \"$w/$d\"\n\
printf '{\"action\":\"node.write\",\"record\":\"%s/%s\",\"adapter\":\"claude\",\"exit\":1}\\n' \"$PWD/$w\" \"$d\" >> \"$w/calls.jsonl\"\n";
    w.fake.hook("tree_run", "1", unknown);
    w.fake.hook("tree_run", "2", unknown);
    let o = w.run(&w.args("r1", u64::MAX));
    assert_eq!(spent(&w), u64::MAX, "the failed call of a run with a named cap books its reservation: {}", stderr(&o));
    assert_eq!((code(&o), tree_runs(&w)), (4, 1), "the second call started with nothing left: {}", stderr(&o));
}

/// The daemon's submit takes no money option: a cap named beside `--detach` is refused by the parser (exit 2)
/// before any call to a daemon, so a named cap never reaches a submit that would drop it.
#[test]
fn a_cap_named_beside_detach_is_refused_and_never_dropped() {
    let w = World::new("capg-detach");
    let mut wrong = Vec::new();
    for (flag, value) in [("--cap-micro-usd", "0"), ("--max-llm-calls", "0"), ("--spend-ledger", "x.json")] {
        let a: Vec<String> = ["run", "sdc", "--intake", "idea.yaml", "--detach", flag, value].iter().map(|s| (*s).to_owned()).collect();
        let o = w.command(&a).current_dir(&w.root).output().unwrap();
        if code(&o) != 64 {
            wrong.push(format!("--detach {flag} {value}: exit {}: {}", code(&o), stderr(&o).lines().next().unwrap_or("")));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

// ---- A: the ledger ----

fn ledger_row(tag: &str, text: Option<&str>) -> (World, std::process::Output) {
    let w = World::new(tag);
    bindings(&w, MODEL_BUDGET);
    steps_then_end(&w, 3);
    if let Some(text) = text {
        std::fs::write(w.ledger(), text).unwrap();
    }
    let o = w.run(&w.args("r1", 5_000_000));
    (w, o)
}

/// A ledger that exists and cannot be read as a ledger is an error: no step starts, the file is left as it was.
/// An empty file is a new ledger (the file the runner makes for a ledger that is absent).
#[test]
fn a_ledger_that_cannot_be_read_as_one_starts_no_step_and_is_never_read_as_empty() {
    let rows: [(&str, &str); 14] = [
        ("garbage", "not json"),
        ("array", "[]"),
        ("empty object", "{}"),
        ("wrong format", "{\"format\":\"x\",\"spent_micro_usd\":0,\"reserved\":{},\"entries\":[]}"),
        ("spent float", "{\"format\":\"runbook.spend-ledger/1\",\"spent_micro_usd\":1.5,\"reserved\":{},\"entries\":[]}"),
        ("spent negative", "{\"format\":\"runbook.spend-ledger/1\",\"spent_micro_usd\":-1,\"reserved\":{},\"entries\":[]}"),
        ("spent text", "{\"format\":\"runbook.spend-ledger/1\",\"spent_micro_usd\":\"0\",\"reserved\":{},\"entries\":[]}"),
        ("spent over u64", "{\"format\":\"runbook.spend-ledger/1\",\"spent_micro_usd\":18446744073709551616,\"reserved\":{},\"entries\":[]}"),
        ("spent 1e30", "{\"format\":\"runbook.spend-ledger/1\",\"spent_micro_usd\":1e30,\"reserved\":{},\"entries\":[]}"),
        ("reserved array", "{\"format\":\"runbook.spend-ledger/1\",\"spent_micro_usd\":0,\"reserved\":[],\"entries\":[]}"),
        ("reservation text", "{\"format\":\"runbook.spend-ledger/1\",\"spent_micro_usd\":0,\"reserved\":{\"a\":\"1\"},\"entries\":[]}"),
        ("entries missing", "{\"format\":\"runbook.spend-ledger/1\",\"spent_micro_usd\":0,\"reserved\":{}}"),
        ("truncated", "{\"format\":\"runbook.spend-ledger/1\",\"spent_micro_usd\":4000"),
        ("trailing text", "{\"format\":\"runbook.spend-ledger/1\",\"spent_micro_usd\":0,\"reserved\":{},\"entries\":[]} x"),
    ];
    let mut wrong = Vec::new();
    for (i, (what, text)) in rows.iter().enumerate() {
        let (w, o) = ledger_row(&format!("capg-ledger-{i}"), Some(text));
        let after = std::fs::read_to_string(w.ledger()).unwrap_or_default();
        if code(&o) != 1 || tree_runs(&w) != 0 || after != *text {
            wrong.push(format!("{what}: exit {}, tree runs {}, file {}: {}", code(&o), tree_runs(&w), if after == *text { "kept" } else { "rewritten" }, stderr(&o).lines().last().unwrap_or("")));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    // spent at the largest number: a cap with nothing left, exit 4, no step
    let max = "{\"format\":\"runbook.spend-ledger/1\",\"spent_micro_usd\":18446744073709551615,\"reserved\":{},\"entries\":[]}";
    let (w, o) = ledger_row("capg-ledger-max", Some(max));
    assert_eq!((code(&o), tree_runs(&w)), (4, 0), "{}", stderr(&o));
    // a ledger that is a directory
    let w = World::new("capg-ledger-dir");
    bindings(&w, MODEL_BUDGET);
    steps_then_end(&w, 3);
    std::fs::create_dir(w.ledger()).unwrap();
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!((code(&o), tree_runs(&w)), (1, 0), "{}", stderr(&o));
    // a ledger that is a link to a ledger is that ledger
    let w = World::new("capg-ledger-link");
    bindings(&w, MODEL_BUDGET);
    steps_then_end(&w, 3);
    let real = w.root.join("real-ledger.json");
    std::fs::write(&real, "{\"format\":\"runbook.spend-ledger/1\",\"spent_micro_usd\":4999999,\"reserved\":{},\"entries\":[]}").unwrap();
    std::os::unix::fs::symlink(&real, w.ledger()).unwrap();
    let o = w.run(&w.args("r1", 5_000_000));
    let text = std::fs::read_to_string(&real).unwrap();
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(text.contains("\"spent_micro_usd\": 4999999"), "the link was not followed or the figure was lost: {text}");
}

/// A ledger that is a FIFO: the runner must end, not wait for a writer (the read of a FIFO blocks).
#[test]
#[ignore = "known gap: the ledger read blocks on a FIFO (fails closed, no step starts, the path is the operator's); refusing what is no regular file is its own change"]
fn a_ledger_that_is_a_fifo_ends_the_run_and_does_not_hang() {
    let w = World::new("capg-ledger-fifo");
    bindings(&w, MODEL_BUDGET);
    steps_then_end(&w, 3);
    let made = Command::new("mkfifo").arg(w.ledger()).status().unwrap();
    assert!(made.success());
    let ended = ends_within_8_s(&mut w.command(&w.args("r1", 5_000_000)));
    assert!(ended, "the runner did not end within 8 s of a ledger that is a FIFO (it was killed by its PID); tree runs {}", tree_runs(&w));
}

/// Starts `command` and says whether it ended within 8 s; one that did not is killed by its PID and reaped.
fn ends_within_8_s(command: &mut Command) -> bool {
    let mut child = command.stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let started = Instant::now();
    let ended = loop {
        if child.try_wait().unwrap().is_some() {
            break true;
        }
        if started.elapsed() >= Duration::from_secs(8) {
            break false;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let _ = child.kill();
    child.wait().unwrap();
    ended
}

// ---- B and D: the places that start a program ----

/// `--conformance observe` writes tables with a model after a run that closed. A run that closed with a named
/// cap that has nothing left (the last step booked the cap whole) starts no observation: it would spend past the
/// cap with nothing booked.
#[test]
fn a_run_that_closes_with_nothing_left_starts_no_conformance_observation() {
    let w = observed_world("capg-conformance", "");
    let o = observe(&w, Some(200_000));
    let outcome = w.outcome("r1");
    assert_eq!((outcome["outcome"].as_str(), outcome["spent_micro_usd"].as_u64(), code(&o)), (Some("ended"), Some(200_000), 0), "the run did not close with the cap spent whole (the case proves nothing): {outcome} {}", stderr(&o));
    let dir = w.run_dir("r1");
    assert!(
        !dir.join("oracle.json").exists() && !dir.join("conformance").exists(),
        "the observation (a model writes the tables, 1 to 3 USD per product) was started after the cap was spent whole: {}",
        stderr(&o)
    );
    assert!(stderr(&o).contains("conformance: not observed (spend cap: 200000 of 200000 micro USD spent, nothing left)"), "the end of the run does not say so: {}", stderr(&o));
    assert_eq!(spent(&w), 200_000, "the stopped observation booked something");
}

/// A cap with room: the observation reserves from the cap what the cap has left and books after it. Its record names
/// the cost of the tables (the stand-in adapter writes none: nothing is booked and nothing stays reserved).
#[test]
fn a_run_that_closes_with_room_left_observes_and_books_what_its_record_names() {
    let w = observed_world("capg-conformance-room", "");
    let o = observe(&w, Some(1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(w.run_dir("r1").join("oracle.json").exists(), "the observation was not started with room left: {}", stderr(&o));
    let ledger: Value = serde_json::from_str(&std::fs::read_to_string(w.ledger()).unwrap()).unwrap();
    assert_eq!(ledger["spent_micro_usd"], 200_000, "{ledger}");
    assert!(ledger["reserved"].as_object().unwrap().is_empty(), "the reservation of the observation stayed: {ledger}");
}

/// An observation whose cost is not known (its record cannot be written) books its reservation, the most it could
/// have spent: all that the cap had left.
#[test]
fn an_observation_of_unknown_cost_books_its_reservation() {
    let w = observed_world("capg-conformance-unknown", "mkdir oracle.json\n");
    let o = observe(&w, Some(1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(stderr(&o).contains("conformance: not observed"), "the observation did not fail (the case proves nothing): {}", stderr(&o));
    let ledger: Value = serde_json::from_str(&std::fs::read_to_string(w.ledger()).unwrap()).unwrap();
    assert_eq!(ledger["spent_micro_usd"], 1_000_000, "{ledger}");
    assert!(ledger["reserved"].as_object().unwrap().is_empty(), "{ledger}");
}

/// With no cap named nothing is held and nothing is stopped: the observation runs as it always did.
#[test]
fn with_no_cap_named_the_observation_after_a_close_is_started() {
    let w = observed_world("capg-conformance-nocap", "");
    let o = observe(&w, None);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(w.run_dir("r1").join("oracle.json").exists(), "{}", stderr(&o));
}

/// A world whose one step leaves a product, books 0.2 USD and ends the tree, with the stand-in for ess that the
/// observation asks (as `conformance.rs`): never an `ess` of its own. `more` is added to the step's program.
fn observed_world(tag: &str, more: &str) -> World {
    use std::os::unix::fs::PermissionsExt;
    let w = World::new(tag);
    bindings(&w, MODEL_BUDGET);
    let tool = "#!/bin/sh\n[ \"$1\" = ess ] || exit 64\nshift\ncase \"$1\" in\n  --version) echo 'ess 0.0.0-stand-in' ;;\n  compile|verify) kind=$1; while [ $# -gt 0 ]; do [ \"$1\" = --out ] && out=$2; shift; done; if [ \"$kind\" = compile ]; then echo '{}' > \"$out\"; else echo '{\"scenarios\": {\"tea/pour\": {\"steps\": []}}}' > \"$out\"; fi ;;\n  *) exit 64 ;;\nesac\n";
    std::fs::create_dir_all(w.root.join("bindings/tools")).unwrap();
    std::fs::write(w.root.join("bindings/tools/sdc-tool.sh"), tool).unwrap();
    std::fs::set_permissions(w.root.join("bindings/tools/sdc-tool.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let writer = "mkdir -p work/spec work/published; echo 'name: tea' > work/spec/system.yaml; echo x > work/published/README.md\n";
    w.fake.answer("tree_run", "1", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.fake.hook("tree_run", "1", &format!("{writer}{more}{}", model_call_hook("0.2")));
    w
}

/// Runs `r1` of `w` with `--conformance observe`; `cap` is the spend cap, none named when `None`.
fn observe(w: &World, cap: Option<u64>) -> std::process::Output {
    let mut a = w.args("r1", cap.unwrap_or(0));
    if cap.is_none() {
        for flag in ["--spend-ledger", "--cap-micro-usd", "--max-llm-calls"] {
            without(&mut a, flag);
        }
    }
    a.extend(["--conformance".to_owned(), "observe".to_owned()]);
    let mut c = w.command(&a);
    c.env("SDC_CONFORMANCE_ADAPTER", "stub");
    c.output().unwrap()
}

fn answers_world(tag: &str) -> World {
    let w = super::answers::world(tag, true);
    super::answers::question_tree(&w, "questions:\n- id: q1\n  scope_changing: true\n  status: open\n  default: A\n");
    w
}

/// The decider's own call (0.02 USD) takes the cap to nothing: the programs of the answers file that follow it
/// (`apply`, `then`) are started by the runner after the cap is spent, and the approve is stopped only after them.
#[test]
fn the_programs_of_an_answers_file_are_not_started_once_the_deciders_own_cost_took_the_cap() {
    let w = answers_world("capg-answers");
    write_settings(&w, &fixture_settings(&w, 20_000));
    let o = w.run(&w.args("r1", 20_000));
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    assert_eq!(w.outcome("r1")["spent_micro_usd"].as_u64(), Some(20_000));
    let then = w.run_dir("r1/work/then.log");
    let questions = std::fs::read_to_string(w.run_dir("r1/work/questions.yaml")).unwrap_or_default();
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty(), "the approve was made");
    assert!(!questions.contains("answered_by"), "the `apply` program of the answers file was started with nothing left: {questions}");
    assert!(!then.exists(), "the `then` program of the answers file was started with nothing left");
}

/// The same decision with room left: the programs run and the decision is approved (the guard of the case above
/// must not stop a run that has money).
#[test]
fn guard_the_programs_of_an_answers_file_run_while_the_cap_has_room() {
    let w = answers_world("capg-answers-room");
    write_settings(&w, &fixture_settings(&w, 5_000_000));
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(w.run_dir("r1/work/then.log").exists());
}

/// The step books the whole cap and prints a decision that an answers file declares: the `requests` program of the
/// file is not started with nothing left (the guard at the top of `answer_declared`; the reservation of the decider
/// would stop the run a moment later, after the program ran).
#[test]
fn guard_the_requests_program_of_an_answers_file_is_not_started_when_the_step_took_the_cap() {
    let w = answers_world("capg-answers-requests");
    let hook = format!("cat > work/questions.yaml <<'EOF'\nquestions:\n- id: q1\n  scope_changing: true\n  status: open\n  default: A\nEOF\n{}", model_call_hook("0.2"));
    w.fake.hook("tree_run", "1", &hook);
    write_settings(&w, &fixture_settings(&w, 200_000));
    let o = w.run(&w.args("r1", 200_000));
    let outcome = w.outcome("r1");
    assert_eq!((code(&o), outcome["spent_micro_usd"].as_u64()), (4, Some(200_000)), "the step did not take the cap whole (the case proves nothing): {outcome} {}", stderr(&o));
    assert!(!w.run_dir("r1/work/program-env.txt").exists(), "a program of the answers file was started with nothing left");
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty());
}

/// The decider's own call takes the cap to nothing and no answers file is declared: the `tree approve` that follows
/// is not made (`take_answer` guard; the reservation before the decider cannot see the cost the decider books).
#[test]
fn guard_a_decider_whose_own_cost_takes_the_cap_is_followed_by_no_tree_approve() {
    let w = World::new("capg-decider-cost");
    bindings(&w, NO_MODEL);
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 1, None, &format!("[{}]", decision_json("owner.approve", "ApproveAction", "person:owner"))), 3);
    w.fake.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
    w.fake.answer("tree_run", "default", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.settings(&json!({"decider": {"kind": "scripted", "max_call_micro_usd": 100_000, "answers": [{"approve": true, "principal": "person:owner", "cost_micro_usd": 100_000, "model_calls": 1}]}}));
    let o = w.run(&w.args("r1", 100_000));
    let outcome = w.outcome("r1");
    assert_eq!((code(&o), outcome["spent_micro_usd"].as_u64()), (4, Some(100_000)), "the decider did not take the cap whole (the case proves nothing): {outcome} {}", stderr(&o));
    assert_eq!(w.fake.calls_of(&["tree", "approve"]).len(), 0, "a tree approve was made after the decider took the cap");
    // the run is not resumed: its directory exists, and a new run on the ledger finds nothing left
    let again = w.run(&w.args("r1", 100_000));
    assert!(code(&again) != 0 && stderr(&again).contains("exists"), "a run with the name of the stopped run was started: {}", stderr(&again));
    let fresh = w.run(&w.args("r2", 100_000));
    assert_eq!((code(&fresh), tree_runs(&w)), (4, 1), "a new run on the spent ledger started a step: {}", stderr(&fresh));
}

/// A cap of 0 and a run in which nothing can spend: one call of unknown cost that a bound program records (a
/// name the copy does not hold) books 0 under a reservation of 0, and is counted: the next step is not started.
#[test]
fn guard_a_call_of_unknown_cost_under_a_cap_of_0_is_counted_and_stops_the_next_step() {
    let w = World::new("capg-unknown-zero");
    bindings(&w, NO_MODEL);
    steps_then_end(&w, 3);
    let unknown = "w=work/.sdc/llm; mkdir -p \"$w\"\n\
i=$(( $(find \"$w\" -mindepth 1 -maxdepth 1 -type d | wc -l) + 1 ))\n\
d=$(printf '%03d-node.write' $i); mkdir -p \"$w/$d\"\n\
printf '{\"action\":\"node.write\",\"record\":\"%s/%s\",\"adapter\":\"claude\",\"exit\":1}\\n' \"$PWD/$w\" \"$d\" >> \"$w/calls.jsonl\"\n";
    w.fake.hook("tree_run", "1", unknown);
    let mut a = w.args("r1", 0);
    without(&mut a, "--spend-ledger");
    let o = w.run(&a);
    let outcome = w.outcome("r1");
    assert_eq!((code(&o), tree_runs(&w), outcome["llm_calls"].as_u64()), (4, 1, Some(1)), "{outcome} {}", stderr(&o));
}

/// A call cap that a step reached with a decision printed: no `tree approve` follows (it runs the program a
/// person action binds), as for the spend cap.
#[test]
fn the_call_cap_reached_by_a_step_starts_no_tree_approve_for_the_decision_that_step_printed() {
    let w = World::new("capg-callcap-approve");
    bindings(&w, MODEL_BUDGET);
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 1, None, &format!("[{}]", decision_json("owner.approve", "ApproveAction", "person:owner"))), 3);
    w.fake.hook("tree_run", "1", &model_call_hook("0.2"));
    w.fake.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
    w.fake.answer("tree_run", "default", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.settings(&json!({"decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [{"approve": true, "principal": "person:owner", "cost_micro_usd": 0, "model_calls": 0}]}}));
    let mut a = w.args("r1", 0);
    for flag in ["--spend-ledger", "--cap-micro-usd"] {
        without(&mut a, flag);
    }
    set_value(&mut a, "--max-llm-calls", "1");
    let o = w.run(&a);
    assert_eq!(w.fake.calls_of(&["tree", "approve"]).len(), 0, "a tree approve was made after the call cap was reached (exit {}): {}", code(&o), stderr(&o));
}

// ---- C: may_spend ----

/// What a bound program leaves in the copy during step 1, and what the run does at step 2 under a cap of 0 in a
/// run opened without a model node: `stopped` is exit 4 (the copy may name a model tool or cannot be read) or an
/// error (exit 1, the copy cannot be parsed); either way step 2 is not started.
fn copy_row(tag: &str, hook: &str, want_exit: i32, restore: Option<&str>) -> Vec<String> {
    let w = World::new(tag);
    bindings(&w, NO_MODEL);
    steps_then_end(&w, 4);
    w.fake.hook("tree_run", "1", hook);
    let mut a = w.args("r1", 0);
    without(&mut a, "--spend-ledger");
    let o = w.run(&a);
    if let Some(rel) = restore {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(w.run_dir("r1").join(rel), std::fs::Permissions::from_mode(0o755));
    }
    let mut wrong = Vec::new();
    if code(&o) != want_exit || tree_runs(&w) != 1 {
        wrong.push(format!("{tag}: exit {} (wanted {want_exit}), tree runs {} (wanted 1): {}", code(&o), tree_runs(&w), stderr(&o).lines().last().unwrap_or("")));
    }
    wrong
}

#[test]
fn a_copy_that_names_a_model_tool_in_any_form_or_cannot_be_read_starts_no_step_under_a_cap_of_0() {
    let mut wrong = Vec::new();
    let rows: Vec<(&str, String, i32, Option<&str>)> = vec![
        ("capg-copy-chmod-dir", "mkdir -p bindings/locked && chmod 000 bindings/locked\n".into(), 4, Some("bindings/locked")),
        ("capg-copy-dangling", "ln -s /nonexistent-capg bindings/dangling.yaml\n".into(), 4, None),
        ("capg-copy-dirlink", "ln -s ../work bindings/linkdir\n".into(), 4, None),
        ("capg-copy-noexecutor", "printf 'actions:\\n  - action: x\\n' > bindings/n.yaml\n".into(), 4, None),
        ("capg-copy-badkind", "printf 'actions:\\n  - action: x\\n    executor: {kind: http}\\n' > bindings/n.yaml\n".into(), 4, None),
        ("capg-copy-anchor", "printf 'x-tool: &t sdc-node\\nactions:\\n  - action: x\\n    executor: {kind: program, argv: [*t, x]}\\n' > bindings/n.yaml\n".into(), 4, None),
        ("capg-copy-escape", "printf 'actions:\\n  - action: x\\n    executor: {kind: program, argv: [\"sdc-nod\\\\u0065\", x]}\\n' > bindings/n.yaml\n".into(), 4, None),
        ("capg-copy-path", "printf 'actions:\\n  - action: x\\n    executor: {kind: program, argv: [/opt/x/sdc-node, x]}\\n' > bindings/n.yaml\n".into(), 4, None),
        ("capg-copy-nested", "mkdir -p bindings/a/b && printf 'k: [sdc-node]\\n' > bindings/a/b/deep.yml\n".into(), 4, None),
        ("capg-copy-invalid-yaml", "printf 'a: [' > bindings/bad.yaml\n".into(), 1, None),
        ("capg-copy-binary", "printf '\\377\\376' > bindings/bin.yaml\n".into(), 1, None),
    ];
    for (tag, hook, want_exit, restore) in rows {
        wrong.extend(copy_row(tag, &hook, want_exit, restore));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// A FIFO named `*.yaml` in the copy: the read of it must end, not wait for a writer.
#[test]
#[ignore = "known gap: ten readers of the copy's yaml files block on a FIFO, with or without a cap; one bounded read for all of them is its own change"]
fn a_fifo_in_the_copy_ends_the_run_and_does_not_hang() {
    let w = World::new("capg-copy-fifo");
    bindings(&w, NO_MODEL);
    steps_then_end(&w, 4);
    w.fake.hook("tree_run", "1", "mkfifo bindings/f.yaml\n");
    let mut a = w.args("r1", 0);
    without(&mut a, "--spend-ledger");
    let ended = ends_within_8_s(&mut w.command(&a));
    assert!(ended, "the runner did not end within 8 s of a FIFO named *.yaml in the copy (it was killed by its PID); tree runs {}", tree_runs(&w));
}

/// The same with no money option named: separates the reads that the caps unit added from the reads that were
/// there before it (`budget::largest_call_budget` reads the copy before every step).
#[test]
#[ignore = "known gap: ten readers of the copy's yaml files block on a FIFO, with or without a cap; one bounded read for all of them is its own change"]
fn a_fifo_in_the_copy_with_no_cap_named_ends_the_run_and_does_not_hang() {
    let w = World::new("capg-copy-fifo-nocap");
    bindings(&w, NO_MODEL);
    steps_then_end(&w, 4);
    w.fake.hook("tree_run", "1", "mkfifo bindings/f.yaml\n");
    let mut a = w.args("r1", 0);
    for flag in ["--spend-ledger", "--cap-micro-usd", "--max-llm-calls"] {
        without(&mut a, flag);
    }
    let ended = ends_within_8_s(&mut w.command(&a));
    assert!(ended, "the runner did not end within 8 s of a FIFO named *.yaml in the copy with no cap named (it was killed by its PID); tree runs {}", tree_runs(&w));
}

// ---- E: two runs, one ledger ----

/// Two runs name one ledger and a cap of 1 micro USD; each would reserve it. Exactly one starts a `tree run`:
/// the other finds nothing left. 20 races at once; the step holds the reservation for a second, so the two runs overlap.
#[test]
fn two_runs_on_one_ledger_with_one_unit_left_start_exactly_one_tree_run_in_20_races() {
    const RACES: usize = 20;
    const AT_ONCE: usize = 20;
    fn race(i: usize) -> Result<bool, String> {
        let a = World::new(&format!("capg-race-{i}-a"));
        let b = World::new(&format!("capg-race-{i}-b"));
        for w in [&a, &b] {
            bindings(w, MODEL_BUDGET);
            w.fake.answer("tree_run", "default", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
            w.fake.hook("tree_run", "default", &format!("sleep 1\n{}", model_call_hook("0.000001")));
        }
        let ledger = a.root.join("shared-ledger.json").display().to_string();
        let mut args_a = a.args("ra", 1);
        let mut args_b = b.args("rb", 1);
        set_value(&mut args_a, "--spend-ledger", &ledger);
        set_value(&mut args_b, "--spend-ledger", &ledger);
        let (mut ca, mut cb) = (a.command(&args_a), b.command(&args_b));
        ca.stdout(Stdio::null()).stderr(Stdio::null());
        cb.stdout(Stdio::null()).stderr(Stdio::null());
        let (mut pa, mut pb) = (ca.spawn().map_err(|e| e.to_string())?, cb.spawn().map_err(|e| e.to_string())?);
        let (ea, eb) = std::thread::scope(|s| {
            let ta = s.spawn(|| {
                let st = pa.wait().unwrap();
                (st.code().unwrap_or(-1), Instant::now())
            });
            let tb = s.spawn(|| {
                let st = pb.wait().unwrap();
                (st.code().unwrap_or(-1), Instant::now())
            });
            (ta.join().unwrap(), tb.join().unwrap())
        });
        let runs = tree_runs(&a) + tree_runs(&b);
        let ledger: Value = serde_json::from_str(&std::fs::read_to_string(&ledger).unwrap()).unwrap();
        let mut codes = [ea.0, eb.0];
        codes.sort();
        if runs != 1 || codes != [0, 4] || ledger["spent_micro_usd"] != 1 || ledger["reserved"].as_object().is_none_or(|r| !r.is_empty()) {
            return Err(format!("race {i}: tree runs {runs}, exits {codes:?}, ledger {ledger}"));
        }
        let (winner_end, loser_end) = if ea.0 == 0 { (ea.1, eb.1) } else { (eb.1, ea.1) };
        Ok(loser_end < winner_end)
    }
    let mut overlapped = 0;
    let mut failures = Vec::new();
    for batch in 0..RACES / AT_ONCE {
        let results: Vec<Result<bool, String>> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..AT_ONCE).map(|k| s.spawn(move || race(batch * AT_ONCE + k))).collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        for r in results {
            match r {
                Ok(true) => overlapped += 1,
                Ok(false) => {}
                Err(e) => failures.push(e),
            }
        }
    }
    let summary = format!("{RACES} races, the loser ended before the winner in {overlapped}, failures {}\n", failures.len());
    std::fs::write(std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("capg-race-summary.txt"), &summary).unwrap();
    assert!(failures.is_empty(), "{summary}{}", failures.join("\n"));
}

// ---- D: a re-drive with a live part under a cap of 0 ----

/// `--recorded` with a live part under a named cap of 0: the re-drive may call a model, so no `tree run` starts.
/// Needs `driver/target/debug` built (`sdc-replay`, `sdc-node`); no model call: the live adapter is `dry`.
#[test]
fn a_recorded_run_with_a_live_part_under_a_cap_of_0_starts_no_tree_run() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let drv = repo.join("driver/target/debug");
    assert!(drv.join("sdc-replay").is_file(), "{} is not built", drv.display());
    let rec = repo.join("driver/examples/sdc/fixtures/recorded/stub");
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("capg-recorded-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("settings.toml"), "").unwrap();
    let o = Command::new(super::core::BIN)
        .args(["run", "sdc", "--run", "r1", "--runs-dir"])
        .arg(dir.join("runs"))
        .arg("--intake")
        .arg(repo.join("driver/examples/sdc/fixtures/idea.yaml"))
        .args(["--confinement", "local", "--settings"])
        .arg(dir.join("settings.toml"))
        .args(["--spend-ledger"])
        .arg(dir.join("ledger.json"))
        .args(["--cap-micro-usd", "0", "--max-wall-seconds", "300", "--recorded"])
        .arg(&rec)
        .args(["--on-divergence", "live", "--live-adapter", "dry", "--live-cap", "1"])
        .env_clear()
        .env("PATH", format!("{}:{}", drv.display(), std::env::var("PATH").unwrap_or_default()))
        .env("HOME", std::env::var("HOME").unwrap_or_default())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let outcome: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("runs/r1/outcome.json")).unwrap_or_else(|e| panic!("no outcome: {e}: {}", String::from_utf8_lossy(&o.stderr)))).unwrap();
    assert_eq!((code(&o), outcome["outcome"].as_str(), outcome["steps"].as_u64(), outcome["has_model_node"].as_bool()), (4, Some("stopped: spend cap"), Some(0), Some(true)), "{outcome} {}", stderr(&o));
}

// ---- the observation after a close: its sessions, its cost, its records ----

fn exe(path: &std::path::Path, body: &str) {
    std::fs::write(path, body).unwrap();
    std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
}

/// A world whose one step leaves a product, books 0.2 USD and ends the tree; `ess_extra` runs inside the stand-in for ess
/// (that is: during the observation, after it was started).
fn observed_world_with_ess(tag: &str, ess_extra: &str) -> World {
    let w = World::new(tag);
    std::fs::write(w.root.join("bindings/sdc.yaml"), MODEL_BUDGET).unwrap();
    let tool = format!("#!/bin/sh\n[ \"$1\" = ess ] || exit 64\nshift\ncase \"$1\" in\n  --version) echo 'ess 0.0.0-stand-in' ;;\n  compile|verify) {ess_extra}\n kind=$1; while [ $# -gt 0 ]; do [ \"$1\" = --out ] && out=$2; shift; done; if [ \"$kind\" = compile ]; then echo '{{}}' > \"$out\"; else echo '{{\"scenarios\": {{\"tea/pour\": {{\"steps\": []}}}}}}' > \"$out\"; fi ;;\n  *) exit 64 ;;\nesac\n");
    std::fs::create_dir_all(w.root.join("bindings/tools")).unwrap();
    exe(&w.root.join("bindings/tools/sdc-tool.sh"), &tool);
    let writer = "mkdir -p work/spec work/published; echo 'name: tea' > work/spec/system.yaml; echo x > work/published/README.md\n";
    w.fake.answer("tree_run", "1", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.fake.hook("tree_run", "1", &format!("{writer}{}", super::core::model_call_hook("0.2")));
    w
}

/// Runs r1 with `--conformance observe`, a stand-in adapter `adapter_body` (a program that is given `request.json`), one table slot.
fn observe_with_adapter(w: &World, cap: Option<u64>, adapter_body: &str, more: &[&str]) -> std::process::Output {
    observe_with_adapter_env(w, cap, adapter_body, more, &[])
}

/// As [`observe_with_adapter`], with `env` set on the run (what the operator names in the environment).
fn observe_with_adapter_env(w: &World, cap: Option<u64>, adapter_body: &str, more: &[&str], env: &[(&str, &str)]) -> std::process::Output {
    let adapter = w.root.join("conform-adapter.sh");
    exe(&adapter, &format!("#!/bin/sh\necho called >> {}/adapter-calls.txt\n{adapter_body}\n", w.root.display()));
    let mut a = w.args("r1", cap.unwrap_or(0));
    if cap.is_none() {
        for flag in ["--spend-ledger", "--cap-micro-usd", "--max-llm-calls"] {
            without(&mut a, flag);
        }
    }
    a.extend(more.iter().map(|s| (*s).to_owned()));
    a.extend(["--conformance".to_owned(), "observe".to_owned()]);
    let mut c = w.command(&a);
    c.env("SDC_CONFORMANCE_ADAPTER", &adapter).env("SDC_CONFORMANCE_TABLES", "1").env("SDC_LLM_EFFORT_CONFORMANCE", "low");
    for (name, value) in env {
        c.env(name, value);
    }
    c.output().unwrap()
}

fn calls_of_adapter(w: &World) -> usize {
    std::fs::read_to_string(w.root.join("adapter-calls.txt")).map(|t| t.lines().count()).unwrap_or(0)
}

fn oracle(w: &World) -> Value {
    serde_json::from_str(&std::fs::read_to_string(w.run_dir("r1").join("oracle.json")).unwrap_or_else(|e| panic!("no oracle.json: {e}"))).unwrap()
}

/// S16: node sessions that ended with no cost record (the adapter was killed or crashed) are sessions that may have spent:
/// the same as a step's call of unknown cost, which books the reservation.
#[test]
fn an_observation_whose_sessions_ended_without_a_cost_books_its_reservation() {
    let w = observed_world_with_ess("obs-unknown", "");
    let o = observe_with_adapter(&w, Some(1_000_000), "exit 1", &[]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let doc = oracle(&w);
    assert!(calls_of_adapter(&w) > 0, "the stand-in adapter (the model) was not started, the case proves nothing: {doc}");
    assert!(doc["conformance"]["node_sessions"].as_u64().unwrap_or(0) > 0, "{doc}");
    assert_eq!(spent(&w), 1_000_000, "{} model sessions ended with no cost record and the observation booked {} micro USD (a step books its reservation): oracle table_cost_usd {}", calls_of_adapter(&w), spent(&w), doc["conformance"]["table_cost_usd"]);
}

/// A cap with room for the whole observation: what its record names (3 sessions of 0.5 USD) is booked, and nothing stays reserved.
#[test]
fn an_observation_books_the_cost_its_record_names() {
    let w = observed_world_with_ess("obs-named", "");
    let o = observe_with_adapter(&w, Some(5_000_000), "echo '{\"cost_usd\":0.5}'\nexit 1", &[]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let doc = oracle(&w);
    assert_eq!(calls_of_adapter(&w), 3, "{doc}");
    let ledger: Value = serde_json::from_str(&std::fs::read_to_string(w.ledger()).unwrap()).unwrap();
    assert_eq!(ledger["spent_micro_usd"], 200_000 + 1_500_000, "{ledger}");
    assert!(ledger["reserved"].as_object().unwrap().is_empty(), "{ledger}");
}

/// S15: what the observation spends is bounded by the cap: the sessions it starts after the reservation is passed are the
/// spend the cap should have stopped.
#[test]
fn an_observation_does_not_book_more_than_the_cap_had_left() {
    let w = observed_world_with_ess("obs-over", "");
    let o = observe_with_adapter(&w, Some(1_000_000), "echo '{\"cost_usd\":0.5}'\nexit 1", &[]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let doc = oracle(&w);
    assert!(calls_of_adapter(&w) > 0, "{doc}");
    assert!(spent(&w) <= 1_000_000, "the cap is 1000000 micro USD, the step booked 200000, the observation started {} sessions of 500000 and the ledger holds {} micro USD", calls_of_adapter(&w), spent(&w));
}

/// S7: a ledger that is garbage when the observation ends is never echoed, and the exit of the run stays.
#[test]
fn a_ledger_that_turns_to_garbage_during_the_observation_is_not_echoed_and_the_exit_stays() {
    let w = observed_world_with_ess("obs-garbage", &format!("printf 'LEDGER-GARBAGE-MARKER {{' > {}\n", w_ledger_path("obs-garbage")));
    let o = observe_with_adapter(&w, Some(1_000_000), "exit 1", &[]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let line = stderr(&o);
    assert!(!line.contains("LEDGER-GARBAGE-MARKER"), "a byte of the ledger file is in the line: {line}");
    assert!(line.contains("could not be booked"), "the failure of the booking is not said: {line}");
}

fn w_ledger_path(tag: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("loomrun-{tag}-{}", std::process::id()));
    root.join("ledger.json").display().to_string()
}


/// S9 (answers file): the decider's own call reaches the call cap: no `apply`, no `then`, no approve.
#[test]
fn a_call_cap_reached_by_the_deciders_own_call_starts_no_program_of_an_answers_file() {
    let w = answers_world("obs-callcap-answers");
    let mut settings = fixture_settings(&w, 5_000_000);
    settings["max_llm_calls"] = json!(1);
    write_settings(&w, &settings);
    let mut a = w.args("r1", 5_000_000);
    set_value(&mut a, "--max-llm-calls", "1");
    let o = w.run(&a);
    let questions = std::fs::read_to_string(w.run_dir("r1/work/questions.yaml")).unwrap_or_default();
    assert_eq!(code(&o), 6, "the decider's call did not reach the call cap (the case proves nothing): {}", stderr(&o));
    assert!(!questions.contains("answered_by"), "the `apply` program was started after the call cap was reached: {questions}");
    assert!(!w.run_dir("r1/work/then.log").exists(), "the `then` program was started after the call cap was reached");
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty(), "a tree approve was made after the call cap was reached");
}

/// S9 (decider path): a model decider whose call reaches the call cap is followed by no approve.
#[test]
fn a_call_cap_reached_by_the_decider_starts_no_tree_approve() {
    let w = World::new("obs-callcap-decider");
    std::fs::write(w.root.join("bindings/sdc.yaml"), NO_MODEL).unwrap();
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 1, None, &format!("[{}]", decision_json("owner.approve", "ApproveAction", "person:owner"))), 3);
    w.fake.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
    w.fake.answer("tree_run", "default", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.settings(&json!({"decider": {"kind": "scripted", "max_call_micro_usd": 1000, "answers": [{"approve": true, "principal": "person:owner", "cost_micro_usd": 1000, "model_calls": 1}]}}));
    let mut a = w.args("r1", 5_000_000);
    set_value(&mut a, "--max-llm-calls", "1");
    let o = w.run(&a);
    assert_eq!((code(&o), w.fake.calls_of(&["tree", "approve"]).len()), (6, 0), "{}", stderr(&o));
}

/// S8, mutant 13: another run on the ledger takes what is left while `apply` runs: `then` is not started. The check before `then`
/// is not redundant when a ledger is shared.
#[test]
fn a_second_run_that_takes_the_rest_of_the_cap_during_apply_stops_the_then_program() {
    let w = answers_world("obs-then");
    write_settings(&w, &fixture_settings(&w, 5_000_000));
    let take = format!(
        "#!/bin/sh\nprintf '{{\"format\":\"runbook.spend-ledger/1\",\"spent_micro_usd\":5000000,\"reserved\":{{}},\"entries\":[]}}' > {}\n",
        w.ledger().display()
    );
    exe(&w.root.join("bindings/tools/q-apply.sh"), &take);
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    assert!(!w.run_dir("r1/work/then.log").exists(), "the `then` program was started with nothing left");
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty());
}

/// S17 (note): a run that names a cap and no ledger counts its spend in the run itself; what the observation booked is in
/// no record of the run directory but `oracle.json` (`outcome.json` is written before it).
#[test]
fn what_an_observation_booked_is_in_the_outcome_of_a_run_without_a_ledger() {
    let w = observed_world_with_ess("obs-noledger", "");
    let mut a = w.args("r1", 5_000_000);
    without(&mut a, "--spend-ledger");
    a.extend(["--conformance".to_owned(), "observe".to_owned()]);
    let adapter = w.root.join("conform-adapter.sh");
    exe(&adapter, "#!/bin/sh\necho '{\"cost_usd\":0.5}'\nexit 1\n");
    let mut c = w.command(&a);
    c.env("SDC_CONFORMANCE_ADAPTER", &adapter).env("SDC_CONFORMANCE_TABLES", "1").env("SDC_LLM_EFFORT_CONFORMANCE", "low");
    let o = c.output().unwrap();
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let doc = oracle(&w);
    let outcome = w.outcome("r1");
    assert_eq!(outcome["spent_micro_usd"].as_u64(), Some(200_000 + 1_500_000), "the observation cost {} USD (oracle.json) and outcome.json says {}", doc["conformance"]["table_cost_usd"], outcome["spent_micro_usd"]);
}

fn event_lines(w: &World, name: &str) -> Vec<Value> {
    let text = std::fs::read_to_string(w.run_dir("r1").join("events.jsonl")).unwrap();
    text.lines().map(|l| serde_json::from_str::<Value>(l).unwrap()).filter(|e| e["event"] == name).collect()
}

/// An adapter that keeps the budget it is given: each session costs what its request allows, and the budgets are
/// written to `limits.txt`.
fn budget_keeping_adapter(w: &World) -> String {
    let body = r#"limit=$(sed -n 's/.*"max_budget_usd": *\([0-9.]*\).*/\1/p' "$1")
rec=$(sed -n 's/.*"record_dir": *"\([^"]*\)".*/\1/p' "$1")
echo "$limit" >> LIMITS
echo "{\"cost_usd\":$limit}" > "$rec/response.json"
exit 1"#;
    body.replace("LIMITS", &format!("{}/limits.txt", w.root.display()))
}

fn limits_given(w: &World) -> Vec<f64> {
    std::fs::read_to_string(w.root.join("limits.txt")).unwrap_or_default().lines().map(|l| l.parse().unwrap()).collect()
}

/// The model sessions of the observation are given the reservation as their one budget: the first session's limit is
/// what the cap has left (not the 2 USD of its own), and when it costs that, no second session starts.
#[test]
fn the_sessions_of_an_observation_are_given_the_reservation_as_their_one_budget() {
    let w = observed_world_with_ess("obs-budget", "");
    let o = observe_with_adapter(&w, Some(1_000_000), &budget_keeping_adapter(&w), &[]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(limits_given(&w), vec![0.8], "the sessions were given other budgets than what the cap had left (1 USD, the step booked 0.2): {}", stderr(&o));
    assert_eq!(spent(&w), 1_000_000);
    let doc = oracle(&w);
    assert_eq!(doc["conformance"]["table_cost_usd"].as_f64(), Some(0.8), "{doc}");
    assert_eq!(doc["conformance"]["cost_unknown_sessions"].as_u64(), Some(0), "{doc}");
}

/// With no cap named nothing bounds the sessions but their own limit, the one the environment names
/// (`SDC_CONFORMANCE_MAX_BUDGET_USD`): the observation starts as it did.
#[test]
fn with_no_cap_named_the_sessions_of_the_observation_keep_their_own_limit() {
    let w = observed_world_with_ess("obs-budget-nocap", "");
    let o = observe_with_adapter_env(&w, None, &budget_keeping_adapter(&w), &[], &[("SDC_CONFORMANCE_MAX_BUDGET_USD", "2")]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(limits_given(&w), vec![2.0, 2.0, 2.0], "{}", stderr(&o));
    assert!(event_lines(&w, "conformance_booked").is_empty(), "an observation without a cap booked something");
}

/// With no cap named and no budget named in the environment the request of a session names no money budget
/// (the story `caps-are-optional`).
#[test]
fn with_no_cap_and_no_budget_named_the_request_of_a_session_names_no_budget() {
    let w = observed_world_with_ess("obs-budget-none", "");
    let body = "grep -c max_budget_usd \"$1\" >> BUDGETS; exit 1".replace("BUDGETS", &format!("{}/named.txt", w.root.display()));
    let o = observe_with_adapter(&w, None, &body, &[]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let named = std::fs::read_to_string(w.root.join("named.txt")).unwrap_or_default();
    assert!(!named.is_empty() && named.lines().all(|l| l == "0"), "a request named a money budget with none named: {named:?} {}", stderr(&o));
}

/// A program that spends past the budget it was given is outside the bound, and the ledger still never holds more than
/// the cap because of the observation: its record names 1 USD, 0.8 USD were reserved, 0.8 USD are booked, and the line,
/// the event and `outcome.json` say both amounts.
#[test]
fn a_record_that_names_more_than_the_reservation_is_booked_at_the_reservation_and_says_so() {
    let w = observed_world_with_ess("obs-clamp", "");
    let o = observe_with_adapter(&w, Some(1_000_000), "echo '{\"cost_usd\":0.5}'\nexit 1", &[]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(stderr(&o).contains("the record names 1000000 micro USD, more than the 800000 reserved: 800000 booked"), "{}", stderr(&o));
    assert_eq!(spent(&w), 1_000_000);
    let booked = event_lines(&w, "conformance_booked");
    assert_eq!(booked.len(), 1, "{booked:?}");
    assert_eq!(
        (booked[0]["reserved_micro_usd"].as_u64(), booked[0]["record_micro_usd"].as_u64(), booked[0]["booked_micro_usd"].as_u64(), booked[0]["spent_micro_usd"].as_u64()),
        (Some(800_000), Some(1_000_000), Some(800_000), Some(1_000_000)),
        "{}",
        booked[0]
    );
    let outcome = w.outcome("r1");
    assert_eq!((outcome["spent_micro_usd"].as_u64(), outcome["observation_micro_usd"].as_u64()), (Some(1_000_000), Some(800_000)), "{outcome}");
}

/// An observation that is not started for the spend cap is one line of the event log with a fixed reason: no path, no
/// byte of a file, and no booking line.
#[test]
fn a_skipped_observation_is_one_line_of_the_event_log_with_a_fixed_reason() {
    let w = observed_world("capg-conformance-skipped", "");
    let o = observe(&w, Some(200_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let skipped = event_lines(&w, "conformance_not_observed");
    assert_eq!(skipped.len(), 1, "{skipped:?}");
    assert_eq!(skipped[0]["reason"], "spend cap: nothing left", "{}", skipped[0]);
    assert_eq!(skipped[0].as_object().map(|m| m.len()), Some(3), "the line names more than its time, its name and its reason: {}", skipped[0]);
    assert!(event_lines(&w, "conformance_booked").is_empty());
}
