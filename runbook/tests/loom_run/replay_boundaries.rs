//! Cases for `runbook run --recorded` (unit 4) and the cycles-suite document that
//! unit 6 changed. No model call, no network: the adapter half of `sdc-replay` is called with a
//! hand-made plan and a stand-in live adapter that only writes a marker file. Needs
//! `driver/target/debug/sdc-replay` built, like the other replay tests.

use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn put(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A run directory shaped like the runner's: `replay/plan.json` and `replay/index.json` above
/// `work/.sdc/llm/001-change.fix/`, an empty recording, and a live adapter that leaves `marker`.
struct Rig {
    root: PathBuf,
    marker: PathBuf,
    live: PathBuf,
}

impl Rig {
    fn new(tag: &str) -> Rig {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("replay-boundaries-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let marker = root.join("live-was-started");
        let live = root.join("live.sh");
        put(&live, &format!("#!/bin/sh\necho started > {}\necho '{{\"exit\": 0, \"cost_usd\": 0}}'\n", marker.display()));
        std::fs::set_permissions(&live, std::fs::Permissions::from_mode(0o755)).unwrap();
        Rig { root, marker, live }
    }

    fn plan(&self, out: &Path, policy: &str, live: Value) {
        let plan = json!({"format": "sdc.replay-plan/1", "out": out.display().to_string(), "policy": policy, "live": live, "injections": []});
        put(&out.join("replay/plan.json"), &plan.to_string());
        put(&out.join("replay/index.json"), &json!({"format": "sdc.replay-index/1", "calls": []}).to_string());
    }

    fn run_plan(&self) -> PathBuf {
        self.root.join("run/replay/plan.json")
    }

    fn live(&self, cap: Value) -> Value {
        json!({"adapter": "stand-in", "bin": self.live.display().to_string(), "cap": cap})
    }

    /// One adapter call of node `change.fix` whose record directory is below `run/work/.sdc/llm`,
    /// started as the runner starts it: `SDC_REPLAY` names the run's plan when `plan` is given.
    fn call(&self, plan: Option<&Path>) -> (i32, String) {
        let rec = self.root.join("run/work/.sdc/llm/001-change.fix");
        std::fs::create_dir_all(rec.join("cwd/in")).unwrap();
        let request = json!({"node": "change.fix", "case": "C-1", "model": "m", "record_dir": rec.display().to_string(),
            "cwd": rec.join("cwd").display().to_string(), "case_dir": self.root.join("run/work").display().to_string()});
        put(&rec.join("request.json"), &request.to_string());
        let bin = repo().join("driver/target/debug/sdc-replay");
        assert!(bin.is_file(), "{} is not built", bin.display());
        let mut command = Command::new(bin);
        command.arg(rec.join("request.json")).stdin(Stdio::null()).env_remove("SDC_REPLAY");
        if let Some(plan) = plan {
            command.env("SDC_REPLAY", plan);
        }
        let out = command.output().unwrap();
        (out.status.code().unwrap(), String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr))
    }
}

/// The plan of a call is the run's, the one the runner named: a project that has its own
/// `replay/plan.json` in the work tree (or a live model that wrote one there) must not replace the
/// run's plan, its policy, its live adapter and its cap.
#[test]
fn a_replay_plan_in_the_work_tree_is_not_the_plan_of_the_run() {
    let rig = Rig::new("decoy");
    let run = rig.root.join("run");
    rig.plan(&run, "stop", rig.live(json!(0)));
    // a data directory of the project under work/, its out pointing at a directory of its own
    let decoy_out = rig.root.join("decoy-out");
    rig.plan(&decoy_out, "live", rig.live(json!(5)));
    std::fs::create_dir_all(run.join("work/replay")).unwrap();
    std::fs::copy(decoy_out.join("replay/plan.json"), run.join("work/replay/plan.json")).unwrap();
    let (code, said) = rig.call(Some(&rig.run_plan()));
    assert!(!rig.marker.exists(), "the live adapter of a plan found in the work tree was started: {said}");
    assert_eq!(code, 75, "the run's plan says stop: {said}");
    assert!(run.join("replay/diverged.json").is_file(), "the divergence is the run's, written beside the run's plan: {said}");
}

/// The adapter is handed the plan; it looks nowhere else: a run's plan above the call, given no
/// path, is not taken, and nothing live starts.
#[test]
fn an_adapter_given_no_plan_takes_none_from_above_its_call() {
    let rig = Rig::new("nopath");
    rig.plan(&rig.root.join("run"), "live", rig.live(json!(5)));
    let (code, said) = rig.call(None);
    assert!(!rig.marker.exists(), "a live call started from a plan nobody named: {said}");
    assert_ne!(code, 0, "{said}");
    assert!(said.contains("SDC_REPLAY"), "the refusal names what is missing: {said}");
}

/// The live cap fails closed: a cap that is absent, text, negative, fractional or null starts no
/// live call; neither does a spent cap.
#[test]
fn a_live_cap_that_cannot_be_read_or_is_spent_starts_no_live_call() {
    for (what, cap) in [("absent", None), ("text", Some(json!("3"))), ("negative", Some(json!(-1))), ("fraction", Some(json!(1.5))), ("null", Some(json!(null))), ("zero", Some(json!(0)))] {
        let rig = Rig::new("cap");
        let mut live = rig.live(json!(null));
        match cap {
            Some(c) => live["cap"] = c,
            None => {
                live.as_object_mut().unwrap().remove("cap");
            }
        }
        rig.plan(&rig.root.join("run"), "live", live);
        let (code, said) = rig.call(Some(&rig.run_plan()));
        assert!(!rig.marker.exists(), "{what}: a live call started: {said}");
        assert_eq!(code, 75, "{what}: {said}");
    }
    let rig = Rig::new("spent");
    rig.plan(&rig.root.join("run"), "live", rig.live(json!(1)));
    put(&rig.root.join("run/replay/calls.jsonl"), "{\"case\": \"C-1\", \"name\": \"change.fix\", \"mode\": \"live\"}\n");
    let (code, said) = rig.call(Some(&rig.run_plan()));
    assert!(!rig.marker.exists() && code == 75, "a spent cap: {code} {said}");
    // and the cap that is readable lets exactly one through
    let rig = Rig::new("open");
    rig.plan(&rig.root.join("run"), "live", rig.live(json!(1)));
    let (code, said) = rig.call(Some(&rig.run_plan()));
    assert!(rig.marker.exists() && code == 0, "a cap of 1 and no live call yet: {code} {said}");
}

/// `docs/cycles-suite.md` quotes `#[ignore = "..."]` attributes of the Rust tests it names; a quoted
/// attribute that no test source holds any more is a row that says a test waits for something
/// that was built (unit 6 built `--dry`'s prompt and level and changed the attribute).
#[test]
fn the_cycles_suite_doc_quotes_no_ignore_attribute_that_no_test_holds() {
    let doc = std::fs::read_to_string(repo().join("docs/cycles-suite.md")).unwrap();
    let mut sources = String::new();
    let mut stack = vec![repo().join("runbook/tests"), repo().join("runbook/src"), repo().join("driver/tests")];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                sources.push_str(&std::fs::read_to_string(&p).unwrap_or_default());
            }
        }
    }
    let mut stale = Vec::new();
    let mut rest = doc.as_str();
    while let Some(i) = rest.find("#[ignore = \"") {
        let from = &rest[i..];
        let end = from.find("\"]").map(|e| e + 2).unwrap_or(from.len());
        let attribute = &from[..end];
        if !sources.contains(attribute) {
            stale.push(attribute.to_owned());
        }
        rest = &from[end.min(from.len())..];
    }
    assert!(stale.is_empty(), "docs/cycles-suite.md quotes attributes no test has: {stale:?}");
}

/// The settings key `replay` belongs to `--recorded` (the plan makes it, `preset` refuses a caller's
/// own). Without `--recorded` a settings file that names it must be refused too: otherwise it puts
/// any program path on every model node as `--adapter`, which `settings.adapter` refuses.
#[test]
fn a_settings_file_with_a_replay_key_and_no_recorded_option_is_refused() {
    let (code, said, made) = run_with_replay_key("key", &[]);
    assert_eq!(code, Some(2), "settings.replay without --recorded must be a refusal: {said}");
    assert!(said.contains("settings.replay") && said.contains("--recorded"), "the refusal names the key and the option: {said}");
    assert!(!made, "a run directory was made");
}

/// With `--recorded` the plan makes the key: a settings file that names it is refused too, so no
/// caller's program path becomes the adapter of a model node.
#[test]
fn a_settings_file_with_a_replay_key_and_the_recorded_option_is_refused() {
    let recording = repo().join("driver/examples/sdc/fixtures/recorded/stub");
    let (code, said, made) = run_with_replay_key("key-recorded", &["--recorded", &recording.display().to_string()]);
    assert_eq!(code, Some(2), "{said}");
    assert!(said.contains("settings.replay"), "{said}");
    assert!(!made, "a run directory was made");
}

/// Runs `runbook run` on the engine fixture with a settings file whose `replay.adapter` is
/// `/bin/true`; the exit code, what it said and whether the run directory exists.
fn run_with_replay_key(tag: &str, extra: &[&str]) -> (Option<i32>, String, bool) {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("replay-boundaries-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("runs")).unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook");
    let mut stack = vec![(fixture.clone(), root.join("fx"))];
    while let Some((from, to)) = stack.pop() {
        std::fs::create_dir_all(&to).unwrap();
        for e in std::fs::read_dir(&from).unwrap().flatten() {
            if e.file_type().unwrap().is_dir() {
                stack.push((e.path(), to.join(e.file_name())));
            } else {
                std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
            }
        }
    }
    put(&root.join("idea.yaml"), "text: a note on tea\n");
    let settings = json!({"loom": {"in_process": true, "confinement": "local"}, "replay": {"adapter": "/bin/true"},
        "decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [{"approve": true, "cost_micro_usd": 0}]}});
    put(&root.join("settings.json"), &settings.to_string());
    let r = |p: &str| root.join(p).display().to_string();
    let out = Command::new(super::core::BIN)
        .args(["run", "sdc", "--run", "r1", "--runs-dir", &r("runs"), "--bindings", &r("fx/bindings"), "--protocols", &r("fx/protocol"), "--root-protocol", "engine-demo@1", "--case", "ROOT-1"])
        .args(["--intake", &r("idea.yaml"), "--confinement", "local", "--settings", &r("settings.json"), "--spend-ledger", &r("ledger.json")])
        .args(["--cap-micro-usd", "1000000", "--max-llm-calls", "100", "--max-wall-seconds", "120"])
        .args(extra)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (out.status.code(), String::from_utf8_lossy(&out.stderr).into_owned(), root.join("runs/r1").exists())
}

/// `sdc-experiment` takes `flags.recorded` for a `loom` variant since unit 4 (`REPLAY_FLAGS`), so the
/// settings page must not say that the runner has no recorded mode and that the key stays refused.
#[test]
fn the_settings_page_does_not_say_the_runner_has_no_recorded_mode() {
    let page = std::fs::read_to_string(repo().join("docs/loom-run-settings.md")).unwrap();
    let flat = page.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(!flat.contains("the runner has no recorded mode"), "docs/loom-run-settings.md: the runner has a recorded mode (`runbook run --recorded`)");
    assert!(!flat.contains("`flags.recorded` and `implement_mode` stay refused"), "docs/loom-run-settings.md: `flags.recorded` is passed on as `--recorded`");
}
