//! End-to-end tests: the real binary on a run dir under SDC_WATCH_TEST_DIR (else the crate's
//! target tmp dir), each writer as a real process, records read back from changes.jsonl.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn base() -> PathBuf {
    let b = std::env::var_os("SDC_WATCH_TEST_DIR").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")));
    fs::create_dir_all(&b).unwrap();
    b
}

fn now_iso(offset_ms: i64) -> String {
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64 + offset_ms;
    // Same civil conversion as the binary; kept local so tests depend on the CLI only.
    let (days, rem) = (ms.div_euclid(86_400_000), ms.rem_euclid(86_400_000));
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:03}Z", rem / 3_600_000, rem / 60_000 % 60, rem / 1000 % 60, rem % 1000)
}

struct Run {
    dir: PathBuf,
    child: Option<Child>,
}

impl Run {
    /// A run with one case tree: store/C (root case, real work/) and store/C/children/C.k (work
    /// is a link to the parent's).
    fn new(name: &str) -> Run {
        let dir = base().join(format!("{name}-{}-{}", std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(dir.join("store/C/work")).unwrap();
        fs::create_dir_all(dir.join("store/C/children/C.k/evidence")).unwrap();
        std::os::unix::fs::symlink("../../work", dir.join("store/C/children/C.k/work")).unwrap();
        fs::write(dir.join("store/C/events.jsonl"), "{\"seq\":0,\"event\":\"opened\",\"at\":\"-\"}\n").unwrap();
        fs::write(dir.join("store/C/children/C.k/events.jsonl"), "{\"seq\":0,\"event\":\"opened\",\"at\":\"-\"}\n").unwrap();
        Run { dir, child: None }
    }
    fn work(&self) -> PathBuf {
        self.dir.join("store/C/work")
    }
    fn start(&mut self) {
        let pid = self.dir.join("fs/sdc-watch.pid");
        let _ = fs::remove_file(&pid);
        let child = Command::new(env!("CARGO_BIN_EXE_sdc-watch")).arg("run").arg(&self.dir).spawn().unwrap();
        let t = Instant::now();
        while !pid.exists() {
            assert!(t.elapsed() < Duration::from_secs(20), "sdc-watch did not start");
            std::thread::sleep(Duration::from_millis(20));
        }
        self.child = Some(child);
    }
    /// Runs a shell script in work/.
    fn sh(&self, script: &str) {
        self.sh_in(&self.work(), script);
    }
    fn sh_in(&self, dir: &Path, script: &str) {
        let st = Command::new("sh").arg("-c").arg(script).current_dir(dir).status().unwrap();
        assert!(st.success(), "{script}");
    }
    fn pause(&self) {
        std::thread::sleep(Duration::from_millis(400));
    }
    fn append(&self, rel: &str, line: &str) {
        use std::io::Write;
        let mut f = fs::OpenOptions::new().create(true).append(true).open(self.dir.join(rel)).unwrap();
        f.write_all(format!("{line}\n").as_bytes()).unwrap();
    }
    /// SIGTERM, wait, and read the record: (changes, attribution lines).
    fn stop_with(&mut self, sig: i32) -> (Vec<Value>, Vec<Value>) {
        let mut child = self.child.take().unwrap();
        unsafe { libc::kill(child.id() as i32, sig) };
        let st = child.wait().unwrap();
        assert!(st.success(), "exit {st:?}");
        assert!(!self.dir.join("fs/sdc-watch.pid").exists(), "pid file removed");
        let text = fs::read_to_string(self.dir.join("fs/changes.jsonl")).unwrap_or_default();
        let all: Vec<Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        let (c, a) = all.into_iter().partition(|v| v["format"] == "sdc.fs-change/1");
        (c, a)
    }
    fn stop(&mut self) -> Vec<Value> {
        self.stop_with(libc::SIGTERM).0
    }
}

impl Drop for Run {
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

fn ops(v: &[Value]) -> Vec<(String, String)> {
    v.iter().map(|c| (c["op"].as_str().unwrap().to_string(), c["path"].as_str().unwrap().to_string())).collect()
}

fn op(o: &str, p: &str) -> (String, String) {
    (o.to_string(), p.to_string())
}

#[test]
fn edit_tool_write_in_place_is_one_modify_with_diff() {
    let mut r = Run::new("edit");
    fs::write(r.work().join("a.txt"), "one\ntwo\nthree\n").unwrap();
    r.start();
    // What an Edit/Write tool does: truncate and write.
    fs::write(r.work().join("a.txt"), "one\nTWO\nthree\nfour\n").unwrap();
    r.pause();
    let c = r.stop();
    assert_eq!(ops(&c), vec![op("modify", "a.txt")]);
    let ch = &c[0];
    assert_eq!(ch["root"], "store/C/work");
    assert_eq!(ch["seq"], 0);
    assert_eq!((ch["lines_added"].as_u64(), ch["lines_removed"].as_u64()), (Some(2), Some(1)));
    let diff = ch["diff"].as_str().unwrap();
    assert!(diff.contains("--- a/a.txt") && diff.contains("-two") && diff.contains("+TWO") && diff.contains("+four"), "{diff}");
    assert!(r.dir.join("fs/blobs").join(ch["before"].as_str().unwrap()).exists());
    assert!(r.dir.join("fs/blobs").join(ch["after"].as_str().unwrap()).exists());
    assert_eq!(ch["attribution"]["basis"], "none");
}

#[test]
fn sed_in_place_is_one_modify_and_no_temp_file() {
    let mut r = Run::new("sed");
    fs::write(r.work().join("lib.rs"), "fn a() {}\nfn b() {}\n").unwrap();
    r.start();
    r.sh("sed -i 's/fn b/fn c/' lib.rs");
    r.pause();
    let c = r.stop();
    assert_eq!(ops(&c), vec![op("modify", "lib.rs")]);
    assert!(c[0]["diff"].as_str().unwrap().contains("+fn c() {}"));
}

#[test]
fn rustfmt_is_one_modify() {
    let mut r = Run::new("rustfmt");
    fs::write(r.work().join("lib.rs"), "pub fn   f( )->u32{ 42 }\n").unwrap();
    r.start();
    r.sh("rustfmt --edition 2021 lib.rs");
    r.pause();
    let c = r.stop();
    assert_eq!(ops(&c), vec![op("modify", "lib.rs")]);
    assert!(c[0]["diff"].as_str().unwrap().contains("+pub fn f() -> u32 {"));
}

#[test]
fn cp_over_and_to_new() {
    let mut r = Run::new("cp");
    fs::write(r.work().join("src.txt"), "new\n").unwrap();
    fs::write(r.work().join("dst.txt"), "old\n").unwrap();
    r.start();
    r.sh("cp src.txt dst.txt && cp src.txt fresh.txt");
    r.pause();
    let mut c = ops(&r.stop());
    c.sort();
    assert_eq!(c, vec![op("create", "fresh.txt"), op("modify", "dst.txt")]);
}

#[test]
fn mv_is_a_rename_and_over_an_existing_file_also_a_delete() {
    let mut r = Run::new("mv");
    fs::write(r.work().join("a.txt"), "a\n").unwrap();
    fs::write(r.work().join("b.txt"), "b\n").unwrap();
    fs::write(r.work().join("c.txt"), "c\n").unwrap();
    r.start();
    r.sh("mv a.txt moved.txt");
    r.pause();
    r.sh("mv b.txt c.txt");
    r.pause();
    let c = r.stop();
    assert_eq!(ops(&c), vec![op("rename", "moved.txt"), op("delete", "c.txt"), op("rename", "c.txt")]);
    assert_eq!(c[0]["from_path"], "a.txt");
    assert_eq!(c[0]["before"], c[0]["after"]);
    assert_eq!(c[0]["diff"], Value::Null);
    assert_eq!(c[2]["from_path"], "b.txt");
}

#[test]
fn rm_is_a_delete_and_rm_r_deletes_each_file() {
    let mut r = Run::new("rm");
    fs::create_dir_all(r.work().join("d/e")).unwrap();
    fs::write(r.work().join("x.txt"), "x\n").unwrap();
    fs::write(r.work().join("d/e/y.txt"), "y\n").unwrap();
    r.start();
    r.sh("rm x.txt && rm -r d");
    r.pause();
    let mut c = r.stop();
    c.sort_by_key(|v| v["path"].as_str().unwrap().to_string());
    assert_eq!(ops(&c), vec![op("delete", "d/e/y.txt"), op("delete", "x.txt")]);
    assert_eq!(c[1]["after"], Value::Null);
    assert_eq!(c[1]["diff"].as_str().unwrap().lines().last(), Some("-x"));
}

#[test]
fn a_script_writing_files_in_a_new_nested_dir() {
    let mut r = Run::new("script");
    r.start();
    r.sh("mkdir -p gen/deep/er && for i in 1 2 3; do echo \"line $i\" > gen/deep/er/$i.txt; done && \
          for i in 1 2 3 4 5; do echo \"more $i\" >> gen/log.txt; done");
    r.pause();
    // A directory created after the first one, inside it, is watched too.
    r.sh("mkdir gen/deep/er/later && echo late > gen/deep/er/later/z.txt");
    r.pause();
    let mut c = ops(&r.stop());
    c.sort();
    assert_eq!(c, vec![op("create", "gen/deep/er/1.txt"), op("create", "gen/deep/er/2.txt"),
        op("create", "gen/deep/er/3.txt"), op("create", "gen/deep/er/later/z.txt"), op("create", "gen/log.txt")]);
}

#[test]
fn editor_writing_through_a_rename_is_a_modify() {
    let mut r = Run::new("editor");
    fs::write(r.work().join("notes.md"), "a\n").unwrap();
    r.start();
    r.sh("printf 'a\\nb\\n' > .notes.md.swp && mv .notes.md.swp notes.md");
    r.pause();
    let c = r.stop();
    assert_eq!(ops(&c), vec![op("modify", "notes.md")]);
    assert_eq!(c[0]["lines_added"], 1);
}

#[test]
fn binary_and_too_large_have_no_diff() {
    let mut r = Run::new("binary");
    r.start();
    fs::write(r.work().join("img.bin"), [0u8, 1, 2, 255, 0, 7]).unwrap();
    fs::write(r.work().join("big.txt"), "x".repeat(2 << 20)).unwrap();
    r.pause();
    let mut c = r.stop();
    c.sort_by_key(|v| v["path"].as_str().unwrap().to_string());
    assert_eq!(ops(&c), vec![op("create", "big.txt"), op("create", "img.bin")]);
    assert_eq!((c[0]["too_large"].as_bool(), c[0]["binary"].as_bool(), &c[0]["diff"]), (Some(true), Some(false), &Value::Null));
    assert!(!r.dir.join("fs/blobs").join(c[0]["after"].as_str().unwrap()).exists(), "no blob for too large");
    assert_eq!((c[1]["binary"].as_bool(), &c[1]["diff"], &c[1]["lines_added"]), (Some(true), &Value::Null, &Value::Null));
}

#[test]
fn unchanged_rewrite_and_back_and_forth_record_nothing() {
    let mut r = Run::new("unchanged");
    fs::write(r.work().join("same.txt"), "same\n").unwrap();
    fs::write(r.work().join("flip.txt"), "orig\n").unwrap();
    r.start();
    r.sh("echo same > same.txt && echo changed > flip.txt && echo orig > flip.txt && echo tmp > t && rm t");
    r.pause();
    assert!(r.stop().is_empty());
}

#[test]
fn excluded_dirs_record_nothing() {
    let mut r = Run::new("excluded");
    r.start();
    r.sh("mkdir -p target/debug .git node_modules/x && echo 1 > target/debug/a && echo 2 > .git/HEAD && echo 3 > node_modules/x/i.js");
    r.sh("mkdir -p ../evidence && echo 4 > ../evidence/e.json && echo 5 > ../case.json");
    r.pause();
    assert!(r.stop().is_empty());
}

#[test]
fn action_window_from_the_case_log() {
    let mut r = Run::new("action");
    r.start();
    r.append("store/C/children/C.k/events.jsonl", r#"{"seq":1,"event":"action_started","action":"change.implement","at":"x"}"#);
    r.sh("echo in > inside.txt");
    r.pause();
    r.append("store/C/children/C.k/events.jsonl", r#"{"seq":2,"event":"action_finished","action":"change.implement","at":"x"}"#);
    r.sh("echo out > outside.txt");
    r.pause();
    let c = r.stop();
    assert_eq!(ops(&c), vec![op("create", "inside.txt"), op("create", "outside.txt")]);
    let a = &c[0]["attribution"];
    assert_eq!((a["case"].as_str(), a["action"].as_str(), a["basis"].as_str()), (Some("C.k"), Some("change.implement"), Some("action-window")));
    assert_eq!(a["record"], Value::Null);
    assert_eq!(c[1]["attribution"]["basis"], "none");
    assert_eq!(c[1]["attribution"]["case"], Value::Null);
}

/// A fake agent call: the case starts the action, the record appears, the metaharness stream
/// shows tool calls while their writes happen.
#[test]
fn tool_window_from_the_metaharness_stream() {
    let mut r = Run::new("tool");
    fs::create_dir_all(r.dir.join("llm")).unwrap();
    r.start();
    r.append("store/C/children/C.k/events.jsonl", r#"{"seq":1,"event":"action_started","action":"change.implement","at":"x"}"#);
    let rec = r.dir.join("llm/001-change.implement");
    fs::create_dir_all(rec.join("cwd/out")).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    let s = "llm/001-change.implement/events.jsonl";
    r.append(s, &format!(r#"{{"format":"metaharness.event/1","seq":1,"event":"session.started","at":"{}"}}"#, now_iso(0)));
    r.append(s, &format!(r#"{{"format":"metaharness.event/1","seq":7,"at":"{}","event":"tool.requested","call_id":"t1","name":"Write","input":{{"file_path":"{}/cwd/out/lib.rs"}}}}"#,
        now_iso(0), rec.display()));
    std::thread::sleep(Duration::from_millis(30));
    fs::write(rec.join("cwd/out/lib.rs"), "pub fn f() {}\n").unwrap();
    std::thread::sleep(Duration::from_millis(30));
    r.append(s, &format!(r#"{{"format":"metaharness.event/1","seq":9,"at":"{}","event":"tool.result","call_id":"t1"}}"#, now_iso(0)));
    std::thread::sleep(Duration::from_millis(100));
    r.append(s, &format!(r#"{{"format":"metaharness.event/1","seq":10,"at":"{}","event":"tool.requested","call_id":"t2","name":"Bash","input":{{"command":"rustfmt out/lib.rs"}}}}"#, now_iso(0)));
    std::thread::sleep(Duration::from_millis(30));
    r.sh_in(&rec.join("cwd"), "printf 'pub fn f() {}\\npub fn g() {}\\n' > out/lib.rs");
    r.pause();
    r.append(s, &format!(r#"{{"format":"metaharness.event/1","seq":12,"at":"{}","event":"tool.result","call_id":"t2"}}"#, now_iso(0)));
    r.append(s, &format!(r#"{{"format":"metaharness.event/1","seq":13,"at":"{}","event":"session.ended"}}"#, now_iso(0)));
    r.pause();
    // The apply step copies the output into work/, still inside the action.
    r.sh_in(&r.work(), "cp ../../../llm/001-change.implement/cwd/out/lib.rs lib.rs");
    r.pause();
    r.append("store/C/children/C.k/events.jsonl", r#"{"seq":2,"event":"action_finished","action":"change.implement","at":"x"}"#);
    r.pause();
    let c = r.stop();
    assert_eq!(ops(&c), vec![op("create", "out/lib.rs"), op("modify", "out/lib.rs"), op("create", "lib.rs")]);
    let a0 = &c[0]["attribution"];
    assert_eq!(c[0]["root"], "llm/001-change.implement/cwd");
    assert_eq!((a0["case"].as_str(), a0["action"].as_str(), a0["record"].as_str(), a0["basis"].as_str()),
        (Some("C.k"), Some("change.implement"), Some("llm/001-change.implement"), Some("tool-window")));
    assert_eq!((a0["tool"]["seq"].as_u64(), a0["tool"]["name"].as_str()), (Some(7), Some("Write")));
    assert_eq!((c[1]["attribution"]["tool"]["seq"].as_u64(), c[1]["attribution"]["tool"]["input"].as_str()),
        (Some(10), Some("rustfmt out/lib.rs")));
    let a2 = &c[2]["attribution"];
    assert_eq!((a2["case"].as_str(), a2["record"].as_str(), a2["basis"].as_str(), &a2["tool"]),
        (Some("C.k"), Some("llm/001-change.implement"), Some("action-window"), &Value::Null));
}

/// The stream lags past the hold: the change goes out without a tool, and an attribution line
/// follows once the stream shows the call.
#[test]
fn late_stream_appends_an_attribution_line() {
    let mut r = Run::new("late");
    fs::create_dir_all(r.dir.join("llm/001-x.y/cwd")).unwrap();
    let s = "llm/001-x.y/events.jsonl";
    r.append(s, &format!(r#"{{"seq":1,"event":"session.started","at":"{}"}}"#, now_iso(-5000)));
    r.append("store/C/events.jsonl", r#"{"seq":1,"event":"action_started","action":"x.y","at":"x"}"#);
    r.start();
    let t0 = now_iso(-10);
    fs::write(r.dir.join("llm/001-x.y/cwd/f.txt"), "f\n").unwrap();
    std::thread::sleep(Duration::from_millis(2200));
    r.append(s, &format!(r#"{{"seq":5,"at":"{t0}","event":"tool.requested","call_id":"a","name":"Bash","input":{{"command":"./gen.sh"}}}}"#));
    r.append(s, &format!(r#"{{"seq":6,"at":"{}","event":"tool.result","call_id":"a"}}"#, now_iso(0)));
    r.pause();
    let (c, a) = r.stop_with(libc::SIGINT);
    assert_eq!(ops(&c), vec![op("create", "f.txt")]);
    assert_eq!(c[0]["attribution"]["basis"], "action-window");
    assert_eq!(c[0]["attribution"]["case"], "C");
    assert_eq!(a.len(), 1, "{a:?}");
    assert_eq!(a[0]["format"], "sdc.fs-attribution/1");
    assert_eq!(a[0]["change"], c[0]["seq"]);
    assert_eq!((a[0]["attribution"]["basis"].as_str(), a[0]["attribution"]["tool"]["seq"].as_u64()), (Some("tool-window"), Some(5)));
}

/// Two cases have the same action open, so the open-action guess takes the one started last
/// (C.k). An agent call whose `request.json` names its case is attributed to that case, even when
/// the request appears after the guess was made; a call without one keeps the guess.
#[test]
fn agent_record_takes_its_case_from_request_json() {
    let mut r = Run::new("request");
    fs::create_dir_all(r.dir.join("llm")).unwrap();
    r.start();
    r.append("store/C/events.jsonl", r#"{"seq":1,"event":"action_started","action":"x.y","at":"x"}"#);
    r.append("store/C/children/C.k/events.jsonl", r#"{"seq":1,"event":"action_started","action":"x.y","at":"x"}"#);
    r.pause();
    let named = r.dir.join("llm/001-x.y");
    fs::create_dir_all(named.join("cwd")).unwrap();
    r.pause();
    fs::write(named.join("request.json"), r#"{"node":"x.y","case":"C","case_dir":"/somewhere/store/C"}"#).unwrap();
    fs::write(named.join("cwd/named.txt"), "n\n").unwrap();
    let unnamed = r.dir.join("llm/002-x.y");
    fs::create_dir_all(unnamed.join("cwd")).unwrap();
    r.pause();
    fs::write(unnamed.join("cwd/guessed.txt"), "g\n").unwrap();
    r.pause();
    let c = r.stop();
    assert_eq!(ops(&c), vec![op("create", "named.txt"), op("create", "guessed.txt")]);
    let (a, g) = (&c[0]["attribution"], &c[1]["attribution"]);
    assert_eq!((a["case"].as_str(), a["action"].as_str(), a["record"].as_str()),
        (Some("C"), Some("x.y"), Some("llm/001-x.y")));
    assert_eq!((g["case"].as_str(), g["record"].as_str()), (Some("C.k"), Some("llm/002-x.y")));
}

#[test]
fn decider_record_takes_its_case_from_the_request() {
    let mut r = Run::new("decider");
    r.start();
    let rec = r.dir.join("decider/001-roadmap.approve");
    fs::create_dir_all(rec.join("cwd")).unwrap();
    fs::write(rec.join("decide-request.json"), format!(r#"{{"case_dir":"{}/store/C/children/C.k","action":"roadmap.approve"}}"#, r.dir.display())).unwrap();
    r.pause();
    fs::write(rec.join("cwd/answer.json"), "{}\n").unwrap();
    r.pause();
    let c = r.stop();
    assert_eq!(ops(&c), vec![op("create", "answer.json")]);
    let a = &c[0]["attribution"];
    assert_eq!((a["case"].as_str(), a["action"].as_str(), a["basis"].as_str()), (Some("C.k"), Some("roadmap.approve"), Some("record")));
}

#[test]
fn restart_records_what_changed_meanwhile_and_continues_seq() {
    let mut r = Run::new("restart");
    fs::write(r.work().join("a.txt"), "1\n").unwrap();
    r.start();
    fs::write(r.work().join("a.txt"), "2\n").unwrap();
    r.pause();
    assert_eq!(r.stop().len(), 1);
    fs::write(r.work().join("a.txt"), "3\n").unwrap();
    fs::write(r.work().join("b.txt"), "b\n").unwrap();
    r.start();
    r.pause();
    let c = r.stop();
    assert_eq!(c.iter().map(|v| v["seq"].as_u64().unwrap()).collect::<Vec<_>>(), vec![0, 1, 2]);
    let mut tail = ops(&c[1..]);
    tail.sort();
    assert_eq!(tail, vec![op("create", "b.txt"), op("modify", "a.txt")]);
    assert!(c[1..].iter().any(|v| v["diff"].as_str().is_some_and(|d| d.contains("-2\n+3"))));
}

#[test]
fn scan_writes_a_snapshot_and_no_record() {
    let r = Run::new("scan");
    fs::write(r.work().join("a.txt"), "a\n").unwrap();
    fs::create_dir_all(r.dir.join("llm/001-q.w/cwd/in")).unwrap();
    fs::write(r.dir.join("llm/001-q.w/cwd/in/i.yaml"), "i: 1\n").unwrap();
    fs::write(r.dir.join("llm/001-q.w/prompt.md"), "not tracked\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_sdc-watch")).arg("scan").arg(&r.dir).output().unwrap();
    assert!(out.status.success());
    let snap: Value = serde_json::from_slice(&fs::read(r.dir.join("fs/snapshot.json")).unwrap()).unwrap();
    let files: Vec<(String, String)> = snap["files"].as_array().unwrap().iter()
        .map(|f| (f["root"].as_str().unwrap().into(), f["path"].as_str().unwrap().into())).collect();
    assert_eq!(files, vec![("llm/001-q.w/cwd".into(), "in/i.yaml".into()), ("store/C/work".into(), "a.txt".into())]);
    assert!(!r.dir.join("fs/changes.jsonl").exists());
}

// ---- the temporary files of the watcher are made as new files that follow no link ----

#[test]
fn a_link_a_program_put_at_the_temporary_name_of_a_blob_is_not_written_through() {
    let mut r = Run::new("blob-link");
    r.start();
    let pid = fs::read_to_string(r.dir.join("fs/sdc-watch.pid")).unwrap().trim().to_string();
    let content = "content a program of the run wrote\n";
    let probe = r.dir.join("probe.txt");
    fs::write(&probe, content).unwrap();
    let sum = Command::new("sha256sum").arg(&probe).output().unwrap();
    let sha = String::from_utf8(sum.stdout).unwrap().split_whitespace().next().unwrap().to_string();
    let outside = r.dir.parent().unwrap().join(format!("outside-{}.txt", std::process::id()));
    fs::write(&outside, "keep\n").unwrap();
    std::os::unix::fs::symlink(&outside, r.dir.join("fs/blobs").join(format!(".{sha}.{pid}"))).unwrap();
    fs::write(r.work().join("a.txt"), content).unwrap();
    r.pause();
    let _ = r.stop();
    let after = fs::read_to_string(&outside).unwrap();
    assert_eq!(after, "keep\n", "the watcher wrote the content of a file of the run into a file outside the run, through a link a program put at the temporary name of its blob");
}

#[test]
fn a_link_a_program_put_at_the_temporary_name_of_the_snapshot_is_not_written_through() {
    let r = Run::new("snapshot-link");
    fs::write(r.work().join("a.txt"), "a\n").unwrap();
    fs::create_dir_all(r.dir.join("fs")).unwrap();
    let outside = r.dir.parent().unwrap().join(format!("outside-snap-{}.txt", std::process::id()));
    fs::write(&outside, "keep\n").unwrap();
    std::os::unix::fs::symlink(&outside, r.dir.join("fs/.snapshot.json.tmp")).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_sdc-watch")).arg("scan").arg(&r.dir).output().unwrap();
    let after = fs::read_to_string(&outside).unwrap();
    assert_eq!(after, "keep\n", "the snapshot was written into a file outside the run, through a link at its temporary name ({:?})", out.status);
}

const BLOB_TEXT: &str = "content a program of the run wrote\n";

/// A run whose watcher found the temporary name of the blob of `BLOB_TEXT` taken (`plant` takes it) when a file holding that text
/// was written; the watcher is stopped, and the run and the sha256 of the text are returned.
fn run_with_the_name_of_a_blob_taken(tag: &str, plant: &dyn Fn(&Path)) -> (Run, String) {
    let mut r = Run::new(tag);
    r.start();
    let pid = fs::read_to_string(r.dir.join("fs/sdc-watch.pid")).unwrap().trim().to_string();
    let probe = r.dir.join("probe.txt");
    fs::write(&probe, BLOB_TEXT).unwrap();
    let sum = Command::new("sha256sum").arg(&probe).output().unwrap();
    let sha = String::from_utf8(sum.stdout).unwrap().split_whitespace().next().unwrap().to_string();
    plant(&r.dir.join("fs/blobs").join(format!(".{sha}.{pid}")));
    fs::write(r.work().join("a.txt"), BLOB_TEXT).unwrap();
    r.pause();
    let _ = r.stop();
    (r, sha)
}

fn mkfifo(path: &Path) {
    assert!(Command::new("mkfifo").arg(path).status().unwrap().success());
}

#[test]
fn a_blob_whose_temporary_name_holds_a_link_a_program_put_there_is_stored_and_the_link_is_left_as_it_is() {
    let outside = base().join(format!("outside-stored-{}.txt", std::process::id()));
    fs::write(&outside, "keep\n").unwrap();
    let (r, sha) = run_with_the_name_of_a_blob_taken("blob-link-stored", &|tmp| std::os::unix::fs::symlink(&outside, tmp).unwrap());
    assert_eq!(fs::read_to_string(r.dir.join("fs/blobs").join(&sha)).unwrap(), BLOB_TEXT, "the blob was not stored");
    assert_eq!(fs::read_to_string(&outside).unwrap(), "keep\n");
    let left: Vec<String> = fs::read_dir(r.dir.join("fs/blobs")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).filter(|n| n.starts_with('.')).collect();
    assert_eq!(left.len(), 1, "the name a program took is the only temporary name left in the blob store: {left:?}");
    assert!(fs::symlink_metadata(r.dir.join("fs/blobs").join(&left[0])).unwrap().file_type().is_symlink(), "the name a program took was removed or replaced");
}

#[test]
fn a_blob_whose_temporary_name_holds_a_named_pipe_a_program_put_there_is_stored_and_the_watcher_goes_on() {
    let (r, sha) = run_with_the_name_of_a_blob_taken("blob-pipe-stored", &mkfifo);
    assert_eq!(fs::read_to_string(r.dir.join("fs/blobs").join(&sha)).unwrap(), BLOB_TEXT, "the blob was not stored");
}

#[test]
fn a_blob_whose_temporary_name_holds_a_second_name_of_a_file_outside_is_stored_and_that_file_is_not_written() {
    let outside = base().join(format!("outside-second-{}.txt", std::process::id()));
    fs::write(&outside, "keep\n").unwrap();
    let (r, sha) = run_with_the_name_of_a_blob_taken("blob-second-name", &|tmp| fs::hard_link(&outside, tmp).unwrap());
    assert_eq!(fs::read_to_string(r.dir.join("fs/blobs").join(&sha)).unwrap(), BLOB_TEXT, "the blob was not stored");
    assert_eq!(fs::read_to_string(&outside).unwrap(), "keep\n");
}

#[test]
fn a_blob_whose_temporary_name_holds_a_directory_is_stored_and_the_directory_is_left_as_it_is() {
    let (r, sha) = run_with_the_name_of_a_blob_taken("blob-directory", &|tmp| fs::create_dir(tmp).unwrap());
    assert_eq!(fs::read_to_string(r.dir.join("fs/blobs").join(&sha)).unwrap(), BLOB_TEXT, "the blob was not stored");
    let taken = fs::read_dir(r.dir.join("fs/blobs")).unwrap().map(|e| e.unwrap()).filter(|e| e.file_name().to_string_lossy().starts_with('.')).collect::<Vec<_>>();
    assert!(taken.len() == 1 && taken[0].path().is_dir(), "the directory at the temporary name was removed or replaced");
}

#[test]
fn a_snapshot_whose_temporary_name_holds_a_link_or_a_named_pipe_a_program_put_there_is_written() {
    for (tag, plant) in [("snapshot-link-written", false), ("snapshot-pipe-written", true)] {
        let r = Run::new(tag);
        fs::write(r.work().join("a.txt"), "a\n").unwrap();
        fs::create_dir_all(r.dir.join("fs")).unwrap();
        let outside = base().join(format!("outside-{tag}-{}.txt", std::process::id()));
        fs::write(&outside, "keep\n").unwrap();
        let tmp = r.dir.join("fs/.snapshot.json.tmp");
        if plant {
            mkfifo(&tmp);
        } else {
            std::os::unix::fs::symlink(&outside, &tmp).unwrap();
        }
        let out = Command::new(env!("CARGO_BIN_EXE_sdc-watch")).arg("scan").arg(&r.dir).output().unwrap();
        assert!(out.status.success(), "{tag}: {:?}", out.status);
        let snap: Value = serde_json::from_slice(&fs::read(r.dir.join("fs/snapshot.json")).unwrap()).unwrap();
        assert_eq!(snap["format"], "sdc.fs-snapshot/1", "{tag}");
        assert_eq!(fs::read_to_string(&outside).unwrap(), "keep\n", "{tag}");
        assert!(!tmp.exists(), "{tag}: the temporary name was left");
    }
}

// ---- what the watcher holds for the names of a call record and its blobs ----

#[test]
fn a_named_pipe_a_program_put_at_the_request_file_of_a_call_does_not_hold_the_watcher() {
    let mut r = Run::new("request-fifo");
    fs::create_dir_all(r.dir.join("llm")).unwrap();
    r.start();
    let rec = r.dir.join("llm/001-x.y");
    fs::create_dir_all(rec.join("cwd")).unwrap();
    mkfifo(&rec.join("request.json"));
    r.pause();
    fs::write(rec.join("cwd/a.txt"), "a\n").unwrap();
    r.pause();
    let mut child = r.child.take().unwrap();
    unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
    let t = Instant::now();
    let ended = loop {
        if child.try_wait().unwrap().is_some() {
            break true;
        }
        if t.elapsed() > Duration::from_secs(10) {
            break false;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    if !ended {
        let _ = child.kill();
        let _ = child.wait();
    }
    assert!(ended, "the watcher did not end within 10 s of SIGTERM: it reads the request file of a call by its path and waits there for a writer of the pipe in its place");
}

#[test]
fn every_blob_is_stored_while_a_program_keeps_taking_the_temporary_names_of_the_blobs() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    let mut r = Run::new("blob-names-taken");
    r.start();
    let pid = fs::read_to_string(r.dir.join("fs/sdc-watch.pid")).unwrap().trim().to_string();
    let texts: Vec<String> = (0..24).map(|i| format!("content {i} of a file of the run\n")).collect();
    let probes = base().join(format!("probes-{}", std::process::id()));
    fs::create_dir_all(&probes).unwrap();
    let mut files = Vec::new();
    for (i, text) in texts.iter().enumerate() {
        let f = probes.join(format!("p{i}.txt"));
        fs::write(&f, text).unwrap();
        files.push(f);
    }
    let sums = Command::new("sha256sum").args(&files).output().unwrap();
    let shas: Vec<String> = String::from_utf8(sums.stdout).unwrap().lines().map(|l| l.split_whitespace().next().unwrap().to_string()).collect();
    assert_eq!(shas.len(), texts.len());
    let outside = base().join(format!("outside-{}.txt", std::process::id()));
    fs::write(&outside, "keep\n").unwrap();
    let names: Vec<PathBuf> = shas.iter().map(|sha| r.dir.join("fs/blobs").join(format!(".{sha}.{pid}"))).collect();
    let stop = Arc::new(AtomicBool::new(false));
    let spinner = {
        let (stop, outside) = (stop.clone(), outside.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                for name in &names {
                    let _ = std::os::unix::fs::symlink(&outside, name);
                }
            }
        })
    };
    for (i, text) in texts.iter().enumerate() {
        fs::write(r.work().join(format!("f{i}.txt")), text).unwrap();
    }
    r.pause();
    stop.store(true, Ordering::Relaxed);
    spinner.join().unwrap();
    let _ = r.stop();
    let missing: Vec<usize> = shas.iter().enumerate().filter(|(_, sha)| !r.dir.join("fs/blobs").join(sha).exists()).map(|(i, _)| i).collect();
    let _ = fs::remove_dir_all(&probes);
    assert_eq!(fs::read_to_string(&outside).unwrap(), "keep\n");
    assert!(missing.is_empty(), "{} of {} blobs were not stored: the watcher lost the record of these contents without a message", missing.len(), texts.len());
}

#[test]
fn a_link_a_program_put_at_the_request_file_of_a_call_is_not_read_for_its_case() {
    let outside = base().join(format!("outside-request-{}.json", std::process::id()));
    fs::write(&outside, r#"{"node":"x.y","case":"C","case_dir":"/somewhere/store/C"}"#).unwrap();
    let mut r = Run::new("request-link");
    fs::create_dir_all(r.dir.join("llm")).unwrap();
    r.start();
    r.append("store/C/events.jsonl", r#"{"seq":1,"event":"action_started","action":"x.y","at":"x"}"#);
    r.append("store/C/children/C.k/events.jsonl", r#"{"seq":1,"event":"action_started","action":"x.y","at":"x"}"#);
    r.pause();
    let rec = r.dir.join("llm/001-x.y");
    fs::create_dir_all(rec.join("cwd")).unwrap();
    r.pause();
    std::os::unix::fs::symlink(&outside, rec.join("request.json")).unwrap();
    fs::write(rec.join("cwd/a.txt"), "a\n").unwrap();
    r.pause();
    let c = r.stop();
    assert_eq!(ops(&c), vec![op("create", "a.txt")]);
    assert_eq!(c[0]["attribution"]["case"].as_str(), Some("C.k"), "the case was taken from a file outside the run, through a link at request.json");
}

#[test]
fn a_link_a_program_put_at_the_request_file_of_a_decider_call_is_not_read_for_its_case() {
    let mut r = Run::new("decide-request-link");
    let outside = base().join(format!("outside-decide-request-{}.json", std::process::id()));
    fs::write(&outside, format!(r#"{{"case_dir":"{}/store/C/children/C.k","action":"roadmap.approve"}}"#, r.dir.display())).unwrap();
    r.start();
    let rec = r.dir.join("decider/001-roadmap.approve");
    fs::create_dir_all(rec.join("cwd")).unwrap();
    std::os::unix::fs::symlink(&outside, rec.join("decide-request.json")).unwrap();
    r.pause();
    fs::write(rec.join("cwd/answer.json"), "{}\n").unwrap();
    r.pause();
    let c = r.stop();
    assert_eq!(ops(&c), vec![op("create", "answer.json")]);
    assert!(c[0]["attribution"]["case"].is_null(), "the case was taken from a file outside the run, through a link at decide-request.json: {}", c[0]["attribution"]);
}

fn blob_name(r: &Run, text: &str) -> String {
    let probe = r.dir.join("blob-name-probe.txt");
    fs::write(&probe, text).unwrap();
    let sum = Command::new("sha256sum").arg(&probe).output().unwrap();
    let sha = String::from_utf8(sum.stdout).unwrap().split_whitespace().next().unwrap().to_string();
    fs::remove_file(&probe).unwrap();
    sha
}

fn ends_within_10s_of_sigterm(r: &mut Run) -> bool {
    let mut child = r.child.take().unwrap();
    unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
    let t = Instant::now();
    let ended = loop {
        if child.try_wait().unwrap().is_some() {
            break true;
        }
        if t.elapsed() > Duration::from_secs(10) {
            break false;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    if !ended {
        let _ = child.kill();
        let _ = child.wait();
    }
    ended
}

#[test]
fn a_named_pipe_a_program_put_at_the_name_of_a_stored_blob_does_not_hold_the_watcher() {
    let mut r = Run::new("blob-fifo");
    r.start();
    let first = "first text of a file of the run\n";
    mkfifo(&r.dir.join("fs/blobs").join(blob_name(&r, first)));
    fs::write(r.work().join("a.txt"), first).unwrap();
    r.pause();
    fs::write(r.work().join("a.txt"), "second text of a file of the run\n").unwrap();
    r.pause();
    assert!(ends_within_10s_of_sigterm(&mut r), "a stored blob is read as a regular file and no pipe is waited for: the watcher ends within 10 s of SIGTERM");
}

#[test]
fn a_link_a_program_put_at_the_name_of_a_stored_blob_is_not_read_into_a_diff() {
    let mut r = Run::new("blob-name-link");
    r.start();
    let first = "first text of a file of the run\n";
    let outside = r.dir.parent().unwrap().join(format!("outside-blob-{}.txt", std::process::id()));
    fs::write(&outside, "text of a file outside the run\n").unwrap();
    std::os::unix::fs::symlink(&outside, r.dir.join("fs/blobs").join(blob_name(&r, first))).unwrap();
    fs::write(r.work().join("a.txt"), first).unwrap();
    r.pause();
    fs::write(r.work().join("a.txt"), "second text of a file of the run\n").unwrap();
    r.pause();
    let c = r.stop();
    let _ = fs::remove_file(&outside);
    let diffs: Vec<String> = c.iter().filter_map(|v| v["diff"].as_str().map(String::from)).collect();
    assert!(diffs.iter().all(|d| !d.contains("outside the run")), "a stored blob is read without following a link at its name: no text from outside the run is in a diff: {diffs:?}");
}

#[test]
fn the_watcher_writes_nothing_below_a_link_that_replaced_its_output_directory() {
    let mut r = Run::new("out-link");
    r.start();
    let outside = r.dir.parent().unwrap().join(format!("outside-out-{}", std::process::id()));
    fs::create_dir_all(outside.join("blobs")).unwrap();
    fs::write(outside.join("sdc-watch.pid"), "keep\n").unwrap();
    fs::rename(r.dir.join("fs"), r.dir.join("fs.moved")).unwrap();
    std::os::unix::fs::symlink(&outside, r.dir.join("fs")).unwrap();
    fs::write(r.work().join("a.txt"), "text of a file of the run\n").unwrap();
    r.pause();
    let mut child = r.child.take().unwrap();
    unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
    let _ = child.wait();
    let mut names: Vec<String> = fs::read_dir(&outside).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    let blobs: Vec<String> = fs::read_dir(outside.join("blobs")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    let kept = fs::read_to_string(outside.join("sdc-watch.pid")).ok();
    let _ = fs::remove_dir_all(&outside);
    names.sort();
    assert!(blobs.is_empty(), "the watcher stored blobs below a link that replaced its output directory: {blobs:?}");
    assert_eq!(names, vec!["blobs".to_owned(), "sdc-watch.pid".to_owned()], "the watcher made files below a link that replaced its output directory");
    assert_eq!(kept.as_deref(), Some("keep\n"), "the watcher removed or wrote a file below a link that replaced its output directory");
}

#[test]
fn a_file_of_the_run_that_takes_hours_to_read_does_not_hold_the_watcher_past_its_end() {
    let mut r = Run::new("huge-file");
    r.start();
    let big = fs::File::create(r.work().join("big.bin")).unwrap();
    big.set_len(1 << 40).unwrap();
    drop(big);
    r.pause();
    r.pause();
    assert!(ends_within_10s_of_sigterm(&mut r), "a file of the run is read within its bound: the watcher ends within 10 s of SIGTERM");
}

#[test]
fn the_pid_record_is_read_as_a_regular_file_and_a_named_pipe_there_does_not_hold_a_starting_watcher() {
    let mut r = Run::new("pid-fifo");
    fs::create_dir_all(r.dir.join("fs/blobs")).unwrap();
    let pid = r.dir.join("fs/sdc-watch.pid");
    mkfifo(&pid);
    r.child = Some(Command::new(env!("CARGO_BIN_EXE_sdc-watch")).arg("run").arg(&r.dir).spawn().unwrap());
    let t = Instant::now();
    while !fs::symlink_metadata(&pid).is_ok_and(|m| m.is_file()) && t.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let written = fs::symlink_metadata(&pid).is_ok_and(|m| m.is_file());
    let ended = ends_within_10s_of_sigterm(&mut r);
    assert!(written && ended, "the pid record is read as a regular file and no pipe is waited for: the watcher writes its pid record ({written}) and ends within 10 s of SIGTERM ({ended})");
}

#[test]
fn a_named_pipe_a_program_put_at_the_record_of_a_run_does_not_hold_a_restarted_watcher() {
    let mut hung = Vec::new();
    for name in ["changes.jsonl", "snapshot.json"] {
        let r = Run::new("record-fifo");
        fs::create_dir_all(r.dir.join("fs/blobs")).unwrap();
        mkfifo(&r.dir.join("fs").join(name));
        let mut child = Command::new(env!("CARGO_BIN_EXE_sdc-watch")).arg("scan").arg(&r.dir).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().unwrap();
        let t = Instant::now();
        let ended = loop {
            if child.try_wait().unwrap().is_some() {
                break true;
            }
            if t.elapsed() > Duration::from_secs(10) {
                break false;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        if !ended {
            let _ = child.kill();
            let _ = child.wait();
        }
        if !ended {
            hung.push(name);
        }
    }
    assert!(hung.is_empty(), "{hung:?}: the records of an earlier session are read as regular files and no pipe is waited for: the watcher ends within 10 s");
}
