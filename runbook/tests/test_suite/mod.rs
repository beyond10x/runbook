//! `runbook test <suite>` on a small fixture suite (`tests/fixtures/test-suite/`): `suites/good.suite.yaml`
//! (two cycles as expected, one pending that still fails), `suites/wrong.suite.yaml` (one cycle wrong on
//! purpose) and `suites/refused.suite.yaml` (a step kind Loom does not know). The bound programs are
//! POSIX sh. No model, no network.

use super::BIN;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

mod amend;
mod notcalled;
mod options;
mod workers;

const MARKER: &str = "RUNBOOK_TEST_MARKER";
const FORBIDDEN: [&str; 4] = ["b10x-loom", "loom", "canon", concat!("sdc-dr", "ive")];

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// A copy of the fixture under the target directory, with a runtime directory beside it.
struct Rig {
    dir: PathBuf,
}

impl Rig {
    fn new(tag: &str) -> Rig {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("test-suite-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test-suite"), &dir.join("home"));
        std::fs::create_dir_all(dir.join("run")).unwrap();
        Rig { dir }
    }

    fn home(&self) -> PathBuf {
        self.dir.join("home")
    }

    fn suite(&self, name: &str) -> PathBuf {
        self.home().join("suites").join(name)
    }

    fn scratch(&self) -> PathBuf {
        self.dir.join("scratch")
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut c = Command::new(BIN);
        c.arg("test").args(args).env("RUNBOOK_HOME", self.home()).env("XDG_RUNTIME_DIR", self.dir.join("run")).env(MARKER, "marker-7c1e").stdin(Stdio::null());
        c
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn lines(o: &Output) -> Vec<String> {
    out(o).lines().map(str::to_owned).collect()
}

/// The command names of every process below this one.
fn descendants() -> Vec<String> {
    let mut table: Vec<(u32, u32, String)> = Vec::new();
    for e in std::fs::read_dir("/proc").unwrap().flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else { continue };
        let Ok(stat) = std::fs::read_to_string(e.path().join("stat")) else { continue };
        let (Some(open), Some(close)) = (stat.find('('), stat.rfind(')')) else { continue };
        let ppid = stat[close + 1..].split_whitespace().nth(1).and_then(|p| p.parse::<u32>().ok()).unwrap_or(0);
        table.push((pid, ppid, stat[open + 1..close].to_string()));
    }
    let mut below: BTreeSet<u32> = BTreeSet::from([std::process::id()]);
    loop {
        let more: Vec<u32> = table.iter().filter(|(p, pp, _)| below.contains(pp) && !below.contains(p)).map(|(p, _, _)| *p).collect();
        if more.is_empty() {
            break;
        }
        below.extend(more);
    }
    table.into_iter().filter(|(p, _, _)| *p != std::process::id() && below.contains(p)).map(|(_, _, c)| c).collect()
}

#[test]
fn a_suite_prints_a_line_per_cycle_then_the_slowest_and_the_count() {
    let rig = Rig::new("lines");
    let scratch = rig.scratch();
    let o = rig.run(&[rig.suite("good.suite.yaml").to_str().unwrap(), "--only", "writes-a-note,reads-a-seed,later", "--scratch", scratch.to_str().unwrap(), "--confinement", "local"]);
    assert_eq!(o.status.code(), Some(0), "{}\n{}", out(&o), err(&o));
    let l = lines(&o);
    assert!(l[0].starts_with("writes-a-note: as expected ("), "{l:?}");
    assert!(l[0].ends_with(" s)"), "{l:?}");
    assert!(l[1].starts_with("reads-a-seed: as expected ("), "{l:?}");
    assert!(l[2].starts_with("later: pending, still failing: not-yet (contains"), "{l:?}");
    assert_eq!(l[3], "selected 3 of 4 cycles", "with --only the report says how many of the suite's cycles were chosen: {l:?}");
    let named = l[4].strip_prefix("slowest: ").unwrap_or_else(|| panic!("Loom's slowest line: {l:?}"));
    let slowest: Vec<(&str, f64)> = named
        .split("), ")
        .map(|x| {
            let (name, seconds) = x.trim_end_matches(')').split_once(" (").unwrap();
            (name, seconds.trim_end_matches(" s").parse().unwrap())
        })
        .collect();
    assert_eq!(slowest.len(), 3, "every cycle that ran is named: {l:?}");
    assert!(slowest.windows(2).all(|w| w[0].1 >= w[1].1), "slowest first: {l:?}");
    let mut names: Vec<&str> = slowest.iter().map(|(n, _)| *n).collect();
    names.sort_unstable();
    assert_eq!(names, ["later", "reads-a-seed", "writes-a-note"], "{l:?}");
    assert_eq!(l.len(), 6, "{l:?}");
    assert_eq!(l[5], "2 of 2 as expected, 1 pending", "{l:?}");
    assert_eq!(std::fs::read_to_string(scratch.join("work/2/note.txt")).unwrap(), "good day\n", "a scratch named by --scratch is kept");
}

#[test]
fn a_wrong_cycle_exits_1_and_names_its_check() {
    let rig = Rig::new("wrong");
    let o = rig.run(&[rig.suite("wrong.suite.yaml").to_str().unwrap(), "--confinement", "local"]);
    assert_eq!(o.status.code(), Some(1), "{}\n{}", out(&o), err(&o));
    let l = lines(&o);
    assert!(l[0].starts_with("writes-a-note: as expected ("), "{l:?}");
    assert!(l[1].starts_with("wrong-on-purpose: FAILED: the-note-says-goodbye (contains"), "{l:?}");
    assert!(l.contains(&"1 of 2 as expected, 0 pending".to_owned()), "{l:?}");
    assert!(l.contains(&"failed: wrong-on-purpose".to_owned()), "{l:?}");
    let kept = l.iter().find_map(|x| x.strip_prefix("scratch kept: ")).expect("the scratch directory is named");
    assert!(Path::new(kept).join("work/2/note.txt").is_file(), "{kept}");
    assert!(Path::new(kept).starts_with(rig.dir.join("run/runbook")), "the default scratch lies under $XDG_RUNTIME_DIR/runbook: {kept}");
}

#[test]
fn the_default_scratch_is_removed_when_every_cycle_is_as_expected() {
    let rig = Rig::new("removed");
    let o = rig.run(&[rig.suite("good.suite.yaml").to_str().unwrap(), "--only", "writes-a-note", "--confinement", "local"]);
    assert_eq!(o.status.code(), Some(0), "{}\n{}", out(&o), err(&o));
    assert!(!out(&o).contains("scratch kept"), "{}", out(&o));
    let left: Vec<_> = std::fs::read_dir(rig.dir.join("run/runbook")).map(|d| d.flatten().map(|e| e.file_name()).collect()).unwrap_or_default();
    assert!(left.is_empty(), "the scratch directory is gone: {left:?}");
}

#[test]
fn a_refused_suite_exits_2_with_loom_s_reason_and_place() {
    let rig = Rig::new("refused");
    let scratch = rig.scratch();
    let o = rig.run(&[rig.suite("refused.suite.yaml").to_str().unwrap(), "--scratch", scratch.to_str().unwrap(), "--confinement", "local"]);
    assert_eq!(o.status.code(), Some(2), "{}\n{}", out(&o), err(&o));
    let e = err(&o);
    assert!(e.contains("StepKindUnknown") && e.contains("cycles[0].steps[1]"), "{e}");
    assert_eq!(out(&o), "", "no cycle ran");
    assert!(!scratch.exists(), "nothing is written before the suite is accepted");
    let o = rig.run(&[rig.home().join("suites/missing.suite.yaml").to_str().unwrap(), "--confinement", "local"]);
    assert_eq!(o.status.code(), Some(2), "{}", err(&o));
    assert!(err(&o).contains("SuiteUnreadable"), "{}", err(&o));
}

#[test]
fn the_confinement_level_is_required_and_substrate_is_refused() {
    let rig = Rig::new("level");
    let suite = rig.suite("good.suite.yaml");
    let o = rig.run(&[suite.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(64), "no default level: {}", err(&o));
    assert!(err(&o).contains("--confinement"), "{}", err(&o));
    let o = rig.run(&[suite.to_str().unwrap(), "--confinement", "substrate"]);
    assert_eq!(o.status.code(), Some(2), "{}\n{}", out(&o), err(&o));
    assert!(err(&o).contains("ConfinementRefused") && err(&o).contains("CgroupUndelegated"), "{}", err(&o));
    assert_eq!(out(&o), "");
    assert!(std::fs::read_dir(rig.dir.join("run")).unwrap().next().is_none(), "a refused level writes nothing");
}

#[test]
fn only_runs_the_named_cycles_and_names_that_do_not_exist_are_refused() {
    let rig = Rig::new("only");
    let suite = rig.suite("good.suite.yaml");
    let o = rig.run(&[suite.to_str().unwrap(), "--only", "later", "--only", "writes-a-note", "--confinement", "local"]);
    assert_eq!(o.status.code(), Some(0), "{}\n{}", out(&o), err(&o));
    let l = lines(&o);
    assert!(l[0].starts_with("writes-a-note: as expected ("), "suite order, not the order of --only: {l:?}");
    assert!(l[1].starts_with("later: pending, still failing"), "{l:?}");
    assert_eq!(l.last().unwrap(), "1 of 1 as expected, 1 pending", "{l:?}");
    assert!(!out(&o).contains("reads-a-seed"), "{l:?}");
    let o = rig.run(&[suite.to_str().unwrap(), "--only", "no-such-cycle", "--confinement", "local"]);
    assert_eq!(o.status.code(), Some(2), "{}", err(&o));
    assert!(err(&o).contains("CycleUnknown") && err(&o).contains("no-such-cycle"), "Loom refuses the name: {}", err(&o));
    let left: Vec<_> = std::fs::read_dir(rig.home().join("suites")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.starts_with('.')).collect();
    assert!(left.is_empty(), "nothing is written beside the suite: {left:?}");
}

#[test]
fn the_process_tree_holds_runbook_and_the_bound_programs_only_and_they_get_path_and_loom_alone() {
    let rig = Rig::new("tree");
    let scratch = rig.scratch();
    let mut child = rig
        .command(&[rig.suite("good.suite.yaml").to_str().unwrap(), "--only", "pauses", "--scratch", scratch.to_str().unwrap(), "--confinement", "local"])
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let limit = Instant::now() + Duration::from_secs(50);
    let status = loop {
        seen.extend(descendants());
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < limit, "the run did not end in 50 s: {seen:?}");
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(status.code(), Some(0));
    assert!(seen.contains("runbook"), "{seen:?}");
    assert!(seen.contains("sleep"), "the bound program ran while it was sampled: {seen:?}");
    for name in FORBIDDEN {
        assert!(!seen.contains(name), "a process of the old runtime ran: {seen:?}");
    }
    let dump = std::fs::read_to_string(scratch.join("work/3/envdump.txt")).unwrap();
    let names: BTreeSet<&str> = dump.lines().filter_map(|l| l.split_once('=').map(|(k, _)| k)).collect();
    assert!(names.contains("PATH") && names.contains("LOOM_CASE"), "{names:?}");
    let others: Vec<&&str> = names.iter().filter(|k| **k != "PATH" && !k.starts_with("LOOM_") && !matches!(**k, "PWD" | "SHLVL" | "_" | "OLDPWD")).collect();
    assert!(others.is_empty(), "a bound program gets PATH and LOOM_* only: {others:?}");
    assert!(!dump.contains("marker-7c1e") && !names.contains(MARKER), "a variable of the command's environment reached the program: {dump}");
}

#[test]
fn a_termination_signal_ends_the_suite_with_exit_1_and_says_so() {
    let rig = Rig::new("signal");
    let scratch = rig.scratch();
    let child = rig
        .command(&[rig.suite("good.suite.yaml").to_str().unwrap(), "--only", "pauses,later", "--scratch", scratch.to_str().unwrap(), "--confinement", "local"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let pid = child.id() as i32;
    let limit = Instant::now() + Duration::from_secs(50);
    while !descendants().iter().any(|c| c == "sleep") {
        assert!(Instant::now() < limit, "the bound program never ran");
        std::thread::sleep(Duration::from_millis(5));
    }
    // SAFETY: a signal to the child this test started.
    assert_eq!(unsafe { libc::kill(pid, libc::SIGTERM) }, 0);
    let o = child.wait_with_output().unwrap();
    assert_eq!(o.status.code(), Some(1), "{}\n{}", out(&o), err(&o));
    assert!(out(&o).contains("cancelled"), "{}", out(&o));
    assert!(!out(&o).contains("later:"), "no cycle after the signal ran: {}", out(&o));
}
