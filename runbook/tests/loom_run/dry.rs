//! `runbook run --dry`: the stub cycle with every model node on the adapter `dry`, the person's
//! decisions approved by the rule decider, every command a model call would have run printed, and no
//! model call, no login and no spend. Holds the cycle entries `models` and `efforts` of
//! `cycles/suite.yaml` (`docs/cycles-suite.md`).
//!
//! Most tests run the loop against a fake `b10x-loom` (`fake.rs`) on the committed `bindings/`; one
//! runs the loop in process on the small fixture runbook, so that Loom itself takes the principal.
//! No test starts a model: the adapter `dry` and the rule decider call none.

use super::core::{code, spent, stderr, World};
use super::fake::tree_run_json;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        let kind = entry.file_type().unwrap();
        if kind.is_symlink() {
            std::os::unix::fs::symlink(std::fs::read_link(entry.path()).unwrap(), &target).unwrap();
        } else if kind.is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// The settings file without a login: a dry run needs none.
fn settings_without_login(w: &World, settings: &Value) {
    std::fs::write(w.root.join("settings.toml"), super::core::toml_text(settings)).unwrap();
}

fn dry_args(w: &World, name: &str) -> Vec<String> {
    let mut a = w.args(name, 5_000_000);
    a.push("--dry".into());
    a
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
}

/// The committed bindings and their tools in place of the world's one-node bindings, with the
/// instruction files and the rule file the tools read from their root.
fn committed_bindings(w: &World) {
    std::fs::remove_dir_all(w.root.join("bindings")).unwrap();
    copy_tree(&repo().join("bindings"), &w.root.join("bindings"));
    copy_tree(&repo().join("instructions"), &w.root.join("instructions"));
    std::fs::create_dir_all(w.root.join("driver/examples/sdc")).unwrap();
    std::fs::copy(repo().join("driver/examples/sdc/rule-ends.json"), w.root.join("driver/examples/sdc/rule-ends.json")).unwrap();
}

/// The scripted answer of a tree that takes one step, asks one decision (no path, so the rule
/// decider may answer it), takes one step, ends.
fn approval_tree(w: &World) {
    let f = &w.fake;
    f.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    f.answer("tree_run", "2", &tree_run_json("AwaitingPerson", 0, None, &format!("[{}]", super::decider::approval())), 3);
    f.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
    f.answer("tree_run", "3", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    f.answer("tree_run", "4", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
}

/// A `tree_run` hook: one call of `node.write` on the adapter `dry`, with the record the adapter
/// leaves (`argv.json`, `request.json`) and the cost line the node writes (`cost_usd` null).
const DRY_CALL_HOOK: &str = "w=work/.sdc/llm; mkdir -p \"$w\"\n\
d=001-node.write; mkdir -p \"$w/$d\"\n\
printf '[\"/x/metaharness-stand-in\",\"run\",\"claude\",\"--model\",\"model-writer\",\"--effort\",\"high\",\"-p\",\"<prompt_file>\"]' > \"$w/$d/argv.json\"\n\
printf '{\"prompt_file\":\"%s/%s/prompt.md\"}' \"$PWD/$w\" \"$d\" > \"$w/$d/request.json\"\n\
printf '{\"action\":\"node.write\",\"adapter\":\"dry\",\"dry\":true,\"cost_usd\":null,\"exit\":0,\"record\":\"%s/%s\"}\\n' \"$PWD/$w\" \"$d\" >> \"$w/calls.jsonl\"\n";

#[test]
fn a_dry_run_prints_every_command_runs_no_model_and_spends_nothing() {
    let w = World::new("dry-run");
    approval_tree(&w);
    w.fake.hook("tree_run", "1", DRY_CALL_HOOK);
    settings_without_login(&w, &json!({}));
    let o = w.run(&dry_args(&w, "r1"));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let dir = w.run_dir("r1");
    let prompt = dir.join("work/.sdc/llm/001-node.write/prompt.md");
    let line = format!("would run: metaharness 'run' 'claude' '--model' 'model-writer' '--effort' 'high' '-p' \"$(cat {})\"", prompt.display());
    assert_eq!(stdout(&o), format!("  {line}\n"), "the command is printed once, indented as the old launcher did");
    assert_eq!(std::fs::read_to_string(dir.join("dry-commands.txt")).unwrap(), format!("{line}\n"));
    assert_eq!(spent(&w), 0, "a dry run books nothing in the spend ledger");
    let outcome = w.outcome("r1");
    assert_eq!((outcome["outcome"].as_str(), outcome["spent_micro_usd"].as_u64()), (Some("ended"), Some(0)), "{outcome}");
}

#[test]
fn a_dry_run_needs_none_of_the_three_money_options() {
    let w = World::new("dry-bare");
    approval_tree(&w);
    w.fake.hook("tree_run", "1", DRY_CALL_HOOK);
    settings_without_login(&w, &json!({}));
    let mut a = dry_args(&w, "r1");
    for flag in ["--spend-ledger", "--cap-micro-usd", "--max-llm-calls"] {
        let i = a.iter().position(|x| x == flag).unwrap();
        a.drain(i..i + 2);
    }
    let o = w.run(&a);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!((outcome["outcome"].as_str(), outcome["spent_micro_usd"].as_u64()), (Some("ended"), Some(0)), "{outcome}");
    assert_eq!(outcome["llm_calls"], 1, "the dry call is counted and costs nothing: {outcome}");
    assert!(!w.ledger().exists(), "no ledger is opened");
    assert!(w.run_dir("r1").join("dry-commands.txt").is_file());
}

#[test]
fn the_person_s_decision_is_approved_by_the_rule_decider_and_the_run_says_it_is_a_stand_in() {
    let w = World::new("dry-person");
    approval_tree(&w);
    settings_without_login(&w, &json!({}));
    let o = w.run(&dry_args(&w, "r1"));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let approves = w.fake.calls_of(&["tree", "approve"]);
    assert_eq!(approves.len(), 1);
    let at = approves[0].argv.iter().position(|x| x == "--principal").unwrap();
    assert_eq!(approves[0].argv[at + 1], "decider:approve");
    let settings = read_json(&w.run_dir("r1").join("settings.json"));
    assert_eq!((settings["dry"].clone(), settings["stand_in"].clone()), (json!(true), json!("runbook run --dry")), "{settings}");
    let first = std::fs::read_to_string(w.run_dir("r1").join("run-settings.jsonl")).unwrap();
    let line: Value = serde_json::from_str(first.lines().next().unwrap()).unwrap();
    assert_eq!((line["stand_in"].clone(), line["adapter"].clone()), (json!("runbook run --dry"), json!("dry")), "{line}");
}

#[test]
fn a_dry_run_needs_no_login_and_names_none() {
    let w = World::new("dry-login");
    approval_tree(&w);
    settings_without_login(&w, &json!({}));
    let o = w.run(&dry_args(&w, "r1"));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let settings = read_json(&w.run_dir("r1").join("settings.json"));
    assert!(settings.get("login_file").is_none(), "{settings}");
    assert!(!w.run_dir("r1").join("private").exists() && std::fs::read_dir(w.run_dir("r1")).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().contains("login")));
}

#[test]
fn every_model_node_of_the_copy_runs_the_adapter_dry_and_the_committed_bindings_are_not_touched() {
    let w = World::new("dry-copy");
    approval_tree(&w);
    settings_without_login(&w, &json!({}));
    let before = std::fs::read_to_string(w.root.join("bindings/sdc.yaml")).unwrap();
    let o = w.run(&dry_args(&w, "r1"));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let copy = std::fs::read_to_string(w.run_dir("r1").join("bindings/sdc.yaml")).unwrap();
    assert!(copy.contains("--adapter") && copy.contains("- dry"), "{copy}");
    assert_eq!(std::fs::read_to_string(w.root.join("bindings/sdc.yaml")).unwrap(), before);
}

#[test]
fn the_adapter_in_the_settings_is_refused_with_the_flag_and_dry_is_refused_without_it() {
    for (flag, settings, want) in [
        (true, json!({"adapter": "metaharness"}), "--dry runs every model node on the adapter dry"),
        (true, json!({"decider": {"kind": "model", "model": "m", "adapter": "claude-cli", "without_claims": true}}), "--dry decides on the adapter dry"),
        (false, json!({"adapter": "dry"}), "dry is what --dry sets"),
    ] {
        let w = World::new("dry-refused");
        approval_tree(&w);
        settings_without_login(&w, &settings);
        let mut a = w.args("r1", 1_000_000);
        if flag {
            a.push("--dry".into());
        }
        let o = w.run(&a);
        assert_eq!(code(&o), 2, "{settings}: {}", stderr(&o));
        assert!(stderr(&o).contains(want), "{settings}: {}", stderr(&o));
        assert!(!w.run_dir("r1").exists() && !w.ledger().exists(), "{settings}: something was written");
        assert_eq!(w.fake.calls_so_far(), 0);
    }
}

#[test]
fn the_required_options_stay_required_with_dry() {
    let w = World::new("dry-required");
    settings_without_login(&w, &json!({}));
    for flag in ["--max-wall-seconds", "--confinement"] {
        let mut a = dry_args(&w, "r1");
        let i = a.iter().position(|x| x == flag).unwrap();
        a.drain(i..i + 2);
        let o = w.run(&a);
        assert_eq!(code(&o), 64, "{flag}: {}", stderr(&o));
        assert!(stderr(&o).contains(flag), "{flag}: {}", stderr(&o));
        assert!(!w.run_dir("r1").exists() && !w.ledger().exists(), "{flag}: something was written");
    }
}

/// The model nodes of the copy: action, `--model` value (if any) and `--adapter` value.
fn model_nodes(copy: &Path) -> Vec<(String, Option<String>, String)> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(copy).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|x| x != "yaml") {
            continue;
        }
        let doc: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for action in doc["actions"].as_sequence().into_iter().flatten() {
            let argv: Vec<String> = action["executor"]["argv"].as_sequence().into_iter().flatten().filter_map(|a| a.as_str().map(str::to_owned)).collect();
            let value = |flag: &str| argv.iter().position(|a| a == flag).and_then(|i| argv.get(i + 1)).cloned();
            if let Some(adapter) = value("--adapter") {
                out.push((action["action"].as_str().unwrap().to_owned(), value("--model"), adapter));
            }
        }
    }
    out.sort();
    out
}

/// The class of an action in `node-classes.json`; `other` when no class lists it.
fn class_of(classes: &Value, action: &str) -> String {
    classes["classes"].as_object().unwrap().iter().find(|(_, names)| names.as_array().unwrap().iter().any(|n| n == action)).map_or("other", |(k, _)| k.as_str()).to_owned()
}

/// A run of the committed bindings with `models` in the settings, and what the copy says.
fn models_run(tag: &str, models: &Value) -> Vec<(String, Option<String>, String)> {
    let w = World::new(tag);
    committed_bindings(&w);
    w.fake.answer("tree_run", "default", &tree_run_json("Terminal", 0, Some("operating"), "[]"), 0);
    let mut settings = json!({"autonomy": "A5", "decider": {"kind": "model", "model": "model-decider", "without_claims": true}});
    if !models.is_null() {
        settings["models"] = models.clone();
    }
    settings_without_login(&w, &settings);
    let o = w.run(&dry_args(&w, "r1"));
    assert_eq!(code(&o), 0, "{tag}: {}", stderr(&o));
    model_nodes(&w.run_dir("r1").join("bindings"))
}

#[test]
fn models_each_call_names_the_binding_s_model_or_the_one_for_all_or_the_one_of_its_class() {
    let classes = read_json(&repo().join("bindings/tools/node-classes.json"));
    let committed: Vec<(String, Option<String>)> = {
        let mut v = Vec::new();
        for entry in std::fs::read_dir(repo().join("bindings")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|x| x != "yaml") {
                continue;
            }
            let doc: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            for action in doc["actions"].as_sequence().into_iter().flatten() {
                let argv: Vec<String> = action["executor"]["argv"].as_sequence().into_iter().flatten().filter_map(|a| a.as_str().map(str::to_owned)).collect();
                let model = argv.iter().position(|a| a == "--model").and_then(|i| argv.get(i + 1)).cloned();
                if model.is_some() {
                    v.push((action["action"].as_str().unwrap().to_owned(), model));
                }
            }
        }
        v.sort();
        v
    };
    assert!(committed.len() >= 10, "the committed bindings hold {} model nodes", committed.len());

    // models-binding: no model in the settings; each node keeps the model its binding names.
    let binding = models_run("models-binding", &Value::Null);
    assert_eq!(binding.len(), committed.len(), "{binding:?}");
    for (action, model, adapter) in &binding {
        assert_eq!(adapter, "dry", "{action}");
        assert_eq!(model, &committed.iter().find(|(a, _)| a == action).unwrap().1, "{action}");
    }

    // models-all: one model for the four roles.
    let all = models_run("models-all", &json!({"writer": {"model": "model-all"}, "implement": {"model": "model-all"}, "review": {"model": "model-all"}, "other": {"model": "model-all"}}));
    assert_eq!(all.len(), committed.len());
    for (action, model, adapter) in &all {
        assert_eq!((model.as_deref(), adapter.as_str()), (Some("model-all"), "dry"), "{action}");
    }

    // models-class: one model per class.
    let class = models_run(
        "models-class",
        &json!({"writer": {"model": "model-writer"}, "implement": {"model": "model-implement"}, "review": {"model": "model-review"}, "other": {"model": "model-other"}}),
    );
    assert_eq!(class.len(), committed.len());
    let mut seen = std::collections::BTreeSet::new();
    for (action, model, adapter) in &class {
        let want = format!("model-{}", class_of(&classes, action));
        assert_eq!((model.as_deref(), adapter.as_str()), (Some(want.as_str()), "dry"), "{action}");
        seen.insert(want);
    }
    assert_eq!(seen.len(), 4, "every class has a node in the committed bindings: {seen:?}");
}

#[test]
fn the_model_decider_runs_the_adapter_dry_prints_its_command_and_costs_nothing() {
    let w = World::new("dry-decider");
    committed_bindings(&w);
    let d = super::decider::approval();
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 0, None, &format!("[{d}]")), 3);
    w.fake.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 0, Some("operating"), "[]"), 0);
    settings_without_login(&w, &json!({"autonomy": "A5", "decider": {"kind": "model", "model": "model-decider", "effort": "max", "without_claims": true}}));
    let o = w.run(&dry_args(&w, "r1"));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let out = stdout(&o);
    assert_eq!(out.lines().count(), 1, "{out}");
    assert!(out.starts_with("  would run: metaharness "), "{out}");
    assert!(out.contains("'--model' 'model-decider'") && out.contains("'--effort' 'max'"), "the decider's model and effort are on the command: {out}");
    let log = std::fs::read_to_string(w.run_dir("r1").join("dry-commands.txt")).unwrap();
    assert_eq!(log.trim_end(), out.trim_start().trim_end());
    assert_eq!(spent(&w), 0);
    let a = &w.fake.calls_of(&["tree", "approve"])[0].argv;
    let at = a.iter().position(|x| x == "--principal").unwrap();
    assert_eq!(a[at + 1], "decider:model", "a decider that is configured keeps its own principal");
}

#[test]
fn efforts_the_decider_s_effort_is_on_the_dry_command_its_default_is_the_class_default_and_a_bad_level_is_refused() {
    let classes = read_json(&repo().join("bindings/tools/node-classes.json"));
    let default = classes["default_efforts"]["decider"].as_str().unwrap().to_owned();
    for (effort, want) in [(None, default.as_str()), (Some("low"), "low"), (Some("max"), "max")] {
        let w = World::new("dry-effort");
        committed_bindings(&w);
        let d = super::decider::approval();
        w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 0, None, &format!("[{d}]")), 3);
        w.fake.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
        w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 0, Some("operating"), "[]"), 0);
        let mut decider = json!({"kind": "model", "model": "model-decider", "without_claims": true});
        if let Some(e) = effort {
            decider["effort"] = json!(e);
        }
        settings_without_login(&w, &json!({"autonomy": "A5", "decider": decider}));
        let o = w.run(&dry_args(&w, "r1"));
        assert_eq!(code(&o), 0, "{effort:?}: {}", stderr(&o));
        assert!(stdout(&o).contains(&format!("'--effort' '{want}'")), "{effort:?}: {}", stdout(&o));
    }
    let w = World::new("dry-effort-bad");
    committed_bindings(&w);
    settings_without_login(&w, &json!({"autonomy": "A5", "decider": {"kind": "model", "model": "m", "effort": "bogus", "without_claims": true}}));
    let o = w.run(&dry_args(&w, "r1"));
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(stderr(&o).contains("low, medium, high, xhigh, max"), "{}", stderr(&o));
    assert!(!w.run_dir("r1").exists(), "a bad effort is refused before the run directory is made");
}

/// The model nodes of the copy of a dry run of the committed bindings with `efforts` in the settings:
/// action and the `--effort` value it was given (`None`: the node was given none).
fn efforts_run(tag: &str, efforts: &Value) -> Vec<(String, Option<String>)> {
    let w = World::new(tag);
    committed_bindings(&w);
    w.fake.answer("tree_run", "default", &tree_run_json("Terminal", 0, Some("operating"), "[]"), 0);
    let mut settings = json!({"autonomy": "A5", "decider": {"kind": "model", "model": "model-decider", "without_claims": true}});
    if !efforts.is_null() {
        settings["efforts"] = efforts.clone();
    }
    settings_without_login(&w, &settings);
    let o = w.run(&dry_args(&w, "r1"));
    assert_eq!(code(&o), 0, "{tag}: {}", stderr(&o));
    let copy = w.run_dir("r1").join("bindings");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&copy).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|x| x != "yaml") {
            continue;
        }
        let doc: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for action in doc["actions"].as_sequence().into_iter().flatten() {
            let argv: Vec<String> = action["executor"]["argv"].as_sequence().into_iter().flatten().filter_map(|a| a.as_str().map(str::to_owned)).collect();
            if argv.iter().any(|a| a == "--adapter") {
                out.push((action["action"].as_str().unwrap().to_owned(), argv.iter().position(|a| a == "--effort").and_then(|i| argv.get(i + 1)).cloned()));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn efforts_the_class_effort_goes_to_each_model_node_of_its_class_and_none_given_leaves_the_class_default() {
    let classes = read_json(&repo().join("bindings/tools/node-classes.json"));
    let none = efforts_run("dry-efforts-default", &Value::Null);
    assert!(none.len() >= 10, "{none:?}");
    assert!(none.iter().all(|(_, effort)| effort.is_none()), "no effort in the settings: the node keeps the class default: {none:?}");

    let all = efforts_run("dry-efforts-class", &json!({"writer": {"effort": "high"}, "implement": {"effort": "xhigh"}, "review": {"effort": "medium"}, "other": {"effort": "low"}}));
    assert_eq!(all.len(), none.len());
    let mut seen = std::collections::BTreeSet::new();
    for (action, effort) in &all {
        let want = match class_of(&classes, action).as_str() {
            "writer" => "high",
            "implement" => "xhigh",
            "review" => "medium",
            _ => "low",
        };
        assert_eq!(effort.as_deref(), Some(want), "{action}");
        seen.insert(want);
    }
    assert_eq!(seen.len(), 4, "every class has a node in the committed bindings: {seen:?}");
}

#[test]
fn efforts_a_level_outside_the_set_or_a_class_that_is_none_is_refused_by_name_before_the_run_directory_is_made() {
    for (efforts, name) in [
        (json!({"writer": {"effort": "bogus"}}), "settings.efforts.writer.effort"),
        (json!({"review": {"effort": 3}}), "settings.efforts.review.effort"),
        (json!({"poet": {"effort": "low"}}), "settings.efforts.poet"),
        (json!({"other": {"level": "low"}}), "settings.efforts.other.level"),
    ] {
        let w = World::new("dry-efforts-bad");
        committed_bindings(&w);
        settings_without_login(&w, &json!({"autonomy": "A5", "decider": {"kind": "model", "model": "m", "without_claims": true}, "efforts": efforts}));
        let o = w.run(&dry_args(&w, "r1"));
        assert_eq!(code(&o), 2, "{efforts}: {}", stderr(&o));
        assert!(stderr(&o).contains(name), "{efforts}: {}", stderr(&o));
        if name.ends_with("writer.effort") {
            assert!(stderr(&o).contains("low, medium, high, xhigh, max"), "{}", stderr(&o));
        }
        assert!(!w.run_dir("r1").exists(), "{efforts}: the run directory was made");
    }
}

// ---- in process: Loom itself takes the scripted person ----

struct Inproc {
    root: PathBuf,
}

impl Inproc {
    fn new(tag: &str) -> Inproc {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("dry-inproc-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("runs")).unwrap();
        copy_tree(&repo().join("runbook/tests/fixtures/engine-runbook"), &root.join("fx"));
        std::fs::write(root.join("idea.yaml"), "text: a note on tea\n").unwrap();
        super::decider::install_routing_table_at(&root.join("fx"));
        std::fs::write(root.join("settings.toml"), "").unwrap();
        Inproc { root }
    }

    fn run(&self, name: &str, extra: &[&str]) -> Output {
        let r = |p: &str| self.root.join(p).display().to_string();
        let mut c = Command::new(super::core::BIN);
        c.args(["run", "sdc", "--run", name, "--runs-dir"])
            .args([r("runs"), "--bindings".into(), r("fx/bindings"), "--protocols".into(), r("fx/protocol")])
            .args(["--root-protocol", "engine-demo@1", "--case", "ROOT-1", "--intake"])
            .arg(r("idea.yaml"))
            .args(["--confinement", "local", "--settings"])
            .arg(r("settings.toml"))
            .args(["--spend-ledger"])
            .arg(r("ledger.json"))
            .args(["--cap-micro-usd", "1000000", "--max-llm-calls", "100", "--max-wall-seconds", "120"])
            .args(extra)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", "/marker-home-dir")
            .stdin(Stdio::null());
        c.output().unwrap()
    }
}

#[test]
fn in_process_the_rule_decider_approves_the_review_with_no_settings_and_no_login() {
    let rig = Inproc::new("person");
    let o = rig.run("r1", &["--dry"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let run = rig.root.join("runs/r1");
    let outcome = read_json(&run.join("outcome.json"));
    assert_eq!((outcome["outcome"].as_str(), outcome["spent_micro_usd"].as_u64()), (Some("ended"), Some(0)), "{outcome}");
    let events = std::fs::read_to_string(run.join("events.jsonl")).unwrap();
    let answered: Vec<Value> = events.lines().map(|l| serde_json::from_str::<Value>(l).unwrap()).filter(|e| e["event"] == "decision_answered").collect();
    assert!(!answered.is_empty());
    assert!(answered.iter().all(|e| e["principal"] == "decider:approve" && e["approve"] == true), "{answered:?}");
    let ledger = read_json(&rig.root.join("ledger.json"));
    assert_eq!(ledger["spent_micro_usd"], 0);
    // Without the flag the same run has no decider and stops at the first decision.
    let o = rig.run("r2", &[]);
    assert_eq!(code(&o), 8, "{}", stderr(&o));
}
