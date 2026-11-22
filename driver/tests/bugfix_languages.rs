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
