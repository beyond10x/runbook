//! The slow lane: the stub cycle through `runbook loom-run` on the real `b10x-loom`, compared with
//! the summary of the same cycle under the old driver (`tests/fixtures/parity/stub-cycle-driver-summary.json`).
//!
//! Ignored in the gate lane. `task test:loom-real` runs it with `RUNBOOK_LOOM_BIN` set to the real
//! binary, `RUNBOOK_LOOM_WORK` set to a directory outside the repository and `--ignored`; it needs
//! `driver/target/debug` built (the bound programs are there) and
//! makes no model call: the decider is scripted, the adapter is the stub, the confinement is `local`.

use super::core::BIN;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn sha256_of(file: &Path) -> String {
    let o = Command::new("sha256sum").arg(file).output().expect("sha256sum");
    String::from_utf8(o.stdout).unwrap().split_whitespace().next().unwrap().to_owned()
}

#[test]
#[ignore = "slow lane: set RUNBOOK_LOOM_BIN to the real b10x-loom and run `task test:loom-real`"]
fn the_stub_cycle_on_the_real_loom_compares_identical_with_the_driver_run() {
    let loom = PathBuf::from(std::env::var_os("RUNBOOK_LOOM_BIN").expect("RUNBOOK_LOOM_BIN names the real b10x-loom binary"));
    assert!(loom.is_file(), "RUNBOOK_LOOM_BIN {} is not a file", loom.display());
    let root = repo_root();
    let driver_bin = root.join("driver/target/debug");
    assert!(driver_bin.join("sdc-findings").is_file(), "{} holds no built driver: run `cargo build --locked` in driver/", driver_bin.display());

    let base = PathBuf::from(std::env::var_os("RUNBOOK_LOOM_WORK").expect("RUNBOOK_LOOM_WORK names a directory outside the repository for the run (cargo finds the repository's workspace above a run directory below it and refuses the product crate)"));
    assert!(base.is_absolute() && !base.starts_with(repo_root()), "RUNBOOK_LOOM_WORK {} is inside the repository", base.display());
    let work = base.join(format!("stub-cycle-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    let answers: Vec<_> = (0..5).map(|_| serde_json::json!({"approve": true, "cost_micro_usd": 0})).collect();
    let settings = serde_json::json!({"decider": {"kind": "scripted", "principal": "person:owner", "max_call_micro_usd": 0, "answers": answers}});
    std::fs::write(work.join("settings.json"), serde_json::to_string_pretty(&settings).unwrap()).unwrap();

    let path = format!("{}:{}", driver_bin.display(), std::env::var("PATH").unwrap_or_default());
    let run = Command::new(BIN)
        .env_clear()
        .env("PATH", &path)
        .env("HOME", std::env::var_os("HOME").unwrap_or_default())
        .args(["loom-run", "--run", "stub-cycle", "--runs-dir"])
        .arg(work.join("runs"))
        .arg("--loom")
        .arg(&loom)
        .args(["--loom-sha256", &sha256_of(&loom)])
        .arg("--bindings")
        .arg(root.join("bindings"))
        .arg("--protocols")
        .arg(root.join("protocol"))
        .args(["--root-protocol", "sdc@8", "--case", "IDEA-1", "--idea"])
        .arg(root.join("driver/examples/sdc/fixtures/idea.yaml"))
        .args(["--confinement", "local", "--settings"])
        .arg(work.join("settings.json"))
        .arg("--spend-ledger")
        .arg(work.join("ledger.json"))
        .args(["--cap-micro-usd", "1000000", "--max-llm-calls", "100", "--max-wall-seconds", "1800"])
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(0), "loom-run: {}", String::from_utf8_lossy(&run.stderr));

    let summary = work.join("summary-loom.json");
    let s = Command::new(BIN).args(["parity", "summarize", "--loom-run"]).arg(work.join("runs/stub-cycle")).arg("--output").arg(&summary).output().unwrap();
    assert_eq!(s.status.code(), Some(0), "summarize: {}", String::from_utf8_lossy(&s.stderr));

    let driver = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parity/stub-cycle-driver-summary.json");
    let c = Command::new(BIN).args(["parity", "compare", "--product-digests"]).arg(&driver).arg(&summary).output().unwrap();
    let out = String::from_utf8_lossy(&c.stdout);
    assert_eq!((c.status.code(), out.lines().last()), (Some(0), Some("parity: identical")), "compare:\n{out}");
}
