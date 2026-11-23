//! Guards of `runbook.toml` (defaults of `runbook run`, runbook directories) beside `config_file.rs`
//! and `runbook_dirs.rs`: what the file can make the binary do, what `runbook config` prints, which
//! trees the walk never reads, and which runbook a name resolves to. Every start of the binary has a
//! private HOME and XDG_CONFIG_HOME and a working directory inside the test's own tree (`Rig`), and
//! reads no file of the host. No model call, no network, no login file.

use super::config_file::Rig;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Output;

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
    let rows: Vec<(String, Option<String>)> = engine.runbooks().into_iter().map(|r| (r.name, r.version)).collect();
    assert_eq!(rows, [("alpha".to_string(), Some("3".to_string()))], "the daemon's lookup let a directory shadow the built-in name: {rows:?}");
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
    use std::process::Stdio;
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

