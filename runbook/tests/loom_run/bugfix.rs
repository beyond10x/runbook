//! `runbook run --bugfix <dir>` and `--bugfix-ticket <file>`: a bug-fix case, root `BUG-1` on the
//! protocol `sdc-bugfix`, seeded by `sdc-bugfix seed` in the root case's work directory before the
//! tree opens.
//!
//! The gate-lane tests run the library host on a one-step fixture runbook
//! (`tests/fixtures/bugfix-runbook`) with a stand-in `sdc-bugfix` the test writes (POSIX sh): the
//! seed's arguments, its directory, its time and what the runner writes are checked on it. The
//! ticket is invented text that carries a marker string. The slow lane at the end runs the real
//! helper and the real bug-fix runbook on a git repository (`cycles/suite.yaml`:
//! `bugfix-from-ticket`).

use super::core::BIN;
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const MARKER: &str = "TICKET-TEXT-MARKER-5503";

const TICKET: &str = "format: sdc.ticket/1
id: PAGER-7
kind: bug
title: The last partial page is never shown
text: An invented ticket, TICKET-TEXT-MARKER-5503. With 11 items and 5 items per page the third page is empty.
repository:
  path: repo
  base:
    ref: main
    commit: \"0000000000000000000000000000000000000000\"
constraints:
- Keep the public API of the crate unchanged.
";

/// The stand-in helper. It records what it was given beside the seed (`.seed-*` in the work
/// directory) and writes what the real seed writes, a ticket's text included, into the work directory.
const STAND_IN: &str = "#!/bin/sh
[ \"$1\" = seed ] || exit 64
printf '%s\\n' \"$@\" > .seed-args
pwd > .seed-cwd
ls ../store > .seed-store 2>&1
date +%s > .seed-time
case $2 in
  --ticket) { echo 'ticket:'; echo '  id: PAGER-7'; sed 's/^/# /' \"$3\"; } > bug.yaml ;;
  --from) printf 'text: %s\\n' \"$5\" > bug.yaml; printf 'story: %s\\n' \"$7\" >> bug.yaml ;;
  *) exit 65 ;;
esac
mkdir mainline impl
echo 'format: sdc.checks/1' > checks.yaml
";

struct Rig {
    root: PathBuf,
}

impl Rig {
    fn new(tag: &str) -> Rig {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("bugfix-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("runs")).unwrap();
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::create_dir_all(root.join("seed")).unwrap();
        copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bugfix-runbook"), &root.join("fx"));
        std::fs::write(root.join("ticket.yaml"), TICKET).unwrap();
        std::fs::write(root.join("idea.yaml"), "text: the pager drops the last page\n").unwrap();
        std::fs::write(root.join("seed/bug.yaml"), "text: a report in the seed directory\n").unwrap();
        let rig = Rig { root };
        rig.helper(STAND_IN);
        let approve = json!({"approve": true, "cost_micro_usd": 0});
        let settings = json!({"in_process": true, "confinement": "local", "decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [approve]}});
        std::fs::write(rig.root.join("settings.toml"), super::core::toml_text(&settings)).unwrap();
        rig
    }

    /// The helper `sdc-bugfix` the runner finds on `PATH`, with `script` as its text.
    fn helper(&self, script: &str) {
        let file = self.root.join("bin/sdc-bugfix");
        std::fs::write(&file, script).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn p(&self, rel: &str) -> String {
        self.root.join(rel).display().to_string()
    }

    fn run_dir(&self) -> PathBuf {
        self.root.join("runs/r1")
    }

    fn work(&self) -> PathBuf {
        self.run_dir().join("work")
    }

    /// `runbook run <runbook> --run r1 ...` with every required option, and `extra` appended.
    fn args(&self, runbook: &str, extra: &[&str]) -> Vec<String> {
        let mut a: Vec<String> = ["run", runbook, "--run", "r1", "--runs-dir"].iter().map(|s| (*s).to_owned()).collect();
        a.extend([self.p("runs"), "--bindings".into(), self.p("fx/bindings"), "--protocols".into(), self.p("fx/protocol"), "--intake".into(), self.p("idea.yaml")]);
        a.extend(["--confinement".into(), "local".into(), "--settings".into(), self.p("settings.toml"), "--spend-ledger".into(), self.p("ledger.json")]);
        a.extend(["--cap-micro-usd", "1000000", "--max-llm-calls", "100", "--max-wall-seconds", "120"].map(String::from));
        a.extend(extra.iter().map(|s| (*s).to_owned()));
        a
    }

    /// Runs the runner with `PATH` = `path_dirs` and the asset root of the fixture, so that the helper
    /// is the stand-in (or none) and never a build of this checkout.
    fn run_with_path(&self, args: &[String], path_dirs: &str) -> Output {
        Command::new(BIN)
            .args(args)
            .env_clear()
            .env("PATH", path_dirs)
            .env("HOME", "/marker-home-dir")
            .env("RUNBOOK_HOME", self.root.join("fx"))
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    fn run(&self, args: &[String]) -> Output {
        self.run_with_path(args, &format!("{}:{}", self.p("bin"), std::env::var("PATH").unwrap_or_default()))
    }

    fn outcome(&self) -> Value {
        serde_json::from_str(&std::fs::read_to_string(self.run_dir().join("outcome.json")).unwrap()).unwrap()
    }

    fn seen(&self, file: &str) -> String {
        std::fs::read_to_string(self.work().join(file)).unwrap_or_else(|e| panic!("{file}: {e}"))
    }
}

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

fn code(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

fn text(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

/// Every file under `dir` whose bytes hold `needle`, except those below `skip`.
fn files_holding(dir: &Path, needle: &str, skip: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path == skip {
            continue;
        }
        if entry.file_type().unwrap().is_dir() {
            found.extend(files_holding(&path, needle, skip));
        } else if std::fs::read(&path).is_ok_and(|b| b.windows(needle.len()).any(|w| w == needle.as_bytes())) {
            found.push(path);
        }
    }
    found
}

#[test]
fn a_ticket_seeds_the_root_case_before_the_tree_opens() {
    let rig = Rig::new("ticket");
    let o = rig.run(&rig.args("sdc-bugfix", &["--bugfix-ticket", &rig.p("ticket.yaml")]));
    assert_eq!(code(&o), 0, "{}", text(&o));
    let outcome = rig.outcome();
    assert_eq!(outcome["outcome"], "ended", "{outcome}");
    assert_eq!(outcome["reason"], "fixed", "{outcome}");
    // the seed ran in the root case's work directory, with the ticket's absolute path
    assert_eq!(rig.seen(".seed-args"), format!("seed\n--ticket\n{}\n", rig.p("ticket.yaml")));
    let cwd = PathBuf::from(rig.seen(".seed-cwd").trim());
    assert_eq!(cwd.canonicalize().unwrap(), rig.work().canonicalize().unwrap());
    // before the tree opened: the case did not exist in the store yet; afterwards it does
    assert!(!rig.seen(".seed-store").contains("BUG-1"), "the tree was open at seed time: {}", rig.seen(".seed-store"));
    assert!(rig.run_dir().join("store/BUG-1").is_dir(), "the root case is not BUG-1");
    // what the seed wrote is what the first step read (it ended `fixed` on it)
    for d in ["mainline", "impl"] {
        assert!(rig.work().join(d).is_dir(), "{d}/ is missing");
    }
    // the root protocol and the case follow from the option
    let settings: Value = serde_json::from_str(&std::fs::read_to_string(rig.run_dir().join("settings.json")).unwrap()).unwrap();
    assert_eq!(settings["case"], "BUG-1", "{settings}");
    assert_eq!(settings["root_protocol"], "sdc-bugfix@1", "{settings}");
}

#[test]
fn the_runbook_argument_sdc_names_the_bug_fix_runbook_when_an_option_is_given() {
    let rig = Rig::new("runbook-sdc");
    let o = rig.run(&rig.args("sdc", &["--bugfix-ticket", &rig.p("ticket.yaml")]));
    assert_eq!(code(&o), 0, "{}", text(&o));
    assert_eq!(rig.outcome()["reason"], "fixed");
    let settings: Value = serde_json::from_str(&std::fs::read_to_string(rig.run_dir().join("settings.json")).unwrap()).unwrap();
    assert_eq!((settings["case"].as_str(), settings["root_protocol"].as_str()), (Some("BUG-1"), Some("sdc-bugfix@1")));
}

#[test]
fn an_explicit_case_is_kept() {
    let rig = Rig::new("case");
    let o = rig.run(&rig.args("sdc-bugfix", &["--bugfix-ticket", &rig.p("ticket.yaml"), "--case", "BUG-9"]));
    assert_eq!(code(&o), 0, "{}", text(&o));
    assert!(rig.run_dir().join("store/BUG-9").is_dir());
}

#[test]
fn a_seed_directory_seeds_with_the_intake_as_the_report() {
    let rig = Rig::new("seed-dir");
    let o = rig.run(&rig.args("sdc-bugfix", &["--bugfix", &rig.p("seed")]));
    assert_eq!(code(&o), 0, "{}", text(&o));
    assert_eq!(rig.seen(".seed-args"), format!("seed\n--from\n{}\n--text\nthe pager drops the last page\n--story\nstory:bug-r1\n", rig.p("seed")));
    assert_eq!(rig.seen("bug.yaml"), "text: the pager drops the last page\nstory: story:bug-r1\n");
    assert!(rig.run_dir().join("store/BUG-1").is_dir());
}

#[test]
fn the_ticket_text_reaches_the_work_directory_and_no_other_file_stream_or_event() {
    let rig = Rig::new("marker");
    let o = rig.run(&rig.args("sdc-bugfix", &["--bugfix-ticket", &rig.p("ticket.yaml")]));
    assert_eq!(code(&o), 0, "{}", text(&o));
    assert!(rig.seen("bug.yaml").contains(MARKER), "the stand-in did not put the ticket into the work directory: the test proves nothing");
    assert!(!text(&o).contains(MARKER), "the ticket's text is on a stream: {}", text(&o));
    let held = files_holding(&rig.root.join("runs"), MARKER, &rig.work());
    assert!(held.is_empty(), "the ticket's text is in files outside the work directory: {held:?}");
    assert!(!std::fs::read_to_string(rig.p("ledger.json")).unwrap().contains(MARKER));
}

#[test]
fn a_failed_seed_ends_the_run_with_the_last_lines_the_seed_printed_and_no_ticket_text() {
    let rig = Rig::new("seed-fails");
    rig.helper("#!/bin/sh\necho 'seed: ticket PAGER-7: repository is dirty: 1 tracked path(s) changed' >&2\nexit 1\n");
    let o = rig.run(&rig.args("sdc-bugfix", &["--bugfix-ticket", &rig.p("ticket.yaml")]));
    assert_eq!(code(&o), 1, "{}", text(&o));
    assert!(text(&o).contains("is dirty"), "{}", text(&o));
    assert!(text(&o).contains("seed of the bug-fix case failed"), "{}", text(&o));
    let outcome = rig.outcome();
    assert_eq!(outcome["outcome"], "error", "{outcome}");
    assert!(outcome["reason"].as_str().unwrap().contains("is dirty"), "{outcome}");
    assert!(!rig.run_dir().join("store/BUG-1").exists(), "the tree opened after a failed seed");
    assert!(files_holding(&rig.root.join("runs"), MARKER, &rig.work()).is_empty());
    assert!(!text(&o).contains(MARKER));
}

#[test]
fn the_options_are_checked_before_anything_is_written() {
    let rig = Rig::new("refusals");
    let nothing_written = |what: &str| assert!(!rig.run_dir().exists() && !rig.root.join("ledger.json").exists(), "{what}: something was written");
    let o = rig.run(&rig.args("sdc-bugfix", &["--bugfix-ticket", &rig.p("ticket.yaml"), "--bugfix", &rig.p("seed")]));
    assert_eq!(code(&o), 64, "{}", text(&o));
    assert!(text(&o).contains("give one"), "{}", text(&o));
    nothing_written("both options");
    let o = rig.run(&rig.args("sdc-bugfix", &["--bugfix", &rig.p("nothing")]));
    assert_eq!(code(&o), 64, "{}", text(&o));
    assert!(text(&o).contains("--bugfix needs a seed directory"), "{}", text(&o));
    nothing_written("no directory");
    let o = rig.run(&rig.args("sdc-bugfix", &["--bugfix-ticket", &rig.p("nothing.yaml")]));
    assert_eq!(code(&o), 64, "{}", text(&o));
    assert!(text(&o).contains("--bugfix-ticket needs a ticket file"), "{}", text(&o));
    nothing_written("no file");
    let o = rig.run(&rig.args("sdc-story", &["--bugfix-ticket", &rig.p("ticket.yaml")]));
    assert_eq!(code(&o), 64, "{}", text(&o));
    assert!(text(&o).contains("open the bug-fix runbook `sdc-bugfix`, not `sdc-story`"), "{}", text(&o));
    nothing_written("another runbook");
    let o = rig.run(&rig.args("sdc-bugfix", &["--bugfix-ticket", &rig.p("ticket.yaml"), "--root-protocol", "sdc@8"]));
    assert_eq!(code(&o), 2, "{}", text(&o));
    assert!(text(&o).contains("root protocol sdc-bugfix, not sdc@8"), "{}", text(&o));
    nothing_written("another root protocol");
    let o = rig.run_with_path(&rig.args("sdc-bugfix", &["--bugfix-ticket", &rig.p("ticket.yaml")]), &rig.p("nobin"));
    assert_eq!(code(&o), 2, "{}", text(&o));
    assert!(text(&o).contains("sdc-bugfix is not found"), "{}", text(&o));
    nothing_written("no helper");
}

#[test]
fn a_run_without_a_bug_fix_option_does_not_start_the_helper() {
    let rig = Rig::new("none");
    rig.helper("#!/bin/sh\ntouch ../../helper-was-started\nexit 1\n");
    let o = rig.run(&rig.args("sdc-bugfix", &[]));
    // the fixture's step finds no bug.yaml: the run ends, but never through the seed
    assert!(!rig.root.join("helper-was-started").exists() && !rig.root.join("runs/helper-was-started").exists(), "{}", text(&o));
    assert!(!text(&o).contains("seed of the bug-fix case"), "{}", text(&o));
}

// ---- the slow lane: the real helper and the real bug-fix runbook ----

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null"])
        .args(args)
        .output()
        .expect("git");
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_owned()
}

/// A marker in the real test's ticket: the fixture ticket's own text is copied into every run
/// directory with the bindings tree (as a fixture), so the marker, not that text, is what must not leak.
const REAL_MARKER: &str = "REAL-TICKET-MARKER-7719";

/// The scratch directory of the real cycle: the invented ticket
/// `driver/examples/sdc/fixtures/bugfix-pager/ticket.yaml` with the marker and the base commit of a git
/// repository (the pager crate with an ignored and an untracked file), and the run settings.
fn real_scratch(tag: &str) -> PathBuf {
    let root = repo_root();
    assert!(root.join("driver/target/debug/sdc-bugfix").is_file(), "{} holds no built driver: run `cargo build --locked --bins` in driver/", root.join("driver/target/debug").display());
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("bugfix-real-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let input = scratch.join("input");
    let fixture = root.join("driver/examples/sdc/fixtures/bugfix-pager");
    copy_tree(&fixture.join("crate"), &input.join("repo"));
    std::fs::write(input.join("repo/.gitignore"), "ignored.txt\n").unwrap();
    git(&input.join("repo"), &["init", "-q"]);
    git(&input.join("repo"), &["add", "."]);
    git(&input.join("repo"), &["commit", "-q", "-m", "fixture"]);
    std::fs::write(input.join("repo/ignored.txt"), "ignored\n").unwrap();
    std::fs::write(input.join("repo/untracked.txt"), "untracked\n").unwrap();
    let head = git(&input.join("repo"), &["rev-parse", "HEAD"]);
    let ticket = std::fs::read_to_string(fixture.join("ticket.yaml"))
        .unwrap()
        .replace("0000000000000000000000000000000000000000", &head)
        .replace("An invented ticket.", &format!("An invented ticket, {REAL_MARKER}."));
    assert!(ticket.contains(REAL_MARKER));
    std::fs::write(input.join("ticket.yaml"), ticket).unwrap();
    std::fs::write(scratch.join("idea.yaml"), "text: a ticket stands in for the report\n").unwrap();
    let answers: Vec<Value> = (0..3).map(|_| json!({"approve": true, "cost_micro_usd": 0})).collect();
    let settings = json!({"decider": {"kind": "scripted", "principal": "person:owner", "max_call_micro_usd": 0, "answers": answers}});
    std::fs::write(scratch.join("settings.toml"), super::core::toml_text(&settings)).unwrap();
    scratch
}

/// `runbook run sdc-bugfix --bugfix-ticket` on the real bindings, protocols and helpers of this
/// checkout, the stub adapter, scripted approvals; `extra` is appended to the options.
fn real_run(scratch: &Path, name: &str, extra: &[&str]) -> Output {
    let root = repo_root();
    let path = format!("{}:{}", root.join("driver/target/debug").display(), std::env::var("PATH").unwrap_or_default());
    let mut args: Vec<String> = ["run", "sdc-bugfix", "--run", name, "--runs-dir"].map(String::from).to_vec();
    args.push(scratch.join("runs").display().to_string());
    args.extend(["--bindings".into(), root.join("bindings").display().to_string(), "--protocols".into(), root.join("protocol").display().to_string()]);
    args.extend(["--intake".into(), scratch.join("idea.yaml").display().to_string(), "--confinement".into(), "local".into()]);
    args.extend(["--settings".into(), scratch.join("settings.toml").display().to_string(), "--spend-ledger".into(), scratch.join("ledger.json").display().to_string()]);
    args.extend(["--cap-micro-usd", "1000000", "--max-llm-calls", "100", "--max-wall-seconds", "600"].map(String::from));
    args.extend(["--bugfix-ticket".into(), scratch.join("input/ticket.yaml").display().to_string()]);
    args.extend(extra.iter().map(|s| (*s).to_owned()));
    let out = Command::new(BIN)
        .args(&args)
        .env_clear()
        .env("PATH", &path)
        .env("HOME", std::env::var_os("HOME").unwrap_or_default())
        .env("RUNBOOK_HOME", &root)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    std::fs::write(scratch.join(format!("{name}-console.txt")), text(&out)).unwrap();
    out
}

fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        if e.file_type().unwrap().is_dir() {
            walk(&e.path(), base, out);
        } else {
            out.push(e.path().strip_prefix(base).unwrap().display().to_string());
        }
    }
}

/// What `cycles/suite.yaml` `bugfix-from-ticket` expects of the run `name`, except the prompt.
fn check_real_run(scratch: &Path, name: &str, console: &str) {
    let run = scratch.join("runs").join(name);
    let work = run.join("work");
    let outcome: Value = serde_json::from_str(&std::fs::read_to_string(run.join("outcome.json")).unwrap()).unwrap();
    assert_eq!((outcome["outcome"].as_str(), outcome["reason"].as_str()), (Some("ended"), Some("fixed")), "{outcome}");
    // only the files git tracks are seeded
    let mainline = work.join("mainline");
    assert!(!mainline.join("ignored.txt").exists() && !mainline.join("untracked.txt").exists());
    let mut seeded: Vec<String> = Vec::new();
    walk(&mainline, &mainline, &mut seeded);
    seeded.sort();
    let tracked = git(&scratch.join("input/repo"), &["ls-files"]);
    let mut want: Vec<&str> = tracked.lines().collect();
    want.sort();
    assert_eq!(seeded, want);
    // the base is recorded, and the ticket's story is the case's
    let seed: Value = serde_json::from_str(&std::fs::read_to_string(work.join(".sdc/seed.json")).unwrap()).unwrap();
    assert_eq!(seed["ticket"]["id"], "PAGER-7");
    assert_eq!(seed["repository"]["base"]["commit"], git(&scratch.join("input/repo"), &["rev-parse", "HEAD"]));
    let bug: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(work.join("bug.yaml")).unwrap()).unwrap();
    assert_eq!(bug["story"].as_str(), Some("story:ticket-pager-7"));
    assert_eq!(bug["ticket"]["id"].as_str(), Some("PAGER-7"));
    assert_eq!(bug["constraints"].as_sequence().map(Vec::len), Some(2));
    assert!(std::fs::read_to_string(work.join("aep/story/ticket-pager-7.md")).unwrap().lines().any(|l| l.starts_with("status: implemented")));
    // the ticket's text is in the work directory (bug.yaml) and nowhere else the runner writes, nor on the console
    assert!(std::fs::read_to_string(work.join("bug.yaml")).unwrap().contains(REAL_MARKER));
    let held = files_holding(&run, REAL_MARKER, &work);
    assert!(held.is_empty(), "the ticket's text is in files outside the work directory: {held:?}");
    for phrase in [REAL_MARKER, "never shown", "multiple of the page size"] {
        assert!(!console.contains(phrase), "the ticket's text is on the console: {console}");
    }
}

#[test]
#[ignore = "slow lane `task test:bugfix-slow`: needs driver/target/debug built (`cargo build --locked --bins` in driver/); the real cycle, no model call"]
fn bugfix_from_ticket_ends_fixed_on_a_git_repository_and_a_dirty_one_is_refused() {
    let scratch = real_scratch("stub");
    let started = std::time::Instant::now();
    let run = real_run(&scratch, "bugfix-from-ticket", &[]);
    eprintln!("the real cycle took {:?}", started.elapsed());
    assert_eq!(code(&run), 0, "{}", text(&run));
    check_real_run(&scratch, "bugfix-from-ticket", &text(&run));
    // the same repository, dirty, is refused before any step: exit 1 and the seed's own line
    let lib = scratch.join("input/repo/src/lib.rs");
    std::fs::write(&lib, format!("{}\npub fn changed() {{}}\n", std::fs::read_to_string(&lib).unwrap())).unwrap();
    let dirty = real_run(&scratch, "bugfix-from-ticket-dirty", &[]);
    assert_eq!(code(&dirty), 1, "{}", text(&dirty));
    assert!(text(&dirty).contains("is dirty"), "{}", text(&dirty));
    for phrase in [REAL_MARKER, "never shown", "multiple of the page size"] {
        assert!(!text(&dirty).contains(phrase), "the ticket's text is on the console: {}", text(&dirty));
    }
    assert!(!scratch.join("runs/bugfix-from-ticket-dirty/store/BUG-1").exists(), "a step ran on a dirty repository");
    let held = files_holding(&scratch.join("runs/bugfix-from-ticket-dirty"), REAL_MARKER, &scratch.join("runs/bugfix-from-ticket-dirty/work"));
    assert!(held.is_empty(), "{held:?}");
}

/// The prompt of the last `change.fix` call of a run: `sdc-node` makes the call directory
/// `work/.sdc/llm/<NNN>-change.fix/` in the root case's work directory (`docs/loom-run-directory.md`)
/// and writes `prompt.md` there before it starts the adapter.
fn fix_prompt(run: &Path) -> Option<String> {
    let mut files = Vec::new();
    walk(&run.join("work/.sdc/llm"), run, &mut files);
    files.sort();
    let last = files.into_iter().rfind(|f| f.ends_with("-change.fix/prompt.md"))?;
    std::fs::read_to_string(run.join(last)).ok()
}

#[test]
#[ignore = "slow lane `task test:bugfix-slow`: needs driver/target/debug built (`cargo build --locked --bins` in driver/); the real cycle on the adapter dry, no model call"]
fn bugfix_from_ticket_with_dry_ends_fixed_and_the_fix_prompt_carries_the_constraints() {
    let scratch = real_scratch("dry");
    // the effort of a class is on the dry command of its nodes (`efforts` of `cycles/suite.yaml`): the
    // cycle runs bug.dissect (class other), test.reproduce and change.fix (class implement)
    let file = scratch.join("settings.toml");
    let mut settings: Value = super::core::from_toml(&std::fs::read_to_string(&file).unwrap());
    settings["efforts"] = json!({"other": {"effort": "xhigh"}, "implement": {"effort": "low"}});
    std::fs::write(&file, super::core::toml_text(&settings)).unwrap();
    let run = real_run(&scratch, "bugfix-from-ticket", &["--dry"]);
    assert_eq!(code(&run), 0, "{}", text(&run));
    check_real_run(&scratch, "bugfix-from-ticket", &text(&run));
    let commands = std::fs::read_to_string(scratch.join("runs/bugfix-from-ticket/dry-commands.txt")).unwrap();
    for (node, level) in [("bug.dissect", "xhigh"), ("test.reproduce", "low"), ("change.fix", "low")] {
        let line = commands.lines().find(|l| l.contains(&format!("-{node}/"))).unwrap_or_else(|| panic!("no dry command of {node}: {commands}"));
        assert!(line.contains(&format!("'--effort' '{level}'")), "{node}: {line}");
    }
    // the dry adapter writes each prompt: the constraints are in the fix's, a constraint the ticket lacks is not
    let prompt = fix_prompt(&scratch.join("runs/bugfix-from-ticket")).expect("the dry adapter wrote no change.fix prompt");
    assert!(prompt.contains("- Keep the public API of the crate unchanged."), "{prompt}");
    assert!(!prompt.contains("Standard library only"));
}
