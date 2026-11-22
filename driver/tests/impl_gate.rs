//! `impl-gate` (`bindings/tools/sdc-tool.sh`) reads cargo's test counts from stdout
//! alone. `slot` and `ess` are stand-ins on PATH: `slot` writes a scripted cargo test output, with
//! a stderr line written into the middle of a stdout `test result:` line, as cargo did under slot
//! and load ("impl-gate: FAILED run: 0 tests ran" on a passing suite).

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};

const ESS: &str = r#"#!/usr/bin/env bash
# ess specify compile --path spec --out <file>: one command
while (($#)); do [[ $1 == --out ]] && { echo '{"commands": {"billing.invoice.CreateInvoice": {}}}' > "$2"; }; shift; done
"#;

/// `slot cargo test --offline`: the output named by SLOT_FAKE.
const SLOT: &str = r#"#!/usr/bin/env bash
case $SLOT_FAKE in
  interleaved)
    printf 'running 1 test\ntest create_invoice ... ok\n\ntest result: ok. '
    printf '    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s\n' >&2
    printf '1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n' ;;
  zero) printf 'running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n' ;;
  unreadable) printf '    Finished `test` profile\n' >&2 ;;
  failing)
    printf 'test create_invoice ... FAILED\n\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out\n'
    exit 101 ;;
esac
"#;

fn gate(mode: &str) -> Output {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("impl-gate").join(mode);
    let _ = fs::remove_dir_all(&dir);
    let bin = dir.join("bin");
    fs::create_dir_all(&bin).unwrap();
    for (name, body) in [("ess", ESS), ("slot", SLOT)] {
        let p = bin.join(name);
        fs::write(&p, body).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let work = dir.join("work");
    fs::create_dir_all(work.join("impl/src")).unwrap();
    fs::create_dir_all(work.join("impl/tests")).unwrap();
    fs::create_dir_all(work.join("spec")).unwrap();
    fs::write(work.join("impl/Cargo.toml"), "[package]\nname = \"billing\"\n").unwrap();
    fs::write(work.join("impl/src/lib.rs"), "pub fn create_invoice() {}\n").unwrap();
    fs::write(work.join("impl/tests/billing.rs"), "#[test]\nfn create_invoice() {}\n").unwrap();
    let tool = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bindings/tools/sdc-tool.sh");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    Command::new("bash")
        .arg(tool)
        .args(["impl-gate", "impl"])
        .current_dir(&work)
        .env("PATH", path)
        .env("SLOT_FAKE", mode)
        .env_remove("SDC_CASE")
        .output()
        .unwrap()
}

fn text(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

#[test]
fn a_stderr_line_inside_the_result_line_still_counts_the_tests() {
    let o = gate("interleaved");
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    assert!(text(&o).contains("1 tests ran"), "{}", text(&o));
}

#[test]
fn no_test_run_is_a_failure_of_the_tree() {
    let o = gate("zero");
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert!(text(&o).contains("run: 0 tests ran"), "{}", text(&o));
}

#[test]
fn output_that_cannot_be_read_is_a_tool_error() {
    let o = gate("unreadable");
    assert_eq!(o.status.code(), Some(126), "{}", text(&o));
    assert!(text(&o).contains("could not be read"), "{}", text(&o));
}

#[test]
fn a_failing_test_is_a_failure_of_the_tree() {
    let o = gate("failing");
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert!(text(&o).contains("run: cargo test exit 101"), "{}", text(&o));
}

// The build pool wait (docs/driver.md, "The build pool wait"). `slot` here is a pool of its own: a
// job prints slot's `slot: job N queued` line, is `queued` in `slot status` until its delay has
// passed, then runs the way POOL_FAKE says. No test calls the real pool.

const POOL_ESS: &str = r#"#!/usr/bin/env bash
case "$1 $2" in
  "specify compile")
    while (($#)); do [[ $1 == --out ]] && { echo '{"commands": {"billing.invoice.CreateInvoice": {}}}' > "$2"; }; shift; done ;;
  "verify conform")
    if [[ $3 == synthesize ]]; then
      while (($#)); do [[ $1 == --out ]] && { echo '{}' > "$2"; }; shift; done
    fi ;;
esac
"#;

const POOL_SLOT: &str = r##"#!/usr/bin/env bash
d=${POOL_FAKE_DIR:?}
case ${1:-} in
  status)
    echo "50 of 59 slots, 0 free, 1 running, 1 queued"
    for f in "$d"/job.*; do
      [[ -e $f ]] || continue
      n=${f##*.}
      if [[ -e $d/started.$n ]]; then echo "#$n running   rust   1 slot(s)  1s  repo  cargo test"
      else echo "#$n queued    rust   1 slot(s)  1s  repo  cargo test"; fi
    done
    exit 0 ;;
  cancel)
    echo "${2#\#}" >> "$d/cancelled"
    exit 0 ;;
esac
n=$(( $(cat "$d/count" 2>/dev/null || echo 0) + 1 ))
echo $n > "$d/count"
: > "$d/job.$n"
echo "$*" >> "$d/jobs"
if [[ $POOL_FAKE == immediate ]]; then
  POOL_FAKE=green
  : > "$d/started.$n"
else
  sleep "${POOL_FAKE_QUEUED_DELAY:-0}"
  echo "slot: job $n queued for 1 slot(s): 0 free, 3 request(s) ahead" >&2
fi
[[ $POOL_FAKE == never ]] && exec sleep "${POOL_FAKE_SLEEP:-30}"
[[ $POOL_FAKE == cancel-queued ]] && { sleep 0.5; echo "slot: job $n cancelled by \`slot cancel\`" >&2; exit 130; }
sleep "${POOL_FAKE_DELAY:-1.2}"
: > "$d/started.$n"
if [[ $1 == task ]]; then
  [[ $POOL_FAKE == killed ]] && { sleep 1; echo "slot: job $n cancelled by \`slot cancel\`" >&2; exit 130; }
  [[ $POOL_FAKE == overtime ]] && exit 124
  exit 0
fi
case $POOL_FAKE in
  green)
    printf 'running 1 test\ntest create_invoice ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n' ;;
  overtime)
    echo "slot: job $n ended at its run limit" >&2
    exit 124 ;;
  red)
    printf 'test create_invoice ... FAILED\n\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out\n'
    exit 101 ;;
  killed)
    sleep 1
    echo "slot: job $n cancelled by \`slot cancel\`" >&2
    exit 130 ;;
esac
"##;

struct Pool {
    out: Output,
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

/// One call of the tool with the pool of this file: `sub` run on `impl`, `fake` the behaviour of
/// every pool job, `args` after the tree, `env` further variables.
fn pool(name: &str, sub: &str, fake: &str, args: &[&str], env: &[(&str, &str)]) -> Pool {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("pool-wait").join(name);
    let _ = fs::remove_dir_all(&dir);
    let bin = dir.join("bin");
    let state = dir.join("state");
    let work = dir.join("work");
    for d in [bin.clone(), state.clone(), work.join("impl/src"), work.join("impl/tests"), work.join("spec")] {
        fs::create_dir_all(d).unwrap();
    }
    for (n, body) in [("ess", POOL_ESS), ("slot", POOL_SLOT)] {
        let p = bin.join(n);
        fs::write(&p, body).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fs::write(work.join("impl/Cargo.toml"), "[package]\nname = \"billing\"\n").unwrap();
    fs::write(work.join("impl/Taskfile.yml"), "version: '3'\n").unwrap();
    fs::write(work.join("impl/src/lib.rs"), "pub fn create_invoice() {}\n").unwrap();
    fs::write(work.join("impl/tests/billing.rs"), "#[test]\nfn create_invoice() {}\n").unwrap();
    fs::write(work.join("spec/system.yaml"), "system: other\n").unwrap();
    let tool = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bindings/tools/sdc-tool.sh");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    let t = std::time::Instant::now();
    let mut c = Command::new("bash");
    c.arg(tool).args([sub, "impl"]).args(args).current_dir(&work)
        .env("PATH", path)
        .env("POOL_FAKE", fake)
        .env("POOL_FAKE_DIR", &state)
        .env("SDC_POOL_POLL", "0.2")
        .env_remove("SDC_CASE")
        .env_remove("SDC_POOL_WAIT")
        .env_remove("SDC_POOL_LEDGER");
    for (k, v) in env {
        c.env(k, v);
    }
    let out = c.output().unwrap();
    let secs = t.elapsed().as_secs_f64();
    eprintln!("pool-wait test {name} ({sub}): {secs:.1} s");
    Pool { out, secs, state }
}

#[test]
fn a_job_that_never_starts_ends_the_step_with_69_and_is_cancelled() {
    let p = pool("never", "impl-gate", "never", &["--pool-wait", "1"], &[]);
    assert_eq!(p.out.status.code(), Some(69), "{}", text(&p.out));
    assert!(p.secs < 5.0, "took {} s", p.secs);
    assert_eq!(p.cancelled(), "1");
    assert!(text(&p.out).contains("pool-wait: job 1 was still queued"), "{}", text(&p.out));
    assert!(!text(&p.out).contains("FAILED"), "{}", text(&p.out));
}

#[test]
fn the_environment_bound_wins_over_the_argument() {
    let p = pool("env-wins", "impl-gate", "never", &["--pool-wait", "600"], &[("SDC_POOL_WAIT", "1")]);
    assert_eq!(p.out.status.code(), Some(69), "{}", text(&p.out));
    assert!(p.secs < 5.0, "took {} s", p.secs);
}

#[test]
fn a_job_that_starts_after_a_wait_inside_the_bound_gives_the_old_green() {
    let p = pool("late-green", "impl-gate", "green", &["--pool-wait", "10"], &[]);
    assert_eq!(p.out.status.code(), Some(0), "{}", text(&p.out));
    assert!(text(&p.out).contains("1 tests ran"), "{}", text(&p.out));
    assert_eq!(p.cancelled(), "");
}

#[test]
fn a_job_that_starts_after_a_wait_inside_the_bound_gives_the_old_red() {
    let p = pool("late-red", "impl-gate", "red", &["--pool-wait", "10"], &[]);
    assert_eq!(p.out.status.code(), Some(1), "{}", text(&p.out));
    assert!(text(&p.out).contains("run: cargo test exit 101"), "{}", text(&p.out));
}

#[test]
fn a_started_job_the_pool_cancels_is_a_failure_that_says_so_and_not_a_pool_wait() {
    let p = pool("run-killed", "impl-gate", "killed", &["--pool-wait", "10"], &[]);
    assert_eq!(p.out.status.code(), Some(1), "{}", text(&p.out));
    assert!(text(&p.out).contains("cancelled by the pool while it ran"), "{}", text(&p.out));
    assert_eq!(p.cancelled(), "");
}

#[test]
fn a_job_the_pool_ends_at_its_run_limit_is_ran_over_its_time_neither_red_nor_a_pool_wait() {
    // slot 0.22: the daemon ends a job after its run limit and the job exits 124.
    for (name, args) in [("overtime-bound", vec!["--pool-wait", "10"]), ("overtime-plain", vec![])] {
        let p = pool(name, "impl-gate", "overtime", &args, &[]);
        assert_eq!(p.out.status.code(), Some(70), "{name}: {}", text(&p.out));
        assert!(text(&p.out).contains("ran over its time"), "{name}: {}", text(&p.out));
        assert!(!text(&p.out).contains("FAILED run"), "{name}: {}", text(&p.out));
        assert_eq!(p.cancelled(), "");
    }
}

#[test]
fn conform_run_passes_a_run_over_its_time_on() {
    let p = pool("conform-overtime", "conform-run", "overtime", &["--pool-wait", "10"], &[]);
    assert_eq!(p.out.status.code(), Some(70), "{}", text(&p.out));
    assert!(!text(&p.out).contains("FAILED `task check`"), "{}", text(&p.out));
}

#[test]
fn a_queued_line_that_comes_late_does_not_make_the_job_started() {
    // The machine is loaded: slot prints its `queued` line 4 s after it was started. Being started
    // is never inferred from a missing line, so the 1 s bound still applies once the line is
    // there. The stand-in's job would stay queued 20 s (shortened from its 30 s so a regression
    // ends the test, with exit 0, and not after minutes).
    let p = pool("late-line", "impl-gate", "never", &["--pool-wait", "1"],
        &[("POOL_FAKE_QUEUED_DELAY", "4"), ("POOL_FAKE_SLEEP", "20")]);
    assert_eq!(p.out.status.code(), Some(69), "{}", text(&p.out));
    assert_eq!(p.cancelled(), "1");
    assert!(p.secs < 12.0, "took {} s", p.secs);
}

#[test]
fn the_bound_holds_under_a_locale_with_a_decimal_comma() {
    // A systemd unit on a host set to de_DE gives bash's EPOCHREALTIME a comma ("1763810464,227523"):
    // the elapsed time was computed from a text that is no number with a point, came out as noise,
    // never reached the 1 s bound, and the queued job ran to its end (finding cycle `pool-wait`,
    // 10 minutes, then "jq: parse error"). The stand-in's job would stay queued 20 s.
    let probe = Command::new("bash").arg("-c").arg("echo $EPOCHREALTIME").env("LC_ALL", "de_DE.UTF-8").output().unwrap();
    if !String::from_utf8_lossy(&probe.stdout).contains(',') {
        eprintln!("skipped: no de_DE.UTF-8 locale on this host (bash's EPOCHREALTIME keeps its point)");
        return;
    }
    let p = pool("comma-locale", "impl-gate", "never", &["--pool-wait", "1"],
        &[("LC_ALL", "de_DE.UTF-8"), ("POOL_FAKE_SLEEP", "20")]);
    assert_eq!(p.out.status.code(), Some(69), "{}", text(&p.out));
    assert_eq!(p.cancelled(), "1");
    assert!(p.secs < 12.0, "took {} s", p.secs);
}

#[test]
fn a_job_admitted_at_once_prints_no_queued_line_and_runs_to_its_end() {
    // 3.5 s of run time with a bound of 1 s: no queued line, so the job was admitted at once and the
    // bound has nothing to cancel.
    let p = pool("immediate", "impl-gate", "immediate", &["--pool-wait", "1"], &[("POOL_FAKE_DELAY", "3.5")]);
    assert_eq!(p.out.status.code(), Some(0), "{}", text(&p.out));
    assert_eq!(p.cancelled(), "");
}

#[test]
fn a_job_cancelled_while_still_queued_is_a_pool_wait() {
    let p = pool("queued-cancel", "impl-gate", "cancel-queued", &["--pool-wait", "10"], &[]);
    assert_eq!(p.out.status.code(), Some(69), "{}", text(&p.out));
    assert!(text(&p.out).contains("cancelled while it was still queued"), "{}", text(&p.out));
}

#[test]
fn conform_run_ends_with_69_when_its_task_check_never_starts() {
    let p = pool("conform-never", "conform-run", "never", &["--pool-wait", "1"], &[]);
    assert_eq!(p.out.status.code(), Some(69), "{}", text(&p.out));
    assert!(p.secs < 5.0, "took {} s", p.secs);
    assert!(!text(&p.out).contains("FAILED `task check`"), "{}", text(&p.out));
}

#[test]
fn one_bound_covers_both_jobs_of_a_step() {
    // conform-run's `task check` waits 1.5 s, impl-gate's `cargo test` would wait 1.5 s more: the
    // step's bound is 2 s, so the second job is cancelled after 0.5 s and not given 2 s of its own.
    let p = pool("shared", "story-tests-run", "green", &["--pool-wait", "2"], &[("POOL_FAKE_DELAY", "1.5")]);
    assert_eq!(p.out.status.code(), Some(69), "{}", text(&p.out));
    assert_eq!(p.cancelled(), "2");
    assert_eq!(p.jobs(), 2);
    assert!(p.secs < 15.0, "took {} s", p.secs);
}

#[test]
fn without_a_bound_the_pool_is_plain_slot() {
    let p = pool("unbounded", "impl-gate", "green", &[], &[("POOL_FAKE_DELAY", "0.3")]);
    assert_eq!(p.out.status.code(), Some(0), "{}", text(&p.out));
    assert_eq!(p.cancelled(), "");
}

/// What the two bindings that call the pool through the tool say, in both formats: the wait bound
/// in argv, and a limit that covers the wait bound, every run bound and the step's other work
/// (docs/driver.md, "The build pool wait": wait + jobs x 600 + 600).
#[test]
fn the_bindings_name_the_wait_bound_and_a_limit_that_covers_it() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let read = |p: &str| fs::read_to_string(root.join(p)).unwrap();
    // (JSON file, YAML file, action, pool jobs of the step)
    for (json_file, yaml_file, action, jobs) in [
        ("driver/examples/sdc/sdc-story.bindings.json", "bindings/sdc-story.yaml", "tests.run", 2u64),
        ("driver/examples/sdc/sdc-release.bindings.json", "bindings/sdc-release.yaml", "acceptance.run", 1u64),
    ] {
        let j: serde_json::Value = serde_json::from_str(&read(json_file)).unwrap();
        let y: serde_yaml::Value = serde_yaml::from_str(&read(yaml_file)).unwrap();
        let ya = y["actions"].as_sequence().unwrap().iter().find(|a| a["action"] == action).unwrap();
        let jargv: Vec<&str> = j[action]["argv"].as_array().unwrap().iter().map(|a| a.as_str().unwrap()).collect();
        let yargv: Vec<&str> = ya["executor"]["argv"].as_sequence().unwrap().iter().map(|a| a.as_str().unwrap()).collect();
        for argv in [&jargv, &yargv] {
            let at = argv.iter().position(|a| *a == "--pool-wait").unwrap_or_else(|| panic!("{action}: no --pool-wait in {argv:?}"));
            assert_eq!(argv[at + 1], "1200", "{action}");
        }
        let need = 1200 + jobs * 600 + 600;
        assert_eq!(j[action]["timeout_s"].as_u64(), Some(need), "{json_file} {action}");
        assert_eq!(ya["timeout_seconds"].as_u64(), Some(need), "{yaml_file} {action}");
    }
}

/// The new exit in the binding documents: the driver's `program_error` knows it (unit test in
/// main.rs); the YAML has no `exits` member in the loader it follows, so the construct is a line of
/// bindings/PENDING.md for each of the two bindings.
#[test]
fn the_pending_list_gives_exit_69_for_both_bindings() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let pending = fs::read_to_string(root.join("bindings/PENDING.md")).unwrap();
    for (json_file, action) in [("sdc-story.bindings.json", "tests.run"), ("sdc-release.bindings.json", "acceptance.run")] {
        let line = pending
            .lines()
            .find(|l| l.starts_with(&format!("- `{json_file}:")) && l.contains(&format!("`{action}`")) && l.contains("69"))
            .unwrap_or_else(|| panic!("no line of PENDING.md for {json_file} {action} naming exit 69"));
        assert!(line.contains("`[69, 75, 126, 127]`") || line.contains("[69"), "{line}");
    }
}

#[test]
fn a_bound_that_is_not_a_number_is_refused() {
    let p = pool("bad-bound", "impl-gate", "green", &["--pool-wait", "soon"], &[]);
    assert_eq!(p.out.status.code(), Some(64), "{}", text(&p.out));
}

/// The stand-in pool of the finding cycles keeps a queued job at most 30 s by default, so a cycle
/// in which nothing cancels the job ends inside the 60 s bound of a test and not after 10 minutes.
#[test]
fn the_cycle_stand_in_keeps_a_queued_job_at_most_30_s_by_default() {
    let fixture = fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/sdc/fixtures/pool-wait/slot")).unwrap();
    let line = fixture.lines().find(|l| l.starts_with("exec sleep")).expect("the stand-in ends in an `exec sleep` line");
    let secs: u64 = line.split(":-").nth(1).and_then(|r| r.split('}').next()).and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("no default in {line}"));
    assert!(secs <= 30, "default {secs} s in {line}");
}

/// A round whose `sdc-drive run` printed text (a step that stopped the run before its verdict) is
/// reported with the program and its first line, not as jq's bare "parse error ... column 10".
#[test]
fn a_run_that_prints_no_json_is_reported_with_the_program_and_its_first_line() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("no-json-run");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let pin = fs::read_to_string(root.join("driver/canon-pin.json")).unwrap();
    let version = pin.lines().find_map(|l| l.trim().strip_prefix("\"version\": \"")).and_then(|r| r.split('"').next()).unwrap().to_string();
    let script = |name: &str, body: String| {
        let p = dir.join(name);
        fs::write(&p, body).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        p
    };
    let canon = script("canon", format!("#!/bin/sh\necho 'canon {version}'\n"));
    let drive = script(
        "drive",
        "#!/bin/bash\nwhile (($#)); do [[ $1 == --case-dir ]] && d=$2; shift; done\n[[ -d $d/work ]] || { mkdir -p \"$d/work\"; exit 0; }\n\
         echo 'sdc-drive: `tests.run` could not run (exit 126, a program error): boom'; echo 'second line'; exit 2\n"
            .into(),
    );
    let out = Command::new("bash")
        .arg(root.join("scripts/run-full-cycle.sh"))
        .args(["no-json", dir.join("store").to_str().unwrap()])
        .env("CANON", canon)
        .env("DRIVE", drive)
        .env("RUNS", dir.join("runs"))
        .output()
        .unwrap();
    let t = text(&out);
    assert_eq!(out.status.code(), Some(5), "{t}");
    assert!(t.contains("`sdc-drive run` printed no JSON (exit 2); its first line: sdc-drive: `tests.run` could not run"), "{t}");
    assert!(!t.contains("jq: parse error"), "{t}");
    assert!(!t.contains("second line"), "{t}");
}
