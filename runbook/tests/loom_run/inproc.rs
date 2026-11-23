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
