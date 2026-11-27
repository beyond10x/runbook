//! `runbooks = ["dir", ...]` of `runbook.toml` (`docs/loom-run.md`, "Runbook directories"): the
//! directories runbooks are loaded from, resolved with the built-in set last, by one function for
//! `list`, `run`, `serve`, `check` and `doctor --for`. The directories here are written by the test,
//! outside the repository tree, from the engine fixture with the ids renamed to `example`; no test
//! makes a model call.

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

fn copy_renamed(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let target = to.join(name.replace("engine-demo", "example").replace("engine-draft", "example-draft"));
        if entry.file_type().unwrap().is_dir() {
            copy_renamed(&entry.path(), &target);
        } else {
            let text = std::fs::read_to_string(entry.path()).unwrap();
            let text = text.replace("engine-demo", "example").replace("engine-draft", "example-draft").replace("engine.draft", "example.draft");
            std::fs::write(&target, &text).unwrap();
            std::fs::set_permissions(&target, std::fs::metadata(entry.path()).unwrap().permissions()).unwrap();
        }
    }
}

/// A runbook directory `rel` of the rig: the root `example` (protocol id `example`, revision 1), which
/// opens the child `example-draft`, and the tables the decider reads.
fn make(rig: &Rig, rel: &str) -> PathBuf {
    let dir = rig.p(rel);
    copy_renamed(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &dir);
    super::decider::install_routing_table_at(&dir);
    dir
}

fn row(o: &Output, id: &str) -> String {
    let text = out(o);
    text.lines().find(|l| l.split_whitespace().next() == Some(id)).unwrap_or_else(|| panic!("no row for {id} in\n{text}\n{}", err(o))).to_owned()
}

fn setup(tag: &str, listed: &[&str]) -> (Rig, Vec<PathBuf>) {
    let rig = Rig::new(tag);
    rig.fixture();
    let dirs: Vec<PathBuf> = listed.iter().map(|d| make(&rig, d)).collect();
    let list = listed.iter().map(|d| format!("\"{}\"", rig.p(d).display())).collect::<Vec<_>>().join(", ");
    rig.put("runbook.toml", &format!("{FOUR}runbooks = [{list}]\n"));
    (rig, dirs)
}

#[test]
fn list_prints_name_version_and_directory_of_every_runbook_and_the_built_in_set_comes_last() {
    let (rig, dirs) = setup("list", &["books/a"]);
    let o = rig.run(&rig.root, &["list"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let example = row(&o, "example");
    let cols: Vec<&str> = example.split_whitespace().collect();
    assert_eq!(&cols[..4], ["example", "runbook", "1", dirs[0].to_str().unwrap()], "{example}");
    let child = row(&o, "example-draft");
    assert!(child.split_whitespace().nth(1) == Some("child") && child.contains(dirs[0].to_str().unwrap()), "{child}");
    // the built-in set (here the engine fixture as RUNBOOK_HOME) is listed with its own directory
    let built_in = row(&o, "engine-demo");
    assert!(built_in.contains(rig.p("fx").to_str().unwrap()) && built_in.split_whitespace().nth(1) == Some("runbook"), "{built_in}");
    let text = out(&o);
    assert!(text.find("example ").unwrap() < text.find("engine-demo").unwrap() || text.find("example-draft").unwrap() < text.find("engine-draft").unwrap(), "{text}");

    let o = rig.run(&rig.root, &["list", "--json"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let v: serde_json::Value = serde_json::from_str(&out(&o)).unwrap();
    let find = |id: &str| v.as_array().unwrap().iter().find(|e| e["id"] == id).unwrap_or_else(|| panic!("{id} not in {v}")).clone();
    assert_eq!((find("example")["version"].as_u64(), find("example")["directory"].as_str()), (Some(1), dirs[0].to_str()));
    assert_eq!(find("example")["startable"], true);
    assert_eq!(find("example-draft")["startable"], false, "a protocol another one opens is a child, by what the files say");
    assert_eq!(find("engine-demo")["directory"].as_str(), rig.p("fx").to_str());
}

#[test]
fn a_directory_is_absolute_or_relative_to_the_file_and_the_file_decides_the_directory_it_names() {
    let rig = Rig::new("relative");
    rig.fixture();
    make(&rig, "conf/books/rel");
    let abs = make(&rig, "elsewhere/abs");
    std::fs::rename(abs.join("protocol/example.protocol.yaml"), abs.join("protocol/other.protocol.yaml")).unwrap();
    std::fs::rename(abs.join("bindings/example.yaml"), abs.join("bindings/other.yaml")).unwrap();
    std::fs::rename(abs.join("protocol/example-draft.protocol.yaml"), abs.join("protocol/other-draft.protocol.yaml")).unwrap();
    std::fs::rename(abs.join("bindings/example-draft.yaml"), abs.join("bindings/other-draft.yaml")).unwrap();
    rig.put("conf/runbook.toml", &format!("runbooks = [\"books/rel\", \"{}\"]\n", abs.display()));
    let o = rig.run(&rig.p("work"), &["list", "--config", &rig.p("conf/runbook.toml").display().to_string()]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(row(&o, "example").contains(rig.p("conf/books/rel").to_str().unwrap()), "{}", out(&o));
    assert!(row(&o, "other").contains(abs.to_str().unwrap()), "{}", out(&o));
}

#[test]
fn two_directories_with_one_runbook_name_are_an_error_that_names_both() {
    let (rig, dirs) = setup("collision", &["books/a", "books/b"]);
    let o = rig.run(&rig.root, &["list"]);
    assert_eq!(code(&o), 1, "{}", out(&o));
    let said = err(&o);
    assert!(said.contains("example") && said.contains(dirs[0].to_str().unwrap()) && said.contains(dirs[1].to_str().unwrap()), "{said}");
    assert!(out(&o).is_empty(), "nothing is listed from a list that is an error");
    // the same name in a configured directory and in the built-in set
    let (rig, dirs) = setup("collision-builtin", &["books/a"]);
    std::fs::rename(dirs[0].join("protocol/example.protocol.yaml"), dirs[0].join("protocol/engine-demo.protocol.yaml")).unwrap();
    std::fs::rename(dirs[0].join("bindings/example.yaml"), dirs[0].join("bindings/engine-demo.yaml")).unwrap();
    let o = rig.run(&rig.root, &["list"]);
    assert_eq!(code(&o), 1, "{}", out(&o));
    let said = err(&o);
    assert!(said.contains("engine-demo") && said.contains(dirs[0].to_str().unwrap()) && said.contains(rig.p("fx").to_str().unwrap()), "{said}");
    // run, serve, check and doctor resolve through the same function and refuse the same way
    for args in [vec!["run", "engine-demo", "--intake", "idea.txt", "--dry"], vec!["check"], vec!["doctor", "--for", "engine-demo"]] {
        let o = rig.run(&rig.root, &args);
        assert_ne!(code(&o), 0, "{args:?}: {}", out(&o));
        assert!(err(&o).contains(dirs[0].to_str().unwrap()) && err(&o).contains(rig.p("fx").to_str().unwrap()), "{args:?}: {}", err(&o));
    }
}

#[test]
fn a_directory_that_is_missing_or_incomplete_is_an_error_that_names_it() {
    let (rig, _) = setup("missing", &["books/a"]);
    let missing = rig.p("books/none");
    rig.put("runbook.toml", &format!("{FOUR}runbooks = [\"{}\"]\n", missing.display()));
    let o = rig.run(&rig.root, &["list"]);
    assert_eq!(code(&o), 1, "{}", out(&o));
    assert!(err(&o).contains(missing.to_str().unwrap()), "{}", err(&o));
    for part in ["protocol", "bindings"] {
        let (rig, dirs) = setup(&format!("incomplete-{part}"), &["books/a"]);
        std::fs::remove_dir_all(dirs[0].join(part)).unwrap();
        let o = rig.run(&rig.root, &["list"]);
        assert_eq!(code(&o), 1, "{part}: {}", out(&o));
        assert!(err(&o).contains(dirs[0].to_str().unwrap()) && err(&o).contains(&format!("{part}/")), "{part}: {}", err(&o));
    }
    // a file in the place of a directory
    let (rig, dirs) = setup("notdir", &["books/a"]);
    std::fs::remove_dir_all(&dirs[0]).unwrap();
    std::fs::write(&dirs[0], "x").unwrap();
    let o = rig.run(&rig.root, &["list"]);
    assert_eq!(code(&o), 1, "{}", out(&o));
    assert!(err(&o).contains(dirs[0].to_str().unwrap()), "{}", err(&o));
}

#[test]
fn a_directory_goes_through_the_checks_of_the_built_in_set_a_link_out_or_a_device_is_refused() {
    let (rig, dirs) = setup("link", &["books/a"]);
    rig.put("outside/secret.yaml", "format: loom.bindings/1\n");
    std::os::unix::fs::symlink(rig.p("outside/secret.yaml"), dirs[0].join("bindings/leak.yaml")).unwrap();
    let o = rig.run(&rig.root, &["list"]);
    assert_eq!(code(&o), 1, "{}", out(&o));
    assert!(err(&o).contains("leak.yaml") && err(&o).contains("link"), "{}", err(&o));
    std::fs::remove_file(dirs[0].join("bindings/leak.yaml")).unwrap();
    // a link that stays inside the directory is one the built-in set has too
    std::os::unix::fs::symlink("example.yaml", dirs[0].join("bindings/inside.yaml")).unwrap();
    let o = rig.run(&rig.root, &["list"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    std::fs::remove_file(dirs[0].join("bindings/inside.yaml")).unwrap();
    // a link to a directory outside, in the place of protocol/
    std::fs::remove_dir_all(dirs[0].join("protocol")).unwrap();
    std::fs::create_dir_all(rig.p("outside/protocol")).unwrap();
    std::os::unix::fs::symlink(rig.p("outside/protocol"), dirs[0].join("protocol")).unwrap();
    let o = rig.run(&rig.root, &["list"]);
    assert_eq!(code(&o), 1, "{}", out(&o));
    assert!(err(&o).contains(dirs[0].to_str().unwrap()), "{}", err(&o));
    // a dangling link
    let (rig, dirs) = setup("dangling", &["books/a"]);
    std::os::unix::fs::symlink("nowhere.yaml", dirs[0].join("bindings/dangling.yaml")).unwrap();
    let o = rig.run(&rig.root, &["list"]);
    assert_eq!(code(&o), 1, "{}", out(&o));
    assert!(err(&o).contains("dangling.yaml"), "{}", err(&o));
}

#[test]
fn nothing_of_a_configured_directory_is_executed_at_list_check_or_doctor() {
    let (rig, dirs) = setup("noexec", &["books/a"]);
    let marker = rig.p("marker");
    let tool = dirs[0].join("bindings/tools/boom.sh");
    std::fs::write(&tool, format!("#!/bin/sh\necho ran > {}\n", marker.display())).unwrap();
    std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
    for args in [vec!["list"], vec!["list", "--json"], vec!["config"], vec!["check", "--for", "example"], vec!["doctor", "--for", "example", "--json"]] {
        let _ = rig.run(&rig.root, &args);
        assert!(!marker.exists(), "{args:?} ran a program of the configured directory");
    }
}

#[test]
fn run_resolves_a_name_of_a_configured_directory_and_starts_it() {
    let (rig, dirs) = setup("run", &["books/a"]);
    let o = rig.run(&rig.root, &["run", "example", "--intake", "idea.txt", "--dry"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let runs = rig.p("runs");
    let names: Vec<String> = std::fs::read_dir(&runs).unwrap().flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names.len(), 1, "{names:?}");
    assert!(names[0].starts_with("example-"), "{names:?}");
    let outcome: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(runs.join(&names[0]).join("outcome.json")).unwrap()).unwrap();
    assert_eq!(outcome["outcome"], "ended", "{outcome}");
    let copied = runs.join(&names[0]).join("bindings");
    assert!(copied.join("example.yaml").is_file(), "the bindings are the configured directory's");
    assert!(dirs[0].join("bindings/example.yaml").is_file());
    // a name no directory has is refused by name, with the directories searched
    let o = rig.run(&rig.root, &["run", "nothing-here", "--intake", "idea.txt", "--dry"]);
    assert_ne!(code(&o), 0);
    assert!(err(&o).contains("nothing-here") && err(&o).contains(dirs[0].to_str().unwrap()), "{}", err(&o));
    assert!(!rig.p("runs/nothing-here").exists());
}

#[test]
fn the_built_in_set_still_starts_by_name_and_a_given_directory_option_wins() {
    let (rig, _) = setup("builtin", &["books/a"]);
    let o = rig.run(&rig.root, &["run", "engine-demo", "--intake", "idea.txt", "--dry"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    // --bindings and --protocols name the files themselves: no lookup of the name is needed
    let a = rig.p("books/a");
    let o = rig.run(
        &rig.root,
        &["run", "example", "--intake", "idea.txt", "--dry", "--run", "by-option", "--bindings", &a.join("bindings").display().to_string(), "--protocols", &a.join("protocol").display().to_string()],
    );
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(rig.p("runs/by-option/outcome.json").is_file());
}

#[test]
fn check_and_doctor_work_on_a_runbook_of_a_configured_directory() {
    let (rig, dirs) = setup("check", &["books/a"]);
    let o = rig.run(&rig.root, &["check", "--for", "example"]);
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    assert!(out(&o).contains(dirs[0].to_str().unwrap()) || err(&o).is_empty(), "{}", out(&o));
    // a finding of the directory is a finding of that file
    std::fs::write(dirs[0].join("bindings/example.yaml"), "format: loom.bindings/1\nprotocol: example@1\nactions:\n  - action: draft.open\n    executor:\n      argv: [tools/missing.sh]\n").unwrap();
    let o = rig.run(&rig.root, &["check", "--for", "example"]);
    assert_eq!(code(&o), 1, "{}", out(&o));
    assert!(out(&o).contains("bindings/example.yaml") && out(&o).contains("missing.sh"), "{}", out(&o));
    // doctor: the runbook is found through the same function and its directory is what the layout lines check
    let (rig, dirs) = setup("doctor", &["books/a"]);
    let o = rig.run(&rig.root, &["doctor", "--for", "example", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&out(&o)).unwrap_or_else(|e| panic!("{e}: {}{}", out(&o), err(&o)));
    assert_eq!(v["for"], "example");
    assert_eq!(v["assets"]["root"].as_str(), dirs[0].to_str());
    for part in ["protocol/", "bindings/", "bindings/*.yaml"] {
        let c = v["checks"].as_array().unwrap().iter().find(|c| c["name"] == part).unwrap_or_else(|| panic!("no {part} line in {v}"));
        assert_eq!(c["found"], true, "{c}");
    }
    assert!(v["checks"].as_array().unwrap().iter().all(|c| c["name"] != "scripts/"), "the layout of the built-in set is not asked of a directory that is not one");
    // a name of no directory is refused, naming the startable ones
    let o = rig.run(&rig.root, &["doctor", "--for", "nothing-here"]);
    assert_eq!(code(&o), 64, "{}", err(&o));
    assert!(err(&o).contains("example") && err(&o).contains("engine-demo"), "{}", err(&o));
    assert!(!err(&o).contains("example-draft"), "a child is no runbook to start: {}", err(&o));
}

#[test]
fn serve_refuses_two_directories_with_one_name_before_it_listens() {
    let (rig, dirs) = setup("serve", &["books/a", "books/b"]);
    let o = rig.run(&rig.root, &["serve", "--runtime", "loom", "--principal", "person:test", "--listen", "127.0.0.1:0", "--state-dir", &rig.p("state").display().to_string(), "--min-confinement", "local", "--max-wall-seconds", "3600", "--max-llm-calls", "10000", "--cap-micro-usd", "1000000000"]);
    assert_eq!(code(&o), 1, "{}", out(&o));
    assert!(err(&o).contains(dirs[0].to_str().unwrap()) && err(&o).contains(dirs[1].to_str().unwrap()), "{}", err(&o));
    assert!(!out(&o).contains("listening"), "{}", out(&o));
}

/// The binary reads the set once itself (`catalog::open`) and refuses a duplicate before it calls the
/// daemon's start, so the case above stays green without the check in `serve::prepare`. This one calls
/// `prepare` with a set the binary would have refused: it must refuse by itself, before it writes its state.
#[test]
fn the_daemons_start_refuses_two_directories_with_one_name_before_it_writes_its_state() {
    let (rig, dirs) = setup("prepare", &["books/a", "books/b"]);
    let args = runbook::serve::ServeArgs {
        listen: "127.0.0.1:0".to_owned(),
        state_dir: Some(rig.p("state")),
        context_root: Vec::new(),
        runbook_dir: None,
        runtime: "loom".to_owned(),
        runbook_home: Some(rig.p("fx")),
        runbooks: dirs.clone(),
        min_confinement: "local".to_owned(),
        default_confinement: None,
        file_confinement: None,
        autonomy: None,
        decider: None,
        arbiter: None,
        routes: Default::default(),
        classify_threshold: None,
        repositories: Default::default(),
        route_table: None,
        assets_root: None,
        token_file: None,
        principal: Some("person:test".to_owned()),
        ui: false,
        mcp: false,
        max_wall_seconds: Some(3600),
        max_llm_calls: Some(10_000),
        cap_micro_usd: Some(1_000_000_000),
        max_wall_seconds_ceiling: None,
        max_llm_calls_ceiling: None,
        cap_micro_usd_ceiling: None,
        drain_seconds: None,
    };
    let refused = match runbook::serve::prepare(&args, &runbook::serve::Env::default(), &[]) {
        Ok(_) => panic!("the daemon's start took a set with one name in two directories"),
        Err(e) => e,
    };
    assert!(refused.contains(dirs[0].to_str().unwrap()) && refused.contains(dirs[1].to_str().unwrap()), "{refused}");
    assert!(!rig.p("state").exists(), "the daemon wrote its state before it refused the set");
}

#[test]
fn config_shows_the_runbooks_of_the_file_with_their_origin_and_the_file_refuses_what_is_no_path() {
    let (rig, dirs) = setup("config", &["books/a"]);
    let o = rig.run(&rig.root, &["config"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let line = out(&o).lines().find(|l| l.starts_with("runbooks = ")).unwrap_or_else(|| panic!("{}", out(&o))).to_owned();
    assert!(line.contains(dirs[0].to_str().unwrap()) && line.contains("file "), "{line}");
    let token = format!("runbooks = [\"{}\"]\n", ["sk-", "ant-api03-abcdefghijklmnop"].concat());
    for (bad, tag) in [("runbooks = \"books/a\"\n", "text"), ("runbooks = [1]\n", "number"), ("runbooks = [\"a\\nb\"]\n", "newline"), ("runbooks = [\"\"]\n", "empty"), (token.as_str(), "token")] {
        let file = rig.put(&format!("bad/{tag}.toml"), bad);
        let o = rig.run(&rig.root, &["config", "--config", &file.display().to_string()]);
        assert_eq!(code(&o), 2, "{tag}: {}", out(&o));
        assert!(err(&o).contains("runbooks"), "{tag}: {}", err(&o));
        assert!(!err(&o).contains("abcdefghijklmnop") && !err(&o).contains("a\nb"), "{tag}: {}", err(&o));
    }
}

/// Adversary: `runbook check` reads a route file with `admission::read` and `parse` only. `parse` takes an entry that is
/// no entry (`default: fast`, a route written without `{route: ...}`) and a `routes_allowed` that is no list as a
/// `Declared`; the daemon's `protocol_word` (`runbook/src/commands.rs`) then calls `admission::check`, which refuses each
/// at every submit whatever routes the daemon declares. `runbook check` reports nothing for either.
#[test]
fn adversary_check_reports_a_route_file_the_daemon_refuses_at_every_submit() {
    use runbook::admission::{check, parse, Facts};
    let fast = [Facts { route: "fast".into(), kind: "managed".into(), wires: vec!["messages".into()], ..Facts::default() }];
    for (tag, text) in [
        ("adv-entry", "format: runbook.routes/1\nmodel_routes:\n  default: fast\n"),
        ("adv-allowed", "format: runbook.routes/1\nmodel_routes:\n  default: {route: fast}\nroutes_allowed: fast\n"),
    ] {
        let declared = parse("example", text).unwrap_or_else(|e| panic!("{tag}: parse refused it: {e}"));
        let refused = check("example", &declared, &fast).expect_err("the daemon admits it");
        let (rig, dirs) = setup(tag, &["books/a"]);
        std::fs::write(dirs[0].join("bindings/example.routes.yaml"), text).unwrap();
        let o = rig.run(&rig.root, &["check", "--for", "example"]);
        assert!(code(&o) == 1 && out(&o).contains("bindings/example.routes.yaml"), "{tag}: the daemon refuses every submit ({refused}), runbook check exits {}:\n{}{}", code(&o), out(&o), err(&o));
    }
}

/// Adversary: the daemon opens `bindings/<id>.routes.yaml` and refuses anything that is not a regular file
/// (`admission::read`). `runbook check` lists `bindings/*.yaml` through `files_ending`, which keeps only
/// `Path::is_file()`: a directory of that name is never read, so the check is green where the daemon refuses every
/// submit and `scripts/check-bindings.sh --exits` refuses by a named line. (A dangling link of that name is refused by
/// the walk of a configured directory, `runbooks.rs`, before the check runs: not part of this case.)
#[test]
fn adversary_check_reports_a_route_file_name_that_is_a_directory() {
    let (rig, dirs) = setup("adv-directory", &["books/a"]);
    let at = dirs[0].join("bindings/example.routes.yaml");
    std::fs::create_dir(&at).unwrap();
    let refused = runbook::admission::read(&at, "example").expect("the daemon reads a file of this name").expect_err("the daemon reads it as text");
    let o = rig.run(&rig.root, &["check", "--for", "example"]);
    assert!(code(&o) == 1 && out(&o).contains("bindings/example.routes.yaml"), "the daemon refuses it ({refused}), runbook check exits {}:\n{}{}", code(&o), out(&o), err(&o));
}
