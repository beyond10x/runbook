//! `sdc-conform-adapter`: a capture is read "within `timeout_s` of the start, or until the product
//! closes the stream" (`driver/src/bin/sdc-conform-adapter.rs`). Each case drives a bare adapter
//! with one `begin_scenario` and a product that is a small shell script.

use std::fs;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value as J};

use super::ess_pin_guard::{executable, stub, table, tmp};

const ADAPTER: &str = env!("CARGO_BIN_EXE_sdc-conform-adapter");

// ---- the capture ends when the product closed the stream ---------------------------------------

/// One `begin_scenario` sent to a bare `sdc-conform-adapter` whose product is the shell script
/// `product` (started as `bin/product.sh <port> <data dir>`, the stub product beside it as
/// `bin/stub-product`) and whose table captures `token` from stdout within `timeout_s`: the reply
/// and how long it took.
fn begin_scenario(name: &str, product: &str, timeout_s: u64) -> (J, Duration) {
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
    (serde_json::from_str(&line).unwrap_or_else(|e| panic!("reply {line:?}: {e}")), took)
}

fn detail(reply: &J) -> String {
    reply["target_error"]["detail"].as_str().unwrap_or("").to_string()
}

/// A product that closed stdout and keeps stderr open is missing the stdout capture at once.
#[test]
fn a_product_that_closed_only_stdout_is_missing_its_stdout_capture_at_once() {
    let (reply, took) = begin_scenario("closed-stdout-only", "exec 1>&-\nexec sleep 30", 20);
    assert_eq!(reply["target_error"]["kind"], "unavailable", "{reply:#}");
    assert!(detail(&reply).contains("closed stdout"), "{reply:#}");
    assert!(took < Duration::from_secs(10), "answered after {took:?}");
}

/// The end of stderr does not end a capture read from stdout: the value printed a second later is
/// still captured.
#[test]
fn a_product_that_closed_only_stderr_still_has_its_stdout_capture_read() {
    let (reply, took) = begin_scenario("closed-stderr-only", "exec 2>&-\nsleep 1\necho 'board token: late'\nexec bin/stub-product \"$1\" \"$2\" token-quiet", 10);
    assert!(reply["target_error"].is_null(), "after {took:?}: {reply:#}");
}

/// The product closed its own stdout, and a process it started kept the stream and prints the
/// value a second later: the stream is open, so the capture is waited for and read.
#[test]
fn a_capture_printed_by_a_child_that_kept_the_stream_is_still_read() {
    let (reply, took) = begin_scenario("child-keeps-stdout", "(sleep 1; echo 'board token: late') &\nexec 1>&-\nexec bin/stub-product \"$1\" \"$2\" token-quiet", 10);
    assert!(reply["target_error"].is_null(), "after {took:?}: {reply:#}");
    assert!(took >= Duration::from_millis(900), "answered after {took:?}, before the child printed");
}

/// The value printed and the stream closed in the same instant: the value is captured, eight
/// times of eight.
#[test]
fn a_capture_printed_as_the_stream_closes_is_read() {
    for round in 0..8 {
        let (reply, took) = begin_scenario("print-and-close", "echo 'board token: now'\nexec 1>&- 2>&-\nexec bin/stub-product \"$1\" \"$2\" token-quiet", 10);
        assert!(reply["target_error"].is_null(), "round {round}, after {took:?}: {reply:#}");
    }
}

/// A product that exits without printing the value is reported as one that did not start, never
/// as a capture on a closed stream and never with a value: the end of its streams comes a moment
/// before it can be waited for, and under load that moment is long. 20 rounds.
#[test]
fn a_product_that_exits_is_reported_as_not_started_and_never_with_a_capture_value() {
    for round in 0..20 {
        let (reply, took) = begin_scenario("exits", "echo 'refusing to start' >&2\nexit 3", 20);
        assert_eq!(reply["target_error"]["kind"], "unavailable", "round {round}, after {took:?}: {reply:#}");
        assert!(reply["ok"].is_null(), "a figure was reported for a product that never started: round {round}: {reply:#}");
        let d = detail(&reply);
        assert!(d.contains("the product did not start") && !d.contains("closed stdout"), "round {round}, after {took:?}: {d:?}");
    }
}

/// The same with the moment made long: the streams end and the product is waited for only 100 ms
/// later (a loaded host does this by itself). The product has exited by the time it is asked
/// for: it did not start.
#[test]
fn a_product_whose_streams_end_a_moment_before_it_exits_is_reported_as_not_started() {
    let (reply, took) = begin_scenario("exits-late", "exec 1>&- 2>&-\nsleep 0.1\nexit 3", 20);
    assert_eq!(reply["target_error"]["kind"], "unavailable", "after {took:?}: {reply:#}");
    let d = detail(&reply);
    assert!(d.contains("the product did not start") && !d.contains("closed stdout"), "after {took:?}: {d:?}");
}

