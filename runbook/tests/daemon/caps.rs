//! The caps of a daemon run (`docs/daemon-api.md`, "Caps of a run"): a daemon run is never unbound.
//! The fixture runbook `caps-demo` has two program steps in a row; each stands in for a model call
//! that books a fixed 0.2 USD in `.sdc/llm/calls.jsonl` and names a per-call budget of 0.3 USD in its
//! argv. No model is called.

use super::http::send;
use super::state::scratch;
use runbook::api::routes;
use runbook::caps::{CapPolicy, Caps};
use runbook::commands::{Config, Daemon, Fault, SubmitRun};
use runbook::engine::Engine;
use runbook::runtime::Confinement;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

const RUNBOOK: &str = "caps-demo";

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/caps-runbook")
}

fn until(what: &str, secs: u64, mut done: impl FnMut() -> bool) {
    let limit = Instant::now() + Duration::from_secs(secs);
    while !done() {
        assert!(Instant::now() < limit, "gave up waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn caps(wall: u64, calls: u64, micro: u64) -> Caps {
    Caps { max_wall_seconds: wall, max_llm_calls: calls, cap_micro_usd: micro }
}

struct Rig {
    dir: PathBuf,
    d: Daemon,
}

impl Rig {
    fn new(tag: &str, policy: CapPolicy) -> Rig {
        Rig::tweaked(tag, policy, |_| {})
    }

    /// A rig whose runbook home `tweak` changes before the daemon starts.
    fn tweaked(tag: &str, policy: CapPolicy, tweak: impl FnOnce(&Path)) -> Rig {
        let dir = scratch(tag);
        let home = dir.join("home");
        copy_tree(&fixture(), &home);
        // the engine tests' runbook too: its person step is a wait on a decision
        copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &home);
        // a second runbook with the same steps, to see that a ledger belongs to one runbook
        let other = std::fs::read_to_string(home.join("protocol/caps-demo.protocol.yaml")).unwrap().replace("id: caps-demo", "id: caps-other");
        std::fs::write(home.join("protocol/caps-other.protocol.yaml"), other).unwrap();
        let bindings = std::fs::read_to_string(home.join("bindings/caps-demo.yaml")).unwrap().replace("caps-demo@1", "caps-other@1");
        std::fs::write(home.join("bindings/caps-other.yaml"), bindings).unwrap();
        std::fs::create_dir_all(dir.join("ctx")).unwrap();
        tweak(&home);
        let engine = Arc::new(Engine::with_levels(home, dir.join("state"), &[Confinement::Local], policy));
        let cfg = Config { state_dir: dir.join("state"), confinement_floor: Confinement::Local, context_roots: vec![dir.join("ctx")], runbook_dir: None, principal: super::PRINCIPAL.into() };
        Rig { d: Daemon::start(cfg, engine).unwrap(), dir }
    }

    /// A context directory whose `pause.txt` holds the seconds each step sleeps first.
    fn context(&self, tag: &str, pause: u32) -> String {
        let ctx = self.dir.join("ctx").join(tag);
        std::fs::create_dir_all(&ctx).unwrap();
        std::fs::write(ctx.join("pause.txt"), format!("{pause}\n")).unwrap();
        ctx.to_string_lossy().into_owned()
    }

    fn try_submit(&self, runbook: &str, context: Option<String>, settings: &[(&str, &str)]) -> Result<String, Fault> {
        let settings: BTreeMap<String, String> = settings.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let o = self.d.submit_run(SubmitRun { runbook_id: runbook.into(), intake: "a note on tea".into(), confinement: Some(Confinement::Local), context_path: context, settings, ..SubmitRun::default() })?;
        assert_eq!(o.name, "submitted", "{o:?}");
        Ok(o.run_id.unwrap())
    }

    fn submit(&self, runbook: &str, context: Option<String>, settings: &[(&str, &str)]) -> String {
        self.try_submit(runbook, context, settings).unwrap_or_else(|f| panic!("the submit was refused: {f:?}"))
    }

    fn row(&self, run: &str) -> Value {
        self.d.run_detail(run)[0].clone()
    }

    fn wait_ended(&self, run: &str) -> Value {
        until(&format!("run {run} to end"), 25, || matches!(self.row(run)["state"].as_str(), Some("Ended" | "Cancelled" | "FailedToStart")));
        let row = self.row(run);
        assert_eq!(row["state"], "Ended", "{row}");
        let failed: Vec<Value> = self.d.events_after(run, 0).into_iter().filter(|e| e["kind"] == "runbook.runtime-failed").collect();
        assert!(failed.is_empty(), "the runtime failed: {failed:?}\n{}", self.runlog(run));
        row
    }

    /// The run log lines of the run's tree, for a message.
    fn runlog(&self, run: &str) -> String {
        let store = self.dir.join("state/runs").join(run).join("tree");
        let mut text = String::new();
        for e in std::fs::read_dir(store).into_iter().flatten().flatten() {
            if e.file_name().to_string_lossy().starts_with("runlog-") {
                text += &std::fs::read_to_string(e.path()).unwrap_or_default();
            }
        }
        text
    }

    fn calls_made(&self, run: &str) -> Vec<String> {
        let dir = self.dir.join("state/workspaces").join(run).join(".sdc/llm");
        let mut names: Vec<String> = std::fs::read_dir(dir).into_iter().flatten().flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    fn ledger(&self, runbook: &str) -> Value {
        let path = self.dir.join("state/ledgers").join(format!("{runbook}.json"));
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
    }

    fn kinds(&self, run: &str) -> Vec<String> {
        self.d.events_after(run, 0).iter().map(|e| e["kind"].as_str().unwrap().to_string()).collect()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.d.stop();
    }
}

fn roomy() -> CapPolicy {
    CapPolicy::new(caps(60, 100, 1_000_000), caps(120, 200, 2_000_000)).unwrap()
}

fn setting<'a>(row: &'a Value, key: &str) -> Option<&'a str> {
    row["settings"][key].as_str()
}

// ---- the daemon's start ----

struct Serve {
    code: Option<i32>,
    started: bool,
    stderr: String,
}

/// `runbook serve --runtime loom` with these options: the process is given 10 s to end by itself (a
/// refused start), else it is killed and counts as started.
fn serve(tag: &str, options: &[&str]) -> Serve {
    let dir = scratch(tag);
    let mut child = Command::new(env!("CARGO_BIN_EXE_runbook"))
        .args(["serve", "--principal", super::PRINCIPAL, "--listen", "127.0.0.1:0", "--runtime", "loom"])
        .arg("--runbook-home")
        .arg(fixture())
        .arg("--state-dir")
        .arg(dir.join("state"))
        .args(options)
        .env_remove("RUNBOOK_TOKEN")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = stdout.read_line(&mut line);
        let _ = tx.send(line);
    });
    let mut stderr_pipe = child.stderr.take().unwrap();
    let (etx, erx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr_pipe.read_to_string(&mut text);
        let _ = etx.send(text);
    });
    let limit = Instant::now() + Duration::from_secs(10);
    let (code, started) = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break (status.code(), false);
        }
        if rx.try_recv().is_ok_and(|l| l.contains("listening")) {
            let _ = child.kill();
            let _ = child.wait();
            break (None, true);
        }
        if Instant::now() > limit {
            let _ = child.kill();
            let _ = child.wait();
            break (None, true);
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stderr = erx.recv_timeout(Duration::from_secs(5)).unwrap_or_default();
    Serve { code, started, stderr }
}

const FULL: [(&str, &str); 3] = [("--max-wall-seconds", "600"), ("--max-llm-calls", "50"), ("--cap-micro-usd", "5000000")];

fn without(missing: &str) -> Vec<&'static str> {
    FULL.iter().filter(|(k, _)| *k != missing).flat_map(|(k, v)| [*k, *v]).collect()
}

#[test]
fn a_daemon_started_without_a_cap_refuses_to_start_and_names_the_cap() {
    for missing in FULL.map(|(k, _)| k) {
        let got = serve(&format!("caps-start-missing{}", missing.replace("--", "-")), &without(missing));
        assert_eq!((got.code, got.started), (Some(1), false), "started without {missing}: {}", got.stderr);
        assert!(got.stderr.contains(missing), "the refusal does not name {missing}: {}", got.stderr);
        for given in FULL.map(|(k, _)| k).into_iter().filter(|k| *k != missing) {
            assert!(!got.stderr.contains(given), "the refusal names {given}, which is given: {}", got.stderr);
        }
    }
    let got = serve("caps-start-none", &[]);
    assert_eq!((got.code, got.started), (Some(1), false), "{}", got.stderr);
    for (k, _) in FULL {
        assert!(got.stderr.contains(k), "the refusal does not name {k}: {}", got.stderr);
    }
}

#[test]
fn a_daemon_with_every_cap_starts_and_a_ceiling_below_its_cap_refuses_naming_the_cap() {
    let all: Vec<&str> = FULL.iter().flat_map(|(k, v)| [*k, *v]).collect();
    let got = serve("caps-start-all", &all);
    assert!(got.started, "{:?} {}", got.code, got.stderr);
    let mut with_ceilings = all.clone();
    with_ceilings.extend(["--max-wall-seconds-ceiling", "3600", "--max-llm-calls-ceiling", "500", "--cap-micro-usd-ceiling", "9000000"]);
    let got = serve("caps-start-ceilings", &with_ceilings);
    assert!(got.started, "{:?} {}", got.code, got.stderr);
    let mut low = all;
    low.extend(["--max-llm-calls-ceiling", "10"]);
    let got = serve("caps-start-low-ceiling", &low);
    assert_eq!((got.code, got.started), (Some(1), false), "{}", got.stderr);
    assert!(got.stderr.contains("max_llm_calls") && got.stderr.contains("10") && got.stderr.contains("50"), "{}", got.stderr);
}

// ---- the submit ----

#[test]
fn a_submit_over_the_ceiling_is_refused_naming_the_cap_and_the_ceiling_and_a_lower_or_higher_cap_under_it_is_taken() {
    let rig = Rig::new("caps-submit", roomy());
    for (key, over, ceiling) in [("max_wall_seconds", "121", "120"), ("max_llm_calls", "201", "200"), ("cap_micro_usd", "2000001", "2000000")] {
        let refused = rig.try_submit(RUNBOOK, None, &[(key, over)]).unwrap_err();
        let Fault::Invalid(why) = refused else { panic!("{refused:?}") };
        assert!(why.contains(key) && why.contains(ceiling), "{why}");
    }
    let not_a_number = rig.try_submit(RUNBOOK, None, &[("max_llm_calls", "many")]).unwrap_err();
    assert!(matches!(&not_a_number, Fault::Invalid(why) if why.contains("max_llm_calls")), "{not_a_number:?}");
    assert!(rig.d.runs().is_empty(), "a refused submit made a run: {:?}", rig.d.runs());
    let lower = rig.submit(RUNBOOK, None, &[("max_llm_calls", "7")]);
    let higher = rig.submit(RUNBOOK, None, &[("max_wall_seconds", "100"), ("cap_micro_usd", "2000000")]);
    let plain = rig.submit(RUNBOOK, None, &[]);
    let (lower, higher, plain) = (rig.wait_ended(&lower), rig.wait_ended(&higher), rig.wait_ended(&plain));
    assert_eq!((setting(&lower, "max_wall_seconds"), setting(&lower, "max_llm_calls"), setting(&lower, "cap_micro_usd")), (Some("60"), Some("7"), Some("1000000")), "{lower}");
    assert_eq!((setting(&higher, "max_wall_seconds"), setting(&higher, "max_llm_calls"), setting(&higher, "cap_micro_usd")), (Some("100"), Some("100"), Some("2000000")), "{higher}");
    assert_eq!((setting(&plain, "max_wall_seconds"), setting(&plain, "max_llm_calls"), setting(&plain, "cap_micro_usd")), (Some("60"), Some("100"), Some("1000000")), "the defaults of the daemon: {plain}");
}

// ---- the caps ----

#[test]
fn a_wall_cap_ends_a_run_whose_action_sleeps_and_the_run_says_so() {
    let rig = Rig::new("caps-wall", roomy());
    let ctx = rig.context("sleepy", 8);
    let started = Instant::now();
    let run = rig.submit(RUNBOOK, Some(ctx), &[("max_wall_seconds", "1")]);
    let row = rig.wait_ended(&run);
    assert!(started.elapsed() < Duration::from_secs(20), "the sleeping action ran on: {:?}", started.elapsed());
    assert_eq!(row["outcome"], "stopped: wall time", "{row}");
    assert!(rig.kinds(&run).iter().any(|k| k == "runbook.cap-reached"), "{:?}", rig.kinds(&run));
    assert!(rig.calls_made(&run).is_empty(), "the step finished its sleep: {:?}", rig.calls_made(&run));
    assert!(setting(&row, "used_wall_seconds").is_some_and(|s| s.parse::<u64>().unwrap() >= 1), "{row}");
    let journal = std::fs::read_to_string(rig.dir.join("state/journal.jsonl")).unwrap();
    assert!(journal.contains("stopped: wall time"), "the journal does not hold the outcome");
}

#[test]
fn a_wall_cap_ends_a_run_that_waits_on_a_decision_and_withdraws_what_it_asked() {
    let rig = Rig::new("caps-wall-waiting", roomy());
    let run = rig.submit("engine-demo", None, &[("max_wall_seconds", "2")]);
    until("the run to wait on a decision", 30, || rig.row(&run)["state"] == "WaitingOnDecision");
    let row = rig.wait_ended(&run);
    assert_eq!(row["outcome"], "stopped: wall time", "{row}");
    assert!(rig.d.open_decisions_of_run(&run).is_empty(), "{:?}", rig.d.open_decisions_of_run(&run));
}

#[test]
fn a_call_cap_stops_the_run_before_the_call_over_it() {
    let rig = Rig::new("caps-calls", roomy());
    let run = rig.submit(RUNBOOK, None, &[("max_llm_calls", "1")]);
    let row = rig.wait_ended(&run);
    assert_eq!(row["outcome"], "stopped: llm calls", "{row}");
    assert_eq!(rig.calls_made(&run), ["001-step.write"], "the second call was made");
    assert_eq!((setting(&row, "used_llm_calls"), setting(&row, "max_llm_calls")), (Some("1"), Some("1")), "{row}");
    assert!(rig.kinds(&run).iter().any(|k| k == "runbook.cap-reached"), "{:?}", rig.kinds(&run));
}

#[test]
fn a_spend_cap_stops_the_run_before_the_reservation_over_it_and_the_ledger_belongs_to_the_runbook() {
    let rig = Rig::new("caps-spend", roomy());
    // 0.4 USD: the first call reserves 0.3 and books 0.2; 0.2 is left, and the second call reserves 0.3.
    let run = rig.submit(RUNBOOK, None, &[("cap_micro_usd", "400000")]);
    let row = rig.wait_ended(&run);
    assert_eq!(row["outcome"], "stopped: spend cap", "{row}");
    assert_eq!(rig.calls_made(&run), ["001-step.write"], "the second call was made");
    assert_eq!((setting(&row, "spent_micro_usd"), setting(&row, "cap_micro_usd")), (Some("200000"), Some("400000")), "{row}");
    let ledger = rig.ledger(RUNBOOK);
    assert_eq!((ledger["spent_micro_usd"].as_u64(), ledger["reserved"].as_object().map(|r| r.len())), (Some(200_000), Some(0)), "{ledger}");
    assert_eq!(ledger["entries"][0]["run"], run.as_str(), "{ledger}");
    // another runbook has its own ledger: with 0.7 USD it makes both calls and the publish (each step reserves 0.3)
    let other = rig.submit("caps-other", None, &[("cap_micro_usd", "700000")]);
    let other_row = rig.wait_ended(&other);
    assert_eq!(other_row["outcome"], "finished", "{other_row}");
    assert_eq!(rig.ledger("caps-other")["spent_micro_usd"], 400_000);
    assert_eq!(rig.ledger(RUNBOOK)["spent_micro_usd"], 200_000, "one runbook's spend reached another's ledger");
}

#[test]
fn two_runs_of_one_runbook_under_one_cap_both_finish_when_their_sum_fits() {
    let rig = Rig::new("caps-two", roomy());
    // each step sleeps 1 s, so the two runs make their calls at the same time: 4 calls of 0.2 USD, and each step reserves 0.3 USD, under a cap of 2 USD
    let ctx = rig.context("slow", 1);
    let a = rig.submit(RUNBOOK, Some(ctx.clone()), &[("cap_micro_usd", "2000000")]);
    let b = rig.submit(RUNBOOK, Some(ctx), &[("cap_micro_usd", "2000000")]);
    let (ra, rb) = (rig.wait_ended(&a), rig.wait_ended(&b));
    assert_eq!((ra["outcome"].as_str(), rb["outcome"].as_str()), (Some("finished"), Some("finished")), "{ra} {rb}");
    assert_eq!((setting(&ra, "spent_micro_usd"), setting(&rb, "spent_micro_usd")), (Some("400000"), Some("400000")), "{ra} {rb}");
    let ledger = rig.ledger(RUNBOOK);
    assert_eq!((ledger["spent_micro_usd"].as_u64(), ledger["reserved"].as_object().map(|r| r.len())), (Some(800_000), Some(0)), "{ledger}");
}

// ---- the cap of a run is the run's own ----

fn per_run(micro: u64) -> CapPolicy {
    CapPolicy::fixed(caps(120, 100, micro)).unwrap()
}

fn outcome_of(row: &Value) -> String {
    row["outcome"].as_str().unwrap_or("-").to_string()
}

#[test]
fn a_spend_cap_applies_to_each_run_and_not_to_the_runs_of_a_runbook_together() {
    // Each run books 0.4 USD (two calls of 0.2) under a cap of 1 USD: every run is under its cap by far.
    let rig = Rig::new("caps-each-run", per_run(1_000_000));
    let mut outcomes = Vec::new();
    for _ in 0..5 {
        let run = rig.submit(RUNBOOK, None, &[]);
        outcomes.push(outcome_of(&rig.wait_ended(&run)));
    }
    assert_eq!(outcomes, ["finished"; 5], "later runs of one runbook were stopped by the spend of the earlier ones; ledger: {}", rig.ledger(RUNBOOK));
}

#[test]
fn three_runs_at_once_each_under_its_own_cap_all_finish() {
    let rig = Rig::new("caps-three", per_run(1_000_000));
    let ctx = rig.context("slow", 1);
    let runs: Vec<String> = (0..3).map(|_| rig.submit(RUNBOOK, Some(ctx.clone()), &[])).collect();
    let outcomes: Vec<String> = runs.iter().map(|r| outcome_of(&rig.wait_ended(r))).collect();
    assert_eq!(outcomes, ["finished"; 3], "three concurrent runs, each of 0.4 USD under its own 1 USD cap; ledger: {}", rig.ledger(RUNBOOK));
}

#[test]
fn a_run_that_spent_less_than_its_cap_is_not_stopped_at_a_step_that_makes_no_model_call() {
    // two calls book 0.4 USD under a cap of 0.5 USD; the publish step calls no model, names no budget and costs nothing
    let rig = Rig::new("caps-publish", per_run(500_000));
    let run = rig.submit(RUNBOOK, None, &[]);
    let row = rig.wait_ended(&run);
    assert_eq!(row["outcome"], "finished", "the run spent {:?} of 500000 and was stopped at the step that costs nothing: {row}", setting(&row, "spent_micro_usd"));
}

#[test]
fn a_step_reserves_the_budget_of_its_own_action() {
    // write names 0.3 USD, check names 0.1 USD; each books 0.2. Under a cap of 0.45 USD the check is
    // held to 0.25 USD left: its own 0.1 fits, the largest of the bindings (0.3) would not.
    let rig = Rig::tweaked("caps-own-action", per_run(450_000), |home| {
        let path = home.join("bindings/caps-demo.yaml");
        let text = std::fs::read_to_string(&path).unwrap().replace("check, --max-budget-usd, \"0.3\"", "check, --max-budget-usd, \"0.1\"");
        assert!(text.contains("\"0.1\""));
        std::fs::write(&path, text).unwrap();
    });
    let run = rig.submit(RUNBOOK, None, &[]);
    let row = rig.wait_ended(&run);
    assert_eq!(row["outcome"], "finished", "{row}");
    assert_eq!(setting(&row, "spent_micro_usd"), Some("400000"), "{row}");
}

#[test]
fn a_step_reserves_the_per_call_budget_of_its_own_runbook_and_not_of_another_runbook_in_the_home() {
    // another runbook of the same home names a per-call budget of 5 USD; this runbook's calls are 0.3 USD under a cap of 1 USD
    let rig = Rig::tweaked("caps-sibling", per_run(1_000_000), |home| {
        let other = "format: loom.bindings/1\nprotocol: other@1\nprogram_root: bindings\nartifacts: {}\nactions:\n  - action: x.y\n    executor:\n      kind: program\n      argv: [tools/model-step.sh, write, --max-budget-usd, \"5\"]\n";
        std::fs::write(home.join("bindings/other.yaml"), other).unwrap();
    });
    let run = rig.submit(RUNBOOK, None, &[]);
    let row = rig.wait_ended(&run);
    assert_eq!(row["outcome"], "finished", "{row}");
}

#[test]
fn a_run_whose_bindings_name_no_per_call_budget_is_stopped_when_its_spend_reaches_the_cap() {
    // each of the two calls books 0.2 USD; the cap is 0.4 USD; nothing is reserved, so only the spend check holds the publish step
    let rig = Rig::tweaked("caps-no-budget", per_run(400_000), |home| {
        let path = home.join("bindings/caps-demo.yaml");
        let text = std::fs::read_to_string(&path).unwrap().replace(", --max-budget-usd, \"0.3\"", "");
        assert!(!text.contains("max-budget-usd"));
        std::fs::write(&path, text).unwrap();
    });
    let run = rig.submit(RUNBOOK, None, &[]);
    let row = rig.wait_ended(&run);
    assert_eq!(row["outcome"], "stopped: spend cap", "the spend had reached the cap and the next step started: {row}");
    assert_eq!(setting(&row, "spent_micro_usd"), Some("400000"), "{row}");
}

#[test]
fn a_failed_call_with_no_cost_counts_as_a_call_and_books_its_reservation() {
    let rig = Rig::tweaked("caps-failed-call", per_run(1_000_000), |home| {
        let script = "#!/bin/sh\nw=.sdc/llm\nmkdir -p \"$w\"\ni=$(( $(find \"$w\" -mindepth 1 -maxdepth 1 -type d | wc -l) + 1 ))\nd=$(printf '%03d-step.%s' \"$i\" \"$1\")\nmkdir -p \"$w/$d\"\necho '{}' > \"$w/$d/request.json\"\nprintf '{\"action\":\"step.%s\",\"adapter\":\"stand-in\",\"exit\":1,\"record\":\"%s/%s\"}\\n' \"$1\" \"$PWD/$w\" \"$d\" >> \"$w/calls.jsonl\"\n[ \"$1\" != write ] || echo note >> note.txt\n";
        std::fs::write(home.join("bindings/tools/model-step.sh"), script).unwrap();
    });
    let run = rig.submit(RUNBOOK, None, &[("max_llm_calls", "1")]);
    let row = rig.wait_ended(&run);
    assert_eq!(row["outcome"], "stopped: llm calls", "{row}");
    assert_eq!((setting(&row, "used_llm_calls"), setting(&row, "spent_micro_usd")), (Some("1"), Some("300000")), "{row}");
}

#[test]
fn the_used_numbers_are_true_after_each_step_and_a_step_with_no_model_call_books_nothing() {
    let rig = Rig::new("caps-used", per_run(1_000_000));
    let ctx = rig.context("slow", 1);
    let run = rig.submit(RUNBOOK, Some(ctx), &[]);
    until("step 1 booked", 20, || setting(&rig.row(&run), "used_llm_calls") == Some("1"));
    let mid = rig.row(&run);
    assert_eq!(setting(&mid, "spent_micro_usd"), Some("200000"), "{mid}");
    until("step 2 booked", 20, || setting(&rig.row(&run), "used_llm_calls") == Some("2"));
    assert_eq!(setting(&rig.row(&run), "spent_micro_usd"), Some("400000"));
    let row = rig.wait_ended(&run);
    assert_eq!(row["outcome"], "finished", "{row}");
    assert_eq!((setting(&row, "used_llm_calls"), setting(&row, "spent_micro_usd")), (Some("2"), Some("400000")), "the publish step booked something: {row}");
    let ledger = rig.ledger(RUNBOOK);
    assert_eq!(ledger["entries"].as_array().unwrap().len(), 2, "{ledger}");
}

// ---- a reservation that nobody will release ----

fn serve_process(dir: &Path, caps_args: &[&str]) -> (std::process::Child, std::net::SocketAddr) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_runbook"))
        .args(["serve", "--principal", super::PRINCIPAL, "--listen", "127.0.0.1:0", "--runtime", "loom", "--min-confinement", "local"])
        .arg("--runbook-home")
        .arg(fixture())
        .arg("--state-dir")
        .arg(dir.join("state"))
        .arg("--context-root")
        .arg(dir.join("ctx"))
        .args(caps_args)
        .env_remove("RUNBOOK_TOKEN")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    out.read_line(&mut line).unwrap();
    let addr = line.trim().rsplit("http://").next().unwrap().parse().unwrap_or_else(|e| panic!("{line:?}: {e}"));
    std::mem::forget(out);
    (child, addr)
}

fn post(addr: std::net::SocketAddr, command: &str, body: &Value) -> (u16, Value) {
    let r = send(addr, "POST", &routes::command_path(command), &[("Content-Type", "application/json")], Some(body.to_string().as_bytes()));
    (r.status, serde_json::from_slice(&r.body).unwrap_or(Value::Null))
}

const HELD: &str = "\"reserved\": {\n    \"";

#[test]
fn a_daemon_killed_during_a_step_leaves_no_reservation_that_stops_the_next_daemons_runs() {
    let dir = scratch("caps-killed");
    std::fs::create_dir_all(dir.join("ctx/slow")).unwrap();
    std::fs::write(dir.join("ctx/slow/pause.txt"), "20\n").unwrap();
    let args = ["--max-wall-seconds", "600", "--max-llm-calls", "50", "--cap-micro-usd", "800000"];
    let (mut first, addr) = serve_process(&dir, &args);
    let (status, body) = post(addr, "submit-run", &json!({"runbook_id": RUNBOOK, "intake": "x", "confinement": "local", "context_path": dir.join("ctx/slow").to_string_lossy()}));
    assert_eq!(status, 202, "{body}");
    let ledger = dir.join("state/ledgers").join(format!("{RUNBOOK}.json"));
    until("the first step to hold its reservation", 20, || std::fs::read_to_string(&ledger).is_ok_and(|t| t.contains(HELD)));
    first.kill().unwrap();
    first.wait().unwrap();
    let (mut second, addr) = serve_process(&dir, &args);
    let (status, body) = post(addr, "submit-run", &json!({"runbook_id": RUNBOOK, "intake": "x", "confinement": "local"}));
    assert_eq!(status, 202, "{body}");
    let run = body["published"][0]["payload"]["run_id"].as_str().unwrap().to_string();
    let mut outcome = String::new();
    until("the second daemon's run to end", 30, || {
        let r = send(addr, "GET", &format!("{}?run_id={run}", routes::view_path("run")), &[], None);
        let row = r.json()["rows"][0].clone();
        outcome = row["outcome"].as_str().unwrap_or("").to_string();
        row["state"] == "Ended"
    });
    let held = std::fs::read_to_string(&ledger).unwrap();
    second.kill().unwrap();
    second.wait().unwrap();
    assert_eq!(outcome, "finished", "the reservation of the killed daemon still counts; ledger: {held}");
}

#[test]
fn a_stopped_daemon_releases_the_reservation_of_its_running_step() {
    let rig = Rig::new("caps-stop-drain", per_run(1_000_000));
    let ctx = rig.context("slow", 20);
    let _run = rig.submit(RUNBOOK, Some(ctx), &[]);
    until("the step to hold its reservation", 20, || std::fs::read_to_string(rig.dir.join("state/ledgers/caps-demo.json")).is_ok_and(|t| t.contains(HELD)));
    rig.d.stop();
    assert_eq!(rig.ledger(RUNBOOK)["reserved"], json!({}), "{}", rig.ledger(RUNBOOK));
}

// ---- the wall cap against a program that ignores SIGTERM ----

fn pid_alive(pid: i32) -> bool {
    // SAFETY: signal 0 only tests for the process.
    unsafe { libc::kill(pid, 0) == 0 }
}

fn deaf_program(home: &Path) {
    std::fs::write(home.join("bindings/tools/model-step.sh"), "#!/bin/sh\necho $$ > deaf.pid\ntrap '' TERM HUP INT\nsleep 40\n").unwrap();
}

fn pid_of(rig: &Rig, run: &str) -> i32 {
    let file = rig.dir.join("state/workspaces").join(run).join("deaf.pid");
    until("the program to write its pid", 15, || std::fs::read_to_string(&file).is_ok_and(|t| t.trim().parse::<i32>().is_ok()));
    std::fs::read_to_string(&file).unwrap().trim().parse().unwrap()
}

#[test]
fn a_wall_cap_ends_a_run_whose_program_ignores_sigterm_and_ends_the_program() {
    let rig = Rig::tweaked("caps-deaf-wall", per_run(1_000_000), deaf_program);
    let started = Instant::now();
    let run = rig.submit(RUNBOOK, None, &[("max_wall_seconds", "2")]);
    let pid = pid_of(&rig, &run);
    let row = rig.wait_ended(&run);
    let took = started.elapsed();
    let alive = pid_alive(pid);
    if alive {
        // SAFETY: the stand-in program of this case.
        unsafe { libc::kill(pid, libc::SIGKILL) };
    }
    assert_eq!(row["outcome"], "stopped: wall time", "after {took:?}: {row}");
    assert!(!alive, "the program (pid {pid}) is still running after the run ended ({took:?})");
}

#[test]
fn a_client_cancel_ends_a_program_that_ignores_sigterm() {
    let rig = Rig::tweaked("caps-deaf-cancel", per_run(1_000_000), deaf_program);
    let run = rig.submit(RUNBOOK, None, &[]);
    let pid = pid_of(&rig, &run);
    rig.d.cancel_run(runbook::commands::CancelRun { run_id: run.clone() }).unwrap();
    let limit = Instant::now() + Duration::from_secs(10);
    while pid_alive(pid) && Instant::now() < limit {
        std::thread::sleep(Duration::from_millis(50));
    }
    let alive = pid_alive(pid);
    if alive {
        // SAFETY: the stand-in program of this case.
        unsafe { libc::kill(pid, libc::SIGKILL) };
    }
    assert!(!alive, "the program (pid {pid}) is still running 10 s after the cancel");
}

// ---- the refusals ----

#[test]
fn a_cap_that_is_not_a_whole_number_of_one_or_more_is_refused_naming_the_cap_and_no_path() {
    let rig = Rig::new("caps-values", per_run(1_000_000));
    let bad = ["0", "-1", "+0", "", " ", "1.5", "1e3", "abc", "0x10", "٣", "18446744073709551616", "99999999999999999999999999", "5 6", "NaN", "∞"];
    for key in ["max_wall_seconds", "max_llm_calls", "cap_micro_usd"] {
        for value in bad {
            match rig.try_submit(RUNBOOK, None, &[(key, value)]) {
                Err(Fault::Invalid(why)) => {
                    assert!(why.contains(key), "{key}={value:?}: the refusal does not name the cap: {why}");
                    assert!(!why.contains("/home") && !why.contains("state/"), "{why}");
                }
                other => panic!("{key}={value:?} was not refused: {other:?}"),
            }
        }
    }
    assert!(rig.d.runs().is_empty(), "a refused submit made a run: {:?}", rig.d.runs());
    for (key, over, ceiling) in [("max_wall_seconds", "121", "120"), ("max_llm_calls", "101", "100"), ("cap_micro_usd", "1000001", "1000000")] {
        let Err(Fault::Invalid(why)) = rig.try_submit(RUNBOOK, None, &[(key, over)]) else { panic!("{key} over the ceiling was not refused") };
        assert!(why.contains(key) && why.contains(ceiling), "{why}");
    }
}

fn http_rig(tag: &str) -> (Rig, tokio::runtime::Runtime, std::net::SocketAddr) {
    let rig = Rig::new(tag, CapPolicy::fixed(caps(120, 100, 1_000_000)).unwrap());
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let listener = rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
    let addr = listener.local_addr().unwrap();
    let daemon = rig.d.clone();
    rt.spawn(async move {
        let _ = runbook::api::serve(listener, daemon, None, std::future::pending()).await;
    });
    (rig, rt, addr)
}

#[test]
fn a_submit_with_a_cap_of_the_wrong_json_type_is_refused_naming_the_cap_over_http() {
    let (rig, rt, addr) = http_rig("caps-http-types");
    for (key, wrong) in [("max_llm_calls", json!(5)), ("max_wall_seconds", json!(1.5)), ("cap_micro_usd", json!(true)), ("max_llm_calls", json!(null)), ("max_wall_seconds", json!([1])), ("cap_micro_usd", json!({"n": 1}))] {
        let (status, body) = post(addr, "submit-run", &json!({"runbook_id": RUNBOOK, "intake": "x", "confinement": "local", "settings": {key: wrong}}));
        assert_eq!(status, 400, "{key}={wrong}: {body}");
        assert!(body.to_string().contains(key), "the refusal of {key}={wrong} does not name the cap: {body}");
    }
    assert!(rig.d.runs().is_empty());
    rt.shutdown_background();
}

#[test]
fn a_cap_named_twice_in_the_body_is_held_to_the_ceiling() {
    let (rig, rt, addr) = http_rig("caps-twice");
    for body in [
        r#"{"runbook_id":"caps-demo","intake":"x","confinement":"local","settings":{"max_llm_calls":"1","max_llm_calls":"999999"}}"#,
        r#"{"runbook_id":"caps-demo","intake":"x","confinement":"local","settings":{"max_llm_calls":"999999","max_llm_calls":"1"}}"#,
    ] {
        let r = send(addr, "POST", &routes::command_path("submit-run"), &[("Content-Type", "application/json")], Some(body.as_bytes()));
        if r.status == 202 {
            let run = r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_string();
            let row = rig.row(&run);
            let cap: u64 = setting(&row, "max_llm_calls").unwrap().parse().unwrap();
            assert!(cap <= 100, "a cap over the ceiling took: {row}");
        } else {
            assert_eq!(r.status, 400, "{}", r.text());
        }
    }
    rt.shutdown_background();
}

#[test]
fn a_daemon_with_a_zero_negative_overflowing_or_doubled_cap_or_a_low_ceiling_does_not_start_and_says_which() {
    let all = ["--max-wall-seconds", "600", "--max-llm-calls", "50", "--cap-micro-usd", "5000000"];
    let with = |extra: &[&'static str]| -> Vec<&'static str> {
        let mut v: Vec<&'static str> = all.to_vec();
        v.extend_from_slice(extra);
        v
    };
    let mut report = Vec::new();
    for (tag, options, names) in [
        ("zero", vec!["--max-wall-seconds", "0", "--max-llm-calls", "50", "--cap-micro-usd", "5000000"], "max-wall-seconds"),
        ("negative", vec!["--max-wall-seconds", "600", "--max-llm-calls", "-1", "--cap-micro-usd", "5000000"], "-1"),
        ("overflow", vec!["--max-wall-seconds", "600", "--max-llm-calls", "50", "--cap-micro-usd", "18446744073709551616"], "cap-micro-usd"),
        ("empty", vec!["--max-wall-seconds", "", "--max-llm-calls", "50", "--cap-micro-usd", "5000000"], "max-wall-seconds"),
        ("twice", with(&["--max-wall-seconds", "1"]), "max-wall-seconds"),
        ("ceiling-zero", with(&["--cap-micro-usd-ceiling", "0"]), "cap-micro-usd-ceiling"),
        ("ceiling-low-wall", with(&["--max-wall-seconds-ceiling", "599"]), "max_wall_seconds"),
        ("ceiling-low-spend", with(&["--cap-micro-usd-ceiling", "4999999"]), "cap_micro_usd"),
    ] {
        let got = serve(&format!("caps-start-{tag}"), &options);
        let named = got.stderr.contains(names);
        let pathless = !got.stderr.contains("/home") && !got.stderr.contains("state");
        if got.started || !named || !pathless || got.code.is_none() {
            report.push(format!("{tag}: code {:?} started {} names-{names} {named} no-path {pathless}: {}", got.code, got.started, got.stderr.lines().next().unwrap_or("")));
        }
    }
    let huge = serve("caps-start-huge", &["--max-wall-seconds", "18446744073709551615", "--max-llm-calls", "18446744073709551615", "--cap-micro-usd", "18446744073709551615"]);
    assert!(huge.started, "{:?} {}", huge.code, huge.stderr);
    assert!(report.is_empty(), "{}", report.join("\n"));
}

#[test]
fn a_daemon_started_with_only_its_caps_refuses_a_submit_over_each_cap_naming_the_ceiling() {
    let dir = scratch("caps-ceiling-default");
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    let args = ["--max-wall-seconds", "600", "--max-llm-calls", "50", "--cap-micro-usd", "5000000"];
    let (mut daemon, addr) = serve_process(&dir, &args);
    let mut report = Vec::new();
    for (key, over, ceiling) in [("max_wall_seconds", "601", "600"), ("max_llm_calls", "51", "50"), ("cap_micro_usd", "5000001", "5000000")] {
        let (status, body) = post(addr, "submit-run", &json!({"runbook_id": RUNBOOK, "intake": "x", "confinement": "local", "settings": {key: over}}));
        let text = body.to_string();
        if status != 400 || !text.contains(key) || !text.contains(ceiling) {
            report.push(format!("{key}={over}: {status} {text}"));
        }
    }
    let _ = daemon.kill();
    let _ = daemon.wait();
    assert!(report.is_empty(), "{}", report.join("\n"));
}
