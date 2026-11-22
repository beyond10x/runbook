//! Canon is the library `runbook` links: `runbook doctor` reports it and needs no `canon` binary,
//! and `runbook canon` answers a bare command line and `-V` as the `canon` binary did.

use super::{failed, line_of, stdout, World, HELPERS, PROGRAMS};
use super::{repo, BIN};
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Output};

/// The row is the Canon the binary links, whatever `canon` binaries the host has: one of another
/// version on `PATH` and a `CANON` that names another change nothing.
#[test]
fn doctor_reports_the_linked_canon_whatever_the_host_has() {
    let w = World::new("doctor-linked-canon");
    w.fakes(&PROGRAMS);
    w.fakes(&HELPERS);
    w.login("{}");
    std::fs::write(w.root.join("bin/canon"), "#!/bin/sh\necho 'canon 0.0.1'\n").unwrap();
    std::fs::set_permissions(w.root.join("bin/canon"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let pin = super::canon_pin();
    let want = format!("{} (linked, revision {})", pin["version"].as_str().unwrap(), &pin["commit"].as_str().unwrap()[..8]);
    for env in [vec![], vec![("CANON", "/nonexistent/canon")]] {
        let o = w.run(&["doctor"], &env);
        assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
        let line = line_of(&o, "canon");
        assert!(line.starts_with("pass") && line.contains(&want) && line.contains("required") && !line.contains("0.0.1"), "{line}");
    }
    let o = w.run(&["doctor", "--json"], &[]);
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    let row = v["checks"].as_array().unwrap().iter().find(|c| c["name"] == "canon").unwrap();
    assert_eq!((row["found"].as_bool(), row["version"].as_str(), row["required"].as_bool()), (Some(true), Some(want.as_str()), Some(true)), "{row}");
    assert!(row["path"].is_null() && row["install"].is_null(), "{row}");
}

fn canon(args: &[&str]) -> Output {
    Command::new(BIN).arg("canon").args(args).current_dir(repo()).env("PATH", "/usr/bin:/bin").output().unwrap()
}

/// A host with every tool but a `canon` binary: Canon is the library `runbook` links, so the
/// doctor has nothing to look for.
#[test]
fn doctor_needs_no_canon_binary() {
    let w = World::new("doctor-no-canon");
    w.fakes(&PROGRAMS);
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
    w.fakes(&PROGRAMS);
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
