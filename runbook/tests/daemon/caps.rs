//! The caps of a daemon run (`docs/daemon-api.md`, "Caps of a run"): a daemon run is never unbound.
//! The fixture runbook `caps-demo` has two program steps in a row; each stands in for a model call
//! that books a fixed 0.2 USD in `.sdc/llm/calls.jsonl` and names a per-call budget of 0.3 USD in its
//! argv. No model is called.

use super::state::scratch;
use runbook::caps::{CapPolicy, Caps};
use runbook::commands::{Config, Daemon, Fault, SubmitRun};
use runbook::engine::Engine;
use runbook::runtime::Confinement;
use serde_json::Value;
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
