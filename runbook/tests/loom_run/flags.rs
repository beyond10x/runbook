//! The options of the six ports of the old driver's launcher, accepted by `runbook run` and refused
//! until their port is built: exit 64 and one line that names the option. No run is started: the
//! refusal comes before the loop opens anything, so no run directory and no ledger is written.

use super::core::BIN;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn scratch(tag: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flags-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("idea.yaml"), "text: a note on tea\n").unwrap();
    root
}

/// `runbook run sdc` with every required option and `extra` appended; nothing it names exists.
fn run(root: &Path, extra: &[&str]) -> Output {
    let p = |name: &str| root.join(name).display().to_string();
    let mut args: Vec<String> = ["run", "sdc", "--intake", &p("idea.yaml"), "--run", "r1", "--runs-dir", &p("runs")]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    args.extend(["--bindings", &p("bindings"), "--protocols", &p("protocols"), "--root-protocol", "sdc@8", "--confinement", "local"].map(String::from));
    args.extend(["--settings", &p("settings.json"), "--spend-ledger", &p("ledger.json")].map(String::from));
    args.extend(["--cap-micro-usd", "1", "--max-llm-calls", "1", "--max-wall-seconds", "1"].map(String::from));
    args.extend(extra.iter().map(|s| (*s).to_owned()));
    Command::new(BIN).args(&args).env_clear().env("PATH", std::env::var("PATH").unwrap_or_default()).stdin(Stdio::null()).output().unwrap()
}

fn refused(root: &Path, extra: &[&str], line: &str) {
    let o = run(root, extra);
    let err = String::from_utf8_lossy(&o.stderr).into_owned();
    assert_eq!(o.status.code(), Some(64), "{extra:?}: {err}");
    assert_eq!(err.trim_end(), format!("runbook run: {line}"), "{extra:?}");
    assert!(!root.join("runs").exists() && !root.join("ledger.json").exists(), "{extra:?}: something was written");
}

#[test]
fn dry_is_accepted_and_refused_until_built() {
    refused(&scratch("dry"), &["--dry"], "--dry is not built yet");
}

#[test]
fn the_replay_options_are_accepted_and_refused_until_built() {
    let root = scratch("replay");
    refused(&root, &["--recorded", "rec"], "--recorded is not built yet");
    refused(
        &root,
        &[
            "--recorded", "rec", "--on-divergence", "nearest", "--live-adapter", "dry", "--live-cap", "3", "--inject", "timeout", "--inject", "reject",
            "--set", "max_llm_calls=5", "--take-recorded", "check", "--source-tree", "abc123",
        ],
        "--recorded is not built yet",
    );
    for policy in ["stop", "nearest", "live"] {
        refused(&root, &["--recorded", "rec", "--on-divergence", policy], "--recorded is not built yet");
    }
}

#[test]
fn a_replay_option_without_recorded_is_refused_for_that() {
    let root = scratch("replay-alone");
    for (extra, option) in [
        (&["--on-divergence", "live"][..], "--on-divergence"),
        (&["--live-adapter", "dry"], "--live-adapter"),
        (&["--live-cap", "3"], "--live-cap"),
        (&["--inject", "timeout"], "--inject"),
        (&["--set", "a=b"], "--set"),
        (&["--take-recorded", "check"], "--take-recorded"),
        (&["--source-tree", "abc123"], "--source-tree"),
    ] {
        refused(&root, extra, &format!("{option} needs --recorded <source>"));
    }
}

#[test]
fn a_policy_that_is_not_one_is_a_usage_error() {
    let o = run(&scratch("policy"), &["--recorded", "rec", "--on-divergence", "sometimes"]);
    assert_ne!(o.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&o.stderr).contains("--on-divergence"));
}

#[test]
fn bugfix_and_bugfix_ticket_are_accepted_and_refused_until_built() {
    let root = scratch("bugfix");
    refused(&root, &["--bugfix", "seed"], "--bugfix is not built yet");
    refused(&root, &["--bugfix-ticket", "ticket.yaml"], "--bugfix-ticket is not built yet");
}

#[test]
fn goals_is_accepted_and_refused_until_built() {
    refused(&scratch("goals"), &["--goals", "goals.yaml"], "--goals is not built yet");
}

#[test]
fn conformance_observe_is_refused_until_built_and_off_is_not() {
    let root = scratch("conformance");
    refused(&root, &["--conformance", "observe"], "--conformance observe is not built yet");
    let o = run(&root, &["--conformance", "off"]);
    let err = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(!err.contains("not built yet"), "{err}");
    let o = run(&root, &["--conformance", "maybe"]);
    assert_ne!(o.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&o.stderr).contains("--conformance"));
}

#[test]
fn the_port_options_belong_to_a_run_in_process_not_to_detach() {
    for extra in [&["--dry"][..], &["--watch"], &["--goals", "g.yaml"], &["--conformance", "observe"]] {
        let mut args = vec!["run", "sdc", "--intake", "i", "--detach", "--daemon", "http://127.0.0.1:1"];
        args.extend(extra);
        let o = Command::new(BIN).args(&args).env_clear().env("PATH", std::env::var("PATH").unwrap_or_default()).stdin(Stdio::null()).output().unwrap();
        assert_ne!(o.status.code(), Some(0), "{extra:?}");
        assert!(!String::from_utf8_lossy(&o.stderr).contains("not built yet"), "{extra:?}");
    }
}
