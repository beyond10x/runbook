//! The watcher on the run directory of `runbook run` (a Loom run): the real binary on a directory
//! written by hand with invented text, as the runner leaves it (`settings.json`, `work/`,
//! `rounds.jsonl`, call records under `work/.sdc/llm/`). A change is recorded with the `tree run`
//! step that was in flight and the call record that was open (docs/fs-changes.md).

use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn base() -> PathBuf {
    let b = std::env::var_os("SDC_WATCH_TEST_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")));
    fs::create_dir_all(&b).unwrap();
    b
}

fn scratch(name: &str) -> PathBuf {
    let dir = base().join(format!("{name}-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(dir.join("work")).unwrap();
    fs::write(dir.join("settings.json"), "{}\n").unwrap();
    dir
}

fn wait_for(what: &str, mut ready: impl FnMut() -> bool) {
    let t = Instant::now();
    while !ready() {
        assert!(t.elapsed() < Duration::from_secs(20), "{what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn pause() {
    std::thread::sleep(Duration::from_millis(150));
}

struct Watched {
    dir: PathBuf,
    child: Option<Child>,
}

impl Watched {
    fn start(dir: PathBuf) -> Watched {
        let child = Command::new(env!("CARGO_BIN_EXE_sdc-watch")).arg("run").arg(&dir).spawn().unwrap();
        let pid = dir.join("fs/sdc-watch.pid");
        wait_for("sdc-watch did not start", || pid.exists());
        Watched { dir, child: Some(child) }
    }

    /// SIGTERM, wait, and read the record.
    fn stop(&mut self) -> Vec<Value> {
        let mut child = self.child.take().unwrap();
        unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
        assert!(child.wait().unwrap().success());
        let text = fs::read_to_string(self.dir.join("fs/changes.jsonl")).unwrap_or_default();
        text.lines().map(|l| serde_json::from_str::<Value>(l).unwrap()).filter(|v| v["format"] == "sdc.fs-change/1").collect()
    }
}

impl Drop for Watched {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        if !std::thread::panicking() {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }
}

fn append(path: &Path, line: &str) {
    let mut f = fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
    f.write_all(format!("{line}\n").as_bytes()).unwrap();
}

/// A step of the runner ends: one line of `rounds.jsonl`.
fn step_made(dir: &Path) {
    append(&dir.join("rounds.jsonl"), r#"{"started_epoch":1,"ended_epoch":2,"seconds":1,"verdict":"budget_spent","case_dir":""}"#);
}

/// `status.json` of a call record, written as the model tool writes it: a temporary file, a rename.
fn call_status(record: &Path, state: &str) {
    let tmp = record.join(".status.json.XXXXXX");
    fs::write(&tmp, json!({"format": "sdc.call-status/1", "state": state, "pid": 1}).to_string()).unwrap();
    fs::rename(tmp, record.join("status.json")).unwrap();
}

fn by_path<'a>(changes: &'a [Value], path: &str) -> &'a Value {
    let hits: Vec<&Value> = changes.iter().filter(|c| c["path"] == path).collect();
    assert_eq!(hits.len(), 1, "{path}: {hits:?} in {changes:?}");
    hits[0]
}

#[test]
fn a_change_is_recorded_with_the_step_in_flight_and_the_call_that_was_open() {
    let dir = scratch("steps");
    let work = dir.join("work");
    let mut w = Watched::start(dir.clone());
    // step 1: a program writes a file; the step ends
    fs::write(work.join("a.txt"), "one\ntwo\n").unwrap();
    pause();
    step_made(&dir);
    pause();
    // step 2: a model call opens, writes in its own directory and in the product tree, and ends
    let record = work.join(".sdc/llm/001-change.implement");
    fs::create_dir_all(record.join("cwd/out")).unwrap();
    fs::write(record.join("request.json"), json!({"case": "IDEA-1.story", "node": "change.implement"}).to_string()).unwrap();
    call_status(&record, "running");
    pause();
    fs::write(record.join("cwd/out/x.rs"), "fn x() {}\n").unwrap();
    fs::write(work.join("b.txt"), "during the call\n").unwrap();
    pause();
    call_status(&record, "finished");
    pause();
    append(&work.join(".sdc/llm/calls.jsonl"), r#"{"action":"change.implement","cost_usd":0}"#);
    fs::write(work.join("c.txt"), "after the call\n").unwrap();
    pause();
    step_made(&dir);
    pause();
    // step 3 never ends
    fs::write(work.join("d.txt"), "last\n").unwrap();
    pause();
    let c = w.stop();
    let rec = ".sdc/llm/001-change.implement";
    let rec = format!("work/{rec}");

    let a = by_path(&c, "a.txt");
    assert_eq!(a["root"], "work");
    assert_eq!(a["attribution"], json!({"case": null, "action": null, "record": null, "tool": null, "basis": "step-window", "step": 1}));

    let b = by_path(&c, "b.txt");
    assert_eq!(b["attribution"], json!({"case": "IDEA-1.story", "action": "change.implement", "record": rec, "tool": null, "basis": "step-window", "step": 2}));

    let x = by_path(&c, "out/x.rs");
    assert_eq!(x["root"], format!("{rec}/cwd"));
    assert_eq!(x["attribution"]["record"], rec);
    assert_eq!(x["attribution"]["action"], "change.implement");
    assert_eq!(x["attribution"]["step"], 2);

    let after = by_path(&c, "c.txt");
    assert_eq!(after["attribution"], json!({"case": null, "action": null, "record": null, "tool": null, "basis": "step-window", "step": 2}));

    assert_eq!(by_path(&c, "d.txt")["attribution"]["step"], 3);
    let paths: Vec<&str> = c.iter().map(|v| v["path"].as_str().unwrap()).collect();
    for transport in ["request.json", "status.json", "calls.jsonl", ".status.json.XXXXXX", "rounds.jsonl", "settings.json"] {
        assert!(!paths.iter().any(|p| p.ends_with(transport)), "{transport} recorded: {paths:?}");
    }
    assert_eq!(c.len(), 5, "{paths:?}");
    let seqs: Vec<u64> = c.iter().map(|v| v["seq"].as_u64().unwrap()).collect();
    assert_eq!(seqs, (0..5).collect::<Vec<u64>>());
}

#[test]
fn the_projection_of_the_store_and_the_logs_of_loom_are_not_roots() {
    let dir = scratch("store");
    fs::create_dir_all(dir.join("store/IDEA-1/work")).unwrap();
    let mut w = Watched::start(dir.clone());
    fs::write(dir.join("store/IDEA-1/work/findings.yaml"), "findings: []\n").unwrap();
    fs::write(dir.join("store/runlog-case-ab.jsonl"), "{}\n").unwrap();
    fs::write(dir.join("work/product.txt"), "kept\n").unwrap();
    pause();
    let c = w.stop();
    assert_eq!(c.len(), 1, "{c:?}");
    assert_eq!(c[0]["path"], "product.txt");
    assert_eq!(c[0]["attribution"]["step"], 1);
}

fn alive(pid: i32) -> bool {
    // a zombie has ended; it only waits for whoever adopted it to read its exit
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(s) => s.rsplit(") ").next().is_some_and(|rest| !rest.starts_with('Z')),
        Err(_) => false,
    }
}

#[test]
fn the_watcher_ends_itself_when_its_parent_is_killed() {
    let dir = scratch("parent");
    let pids = dir.join("watcher.pid");
    // the shell is the parent of the watcher, as the runner is
    let mut parent = Command::new("sh")
        .arg("-c")
        .arg(r#""$BIN" run "$DIR" --parent $$ & echo $! > "$PIDS"; wait"#)
        .env("BIN", env!("CARGO_BIN_EXE_sdc-watch"))
        .env("DIR", &dir)
        .env("PIDS", &pids)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_for("the watcher did not start", || dir.join("fs/sdc-watch.pid").exists() && fs::read_to_string(&pids).is_ok_and(|s| s.trim().parse::<i32>().is_ok()));
    let watcher: i32 = fs::read_to_string(&pids).unwrap().trim().parse().unwrap();
    assert!(alive(watcher));
    unsafe { libc::kill(parent.id() as i32, libc::SIGKILL) };
    parent.wait().unwrap();
    let t = Instant::now();
    wait_for("the watcher outlived its killed parent", || !alive(watcher));
    assert!(t.elapsed() < Duration::from_secs(3), "{:?}", t.elapsed());
    assert!(!dir.join("fs/sdc-watch.pid").exists(), "it stopped as on SIGTERM: flushed, pid file removed");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_parent_that_is_not_the_parent_is_refused() {
    let dir = scratch("not-parent");
    let out = Command::new(env!("CARGO_BIN_EXE_sdc-watch")).arg("run").arg(&dir).args(["--parent", "1"]).stdin(Stdio::null()).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("already gone"), "{}", String::from_utf8_lossy(&out.stderr));
    let _ = fs::remove_dir_all(&dir);
}
