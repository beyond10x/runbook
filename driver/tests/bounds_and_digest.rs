//! Bounds of `sdc-conform` and `sdc-conform-adapter`: a build step over its bound is gone when the
//! observation ended; the digest that stands for the pin of `ess` covers every byte of the file
//! (a short read does not end it, a failed read is an error and never the digest of a prefix); the
//! adapter does not say that a product closed a stream without printing a value the reader dropped.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value as J};

use super::ess_pin_guard::{executable, installed_ess, stub, table, tmp};

const BIN: &str = env!("CARGO_BIN_EXE_sdc-conform");
const ADAPTER: &str = env!("CARGO_BIN_EXE_sdc-conform-adapter");

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/conformance")
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().flatten() {
        let d = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy(&e.path(), &d)
        } else {
            fs::copy(e.path(), &d).unwrap();
        }
    }
}

fn alive(pid: &str) -> bool {
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(s) => s.rsplit(')').next().is_some_and(|rest| !rest.trim_start().starts_with('Z')),
        Err(_) => false,
    }
}

/// Whether the process is gone within `limit`; a process still there is ended by its PID (it is a
/// process of this test).
fn gone_within(pid: &str, limit: Duration) -> bool {
    let end = Instant::now() + limit;
    while alive(pid) {
        if Instant::now() > end {
            let _ = Command::new("kill").arg(pid).status();
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    true
}

/// One `sdc-conform observe` of a stub product whose build step is the shell script `build`, with
/// `SDC_CONFORMANCE_BUILD_TIMEOUT=1`. The script writes one PID to the file `$PID_FILE` names.
/// Answers the PID after asserting that the observation recorded the bound.
fn observe_with_a_build_step_over_its_bound(name: &str, build: &str) -> String {
    let pin: J = serde_json::from_str(&fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("ess-pin.json")).unwrap()).unwrap();
    let ess = installed_ess(&pin);
    assert!(ess.is_file(), "the pinned ess is not installed (run `sdc-conform ess-install` or set ESS_BIN): {}", ess.display());

    let dir = tmp(name);
    let run = dir.join("run");
    let work = run.join("store/IDEA-1/work");
    copy(&fixtures().join("spec"), &work.join("spec"));
    fs::create_dir_all(work.join("published/bin")).unwrap();
    fs::copy(stub(), work.join("published/bin/stub-product")).unwrap();
    fs::write(work.join("published/README.md"), "Stub chores service.\nRun: bin/stub-product <port> <data dir> <ok>\n").unwrap();
    fs::write(work.join("published/PROVENANCE"), "stub\n").unwrap();
    let pid_file = dir.join("started.pid");
    executable(&work.join("published/bin/build.sh"), &format!("#!/bin/sh\nPID_FILE='{}'\n{build}\n", pid_file.display()));
    let mut t = table();
    t["build"] = json!({"argv": ["bin/build.sh"]});
    let table_file = dir.join("t.json");
    fs::write(&table_file, t.to_string()).unwrap();

    let started = Instant::now();
    let o = Command::new(BIN)
        .arg("observe")
        .arg(&run)
        .env("SDC_CONFORMANCE_ADAPTER", stub())
        .env("SDC_STUB_TABLE_NODE", &table_file)
        .env("SDC_CONFORMANCE_TARGET_TIMEOUT", "10")
        .env("SDC_CONFORMANCE_TABLES", "1")
        .env("SDC_CONFORMANCE_BUILD_TIMEOUT", "1")
        .env_remove("SDC_LLM_MODEL_CONFORMANCE")
        .env_remove("SDC_LLM_EFFORT_CONFORMANCE")
        .env_remove("SDC_LLM_EFFORT_OTHER")
        .env_remove("SDC_LLM_EFFORT")
        .output()
        .unwrap();
    let took = started.elapsed();
    let pid = fs::read_to_string(&pid_file).unwrap_or_default().trim().to_string();
    assert!(!pid.is_empty(), "the build step did not run: {}", String::from_utf8_lossy(&o.stderr));
    let recorded = (|| {
        let oracle: J = serde_json::from_str(&fs::read_to_string(run.join("oracle.json")).ok()?).ok()?;
        let c = &oracle["conformance"];
        (o.status.success() && c["cause"] == "product_build_failed" && c["why"].as_str()?.contains("killed after 1 s") && took < Duration::from_secs(20)).then_some(())
    })();
    if recorded.is_none() {
        gone_within(&pid, Duration::ZERO);
        panic!("the observation did not record a build step ended at its bound of 1 s (took {took:?}): {}", String::from_utf8_lossy(&o.stderr));
    }
    pid
}

/// A build step over its bound is ended at the bound: the process of the build step itself is gone
/// when the observation that reports "killed after 1 s" has ended. (The unit test of the bounded
/// command, `conform::tests::a_child_over_its_bound_is_ended_at_the_bound`, reads the answer and
/// the time and not whether the child is gone.)
#[test]
fn a_build_step_over_its_bound_is_gone_when_the_observation_ended() {
    let pid = observe_with_a_build_step_over_its_bound("p3-build-ended", "echo $$ > \"$PID_FILE\"\nexec sleep 50");
    assert!(gone_within(&pid, Duration::from_secs(3)), "the build step (process {pid}) still ran 3 s after the observation ended with \"killed after 1 s\"");
}

/// The digest that stands for the pin is the digest of every byte of the file, or an error: a read
/// that fails after the file was opened never yields the digest of what was read until then. A
/// directory opens for reading on Linux and fails at the first read.
#[test]
fn a_read_that_fails_after_the_open_is_an_error_and_never_the_digest_of_a_prefix() {
    let dir = tmp("p3-digest-read-error");
    let got = sdc_tools::conformance::sha256_file(&dir);
    assert!(got.is_err(), "the digest of a path whose read fails: {got:?}");
}

/// A read that returns fewer bytes than the buffer holds is not the end of the file: the digest
/// covers what follows it. A named pipe gives such a read: the writer sends two parts with a pause
/// between them.
#[test]
fn a_short_read_does_not_end_the_digest() {
    use sha2::{Digest, Sha256};
    use std::io::Write;
    let dir = tmp("p3-digest-short-read");
    let pipe = dir.join("pipe");
    assert!(Command::new("mkfifo").arg(&pipe).status().unwrap().success());
    let (first, second) = (vec![b'a'; 1000], vec![b'b'; 70_000]);
    let writer = {
        let (pipe, first, second) = (pipe.clone(), first.clone(), second.clone());
        std::thread::spawn(move || {
            let mut f = fs::OpenOptions::new().write(true).open(pipe).unwrap();
            f.write_all(&first).unwrap();
            f.flush().unwrap();
            std::thread::sleep(Duration::from_millis(300));
            // a reader that stopped early closed the pipe: the comparison below reports it
            let _ = f.write_all(&second);
        })
    };
    let got = sdc_tools::conformance::sha256_file(&pipe).unwrap();
    let _ = writer.join();
    let whole: String = Sha256::digest([first, second].concat()).iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(got, whole, "the digest is not the digest of all 71,000 bytes");
}

/// One `begin_scenario` sent to a bare adapter whose product is the shell script `product` and
/// whose table captures `token` from stdout within `timeout_s`: the reply, how long it took and
/// the product's log.
fn begin_scenario(name: &str, product: &str, timeout_s: u64) -> (J, Duration, String) {
    use std::io::{BufRead, BufReader, Write};
    let dir = tmp(name);
    let tree = dir.join("tree");
    fs::create_dir_all(tree.join("bin")).unwrap();
    fs::copy(stub(), tree.join("bin/stub-product")).unwrap();
    executable(&tree.join("bin/product.sh"), &format!("#!/bin/sh\n{product}\n"));
    let mut t = table();
    t["start"]["argv"] = json!(["bin/product.sh", "{port}", "{data_dir}"]);
    t["captures"] = json!([{"name": "token", "stream": "stdout", "regex": "board token: (\\S+)", "timeout_s": timeout_s}]);
    let table_file = dir.join("t.json");
    fs::write(&table_file, t.to_string()).unwrap();
    let mut child = Command::new(ADAPTER)
        .arg("--table")
        .arg(&table_file)
        .arg("--tree")
        .arg(&tree)
        .arg("--work")
        .arg(dir.join("adapter-work"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let started = Instant::now();
    let mut stdin = child.stdin.take().unwrap();
    writeln!(stdin, "{}", json!({"id": 1, "op": "begin_scenario", "request": {"scenario": "x"}})).unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let took = started.elapsed();
    drop(stdin);
    child.wait().unwrap();
    let log = fs::read_to_string(dir.join("adapter-work/0001/product.log")).unwrap_or_default();
    (serde_json::from_str(&line).unwrap_or_else(|e| panic!("reply {line:?}: {e}")), took, log)
}

fn detail(reply: &J) -> String {
    reply["target_error"]["detail"].as_str().unwrap_or("").to_string()
}

/// 12,000 lines of 100 characters, 1.2 MB, as a product with a long start-up log prints them.
const NOISE: &str = "head -c 1200000 /dev/zero | tr '\\0' x | fold -w 100";

/// A product prints 1.2 MB of lines, then the value, then closes stdout and serves. Whatever else the adapter answers,
/// it does not say that the product closed stdout without printing a value that is in its log.
#[test]
fn a_product_that_printed_the_value_is_not_said_to_have_closed_the_stream_without_printing_it() {
    let (reply, took, log) =
        begin_scenario("p3-value-after-noise-closed", &format!("{NOISE}\necho 'board token: abc'\nexec 1>&-\nexec bin/stub-product \"$1\" \"$2\" token-quiet"), 5);
    assert!(log.contains("board token: abc"), "the product did not print the value: the case measured nothing");
    let d = detail(&reply);
    assert!(!d.contains("without printing it"), "after {took:?}: {d:?}");
}
