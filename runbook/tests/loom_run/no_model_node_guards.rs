//! Security review pass 1 of the unit `run-without-model-node-needs-no-caps` (untracked): the
//! cases that hold the unit to its own contract (`docs/loom-run.md`, "A run without a model
//! node"). The `real_loom_*` cases run the loop over Loom as a library with POSIX sh programs
//! (the draft case of `tests/fixtures/engine-runbook` as the root, so no `open` executor); the
//! others run it against the fake `b10x-loom`. The "model program" is a stand-in named `sdc-node`
//! that writes a marker file with invented text: no model call, no network request, no real login
//! file read or copied.

use super::core::{code, model_call_hook, stderr, World, BIN};
use super::fake::tree_run_json;
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

// ---- the fake world: caps of 0, no ledger ----

const NO_MODEL: &str = "format: loom.bindings/1
protocol: sdc@8
actions:
  - action: check.run
    executor:
      kind: program
      argv: [tools/sdc-tool.sh, check-idea]
  - action: scope.approve
    executor: {kind: person, principal: person:owner}
";

fn set(a: &mut [String], flag: &str, value: &str) {
    let i = a.iter().position(|x| x == flag).unwrap();
    a[i + 1] = value.to_owned();
}

fn without(a: &mut Vec<String>, flag: &str) {
    let i = a.iter().position(|x| x == flag).unwrap();
    a.drain(i..=i + 1);
}

/// The options of run `r1` with every cap at 0 and no ledger, under `substrate`: the one level where
/// such a run starts (the fake `b10x-loom` takes the level as an argument and does not enforce it).
fn zero(w: &World) -> Vec<String> {
    let mut a = w.args("r1", 0);
    set(&mut a, "--max-llm-calls", "0");
    without(&mut a, "--spend-ledger");
    set(&mut a, "--confinement", "substrate");
    a
}

fn file(w: &World, rel: &str, text: &str) {
    let path = w.root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn one_action(executor: &str) -> String {
    format!("format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: a.b\n    executor: {executor}\n")
}

/// Every form here names the model tool, or is a form the rule says it cannot read, in a place the
/// implementor's cases do not put it. Each must count as a model node: the run that names no
/// ledger is refused before anything is written.
#[test]
fn forms_of_the_copy_that_must_count_as_a_model_node() {
    type Setup = Box<dyn Fn(&World)>;
    let forms: Vec<(&str, Setup)> = vec![
        ("a second YAML document that names the tool", Box::new(|w: &World| file(w, "bindings/sdc.yaml", &format!("format: loom.bindings/1\nprotocol: sdc@8\n---\n{}", one_action("{kind: program, argv: [sdc-node, a.b]}"))))),
        ("a file with the extension yml", Box::new(|w: &World| {
            file(w, "bindings/sdc.yaml", NO_MODEL);
            file(w, "bindings/extra.yml", &one_action("{kind: program, argv: [sdc-node, a.b]}"));
        })),
        ("a file two directories below the copy", Box::new(|w: &World| {
            file(w, "bindings/sdc.yaml", NO_MODEL);
            file(w, "bindings/sub/deep/extra.yaml", &one_action("{kind: program, argv: [sdc-node, a.b]}"));
        })),
        ("a yaml file that cannot be read", Box::new(|w: &World| {
            file(w, "bindings/sdc.yaml", NO_MODEL);
            file(w, "bindings/locked.yaml", &one_action("{kind: program, argv: [sdc-node, a.b]}"));
            std::fs::set_permissions(w.root.join("bindings/locked.yaml"), std::fs::Permissions::from_mode(0o000)).unwrap();
        })),
        ("a link to a directory", Box::new(|w: &World| {
            file(w, "bindings/sdc.yaml", NO_MODEL);
            file(w, "elsewhere/extra.yaml", &one_action("{kind: program, argv: [sdc-node, a.b]}"));
            std::os::unix::fs::symlink(w.root.join("elsewhere"), w.root.join("bindings/linked")).unwrap();
        })),
        ("a link to nothing", Box::new(|w: &World| {
            file(w, "bindings/sdc.yaml", NO_MODEL);
            std::os::unix::fs::symlink(w.root.join("absent-target"), w.root.join("bindings/gone")).unwrap();
        })),
        ("a yaml file that is a link to a file outside the copy", Box::new(|w: &World| {
            file(w, "bindings/sdc.yaml", NO_MODEL);
            file(w, "elsewhere/plain.txt", &one_action("{kind: program, argv: [sdc-node, a.b]}"));
            std::os::unix::fs::symlink(w.root.join("elsewhere/plain.txt"), w.root.join("bindings/extra.yaml")).unwrap();
        })),
        ("the tool as the key of a mapping", Box::new(|w: &World| file(w, "bindings/sdc.yaml", &format!("{NO_MODEL}notes:\n  sdc-node: runs here\n")))),
        ("the tool as a tagged text", Box::new(|w: &World| file(w, "bindings/sdc.yaml", &one_action("{kind: program, argv: [!bin sdc-node, a.b]}")))),
        ("the tool written with an escape", Box::new(|w: &World| file(w, "bindings/sdc.yaml", &one_action("{kind: program, argv: [\"sdc\\x2dnode\", a.b]}")))),
        ("the tool in handoff_when", Box::new(|w: &World| file(w, "bindings/sdc.yaml", &format!("{}    handoff_when: [sdc-node, check]\n", one_action("{kind: program, argv: [tools/sdc-tool.sh, x]}"))))),
        ("the tool in arguments_from", Box::new(|w: &World| file(w, "bindings/sdc.yaml", &format!("{}    arguments_from: [sdc-node, args]\n", one_action("{kind: program, argv: [tools/sdc-tool.sh, x]}"))))),
        ("the tool as the program of a person action", Box::new(|w: &World| file(w, "bindings/sdc.yaml", &one_action("{kind: person, principal: person:owner, argv: [sdc-node, a.b]}")))),
        ("an executor whose kind comes through a merge key", Box::new(|w: &World| file(w, "bindings/sdc.yaml", &one_action("{<<: {kind: program, argv: [tools/sdc-tool.sh, x]}}")))),
        ("an executor that opens a child case", Box::new(|w: &World| file(w, "bindings/sdc.yaml", &one_action("{kind: open, name: intake, bindings: sdc-intake.yaml}")))),
        ("an argv that is a block text", Box::new(|w: &World| file(w, "bindings/sdc.yaml", "format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: a.b\n    executor:\n      kind: program\n      argv: |\n        tools/sdc-tool.sh x\n"))),
        ("JSON text in the bindings file", Box::new(|w: &World| file(w, "bindings/sdc.yaml", "{\"format\":\"loom.bindings/1\",\"protocol\":\"sdc@8\",\"actions\":[{\"action\":\"a.b\",\"executor\":{\"kind\":\"program\",\"argv\":[\"sdc-node\",\"a.b\"]}}]}\n"))),
    ];
    let mut wrong: Vec<String> = Vec::new();
    for (what, setup) in &forms {
        let w = World::new("review1-forms");
        setup(&w);
        let o = w.run(&zero(&w));
        if code(&o) != 64 || !stderr(&o).contains("--spend-ledger is required") {
            wrong.push(format!("{what}: exit {} ({})", code(&o), stderr(&o).lines().last().unwrap_or("")));
        }
        if w.run_dir("r1").exists() {
            wrong.push(format!("{what}: a run directory was made"));
        }
        // a locked file would stop the removal of the world by the next case of this tag
        let _ = std::fs::set_permissions(w.root.join("bindings/locked.yaml"), std::fs::Permissions::from_mode(0o600));
    }
    assert!(wrong.is_empty(), "forms that did not count as a model node:\n{}", wrong.join("\n"));
}

/// A ledger of other runs is not opened, written or replaced by a run without a model node: here
/// it is a file the ledger reader would refuse, and the run ends as if it were not named.
#[test]
fn a_ledger_of_other_runs_is_left_as_it_is_by_a_run_without_a_model_node() {
    let w = World::new("review1-shared-ledger");
    file(&w, "bindings/sdc.yaml", NO_MODEL);
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let before = "the ledger of other runs, not this run's to read\n";
    std::fs::write(w.ledger(), before).unwrap();
    std::fs::set_permissions(w.ledger(), std::fs::Permissions::from_mode(0o400)).unwrap();
    let mut a = w.args("r1", 0);
    set(&mut a, "--max-llm-calls", "0");
    set(&mut a, "--confinement", "substrate");
    let o = w.run(&a);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(std::fs::read_to_string(w.ledger()).unwrap(), before);
    let names: Vec<String> = std::fs::read_dir(&w.root).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.starts_with("ledger")).collect();
    assert_eq!(names, ["ledger.json"], "a lock or a temporary file was left beside the ledger");
}

/// A model call that a step recorded in a run without a model node: the run ends by the named
/// cause, and its records state the call and the cost they saw, never 0, with `has_model_node`
/// false beside them, so a reader can tell it from a run with a ledger.
#[test]
fn a_recorded_call_in_a_run_without_a_model_node_is_stated_in_the_records_with_its_cost() {
    let w = World::new("review1-recorded");
    file(&w, "bindings/sdc.yaml", NO_MODEL);
    w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "default", &model_call_hook("0.1"));
    let o = w.run(&zero(&w));
    assert_eq!(code(&o), 13, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!((outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64(), outcome["has_model_node"].as_bool()), (Some(1), Some(100_000), Some(false)), "{outcome}");
    assert_eq!(outcome["cause"], "model_call_without_model_node", "{outcome}");
    let events = std::fs::read_to_string(w.run_dir("r1").join("events.jsonl")).unwrap();
    assert!(events.contains("\"cap\":\"model_node\""), "{events}");
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 1, "no step after the one that recorded the call");
}

/// Cost records a step left that cannot be read: a model call cannot be ruled out, so the run
/// without a model node ends by the named cause and takes no further step.
#[test]
fn cost_records_that_cannot_be_read_end_a_run_without_a_model_node() {
    let w = World::new("review1-unreadable-records");
    file(&w, "bindings/sdc.yaml", NO_MODEL);
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.fake.hook("tree_run", "1", "mkdir -p work/.sdc/llm; printf 'not a record {{{\\n' >> work/.sdc/llm/calls.jsonl\n");
    let o = w.run(&zero(&w));
    assert_eq!(code(&o), 13, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["cause"], "model_call_without_model_node", "{outcome}");
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 1, "no step after the one that left the records");
}

// ---- Loom as a library: the draft case of the engine fixture as the root ----

struct Lib {
    root: PathBuf,
    /// The Loom binary of a run through the child process; `None`: Loom as a library.
    loom: Option<PathBuf>,
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

fn write_exe(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

const LOGIN_TEXT: &str = "STAND-IN-LOGIN-TEXT-7731";

impl Lib {
    fn new(tag: &str) -> Lib {
        Lib::with_loom(tag, None)
    }

    fn with_loom(tag: &str, loom: Option<PathBuf>) -> Lib {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("review1-lib-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for d in ["runs", "xdg", "bin", "login"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        std::fs::set_permissions(root.join("xdg"), std::fs::Permissions::from_mode(0o700)).unwrap();
        copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &root.join("fx"));
        // the root of the fixture opens a child case, and an `open` executor counts as a model node
        std::fs::remove_file(root.join("fx/bindings/engine-demo.yaml")).unwrap();
        std::fs::write(root.join("idea.yaml"), "text: a note on tea\n").unwrap();
        std::fs::write(root.join("login/credentials.json"), LOGIN_TEXT).unwrap();
        super::decider::install_routing_table_at(&root.join("fx"));
        let lib = Lib { root, loom };
        lib.settings(&json!({}));
        lib
    }

    /// A scripted decider that approves and reserves nothing, so it is no model node.
    fn settings(&self, extra: &Value) {
        let approve = json!({"approve": true, "cost_micro_usd": 0});
        let loom = if self.loom.is_some() { json!({"confinement": "local"}) } else { json!({"in_process": true, "confinement": "local"}) };
        let mut v = json!({"loom": loom, "decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [approve, approve, approve, approve]}});
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        std::fs::write(self.root.join("settings.json"), serde_json::to_string_pretty(&v).unwrap()).unwrap();
    }

    fn marker(&self) -> PathBuf {
        self.root.join("marker-model-program")
    }

    /// The stand-in for the model tool, first on the PATH of the run: it writes the marker and,
    /// with `records`, the cost record the model tool writes for a call (0.25 USD).
    fn stand_in(&self, records: bool) -> String {
        let xdg = self.root.join("xdg").display().to_string();
        let mut text = format!(
            "#!/bin/sh\necho started >> '{}'\nenv > env-model.txt\ncat '{xdg}'/runbook/\"$(basename \"$(dirname \"$PWD\")\")\"/login > reach-login.txt 2>/dev/null\n",
            self.marker().display()
        );
        if records {
            text.push_str("w=.sdc/llm; mkdir -p \"$w/001-draft.check\"\nprintf '{\"action\":\"draft.check\",\"record\":\"%s/%s/001-draft.check\",\"cost_usd\":0.25}\\n' \"$PWD\" \"$w\" >> \"$w/calls.jsonl\"\n");
        }
        text.push_str("[ -s draft.txt ]\n");
        write_exe(&self.root.join("bin/sdc-node"), &text);
        text
    }

    fn tool(&self, name: &str, script: &str) {
        write_exe(&self.root.join("fx/bindings/tools").join(name), script);
    }

    fn bindings(&self, change: impl Fn(&str) -> String) {
        let path = self.root.join("fx/bindings/engine-draft.yaml");
        let text = std::fs::read_to_string(&path).unwrap();
        let changed = change(&text);
        assert_ne!(changed, text, "the bindings were not changed");
        std::fs::write(path, changed).unwrap();
    }

    fn run_dir(&self) -> PathBuf {
        self.root.join("runs/r1")
    }

    fn work(&self) -> PathBuf {
        self.run_dir().join("work")
    }

    /// Run `r1`, the library way, under `local` with caps: a ledger and both caps above 0. A run without
    /// a model node starts with caps of 0 only under `substrate`, which this host cannot run.
    fn run(&self) -> Output {
        let r = |p: &str| self.root.join(p).display().to_string();
        let mut args = vec![
            "run".to_owned(), "sdc".into(), "--run".into(), "r1".into(), "--runs-dir".into(), r("runs"), "--bindings".into(), r("fx/bindings"), "--protocols".into(), r("fx/protocol"),
            "--root-protocol".into(), "engine-draft@1".into(), "--case".into(), "DRAFT-1".into(), "--intake".into(), r("idea.yaml"), "--confinement".into(), "local".into(),
            "--settings".into(), r("settings.json"), "--spend-ledger".into(), r("ledger.json"), "--cap-micro-usd".into(), "5000000".into(), "--max-llm-calls".into(), "100".into(), "--max-wall-seconds".into(), "50".into(),
        ];
        if let Some(loom) = &self.loom {
            let digest = Command::new("sha256sum").arg(loom).output().unwrap();
            let digest = String::from_utf8_lossy(&digest.stdout).split_whitespace().next().unwrap().to_owned();
            args.extend(["--loom".into(), loom.display().to_string(), "--loom-sha256".into(), digest]);
        }
        Command::new(BIN)
            .args(args)
            .env_clear()
            .env("PATH", format!("{}:{}", r("bin"), std::env::var("PATH").unwrap_or_default()))
            .env("HOME", "/marker-home-dir")
            .env("XDG_RUNTIME_DIR", self.root.join("xdg"))
            .env("ANTHROPIC_API_KEY", "sk-fake-token-5517")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    fn outcome(&self) -> Value {
        let text = std::fs::read_to_string(self.run_dir().join("outcome.json")).unwrap_or_else(|e| panic!("no outcome.json: {e}"));
        serde_json::from_str(&text).unwrap()
    }

    /// Exit code, whether the model program started, and the outcome's label, calls and cost.
    fn seen(&self, o: &Output) -> String {
        let outcome = if self.run_dir().join("outcome.json").is_file() { self.outcome() } else { Value::Null };
        format!(
            "exit {}, model program started: {}, outcome {}, has_model_node {}, llm_calls {}, spent_micro_usd {}, cause {}; stderr: {}",
            code(o),
            self.marker().exists(),
            outcome["outcome"],
            outcome["has_model_node"],
            outcome["llm_calls"],
            outcome["spent_micro_usd"],
            outcome["cause"],
            stderr(o).lines().last().unwrap_or("")
        )
    }
}

const NAME_THE_TOOL: &str = "s#\\[tools/draft-check.sh\\]#[sdc-node, draft.check]#";

/// The real library Loom, under `local`, with caps: a run whose bindings name no model node starts,
/// counts nothing and states `has_model_node` false; the ledger is opened and holds 0.
#[test]
fn real_loom_under_local_a_run_without_a_model_node_with_caps_counts_nothing() {
    let lib = Lib::new("plain");
    lib.stand_in(true);
    let o = lib.run();
    assert_eq!(code(&o), 0, "{}", lib.seen(&o));
    let outcome = lib.outcome();
    assert_eq!((outcome["outcome"].as_str(), outcome["has_model_node"].as_bool(), outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some("ended"), Some(false), Some(0), Some(0)), "{outcome}");
    assert!(!lib.marker().exists());
    assert_eq!(std::fs::read_to_string(lib.work().join("draft.txt")).unwrap(), "draft\n");
    let ledger: Value = serde_json::from_str(&std::fs::read_to_string(lib.root.join("ledger.json")).unwrap()).unwrap();
    assert_eq!(ledger["spent_micro_usd"].as_u64(), Some(0), "{ledger}");
}

/// The real library Loom, under `local`, with caps: the writer of the first step rewrites the run's
/// copy of the bindings so that the next step is the model tool. Loom reads that file at every `tree
/// run` and the program starts; the caps count its recorded call (a run opened without caps would end
/// before the program starts: `no_model_node.rs`, with the fake, under `substrate`).
#[test]
fn real_loom_under_local_with_caps_a_copy_rewritten_to_name_the_model_tool_starts_it_and_the_call_is_booked() {
    let lib = Lib::new("rewrite-copy");
    lib.stand_in(true);
    lib.tool("draft-write.sh", &format!("#!/bin/sh\nsed -i '{NAME_THE_TOOL}' ../bindings/engine-draft.yaml\necho draft >> draft.txt\n"));
    let o = lib.run();
    let copy = std::fs::read_to_string(lib.run_dir().join("bindings/engine-draft.yaml")).unwrap();
    assert!(copy.contains("sdc-node"), "the writer did not rewrite the copy: {copy}");
    assert!(lib.marker().exists(), "the model program did not start: {}", lib.seen(&o));
    let outcome = lib.outcome();
    assert_ne!(outcome["cause"].as_str(), Some("model_call_without_model_node"), "{outcome}");
    assert_eq!((outcome["has_model_node"].as_bool(), outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some(false), Some(1), Some(250_000)), "{outcome}");
    let ledger: Value = serde_json::from_str(&std::fs::read_to_string(lib.root.join("ledger.json")).unwrap()).unwrap();
    assert_eq!(ledger["spent_micro_usd"].as_u64(), Some(250_000), "{ledger}");
}

/// The documented limit (`docs/loom-run.md`, "The limit of the rule"), under `local` with caps. The
/// forms the rule does not read, present at the opening: the model tool reached by a name that is in
/// no YAML text. The runner cannot see a name that a program of the runbook's author builds when it
/// runs, so the program starts. This case asserts what holds then, for each form: it is started; no
/// login of the runner reaches it (not in its environment, not through a file of the run's private
/// directory); a call it records is booked in the ledger and counted, and does not end the run; a
/// call it does not record is counted by no cap. (With caps of 0, under `substrate`, a recorded call
/// ends the run: `no_model_node_limit.rs`.)
#[test]
fn documented_limit_under_local_with_caps_a_model_program_reached_by_a_name_the_rule_does_not_read_starts_without_a_login_and_a_recorded_call_is_booked() {
    type Setup = Box<dyn Fn(&Lib) + Sync>;
    let forms: Vec<(&str, bool, Setup)> = vec![
        ("a script of tools/ that builds the name from two parts", true, Box::new(|lib: &Lib| lib.tool("draft-check.sh", "#!/bin/sh\np=sdc-\n\"${p}node\" draft.check\n"))),
        ("a shell word of the argv that builds the name", true, Box::new(|lib: &Lib| lib.bindings(|t| t.replace("argv: [tools/draft-check.sh]", "argv: [sh, -c, 'n=node; \"sdc-$n\" draft.check']")))),
        ("a copy of the program in tools/ under another name", true, Box::new(|lib: &Lib| {
            let text = std::fs::read_to_string(lib.root.join("bin/sdc-node")).unwrap();
            lib.tool("draft-check.sh", &text);
        })),
        ("the same, and the program writes no cost record", false, Box::new(|lib: &Lib| {
            let text = std::fs::read_to_string(lib.root.join("bin/sdc-node")).unwrap();
            lib.tool("draft-check.sh", &text);
        })),
    ];
    // the forms run side by side: each has a run directory of its own
    let results: Vec<Vec<String>> = std::thread::scope(|s| {
        let handles: Vec<_> = forms.iter().enumerate().map(|(n, (what, records, setup))| s.spawn(move || unread_form(n, what, *records, setup))).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let wrong: Vec<String> = results.into_iter().flatten().collect();
    assert!(wrong.is_empty(), "the documented limit does not hold as documented:\n{}", wrong.join("\n"));
}

/// One form of the limit: what is wrong with it, nothing when it holds as documented.
fn unread_form(n: usize, what: &str, records: bool, setup: &(dyn Fn(&Lib) + Sync)) -> Vec<String> {
    let mut wrong: Vec<String> = Vec::new();
    let lib = Lib::new(&format!("unread-{n}"));
    lib.stand_in(records);
    lib.settings(&json!({"login_file": lib.root.join("login/credentials.json").display().to_string()}));
    setup(&lib);
    let o = lib.run();
    let seen = lib.seen(&o);
    if !lib.marker().exists() {
        wrong.push(format!("{what}: the program did not start (the limit is gone: say so in the document): {seen}"));
        return wrong;
    }
    let outcome = lib.outcome();
    // every start of the program that records is booked once: calls and cost follow the starts
    let starts = std::fs::read_to_string(lib.marker()).unwrap_or_default().lines().count() as u64;
    let want = if records { (0, None, Some(starts), Some(250_000 * starts)) } else { (0, None, Some(0), Some(0)) };
    let got = (code(&o), outcome["cause"].as_str(), outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64());
    let booked = std::fs::read_to_string(lib.root.join("ledger.json")).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v["spent_micro_usd"].as_u64());
    if got != want || outcome["has_model_node"] != false || booked != want.3 {
        wrong.push(format!("{what}: {seen}"));
    }
    let env = std::fs::read_to_string(lib.work().join("env-model.txt")).unwrap_or_default();
    for needle in ["sk-fake-token-5517", "ANTHROPIC_API_KEY", "/marker-home-dir", "XDG_RUNTIME_DIR", LOGIN_TEXT, "credentials.json"] {
        if env.contains(needle) {
            wrong.push(format!("{what}: {needle} is in the environment of the model program"));
        }
    }
    if std::fs::read_to_string(lib.work().join("reach-login.txt")).unwrap_or_default().contains(LOGIN_TEXT) {
        wrong.push(format!("{what}: the model program read the login file through the private directory"));
    }
    assert!(!env.is_empty(), "{what}: the stand-in did not dump its environment");
    wrong
}

/// What a bound program of a run without a model node can reach of the login, under `local` with
/// caps: the settings name a login file (a stand-in with invented text), as one settings file shared
/// by every runbook does. No model node of this run reads it, so nothing of the run holds a way to it.
#[test]
fn real_loom_under_local_with_caps_a_run_without_a_model_node_gives_its_bound_programs_no_way_to_the_login_file() {
    let lib = Lib::new("login");
    lib.stand_in(true);
    lib.settings(&json!({"login_file": lib.root.join("login/credentials.json").display().to_string()}));
    // the private directory of a run is $XDG_RUNTIME_DIR/runbook/<run>/ and the run's name is the
    // name of the directory above the work directory; a program is not told XDG_RUNTIME_DIR, the
    // test's stands for the well-known one of the user
    let xdg = lib.root.join("xdg").display().to_string();
    lib.tool("draft-write.sh", &format!("#!/bin/sh\nrun=$(basename \"$(dirname \"$PWD\")\")\nenv > env-dump.txt\ncat '{xdg}'/runbook/\"$run\"/login > reach-login.txt 2>/dev/null\necho draft >> draft.txt\n"));
    let o = lib.run();
    assert_eq!(code(&o), 0, "{}", lib.seen(&o));
    assert_eq!(lib.outcome()["has_model_node"], false);
    let env = std::fs::read_to_string(lib.work().join("env-dump.txt")).unwrap();
    for needle in ["sk-fake-token-5517", "ANTHROPIC_API_KEY", "/marker-home-dir", "XDG_RUNTIME_DIR", LOGIN_TEXT, "credentials.json"] {
        assert!(!env.contains(needle), "{needle} is in the environment of the bound program:\n{env}");
    }
    let reached = std::fs::read_to_string(lib.work().join("reach-login.txt")).unwrap_or_default();
    assert!(!reached.contains(LOGIN_TEXT), "a bound program of a run without a model node read the login file through the link of the run's private directory");
    assert!(!lib.root.join("xdg/runbook").join("r1").exists(), "the private directory is left");
}

// ---- the same refusals against the fake: forms the library cannot make cheaply ----

/// Every form of the manifest the runner must not accept: each ends the run by the named cause after
/// the first `tree run` and before a second. The hook runs in the run directory; `$store` is the
/// store directory. The first answer is `BudgetSpent` (the run goes on), the second ends it, so a
/// check that is removed makes this case fail by its assertions and not by waiting.
#[test]
fn a_manifest_that_does_not_name_the_root_file_of_the_copy_ends_a_run_without_a_model_node() {
    let forms: Vec<(&str, &str)> = vec![
        ("removed", "rm \"$store/tree.json\""),
        ("replaced by a link", "mv \"$store/tree.json\" \"$store/real.json\" && ln -s real.json \"$store/tree.json\""),
        ("not JSON", "echo '{{ not json' > \"$store/tree.json\""),
        ("a JSON text that is not an object", "echo '[\"bindings\"]' > \"$store/tree.json\""),
        ("without a bindings member", "echo '{\"root\":\"r\"}' > \"$store/tree.json\""),
        ("a bindings member that is not a text", "echo '{\"bindings\":3}' > \"$store/tree.json\""),
        ("a bindings member that is not absolute", "echo '{\"bindings\":\"bindings/sdc.yaml\"}' > \"$store/tree.json\""),
        ("another file of the copy", "cp bindings/sdc.yaml bindings/other.yaml && printf '{\"bindings\":\"%s/bindings/other.yaml\"}' \"$PWD\" > \"$store/tree.json\""),
        ("a file that does not exist", "printf '{\"bindings\":\"%s/bindings/none.yaml\"}' \"$PWD\" > \"$store/tree.json\""),
        ("a link to the root file", "ln -s \"$PWD/bindings/sdc.yaml\" \"$PWD/same.yaml\" && printf '{\"bindings\":\"%s/same.yaml\"}' \"$PWD\" > \"$store/tree.json\""),
        ("a directory instead of the file", "rm \"$store/tree.json\" && mkdir \"$store/tree.json\""),
        ("two bindings members, the last names other bindings", "printf '{\"bindings\":\"%s/bindings/sdc.yaml\",\"bindings\":\"%s/bindings/none.yaml\"}' \"$PWD\" \"$PWD\" > \"$store/tree.json\""),
        ("a bindings member that is a list holding the path", "printf '{\"bindings\":[\"%s/bindings/sdc.yaml\"]}' \"$PWD\" > \"$store/tree.json\""),
        ("a hard link to the root file", "ln bindings/sdc.yaml bindings/hard.yaml && printf '{\"bindings\":\"%s/bindings/hard.yaml\"}' \"$PWD\" > \"$store/tree.json\""),
    ];
    let mut wrong: Vec<String> = Vec::new();
    for (what, hook) in &forms {
        let w = World::new("manifest-forms");
        file(&w, "bindings/sdc.yaml", NO_MODEL);
        w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
        w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
        w.fake.hook("tree_run", "1", &format!("{hook}\n"));
        let o = w.run(&zero(&w));
        let outcome = w.outcome("r1");
        let reason = outcome["reason"].as_str().unwrap_or("");
        if code(&o) != 13 || outcome["cause"] != "model_call_without_model_node" || !reason.contains("tree manifest") || w.fake.calls_of(&["tree", "run"]).len() != 1 || w.ledger().exists() {
            wrong.push(format!("{what}: exit {}, {outcome}, tree runs {}", code(&o), w.fake.calls_of(&["tree", "run"]).len()));
        }
    }
    assert!(wrong.is_empty(), "forms of the manifest that did not end the run:\n{}", wrong.join("\n"));
}

/// A manifest that is as the opening wrote it, or names the root file by a spelling that resolves
/// to it, does not stop a run; with a model node the manifest is not looked at.
#[test]
fn a_manifest_that_names_the_root_file_of_the_copy_does_not_stop_a_run_without_a_model_node() {
    for (what, hook) in [
        ("as written", ":"),
        ("with dots in the path", "printf '{\"bindings\":\"%s/bindings/../bindings/./sdc.yaml\"}' \"$PWD\" > \"$store/tree.json\""),
        ("two bindings members, the first names other bindings and the last the root file", "printf '{\"bindings\":\"%s/bindings/none.yaml\",\"bindings\":\"%s/bindings/sdc.yaml\"}' \"$PWD\" \"$PWD\" > \"$store/tree.json\""),
        ("a path through a directory link that resolves to the root file", "ln -s bindings lnk && printf '{\"bindings\":\"%s/lnk/sdc.yaml\"}' \"$PWD\" > \"$store/tree.json\""),
    ] {
        let w = World::new("manifest-ok");
        file(&w, "bindings/sdc.yaml", NO_MODEL);
        w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
        w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
        w.fake.hook("tree_run", "1", &format!("{hook}\n"));
        let o = w.run(&zero(&w));
        assert_eq!(code(&o), 0, "{what}: {}", stderr(&o));
        assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 2, "{what}");
    }
    // a run with a model node: the manifest is not the runner's to judge, the caps hold
    let w = World::new("manifest-with-node");
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.fake.hook("tree_run", "1", "rm \"$store/tree.json\"\n");
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(w.outcome("r1")["has_model_node"], true);
}

// ---- the login link: a run without a model node makes none ----

/// The private directory of a run holds the one link to the login file. A run without a model node
/// makes neither the directory nor the link (a hook looks while the run is on and the test looks
/// after it); a run with a model node makes both, as before.
#[test]
fn the_link_to_the_login_file_is_made_in_a_run_with_a_model_node_and_in_no_other() {
    let seen_link = |w: &World| format!("[ -L \"{}/runbook/r1/login\" ] && touch \"$D/had-link\"\n[ -e \"{}/runbook\" ] && ls -A \"{}/runbook\" > \"$D/runtime-ls\"\n:\n", w.root.join("xdg").display(), w.root.join("xdg").display(), w.root.join("xdg").display());
    let w = World::new("login-without");
    file(&w, "bindings/sdc.yaml", NO_MODEL);
    w.fake.answer("tree_run", "1", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.fake.hook("tree_run", "1", &seen_link(&w));
    let o = w.run(&zero(&w));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(!w.fake.dir.join("had-link").exists(), "a run without a model node made the link to the login file");
    assert!(!w.fake.dir.join("runtime-ls").exists() || std::fs::read_to_string(w.fake.dir.join("runtime-ls")).unwrap().trim().is_empty(), "a run without a model node made a private directory");
    assert!(!w.root.join("xdg/runbook/r1").exists());
    let w = World::new("login-with");
    w.fake.answer("tree_run", "1", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.fake.hook("tree_run", "1", &seen_link(&w));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(w.fake.dir.join("had-link").exists(), "a run with a model node made no link to the login file");
    assert!(!w.root.join("xdg/runbook/r1").exists(), "the private directory is left");
}

/// A login file that is named and cannot be used refuses a run without a model node at the start,
/// as before: nothing is written. The link is not made, the checks are.
#[test]
fn a_login_file_that_cannot_be_used_refuses_a_run_without_a_model_node_before_anything_is_written() {
    let w = World::new("login-unusable");
    file(&w, "bindings/sdc.yaml", NO_MODEL);
    w.settings(&json!({"login_file": w.root.join("no-such-login").display().to_string()}));
    let o = w.run(&zero(&w));
    assert_ne!(code(&o), 0, "{}", stderr(&o));
    assert!(stderr(&o).contains("login_file"), "{}", stderr(&o));
    assert!(!w.run_dir("r1").exists(), "a run directory was made");
    assert!(w.fake.calls().is_empty(), "Loom was called");
}

// ---- the sentences of the document ----

/// With a model node, a call whose cost records a step leaves (the model tool, started by a
/// wrapper or not: the runner reads the records, not the process) is booked in the ledger and
/// counted. The world's bindings name the model tool.
#[test]
fn with_a_model_node_a_call_recorded_whatever_started_it_is_booked_and_counted() {
    let w = World::new("wrapper-booked");
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.fake.hook("tree_run", "1", &model_call_hook("0.4"));
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(super::core::spent(&w), 400_000);
    let outcome = w.outcome("r1");
    assert_eq!((outcome["has_model_node"].as_bool(), outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some(true), Some(1), Some(400_000)), "{outcome}");
}

// ---- the slow lane: the child `b10x-loom` of the pinned revision ----

fn pinned_loom_binary() -> PathBuf {
    PathBuf::from(std::env::var("RUNBOOK_LOOM_BIN").expect("RUNBOOK_LOOM_BIN names the b10x-loom binary of the pinned Loom revision"))
}

/// A handoff's printed command is `tree run` with no bound on its steps, and Loom reads the bindings
/// again before every step of one call. The runner bounds it to one step in a run without a model node
/// (`child_host.rs`, `one_step_argv`), so it reads its copy again between two steps. With the real
/// `b10x-loom` of the pinned revision, behind a wrapper that logs every call, under `local` with caps:
/// the run takes a handoff, and every `tree run` call of the run, the handoff's included, carries
/// `--max-steps 1`.
#[test]
#[ignore = "slow lane: set RUNBOOK_LOOM_BIN to the b10x-loom of the pinned Loom revision"]
fn child_loom_the_printed_command_of_a_handoff_takes_one_step_in_a_run_without_a_model_node() {
    let real = pinned_loom_binary();
    let wrapper = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("review1-lib-handoff-wrapper-{}", std::process::id()));
    let lib = Lib::with_loom("child-handoff", Some(wrapper.clone()));
    let log = lib.root.join("loom-calls.log");
    write_exe(&wrapper, &format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexec '{}' \"$@\"\n", log.display(), real.display()));
    lib.stand_in(false);
    lib.bindings(|t| t.replace("      argv: [tools/draft-write.sh]\n", "      argv: [tools/draft-write.sh]\n    handoff: \"decide, then run again\"\n    handoff_when: [tools/draft-wait.sh]\n"));
    lib.tool("draft-wait.sh", "#!/bin/sh\n[ -e ../waited ] && exit 1\ntouch ../waited\nexit 0\n");
    let o = lib.run();
    let events = std::fs::read_to_string(lib.run_dir().join("events.jsonl")).unwrap_or_default();
    assert!(events.contains("Handoff"), "the run took no handoff: {}\n{events}", lib.seen(&o));
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    let runs: Vec<&str> = calls.lines().filter(|l| l.starts_with("tree run ")).collect();
    assert!(runs.len() >= 2, "too few calls of tree run to hold a handoff: {calls}");
    for call in &runs {
        assert!(call.ends_with(" --max-steps 1"), "a tree run call of the run takes more than one step: {call}");
    }
    let _ = std::fs::remove_file(&wrapper);
}
