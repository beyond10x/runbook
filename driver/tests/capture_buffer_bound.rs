//! The bound of the buffer the captures are read from (`CAPTURE_BUFFER`, 1 MiB, in
//! `src/bin/sdc-conform-adapter.rs`), to the byte. The adapter says "the product closed stdout
//! without printing it" only of a stream its buffer took whole; of a stream whose lines it left out
//! it names the part it read.

use std::fs;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value as J};

use super::ess_pin_guard::{executable, stub, table, tmp};

const ADAPTER: &str = env!("CARGO_BIN_EXE_sdc-conform-adapter");

/// The bound of the buffer, as `CAPTURE_BUFFER` has it.
const BOUND: usize = 1 << 20;

/// The line that carries the value, 17 bytes with its newline.
const VALUE: &str = "board token: abc\n";

/// `total` bytes of lines: 63 characters and a newline each, the last line shorter.
fn lines(total: usize) -> String {
    let mut s = String::with_capacity(total);
    while total - s.len() >= 64 {
        s.push_str(&"x".repeat(63));
        s.push('\n');
    }
    let rest = total - s.len();
    if rest > 0 {
        s.push_str(&"x".repeat(rest - 1));
        s.push('\n');
    }
    assert_eq!(s.len(), total);
    s
}

/// One `begin_scenario` sent to a bare adapter whose product prints `output` on stdout, then runs
/// the shell lines `then`, and whose table captures `token` from stdout within `timeout_s`: the
/// reply and how long it took. The product's log must hold every line of `output`.
fn begin_scenario(name: &str, output: &str, then: &str, timeout_s: u64) -> (J, Duration) {
    use std::io::{BufRead, BufReader, Write};
    let dir = tmp(name);
    let tree = dir.join("tree");
    fs::create_dir_all(tree.join("bin")).unwrap();
    fs::copy(stub(), tree.join("bin/stub-product")).unwrap();
    fs::write(tree.join("output.txt"), output).unwrap();
    executable(&tree.join("bin/product.sh"), &format!("#!/bin/sh\ncat output.txt\n{then}\n"));
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
    assert_eq!(log.len(), output.len(), "the product's log does not hold what the product printed: the case measured nothing");
    (serde_json::from_str(&line).unwrap_or_else(|e| panic!("reply {line:?}: {e}")), took)
}

fn detail(reply: &J) -> String {
    reply["target_error"]["detail"].as_str().unwrap_or("").to_string()
}

/// The product closes stdout and serves.
const CLOSES_AND_SERVES: &str = "exec 1>&-\nexec bin/stub-product \"$1\" \"$2\" token-quiet";

/// A stream of 1 MiB to the byte whose last line is the value: the buffer takes every line, and
/// the value is captured.
#[test]
fn the_value_as_the_last_line_of_a_stream_of_one_mebibyte_is_captured() {
    let output = lines(BOUND - VALUE.len()) + VALUE;
    assert_eq!(output.len(), BOUND);
    let (reply, took) = begin_scenario("p4-last-line-at-the-bound", &output, CLOSES_AND_SERVES, 5);
    assert!(reply["target_error"].is_null(), "after {took:?}: {}", detail(&reply));
}

/// The value's line starts 5 bytes before the bound and ends 12 bytes after it: the buffer takes a
/// line it has room to start, and the value is captured.
#[test]
fn a_value_whose_line_crosses_the_bound_is_captured() {
    let output = lines(BOUND - 5) + VALUE;
    let (reply, took) = begin_scenario("p4-value-across-the-bound", &output, CLOSES_AND_SERVES, 5);
    assert!(reply["target_error"].is_null(), "after {took:?}: {}", detail(&reply));
}

/// 1 MiB of lines to the byte, then the value, then stdout closed: the buffer is full when the
/// value's line comes and leaves it out. The adapter names the part it read and its bound, and
/// does not say that the product closed stdout without printing the value.
#[test]
fn a_value_one_line_past_the_bound_is_missing_in_the_part_read_and_not_unprinted() {
    let output = lines(BOUND) + VALUE;
    let (reply, took) = begin_scenario("p4-value-past-the-bound", &output, CLOSES_AND_SERVES, 5);
    assert_eq!(reply["target_error"]["kind"], "unavailable", "after {took:?}: {reply:#}");
    let d = detail(&reply);
    assert!(d.contains("did not appear on stdout in the part the adapter read"), "after {took:?}: {d:?}");
    assert!(d.contains(&BOUND.to_string()), "the bound is not named, after {took:?}: {d:?}");
    assert!(!d.contains("without printing it"), "after {took:?}: {d:?}");
}

/// 1 MiB of lines to the byte without the value, then stdout closed: the buffer took every line of
/// the stream, so the product did close stdout without printing the value, and the adapter says so.
#[test]
fn a_stream_that_ends_at_the_bound_without_the_value_was_taken_whole() {
    let output = lines(BOUND);
    let (reply, took) = begin_scenario("p4-ends-at-the-bound", &output, CLOSES_AND_SERVES, 5);
    assert_eq!(reply["target_error"]["kind"], "unavailable", "after {took:?}: {reply:#}");
    let d = detail(&reply);
    assert!(d.contains("closed stdout without printing it"), "after {took:?}: {d:?}");
}

/// The same stream with one more line, which is not the value: the buffer left a line out, so the
/// adapter cannot say what the product did not print, and names the part it read.
#[test]
fn a_stream_one_line_longer_than_the_bound_is_not_said_to_be_without_the_value() {
    let output = lines(BOUND) + "y\n";
    let (reply, took) = begin_scenario("p4-one-line-past-the-bound", &output, CLOSES_AND_SERVES, 5);
    assert_eq!(reply["target_error"]["kind"], "unavailable", "after {took:?}: {reply:#}");
    let d = detail(&reply);
    assert!(d.contains("in the part the adapter read"), "after {took:?}: {d:?}");
    assert!(!d.contains("without printing it"), "after {took:?}: {d:?}");
}
