//! Cases for `runbook run --recorded` (unit 4) and the cycles-suite document that
//! unit 6 changed. No model call, no network: the adapter half of `sdc-replay` is called with a
//! hand-made plan and a stand-in live adapter that only writes a marker file. Needs
//! `driver/target/debug/sdc-replay` built, like the other replay tests.

use serde_json::{json, Value};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

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

/// A live cap that is named fails closed: one that is text, negative or fractional starts no live
/// call; neither does a spent cap (a cap that is absent or null is no cap named: no limit, see
/// `with_no_live_cap_named_every_live_call_starts_and_a_named_cap_still_stops`).
#[test]
fn a_live_cap_that_cannot_be_read_or_is_spent_starts_no_live_call() {
    for (what, cap) in [("text", Some(json!("3"))), ("negative", Some(json!(-1))), ("fraction", Some(json!(1.5))), ("zero", Some(json!(0)))] {
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

/// A plan that `prepare` did not write has no readable `live` member (missing, null, text, a number, a
/// list): it names no adapter and no cap, and it is refused like an unreadable cap, never read as "no
/// cap named". No live call starts and the call ends 75.
#[test]
fn a_plan_whose_live_member_is_missing_or_no_object_starts_no_live_call() {
    for (what, live) in [("missing", None), ("null", Some(json!(null))), ("text", Some(json!("x"))), ("number", Some(json!(5))), ("list", Some(json!([])))] {
        let rig = Rig::new("live-member");
        let run = rig.root.join("run");
        rig.plan(&run, "live", json!(null));
        let mut plan: Value = serde_json::from_str(&std::fs::read_to_string(run.join("replay/plan.json")).unwrap()).unwrap();
        match live {
            Some(v) => plan["live"] = v,
            None => {
                plan.as_object_mut().unwrap().remove("live");
            }
        }
        put(&run.join("replay/plan.json"), &plan.to_string());
        let (code, said) = rig.call(Some(&rig.run_plan()));
        assert!(!rig.marker.exists(), "{what}: a live call started: {said}");
        assert_eq!(code, 75, "{what}: {said}");
        assert!(said.contains("live"), "{what}: the refusal says what is wrong: {said}");
    }
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
    let settings = json!({"in_process": true, "confinement": "local", "replay": {"adapter": "/bin/true"},
        "decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [{"approve": true, "cost_micro_usd": 0}]}});
    put(&root.join("settings.toml"), &super::core::toml_text(&settings));
    let r = |p: &str| root.join(p).display().to_string();
    let out = Command::new(super::core::BIN)
        .args(["run", "sdc", "--run", "r1", "--runs-dir", &r("runs"), "--bindings", &r("fx/bindings"), "--protocols", &r("fx/protocol"), "--root-protocol", "engine-demo@1", "--case", "ROOT-1"])
        .args(["--intake", &r("idea.yaml"), "--confinement", "local", "--settings", &r("settings.toml"), "--spend-ledger", &r("ledger.json")])
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

fn sha(text: &str) -> String {
    let mut child = Command::new("sha256sum").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(text.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    String::from_utf8(out.stdout).unwrap().split_whitespace().next().unwrap().to_owned()
}

/// One recorded call of node `change.fix` whose inputs digest `recorded_in` (file name, text as it
/// was digested), and one adapter call whose `in/` holds `now_in`. Returns the `mode` the adapter
/// logged: `recorded` or `diverged`.
fn replayed(tag: &str, recorded_in: &[(&str, &str)], now_in: &[(&str, &str)]) -> (String, String) {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("label-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let out = root.join("run");
    let inputs: serde_json::Map<String, Value> = recorded_in.iter().map(|(f, t)| (format!("in/{f}"), json!(sha(t)))).collect();
    let call = json!({"id": "s1", "kind": "work", "case": "C-1", "name": "change.fix", "arrival": 0, "try": 1, "attempt": 1,
        "inputs": inputs, "request": {"node": "change.fix", "case": "C-1", "model": "m"}, "response": {"exit": 0}});
    put(&out.join("replay/plan.json"), &json!({"format": "sdc.replay-plan/1", "out": out.display().to_string(), "policy": "stop", "live": {}, "injections": []}).to_string());
    put(&out.join("replay/index.json"), &json!({"format": "sdc.replay-index/1", "calls": [call]}).to_string());
    let rec = out.join("work/.sdc/llm/001-change.fix");
    for (f, t) in now_in {
        put(&rec.join("cwd/in").join(f), t);
    }
    std::fs::create_dir_all(rec.join("cwd/in")).unwrap();
    let request = json!({"node": "change.fix", "case": "C-1", "model": "m", "record_dir": rec.display().to_string(),
        "cwd": rec.join("cwd").display().to_string(), "case_dir": out.join("work").display().to_string()});
    put(&rec.join("request.json"), &request.to_string());
    let bin = repo().join("driver/target/debug/sdc-replay");
    let o = Command::new(bin).arg(rec.join("request.json")).stdin(Stdio::null()).env("SDC_REPLAY", out.join("replay/plan.json")).output().unwrap();
    let said = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
    let logged = std::fs::read_to_string(rec.join("replay.json")).unwrap_or_default();
    let mode = serde_json::from_str::<Value>(&logged).ok().and_then(|v| v["mode"].as_str().map(str::to_owned)).unwrap_or_else(|| format!("no replay.json: {said}"));
    (mode, said)
}

const HEADER_0973: &str = "billing v3 against billing-reference 0.97.3 \u{2014} passed\n  | test a ... ok\n";
const HEADER_0980: &str = "billing v3 against billing-reference 0.98.0 \u{2014} passed\n  | test a ... ok\n";

/// F1 control: a recording packed at 0.97.3 replays under a pin at 0.98.0.
#[test]
fn a_recording_of_label_0_97_3_replays_when_the_pin_prints_0_98_0() {
    let (mode, said) = replayed("pin-moves", &[("tests.txt", HEADER_0973)], &[("tests.txt", HEADER_0980)]);
    assert_eq!(mode, "recorded", "{said}");
}

/// The real log has two lines of preamble above its header (the lane's recording does): the header
/// is masked wherever it stands in `in/tests.txt`, so the recording still replays under a new pin.
#[test]
fn the_header_of_the_log_is_masked_below_the_preamble_the_real_log_has() {
    let preamble = "Requires an empty logical namespace before each scenario setup.\nRequires an empty logical namespace before each scenario setup.\n";
    let (recorded, now) = (format!("{preamble}{HEADER_0973}"), format!("{preamble}{HEADER_0980}"));
    let (mode, said) = replayed("preamble", &[("tests.txt", &recorded)], &[("tests.txt", &now)]);
    assert_eq!(mode, "recorded", "{said}");
}

/// F1: a recording packed by the code before the mask, at another label (0.97.1), holds the digest of
/// its own text. The mask is not widened to meet it: the call diverges at `in/tests.txt`. The digests
/// cannot prove the label is the cause, so the message states the file changed and, as its own
/// sentence, that an old recording diverges here for that reason alone; it gives no advice.
#[test]
fn a_recording_packed_at_another_label_than_the_mask_diverges_and_the_message_names_the_old_label_case() {
    let header_0971 = HEADER_0973.replace("0.97.3", "0.97.1");
    let (mode, said) = replayed("old-pack", &[("tests.txt", &header_0971)], &[("tests.txt", HEADER_0973)]);
    assert_eq!(mode, "diverged", "{said}");
    assert!(said.contains("in/tests.txt changed. A recording packed before the ESS version label"), "{said}");
    assert!(said.contains("diverges here for that reason alone") && !said.contains("pack the recording again"), "{said}");
}

/// The same, where the recording was packed at the label the pin prints now (0.98.0): the digest of
/// the text as it is, unmasked, is the recorded one, so the message says the label is the only
/// difference.
#[test]
fn a_recording_packed_at_the_label_of_the_pin_before_the_mask_is_told_so() {
    let (mode, said) = replayed("old-pack-pin", &[("tests.txt", HEADER_0980)], &[("tests.txt", HEADER_0980)]);
    assert_eq!(mode, "diverged", "{said}");
    assert!(said.contains("only the ESS version label of its header line differs"), "{said}");
}

/// F1 scope: another input file whose line has the same shape is not the ESS label. Two versions of
/// it are two inputs.
#[test]
fn a_version_in_another_input_file_that_looks_like_the_label_is_still_an_input() {
    let notes = |v: &str| format!("shop v2 against upstream-api {v} \u{2014} the contract the code is written to\n");
    let (mode, said) = replayed("scope", &[("notes.md", &notes("0.97.3"))], &[("notes.md", &notes("9.9.9"))]);
    assert_eq!(mode, "diverged", "an input that differs only in a version-like string collapsed: {said}");
}

/// F1: a test name that changes under a new label is a changed input.
#[test]
fn a_changed_test_name_under_a_new_label_diverges() {
    let (mode, said) = replayed("test-name", &[("tests.txt", HEADER_0973)], &[("tests.txt", &HEADER_0980.replace("test a", "test b"))]);
    assert_eq!(mode, "diverged", "{said}");
}

fn run_with_settings(tag: &str, settings: Value, extra: &[&str]) -> (Option<i32>, String, bool) {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("settings-{tag}-{}", std::process::id()));
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
    put(&root.join("settings.toml"), &super::core::toml_text(&settings));
    let r = |p: &str| root.join(p).display().to_string();
    let out = Command::new(super::core::BIN)
        .args(["run", "sdc", "--run", "r1", "--runs-dir", &r("runs"), "--bindings", &r("fx/bindings"), "--protocols", &r("fx/protocol"), "--root-protocol", "engine-demo@1", "--case", "ROOT-1"])
        .args(["--intake", &r("idea.yaml"), "--confinement", "local", "--settings", &r("settings.toml"), "--spend-ledger", &r("ledger.json")])
        .args(["--cap-micro-usd", "1000000", "--max-llm-calls", "100", "--max-wall-seconds", "120"])
        .args(extra)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (out.status.code(), String::from_utf8_lossy(&out.stderr).into_owned(), root.join("runs/r1").exists())
}

fn base(extra: Value) -> Value {
    let mut v = json!({"in_process": true, "confinement": "local",
        "decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [{"approve": true, "cost_micro_usd": 0}]}});
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

/// F3: an empty or a typed `replay` (TOML has no null) is refused without `--recorded`.
#[test]
fn an_empty_or_a_typed_replay_key_without_recorded_is_refused() {
    for (what, value) in [("empty object", json!({})), ("empty array", json!([])), ("string", json!("x")), ("false", json!(false))] {
        let (code, said, made) = run_with_settings(&format!("key-{}", what.replace(' ', "-")), base(json!({"replay": value})), &[]);
        assert_eq!(code, Some(2), "replay: {what}: {said}");
        assert!(!made, "replay: {what}: a run directory was made");
    }
}

/// F3: the same key one level down is not the replay key of the run and changes nothing: it is
/// refused or ignored but never starts a run on another adapter.
#[test]
fn a_replay_key_inside_the_loom_member_does_not_make_a_run_on_another_adapter() {
    let (code, said, _) = run_with_settings("nested", base(json!({"loom": {"in_process": true, "confinement": "local", "replay": {"adapter": "/bin/true", "plan": "/x"}}})), &[]);
    assert!(code == Some(2) || code == Some(0) || code == Some(8), "{code:?} {said}");
    assert!(!said.contains("/bin/true"), "{said}");
}

/// F3: the option given twice names two recordings; one must win or the call is refused, never both.
#[test]
fn the_recorded_option_given_twice_is_refused() {
    let one = repo().join("driver/examples/sdc/fixtures/recorded/stub");
    let one = one.display().to_string();
    let (code, said, made) = run_with_settings("twice", base(json!({})), &["--recorded", &one, "--recorded", &one]);
    assert_eq!(code, Some(64), "{said}");
    assert!(!made, "a run directory was made");
}

/// F2: an adapter given no `SDC_REPLAY` finds no plan, whatever the directory holds.
#[test]
fn an_adapter_with_a_relative_plan_path_reads_it_from_its_own_directory_only() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("relative-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    put(&root.join("replay/plan.json"), &json!({"format": "sdc.replay-plan/1", "out": root.display().to_string(), "policy": "stop", "live": {}, "injections": []}).to_string());
    put(&root.join("replay/index.json"), &json!({"calls": []}).to_string());
    let rec = root.join("rec");
    put(&rec.join("request.json"), &json!({"node": "n", "case": "C-1", "model": "m", "record_dir": rec.display().to_string(), "cwd": rec.display().to_string(), "case_dir": root.display().to_string()}).to_string());
    let bin = repo().join("driver/target/debug/sdc-replay");
    let o = Command::new(bin).arg(rec.join("request.json")).current_dir(&root).env("SDC_REPLAY", "replay/plan.json").stdin(Stdio::null()).output().unwrap();
    let said = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
    assert!(!said.contains("is not set"), "{said}");
    assert!(said.contains("recording_diverged") || o.status.code() == Some(75), "a relative SDC_REPLAY is read against the adapter's directory: {said}");
}

fn preamble_log(label: &str, second: &str) -> String {
    format!("Requires an empty namespace.\nRequires an empty namespace.\nshop v2 against interpreted {label} \u{2014} passed\n  | test a ... ok\nshop v2 against upstream-api {second} \u{2014} passed\n")
}

#[test]
fn the_committed_label_replays_under_another_pin_with_a_preamble_and_a_second_label_shaped_line() {
    let (mode, said) = replayed("pin", &[("tests.txt", &preamble_log("0.97.3", "1.0.0"))], &[("tests.txt", &preamble_log("0.98.0", "1.0.0"))]);
    assert_eq!(mode, "recorded", "{said}");
}

#[test]
fn a_second_label_shaped_line_in_the_log_is_still_an_input() {
    let (mode, said) = replayed("second", &[("tests.txt", &preamble_log("0.97.3", "1.0.0"))], &[("tests.txt", &preamble_log("0.98.0", "9.9.9"))]);
    assert_eq!(mode, "diverged", "{said}");
}

#[test]
fn a_tests_txt_in_a_subdirectory_is_not_the_log() {
    let (mode, said) = replayed("subdir", &[("sub/tests.txt", &preamble_log("0.97.3", "1.0.0"))], &[("sub/tests.txt", &preamble_log("0.98.0", "1.0.0"))]);
    assert_eq!(mode, "diverged", "{said}");
}

#[test]
fn the_message_says_the_label_is_the_only_difference_when_the_unmasked_digest_is_the_recorded_one() {
    let (mode, said) = replayed("proven", &[("tests.txt", &preamble_log("0.97.4", "1.0.0"))], &[("tests.txt", &preamble_log("0.97.4", "1.0.0"))]);
    assert_eq!(mode, "diverged", "{said}");
    assert!(said.contains("only the ESS version label") && said.contains("pack the recording again"), "{said}");
}

#[test]
fn the_message_names_the_label_as_a_possible_cause_when_it_cannot_prove_it() {
    let (mode, said) = replayed("possible", &[("tests.txt", &preamble_log("0.97.1", "1.0.0"))], &[("tests.txt", &preamble_log("0.97.4", "1.0.0"))]);
    assert_eq!(mode, "diverged", "{said}");
    assert!(said.contains("diverges here for that reason alone") && !said.contains("only the ESS version label"), "{said}");
    assert!(!said.contains("pack the recording again"), "{said}");
}

/// A changed test name at the committed label has nothing to do with the label: the message must
/// not tell the reader to pack the recording again as if it were the cause.
#[test]
fn a_changed_test_at_the_committed_label_is_not_told_to_be_packed_again() {
    let (mode, said) = replayed("test-changed", &[("tests.txt", &preamble_log("0.97.3", "1.0.0"))], &[("tests.txt", &preamble_log("0.97.3", "1.0.0").replace("test a", "test b"))]);
    assert_eq!(mode, "diverged", "{said}");
    assert!(!said.contains("pack the recording again"), "{said}");
}

fn executable(path: &Path, text: &str) {
    put(path, text);
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A run directory shaped like the runner's, a stub live adapter that appends a line to `marker` at
/// each start, and the adapter half of `sdc-replay`, started as the runner starts it (`SDC_REPLAY`).
struct Replay {
    root: PathBuf,
    marker: PathBuf,
    live: PathBuf,
}

impl Replay {
    fn new(tag: &str) -> Replay {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("run-copy-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let marker = root.join("live-starts");
        let live = root.join("live.sh");
        executable(&live, &format!("#!/bin/sh\necho started >> {}\necho '{{\"exit\": 0, \"cost_usd\": 0}}'\n", marker.display()));
        Replay { root, marker, live }
    }

    fn run(&self) -> PathBuf {
        self.root.join("run")
    }

    fn plan_path(&self) -> PathBuf {
        self.run().join("replay/plan.json")
    }

    fn plan_text(&self, policy: &str, bin: &Path, cap: Value) -> String {
        json!({"format": "sdc.replay-plan/1", "out": self.run().display().to_string(), "policy": policy,
            "live": {"adapter": "stand-in", "bin": bin.display().to_string(), "cap": cap}, "injections": []})
        .to_string()
    }

    fn write_plan(&self, policy: &str, cap: Value) {
        put(&self.plan_path(), &self.plan_text(policy, &self.live, cap));
        put(&self.run().join("replay/index.json"), &json!({"format": "sdc.replay-index/1", "calls": []}).to_string());
    }

    fn request(&self) -> PathBuf {
        let rec = self.run().join("work/.sdc/llm/001-change.fix");
        std::fs::create_dir_all(rec.join("cwd/in")).unwrap();
        let request = json!({"node": "change.fix", "case": "C-1", "model": "m", "record_dir": rec.display().to_string(),
            "cwd": rec.join("cwd").display().to_string(), "case_dir": self.run().join("work").display().to_string()});
        put(&rec.join("request.json"), &request.to_string());
        rec.join("request.json")
    }

    fn command(&self) -> Command {
        let bin = repo().join("driver/target/debug/sdc-replay");
        assert!(bin.is_file(), "{} is not built", bin.display());
        let mut c = Command::new(bin);
        c.arg(self.request()).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).env("SDC_REPLAY", self.plan_path());
        c
    }

    fn call(&self) -> i32 {
        self.command().status().unwrap().code().unwrap()
    }

    fn starts(&self) -> usize {
        std::fs::read_to_string(&self.marker).map(|t| t.lines().count()).unwrap_or(0)
    }
}

/// Records what `local` allows: a program that can write the run directory rewrites the plan and the call log and the adapter obeys them; turned around by `story:replay-plan-out-of-the-run-copy`.
#[test]
fn under_local_the_adapter_obeys_a_plan_and_a_call_log_rewritten_after_the_run_began() {
    let r = Replay::new("rewrite");
    r.write_plan("stop", json!(0));
    assert_eq!((r.call(), r.starts()), (75, 0), "the plan of the run says stop: no live call");
    let rewrite = r.root.join("rewrite.sh");
    executable(
        &rewrite,
        &format!("#!/bin/sh\ncat > '{}' <<'PLAN'\n{}\nPLAN\n: > '{}'\n", r.plan_path().display(), r.plan_text("live", &r.live, json!(99)), r.run().join("replay/calls.jsonl").display()),
    );
    assert!(Command::new(&rewrite).status().unwrap().success());
    assert_eq!((r.call(), r.starts()), (0, 1), "a live call that the run's plan did not allow was started as the adapter's program (a fix that pins the plan flips this case)");
}


/// A live call counts against the cap from its start: the adapter appends its started line to
/// `replay/calls.jsonl` before it starts the live program, so a live call that is ended before it
/// finishes (the action's time limit, a signal) is counted and the next call diverges under a cap of 1.
#[test]
fn a_live_call_that_is_ended_before_it_finishes_counts_against_the_cap() {
    let r = Replay::new("cap-late");
    let pid_file = r.root.join("live-pid");
    // The first start waits (and is ended); the second answers at once.
    executable(
        &r.live,
        &format!(
            "#!/bin/sh\necho started >> '{m}'\nif [ \"$(wc -l < '{m}')\" -ge 2 ]; then echo '{{\"exit\": 0, \"cost_usd\": 0}}'; exit 0; fi\necho $$ > '{p}'\nexec sleep 60\n",
            m = r.marker.display(),
            p = pid_file.display()
        ),
    );
    r.write_plan("live", json!(1));
    let mut first = r.command().spawn().unwrap();
    let limit = Instant::now() + Duration::from_secs(20);
    while !pid_file.exists() {
        assert!(Instant::now() < limit, "the live adapter did not start");
        std::thread::sleep(Duration::from_millis(20));
    }
    first.kill().unwrap();
    let _ = first.wait();
    let live_pid = std::fs::read_to_string(&pid_file).unwrap().trim().to_owned();
    let _ = Command::new("kill").args(["-KILL", &live_pid]).status();
    let second = r.call();
    assert_eq!((r.starts(), second), (1, 75), "the cap is 1 and one live call was started and ended: the second call must diverge, not start another live call (live starts: {}, exit {second})", r.starts());
}

/// The cap is one step: two adapter processes that start at the same moment at a cap of 1 start one
/// live program between them; the other diverges, whichever is first.
#[test]
fn two_adapters_that_start_together_at_a_cap_of_one_start_one_live_program_between_them() {
    let r = Replay::new("cap-two");
    executable(&r.live, &format!("#!/bin/sh\necho started >> '{}'\nsleep 1\necho '{{\"exit\": 0, \"cost_usd\": 0}}'\n", r.marker.display()));
    r.write_plan("live", json!(1));
    let mut first = r.command().spawn().unwrap();
    let mut second = r.command().spawn().unwrap();
    let mut codes = vec![first.wait().unwrap().code().unwrap(), second.wait().unwrap().code().unwrap()];
    codes.sort();
    assert_eq!((r.starts(), codes.clone()), (1, vec![0, 75]), "one live program between two processes at cap 1: starts {}, exits {codes:?}", r.starts());
}

/// A log that an older adapter wrote has no mark: its live line is a finished call and counts once.
/// A log with a started line and no finish counts that call too.
#[test]
fn a_live_line_without_the_mark_is_a_finished_call_and_a_started_line_without_a_finish_counts() {
    let live_line = |extra: Value| {
        let mut l = json!({"kind": "work", "case": "C-0", "name": "change.fix", "mode": "live", "clock": "real", "record": "work/.sdc/llm/000-x"});
        l.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        l.to_string() + "\n"
    };
    for (what, log, cap, starts, exit) in [
        ("an old finished line, cap 1", live_line(json!({})), 1, 0, 75),
        ("an old finished line, cap 2", live_line(json!({})), 2, 1, 0),
        ("a started line without a finish, cap 1", live_line(json!({"phase": "started"})), 1, 0, 75),
        ("a started line and its finish, cap 1", live_line(json!({"phase": "started"})) + &live_line(json!({"phase": "finished"})), 1, 0, 75),
        ("a started line and its finish, cap 2", live_line(json!({"phase": "started"})) + &live_line(json!({"phase": "finished"})), 2, 1, 0),
    ] {
        let r = Replay::new("cap-marks");
        r.write_plan("live", json!(cap));
        put(&r.run().join("replay/calls.jsonl"), &log);
        let code = r.call();
        assert_eq!((r.starts(), code), (starts, exit), "{what}");
    }
}

struct CapRig {
    root: PathBuf,
    marker: PathBuf,
    next: std::cell::Cell<u32>,
}

impl CapRig {
    /// `nap`: how long the stand-in live program sleeps after it wrote its marker line.
    fn new(tag: &str, cap: Value, nap: &str) -> CapRig {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("cap-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let marker = root.join("live-starts");
        let live = root.join("live.sh");
        put(&live, &format!("#!/bin/sh\necho started >> {}\n[ -f {} ] || sleep {nap}\necho '{{\"exit\": 0, \"cost_usd\": 0}}'\n", marker.display(), root.join("fast").display()));
        std::fs::set_permissions(&live, std::fs::Permissions::from_mode(0o755)).unwrap();
        let out = root.join("run");
        let mut live_member = json!({"adapter": "stand-in", "bin": live.display().to_string()});
        if !cap.is_null() {
            live_member["cap"] = cap;
        }
        put(&out.join("replay/plan.json"), &json!({"format": "sdc.replay-plan/1", "out": out.display().to_string(), "policy": "live", "live": live_member, "injections": []}).to_string());
        put(&out.join("replay/index.json"), &json!({"format": "sdc.replay-index/1", "calls": []}).to_string());
        CapRig { root, marker, next: std::cell::Cell::new(0) }
    }

    fn log(&self) -> PathBuf {
        self.root.join("run/replay/calls.jsonl")
    }

    fn starts(&self) -> usize {
        std::fs::read_to_string(&self.marker).unwrap_or_default().lines().count()
    }

    fn spawn(&self) -> Child {
        let n = self.next.get() + 1;
        self.next.set(n);
        let out = self.root.join("run");
        let rec = out.join(format!("work/.sdc/llm/{n:03}-change.fix"));
        std::fs::create_dir_all(rec.join("cwd/in")).unwrap();
        let request = json!({"node": "change.fix", "case": "C-1", "model": "m", "record_dir": rec.display().to_string(),
            "cwd": rec.join("cwd").display().to_string(), "case_dir": out.join("work").display().to_string()});
        put(&rec.join("request.json"), &request.to_string());
        Command::new(repo().join("driver/target/debug/sdc-replay"))
            .arg(rec.join("request.json"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .env("SDC_REPLAY", out.join("replay/plan.json"))
            .spawn()
            .unwrap()
    }

    fn call(&self) -> i32 {
        self.spawn().wait().unwrap().code().unwrap_or(-1)
    }

    fn wait_for_starts(&self, n: usize) {
        let until = Instant::now() + Duration::from_secs(10);
        while self.starts() < n {
            assert!(Instant::now() < until, "the live program was not started");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn fast(&self) {
        put(&self.root.join("fast"), "");
    }
}

#[test]
fn a_first_call_ended_mid_call_is_counted_at_cap_1() {
    let r = CapRig::new("killed", json!(1), "3");
    let mut first = r.spawn();
    r.wait_for_starts(1);
    first.kill().unwrap();
    first.wait().unwrap();
    r.fast();
    assert_eq!(r.call(), 75, "the second call must be refused");
    assert_eq!(r.starts(), 1, "one live start in all");
}

#[test]
fn processes_starting_together_never_start_more_live_programs_than_the_cap() {
    for (procs, cap) in [(2, 1), (4, 1), (4, 2), (2, 2)] {
        for round in 0..4 {
            let r = CapRig::new(&format!("race-{procs}-{cap}-{round}"), json!(cap), "0.3");
            let children: Vec<Child> = (0..procs).map(|_| r.spawn()).collect();
            let codes: Vec<i32> = children.into_iter().map(|mut c| c.wait().unwrap().code().unwrap_or(-1)).collect();
            assert!(r.starts() <= cap, "{procs} processes at cap {cap} round {round}: {} live starts, exit codes {codes:?}", r.starts());
            assert_eq!(r.starts(), cap.min(procs), "{procs} processes at cap {cap}: the cap is used, not undershot: {codes:?}");
        }
    }
}

#[test]
fn a_finished_call_is_counted_once_at_cap_2() {
    let r = CapRig::new("once", json!(2), "0");
    assert_eq!((r.call(), r.call()), (0, 0));
    assert_eq!(r.call(), 75, "the third call is refused: two live calls, counted once each");
    assert_eq!(r.starts(), 2);
}

#[test]
fn an_old_log_without_phases_counts_its_finished_live_call() {
    let r = CapRig::new("old-log", json!(1), "0");
    put(&r.log(), &(json!({"mode": "live", "record": "work/.sdc/llm/000-x", "case": "C-1", "name": "change.fix"}).to_string() + "\n"));
    assert_eq!(r.call(), 75);
    assert_eq!(r.starts(), 0);
}

#[test]
fn an_empty_log_starts_one_call_at_cap_1_and_refuses_the_next() {
    let r = CapRig::new("empty", json!(1), "0");
    put(&r.log(), "");
    assert_eq!((r.call(), r.call()), (0, 75));
    assert_eq!(r.starts(), 1);
}

/// A cut-off line counts as a started live call, so the cap of 1 is spent and no call starts live.
#[test]
fn a_cut_off_last_line_does_not_make_a_call_free() {
    let r = CapRig::new("partial", json!(1), "0");
    put(&r.log(), "{\"mode\": \"live\", \"phase\": \"star");
    let codes = [r.call(), r.call(), r.call()];
    assert_eq!((codes, r.starts()), ([75, 75, 75], 0), "cap 1 after a cut-off line: the fragment is one started call");
}

/// A log whose last line is complete but has no newline keeps that line apart from the lines the
/// adapter appends: the first call starts live (a recorded line counts for nothing), the second
/// gets 75, and the log holds the old line and three new lines, each a JSON object.
#[test]
fn a_log_whose_last_line_has_no_newline_keeps_its_line_and_the_new_lines_apart() {
    let r = CapRig::new("no-newline", json!(1), "0");
    let old = json!({"mode": "recorded", "kind": "work", "case": "C-0", "name": "change.fix", "phase": "finished"}).to_string();
    put(&r.log(), &old);
    assert_eq!((r.call(), r.call()), (0, 75));
    assert_eq!(r.starts(), 1);
    let text = std::fs::read_to_string(r.log()).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 4, "{text}");
    assert_eq!(lines[0], old);
    assert!(lines.iter().all(|l| serde_json::from_str::<Value>(l).is_ok_and(|v| v.is_object())), "{text}");
}

#[test]
fn a_log_with_a_byte_that_is_not_text_does_not_lift_the_cap() {
    let r = CapRig::new("bytes", json!(1), "0");
    let mut bytes = (json!({"mode": "live", "phase": "started", "record": "work/.sdc/llm/000-x"}).to_string() + "\n").into_bytes();
    bytes.extend_from_slice(b"\xff\xfe\n");
    std::fs::create_dir_all(r.log().parent().unwrap()).unwrap();
    std::fs::write(r.log(), bytes).unwrap();
    assert_eq!(r.call(), 75, "one live call is in the log: the cap of 1 is spent");
    assert_eq!(r.starts(), 0);
}

#[test]
fn a_log_that_is_a_directory_or_unreadable_starts_no_live_call() {
    let r = CapRig::new("dir", json!(5), "0");
    std::fs::create_dir_all(r.log()).unwrap();
    assert_ne!(r.call(), 0);
    assert_eq!(r.starts(), 0, "a directory");
    let r = CapRig::new("unreadable", json!(5), "0");
    put(&r.log(), "");
    std::fs::set_permissions(r.log(), std::fs::Permissions::from_mode(0o000)).unwrap();
    assert_ne!(r.call(), 0);
    assert_eq!(r.starts(), 0, "unreadable");
}

/// No cap named is no limit: a plan with no `live.cap` (or a null one, as `prepare` writes it when
/// `--live-cap` is not given) lets every live call start, past the fifth too; a named cap still stops.
#[test]
fn with_no_live_cap_named_every_live_call_starts_and_a_named_cap_still_stops() {
    let r = CapRig::new("no-cap", Value::Null, "0");
    let codes: Vec<i32> = (0..7).map(|_| r.call()).collect();
    assert_eq!((codes, r.starts()), (vec![0; 7], 7), "no cap in the plan: no limit on live calls");
    let rig = Rig::new("null-cap");
    rig.plan(&rig.root.join("run"), "live", rig.live(json!(null)));
    for n in 1..=7 {
        let (code, said) = rig.call(Some(&rig.run_plan()));
        assert!(rig.marker.exists() && code == 0, "a null cap, call {n}: {code} {said}");
    }
    let r = CapRig::new("cap-named", json!(6), "0");
    let codes: Vec<i32> = (0..7).map(|_| r.call()).collect();
    assert_eq!((codes, r.starts()), (vec![0, 0, 0, 0, 0, 0, 75], 6), "a named cap of 6 stops the seventh");
}

#[test]
fn without_a_readable_cap_no_live_call_starts() {
    for (tag, cap) in [("text", json!("2")), ("negative", json!(-1))] {
        let r = CapRig::new(&format!("cap-{tag}"), cap, "0");
        assert_eq!(r.call(), 75, "{tag}");
        assert_eq!(r.starts(), 0, "{tag}");
    }
}

/// A plan whose `live` object names the cap twice (a plan edited by hand) cannot be read: no live
/// program starts, and the later (here larger) cap is not the one that stays.
#[test]
fn a_plan_whose_live_object_names_the_cap_twice_starts_no_live_call() {
    let r = CapRig::new("dup", json!("@@CAP@@"), "0");
    let path = r.root.join("run/replay/plan.json");
    let text = std::fs::read_to_string(&path).unwrap().replace("\"cap\":\"@@CAP@@\"", "\"cap\":1,\"cap\":100");
    assert!(text.contains("\"cap\":100"), "{text}");
    put(&path, &text);
    let codes: Vec<i32> = (0..3).map(|_| r.call()).collect();
    assert_eq!(r.starts(), 0, "two caps named, 1 and 100: {} live starts, exit codes {codes:?}", r.starts());
}
