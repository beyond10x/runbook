//! The engine (`runbook::engine`): a case tree run through Loom's library calls inside the daemon's
//! process. The fixture runbook has two levels: a root case that opens a drafting case; the drafting
//! case has a writer (a POSIX sh program) and one person step with the named results `performed` and
//! `rejected`.

use super::state::scratch;
use runbook::commands::{AnswerDecision, CancelRun, Config, Daemon, SubmitRun};
use runbook::engine::{Engine, StoreState};
use runbook::runtime::{Confinement, Recovery, Runtime};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

const RUNBOOK: &str = "engine-demo";
const FORBIDDEN: [&str; 4] = ["b10x-loom", "loom", "canon", "sdc-drive"];

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
        let dir = scratch(tag);
        let home = dir.join("home");
        copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &home);
        // A person step is answered as the principal its binding names: here, the daemon's.
        let binding = home.join("bindings/engine-draft.yaml");
        let text = std::fs::read_to_string(&binding).unwrap();
        assert!(text.contains("principal: reviewer"));
        std::fs::write(&binding, text.replace("principal: reviewer", &format!("principal: {}", super::PRINCIPAL))).unwrap();
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
