//! Canon is the library `runbook` links: `runbook doctor` reports it and needs no `canon` binary,
//! and `runbook canon` answers a bare command line and `-V` as the `canon` binary did.

use super::{failed, line_of, stdout, World, HELPERS, PROGRAMS};
use super::{repo, BIN};
use std::process::{Command, Output};

fn canon(args: &[&str]) -> Output {
    Command::new(BIN).arg("canon").args(args).current_dir(repo()).env("PATH", "/usr/bin:/bin").output().unwrap()
}

/// A host with every tool but a `canon` binary: Canon is the library `runbook` links, so the
/// doctor has nothing to look for.
#[test]
fn doctor_needs_no_canon_binary() {
    let w = World::new("doctor-no-canon");
    w.fakes(&PROGRAMS[1..]);
    w.fakes(&HELPERS);
    w.login("{}");
    let o = w.run(&["doctor"], &[]);
    assert_eq!(failed(&o), Vec::<String>::new(), "canon line: {}", line_of(&o, "canon"));
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
}

/// Whatever the doctor says about Canon names no file this change deleted, no install that no
/// script makes any more and no variable that picks a binary.
#[test]
fn doctor_names_no_deleted_canon_file() {
    let w = World::new("doctor-canon-hint");
    w.fakes(&PROGRAMS[1..]);
    w.fakes(&HELPERS);
    w.login("{}");
    let o = w.run(&["doctor"], &[]);
    let line = line_of(&o, "canon");
    for gone in ["canon-pin.json", "install the pinned Canon", "CANON=", "sdc-drive", "check-protocols"] {
        assert!(!line.contains(gone), "{gone} in {line}");
    }
}

/// With no subcommand the command line is refused with the reason and the subcommands there are.
#[test]
fn canon_without_a_subcommand_says_why() {
    let o = canon(&[]);
    assert_eq!(o.status.code(), Some(64));
    let err = String::from_utf8_lossy(&o.stderr).into_owned();
    for sub in ["validate", "compile", "check", "simulate", "evaluate"] {
        assert!(err.contains(sub), "{sub} not named in {err:?}");
    }
    assert!(o.stdout.is_empty(), "{:?}", String::from_utf8_lossy(&o.stdout));
}

/// `-V` and `--version` print the version line of the linked Canon and exit 0.
#[test]
fn canon_prints_its_version_for_dash_v_too() {
    let long = canon(&["--version"]);
    assert_eq!(long.status.code(), Some(0));
    let short = canon(&["-V"]);
    assert_eq!(short.status.code(), Some(0), "{}", String::from_utf8_lossy(&short.stderr));
    assert_eq!(short.stdout, long.stdout);
    assert!(String::from_utf8_lossy(&short.stdout).starts_with("canon "));
}
