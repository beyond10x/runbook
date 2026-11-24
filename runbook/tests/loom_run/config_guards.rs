//! Guards of `runbook.toml` (defaults of `runbook run`, runbook directories) beside `config_file.rs`
//! and `runbook_dirs.rs`: what the file can make the binary do, what `runbook config` prints, which
//! trees the walk never reads, and which runbook a name resolves to. Every start of the binary has a
//! private HOME and XDG_CONFIG_HOME and a working directory inside the test's own tree (`Rig`), and
//! reads no file of the host. No model call, no network, no login file.

use super::config_file::Rig;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const FOUR: &str = "confinement = \"local\"\nmax_wall_seconds = 120\ncap_micro_usd = 1000000\nmax_llm_calls = 100\n";

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

fn first_line(o: &Output) -> String {
    out(o).lines().next().unwrap_or("").to_owned()
}

/// Every file below `dir`, as text where it is text.
fn all_text(dir: &Path, into: &mut String) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            all_text(&path, into);
        } else if kind.is_file() {
            if let Ok(text) = std::fs::read_to_string(&path) {
                into.push_str(&format!("\n--- {}\n{text}", path.display()));
            }
        }
    }
}

// ---- what a file of the walk takes without the user naming it ----

/// One dry run; the four things it holds share its start (a start of a dry run costs seconds): the run
/// names the file of the walk it took its values from, before anything starts and in its own record
/// without a path; a bound program gets the environment the documents say; the autonomy of the file does
/// not raise what a settings file sets; the login path of the file is in no record of the run.
#[test]
fn one_dry_run_names_its_file_and_keeps_its_environment_its_autonomy_and_its_login_path() {
    let rig = Rig::new("dry-run-guards");
    rig.fixture();
    let tool = rig.p("fx/bindings/tools/draft-write.sh");
    // the program writes a `runbook.toml` into its own work directory (what a model may do) and runs `runbook config` there
    let script = format!("#!/bin/sh\nenv > \"$PWD/env-dump.txt\"\nprintf 'max_wall_seconds = 1\\n' > \"$PWD/runbook.toml\"\n\"{}\" config > \"$PWD/config-dump.txt\" 2>&1\necho draft >> draft.txt\n", super::core::BIN);
    std::fs::write(&tool, script).unwrap();
    std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
    let login = rig.put("secret-place/login-MARKERPATH.json", "{\"marker\": \"LOGINCONTENT-MARKER\"}\n");
    let file = rig.put("runbook.toml", &format!("{FOUR}autonomy = \"A5\"\nlogin_file = \"{}\"\n[decider]\nkind = \"rule\"\nverdict = \"approve\"\n", login.display()));
    let settings = rig.put("settings.toml", "autonomy = \"A2\"\n");
    std::fs::create_dir_all(rig.p("xdg-run")).unwrap();
    std::fs::set_permissions(rig.p("xdg-run"), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut c = rig.command(&rig.root, &["run", "engine-demo", "--intake", "idea.txt", "--dry", "--run", "envrun", "--settings", &settings.display().to_string()]);
    c.env("XDG_RUNTIME_DIR", rig.p("xdg-run"));
    let o = c.output().unwrap();
    // autonomy A2 (the settings file's) stops the run at the first approval: exit 8, the stop is the held guard
    assert_eq!(code(&o), 8, "{}", err(&o));
    assert!(err(&o).contains("autonomy A2"), "the run did not take the autonomy of the settings file: {}", err(&o));
    // the file is named on standard error, before the run says anything else
    let said = err(&o);
    let named = said.lines().position(|l| l.contains(&file.display().to_string()));
    assert!(named.is_some(), "the run took confinement, wall time and caps from a file it found by itself and does not say which one:\n{said}");
    assert_eq!(named, Some(0), "the file is named after the run said something else:\n{said}");
    // a bound program gets what the documents say it gets: `PATH` and `LOOM_*` (Loom's program contract), not
    // `RUNBOOK_NO_CONFIG` (the runner gives that to the programs it starts itself). A `runbook` it starts in the
    // run's tree is kept from the file by the run-tree rule: the `runbook.toml` it wrote there is not read, the
    // file of the project above `runs/` is, and `runbook config` says which file it skipped.
    let dump = std::fs::read_to_string(rig.p("runs/envrun/work/env-dump.txt")).unwrap();
    let names: Vec<&str> = dump.lines().filter_map(|l| l.split_once('=').map(|(k, _)| k)).collect();
    assert!(names.contains(&"PATH") && names.iter().any(|k| k.starts_with("LOOM_")), "a bound program lacks PATH or LOOM_*; its environment:\n{dump}");
    let others: Vec<&&str> = names.iter().filter(|k| !(k.starts_with("LOOM_") || matches!(**k, "PATH" | "PWD" | "SHLVL" | "_" | "OLDPWD"))).collect();
    assert!(others.is_empty(), "a bound program gets more than PATH and LOOM_*: {others:?}\n{dump}");
    let nested = std::fs::read_to_string(rig.p("runs/envrun/work/config-dump.txt")).unwrap();
    assert!(nested.lines().next().is_some_and(|l| l.contains(&file.display().to_string())), "a runbook started inside the run read another file than the project's:\n{nested}");
    assert!(nested.lines().any(|l| l.starts_with("max_wall_seconds = 120")), "the file a program wrote into its work directory set a value:\n{nested}");
    assert!(nested.lines().any(|l| l.contains("work/runbook.toml") && l.contains("not read")), "the file skipped inside the run is not named:\n{nested}");
    // the record: the settings file wins the autonomy, and the file is named by name and origin, not by path
    let written = std::fs::read_to_string(rig.p("runs/envrun/settings.json")).unwrap();
    let v: serde_json::Value = serde_json::from_str(&written).unwrap();
    assert_eq!(v["autonomy"], "A2", "{written}");
    let note = v["config"].as_str().unwrap_or_else(|| panic!("the start record of the run does not name the file it took values from:\n{written}"));
    assert!(note.contains("runbook.toml") && note.contains("working directory") && !note.contains(&rig.root.display().to_string()), "{note}");
    // the two other files that carry the settings of the run hold the same member, without the path
    let json_lines = |name: &str| -> Vec<serde_json::Value> { std::fs::read_to_string(rig.p(&format!("runs/envrun/{name}"))).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect() };
    let record = json_lines("run-settings.jsonl");
    let started = json_lines("events.jsonl").iter().map(ToString::to_string).find(|e| e.contains("run_started") && e.contains("\"config\""));
    for (name, held) in [("run-settings.jsonl", record.first().and_then(|l| l["config"].as_str()).map(str::to_owned)), ("run_started event", started)] {
        let held = held.unwrap_or_else(|| panic!("{name} does not hold the member config"));
        assert!(held.contains("runbook.toml") && !held.contains(&rig.root.display().to_string()), "{name} holds the path of the file: {held}");
    }
    // the login path is in no file of the run and no line of its output
    // (not the dumps the bound program itself wrote: `runbook config` prints a login path as a path, by design)
    let mut text = String::new();
    all_text(&rig.p("runs/envrun"), &mut text);
    let text: String = text.split("\n--- ").filter(|section| !section.starts_with(&rig.p("runs/envrun/work/").display().to_string()) || !section.lines().next().is_some_and(|h| h.ends_with("-dump.txt"))).collect::<Vec<_>>().join("\n--- ");
    assert!(!text.contains("LOGINCONTENT-MARKER"), "login content reached a record");
    assert!(!text.contains("MARKERPATH"), "the login path reached a record of the run: {}", text.lines().filter(|l| l.contains("MARKERPATH")).take(3).collect::<Vec<_>>().join("\n"));
    assert!(!said.contains("MARKERPATH") && !said.contains("LOGINCONTENT"), "{said}");
}


#[test]
fn a_workspace_of_the_daemon_is_no_source_of_a_file_for_the_walk() {
    // `State::workspace_dir` is `<state>/workspaces/<run id>` and the run's own state `<state>/runs/<run id>`
    // (state.rs): no `events.jsonl`, `work/` and `bindings/` there, and the daemon's bound programs get no
    // RUNBOOK_NO_CONFIG (Loom gives a program PATH and LOOM_* only). The tree below is what a model wrote.
    // The state directory is made by the daemon's own `State::open` (it writes `daemon.lock` and `journal.jsonl`).
    let rig = Rig::new("guards-daemon-workspace");
    drop(runbook::state::State::open(&rig.p("state")).unwrap());
    let workspace = rig.p("state/workspaces/r1");
    std::fs::create_dir_all(workspace.join("repo")).unwrap();
    std::fs::create_dir_all(rig.p("state/runs/r1/tree")).unwrap();
    for planted in ["state/workspaces/r1/runbook.toml", "state/runs/r1/tree/runbook.toml"] {
        rig.put(planted, "confinement = \"local\"\nmax_wall_seconds = 1\n");
    }
    for cwd in [workspace.join("repo"), workspace.clone(), rig.p("state/runs/r1/tree")] {
        let o = rig.run(&cwd, &["config"]);
        assert_eq!(code(&o), 0, "{}", err(&o));
        assert!(first_line(&o).contains("none found"), "a file inside a tree of a run of the daemon was read from {}: {}", cwd.display(), first_line(&o));
    }
}

#[test]
fn the_walk_never_reads_a_file_in_the_directory_of_runs_it_is_started_in() {
    // docs/loom-run.md: "The walk never reads a `runbook.toml` inside a run directory or in the directory of
    // runs that holds it"; the code skips the directory of runs only for a working directory below a run
    let rig = Rig::new("guards-runs-dir-itself");
    rig.put("runbook.toml", "max_wall_seconds = 90\n");
    std::fs::create_dir_all(rig.p("runs/r1/work")).unwrap();
    std::fs::create_dir_all(rig.p("runs/r1/bindings")).unwrap();
    std::fs::write(rig.p("runs/r1/events.jsonl"), "").unwrap();
    let planted = rig.put("runs/runbook.toml", "max_wall_seconds = 1\n");
    let o = rig.run(&rig.p("runs"), &["config"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(!first_line(&o).contains(&planted.display().to_string()), "the file in the directory of runs was read: {}", first_line(&o));
    assert!(out(&o).lines().any(|l| l.contains(&planted.display().to_string()) && l.contains("not read") && l.contains("directory of runs")), "the file skipped in the directory of runs is not named with its reason:\n{}", out(&o));
}

#[test]
fn a_file_that_the_walk_skips_as_inside_a_run_says_so() {
    // a tree that only looks like a run directory: its own runbook.toml is skipped, and the user is told
    // nothing: `config` says "none found"
    let rig = Rig::new("guards-lookalike");
    std::fs::create_dir_all(rig.p("proj/work")).unwrap();
    std::fs::create_dir_all(rig.p("proj/bindings")).unwrap();
    std::fs::write(rig.p("proj/events.jsonl"), "").unwrap();
    let file = rig.put("proj/runbook.toml", FOUR);
    let o = rig.run(&rig.p("proj"), &["config"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let said = format!("{}{}", out(&o), err(&o));
    assert!(said.contains(&file.display().to_string()), "the file of the directory was skipped and the user is not told which file or why:\n{said}");
    assert!(said.lines().any(|l| l.contains(&file.display().to_string()) && l.contains("not read") && l.contains("inside a run directory")), "{said}");
}

// ---- what `runbook config` prints ----

#[test]
fn config_prints_no_value_of_the_form_of_a_token_from_any_source() {
    const FORM: [&str; 2] = ["sk-", "ant-api03-aaaaaaaaaaaaaaaaaaaaaaaa"];
    let form = FORM.concat();
    let rig = Rig::new("guards-config-sources");
    let settings = rig.put("settings.toml", &format!("login_file = \"{form}\"\n"));
    let settings = settings.display().to_string();
    let named = rig.put(&format!("{form}.toml"), "max_wall_seconds = 5\n").display().to_string();
    let runs = [
        ("a member of --settings", rig.command(&rig.root, &["config", "--settings", &settings])),
        ("--run", rig.command(&rig.root, &["config", "--run", &form])),
        ("--runs-dir", rig.command(&rig.root, &["config", "--runs-dir", &form])),
        ("--spend-ledger", rig.command(&rig.root, &["config", "--spend-ledger", &form])),
        ("--config", rig.command(&rig.root, &["config", "--config", &named])),
        ("RUNBOOK_CONFIG", {
            let mut c = rig.command(&rig.root, &["config"]);
            c.env("RUNBOOK_CONFIG", &named);
            c
        }),
    ];
    for (source, mut c) in runs {
        let o = c.output().unwrap();
        assert_eq!(code(&o), 0, "{source}: {}", err(&o));
        let all = format!("{}{}", out(&o), err(&o));
        assert!(!all.contains(&form[..8]), "a value of the form of a key from {source} reached the output of `runbook config`:\n{all}");
    }
    // the key and its origin are printed, the value is a fixed word
    let o = rig.run(&rig.root, &["config", "--settings", &settings]);
    let line = out(&o).lines().find(|l| l.starts_with("login_file = ")).unwrap_or_else(|| panic!("{}", out(&o))).to_owned();
    assert!(line.contains("<a key or token>") && line.contains(&settings), "{line}");
}

// ---- a configured directory and the built-in set ----

fn directory(rig: &Rig, rel: &str, id: &str, protocol: &str) -> PathBuf {
    let dir = rig.p(rel);
    std::fs::create_dir_all(dir.join("protocol")).unwrap();
    std::fs::create_dir_all(dir.join("bindings")).unwrap();
    std::fs::write(dir.join(format!("protocol/{id}.protocol.yaml")), protocol).unwrap();
    std::fs::write(dir.join(format!("bindings/{id}.yaml")), "format: loom.bindings/1\n").unwrap();
    dir
}

#[test]
fn a_configured_runbook_cannot_turn_a_built_in_runbook_into_a_child() {
    let rig = Rig::new("guards-demote");
    rig.fixture();
    let dir = directory(&rig, "books/hide", "hide", "protocol:\n  id: hide\n  revision: 1\nactions:\n  a.open:\n    case_open: {protocol: engine-demo, subject: idea}\n");
    rig.put("runbook.toml", &format!("runbooks = [\"{}\"]\n", dir.display()));
    let o = rig.run(&rig.root, &["list"]);
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    let line = out(&o).lines().find(|l| l.split_whitespace().next() == Some("engine-demo")).unwrap_or_else(|| panic!("{}", out(&o))).to_owned();
    assert_eq!(line.split_whitespace().nth(1), Some("runbook"), "a configured directory made the built-in runbook a child: {line}");
}

#[test]
fn a_directory_that_changes_after_the_daemon_started_never_shadows_a_built_in_name() {
    // `runbook serve` accepts the set once (`runbooks::load`: no name twice, no link out); its engine then
    // looks a name up, per run, in the first directory that has the file. A name that a configured
    // directory gains later is that directory's, and the built-in set's runbook is no longer the one a
    // submit of its name starts.
    use runbook::runtime::Runtime;
    let rig = Rig::new("guards-daemon-shadow");
    let put = |rel: &str, text: &str| {
        let path = rig.p(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    put("home/protocol/alpha.protocol.yaml", "format: protocol/1\nprotocol:\n  id: alpha\n  revision: 3\n");
    put("home/bindings/alpha.yaml", "format: loom.bindings/1\n");
    std::fs::create_dir_all(rig.p("books/protocol")).unwrap();
    std::fs::create_dir_all(rig.p("books/bindings")).unwrap();
    runbook::runbooks::load(&[rig.p("books")], Some(&rig.p("home"))).unwrap();
    let engine = runbook::engine::Engine::new(rig.p("home"), rig.p("state")).with_runbook_dirs(vec![rig.p("books")]);
    // the directory gains the built-in name after the start
    put("books/protocol/alpha.protocol.yaml", "format: protocol/1\nprotocol:\n  id: alpha\n  revision: 9\n");
    put("books/bindings/alpha.yaml", "format: loom.bindings/1\n");
    assert!(runbook::runbooks::load(&[rig.p("books")], Some(&rig.p("home"))).is_err(), "`list` refuses this set");
    // the name is in two directories now: the view lists none of the two (a submit of the name is refused),
    // and in particular not the configured directory's revision
    let rows: Vec<(String, Option<String>)> = engine.runbooks().into_iter().map(|r| (r.name, r.version)).collect();
    assert!(rows.is_empty(), "the daemon's view lists a name that two directories hold, or lets a directory shadow the built-in name: {rows:?}");
    assert!(!engine.knows_runbook("alpha", None), "the daemon accepts a submit of a name that two directories hold");
}

#[test]
fn a_name_a_directory_gained_after_the_start_is_refused_at_the_submit_by_name_and_names_no_path() {
    use runbook::runtime::Runtime;
    let rig = Rig::new("guards-daemon-refuse");
    let put = |rel: &str, text: &str| {
        let path = rig.p(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    for (home, id) in [("home", "alpha"), ("books", "gamma")] {
        put(&format!("{home}/protocol/{id}.protocol.yaml"), &format!("format: protocol/1\nprotocol:\n  id: {id}\n  revision: 3\n"));
        put(&format!("{home}/bindings/{id}.yaml"), "format: loom.bindings/1\n");
    }
    let homes = vec![rig.p("books"), rig.p("home")];
    let engine = runbook::engine::Engine::new(rig.p("home"), rig.p("state")).with_runbook_dirs(vec![rig.p("books")]);
    assert!(runbook::engine::layout::resolve_in(&homes, "alpha", None).is_ok() && engine.knows_runbook("alpha", None));
    assert!(runbook::engine::layout::resolve_in(&homes, "gamma", None).is_ok(), "a name only a configured directory has is refused");
    // the directory gains the built-in name after the start
    put("books/protocol/alpha.protocol.yaml", "format: protocol/1\nprotocol:\n  id: alpha\n  revision: 9\n");
    put("books/bindings/alpha.yaml", "format: loom.bindings/1\n");
    let refused = runbook::engine::layout::resolve_in(&homes, "alpha", None).unwrap_err();
    assert!(refused.contains("alpha") && !refused.contains(&rig.root.display().to_string()) && !refused.contains("books"), "the refusal does not name the runbook, or names a path: {refused}");
    assert!(!engine.knows_runbook("alpha", None), "the daemon accepts a submit of a name that two directories have");
    assert!(engine.knows_runbook("gamma", None), "the other runbooks of the directory are refused too");
}

// ---- guards that hold ----

#[test]
fn the_file_has_no_key_for_the_floor_of_the_daemon() {
    let rig = Rig::new("guards-floor");
    for (i, text) in [
        "min_confinement = \"local\"\n",
        "floor = \"local\"\n",
        "confinement_floor = \"local\"\n",
        "[daemon]\nmin_confinement = \"local\"\n",
        "[serve]\nmin_confinement = \"local\"\n",
        "[serve]\nconfinement = \"local\"\n",
    ]
    .iter()
    .enumerate()
    {
        let file = rig.put(&format!("floor-{i}.toml"), text);
        let o = rig.run(&rig.root, &["config", "--config", &file.display().to_string()]);
        assert_eq!(code(&o), 2, "{text}: {}{}", out(&o), err(&o));
        assert!(err(&o).contains(&file.display().to_string()) && !err(&o).contains("\"local\""), "{text}: {}", err(&o));
        assert!(err(&o).contains("not a key of runbook.toml"), "the key is refused by the file's own key check, not by a reader further down: {text}: {}", err(&o));
    }
    let file = rig.put("floor-serve.toml", "min_confinement = \"local\"\n");
    let o = rig.run(
        &rig.root,
        &["serve", "--config", &file.display().to_string(), "--runtime", "loom", "--principal", "person:test", "--listen", "127.0.0.1:0", "--state-dir", &rig.p("state").display().to_string()],
    );
    assert_eq!(code(&o), 2, "{}{}", out(&o), err(&o));
    assert!(!rig.p("state").exists(), "the daemon wrote its state before it refused the file");
}

#[test]
fn the_members_that_belong_to_one_run_are_no_key_of_the_file() {
    let rig = Rig::new("guards-run-members");
    for (i, text) in [
        "format = \"loom.run-settings/1\"\n",
        "run = \"x\"\n",
        "loom_sha256 = \"0000\"\n",
        "in_process = true\n",
        "replay = \"/somewhere/adapter\"\n",
        "[loom]\nbin = \"/somewhere/b10x-loom\"\n",
    ]
    .iter()
    .enumerate()
    {
        let file = rig.put(&format!("member-{i}.toml"), text);
        let o = rig.run(&rig.root, &["config", "--config", &file.display().to_string()]);
        assert_eq!(code(&o), 2, "{text}: {}{}", out(&o), err(&o));
        assert!(!err(&o).contains("somewhere") && !out(&o).contains("somewhere"), "{text}: {}{}", out(&o), err(&o));
    }
}

#[test]
fn init_follows_no_link_and_overwrites_nothing() {
    let rig = Rig::new("guards-init");
    let target = rig.p("elsewhere/created-by-init");
    std::fs::create_dir_all(rig.p("elsewhere")).unwrap();
    std::os::unix::fs::symlink(&target, rig.p("runbook.toml")).unwrap();
    let o = rig.run(&rig.root, &["init"]);
    assert_eq!(code(&o), 2, "{}{}", out(&o), err(&o));
    assert!(!target.exists(), "init followed a link and created its target");
    // a permissive umask does not make the file writable by others
    std::fs::remove_file(rig.p("runbook.toml")).unwrap();
    let mut c = rig.command(&rig.root, &["init"]);
    unsafe {
        use std::os::unix::process::CommandExt;
        c.pre_exec(|| {
            libc::umask(0);
            Ok(())
        });
    }
    let o = c.output().unwrap();
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert_eq!(std::fs::metadata(rig.p("runbook.toml")).unwrap().permissions().mode() & 0o777, 0o600);
}

fn mkfifo(path: &Path) {
    let status = std::process::Command::new("mkfifo").arg(path).status().unwrap();
    assert!(status.success());
}

/// The output of the command, or a panic when it does not end in 20 seconds (a read of a pipe that
/// nobody writes into).
fn ends(mut c: std::process::Command) -> Output {
    let mut child = c.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let pid = child.id();
    let started = std::time::Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if started.elapsed() > std::time::Duration::from_secs(20) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("process {pid} did not end: a named pipe was read");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[test]
fn a_named_pipe_is_no_file_and_no_runbook_directory_entry() {
    let rig = Rig::new("guards-fifo");
    // as the walk finds it, and as --config names it
    mkfifo(&rig.p("runbook.toml"));
    let o = ends(rig.command(&rig.root, &["config"]));
    assert_eq!(code(&o), 2, "{}{}", out(&o), err(&o));
    assert!(err(&o).contains("runbook.toml") && err(&o).contains("not a regular file"), "{}", err(&o));
    let o = ends(rig.command(&rig.root, &["config", "--config", &rig.p("runbook.toml").display().to_string()]));
    assert_eq!(code(&o), 2, "{}{}", out(&o), err(&o));
    // in a configured directory: an entry, and a link to one
    std::fs::remove_file(rig.p("runbook.toml")).unwrap();
    rig.fixture();
    let dir = directory(&rig, "books/p", "p", "protocol:\n  id: p\n  revision: 1\n");
    rig.put("runbook.toml", &format!("runbooks = [\"{}\"]\n", dir.display()));
    mkfifo(&dir.join("bindings/pipe.yaml"));
    let o = ends(rig.command(&rig.root, &["list"]));
    assert_eq!(code(&o), 1, "{}{}", out(&o), err(&o));
    assert!(err(&o).contains("pipe.yaml") && err(&o).contains("not a regular file"), "{}", err(&o));
    std::fs::remove_file(dir.join("bindings/pipe.yaml")).unwrap();
    mkfifo(&dir.join("pipe"));
    std::os::unix::fs::symlink("../pipe", dir.join("bindings/pipe.yaml")).unwrap();
    let o = ends(rig.command(&rig.root, &["check"]));
    assert_ne!(code(&o), 0, "a link to a pipe inside the directory was accepted:\n{}{}", out(&o), err(&o));
    // a link to a device
    std::fs::remove_file(dir.join("bindings/pipe.yaml")).unwrap();
    std::os::unix::fs::symlink("/dev/null", dir.join("bindings/null.yaml")).unwrap();
    let o = ends(rig.command(&rig.root, &["list"]));
    assert_eq!(code(&o), 1, "{}{}", out(&o), err(&o));
    assert!(err(&o).contains("null.yaml") && err(&o).contains("link"), "{}", err(&o));
}


// ---- the walk: trees a model writes into, and what a start says about the file it took ----

/// An invented body that no real token has.
const BODY: &str = "ZZinventedbodyZZ0123456789";

/// A run directory as `runbook run` writes it: `events.jsonl`, `work/`, `bindings/`.
fn run_dir(rig: &Rig, rel: &str) {
    std::fs::create_dir_all(rig.p(&format!("{rel}/work/repo"))).unwrap();
    std::fs::create_dir_all(rig.p(&format!("{rel}/bindings"))).unwrap();
    rig.put(&format!("{rel}/events.jsonl"), "");
}

fn have_bwrap() -> bool {
    Command::new("bwrap").arg("--version").stdout(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

/// `runbook <args>` with the work directory of the run bound at `/mnt` and started there: the same directory
/// under a second path, with none of the run's files above it (what confinement `substrate` does at
/// `/workspace`).
fn bound_at_mnt(rig: &Rig, work: &str, args: &[&str]) -> Output {
    Command::new("bwrap")
        .args(["--ro-bind", "/", "/", "--dev", "/dev", "--proc", "/proc", "--bind"])
        .arg(rig.p(work))
        .args(["/mnt", "--chdir", "/mnt", super::core::BIN])
        .args(args)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", rig.p("home"))
        .env("XDG_CONFIG_HOME", rig.p("xdg"))
        .env("LOOM_CASE", "a-case")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn a_work_directory_bound_at_another_path_reads_its_own_file_by_the_keys_of_any_file_and_the_floor_stays() {
    // docs/loom-run.md: under a confinement that binds the work directory at another path (`substrate`, at
    // `/workspace`) the run directory is not above the working directory: a `runbook` started there reads the
    // work tree's own `runbook.toml` as a file of the walk. `/mnt` stands in for `/workspace`. Its keys are
    // checked like any file's: it has no key for a floor or a level, and a key that would lower one is refused
    // by name, with no value printed.
    if !have_bwrap() {
        eprintln!("no bwrap on this host: the case does not run");
        return;
    }
    let rig = Rig::new("bound-work");
    run_dir(&rig, "runs/r1");
    rig.put("runs/r1/work/runbook.toml", "max_wall_seconds = 1\n");
    let o = bound_at_mnt(&rig, "runs/r1/work", &["config"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(first_line(&o).contains("/mnt/runbook.toml") && first_line(&o).contains("found in the working directory or a parent"), "{}", first_line(&o));
    assert!(out(&o).lines().any(|l| l.starts_with("max_wall_seconds = 1 ")), "{}", out(&o));
    for lowering in ["min_confinement = \"local\"\n", "[serve]\nmin_confinement = \"local\"\n", "floor = \"local\"\n", "confinement = \"local\"\n", "autonomy = \"A5\"\n"] {
        rig.put("runs/r1/work/runbook.toml", lowering);
        let o = bound_at_mnt(&rig, "runs/r1/work", &["config"]);
        assert_eq!(code(&o), 2, "{lowering}: {}{}", out(&o), err(&o));
        assert!(err(&o).contains("/mnt/runbook.toml") && err(&o).contains("not a key of runbook.toml"), "{lowering}: {}", err(&o));
        assert!(!err(&o).contains("\"local\"") && !out(&o).contains("local"), "{lowering}: a value of the file was printed: {}{}", out(&o), err(&o));
    }
}

#[test]
fn a_link_into_a_run_tree_is_no_way_around_the_skip() {
    let rig = Rig::new("link-into-run");
    run_dir(&rig, "runs/r1");
    let planted = rig.put("runs/r1/work/repo/runbook.toml", "max_wall_seconds = 4242424\n");
    std::os::unix::fs::symlink(rig.p("runs/r1/work/repo"), rig.p("view")).unwrap();
    let o = rig.run(&rig.p("view"), &["config"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(first_line(&o).contains("none found"), "a link into a run's tree made its file a source: {}", first_line(&o));
    // the skipped file is named by path and reason; nothing of its content is printed
    assert!(out(&o).lines().any(|l| l.contains(&planted.display().to_string()) && l.contains("not read")), "{}", out(&o));
    assert!(!out(&o).contains("4242424") && !err(&o).contains("4242424"), "a value of a skipped file was printed");
}

#[test]
fn the_users_file_inside_a_run_tree_is_not_read() {
    let rig = Rig::new("userdir-in-run");
    run_dir(&rig, "runs/r1");
    let planted = rig.put("runs/r1/work/cfg/runbook/runbook.toml", "max_wall_seconds = 1\n");
    let mut c = rig.command(&rig.root, &["config"]);
    c.env("XDG_CONFIG_HOME", rig.p("runs/r1/work/cfg"));
    let o = c.output().unwrap();
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(!first_line(&o).contains(&planted.display().to_string()), "the user's file inside a run's tree was read: {}", first_line(&o));
}

#[test]
fn a_run_names_the_file_for_each_way_it_was_found_and_names_none_that_took_no_value() {
    let rig = Rig::new("run-says");
    rig.fixture();
    let says = |o: &Output| err(o).lines().find(|l| l.contains("values from runbook.toml")).map(str::to_owned);
    let idea = rig.put("idea.txt", "an idea\n").display().to_string();
    let args = ["run", "engine-demo", "--intake", idea.as_str(), "--dry"];
    // the four ways; the run is refused for the confinement the file does not state, after it said where its values come from
    let named = rig.put("named.toml", "max_wall_seconds = 90\n");
    let user = rig.put("xdg/runbook/runbook.toml", "max_wall_seconds = 90\n");
    let walked = rig.put("sub/runbook.toml", "max_wall_seconds = 90\n");
    let mut by_option = rig.command(&rig.root, &args);
    by_option.args(["--config", &named.display().to_string()]);
    let mut by_variable = rig.command(&rig.root, &args);
    by_variable.env("RUNBOOK_CONFIG", &named);
    let mut by_user_dir = rig.command(&rig.root, &args);
    let mut by_walk = rig.command(&rig.p("sub"), &args);
    for (c, file, via) in [(&mut by_option, &named, "--config"), (&mut by_variable, &named, "RUNBOOK_CONFIG"), (&mut by_user_dir, &user, "found in the user's configuration directory"), (&mut by_walk, &walked, "found in the working directory or a parent")] {
        let o = c.output().unwrap();
        assert_eq!(code(&o), 64, "{via}: {}", err(&o));
        let line = says(&o).unwrap_or_else(|| panic!("{via}: the run does not name its file: {}", err(&o)));
        assert_eq!(line, format!("runbook run: values from runbook.toml {} ({via})", file.display()));
        assert_eq!(err(&o).lines().next(), Some(line.as_str()), "{via}: the file is not the first thing the run says");
        assert!(!err(&o).contains("= 90"), "a value of the file was printed: {}", err(&o));
    }
    // a file that states no key took no value, and with no file nothing is said
    rig.put("sub/runbook.toml", "runbooks = []\n");
    let o = rig.run(&rig.p("sub"), &args);
    assert!(says(&o).is_none(), "a file that stated nothing is named: {}", err(&o));
    std::fs::remove_file(rig.p("sub/runbook.toml")).unwrap();
    std::fs::remove_file(rig.p("xdg/runbook/runbook.toml")).unwrap();
    let o = rig.run(&rig.p("sub"), &args);
    assert!(says(&o).is_none(), "{}", err(&o));
}

#[test]
fn a_file_in_a_directory_others_can_write_passes_as_the_documents_say() {
    let rig = Rig::new("dirmode");
    let file = rig.put("open/runbook.toml", "max_wall_seconds = 90\n");
    std::fs::set_permissions(rig.p("open"), std::fs::Permissions::from_mode(0o777)).unwrap();
    let o = rig.run(&rig.p("open"), &["config"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(first_line(&o).contains(&file.display().to_string()), "{}", first_line(&o));
    std::fs::set_permissions(rig.p("open"), std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn a_file_that_states_only_a_settings_member_is_named_by_a_run() {
    let rig = Rig::new("member-only");
    rig.fixture();
    let idea = rig.put("idea.txt", "an idea\n").display().to_string();
    let file = rig.put("runbook.toml", "autonomy = \"A5\"\n");
    let o = rig.run(&rig.root, &["run", "engine-demo", "--intake", idea.as_str(), "--dry"]);
    assert!(err(&o).contains(&format!("values from runbook.toml {}", file.display())), "a file that took the autonomy of a run is not named: {}", err(&o));
}

// ---- guards for the shape of the skip: what a model can build inside its own tree ----

#[test]
fn a_lookalike_run_directory_inside_a_run_hides_nothing_above_it() {
    let rig = Rig::new("nest-run");
    run_dir(&rig, "runs/r1");
    run_dir(&rig, "runs/r1/work/repo/fake");
    let planted = rig.put("runs/r1/work/runbook.toml", "max_wall_seconds = 4242424\n");
    let o = rig.run(&rig.p("runs/r1/work/repo/fake"), &["config"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(first_line(&o).contains("none found"), "a run directory a model built inside its work tree made the file above it a source: {}", first_line(&o));
    assert!(!first_line(&o).contains(&planted.display().to_string()), "{}", first_line(&o));
}

#[test]
fn a_lookalike_state_directory_inside_a_workspace_hides_nothing_above_it() {
    let rig = Rig::new("nest-state");
    for state in ["st", "st/workspaces/w1/repo/fake"] {
        rig.put(&format!("{state}/daemon.lock"), "");
        rig.put(&format!("{state}/journal.jsonl"), "");
    }
    std::fs::create_dir_all(rig.p("st/workspaces/w1/repo/fake/workspaces/a")).unwrap();
    let planted = rig.put("st/workspaces/w1/repo/runbook.toml", "max_wall_seconds = 4242424\n");
    let o = rig.run(&rig.p("st/workspaces/w1/repo/fake/workspaces/a"), &["config"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(first_line(&o).contains("none found"), "a state directory a model built inside its workspace made the file above it a source: {}", first_line(&o));
    assert!(!first_line(&o).contains(&planted.display().to_string()), "{}", first_line(&o));
}

#[test]
fn the_workspaces_and_runs_directories_of_the_daemon_are_no_source_either() {
    let rig = Rig::new("state-top");
    rig.put("st/daemon.lock", "");
    rig.put("st/journal.jsonl", "");
    for kind in ["workspaces", "runs"] {
        std::fs::create_dir_all(rig.p(&format!("st/{kind}/w1"))).unwrap();
        let planted = rig.put(&format!("st/{kind}/runbook.toml"), "max_wall_seconds = 4242424\n");
        let o = rig.run(&rig.p(&format!("st/{kind}/w1")), &["config"]);
        assert_eq!(code(&o), 0, "{kind}: {}", err(&o));
        assert!(!first_line(&o).contains(&planted.display().to_string()), "the {kind} directory of the daemon was a source: {}", first_line(&o));
    }
}

#[test]
fn a_directory_with_only_a_lock_file_is_no_state_directory_of_the_daemon() {
    let rig = Rig::new("state-half");
    rig.put("st/daemon.lock", "");
    std::fs::create_dir_all(rig.p("st/workspaces/w1")).unwrap();
    let planted = rig.put("st/workspaces/w1/runbook.toml", "max_wall_seconds = 90\n");
    let o = rig.run(&rig.p("st/workspaces/w1"), &["config"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(first_line(&o).contains(&planted.display().to_string()), "a directory that is not the state directory of the daemon was skipped: {}", first_line(&o));
}

#[test]
fn the_skipped_file_says_which_rule_skipped_it() {
    let rig = Rig::new("says-why");
    run_dir(&rig, "runs/r1");
    let planted = rig.put("runs/runbook.toml", "max_wall_seconds = 4242424\n");
    let o = rig.run(&rig.p("runs/r1/work"), &["config"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let line = out(&o).lines().find(|l| l.contains(&planted.display().to_string())).map(str::to_owned).unwrap_or_default();
    assert!(line.contains("the directory of runs that holds the run directory it is in"), "the reason of the skip is not the one that holds: {line:?}");
}

#[test]
fn a_directory_of_runs_is_known_by_any_of_its_entries_and_not_by_the_first_one_alone() {
    // the directory of runs the walk is started in holds a run directory among many other entries
    let rig = Rig::new("runs-many");
    rig.put("runbook.toml", "max_wall_seconds = 90\n");
    for i in 0..300 {
        std::fs::create_dir_all(rig.p(&format!("runs/d{i:03}"))).unwrap();
    }
    run_dir(&rig, "runs/r-real");
    let planted = rig.put("runs/runbook.toml", "max_wall_seconds = 1\n");
    let o = rig.run(&rig.p("runs"), &["config"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(!first_line(&o).contains(&planted.display().to_string()), "the file in a directory of runs with many entries was read: {}", first_line(&o));
    assert!(out(&o).lines().any(|l| l.contains(&planted.display().to_string()) && l.contains("not read")), "{}", out(&o));
}

#[test]
fn a_file_named_by_the_variable_leaves_no_file_of_the_walk_listed_as_skipped() {
    let rig = Rig::new("variable-no-skips");
    run_dir(&rig, "runs/r1");
    let planted = rig.put("runs/r1/work/runbook.toml", "max_wall_seconds = 4242424\n");
    let named = rig.put("named.toml", "max_wall_seconds = 90\n");
    let cwd = rig.p("runs/r1/work");
    // without the variable the walk lists the file it did not read
    let o = rig.run(&cwd, &["config"]);
    assert!(out(&o).lines().any(|l| l.contains("skipped") && l.contains(&planted.display().to_string())), "{}", out(&o));
    // with it the walk does not run: nothing is listed
    let mut c = rig.command(&cwd, &["config"]);
    c.env("RUNBOOK_CONFIG", &named);
    let o = c.output().unwrap();
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(first_line(&o).contains(&named.display().to_string()) && first_line(&o).contains("RUNBOOK_CONFIG"), "{}", first_line(&o));
    assert!(!out(&o).contains("skipped"), "a file of the walk is listed although the variable named the file: {}", out(&o));
}

// ---- the daemon: what it says about its file, the view and the submit ----

fn with_books(rig: &Rig) -> PathBuf {
    rig.fixture();
    let dir = rig.p("books/a");
    std::fs::create_dir_all(dir.join("protocol")).unwrap();
    std::fs::create_dir_all(dir.join("bindings")).unwrap();
    std::fs::write(dir.join("protocol/example.protocol.yaml"), "protocol:\n  id: example\n  revision: 1\n").unwrap();
    std::fs::write(dir.join("bindings/example.yaml"), "format: loom.bindings/1\n").unwrap();
    rig.put("runbook.toml", &format!("runbooks = [\"{}\"]\n", dir.display()));
    dir
}

#[test]
fn a_daemon_started_below_a_file_of_the_walk_names_the_file_it_took_its_directories_from() {
    use std::io::{BufRead, BufReader};
    use std::sync::mpsc;
    let rig = Rig::new("serve-says");
    with_books(&rig);
    let file = rig.p("runbook.toml");
    let mut c = rig.command(&rig.root, &["serve", "--runtime", "loom", "--principal", "person:test", "--listen", "127.0.0.1:0", "--state-dir", &rig.p("state").display().to_string(), "--min-confinement", "local"]);
    let mut child = c.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let (tx, rx) = mpsc::channel::<String>();
    let out_pipe = child.stdout.take().unwrap();
    let err_pipe = child.stderr.take().unwrap();
    std::thread::spawn(move || BufReader::new(out_pipe).lines().map_while(Result::ok).for_each(|l| drop(tx.send(l))));
    let errors = std::thread::spawn(move || BufReader::new(err_pipe).lines().map_while(Result::ok).collect::<Vec<String>>());
    let mut out = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while let Ok(l) = rx.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now())) {
        let listening = l.contains("listening");
        out.push(l);
        if listening {
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    // One pipe keeps its order: the line about the file comes before the line the daemon writes on
    // standard error just before it prints that it listens.
    let lines = errors.join().unwrap();
    assert!(out.iter().any(|l| l.contains("listening")), "the daemon did not start: {out:?} {lines:?}");
    let said = lines.iter().position(|l| l.contains(&file.display().to_string())).unwrap_or_else(|| panic!("the daemon took its runbook directories from a file it found by itself and does not say which one: {lines:?}"));
    let starts = lines.iter().position(|l| l.contains("this daemon runs")).unwrap_or_else(|| panic!("the daemon wrote no line about its runtime: {lines:?}"));
    assert!(said < starts, "the daemon names its file after it starts: {lines:?}");
    assert_eq!(lines[said], format!("runbook serve: values from runbook.toml {} (found in the working directory or a parent)", file.display()));
}

#[test]
fn the_view_lists_no_runbook_the_same_daemon_refuses_to_start() {
    // runtime.rs:130-133: the runbooks of `runbooks()` are each a name `knows_runbook` accepts, and
    // docs/loom-run.md says the view no longer lists a name two directories hold
    use runbook::runtime::Runtime;
    let rig = Rig::new("view-two-homes");
    let put = |rel: &str, text: &str| {
        let path = rig.p(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    put("home/protocol/alpha.protocol.yaml", "format: protocol/1\nprotocol:\n  id: alpha\n  revision: 3\n");
    put("home/bindings/alpha.yaml", "format: loom.bindings/1\n");
    put("home/protocol/beta.protocol.yaml", "format: protocol/1\nprotocol:\n  id: beta\n  revision: 2\n");
    put("home/bindings/beta.yaml", "format: loom.bindings/1\n");
    std::fs::create_dir_all(rig.p("books/protocol")).unwrap();
    std::fs::create_dir_all(rig.p("books/bindings")).unwrap();
    let engine = runbook::engine::Engine::new(rig.p("home"), rig.p("state")).with_runbook_dirs(vec![rig.p("books")]);
    put("books/protocol/alpha.protocol.yaml", "format: protocol/1\nprotocol:\n  id: alpha\n  revision: 9\n");
    put("books/bindings/alpha.yaml", "format: loom.bindings/1\n");
    put("books/protocol/gamma.protocol.yaml", "format: protocol/1\nprotocol:\n  id: gamma\n  revision: 1\n");
    put("books/bindings/gamma.yaml", "format: loom.bindings/1\n");
    let listed: Vec<(String, Option<String>)> = engine.runbooks().into_iter().map(|r| (r.name, r.version)).collect();
    let refused: Vec<&(String, Option<String>)> = listed.iter().filter(|(n, _)| !engine.knows_runbook(n, None)).collect();
    assert!(refused.is_empty(), "the view lists {refused:?}, and a submit of it is refused (docs/loom-run.md says the view no longer lists it)");
    // the names the daemon accepts are all listed, each once, with the revision it would start
    let mut names: Vec<(String, Option<String>)> = listed;
    names.sort();
    assert_eq!(names, [("beta".to_string(), Some("2".to_string())), ("gamma".to_string(), Some("1".to_string()))]);
}

#[test]
fn one_directory_named_under_two_paths_is_one_home_for_the_daemon() {
    use runbook::runtime::Runtime;
    let rig = Rig::new("one-home-two-paths");
    let put = |rel: &str, text: &str| {
        let path = rig.p(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    put("home/protocol/alpha.protocol.yaml", "format: protocol/1\nprotocol:\n  id: alpha\n  revision: 3\n");
    put("home/bindings/alpha.yaml", "format: loom.bindings/1\n");
    put("books/protocol/gamma.protocol.yaml", "format: protocol/1\nprotocol:\n  id: gamma\n  revision: 1\n");
    put("books/bindings/gamma.yaml", "format: loom.bindings/1\n");
    std::os::unix::fs::symlink(rig.p("books"), rig.p("books-again")).unwrap();
    let homes = vec![rig.p("books"), rig.p("books-again"), rig.p("home")];
    assert_eq!(runbook::runbooks::locate(&homes, "gamma"), Ok(Some(rig.p("books"))), "one directory under two paths was counted as two homes");
    let engine = runbook::engine::Engine::new(rig.p("home"), rig.p("state")).with_runbook_dirs(vec![rig.p("books"), rig.p("books-again")]);
    assert!(engine.knows_runbook("gamma", None), "a name of a directory that is listed twice is refused");
    let mut names: Vec<String> = engine.runbooks().into_iter().map(|r| r.name).collect();
    names.sort();
    assert_eq!(names, ["alpha", "gamma"]);
}

// ---- what `runbook config` prints: a token in any shape a member of the settings has ----

fn config_of(rig: &Rig, settings_text: &str) -> String {
    let settings = rig.put("settings-shape.toml", settings_text);
    let o = rig.run(&rig.root, &["config", "--settings", &settings.display().to_string()]);
    format!("{}{}", out(&o), err(&o))
}

#[test]
fn config_prints_no_body_of_a_token_in_any_shape_a_settings_member_has() {
    let aws = ["AKIA", "ZZINVENTEDBODY01"].concat();
    let google = ["AIza", "ZZinventedbodyZZ0123456789abcdefghi"].concat();
    assert_eq!(google.len(), 39, "the invented key must have the length of the form");
    let shapes: Vec<(&str, String)> = vec![
        ("a vendor prefix in a table member", format!("[models]\nx = \"{}\"\n", ["ghp", "_", BODY].concat())),
        ("a web token", format!("autonomy = \"{}\"\n", ["eyJ", BODY, ".", BODY, ".", BODY].concat())),
        ("a token in an array", format!("review = [\"a\", \"{}\"]\n", ["xoxb", "-1-", BODY].concat())),
        ("a private key block", format!("[decider]\nkind = \"{}\"\n", ["-----BEGIN", " PRIVATE KEY-----\\n", BODY, "\\n-----END PRIVATE KEY-----"].concat())),
        ("a token as a table key", format!("[{}]\nx = 1\n", ["sk-", "ant-api03-", BODY].concat())),
        ("a token as a key of a member table", format!("[models]\n{} = 1\n", ["gh", "p_", BODY].concat())),
        ("a token as a table inside a member table", format!("[models.{}]\nmodel = \"m\"\n", ["gh", "p_", BODY].concat())),
        ("a token as a key two tables down", format!("[models.writer.{}]\nx = 1\n", ["gh", "p_", BODY].concat())),
        ("a token after other words", format!("adapter = \"curl -H x {}\"\n", ["gh", "p_", BODY].concat())),
        ("a token after a slash", format!("adapter = \"/usr/bin/{}\"\n", ["gl", "pat-", BODY].concat())),
        ("a token after a colon", format!("adapter = \"x:{}\"\n", ["gh", "p_", BODY].concat())),
        ("an aws key", format!("adapter = \"{aws}\"\n")),
        ("a google key", format!("adapter = \"{google}\"\n")),
        ("a generic key", format!("adapter = \"{}\"\n", ["sk", "-", BODY].concat())),
        ("a bearer value", format!("adapter = \"{}\"\n", ["Bear", "er ", BODY].concat())),
        ("a bearer value after other words", format!("adapter = \"Authorization: {}\"\n", ["Bear", "er ", BODY].concat())),
        ("a bearer value in capitals after a quote", format!("adapter = \"curl -H 'Authorization: {}'\"\n", ["BEAR", "ER ", BODY].concat())),
    ];
    let mut leaked = Vec::new();
    for (name, text) in &shapes {
        let rig = Rig::new("shapes");
        let shown = config_of(&rig, text);
        if shown.contains(BODY) || shown.contains(&aws) || shown.contains(&google) {
            leaked.push(format!("{name}: {}", shown.lines().find(|l| l.contains(BODY) || l.contains(&aws) || l.contains(&google)).unwrap_or("")));
        }
    }
    assert!(leaked.is_empty(), "runbook config printed a token in a shape of a settings member: {leaked:#?}");
}

#[test]
fn config_hides_a_bearer_value_that_follows_other_words() {
    let rig = Rig::new("bearer");
    let shown = config_of(&rig, &format!("adapter = \"Authorization: {}\"\n", ["Bear", "er ", BODY].concat()));
    assert!(!shown.contains(BODY), "a bearer value after other words reached the output of `runbook config`: {shown}");
    // the key and its origin stay, the value is the fixed word
    assert!(shown.lines().any(|l| l.starts_with("adapter = ") && l.contains("<a key or token>")), "{shown}");
}

#[test]
fn config_hides_a_token_after_a_slash_a_colon_and_in_an_array() {
    let shapes: Vec<(&str, String)> = vec![
        ("a token in an array", format!("review = [\"a\", \"{}\"]\n", ["xoxb", "-1-", BODY].concat())),
        ("a token after a slash", format!("adapter = \"/usr/bin/{}\"\n", ["gl", "pat-", BODY].concat())),
        ("a token after a colon", format!("adapter = \"x:{}\"\n", ["gh", "p_", BODY].concat())),
        ("a token after other words", format!("adapter = \"curl -H x {}\"\n", ["gh", "p_", BODY].concat())),
        ("a bearer value", format!("adapter = \"{}\"\n", ["Bear", "er ", BODY].concat())),
    ];
    let mut leaked = Vec::new();
    for (name, text) in &shapes {
        let rig = Rig::new("shapes-ok");
        let shown = config_of(&rig, text);
        if shown.contains(BODY) {
            leaked.push((*name).to_owned());
        }
    }
    assert!(leaked.is_empty(), "runbook config printed the body of a token in: {leaked:?}");
}

#[test]
fn config_refuses_a_settings_file_that_is_not_toml_without_a_part_of_a_value() {
    let rig = Rig::new("nottoml");
    for text in [
        format!("adapter = {}\n", ["sk", "-ant-api03-", BODY].concat()),
        format!("adapter = \"{}\n", ["gh", "p_", BODY].concat()),
        format!("{} = \n", ["gh", "p_", BODY].concat()),
        format!("[models\nx = \"{}\"\n", ["sk", "-ant-api03-", BODY].concat()),
    ] {
        let shown = config_of(&rig, &text);
        assert!(!shown.contains(BODY), "a refusal of a settings file that is not TOML printed a part of a value: {shown}");
    }
}

// ---- what `runbook config` and the refusals print: a token after a name and an equals sign, key paths, word forms ----

fn both(o: &Output) -> String {
    format!("{}{}", out(o), err(o))
}

/// A TOML basic string with the characters that need an escape escaped.
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

/// The output of `runbook config --settings <text>` (stdout and stderr), in the rig's own directory.
fn config_of_settings(rig: &Rig, name: &str, text: &str) -> String {
    let settings = rig.put(name, text);
    both(&rig.run(&rig.root, &["config", "--settings", &settings.display().to_string()]))
}

/// The output of `runbook config` of a `runbook.toml` with the text, found by the walk.
fn config_of_file(rig: &Rig, text: &str) -> String {
    rig.put("runbook.toml", text);
    both(&rig.run(&rig.root, &["config"]))
}

// ---- F24: a token that follows a name and an equals sign ----

#[test]
fn config_hides_a_token_that_follows_a_name_and_an_equals_sign() {
    let aws = ["AKIA", "ZZINVENTEDBODY01"].concat();
    let jwt = ["eyJ", BODY, ".", BODY, ".", BODY].concat();
    let words: Vec<(&str, String)> = vec![
        ("an environment assignment of a vendor token", format!("GH_TOKEN={} runner", ["gh", "p_", BODY].concat())),
        ("an option with a vendor token", format!("runner --token={}", ["gl", "pat-", BODY].concat())),
        ("an assignment of an aws key", format!("AWS_ACCESS_KEY_ID={aws} runner")),
        ("an assignment of a web token", format!("API_TOKEN={jwt}")),
        ("an assignment of a bearer value", format!("AUTH=Bearer {BODY}")),
        ("a vendor token after a dot", format!("x.{}", ["gh", "p_", BODY].concat())),
    ];
    // the string members the settings file has, and the members `runbook.toml` takes (it refuses an adapter
    // that is no adapter name)
    let members: Vec<(&str, bool, Box<dyn Fn(&str) -> String>)> = vec![
        ("adapter", false, Box::new(|v| format!("adapter = {}\n", basic(v)))),
        ("a model", true, Box::new(|v| format!("[models.writer]\nmodel = {}\n", basic(v)))),
        ("a path", true, Box::new(|v| format!("runs_dir = {}\n", basic(&format!("/srv/{v}/runs"))))),
    ];
    let mut leaked = Vec::new();
    for (name, v) in &words {
        for (member, in_file, make) in &members {
            let text = make(v);
            let mut sources = vec![("--settings", config_of_settings(&Rig::new("assign"), "settings-shape.toml", &text))];
            if *in_file {
                sources.push(("runbook.toml", config_of_file(&Rig::new("assign-file"), &text)));
            }
            for (source, shown) in sources {
                if shown.contains(BODY) || shown.contains(&aws) {
                    leaked.push(format!("{name} in {member} via {source}: {}", shown.lines().find(|l| l.contains(BODY) || l.contains(&aws)).unwrap_or("")));
                }
            }
        }
    }
    assert!(leaked.is_empty(), "runbook config printed a token that follows a name and an equals sign: {leaked:#?}");
}

// ---- F26: a refusal that names the key path of a member ----

#[test]
fn a_refusal_that_names_the_key_of_a_member_names_no_key_in_the_form_of_a_token() {
    // a date is no value of any setting: the refusal names the key path of the member and no value
    let shapes: Vec<(&str, String)> = vec![
        ("a vendor token as a key", format!("[models.{}]\nx = 1979-05-27\n", ["gh", "p_", BODY].concat())),
        ("a bearer value as a quoted key", format!("[models.{}]\nx = 1979-05-27\n", basic(&["Bear", "er ", BODY].concat()))),
        ("a vendor token as a top level key", format!("{} = 1979-05-27\n", ["gh", "p_", BODY].concat())),
    ];
    let mut leaked = Vec::new();
    for (name, text) in &shapes {
        let rig = Rig::new("refuse-key");
        for (source, shown) in [("--settings", config_of_settings(&rig, "settings-shape.toml", text)), ("runbook.toml", config_of_file(&Rig::new("refuse-key-file"), text))] {
            if shown.contains(BODY) {
                leaked.push(format!("{name} via {source}: {}", shown.lines().find(|l| l.contains(BODY)).unwrap_or("")));
            }
        }
    }
    assert!(leaked.is_empty(), "a refusal printed a key in the form of a token: {leaked:#?}");
}

// ---- item 4: the word forms and key forms that hold ----

#[test]
fn config_hides_a_secret_in_every_key_form_and_in_the_word_forms_of_a_bearer_value() {
    let vendor = ["gh", "p_", BODY].concat();
    let bearer = ["Bear", "er"].concat();
    // a secret as a value or a key, in the words of the forms the round claims to hide
    let words: Vec<(&str, String)> = vec![
        ("a bearer value", format!("{bearer} {BODY}")),
        ("bearer in lower case", format!("{} {BODY}", bearer.to_lowercase())),
        ("bearer in capitals", format!("{} {BODY}", bearer.to_uppercase())),
        ("a header", format!("Authorization: {bearer} {BODY}")),
        ("a header without a space after the colon", format!("Authorization:{bearer} {BODY}")),
        ("a colon after the word", format!("{bearer}:  {BODY}")),
        ("a tab after the word", format!("{bearer}\t{BODY}")),
        ("a new line after the word", format!("{bearer}\n{BODY}")),
        ("a new line and an indent after the word", format!("Authorization: {bearer}\n    {BODY}\n")),
        ("a quoted bearer value", format!("curl -H 'Authorization: {bearer} {BODY}'")),
        ("a quote between the word and the value", format!("{bearer} \"{BODY}\"")),
        ("a bearer value with a slash in it", format!("{bearer} abc/{BODY}")),
        ("a bearer value with a colon in it", format!("Authorization: {bearer} abc:{BODY}")),
        ("a vendor token", vendor.clone()),
        ("a vendor token after a colon", format!("x:{vendor}")),
        ("a vendor token after a slash", format!("/usr/bin/{vendor}")),
        ("a vendor token after words", format!("curl -H x {vendor}")),
        ("a vendor token in quotes", format!("'{vendor}'")),
        ("a vendor token after a bearer word", format!("{bearer} {vendor}")),
    ];
    let value_forms: Vec<(&str, Box<dyn Fn(&str) -> String>)> = vec![
        ("a string member", Box::new(|v| format!("adapter = {}\n", basic(v)))),
        ("a member of a table", Box::new(|v| format!("[models.writer]\nmodel = {}\n", basic(v)))),
        ("a dotted key", Box::new(|v| format!("models.writer.model = {}\n", basic(v)))),
        ("an inline table", Box::new(|v| format!("models = {{ writer = {{ model = {} }} }}\n", basic(v)))),
        ("an array", Box::new(|v| format!("review = [\"a\", {}]\n", basic(v)))),
        ("an array of inline tables", Box::new(|v| format!("review = [{{ x = {} }}]\n", basic(v)))),
        ("a multi-line string", Box::new(|v| format!("adapter = \"\"\"\n{v}\"\"\"\n"))),
    ];
    let key_forms: Vec<(&str, Box<dyn Fn(&str) -> String>)> = vec![
        ("a table inside a member table", Box::new(|v| format!("[models.{}]\nx = 1\n", basic(v)))),
        ("a key of a member table", Box::new(|v| format!("[models]\n{} = 1\n", basic(v)))),
        ("a dotted key", Box::new(|v| format!("models.writer.{} = 1\n", basic(v)))),
        ("an inline table", Box::new(|v| format!("models = {{ {} = 1 }}\n", basic(v)))),
        ("a key in an array of inline tables", Box::new(|v| format!("review = [{{ {} = 1 }}]\n", basic(v)))),
        ("a table of an array of tables", Box::new(|v| format!("[[review]]\n{} = 1\n", basic(v)))),
    ];
    let mut leaked = Vec::new();
    let rig = Rig::new("matrix");
    let mut n = 0;
    for (word, v) in &words {
        for (form, make) in value_forms.iter().chain(key_forms.iter()) {
            let text = make(v);
            n += 1;
            let shown = config_of_settings(&rig, &format!("shape-{n}.toml"), &text);
            if shown.contains(BODY) {
                leaked.push(format!("{word} as {form} (--settings): {}", shown.lines().find(|l| l.contains(BODY)).unwrap_or("")));
            }
        }
    }
    assert!(leaked.is_empty(), "runbook config printed the body of a secret: {leaked:#?}");
}
// ---- the view and the submit ----

#[test]
fn the_view_lists_exactly_the_names_a_submit_accepts_in_every_source() {
    use runbook::runtime::Runtime;
    let rig = Rig::new("view-matrix");
    let put = |rel: &str, text: &str| {
        let path = rig.p(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    let book = |dir: &str, name: &str, revision: u32| {
        put(&format!("{dir}/protocol/{name}.protocol.yaml"), &format!("format: protocol/1\nprotocol:\n  id: {name}\n  revision: {revision}\n"));
        put(&format!("{dir}/bindings/{name}.yaml"), "format: loom.bindings/1\n");
    };
    // the built-in home, two configured directories (one of them also named by a link), a directory that
    // repeats a built-in name, a directory that repeats it with a protocol that has no bindings, a link to a
    // protocol file, a protocol that is not YAML, and a name that is no runbook name
    book("home", "alpha", 3);
    book("home", "beta", 2);
    book("a", "gamma", 1);
    book("a", "delta", 4);
    std::os::unix::fs::symlink(rig.p("a"), rig.p("a-link")).unwrap();
    book("b", "beta", 8);
    put("e/protocol/alpha.protocol.yaml", "format: protocol/1\nprotocol:\n  id: alpha\n  revision: 5\n");
    std::fs::create_dir_all(rig.p("e/bindings")).unwrap();
    put("c-real/eps.protocol.yaml", "format: protocol/1\nprotocol:\n  id: eps\n  revision: 6\n");
    put("c/bindings/eps.yaml", "format: loom.bindings/1\n");
    std::fs::create_dir_all(rig.p("c/protocol")).unwrap();
    std::os::unix::fs::symlink(rig.p("c-real/eps.protocol.yaml"), rig.p("c/protocol/eps.protocol.yaml")).unwrap();
    put("d/protocol/zeta.protocol.yaml", ": : not yaml [\n");
    put("d/bindings/zeta.yaml", "format: loom.bindings/1\n");
    put("d/protocol/a b.protocol.yaml", "format: protocol/1\nprotocol:\n  id: ab\n  revision: 1\n");
    let dirs: Vec<PathBuf> = ["a", "a-link", "b", "c", "d", "e", "late"].iter().map(|d| rig.p(d)).collect();
    let engine = runbook::engine::Engine::new(rig.p("home"), rig.p("state")).with_runbook_dirs(dirs.clone());
    // a directory that gains a runbook after the daemon started
    book("late", "omega", 7);
    let mut listed: Vec<(String, Option<String>)> = engine.runbooks().into_iter().map(|r| (r.name, r.version)).collect();
    listed.sort();
    let mut every = std::collections::BTreeSet::new();
    for home in dirs.iter().chain(std::iter::once(&rig.p("home"))) {
        for entry in std::fs::read_dir(home.join("protocol")).into_iter().flatten().flatten() {
            if let Some(name) = entry.file_name().to_string_lossy().strip_suffix(".protocol.yaml") {
                every.insert(name.to_owned());
            }
        }
    }
    for name in &every {
        let accepted = engine.knows_runbook(name, None);
        let shown = listed.iter().any(|(n, _)| n == name);
        assert_eq!(accepted, shown, "the name {name:?}: a submit accepts it ({accepted}), the view lists it ({shown}); view {listed:?}");
    }
    for (name, version) in &listed {
        let version = version.as_deref().unwrap();
        assert!(engine.knows_runbook(name, Some(version)), "{name}: a submit of the listed version {version} is refused");
        assert!(!engine.knows_runbook(name, Some("999")), "{name}: a submit of a version that is not there is accepted");
    }
    let names: Vec<&str> = listed.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["delta", "eps", "gamma", "omega"], "{listed:?}");
}

// ---- every start of the binary in the cases names the empty configuration, the daemon names its file ----

fn test_sources(dir: &Path, into: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            test_sources(&path, into);
        } else if path.extension().is_some_and(|e| e == "rs") {
            into.push(path);
        }
    }
}

#[test]
fn every_start_of_the_binary_in_every_case_file_names_the_empty_configuration() {
    // `serve`, `run`, `list`, `check`, `doctor` and `config` read a `runbook.toml` of the working directory's
    // parents and of the user's directories; a case that starts one of them without naming the empty file reads
    // what the host has
    let mut files = Vec::new();
    test_sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests"), &mut files);
    let shapes = ["env!(\"CARGO_BIN_EXE_runbook\")", "BIN)", "super::core::BIN)", "&odd)", "prefix.join(\"bin/runbook\")", "&program)", "program)"];
    // `XDG_CONFIG_HOME` is the private HOME and configuration directory of a `Rig`, whose working directory
    // lies in its own tree
    let guards = ["isolate(", "RUNBOOK_CONFIG", "RUNBOOK_NO_CONFIG", "--no-config", "XDG_CONFIG_HOME"];
    let others = ["\"parity\"", "\"canon\"", "\"mcp\"", "\"runs\"", "\"test\"", "\"--version\"", "\"frobnicate\"", "\"help\"", "\"decide\""];
    let mut unguarded = Vec::new();
    let mut starts = 0;
    for file in files {
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&file).unwrap();
        for (at, _) in text.match_indices("Command::new(") {
            let after = &text[at + "Command::new(".len()..];
            if !shapes.iter().any(|s| after.starts_with(s)) {
                continue;
            }
            let from = ["\nfn ", "\npub fn ", "\n    fn ", "\n#[test]"].iter().filter_map(|m| text[..at].rfind(m)).max().unwrap_or(0);
            let to = ["\nfn ", "\npub fn ", "\n#[test]"].iter().filter_map(|m| text[at..].find(m)).min().map_or(text.len(), |n| at + n);
            let region = &text[from..to];
            if others.iter().any(|o| region.contains(o)) && !region.contains("\"serve\"") {
                continue;
            }
            starts += 1;
            if !guards.iter().any(|g| region.contains(g)) {
                unguarded.push(format!("{name}:{}", text[..at].matches('\n').count() + 1));
            }
        }
    }
    assert!(starts >= 8, "the scan found only {starts} starts of the binary");
    assert!(unguarded.is_empty(), "starts of the binary in the cases that name no empty configuration and read what the host has: {unguarded:?}");
}

/// The lines `runbook serve` writes before it listens: standard error from its own pipe, and whether it listened.
fn serve_says(rig: &Rig, args: &[&str]) -> (Option<i32>, Vec<String>) {
    use std::io::{BufRead, BufReader};
    use std::sync::mpsc;
    let mut c = rig.command(&rig.root, args);
    let mut child = c.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let (tx, rx) = mpsc::channel::<String>();
    let out_pipe = child.stdout.take().unwrap();
    let err_pipe = child.stderr.take().unwrap();
    std::thread::spawn(move || BufReader::new(out_pipe).lines().map_while(Result::ok).for_each(|l| drop(tx.send(l))));
    let errors = std::thread::spawn(move || BufReader::new(err_pipe).lines().map_while(Result::ok).collect::<Vec<String>>());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut exited = None;
    loop {
        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(l) if l.contains("listening") => break,
            Ok(_) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                exited = child.wait().ok().and_then(|s| s.code());
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if std::time::Instant::now() > deadline {
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    (exited, errors.join().unwrap())
}

fn serve_args(rig: &Rig) -> Vec<String> {
    ["serve", "--runtime", "loom", "--principal", "person:test", "--listen", "127.0.0.1:0", "--state-dir", &rig.p("state").display().to_string(), "--min-confinement", "local"].iter().map(ToString::to_string).collect()
}

#[test]
fn serve_reads_the_file_of_the_users_directory_when_nothing_names_one() {
    // what an unisolated start of `serve` in the cases would read of the host: the user's directory
    let rig = Rig::new("serve-user-dir");
    rig.fixture();
    let dir = rig.p("books/a");
    std::fs::create_dir_all(dir.join("protocol")).unwrap();
    std::fs::create_dir_all(dir.join("bindings")).unwrap();
    std::fs::write(dir.join("protocol/example.protocol.yaml"), "protocol:\n  id: example\n  revision: 1\n").unwrap();
    std::fs::write(dir.join("bindings/example.yaml"), "format: loom.bindings/1\n").unwrap();
    let file = rig.put("xdg/runbook/runbook.toml", &format!("runbooks = [\"{}\"]\n", dir.display()));
    let args = serve_args(&rig);
    let (_, lines) = serve_says(&rig, &args.iter().map(String::as_str).collect::<Vec<_>>());
    assert!(lines.iter().any(|l| l == &format!("runbook serve: values from runbook.toml {} (found in the user's configuration directory)", file.display())), "{lines:?}");
}

#[test]
fn serve_says_nothing_of_a_file_that_states_no_key() {
    // docs/loom-run.md: with no file, or a file that states no key, nothing is printed
    let rig = Rig::new("serve-empty-file");
    rig.fixture();
    rig.put("runbook.toml", "runbooks = []\n");
    let args = serve_args(&rig);
    let (_, lines) = serve_says(&rig, &args.iter().map(String::as_str).collect::<Vec<_>>());
    assert!(lines.iter().any(|l| l.contains("this daemon runs")), "the daemon did not start: {lines:?}");
    assert!(!lines.iter().any(|l| l.contains("values from")), "a file that states no key is named: {lines:?}");
    std::fs::remove_file(rig.p("runbook.toml")).unwrap();
    let (_, lines) = serve_says(&rig, &args.iter().map(String::as_str).collect::<Vec<_>>());
    assert!(lines.iter().any(|l| l.contains("this daemon runs")), "the daemon did not start: {lines:?}");
    assert!(!lines.iter().any(|l| l.contains("values from")), "{lines:?}");
}

#[test]
fn serve_names_its_file_before_it_refuses_the_directories_the_file_names() {
    // the line comes before the daemon reads the directories: a refusal of them says which file they came from
    let rig = Rig::new("serve-refuses");
    rig.fixture();
    let file = rig.put("runbook.toml", "runbooks = [\"missing-books\"]\n");
    let args = serve_args(&rig);
    let o = rig.run(&rig.root, &args.iter().map(String::as_str).collect::<Vec<_>>());
    assert_ne!(code(&o), 0, "{}", both(&o));
    let text = err(&o);
    let lines: Vec<&str> = text.lines().collect();
    let said = lines.iter().position(|l| l.contains(&file.display().to_string()) && l.contains("values from"));
    assert!(said.is_some(), "a refusal of the directories of a file does not say which file they came from: {lines:?}");
    let refused = lines.iter().position(|l| l.contains("missing-books")).unwrap_or(usize::MAX);
    assert!(said.unwrap() < refused, "{lines:?}");
}

// ---- the file the option or the variable names ----

#[test]
fn a_file_named_by_the_option_or_no_file_at_all_lists_no_file_of_the_walk() {
    let rig = Rig::new("named-no-skips");
    rig.put("runs/r1/events.jsonl", "");
    std::fs::create_dir_all(rig.p("runs/r1/work")).unwrap();
    std::fs::create_dir_all(rig.p("runs/r1/bindings")).unwrap();
    let planted = rig.put("runs/r1/work/runbook.toml", "max_wall_seconds = 4242424\n");
    let named = rig.put("named.toml", "max_wall_seconds = 90\n");
    let cwd = rig.p("runs/r1/work");
    let lists = |o: &Output| out(o).lines().any(|l| l.contains("skipped") && l.contains(&planted.display().to_string()));
    assert!(lists(&rig.run(&cwd, &["config"])), "without a named file the walk lists the file it did not read");
    for args in [vec!["config", "--config", named.to_str().unwrap()], vec!["config", "--no-config"]] {
        let o = rig.run(&cwd, &args);
        assert_eq!(code(&o), 0, "{}", both(&o));
        assert!(!lists(&o), "{args:?} lists a file of the walk: {}", out(&o));
        assert!(!both(&o).contains("4242424"), "{args:?} printed a value of the planted file");
    }
    let mut c = rig.command(&cwd, &["config"]);
    c.env("RUNBOOK_NO_CONFIG", "1");
    let o = c.output().unwrap();
    assert!(!lists(&o), "{}", out(&o));
}

// ---- a file found inside a program of a run: no key for a floor or a level ----

#[test]
fn a_file_found_inside_a_program_of_a_run_has_no_key_for_a_floor_or_a_level() {
    // docs/loom-run.md (a nested `runbook` under `substrate` reads the work tree's own file): "it has no key for
    // a floor or a level". Loom gives every bound program `LOOM_CASE`; a start that has it is a program of a run.
    for (key, text) in [("confinement", "confinement = \"local\"\n"), ("autonomy", "autonomy = \"A5\"\n")] {
        let rig = Rig::new("level-key");
        let file = rig.put("runbook.toml", text);
        let mut nested = rig.command(&rig.root, &["config"]);
        nested.env("LOOM_CASE", "a-case");
        let o = nested.output().unwrap();
        assert_eq!(code(&o), 2, "{key}: a file found inside a program of a run states a level and is read: {}{}", out(&o), err(&o));
        assert!(err(&o).contains(key) && err(&o).contains("runbook.toml"), "{key}: the refusal does not name the key: {}", err(&o));
        assert!(!err(&o).contains("\"local\"") && !err(&o).contains("A5") && !out(&o).contains("local"), "{key}: a value of the file was printed: {}{}", out(&o), err(&o));
        // the same file, started by a person (no `LOOM_CASE`), is read: the file's confinement is the story
        let o = rig.run(&rig.root, &["config"]);
        assert_eq!(code(&o), 0, "{key}: {}", err(&o));
        // a file the caller names is the caller's own choice
        let mut named = rig.command(&rig.root, &["config", "--config", &file.display().to_string()]);
        named.env("LOOM_CASE", "a-case");
        let o = named.output().unwrap();
        assert_eq!(code(&o), 0, "{key}: {}", err(&o));
    }
}

// ---- the tree of a case stands outside the checkout ----

#[test]
fn a_rig_stands_outside_the_checkout_and_a_file_in_a_tree_around_another_rig_changes_no_result() {
    // The walk for `runbook.toml` climbs from the working directory to the root of the file system. A rig below
    // the checkout (or `target/` of it) found the file at the root of the checkout: seven cases went red when
    // one stood there. The tree of a rig is a fresh directory of its own in the temporary directory.
    let canon = |p: &Path| p.canonicalize().unwrap();
    let rig = Rig::new("outside");
    let checkout = canon(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap());
    assert!(!canon(&rig.root).starts_with(&checkout), "the tree of a rig is below the checkout: {}", rig.root.display());
    assert!(!canon(&rig.root).starts_with(canon(Path::new(env!("CARGO_TARGET_TMPDIR")))), "the tree of a rig is below target/tmp");
    // a checkout of its own in another rig's tree: a file at its root and one in its build tree
    let stand_in = Rig::new("stand-in");
    stand_in.put("checkout/runbook.toml", "max_wall_seconds = 4242424\nrunbooks = [\"nowhere\"]\n");
    stand_in.put("checkout/runbook/target/tmp/keep", "");
    let before = rig.run(&rig.p("work/sub"), &["config"]);
    stand_in.put("checkout/runbook/runbook.toml", "max_llm_calls = 4242424\n");
    let after = rig.run(&rig.p("work/sub"), &["config"]);
    assert_eq!((code(&before), out(&before), err(&before)), (code(&after), out(&after), err(&after)));
    assert!(!out(&after).contains("4242424") && !err(&after).contains("nowhere"), "{}{}", out(&after), err(&after));
}
