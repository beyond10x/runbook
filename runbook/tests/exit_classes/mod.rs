//! The exit classes of the program bindings (`story:bindings-state-their-exits`): every program
//! binding of `bindings/*.yaml` lists the exits that are a program error, every model-node binding
//! the one exit that means "not called", and the pinned Loom treats both as the binding says: an
//! error exit ends the run `ProgramError` and records nothing, a not-called exit records nothing,
//! and neither counts against `budget.max_runs`. The two runs are `runbook test` on
//! `tests/fixtures/exit-classes/exit-classes.suite.yaml`: the real `bindings/sdc-bugfix.yaml`, a
//! stand-in `sdc-node` (POSIX sh) first on `PATH` that ends with the code the cycle seeds. No
//! model, no network.

use super::BIN;
use serde_json::Value;
use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// The exit code of "not called": the one `exits.not_called` of every model-node binding lists, and
/// the one `sdc-node` ends with (`driver/tests/sdc_node.rs` holds the same number).
const NOT_CALLED: i64 = 79;

const POOL_WAIT: [(&str, &str); 5] = [
    ("sdc-story", "tests.run"),
    ("sdc-release", "acceptance.run"),
    ("sdc-bugfix", "repro.run"),
    ("sdc-bugfix", "tests.run"),
    ("sdc-bugfix", "gate.run"),
];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

/// A program binding of `bindings/<file>.yaml` as the file states it.
struct Program {
    file: String,
    action: String,
    argv0: String,
    error: Vec<i64>,
    not_called: Vec<i64>,
    /// The exit codes the binding's `from: exit` tables name (`*` is none).
    table: Vec<i64>,
}

fn codes(exits: Option<&serde_yaml::Value>, class: &str) -> Vec<i64> {
    let mut v: Vec<i64> = exits
        .and_then(|e| e.get(class))
        .and_then(|c| c.as_sequence())
        .map(|s| s.iter().filter_map(|x| x.as_i64()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

fn programs() -> Vec<Program> {
    let mut found = Vec::new();
    for e in std::fs::read_dir(repo().join("bindings")).unwrap() {
        let path = e.unwrap().path();
        if path.extension().is_none_or(|x| x != "yaml") {
            continue;
        }
        let doc: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let file = path.file_stem().unwrap().to_string_lossy().into_owned();
        for a in doc
            .get("actions")
            .and_then(|a| a.as_sequence())
            .into_iter()
            .flatten()
        {
            let executor = a.get("executor").unwrap();
            if executor.get("kind").and_then(|k| k.as_str()) != Some("program") {
                continue;
            }
            let table = a
                .get("evidence")
                .and_then(|e| e.as_sequence())
                .into_iter()
                .flatten()
                .filter(|entry| entry.get("from").and_then(|f| f.as_str()) == Some("exit"))
                .filter_map(|entry| entry.get("results").and_then(|r| r.as_mapping()))
                .flat_map(|m| {
                    m.keys()
                        .filter_map(|k| k.as_str().and_then(|s| s.parse::<i64>().ok()))
                })
                .collect();
            found.push(Program {
                file: file.clone(),
                action: a["action"].as_str().unwrap().to_string(),
                argv0: executor["argv"][0].as_str().unwrap().to_string(),
                error: codes(a.get("exits"), "error"),
                not_called: codes(a.get("exits"), "not_called"),
                table,
            });
        }
    }
    found
}

#[test]
fn every_program_binding_lists_the_exits_that_are_a_program_error() {
    let all = programs();
    assert!(
        all.len() >= 50,
        "the bindings hold at least 50 program bindings: {}",
        all.len()
    );
    let mut waits = BTreeSet::new();
    for p in &all {
        let pool = POOL_WAIT.contains(&(p.file.as_str(), p.action.as_str()));
        let want: Vec<i64> = if pool {
            vec![69, 70, 75, 126, 127]
        } else {
            vec![75, 126, 127]
        };
        assert_eq!(p.error, want, "{} {}: exits.error", p.file, p.action);
        if pool {
            waits.insert((p.file.clone(), p.action.clone()));
        }
    }
    assert_eq!(
        waits.len(),
        POOL_WAIT.len(),
        "every binding that waits for the build pool was found: {waits:?}"
    );
}

#[test]
fn a_model_node_lists_one_not_called_exit_that_no_error_list_and_no_result_table_holds() {
    let all = programs();
    let model: Vec<&Program> = all.iter().filter(|p| p.argv0 == "sdc-node").collect();
    assert_eq!(model.len(), 14, "the fourteen model nodes");
    let code = vec![NOT_CALLED];
    for p in &all {
        let want = if p.argv0 == "sdc-node" {
            code.clone()
        } else {
            Vec::new()
        };
        assert_eq!(
            p.not_called, want,
            "{} {}: exits.not_called",
            p.file, p.action
        );
        assert!(
            !p.error.contains(&code[0]),
            "{} {}: {} is a program error there",
            p.file,
            p.action,
            code[0]
        );
    }
    for p in &all {
        assert!(
            !p.table.contains(&code[0]),
            "{} {}: {} is a key of a result table",
            p.file,
            p.action,
            code[0]
        );
        for c in &p.table {
            assert!(
                !p.error.contains(c),
                "{} {}: {c} is a program error and a key of its result table",
                p.file,
                p.action
            );
        }
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

/// The suite of the fixture beside a copy of the seeds of the bug-fix cycles and a link to the
/// real `bindings/`, and a `sdc-node` that ends with the code in `exit-code.txt` of its workspace.
struct Rig {
    dir: PathBuf,
}

impl Rig {
    fn new(tag: &str, error: i64, not_called: i64) -> Rig {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("exit-classes-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let suite = dir.join("suite");
        std::fs::create_dir_all(suite.join("seeds")).unwrap();
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::create_dir_all(dir.join("run")).unwrap();
        std::fs::copy(
            repo().join("runbook/tests/fixtures/exit-classes/exit-classes.suite.yaml"),
            suite.join("exit-classes.suite.yaml"),
        )
        .unwrap();
        copy_tree(
            &repo().join("cycles/seeds/bugfix-pager"),
            &suite.join("seeds/bugfix-pager"),
        );
        std::fs::write(suite.join("seeds/code-error.txt"), format!("{error}\n")).unwrap();
        std::fs::write(
            suite.join("seeds/code-not-called.txt"),
            format!("{not_called}\n"),
        )
        .unwrap();
        std::os::unix::fs::symlink(repo().join("bindings"), suite.join("bindings")).unwrap();
        let node = dir.join("bin/sdc-node");
        std::fs::write(&node, "#!/bin/sh\nexit \"$(cat exit-code.txt)\"\n").unwrap();
        std::fs::set_permissions(&node, std::fs::Permissions::from_mode(0o755)).unwrap();
        Rig { dir }
    }

    fn run(&self, cycle: &str) -> Output {
        Command::new(BIN)
            .args(["test"])
            .arg(self.dir.join("suite/exit-classes.suite.yaml"))
            .args(["--only", cycle, "--confinement", "local", "--scratch"])
            .arg(self.dir.join("scratch"))
            .env("RUNBOOK_HOME", repo())
            .env("XDG_RUNTIME_DIR", self.dir.join("run"))
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.dir.join("bin").display()),
            )
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    /// The run log lines of the root case of the cycle that ran, and the observations of its store.
    fn store(&self) -> (Vec<Value>, Vec<Value>) {
        let store = self.dir.join("scratch/1/store");
        let log = std::fs::read_dir(&store)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("runlog-case-"))
            })
            .expect("a run log in the store");
        let lines = std::fs::read_to_string(log)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        let observations: Value = serde_json::from_str(
            &std::fs::read_to_string(store.join("observations.json")).unwrap(),
        )
        .unwrap();
        (lines, observations.as_array().cloned().unwrap_or_default())
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn text(o: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

/// The line `ActionFinished` of `bug.dissect`, the only action the cycle takes.
fn finished(lines: &[Value]) -> &Value {
    let mut all = lines
        .iter()
        .filter(|l| l["kind"] == "ActionFinished" && l["action"] == "bug.dissect");
    let one = all.next().expect("bug.dissect was taken");
    assert!(
        all.next().is_none(),
        "bug.dissect was taken once: {lines:?}"
    );
    one
}

/// The runs of the bound program of `bug.dissect` the pinned Loom kept, each with its class.
fn runs(observations: &[Value]) -> Vec<(i64, Option<String>)> {
    observations
        .iter()
        .filter(|o| o["source"] == "loom/bound-program" && o["payload"]["action"] == "bug.dissect")
        .map(|o| {
            (
                o["payload"]["exit"].as_i64().unwrap(),
                o["payload"]["class"].as_str().map(str::to_owned),
            )
        })
        .collect()
}

#[test]
fn a_bound_program_that_exits_75_ends_the_run_program_error_and_records_nothing_and_no_run_is_counted(
) {
    let rig = Rig::new("error", 75, NOT_CALLED);
    let o = rig.run("exits-error");
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    assert!(
        text(&o).contains("exits-error: as expected"),
        "{}",
        text(&o)
    );
    let (lines, observations) = rig.store();
    let line = finished(&lines);
    let refusal = line["refusal"].as_str().unwrap_or_default();
    assert!(
        refusal.starts_with("ProgramError"),
        "the run ends ProgramError: {line}"
    );
    assert_eq!(
        line["evidence"].as_array().map(Vec::len),
        Some(0),
        "no result for the node: {line}"
    );
    assert!(
        line["exit"].is_null(),
        "no exit is held against the node: {line}"
    );
    assert!(
        !lines
            .iter()
            .any(|l| l.to_string().contains("dissection_written")),
        "no result of the node is in the log: {lines:?}"
    );
    assert_eq!(
        runs(&observations),
        vec![(75, Some("error".to_string()))],
        "{observations:?}"
    );
    let counted = runs(&observations)
        .iter()
        .filter(|(_, class)| class.is_none())
        .count();
    assert_eq!(counted, 0, "max_runs (3 for bug.dissect) is not reduced: the runs without a class are the counted ones");
}

#[test]
fn a_bound_program_that_exits_the_not_called_code_records_nothing_and_no_run_is_counted() {
    let code = NOT_CALLED;
    let rig = Rig::new("not-called", 75, code);
    let o = rig.run("exits-not-called");
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    assert!(
        text(&o).contains("exits-not-called: as expected"),
        "{}",
        text(&o)
    );
    let (lines, observations) = rig.store();
    let line = finished(&lines);
    assert!(
        line["refusal"].is_null(),
        "the step is performed, not refused: {line}"
    );
    assert_eq!(line["exit"].as_i64(), Some(code), "{line}");
    assert_eq!(
        line["evidence"].as_array().map(Vec::len),
        Some(0),
        "no result for the node: {line}"
    );
    assert!(
        !lines
            .iter()
            .any(|l| l.to_string().contains("dissection_written")),
        "no result of the node is in the log: {lines:?}"
    );
    assert_eq!(
        runs(&observations),
        vec![(code, Some("not_called".to_string()))],
        "{observations:?}"
    );
    let counted = runs(&observations)
        .iter()
        .filter(|(_, class)| class.is_none())
        .count();
    assert_eq!(counted, 0, "max_runs (3 for bug.dissect) is not reduced: the runs without a class are the counted ones");
}

/// `spec.gate` of the epic runs before the spec exists and has to fail, not end the run: its
/// `exits.error` lists 126, which `sdc-tool.sh` used to answer when it could not keep the gate's log
/// of a tree that is not there.
#[test]
fn the_gate_on_a_missing_spec_fails_and_is_no_program_error() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("exit-classes-gate-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".sdc")).unwrap();
    let o = Command::new(repo().join("bindings/tools/sdc-tool.sh"))
        .args(["ess-gate", "--action", "spec.gate"])
        .current_dir(&dir)
        .env("LOOM_CASE", "root.billing")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
}
