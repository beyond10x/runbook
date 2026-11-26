//! The example of the long-step pattern, `examples/long-step/` (`docs/loom-bindings.md`, "A long step"): its
//! cycles run through `runbook test` with the built `sdc-command` of `driver/` first on the PATH, and
//! `runbook check` accepts the directory as a configured runbook. The poll command is a POSIX sh stand-in
//! the cycle's seed steers; no model, no network.

use super::{err, out, BIN};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn example() -> PathBuf {
    crate::repo().join("examples/long-step")
}

fn scratch(tag: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("long-step-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("run")).unwrap();
    d
}

#[test]
fn the_long_step_example_runs_its_four_cycles_as_expected() {
    let helpers = crate::repo().join("driver/target/debug");
    assert!(helpers.join("sdc-command").is_file(), "{} holds no built sdc-command: run `cargo build --locked --bins` in driver/", helpers.display());
    let d = scratch("cycles");
    let mut c = Command::new(BIN);
    c.arg("test")
        .arg(example().join("long-step.suite.yaml"))
        .args(["--scratch", d.join("scratch").to_str().unwrap(), "--confinement", "local"])
        .env("RUNBOOK_HOME", example())
        .env("XDG_RUNTIME_DIR", d.join("run"))
        .env("PATH", format!("{}:{}", helpers.display(), std::env::var("PATH").unwrap_or_default()))
        .stdin(Stdio::null());
    crate::isolate(&mut c);
    let o = c.output().unwrap();
    assert_eq!(o.status.code(), Some(0), "{}\n{}", out(&o), err(&o));
    let text = out(&o);
    for cycle in ["passes-on-third-try", "never-passes", "red-checks", "head-moved"] {
        assert!(text.lines().any(|l| l.starts_with(&format!("{cycle}: as expected ("))), "{cycle} is not as expected:\n{text}\n{}", err(&o));
    }
    assert!(text.contains("4 of 4 as expected"), "{text}");
}

#[test]
fn runbook_check_accepts_the_long_step_example_as_a_configured_runbook() {
    let d = scratch("check");
    let config = d.join("runbook.toml");
    std::fs::write(&config, format!("runbooks = [\"{}\"]\n", example().display())).unwrap();
    std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o600)).unwrap();
    let o = Command::new(BIN)
        .args(["check", "--for", "long-step"])
        .env("RUNBOOK_CONFIG", &config)
        .env("RUNBOOK_HOME", crate::repo())
        .env("XDG_RUNTIME_DIR", d.join("run"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0), "{}\n{}", out(&o), err(&o));
    assert!(out(&o).contains("1 bindings files"), "{}", out(&o));
}
