//! The engine (`runbook::engine`): a case tree run through Loom's library calls inside the daemon's
//! process. The fixture runbook has two levels: a root case that opens a drafting case; the drafting
//! case has a writer (a POSIX sh program) and one person step with the named results `performed` and
//! `rejected`.

use super::state::scratch;
use runbook::clock::SetClock;
use runbook::commands::{AnswerDecision, CancelRun, Config, Daemon, SubmitRun};
use runbook::engine::{Engine, StoreState};
use runbook::runtime::{AnswerFailure, Confinement, DecisionRequest, EventSink, Finish, Recovery, RunbookInfo, Runtime, RuntimeFailure, StartRequest, StartTarget, Started};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const RUNBOOK: &str = "engine-demo";
const FORBIDDEN: [&str; 4] = ["b10x-loom", "loom", "canon", concat!("sdc-dr", "ive")];

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

fn listing(dir: &Path) -> BTreeSet<String> {
    fn walk(dir: &Path, base: &Path, out: &mut BTreeSet<String>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            let rel = e.path().strip_prefix(base).unwrap().to_string_lossy().into_owned();
            if e.file_type().unwrap().is_dir() {
                walk(&e.path(), base, out);
            } else {
                out.insert(format!("{rel} {}", std::fs::metadata(e.path()).unwrap().len()));
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(dir, dir, &mut out);
    out
}

/// The command names of every process below this one.
fn descendants() -> Vec<String> {
    let mut table: Vec<(u32, u32, String)> = Vec::new();
    for e in std::fs::read_dir("/proc").unwrap().flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else { continue };
        let Ok(stat) = std::fs::read_to_string(e.path().join("stat")) else { continue };
        let (Some(open), Some(close)) = (stat.find('('), stat.rfind(')')) else { continue };
        let ppid = stat[close + 1..].split_whitespace().nth(1).and_then(|p| p.parse::<u32>().ok()).unwrap_or(0);
        table.push((pid, ppid, stat[open + 1..close].to_string()));
    }
    let mut below: BTreeSet<u32> = BTreeSet::from([std::process::id()]);
    loop {
        let more: Vec<u32> = table.iter().filter(|(p, pp, _)| below.contains(pp) && !below.contains(p)).map(|(p, _, _)| *p).collect();
        if more.is_empty() {
            break;
        }
        below.extend(more);
    }
    table.into_iter().filter(|(p, _, _)| *p != std::process::id() && below.contains(p)).map(|(_, _, c)| c).collect()
}

fn until(what: &str, secs: u64, mut done: impl FnMut() -> bool) {
    let limit = Instant::now() + Duration::from_secs(secs);
    while !done() {
        assert!(Instant::now() < limit, "gave up waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

struct Rig {
    dir: PathBuf,
    home: PathBuf,
    levels: Vec<Confinement>,
    engine: Arc<Engine>,
    d: Option<Daemon>,
}

impl Rig {
    fn new(tag: &str, levels: &[Confinement]) -> Rig {
        Rig::build(tag, levels, false)
    }

    /// `hold`: the writer's binding carries a `handoff` text and a `handoff_when` check
    /// (`tools/draft-hold.sh`: sleeps the seconds of `pause.txt`, exits 1), so a check runs before
    /// the writer's step.
    fn build(tag: &str, levels: &[Confinement], hold: bool) -> Rig {
        let dir = scratch(tag);
        let home = dir.join("home");
        copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &home);
        // A person step is answered as the principal its binding names: here, the daemon's.
        let binding = home.join("bindings/engine-draft.yaml");
        let mut text = std::fs::read_to_string(&binding).unwrap();
        assert!(text.contains("principal: reviewer"));
        text = text.replace("principal: reviewer", &format!("principal: {}", super::PRINCIPAL));
        if hold {
            let writer = "      argv: [tools/draft-write.sh]\n";
            assert!(text.contains(writer));
            text = text.replace(writer, &format!("{writer}    handoff: Write the draft by hand.\n    handoff_when: [tools/draft-hold.sh]\n"));
        }
        std::fs::write(&binding, text).unwrap();
        std::fs::create_dir_all(dir.join("ctx")).unwrap();
        let engine = Arc::new(Engine::with_levels(home.clone(), dir.join("state"), levels));
        let mut rig = Rig { dir, home, levels: levels.to_vec(), engine, d: None };
        rig.d = Some(Daemon::start(rig.config(), rig.engine.clone()).unwrap());
        rig
    }

    fn config(&self) -> Config {
        Config { state_dir: self.dir.join("state"), confinement_floor: Confinement::Local, context_roots: vec![self.dir.join("ctx")], runbook_dir: None, principal: super::PRINCIPAL.into() }
    }

    fn d(&self) -> &Daemon {
        self.d.as_ref().unwrap()
    }

    /// Drop the daemon and its engine and start both again on the same directory.
    fn restart(&mut self) {
        self.d.take().unwrap().stop();
        self.engine = Arc::new(Engine::with_levels(self.home.clone(), self.dir.join("state"), &self.levels));
        self.d = Some(Daemon::start(self.config(), self.engine.clone()).unwrap());
    }

    /// A context directory whose `pause.txt` holds the seconds the writer sleeps first.
    fn context(&self, pause: u32) -> String {
        let ctx = self.dir.join("ctx").join("project");
        std::fs::create_dir_all(&ctx).unwrap();
        std::fs::write(ctx.join("pause.txt"), format!("{pause}\n")).unwrap();
        ctx.to_string_lossy().into_owned()
    }

    fn submit_at(&self, confinement: Confinement, context: Option<String>) -> String {
        let o = self.d().submit_run(SubmitRun { runbook_id: RUNBOOK.into(), intake: "a note on tea".into(), confinement: Some(confinement), context_path: context, ..SubmitRun::default() }).unwrap();
        assert_eq!(o.name, "submitted", "{o:?}");
        o.run_id.unwrap()
    }

    fn submit(&self) -> String {
        self.submit_at(Confinement::Local, None)
    }

    fn state_of(&self, run: &str) -> String {
        self.d().run_detail(run)[0]["state"].as_str().unwrap().to_string()
    }

    fn wait_state(&self, run: &str, want: &str) {
        until(&format!("run {run} to be {want}"), 40, || {
            let now = self.state_of(run);
            assert!(now == want || !matches!(now.as_str(), "Ended" | "Cancelled" | "FailedToStart"), "run {run} is {now} and not {want}: {}", self.d().run_detail(run)[0]);
            now == want
        });
    }

    fn row(&self, run: &str) -> Value {
        self.d().run_detail(run)[0].clone()
    }

    fn workspace(&self, run: &str) -> PathBuf {
        self.dir.join("state").join("workspaces").join(run)
    }

    fn store(&self, run: &str) -> PathBuf {
        self.dir.join("state").join("runs").join(run).join("tree")
    }

    fn answer(&self, run: &str, request: &str, answer: &str, principal: &str) {
        let o = self.d().answer_decision(AnswerDecision { run_id: run.into(), request_id: request.into(), answer: answer.into(), answered_by: principal.into() }).unwrap();
        assert!(!o.is_refusal(), "{o:?}");
    }

    fn cancel(&self, run: &str) {
        let o = self.d().cancel_run(CancelRun { run_id: run.into() }).unwrap();
        assert!(!o.is_refusal(), "{o:?}");
    }

    /// Answer what the run asks until it is over: the person step with the next of `reviews` (a
    /// result name, or a JSON answer), every other decision with `approve`.
    fn settle(&self, run: &str, reviews: &[&str]) {
        let mut reviews = reviews.iter();
        let mut answered: BTreeSet<String> = BTreeSet::new();
        until(&format!("run {run} to be over"), 50, || {
            if matches!(self.state_of(run).as_str(), "Ended" | "Cancelled" | "FailedToStart") {
                return true;
            }
            for d in self.d().open_decisions_of_run(run) {
                let id = d["request_id"].as_str().unwrap().to_string();
                if !answered.insert(id.clone()) {
                    continue;
                }
                let review = d["options"].as_array().unwrap().iter().any(|o| o["answer"].as_str().is_some_and(|a| a.contains("result")));
                if review {
                    self.answer(run, &id, reviews.next().copied().unwrap_or("performed"), "reviewer");
                } else {
                    self.answer(run, &id, "approve", "owner");
                }
            }
            false
        });
    }

    fn wait_review(&self, run: &str) -> String {
        let mut found = None;
        until(&format!("run {run} to wait on the review"), 40, || {
            found = self.d().open_decisions_of_run(run).into_iter().find(|d| d["options"].as_array().unwrap().iter().any(|o| o["answer"].as_str().is_some_and(|a| a.contains("result"))));
            found.is_some()
        });
        found.unwrap()["request_id"].as_str().unwrap().to_string()
    }

    fn kinds(&self, run: &str) -> Vec<String> {
        self.d().events_after(run, 0).iter().map(|e| e["kind"].as_str().unwrap().to_string()).collect()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        if let Some(d) = &self.d {
            d.stop();
        }
    }
}

#[test]
fn a_run_is_delivered_after_the_approvals() {
    let rig = Rig::new("engine-delivered", &[Confinement::Local]);
    assert!(rig.engine.knows_runbook(RUNBOOK, None));
    assert!(!rig.engine.knows_runbook("no-such-runbook", None));
    let run = rig.submit();
    rig.settle(&run, &["performed"]);
    let row = rig.row(&run);
    assert_eq!((row["state"].as_str(), row["outcome"].as_str()), (Some("Ended"), Some("delivered")), "{row}");
    assert_eq!(std::fs::read_to_string(rig.workspace(&run).join("draft.txt")).unwrap(), "draft\n");
    assert!(std::fs::read_to_string(rig.workspace(&run).join("idea.yaml")).unwrap().contains("a note on tea"));
    assert!(rig.d().decisions_of_run(&run).iter().all(|d| d["state"] != "Open"));
}

#[test]
fn a_rejection_with_reasons_sends_the_writer_back() {
    let rig = Rig::new("engine-rejected", &[Confinement::Local]);
    let run = rig.submit();
    let first = rig.wait_review(&run);
    assert_eq!(std::fs::read_to_string(rig.workspace(&run).join("draft.txt")).unwrap(), "draft\n");
    rig.answer(&run, &first, r#"{"result":"rejected","reasons":"too short, add the tea"}"#, "reviewer");
    until("the writer's second draft", 40, || std::fs::read_to_string(rig.workspace(&run).join("draft.txt")).is_ok_and(|t| t == "draft\ndraft\n"));
    let feedback = std::fs::read_to_string(rig.workspace(&run).join("feedback.json")).unwrap();
    assert!(feedback.contains("too short, add the tea"), "{feedback}");
    let second = {
        let mut next = None;
        until("a second review request", 40, || {
            next = rig.d().open_decisions_of_run(&run).into_iter().map(|d| d["request_id"].as_str().unwrap().to_string()).find(|id| *id != first);
            next.is_some()
        });
        next.unwrap()
    };
    rig.answer(&run, &second, "performed", "reviewer");
    rig.settle(&run, &[]);
    let row = rig.row(&run);
    assert_eq!((row["state"].as_str(), row["outcome"].as_str()), (Some("Ended"), Some("delivered")), "{row}");
}

#[test]
fn a_cancel_while_the_run_waits_ends_it_and_leaves_the_store_idle() {
    let rig = Rig::new("engine-cancel-waiting", &[Confinement::Local]);
    let run = rig.submit();
    rig.wait_review(&run);
    rig.cancel(&run);
    rig.wait_state(&run, "Cancelled");
    until("the engine to let go of the run", 10, || rig.engine.store_state(&run) == StoreState::Idle);
    assert!(rig.d().open_decisions_of_run(&run).is_empty());
}

#[test]
fn a_cancel_while_a_program_runs_ends_the_program_and_the_run_and_starts_no_later_step() {
    // Loom's cancel handle ends a running bound program (docs/engine.md, "Cancel and recover"): the
    // run is Cancelled, the program's process is gone, and no step after the writer starts.
    let rig = Rig::new("engine-cancel-running", &[Confinement::Local]);
    let run = rig.submit_at(Confinement::Local, Some(rig.context(4)));
    until("the writer to sleep", 40, || descendants().iter().any(|c| c == "sleep"));
    assert_eq!(rig.state_of(&run), "Running");
    rig.cancel(&run);
    rig.wait_state(&run, "Cancelled");
    until("the program to end", 20, || !descendants().iter().any(|c| c == "sleep"));
    until("the engine to let go of the run", 20, || rig.engine.store_state(&run) == StoreState::Idle);
    assert!(rig.d().open_decisions_of_run(&run).is_empty());
    assert_eq!(rig.row(&run)["state"], "Cancelled");
    let (mut writer_seen, mut later_steps) = (false, Vec::new());
    for e in std::fs::read_dir(rig.store(&run)).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with("runlog-") && name.ends_with(".jsonl") {
            let lines: Vec<Value> = std::fs::read_to_string(e.path()).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
            if let Some(at) = lines.iter().position(|l| l["action"] == "draft.write") {
                writer_seen = true;
                later_steps.extend(lines[at..].iter().filter(|l| !l["action"].is_null() && l["action"] != "draft.write").map(Value::to_string));
            }
        }
    }
    assert!(writer_seen, "the writer never ran, so the cancel ended nothing");
    assert!(later_steps.is_empty(), "a step ran after the cancel: {later_steps:?}");
}

#[test]
fn a_cancel_while_a_handoff_check_runs_ends_the_check_and_the_run_and_starts_no_later_step() {
    // Loom's `handoff_when` check holds the run's cancel (Loom 5b3142c7): the check ends, the run is
    // Cancelled, and the writer whose check it was never starts. The check would sleep 30 s.
    let rig = Rig::build("engine-cancel-check", &[Confinement::Local], true);
    let run = rig.submit_at(Confinement::Local, Some(rig.context(30)));
    until("the check to sleep", 40, || descendants().iter().any(|c| c == "sleep"));
    assert_eq!(rig.state_of(&run), "Running");
    let at = Instant::now();
    rig.cancel(&run);
    rig.wait_state(&run, "Cancelled");
    until("the check to end", 10, || !descendants().iter().any(|c| c == "sleep"));
    assert!(at.elapsed() < Duration::from_secs(5), "the cancel took {:?}", at.elapsed());
    until("the engine to let go of the run", 10, || rig.engine.store_state(&run) == StoreState::Idle);
    assert!(rig.d().open_decisions_of_run(&run).is_empty());
    let (mut checked, mut stopped, mut started) = (false, false, Vec::new());
    for e in std::fs::read_dir(rig.store(&run)).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with("runlog-") && name.ends_with(".jsonl") {
            for line in std::fs::read_to_string(e.path()).unwrap().lines() {
                let l: Value = serde_json::from_str(line).unwrap();
                match l["kind"].as_str() {
                    Some("HandoffChecked") => checked |= l["action"] == "draft.write" && l["exit"].is_null() && l["refusal"].as_str().is_some_and(|r| r.starts_with("Cancelled: action draft.write: the check")),
                    Some("Stopped") => stopped |= l["stop"] == "Cancelled" && l["next"] == serde_json::json!(["draft.write"]),
                    Some("ActionStarted") if l["action"] == "draft.write" => started.push(line.to_owned()),
                    _ => {}
                }
            }
        }
    }
    assert!(checked, "no HandoffChecked line holds the cancelled check's refusal and no exit");
    assert!(stopped, "no Stopped line says Cancelled with next [draft.write]");
    assert!(started.is_empty(), "the writer started after the cancel: {started:?}");
    assert!(!rig.workspace(&run).join("draft.txt").exists(), "the writer wrote a draft");
}

#[test]
fn every_run_log_line_reaches_the_record_unchanged() {
    let rig = Rig::new("engine-record", &[Confinement::Local]);
    let run = rig.submit();
    rig.settle(&run, &["rejected", "performed"]);
    assert_eq!(rig.row(&run)["outcome"], "delivered");
    let mut logged: Vec<String> = Vec::new();
    let mut files = 0;
    for e in std::fs::read_dir(rig.store(&run)).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with("runlog-") && name.ends_with(".jsonl") {
            files += 1;
            for line in std::fs::read_to_string(e.path()).unwrap().lines() {
                logged.push(serde_json::from_str::<Value>(line).unwrap().to_string());
            }
        }
    }
    assert!(files >= 2, "one log per case: {files}");
    let mut reported: Vec<String> = rig.d().events_after(&run, 0).iter().filter(|e| !e["kind"].as_str().unwrap().starts_with("runbook.")).map(|e| e["record"].to_string()).collect();
    logged.sort();
    reported.sort();
    assert!(!logged.is_empty());
    assert_eq!(reported, logged);
}

#[test]
fn the_engine_acts_as_the_daemons_principal_and_never_as_the_label_a_client_types() {
    let rig = Rig::new("engine-principal", &[Confinement::Local]);
    let run = rig.submit();
    let review = rig.wait_review(&run);
    rig.answer(&run, &review, "performed", "person:someone-else");
    rig.settle(&run, &[]);
    assert_eq!(rig.row(&run)["outcome"], "delivered");
    // The run's record in Loom's store: the run logs, the cases and the observations (who acted).
    let mut log = String::new();
    for e in std::fs::read_dir(rig.store(&run)).unwrap().flatten() {
        if e.path().is_file() && !e.file_name().to_string_lossy().ends_with(".lock") {
            log.push_str(&std::fs::read_to_string(e.path()).unwrap());
        }
    }
    let actors = log.matches(&format!("\"{}\"", super::PRINCIPAL)).count();
    assert!(actors >= 2, "the daemon's principal acts in the run's store: {actors} times");
    assert!(!log.contains("someone-else"), "the typed label reached the run's store");
    let answered = rig.d().decisions_of_run(&run).into_iter().filter(|d| d["answered_by"] == "person:someone-else").count();
    assert_eq!(answered, 1, "the label stays in the daemon's record of the answer");
}

#[test]
fn a_dropped_engine_leaves_a_store_and_the_run_ends_as_orphaned() {
    let mut rig = Rig::new("engine-recover", &[Confinement::Local]);
    let run = rig.submit();
    rig.wait_review(&run);
    let before = listing(&rig.store(&run));
    assert_eq!(rig.engine.store_state(&run), StoreState::Idle);
    rig.restart();
    let row = rig.row(&run);
    assert_eq!((row["state"].as_str(), row["outcome"].as_str()), (Some("Ended"), Some("orphaned")), "{row}");
    assert_eq!(rig.engine.recover(&run), Recovery::Gone);
    assert_eq!(rig.engine.store_state(&run), StoreState::Idle);
    assert_eq!(listing(&rig.store(&run)), before, "recovery wrote into the store");
    assert!(rig.d().open_decisions_of_run(&run).is_empty());
}

#[test]
fn a_confinement_level_the_engine_cannot_give_is_refused_before_anything_starts() {
    let rig = Rig::new("engine-confinement", &[Confinement::Local]);
    let home_before = listing(&rig.home);
    let run = rig.submit_at(Confinement::Linux, None);
    rig.wait_state(&run, "FailedToStart");
    let failure = rig.row(&run)["failure"].as_str().unwrap().to_string();
    assert!(failure.contains("linux"), "{failure}");
    assert!(!rig.store(&run).exists(), "a store was made");
    assert!(!rig.workspace(&run).join("draft.txt").exists());
    assert!(rig.kinds(&run).iter().all(|k| k.starts_with("runbook.")), "{:?}", rig.kinds(&run));
    assert_eq!(listing(&rig.home), home_before);
}

#[test]
fn only_the_programs_of_the_runbook_run_and_nothing_is_written_beside_the_state_directory() {
    let rig = Rig::new("engine-sample", &[Confinement::Local]);
    let home_before = listing(&rig.home);
    let run = rig.submit_at(Confinement::Local, Some(rig.context(3)));
    let mut seen: BTreeSet<String> = BTreeSet::new();
    until("the run to end", 50, || {
        seen.extend(descendants());
        match rig.state_of(&run).as_str() {
            "WaitingOnDecision" => {
                for d in rig.d().open_decisions_of_run(&run) {
                    let id = d["request_id"].as_str().unwrap().to_string();
                    let review = d["options"].as_array().unwrap().iter().any(|o| o["answer"].as_str().is_some_and(|a| a.contains("result")));
                    rig.answer(&run, &id, if review { "performed" } else { "approve" }, if review { "reviewer" } else { "owner" });
                }
                false
            }
            s => s == "Ended",
        }
    });
    assert!(seen.contains("sleep"), "the sample saw {seen:?}");
    for name in &seen {
        assert!(!FORBIDDEN.contains(&name.as_str()), "a forbidden process ran: {seen:?}");
        assert!(matches!(name.as_str(), "sh" | "sleep" | "cp" | "cat" | "draft-write.sh" | "draft-check.sh"), "an unexpected process ran: {name} in {seen:?}");
    }
    assert_eq!(listing(&rig.home), home_before, "the runbook directory was written into");
    assert!(rig.store(&run).starts_with(rig.dir.join("state")));
}

// ---- the step in flight ----

const STARTED: &str = "runbook.action-started";

impl Rig {
    /// The row of the view `run` as `GET /daemon/views/run` and the MCP tool `read_run` answer it.
    fn run_view(&self, run: &str) -> Value {
        let query = std::collections::HashMap::from([("run_id".to_string(), run.to_string())]);
        let answer = runbook::api::routes::view_answer("run", self.d(), &query);
        assert_eq!(answer.status, 200, "{}", answer.body);
        answer.body["rows"][0].clone()
    }

    fn started(&self, run: &str) -> Vec<Value> {
        self.d().events_after(run, 0).into_iter().filter(|e| e["kind"] == STARTED).collect()
    }

    fn wait_started(&self, run: &str, action: &str) -> Value {
        let mut found = None;
        until(&format!("{STARTED} of {action}"), 20, || {
            found = self.started(run).into_iter().find(|e| e["record"]["action"] == action);
            found.is_some()
        });
        found.unwrap()
    }
}

#[test]
fn a_step_is_recorded_and_shown_while_its_program_runs_and_gone_when_it_ends() {
    let rig = Rig::new("engine-in-flight", &[Confinement::Local]);
    let run = rig.submit_at(Confinement::Local, Some(rig.context(3)));
    let event = rig.wait_started(&run, "draft.write");
    // the program has not ended: no draft, no finished line of the action
    assert!(!rig.workspace(&run).join("draft.txt").exists(), "the writer ended before its start was recorded");
    let finished = rig.d().events_after(&run, 0).into_iter().filter(|e| e["kind"] == "ActionFinished" && e["record"]["action"] == "draft.write").count();
    assert_eq!(finished, 0, "the action was finished when its start was read");
    assert_eq!(event["record"]["role"], "program");
    let case = event["record"]["case"].as_str().expect("the event names the case");
    assert!(!case.is_empty());
    // the run view shows it
    let row = rig.run_view(&run);
    super::openapi::check_row("RunDetail", &row);
    let step = &row["step_in_flight"];
    assert_eq!((step["action"].as_str(), step["case_id"].as_str()), (Some("draft.write"), Some(case)), "{row}");
    assert_eq!(step["started_at"], event["at"], "{row}");
    assert!(step["seconds"].as_u64().is_some_and(|s| s <= 3), "{row}");
    // it is gone once the run waits on the review: the writer and the check ended
    rig.wait_review(&run);
    let row = rig.run_view(&run);
    super::openapi::check_row("RunDetail", &row);
    assert!(row.get("step_in_flight").is_none(), "a step is shown while the run waits: {row}");
    assert!(rig.workspace(&run).join("draft.txt").exists());
}

#[test]
fn actions_in_turn_give_one_event_each_in_the_order_they_started() {
    let rig = Rig::new("engine-in-flight-turns", &[Confinement::Local]);
    let run = rig.submit_at(Confinement::Local, Some(rig.context(1)));
    rig.wait_review(&run);
    let events = rig.d().events_after(&run, 0);
    let mut seen: Vec<(String, u64)> = Vec::new();
    for e in &events {
        let position = e["position"].as_u64().unwrap();
        match e["kind"].as_str().unwrap() {
            STARTED => seen.push((format!("started {}", e["record"]["action"].as_str().unwrap()), position)),
            "ActionFinished" => seen.push((format!("finished {}", e["record"]["action"].as_str().unwrap()), position)),
            _ => {}
        }
    }
    let names: Vec<&str> = seen.iter().map(|(n, _)| n.as_str()).collect();
    // draft.open is no program (no start event); then the check (no draft yet: invalid), the writer, the check again
    let mut want = vec!["finished draft.open".to_string()];
    want.extend(["draft.check", "draft.write", "draft.check"].iter().flat_map(|a| [format!("started {a}"), format!("finished {a}")]));
    assert_eq!(names, want, "{names:?}");
    assert!(seen.windows(2).all(|w| w[0].1 < w[1].1), "positions grow: {seen:?}");
    assert_eq!(rig.started(&run).len(), 3);
}

#[test]
fn a_handoff_check_is_recorded_as_a_check_of_its_action() {
    let rig = Rig::build("engine-in-flight-check", &[Confinement::Local], true);
    let run = rig.submit_at(Confinement::Local, Some(rig.context(3)));
    let event = rig.wait_started(&run, "draft.write");
    assert_eq!(event["record"]["role"], "check", "{event}");
    assert_eq!(rig.run_view(&run)["step_in_flight"]["action"], "draft.write");
    rig.cancel(&run);
    rig.wait_state(&run, "Cancelled");
    assert!(rig.run_view(&run).get("step_in_flight").is_none());
}

#[test]
fn after_a_restart_the_orphaned_run_shows_no_step_and_keeps_the_event() {
    let mut rig = Rig::new("engine-in-flight-orphan", &[Confinement::Local]);
    let run = rig.submit_at(Confinement::Local, Some(rig.context(20)));
    rig.wait_started(&run, "draft.write");
    assert!(rig.run_view(&run).get("step_in_flight").is_some());
    rig.restart();
    let row = rig.run_view(&run);
    assert_eq!((row["state"].as_str(), row["outcome"].as_str()), (Some("Ended"), Some("orphaned")), "{row}");
    assert!(row.get("step_in_flight").is_none(), "an orphaned run shows a step: {row}");
    let actions: Vec<String> = rig.started(&run).iter().map(|e| e["record"]["action"].as_str().unwrap().to_string()).collect();
    assert_eq!(actions, ["draft.check", "draft.write"], "the journal keeps the start events of the check that failed and of the writer");
}

// ---- the step in flight, with a runtime the test drives step by step ----

type Act = Box<dyn FnOnce(&Arc<dyn StartTarget>, &dyn EventSink) -> Option<Result<Finish, RuntimeFailure>> + Send>;
type Call = (Act, mpsc::Sender<()>);
type Lane = (mpsc::Sender<Call>, Option<mpsc::Receiver<Call>>);

#[derive(Default)]
struct Gated {
    chans: Mutex<HashMap<String, Lane>>,
}

impl Gated {
    fn tx(&self, run: &str) -> mpsc::Sender<Call> {
        let mut m = self.chans.lock().unwrap();
        m.entry(run.to_string()).or_insert_with(|| {
            let (tx, rx) = mpsc::channel();
            (tx, Some(rx))
        })
        .0
        .clone()
    }

    fn rx(&self, run: &str) -> mpsc::Receiver<Call> {
        self.tx(run);
        self.chans.lock().unwrap().get_mut(run).unwrap().1.take().expect("one start per run")
    }

    /// Runs `act` on the run's own thread and returns when it has run.
    fn on_thread(&self, run: &str, act: impl FnOnce(&Arc<dyn StartTarget>, &dyn EventSink) -> Option<Result<Finish, RuntimeFailure>> + Send + 'static) {
        let (ack, wait) = mpsc::channel();
        self.tx(run).send((Box::new(act), ack)).unwrap();
        wait.recv_timeout(Duration::from_secs(20)).expect("the run's thread did not take the call");
    }
}

impl Runtime for Gated {
    fn recover(&self, _run_id: &str) -> Recovery {
        Recovery::Gone
    }

    fn knows_runbook(&self, runbook: &str, _version: Option<&str>) -> bool {
        runbook == "gated"
    }

    fn runbooks(&self) -> Vec<RunbookInfo> {
        vec![RunbookInfo { name: "gated".into(), version: None }]
    }

    fn kind(&self) -> &'static str {
        "loom"
    }

    fn start(&self, request: StartRequest, sink: &dyn EventSink, _cancel: &AtomicBool) -> Result<Finish, RuntimeFailure> {
        let rx = self.rx(&request.run_id);
        sink.started();
        let target = sink.steps().expect("the daemon's sink takes step reports");
        loop {
            let (act, ack) = rx.recv_timeout(Duration::from_secs(40)).map_err(|e| RuntimeFailure { reason: format!("no call: {e}") })?;
            let done = act(&target, sink);
            let _ = ack.send(());
            if let Some(result) = done {
                return result;
            }
        }
    }

    fn answer(&self, _run_id: &str, _request_id: &str, _answer: &str, _principal: &str) -> Result<(), AnswerFailure> {
        Err(AnswerFailure::UnknownRun)
    }

    fn cancel(&self, _run_id: &str) {}
}

struct Gate {
    rt: Arc<Gated>,
    clock: Arc<SetClock>,
    d: Daemon,
    runs: Mutex<Vec<String>>,
}

const T0: &str = "2026-11-24T10:00:00Z";

fn st(serial: u64, role: &'static str, case: Option<&str>, action: Option<&str>, try_number: Option<u64>) -> Started {
    Started { serial, role, case: case.map(str::to_owned), action: action.map(str::to_owned), try_number }
}

impl Gate {
    fn new(tag: &str) -> Gate {
        let dir = scratch(tag);
        std::fs::create_dir_all(dir.join("ctx")).unwrap();
        let rt = Arc::new(Gated::default());
        let clock = Arc::new(SetClock::at(T0).unwrap());
        let cfg = Config { state_dir: dir.join("state"), confinement_floor: Confinement::Local, context_roots: vec![dir.join("ctx")], runbook_dir: None, principal: super::PRINCIPAL.into() };
        let d = Daemon::start_with_clock(cfg, rt.clone(), clock.clone()).unwrap();
        Gate { rt, clock, d, runs: Mutex::new(Vec::new()) }
    }

    fn submit(&self) -> String {
        let o = self.d.submit_run(SubmitRun { runbook_id: "gated".into(), intake: "x".into(), confinement: Some(Confinement::Local), ..SubmitRun::default() }).unwrap();
        assert_eq!(o.name, "submitted", "{o:?}");
        let run = o.run_id.unwrap();
        self.runs.lock().unwrap().push(run.clone());
        run
    }

    fn view(&self, run: &str) -> Value {
        let query = HashMap::from([("run_id".to_string(), run.to_string())]);
        let answer = runbook::api::routes::view_answer("run", &self.d, &query);
        assert_eq!(answer.status, 200, "{}", answer.body);
        answer.body["rows"][0].clone()
    }

    fn started_events(&self, run: &str) -> Vec<Value> {
        self.d.events_after(run, 0).into_iter().filter(|e| e["kind"] == STARTED).collect()
    }

    fn start(&self, run: &str, s: Started) {
        self.rt.on_thread(run, move |t, _| {
            t.started(&s);
            None
        });
    }

    fn end(&self, run: &str, s: Started) {
        self.rt.on_thread(run, move |t, _| {
            t.ended(&s);
            None
        });
    }

    fn finish(&self, run: &str) {
        self.rt.on_thread(run, |_, _| Some(Ok(Finish::Outcome("done".into()))));
    }
}

impl Drop for Gate {
    fn drop(&mut self) {
        for run in self.runs.lock().unwrap().iter() {
            let (ack, _wait) = mpsc::channel();
            let _ = self.rt.tx(run).send((Box::new(|_, _| Some(Ok(Finish::Outcome("done".into())))), ack));
        }
        self.d.stop();
    }
}

#[test]
fn the_seconds_of_a_step_are_whole_seconds_since_its_start_by_the_daemons_clock_and_never_negative() {
    let g = Gate::new("gate-seconds");
    let run = g.submit();
    g.start(&run, st(1, "program", Some("c1"), Some("a.one"), Some(1)));
    let step = g.view(&run)["step_in_flight"].clone();
    assert_eq!((step["started_at"].as_str(), step["seconds"].as_u64()), (Some("2026-11-24T10:00:00.000Z"), Some(0)), "{step}");
    assert!(g.clock.set("2026-11-24T10:00:07.900Z"));
    assert_eq!(g.view(&run)["step_in_flight"]["seconds"], 7);
    assert!(g.clock.set("2026-11-24T10:25:00Z"));
    assert_eq!(g.view(&run)["step_in_flight"]["seconds"], 1500);
    assert!(g.clock.set("2026-11-24T09:59:00Z"));
    assert_eq!(g.view(&run)["step_in_flight"]["seconds"], 0, "a clock behind the start gives a negative count");
}

#[test]
fn the_record_names_only_what_is_known_and_the_step_does_the_same() {
    let g = Gate::new("gate-record");
    let run = g.submit();
    g.start(&run, st(1, "program", None, None, None));
    let step = g.view(&run)["step_in_flight"].clone();
    let mut keys: Vec<&str> = step.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["seconds", "started_at"], "{step}");
    g.end(&run, st(1, "program", None, None, None));
    g.start(&run, st(2, "check", Some("c9"), Some("a.nine"), Some(4)));
    let events = g.started_events(&run);
    assert_eq!(events.len(), 2, "{events:?}");
    assert_eq!(events[0]["record"], json!({"role": "program"}));
    assert_eq!(events[1]["record"], json!({"case": "c9", "action": "a.nine", "role": "check", "try": 4}));
    assert_eq!(events[1]["sequence"].as_u64().unwrap(), events[0]["sequence"].as_u64().unwrap() + 1);
}

#[test]
fn the_start_is_in_the_journal_and_the_view_before_the_announcing_call_returns() {
    let g = Gate::new("gate-sync");
    let run = g.submit();
    let seen: Arc<Mutex<Option<(usize, bool)>>> = Arc::new(Mutex::new(None));
    let (d, r, s) = (g.d.clone(), run.clone(), seen.clone());
    g.rt.on_thread(&run, move |t, _| {
        t.started(&st(1, "program", Some("c1"), Some("a.one"), Some(1)));
        let n = d.events_after(&r, 0).iter().filter(|e| e["kind"] == STARTED).count();
        let shown = d.run_detail(&r)[0].get("step_in_flight").is_some();
        *s.lock().unwrap() = Some((n, shown));
        None
    });
    assert_eq!(*seen.lock().unwrap(), Some((1, true)));
}

#[test]
fn two_runs_at_once_each_show_their_own_step_and_one_ending_leaves_the_other() {
    let g = Gate::new("gate-two");
    let (a, b) = (g.submit(), g.submit());
    g.start(&a, st(1, "program", Some("case-a"), Some("a.one"), Some(1)));
    assert!(g.clock.set("2026-11-24T10:00:05Z"));
    g.start(&b, st(1, "program", Some("case-b"), Some("a.two"), Some(1)));
    assert!(g.clock.set("2026-11-24T10:00:08Z"));
    let (va, vb) = (g.view(&a)["step_in_flight"].clone(), g.view(&b)["step_in_flight"].clone());
    assert_eq!((va["case_id"].as_str(), va["action"].as_str(), va["seconds"].as_u64()), (Some("case-a"), Some("a.one"), Some(8)), "{va}");
    assert_eq!((vb["case_id"].as_str(), vb["action"].as_str(), vb["seconds"].as_u64()), (Some("case-b"), Some("a.two"), Some(3)), "{vb}");
    g.end(&a, st(1, "program", Some("case-a"), Some("a.one"), Some(1)));
    assert!(g.view(&a).get("step_in_flight").is_none());
    assert_eq!(g.view(&b)["step_in_flight"]["case_id"], "case-b");
}

#[test]
fn a_cancel_clears_the_step_while_the_program_is_reported_running_and_no_start_is_recorded_after_it() {
    let g = Gate::new("gate-cancel");
    let run = g.submit();
    g.start(&run, st(1, "program", Some("c1"), Some("a.one"), Some(1)));
    assert!(g.view(&run).get("step_in_flight").is_some());
    let o = g.d.cancel_run(CancelRun { run_id: run.clone() }).unwrap();
    assert!(!o.is_refusal(), "{o:?}");
    let row = g.view(&run);
    assert_eq!(row["state"], "Cancelled");
    assert!(row.get("step_in_flight").is_none(), "a cancelled run shows a step: {row}");
    // the runtime still reports a program (a report after a cancel): nothing is recorded, nothing shows
    g.start(&run, st(2, "program", Some("c1"), Some("a.two"), Some(1)));
    assert_eq!(g.started_events(&run).len(), 1, "a start was recorded after the cancel");
    assert!(g.view(&run).get("step_in_flight").is_none());
}

#[test]
fn an_end_of_an_older_program_leaves_the_newer_step_and_a_run_that_ends_shows_none() {
    let g = Gate::new("gate-serial");
    let run = g.submit();
    g.start(&run, st(1, "program", Some("c1"), Some("a.one"), Some(1)));
    g.start(&run, st(2, "program", Some("c2"), Some("a.two"), Some(1)));
    g.end(&run, st(1, "program", Some("c1"), Some("a.one"), Some(1)));
    assert_eq!(g.view(&run)["step_in_flight"]["case_id"], "c2");
    g.finish(&run);
    until("the run to end", 10, || g.view(&run)["state"] == "Ended");
    assert!(g.view(&run).get("step_in_flight").is_none());
}

#[test]
fn a_run_that_waits_on_a_decision_shows_no_step() {
    let g = Gate::new("gate-waiting");
    let run = g.submit();
    g.start(&run, st(1, "program", Some("c1"), Some("a.one"), Some(1)));
    assert!(g.view(&run).get("step_in_flight").is_some());
    g.rt.on_thread(&run, |_, sink| {
        sink.decision_requested(DecisionRequest { request_id: "q1".into(), case_id: "c1".into(), action: "a.review".into(), capability: "person".into(), question: "ok?".into(), options: vec![] });
        None
    });
    let row = g.view(&run);
    assert_eq!(row["state"], "WaitingOnDecision", "{row}");
    assert!(row.get("step_in_flight").is_none(), "a waiting run shows a step: {row}");
}

#[test]
fn a_runtime_failure_while_a_program_runs_leaves_no_step() {
    let g = Gate::new("gate-failure");
    let run = g.submit();
    g.start(&run, st(1, "program", Some("c1"), Some("a.one"), Some(1)));
    g.rt.on_thread(&run, |_, _| Some(Err(RuntimeFailure { reason: "boom".into() })));
    until("the run to end", 10, || g.view(&run)["state"] == "Ended");
    let row = g.view(&run);
    assert_eq!(row["outcome"], "runtime-failure", "{row}");
    assert!(row.get("step_in_flight").is_none());
}

// ---- the step in flight, real programs ----

#[test]
fn a_rejected_draft_gives_the_writer_a_second_start_with_try_two_and_the_event_stays_one_per_start() {
    let rig = Rig::new("engine-flight-try", &[Confinement::Local]);
    let run = rig.submit();
    let first = rig.wait_review(&run);
    rig.answer(&run, &first, r#"{"result":"rejected","reasons":"x"}"#, "reviewer");
    until("a second review", 40, || rig.d().open_decisions_of_run(&run).iter().any(|d| d["request_id"].as_str() != Some(first.as_str())));
    let events = rig.started(&run);
    let tries = |action: &str| -> Vec<u64> { events.iter().filter(|e| e["record"]["action"] == action).map(|e| e["record"]["try"].as_u64().unwrap()).collect() };
    assert_eq!(tries("draft.write"), [1, 2], "{events:?}");
    let checks = tries("draft.check");
    assert_eq!(checks, (1..=checks.len() as u64).collect::<Vec<_>>(), "{events:?}");
    assert!(events.iter().all(|e| e["record"]["role"] == "program"));
    // the second writer's start comes after the rejection's answer: the positions of the events grow
    let positions: Vec<u64> = events.iter().map(|e| e["position"].as_u64().unwrap()).collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn two_engine_runs_at_once_each_show_the_step_of_their_own_case() {
    let rig = Rig::new("engine-flight-two", &[Confinement::Local]);
    let a = rig.submit_at(Confinement::Local, Some(rig.context(8)));
    let ea = rig.wait_started(&a, "draft.write");
    let b = rig.submit_at(Confinement::Local, Some(rig.context(8)));
    let eb = rig.wait_started(&b, "draft.write");
    let (va, vb) = (rig.run_view(&a)["step_in_flight"].clone(), rig.run_view(&b)["step_in_flight"].clone());
    assert_eq!((va["started_at"].clone(), va["case_id"].clone()), (ea["at"].clone(), ea["record"]["case"].clone()), "{va}");
    assert_eq!((vb["started_at"].clone(), vb["case_id"].clone()), (eb["at"].clone(), eb["record"]["case"].clone()), "{vb}");
    assert_ne!(va["case_id"], vb["case_id"], "two runs show one case");
    rig.cancel(&a);
    rig.cancel(&b);
    rig.wait_state(&a, "Cancelled");
    rig.wait_state(&b, "Cancelled");
    assert!(rig.run_view(&a).get("step_in_flight").is_none() && rig.run_view(&b).get("step_in_flight").is_none());
}

#[test]
fn a_writer_that_cannot_start_is_recorded_once_and_leaves_no_step_when_the_run_goes_on() {
    let rig = Rig::new("engine-flight-cannot-start", &[Confinement::Local]);
    {
        use std::os::unix::fs::PermissionsExt;
        let writer = rig.home.join("bindings/tools/draft-write.sh");
        std::fs::set_permissions(&writer, std::fs::Permissions::from_mode(0o644)).unwrap();
    }
    let run = rig.submit_at(Confinement::Local, Some(rig.context(1)));
    until("the run to stop", 40, || matches!(rig.state_of(&run).as_str(), "Ended" | "WaitingOnDecision" | "Cancelled" | "FailedToStart"));
    let writers: Vec<Value> = rig.started(&run).into_iter().filter(|e| e["record"]["action"] == "draft.write").collect();
    assert_eq!(writers.len(), 1, "{writers:?}");
    assert!(rig.run_view(&run).get("step_in_flight").is_none(), "{}", rig.run_view(&run));
}

#[test]
fn a_cancel_while_the_writer_runs_leaves_one_start_event_of_it_and_no_step() {
    let rig = Rig::new("engine-flight-cancel-writer", &[Confinement::Local]);
    let run = rig.submit_at(Confinement::Local, Some(rig.context(20)));
    let event = rig.wait_started(&run, "draft.write");
    assert_eq!(event["record"]["try"], 1);
    rig.cancel(&run);
    rig.wait_state(&run, "Cancelled");
    until("the engine to let go of the run", 20, || rig.engine.store_state(&run) == runbook::engine::StoreState::Idle);
    let writers = rig.started(&run).into_iter().filter(|e| e["record"]["action"] == "draft.write").count();
    assert_eq!(writers, 1);
    assert!(rig.run_view(&run).get("step_in_flight").is_none());
    assert!(!rig.workspace(&run).join("draft.txt").exists());
}
