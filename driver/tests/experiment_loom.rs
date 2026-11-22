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
/// The file of the `runbook` crate that makes `runbook run` run in process (`sdc-experiment` reads
/// it as the sign of a build that has it).
const IN_PROCESS_RUN: &str = "runbook/src/loom_run/here.rs";
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
        ("decider.kind", "model"),
        ("decider.model", "claude-opus-5-5"),
        ("decider.escalate", "final"),
        ("decider.max_decider_calls", "40"),
        ("decider.adapter", "claude-cli"),
        ("autonomy", "A5"),
        ("adapter", "metaharness"),
        ("review.adapter", "claude-cli"),
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
        fs::create_dir_all(b.join("runbook/src/loom_run")).unwrap();
        fs::write(b.join(IN_PROCESS_RUN), "// runbook run in process\n").unwrap();
    }
    d.join("b")
}

#[test]
fn a_loom_record_is_checked_and_a_missing_or_old_setting_is_refused_by_name() {
    let d = scratch("check");
    write_record(&d, &example("null"));
    let (code, out) = experiment(&["check", "lx"], &d);
    assert_eq!(code, 0, "a planned loom record without runs is accepted: {out}");
    // Every required setting, taken out, is named; an optional one, taken out, is not missed.
    let optional = ["decider.escalate", "decider.max_decider_calls", "decider.adapter", "adapter", "review.adapter"];
    for (k, _) in fixed().iter().filter(|(k, _)| k != "max_llm_calls") {
        let f: Vec<_> = fixed().into_iter().filter(|(x, _)| x != k && x != "max_llm_calls").collect();
        write_record(&d, &record_with(&f, "null", "planned", ("", "")));
        let (code, out) = experiment(&["check", "lx"], &d);
        if optional.contains(&k.as_str()) {
            assert_eq!(code, 0, "{k} is optional: {out}");
        } else {
            assert_eq!(code, 1, "{k}: {out}");
            assert!(out.contains(&format!("the settings do not set {k}")), "{k}: {out}");
        }
    }
    // The decider's kind and the autonomy level, the decider's limits and the adapters are settings.
    let with = |extra: &[(&str, &str)], without: &[&str]| -> (i32, String) {
        let mut f: Vec<_> = fixed().into_iter().filter(|(x, _)| x != "max_llm_calls" && !without.contains(&x.as_str())).collect();
        f.extend(extra.iter().map(|(k, v)| (k.to_string(), v.to_string())));
        write_record(&d, &record_with(&f, "null", "planned", ("", "")));
        experiment(&["check", "lx"], &d)
    };
    let (code, out) = with(&[], &[]);
    assert_eq!(code, 0, "the whole strong variant is accepted: {out}");
    for (extra, without, want) in [
        (vec![("decider.kind", "haiku")], vec!["decider.kind"], "decider.kind must be model or rule"),
        (vec![("autonomy", "A9")], vec!["autonomy"], "autonomy must be one of A0 to A5"),
        (vec![("decider.escalate", "maybe")], vec!["decider.escalate"], "decider.escalate must be final or stop"),
        (vec![("adapter", "dry")], vec!["adapter"], "adapter must be metaharness or claude-cli"),
        (vec![("decider.adapter", "dry")], vec!["decider.adapter"], "decider.adapter must be metaharness or claude-cli"),
        (vec![("review.adapter", "metaharness")], vec!["review.adapter"], "review.adapter must be claude-cli"),
        (vec![("decider.max_decider_calls", "0")], vec!["decider.max_decider_calls"], "decider.max_decider_calls must be a whole number of at least 1"),
        (vec![("decider.verdict", "approve")], vec![], "decider.verdict is a setting of a rule decider, not of kind model"),
        (vec![], vec!["decider.model"], "the settings do not set decider.model"),
    ] {
        let (code, out) = with(&extra, &without);
        assert_eq!(code, 1, "{want}: {out}");
        assert!(out.contains(want), "{want}: {out}");
    }
    // A rule decider sets a verdict and nothing of a model decider.
    let model_only = ["decider.model", "decider.escalate", "decider.max_decider_calls", "decider.adapter"];
    let (code, out) = with(&[("decider.kind", "rule"), ("decider.verdict", "approve")], &["decider.kind", "decider.model", "decider.escalate", "decider.max_decider_calls", "decider.adapter"]);
    assert_eq!(code, 0, "{out}");
    let (code, out) = with(&[("decider.kind", "rule")], &["decider.kind", "decider.model", "decider.escalate", "decider.max_decider_calls", "decider.adapter"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("the settings do not set decider.verdict"), "{out}");
    let (code, out) = with(&[("decider.kind", "rule"), ("decider.verdict", "reject")], &["decider.kind"]);
    assert_eq!(code, 1, "{out}");
    for k in model_only {
        assert!(out.contains(&format!("{k} is a setting of a model decider, not of kind rule")), "{k}: {out}");
    }
    // A setting that has no counterpart under Loom stays refused, and the refusal names the key.
    for (k, v) in [("implement_mode", "x"), ("review_mode", "x"), ("efforts.writer.effort", "low"), ("flags.dry", "true"), ("flags.recorded", "true"),
        ("conformance", "observe"), ("rule_approvals", "x"), ("free.model", "m")] {
        let mut f = fixed();
        f.push((k.to_string(), v.to_string()));
        write_record(&d, &record_with(&f, "null", "planned", ("", "")));
        let (code, out) = experiment(&["check", "lx"], &d);
        assert_eq!(code, 1, "{k}: {out}");
        assert!(out.contains(&format!("{k} is refused for a loom variant")), "{k}: {out}");
        // The settings that wait for a port of `runbook run` say so.
        if ["flags.dry", "flags.recorded", "conformance"].contains(&k) {
            assert!(out.contains("`runbook run` "), "{k}: {out}");
        }
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
    // A seed input is the bug-fix case that `runbook run --bugfix` opens: it needs its directory.
    write_record(&d, &example("null").replace("kind: idea", "kind: seed"));
    let (code, out) = experiment(&["check", "lx"], &d);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("a seed needs `seed`, its directory relative to the tree") && !out.contains("a loom variant starts from"), "{out}");
    write_record(&d, &example("null").replace("kind: idea", "kind: seed, seed: driver/examples/sdc/fixtures/bugfix-pager"));
    let (code, out) = experiment(&["check", "lx"], &d);
    assert_eq!(code, 0, "{out}");
}

/// A seed input starts `runbook run sdc-bugfix --bugfix <seed directory of the tree>`: the root
/// protocol is the bug-fix one, the case is `BUG-1`, the intake is the bug report.
#[test]
fn a_seed_input_starts_the_bug_fix_runbook_with_bugfix_and_its_root_protocol() {
    let d = scratch("seed-input");
    let builds = fake_build(&d, 11, true);
    fs::write(builds.join(COMMIT).join("protocol/sdc-bugfix.protocol.yaml"), "format: protocol/1\nprotocol:\n  id: sdc.bugfix\n  revision: 1\n").unwrap();
    write_record(&d, &example(COMMIT).replace("kind: idea", "kind: seed, seed: driver/examples/sdc/fixtures/bugfix-pager"));
    let (code, out) = experiment(&["run", "lx", "--print", "--builds-dir", builds.to_str().unwrap(), "--runs-dir", "runs"], &d);
    assert_eq!(code, 0, "{out}");
    let build = builds.join(COMMIT);
    let launch = builds.parent().unwrap().join("launch").join("lx");
    let runs = d.join("runs");
    let want = format!(
        "  {} run sdc-bugfix --run lx.low.1 --runs-dir {} --loom /opt/loom/b10x-loom --loom-sha256 {LOOM_SHA} --bindings {} --protocols {} \
         --root-protocol sdc-bugfix@1 --case BUG-1 --bugfix {} --intake {} --confinement local --settings {} --spend-ledger {} --cap-micro-usd 5000000 \
         --max-llm-calls 100 --max-wall-seconds 3600",
        build.join("runbook/target/debug/runbook").display(), runs.display(), build.join("bindings").display(), build.join("protocol").display(),
        build.join("driver/examples/sdc/fixtures/bugfix-pager").display(), launch.join("lx.low.1").join("idea.yaml").display(),
        launch.join("lx.low.1").join("settings.json").display(), runs.join("lx.loom-spend.json").display());
    assert!(out.lines().any(|l| l == want), "no line\n{want}\nin\n{out}");
    // a build without the bug-fix root protocol is refused for that file
    fs::remove_file(build.join("protocol/sdc-bugfix.protocol.yaml")).unwrap();
    let (code, out) = experiment(&["run", "lx", "--print", "--builds-dir", builds.to_str().unwrap()], &d);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("has no protocol/sdc-bugfix.protocol.yaml"), "{out}");
}

/// A planned record of the old driver: no `runner`, the settings of its launch table.
fn old_driver_record(status: &str) -> String {
    format!(
        r#"format: sdc.experiment/1
id: lx
kind: prospective
status: {status}
created: 2026-11-21
title: A record of the old driver
purpose: Whether the launcher still starts a run of the old driver.
inputs:
- {{name: invoices, kind: idea, text: {IDEA}, sha256: {}}}
tree_commit: {COMMIT}
fixed:
  models.writer.model: claude-sonnet-5-5
  models.implement.model: claude-sonnet-5-5
  models.review.model: claude-sonnet-5-5
  models.other.model: claude-sonnet-5-5
  decider.kind: haiku
  decider.model: claude-opus-5-5
  decider.escalate: final
  decider.max_decider_calls: 40
  adapter: metaharness
  max_llm_calls: 150
  autonomy: A5
variants:
- {{id: old, settings: {{}}, runs: []}}
runs_per_variant: 1
pass_rule: clean-delivery
decision_rule: {{rule: descriptive, text: the record describes its runs and decides nothing}}
"#,
        sha(IDEA)
    )
}

#[test]
fn a_record_of_the_old_driver_is_still_checked_and_is_refused_at_run_with_one_line() {
    let d = scratch("old-driver");
    let builds = fake_build(&d, 8, true);
    let units = d.join("units.log");
    let tools = d.join("tools");
    fs::create_dir_all(&tools).unwrap();
    // A launch that got past the refusal would call `systemd-run`: this one writes it down.
    executable(&tools.join("systemd-run"), &format!("#!/bin/sh\necho \"$@\" >> {}\n", units.display()));
    let path = format!("{}:/usr/bin:/bin", tools.display());
    for runner_line in ["", "runner: sdc, "] {
        let text = old_driver_record("planned").replace("- {id: old, ", &format!("- {{id: old, {runner_line}"));
        write_record(&d, &text);
        let (code, out) = experiment(&["check", "lx"], &d);
        assert_eq!(code, 0, "check still reads a record of the old driver ({runner_line:?}): {out}");
        let runs = d.join("runs");
        let args = ["run", "lx", "--builds-dir", builds.to_str().unwrap(), "--runs-dir", runs.to_str().unwrap()];
        let (code, out) = experiment_env(&args, &d, &[("PATH", &path), ("HOME", d.to_str().unwrap())]);
        assert_eq!(code, 1, "{out}");
        assert_eq!(out.lines().count(), 1, "one line: {out}");
        assert!(out.contains("the old driver, which is gone") && out.contains("runner: loom"), "{out}");
        assert!(!out.contains("run-trial.sh") && !out.contains("sdc-drive"), "{out}");
        assert!(!units.exists(), "no unit was started");
        assert!(!d.join("runs").exists(), "no run directory was made");
    }
    // `--print` takes the same path: nothing is printed for a record of the old driver.
    let (code, out) = experiment(&["run", "lx", "--print", "--builds-dir", builds.to_str().unwrap()], &d);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("the old driver, which is gone"), "{out}");
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
            "  {} run sdc --run {name} --runs-dir {} --loom /opt/loom/b10x-loom --loom-sha256 {LOOM_SHA} --bindings {} --protocols {} \
             --root-protocol sdc@11 --case IDEA-1 --intake {} --confinement local --settings {} --spend-ledger {} --cap-micro-usd 5000000 \
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
    assert!(out.contains("\"decider\":{\"adapter\":\"claude-cli\",\"escalate\":\"final\",\"kind\":\"model\",\"max_decider_calls\":40,\"model\":\"claude-opus-5-5\",\"without_claims\":true}"), "{out}");
    assert!(out.contains("\"autonomy\":\"A5\"") && out.contains("\"adapter\":\"metaharness\"") && out.contains("\"review\":{\"adapter\":\"claude-cli\"}"), "{out}");
    assert!(!out.contains("principal"), "the principal of the decider's answers is the runner's to derive: {out}");
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
    let old = git(&["rev-parse", "HEAD"]);
    fs::create_dir_all(repo.join("runbook/src/loom_run")).unwrap();
    fs::write(repo.join(IN_PROCESS_RUN), "// runbook run in process\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "--quiet", "-m", "in process"]);
    let with = git(&["rev-parse", "HEAD"]);
    fs::create_dir_all(repo.join("experiments")).unwrap();
    let builds = d.join("nobuilds");
    let args = ["run", "lx", "--print", "--builds-dir", builds.to_str().unwrap()];
    fs::write(repo.join("experiments/lx.yaml"), example(&without)).unwrap();
    let (code, out) = experiment(&args, &repo);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains(&format!("refused: tree_commit {without} has no runbook/ crate")), "{out}");
    // A build whose `runbook` has no `run` in process (only `loom-run`) is refused in one line.
    fs::write(repo.join("experiments/lx.yaml"), example(&old)).unwrap();
    let (code, out) = experiment(&args, &repo);
    assert_eq!(code, 1, "{out}");
    assert_eq!(out.trim_end().lines().count(), 1, "one line: {out}");
    assert!(out.contains(&format!("refused: tree_commit {old} has a runbook/ crate with no in-process `runbook run`")), "{out}");
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
    // A variable the login does not have is not invented; PATH always holds the build's tools.
    let (_, out) = experiment_env(&args, &d, &[("PATH", "/usr/bin")]);
    let lines: BTreeSet<&str> = out.lines().filter(|l| l.starts_with("  env ")).collect();
    let tools = format!("  env PATH={}:/usr/bin", builds.join(COMMIT).join("driver/target/debug").display());
    assert_eq!(lines, BTreeSet::from([tools.as_str()]), "{out}");
}

#[test]
fn the_unit_finds_the_builds_binaries_and_the_tools_of_the_decider_on_its_path() {
    // sdc-reply, sdc-findings and sdc-bugfix are found through PATH by the model nodes and the
    // decider: the build's driver/target/debug comes first. jq and yq come from the login; rsync is not asked for.
    let d = scratch("path");
    let builds = fake_build(&d, 8, true);
    write_record(&d, &example(COMMIT));
    let bin = builds.join(COMMIT).join("driver/target/debug");
    let (code, out) = experiment_env(&["run", "lx", "--print", "--builds-dir", builds.to_str().unwrap()], &d, &[("PATH", "/nonexistent")]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains(&format!("  env PATH={}:/nonexistent\n", bin.display())), "{out}");
    assert!(out.contains("  missing on PATH: jq, yq\n"), "{out}");
    assert!(out.contains("  needs on PATH: jq, yq (the shell programs of the bindings); sdc-reply, sdc-findings and sdc-bugfix from "), "{out}");
    // The tools are found when a directory of PATH holds them as executable files.
    let tools = d.join("tools");
    fs::create_dir_all(&tools).unwrap();
    for t in ["jq", "yq"] {
        fs::write(tools.join(t), "#!/bin/sh\n").unwrap();
        fs::set_permissions(tools.join(t), std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    }
    fs::write(tools.join("yq"), "not executable\n").unwrap();
    fs::set_permissions(tools.join("yq"), std::os::unix::fs::PermissionsExt::from_mode(0o644)).unwrap();
    let (_, out) = experiment_env(&["run", "lx", "--print", "--builds-dir", builds.to_str().unwrap()], &d, &[("PATH", tools.to_str().unwrap())]);
    assert!(out.contains("  missing on PATH: yq\n"), "an executable file is needed: {out}");
    // A launch without them is refused before anything is built or started.
    let (code, out) = experiment_env(&["run", "lx", "--builds-dir", builds.to_str().unwrap()], &d, &[("PATH", "/nonexistent")]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("refused: jq, yq not on PATH"), "{out}");
}

#[test]
fn the_settings_file_is_the_committed_fixture_the_runner_reads_and_a_rule_decider_writes_no_model() {
    // ONE copy of the file `sdc-experiment run` writes for the record below is committed under
    // runbook/tests/; the runner's tests (runbook/tests/loom_run/settings.rs) read the same file and
    // run it. This test keeps the writer equal to it. The login is printed as its name.
    let d = scratch("fixture");
    let builds = fake_build(&d, 8, true);
    let mut f: Vec<_> = fixed().into_iter().filter(|(k, _)| k != "max_llm_calls").collect();
    f.push(("loom.login_file".into(), "/opt/marker-login-9f3a/credentials.json".into()));
    write_record(&d, &record_with(&f, COMMIT, "planned", ("", "")));
    let (code, out) = experiment(&["run", "lx", "--print", "--builds-dir", builds.to_str().unwrap()], &d);
    assert_eq!(code, 0, "{out}");
    let printed = out.lines().filter_map(|l| l.strip_prefix("  settings {").map(|r| format!("{{{r}"))).next().expect("a settings line");
    let printed: Value = serde_json::from_str(&printed).unwrap();
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../runbook/tests/fixtures/loom-run-settings.json");
    let committed: Value = serde_json::from_str(&fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    assert_eq!(printed, committed, "the file sdc-experiment writes and runbook/tests/fixtures/loom-run-settings.json differ");
    // A rule decider: kind and verdict, nothing a model decider has, no claims member.
    let mut f: Vec<_> = fixed().into_iter().filter(|(k, _)| k != "max_llm_calls" && !k.starts_with("decider.") && k != "review.adapter" && k != "adapter").collect();
    f.extend([("decider.kind", "rule"), ("decider.verdict", "approve")].map(|(k, v)| (k.to_string(), v.to_string())));
    write_record(&d, &record_with(&f, COMMIT, "planned", ("", "")));
    let (code, out) = experiment(&["run", "lx", "--print", "--builds-dir", builds.to_str().unwrap()], &d);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("\"decider\":{\"kind\":\"rule\",\"verdict\":\"approve\"}"), "{out}");
    assert!(!out.contains("without_claims") && !out.contains("\"adapter\""), "{out}");
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

#[test]
fn a_login_file_named_at_start_reaches_the_run_and_no_record_holds_it() {
    let d = scratch("marker-start");
    let builds = fake_build(&d, 8, true);
    write_record(&d, &example(COMMIT));
    let before = fs::read_to_string(d.join("experiments/lx.yaml")).unwrap();
    let args = ["run", "lx", "--print", "--login-file", "/opt/marker-login-9f3a/credentials.json", "--builds-dir", builds.to_str().unwrap()];
    let (code, out) = experiment(&args, &d);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("marker-login-9f3a"), "the login path is printed: {out}");
    assert_eq!(out.matches("login: loom.login_file is set").count(), 2, "{out}");
    assert_eq!(out.matches("\"login_file\":\"<loom.login_file>\"").count(), 2, "{out}");
    assert_eq!(fs::read_to_string(d.join("experiments/lx.yaml")).unwrap(), before, "the record is untouched");
    // A path that is neither absolute nor ~/ is refused as a usage error.
    let (code, out) = experiment(&["run", "lx", "--print", "--login-file", "creds.json", "--builds-dir", builds.to_str().unwrap()], &d);
    assert_eq!(code, 64, "{out}");
    assert!(out.contains("--login-file needs an absolute path"), "{out}");
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
        "decider": {"kind": "model", "model": "claude-opus-5-5", "without_claims": true, "escalate": "final", "max_decider_calls": 40, "adapter": "claude-cli"},
        "autonomy": "A5", "adapter": "metaharness", "review": {"adapter": "claude-cli"}, "max_llm_calls": 100, "max_wall_seconds": 3600})]);
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

/// The `fixed` settings of a record that runs Loom as a library: no binary, no digest, and
/// `loom.in_process: 'true'`.
fn in_process_fixed() -> Vec<(String, String)> {
    let mut f: Vec<_> = fixed().into_iter().filter(|(k, _)| k != "loom.bin" && k != "loom.sha256" && k != "max_llm_calls").collect();
    f.push(("loom.in_process".into(), "'true'".into()));
    f
}

#[test]
fn a_loom_record_may_run_loom_as_a_library_and_names_the_way_to_loom_once() {
    let d = scratch("in-process");
    let builds = fake_build(&d, 11, true);
    // no loom.bin and no loom.sha256: the record is accepted
    write_record(&d, &record_with(&in_process_fixed(), COMMIT, "planned", ("", "")));
    let (code, out) = experiment(&["check", "lx"], &d);
    assert_eq!(code, 0, "a record that runs the library needs no binary and no digest: {out}");
    // naming both forms is refused, by the key that is one too many
    for k in ["loom.bin", "loom.sha256"] {
        let mut f = in_process_fixed();
        f.push((k.into(), fixed().into_iter().find(|(x, _)| x == k).unwrap().1));
        write_record(&d, &record_with(&f, COMMIT, "planned", ("", "")));
        let (code, out) = experiment(&["check", "lx"], &d);
        assert_eq!(code, 1, "{k}: {out}");
        assert!(out.contains(&format!("{k} and loom.in_process both name the way to Loom")), "{k}: {out}");
    }
    // the only value is 'true'; leave the setting out to run the binary
    let f: Vec<_> = in_process_fixed().into_iter().filter(|(k, _)| k != "loom.in_process").chain([("loom.in_process".to_string(), "'false'".to_string())]).collect();
    write_record(&d, &record_with(&f, COMMIT, "planned", ("", "")));
    let (code, out) = experiment(&["check", "lx"], &d);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("loom.in_process must be 'true'"), "{out}");
    // neither form: both keys are named as missing
    let f: Vec<_> = in_process_fixed().into_iter().filter(|(k, _)| k != "loom.in_process").collect();
    write_record(&d, &record_with(&f, COMMIT, "planned", ("", "")));
    let (code, out) = experiment(&["check", "lx"], &d);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("do not set loom.bin") && out.contains("do not set loom.sha256"), "{out}");
    // run --print shows the line: no --loom and --loom-sha256, so Loom is a library of the build
    write_record(&d, &record_with(&in_process_fixed(), COMMIT, "planned", ("", "")));
    let (code, out) = experiment(&["run", "lx", "--print", "--builds-dir", builds.to_str().unwrap(), "--runs-dir", "runs"], &d);
    assert_eq!(code, 0, "{out}");
    let build = builds.join(COMMIT);
    let runs = d.join("runs");
    let launch = builds.parent().unwrap().join("launch").join("lx");
    for (name, calls) in [("lx.low.1", "100"), ("lx.high.1", "200")] {
        let want = format!(
            "  {} run sdc --run {name} --runs-dir {} --bindings {} --protocols {} \
             --root-protocol sdc@11 --case IDEA-1 --intake {} --confinement local --settings {} --spend-ledger {} --cap-micro-usd 5000000 \
             --max-llm-calls {calls} --max-wall-seconds 3600",
            build.join("runbook/target/debug/runbook").display(), runs.display(), build.join("bindings").display(), build.join("protocol").display(),
            launch.join(name).join("idea.yaml").display(), launch.join(name).join("settings.json").display(), runs.join("lx.loom-spend.json").display());
        assert!(out.lines().any(|l| l == want), "no line\n{want}\nin\n{out}");
    }
    assert!(!out.contains("--loom ") && !out.contains("--loom-sha256"), "{out}");
    // the settings file the run reads says in_process and holds no digest
    let printed = out.lines().filter_map(|l| l.strip_prefix("  settings {").map(|r| format!("{{{r}"))).next().expect("a settings line");
    let printed: Value = serde_json::from_str(&printed).unwrap();
    assert_eq!(printed["loom"], json!({"in_process": true, "confinement": "local", "cap_micro_usd": 5000000}), "{printed}");
}

#[test]
fn check_compares_a_library_run_with_the_record_that_names_the_library() {
    let d = scratch("in-process-runs");
    minimal_run(&d.join("runs/lx.low.1"), "operating", "5000000");
    let record = || {
        let f: Vec<_> = in_process_fixed();
        write_record(&d, &record_with(&f, COMMIT, "running", ("lx.low.1", "")));
    };
    record();
    // a run that was made by the binary does not match a record that says library
    let (code, out) = experiment(&["check", "lx", "--runs-dir", "runs"], &d);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("run lx.low.1 has loom.in_process = not recorded, the record says true"), "{out}");
    // a run that recorded the library and its commit does
    let file = d.join("runs/lx.low.1/run-settings.jsonl");
    let text = fs::read_to_string(&file).unwrap();
    assert!(text.contains(&format!("\"sha256\":\"{LOOM_SHA}\"")), "{text}");
    fs::write(&file, text.replace(&format!("\"sha256\":\"{LOOM_SHA}\""), &format!("\"in_process\":true,\"commit\":\"{COMMIT}\""))).unwrap();
    let (code, out) = experiment(&["check", "lx", "--runs-dir", "runs"], &d);
    assert_eq!(code, 0, "{out}");
}

#[test]
fn status_names_a_run_directory_it_cannot_read_instead_of_saying_not_started() {
    // Found by the chain `run --print`, `runbook run`, `status`: a run directory that holds
    // `outcome.json` (the runner ended) but no root event log was shown as "not started".
    let d = scratch("status-unreadable");
    let f: Vec<_> = fixed().into_iter().filter(|(k, _)| k != "max_llm_calls").collect();
    write_record(&d, &record_with(&f, COMMIT, "running", ("lx.low.1", "")));
    fs::create_dir_all(d.join("runs/lx.low.1")).unwrap();
    let (_, out) = experiment(&["status", "lx", "--runs-dir", "runs"], &d);
    let row = out.lines().find(|l| l.starts_with("lx.low.1")).unwrap().to_string();
    assert!(row.ends_with("not started"), "a directory the runner has just made is not started: {row}");
    fs::write(d.join("runs/lx.low.1/outcome.json"), "{\"outcome\":\"ended\"}\n").unwrap();
    let (code, out) = experiment(&["status", "lx", "--runs-dir", "runs"], &d);
    assert_eq!(code, 0, "{out}");
    let row = out.lines().find(|l| l.starts_with("lx.low.1")).unwrap().to_string();
    assert!(row.contains("unreadable:") && row.contains("events.jsonl") && !row.contains("not started"), "{row}");
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
    let (code, out) = experiment(&["stop", "lx", "--reason", "the cap was wrong", "--run", "lx.low.1", "--runs-dir", "runs"], &d);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("run-trial.sh"), "a loom run is not stopped with run-trial.sh: {out}");
    let stop: Value = serde_json::from_str(&fs::read_to_string(d.join("runs/lx.low.1/run-stopped.json")).unwrap()).unwrap();
    assert_eq!(stop["format"], "sdc.run-stopped/1");
    assert_eq!(stop["reason"], "the cap was wrong");
    assert_eq!(stop["by"], "agent:sdc-experiment");
    assert!(stop["at"].as_str().is_some_and(|t| t.ends_with('Z')), "{stop}");
    // A second stop does not overwrite the record.
    let (code, out) = experiment(&["stop", "lx", "--reason", "again", "--run", "lx.low.1", "--runs-dir", "runs"], &d);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("already stopped"), "{out}");
    let stop: Value = serde_json::from_str(&fs::read_to_string(d.join("runs/lx.low.1/run-stopped.json")).unwrap()).unwrap();
    assert_eq!(stop["reason"], "the cap was wrong");
}

#[test]
fn a_launch_is_refused_when_a_program_of_the_answers_file_is_not_on_the_path() {
    let d = scratch("answers-path");
    let builds = fake_build(&d, 8, true);
    write_record(&d, &example(COMMIT));
    let build = builds.join(COMMIT);
    fs::create_dir_all(build.join("bindings/tools")).unwrap();
    fs::write(
        build.join("bindings/tools/answers.yaml"),
        "format: runbook.loom-run-answers/1\nanswers:\n- action: scope.answer\n  on: ApproveAction\n  requests: [sdc-answers, requests, questions]\n  apply: [sdc-answers, apply, question, '{id}', '{answer}', '{principal}']\n  then: [tools/sdc-tool.sh, scope-answer, '{principal}']\n",
    )
    .unwrap();
    let tools = d.join("tools");
    fs::create_dir_all(&tools).unwrap();
    for t in ["jq", "yq"] {
        fs::write(tools.join(t), "#!/bin/sh\n").unwrap();
        fs::set_permissions(tools.join(t), std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    }
    let path = tools.to_str().unwrap();
    let run = ["run", "lx", "--builds-dir", builds.to_str().unwrap()];
    let (code, out) = experiment_env(&run, &d, &[("PATH", path)]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("refused: sdc-answers not on PATH: `bindings/tools/answers.yaml`"), "{out}");
    fs::write(tools.join("sdc-answers"), "#!/bin/sh\n").unwrap();
    fs::set_permissions(tools.join("sdc-answers"), std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let (_, out) = experiment_env(&run, &d, &[("PATH", path)]);
    assert!(!out.contains("sdc-answers not on PATH"), "a PATH with the program passes the check: {out}");
    fs::remove_file(tools.join("sdc-answers")).unwrap();
    fs::create_dir_all(build.join("driver/src/bin")).unwrap();
    fs::write(build.join("driver/src/bin/sdc-answers.rs"), "fn main() {}\n").unwrap();
    let (_, out) = experiment_env(&run, &d, &[("PATH", path)]);
    assert!(!out.contains("sdc-answers not on PATH"), "a program the build makes passes the check: {out}");
}

fn executable(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
}

/// `sdc-experiment run <id>` on a record of one stub run (a rule decider, no model call, no network)
/// starts the run with `runbook run` of the build and ends with exit 0. The build is a directory
/// that was built before (its record says so), so nothing is compiled; its `runbook` is a program
/// that writes its command line down and makes the run directory, and `systemd-run` is a script
/// that runs the unit's command at once.
#[test]
fn run_starts_a_stub_run_with_runbook_run_of_the_build_and_ends_with_exit_0() {
    let d = scratch("start");
    let git = |dir: &Path, args: &[&str]| {
        let o = Command::new("git").arg("-C").arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid", "-c", "commit.gpgsign=false"]).args(args).output().unwrap();
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    };
    let repo = d.join("repo");
    fs::create_dir_all(repo.join("protocol")).unwrap();
    fs::create_dir_all(repo.join("runbook/src/loom_run")).unwrap();
    fs::write(repo.join("protocol/sdc.protocol.yaml"), "protocol:\n  id: sdc\n  revision: 8\n").unwrap();
    fs::write(repo.join("runbook/Cargo.toml"), "[package]\nname = \"runbook\"\n").unwrap();
    fs::write(repo.join(IN_PROCESS_RUN), "// runbook run in process\n").unwrap();
    fs::write(repo.join(".gitignore"), "target/\n").unwrap();
    git(&repo, &["init", "--quiet"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "--quiet", "-m", "tree"]);
    let commit = git(&repo, &["rev-parse", "HEAD"]);
    let builds = d.join("b");
    fs::create_dir_all(&builds).unwrap();
    git(&d, &["clone", "--quiet", repo.to_str().unwrap(), builds.join(&commit).to_str().unwrap()]);
    fs::write(
        builds.join(format!("{commit}.built.json")),
        format!("{{\"format\":\"sdc.experiment-build/1\",\"commit\":\"{commit}\",\"built_at\":\"2026-11-22T00:00:00Z\",\"crates\":[\"driver\",\"goals\",\"watch\",\"runbook\"]}}\n"),
    )
    .unwrap();
    let started = d.join("started.txt");
    let bin = builds.join(&commit).join("runbook/target/debug");
    fs::create_dir_all(&bin).unwrap();
    executable(
        &bin.join("runbook"),
        &format!("#!/bin/sh\nprintf '%s\\n' \"$0\" \"$@\" > {}\nwhile [ $# -gt 0 ]; do case \"$1\" in --runs-dir) rd=$2;; --run) rn=$2;; esac; shift; done\nmkdir -p \"$rd/$rn\"\n", started.display()),
    );
    let tools = d.join("tools");
    fs::create_dir_all(&tools).unwrap();
    for t in ["jq", "yq"] {
        executable(&tools.join(t), "#!/bin/sh\n");
    }
    executable(&tools.join("systemctl"), "#!/bin/sh\necho inactive\n");
    let wrapper_log = d.join("systemd-run.log");
    executable(
        &tools.join("systemd-run"),
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> {}\nwhile [ $# -gt 0 ]; do case \"$1\" in --working-directory=*) cd \"${{1#--working-directory=}}\";; /usr/bin/env) break;; esac; shift; done\nexec \"$@\"\n",
            wrapper_log.display()
        ),
    );
    let settings: String = [
        ("loom.in_process", "'true'"), ("loom.confinement", "local"), ("loom.cap_micro_usd", "5000000"),
        ("models.writer.model", "claude-sonnet-5-5"), ("models.implement.model", "claude-sonnet-5-5"),
        ("models.review.model", "claude-sonnet-5-5"), ("models.other.model", "claude-sonnet-5-5"),
        ("decider.kind", "rule"), ("decider.verdict", "approve"), ("autonomy", "A5"), ("max_llm_calls", "10"), ("max_wall_seconds", "60"),
    ]
    .iter()
    .map(|(k, v)| format!("  {k}: {v}\n"))
    .collect();
    fs::create_dir_all(repo.join("experiments")).unwrap();
    fs::write(
        repo.join("experiments/lx.yaml"),
        format!(
            "format: sdc.experiment/1\nid: lx\nkind: prospective\nstatus: planned\ncreated: 2026-11-22\ntitle: A stub run of the cycle through Loom\n\
             purpose: Whether the launcher starts a run with runbook run.\ninputs:\n- {{name: invoices, kind: idea, text: {IDEA}, sha256: {}}}\n\
             tree_commit: {commit}\nfixed:\n{settings}variants:\n- {{id: stub, runner: loom, settings: {{}}, runs: []}}\nruns_per_variant: 1\n\
             pass_rule: clean-delivery\ndecision_rule: {{rule: descriptive, text: the record describes its runs and decides nothing}}\n",
            sha(IDEA)
        ),
    )
    .unwrap();
    let path = format!("{}:/usr/bin:/bin", tools.display());
    let runs = d.join("runs");
    let began = std::time::Instant::now();
    let (code, out) = experiment_env(
        &["run", "lx", "--builds-dir", builds.to_str().unwrap(), "--runs-dir", runs.to_str().unwrap(), "--max-parallel", "1", "--stagger", "0"],
        &repo,
        &[("PATH", &path), ("HOME", d.to_str().unwrap())],
    );
    eprintln!("sdc-experiment run on the stub record: {:?}", began.elapsed());
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("started lx.stub.1 as sdc-exp-lx-stub-1") && !out.contains("building "), "nothing is built: {out}");
    let line: Vec<String> = fs::read_to_string(&started).expect("the build's runbook was started").lines().map(String::from).collect();
    assert_eq!(line[0], bin.join("runbook").display().to_string(), "{line:?}");
    assert_eq!(&line[1..3], ["run", "sdc"], "the started command line begins with the build's runbook and run: {line:?}");
    let at = |flag: &str| line.iter().position(|a| a == flag).map(|i| line[i + 1].clone()).unwrap_or_else(|| panic!("no {flag} in {line:?}"));
    assert_eq!((at("--run").as_str(), at("--runs-dir")), ("lx.stub.1", runs.display().to_string()));
    assert!(!line.iter().any(|a| a == "--loom" || a == "--in-process" || a == "--idea"), "{line:?}");
    assert_eq!(fs::read_to_string(at("--intake")).unwrap(), format!("text: {IDEA}\n"));
    assert!(runs.join("lx.stub.1/experiment.json").is_file(), "the run is recorded");
    assert!(fs::read_to_string(repo.join("experiments/lx.yaml")).unwrap().contains("status: running"));
    // The process tree of the run holds no old driver: the wrapper log (the one `systemd-run` call,
    // with the unit's whole command) and the command line of the build's `runbook` name neither.
    let wrapped = fs::read_to_string(&wrapper_log).expect("the launcher called systemd-run");
    assert_eq!(wrapped.lines().count(), 1, "one unit for one run: {wrapped}");
    assert!(wrapped.contains(&format!("{} run sdc", bin.join("runbook").display())), "the unit runs `runbook run`: {wrapped}");
    for old in ["run-trial.sh", "sdc-drive"] {
        assert!(!wrapped.contains(old) && !line.iter().any(|a| a.contains(old)), "{old} is no part of the run: {wrapped} {line:?}");
    }
}

/// `stop` reaches a run through its unit: the stop record, then `systemctl --user stop <unit>`
/// (systemd's signal to the run's process), never a name and never a script.
#[test]
fn stop_signals_the_unit_of_the_run_and_calls_no_script() {
    let d = scratch("stop-unit");
    fs::create_dir_all(d.join("runs/lx.low.1")).unwrap();
    write_record(&d, &record_with(&fixed().into_iter().filter(|(k, _)| k != "max_llm_calls").collect::<Vec<_>>(), COMMIT, "running", ("lx.low.1", "")));
    let calls = d.join("systemctl.log");
    let tools = d.join("tools");
    fs::create_dir_all(&tools).unwrap();
    executable(
        &tools.join("systemctl"),
        &format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> {}\ncase \"$*\" in *ActiveState*) echo active;; esac\n", calls.display()),
    );
    let path = format!("{}:/usr/bin:/bin", tools.display());
    let (code, out) = experiment_env(&["stop", "lx", "--reason", "wrong cap", "--run", "lx.low.1", "--runs-dir", "runs"], &d, &[("PATH", &path), ("HOME", "/nonexistent-home")]);
    assert_eq!(code, 0, "{out}");
    assert!(d.join("runs/lx.low.1/run-stopped.json").is_file(), "{out}");
    let log = fs::read_to_string(&calls).unwrap();
    assert!(log.lines().any(|l| l == "--user stop sdc-exp-lx-low-1"), "the unit of the run is stopped: {log}");
    assert!(!out.contains("run-trial.sh"), "{out}");
}
