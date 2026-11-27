//! The engine (`runbook::engine`): a case tree run through Loom's library calls inside the daemon's
//! process. The fixture runbook has two levels: a root case that opens a drafting case; the drafting
//! case has a writer (a POSIX sh program) and one person step with the named results `performed` and
//! `rejected`.

use super::state::scratch;
use runbook::clock::SetClock;
use runbook::commands::{AnswerDecision, CancelRun, Config, Daemon, SubmitRun};
use runbook::engine::copy::digest;
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

pub(super) fn copy_tree(from: &Path, to: &Path) {
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

/// The names of the programs below this process (`crate::process_sample`).
fn descendants() -> Vec<String> {
    crate::process_sample::programs_below(std::process::id())
}

/// Ends every `sleep` below this process: a step whose program pauses (`pause.txt`) goes on at once, when the case has seen
/// what it waited for, instead of after the pause. Whether it ended one: a case that waits to see the pause reads it
/// here, from the scan that ends it, so no pause is ended without being seen.
fn end_the_pauses() -> bool {
    let pauses: Vec<u32> = descendant_processes().into_iter().filter(|(_, c)| c.as_deref() == Some("sleep")).map(|(pid, _)| pid).collect();
    for pid in &pauses {
        // SAFETY: a plain signal to a process id this case just read below its own process.
        unsafe { libc::kill(*pid as i32, libc::SIGTERM) };
    }
    !pauses.is_empty()
}

/// Ends the pauses of `count` programs (each a `sleep` of `pause.txt`, below this process) once that many pause at the
/// same time, and not before: the programs of several runs overlap for sure, and each goes on at once when the case
/// lets it, instead of after a pause of fixed seconds that had to be long enough for both. It returns once the pauses it
/// ended have ended, so the next call counts only the pauses of the next step.
pub(super) fn end_pauses_together(count: usize) {
    let mut pauses = Vec::new();
    until(&format!("{count} pauses at the same time"), 40, || {
        pauses = descendant_processes().into_iter().filter(|(pid, c)| c.as_deref() == Some("sleep") && alive(*pid)).map(|(pid, _)| pid).collect();
        pauses.len() >= count
    });
    for pid in &pauses {
        // SAFETY: a plain signal to a process id this case just read below its own process.
        unsafe { libc::kill(*pid as i32, libc::SIGTERM) };
    }
    until("the ended pauses to end", 20, || pauses.iter().all(|pid| !alive(*pid)));
}

/// Whether the process `pid` runs: it exists and is no zombie (a `sleep` that ended stays one until its shell reaps it).
fn alive(pid: u32) -> bool {
    let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else { return false };
    stat.rfind(')').and_then(|close| stat[close + 1..].split_whitespace().next()).is_some_and(|state| state != "Z" && state != "X")
}

/// The process id of every process below this one and the name of the program it runs, `None` for a process that has not
/// called `exec` yet (`crate::process_sample::below`): a scan reads the processes below this one and not every process
/// of the host; the cases that wait on a pause scan every 10 ms.
fn descendant_processes() -> Vec<(u32, Option<String>)> {
    crate::process_sample::below(std::process::id())
}

pub(super) fn until(what: &str, secs: u64, mut done: impl FnMut() -> bool) {
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
        Rig::build_with(tag, levels, hold, false)
    }

    /// `refuse`: the writer's binding lists the exit 69 in `exits.error` and the writer ends with
    /// it, so Loom refuses the step `ProgramError` and the tree is left without a move.
    fn build_with(tag: &str, levels: &[Confinement], hold: bool, refuse: bool) -> Rig {
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
        if refuse {
            let writer = "      argv: [tools/draft-write.sh]\n";
            assert!(text.contains(writer));
            text = text.replace(writer, &format!("{writer}    exits: {{error: [69]}}\n"));
            std::fs::write(home.join("bindings/tools/draft-write.sh"), "#!/bin/sh\nexit 69\n").unwrap();
        }
        std::fs::write(&binding, text).unwrap();
        std::fs::create_dir_all(dir.join("ctx")).unwrap();
        let engine = Arc::new(Engine::with_levels(home.clone(), dir.join("state"), levels, super::policy()));
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
        self.engine = Arc::new(Engine::with_levels(self.home.clone(), self.dir.join("state"), &self.levels, super::policy()));
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
fn a_run_left_without_a_move_by_a_refused_action_says_which_action_and_exit() {
    // A bound program that is no agent node and whose exit Loom refuses (`ProgramError`, exit 69) left the tree with
    // nothing to do: the run ends naming the step and its exit, as an agent node's step does, the reason is kept in
    // `runbook.step-failed`, and the view `run` says it in `failure`. It is no `runtime-failure` with no reason.
    let rig = Rig::build_with("engine-program-error", &[Confinement::Local], false, true);
    let run = rig.submit();
    until("the run to end", 40, || rig.state_of(&run) == "Ended");
    let row = rig.row(&run);
    assert_eq!(row["outcome"], "failed: step draft.write exit 69", "{row}");
    let failed = rig.d().events_after(&run, 0).into_iter().find(|e| e["kind"] == "runbook.step-failed").expect("no runbook.step-failed event");
    let reason = failed["record"]["reason"].as_str().unwrap().to_owned();
    assert!(reason.contains("draft.write") && reason.contains("exit 69"), "{reason}");
    assert_eq!((failed["record"]["action"].clone(), failed["record"]["exit"].clone()), (json!("draft.write"), json!(69)), "{failed}");
    assert_eq!(row["failure"], json!(reason), "the view run does not say why the run failed: {row}");
}

#[test]
fn the_runtime_failed_reason_of_a_refused_action_names_no_state_or_home_path() {
    let rig = Rig::build_with("fix1-engine-path", &[Confinement::Local], false, true);
    let run = rig.submit();
    until("the run to end", 40, || rig.state_of(&run) == "Ended");
    let failed = rig.d().events_after(&run, 0).into_iter().find(|e| e["kind"] == "runbook.step-failed").expect("no runbook.step-failed event");
    let reason = failed["record"]["reason"].as_str().unwrap().to_owned();
    let (dir, home) = (rig.dir.display().to_string(), rig.home.display().to_string());
    assert!(reason.contains("the step draft.write failed") && reason.contains("exit 69"), "{reason}");
    assert!(!reason.contains(&dir) && !reason.contains(&home), "the reason names a path of the state directory or the home: {reason}");
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
    // the writer pauses until the sample has seen it (its `sleep`); the sample covers the whole run
    let run = rig.submit_at(Confinement::Local, Some(rig.context(30)));
    let mut seen: BTreeSet<String> = BTreeSet::new();
    until("the run to end", 50, || {
        seen.extend(descendants());
        if seen.contains("sleep") {
            end_the_pauses();
        }
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
    // the writer pauses until the case has read the step in flight; then the case ends the pause
    let run = rig.submit_at(Confinement::Local, Some(rig.context(30)));
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
    until("the writer's pause to be seen and ended", 20, end_the_pauses);
    rig.wait_review(&run);
    let row = rig.run_view(&run);
    super::openapi::check_row("RunDetail", &row);
    assert!(row.get("step_in_flight").is_none(), "a step is shown while the run waits: {row}");
    assert!(rig.workspace(&run).join("draft.txt").exists());
}

/// Whether a process below this one other than itself has `lock` open.
fn held_below(lock: &Path) -> bool {
    descendant_processes().into_iter().any(|(pid, _)| {
        std::fs::read_dir(format!("/proc/{pid}/fd")).into_iter().flatten().flatten().any(|fd| std::fs::read_link(fd.path()).is_ok_and(|t| t == lock))
    })
}

#[test]
#[ignore = "red until Loom releases its store lock explicitly; story:daemon-tests-back-when-loom-unlocks-the-store"]
fn a_forked_copy_of_the_store_lock_held_past_the_step_does_not_end_the_run() {
    use std::os::fd::{FromRawFd, OwnedFd};
    use std::os::unix::process::CommandExt;
    let rig = Rig::new("engine-forked-hold", &[Confinement::Local]);
    let run = rig.submit_at(Confinement::Local, Some(rig.context(30)));
    rig.wait_started(&run, "draft.write");
    // The writer runs inside Loom's step, which holds the lock of the run's store. A process forked now, as the
    // daemon forks the program of another run, holds a copy of the lock's descriptor until it execs: this one
    // execs when the case closes the write end of a pipe, after the run's next step was refused or went on.
    let lock = rig.store(&run).join("tree.lock");
    let mut fds = [0; 2];
    // SAFETY: pipe2 writes two descriptors into the array it is given.
    assert_eq!(unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) }, 0);
    let (read_end, write_end) = fds.into();
    // SAFETY: both descriptors were just opened by pipe2 and are owned here alone.
    let (gate, release) = unsafe { (OwnedFd::from_raw_fd(read_end), OwnedFd::from_raw_fd(write_end)) };
    let forked = std::thread::spawn(move || {
        let mut command = std::process::Command::new("true");
        // SAFETY: close and read are async-signal-safe and nothing is allocated between the fork and the exec.
        unsafe {
            command.pre_exec(move || {
                libc::close(write_end);
                let mut byte = 0u8;
                while libc::read(read_end, (&mut byte as *mut u8).cast(), 1) < 0 && *libc::__errno_location() == libc::EINTR {}
                Ok(())
            })
        };
        command.status().unwrap()
    });
    until("a forked process to hold the run's store lock", 10, || held_below(&lock));
    // the writer ends and the step lets go of the lock; the next step takes it while the forked copy is open
    until("the writer's pause to be seen and ended", 20, end_the_pauses);
    until("the run to wait on the review or end", 40, || rig.state_of(&run) == "Ended" || !rig.d().open_decisions_of_run(&run).is_empty());
    drop(release);
    assert!(forked.join().unwrap().success());
    drop(gate);
    let failed: Vec<Value> = rig.d().events_after(&run, 0).into_iter().filter(|e| e["kind"] == "runbook.runtime-failed").map(|e| e["record"]["reason"].clone()).collect();
    assert!(failed.is_empty(), "{failed:?}");
    rig.wait_review(&run);
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
        sink.decision_requested(DecisionRequest { request_id: "q1".into(), case_id: "c1".into(), action: "a.review".into(), capability: "person".into(), question: "ok?".into(), options: vec![], artifact: None });
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

fn replace_in(home: &Path, file: &str, from: &str, to: &str) {
    let path = home.join(file);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(from), "{file} lacks {from:?}");
    std::fs::write(path, text.replace(from, to)).unwrap();
}

/// Revision 2 of the fixture runbook: both protocols, both bindings, and a writer that writes another text.
fn revise(home: &Path) {
    replace_in(home, "protocol/engine-demo.protocol.yaml", "revision: 1", "revision: 2");
    replace_in(home, "protocol/engine-draft.protocol.yaml", "revision: 1", "revision: 2");
    replace_in(home, "bindings/engine-demo.yaml", "protocol: engine-demo@1", "protocol: engine-demo@2");
    replace_in(home, "bindings/engine-draft.yaml", "protocol: engine-draft@1", "protocol: engine-draft@2");
    replace_in(home, "bindings/tools/draft-write.sh", "echo draft >>", "echo draft-two >>");
}

impl Rig {
    /// Reject the draft the review `first` waits on, so the writer runs again, then approve the second draft and settle.
    fn finish_after_a_rejection(&self, run: &str, first: &str) {
        self.answer(run, first, r#"{"result":"rejected","reasons":"once more"}"#, "reviewer");
        let mut next = None;
        until("a second review request", 40, || {
            let state = self.state_of(run);
            assert!(!matches!(state.as_str(), "Cancelled" | "FailedToStart"), "the run is {state}: {}", self.row(run));
            next = self.d().open_decisions_of_run(run).into_iter().find(|d| d["request_id"] != first);
            next.is_some()
        });
        let next = next.unwrap();
        let question = next["question"].as_str().unwrap_or_default();
        assert!(!question.contains("The earlier answer was refused"), "the answer was refused by Loom: {question}");
        self.answer(run, next["request_id"].as_str().unwrap(), "performed", "reviewer");
        self.settle(run, &[]);
    }

    fn draft(&self, run: &str) -> String {
        std::fs::read_to_string(self.workspace(run).join("draft.txt")).unwrap_or_default()
    }
}

/// A run that waits at its review while `damage` is done to the runbook home ends on the revision it was submitted with.
fn survives(tag: &str, damage: impl Fn(&Path)) {
    let rig = Rig::new(tag, &[Confinement::Local]);
    let run = rig.submit();
    let first = rig.wait_review(&run);
    damage(&rig.home);
    rig.finish_after_a_rejection(&run, &first);
    let row = rig.row(&run);
    assert_eq!((row["state"].as_str(), row["outcome"].as_str()), (Some("Ended"), Some("delivered")), "{row}");
    assert_eq!(rig.draft(&run), "draft\ndraft\n");
}

#[test]
fn a_revision_of_the_runbook_home_during_a_run_changes_nothing_for_that_run() {
    survives("engine-pin-revised", revise);
}

#[test]
fn bindings_that_do_not_parse_in_the_runbook_home_change_nothing_for_a_run_in_flight() {
    survives("engine-pin-garbled", |home| {
        for file in ["bindings/engine-demo.yaml", "bindings/engine-draft.yaml", "protocol/engine-demo.protocol.yaml", "protocol/engine-draft.protocol.yaml"] {
            std::fs::write(home.join(file), ": : [\n").unwrap();
        }
    });
}

#[test]
fn a_runbook_home_removed_during_a_run_changes_nothing_for_that_run() {
    survives("engine-pin-removed", |home| {
        std::fs::remove_dir_all(home.join("bindings")).unwrap();
        std::fs::remove_dir_all(home.join("protocol")).unwrap();
    });
}

impl Rig {
    fn state_root(&self) -> PathBuf {
        self.dir.join("state")
    }

    /// The run's own directory: its store, its copy of the runbook.
    fn run_dir(&self, run: &str) -> PathBuf {
        self.state_root().join("runs").join(run)
    }

    fn copy_of(&self, run: &str) -> PathBuf {
        self.run_dir(run).join("runbook")
    }

    fn submit_version(&self, version: &str) -> runbook::commands::Outcome {
        self.d().submit_run(SubmitRun { runbook_id: RUNBOOK.into(), runbook_version: Some(version.into()), intake: "a note on tea".into(), confinement: Some(Confinement::Local), ..SubmitRun::default() }).unwrap()
    }

    fn event_of(&self, run: &str, kind: &str) -> Value {
        let events = self.d().events_after(run, 0);
        let found: Vec<&Value> = events.iter().filter(|e| e["kind"] == kind).collect();
        assert_eq!(found.len(), 1, "one {kind} event: {events:?}");
        found[0]["record"].clone()
    }
}

#[test]
fn a_second_submit_takes_the_new_revision_and_each_record_names_its_revision_and_digest() {
    let rig = Rig::new("engine-pin-second", &[Confinement::Local]);
    let digest_one = digest(&rig.home.join("bindings")).unwrap();
    let one = rig.submit();
    let review = rig.wait_review(&one);
    revise(&rig.home);
    let digest_two = digest(&rig.home.join("bindings")).unwrap();
    assert_ne!(digest_one, digest_two);
    let stale = rig.submit_version("1");
    assert!(stale.is_refusal() && stale.name == "no-runbook", "{stale:?}");
    let accepted = rig.submit_version("2");
    assert_eq!(accepted.name, "submitted", "{accepted:?}");
    let two = accepted.run_id.unwrap();
    rig.settle(&two, &["performed"]);
    assert_eq!(rig.row(&two)["outcome"], "delivered");
    assert_eq!(rig.draft(&two), "draft-two\n");
    rig.finish_after_a_rejection(&one, &review);
    assert_eq!(rig.row(&one)["outcome"], "delivered");
    assert_eq!(rig.draft(&one), "draft\ndraft\n");
    let (pin_one, pin_two) = (rig.event_of(&one, "runbook.pinned"), rig.event_of(&two, "runbook.pinned"));
    assert_eq!((pin_one["protocol"].as_str(), pin_one["revision"].as_i64(), pin_one["bindings_sha256"].as_str()), (Some("engine-demo"), Some(1), Some(digest_one.as_str())), "{pin_one}");
    assert_eq!((pin_two["protocol"].as_str(), pin_two["revision"].as_i64(), pin_two["bindings_sha256"].as_str()), (Some("engine-demo"), Some(2), Some(digest_two.as_str())), "{pin_two}");
    // The copy of a run is the runbook as it was at its submit.
    assert_eq!(digest(&rig.copy_of(&one).join("bindings")).unwrap(), digest_one);
    assert_eq!(digest(&rig.copy_of(&two).join("bindings")).unwrap(), digest_two);
}

/// A run whose copy of the runbook is changed while it waits ends with a named refusal and runs
/// nothing of the changed copy: the writer would run again after the rejection and leave `marker.txt`.
fn copy_changed(tag: &str, damage: impl Fn(&Path)) {
    let rig = Rig::new(tag, &[Confinement::Local]);
    let run = rig.submit();
    let first = rig.wait_review(&run);
    damage(&rig.copy_of(&run));
    rig.answer(&run, &first, r#"{"result":"rejected","reasons":"once more"}"#, "reviewer");
    rig.wait_state(&run, "Ended");
    assert_eq!(rig.row(&run)["outcome"], "runtime-failure");
    let failed = rig.event_of(&run, "runbook.runtime-failed");
    assert!(failed["reason"].as_str().is_some_and(|r| r.starts_with("RunbookCopyChanged")), "{failed}");
    assert!(!rig.workspace(&run).join("marker.txt").exists(), "a changed file was run");
    assert_eq!(rig.draft(&run), "draft\n", "a step ran after the copy changed");
}

#[test]
fn a_program_of_the_copy_that_is_edited_during_the_run_is_never_run() {
    copy_changed("engine-pin-edited-program", |copy| std::fs::write(copy.join("bindings/tools/draft-write.sh"), "#!/bin/sh\necho x > marker.txt\n").unwrap());
}

#[test]
fn a_file_added_to_the_copy_during_the_run_ends_the_run() {
    copy_changed("engine-pin-added", |copy| std::fs::write(copy.join("bindings/tools/extra.sh"), "#!/bin/sh\necho x > marker.txt\n").unwrap());
}

#[test]
fn a_file_removed_from_the_copy_during_the_run_ends_the_run() {
    copy_changed("engine-pin-removed-from-copy", |copy| std::fs::remove_file(copy.join("bindings/tools/draft-check.sh")).unwrap());
}

#[test]
fn bindings_of_the_copy_that_are_edited_during_the_run_are_never_read() {
    copy_changed("engine-pin-edited-bindings", |copy| replace_in(copy, "bindings/engine-draft.yaml", "protocol: engine-draft@1", "protocol: engine-draft@9"));
}

#[test]
fn a_protocol_of_the_copy_that_is_edited_during_the_run_ends_the_run() {
    copy_changed("engine-pin-edited-protocol", |copy| replace_in(copy, "protocol/engine-demo.protocol.yaml", "revision: 1", "revision: 7"));
}

#[test]
fn a_link_out_of_the_runbook_home_is_refused_before_the_run_holds_anything_of_the_runbook() {
    for (tag, make_link) in [("file", true), ("directory", false)] {
        let rig = Rig::new(&format!("engine-pin-link-{tag}"), &[Confinement::Local]);
        let outside = rig.dir.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.txt"), "not for a run\n").unwrap();
        let (target, name) = if make_link { (outside.join("secret.txt"), "leak.sh") } else { (outside.clone(), "leak") };
        std::os::unix::fs::symlink(target, rig.home.join("bindings/tools").join(name)).unwrap();
        let run = rig.submit();
        rig.wait_state(&run, "FailedToStart");
        let failure = rig.row(&run)["failure"].as_str().unwrap().to_string();
        assert!(failure.contains("leaves the runbook home"), "{failure}");
        assert!(!rig.copy_of(&run).exists(), "the run's directory holds a copy");
        assert!(!rig.run_dir(&run).join("runbook-pin.json").exists());
        assert!(!rig.store(&run).exists(), "a store was made");
        assert!(!rig.workspace(&run).join("draft.txt").exists());
    }
}

#[test]
fn a_link_inside_the_runbook_home_is_copied_as_the_file_it_names() {
    let rig = Rig::new("engine-pin-link-inside", &[Confinement::Local]);
    std::os::unix::fs::symlink("draft-write.sh", rig.home.join("bindings/tools/draft-again.sh")).unwrap();
    let run = rig.submit();
    rig.settle(&run, &["performed"]);
    assert_eq!(rig.row(&run)["outcome"], "delivered");
    let meta = std::fs::symlink_metadata(rig.copy_of(&run).join("bindings/tools/draft-again.sh")).unwrap();
    assert!(meta.is_file() && !meta.file_type().is_symlink());
}

#[test]
fn a_submit_names_a_runbook_by_name_and_version_and_never_by_a_path() {
    let rig = Rig::new("engine-pin-by-name", &[Confinement::Local]);
    let runs = |rig: &Rig| std::fs::read_dir(rig.state_root().join("runs")).map(|d| d.count()).unwrap_or(0);
    let before = runs(&rig);
    let home = rig.home.to_string_lossy().into_owned();
    for id in ["../elsewhere/engine-demo","bindings/engine-demo", &format!("{home}/bindings/engine-demo"), "/etc/hostname", ".hidden", ""] {
        let o = rig.d().submit_run(SubmitRun { runbook_id: id.into(), intake: "x".into(), confinement: Some(Confinement::Local), ..SubmitRun::default() }).unwrap();
        assert!(o.is_refusal() && o.name == "no-runbook", "{id:?}: {o:?}");
    }
    let o = rig.submit_version("../x");
    assert!(o.is_refusal() && o.name == "no-runbook", "{o:?}");
    assert_eq!(runs(&rig), before, "a refused submit made a run directory");
}

#[test]
fn the_copy_lies_outside_the_work_directory_and_no_record_names_its_path() {
    let rig = Rig::new("engine-pin-not-served", &[Confinement::Local]);
    let run = rig.submit();
    rig.settle(&run, &["performed"]);
    assert_eq!(rig.row(&run)["outcome"], "delivered");
    let copy = rig.copy_of(&run);
    assert!(copy.join("bindings/engine-demo.yaml").is_file());
    assert!(!copy.starts_with(rig.workspace(&run)) && !copy.starts_with(rig.state_root().join("workspaces")));
    let product = listing(&rig.workspace(&run));
    assert!(product.iter().all(|f| !f.starts_with("bindings/") && !f.starts_with("protocol/") && !f.starts_with("runbook/") && !f.contains("draft-write.sh")), "{product:?}");
    let state = rig.state_root().to_string_lossy().into_owned();
    let own: Vec<Value> = rig.d().events_after(&run, 0).into_iter().filter(|e| e["kind"].as_str().unwrap().starts_with("runbook.")).collect();
    assert!(own.iter().any(|e| e["kind"] == "runbook.pinned"));
    for e in own {
        assert!(!e.to_string().contains(&state), "an event of the daemon names a path of the state directory: {e}");
    }
}

// ---- the case tree of a run (story daemon-page-shows-a-run-tree-and-products) ----

/// The rows of the view `cases` for `run`, each checked against the generated OpenAPI row schema.
fn cases(rig: &Rig, run: &str) -> Vec<Value> {
    let query = HashMap::from([("run_id".to_string(), run.to_string())]);
    let answer = runbook::api::routes::view_answer("cases", rig.d(), &query);
    assert_eq!(answer.status, 200, "{}", answer.body);
    let rows = answer.body["rows"].as_array().unwrap().clone();
    for r in &rows {
        super::openapi::check_row("CasesOfRun", r);
    }
    rows
}

/// The tree as lines, a case under its parent, indented by its depth: `<name> <protocol> <state>[ <outcome>]`, the
/// root named `(root)`. The case ids are Loom's and are not compared; each parent is found by its id.
fn tree_lines(rows: &[Value]) -> Vec<String> {
    let mut out = Vec::new();
    fn visit(rows: &[Value], parent: Option<&str>, depth: usize, out: &mut Vec<String>) {
        for r in rows.iter().filter(|r| r["parent"].as_str() == parent) {
            let name = r["name"].as_str().unwrap_or("(root)");
            let outcome = r["outcome"].as_str().map(|o| format!(" {o}")).unwrap_or_default();
            out.push(format!("{}{name} {} {}{outcome}", "  ".repeat(depth), r["protocol"].as_str().unwrap(), r["case_state"].as_str().unwrap()));
            visit(rows, r["case_id"].as_str(), depth + 1, out);
        }
    }
    visit(rows, None, 0, &mut out);
    assert_eq!(out.len(), rows.len(), "every case hangs below the root: {rows:?}");
    out
}

/// A middle level in the runbook home: the root opens `section` (protocol `engine.section`), which opens the drafting
/// case `draft` and passes once it passed. The tree is three levels deep.
fn deepen(home: &Path) {
    std::fs::write(
        home.join("protocol/engine-section.protocol.yaml"),
        "format: protocol/1\nprotocol:\n  id: engine.section\n  revision: 1\n  description: >-\n    A section of the engine tests' runbook: it opens one drafting case and passes once that case passed.\nartifacts:\n  idea: {}\nevidence_kinds:\n  case_outcome: {}\n  draft_opened: {}\nclaims:\n  draft.opened:\n    true_when:\n      evidence: {kind: draft_opened, subject: idea, result: opened}\n  draft.passed:\n    true_when:\n      children: {in: [passed]}\nactions:\n  draft.open:\n    effect: write\n    case_open: {protocol: engine.draft, subject: idea}\n    may_produce:\n      - evidence: draft_opened\noutcomes:\n  passed:\n    requires:\n      all:\n        - {claim: draft.opened}\n        - {claim: draft.passed}\n",
    )
    .unwrap();
    std::fs::write(
        home.join("bindings/engine-section.yaml"),
        "format: loom.bindings/1\nprotocol: engine-section@1\nprogram_root: bindings\nartifacts:\n  idea: idea.yaml\nactions:\n  - action: draft.open\n    executor:\n      kind: open\n      name: draft\n      bindings: engine-draft.yaml\n    evidence:\n      - kind: draft_opened\n        subject: idea\n        producer: loom\n        from: effect\n        results: {performed: opened}\n",
    )
    .unwrap();
    replace_in(home, "protocol/engine-demo.protocol.yaml", "case_open: {protocol: engine.draft, subject: idea}", "case_open: {protocol: engine.section, subject: idea}");
    replace_in(home, "bindings/engine-demo.yaml", "      name: draft\n      bindings: engine-draft.yaml", "      name: section\n      bindings: engine-section.yaml");
}

#[test]
fn the_cases_of_a_run_are_its_tree_with_each_case_state_and_the_tree_follows_the_run_to_its_end() {
    let rig = Rig::new("cases-tree", &[Confinement::Local]);
    let run = rig.submit();
    rig.wait_review(&run);
    let rows = cases(&rig, &run);
    assert_eq!(tree_lines(&rows), ["(root) engine-demo Open", "  draft engine-draft AwaitingPerson"], "{rows:?}");
    rig.settle(&run, &["performed"]);
    let rows = cases(&rig, &run);
    assert_eq!(tree_lines(&rows), ["(root) engine-demo Ended delivered", "  draft engine-draft Ended passed"], "{rows:?}");
    let text = Value::Array(rows).to_string();
    assert!(!text.contains(rig.dir.to_str().unwrap()), "a row names a path of the host: {text}");
}

#[test]
fn a_case_whose_program_runs_is_running_while_the_run_goes_on() {
    let rig = Rig::new("cases-running", &[Confinement::Local]);
    let run = rig.submit_at(Confinement::Local, Some(rig.context(30)));
    rig.wait_started(&run, "draft.write");
    let rows = cases(&rig, &run);
    assert_eq!(tree_lines(&rows), ["(root) engine-demo Open", "  draft engine-draft Running"], "{rows:?}");
    until("the writer's pause to be seen and ended", 20, end_the_pauses);
    rig.wait_review(&run);
    assert_eq!(tree_lines(&cases(&rig, &run))[1], "  draft engine-draft AwaitingPerson");
}

#[test]
fn a_tree_three_levels_deep_is_answered_to_its_last_case() {
    let rig = Rig::new("cases-deep", &[Confinement::Local]);
    deepen(&rig.home);
    let run = rig.submit();
    until(&format!("run {run} to wait or end"), 40, || rig.state_of(&run) != "Running" && rig.state_of(&run) != "Submitted");
    assert_eq!(rig.state_of(&run), "WaitingOnDecision", "{} {:?}", rig.row(&run), rig.kinds(&run));
    rig.wait_review(&run);
    let rows = cases(&rig, &run);
    assert_eq!(tree_lines(&rows), ["(root) engine-demo Open", "  section engine-section Open", "    draft engine-draft AwaitingPerson"], "{rows:?}");
    rig.settle(&run, &["performed"]);
    assert_eq!(
        tree_lines(&cases(&rig, &run)),
        ["(root) engine-demo Ended delivered", "  section engine-section Ended passed", "    draft engine-draft Ended passed"]
    );
}

#[test]
fn the_cases_of_a_run_the_daemon_does_not_have_are_refused_by_name() {
    let rig = Rig::new("cases-unknown", &[Confinement::Local]);
    for id in ["run-that-is-not-there", "../runs", ""] {
        let query = HashMap::from([("run_id".to_string(), id.to_string())]);
        let answer = runbook::api::routes::view_answer("cases", rig.d(), &query);
        assert_eq!((answer.status.as_u16(), answer.body["refused"].clone()), (404, json!("no such run")), "{id:?}: {}", answer.body);
        assert!(answer.body["message"].as_str().unwrap().contains(id), "{}", answer.body);
    }
    let answer = runbook::api::routes::view_answer("cases", rig.d(), &HashMap::new());
    assert_eq!(answer.status.as_u16(), 400, "{}", answer.body);
}

// ---- the adversary's cases against the case tree ----

#[test]
fn a_case_that_waited_when_its_run_was_cancelled_is_not_stuck() {
    let rig = Rig::new("cases-cancelled", &[Confinement::Local]);
    let run = rig.submit();
    rig.wait_review(&run);
    rig.cancel(&run);
    rig.wait_state(&run, "Cancelled");
    // the wait ended with the run: the case is neither waiting on a person nor stuck on a stop of its own
    assert_eq!(tree_lines(&cases(&rig, &run)), ["(root) engine-demo Open", "  draft engine-draft Open"]);
}

#[test]
fn a_case_above_the_one_that_waited_is_not_stuck_after_a_cancel_either() {
    let rig = Rig::new("cases-cancelled-deep", &[Confinement::Local]);
    deepen(&rig.home);
    let run = rig.submit();
    rig.wait_review(&run);
    rig.cancel(&run);
    rig.wait_state(&run, "Cancelled");
    assert_eq!(tree_lines(&cases(&rig, &run)), ["(root) engine-demo Open", "  section engine-section Open", "    draft engine-draft Open"]);
}

#[test]
fn a_tree_left_without_a_move_shows_its_case_stuck() {
    let rig = Rig::build_with("cases-stuck", &[Confinement::Local], false, true);
    let run = rig.submit();
    rig.wait_state(&run, "Ended");
    assert_eq!(rig.row(&run)["outcome"], "failed: step draft.write exit 69", "{}", rig.row(&run));
    let lines = tree_lines(&cases(&rig, &run));
    assert_eq!(lines[1], "  draft engine-draft Stuck", "{lines:?}");
}

#[test]
fn a_reader_that_asks_for_the_tree_in_a_loop_while_the_run_writes_gets_the_tree_or_a_retry_and_never_a_path() {
    let rig = Rig::new("cases-loop", &[Confinement::Local]);
    let run = rig.submit();
    let state = rig.state_root().to_string_lossy().into_owned();
    let (mut trees, mut retries) = (0, 0);
    let limit = Instant::now() + Duration::from_secs(40);
    let mut answered: BTreeSet<String> = BTreeSet::new();
    loop {
        let query = HashMap::from([("run_id".to_string(), run.clone())]);
        let answer = runbook::api::routes::view_answer("cases", rig.d(), &query);
        let text = answer.body.to_string();
        assert!(!text.contains(&state), "an answer names a path of the host: {text}");
        match answer.status.as_u16() {
            200 => trees += 1,
            503 => {
                assert_eq!(answer.body["refused"], "tree not readable", "{text}");
                retries += 1;
            }
            other => panic!("{other}: {text}"),
        }
        if rig.state_of(&run) == "Ended" {
            break;
        }
        for d in rig.d().open_decisions_of_run(&run) {
            let id = d["request_id"].as_str().unwrap().to_string();
            if answered.insert(id.clone()) {
                let review = d["options"].as_array().unwrap().iter().any(|o| o["answer"].as_str().is_some_and(|a| a.contains("result")));
                rig.answer(&run, &id, if review { "performed" } else { "approve" }, "reviewer");
            }
        }
        assert!(Instant::now() < limit, "the run did not end");
    }
    assert!(trees > 0, "{trees} trees, {retries} retries");
    assert_eq!(tree_lines(&cases(&rig, &run)), ["(root) engine-demo Ended delivered", "  draft engine-draft Ended passed"]);
}

/// `levels` sections between the root and the drafting case: `section1` opens `section2` and so on; the last opens
/// `draft`.
fn deepen_to(home: &Path, levels: usize) {
    for i in 1..=levels {
        let next = if i == levels { ("engine.draft", "draft", "engine-draft.yaml".to_string()) } else { ("", "", String::new()) };
        let (opens, name, bindings) = if i == levels { (next.0.to_string(), next.1.to_string(), next.2) } else { (format!("engine.section{}", i + 1), format!("section{}", i + 1), format!("engine-section{}.yaml", i + 1)) };
        std::fs::write(
            home.join(format!("protocol/engine-section{i}.protocol.yaml")),
            format!("format: protocol/1\nprotocol:\n  id: engine.section{i}\n  revision: 1\n  description: >-\n    Level {i} of a deep tree.\nartifacts:\n  idea: {{}}\nevidence_kinds:\n  case_outcome: {{}}\n  draft_opened: {{}}\nclaims:\n  draft.opened:\n    true_when:\n      evidence: {{kind: draft_opened, subject: idea, result: opened}}\n  draft.passed:\n    true_when:\n      children: {{in: [passed]}}\nactions:\n  draft.open:\n    effect: write\n    case_open: {{protocol: {opens}, subject: idea}}\n    may_produce:\n      - evidence: draft_opened\noutcomes:\n  passed:\n    requires:\n      all:\n        - {{claim: draft.opened}}\n        - {{claim: draft.passed}}\n"),
        )
        .unwrap();
        std::fs::write(
            home.join(format!("bindings/engine-section{i}.yaml")),
            format!("format: loom.bindings/1\nprotocol: engine-section{i}@1\nprogram_root: bindings\nartifacts:\n  idea: idea.yaml\nactions:\n  - action: draft.open\n    executor:\n      kind: open\n      name: {name}\n      bindings: {bindings}\n    evidence:\n      - kind: draft_opened\n        subject: idea\n        producer: loom\n        from: effect\n        results: {{performed: opened}}\n"),
        )
        .unwrap();
    }
    replace_in(home, "protocol/engine-demo.protocol.yaml", "case_open: {protocol: engine.draft, subject: idea}", "case_open: {protocol: engine.section1, subject: idea}");
    replace_in(home, "bindings/engine-demo.yaml", "      name: draft\n      bindings: engine-draft.yaml", "      name: section1\n      bindings: engine-section1.yaml");
}

#[test]
fn a_tree_eight_levels_deep_is_answered_whole_with_each_case_under_its_parent() {
    let rig = Rig::new("cases-eight", &[Confinement::Local]);
    deepen_to(&rig.home, 6);
    let run = rig.submit();
    until(&format!("run {run} to wait or end"), 40, || !matches!(rig.state_of(&run).as_str(), "Running" | "Submitted"));
    assert_eq!(rig.state_of(&run), "WaitingOnDecision", "{} {:?}", rig.row(&run), rig.kinds(&run));
    let mut want = vec!["(root) engine-demo Open".to_string()];
    for i in 1..=6 {
        want.push(format!("{}section{i} engine-section{i} Open", "  ".repeat(i)));
    }
    want.push(format!("{}draft engine-draft AwaitingPerson", "  ".repeat(7)));
    assert_eq!(tree_lines(&cases(&rig, &run)), want);
}

/// The run event that names a Loom `Stopped` line the engine took as the end of one step.
const STEP_ENDED: &str = "runbook.step-ended";

/// What a reader of a run's events sees of each Loom `Stopped` line: its stop, its `seq`, and the
/// `runbook.step-ended` records between it and the next run-log line.
fn stops_and_step_ends(events: &[Value]) -> Vec<(String, i64, Vec<Value>)> {
    let mut out: Vec<(String, i64, Vec<Value>)> = Vec::new();
    let mut after_stop = false;
    for e in events {
        let kind = e["kind"].as_str().unwrap();
        if kind == STEP_ENDED {
            assert!(after_stop, "a {STEP_ENDED} with no Stopped line before it: {e}");
            out.last_mut().unwrap().2.push(e["record"].clone());
        } else if !kind.starts_with("runbook.") {
            after_stop = kind == "Stopped";
            if after_stop {
                out.push((e["record"]["stop"].as_str().unwrap().to_owned(), e["record"]["seq"].as_i64().unwrap(), Vec::new()));
            }
        }
    }
    out
}

#[test]
fn a_stopped_line_that_ends_one_step_is_named_so_and_one_that_ends_or_holds_the_run_is_not() {
    // The engine runs the tree one step at a time; Loom ends each call with `Stopped` `BudgetSpent`. A reader of the
    // events tells that end of a step from a wait and from the run's end by the event `runbook.step-ended`.
    let rig = Rig::new("engine-step-ended", &[Confinement::Local]);
    let run = rig.submit();
    rig.settle(&run, &["rejected", "performed"]);
    assert_eq!(rig.row(&run)["outcome"], "delivered");
    let seen = stops_and_step_ends(&rig.d().events_after(&run, 0));
    let steps = seen.iter().filter(|(stop, ..)| stop == "BudgetSpent").count();
    assert!(steps >= 2, "the run took fewer than two steps that a budget stop ended: {seen:?}");
    assert!(seen.iter().any(|(stop, ..)| stop == "AwaitingPerson"), "the run never waited on the review: {seen:?}");
    for (stop, seq, ends) in &seen {
        if stop != "BudgetSpent" {
            assert!(ends.is_empty(), "a {stop} line is named as a step's end: {ends:?}");
            continue;
        }
        assert_eq!(ends.len(), 1, "the BudgetSpent line {seq} has not exactly one {STEP_ENDED}: {seen:?}");
        assert_eq!(ends[0]["seq"], json!(seq), "{ends:?}");
        let case = ends[0]["case"].as_str().expect("the event names its case");
        let log = std::fs::read_to_string(rig.store(&run).join(b10x_loom_tree::log_file_name(case))).unwrap();
        let line: Value = log.lines().map(|l| serde_json::from_str::<Value>(l).unwrap()).find(|l| l["seq"] == json!(seq)).expect("the named line is in the case's log");
        assert_eq!((line["kind"].as_str(), line["stop"].as_str(), line["budget"].as_str()), (Some("Stopped"), Some("BudgetSpent"), Some("MaxSteps")), "{line}");
    }

    // A run whose tree is left with no move ends on a `Stuck` line: no step's end is named for it.
    let rig = Rig::build_with("engine-step-ended-stuck", &[Confinement::Local], false, true);
    let run = rig.submit();
    until("the run to end", 40, || rig.state_of(&run) == "Ended");
    let seen = stops_and_step_ends(&rig.d().events_after(&run, 0));
    let (stop, _, ends) = seen.last().expect("the run has a Stopped line");
    assert_eq!((stop.as_str(), ends.len()), ("Stuck", 0), "{seen:?}");
}
