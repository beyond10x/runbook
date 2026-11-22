//! `sdc-experiment` with `runner: loom` (story:experiment-runner-loom, docs/workflow-metrics.md
//! "Runner loom"): the record format, the printed launch plan, the environment of the unit, the
//! login file that is named and never printed, status and stop, and the run directory `report`
//! reads. Nothing here starts a unit, calls a model or runs `systemd-run`: `run` is only ever
//! called with `--print`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

const IDEA: &str = "Bill customers with invoices.";
const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const LOOM_SHA: &str = "d80830ea5b1b2c3d4e5f60718293a4b5c6d7e8f90123456789abcdef01234567";

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("experiment-loom").join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn sha(text: &str) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// The `fixed` settings of the example record.
fn fixed() -> Vec<(String, String)> {
    [
        ("loom.bin", "/opt/loom/b10x-loom"),
        ("loom.sha256", LOOM_SHA),
        ("loom.confinement", "local"),
        ("loom.cap_micro_usd", "5000000"),
        ("models.writer.model", "claude-sonnet-5-5"),
        ("models.implement.model", "claude-sonnet-5-5"),
        ("models.review.model", "claude-sonnet-5-5"),
        ("models.other.model", "claude-sonnet-5-5"),
        ("decider.model", "claude-opus-5-5"),
        ("max_llm_calls", "150"),
        ("max_wall_seconds", "3600"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

/// A planned prospective record with two `loom` variants; `fixed` is the list of settings shared
/// by both, `commit` the pinned commit (`null` while planned).
fn record_with(fixed: &[(String, String)], commit: &str, status: &str, runs: (&str, &str)) -> String {
    let settings: String = fixed.iter().map(|(k, v)| format!("  {k}: {v}\n")).collect();
    format!(
        r#"format: sdc.experiment/1
id: lx
kind: prospective
status: {status}
created: 2026-11-22
title: A model run of the cycle through Loom
purpose: Whether a registered record starts a model run through Loom.
inputs:
- {{name: invoices, kind: idea, text: {IDEA}, sha256: {}}}
tree_commit: {commit}
fixed:
{settings}variants:
- {{id: low, runner: loom, settings: {{max_llm_calls: 100}}, runs: [{}]}}
- {{id: high, runner: loom, settings: {{max_llm_calls: 200}}, runs: [{}]}}
runs_per_variant: 1
pass_rule: clean-delivery
decision_rule: {{rule: descriptive, text: the record describes its runs and decides nothing}}
"#,
        sha(IDEA),
        runs.0,
        runs.1
    )
}

/// The example record of docs/workflow-metrics.md: `fixed` without `max_llm_calls` (each variant
/// sets its own).
fn example(commit: &str) -> String {
    let f: Vec<(String, String)> = fixed().into_iter().filter(|(k, _)| k != "max_llm_calls").collect();
    record_with(&f, commit, "planned", ("", ""))
}

fn write_record(d: &Path, text: &str) {
    fs::create_dir_all(d.join("experiments")).unwrap();
    fs::write(d.join("experiments/lx.yaml"), text).unwrap();
}

fn experiment_env(args: &[&str], cwd: &Path, env: &[(&str, &str)]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_sdc-experiment")).args(args).current_dir(cwd).env_clear().envs(env.iter().copied()).output().unwrap();
    (o.status.code().unwrap_or(-1), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
}

fn experiment(args: &[&str], cwd: &Path) -> (i32, String) {
    experiment_env(args, cwd, &[("PATH", "/usr/bin:/bin"), ("HOME", "/nonexistent-home")])
}

/// A build directory of `COMMIT` that holds what `run` reads from the tree: the root protocol and
/// the `runbook/` crate.
fn fake_build(d: &Path, revision: u32, with_crate: bool) -> PathBuf {
    let b = d.join("b").join(COMMIT);
    fs::create_dir_all(b.join("protocol")).unwrap();
    fs::write(b.join("protocol/sdc.protocol.yaml"), format!("format: protocol/1\nprotocol:\n  id: sdc\n  revision: {revision}\n")).unwrap();
    if with_crate {
        fs::create_dir_all(b.join("runbook")).unwrap();
        fs::write(b.join("runbook/Cargo.toml"), "[package]\nname = \"runbook\"\n").unwrap();
    }
    d.join("b")
}

#[test]
fn a_loom_record_is_checked_and_a_missing_or_old_setting_is_refused_by_name() {
    let d = scratch("check");
    write_record(&d, &example("null"));
    let (code, out) = experiment(&["check", "lx"], &d);
    assert_eq!(code, 0, "a planned loom record without runs is accepted: {out}");
    // Every required setting, taken out, is named.
    for (k, _) in fixed().iter().filter(|(k, _)| k != "max_llm_calls") {
        let f: Vec<_> = fixed().into_iter().filter(|(x, _)| x != k && x != "max_llm_calls").collect();
        write_record(&d, &record_with(&f, "null", "planned", ("", "")));
        let (code, out) = experiment(&["check", "lx"], &d);
        assert_eq!(code, 1, "{k}: {out}");
        assert!(out.contains(&format!("the settings do not set {k}")), "{k}: {out}");
    }
    // A setting only the old driver understands is refused by its name, with the reason.
    for (k, v) in [("decider.kind", "haiku"), ("decider.escalate", "final"), ("decider.max_decider_calls", "40"), ("adapter", "metaharness"),
        ("decider.adapter", "claude-cli"), ("autonomy", "A5"), ("implement_mode", "x"), ("efforts.writer.effort", "low"), ("flags.dry", "true")] {
        let mut f = fixed();
        f.push((k.to_string(), v.to_string()));
        write_record(&d, &record_with(&f, "null", "planned", ("", "")));
        let (code, out) = experiment(&["check", "lx"], &d);
        assert_eq!(code, 1, "{k}: {out}");
        assert!(out.contains(&format!("{k} is refused for a loom variant")), "{k}: {out}");
    }
    // A setting nobody knows, and bad values.
    for (k, v, want) in [("bogus.key", "1", "bogus.key is no loom launch setting"),
        ("loom.confinement", "none", "loom.confinement must be local or substrate"),
        ("loom.sha256", "abc", "loom.sha256 must be the 64 hex digits"),
        ("loom.cap_micro_usd", "0", "loom.cap_micro_usd must be a whole number of at least 1"),
        ("loom.bin", "b10x-loom", "loom.bin must be an absolute path or start with ~/"),
        ("max_wall_seconds", "soon", "max_wall_seconds must be a whole number of at least 1")] {
        let mut f: Vec<_> = fixed().into_iter().filter(|(x, _)| x != k && x != "max_llm_calls").collect();
        f.push((k.to_string(), v.to_string()));
        write_record(&d, &record_with(&f, "null", "planned", ("", "")));
        let (code, out) = experiment(&["check", "lx"], &d);
        assert_eq!(code, 1, "{k}: {out}");
        assert!(out.contains(want), "{k}: {out}");
    }
    // A variant sets the call cap itself or inherits it.
    write_record(&d, &example("null").replace("{max_llm_calls: 200}", "{}"));
    let (code, out) = experiment(&["check", "lx"], &d);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("variant \"high\": the settings do not set max_llm_calls"), "{out}");
    // The runner name is accepted beside sdc and free-agent and nothing else is.
    write_record(&d, &example("null").replace("runner: loom, settings: {max_llm_calls: 100}", "runner: other, settings: {max_llm_calls: 100}"));
    let (code, out) = experiment(&["check", "lx"], &d);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("`runner` must be sdc, free-agent or loom"), "{out}");
    // A loom run starts from an idea.
    write_record(&d, &example("null").replace("kind: idea", "kind: fixture"));
    let (code, out) = experiment(&["check", "lx"], &d);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("a loom variant starts from an idea input"), "{out}");
}

#[test]
fn run_print_gives_each_run_its_unit_and_the_exact_loom_run_command() {
    let d = scratch("print");
    let builds = fake_build(&d, 11, true);
    write_record(&d, &example(COMMIT));
    let (code, out) = experiment(&["run", "lx", "--print", "--builds-dir", builds.to_str().unwrap(), "--runs-dir", "runs"], &d);
    assert_eq!(code, 0, "{out}");
    let build = builds.join(COMMIT);
    let runs = d.join("runs");
    assert!(out.contains("lx.low.1 (sdc-exp-lx-low-1):") && out.contains("lx.high.1 (sdc-exp-lx-high-1):"), "{out}");
    let launch = builds.parent().unwrap().join("launch").join("lx");
    for (name, calls) in [("lx.low.1", "100"), ("lx.high.1", "200")] {
        let want = format!(
            "  {} loom-run --run {name} --runs-dir {} --loom /opt/loom/b10x-loom --loom-sha256 {LOOM_SHA} --bindings {} --protocols {} \
             --root-protocol sdc@11 --case IDEA-1 --idea {} --confinement local --settings {} --spend-ledger {} --cap-micro-usd 5000000 \
             --max-llm-calls {calls} --max-wall-seconds 3600",
            build.join("runbook/target/debug/runbook").display(), runs.display(), build.join("bindings").display(), build.join("protocol").display(),
            launch.join(name).join("idea.yaml").display(), launch.join(name).join("settings.json").display(), runs.join("lx.loom-spend.json").display());
        assert!(out.lines().any(|l| l == want), "no line\n{want}\nin\n{out}");
    }
    // The root protocol is the registration of the tree's file: another revision, another argument.
    fake_build(&d, 12, true);
    let (_, out) = experiment(&["run", "lx", "--print", "--builds-dir", builds.to_str().unwrap()], &d);
    assert!(out.contains("--root-protocol sdc@12") && !out.contains("sdc@11"), "{out}");
    // The build step names the runbook crate beside the driver's.
    assert!(out.lines().any(|l| l.starts_with("build: ") && l.contains("runbook (cargo build --locked)")), "{out}");
    // The settings the file will hold are printed, and the files are not written by --print.
    assert!(out.contains("\"decider\":{\"model\":\"claude-opus-5-5\",\"principal\":\"decider:model\"}"), "{out}");
    assert!(!launch.exists(), "--print wrote {}", launch.display());
}

#[test]
fn the_root_protocol_and_the_runbook_crate_are_read_from_the_commit_when_no_build_exists() {
    let d = scratch("git");
    let repo = d.join("repo");
    fs::create_dir_all(repo.join("protocol")).unwrap();
    let git = |args: &[&str]| {
        let o = Command::new("git").arg("-C").arg(&repo)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid", "-c", "commit.gpgsign=false"]).args(args).output().unwrap();
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    };
    git(&["init", "--quiet"]);
    fs::write(repo.join("protocol/sdc.protocol.yaml"), "protocol:\n  id: sdc\n  revision: 9\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "--quiet", "-m", "no crate"]);
    let without = git(&["rev-parse", "HEAD"]);
    fs::create_dir_all(repo.join("runbook")).unwrap();
    fs::write(repo.join("runbook/Cargo.toml"), "[package]\nname = \"runbook\"\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "--quiet", "-m", "crate"]);
    let with = git(&["rev-parse", "HEAD"]);
    fs::create_dir_all(repo.join("experiments")).unwrap();
    let builds = d.join("nobuilds");
    let args = ["run", "lx", "--print", "--builds-dir", builds.to_str().unwrap()];
    fs::write(repo.join("experiments/lx.yaml"), example(&without)).unwrap();
    let (code, out) = experiment(&args, &repo);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains(&format!("refused: tree_commit {without} has no runbook/ crate")), "{out}");
    fs::write(repo.join("experiments/lx.yaml"), example(&with)).unwrap();
    let (code, out) = experiment(&args, &repo);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("--root-protocol sdc@9"), "{out}");
    // A commit the repository does not know is refused with git's words, not read as "no crate".
    fs::write(repo.join("experiments/lx.yaml"), example(COMMIT)).unwrap();
    let (code, out) = experiment(&args, &repo);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("refused:") && out.contains(COMMIT), "{out}");
}

#[test]
fn a_tree_without_a_runbook_crate_is_refused_at_run_and_a_loom_run_gets_the_base_environment_only() {
    let d = scratch("env");
    let builds = fake_build(&d, 8, false);
    write_record(&d, &example(COMMIT));
    let args = ["run", "lx", "--print", "--builds-dir", builds.to_str().unwrap()];
    let (code, out) = experiment(&args, &d);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains(&format!("refused: tree_commit {COMMIT} has no runbook/ crate")), "{out}");
    fake_build(&d, 8, true);
    // The login's base variables pass; an SDC_ variable, a token, a Claude directory do not.
    let base = [("HOME", "/srv/someone"), ("PATH", "/usr/bin:/bin"), ("USER", "someone"), ("LANG", "C"), ("XDG_RUNTIME_DIR", "/run/user/1000"),
        ("DBUS_SESSION_BUS_ADDRESS", "unix:path=/run/user/1000/bus")];
    let polluted = [("SDC_RUNS_DIR", "x"), ("SDC_LLM_ADAPTER", "y"), ("SDC_PRINCIPAL", "p"), ("ANTHROPIC_API_KEY", "k"), ("CLAUDE_CONFIG_DIR", "c"),
        ("CLAUDE_CODE_OAUTH_TOKEN", "t"), ("LOOM_CASE", "l")];
    let env: Vec<(&str, &str)> = base.iter().chain(polluted.iter()).copied().collect();
    let (code, out) = experiment_env(&args, &d, &env);
    assert_eq!(code, 0, "{out}");
    let printed: BTreeSet<String> = out.lines().filter_map(|l| l.strip_prefix("  env ")).filter_map(|l| l.split_once('=')).map(|(k, _)| k.to_string()).collect();
    let want: BTreeSet<String> = base.iter().map(|(k, _)| k.to_string()).collect();
    assert_eq!(printed, want, "{out}");
    assert!(out.contains("  env HOME=/srv/someone") && out.contains("  env LANG=C"), "{out}");
    for (k, v) in polluted {
        assert!(!out.contains(&format!("{k}=")) && !out.contains(&format!("={v}\n")), "{k} leaked: {out}");
    }
    assert!(!out.contains("SDC_"), "a loom run is given no SDC_ variable: {out}");
    // A variable the login does not have is not invented.
    let (_, out) = experiment_env(&args, &d, &[("PATH", "/usr/bin")]);
    let lines: BTreeSet<&str> = out.lines().filter(|l| l.starts_with("  env ")).collect();
    assert_eq!(lines, BTreeSet::from(["  env PATH=/usr/bin"]), "{out}");
}

#[test]
fn the_login_file_is_named_by_its_setting_and_its_path_is_never_printed() {
    let d = scratch("marker-named");
    let builds = fake_build(&d, 8, true);
    let mut f: Vec<_> = fixed().into_iter().filter(|(k, _)| k != "max_llm_calls").collect();
    f.push(("loom.login_file".into(), "/opt/marker-login-9f3a/credentials.json".into()));
    write_record(&d, &record_with(&f, COMMIT, "planned", ("", "")));
    let (code, out) = experiment(&["check", "lx"], &d);
    assert_eq!(code, 0, "{out}");
    let (code, out) = experiment(&["run", "lx", "--print", "--builds-dir", builds.to_str().unwrap()], &d);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("marker-login-9f3a"), "the login path is printed: {out}");
    assert!(out.contains("login: loom.login_file is set (named in the settings file; the launcher never reads it)"), "{out}");
    assert!(out.contains("\"login_file\":\"<loom.login_file>\""), "{out}");
    let desc: Vec<&str> = out.lines().filter(|l| l.starts_with("  description: ")).collect();
    assert_eq!(desc, ["  description: sdc experiment lx: run lx.low.1", "  description: sdc experiment lx: run lx.high.1"], "{out}");
    // Without the setting there is no login line and no member.
    write_record(&d, &example(COMMIT));
    let (_, out) = experiment(&["run", "lx", "--print", "--builds-dir", builds.to_str().unwrap()], &d);
    assert!(!out.contains("login") && !out.contains("login_file"), "{out}");
}

fn jsonl(path: &Path, lines: &[Value]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
}

/// A run directory holding only the members of the contract table (docs/workflow-metrics.md,
/// "Run directories of a loom run"): the root case's `events.jsonl`, `llm-calls.jsonl`,
/// `run-settings.jsonl`.
fn minimal_run(run: &Path, outcome: &str, cap: &str) {
    jsonl(&run.join("store/IDEA-1/events.jsonl"), &[
        json!({"event": "opened", "protocol": {"id": "sdc", "revision": 8}}),
        json!({"event": "terminated", "outcome": outcome}),
    ]);
    jsonl(&run.join("llm-calls.jsonl"), &[
        json!({"case": "IDEA-1", "action": "idea.dissect", "cost_usd": 0.25, "model": "claude-sonnet-5-5"}),
        json!({"case": "IDEA-1", "action": "spec.author", "cost_usd": 0.5, "model": "claude-sonnet-5-5"}),
    ]);
    jsonl(&run.join("run-settings.jsonl"), &[json!({"format": "sdc.run-settings/1", "at": "2026-11-22T09:00:00Z",
        "loom": {"sha256": LOOM_SHA, "confinement": "local", "cap_micro_usd": cap.parse::<u64>().unwrap()},
        "models": {"writer": {"model": "claude-sonnet-5-5"}, "implement": {"model": "claude-sonnet-5-5"},
            "review": {"model": "claude-sonnet-5-5"}, "other": {"model": "claude-sonnet-5-5"}},
        "decider": {"model": "claude-opus-5-5"}, "max_llm_calls": 100, "max_wall_seconds": 3600})]);
}

#[test]
fn report_of_a_loom_record_reads_a_run_directory_that_holds_only_the_contract_members() {
    let d = scratch("report");
    minimal_run(&d.join("runs/lx.low.1"), "operating", "5000000");
    let f: Vec<_> = fixed().into_iter().filter(|(k, _)| k != "max_llm_calls").collect();
    write_record(&d, &record_with(&f, COMMIT, "running", ("lx.low.1", "")));
    // The settings the run recorded are the record's; loom.bin is never recorded.
    let (code, out) = experiment(&["check", "lx", "--runs-dir", "runs"], &d);
    assert_eq!(code, 0, "{out}");
    let (code, out) = experiment(&["report", "lx", "--runs-dir", "runs"], &d);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("low: listed 1, ended 1, clean passes 1"), "{out}");
    let s: Value = serde_json::from_str(&fs::read_to_string(d.join("experiments/lx/runs/lx.low.1.json")).unwrap()).unwrap();
    assert_eq!(s["metrics"]["delivered"], true);
    assert_eq!(s["metrics"]["closed"], true);
    assert_eq!(s["metrics"]["clean"], true);
    assert_eq!(s["metrics"]["finished"], true);
    assert_eq!(s["metrics"]["cost_usd"].as_f64(), Some(0.75));
    assert_eq!(s["metrics"]["llm_calls"], 2);
    assert_eq!(s["settings"]["loom.confinement"], "local");
    assert_eq!(s["settings"]["decider.model"], "claude-opus-5-5");
    // A run that did not say it closed is open and has no summary; a tree outcome that is not
    // `operating` is a delivery only with status.json saying so.
    minimal_run(&d.join("runs/lx.low.1"), "refused", "5000000");
    let (code, out) = experiment(&["report", "lx", "--runs-dir", "runs"], &d);
    assert_eq!(code, 0, "{out}");
    let s: Value = serde_json::from_str(&fs::read_to_string(d.join("experiments/lx/runs/lx.low.1.json")).unwrap()).unwrap();
    assert_eq!(s["metrics"]["delivered"], false);
    assert_eq!(s["metrics"]["clean"], true);
    fs::write(d.join("runs/lx.low.1/status.json"), json!({"state": "closed", "outcome": "refused", "delivered": true}).to_string()).unwrap();
    let (code, _) = experiment(&["report", "lx", "--runs-dir", "runs"], &d);
    assert_eq!(code, 0);
    let s: Value = serde_json::from_str(&fs::read_to_string(d.join("experiments/lx/runs/lx.low.1.json")).unwrap()).unwrap();
    assert_eq!(s["metrics"]["delivered"], true);
    // A run that recorded another cap than its variant says is refused by `check`.
    // (`check` reads a run's summary before its directory, so the summary goes first.)
    fs::remove_file(d.join("experiments/lx/runs/lx.low.1.json")).unwrap();
    minimal_run(&d.join("runs/lx.low.1"), "operating", "9000000");
    let (code, out) = experiment(&["check", "lx", "--runs-dir", "runs"], &d);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("run lx.low.1 has loom.cap_micro_usd = 9000000, the record says 5000000"), "{out}");
    // Without the root case's event log there is nothing to read.
    fs::remove_file(d.join("runs/lx.low.1/store/IDEA-1/events.jsonl")).unwrap();
    let (code, out) = experiment(&["report", "lx", "--runs-dir", "runs"], &d);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("events.jsonl: no such file"), "{out}");
}

#[test]
fn status_and_stop_map_the_unit_names_and_stop_writes_the_stop_record_before_the_unit_is_stopped() {
    let d = scratch("stop");
    fs::create_dir_all(d.join("runs/lx.low.1")).unwrap();
    write_record(&d, &record_with(&fixed().into_iter().filter(|(k, _)| k != "max_llm_calls").collect::<Vec<_>>(), COMMIT, "running", ("lx.low.1", "")));
    let (code, out) = experiment(&["status", "lx", "--runs-dir", "runs"], &d);
    assert_eq!(code, 0, "{out}");
    let row = |name: &str| out.lines().find(|l| l.starts_with(name)).unwrap_or_else(|| panic!("no row {name} in {out}")).to_string();
    // Each planned run maps to a unit (the state column is not `-`); the names are those `run --print` shows.
    let state = |name: &str| row(name).split_whitespace().nth(2).unwrap_or("").to_string();
    assert!(row("lx.low.1").contains("low") && state("lx.low.1") != "-", "{out}");
    assert!(row("lx.high.1").contains("high") && state("lx.high.1") != "-" && row("lx.high.1").contains("not started"), "{out}");
    let (code, out) = experiment(&["stop", "lx", "--reason", "the cap was wrong", "--run", "lx.low.1", "--runs-dir", "runs", "--builds-dir", "none"], &d);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("run-trial.sh"), "a loom run is not stopped with run-trial.sh: {out}");
    let stop: Value = serde_json::from_str(&fs::read_to_string(d.join("runs/lx.low.1/run-stopped.json")).unwrap()).unwrap();
    assert_eq!(stop["format"], "sdc.run-stopped/1");
    assert_eq!(stop["reason"], "the cap was wrong");
    assert_eq!(stop["by"], "agent:sdc-experiment");
    assert!(stop["at"].as_str().is_some_and(|t| t.ends_with('Z')), "{stop}");
    // A second stop does not overwrite the record.
    let (code, out) = experiment(&["stop", "lx", "--reason", "again", "--run", "lx.low.1", "--runs-dir", "runs", "--builds-dir", "none"], &d);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("already stopped"), "{out}");
    let stop: Value = serde_json::from_str(&fs::read_to_string(d.join("runs/lx.low.1/run-stopped.json")).unwrap()).unwrap();
    assert_eq!(stop["reason"], "the cap was wrong");
}
