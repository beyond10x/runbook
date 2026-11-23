//! The limit of the rule that finds a model node (`docs/loom-run.md`, "A run without a model node",
//! "The limit of the rule"): a bound program that starts a model program by a name the scan does not
//! read is not seen. One case per sentence of the page. Programs are sh scripts with invented text; a
//! "model program" is a stand-in that writes a marker file, and "workspace code" is a hand-written
//! script. A fake `b10x-loom` stands in for `substrate`, which this host cannot run (no delegated
//! cgroup); the real library Loom runs the `local` cases. No model call, no network request.

use super::core::{code, model_call_hook, spent, stderr, World};
use super::fake::tree_run_json;
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

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

fn write_exe(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn two_steps(w: &World) {
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
}

fn world(tag: &str) -> World {
    let w = World::new(tag);
    std::fs::write(w.root.join("bindings/sdc.yaml"), NO_MODEL).unwrap();
    two_steps(&w);
    w
}

/// Run `r1` under `substrate` with every cap at 0 and no ledger: the one way a run starts without caps.
fn zero_substrate(w: &World) -> Vec<String> {
    let mut a = w.args("r1", 0);
    set(&mut a, "--max-llm-calls", "0");
    without(&mut a, "--spend-ledger");
    set(&mut a, "--confinement", "substrate");
    a
}

/// A program the scan does not read: it builds the model tool's name from two parts and starts it.
/// `sdc-node` on the PATH is the stand-in; it writes `marker` and, when `records` is set, the cost
/// record of one call of 0.4 USD (the model tool's own record, as `model_call_hook` writes it).
fn hidden_start(w: &World, records: bool) -> PathBuf {
    let marker = w.root.join("marker-unread-program");
    let stand_in = w.root.join("sdc-node");
    write_exe(&stand_in, &format!("#!/bin/sh\necho started >> '{}'\n", marker.display()));
    let start = "p=sdc-; PATH=\"$ROOT:$PATH\" \"${p}node\" node.write\n".replace("$ROOT", &w.root.display().to_string());
    let hook = if records { format!("{start}{}", model_call_hook("0.4")) } else { start };
    w.fake.hook("tree_run", "1", &hook);
    marker
}

fn outcome_triple(w: &World) -> (Option<bool>, Option<u64>, Option<u64>) {
    let o = w.outcome("r1");
    (o["has_model_node"].as_bool(), o["llm_calls"].as_u64(), o["spent_micro_usd"].as_u64())
}

#[test]
fn an_unread_model_program_is_started_and_gets_no_login() {
    let w = world("limit-login");
    let dump = w.root.join("env-of-the-program.txt");
    let reach = w.root.join("reach-of-the-program.txt");
    let xdg = w.root.join("xdg").display().to_string();
    let marker = hidden_start(&w, false);
    // the program also looks for the login where a model node's program would find it
    let looks = format!("env > '{}'\ncat '{xdg}'/runbook/r1/login > '{}' 2>/dev/null\n", dump.display(), reach.display());
    let hook = std::fs::read_to_string(w.fake.dir.join("script/tree_run.1.sh")).unwrap();
    w.fake.hook("tree_run", "1", &format!("{hook}{looks}"));
    let o = w.run(&zero_substrate(&w));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(marker.exists(), "the program was not started");
    let env = std::fs::read_to_string(&dump).unwrap();
    for needle in ["LOGIN-FILE-MARKER-4711", "credentials.json", "loginhome", "ANTHROPIC", "XDG_RUNTIME_DIR"] {
        assert!(!env.contains(needle), "{needle} is in the environment of the program:\n{env}");
    }
    let names: Vec<&str> = env.lines().filter_map(|l| l.split_once('=').map(|(k, _)| k)).filter(|k| !["PWD", "OLDPWD", "SHLVL", "_"].contains(k)).collect();
    let other: Vec<&&str> = names.iter().filter(|k| !["PATH", "HOME", "XDG_DATA_HOME"].contains(k) && !k.starts_with("LOOM_")).collect();
    assert!(other.is_empty(), "the program is given more than PATH, HOME, XDG_DATA_HOME (inside the run directory) and LOOM_*: {other:?}");
    let run_dir = w.run_dir("r1").display().to_string();
    for l in env.lines().filter(|l| l.starts_with("HOME=") || l.starts_with("XDG_DATA_HOME=")) {
        assert!(l.split_once('=').unwrap().1.starts_with(&run_dir), "{l} is not inside the run directory");
    }
    assert!(std::fs::read_to_string(&reach).unwrap_or_default().is_empty(), "the program read a login through a link of the run");
    assert!(!w.root.join("xdg/runbook").exists(), "a private directory was made");
}

#[test]
fn an_unread_model_program_that_records_a_call_ends_the_run_without_caps() {
    let w = world("limit-recorded");
    let marker = hidden_start(&w, true);
    let o = w.run(&zero_substrate(&w));
    assert_eq!(code(&o), 13, "{}", stderr(&o));
    assert!(marker.exists(), "the program was not started");
    let outcome = w.outcome("r1");
    assert_eq!(outcome["cause"], "model_call_without_model_node", "{outcome}");
    assert_eq!(outcome_triple(&w), (Some(false), Some(1), Some(400_000)), "{outcome}");
    assert!(!w.ledger().exists(), "a ledger was made");
}

#[test]
fn an_unread_model_program_that_records_nothing_is_not_seen() {
    let w = world("limit-unrecorded");
    let marker = hidden_start(&w, false);
    let o = w.run(&zero_substrate(&w));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(marker.exists(), "the program was not started");
    assert_eq!(outcome_triple(&w), (Some(false), Some(0), Some(0)), "{}", w.outcome("r1"));
}

#[test]
fn under_local_with_caps_an_unread_model_program_that_records_a_call_is_booked_and_counted() {
    let w = world("limit-local-recorded");
    let marker = hidden_start(&w, true);
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(marker.exists(), "the program was not started");
    assert_eq!(outcome_triple(&w), (Some(false), Some(1), Some(400_000)), "{}", w.outcome("r1"));
    assert_eq!(spent(&w), 400_000, "the call was not booked in the ledger");
}

#[test]
fn under_local_with_caps_an_unread_model_program_that_records_nothing_is_counted_by_no_cap() {
    let w = world("limit-local-unrecorded");
    let marker = hidden_start(&w, false);
    let mut a = w.args("r1", 1);
    set(&mut a, "--max-llm-calls", "1");
    let o = w.run(&a);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(marker.exists(), "the program was not started");
    assert_eq!(outcome_triple(&w), (Some(false), Some(0), Some(0)), "{}", w.outcome("r1"));
    assert_eq!(spent(&w), 0);
}

#[test]
fn the_tool_name_in_another_letter_case_is_decided_by_the_scan_as_it_is_written() {
    let w = world("limit-letter-case");
    let bindings = "format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: a.b\n    executor: {kind: program, argv: [SDC-NODE, a.b]}\n";
    std::fs::write(w.root.join("bindings/sdc.yaml"), bindings).unwrap();
    let o = w.run(&zero_substrate(&w));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["has_model_node"], false, "the other letter case counted as the model tool: {outcome}");
    assert!(outcome["because"].is_string() || std::fs::read_to_string(w.run_dir("r1").join("events.jsonl")).unwrap().contains("model_node_decided"), "the decision is not stated");
}

// ---- the code of a ticket's repository: the real library Loom, under local, with caps ----

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if src.is_dir() {
            copy_tree(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).unwrap();
        }
    }
}

/// The step that a committed binding gives a workspace: it runs the workspace's own script (here a
/// stand-in for a ticket repository's test command) from the workspace, as `bindings/sdc-bugfix.yaml`
/// runs `task check`. The script writes a file outside the run directory, lists its environment, and
/// starts a program that is a model program by its work and writes no cost record.
#[test]
fn the_code_of_a_ticket_repository_runs_with_the_reach_of_any_bound_program_and_the_caps_count_nothing_it_does_by_itself() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("limit-ticket-code-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for d in ["runs", "xdg", "bin", "login"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    std::fs::set_permissions(root.join("xdg"), std::fs::Permissions::from_mode(0o700)).unwrap();
    copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &root.join("fx"));
    std::fs::remove_file(root.join("fx/bindings/engine-demo.yaml")).unwrap();
    for tool in ["draft-check.sh", "draft-hold.sh", "draft-write.sh"] {
        std::fs::set_permissions(root.join("fx/bindings/tools").join(tool), std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    super::decider::install_routing_table_at(&root.join("fx"));
    std::fs::write(root.join("idea.yaml"), "text: a note on tea\n").unwrap();
    let login_text = "STAND-IN-LOGIN-TEXT-5518";
    std::fs::write(root.join("login/credentials.json"), login_text).unwrap();
    let marker = root.join("marker-model-program");
    let outside = root.join("written-outside-the-run");
    // the stand-in model program: a marker and no cost record
    write_exe(&root.join("bin/model-stand-in"), &format!("#!/bin/sh\necho started >> '{}'\n", marker.display()));
    // the repository's own code: runs from the workspace, writes outside, starts the program
    let code_of_the_repository = format!(
        "#!/bin/sh\npwd > code-cwd.txt\nenv > code-env.txt\necho written > '{}'\nmodel-stand-in\necho draft >> draft.txt\n",
        outside.display()
    );
    write_exe(&root.join("fx/bindings/tools/draft-write.sh"), &code_of_the_repository);
    let approve = json!({"approve": true, "cost_micro_usd": 0});
    let settings = json!({"login_file": root.join("login/credentials.json").display().to_string(), "loom": {"in_process": true, "confinement": "local"}, "decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [approve, approve, approve, approve]}});
    std::fs::write(root.join("settings.json"), serde_json::to_string_pretty(&settings).unwrap()).unwrap();
    let r = |p: &str| root.join(p).display().to_string();
    let args: Vec<String> = [
        "run", "sdc", "--run", "r1", "--runs-dir", &r("runs"), "--bindings", &r("fx/bindings"), "--protocols", &r("fx/protocol"), "--root-protocol", "engine-draft@1", "--case", "DRAFT-1", "--intake", &r("idea.yaml"), "--confinement", "local",
        "--settings", &r("settings.json"), "--spend-ledger", &r("ledger.json"), "--cap-micro-usd", "1", "--max-llm-calls", "1", "--max-wall-seconds", "50",
    ]
    .map(String::from)
    .to_vec();
    let o: Output = Command::new(super::core::BIN)
        .args(&args)
        .env_clear()
        .env("PATH", format!("{}:{}", r("bin"), std::env::var("PATH").unwrap_or_default()))
        .env("HOME", "/marker-home-dir")
        .env("XDG_RUNTIME_DIR", root.join("xdg"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let outcome: Value = std::fs::read_to_string(root.join("runs/r1/outcome.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
    assert_eq!(code(&o), 0, "{outcome} {}", stderr(&o));
    let work = root.join("runs/r1/work");
    // the same reach: the workspace as its directory, PATH and LOOM_* as its environment, and a file outside the run
    assert_eq!(std::fs::read_to_string(work.join("code-cwd.txt")).unwrap().trim(), work.display().to_string());
    let env = std::fs::read_to_string(work.join("code-env.txt")).unwrap();
    let names: Vec<&str> = env.lines().filter_map(|l| l.split_once('=').map(|(k, _)| k)).filter(|k| !["PWD", "OLDPWD", "SHLVL", "_"].contains(k)).collect();
    let other: Vec<&&str> = names.iter().filter(|k| **k != "PATH" && !k.starts_with("LOOM_")).collect();
    assert!(other.is_empty(), "the code is given more than PATH and LOOM_*: {other:?}");
    assert_eq!(std::fs::read_to_string(&outside).unwrap_or_default().trim(), "written", "the code could not write outside the run directory");
    // no login: not in its environment, not in a file of the run, no private directory
    assert!(!env.contains(login_text) && !env.contains("credentials.json"), "the login is in the environment of the code:\n{env}");
    let mut found = Vec::new();
    super::core::grep_tree(&root.join("runs/r1"), login_text, &mut found);
    assert!(found.is_empty(), "files of the run hold the text of the login: {found:?}");
    assert!(!root.join("xdg/runbook").exists(), "a private directory was made");
    // and what it started by itself is counted by no cap
    assert!(marker.exists(), "the program the code started did not run");
    assert_eq!((outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some(0), Some(0)), "{outcome}");
    let ledger: Value = serde_json::from_str(&std::fs::read_to_string(root.join("ledger.json")).unwrap()).unwrap();
    assert_eq!(ledger["spent_micro_usd"].as_u64(), Some(0), "{ledger}");
}
