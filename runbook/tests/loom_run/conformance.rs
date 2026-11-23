//! `runbook run --conformance observe`: after a run that ended closed, the product in the run's
//! workspace is held to the conformance suite of the run's own specification and `oracle.json` is
//! written into the run directory (`docs/conformance.md`); it decides nothing. The runbook is the
//! small `sh` fixture of the engine tests, whose writer here leaves a `spec/` and a `published/` in
//! the workspace; ess is a stand-in `bindings/tools/sdc-tool.sh` the test writes (the observation
//! asks that tool for ess, never an `ess` of its own), and the model adapter is `stub`, so no table
//! is written and no model, no product and no network is started.
//!
//! Cycle entry held here: `conformance-default-off` of `cycles/suite.yaml`.

use super::core::BIN;
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// What the writer leaves: an invented specification and an invented delivered tree.
const WRITER: &str = "#!/bin/sh\nmkdir -p spec published\necho 'name: tea' > spec/system.yaml\necho 'a pot of tea' > published/README.md\necho draft >> draft.txt\n";

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
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("conformance-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("runs")).unwrap();
        copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &root.join("fx"));
        std::fs::write(root.join("idea.yaml"), "text: a note on tea\n").unwrap();
        let rig = Rig { root };
        rig.executable("fx/bindings/tools/draft-write.sh", WRITER);
        rig.answers(&[json!({"approve": true, "cost_micro_usd": 0})], 0);
        rig
    }

    fn executable(&self, rel: &str, text: &str) {
        let file = self.root.join(rel);
        std::fs::write(&file, text).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn answers(&self, answers: &[Value], max_call: u64) {
        let settings = json!({"in_process": true, "confinement": "local", "decider": {"kind": "scripted", "max_call_micro_usd": max_call, "answers": answers}});
        std::fs::write(self.root.join("settings.toml"), super::core::toml_text(&settings)).unwrap();
    }

    fn run_dir(&self, name: &str) -> PathBuf {
        self.root.join("runs").join(name)
    }

    fn args(&self, name: &str, cap: u64, extra: &[&str]) -> Vec<String> {
        let r = |p: &str| self.root.join(p).display().to_string();
        let mut a: Vec<String> = ["run", "sdc", "--run", name, "--runs-dir"].iter().map(|s| (*s).to_owned()).collect();
        a.extend(
            [
                &r("runs"), "--bindings", &r("fx/bindings"), "--protocols", &r("fx/protocol"), "--root-protocol", "engine-demo@1", "--case", "ROOT-1", "--intake",
                &r("idea.yaml"), "--confinement", "local", "--settings", &r("settings.toml"), "--spend-ledger", &r("ledger.json"), "--cap-micro-usd",
                &cap.to_string(), "--max-llm-calls", "100", "--max-wall-seconds", "120",
            ]
            .map(String::from),
        );
        a.extend(extra.iter().map(|s| (*s).to_owned()));
        a
    }

    fn run(&self, name: &str, cap: u64, extra: &[&str]) -> Output {
        Command::new(BIN)
            .args(self.args(name, cap, extra))
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("SDC_CONFORMANCE_ADAPTER", "stub")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    fn json(&self, name: &str, file: &str) -> Value {
        serde_json::from_str(&std::fs::read_to_string(self.run_dir(name).join(file)).unwrap_or_else(|e| panic!("{file}: {e}"))).unwrap()
    }
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// The stand-in tool: `ess --version`, `ess compile ... --out F` and `ess verify conform synthesize
/// ... --out F`, which writes a suite of one scenario.
fn stand_in_tool(rig: &Rig) {
    let text = "#!/bin/sh\n\
[ \"$1\" = ess ] || exit 64\n\
shift\n\
case \"$1\" in\n\
  --version) echo 'ess 0.0.0-stand-in' ;;\n\
  compile|verify)\n\
    kind=$1\n\
    while [ $# -gt 0 ]; do\n\
      if [ \"$1\" = --out ]; then out=$2; fi\n\
      shift\n\
    done\n\
    if [ \"$kind\" = compile ]; then echo '{}' > \"$out\"; else echo '{\"scenarios\": {\"tea/pour\": {\"steps\": []}}}' > \"$out\"; fi ;;\n\
  *) exit 64 ;;\n\
esac\n";
    rig.executable("fx/bindings/tools/sdc-tool.sh", text);
}

#[test]
fn conformance_default_off() {
    let rig = Rig::new("off");
    stand_in_tool(&rig);
    let o = rig.run("r1", 1_000_000, &[]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(rig.json("r1", "outcome.json")["outcome"], "ended");
    let dir = rig.run_dir("r1");
    assert!(dir.join("work/published/README.md").is_file(), "the writer left no product: the test would prove nothing");
    assert!(!dir.join("oracle.json").exists(), "oracle.json without the option");
    assert!(!dir.join("conformance").exists(), "an observer started without the option");
    assert!(!stderr(&o).contains("conformance"), "{}", stderr(&o));
    let o = rig.run("r2", 1_000_000, &["--conformance", "off"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(!rig.run_dir("r2").join("oracle.json").exists() && !rig.run_dir("r2").join("conformance").exists());
}

#[test]
fn observe_on_a_closed_run_writes_oracle_json_from_the_run_workspace_through_the_pinned_tool() {
    let rig = Rig::new("observe");
    stand_in_tool(&rig);
    let o = rig.run("r1", 1_000_000, &["--conformance", "observe"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(stderr(&o).contains("runbook run: conformance: "), "{}", stderr(&o));
    let outcome = rig.json("r1", "outcome.json");
    assert_eq!((outcome["outcome"].as_str(), outcome["reason"].as_str()), (Some("ended"), Some("delivered")), "{outcome}");
    let oracle = rig.json("r1", "oracle.json");
    assert_eq!(oracle["format"], "sdc.oracle/1", "{oracle}");
    assert_eq!(oracle["run"], "r1");
    let c = &oracle["conformance"];
    // the case is named ROOT-1: the workspace is the run's work/, not store/IDEA-1/work
    assert_eq!(c["product"]["tree"], "work/published", "{c}");
    assert_eq!(c["n"], 1, "the suite the stand-in synthesized holds one scenario: {c}");
    assert_eq!(c["ess"]["source"], "sdc-tool.sh", "{c}");
    // the adapter is `stub`: no model, so no table, and the figure is unknown, never a pass
    assert_eq!((c["result"].as_str(), c["cause"].as_str()), (Some("unknown"), Some("table_missing")), "{c}");
    assert_eq!(c["measured"], false);
    assert!(c["k"].is_null(), "{c}");
    assert!(rig.run_dir("r1").join("conformance/suite.json").is_file());
}

#[test]
fn a_failed_observation_is_written_into_oracle_json_and_the_run_exit_stays() {
    let rig = Rig::new("failed");
    // no sdc-tool.sh in the bindings: ess cannot be run
    let o = rig.run("r1", 1_000_000, &["--conformance", "observe"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(rig.json("r1", "outcome.json")["outcome"], "ended");
    let c = &rig.json("r1", "oracle.json")["conformance"];
    assert_eq!((c["result"].as_str(), c["cause"].as_str()), (Some("unknown"), Some("ess_missing")), "{c}");
    assert!(c["why"].as_str().unwrap_or("").contains("sdc-tool.sh"), "{c}");
    assert!(c["k"].is_null() && c["measured"] == false, "{c}");
}

#[test]
fn a_run_that_did_not_close_is_not_observed() {
    let rig = Rig::new("not-closed");
    stand_in_tool(&rig);
    // the one decision costs the whole cap: the run stops on the spend cap, exit 4
    rig.answers(&[json!({"approve": false, "reasons": "again", "cost_micro_usd": 400_000}), json!({"approve": true, "cost_micro_usd": 0})], 400_000);
    let o = rig.run("r1", 400_000, &["--conformance", "observe"]);
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    assert!(stderr(&o).contains("conformance: not observed (the run did not close"), "{}", stderr(&o));
    assert!(!rig.run_dir("r1").join("oracle.json").exists() && !rig.run_dir("r1").join("conformance").exists());
}

#[test]
fn the_tables_setting_is_checked_and_never_changes_the_exit() {
    let rig = Rig::new("tables");
    stand_in_tool(&rig);
    let o = Command::new(BIN)
        .args(rig.args("r1", 1_000_000, &["--conformance", "observe"]))
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("SDC_CONFORMANCE_TABLES", "12")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(stderr(&o).contains("not observed (SDC_CONFORMANCE_TABLES is a number from 1 to 9)"), "{}", stderr(&o));
    assert!(!rig.run_dir("r1").join("oracle.json").exists());
}
