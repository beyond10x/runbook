//! The slow lane: the stub cycle through `runbook run` on the real `b10x-loom`, compared with
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
        .args(["run", "sdc", "--run", "stub-cycle", "--runs-dir"])
        .arg(work.join("runs"))
        .arg("--loom")
        .arg(&loom)
        .args(["--loom-sha256", &sha256_of(&loom)])
        .arg("--bindings")
        .arg(root.join("bindings"))
        .arg("--protocols")
        .arg(root.join("protocol"))
        .args(["--root-protocol", "sdc@8", "--case", "IDEA-1", "--intake"])
        .arg(root.join("driver/examples/sdc/fixtures/idea.yaml"))
        .args(["--confinement", "local", "--settings"])
        .arg(work.join("settings.json"))
        .arg("--spend-ledger")
        .arg(work.join("ledger.json"))
        .args(["--cap-micro-usd", "1000000", "--max-llm-calls", "100", "--max-wall-seconds", "1800"])
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(0), "run: {}", String::from_utf8_lossy(&run.stderr));

    let summary = work.join("summary-loom.json");
    let s = Command::new(BIN).args(["parity", "summarize", "--loom-run"]).arg(work.join("runs/stub-cycle")).arg("--output").arg(&summary).output().unwrap();
    assert_eq!(s.status.code(), Some(0), "summarize: {}", String::from_utf8_lossy(&s.stderr));

    let driver = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parity/stub-cycle-driver-summary.json");
    let c = Command::new(BIN).args(["parity", "compare", "--product-digests"]).arg(&driver).arg(&summary).output().unwrap();
    let out = String::from_utf8_lossy(&c.stdout);
    assert_eq!((c.status.code(), out.lines().last()), (Some(0), Some("parity: identical")), "compare:\n{out}");
}

/// The sha256 of the `b10x-loom` binary the in-process library is compared with (Loom `5bcc26d9`).
const CHILD_LOOM_SHA256: &str = "28378a17470e09fabdaa04de29f1079e379b0e1cb030740aa78049a758819f80";

/// The stub cycle once through the child and once with Loom as a library (no `--loom`), on the same inputs.
/// Returns the directory of the two summaries, or `None` (with a message) when the child binary is absent.
fn stub_cycle_both_ways(tag: &str) -> Option<(PathBuf, PathBuf)> {
    let home = PathBuf::from(std::env::var_os("HOME").expect("HOME"));
    let loom = std::env::var_os("RUNBOOK_LOOM_BIN").map(PathBuf::from).unwrap_or_else(|| home.join(".local/state/runbook/loom/5bcc26d9/b10x-loom"));
    if !loom.is_file() {
        eprintln!("SKIPPED: the child binary {} is absent (RUNBOOK_LOOM_BIN names another); build Loom 5bcc26d9 there to run this lane", loom.display());
        return None;
    }
    let sha = sha256_of(&loom);
    assert_eq!(sha, CHILD_LOOM_SHA256, "{} is not the binary this lane is pinned to", loom.display());
    let root = repo_root();
    let driver_bin = root.join("driver/target/debug");
    assert!(driver_bin.join("sdc-findings").is_file(), "{} holds no built driver: run `cargo build --locked` in driver/", driver_bin.display());
    let base = PathBuf::from(std::env::var_os("RUNBOOK_LOOM_WORK").expect("RUNBOOK_LOOM_WORK names a directory outside the repository"));
    assert!(base.is_absolute() && !base.starts_with(repo_root()), "RUNBOOK_LOOM_WORK {} is inside the repository", base.display());
    let work = base.join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    let answers: Vec<_> = (0..5).map(|_| serde_json::json!({"approve": true, "cost_micro_usd": 0})).collect();
    let settings = serde_json::json!({"decider": {"kind": "scripted", "principal": "person:owner", "max_call_micro_usd": 0, "answers": answers}});
    std::fs::write(work.join("settings.json"), serde_json::to_string_pretty(&settings).unwrap()).unwrap();
    let path = format!("{}:{}", driver_bin.display(), std::env::var("PATH").unwrap_or_default());
    let mut summaries = Vec::new();
    for (name, way) in [("child", vec!["--loom".to_owned(), loom.display().to_string(), "--loom-sha256".to_owned(), sha.clone()]), ("library", Vec::new())] {
        let run = format!("stub-cycle-{name}");
        let out = Command::new(BIN)
            .env_clear()
            .env("PATH", &path)
            .env("HOME", &home)
            .args(["run", "sdc", "--run", &run, "--runs-dir"])
            .arg(work.join("runs"))
            .args(&way)
            // the bindings, the protocols and the root protocol come from the asset root (this checkout)
            .arg("--intake")
            .arg(root.join("driver/examples/sdc/fixtures/idea.yaml"))
            .args(["--confinement", "local", "--settings"])
            .arg(work.join("settings.json"))
            .arg("--spend-ledger")
            .arg(work.join(format!("ledger-{name}.json")))
            .args(["--cap-micro-usd", "1000000", "--max-llm-calls", "100", "--max-wall-seconds", "1800"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "run {name}: {}", String::from_utf8_lossy(&out.stderr));
        let summary = work.join(format!("summary-{name}.json"));
        let s = Command::new(BIN).args(["parity", "summarize", "--loom-run"]).arg(work.join("runs").join(&run)).arg("--output").arg(&summary).output().unwrap();
        assert_eq!(s.status.code(), Some(0), "summarize {name}: {}", String::from_utf8_lossy(&s.stderr));
        summaries.push(summary);
    }
    let library = summaries.pop().unwrap();
    let child = summaries.pop().unwrap();
    Some((child, library))
}

#[test]
#[ignore = "slow lane: `task test:loom-in-process` (needs RUNBOOK_LOOM_WORK and the child binary of Loom 5bcc26d9)"]
fn the_stub_cycle_through_the_child_and_in_process_compares_identical() {
    let Some((child, library)) = stub_cycle_both_ways("both-ways") else { return };
    let c = Command::new(BIN).args(["parity", "compare", "--product-digests"]).arg(&child).arg(&library).output().unwrap();
    let out = String::from_utf8_lossy(&c.stdout);
    println!("{out}");
    assert_eq!((c.status.code(), out.lines().last()), (Some(0), Some("parity: identical")), "compare child against in-process:\n{out}");
}
