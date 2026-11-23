
//! What `sdc-conform-adapter` says about a product whose output stream ended. The label decides the
//! cause `sdc-conform` records: an event `start` makes the run `product_not_started`, an event
//! `capture` makes it `capture_missing`, the table's fault, and the table node is asked again
//! (`driver/src/conform.rs`, `table_evidence` and the lines before its call).

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value as J};

use super::ess_pin_guard::{executable, stub, table, tmp};

const ADAPTER: &str = env!("CARGO_BIN_EXE_sdc-conform-adapter");

/// One `begin_scenario` sent to a bare adapter whose product is the shell script `product`
/// (`bin/product.sh <port> <data dir>`) and whose table captures `token` from stdout within
/// `timeout_s`: the reply and how long it took.
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

/// A product that sends its output to a file of its own and ends a second later with an error did
/// not start: the adapter says so for any exit within `timeout_s` ("the product did not start: the
/// product exited before it answered (exit status: 1)"), as the base commit did after 1.1 s. A
/// fixed wait after the end of the stream would call an exit that comes later a capture the table
/// got wrong.
#[test]
fn a_product_that_redirects_its_output_and_ends_a_second_later_did_not_start() {
    let (reply, took) = begin_scenario("p2-redirect-then-exit", "exec > \"$2/server.log\" 2>&1\nsleep 1\necho 'cannot bind'\nexit 1", 20);
    assert_eq!(reply["target_error"]["kind"], "unavailable", "after {took:?}: {reply:#}");
    let d = detail(&reply);
    assert!(d.contains("the product did not start"), "after {took:?} the adapter blames the table's capture for a product that ends with exit status 1: {d:?}");
}

/// The same with the streams closed and the exit 300 ms later: 50 ms past the bound.
#[test]
fn a_product_that_exits_300_ms_after_its_streams_ended_did_not_start() {
    let (reply, took) = begin_scenario("p2-exit-300ms", "exec 1>&- 2>&-\nsleep 0.3\nexit 3", 20);
    assert_eq!(reply["target_error"]["kind"], "unavailable", "after {took:?}: {reply:#}");
    let d = detail(&reply);
    assert!(d.contains("the product did not start"), "after {took:?}: {d:?}");
}

/// A capture is read within `timeout_s` of the start. A product that prints one line that is not
/// UTF-8 (a Latin-1 banner) and then the value has not closed the stream and has printed the value:
/// it is captured. A reader that ends at the first such line misses the value.
#[test]
fn a_capture_printed_after_a_line_that_is_not_utf8_is_read() {
    let (reply, took) = begin_scenario("p2-not-utf8", "printf 'caf\\351 open\\n'\necho 'board token: abc'\nexec bin/stub-product \"$1\" \"$2\" token-quiet", 5);
    assert!(reply["target_error"].is_null(), "after {took:?}: {}", detail(&reply));
}

fn expect_did_not_start(name: &str, product: &str) {
    let (reply, took) = begin_scenario(name, product, 20);
    assert_eq!(reply["target_error"]["kind"], "unavailable", "after {took:?}: {reply:#}");
    let d = detail(&reply);
    assert!(d.contains("the product did not start"), "after {took:?} a product that ended is not a capture the table got wrong: {d:?}");
    assert!(took < Duration::from_secs(10), "an exited product is answered when it exited, not at the capture's bound: {took:?}");
}

/// A product that exits before it prints anything.
#[test]
fn a_product_that_exits_at_once_did_not_start() {
    expect_did_not_start("p2-exit-at-once", "exit 3");
}

/// A product that closes its streams and exits 100 ms, 200 ms, 500 ms later: whatever the delay,
/// the product ended, it did not serve.
#[test]
fn a_product_that_exits_after_its_streams_ended_did_not_start() {
    for ms in [100, 200, 500] {
        expect_did_not_start(&format!("p2-exit-{ms}ms"), &format!("exec 1>&- 2>&-\nsleep 0.{ms:03}\nexit 3"));
    }
}

/// A product that ends by a signal 100 ms after its streams ended, or by SIGKILL at once.
#[test]
fn a_product_that_ends_by_a_signal_did_not_start() {
    expect_did_not_start("p2-term-100ms", "exec 1>&- 2>&-\nsleep 0.1\nkill -TERM $$\nsleep 5");
    expect_did_not_start("p2-kill-at-once", "kill -KILL $$\nsleep 5");
}

/// A product that closed stdout and serves answers `start.ready`: the table's capture is what is
/// wrong, and the adapter says so with the capture's own message, not that the product did not start.
#[test]
fn a_product_that_closed_stdout_and_serves_is_a_capture_the_table_got_wrong() {
    let (reply, took) = begin_scenario("p2-closed-serving", "exec bin/stub-product \"$1\" \"$2\" token-silent", 20);
    assert_eq!(reply["target_error"]["kind"], "unavailable", "after {took:?}: {reply:#}");
    let d = detail(&reply);
    assert!(d.contains("capture token did not appear on stdout"), "after {took:?}: {d:?}");
    assert!(d.contains("closed stdout without printing it"), "after {took:?}: {d:?}");
    assert!(!d.contains("the product did not start"), "{d:?}");
    assert!(took < Duration::from_secs(10), "{took:?}");
}

fn alive(pid: &str) -> bool {
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(s) => s.rsplit(')').next().is_some_and(|rest| !rest.trim_start().starts_with('Z')),
        Err(_) => false,
    }
}

fn gone_within(pid: &str, limit: Duration) -> bool {
    let end = Instant::now() + limit;
    while alive(pid) {
        if Instant::now() > end {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    true
}

fn grandchild(name: &str) -> String {
    let file = Path::new(env!("CARGO_TARGET_TMPDIR")).join("ess-pin-guard").join(name).join("tree/gc.pid");
    fs::read_to_string(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display())).trim().to_string()
}

/// A product that closed stdout and neither serves nor exits is bounded by the capture's own
/// `timeout_s` (1 s here), with the message of a capture that did not appear in time. The process
/// it left behind does not outlive the adapter.
#[test]
fn a_product_that_closed_stdout_and_neither_serves_nor_exits_is_bounded_by_timeout_s() {
    let (reply, took) = begin_scenario("p2-closed-silent", "exec 1>&-\nsleep 60 &\necho $! > gc.pid\nwait", 1);
    assert_eq!(reply["target_error"]["kind"], "unavailable", "after {took:?}: {reply:#}");
    let d = detail(&reply);
    assert!(d.contains("capture token did not appear on stdout within 1 s"), "after {took:?}: {d:?}");
    assert!(took >= Duration::from_millis(900) && took < Duration::from_secs(8), "{took:?}");
    assert!(gone_within(&grandchild("p2-closed-silent"), Duration::from_secs(5)), "the product's child outlived the adapter");
}

/// A product that ended while a child it started holds its streams open: the streams do not end,
/// the product did; it did not start, and the child does not outlive the adapter.
#[test]
fn a_product_that_ended_while_a_child_holds_its_streams_did_not_start() {
    let name = "p2-grandchild";
    let (reply, took) = begin_scenario(name, "sleep 60 &\necho $! > gc.pid\nexit 2", 20);
    assert_eq!(reply["target_error"]["kind"], "unavailable", "after {took:?}: {reply:#}");
    let d = detail(&reply);
    assert!(d.contains("the product did not start"), "after {took:?}: {d:?}");
    assert!(took < Duration::from_secs(10), "{took:?}");
    assert!(gone_within(&grandchild(name), Duration::from_secs(5)), "the product's child outlived the adapter");
}
