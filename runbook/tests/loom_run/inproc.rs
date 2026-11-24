//! `runbook run` in process: the run loop over Loom as a library, on the small `sh` fixture
//! runbook of the engine tests (`tests/fixtures/engine-runbook`: a root case that opens a drafting
//! case; the drafting case has a writer program and one person step, the review). No `b10x-loom`
//! binary, no model call: every approval is scripted and the bound programs are POSIX sh.

use super::core::BIN;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

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

struct Rig {
    root: PathBuf,
}

impl Rig {
    fn new(tag: &str) -> Rig {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("inproc-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("runs")).unwrap();
        copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &root.join("fx"));
        std::fs::write(root.join("idea.yaml"), "text: a note on tea\n").unwrap();
        super::decider::install_routing_table_at(&root.join("fx"));
        let rig = Rig { root };
        rig.answers(&[json!({"approve": true, "cost_micro_usd": 0})], 0);
        rig
    }

    /// The scripted decider: `answers` in order, each call reserving up to `max_call` micro USD.
    fn answers(&self, answers: &[Value], max_call: u64) {
        // the `loom` member is what `sdc-experiment run` writes for a record that runs the library
        let settings = json!({"in_process": true, "confinement": "local", "decider": {"kind": "scripted", "max_call_micro_usd": max_call, "answers": answers}});
        self.settings(&settings);
    }

    fn settings(&self, settings: &Value) {
        std::fs::write(self.root.join("settings.toml"), super::core::toml_text(settings)).unwrap();
    }

    /// The writer of the fixture replaced by `script`.
    fn writer(&self, script: &str) {
        let file = self.root.join("fx/bindings/tools/draft-write.sh");
        std::fs::write(&file, script).unwrap();
    }

    fn run_dir(&self, name: &str) -> PathBuf {
        self.root.join("runs").join(name)
    }

    fn work(&self, name: &str) -> PathBuf {
        self.run_dir(name).join("work")
    }

    fn ledger(&self) -> PathBuf {
        self.root.join("ledger.json")
    }

    /// The full option list for run `name`, the library way.
    fn args(&self, name: &str, cap: u64) -> Vec<String> {
        let r = |p: &str| self.root.join(p).display().to_string();
        let mut a: Vec<String> = ["run", "sdc", "--run", name, "--runs-dir"].iter().map(|s| (*s).to_owned()).collect();
        a.extend([
            r("runs"),
            "--bindings".into(),
            r("fx/bindings"),
            "--protocols".into(),
            r("fx/protocol"),
            "--root-protocol".into(),
            "engine-demo@1".into(),
            "--case".into(),
            "ROOT-1".into(),
            "--intake".into(),
            r("idea.yaml"),
            "--confinement".into(),
            "local".into(),
            "--settings".into(),
            r("settings.toml"),
            "--spend-ledger".into(),
            r("ledger.json"),
            "--cap-micro-usd".into(),
            cap.to_string(),
            "--max-llm-calls".into(),
            "100".into(),
            "--max-wall-seconds".into(),
            "120".into(),
        ]);
        a
    }

    fn command(&self, args: &[String]) -> Command {
        let mut c = Command::new(BIN);
        c.args(args)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", "/marker-home-dir")
            .env("RUNBOOK_MARKER_VARIABLE", "marker-value-8842")
            .env("ANTHROPIC_API_KEY", "sk-fake-token-5517")
            .stdin(Stdio::null());
        c
    }

    fn run(&self, args: &[String]) -> Output {
        self.command(args).output().unwrap()
    }

    fn outcome(&self, name: &str) -> Value {
        let text = std::fs::read_to_string(self.run_dir(name).join("outcome.json")).unwrap_or_else(|e| panic!("no outcome.json: {e}"));
        serde_json::from_str(&text).unwrap()
    }

    fn events(&self, name: &str) -> Vec<Value> {
        let text = std::fs::read_to_string(self.run_dir(name).join("events.jsonl")).unwrap();
        text.lines().map(|l| serde_json::from_str(l).unwrap()).collect()
    }
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn without(args: &[String], flag: &str, takes_value: bool) -> Vec<String> {
    let mut out = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == flag {
            if takes_value {
                it.next();
            }
            continue;
        }
        out.push(a.clone());
    }
    out
}

fn approve() -> Value {
    json!({"approve": true, "cost_micro_usd": 0})
}

/// The command names of every process below `root`.
fn descendants(root: u32) -> Vec<String> {
    let mut table: Vec<(u32, u32, String)> = Vec::new();
    for e in std::fs::read_dir("/proc").unwrap().flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else { continue };
        let Ok(stat) = std::fs::read_to_string(e.path().join("stat")) else { continue };
        let (Some(open), Some(close)) = (stat.find('('), stat.rfind(')')) else { continue };
        let ppid = stat[close + 1..].split_whitespace().nth(1).and_then(|p| p.parse::<u32>().ok()).unwrap_or(0);
        table.push((pid, ppid, stat[open + 1..close].to_string()));
    }
    let mut below: BTreeSet<u32> = BTreeSet::from([root]);
    loop {
        let more: Vec<u32> = table.iter().filter(|(p, pp, _)| below.contains(pp) && !below.contains(p)).map(|(p, _, _)| *p).collect();
        if more.is_empty() {
            break;
        }
        below.extend(more);
    }
    table.into_iter().filter(|(p, _, _)| *p != root && below.contains(p)).map(|(_, _, c)| c).collect()
}

/// The commit `Cargo.lock` pins for `b10x-loom-tree`, read here by line, independently of `build.rs`.
fn locked_commit() -> String {
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.lock")).unwrap();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line == "name = \"b10x-loom-tree\"" {
            let source = lines.find(|l| l.starts_with("source = ")).expect("a source line");
            return source.rsplit('#').next().unwrap().trim_end_matches('"').to_owned();
        }
    }
    panic!("Cargo.lock holds no b10x-loom-tree");
}

// ---- the way to Loom: the library, or the binary with its digest ----

#[test]
fn a_binary_without_its_digest_or_a_digest_without_its_binary_is_refused_and_writes_nothing() {
    let rig = Rig::new("usage");
    let library = rig.args("r1", 1_000_000);
    let hex = "0".repeat(64);
    let mut loom_alone = library.clone();
    loom_alone.extend(["--loom".into(), "/bin/true".into()]);
    let mut sha_alone = library;
    sha_alone.extend(["--loom-sha256".into(), hex]);
    for (what, args) in [("--loom without its digest", loom_alone), ("a digest without --loom", sha_alone)] {
        let o = rig.run(&args);
        assert_eq!(code(&o), 2, "{what}: {}", stderr(&o));
        assert!(stderr(&o).contains("usage"), "{what}: {}", stderr(&o));
        assert!(!rig.run_dir("r1").exists(), "{what}: a run directory was made");
        assert!(!rig.ledger().exists(), "{what}: a ledger was made");
    }
}

#[test]
fn the_options_of_a_run_in_process_that_are_required_are_named_when_missing_and_detach_only_options_are_refused() {
    let rig = Rig::new("required");
    for flag in ["--run", "--runs-dir", "--settings", "--spend-ledger", "--cap-micro-usd", "--max-llm-calls", "--max-wall-seconds", "--confinement"] {
        let o = rig.run(&without(&rig.args("r1", 1_000_000), flag, true));
        assert_eq!(code(&o), 64, "{flag}: {}", stderr(&o));
        assert!(stderr(&o).contains(flag), "{flag} is not named: {}", stderr(&o));
        assert!(!rig.run_dir("r1").exists() && !rig.ledger().exists(), "{flag}: something was written");
    }
    let o = rig.run(&without(&rig.args("r1", 1_000_000), "--intake", true));
    assert_eq!(code(&o), 64, "{}", stderr(&o));
    assert!(stderr(&o).contains("--intake"), "{}", stderr(&o));
    for (flag, value) in [("--context", Some(".")), ("--set", Some("a=b")), ("--daemon", Some("http://127.0.0.1:1")), ("--json", None)] {
        let mut args = rig.args("r1", 1_000_000);
        args.push(flag.into());
        args.extend(value.map(String::from));
        let o = rig.run(&args);
        assert_eq!(code(&o), 64, "{flag}: {}", stderr(&o));
        assert!(stderr(&o).contains(flag), "{flag} is not named: {}", stderr(&o));
        assert!(!rig.run_dir("r1").exists() && !rig.ledger().exists(), "{flag}: something was written");
    }
    let mut detached = rig.args("r1", 1_000_000);
    detached.push("--detach".into());
    let o = rig.run(&detached);
    assert_eq!(code(&o), 64, "an option of a run in process with --detach: {}", stderr(&o));
}

#[test]
fn the_confinement_stays_required_with_no_default() {
    let rig = Rig::new("confinement-required");
    let o = rig.run(&without(&rig.args("r1", 1_000_000), "--confinement", true));
    assert_ne!(code(&o), 0, "{}", stderr(&o));
    assert!(stderr(&o).contains("--confinement"), "{}", stderr(&o));
    assert!(!rig.run_dir("r1").exists());
}

#[test]
fn a_level_the_library_cannot_give_is_refused_before_anything_is_written() {
    if runbook::engine::calls::host_levels().contains(&runbook::runtime::Confinement::Linux) {
        eprintln!("this host delegates a cgroup: the linux level is offered, so there is no level to refuse here");
        return;
    }
    let rig = Rig::new("level");
    let mut args = rig.args("r1", 1_000_000);
    let i = args.iter().position(|a| a == "--confinement").unwrap();
    args[i + 1] = "substrate".into();
    let o = rig.run(&args);
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(stderr(&o).contains("--confinement substrate is not a level the library can give here"), "{}", stderr(&o));
    assert!(!stderr(&o).contains("ConfinementRefused:"), "a library run refuses the level in its own words, not with the reason's name: {}", stderr(&o));
    assert!(!rig.run_dir("r1").exists(), "a run directory was made");
    assert!(!rig.ledger().exists(), "a ledger was made");
}

#[test]
fn the_settings_file_must_agree_with_the_way_the_options_name() {
    let rig = Rig::new("settings");
    let decider = json!({"kind": "scripted", "max_call_micro_usd": 0, "answers": [approve()]});
    for (what, members, want) in [
        ("a digest under --in-process", json!({"in_process": true, "loom_sha256": "0".repeat(64)}), "loom_sha256"),
        ("in_process false", json!({"in_process": false}), "in_process"),
        ("a binary path", json!({"in_process": true, "loom": {"bin": "/opt/b10x-loom"}}), "loom.bin"),
    ] {
        let mut settings = members;
        settings["decider"] = decider.clone();
        rig.settings(&settings);
        let o = rig.run(&rig.args("r1", 1_000_000));
        assert_eq!(code(&o), 2, "{what}: {}", stderr(&o));
        assert!(stderr(&o).contains(want), "{what}: {}", stderr(&o));
        assert!(!rig.run_dir("r1").exists(), "{what}: a run directory was made");
    }
    // `in_process` under a child (a binary and its digest; the runbook binary stands in, the
    // settings are refused before any call) is refused too
    rig.settings(&json!({"in_process": true, "decider": decider}));
    let digest = Command::new("sha256sum").arg(BIN).output().expect("sha256sum").stdout;
    let digest = String::from_utf8(digest).unwrap().split_whitespace().next().unwrap().to_owned();
    let mut args = rig.args("r1", 1_000_000);
    args.extend(["--loom".into(), BIN.into(), "--loom-sha256".into(), digest]);
    let o = rig.run(&args);
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(!rig.run_dir("r1").exists());
}

// ---- the runs ----

#[test]
fn a_run_is_delivered_with_scripted_approvals_and_records_the_library_it_used() {
    let rig = Rig::new("delivered");
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let outcome = rig.outcome("r1");
    assert_eq!(outcome["outcome"], "ended", "{outcome}");
    assert_eq!(outcome["reason"], "delivered", "{outcome}");
    assert_eq!(std::fs::read_to_string(rig.work("r1").join("draft.txt")).unwrap(), "draft\n");
    // the files a run directory holds under the child
    let dir = rig.run_dir("r1");
    for file in ["outcome.json", "events.jsonl", "settings.json", "run-settings.jsonl", "decisions.jsonl", "tree-status.json", "tree-verify.json", "status.json"] {
        assert!(dir.join(file).is_file(), "{file} is missing");
    }
    for sub in ["store", "work", "bindings", "protocols"] {
        assert!(dir.join(sub).is_dir(), "{sub}/ is missing");
    }
    // the members only a child has are null, never absent
    assert!(outcome.get("loom_exit_code").is_some_and(Value::is_null), "{outcome}");
    assert!(outcome.get("stderr_tail").is_some_and(Value::is_null), "{outcome}");
    let ended = rig.events("r1").into_iter().find(|e| e["event"] == "ended").unwrap();
    assert!(ended["loom_exit_code"].is_null() && ended["stderr_tail"].is_null(), "{ended}");
    // the Loom used: the pinned commit, no digest
    let settings: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("settings.json")).unwrap()).unwrap();
    assert_eq!(settings["loom"]["in_process"], true, "{settings}");
    assert_eq!(settings["loom"]["commit"], locked_commit(), "{settings}");
    assert!(settings["loom"].get("sha256").is_none(), "{settings}");
    assert_eq!(settings["loom"]["confinement"], "local");
    let started = rig.events("r1").into_iter().find(|e| e["event"] == "run_started").unwrap();
    assert_eq!(started["settings"]["loom"]["commit"], locked_commit());
    // the review was answered as the principal the bindings bind the action to
    let answered: Vec<Value> = rig.events("r1").into_iter().filter(|e| e["event"] == "decision_answered").collect();
    assert_eq!(answered.len(), 1, "{answered:?}");
    assert_eq!(answered[0]["principal"], "reviewer");
    assert_eq!(answered[0]["approve"], true);
}

#[test]
fn a_rejection_with_reasons_sends_the_writer_back() {
    let rig = Rig::new("rejected");
    rig.answers(&[json!({"approve": false, "reasons": "the draft needs a title line", "cost_micro_usd": 0}), approve()], 0);
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(rig.outcome("r1")["reason"], "delivered");
    let work = rig.work("r1");
    assert_eq!(std::fs::read_to_string(work.join("draft.txt")).unwrap(), "draft\ndraft\n", "the writer ran twice");
    let feedback = std::fs::read_to_string(work.join("feedback.json")).unwrap();
    assert!(feedback.contains("the draft needs a title line"), "the writer was not given the reasons: {feedback}");
    assert!(feedback.contains("\"result\":\"rejected\""), "{feedback}");
    let answered: Vec<Value> = rig.events("r1").into_iter().filter(|e| e["event"] == "decision_answered").collect();
    assert_eq!(answered.iter().map(|a| a["approve"].as_bool().unwrap()).collect::<Vec<_>>(), [false, true]);
    assert!(answered.iter().all(|a| a["principal"] == "reviewer"), "{answered:?}");
}

#[test]
fn the_spend_cap_ends_the_run_as_it_does_under_the_child() {
    let rig = Rig::new("cap");
    // the one decision costs the whole cap; the writer the rejection sends back is never started
    rig.answers(&[json!({"approve": false, "reasons": "again", "cost_micro_usd": 400_000}), approve()], 400_000);
    let o = rig.run(&rig.args("r1", 400_000));
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    let outcome = rig.outcome("r1");
    assert_eq!(outcome["outcome"], "stopped: spend cap", "{outcome}");
    assert_eq!(outcome["spent_micro_usd"], 400_000, "{outcome}");
    let ledger: Value = serde_json::from_str(&std::fs::read_to_string(rig.ledger()).unwrap()).unwrap();
    assert_eq!(ledger["spent_micro_usd"], 400_000, "{ledger}");
    assert_eq!(std::fs::read_to_string(rig.work("r1").join("draft.txt")).unwrap(), "draft\n", "the writer ran again after the cap");
    assert!(rig.events("r1").iter().any(|e| e["event"] == "cap_reached" && e["cap"] == "spend"));
}

#[test]
fn only_the_programs_of_the_runbook_run_and_never_a_loom_canon_or_driver_process() {
    let rig = Rig::new("tree");
    rig.writer("#!/bin/sh\nsleep 3\necho draft >> draft.txt\n");
    let mut child = rig.command(&rig.args("r1", 1_000_000)).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let pid = child.id();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let limit = Instant::now() + Duration::from_secs(50);
    let status = loop {
        seen.extend(descendants(pid));
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < limit, "the run did not end; the sample saw {seen:?}");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(status.success(), "{status:?}; the sample saw {seen:?}");
    assert!(seen.contains("sleep"), "the sample saw {seen:?}");
    for name in &seen {
        assert!(!FORBIDDEN.contains(&name.as_str()), "a forbidden process ran: {seen:?}");
        assert!(matches!(name.as_str(), "sh" | "sleep" | "cp" | "cat" | "draft-write.sh" | "draft-check.sh"), "an unexpected process ran: {name} in {seen:?}");
    }
}

// ---- the decider's answers for a declared step ----

const ANSWERS_FILE: &str = "format: runbook.loom-run-answers/1
answers:
- action: draft.review
  on: ApproveAction
  requests: [tools/ask-requests.sh]
  apply: [tools/ask-apply.sh, '{id}', '{answer}', '{principal}']
  then: [tools/ask-then.sh, '{principal}', '{case}']
";

/// The approve rule decides inside the runner and answers a question with its default, `A`.
#[test]
fn a_declared_answer_is_given_on_the_library_host_through_the_same_loop() {
    let rig = Rig::new("declared");
    let tools = rig.root.join("fx/bindings/tools");
    let exe = |name: &str, body: &str| {
        let file = tools.join(name);
        std::fs::write(&file, format!("#!/bin/sh\n{body}")).unwrap();
        std::fs::set_permissions(&file, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    };
    std::fs::write(tools.join("answers.yaml"), ANSWERS_FILE).unwrap();
    exe("ask-requests.sh", "printf '%s\\n' '{\"type\":\"question\",\"id\":\"q1\",\"question\":{\"id\":\"q1\",\"default\":\"A\"}}'\n");
    exe("ask-apply.sh", "printf '%s %s %s\\n' \"$1\" \"$2\" \"$3\" >> asked.txt\n");
    exe("ask-then.sh", "printf '%s %s\\n' \"$1\" \"$2\" >> then.txt\n");
    rig.settings(&json!({"in_process": true, "confinement": "local", "autonomy": "A5", "decider": {"kind": "rule", "verdict": "approve"}}));
    let started = Instant::now();
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(started.elapsed() < Duration::from_secs(20), "slow: {:?}", started.elapsed());
    assert_eq!(rig.outcome("r1")["reason"], "delivered");
    let work = rig.work("r1");
    assert_eq!(std::fs::read_to_string(work.join("asked.txt")).unwrap(), "q1 A decider:approve\n", "the answer was not applied by the program of the file");
    assert_eq!(std::fs::read_to_string(work.join("then.txt")).unwrap(), format!("decider:approve {}\n", then_case(&rig)),"the `then` program did not run once");
    let events = rig.events("r1");
    let taken: Vec<&Value> = events.iter().filter(|e| e["event"] == "declared_taken").collect();
    assert_eq!(taken.len(), 1, "{taken:?}");
    assert_eq!(taken[0]["principal"], "decider:approve");
    assert!(events.iter().all(|e| e["event"] != "decision_answered"), "the decision went to the decider as an ordinary approval");
    let decisions = std::fs::read_to_string(rig.run_dir("r1").join("decisions.jsonl")).unwrap();
    assert!(decisions.contains("\"question\":\"q1\"") && decisions.contains("\"answer\":\"A\""), "{decisions}");
}

const ANSWERS_FILE_WITHOUT_THEN: &str = "format: runbook.loom-run-answers/1
answers:
- action: draft.review
  on: ApproveAction
  requests: [tools/ask-requests.sh]
  apply: [tools/ask-apply.sh, '{id}', '{answer}', '{principal}']
";

/// The way `scope.answer` is bound in `bindings/sdc-intake.yaml`: the answers file has no `then`, the
/// step's own program is run by the engine at the approval, and gets the principal the decider took
/// the step as (the copy of the bindings binds the step to it).
#[test]
fn a_declared_step_with_a_program_gets_the_decider_as_principal_from_the_engine_and_no_then_runs() {
    let rig = Rig::new("declared-program");
    let tools = rig.root.join("fx/bindings/tools");
    let exe = |name: &str, body: &str| {
        let file = tools.join(name);
        std::fs::write(&file, format!("#!/bin/sh\n{body}")).unwrap();
        std::fs::set_permissions(&file, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    };
    std::fs::write(tools.join("answers.yaml"), ANSWERS_FILE_WITHOUT_THEN).unwrap();
    exe("ask-requests.sh", "printf '%s\\n' '{\"type\":\"question\",\"id\":\"q1\",\"question\":{\"id\":\"q1\",\"default\":\"A\"}}'\n");
    exe("ask-apply.sh", "printf '%s %s %s\\n' \"$1\" \"$2\" \"$3\" >> asked.txt\n");
    review_program(&rig, "#!/bin/sh\nprintf '%s\\n' \"$1\" >> review-note.txt\n");
    rig.settings(&json!({"in_process": true, "confinement": "local", "autonomy": "A5", "decider": {"kind": "rule", "verdict": "approve"}}));
    let started = Instant::now();
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(started.elapsed() < Duration::from_secs(20), "slow: {:?}", started.elapsed());
    assert_eq!(rig.outcome("r1")["reason"], "delivered");
    let work = rig.work("r1");
    assert_eq!(std::fs::read_to_string(work.join("asked.txt")).unwrap(), "q1 A decider:approve\n");
    assert_eq!(std::fs::read_to_string(work.join("review-note.txt")).unwrap(), "decider:approve\n", "the engine ran the step's program once, as the decider");
    assert_eq!(rig.events("r1").iter().filter(|e| e["event"] == "declared_taken").count(), 1);
}

/// The case of the review decision, read from the run's own events.
fn then_case(rig: &Rig) -> String {
    let events = rig.events("r1");
    let taken = events.iter().find(|e| e["event"] == "declared_taken").unwrap();
    taken["decision"]["case"].as_str().unwrap().to_owned()
}

// ---- the environment of a bound program ----

#[test]
fn a_bound_program_gets_path_and_loom_variables_and_nothing_of_the_runbook_process() {
    let rig = Rig::new("env");
    rig.writer("#!/bin/sh\nenv > \"$PWD/env-dump.txt\"\necho draft >> draft.txt\n");
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let dump = std::fs::read_to_string(rig.work("r1").join("env-dump.txt")).unwrap();
    for needle in ["marker-value-8842", "RUNBOOK_MARKER_VARIABLE", "sk-fake-token-5517", "ANTHROPIC_API_KEY", "/marker-home-dir"] {
        assert!(!dump.contains(needle), "{needle} reached the bound program:\n{dump}");
    }
    let names: BTreeSet<&str> = dump.lines().filter_map(|l| l.split_once('=').map(|(k, _)| k)).collect();
    assert!(names.contains("PATH"), "{dump}");
    assert!(names.iter().any(|n| n.starts_with("LOOM_")), "{dump}");
    for name in &names {
        // PWD, SHLVL and _ are the shell's own
        assert!(*name == "PATH" || name.starts_with("LOOM_") || matches!(*name, "PWD" | "SHLVL" | "_" | "OLDPWD"), "{name} is in the environment of the bound program:\n{dump}");
    }
}

// ---- a person's step that runs a program (Loom story person-action-runs-a-program) ----

/// The review step of the fixture bound to a program `tools/review-note.sh` with `script`, given the
/// principal in one whole argv entry.
fn review_program(rig: &Rig, script: &str) {
    use std::os::unix::fs::PermissionsExt;
    let bindings = rig.root.join("fx/bindings");
    let file = bindings.join("engine-draft.yaml");
    let text = std::fs::read_to_string(&file).unwrap();
    let bound = text.replace("      principal: reviewer\n", "      principal: reviewer\n      argv: [tools/review-note.sh, \"{principal}\"]\n");
    assert_ne!(bound, text, "the review step was not found");
    std::fs::write(&file, bound).unwrap();
    let tool = bindings.join("tools/review-note.sh");
    std::fs::write(&tool, script).unwrap();
    std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn the_approval_of_a_person_step_runs_its_program_with_the_approving_principal() {
    let rig = Rig::new("person-program");
    review_program(&rig, "#!/bin/sh\nprintf '%s\\n' \"$1\" >> review-note.txt\n");
    let started = Instant::now();
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(started.elapsed() < Duration::from_secs(60), "slow: {:?}", started.elapsed());
    assert_eq!(rig.outcome("r1")["reason"], "delivered");
    assert_eq!(std::fs::read_to_string(rig.work("r1").join("review-note.txt")).unwrap(), "reviewer\n", "the program ran once, with the principal of the approval");
}

#[test]
fn a_person_program_that_fails_ends_the_run_with_the_engines_refusal() {
    let rig = Rig::new("person-program-fails");
    review_program(&rig, "#!/bin/sh\nexit 3\n");
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 12, "a refusal of Loom by a named reason has its own exit code: {}", stderr(&o));
    let outcome = rig.outcome("r1");
    assert_eq!((outcome["outcome"].as_str(), outcome["cause"].as_str()), (Some("refused"), Some("person_program_failed")), "{outcome}");
    assert!(outcome["reason"].as_str().unwrap().contains("PersonProgramFailed"), "{outcome}");
    let stopped: Value = serde_json::from_str(&std::fs::read_to_string(rig.run_dir("r1").join("run-stopped.json")).unwrap()).unwrap();
    assert_eq!(stopped["cause"], "person_program_failed", "{stopped}");
    assert!(!rig.work("r1").join("review-note.txt").exists());
}

#[test]
fn a_bound_program_ended_by_a_signal_ends_the_run_with_the_signal_as_the_cause_not_stuck() {
    let rig = Rig::new("program-signalled");
    rig.writer("#!/bin/sh\nexit 137\n");
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 12, "a tree left stuck by a program that a signal ended is a refusal of Loom by name, not exit 3: {}", stderr(&o));
    let outcome = rig.outcome("r1");
    assert_eq!((outcome["outcome"].as_str(), outcome["cause"].as_str(), outcome["exit_code"].as_u64()), (Some("refused"), Some("program_signalled"), Some(12)), "{outcome}");
    assert!(outcome["reason"].as_str().unwrap().starts_with("ProgramSignalled:"), "{outcome}");
    let stopped: Value = serde_json::from_str(&std::fs::read_to_string(rig.run_dir("r1").join("run-stopped.json")).unwrap()).unwrap();
    assert_eq!(stopped["cause"], "program_signalled", "{stopped}");
}

/// The writer listed an exit in `exits.error` and the program ended with it: Loom refuses the step
/// `ProgramError`, records nothing and leaves the tree without a move. The run says so: `refused`,
/// exit 12, cause `program_error`, Loom's text (the action and the exit) as the reason.
#[test]
fn a_bound_program_refused_by_its_exits_error_list_ends_the_run_refused_naming_the_action_and_its_exit() {
    let rig = Rig::new("program-error");
    writer_binding(&rig, "    exits: {error: [69]}\n");
    rig.writer("#!/bin/sh\nexit 69\n");
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 12, "a tree left without a move by a refused action is a refusal, not exit 3: {}", stderr(&o));
    let outcome = rig.outcome("r1");
    assert_eq!((outcome["outcome"].as_str(), outcome["cause"].as_str(), outcome["exit_code"].as_u64()), (Some("refused"), Some("program_error"), Some(12)), "{outcome}");
    let reason = outcome["reason"].as_str().unwrap();
    assert!(reason.starts_with("ProgramError: action draft.write: ") && reason.contains("exit status 69"), "{outcome}");
    let stopped: Value = serde_json::from_str(&std::fs::read_to_string(rig.run_dir("r1").join("run-stopped.json")).unwrap()).unwrap();
    assert_eq!(stopped["cause"], "program_error", "{stopped}");
    assert!(stopped["reason"].as_str().unwrap().contains("draft.write"), "{stopped}");
}

// ---- the program of a person's action: its environment, and who may take it ----

#[test]
fn the_program_of_a_person_action_gets_path_and_loom_variables_and_nothing_of_the_runbook_process() {
    let rig = Rig::new("person-env");
    review_program(&rig, "#!/bin/sh\nenv > \"$PWD/person-env-dump.txt\"\n");
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let dump = std::fs::read_to_string(rig.work("r1").join("person-env-dump.txt")).unwrap();
    for needle in ["marker-value-8842", "RUNBOOK_MARKER_VARIABLE", "sk-fake-token-5517", "ANTHROPIC_API_KEY", "/marker-home-dir"] {
        assert!(!dump.contains(needle), "{needle} reached the program of a person action:\n{dump}");
    }
    for name in dump.lines().filter_map(|l| l.split_once('=').map(|(k, _)| k)) {
        assert!(name == "PATH" || name.starts_with("LOOM_") || matches!(name, "PWD" | "SHLVL" | "_" | "OLDPWD"), "{name} is in the environment of the program of a person action:\n{dump}");
    }
}

/// A bindings file Loom refuses at `tree open` (a format it does not know) ends the run `refused`,
/// exit 12, with the reason as its cause, where it was exit 1 `error` before the pin; no run of a
/// program, no model.
#[test]
fn a_bindings_file_loom_refuses_at_the_opening_ends_the_run_refused_with_its_reason_as_the_cause() {
    let rig = Rig::new("open-refused");
    let file = rig.root.join("fx/bindings/engine-demo.yaml");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("format: loom.bindings/1"), "the fixture changed: {text}");
    std::fs::write(&file, text.replace("format: loom.bindings/1", "format: loom.bindings/99")).unwrap();
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 12, "{}", stderr(&o));
    let outcome = rig.outcome("r1");
    assert_eq!((outcome["outcome"].as_str(), outcome["cause"].as_str()), (Some("refused"), Some("bindings_refused")), "{outcome}");
    let stopped: Value = serde_json::from_str(&std::fs::read_to_string(rig.run_dir("r1").join("run-stopped.json")).unwrap()).unwrap();
    assert_eq!(stopped["cause"], "bindings_refused");
    assert!(!rig.work("r1").join("draft.txt").exists(), "a program ran after the opening was refused");
}

#[test]
fn an_answer_under_a_model_principal_cannot_take_a_person_action() {
    let rig = Rig::new("person-model");
    review_program(&rig, "#!/bin/sh\nprintf '%s\\n' \"$1\" >> review-note.txt\n");
    rig.settings(&json!({"in_process": true, "confinement": "local", "decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [{"approve": true, "principal": "decider:model", "cost_micro_usd": 0}]}}));
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 12, "a refusal of Loom by a named reason has its own exit code: {}", stderr(&o));
    let outcome = rig.outcome("r1");
    assert_eq!((outcome["outcome"].as_str(), outcome["cause"].as_str()), (Some("refused"), Some("not_a_person_action")), "{outcome}");
    assert!(!rig.work("r1").join("review-note.txt").exists(), "the program of the person action ran for a model");
}

// ---- the allowance of runs after a rejection (`budget.runs_per: rejection`) ----

/// The writer of the fixture bound with `members` more lines under its executor (4 spaces each).
fn writer_binding(rig: &Rig, members: &str) {
    let file = rig.root.join("fx/bindings/engine-draft.yaml");
    let text = std::fs::read_to_string(&file).unwrap();
    let anchor = "      argv: [tools/draft-write.sh]\n";
    assert!(text.contains(anchor), "the writer was not found: {text}");
    std::fs::write(&file, text.replacen(anchor, &format!("{anchor}{members}"), 1)).unwrap();
}

fn reject(reasons: &str) -> Value {
    json!({"approve": false, "reasons": reasons, "cost_micro_usd": 0})
}

/// The `refusal` of every `ActionFinished` line of the run's logs that names `RunsSpent`.
fn runs_spent_refusals(rig: &Rig, name: &str) -> Vec<String> {
    let store = rig.run_dir(name).join("store");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&store).unwrap().flatten() {
        let file = entry.file_name().to_string_lossy().into_owned();
        if !file.starts_with("runlog-case-") {
            continue;
        }
        for line in std::fs::read_to_string(entry.path()).unwrap().lines() {
            let line: Value = serde_json::from_str(line).unwrap();
            if let Some(text) = line["refusal"].as_str().filter(|t| t.starts_with("RunsSpent:")) {
                found.push(text.to_owned());
            }
        }
    }
    found
}

/// The writer's `rework_limit` bounds the rejections it answers: with 2, the writer runs three times
/// (the first run and one after each of the first two rejections) and the third rejection is
/// answered by Loom's `RunsSpent` in its first text, naming the rejection and who made it. The
/// runner ends the run `stuck` (exit 3, cause `stuck`): Loom's refusal is no reason of a tree call.
#[test]
fn a_rejection_past_the_rework_limit_ends_the_run_stuck_with_the_first_runs_spent_text() {
    let rig = Rig::new("rework-limit");
    writer_binding(&rig, "    budget: {max_runs: 3, runs_per: rejection, rework_limit: 2, rework_on: {draft_review: [rejected]}}\n");
    rig.answers(&[reject("first"), reject("second"), reject("third"), approve()], 0);
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 3, "{}", stderr(&o));
    let outcome = rig.outcome("r1");
    assert_eq!(outcome["outcome"], "stuck", "{outcome}");
    let stopped: Value = serde_json::from_str(&std::fs::read_to_string(rig.run_dir("r1").join("run-stopped.json")).unwrap()).unwrap();
    assert_eq!(stopped["cause"], "stuck", "{stopped}");
    assert_eq!(std::fs::read_to_string(rig.work("r1").join("draft.txt")).unwrap(), "draft\ndraft\ndraft\n", "the writer ran three times: once, and once after each of two rejections");
    let answered: Vec<Value> = rig.events("r1").into_iter().filter(|e| e["event"] == "decision_answered").collect();
    assert_eq!(answered.len(), 3, "three rejections were answered and the fourth answer was never asked for: {answered:?}");
    let refusals = runs_spent_refusals(&rig, "r1");
    assert_eq!(refusals.len(), 1, "{refusals:?}");
    let text = &refusals[0];
    assert!(text.contains("draft.write was rejected 3 times in case "), "{text}");
    assert!(text.contains(", the latest ") && text.contains(" by reviewer; its budget.rework_limit is 2"), "{text}");
}

/// The second text: the runs since the latest rejection reach `max_runs` without another rejection.
/// The first check passes the (empty) draft and the review rejects it; the rework then writes, and
/// every check fails what it wrote. A failed check is in `retry_when` and not in `rework_on`, so it
/// sends the writer back and opens no window: two runs fill the window of 2, and the third is
/// refused naming the rejection it counts from. Same end as the first text: `stuck`, exit 3, cause
/// `stuck`.
#[test]
fn a_full_window_after_a_rejection_ends_the_run_stuck_with_the_second_runs_spent_text() {
    let rig = Rig::new("rework-window");
    writer_binding(&rig, "    budget: {max_runs: 2, runs_per: rejection, rework_limit: 5, rework_on: {draft_review: [rejected]}}\n");
    let file = rig.root.join("fx/bindings/engine-draft.yaml");
    let text = std::fs::read_to_string(&file).unwrap();
    let retry_when = "    retry_when:\n      draft_review: [rejected]\n";
    assert!(text.contains(retry_when), "the retry_when was not found: {text}");
    std::fs::write(&file, text.replacen(retry_when, &format!("{retry_when}      draft_check: [invalid]\n"), 1)).unwrap();
    let check = rig.root.join("fx/bindings/tools/draft-check.sh");
    std::fs::write(&check, "#!/bin/sh\nn=$(cat checks.txt 2>/dev/null || echo 0)\necho $((n + 1)) > checks.txt\n[ \"$n\" = 0 ]\n").unwrap();
    rig.answers(&[reject("first"), approve()], 0);
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 3, "{}", stderr(&o));
    let outcome = rig.outcome("r1");
    assert_eq!(outcome["outcome"], "stuck", "{outcome}");
    let stopped: Value = serde_json::from_str(&std::fs::read_to_string(rig.run_dir("r1").join("run-stopped.json")).unwrap()).unwrap();
    assert_eq!(stopped["cause"], "stuck", "{stopped}");
    assert_eq!(std::fs::read_to_string(rig.work("r1").join("draft.txt")).unwrap(), "draft\ndraft\n", "the two runs of the window after the rejection");
    let refusals = runs_spent_refusals(&rig, "r1");
    assert_eq!(refusals.len(), 1, "{refusals:?}");
    let text = &refusals[0];
    assert!(text.contains("draft.write has run 2 times in case "), "{text}");
    assert!(text.contains(" since the rejection ") && text.contains(" by reviewer, its budget.max_runs is 2"), "{text}");
}

/// What Loom refuses at the opening, by name: each of these ends the run `refused` (exit 12, cause
/// `bindings_refused`) with the refusal's text, and no program ran.
#[test]
fn the_rework_members_loom_refuses_end_the_run_refused_by_name_before_any_program_runs() {
    let original = "      argv: [tools/draft-write.sh]\n    retry_when:\n      draft_review: [rejected]\n";
    let head = "      argv: [tools/draft-write.sh]\n";
    let retry_when = "    retry_when:\n      draft_review: [rejected]\n";
    for (what, members, listed, want) in [
        ("runs_per rejection with a retry_when that lists no result", "    budget: {runs_per: rejection}\n", false, "budget.runs_per"),
        ("rework_on a pair retry_when does not list", "    budget: {runs_per: rejection, rework_on: {draft_check: [invalid]}}\n", true, "budget.rework_on"),
        ("rework_limit beside runs_per case", "    budget: {runs_per: case, rework_limit: 2}\n", true, "budget.rework_limit"),
        ("an empty rework_on", "    budget: {runs_per: rejection, rework_on: {}}\n", true, "budget.rework_on"),
        ("a runs_per that is neither case nor rejection", "    budget: {runs_per: window}\n", true, "budget.runs_per"),
        ("a rework_limit of 0", "    budget: {runs_per: rejection, rework_limit: 0}\n", true, "budget.rework_limit"),
    ] {
        let rig = Rig::new("rework-refused");
        let file = rig.root.join("fx/bindings/engine-draft.yaml");
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains(original), "the writer was not found: {text}");
        let tail = if listed { retry_when } else { "" };
        std::fs::write(&file, text.replacen(original, &format!("{head}{members}{tail}"), 1)).unwrap();
        let o = rig.run(&rig.args("r1", 1_000_000));
        assert_eq!(code(&o), 12, "{what}: {}", stderr(&o));
        let outcome = rig.outcome("r1");
        assert_eq!((outcome["outcome"].as_str(), outcome["cause"].as_str()), (Some("refused"), Some("bindings_refused")), "{what}: {outcome}");
        let reason = outcome["reason"].as_str().unwrap_or_default();
        assert!(reason.contains("InvalidBudget") && reason.contains(want), "{what}: {outcome}");
        assert!(!rig.work("r1").join("draft.txt").exists(), "{what}: a program ran after the opening was refused");
    }
}

// ---- the step in flight: the event of an action that starts and the record of the runner ----

/// Polls `ready` until it holds; the limit is only the bound of the test, never a pause the test relies on.
fn until(what: &str, mut ready: impl FnMut() -> bool) {
    let limit = Instant::now() + Duration::from_secs(50);
    while !ready() {
        assert!(Instant::now() < limit, "{what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Field 22 of `/proc/<pid>/stat`: the start time of the process, in clock ticks since boot.
fn start_ticks(pid: u32) -> u64 {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    let after = &stat[stat.rfind(')').unwrap() + 2..];
    after.split_whitespace().nth(19).unwrap().parse().unwrap()
}

fn runner_record(rig: &Rig, name: &str) -> Option<Value> {
    let text = std::fs::read_to_string(rig.run_dir(name).join("runner.json")).ok()?;
    Some(serde_json::from_str(&text).unwrap())
}

/// A writer that ends when the case writes `go.txt` in its work directory.
const WAITING_WRITER: &str = "#!/bin/sh\nwhile [ ! -f go.txt ]; do sleep 0.05; done\necho draft >> draft.txt\n";

fn started_events(rig: &Rig, name: &str) -> Vec<Value> {
    let Ok(text) = std::fs::read_to_string(rig.run_dir(name).join("events.jsonl")) else { return Vec::new() };
    text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter(|e| e["event"] == "action_started").collect()
}

#[test]
fn an_action_that_starts_is_in_the_events_file_before_its_program_ends_and_the_runner_is_on_record() {
    let rig = Rig::new("in-flight");
    rig.writer(WAITING_WRITER);
    let mut child = rig.command(&rig.args("r1", 1_000_000)).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let pid = child.id();
    until("no action_started event for draft.write while its program waits", || started_events(&rig, "r1").iter().any(|e| e["action"] == "draft.write"));
    // the program is still waiting: the event is not a record written after the step
    assert!(!rig.work("r1").join("draft.txt").exists());
    let events = rig.events("r1");
    let at = events.iter().position(|e| e["event"] == "action_started" && e["action"] == "draft.write").unwrap();
    assert!(events[at + 1..].iter().all(|e| e["event"] != "step_made"), "a step was made after the start of draft.write, with its program still waiting");
    let e = &events[at];
    assert_eq!((e["case"].as_str().map(|c| !c.is_empty()), e["try"].as_u64(), e["source"].as_str(), e["role"].as_str()), (Some(true), Some(1), Some("program_start"), Some("program")), "{e}");
    assert!(e["at"].as_u64().is_some_and(|t| t > 1_700_000_000), "{e}");
    // the record of the runner: its process and the start time the kernel gave it, nothing else
    let record = runner_record(&rig, "r1").expect("runner.json while the run is live");
    assert_eq!(record["format"], "runbook.runner/1", "{record}");
    assert_eq!(record["pid"].as_u64(), Some(u64::from(pid)), "{record}");
    assert_eq!(record["start_ticks"].as_u64(), Some(start_ticks(pid)), "{record}");
    assert!(record["started_at"].as_u64().is_some() && record["closed_at"].is_null(), "{record}");
    let mut keys: Vec<&str> = record.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["closed_at", "format", "pid", "start_ticks", "started_at"], "{record}");
    // no environment, no argv, no path in either
    let dir = rig.run_dir("r1");
    let files = format!("{}\n{}", std::fs::read_to_string(dir.join("runner.json")).unwrap(), serde_json::to_string(e).unwrap());
    for needle in ["marker-value-8842", "RUNBOOK_MARKER_VARIABLE", "sk-fake-token-5517", "/marker-home-dir", "draft-write.sh", "/home", "/tmp", "/usr", "/var", "/root"] {
        assert!(!files.contains(needle), "{needle} is in the records:\n{files}");
    }
    assert!(!files.contains(&*dir.to_string_lossy()), "the path of the run directory is in the records:\n{files}");
    std::fs::write(rig.work("r1").join("go.txt"), "").unwrap();
    let status = child.wait().unwrap();
    assert!(status.success(), "{status:?}");
    let events = rig.events("r1");
    let started = events.iter().position(|e| e["event"] == "action_started" && e["action"] == "draft.write").unwrap();
    let made = events.iter().rposition(|e| e["event"] == "step_made").unwrap();
    assert!(started < made, "the start of draft.write is after the last step");
    let closed = runner_record(&rig, "r1").expect("runner.json closed, not removed");
    assert!(closed["closed_at"].as_u64().is_some_and(|t| t >= record["started_at"].as_u64().unwrap()), "{closed}");
    assert_eq!(closed["pid"], record["pid"], "{closed}");
}

#[test]
fn a_try_after_a_rejection_is_counted_and_a_check_program_names_its_action() {
    let rig = Rig::new("tries");
    rig.answers(&[json!({"approve": false, "reasons": "again", "cost_micro_usd": 0}), approve()], 0);
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let started = started_events(&rig, "r1");
    let writes: Vec<u64> = started.iter().filter(|e| e["action"] == "draft.write").filter_map(|e| e["try"].as_u64()).collect();
    assert_eq!(writes, [1, 2], "{started:?}");
    assert!(started.iter().any(|e| e["action"] == "draft.check"), "{started:?}");
    assert!(started.iter().all(|e| e["role"] == "program" && e["source"] == "program_start"), "{started:?}");
}

#[test]
fn a_runner_ended_by_sigterm_mid_step_closes_its_record() {
    let rig = Rig::new("in-flight-term");
    rig.writer("#!/bin/sh\nwhile [ ! -f go.txt ]; do sleep 0.05; done\n");
    let mut child = rig.command(&rig.args("r1", 1_000_000)).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    until("no action_started event while the program waits", || started_events(&rig, "r1").iter().any(|e| e["action"] == "draft.write"));
    assert!(runner_record(&rig, "r1").is_some_and(|r| r["closed_at"].is_null()));
    assert!(Command::new("kill").args(["-TERM", &child.id().to_string()]).status().unwrap().success());
    let status = child.wait().unwrap();
    assert_eq!(status.code(), Some(7), "{status:?}");
    let closed = runner_record(&rig, "r1").expect("runner.json after the signal");
    assert!(closed["closed_at"].as_u64().is_some(), "{closed}");
}

#[test]
fn a_run_that_fails_closes_the_record_of_the_runner() {
    let rig = Rig::new("in-flight-failed");
    // the ledger path is a directory: the run directory exists, the run fails when it opens the ledger
    std::fs::create_dir_all(rig.ledger()).unwrap();
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    let closed = runner_record(&rig, "r1").expect("runner.json after a failed run");
    assert!(closed["started_at"].as_u64().is_some() && closed["closed_at"].as_u64().is_some(), "{closed}");
}

// ---- what a program that can write the run directory (`--confinement local`) does to the records of the runner ----

/// The time the runner has to end after a program planted its file: what is left of a run then is one more program start and
/// the end, which take seconds even under load. A runner that waits on a file the program planted is still there.
const AFTER_PLANT_SECS: u64 = 12;

/// Waits for the child to end: up to 45 s for the program to plant (`planted.txt` in the work directory of `r1`), then
/// `AFTER_PLANT_SECS` more. A child still running at the limit is killed (it is the child of this test) and `None` is answered.
fn ended_after_plant(rig: &Rig, child: &mut std::process::Child) -> Option<std::process::ExitStatus> {
    ended_after_plant_within(rig, child, AFTER_PLANT_SECS)
}

/// [`ended_after_plant`] with `secs` after the plant instead of `AFTER_PLANT_SECS`.
fn ended_after_plant_within(rig: &Rig, child: &mut std::process::Child, secs: u64) -> Option<std::process::ExitStatus> {
    let planted = rig.work("r1").join("planted.txt");
    let mut limit = Instant::now() + Duration::from_secs(45);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return Some(status);
        }
        if planted.exists() && limit > Instant::now() + Duration::from_secs(secs) {
            limit = Instant::now() + Duration::from_secs(secs);
        }
        if Instant::now() >= limit {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Waits up to `secs` for the child to end; a child still running at the limit is killed (it is the child of this test) and `None` is answered.
fn ended_within(child: &mut std::process::Child, secs: u64) -> Option<std::process::ExitStatus> {
    let limit = Instant::now() + Duration::from_secs(secs);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return Some(status);
        }
        if Instant::now() >= limit {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A writer that makes its draft, runs `plant` (sh) from the work directory, whose parent is the run directory, and then
/// writes `planted.txt` in the work directory.
fn planting_writer(rig: &Rig, plant: &str) {
    rig.writer(&format!("#!/bin/sh\necho draft >> draft.txt\n{plant}\ntouch planted.txt\n"));
}

/// A file outside the run directory that must keep its text.
fn outside(rig: &Rig) -> PathBuf {
    let file = rig.root.join("outside.txt");
    std::fs::write(&file, "keep\n").unwrap();
    file
}

#[test]
fn an_events_file_a_program_replaced_by_a_named_pipe_does_not_hold_the_next_program_start() {
    let rig = Rig::new("events-fifo");
    planting_writer(&rig, "rm -f ../events.jsonl\nmkfifo ../events.jsonl");
    let mut child = rig.command(&rig.args("r1", 1_000_000)).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let status = ended_after_plant(&rig, &mut child);
    assert!(status.is_some_and(|s| s.success()), "the run did not end within {AFTER_PLANT_SECS} s of the plant: the start event of the next program (draft.check) opens events.jsonl by its path and waits there for a reader of the pipe that a program put in its place; the event is a record for a reader and must not hold the step");
}

#[test]
fn a_named_pipe_a_program_left_at_the_temporary_name_of_the_record_does_not_hold_the_end_of_the_run() {
    let rig = Rig::new("tmp-fifo");
    planting_writer(&rig, "mkfifo ../runner.tmp");
    let mut child = rig.command(&rig.args("r1", 1_000_000)).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let status = ended_after_plant(&rig, &mut child);
    assert!(status.is_some_and(|s| s.success()), "the run did not end within {AFTER_PLANT_SECS} s of the plant: closing runner.json writes runner.tmp by its path and waits there for a reader of the pipe that a program left");
}

#[test]
fn an_events_file_a_program_replaced_by_a_link_is_not_appended_through() {
    let rig = Rig::new("events-link");
    let outside = outside(&rig);
    planting_writer(&rig, &format!("rm -f ../events.jsonl\nln -s '{}' ../events.jsonl", outside.display()));
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep\n", "the start event of the next program was appended to a file outside the run directory, through a link a program put at events.jsonl");
}

#[test]
fn a_link_a_program_left_at_the_temporary_name_of_the_record_is_not_written_through() {
    let rig = Rig::new("tmp-link");
    let outside = outside(&rig);
    planting_writer(&rig, &format!("ln -s '{}' ../runner.tmp", outside.display()));
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep\n", "closing runner.json wrote the record into a file outside the run directory, through a link a program put at runner.tmp");
}

#[test]
fn an_events_file_a_program_replaced_by_a_second_name_of_a_file_outside_is_not_appended_to() {
    let rig = Rig::new("events-hardlink");
    let outside = outside(&rig);
    planting_writer(&rig, &format!("rm -f ../events.jsonl\nln '{}' ../events.jsonl", outside.display()));
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep\n", "the start event of the next program was appended to a file with another name outside the run directory");
}

#[test]
fn a_named_pipe_a_program_left_at_the_temporary_name_of_the_outcome_does_not_hold_the_end_of_the_run() {
    let rig = Rig::new("outcome-fifo");
    planting_writer(&rig, "mkfifo ../outcome.tmp");
    let mut child = rig.command(&rig.args("r1", 1_000_000)).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let status = ended_after_plant(&rig, &mut child);
    assert!(status.is_some_and(|s| s.success()), "the run did not end within {AFTER_PLANT_SECS} s of the plant: writing outcome.json waits on a pipe at its temporary name");
    assert!(rig.outcome("r1")["outcome"].is_string(), "outcome.json is not the runner's record");
}

#[test]
fn a_link_a_program_left_at_the_temporary_name_of_the_outcome_is_not_written_through() {
    let rig = Rig::new("outcome-link");
    let outside = outside(&rig);
    planting_writer(&rig, &format!("ln -s '{}' ../outcome.tmp", outside.display()));
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep\n", "writing outcome.json wrote the record into a file outside the run directory, through a link at its temporary name");
    assert!(std::fs::symlink_metadata(rig.run_dir("r1").join("outcome.json")).unwrap().is_file(), "outcome.json is not a regular file");
}

#[test]
fn the_tree_records_are_written_past_a_pipe_and_a_link_left_at_their_temporary_names() {
    let rig = Rig::new("tree-tmp");
    let outside = outside(&rig);
    planting_writer(&rig, &format!("mkfifo ../tree-status.tmp\nln -s '{}' ../tree-verify.tmp", outside.display()));
    let mut child = rig.command(&rig.args("r1", 1_000_000)).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let status = ended_after_plant(&rig, &mut child);
    assert!(status.is_some_and(|s| s.success()), "the run did not end within {AFTER_PLANT_SECS} s of the plant: a tree record waits on a pipe at its temporary name");
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep\n", "tree-verify.json was written through a link at its temporary name");
    for name in ["tree-status.json", "tree-verify.json"] {
        assert!(std::fs::symlink_metadata(rig.run_dir("r1").join(name)).unwrap().is_file(), "{name} is not a regular file");
    }
}

#[test]
fn guard_a_runner_json_a_program_replaced_by_a_link_or_a_lie_is_replaced_by_the_runners_own_close() {
    let rig = Rig::new("record-link");
    let outside = outside(&rig);
    planting_writer(&rig, &format!("rm -f ../runner.json\nln -s '{}' ../runner.json", outside.display()));
    let mut child = rig.command(&rig.args("r1", 1_000_000)).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let pid = child.id();
    assert!(ended_within(&mut child, 40).is_some_and(|s| s.success()));
    let path = rig.run_dir("r1").join("runner.json");
    assert!(std::fs::symlink_metadata(&path).unwrap().is_file(), "runner.json is still a link after the close");
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep\n", "the close wrote through the link");
    let record = runner_record(&rig, "r1").unwrap();
    assert_eq!((record["pid"].as_u64(), record["closed_at"].as_u64().is_some()), (Some(u64::from(pid)), true), "{record}");
    // a lie in place of the record: replaced by the close as well
    let rig = Rig::new("record-lie");
    planting_writer(&rig, "echo '{\"format\":\"runbook.runner/1\",\"pid\":1,\"start_ticks\":1,\"started_at\":1,\"closed_at\":null}' > ../runner.json");
    let mut child = rig.command(&rig.args("r1", 1_000_000)).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let pid = child.id();
    assert!(ended_within(&mut child, 40).is_some_and(|s| s.success()));
    let record = runner_record(&rig, "r1").unwrap();
    assert_eq!((record["pid"].as_u64(), record["closed_at"].as_u64().is_some()), (Some(u64::from(pid)), true), "{record}");
}

#[test]
fn guard_a_second_runner_on_the_same_run_directory_is_refused_and_leaves_the_record_of_the_first() {
    let rig = Rig::new("second");
    let first = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&first), 0, "{}", stderr(&first));
    let before = std::fs::read_to_string(rig.run_dir("r1").join("runner.json")).unwrap();
    let events_before = std::fs::read_to_string(rig.run_dir("r1").join("events.jsonl")).unwrap();
    let second = rig.run(&rig.args("r1", 1_000_000));
    assert_ne!(code(&second), 0, "{}", stderr(&second));
    assert_eq!(std::fs::read_to_string(rig.run_dir("r1").join("runner.json")).unwrap(), before, "the second runner rewrote the record of the first");
    assert_eq!(std::fs::read_to_string(rig.run_dir("r1").join("events.jsonl")).unwrap(), events_before, "the second runner wrote into the events of the first");
}

#[test]
fn guard_a_runner_whose_file_name_holds_a_closing_parenthesis_records_its_own_start_time() {
    let rig = Rig::new("odd-name");
    rig.writer(WAITING_WRITER);
    // the name of the file is the name of the process in field 2 of its stat: spaces, a bracket and a closing parenthesis
    let odd = rig.root.join("a) 1 (b]");
    if std::fs::hard_link(BIN, &odd).is_err() {
        std::fs::copy(BIN, &odd).unwrap();
    }
    let mut command = Command::new(&odd);
    command.args(rig.args("r1", 1_000_000)).env_clear().env("PATH", std::env::var("PATH").unwrap_or_default()).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    let mut child = command.spawn().unwrap();
    let pid = child.id();
    let limit = Instant::now() + Duration::from_secs(40);
    let record = loop {
        if let Some(record) = runner_record(&rig, "r1") {
            break record;
        }
        if let Some(status) = child.try_wait().unwrap() {
            panic!("the runner ended ({status:?}) before it wrote runner.json");
        }
        assert!(Instant::now() < limit, "no runner.json in 40 s");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap().contains(')'), "the process name has no closing parenthesis: the case does not test the parse");
    assert_eq!(record["start_ticks"].as_u64(), Some(start_ticks(pid)), "{record}");
    until("no action_started event while the program waits", || !started_events(&rig, "r1").is_empty());
    std::fs::write(rig.work("r1").join("go.txt"), "").unwrap();
    assert!(ended_within(&mut child, 40).is_some_and(|s| s.success()));
}

// ---- the names the writer of the records tries, and an events file a program made unusable ----

/// sh that plants, for the four records the runner writes through `write_atomic`, an entry at every temporary name the
/// writer can try for its own process id (the parent of the program: `$PPID`) and the counters `from` to `to`: a directory,
/// a pipe, a link to `outside` and a second name of `outside`. The pid is written to `ppid.txt` in the work directory.
fn plant_names(outside: &Path, from: u32, to: u32) -> String {
    let o = outside.display();
    format!(
        "P=$PPID\necho $P > ppid.txt\nn={from}\nwhile [ $n -le {to} ]; do\n for leaf in runner.json outcome.json tree-status.json tree-verify.json; do\n  t=\"../.$leaf.$P.$n.tmp\"\n  case $((n % 4)) in\n   0) mkdir \"$t\";;\n   1) mkfifo \"$t\";;\n   2) ln -s '{o}' \"$t\";;\n   3) ln '{o}' \"$t\";;\n  esac\n done\n n=$((n+1))\ndone\n"
    )
}

fn spawn_to_files(rig: &Rig, tag: &str) -> (std::process::Child, PathBuf, PathBuf) {
    let out = rig.root.join(format!("{tag}-stdout.txt"));
    let err = rig.root.join(format!("{tag}-stderr.txt"));
    let child = rig
        .command(&rig.args("r1", 1_000_000))
        .stdout(Stdio::from(std::fs::File::create(&out).unwrap()))
        .stderr(Stdio::from(std::fs::File::create(&err).unwrap()))
        .spawn()
        .unwrap();
    (child, out, err)
}

#[test]
fn a_run_whose_next_temporary_names_a_program_took_still_writes_its_records() {
    let rig = Rig::new("temp-names-taken");
    let outside = outside(&rig);
    planting_writer(&rig, &plant_names(&outside, 0, 12));
    let (mut child, _out, err) = spawn_to_files(&rig, "taken");
    let status = ended_after_plant(&rig, &mut child);
    let err = std::fs::read_to_string(err).unwrap_or_default();
    assert!(status.is_some_and(|s| s.success()), "the run did not end well within {AFTER_PLANT_SECS} s of the plant: {status:?} {err}");
    let record = runner_record(&rig, "r1").unwrap();
    let ppid: u64 = std::fs::read_to_string(rig.work("r1").join("ppid.txt")).unwrap().trim().parse().unwrap();
    assert_eq!(record["pid"].as_u64(), Some(ppid), "the program's parent is not the runner: the case plants the wrong names ({record})");
    assert!(record["closed_at"].as_u64().is_some(), "the close of runner.json did not get past the names a program took: {record}");
    assert!(rig.outcome("r1")["outcome"].is_string(), "outcome.json is not the runner's record");
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep\n");
    for name in ["runner.json", "outcome.json", "tree-status.json", "tree-verify.json"] {
        let meta = std::fs::symlink_metadata(rig.run_dir("r1").join(name));
        assert!(meta.is_ok_and(|m| m.is_file()), "{name} is not a regular file");
    }
}

#[test]
fn a_run_whose_every_temporary_name_a_program_took_ends_with_an_error_that_names_nothing_outside_the_run() {
    let rig = Rig::new("temp-names-all-taken");
    let outside = outside(&rig);
    planting_writer(&rig, &plant_names(&outside, 0, 400));
    let (mut child, out, err) = spawn_to_files(&rig, "all-taken");
    let status = ended_after_plant(&rig, &mut child);
    let text = format!("{}{}", std::fs::read_to_string(&out).unwrap_or_default(), std::fs::read_to_string(&err).unwrap_or_default());
    assert!(status.is_some(), "the run did not end within {AFTER_PLANT_SECS} s of the plant: the writer of a record does not stop at its bound ({text})");
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep\n", "a record was written into a file outside the run");
    assert!(!text.contains("outside.txt"), "a message names the file outside the run: {text}");
    assert!(status.is_some_and(|s| !s.success()), "the run ended without an error although no record could be written ({status:?}): {text}");
    let ppid: u64 = std::fs::read_to_string(rig.work("r1").join("ppid.txt")).unwrap().trim().parse().unwrap();
    assert!(text.contains(&format!(".{ppid}.")), "the error does not name the temporary file in the run directory: {text}");
}

/// The time the runner has to end after a program planted a name for a record: the writer of a record tries at most 16
/// names and none of them waits, so a run that has not ended after this long waits on something a program left.
const RECORD_AFTER_PLANT_SECS: u64 = 6;

#[test]
fn a_named_pipe_a_program_left_at_the_temporary_name_of_status_json_does_not_hold_the_end_of_the_run() {
    let rig = Rig::new("status-fifo");
    planting_writer(&rig, "mkfifo ../status.json.tmp");
    let (mut child, _out, _err) = spawn_to_files(&rig, "status-fifo");
    let status = ended_after_plant_within(&rig, &mut child, RECORD_AFTER_PLANT_SECS);
    assert!(status.is_some_and(|s| s.success()), "the run did not end within {RECORD_AFTER_PLANT_SECS} s of the plant: the writer of status.json waits on a pipe that a program left at its temporary name ({status:?})");
}

#[test]
fn a_link_a_program_left_at_the_temporary_name_of_status_json_is_not_written_through() {
    let rig = Rig::new("status-link");
    let outside = outside(&rig);
    planting_writer(&rig, &format!("ln -s '{}' ../status.json.tmp", outside.display()));
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep\n", "status.json was written into a file outside the run directory, through a link at its temporary name");
}

#[test]
fn guard_a_run_leaves_no_temporary_file_in_its_directory() {
    let rig = Rig::new("no-temp-file-left");
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let left: Vec<String> = std::fs::read_dir(rig.run_dir("r1")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".tmp")).collect();
    assert!(left.is_empty(), "temporary files left: {left:?}");
}

fn an_events_file_a_program_made_unusable_does_not_stop_the_run(tag: &str, plant: &str) {
    let rig = Rig::new(&format!("events-unusable-{tag}"));
    planting_writer(&rig, plant);
    let (mut child, _out, err) = spawn_to_files(&rig, tag);
    let status = ended_after_plant(&rig, &mut child);
    let err = std::fs::read_to_string(err).unwrap_or_default();
    assert!(status.is_some_and(|s| s.success()), "{tag}: the run did not end well within {AFTER_PLANT_SECS} s of the plant: {status:?} {err}");
    assert!(rig.outcome("r1")["outcome"].is_string(), "{tag}: outcome.json is not the runner's record");
}

#[test]
fn guard_an_events_file_that_is_a_directory_does_not_stop_the_run() {
    an_events_file_a_program_made_unusable_does_not_stop_the_run("dir", "rm -f ../events.jsonl\nmkdir ../events.jsonl");
}

#[test]
fn guard_an_events_file_with_no_permission_does_not_stop_the_run() {
    an_events_file_a_program_made_unusable_does_not_stop_the_run("mode0", "chmod 000 ../events.jsonl");
}

#[test]
fn guard_an_events_file_that_is_read_only_does_not_stop_the_run() {
    an_events_file_a_program_made_unusable_does_not_stop_the_run("mode444", "chmod 444 ../events.jsonl");
}

/// A record the runner appends to (`rounds.jsonl` after the first step, `decisions.jsonl` after the first answer) that a
/// program of the run replaced by `plant` (sh, `OUTSIDE` is a file outside the run): the run ends within
/// `RECORD_AFTER_PLANT_SECS` of the plant with an error that names the file in the run directory, and nothing is written
/// outside the run.
fn an_appended_record_a_program_replaced_ends_the_run_with_an_error(tag: &str, leaf: &str, plant: &str) {
    let rig = Rig::new(&format!("appended-{tag}"));
    let outside = outside(&rig);
    planting_writer(&rig, &plant.replace("OUTSIDE", &outside.display().to_string()));
    let (mut child, out, err) = spawn_to_files(&rig, tag);
    let status = ended_after_plant_within(&rig, &mut child, RECORD_AFTER_PLANT_SECS);
    let text = format!("{}{}", std::fs::read_to_string(&out).unwrap_or_default(), std::fs::read_to_string(&err).unwrap_or_default());
    assert!(status.is_some(), "{tag}: the run did not end within {RECORD_AFTER_PLANT_SECS} s of the plant: the writer of {leaf} waits on what a program put there ({text})");
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep\n", "{tag}: {leaf} was appended to a file outside the run directory");
    assert!(status.is_some_and(|s| !s.success()), "{tag}: the run ended without an error although {leaf} was not a regular file with one name ({status:?}): {text}");
    assert!(text.contains(leaf), "{tag}: the error does not name {leaf} in the run directory: {text}");
    assert!(!text.contains("outside.txt"), "{tag}: a message names the file outside the run: {text}");
}

#[test]
fn a_link_a_program_put_at_rounds_jsonl_is_not_appended_through() {
    an_appended_record_a_program_replaced_ends_the_run_with_an_error("rounds-link", "rounds.jsonl", "rm -f ../rounds.jsonl\nln -s 'OUTSIDE' ../rounds.jsonl");
}

#[test]
fn a_named_pipe_a_program_put_at_rounds_jsonl_does_not_hold_the_end_of_the_run() {
    an_appended_record_a_program_replaced_ends_the_run_with_an_error("rounds-pipe", "rounds.jsonl", "rm -f ../rounds.jsonl\nmkfifo ../rounds.jsonl");
}

#[test]
fn a_second_name_of_a_file_outside_the_run_put_at_rounds_jsonl_is_not_appended_to() {
    an_appended_record_a_program_replaced_ends_the_run_with_an_error("rounds-name", "rounds.jsonl", "rm -f ../rounds.jsonl\nln 'OUTSIDE' ../rounds.jsonl");
}

#[test]
fn a_link_a_program_put_at_decisions_jsonl_is_not_appended_through() {
    an_appended_record_a_program_replaced_ends_the_run_with_an_error("decisions-link", "decisions.jsonl", "rm -f ../decisions.jsonl\nln -s 'OUTSIDE' ../decisions.jsonl");
}

#[test]
fn the_reasons_of_a_rejection_replace_a_link_a_program_left_at_their_file() {
    let rig = Rig::new("reasons-link");
    rig.answers(&[json!({"approve": false, "reasons": "the draft needs a title line", "cost_micro_usd": 0}), approve()], 0);
    let outside = outside(&rig);
    planting_writer(&rig, &format!("mkdir -p .sdc/reasons\nln -s '{}' .sdc/reasons/001-draft.review.txt", outside.display()));
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep\n", "the reasons of a rejection were written into a file outside the run directory, through a link");
    let reasons = rig.work("r1").join(".sdc/reasons/001-draft.review.txt");
    assert!(std::fs::symlink_metadata(&reasons).unwrap().is_file(), "{} is not a regular file", reasons.display());
    assert!(std::fs::read_to_string(&reasons).unwrap().contains("the draft needs a title line"));
}

// ---- the directories of the run are made without following a link, and what the runner reads is read as a regular file ----

/// An empty directory outside the run that must stay empty.
fn empty_directory_outside(rig: &Rig) -> PathBuf {
    let dir = rig.root.join("outdir");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

/// Runs the writer `plant` and checks that the run ends within `RECORD_AFTER_PLANT_SECS` and that nothing was made in the
/// empty directory outside the run.
fn directory_link_leaves_outside_empty(tag: &str, plant: &str) {
    let rig = Rig::new(tag);
    let out = empty_directory_outside(&rig);
    planting_writer(&rig, &plant.replace("OUTDIR", &out.display().to_string()));
    let (mut child, o, e) = spawn_to_files(&rig, tag);
    let status = ended_after_plant_within(&rig, &mut child, RECORD_AFTER_PLANT_SECS);
    let text = format!("{}{}", std::fs::read_to_string(&o).unwrap_or_default(), std::fs::read_to_string(&e).unwrap_or_default());
    assert!(status.is_some(), "{tag}: the run did not end within {RECORD_AFTER_PLANT_SECS} s of the plant ({text})");
    assert_eq!(names_in(&out), Vec::<String>::new(), "{tag}: the runner made these files in a directory outside the run, through a link a program put at a directory of the run ({status:?}: {text})");
}

#[test]
fn a_link_a_program_put_at_the_loom_directory_is_not_written_through() {
    directory_link_leaves_outside_empty("loom-link", "rm -rf ../loom\nln -s 'OUTDIR' ../loom");
}

#[test]
fn a_link_a_program_put_at_the_store_directory_of_the_root_case_is_not_written_through() {
    directory_link_leaves_outside_empty("store-root-link", "rm -rf ../store/ROOT-1\nln -s 'OUTDIR' ../store/ROOT-1");
}

#[test]
fn a_link_a_program_put_at_the_directory_of_the_reasons_is_not_written_through() {
    let rig = Rig::new("reasons-dir-link");
    rig.answers(&[json!({"approve": false, "reasons": "the draft needs a title line", "cost_micro_usd": 0}), approve()], 0);
    let out = empty_directory_outside(&rig);
    planting_writer(&rig, &format!("[ -L .sdc ] || ln -s '{}' .sdc", out.display()));
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(names_in(&out), Vec::<String>::new(), "the reasons of a rejection were written into a directory outside the run, through a link a program put at .sdc ({}: {})", code(&o), stderr(&o));
}

#[test]
fn a_named_pipe_a_program_put_at_findings_yaml_does_not_hold_the_end_of_the_run() {
    let rig = Rig::new("findings-fifo");
    planting_writer(&rig, "mkfifo findings.yaml");
    let (mut child, o, e) = spawn_to_files(&rig, "findings");
    let status = ended_after_plant_within(&rig, &mut child, RECORD_AFTER_PLANT_SECS);
    let text = format!("{}{}", std::fs::read_to_string(&o).unwrap_or_default(), std::fs::read_to_string(&e).unwrap_or_default());
    assert!(status.is_some(), "the run did not end within {RECORD_AFTER_PLANT_SECS} s of the plant: the copy of work/findings.yaml into the store reads a pipe that a program put there and waits for a writer ({text})");
}

#[test]
fn a_link_a_program_put_at_findings_yaml_is_not_copied_into_the_store() {
    let rig = Rig::new("findings-link");
    let outside = rig.root.join("outside-findings.txt");
    std::fs::write(&outside, "secret-outside-text\n").unwrap();
    planting_writer(&rig, &format!("ln -s '{}' findings.yaml", outside.display()));
    let o = rig.run(&rig.args("r1", 1_000_000));
    let copy = rig.run_dir("r1").join("store/ROOT-1/work/findings.yaml");
    let text = std::fs::read_to_string(&copy).unwrap_or_default();
    assert!(!text.contains("secret-outside-text"), "the text of a file outside the run was copied into the store of the run through a link at work/findings.yaml ({}: {})", code(&o), stderr(&o));
}

#[test]
fn runbook_run_writes_one_action_started_line_per_program_in_order_with_tries_before_the_steps_they_start() {
    let rig = Rig::new("start-turns");
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let events = rig.events("r1");
    let started: Vec<(usize, &Value)> = events.iter().enumerate().filter(|(_, e)| e["event"] == "action_started" && e["source"] == "program_start").collect();
    let shape: Vec<(&str, u64, &str)> = started.iter().map(|(_, e)| (e["action"].as_str().unwrap(), e["try"].as_u64().unwrap(), e["role"].as_str().unwrap())).collect();
    assert_eq!(shape, [("draft.check", 1, "program"), ("draft.write", 1, "program"), ("draft.check", 2, "program")], "{shape:?}");
    // each start is before the step its program made, and the starts are in the order of the steps
    let made: Vec<usize> = events.iter().enumerate().filter(|(_, e)| e["event"] == "step_made").map(|(i, _)| i).collect();
    for (n, (at, e)) in started.iter().enumerate() {
        assert!(made.iter().any(|m| m > at), "no step after the start {e}");
        let before = made.iter().filter(|m| *m < at).count();
        assert!(before <= n + 1, "{e} comes after more steps than programs started: {before} steps before start {n}");
    }
    let record: Value = serde_json::from_str(&std::fs::read_to_string(rig.run_dir("r1").join("runner.json")).unwrap()).unwrap();
    assert!(record["closed_at"].as_u64().is_some(), "{record}");
}

/// The text of a refused action leaves the run in `outcome.json`, `events.jsonl` and `run-stopped.json`: it names
/// the action and the exit, and no absolute path of the machine (the run dir, the runbook home, the fixture root).
#[test]
fn a_refused_action_reason_names_no_absolute_path() {
    let rig = Rig::new("fix1-path");
    writer_binding(&rig, "    exits: {error: [69]}\n");
    rig.writer("#!/bin/sh\nexit 69\n");
    let o = rig.run(&rig.args("r1", 1_000_000));
    assert_eq!(code(&o), 12, "{}", stderr(&o));
    let root = rig.root.display().to_string();
    for file in ["outcome.json", "events.jsonl", "run-stopped.json"] {
        let text = std::fs::read_to_string(rig.run_dir("r1").join(file)).unwrap();
        assert!(!text.contains(&root), "{file} names the path {root}: {text}");
    }
    let reason = rig.outcome("r1")["reason"].as_str().unwrap().to_owned();
    assert!(!reason.contains("/home") && !reason.contains("/tmp") && !reason.contains(env!("CARGO_MANIFEST_DIR")), "{reason}");
}

/// The same with `--runs-dir` given as a relative word and the working directory beside it: the text Loom wrote
/// names the bindings copy by the path the runner gave it; no absolute path of the machine reaches the outcome.
#[test]
fn a_refused_action_reason_names_no_absolute_path_under_a_relative_runs_dir() {
    let rig = Rig::new("fix1-relpath");
    writer_binding(&rig, "    exits: {error: [69]}\n");
    rig.writer("#!/bin/sh\nexit 69\n");
    let mut args = rig.args("r1", 1_000_000);
    let i = args.iter().position(|a| a == "--runs-dir").unwrap();
    args[i + 1] = "runs".to_owned();
    let o = rig.command(&args).current_dir(&rig.root).output().unwrap();
    assert_eq!(code(&o), 12, "{}", stderr(&o));
    let root = rig.root.display().to_string();
    for file in ["outcome.json", "events.jsonl", "run-stopped.json"] {
        let text = std::fs::read_to_string(rig.run_dir("r1").join(file)).unwrap();
        assert!(!text.contains(&root), "{file} names the path {root}: {text}");
    }
}

/// A writer that is run again after a rejection and is refused on that second try ends the run `refused` too.
#[test]
fn a_retry_refused_on_its_second_try_ends_the_run_refused_program_error() {
    let rig = Rig::new("fix1-retry");
    writer_binding(&rig, "    exits: {error: [69]}\n");
    rig.writer("#!/bin/sh\n[ -z \"$LOOM_RETRY_FEEDBACK\" ] || exit 69\necho draft >> draft.txt\n");
    rig.answers(&[reject("again"), json!({"approve": true, "cost_micro_usd": 0}), json!({"approve": true, "cost_micro_usd": 0})], 0);
    let o = rig.run(&rig.args("r1", 1_000_000));
    let outcome = rig.outcome("r1");
    assert_eq!((outcome["outcome"].as_str(), outcome["cause"].as_str(), code(&o)), (Some("refused"), Some("program_error"), 12), "{outcome} {}", stderr(&o));
    assert!(outcome["reason"].as_str().unwrap().starts_with("ProgramError: action draft.write: "), "{outcome}");
}

/// A bound program that cannot be started (its file is gone) is a refusal too: the run must not end `stuck` with a
/// null reason, whatever Loom calls the refusal.
#[test]
fn a_bound_program_that_cannot_start_does_not_end_the_run_stuck_with_a_null_reason() {
    let rig = Rig::new("fix1-nospawn");
    std::fs::remove_file(rig.root.join("fx/bindings/tools/draft-write.sh")).unwrap();
    let o = rig.run(&rig.args("r1", 1_000_000));
    let outcome = rig.outcome("r1");
    assert!(!(outcome["outcome"] == "stuck" && outcome["reason"].is_null()), "exit {}: {outcome} {}", code(&o), stderr(&o));
}

/// A program that Loom refuses for an exit its binding has no result for (`ResultNotInTable`, what a program killed
/// at its timeout, exit 124, reaches) leaves the tree without a move with Loom's refusal text in the case's run log.
/// The run must not end `stuck` with `reason: null` and the text only in the log.
#[test]
fn a_refusal_that_is_not_a_program_error_is_not_dropped_with_a_null_reason() {
    let rig = Rig::new("fix1-table");
    let file = rig.root.join("fx/bindings/engine-draft.yaml");
    let text = std::fs::read_to_string(&file).unwrap();
    let anchor = "results: {\"0\": valid, \"*\": invalid}";
    assert!(text.contains(anchor), "{text}");
    std::fs::write(&file, text.replace(anchor, "results: {\"0\": valid}")).unwrap();
    std::fs::write(rig.root.join("fx/bindings/tools/draft-check.sh"), "#!/bin/sh\nexit 124\n").unwrap();
    let o = rig.run(&rig.args("r1", 1_000_000));
    let outcome = rig.outcome("r1");
    assert!(outcome["reason"].is_string(), "outcome {} exit {}: the refusal text is in the case log only: {outcome}", outcome["outcome"], code(&o));
    let reason = outcome["reason"].as_str().unwrap();
    assert!(reason.starts_with("ResultNotInTable: action ") && !reason.contains('/'), "the reason is the refusal's lead, no path: {reason}");
    assert_eq!((outcome["outcome"].as_str(), code(&o)), (Some("stuck"), 3), "{outcome}");
    let stopped: Value = serde_json::from_str(&std::fs::read_to_string(rig.run_dir("r1").join("run-stopped.json")).unwrap()).unwrap();
    assert!(stopped["reason"].as_str().is_some_and(|r| r.ends_with(reason)), "run-stopped.json says the same: {stopped}");
}
