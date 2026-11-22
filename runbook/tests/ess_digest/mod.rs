//! The digest of the sources of the committed ESS suite (`task ess-suite:digest`) does not depend on
//! the `ess` binary of the host, and `task ess-suite:pin` refuses a binary that is not the release
//! `ess/ess-inputs.yaml` pins. Two fake `ess` programs print different version texts; no real `ess`
//! starts.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fake_ess(tag: &str, prints: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("ess-digest-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("ess");
    std::fs::write(&p, format!("#!/bin/sh\nprintf '%s\\n' '{prints}'\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    dir
}

fn task(fake: &Path, name: &str) -> Output {
    let path = format!("{}:{}", fake.display(), std::env::var("PATH").unwrap());
    Command::new("task").args(["--silent", name]).current_dir(super::repo()).env("PATH", path).output().expect("task is on PATH")
}

fn pin() -> String {
    let text = std::fs::read_to_string(super::repo().join("ess/ess-inputs.yaml")).unwrap();
    text.lines().find_map(|l| l.strip_prefix("requires: ess ")).expect("a requires: ess line").trim().to_string()
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

#[test]
fn the_digest_is_the_same_whatever_ess_prints() {
    let pin = pin();
    let a = task(&fake_ess("a", &format!("ess {pin}")), "ess-suite:digest");
    let b = task(&fake_ess("b", "ess 0.1.0\nsomething else"), "ess-suite:digest");
    assert!(a.status.success() && b.status.success(), "{} {}", String::from_utf8_lossy(&a.stderr), String::from_utf8_lossy(&b.stderr));
    assert_eq!(out(&a).len(), 64, "a sha256: {}", out(&a));
    assert_eq!(out(&a), out(&b), "the digest read the host's ess");
}

#[test]
fn the_pin_accepts_the_pinned_release_and_a_dispatcher_that_delegates_to_it() {
    let pin = pin();
    let direct = task(&fake_ess("pin-direct", &format!("ess {pin}")), "ess-suite:pin");
    assert!(direct.status.success(), "{}", String::from_utf8_lossy(&direct.stderr));
    assert_eq!(out(&direct), pin);
    let dispatcher = task(&fake_ess("pin-dispatcher", &format!("ess 0.0.1\ness {pin}")), "ess-suite:pin");
    assert!(dispatcher.status.success(), "{}", String::from_utf8_lossy(&dispatcher.stderr));
}

#[test]
fn the_pin_refuses_another_release_naming_both() {
    let pin = pin();
    let o = task(&fake_ess("pin-other", "ess 0.0.1"), "ess-suite:pin");
    assert!(!o.status.success(), "a host ess of another release was accepted");
    let text = format!("{}{}", out(&o), String::from_utf8_lossy(&o.stderr));
    assert!(text.contains("ess 0.0.1") && text.contains(&format!("pins ess {pin}")), "{text}");
}
