//! `impl-gate` (`bindings/tools/sdc-tool.sh`) reads cargo's test counts from stdout
//! alone. `slot` and `ess` are stand-ins on PATH: `slot` writes a scripted cargo test output, with
//! a stderr line written into the middle of a stdout `test result:` line, as cargo did under slot
//! and load ("impl-gate: FAILED run: 0 tests ran" on a passing suite).

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};

const ESS: &str = r#"#!/usr/bin/env bash
# ess specify compile --path spec --out <file>: one command
while (($#)); do [[ $1 == --out ]] && { echo '{"commands": {"billing.invoice.CreateInvoice": {}}}' > "$2"; }; shift; done
"#;

/// `slot cargo test --offline`: the output named by SLOT_FAKE.
const SLOT: &str = r#"#!/usr/bin/env bash
case $SLOT_FAKE in
  interleaved)
    printf 'running 1 test\ntest create_invoice ... ok\n\ntest result: ok. '
    printf '    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s\n' >&2
    printf '1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n' ;;
  zero) printf 'running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n' ;;
  unreadable) printf '    Finished `test` profile\n' >&2 ;;
  failing)
    printf 'test create_invoice ... FAILED\n\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out\n'
    exit 101 ;;
esac
"#;

fn gate(mode: &str) -> Output {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("impl-gate").join(mode);
    let _ = fs::remove_dir_all(&dir);
    let bin = dir.join("bin");
    fs::create_dir_all(&bin).unwrap();
    for (name, body) in [("ess", ESS), ("slot", SLOT)] {
        let p = bin.join(name);
        fs::write(&p, body).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let work = dir.join("work");
    fs::create_dir_all(work.join("impl/src")).unwrap();
    fs::create_dir_all(work.join("impl/tests")).unwrap();
    fs::create_dir_all(work.join("spec")).unwrap();
    fs::write(work.join("impl/Cargo.toml"), "[package]\nname = \"billing\"\n").unwrap();
    fs::write(work.join("impl/src/lib.rs"), "pub fn create_invoice() {}\n").unwrap();
    fs::write(work.join("impl/tests/billing.rs"), "#[test]\nfn create_invoice() {}\n").unwrap();
    let tool = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bindings/tools/sdc-tool.sh");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    Command::new("bash")
        .arg(tool)
        .args(["impl-gate", "impl"])
        .current_dir(&work)
        .env("PATH", path)
        .env("SLOT_FAKE", mode)
        .env_remove("SDC_CASE")
        .output()
        .unwrap()
}

fn text(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

#[test]
fn a_stderr_line_inside_the_result_line_still_counts_the_tests() {
    let o = gate("interleaved");
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    assert!(text(&o).contains("1 tests ran"), "{}", text(&o));
}

#[test]
fn no_test_run_is_a_failure_of_the_tree() {
    let o = gate("zero");
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert!(text(&o).contains("run: 0 tests ran"), "{}", text(&o));
}

#[test]
fn output_that_cannot_be_read_is_a_tool_error() {
    let o = gate("unreadable");
    assert_eq!(o.status.code(), Some(126), "{}", text(&o));
    assert!(text(&o).contains("could not be read"), "{}", text(&o));
}

#[test]
fn a_failing_test_is_a_failure_of_the_tree() {
    let o = gate("failing");
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert!(text(&o).contains("run: cargo test exit 101"), "{}", text(&o));
}
