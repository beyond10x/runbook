//! `runbook canon`: the Canon subcommands the scripts call, over the linked library, on a committed
//! protocol file. No `canon` binary is on the path of these tests.

use super::{repo, BIN};
use std::process::{Command, Output};

fn canon(args: &[&str]) -> Output {
    Command::new(BIN).arg("canon").args(args).current_dir(repo()).env("PATH", "/usr/bin:/bin").output().unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// The `b10x-canon` version of the lock file, which is the pin.
fn locked_version() -> String {
    let lock = std::fs::read_to_string(repo().join("runbook/Cargo.lock")).unwrap();
    let at = lock.find("name = \"b10x-canon\"\n").expect("the lock file holds b10x-canon");
    lock[at..].lines().nth(1).unwrap().trim_start_matches("version = \"").trim_end_matches('"').to_string()
}

#[test]
fn canon_version_is_the_locked_release() {
    let o = canon(&["--version"]);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(text(&o.stdout), format!("canon {}\n", locked_version()));
}

#[test]
fn canon_validate_accepts_a_committed_protocol() {
    let o = canon(&["validate", "--path", "protocol/sdc-intake.protocol.yaml"]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
    assert!(text(&o.stdout).starts_with("valid: protocol `sdc.intake` revision "), "{}", text(&o.stdout));
}

#[test]
fn canon_compile_gives_the_committed_ir_byte_for_byte() {
    let o = canon(&["compile", "--path", "protocol/sdc-intake.protocol.yaml"]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
    let want = std::fs::read(repo().join("protocol/compiled/sdc-intake.ir.json")).unwrap();
    assert!(o.stdout == want, "the compiled IR differs from protocol/compiled/sdc-intake.ir.json");
}

#[test]
fn canon_exit_statuses_are_the_binarys() {
    let missing = canon(&["validate", "--path", "protocol/no-such.protocol.yaml"]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(text(&missing.stderr).starts_with("error[unreadable]: "), "{}", text(&missing.stderr));
    let usage = canon(&["validate"]);
    assert_eq!(usage.status.code(), Some(64));
    let unknown = canon(&["no-such-command"]);
    assert_eq!(unknown.status.code(), Some(64));
    let bare = canon(&[]);
    assert_eq!(bare.status.code(), Some(64));
}
