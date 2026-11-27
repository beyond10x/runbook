//! The tests of the loop, the ledger and the caps, against a fake `b10x-loom` (`fake.rs`).

use super::fake::{decision_json, tree_run_json, Fake};
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

pub const BIN: &str = env!("CARGO_BIN_EXE_runbook");

/// A bindings file with one model node: block style, one per-call budget of 1 USD.
const BINDINGS: &str = "format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: node.write\n    executor:\n      kind: program\n      argv:\n        - sdc-node\n        - node.write\n        - --max-budget-usd\n        - \"1\"\n";

/// A shell snippet for a `tree_run` hook: one model call that costs `COST` USD, or less when the
/// budget the runner wrote into the bindings copy is lower (as the claude CLI honours its budget).
/// The budget is the line after the last `--max-budget-usd` of the copy, its digits and dots (1
/// when there is none). The snippet runs once per step of many cases: it starts one `awk` and at
/// most two `mkdir`, the record directory is counted and named with the shell's own words.
pub fn model_call_hook(cost: &str) -> String {
    format!(
        "w=work/.sdc/llm; [ -d \"$w\" ] || mkdir -p \"$w\"\n\
{RECORD_DIR}\
c=$(awk -v c={cost} 'take {{ b = $0; take = 0 }} /--max-budget-usd/ {{ b = $0; take = 1 }} END {{ gsub(/[^0-9.]/, \"\", b); if (b == \"\") b = 1; if (c+0 > b+0) c = b; printf \"%s\", c }}' bindings/sdc.yaml)\n\
[ -n \"$c\" ] || c=$(awk -v c={cost} 'BEGIN {{ if (c+0 > 1) c = 1; printf \"%s\", c }}')\n\
printf '{{\"action\":\"node.write\",\"record\":\"%s/%s\",\"cost_usd\":%s}}\\n' \"$PWD/$w\" \"$d\" \"$c\" >> \"$w/calls.jsonl\"\n"
    )
}

/// sh that makes the next record directory `NNN-node.write` under `$w` and names it `$d`: one more
/// than the directories `$w` holds, three digits.
pub const RECORD_DIR: &str = "i=1; for e in \"$w\"/*/; do [ -d \"$e\" ] && i=$((i + 1)); done\n\
d=$i; while [ ${#d} -lt 3 ]; do d=\"0$d\"; done; d=\"$d-node.write\"; mkdir -p \"$w/$d\"\n";

/// The TOML text of a settings value: the form `runbook run --settings` reads.
pub fn toml_text(v: &Value) -> String {
    toml::to_string(v).unwrap_or_else(|e| panic!("{e}: {v}"))
}

/// A settings value read back from TOML text.
pub fn from_toml(text: &str) -> Value {
    toml::from_str(text).unwrap_or_else(|e| panic!("{e}: {text}"))
}

pub struct World {
    pub root: PathBuf,
    pub fake: Fake,
}

impl World {
    pub fn new(tag: &str) -> World {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("loomrun-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for d in ["protocols", "bindings", "runs", "loginhome/.claude", "xdg"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        std::fs::set_permissions(root.join("xdg"), std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::write(root.join("protocols/sdc.protocol.yaml"), "format: protocol/1\nprotocol:\n  id: sdc\n  revision: 8\n").unwrap();
        std::fs::write(root.join("protocols/sdc-intake.protocol.yaml"), "format: protocol/1\nprotocol:\n  id: sdc.intake\n  revision: 9\n").unwrap();
        std::fs::write(root.join("bindings/sdc.yaml"), BINDINGS).unwrap();
        std::fs::write(root.join("idea.yaml"), "text: an idea\n").unwrap();
        std::fs::write(root.join("loginhome/.claude/.credentials.json"), "{\"token\":\"LOGIN-FILE-MARKER-4711\"}").unwrap();
        let fake = Fake::new(&root.join("fake"));
        fake.open_ok();
        let w = World { root, fake };
        w.settings(&json!({}));
        super::decider::install_routing_table(&w);
        w
    }

    pub fn login_path(&self) -> PathBuf {
        self.root.join("loginhome/.claude/.credentials.json")
    }

    /// Writes the settings file with the login file named and `extra` merged in.
    pub fn settings(&self, extra: &Value) {
        let mut v = json!({"login_file": self.login_path().display().to_string()});
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        std::fs::write(self.root.join("settings.toml"), super::core::toml_text(&v)).unwrap();
    }

    pub fn ledger(&self) -> PathBuf {
        self.root.join("ledger.json")
    }

    pub fn run_dir(&self, name: &str) -> PathBuf {
        self.root.join("runs").join(name)
    }

    /// The full option list for run `name`; `cap` micro USD.
    pub fn args(&self, name: &str, cap: u64) -> Vec<String> {
        let r = |p: &str| self.root.join(p).display().to_string();
        let mut a: Vec<String> = ["run", "sdc", "--run", name, "--runs-dir"].iter().map(|s| (*s).to_owned()).collect();
        a.extend([
            r("runs"),
            "--loom".into(),
            self.fake.bin().display().to_string(),
            "--loom-sha256".into(),
            self.fake.sha256(),
            "--bindings".into(),
            r("bindings"),
            "--protocols".into(),
            r("protocols"),
            "--root-protocol".into(),
            "sdc@8".into(),
            "--case".into(),
            "IDEA-1".into(),
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
            "600".into(),
        ]);
        a
    }

    /// The option list for the run named by `settings["run"]`, with the options that members of the
    /// settings repeat (`--cap-micro-usd`, `--max-llm-calls`, `--max-wall-seconds`, `--confinement`)
    /// taken from the settings themselves.
    pub fn args_for(&self, settings: &Value) -> Vec<String> {
        let mut a = self.args(settings["run"].as_str().unwrap(), settings["cap_micro_usd"].as_u64().unwrap());
        let mut set = |flag: &str, value: String| {
            let i = a.iter().position(|x| x == flag).unwrap();
            a[i + 1] = value;
        };
        set("--max-llm-calls", settings["max_llm_calls"].as_u64().unwrap().to_string());
        set("--max-wall-seconds", settings["max_wall_seconds"].as_u64().unwrap().to_string());
        set("--confinement", settings["confinement"].as_str().unwrap().to_owned());
        a
    }

    /// The PATH of the runner: the world's root first (where a test puts `sdc-reply`, as the launcher
    /// puts the build's `driver/target/debug` first), then the test's own.
    pub fn path(&self) -> String {
        format!("{}:{}", self.root.display(), std::env::var("PATH").unwrap_or_default())
    }

    pub fn command(&self, args: &[String]) -> Command {
        let mut c = Command::new(BIN);
        c.args(args)
            .env_clear().envs(crate::kept())
            .env("PATH", self.path())
            .env("HOME", "/marker-home-dir")
            .env("XDG_RUNTIME_DIR", self.root.join("xdg"))
            .env("SDC_SECRET_MARKER", "marker-secret-value")
            .env("ANTHROPIC_API_KEY", "marker")
            .stdin(Stdio::null());
        crate::isolate(&mut c);
        c
    }

    pub fn run(&self, args: &[String]) -> Output {
        self.command(args).output().unwrap()
    }

    pub fn outcome(&self, name: &str) -> Value {
        let text = std::fs::read_to_string(self.run_dir(name).join("outcome.json")).unwrap_or_else(|e| panic!("no outcome.json: {e}"));
        serde_json::from_str(&text).unwrap()
    }

    /// The scripted answer of a tree that takes one step, asks one decision, takes one step, ends.
    pub fn one_decision_tree(&self) {
        let f = &self.fake;
        f.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
        let d = decision_json("owner.approve", "ApproveAction", "person:owner");
        f.answer("tree_run", "2", &tree_run_json("AwaitingPerson", 0, None, &format!("[{d}]")), 3);
        f.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
        f.answer("tree_run", "3", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
        f.answer("tree_run", "4", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    }

    pub fn scripted_approve(&self) {
        self.settings(&json!({"decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [
            {"approve": true, "principal": "person:owner", "cost_micro_usd": 0, "model_calls": 0}]}}));
    }
}

pub fn spent(world: &World) -> u64 {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(world.ledger()).unwrap()).unwrap();
    v["spent_micro_usd"].as_u64().unwrap()
}

pub fn code(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

pub fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

pub fn grep_tree(dir: &Path, needle: &str, hits: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if std::fs::symlink_metadata(&p).unwrap().file_type().is_symlink() {
            continue;
        }
        if p.is_dir() {
            grep_tree(&p, needle, hits);
        } else if std::fs::read(&p).map(|b| b.windows(needle.len()).any(|w| w == needle.as_bytes())).unwrap_or(false) {
            hits.push(p);
        }
    }
}

// 1. sha256 of --loom

#[test]
fn wrong_loom_sha256_is_refused_before_anything_is_written() {
    let w = World::new("sha");
    let mut a = w.args("r1", 1_000_000);
    let i = a.iter().position(|x| x == "--loom-sha256").unwrap();
    a[i + 1] = "0".repeat(64);
    let o = w.run(&a);
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(stderr(&o).contains("sha256"), "{}", stderr(&o));
    assert!(!w.run_dir("r1").exists());
    assert!(!w.ledger().exists());
    assert_eq!(w.fake.calls_so_far(), 0);
}

// 2. the run directory

#[test]
fn an_existing_run_directory_is_refused() {
    let w = World::new("exists");
    std::fs::create_dir_all(w.run_dir("r1")).unwrap();
    std::fs::write(w.run_dir("r1/keep"), "mine").unwrap();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(stderr(&o).contains("exists"), "{}", stderr(&o));
    assert_eq!(std::fs::read_to_string(w.run_dir("r1/keep")).unwrap(), "mine");
    assert_eq!(w.fake.calls_so_far(), 0);
}

#[test]
fn a_full_run_writes_the_settings_and_no_login_path_or_content() {
    let w = World::new("login");
    w.one_decision_tree();
    w.scripted_approve();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let dir = w.run_dir("r1");
    let settings: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("settings.json")).unwrap()).unwrap();
    assert_eq!(settings["login_file"], "named");
    for needle in [w.login_path().display().to_string(), "LOGIN-FILE-MARKER-4711".to_owned(), "loginhome".to_owned()] {
        let mut hits = Vec::new();
        grep_tree(&dir, &needle, &mut hits);
        assert!(hits.is_empty(), "{needle} found in {hits:?}");
    }
    // No link either: the link the model nodes read lives in the private directory (login.rs).
    assert!(std::fs::symlink_metadata(dir.join("login")).is_err(), "a link to the login file is in the run directory");
}

// 3. the environment of the child

#[test]
fn the_child_environment_is_built_and_not_inherited() {
    let w = World::new("env");
    w.fake.record_env();
    w.one_decision_tree();
    w.scripted_approve();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let calls = w.fake.calls();
    assert!(calls.len() >= 8);
    let run_dir = w.run_dir("r1").display().to_string();
    for call in &calls {
        let keys: Vec<&str> = call.env().iter().map(|(k, _)| k.as_str()).collect();
        for k in &keys {
            assert!(["PATH", "HOME", "XDG_DATA_HOME", "RUNBOOK_NO_CONFIG", "PWD", "SHLVL", "_", "OLDPWD"].contains(k), "unexpected variable {k} in {keys:?}");
        }
        assert!(call.env().iter().any(|(k, v)| k == "RUNBOOK_NO_CONFIG" && v == "1"), "a program of the run is not told to read no runbook.toml: {keys:?}");
        let text = call.env().iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("\n");
        for marker in ["marker-secret-value", "ANTHROPIC_API_KEY", "/marker-home-dir"] {
            assert!(!text.contains(marker), "{marker} reached the child: {text}");
        }
        let get = |k: &str| call.env().iter().find(|(x, _)| x == k).map(|(_, v)| v.clone());
        assert_eq!(get("PATH"), Some(w.path()));
        assert!(get("HOME").unwrap().starts_with(&run_dir));
        assert!(get("XDG_DATA_HOME").unwrap().starts_with(&run_dir));
    }
}

// 4. --confinement

#[test]
fn confinement_is_required_and_has_no_default() {
    let w = World::new("conf-missing");
    let mut a = w.args("r1", 1_000_000);
    let i = a.iter().position(|x| x == "--confinement").unwrap();
    a.drain(i..i + 2);
    let o = w.run(&a);
    assert_eq!(code(&o), 64, "{}", stderr(&o));
    assert!(stderr(&o).contains("--confinement"), "{}", stderr(&o));
    assert!(!w.run_dir("r1").exists());
    assert_eq!(w.fake.calls_so_far(), 0);
}

#[test]
fn confinement_reaches_every_tree_run_unchanged() {
    for level in ["local", "substrate"] {
        let w = World::new(&format!("conf-{level}"));
        w.one_decision_tree();
        w.scripted_approve();
        let mut a = w.args("r1", 1_000_000);
        let i = a.iter().position(|x| x == "--confinement").unwrap();
        a[i + 1] = level.to_owned();
        let o = w.run(&a);
        assert_eq!(code(&o), 0, "{}", stderr(&o));
        let runs = w.fake.calls_of(&["tree", "run"]);
        assert_eq!(runs.len(), 4);
        for r in runs {
            let at = r.argv.iter().position(|x| x == "--confinement").expect("--confinement passed");
            assert_eq!(r.argv[at + 1], level);
            assert_eq!(r.argv.iter().filter(|x| *x == "--confinement").count(), 1);
        }
    }
}

// 5. the loop

#[test]
fn the_loop_runs_one_step_at_a_time_and_approves_what_loom_printed() {
    let w = World::new("loop");
    w.one_decision_tree();
    w.scripted_approve();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let runs = w.fake.calls_of(&["tree", "run"]);
    assert_eq!(runs.len(), 4);
    for r in &runs {
        let at = r.argv.iter().position(|x| x == "--max-steps").unwrap();
        assert_eq!(r.argv[at + 1], "1");
    }
    let approves = w.fake.calls_of(&["tree", "approve"]);
    assert_eq!(approves.len(), 1);
    let store = w.run_dir("r1/store").display().to_string();
    let expect: Vec<String> =
        ["tree", "approve", "--store", &store, "--case", "root", "--capability", "owner.approve", "--principal", "person:owner", "--action", "owner.approve"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
    assert_eq!(approves[0].argv, expect);
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "ended");
    assert_eq!(outcome["reason"], "operating");
    assert_eq!(outcome["exit_code"], 0);
}

#[test]
fn the_deciders_principal_replaces_loom_s() {
    let w = World::new("principal");
    w.one_decision_tree();
    w.settings(&json!({"decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [
        {"approve": true, "principal": "decider:model", "cost_micro_usd": 0, "model_calls": 0}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let a = &w.fake.calls_of(&["tree", "approve"])[0].argv;
    assert!(!a.iter().any(|x| x == "--deny"), "{a:?}");
    let at = a.iter().position(|x| x == "--principal").unwrap();
    assert_eq!(a[at + 1], "decider:model");
}

#[test]
fn without_a_decider_the_run_stops_at_the_first_decision() {
    let w = World::new("nodecider");
    w.one_decision_tree();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 8, "{}", stderr(&o));
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty());
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "stopped: decider");
    assert!(outcome["reason"].as_str().unwrap().contains("no decider configured"));
}

/// A decision with the four stable members only.
fn bare_decision(kind: &str, command: &[&str]) -> String {
    let words: Vec<String> = command.iter().map(|w| format!("\"{w}\"")).collect();
    format!("{{\"case\":\"root\",\"kind\":\"{kind}\",\"decides\":\"something of case root\",\"command\":[{}]}}", words.join(","))
}

#[test]
fn the_printed_command_is_run_as_data_whatever_it_is() {
    let w = World::new("asdata");
    let d = bare_decision("Review", &["b10x-loom", "decisions", "decide", "--tree", "@STORE@", "--request", "r-17"]);
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 0, None, &format!("[{d}]")), 3);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 0, Some("operating"), "[]"), 0);
    w.fake.answer("decisions_decide", "default", "{}\n", 0);
    w.settings(&json!({"decider": {"kind": "scripted", "answers": [{"approve": true, "cost_micro_usd": 0}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let calls = w.fake.calls_of(&["decisions", "decide"]);
    assert_eq!(calls.len(), 1);
    let store = w.run_dir("r1/store").display().to_string();
    assert_eq!(calls[0].argv, ["decisions", "decide", "--tree", store.as_str(), "--request", "r-17"]);
}

#[test]
fn an_unknown_decision_kind_goes_to_the_decider_and_a_refusal_names_it() {
    let w = World::new("unknownkind");
    let d = bare_decision("Frobnicate", &["b10x-loom", "tree", "frobnicate", "--store", "@STORE@"]);
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 0, None, &format!("[{d}]")), 3);
    w.settings(&json!({"decider": {"kind": "scripted", "answers": [{"cannot_decide": "not mine"}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 8, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "stopped: decider");
    let reason = outcome["reason"].as_str().unwrap();
    assert!(reason.contains("not mine") && reason.contains("Frobnicate"), "{reason}");
}

#[test]
fn a_printed_command_that_fails_ends_the_run_with_its_exit_code_and_stderr() {
    let w = World::new("cmdfails");
    let d = decision_json("owner.approve", "ApproveAction", "person:owner");
    w.fake.answer("tree_run", "default", &tree_run_json("AwaitingPerson", 0, None, &format!("[{d}]")), 3);
    w.fake.answer("tree_approve", "default", "", 2);
    w.fake.stderr("tree_approve", "default", "error: the following required arguments were not provided: --capability\n");
    w.scripted_approve();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "error");
    assert_eq!(outcome["loom_exit_code"], 2);
    assert!(outcome["stderr_tail"].as_str().unwrap().contains("--capability"));
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 1, "no retry");
    assert_eq!(w.fake.calls_of(&["tree", "approve"]).len(), 1, "no repair of the argv");
}

#[test]
fn a_denial_is_asked_once_and_the_run_ends_denied_when_the_same_decision_returns() {
    let w = World::new("denied");
    std::fs::write(w.root.join("bindings/sdc.yaml"), "format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: owner.approve\n    executor: {kind: person, principal: person:owner}\n    evidence:\n      - {kind: draft_review, subject: draft, producer: person:owner, from: effect, results: {performed: approved, rejected: rejected}}\n").unwrap();
    let d = decision_json("owner.approve", "ApproveAction", "person:owner");
    w.fake.answer("tree_run", "default", &tree_run_json("AwaitingPerson", 0, None, &format!("[{d}]")), 3);
    w.fake.answer("tree_approve", "default", "{\"decision\":\"Deny\"}\n", 0);
    w.settings(&json!({"decider": {"kind": "scripted", "answers": [{"approve": false, "cost_micro_usd": 0}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 9, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "denied");
    assert_eq!(outcome["reason"], "owner.approve");
    assert_eq!(w.fake.calls_of(&["tree", "approve"]).len(), 1, "the decider was asked once");
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 2);
}

#[test]
fn a_stuck_tree_ends_the_run_with_its_own_exit_code() {
    let w = World::new("stuck");
    w.fake.answer("tree_run", "1", &tree_run_json("Stuck", 0, None, "[]"), 4);
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 3, "{}", stderr(&o));
    assert_eq!(w.outcome("r1")["outcome"], "stuck");
}

// 6. caps

#[test]
fn the_llm_call_cap_stops_the_run_before_the_next_step() {
    let w = World::new("llmcap");
    w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "default", &model_call_hook("0.1"));
    let mut a = w.args("r1", 10_000_000);
    let i = a.iter().position(|x| x == "--max-llm-calls").unwrap();
    a[i + 1] = "2".to_owned();
    let o = w.run(&a);
    assert_eq!(code(&o), 6, "{}", stderr(&o));
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 2);
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "stopped: llm calls");
    assert_eq!(outcome["llm_calls"], 2);
    assert_eq!(spent(&w), 200_000);
}

#[test]
fn the_spend_cap_gives_each_call_at_most_what_remains_and_then_stops() {
    let w = World::new("spendcap");
    w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "default", &format!("{}cp bindings/sdc.yaml \"$D/bind.$n\"\n", model_call_hook("0.6")));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 2, "the third call must not start");
    assert_eq!(spent(&w), 1_000_000);
    let bind = |n: u64| std::fs::read_to_string(w.fake.dir.join(format!("bind.{n}"))).unwrap();
    let budgets: Vec<String> = w.fake.calls().iter().enumerate().filter(|(_, c)| c.argv.starts_with(&["tree".into(), "run".into()])).map(|(i, _)| bind(i as u64 + 1)).collect();
    assert!(budgets[0].contains("'1'") || budgets[0].contains("\"1\""), "first call: {}", budgets[0]);
    assert!(budgets[1].contains("'0.4'"), "second call gets what remains: {}", budgets[1]);
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "stopped: spend cap");
    assert_eq!(outcome["exit_code"], 4);
    let committed = std::fs::read_to_string(w.root.join("bindings/sdc.yaml")).unwrap();
    assert_eq!(committed, BINDINGS, "the source bindings are never edited");
}

#[test]
fn two_runs_sharing_one_ledger_never_exceed_the_cap_together() {
    let a = World::new("shared-a");
    let b = World::new("shared-b");
    for w in [&a, &b] {
        w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
        w.fake.hook("tree_run", "default", &format!("sleep 0.2\n{}", model_call_hook("0.6")));
    }
    let ledger = a.ledger();
    let mut args_b = b.args("r2", 2_000_000);
    let i = args_b.iter().position(|x| x == "--spend-ledger").unwrap();
    args_b[i + 1] = ledger.display().to_string();
    let args_a = a.args("r1", 2_000_000);
    let ca = a.command(&args_a).output_thread();
    let cb = b.command(&args_b).output_thread();
    let (oa, ob) = (ca.join().unwrap(), cb.join().unwrap());
    assert_eq!(code(&oa), 4, "{}", stderr(&oa));
    assert_eq!(code(&ob), 4, "{}", stderr(&ob));
    let total = spent(&a);
    assert!(total <= 2_000_000, "spent {total}");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&ledger).unwrap()).unwrap();
    assert!(v["reserved"].as_object().unwrap().is_empty(), "no reservation is left: {v}");
    let calls = a.fake.calls_of(&["tree", "run"]).len() + b.fake.calls_of(&["tree", "run"]).len();
    assert!(calls >= 3, "calls {calls}");
    let booked: u64 = v["entries"].as_array().unwrap().iter().map(|e| e["micro_usd"].as_u64().unwrap()).sum();
    assert_eq!(booked, total);
}

trait OutputThread {
    fn output_thread(self) -> std::thread::JoinHandle<Output>;
}

impl OutputThread for Command {
    fn output_thread(mut self) -> std::thread::JoinHandle<Output> {
        std::thread::spawn(move || self.output().unwrap())
    }
}

// 7. fail closed

fn assert_failed_closed(w: &World, o: &Output, reason: &str, tree_runs: usize) {
    assert_eq!(code(o), 1, "{}", stderr(o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "error");
    let text = outcome["reason"].as_str().unwrap();
    assert!(text.contains(reason), "reason {text:?} lacks {reason:?}");
    assert_eq!(outcome["exit_code"], 1);
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), tree_runs, "a model call was started after the failure");
}

#[test]
fn an_unreadable_ledger_stops_the_run_before_the_first_call() {
    let w = World::new("ledger-unreadable");
    std::fs::create_dir_all(w.ledger()).unwrap();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_failed_closed(&w, &o, "spend ledger", 0);
}

#[test]
fn a_ledger_that_is_not_valid_stops_the_run_before_the_first_call() {
    let w = World::new("ledger-invalid");
    std::fs::write(w.ledger(), "this is not a ledger").unwrap();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_failed_closed(&w, &o, "spend ledger", 0);
    assert_eq!(std::fs::read_to_string(w.ledger()).unwrap(), "this is not a ledger", "an invalid ledger is not overwritten");
}

#[test]
fn a_decider_answer_without_a_cost_stops_the_run() {
    let w = World::new("nocost");
    w.one_decision_tree();
    w.settings(&json!({"decider": {"kind": "scripted", "max_call_micro_usd": 500000, "answers": [{"approve": true, "principal": "person:owner"}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_failed_closed(&w, &o, "without a cost", 2);
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty());
    assert_eq!(spent(&w), 500_000, "an unknown cost books what the call could have spent");
}

#[test]
fn an_exit_code_of_loom_the_runner_does_not_know_stops_the_run() {
    let w = World::new("exit7");
    w.fake.answer("tree_run", "1", "", 7);
    w.fake.stderr("tree_run", "1", "b10x-loom: something new\n");
    let o = w.run(&w.args("r1", 1_000_000));
    assert_failed_closed(&w, &o, "does not know", 1);
    let outcome = w.outcome("r1");
    assert_eq!(outcome["loom_exit_code"], 7);
    assert!(outcome["stderr_tail"].as_str().unwrap().contains("something new"));
}

#[test]
fn tree_run_output_that_is_not_json_stops_the_run() {
    let w = World::new("notjson");
    w.fake.answer("tree_run", "default", "this is not json\n", 5);
    let o = w.run(&w.args("r1", 1_000_000));
    assert_failed_closed(&w, &o, "no JSON", 1);
}

#[test]
fn a_model_step_without_a_cost_record_stops_the_run() {
    let w = World::new("nocostrecord");
    w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "default", "mkdir -p work/.sdc/llm/001-node.write\nprintf '{}' > work/.sdc/llm/001-node.write/request.json\n");
    let o = w.run(&w.args("r1", 1_000_000));
    assert_failed_closed(&w, &o, "no cost record", 1);
    assert_eq!(spent(&w), 1_000_000, "an unknown cost books the call's whole budget");
}

#[test]
fn a_cost_record_with_a_null_cost_stops_the_run() {
    let w = World::new("nullcost");
    w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook(
        "tree_run",
        "default",
        "mkdir -p work/.sdc/llm/001-node.write\nprintf '{\"record\":\"x/001-node.write\",\"cost_usd\":null}\\n' >> work/.sdc/llm/calls.jsonl\n",
    );
    let o = w.run(&w.args("r1", 1_000_000));
    assert_failed_closed(&w, &o, "cost_usd", 1);
}

// 8. the end

#[test]
fn the_end_stores_tree_status_and_tree_verify() {
    let w = World::new("end");
    w.one_decision_tree();
    w.scripted_approve();
    w.fake.answer("tree_status", "default", "{\"cases\":[{\"case\":\"IDEA-1\"}],\"format\":\"loom.tree-status/1\"}\n", 0);
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let dir = w.run_dir("r1");
    let status = std::fs::read_to_string(dir.join("tree-status.json")).unwrap();
    assert_eq!(status, "{\"cases\":[{\"case\":\"IDEA-1\"}],\"format\":\"loom.tree-status/1\"}\n");
    let verify: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("tree-verify.json")).unwrap()).unwrap();
    assert_eq!(verify["exit"], 0);
    assert_eq!(verify["stdout"], "{\"cases\":1}\n");
}

#[test]
fn an_error_before_the_tree_is_opened_keeps_its_own_reason_and_runs_no_verify() {
    let w = World::new("early");
    w.one_decision_tree();
    w.scripted_approve();
    std::fs::remove_file(w.root.join("bindings/sdc.yaml")).unwrap();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "error");
    assert!(outcome["reason"].as_str().unwrap().contains("the root bindings file is missing"), "{outcome}");
    assert!(!w.run_dir("r1").join("tree-verify.json").exists());
}

#[test]
fn a_failing_tree_verify_makes_the_outcome_an_error() {
    let w = World::new("verify");
    w.one_decision_tree();
    w.scripted_approve();
    w.fake.answer("tree_verify", "default", "", 3);
    w.fake.stderr("tree_verify", "default", "case IDEA-1 line 4\n");
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "error");
    assert!(outcome["reason"].as_str().unwrap().contains("tree verify exited 3"));
    let verify: Value = serde_json::from_str(&std::fs::read_to_string(w.run_dir("r1").join("tree-verify.json")).unwrap()).unwrap();
    assert_eq!(verify["exit"], 3);
    assert!(verify["stderr"].as_str().unwrap().contains("line 4"));
}

// 9. a signal

#[test]
fn sigterm_ends_the_child_and_writes_stopped_signal() {
    let w = World::new("signal");
    w.fake.answer("tree_run", "1", "", 0);
    w.fake.hook("tree_run", "1", "echo $$ > \"$D/pid\"\nsleep 30\n");
    let mut child = w.command(&w.args("r1", 1_000_000)).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let started = std::time::Instant::now();
    while !w.fake.dir.join("pid").exists() {
        assert!(started.elapsed().as_secs() < 20, "the fake tree run did not start");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let pid = std::fs::read_to_string(w.fake.dir.join("pid")).unwrap().trim().to_owned();
    assert!(Command::new("kill").args(["-TERM", &child.id().to_string()]).status().unwrap().success());
    let end = std::time::Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        assert!(end.elapsed().as_secs() < 8, "the runner did not end after SIGTERM");
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(7));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "stopped: signal");
    assert!(!Command::new("kill").args(["-0", &pid]).stderr(Stdio::null()).status().unwrap().success(), "the fake child is still alive");
    assert_eq!(spent(&w), 0, "the reservation was released or booked at its cost");
}

/// Adversary on story:the-runbook-test-step-stays-under-its-budget (`child.rs` now waits on a thread of its own): a
/// `b10x-loom` child that ignores SIGTERM, and its own child with it, is ended by the SIGKILL 3 s later, and the runner
/// ends with `stopped: signal`. No other case reaches that SIGKILL: without it the runner waits for such a child forever.
#[test]
fn adversary_a_child_that_ignores_sigterm_is_killed_after_three_seconds_and_the_run_stops_on_the_signal() {
    let w = World::new("adv-sigterm-ignored");
    w.fake.answer("tree_run", "1", "", 0);
    w.fake.hook("tree_run", "1", "trap '' TERM\necho $$ > \"$D/pid\"\nsleep 30\n");
    let mut child = w.command(&w.args("r1", 1_000_000)).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let started = std::time::Instant::now();
    while !w.fake.dir.join("pid").exists() {
        assert!(started.elapsed().as_secs() < 20, "the fake tree run did not start");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let pid = std::fs::read_to_string(w.fake.dir.join("pid")).unwrap().trim().to_owned();
    assert!(Command::new("kill").args(["-TERM", &child.id().to_string()]).status().unwrap().success());
    let end = std::time::Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        if end.elapsed().as_secs() >= 10 {
            let _ = child.kill();
            let _ = child.wait();
            let _ = Command::new("kill").args(["-KILL", &pid]).status();
            panic!("the runner did not end within 10 s of SIGTERM while its child ignored SIGTERM");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let took = end.elapsed();
    assert!(took >= std::time::Duration::from_millis(2500), "the runner ended {took:?} after SIGTERM: its child ignored SIGTERM, so it cannot have been ended before the SIGKILL 3 s later");
    assert_eq!(status.code(), Some(7));
    assert_eq!(w.outcome("r1")["outcome"], "stopped: signal");
    assert!(!Command::new("kill").args(["-0", &pid]).stderr(Stdio::null()).status().unwrap().success(), "the fake child is still alive");
}

// the cost of a step

#[test]
fn the_cost_of_a_step_is_read_from_the_calls_file_into_the_ledger() {
    let w = World::new("cost");
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.hook("tree_run", "1", "w=work/.sdc/llm; mkdir -p $w/001-a.b $w/002-a.c\nprintf '%s\\n' '{\"record\":\"/x/001-a.b\",\"cost_usd\":0.125}' '{\"record\":\"/x/002-a.c\",\"cost_usd\":0.25}' > $w/calls.jsonl\n");
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 0, Some("operating"), "[]"), 0);
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(spent(&w), 375_000);
    let outcome = w.outcome("r1");
    assert_eq!(outcome["llm_calls"], 2);
    assert_eq!(outcome["spent_micro_usd"], 375_000);
    let log = std::fs::read_to_string(w.run_dir("r1/events.jsonl")).unwrap();
    assert_eq!(log.lines().filter(|l| l.contains("\"costed_call\"")).count(), 2);
}

/// Output of the real `b10x-loom` (sha256 d80830ea...), paths replaced by `/runs/example`:
/// `real/tree-run-exit3.json` is the exit-3 answer of the stub-cycle tree, `real/tree-run-exit5.json`
/// the exit-5 answer of `tree run --max-steps 1`.
#[test]
fn real_loom_output_is_read_and_its_printed_command_runs_as_printed() {
    let w = World::new("realfixtures");
    let exit3 = include_str!("real/tree-run-exit3.json");
    let exit5 = include_str!("real/tree-run-exit5.json");
    let printed: Value = serde_json::from_str(exit3).unwrap();
    let words: Vec<String> = printed["decisions"][0]["command"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()).collect();
    w.fake.answer("tree_run", "1", exit5, 5);
    w.fake.answer("tree_run", "2", exit3, 3);
    w.fake.answer("tree_run", "3", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.fake.answer("tree_approve", "default", "{}\n", 0);
    w.settings(&json!({"decider": {"kind": "scripted", "answers": [{"approve": true, "cost_micro_usd": 0}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let calls = w.fake.calls_of(&["tree", "approve"]);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].argv, words[1..].to_vec(), "the printed command, word for word");
}

// the requests of unit R3

#[test]
fn run_started_settings_carry_what_the_command_was_given_and_not_the_binary_path() {
    let w = World::new("runsettings");
    w.one_decision_tree();
    w.scripted_approve();
    let o = w.run(&w.args("r1", 1_234_567));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let text = std::fs::read_to_string(w.run_dir("r1/settings.json")).unwrap();
    let s: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(s["loom"]["sha256"], w.fake.sha256());
    assert_eq!(s["loom"]["confinement"], "local");
    assert_eq!(s["loom"]["cap_micro_usd"], 1_234_567);
    assert_eq!(s["max_llm_calls"], 100);
    assert_eq!(s["max_wall_seconds"], 600);
    assert_eq!(s["case"], "IDEA-1");
    assert_eq!(s["root_protocol"], "sdc@8");
    assert_eq!(s["login_file"], "named");
    assert!(!text.contains(&w.fake.bin().display().to_string()), "the path of the binary is not recorded");
    assert!(!text.contains(&w.root.display().to_string()), "no path of the runs directory: {text}");
}

#[test]
fn the_protocol_files_are_copied_into_the_run_directory_before_tree_open() {
    let w = World::new("protocopy");
    w.one_decision_tree();
    w.scripted_approve();
    w.fake.hook("tree_open", "default", "mkdir -p \"$store\" && echo {} > \"$store/tree.json\"\nls protocols > \"$D/protocols-at-open\"\n");
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    for name in ["sdc.protocol.yaml", "sdc-intake.protocol.yaml"] {
        let copy = std::fs::read_to_string(w.run_dir("r1/protocols").join(name)).unwrap();
        assert_eq!(copy, std::fs::read_to_string(w.root.join("protocols").join(name)).unwrap());
    }
    let at_open = std::fs::read_to_string(w.fake.dir.join("protocols-at-open")).unwrap();
    assert!(at_open.contains("sdc.protocol.yaml") && at_open.contains("sdc-intake.protocol.yaml"), "{at_open}");
}

#[test]
fn the_files_the_runner_writes_name_no_path_of_the_runs_directory() {
    let w = World::new("relative");
    let d = decision_json("owner.approve", "ApproveAction", "person:owner");
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 0, None, &format!("[{d}]")), 3);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.fake.answer("tree_approve", "default", "{}\n", 0);
    let status = "{\"cases\":[{\"case\":\"IDEA-1\",\"case_file\":\"@STORE@/case-1.json\",\"run_log\":\"@STORE@/runlog-1.jsonl\"}],\"format\":\"loom.tree-status/1\",\"store\":\"@STORE@\",\"workspace\":\"@STORE@/../work\"}\n";
    w.fake.answer("tree_status", "default", status, 0);
    w.fake.answer("tree_verify", "default", "{\"cases\":1,\"store\":\"@STORE@\"}\n", 0);
    w.scripted_approve();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let prefix = w.root.display().to_string();
    let mut hits = Vec::new();
    for file in ["tree-status.json", "tree-verify.json", "events.jsonl", "outcome.json", "settings.json"] {
        let text = std::fs::read_to_string(w.run_dir("r1").join(file)).unwrap();
        if text.contains(&prefix) {
            hits.push(file);
        }
    }
    assert!(hits.is_empty(), "a path of the runs directory is in {hits:?}");
    let s: Value = serde_json::from_str(&std::fs::read_to_string(w.run_dir("r1/tree-status.json")).unwrap()).unwrap();
    assert_eq!(s["store"], "store");
    assert_eq!(s["cases"][0]["case_file"], "store/case-1.json");
    assert_eq!(s["workspace"], "store/../work");
}

#[test]
fn the_status_is_refreshed_after_every_step_and_never_left_half_written() {
    let w = World::new("statusrefresh");
    w.one_decision_tree();
    w.scripted_approve();
    w.fake.hook("tree_status", "default", "[ -e tree-status.tmp ] && echo leftover >> \"$D/tmp-seen\"\n");
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 4);
    assert_eq!(w.fake.calls_of(&["tree", "status"]).len(), 5, "one after each of the 4 steps and one at the end");
    assert!(!w.fake.dir.join("tmp-seen").exists(), "a temporary file was left behind");
    assert!(!w.run_dir("r1/tree-status.tmp").exists());
}

#[test]
fn a_status_the_binary_refuses_ends_the_run_with_its_exit_code_and_stderr() {
    let w = World::new("statusrefused");
    w.fake.answer("tree_run", "default", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.answer("tree_status", "default", "", 1);
    w.fake.stderr("tree_status", "default", "b10x-loom: tree refused: StoreUnreadable: case IDEA-1: GovernorUnavailable\n");
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    let outcome = w.outcome("r1");
    assert_eq!(outcome["outcome"], "error");
    assert_eq!(outcome["loom_exit_code"], 1);
    assert!(outcome["stderr_tail"].as_str().unwrap().contains("GovernorUnavailable"));
    assert!(outcome["reason"].as_str().unwrap().contains("protocol catalog"));
    assert_eq!(w.fake.calls_of(&["tree", "run"]).len(), 1, "no step after the refused status");
}

// 10. the step in flight, under the child

#[test]
fn the_child_path_writes_its_start_event_before_the_step_ends_and_names_no_action() {
    let w = World::new("in-flight");
    w.fake.answer("tree_run", "1", &tree_run_json("Terminal", 1, None, "[]"), 0);
    w.fake.hook("tree_run", "1", "while [ ! -f \"$D/go\" ]; do sleep 0.05; done\n");
    let mut child = w.command(&w.args("r1", 1_000_000)).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let events = w.run_dir("r1").join("events.jsonl");
    let limit = std::time::Instant::now() + std::time::Duration::from_secs(50);
    let started = loop {
        let text = std::fs::read_to_string(&events).unwrap_or_default();
        if let Some(line) = text.lines().find(|l| l.contains("\"action_started\"")) {
            break serde_json::from_str::<Value>(line).unwrap();
        }
        assert!(std::time::Instant::now() < limit, "no action_started event while the fake tree run waits");
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert!(!std::fs::read_to_string(&events).unwrap().contains("\"step_made\""), "the step was made before the fake tree run ended");
    assert_eq!((started["source"].as_str(), started["case"].is_null(), started["action"].is_null(), started["try"].is_null()), (Some("step_call"), true, true, true), "{started}");
    assert!(started["at"].as_u64().is_some(), "{started}");
    let record: Value = serde_json::from_str(&std::fs::read_to_string(w.run_dir("r1").join("runner.json")).unwrap()).unwrap();
    assert_eq!(record["pid"].as_u64(), Some(u64::from(child.id())), "{record}");
    assert!(record["closed_at"].is_null(), "{record}");
    std::fs::write(w.fake.dir.join("go"), "").unwrap();
    let status = child.wait().unwrap();
    assert!(status.code().is_some(), "{status:?}");
    let record: Value = serde_json::from_str(&std::fs::read_to_string(w.run_dir("r1").join("runner.json")).unwrap()).unwrap();
    assert!(record["closed_at"].as_u64().is_some(), "{record}");
}

/// Adversary on story:the-runbook-test-step-stays-under-its-budget: the fake's answers are sourced `printf` lines now.
/// Every text a test may hand to `Fake::answer` or `Fake::stderr` comes back byte for byte, `@STORE@` replaced by the
/// `--store` of the call, whatever quotes, backslashes, `%`, a leading `-` or a missing last newline the text holds,
/// and whatever the store's path holds.
#[test]
fn adversary_the_fake_prints_every_answer_text_as_written() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("loomrun-adv-printf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let fake = Fake::new(&dir);
    let texts = [
        "it's", "'", "''", "a'b'c", "\\", "a\\nb\\t", "100% %s %d %% %b", "-n", "--", "-e x", "'@STORE@'", "\"@STORE@\"",
        "@STORE@@STORE@", "@STORE@", "x'@STORE@'y\n", "@STORE", "@@STORE@@", "$HOME $(echo x) `id` ${D}", "no newline",
        "two\n\nlines\n\n", "\n", "tab\there\r\n", "ü → ✓", "{\"command\":[\"b10x-loom\",\"--store\",\"@STORE@\"]}\n",
    ];
    for (i, t) in texts.iter().enumerate() {
        fake.answer("tree_run", &(i + 1).to_string(), t, 0);
        fake.stderr("tree_run", &(i + 1).to_string(), t);
    }
    let stores = ["/plain/store", "/a|b&c\\d/'q'/\"dq\"/$x/`y`/ sp/%s"];
    let mut bad = Vec::new();
    let mut n = 0;
    for store in stores {
        for t in texts {
            n += 1;
            if n > texts.len() {
                // the second store reads its answers through `default`
                fake.answer("tree_run", "default", t, 0);
                fake.stderr("tree_run", "default", t);
            }
            let o = Command::new(fake.bin()).args(["tree", "run", "--store", store]).env("RUNBOOK_NO_CONFIG", "1").stdin(Stdio::null()).output().unwrap();
            let want = t.replace("@STORE@", store);
            if o.status.code() != Some(0) || o.stdout != want.as_bytes() || o.stderr != want.as_bytes() {
                bad.push(format!("{t:?} with store {store:?}: exit {:?}, stdout {:?}, stderr {:?}", o.status.code(), String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)));
            }
        }
    }
    let empty = Fake::new(&dir.join("empty"));
    empty.answer("tree_run", "1", "", 3).stderr("tree_run", "1", "");
    let o = Command::new(empty.bin()).args(["tree", "run"]).env("RUNBOOK_NO_CONFIG", "1").stdin(Stdio::null()).output().unwrap();
    if o.status.code() != Some(3) || !o.stdout.is_empty() || !o.stderr.is_empty() {
        bad.push(format!("the empty text: {o:?}"));
    }
    assert!(bad.is_empty(), "the fake mangled {} answer(s):\n{}", bad.len(), bad.join("\n"));
}
