//! The bug-fix programs on repositories of other languages (docs/bugfix.md, "The commands of a
//! case"): the commands are the case's `checks.yaml` (`sdc.checks/1`), found at seed time from
//! `--checks`, the repository's `.runbook/checks.yaml` or the detection table
//! (`examples/sdc/checks-detect.yaml`).
//!
//! Fixtures, no network and no toolchain download: `bugfix-js` (JavaScript, `node --test`, no
//! dependencies; Node 22 is a requirement of this repository) and `bugfix-script` (plain executable
//! test scripts behind `--checks`). Each: seed, `repro-run` red on the unchanged code, `tests-run`
//! passed after the fix, `gate-run` with a failing gate command exits 1 naming it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/sdc/fixtures")
}

/// A fresh case work directory.
fn work(name: &str) -> PathBuf {
    let w = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bugfix-languages").join(name);
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(&w).unwrap();
    w
}

/// `sdc-bugfix <args>` in `work`: (exit code, stdout and stderr).
fn bugfix(work: &Path, args: &[&str]) -> (i32, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_sdc-bugfix"));
    c.args(args).current_dir(work);
    for (k, _) in std::env::vars().filter(|(k, _)| k.starts_with("SDC_")) {
        c.env_remove(k);
    }
    let o = c.output().unwrap();
    let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    (o.status.code().unwrap_or(-1), text)
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dst = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &dst);
        } else {
            fs::copy(e.path(), dst).unwrap();
        }
    }
}

/// Add a gate command to the case's checks.yaml.
fn add_gate(work: &Path, name: &str, argv: &[&str]) {
    let f = work.join("checks.yaml");
    let mut v: serde_yaml::Value = serde_yaml::from_str(&fs::read_to_string(&f).unwrap()).unwrap();
    let step = serde_yaml::to_value(serde_json::json!({"name": name, "argv": argv})).unwrap();
    v["gate"].as_sequence_mut().unwrap().push(step);
    fs::write(&f, serde_yaml::to_string(&v).unwrap()).unwrap();
}

/// Replace one key of the case's checks.yaml.
fn set_checks(work: &Path, key: &str, value: serde_json::Value) {
    let f = work.join("checks.yaml");
    let mut v: serde_yaml::Value = serde_yaml::from_str(&fs::read_to_string(&f).unwrap()).unwrap();
    v[key] = serde_yaml::to_value(value).unwrap();
    fs::write(&f, serde_yaml::to_string(&v).unwrap()).unwrap();
}

/// The reproducing test of `fx/stub-repro` written and installed, as test.reproduce does.
fn install_repro(w: &Path, fx: &Path) {
    copy_dir(&fx.join("stub-repro"), &w.join("repro"));
    let (code, out) = bugfix(w, &["repro-install"]);
    assert_eq!(code, 0, "{out}");
}

#[test]
fn a_javascript_repository_with_node_test_goes_red_then_green() {
    let fx = fixtures().join("bugfix-js");
    let w = work("js");
    let (code, out) = bugfix(&w, &["seed", "--from", fx.to_str().unwrap()]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("detected javascript-node-test (package.json)"), "{out}");
    let checks = fs::read_to_string(w.join("checks.yaml")).unwrap();
    assert!(checks.contains("format: sdc.checks/1") && checks.contains("- --test"), "{checks}");
    assert!(w.join("mainline/src/pager.js").is_file() && w.join("impl/package.json").is_file());

    // A test file that does not load fails, but not as the test: not red.
    fs::create_dir_all(w.join("repro")).unwrap();
    fs::copy(fx.join("stub-repro/repro.json"), w.join("repro/repro.json")).unwrap();
    fs::write(w.join("repro/last_page.test.js"), "// last partial page is shown\nsyntax error(\n").unwrap();
    assert_eq!(bugfix(&w, &["repro-install"]).0, 0);
    let (code, out) = bugfix(&w, &["repro-run"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("repro: not red: test last partial page is shown did not fail as a test"), "{out}");

    install_repro(&w, &fx);
    let (code, out) = bugfix(&w, &["repro-run"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("repro: red: test last partial page is shown (test/last_page.test.js) fails on the unchanged code"), "{out}");

    let (code, out) = bugfix(&w, &["tests-run"]);
    assert_eq!(code, 1, "the unfixed code: {out}");
    fs::copy(fx.join("stub-fix/src/pager.js"), w.join("impl/src/pager.js")).unwrap();
    let (code, out) = bugfix(&w, &["tests-run"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("tests: passed: test last partial page is shown (test/last_page.test.js) ok"), "{out}");

    let (code, out) = bugfix(&w, &["gate-run"]);
    assert_eq!(code, 0, "{out}");
    add_gate(&w, "never-passes", &["false"]);
    let (code, out) = bugfix(&w, &["gate-run"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("gate: ok     test: node --test --test-reporter=tap"), "{out}");
    assert!(out.contains("gate: FAILED never-passes: false exit 1"), "{out}");
}

#[test]
fn any_command_behind_checks_goes_red_then_green() {
    let fx = fixtures().join("bugfix-script");
    let w = work("script");
    let checks = fx.join("checks.yaml");
    let (code, out) = bugfix(&w, &["seed", "--from", fx.to_str().unwrap(), "--checks", checks.to_str().unwrap()]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("checks.yaml from --checks"), "{out}");
    assert!(w.join("impl/bin/total").is_file());

    install_repro(&w, &fx);
    let (code, out) = bugfix(&w, &["repro-run"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("repro: red: test last_number_is_added (tests/last_number.test.sh) fails on the unchanged code"), "{out}");

    fs::copy(fx.join("stub-fix/bin/total"), w.join("impl/bin/total")).unwrap();
    let (code, out) = bugfix(&w, &["tests-run"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("tests: passed: test last_number_is_added (tests/last_number.test.sh) ok"), "{out}");

    add_gate(&w, "never-passes", &["false"]);
    let (code, out) = bugfix(&w, &["gate-run"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("gate: ok     suite: ./checks/run-all"), "{out}");
    assert!(out.contains("gate: FAILED never-passes: false exit 1"), "{out}");
}

#[test]
fn without_a_red_rule_the_suite_without_the_test_must_pass() {
    let fx = fixtures().join("bugfix-script");
    let w = work("script-other-fails");
    let checks = fx.join("checks.yaml");
    assert_eq!(bugfix(&w, &["seed", "--from", fx.to_str().unwrap(), "--checks", checks.to_str().unwrap()]).0, 0);
    // Another test fails on the unchanged code: a failing reproducing test proves nothing.
    fs::write(w.join("mainline/tests/broken.test.sh"), "# broken\nexit 1\n").unwrap();
    install_repro(&w, &fx);
    let (code, out) = bugfix(&w, &["repro-run"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("repro: not red: other tests do not pass on the unchanged code"), "{out}");
}

#[test]
fn an_unknown_repository_without_checks_is_refused_at_seed() {
    let w = work("unknown");
    let tree = fixtures().join("bugfix-script/repo");
    let (code, out) = bugfix(&w, &["seed", "--from", tree.to_str().unwrap(), "--text", "an invented report"]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("--checks <file>") && out.contains("does not guess"), "{out}");
    assert!(!w.join("mainline").exists() && !w.join("checks.yaml").exists(), "nothing is written before the refusal");
}

#[test]
fn a_program_that_does_not_exist_exits_127() {
    let fx = fixtures().join("bugfix-script");
    let w = work("missing-program");
    let checks = fx.join("checks.yaml");
    assert_eq!(bugfix(&w, &["seed", "--from", fx.to_str().unwrap(), "--checks", checks.to_str().unwrap()]).0, 0);
    install_repro(&w, &fx);
    set_checks(&w, "repro", serde_json::json!(["no-such-program-of-sdc-bugfix", "{file}"]));
    let (code, out) = bugfix(&w, &["repro-run"]);
    assert_eq!(code, 127, "{out}");
    set_checks(&w, "test", serde_json::json!(["./checks/no-such-script"]));
    let (code, out) = bugfix(&w, &["tests-run"]);
    assert_eq!(code, 127, "{out}");
    assert!(out.contains("./checks/no-such-script is not on PATH"), "{out}");
}

#[test]
fn the_lookup_order_is_the_flag_then_the_repository_then_detection() {
    let fx = fixtures().join("bugfix-script");
    let tree = work("lookup-tree");
    copy_dir(&fx.join("repo"), &tree);
    fs::create_dir_all(tree.join(".runbook")).unwrap();
    fs::copy(fx.join("checks.yaml"), tree.join(".runbook/checks.yaml")).unwrap();
    let w = work("lookup-repo");
    let (code, out) = bugfix(&w, &["seed", "--from", tree.to_str().unwrap(), "--text", "an invented report"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("checks.yaml from .runbook/checks.yaml"), "{out}");

    let other = work("lookup-flag-file").join("checks.yaml");
    fs::write(&other, fs::read_to_string(fx.join("checks.yaml")).unwrap().replace("LC_ALL: C", "LC_ALL: POSIX")).unwrap();
    let w = work("lookup-flag");
    let (code, out) = bugfix(&w, &["seed", "--from", tree.to_str().unwrap(), "--text", "x", "--checks", other.to_str().unwrap()]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("checks.yaml from --checks"), "{out}");
    assert!(fs::read_to_string(w.join("checks.yaml")).unwrap().contains("POSIX"));

    // A Rust crate with no checks.yaml anywhere gets today's cargo commands.
    let w = work("lookup-rust");
    let (code, out) = bugfix(&w, &["seed", "--from", fixtures().join("bugfix-pager").to_str().unwrap()]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("detected rust (Cargo.toml)"), "{out}");
    let (code, shown) = bugfix(&w, &["checks"]);
    assert_eq!(code, 0, "{shown}");
    let v: serde_yaml::Value = serde_yaml::from_str(&shown).unwrap();
    assert_eq!(v["test"], serde_yaml::to_value(["cargo", "test", "--offline", "--no-fail-fast"]).unwrap());
    let gate: Vec<&str> = v["gate"].as_sequence().unwrap().iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert_eq!(gate, ["build", "test", "clippy", "fmt"]);
}

/// git in a fixture repository, with no hook, signing or identity of the machine's.
fn g(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .args(["-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false",
            "-c", "core.hooksPath=/dev/null", "-c", "init.defaultBranch=main"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

#[test]
fn a_ticket_seeds_a_javascript_repository() {
    let fx = fixtures().join("bugfix-js");
    let root = work("ticket-js");
    let repo = root.join("repo");
    copy_dir(&fx.join("repo"), &repo);
    g(&repo, &["init", "-q"]);
    g(&repo, &["add", "."]);
    g(&repo, &["commit", "-q", "-m", "fixture"]);
    let head = g(&repo, &["rev-parse", "HEAD"]);
    let ticket = root.join("ticket.yaml");
    fs::write(&ticket, format!("format: sdc.ticket/1\nid: JS-1\nkind: bug\ntitle: An invented ticket\n\
        text: pageCount(11, 5) returns 2.\nrepository:\n  path: repo\n  base:\n    ref: main\n    commit: \"{head}\"\n")).unwrap();
    let w = root.join("work");
    fs::create_dir_all(&w).unwrap();
    let (code, out) = bugfix(&w, &["seed", "--ticket", ticket.to_str().unwrap(), "--repo", repo.to_str().unwrap()]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("detected javascript-node-test (package.json)"), "{out}");
    let seed: serde_json::Value = serde_json::from_str(&fs::read_to_string(w.join(".sdc/seed.json")).unwrap()).unwrap();
    assert_eq!(seed["checks"], "detected javascript-node-test (package.json)");
}

mod pool {
use super::*;

// The build pool wait of the bug-fix steps (docs/driver.md, "The build pool wait"): `repro-run`,
// `tests-run` and `gate-run` send every command of checks.yaml through `slot`. `slot` here is a pool
// of its own: a job prints slot's `slot: job N queued` line, is `queued` in `slot status` until its
// delay has passed, then runs the command or ends the way POOL_FAKE says. No test calls the real
// pool.

const POOL_SLOT: &str = r##"#!/bin/sh
d=${POOL_FAKE_DIR:?}
case ${1:-} in
  status)
    echo "50 of 59 slots, 0 free, 1 running, 1 queued"
    for f in "$d"/job.*; do
      [ -e "$f" ] || continue
      n=${f##*.}
      if [ -e "$d/started.$n" ]; then echo "#$n running   rust   1 slot(s)  1s  repo  job"
      else echo "#$n queued    rust   1 slot(s)  1s  repo  job"; fi
    done
    exit 0 ;;
  cancel)
    echo "${2#\#}" >> "$d/cancelled"
    exit 0 ;;
esac
n=$(( $(cat "$d/count" 2>/dev/null || echo 0) + 1 ))
echo "$n" > "$d/count"
: > "$d/job.$n"
echo "$*" >> "$d/jobs"
case $POOL_FAKE in
  overtime)
    : > "$d/started.$n"
    echo "slot: job $n ended at its run limit" >&2
    exit 124 ;;
esac
echo "slot: job $n queued for 1 slot(s): 0 free, 3 request(s) ahead" >&2
case $POOL_FAKE in
  never) exec sleep 30 ;;
  cancel-queued) sleep 0.5; echo "slot: job $n cancelled by \`slot cancel\`" >&2; exit 130 ;;
esac
sleep "${POOL_FAKE_DELAY:-1.2}"
: > "$d/started.$n"
case $POOL_FAKE in
  killed) sleep 1; echo "slot: job $n cancelled by \`slot cancel\`" >&2; exit 130 ;;
esac
exec "$@"
"##;

struct Pool {
    code: i32,
    text: String,
    secs: f64,
    state: PathBuf,
}

impl Pool {
    fn cancelled(&self) -> String {
        fs::read_to_string(self.state.join("cancelled")).unwrap_or_default().trim().replace('\n', " ")
    }
    fn jobs(&self) -> usize {
        fs::read_to_string(self.state.join("jobs")).map_or(0, |s| s.lines().count())
    }
}

/// A seeded script case with the reproducing test installed and the stand-in pool in pool-bin/;
/// `fix` puts the fixed code in impl/.
fn pool_case(name: &str, fix: bool) -> (PathBuf, PathBuf) {
    let fx = fixtures().join("bugfix-script");
    let w = work(&format!("pool-{name}"));
    let checks = fx.join("checks.yaml");
    assert_eq!(bugfix(&w, &["seed", "--from", fx.to_str().unwrap(), "--checks", checks.to_str().unwrap()]).0, 0);
    install_repro(&w, &fx);
    if fix {
        fs::copy(fx.join("stub-fix/bin/total"), w.join("impl/bin/total")).unwrap();
    }
    let state = w.join("pool-state");
    let bin = w.join("pool-bin");
    fs::create_dir_all(&state).unwrap();
    fs::create_dir_all(&bin).unwrap();
    let p = bin.join("slot");
    fs::write(&p, POOL_SLOT).unwrap();
    fs::set_permissions(&p, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    (w, state)
}

/// `sdc-bugfix <args>` in the case with the stand-in pool: `fake` the behaviour of every pool job,
/// `env` further variables (no SDC_POOL_WAIT unless given).
fn pool_run(name: &str, fix: bool, args: &[&str], fake: &str, env: &[(&str, &str)]) -> Pool {
    let (w, state) = pool_case(name, fix);
    let path = format!("{}:{}", w.join("pool-bin").display(), std::env::var("PATH").unwrap_or_default());
    let t = std::time::Instant::now();
    let mut c = Command::new(env!("CARGO_BIN_EXE_sdc-bugfix"));
    for (k, _) in std::env::vars().filter(|(k, _)| k.starts_with("SDC_")) {
        c.env_remove(k);
    }
    c.args(args)
        .current_dir(&w)
        .env("PATH", path)
        .env("POOL_FAKE", fake)
        .env("POOL_FAKE_DIR", &state)
        .env("SDC_POOL_POLL", "0.2");
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c.output().unwrap();
    let secs = t.elapsed().as_secs_f64();
    eprintln!("pool-wait test {name} ({}): {secs:.1} s", args.join(" "));
    let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    Pool { code: o.status.code().unwrap_or(-1), text, secs, state }
}

#[test]
fn a_job_that_never_starts_ends_every_bug_fix_step_with_69_and_is_cancelled() {
    for step in ["repro-run", "tests-run", "gate-run"] {
        let p = pool_run(&format!("never-{step}"), false, &[step], "never", &[("SDC_POOL_WAIT", "1")]);
        assert_eq!(p.code, 69, "{step}: {}", p.text);
        assert!(p.secs < 5.0, "{step}: took {} s", p.secs);
        assert_eq!(p.cancelled(), "1", "{step}");
        assert_eq!(p.jobs(), 1, "{step}: the command did not run twice or go on");
        assert!(p.text.contains("pool-wait: job 1 was still queued"), "{step}: {}", p.text);
        assert!(!p.text.contains("not red") && !p.text.contains("FAILED"), "{step}: no verdict about the code: {}", p.text);
    }
}

#[test]
fn the_option_sets_the_bound_and_the_environment_wins() {
    let p = pool_run("opt", false, &["repro-run", "--pool-wait", "1"], "never", &[]);
    assert_eq!(p.code, 69, "{}", p.text);
    assert!(p.secs < 5.0, "took {} s", p.secs);
    let p = pool_run("env-wins", false, &["repro-run", "--pool-wait", "600"], "never", &[("SDC_POOL_WAIT", "1")]);
    assert_eq!(p.code, 69, "{}", p.text);
    assert!(p.secs < 5.0, "took {} s", p.secs);
    let p = pool_run("bad-bound", false, &["repro-run", "--pool-wait", "soon"], "never", &[]);
    assert_eq!(p.code, 64, "{}", p.text);
    assert_eq!(p.jobs(), 0);
}

#[test]
fn a_job_the_pool_ends_at_its_run_limit_ends_the_step_with_70_and_no_verdict() {
    for (step, bound) in [("repro-run", Some("10")), ("tests-run", None), ("gate-run", Some("10"))] {
        let env: Vec<(&str, &str)> = bound.map(|b| ("SDC_POOL_WAIT", b)).into_iter().collect();
        let p = pool_run(&format!("overtime-{step}"), false, &[step], "overtime", &env);
        assert_eq!(p.code, 70, "{step}: {}", p.text);
        assert!(p.text.contains("ran over its time"), "{step}: {}", p.text);
        assert!(!p.text.contains("not red") && !p.text.contains("FAILED"), "{step}: {}", p.text);
        assert_eq!(p.jobs(), 1, "{step}: the step stops at the first job");
        assert_eq!(p.cancelled(), "", "{step}");
    }
}

#[test]
fn a_job_cancelled_while_still_queued_is_a_pool_wait() {
    let p = pool_run("queued-cancel", false, &["repro-run"], "cancel-queued", &[("SDC_POOL_WAIT", "10")]);
    assert_eq!(p.code, 69, "{}", p.text);
    assert!(p.text.contains("cancelled while it was still queued"), "{}", p.text);
}

#[test]
fn a_started_job_the_pool_cancels_is_the_commands_result_and_says_so() {
    let p = pool_run("run-killed", false, &["repro-run"], "killed", &[("SDC_POOL_WAIT", "10")]);
    assert_eq!(p.code, 1, "{}", p.text);
    assert!(p.text.contains("cancelled by the pool while it ran"), "{}", p.text);
    assert_eq!(p.cancelled(), "");
}

#[test]
fn jobs_that_start_after_a_wait_inside_the_bound_give_the_old_results() {
    let p = pool_run("late-red", false, &["repro-run"], "late", &[("SDC_POOL_WAIT", "10")]);
    assert_eq!(p.code, 0, "{}", p.text);
    assert!(p.text.contains("repro: red:"), "{}", p.text);
    assert_eq!(p.jobs(), 2);
    let p = pool_run("late-green", true, &["tests-run"], "late", &[("SDC_POOL_WAIT", "10")]);
    assert_eq!(p.code, 0, "{}", p.text);
    assert!(p.text.contains("tests: passed:"), "{}", p.text);
    assert_eq!(p.cancelled(), "");
    let p = pool_run("late-gate", true, &["gate-run"], "late", &[("SDC_POOL_WAIT", "10")]);
    assert_eq!(p.code, 0, "{}", p.text);
    assert!(p.text.contains("gate: ok     suite"), "{}", p.text);
}

#[test]
fn one_bound_covers_every_job_of_a_step() {
    // tests-run runs two jobs. The first waits 1.5 s, the second would wait 1.5 s more: the step's
    // bound is 2 s, so the second is cancelled after 0.5 s and not given 2 s of its own.
    let p = pool_run("shared", true, &["tests-run"], "late", &[("SDC_POOL_WAIT", "2"), ("POOL_FAKE_DELAY", "1.5")]);
    assert_eq!(p.code, 69, "{}", p.text);
    assert_eq!(p.cancelled(), "2");
    assert_eq!(p.jobs(), 2);
    assert!(p.secs < 15.0, "took {} s", p.secs);
}

#[test]
fn without_a_bound_the_pool_is_plain_slot() {
    let p = pool_run("unbounded", true, &["tests-run"], "late", &[("POOL_FAKE_DELAY", "0.3")]);
    assert_eq!(p.code, 0, "{}", p.text);
    assert_eq!(p.cancelled(), "");
}

/// What the three bindings that send commands through the pool say, in both formats: the wait
/// bound in argv, and a limit that covers the wait bound, every run bound and the step's other
/// work (docs/driver.md, "The build pool wait": wait + jobs x 600 + 600).
#[test]
fn the_bug_fix_bindings_name_the_wait_bound_and_a_limit_that_covers_it() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let read = |p: &str| fs::read_to_string(root.join(p)).unwrap();
    let j: serde_json::Value = serde_json::from_str(&read("driver/examples/sdc/sdc-bugfix.bindings.json")).unwrap();
    let y: serde_yaml::Value = serde_yaml::from_str(&read("bindings/sdc-bugfix.yaml")).unwrap();
    // (action, pool jobs of the step: repro-run 2, tests-run 2, gate-run 4 of the rust detection row)
    for (action, jobs) in [("repro.run", 2u64), ("tests.run", 2), ("gate.run", 4)] {
        let ya = y["actions"].as_sequence().unwrap().iter().find(|a| a["action"] == action).unwrap();
        let jargv: Vec<&str> = j[action]["argv"].as_array().unwrap().iter().map(|a| a.as_str().unwrap()).collect();
        let yargv: Vec<&str> = ya["executor"]["argv"].as_sequence().unwrap().iter().map(|a| a.as_str().unwrap()).collect();
        for argv in [&jargv, &yargv] {
            let at = argv.iter().position(|a| *a == "--pool-wait").unwrap_or_else(|| panic!("{action}: no --pool-wait in {argv:?}"));
            assert_eq!(argv[at + 1], "1200", "{action}");
        }
        let need = 1200 + jobs * 600 + 600;
        assert_eq!(j[action]["timeout_s"].as_u64(), Some(need), "{action} JSON");
        assert_eq!(ya["timeout_seconds"].as_u64(), Some(need), "{action} YAML");
    }
}

/// The Loom form of the exits is a line of bindings/PENDING.md per binding, as for story and release.
#[test]
fn the_pending_list_gives_exits_69_and_70_for_the_three_bug_fix_bindings() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let pending = fs::read_to_string(root.join("bindings/PENDING.md")).unwrap();
    for action in ["repro.run", "tests.run", "gate.run"] {
        let line = pending
            .lines()
            .find(|l| l.starts_with("- `sdc-bugfix.bindings.json:") && l.contains(&format!("`{action}`")) && l.contains("69"))
            .unwrap_or_else(|| panic!("no line of PENDING.md for sdc-bugfix {action} naming exit 69"));
        assert!(line.contains("[69, 70, 75, 126, 127]"), "{line}");
    }
}
}
