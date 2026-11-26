//! What `runbook config` and the refusals of `runbook.toml` and of a settings file print, whatever joins a token form
//! to the text around it, and what a file found inside a program of a run may state. Every start has a private HOME
//! and XDG_CONFIG_HOME and a working directory inside the case's own tree (`Rig`).

use super::config_file::Rig;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output, Stdio};

/// A marker that no output may carry once a secret that holds it is printed.
const MARK: &str = "ZZmarkedsecretZZ";

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

fn both(o: &Output) -> String {
    format!("{}{}", out(o), err(o))
}

/// A TOML basic string.
fn basic(v: &str) -> String {
    let mut s = String::from("\"");
    for c in v.chars() {
        match c {
            '\\' => s.push_str("\\\\"),
            '"' => s.push_str("\\\""),
            '\n' => s.push_str("\\n"),
            '\t' => s.push_str("\\t"),
            c => s.push(c),
        }
    }
    s.push('"');
    s
}

/// The secrets of every form the unit claims to hide, each holding `MARK`.
fn secrets() -> Vec<(String, String)> {
    let mut all: Vec<(String, String)> = Vec::new();
    for p in ["sk-ant-", "ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_", "glpat-", "xoxb-", "xoxp-", "xoxa-", "xoxs-", "xapp-", "npm_"] {
        all.push((format!("prefix {p}"), format!("{p}{MARK}")));
    }
    all.push(("an aws key".to_owned(), format!("AKIA{}", MARK.to_uppercase())));
    all.push(("a google key".to_owned(), format!("AIza{MARK}{}", "x".repeat(35 - MARK.len()))));
    all.push(("a generic sk key".to_owned(), format!("sk-{MARK}abcdefghijkl")));
    all.push(("a web token".to_owned(), format!("eyJ{MARK}.eyJ{MARK}.sig{MARK}")));
    all
}

fn leak_line(shown: &str) -> String {
    shown.lines().find(|l| l.to_lowercase().contains(&MARK.to_lowercase())).unwrap_or("").chars().take(200).collect()
}

fn has_mark(shown: &str) -> bool {
    shown.to_lowercase().contains(&MARK.to_lowercase())
}

// ---- a token-form word is hidden after every joiner ----

#[test]
fn config_hides_every_token_form_after_every_joiner_the_unit_names() {
    // `runbook config --settings` prints a string member as a value
    let joiners: [(&str, &str); 9] = [("an equals sign", "NAME="), ("a dot", "name."), ("a colon", "name:"), ("a slash", "/usr/bin/"), ("a backslash", "C:\\dir\\"), ("a new line", "line\n"), ("a space", "x "), ("a dot and an equals sign", "a.b=c."), ("an equals sign and a slash", "PATH=/x/")];
    let rig = Rig::new("joiners");
    let mut leaked = Vec::new();
    let mut n = 0;
    for (kind, secret) in secrets() {
        for (joiner, pre) in joiners {
            for (member, make) in [("adapter", 0), ("models.writer.model", 1)] {
                n += 1;
                let word = format!("{pre}{secret}");
                let text = if make == 0 { format!("adapter = {}\n", basic(&word)) } else { format!("[models.writer]\nmodel = {}\n", basic(&word)) };
                let file = rig.put(&format!("s-{n}.toml"), &text);
                let o = rig.run(&rig.root, &["config", "--settings", &file.display().to_string()]);
                if has_mark(&both(&o)) {
                    leaked.push(format!("{kind} after {joiner} in {member}: {}", leak_line(&both(&o))));
                }
            }
        }
    }
    assert!(leaked.is_empty(), "runbook config printed a token form after a joiner the unit says it hides ({} of {n}):\n{}", leaked.len(), leaked.join("\n"));
}

// ---- a refusal that names a key prints no part of a key that holds a token form ----

/// A file of a shape, made from a key.
type Shape<'a> = (&'static str, Box<dyn Fn(&str) -> String + 'a>);

/// Joiners that a key can hold between a name and a token form; the form of the whole key does not begin at them.
fn key_joiners() -> Vec<(&'static str, &'static str)> {
    vec![("a slash", "see/"), ("a colon", "see:"), ("a space", "see "), ("a backslash", "see\\"), ("an at sign", "see@"), ("a comma", "see,"), ("a semicolon", "see;"), ("a new line", "see\n")]
}

#[test]
fn a_refusal_of_runbook_toml_prints_no_key_that_holds_a_token_form() {
    // `runbook.toml` is found by the walk; every command that reads it prints its refusal as it is
    let rig = Rig::new("refusal-keys");
    rig.fixture();
    let secret = format!("ghp_{MARK}");
    let shapes: Vec<Shape<'_>> = vec![
        ("an unknown top level key", Box::new(|k| format!("{} = 1\n", basic(k)))),
        ("an unknown key of the decider", Box::new(|k| format!("[decider]\n{} = 1\n", basic(k)))),
        ("a table of the review", Box::new(|k| format!("[review.{}]\nx = 1\n", basic(k)))),
        ("a key of the efforts", Box::new(|k| format!("[efforts]\n{} = 1\n", basic(k)))),
        ("a table of the models", Box::new(|k| format!("[models.{}]\nx = 1\n", basic(k)))),
        ("a key of a model", Box::new(|k| format!("[models.writer]\n{} = 1\n", basic(k)))),
        ("a value that has a key above it", Box::new(|k| format!("[models.{}]\nmodel = \"{}\"\n", basic(k), secret))),
    ];
    let commands: [(&str, &[&str]); 4] = [("config", &["config"]), ("list", &["list"]), ("check", &["check"]), ("run", &["run", "engine-demo", "--intake", "idea.txt", "--dry"])];
    let mut leaked = Vec::new();
    let mut starts = 0;
    for (joiner, pre) in key_joiners() {
        let key = format!("{pre}{secret}");
        for (shape, make) in &shapes {
            rig.put("runbook.toml", &make(&key));
            for (name, args) in commands {
                starts += 1;
                let o = rig.run(&rig.root, args);
                if has_mark(&both(&o)) {
                    leaked.push(format!("{shape}, a key with {joiner}, `runbook {name}`: {}", leak_line(&both(&o))));
                }
            }
        }
    }
    assert!(leaked.is_empty(), "a refusal of runbook.toml printed a key that holds a token form ({} of {starts}):\n{}", leaked.len(), leaked.join("\n"));
}

#[test]
fn a_refusal_of_a_settings_file_prints_no_key_that_holds_a_token_form() {
    // a date is no value of any setting: the refusal names the key path of the member
    let rig = Rig::new("refusal-settings");
    let secret = format!("ghp_{MARK}");
    let shapes: Vec<Shape<'_>> = vec![
        ("a top level key", Box::new(|k| format!("{} = 1979-05-27\n", basic(k)))),
        ("a table of the models", Box::new(|k| format!("[models.{}]\nx = 1979-05-27\n", basic(k)))),
        ("a key of the models", Box::new(|k| format!("[models]\n{} = 1979-05-27\n", basic(k)))),
        ("a table of an array of tables", Box::new(|k| format!("[[review]]\n{} = 1979-05-27\n", basic(k)))),
        ("an inline table in an array", Box::new(|k| format!("review = [{{ {} = 1979-05-27 }}]\n", basic(k)))),
    ];
    let mut leaked = Vec::new();
    let mut n = 0;
    for (joiner, pre) in key_joiners() {
        let key = format!("{pre}{secret}");
        for (shape, make) in &shapes {
            n += 1;
            let file = rig.put(&format!("s-{n}.toml"), &make(&key));
            let o = rig.run(&rig.root, &["config", "--settings", &file.display().to_string()]);
            assert_eq!(code(&o), 2, "{shape}: {}", both(&o));
            if has_mark(&both(&o)) {
                leaked.push(format!("{shape}, a key with {joiner}: {}", leak_line(&both(&o))));
            }
        }
    }
    assert!(leaked.is_empty(), "a refusal of a settings file printed a key that holds a token form ({} of {n}):\n{}", leaked.len(), leaked.join("\n"));
}

const FOUR: &str = "confinement = \"local\"\nmax_wall_seconds = 120\ncap_micro_usd = 1000000\nmax_llm_calls = 100\n";

#[test]
fn a_refusal_of_a_run_that_names_a_key_of_its_settings_prints_no_key_that_holds_a_token_form() {
    // the settings reader of the run names an unknown top level key; `runbook run` prints what the run refused
    let rig = Rig::new("refusal-run");
    rig.fixture();
    rig.put("runbook.toml", FOUR);
    let secret = format!("ghp_{MARK}");
    let mut leaked = Vec::new();
    for (n, (joiner, pre)) in key_joiners().into_iter().enumerate() {
        let file = rig.put(&format!("s-{n}.toml"), &format!("{} = 1\n", basic(&format!("{pre}{secret}"))));
        let o = rig.run(&rig.root, &["run", "engine-demo", "--intake", "idea.txt", "--dry", "--settings", &file.display().to_string()]);
        assert_eq!(code(&o), 2, "{joiner}: {}", both(&o));
        assert!(err(&o).contains("not a setting of the format"), "{joiner}: the run does not refuse the key: {}", err(&o));
        if has_mark(&both(&o)) {
            leaked.push(format!("{joiner}: {}", leak_line(&both(&o))));
        }
    }
    assert!(leaked.is_empty(), "a run printed a key of its settings that holds a token form:\n{}", leaked.join("\n"));
}

#[test]
fn a_refusal_that_names_a_runbook_directory_prints_no_part_of_a_name_that_holds_a_token_form() {
    // the catalog names the directory it cannot load; `list`, `check`, `run` and `config` print its path
    let rig = Rig::new("refusal-dir");
    rig.fixture();
    let secret = format!("ghp_{MARK}");
    let mut leaked = Vec::new();
    for (joiner, pre) in key_joiners() {
        rig.put("runbook.toml", &format!("runbooks = [{}]\n", basic(&format!("{pre}{secret}"))));
        for args in [vec!["list"], vec!["check"], vec!["config"], vec!["run", "engine-demo", "--intake", "idea.txt", "--dry"]] {
            let o = rig.run(&rig.root, &args);
            if has_mark(&both(&o)) {
                leaked.push(format!("a directory with {joiner}, `runbook {}`: {}", args[0], leak_line(&both(&o))));
            }
        }
    }
    assert!(leaked.is_empty(), "a refusal printed the name of a runbook directory that holds a token form:\n{}", leaked.join("\n"));
}

// ---- a file found inside a program of a run ----

fn executable(path: &Path) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A shell program of a run: `LOOM_CASE` set as Loom sets it, PATH only, the working directory `cwd`; it runs `body`.
fn program_of_a_run(rig: &Rig, cwd: &Path, body: &str) -> Output {
    let script = rig.put("program.sh", &format!("#!/bin/sh\n{body}\n"));
    executable(&script);
    Command::new("sh")
        .arg(&script)
        .env_clear().envs(crate::kept())
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", rig.p("home"))
        .env("XDG_CONFIG_HOME", rig.p("xdg"))
        .env("LOOM_CASE", "a-case")
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn confinement_row(o: &Output) -> String {
    out(o).lines().find(|l| l.starts_with("confinement = ")).unwrap_or("").to_owned()
}

#[test]
fn loom_case_is_a_hint_and_a_program_that_clears_it_reads_the_file_but_not_below_the_option() {
    // docs/loom-run.md: `LOOM_CASE` is a hint, not a guard. A program of a run controls its own environment, so one that
    // clears it is a person's start to the walk; what holds is that the nested run keeps its sandbox and that
    // `--confinement` is never lowered by a file.
    let rig = Rig::new("hint");
    rig.put("runbook.toml", "confinement = \"local\"\nautonomy = \"A5\"\n");
    let bin = super::core::BIN;
    let plain = program_of_a_run(&rig, &rig.root, &format!("\"{bin}\" config"));
    assert_eq!(code(&plain), 2, "the program of a run with LOOM_CASE reads a level of the tree's file: {}", both(&plain));
    for (how, body) in [("env -u", format!("env -u LOOM_CASE \"{bin}\" config")), ("unset", format!("unset LOOM_CASE; \"{bin}\" config")), ("empty", format!("LOOM_CASE= \"{bin}\" config"))] {
        let o = program_of_a_run(&rig, &rig.root, &body);
        assert_eq!(code(&o), 0, "a program that clears the hint ({how}) is read as a person's start, as the page says: {}", both(&o));
        assert!(confinement_row(&o).contains("local"), "{how}: {}", confinement_row(&o));
        let o = program_of_a_run(&rig, &rig.root, &format!("env -u LOOM_CASE \"{bin}\" config --confinement substrate"));
        assert_eq!(code(&o), 0, "{how}: {}", both(&o));
        assert!(confinement_row(&o).contains("substrate") && confinement_row(&o).contains("option --confinement") && !confinement_row(&o).contains("local"), "{how}: the option does not win: {}", confinement_row(&o));
    }
}

#[test]
fn a_run_directory_made_above_the_working_directory_is_the_documented_project_file_case() {
    // the walk skips a run directory and the directory of runs above it and reads the file above those; with
    // `LOOM_CASE` and a run directory above the working directory a file above them is "the project's own file above
    // `runs/`" (docs/loom-run.md). A program that makes both gets the documented reading, and no more.
    let rig = Rig::new("fake-run");
    rig.put("runbook.toml", "confinement = \"local\"\nautonomy = \"A5\"\n");
    rig.put("x/a/events.jsonl", "");
    std::fs::create_dir_all(rig.p("x/a/work/sub")).unwrap();
    std::fs::create_dir_all(rig.p("x/a/bindings")).unwrap();
    let bin = super::core::BIN;
    let o = program_of_a_run(&rig, &rig.p("x/a/work/sub"), &format!("\"{bin}\" config"));
    assert_eq!(code(&o), 0, "a run directory above the working directory is read as the project's file case, as the page says: {}", both(&o));
    assert!(confinement_row(&o).contains("local"), "{}", confinement_row(&o));
    let o = program_of_a_run(&rig, &rig.p("x/a/work/sub"), &format!("\"{bin}\" config --confinement substrate"));
    assert_eq!(code(&o), 0, "{}", both(&o));
    assert!(confinement_row(&o).contains("substrate") && confinement_row(&o).contains("option --confinement") && !confinement_row(&o).contains("local"), "{}", confinement_row(&o));
}

#[test]
fn no_file_lowers_confinement_the_option_names() {
    // the option wins over every file, found in every way, for a start with and without LOOM_CASE
    let rig = Rig::new("option-wins");
    let named = rig.put("named.toml", "confinement = \"local\"\n");
    rig.put("runbook.toml", "confinement = \"local\"\n");
    for with_case in [false, true] {
        for args in [vec!["config", "--confinement", "substrate"], vec!["config", "--confinement", "substrate", "--config", named.to_str().unwrap()]] {
            let mut c = rig.command(&rig.root, &args);
            if with_case {
                c.env("LOOM_CASE", "a-case");
            }
            let o = c.output().unwrap();
            let row = confinement_row(&o);
            assert!(code(&o) == 0 || with_case, "{args:?}: {}", both(&o));
            if code(&o) == 0 {
                assert!(row.contains("substrate") && row.contains("option --confinement") && !row.contains("local"), "{args:?} (LOOM_CASE {with_case}): {row}");
            }
        }
    }
}

#[test]
fn a_file_found_inside_a_program_of_a_run_has_no_key_that_raises_a_cap_or_moves_the_ledger() {
    // a file the tree holds is refused, by name and with no value, for the keys that state a cap, the spend ledger and the
    // directories runbooks are loaded from, as it is for a level (docs/loom-run.md)
    let rig = Rig::new("caps");
    std::fs::create_dir_all(rig.p("model-books/protocol")).unwrap();
    std::fs::create_dir_all(rig.p("model-books/bindings")).unwrap();
    let bin = super::core::BIN;
    let keys = [
        ("cap_micro_usd", "cap_micro_usd = 777777777\n", "777777777"),
        ("max_llm_calls", "max_llm_calls = 777777\n", "777777"),
        ("max_wall_seconds", "max_wall_seconds = 77777777\n", "77777777"),
        ("spend_ledger", "spend_ledger = \"fresh-ledger.json\"\n", "fresh-ledger"),
        ("runbooks", "runbooks = [\"model-books\"]\n", "model-books"),
    ];
    for (key, text, value) in keys {
        let file = rig.put("runbook.toml", text);
        let o = program_of_a_run(&rig, &rig.root, &format!("\"{bin}\" config"));
        assert_eq!(code(&o), 2, "{key}: a file of the tree states it for a nested runbook: {}", both(&o));
        assert!(err(&o).contains(key) && err(&o).contains("runbook.toml"), "{key}: the refusal does not name the key: {}", err(&o));
        assert!(!both(&o).contains(value), "{key}: a value of the file was printed: {}", both(&o));
        // a person's start, and a file the caller names, read it
        let o = rig.run(&rig.root, &["config"]);
        assert_eq!(code(&o), 0, "{key}: {}", both(&o));
        let o = program_of_a_run(&rig, &rig.root, &format!("\"{bin}\" config --config \"{}\"", file.display()));
        assert_eq!(code(&o), 0, "{key}: {}", both(&o));
    }
}

// ---- a rig stands where no file above it is read ----

#[test]
fn a_rig_fails_loudly_when_a_directory_above_it_holds_a_runbook_toml_or_is_a_run() {
    // the walk climbs from the rig's working directory to `/`: a `runbook.toml` that a person keeps in the temporary
    // directory's parent, or a run directory around the rig, changes every case. The rig refuses to stand there.
    let outer = std::env::temp_dir().join(format!("runbook-ancestor-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&outer);
    let inner = outer.join("tmp");
    std::fs::create_dir_all(&inner).unwrap();
    let say = |r: std::thread::Result<Rig>| -> Option<String> {
        r.err().map(|p| p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned())).unwrap_or_default())
    };
    // control: nothing above holds a file
    assert_eq!(say(std::panic::catch_unwind(|| Rig::new_in(&inner, "ancestor-clean"))), None, "a clean temporary directory is refused");
    let ancestor = outer.join("runbook.toml");
    std::fs::write(&ancestor, "max_wall_seconds = 4242424\n").unwrap();
    let said = say(std::panic::catch_unwind(|| Rig::new_in(&inner, "ancestor")));
    std::fs::remove_file(&ancestor).unwrap();
    assert!(said.as_deref().is_some_and(|s| s.contains("runbook.toml") && s.contains(&outer.display().to_string())), "a rig stood below a runbook.toml: {said:?}");
    // a run directory above
    std::fs::write(outer.join("events.jsonl"), "").unwrap();
    std::fs::create_dir_all(outer.join("work")).unwrap();
    std::fs::create_dir_all(outer.join("bindings")).unwrap();
    let said = say(std::panic::catch_unwind(|| Rig::new_in(&inner, "ancestor-run")));
    let _ = std::fs::remove_dir_all(&outer);
    assert!(said.as_deref().is_some_and(|s| s.contains("run") && s.contains(&outer.display().to_string())), "a rig stood below a run directory: {said:?}");
}
