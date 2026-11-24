//! `runbook.toml` (`docs/loom-run.md`, "Where a value comes from"): the file the binary finds by
//! itself, the order it is found in, what a value of it can and cannot do, and what a refusal of it
//! never prints. No test starts a model; the dry runs use the fixture runbook and no login.
//!
//! The binary is started here without the harness isolation (`crate::isolate`), with a private
//! `HOME` and `XDG_CONFIG_HOME` and a working directory inside the test's own tree, because the file
//! is what is tested. The last cases hold the isolation itself.

use super::core::BIN;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const FOUR: &str = "confinement = \"local\"\nmax_wall_seconds = 120\ncap_micro_usd = 1000000\nmax_llm_calls = 100\n";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
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

/// Fails loudly when the walk from a rig below `base` would read a file the case did not write: a `runbook.toml` in
/// `base` or above it, or a directory the walk stops at (a run directory, the state directory of a daemon).
fn refuse_ancestors(base: &Path) {
    for dir in base.ancestors() {
        if std::fs::symlink_metadata(dir.join("runbook.toml")).is_ok() {
            panic!("a rig cannot stand below {}: it holds a runbook.toml that every case below it would find; take it out of the way", dir.display());
        }
        if dir.join("events.jsonl").is_file() && dir.join("work").is_dir() && dir.join("bindings").is_dir() {
            panic!("a rig cannot stand below {}: it is a run directory, and the walk stops at it", dir.display());
        }
        if dir.join("daemon.lock").is_file() && dir.join("journal.jsonl").is_file() {
            panic!("a rig cannot stand below {}: it is the state directory of a daemon, and the walk stops at it", dir.display());
        }
    }
}

pub struct Rig {
    pub root: PathBuf,
}

impl Drop for Rig {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

impl Rig {
    /// A fresh tree of its own in the temporary directory of the test run, never below the checkout: the walk
    /// for `runbook.toml` climbs from the working directory to the root of the file system, and a file in the
    /// checkout (or in `target/` of it) must not be one of the files a case finds.
    pub fn new(tag: &str) -> Rig {
        Rig::new_in(&std::env::temp_dir(), tag)
    }

    /// The same in the directory `base`.
    pub fn new_in(base: &Path, tag: &str) -> Rig {
        refuse_ancestors(base);
        let root = base.join(format!("runbook-rig-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for d in ["home", "xdg", "work/sub"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        Rig { root }
    }

    pub fn p(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    /// Writes `text` to `rel` (mode 600, parents made) and returns the path.
    pub fn put(&self, rel: &str, text: &str) -> PathBuf {
        let path = self.p(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        path
    }

    /// The fixture runbook as the asset root `fx`, with the idea file `idea.txt` and what the decider needs.
    pub fn fixture(&self) {
        copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &self.p("fx"));
        super::decider::install_routing_table_at(&self.p("fx"));
        self.put("idea.txt", "text: a note on tea\n");
    }

    /// `runbook <args>` in `cwd` with a private HOME and XDG_CONFIG_HOME and no `RUNBOOK_CONFIG`.
    pub fn command(&self, cwd: &Path, args: &[&str]) -> Command {
        let mut c = Command::new(BIN);
        c.args(args)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", self.p("home"))
            .env("XDG_CONFIG_HOME", self.p("xdg"))
            .env("RUNBOOK_HOME", self.p("fx"))
            .current_dir(cwd)
            .stdin(Stdio::null());
        c
    }

    pub fn run(&self, cwd: &Path, args: &[&str]) -> Output {
        self.command(cwd, args).output().unwrap()
    }
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

/// The line of `runbook config` for `key`.
fn row(o: &Output, key: &str) -> String {
    let text = out(o);
    text.lines().find(|l| l.starts_with(&format!("{key} = "))).unwrap_or_else(|| panic!("no {key} line in\n{text}\n{}", err(o))).to_owned()
}

fn first_line(o: &Output) -> String {
    out(o).lines().next().unwrap_or("").to_owned()
}

// ---- the short command ----

fn dry(rig: &Rig, cwd: &Path, intake: &str) -> Output {
    rig.run(cwd, &["run", "engine-demo", "--intake", intake, "--dry"])
}

fn run_names(runs: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(runs).unwrap().flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

#[test]
fn a_file_that_states_the_four_values_starts_a_dry_run_from_its_directory_and_from_a_subdirectory() {
    let rig = Rig::new("short");
    rig.fixture();
    rig.put("runbook.toml", &format!("{FOUR}\n[models.writer]\nmodel = \"model-from-the-file\"\n"));
    let o = dry(&rig, &rig.root, "idea.txt");
    assert_eq!(code(&o), 0, "{}", err(&o));
    let runs = rig.p("runs");
    let names = run_names(&runs);
    assert_eq!(names.len(), 1, "{names:?}");
    assert!(err(&o).contains(&names[0]), "the generated name is printed: {}", err(&o));
    let outcome: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(runs.join(&names[0]).join("outcome.json")).unwrap()).unwrap();
    assert_eq!(outcome["outcome"], "ended", "{outcome}");
    assert!(!runs.join("spend-ledger.json").exists(), "no ledger is named: none is opened, the cap is counted in the run itself");
    let settings: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(runs.join(&names[0]).join("settings.json")).unwrap()).unwrap();
    assert_eq!(settings["models"]["writer"]["model"], "model-from-the-file", "{settings}");
    assert_eq!((settings["max_wall_seconds"].as_u64(), settings["loom"]["cap_micro_usd"].as_u64()), (Some(120), Some(1_000_000)), "{settings}");

}

#[test]
fn a_file_is_found_from_a_subdirectory_and_the_runs_directory_is_the_one_beside_it() {
    let rig = Rig::new("short-sub");
    rig.fixture();
    rig.put("runbook.toml", FOUR);
    let o = dry(&rig, &rig.p("work/sub"), "../../idea.txt");
    assert_eq!(code(&o), 0, "{}", err(&o));
    let names = run_names(&rig.p("runs"));
    assert_eq!(names.len(), 1, "the runs directory is the one beside the file: {names:?}");
    assert!(names[0].starts_with("engine-demo-"), "{names:?}");
    assert!(!rig.p("work/sub/runs").exists() && !rig.p("work/runs").exists());
}

#[test]
fn a_value_that_has_no_default_is_refused_with_its_name_and_the_line_to_add() {
    for (without, flag, key) in [("confinement", "--confinement", "confinement = "), ("max_wall_seconds", "--max-wall-seconds", "max_wall_seconds = ")] {
        let rig = Rig::new(&format!("nodefault-{without}"));
        rig.fixture();
        let text: String = FOUR.lines().filter(|l| !l.starts_with(without)).map(|l| format!("{l}\n")).collect();
        let file = rig.put("runbook.toml", &text);
        let o = dry(&rig, &rig.root, "idea.txt");
        assert_eq!(code(&o), 64, "{without}: {}", err(&o));
        let said = err(&o);
        assert!(said.contains(flag) && said.contains(key) && said.contains(&file.display().to_string()), "{without}: {said}");
        assert!(!rig.p("runs").exists(), "{without}: something was written");
    }
    // no file at all: the same refusal, and it says that no file was found
    let rig = Rig::new("nodefault-nofile");
    rig.fixture();
    let o = rig.run(&rig.root, &["--no-config", "run", "engine-demo", "--intake", "idea.txt", "--dry"]);
    assert_eq!(code(&o), 64, "{}", err(&o));
    assert!(err(&o).contains("--confinement") && err(&o).contains("runbook.toml"), "{}", err(&o));
}

/// The settings the one run under `runs` started with.
fn started_settings(runs: &Path) -> serde_json::Value {
    let names = run_names(runs);
    assert_eq!(names.len(), 1, "{names:?}");
    serde_json::from_str(&std::fs::read_to_string(runs.join(&names[0]).join("settings.json")).unwrap()).unwrap()
}

/// A cap the file does not state and no option names is no limit (`AGENTS.md`: no run is refused for a
/// missing money cap): the run starts and records the cap as null; the option or the line names it.
fn a_cap_is_optional(tag: &str, drop: &str, flag: &str, value: &str, recorded: fn(&serde_json::Value) -> &serde_json::Value) {
    let rig = Rig::new(tag);
    rig.fixture();
    let text: String = FOUR.lines().filter(|l| !l.starts_with(drop)).map(|l| format!("{l}\n")).collect();
    rig.put("runbook.toml", &text);
    let o = dry(&rig, &rig.root, "idea.txt");
    assert_eq!(code(&o), 0, "{drop}: {}", err(&o));
    assert!(recorded(&started_settings(&rig.p("runs"))).is_null(), "{drop}: a cap nobody named is recorded as null");
    // the option names it
    let rig = Rig::new(&format!("{tag}-option"));
    rig.fixture();
    rig.put("runbook.toml", &text);
    let o = rig.run(&rig.root, &["run", "engine-demo", "--intake", "idea.txt", "--dry", flag, value]);
    assert_eq!(code(&o), 0, "{drop} given as an option: {}", err(&o));
    assert_eq!(recorded(&started_settings(&rig.p("runs"))).to_string(), value, "{drop}: the option names the cap");
}

#[test]
fn the_cap_in_micro_usd_is_optional_and_the_file_or_the_option_may_state_it() {
    a_cap_is_optional("cap-usd", "cap_micro_usd", "--cap-micro-usd", "1000000", |s| &s["loom"]["cap_micro_usd"]);
}

#[test]
fn the_cap_on_model_calls_is_optional_and_the_file_or_the_option_may_state_it() {
    a_cap_is_optional("cap-calls", "max_llm_calls", "--max-llm-calls", "100", |s| &s["max_llm_calls"]);
}

// ---- where a value comes from ----

#[test]
fn config_prints_every_value_with_its_origin_and_an_option_is_the_origin_of_its_value() {
    let rig = Rig::new("origin");
    let file = rig.put("runbook.toml", &format!("{FOUR}autonomy = \"A5\"\n\n[decider]\nkind = \"rule\"\nverdict = \"approve\"\n"));
    let o = rig.run(&rig.root, &["config"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let path = file.display().to_string();
    assert!(first_line(&o).contains(&path) && first_line(&o).contains("working directory"), "{}", first_line(&o));
    for key in ["confinement", "max_wall_seconds", "cap_micro_usd", "max_llm_calls", "autonomy", "decider.kind", "decider.verdict"] {
        let r = row(&o, key);
        assert!(r.contains(&format!("file {path}")), "{r}");
    }
    assert!(row(&o, "confinement").contains("\"local\""), "{}", row(&o, "confinement"));
    assert!(row(&o, "runs_dir").contains(&rig.p("runs").display().to_string()) && row(&o, "runs_dir").contains("built-in"), "{}", row(&o, "runs_dir"));
    assert!(row(&o, "spend_ledger").contains("not stated") && row(&o, "spend_ledger").contains("no ledger is opened"), "{}", row(&o, "spend_ledger"));
    assert!(row(&o, "run").contains("generated") && row(&o, "run").contains("built-in"), "{}", row(&o, "run"));

    let o = rig.run(&rig.root, &["config", "--confinement", "substrate", "--cap-micro-usd", "7", "--runs-dir", "elsewhere"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    for (key, flag, value) in [("confinement", "--confinement", "\"substrate\""), ("cap_micro_usd", "--cap-micro-usd", "7")] {
        let r = row(&o, key);
        assert!(r.contains(&format!("option {flag}")) && r.contains(value) && !r.contains(&path), "{r}");
    }
    assert!(row(&o, "runs_dir").contains("option --runs-dir"), "{}", row(&o, "runs_dir"));
    assert!(row(&o, "max_wall_seconds").contains(&format!("file {path}")), "the values the option did not name stay the file's");
}

#[test]
fn a_value_of_the_file_never_lowers_what_an_option_sets() {
    let rig = Rig::new("lower");
    rig.put("runbook.toml", "confinement = \"local\"\nautonomy = \"A5\"\n");
    let o = rig.run(&rig.root, &["config", "--confinement", "substrate"]);
    assert!(row(&o, "confinement").contains("\"substrate\"") && row(&o, "confinement").contains("option"), "{}", row(&o, "confinement"));
    // the file states the stronger level and the option the weaker one: the option is what the caller asked for
    rig.put("runbook.toml", "confinement = \"substrate\"\n");
    let o = rig.run(&rig.root, &["config", "--confinement", "local"]);
    assert!(row(&o, "confinement").contains("\"local\"") && row(&o, "confinement").contains("option"), "{}", row(&o, "confinement"));
    // a `--settings` file that states a lower autonomy level than the file keeps its level
    rig.put("runbook.toml", "autonomy = \"A5\"\n[decider]\nkind = \"rule\"\nverdict = \"approve\"\n");
    let settings = rig.put("settings.toml", "autonomy = \"A2\"\n");
    let o = rig.run(&rig.root, &["config", "--settings", &settings.display().to_string()]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let a = row(&o, "autonomy");
    assert!(a.contains("\"A2\"") && a.contains("settings") && !a.contains("A5"), "{a}");
    assert!(row(&o, "decider.kind").contains("file"), "a member the settings file does not state stays the file's: {}", row(&o, "decider.kind"));
}

#[test]
fn the_file_is_found_in_the_order_option_variable_walk_user_directory_and_no_two_are_merged() {
    let rig = Rig::new("order");
    let by_option = rig.put("chosen/option.toml", "max_wall_seconds = 4\n");
    let by_env = rig.put("chosen/env.toml", "max_wall_seconds = 3\n");
    let walk = rig.put("work/runbook.toml", "max_wall_seconds = 1\n");
    let user = rig.put("xdg/runbook/runbook.toml", "max_wall_seconds = 2\ncap_micro_usd = 55\n");
    let cwd = rig.p("work/sub");
    let wall = |o: &Output| row(o, "max_wall_seconds");
    let mut c = rig.command(&cwd, &["config", "--config", &by_option.display().to_string()]);
    c.env("RUNBOOK_CONFIG", &by_env);
    let o = c.output().unwrap();
    assert!(wall(&o).contains(" 4 ") || wall(&o).contains("= 4"), "{}", wall(&o));
    assert!(first_line(&o).contains(&by_option.display().to_string()), "{}", first_line(&o));
    let mut c = rig.command(&cwd, &["config"]);
    c.env("RUNBOOK_CONFIG", &by_env);
    let o = c.output().unwrap();
    assert!(wall(&o).contains("= 3"), "{}", wall(&o));
    assert!(first_line(&o).contains("RUNBOOK_CONFIG"), "{}", first_line(&o));
    let o = rig.run(&cwd, &["config"]);
    assert!(wall(&o).contains("= 1") && first_line(&o).contains(&walk.display().to_string()), "{}", first_line(&o));
    assert!(row(&o, "cap_micro_usd").contains("not stated"), "the file of the user's directory is not merged in: {}", row(&o, "cap_micro_usd"));
    std::fs::remove_file(&walk).unwrap();
    let o = rig.run(&cwd, &["config"]);
    assert!(wall(&o).contains("= 2") && row(&o, "cap_micro_usd").contains("= 55"), "{}", first_line(&o));
    assert!(first_line(&o).contains(&user.display().to_string()), "{}", first_line(&o));
    // without XDG_CONFIG_HOME the directory is <home>/.config/runbook
    rig.put("home/.config/runbook/runbook.toml", "max_wall_seconds = 6\n");
    let mut c = rig.command(&cwd, &["config"]);
    c.env_remove("XDG_CONFIG_HOME");
    let o = c.output().unwrap();
    assert!(wall(&o).contains("= 6"), "{}", first_line(&o));
    // no file is read with --no-config, whatever else names one
    rig.put("work/runbook.toml", "max_wall_seconds = 1\n");
    let mut c = rig.command(&cwd, &["--no-config", "config"]);
    c.env("RUNBOOK_CONFIG", &by_env);
    let o = c.output().unwrap();
    assert!(wall(&o).contains("not stated") && first_line(&o).contains("none"), "{}", out(&o));
    let mut c = rig.command(&cwd, &["config"]);
    c.env("RUNBOOK_NO_CONFIG", "1");
    let o = c.output().unwrap();
    assert!(wall(&o).contains("not stated"), "{}", out(&o));
    // a file a caller named that is not there is refused, not skipped
    let o = rig.run(&cwd, &["config", "--config", &rig.p("nothing.toml").display().to_string()]);
    assert_eq!(code(&o), 2, "{}", err(&o));
    assert!(err(&o).contains("nothing.toml"), "{}", err(&o));
}

#[test]
fn a_relative_path_of_the_file_is_relative_to_the_file() {
    let rig = Rig::new("relative");
    rig.put("conf/runbook.toml", "runs_dir = \"out/runs\"\nspend_ledger = \"ledger.json\"\n");
    let o = rig.run(&rig.p("work"), &["config", "--config", &rig.p("conf/runbook.toml").display().to_string()]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(row(&o, "runs_dir").contains(&rig.p("conf/out/runs").display().to_string()), "{}", row(&o, "runs_dir"));
    assert!(row(&o, "spend_ledger").contains(&rig.p("conf/ledger.json").display().to_string()), "{}", row(&o, "spend_ledger"));
}

// ---- init ----

#[test]
fn init_writes_a_starter_that_is_read_and_refuses_to_overwrite() {
    let rig = Rig::new("init");
    let o = rig.run(&rig.root, &["init"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let file = rig.p("runbook.toml");
    let starter = std::fs::read_to_string(&file).unwrap();
    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
    let o = rig.run(&rig.root, &["init"]);
    assert_ne!(code(&o), 0);
    assert!(err(&o).contains("runbook.toml") && err(&o).contains("exists"), "{}", err(&o));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), starter, "the file was not changed");
    let o = rig.run(&rig.root, &["config"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    for key in ["runs_dir", "confinement", "max_wall_seconds", "cap_micro_usd", "max_llm_calls"] {
        assert!(row(&o, key).contains(&format!("file {}", file.display())), "{}", row(&o, key));
    }
    assert!(row(&o, "confinement").contains("\"substrate\""), "the starter names the stronger level: {}", row(&o, "confinement"));
    // the starter names no ledger: none is opened (`AGENTS.md`: no ledger nobody named)
    assert!(row(&o, "spend_ledger").contains("not stated") && row(&o, "run").contains("built-in"), "{}", row(&o, "spend_ledger"));
    assert!(!starter.contains('#'), "a starter has no comment");
}

// ---- what a refusal never prints ----

fn refuse(rig: &Rig, name: &str, bytes: &[u8]) -> (Output, PathBuf) {
    let path = rig.p(&format!("bad/{name}.toml"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, bytes).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    (rig.run(&rig.root, &["config", "--config", &path.display().to_string()]), path)
}

#[test]
fn a_refusal_names_the_file_and_the_key_and_never_prints_a_line_a_value_or_a_part_of_one() {
    const S: &str = "SECRETVALUE-4711";
    let rig = Rig::new("refuse");
    let cases: Vec<(&str, String, &str)> = vec![
        ("unknown-key", format!("api_key = \"{S}\"\n"), "api_key"),
        ("unknown-nested", format!("[models.writer]\ntoken = \"{S}\"\n"), "models.writer.token"),
        ("unknown-decider", format!("[decider]\nkind = \"rule\"\nverdict = \"approve\"\nauth = \"{S}\"\n"), "decider"),
        ("old-place", format!("[loom]\nbin = \"{S}\"\n"), "loom"),
        ("syntax", format!("confinement = {S} more\n"), "line 1"),
        ("syntax-line-3", format!("a = 1\nb = 2\nc = {S}\n"), "line 3"),
        ("wrong-type", format!("max_wall_seconds = \"{S}\"\n"), "max_wall_seconds"),
        ("wrong-level", format!("confinement = \"{S}\"\n"), "confinement"),
        ("wrong-path", format!("runs_dir = 5\nlogin_file = \"{S}\"\n"), "runs_dir"),
        ("not-a-limit", format!("[models.writer]\nmax_turns = 0\nmodel = \"{S}\"\n"), "models.writer"),
        ("json", format!("{{\"confinement\": \"{S}\"}}\n"), "JSON"),
        ("login-content", format!("login_file = \"{{\\\"token\\\": \\\"{S}\\\"}}\"\n"), "login_file"),
    ];
    for (name, text, names) in &cases {
        let (o, path) = refuse(&rig, name, text.as_bytes());
        let all = format!("{}{}", out(&o), err(&o));
        assert_eq!(code(&o), 2, "{name}: {all}");
        assert!(all.contains(&path.display().to_string()), "{name}: the file is named: {all}");
        assert!(all.contains(names), "{name}: names `{names}`: {all}");
        assert!(!all.contains(S) && !all.contains("SECRET"), "{name}: a value was printed: {all}");
        assert!(all.len() < 2048, "{name}: {} bytes", all.len());
    }
}

#[test]
fn a_value_or_a_key_that_has_the_form_of_a_key_or_a_token_is_refused_by_the_key_without_printing_it() {
    let rig = Rig::new("tokens");
    // each form is joined from two pieces, so no line of this file has the form of a secret
    let forms: Vec<String> = [
        ["sk-", "ant-api03-aaaaaaaaaaaaaaaaaaaaaaaa"],
        ["gh", "p_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],
        ["github_", "pat_11AAAAAAA0aaaaaaaaaaaa_bbbb"],
        ["gl", "pat-aaaaaaaaaaaaaaaaaaaa"],
        ["xo", "xb-1111-2222-aaaaaaaaaaaa"],
        ["AK", "IAABCDEFGHIJKLMNOP"],
        ["-----BEGIN ", "PRIVATE KEY-----"],
        ["ey", "JhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.abcdefgh"],
        ["Bear", "er aaaaaaaaaaaaaaaaaaaa"],
    ]
    .iter()
    .map(|p| p.concat())
    .collect();
    for (i, form) in forms.iter().enumerate() {
        for key in ["login_file", "runs_dir", "adapter"] {
            let (o, path) = refuse(&rig, &format!("form-{i}-{key}"), format!("{key} = \"{form}\"\n").as_bytes());
            let all = format!("{}{}", out(&o), err(&o));
            assert_eq!(code(&o), 2, "{form} in {key}: {all}");
            assert!(all.contains(key) && all.contains(&path.display().to_string()) && all.contains("token"), "{form} in {key}: {all}");
            assert!(!all.contains(&form[..8]), "{form} in {key}: a part of the value was printed: {all}");
        }
        let (o, _) = refuse(&rig, &format!("form-{i}-nested"), format!("[models.writer]\nmodel = \"{form}\"\n").as_bytes());
        let all = format!("{}{}", out(&o), err(&o));
        assert_eq!(code(&o), 2, "{form}: {all}");
        assert!(all.contains("models.writer.model") && !all.contains(&form[..8]), "{form}: {all}");
        let (o, _) = refuse(&rig, &format!("form-{i}-key"), format!("\"{form}\" = 1\n").as_bytes());
        let all = format!("{}{}", out(&o), err(&o));
        assert_eq!(code(&o), 2, "{form} as a key: {all}");
        assert!(!all.contains(&form[..8]), "{form} as a key: {all}");
    }
    // a login file is named by path: a path is accepted and never opened
    let o = rig.run(&rig.root, &["config", "--config", &rig.put("named.toml", "login_file = \"/nowhere/login.json\"\n").display().to_string()]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(row(&o, "login_file").contains("/nowhere/login.json"), "{}", row(&o, "login_file"));
}

// ---- which file is read ----

#[test]
fn a_file_that_others_can_write_or_that_is_a_link_is_refused_by_name_where_the_walk_finds_it() {
    let rig = Rig::new("modes");
    let cwd = rig.p("work/sub");
    // group-writable, found by the walk
    let file = rig.put("work/runbook.toml", "max_wall_seconds = 1\n");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o664)).unwrap();
    let o = rig.run(&cwd, &["config"]);
    assert_eq!(code(&o), 2, "{}", err(&o));
    assert!(err(&o).contains(&file.display().to_string()) && err(&o).contains("writable"), "{}", err(&o));
    // the same file named by the caller: the same rule
    let o = rig.run(&cwd, &["config", "--config", &file.display().to_string()]);
    assert_eq!(code(&o), 2, "{}", err(&o));
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o602)).unwrap();
    let o = rig.run(&cwd, &["config"]);
    assert_eq!(code(&o), 2, "{}", err(&o));
    std::fs::remove_file(&file).unwrap();
    // a link, found by the walk
    let target = rig.put("elsewhere/real.toml", "max_wall_seconds = 1\n");
    std::os::unix::fs::symlink(&target, rig.p("work/runbook.toml")).unwrap();
    let o = rig.run(&cwd, &["config"]);
    assert_eq!(code(&o), 2, "{}", err(&o));
    assert!(err(&o).contains(&rig.p("work/runbook.toml").display().to_string()), "{}", err(&o));
    // a link the caller names is the caller's choice
    let o = rig.run(&cwd, &["config", "--config", &rig.p("work/runbook.toml").display().to_string()]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    std::fs::remove_file(rig.p("work/runbook.toml")).unwrap();
    // a file with two names
    std::fs::hard_link(&target, rig.p("work/runbook.toml")).unwrap();
    let o = rig.run(&cwd, &["config"]);
    assert_eq!(code(&o), 2, "{}", err(&o));
    assert!(err(&o).contains("names"), "{}", err(&o));
    std::fs::remove_file(rig.p("work/runbook.toml")).unwrap();
    // a directory of that name, and the user's directory with a bad mode
    std::fs::create_dir(rig.p("work/runbook.toml")).unwrap();
    let o = rig.run(&cwd, &["config"]);
    assert_eq!(code(&o), 2, "{}", err(&o));
    std::fs::remove_dir(rig.p("work/runbook.toml")).unwrap();
    let user = rig.put("xdg/runbook/runbook.toml", "max_wall_seconds = 2\n");
    std::fs::set_permissions(&user, std::fs::Permissions::from_mode(0o666)).unwrap();
    let o = rig.run(&cwd, &["config"]);
    assert_eq!(code(&o), 2, "{}", err(&o));
    assert!(err(&o).contains(&user.display().to_string()), "{}", err(&o));
}

/// The files a run holds that mark its directory: `events.jsonl`, `work/` and `bindings/`.
fn fake_run(rig: &Rig, rel: &str) -> PathBuf {
    let run = rig.p(rel);
    std::fs::create_dir_all(run.join("work/sub")).unwrap();
    std::fs::create_dir_all(run.join("bindings")).unwrap();
    std::fs::write(run.join("events.jsonl"), "").unwrap();
    run
}

#[test]
fn the_walk_never_reads_a_file_inside_a_run_or_a_directory_of_runs() {
    let rig = Rig::new("runtree");
    let outer = rig.put("runbook.toml", "confinement = \"substrate\"\nmax_wall_seconds = 90\n");
    let run = fake_run(&rig, "runs/r1");
    // what a model wrote into the work directory, into the run directory and into the directory of runs
    rig.put("runs/r1/work/runbook.toml", "confinement = \"local\"\nmax_wall_seconds = 1\n");
    rig.put("runs/r1/work/sub/runbook.toml", "confinement = \"local\"\nmax_wall_seconds = 2\n");
    rig.put("runs/r1/runbook.toml", "confinement = \"local\"\nmax_wall_seconds = 3\n");
    rig.put("runs/runbook.toml", "confinement = \"local\"\nmax_wall_seconds = 4\n");
    for cwd in [run.join("work/sub"), run.join("work"), run.clone()] {
        let o = rig.run(&cwd, &["config"]);
        assert_eq!(code(&o), 0, "{}", err(&o));
        assert!(first_line(&o).contains(&outer.display().to_string()), "{}: {}", cwd.display(), first_line(&o));
        assert!(row(&o, "confinement").contains("\"substrate\""), "{}", row(&o, "confinement"));
    }
    // the user's directory inside a run (HOME of a program the runner starts) is no source either
    rig.put("runs/r1/user/.config/runbook/runbook.toml", "confinement = \"local\"\nmax_wall_seconds = 5\n");
    let mut c = rig.command(&run.join("work"), &["config"]);
    c.env_remove("XDG_CONFIG_HOME").env("HOME", run.join("user"));
    let o = c.output().unwrap();
    assert!(row(&o, "max_wall_seconds").contains("= 90"), "{}", out(&o));
    // the runner states it for the programs it starts
    let mut c = rig.command(&run.join("work"), &["config"]);
    c.env("RUNBOOK_NO_CONFIG", "1");
    let o = c.output().unwrap();
    assert!(row(&o, "max_wall_seconds").contains("not stated"), "{}", out(&o));
}

#[test]
fn a_run_resolves_its_file_before_its_directory_exists_and_a_program_in_it_gets_no_walk() {
    let rig = Rig::new("child-env");
    rig.fixture();
    rig.put("runbook.toml", FOUR);
    let o = dry(&rig, &rig.root, "idea.txt");
    assert_eq!(code(&o), 0, "{}", err(&o));
    // the environment the runner builds for the `b10x-loom` child (`child_env`): every call of the child is
    // told to read no file, whatever a program of the run wrote into the tree
    let w = super::core::World::new("cfgfile-child-env");
    w.one_decision_tree();
    w.scripted_approve();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", err(&o));
    let calls = w.fake.calls();
    assert!(!calls.is_empty(), "the child was not called");
    for call in &calls {
        assert!(call.env.iter().any(|(k, v)| k == "RUNBOOK_NO_CONFIG" && v == "1"), "a call of the child is not told to read no runbook.toml: {:?}", call.env.iter().map(|(k, _)| k).collect::<Vec<_>>());
    }
}

// ---- the isolation of this test binary ----

#[test]
fn the_harness_reads_no_runbook_toml_of_the_host_and_no_start_of_the_binary_skips_it() {
    // a file in the user's directories and in the working directory that would change the outcome
    let rig = Rig::new("isolation");
    let poison = "max_wall_seconds = 7\nconfinement = \"local\"\n";
    rig.put("xdg/runbook/runbook.toml", poison);
    rig.put("home/.config/runbook/runbook.toml", poison);
    rig.put("runbook.toml", poison);
    let mut c = rig.command(&rig.root, &["config"]);
    crate::isolate(&mut c);
    let o = c.output().unwrap();
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(first_line(&o).contains("empty-config") && first_line(&o).contains("RUNBOOK_CONFIG"), "{}", first_line(&o));
    assert!(row(&o, "max_wall_seconds").contains("not stated") && row(&o, "confinement").contains("not stated"), "{}", out(&o));
    // every start of the binary in the sources of the tests names the empty file when it can read one: the one scan
    let (starts, unguarded) = super::config_guards::unguarded_starts();
    assert!(starts >= 8, "the scan found only {starts} starts of the binary");
    assert!(unguarded.is_empty(), "starts of the binary that read a file of the host: {unguarded:?}");
}

#[test]
fn every_start_of_the_binary_in_the_install_cases_names_the_empty_configuration() {
    // the install cases start the binary from a working directory below the target directory, with their own
    // environment: without a file or `RUNBOOK_NO_CONFIG` named the walk goes up to the host's directories
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/install");
    let mut unguarded = Vec::new();
    let mut starts = 0;
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        for (at, _) in text.match_indices("Command::new(") {
            let head: String = text[at..].chars().take(60).collect();
            let runbook = ["bin/runbook", "(BIN)", "(&program)", "(program)", "(\"runbook\")"].iter().any(|m| head.contains(m));
            if !runbook {
                continue;
            }
            let end = ["output()", ".status()", ".spawn()"].iter().filter_map(|m| text[at..].find(m)).min().map_or(text.len(), |n| at + n);
            let statement = &text[at..end];
            starts += 1;
            if !["isolate(", "RUNBOOK_CONFIG", "--no-config", "RUNBOOK_NO_CONFIG"].iter().any(|m| statement.contains(m)) {
                unguarded.push(format!("{}:{}", path.file_name().unwrap().to_string_lossy(), text[..at].matches('\n').count() + 1));
            }
        }
    }
    assert!(starts >= 4, "the scan found only {starts} starts of the binary");
    assert!(unguarded.is_empty(), "starts of the binary in the install cases that read a runbook.toml of the host's directories: {unguarded:?}");
}

// ---- the docs ----

fn blocks(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut lines = text.lines();
    while let Some(l) = lines.next() {
        if l.trim_end() == "```toml runbook.toml" {
            let mut block = String::new();
            for b in lines.by_ref() {
                if b.starts_with("```") {
                    break;
                }
                block.push_str(b);
                block.push('\n');
            }
            out.push(block);
        }
    }
    out
}

#[test]
fn every_runbook_toml_block_of_the_docs_is_read_and_the_readme_shows_the_starter_and_the_short_command() {
    let rig = Rig::new("docs");
    let mut pages = vec![repo().join("README.md")];
    for entry in std::fs::read_dir(repo().join("docs")).unwrap().flatten() {
        if entry.path().extension().is_some_and(|e| e == "md") {
            pages.push(entry.path());
        }
    }
    let mut total = 0;
    for page in &pages {
        let text = std::fs::read_to_string(page).unwrap();
        for (i, block) in blocks(&text).iter().enumerate() {
            total += 1;
            let file = rig.put(&format!("docs/{total}.toml"), block);
            let o = rig.run(&rig.root, &["config", "--config", &file.display().to_string()]);
            assert_eq!(code(&o), 0, "block {i} of {}: {}", page.display(), err(&o));
        }
    }
    let readme = std::fs::read_to_string(repo().join("README.md")).unwrap();
    assert!(!blocks(&readme).is_empty(), "README.md has no runbook.toml block");
    assert!(readme.contains("runbook run sdc --intake idea.txt --dry"), "README.md does not show the short command");
    assert!(std::fs::read_to_string(repo().join("docs/loom-run.md")).unwrap().contains("```toml runbook.toml"), "docs/loom-run.md has no runbook.toml block");
    // the starter of `runbook init` is a block of the README, byte for byte
    let dir = rig.p("starter");
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(code(&rig.run(&dir, &["init"])), 0);
    let starter = std::fs::read_to_string(dir.join("runbook.toml")).unwrap();
    assert!(blocks(&readme).contains(&starter), "the starter of `runbook init` is not a block of README.md");
}

#[test]
fn the_value_options_belong_to_a_run_in_process_and_conflict_with_detach() {
    let rig = Rig::new("detach");
    let o = rig.run(&rig.root, &["run", "engine-demo", "--intake", "idea.txt", "--detach", "--runs-dir", "x"]);
    assert_eq!(code(&o), 64, "{}", err(&o));
    assert!(err(&o).contains("--runs-dir") && err(&o).contains("--detach"), "{}", err(&o));
}
