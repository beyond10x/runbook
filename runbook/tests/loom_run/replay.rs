//! `runbook run --recorded`: the stub cycle re-driven from the committed recording
//! `driver/examples/sdc/fixtures/recorded/stub`, every model call and every decision answered from
//! it. Holds the ten replay entries of `cycles/suite.yaml` (`docs/cycles-suite.md` names the test of
//! each). No model call, no network: the adapter is `sdc-replay`, which answers from the recording.
//!
//! A re-drive takes about 70 steps of Loom, so no test makes one: the setup step `replay-runs` of
//! cargo-nextest (`runbook/.config/nextest.toml`, `runbook/tests/replay-runs.sh`) makes the six runs
//! of the suite once per test run, all at once, in a directory outside the repository (cargo refuses
//! the crate of a run below the product crate's `[workspace]`) and names it in `RUNBOOK_REPLAY_RUNS`;
//! every test reads the run of its scenario. The options of a scenario are in the script. Needs
//! `driver/target/debug` built (`sdc-node` and the other bound programs are there).

use serde_json::Value;
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn recording() -> PathBuf {
    repo().join("driver/examples/sdc/fixtures/recorded/stub")
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            files(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// One finished run of the runner: where it is and what it said.
struct Run {
    /// The run directory.
    dir: PathBuf,
    /// The exit code.
    code: i32,
    /// Standard output (the re-drive's first line comes first).
    stdout: String,
    /// Standard error (the outcome line is last).
    stderr: String,
}

impl Run {
    fn json(&self, file: &str) -> Value {
        read_json(&self.dir.join(file))
    }

    fn lines(&self, file: &str) -> Vec<Value> {
        std::fs::read_to_string(self.dir.join(file)).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
    }

    fn calls(&self) -> Value {
        self.json("replay/summary.json")["calls"].clone()
    }

    fn explain(&self) -> String {
        format!("exit {}\n{}", self.code, self.stderr.lines().rev().take(3).collect::<Vec<_>>().join("\n"))
    }
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
}

/// The run of scenario `name`, made by the setup step.
fn run(name: &str) -> Run {
    let runs = std::env::var_os("RUNBOOK_REPLAY_RUNS").map(PathBuf::from).expect("RUNBOOK_REPLAY_RUNS is not set: run this test with cargo-nextest, whose setup step `replay-runs` makes the runs");
    let work = runs.join(name);
    let done = work.join("done.json");
    assert!(done.is_file(), "{} is missing: the setup step `replay-runs` did not finish scenario {name}", done.display());
    Run {
        dir: work.join("runs").join(name),
        code: read_json(&done)["code"].as_i64().unwrap() as i32,
        stdout: std::fs::read_to_string(work.join("stdout.txt")).unwrap(),
        stderr: std::fs::read_to_string(work.join("stderr.txt")).unwrap(),
    }
}

/// The run of the four entries that read the run with no option.
fn defaults() -> Run {
    let r = run("unchanged");
    assert_eq!(r.code, 0, "{}", r.explain());
    r
}

/// Suite entry `replay-unchanged`.
#[test]
fn replay_unchanged_closes_operating_on_recorded_replies_only() {
    let r = run("unchanged");
    assert_eq!(r.code, 0, "{}", r.explain());
    let first = r.stdout.lines().next().unwrap();
    assert!(first.starts_with("re-drive of stub (recorded on tree "), "{first}");
    let status = r.json("status.json");
    assert_eq!((status["state"].as_str(), status["outcome"].as_str()), (Some("closed"), Some("operating")), "{status}");
    assert_eq!(status["replay"]["label"], "re-driven: recorded replies only");
    let calls = &status["replay"]["calls"];
    let recorded = read_json(&recording().join("recording.json"))["calls"].as_array().unwrap().len();
    assert_eq!((calls["reused"].as_u64(), calls["live"].as_u64(), calls["diverged"].as_u64(), calls["paid"].as_u64()), (Some(0), Some(0), Some(0), Some(0)), "{calls}");
    assert_eq!(calls["recorded"].as_u64(), Some(recorded as u64), "every call of the recording is asked once: {calls}");
    assert!(!r.dir.join("replay/diverged.json").exists());
}

/// Suite entry `rule-approvals-default`.
#[test]
fn rule_approvals_default_is_off_and_every_decision_is_the_deciders() {
    let r = defaults();
    let status = r.json("status.json");
    let calls = &status["replay"]["calls"];
    assert_eq!((calls["diverged"].as_u64(), calls["live"].as_u64(), calls["prompt_changed"].as_u64()), (Some(0), Some(0), Some(0)), "{calls}");
    assert_eq!((status["state"].as_str(), status["outcome"].as_str()), (Some("closed"), Some("operating")));
    assert!(!r.dir.join("rule-approvals").exists());
    let decisions = r.lines("decisions.jsonl");
    assert!(decisions.iter().all(|d| d["principal"].as_str().unwrap().starts_with("decider:")), "{decisions:?}");
    assert!(decisions.len() >= 3, "the context, the roadmap and the release are decided: {decisions:?}");
}

/// Suite entry `review-mode-default`. The engine records no `review_mode` setting: the review
/// calls' operations and turns are what the mode is.
#[test]
fn review_mode_default_reads_the_tree_with_list_read_and_search() {
    let r = defaults();
    let calls = r.calls();
    assert_eq!((calls["diverged"].as_u64(), calls["prompt_changed"].as_u64()), (Some(0), Some(0)), "{calls}");
    let reviews: Vec<Value> = r.lines("llm-calls.jsonl").into_iter().filter(|c| c["action"] == "change.review").collect();
    assert!(!reviews.is_empty());
    for c in &reviews {
        assert_eq!(c["allowed_operations"], serde_json::json!(["dir.list", "file.read", "search"]), "{c}");
        assert_eq!(c["max_turns"].as_u64(), Some(30), "{c}");
    }
}

/// Suite entry `implement-coverage-default`.
#[test]
fn implement_coverage_default_is_off() {
    let r = defaults();
    let calls = r.calls();
    assert_eq!((calls["diverged"].as_u64(), calls["prompt_changed"].as_u64()), (Some(0), Some(0)), "{calls}");
    for call in std::fs::read_dir(r.dir.join("work/.sdc/llm")).unwrap().map(|e| e.unwrap().path()).filter(|p| p.is_dir()) {
        let name = call.file_name().unwrap().to_string_lossy().into_owned();
        if name.ends_with("change.implement") {
            assert!(!call.join("cwd/in/story-requirements.json").exists() && !call.join("coverage-schema.json").exists(), "{name}");
        }
        if name.ends_with("change.review") {
            assert!(!call.join("cwd/in/coverage.json").exists(), "{name}");
        }
    }
    let mut all = Vec::new();
    files(&r.dir.join("store"), &mut all);
    assert!(all.iter().all(|f| !f.ends_with("impl/.sdc/coverage.json")));
    let refused = all.iter().filter(|f| f.to_string_lossy().contains("/blobs/")).any(|f| std::fs::read_to_string(f).is_ok_and(|t| t.contains("sdc-drive: postcondition")));
    assert!(!refused, "no postcondition refusal");
}

/// Suite entry `replay-reject-stop`. The old driver exited 9; 9 is `Denied` here, so a diverged
/// recording is exit 10.
#[test]
fn replay_reject_stop_ends_recording_diverged_when_the_rework_asks_a_call_the_source_never_made() {
    let r = run("reject-stop");
    assert_eq!(r.code, 10, "{}", r.explain());
    let stopped = r.json("run-stopped.json");
    assert_eq!(stopped["cause"], "recording_diverged", "{stopped}");
    assert!(stopped["reason"].as_str().unwrap().contains("IDEA-1.billing.invoices/change.implement@1.2"), "{stopped}");
    assert_eq!(stopped["replay"]["calls"]["diverged"].as_u64(), Some(1));
    assert_eq!(r.json("outcome.json")["outcome"], "stopped: recording diverged");
    let decisions = r.lines("decisions.jsonl");
    assert!(decisions.iter().any(|d| d["action"] == "merge.approve" && d["decision"] == "reject"), "{decisions:?}");
    let reasons: Vec<String> = std::fs::read_dir(r.dir.join("work/.sdc/reasons")).unwrap().map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap()).collect();
    assert!(reasons.iter().any(|t| t.contains("the change misses a refusal")), "{reasons:?}");
}

/// Suite entry `replay-reject-nearest`. The old driver ended `writer_spent` (exit 4): under Loom
/// the tree is stuck (exit 3) when the writer's runs are spent.
#[test]
fn replay_reject_nearest_reuses_the_recorded_reply_for_the_rework_and_stops_stuck() {
    let r = run("reject-nearest");
    assert_eq!(r.code, 3, "{}", r.explain());
    let stopped = r.json("run-stopped.json");
    assert_eq!(stopped["cause"], "stuck", "{stopped}");
    assert_eq!(stopped["replay"]["calls"]["diverged"].as_u64(), Some(0));
    assert!(stopped["replay"]["calls"]["reused"].as_u64().unwrap() >= 1, "{stopped}");
    assert_eq!(stopped["replay"]["label"], "re-driven: with reused replies");
    let last = r.lines("run-settings.jsonl").pop().unwrap();
    assert_eq!(last["unstick"]["on"], false, "{last}");
    assert_eq!(last["replay"]["unstick"]["on"], false, "{last}");
}

/// Suite entry `replay-timeout`. The exit is the timeout of the virtual clock; what the old driver
/// did next (retry the writer, which reused the recorded reply, and close) the engine does not: the
/// binding of `change.implement` has no result for exit 124, Loom refuses it (`ResultNotInTable`)
/// and the tree stops stuck. `docs/loom-run.md` names it.
#[test]
fn replay_timeout_kills_the_call_on_the_virtual_clock_and_the_tree_stops_on_the_unknown_result() {
    let r = run("timeout");
    assert_eq!(r.code, 3, "{}", r.explain());
    let calls = r.calls();
    assert_eq!(calls["faults"].as_u64(), Some(1), "{calls}");
    assert_eq!(r.json("replay/summary.json")["virtual_seconds"].as_f64(), Some(1500.0));
    let mut all = Vec::new();
    files(&r.dir.join("store/blobs"), &mut all);
    assert!(all.iter().any(|f| std::fs::read_to_string(f).is_ok_and(|t| t.contains("killed after 1500 s (virtual clock"))), "no blob holds the kill line");
    let log = std::fs::read_dir(r.dir.join("store")).unwrap().map(|e| e.unwrap().path()).filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("runlog-case-")).any(|p| std::fs::read_to_string(p).unwrap().contains("ResultNotInTable: action change.implement"));
    assert!(log, "Loom refuses the result 124 of change.implement");
}

/// Suite entry `replay-live-dry`.
#[test]
fn replay_live_dry_makes_one_unpaid_live_call_and_stops_at_the_cap() {
    let r = run("live-dry");
    assert_eq!(r.code, 10, "{}", r.explain());
    let stopped = r.json("run-stopped.json");
    assert_eq!(stopped["cause"], "recording_diverged", "{stopped}");
    assert!(stopped["reason"].as_str().unwrap().contains("live call cap is spent"), "{stopped}");
    let replay = &stopped["replay"];
    assert_eq!((replay["calls"]["live"].as_u64(), replay["calls"]["paid"].as_u64()), (Some(1), Some(0)), "{replay}");
    assert_eq!(replay["label"], "re-driven: with live model calls");
    let dry = std::fs::read_to_string(r.dir.join("dry-commands.txt")).unwrap();
    assert_eq!(dry.lines().filter(|l| l.starts_with("would run: metaharness")).count(), 1, "{dry}");
}

/// Suite entry `replay-check-fail`. The old driver ended `writer_spent` (exit 4): stuck (exit 3).
#[test]
fn replay_check_fail_fails_tests_run_once_and_the_writer_is_spent_at_the_condition() {
    let r = run("check-fail");
    assert_eq!(r.code, 3, "{}", r.explain());
    let stopped = r.json("run-stopped.json");
    assert_eq!(stopped["cause"], "stuck", "{stopped}");
    assert_eq!(stopped["replay"]["programs"]["check_fail"].as_u64(), Some(1), "{stopped}");
    let last = r.lines("run-settings.jsonl").pop().unwrap();
    assert_eq!(last["replay"]["conditions"]["max_runs.implement"], 2, "{last}");
    let first = r.stdout.lines().next().unwrap();
    assert!(first.contains("(max_runs 2)"), "{first}");
}

/// Suite entry `replay-recording-clean`: the committed recording holds no path of a home directory,
/// no prompt text and no transcript. No run.
#[test]
fn replay_recording_clean_holds_no_home_path_prompt_or_transcript() {
    let mut all = Vec::new();
    files(&recording(), &mut all);
    assert!(!all.is_empty());
    for f in &all {
        let name = f.file_name().unwrap().to_string_lossy();
        assert!(name != "prompt.md" && !name.contains("transcript"), "{}", f.display());
        let text = std::fs::read(f).unwrap();
        assert!(!String::from_utf8_lossy(&text).contains("/home/"), "{} names a home directory", f.display());
    }
    assert_eq!(read_json(&recording().join("recording.json"))["format"], "sdc.recording/1");
}
