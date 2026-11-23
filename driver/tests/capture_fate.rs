
//! What `sdc-conform-adapter` says about a product whose output stream ended. The label decides the
//! cause `sdc-conform` records: an event `start` makes the run `product_not_started`, an event
//! `capture` makes it `capture_missing`, the table's fault, and the table node is asked again
//! (`driver/src/conform.rs`, `table_evidence` and the lines before its call).

use std::fs;
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
/// not start. Before this unit the adapter said so for any exit within `timeout_s` (the base
/// commit answers "the product did not start: the product exited before it answered (exit status:
/// 1)" after 1.1 s). With the wait of 250 ms after the end of the stream, an exit 251 ms or later
/// is reported as a capture the table got wrong.
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

/// A capture is read "within `timeout_s` of the start, or until the product closes the stream". A
/// product that prints one line that is not UTF-8 (a Latin-1 banner) and then the value has not
/// closed the stream and has printed the value: it is captured. The adapter's reader ends at the
/// first such line (`lines().map_while(Result::ok)`); the base commit then waited `timeout_s` and
/// said the capture did not appear, and this head says at once that "the product closed stdout
/// without printing it", which is false on both counts.
#[test]
fn a_capture_printed_after_a_line_that_is_not_utf8_is_read() {
    let (reply, took) = begin_scenario("p2-not-utf8", "printf 'caf\\351 open\\n'\necho 'board token: abc'\nexec bin/stub-product \"$1\" \"$2\" token-quiet", 5);
    assert!(reply["target_error"].is_null(), "after {took:?}: {}", detail(&reply));
}
