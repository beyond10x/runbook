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

/// The options of run `r1` with every cap at 0 and no ledger.
fn zero(w: &World) -> Vec<String> {
    let mut a = w.args("r1", 0);
    set(&mut a, "--max-llm-calls", "0");
    without(&mut a, "--spend-ledger");
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
        if code(&o) != 2 || !stderr(&o).contains("--spend-ledger is required") {
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
        let lib = Lib { root };
        lib.settings(&json!({}));
        lib
    }

    /// A scripted decider that approves and reserves nothing, so it is no model node.
    fn settings(&self, extra: &Value) {
        let approve = json!({"approve": true, "cost_micro_usd": 0});
        let mut v = json!({"loom": {"in_process": true, "confinement": "local"}, "decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [approve, approve, approve, approve]}});
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

    /// Run `r1`, the library way, with caps of 0 and no ledger.
    fn run(&self) -> Output {
        let r = |p: &str| self.root.join(p).display().to_string();
        let args = [
            "run".to_owned(), "sdc".into(), "--run".into(), "r1".into(), "--runs-dir".into(), r("runs"), "--bindings".into(), r("fx/bindings"), "--protocols".into(), r("fx/protocol"),
            "--root-protocol".into(), "engine-draft@1".into(), "--case".into(), "DRAFT-1".into(), "--intake".into(), r("idea.yaml"), "--confinement".into(), "local".into(),
            "--settings".into(), r("settings.json"), "--cap-micro-usd".into(), "0".into(), "--max-llm-calls".into(), "0".into(), "--max-wall-seconds".into(), "50".into(),
        ];
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

#[test]
fn real_loom_a_run_without_a_model_node_ends_with_caps_of_0_and_no_ledger() {
    let lib = Lib::new("plain");
    lib.stand_in(true);
    let o = lib.run();
    assert_eq!(code(&o), 0, "{}", lib.seen(&o));
    let outcome = lib.outcome();
    assert_eq!((outcome["outcome"].as_str(), outcome["has_model_node"].as_bool(), outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some("ended"), Some(false), Some(0), Some(0)), "{outcome}");
    assert!(!lib.marker().exists());
    assert!(!lib.root.join("ledger.json").exists());
    assert_eq!(std::fs::read_to_string(lib.work().join("draft.txt")).unwrap(), "draft\n");
}

/// The line of the unit with Loom itself, not the fake: the writer of the first step rewrites the
/// run's copy of the bindings so that the next step is the model tool. Loom reads that file at
/// every `tree run`, so only the runner's check before the call keeps the program from starting.
#[test]
fn real_loom_a_copy_rewritten_by_a_bound_program_to_name_the_model_tool_is_refused_before_the_program_starts() {
    let lib = Lib::new("rewrite-copy");
    lib.stand_in(true);
    lib.tool("draft-write.sh", &format!("#!/bin/sh\nsed -i '{NAME_THE_TOOL}' ../bindings/engine-draft.yaml\necho draft >> draft.txt\n"));
    let o = lib.run();
    let copy = std::fs::read_to_string(lib.run_dir().join("bindings/engine-draft.yaml")).unwrap();
    assert!(copy.contains("sdc-node"), "the writer did not rewrite the copy: {copy}");
    assert!(!lib.marker().exists(), "the model program started: {}", lib.seen(&o));
    assert_eq!(code(&o), 13, "{}", lib.seen(&o));
    let outcome = lib.outcome();
    assert_eq!((outcome["cause"].as_str(), outcome["has_model_node"].as_bool(), outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some("model_call_without_model_node"), Some(false), Some(0), Some(0)), "{outcome}");
    assert!(!lib.root.join("ledger.json").exists());
}

/// The same bound program, one file further: it leaves the copy as it is and makes the tree's
/// manifest (`store/tree.json`, member `bindings`) name a bindings file elsewhere in the run
/// directory, in which the next step is the model tool. The check before a `tree run` has to hold
/// for the bindings Loom reads, whichever file that is.
#[test]
fn real_loom_a_manifest_rewritten_to_name_other_bindings_does_not_start_a_model_program() {
    let lib = Lib::new("rewrite-manifest");
    lib.stand_in(true);
    lib.tool(
        "draft-write.sh",
        &format!(
            "#!/bin/sh\nif [ ! -d ../alt ]; then\n  cp -r ../bindings ../alt\n  sed -i '{NAME_THE_TOOL}' ../alt/engine-draft.yaml\n  alt=$(cd ../alt && pwd)\n  sed -i \"s#\\\"bindings\\\": *\\\"[^\\\"]*\\\"#\\\"bindings\\\":\\\"$alt/engine-draft.yaml\\\"#\" ../store/tree.json\nfi\necho draft >> draft.txt\n"
        ),
    );
    let o = lib.run();
    let manifest = std::fs::read_to_string(lib.run_dir().join("store/tree.json")).unwrap();
    assert!(manifest.contains("/alt/engine-draft.yaml"), "the writer did not rewrite the manifest: {manifest}");
    assert!(!std::fs::read_to_string(lib.run_dir().join("bindings/engine-draft.yaml")).unwrap().contains("sdc-node"), "the copy is as it was");
    assert!(!lib.marker().exists(), "the model program started by a tree run of a run opened without a model node: {}", lib.seen(&o));
}

/// The documented limit (`docs/loom-run.md`, "A run without a model node"). The forms the rule
/// does not read, present at the opening: the model tool reached by a name that is in no YAML
/// text. The runner cannot see a name that a program of the runbook's author builds when it runs, so
/// the program starts. This case asserts what holds then, for each form: it is started; no login of
/// the runner reaches it (not in its environment, not through a file of the run's private
/// directory); a call it records ends the run by exit 13 after that step with the call and the
/// cost in the records and nothing in a ledger; a call it does not record is not seen (exit 0, no
/// call counted).
///
/// The first version of this case, the review's, asserted that no such program starts, and was red
/// by design. That assertion is replaced by these: the limit is documented, not closed here.
#[test]
fn documented_limit_a_model_program_reached_by_a_name_the_rule_does_not_read_starts_without_a_login_and_a_recorded_call_ends_the_run() {
    type Setup = Box<dyn Fn(&Lib)>;
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
    let mut wrong: Vec<String> = Vec::new();
    for (n, (what, records, setup)) in forms.iter().enumerate() {
        let lib = Lib::new(&format!("unread-{n}"));
        lib.stand_in(*records);
        lib.settings(&json!({"login_file": lib.root.join("login/credentials.json").display().to_string()}));
        setup(&lib);
        let o = lib.run();
        let seen = lib.seen(&o);
        if !lib.marker().exists() {
            wrong.push(format!("{what}: the program did not start (the limit is gone: say so in the document): {seen}"));
            continue;
        }
        let outcome = lib.outcome();
        let want = if *records { (13, Some("model_call_without_model_node"), Some(1), Some(250_000)) } else { (0, None, Some(0), Some(0)) };
        let got = (code(&o), outcome["cause"].as_str(), outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64());
        if got != want || outcome["has_model_node"] != false || lib.root.join("ledger.json").exists() {
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
    }
    assert!(wrong.is_empty(), "the documented limit does not hold as documented:\n{}", wrong.join("\n"));
}

/// What the runner does for the forms above when the program is the model tool (it writes the
/// cost record of its call): the run ends after that step by the named cause, and its records
/// state the call and the cost.
#[test]
fn real_loom_a_call_recorded_by_a_model_program_the_rule_did_not_read_ends_the_run_after_the_step() {
    let lib = Lib::new("unread-recorded");
    lib.stand_in(true);
    lib.tool("draft-check.sh", "#!/bin/sh\np=sdc-\n\"${p}node\" draft.check\n");
    let o = lib.run();
    assert_eq!(code(&o), 13, "{}", lib.seen(&o));
    let outcome = lib.outcome();
    assert_eq!((outcome["cause"].as_str(), outcome["has_model_node"].as_bool(), outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some("model_call_without_model_node"), Some(false), Some(1), Some(250_000)), "{outcome}");
    assert_eq!(std::fs::read_to_string(lib.marker()).unwrap().lines().count(), 1, "one call, then the run ended");
    assert!(!lib.root.join("ledger.json").exists());
}

/// What a bound program of a run without a model node can reach of the login: the settings name a
/// login file (a stand-in with invented text), as one settings file shared by every runbook does.
/// No model node of this run reads it, so nothing of the run should hold a way to it.
#[test]
fn real_loom_a_run_without_a_model_node_gives_its_bound_programs_no_way_to_the_login_file() {
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

// ---- the tree manifest: what Loom reads to decide which bindings apply ----

/// The first step's writer, run once: it leaves the copy as it is and changes only the tree
/// manifest (`$change` is a shell snippet run in the work directory, the run directory is `..`).
/// `alt` is a copy of the bindings that names the model tool for the step after it, so a manifest
/// that names it starts the stand-in unless the runner's check refuses first.
fn manifest_writer(change: &str) -> String {
    format!(
        "#!/bin/sh\nif [ ! -e ../done ]; then\n  touch ../done\n  cp -r ../bindings ../alt\n  sed -i '{NAME_THE_TOOL}' ../alt/engine-draft.yaml\n  alt=$(cd ../alt && pwd)\n  run=$(cd .. && pwd)\n  {change}\nfi\necho draft >> draft.txt\n"
    )
}

const POINT_AT_ALT: &str = "sed \"s#\\\"bindings\\\": *\\\"[^\\\"]*\\\"#\\\"bindings\\\":\\\"$alt/engine-draft.yaml\\\"#\" ../store/tree.json";

/// What every refused form of the manifest must show: exit 13 by the named cause, no model
/// program started, no ledger, and the records of a run that did not change its opening decision.
fn assert_refused_for_the_manifest(lib: &Lib, o: &Output) {
    assert!(!lib.marker().exists(), "the model program started: {}", lib.seen(o));
    assert_eq!(code(o), 13, "{}", lib.seen(o));
    let outcome = lib.outcome();
    assert_eq!((outcome["cause"].as_str(), outcome["has_model_node"].as_bool(), outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some("model_call_without_model_node"), Some(false), Some(0), Some(0)), "{outcome}");
    assert!(outcome["reason"].as_str().unwrap_or("").contains("tree manifest"), "the reason does not name the manifest: {outcome}");
    assert!(!lib.root.join("ledger.json").exists());
}

#[test]
fn real_loom_a_manifest_replaced_by_a_link_is_refused_before_a_program_starts() {
    let lib = Lib::new("manifest-link");
    lib.stand_in(true);
    lib.tool("draft-write.sh", &manifest_writer(&format!("{POINT_AT_ALT} > ../tree-alt.json\n  rm ../store/tree.json\n  ln -s ../tree-alt.json ../store/tree.json")));
    let o = lib.run();
    assert!(std::fs::symlink_metadata(lib.run_dir().join("store/tree.json")).unwrap().file_type().is_symlink(), "the writer did not replace the manifest by a link");
    assert_refused_for_the_manifest(&lib, &o);
}

#[test]
fn real_loom_a_manifest_that_is_removed_is_refused_before_a_program_starts() {
    let lib = Lib::new("manifest-removed");
    lib.stand_in(true);
    lib.tool("draft-write.sh", &manifest_writer("rm ../store/tree.json"));
    let o = lib.run();
    assert!(!lib.run_dir().join("store/tree.json").exists(), "the writer did not remove the manifest");
    assert_refused_for_the_manifest(&lib, &o);
}

/// A manifest member that names the root file of the copy by a path that is not the one the
/// runner wrote is accepted when it resolves to that file (`.` and `..` inside the path), and the
/// run goes on as it would have.
#[test]
fn real_loom_a_manifest_that_names_the_root_file_by_another_spelling_is_accepted() {
    let lib = Lib::new("manifest-spelling");
    lib.stand_in(true);
    lib.tool(
        "draft-write.sh",
        &manifest_writer("sed -i \"s#/engine-draft.yaml\\\"#/../bindings/./engine-draft.yaml\\\"#\" ../store/tree.json"),
    );
    let o = lib.run();
    let manifest = std::fs::read_to_string(lib.run_dir().join("store/tree.json")).unwrap();
    assert!(manifest.contains("/../bindings/./engine-draft.yaml"), "the writer did not change the spelling: {manifest}");
    assert_eq!(code(&o), 0, "{}", lib.seen(&o));
    assert!(!lib.marker().exists(), "{}", lib.seen(&o));
    assert_eq!(lib.outcome()["outcome"], "ended");
}

/// A manifest member that names the root file through a link is refused although it resolves to
/// the same file: what the manifest names must be the file the runner wrote, not a way to it.
#[test]
fn real_loom_a_manifest_that_names_the_root_file_through_a_link_is_refused() {
    let lib = Lib::new("manifest-named-link");
    lib.stand_in(true);
    lib.tool("draft-write.sh", &manifest_writer("ln -s \"$run/bindings/engine-draft.yaml\" \"$run/same.yaml\"\n  sed -i \"s#\\\"bindings\\\": *\\\"[^\\\"]*\\\"#\\\"bindings\\\":\\\"$run/same.yaml\\\"#\" ../store/tree.json"));
    let o = lib.run();
    assert!(std::fs::read_to_string(lib.run_dir().join("store/tree.json")).unwrap().contains("/same.yaml"), "the writer did not rewrite the manifest");
    assert_refused_for_the_manifest(&lib, &o);
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
    for (what, hook) in [("as written", ":"), ("with dots in the path", "printf '{\"bindings\":\"%s/bindings/../bindings/./sdc.yaml\"}' \"$PWD\" > \"$store/tree.json\"")] {
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
