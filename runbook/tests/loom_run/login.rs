//! The login file leaves no path in the run directory: the link the model nodes read lives in a
//! private directory below `XDG_RUNTIME_DIR` (`<XDG_RUNTIME_DIR>/runbook/<run>/`, mode 0700) for the
//! length of the run, and is gone when the run ends, on an error and on SIGTERM too.

use super::core::{code, grep_tree, stderr, World};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn private_dir(w: &World, run: &str) -> PathBuf {
    w.root.join("xdg").join("runbook").join(run)
}

/// Every link below `dir` with its target.
fn links(dir: &Path, out: &mut Vec<(PathBuf, PathBuf)>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let meta = std::fs::symlink_metadata(&p).unwrap();
        if meta.file_type().is_symlink() {
            out.push((p.clone(), std::fs::read_link(&p).unwrap()));
        } else if meta.is_dir() {
            links(&p, out);
        }
    }
}

/// A hook for the first `tree run`: what the private directory looks like while the run is on.
fn look_at_private_dir(w: &World, run: &str) -> String {
    let p = private_dir(w, run).display().to_string();
    format!("stat -c '%a' \"{p}\" > \"$D/private-mode\"\nreadlink \"{p}/login\" > \"$D/private-link\"\nls -A \"{p}\" > \"$D/private-ls\"\n")
}

#[test]
fn a_login_file_leaves_no_path_and_no_link_in_the_run_directory_and_the_private_directory_is_gone() {
    let w = World::new("login-private");
    w.one_decision_tree();
    w.scripted_approve();
    w.fake.hook("tree_run", "1", &look_at_private_dir(&w, "r1"));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let login = w.login_path();
    // While the run was on: a directory of mode 0700 that holds the link and nothing else.
    let seen = |name: &str| std::fs::read_to_string(w.fake.dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}")).trim().to_owned();
    assert_eq!(seen("private-mode"), "700");
    assert_eq!(seen("private-ls"), "login");
    assert_eq!(seen("private-link"), login.display().to_string());
    // The model nodes of the copy read the login through that link.
    let link = private_dir(&w, "r1").join("login").display().to_string();
    let bindings = std::fs::read_to_string(w.run_dir("r1/bindings/sdc.yaml")).unwrap();
    assert!(bindings.contains("--claude-credentials") && bindings.contains(&link), "{bindings}");
    // After it: no file of the run directory holds the login's path or content, no link in it points
    // there, and the private directory is gone.
    let dir = w.run_dir("r1");
    for needle in [login.display().to_string(), "LOGIN-FILE-MARKER-4711".to_owned(), "loginhome".to_owned()] {
        let mut hits = Vec::new();
        grep_tree(&dir, &needle, &mut hits);
        assert!(hits.is_empty(), "{needle} found in {hits:?}");
    }
    let mut found = Vec::new();
    links(&dir, &mut found);
    assert!(found.iter().all(|(_, to)| to != &login && !to.starts_with("loginhome") && !to.display().to_string().contains("loginhome")), "a link points to the login file: {found:?}");
    assert!(std::fs::symlink_metadata(dir.join("login")).is_err());
    assert!(!private_dir(&w, "r1").exists(), "the private directory is left");
    let settings: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("settings.json")).unwrap()).unwrap();
    assert_eq!(settings["login_file"], "named", "the run directory says only that a login file was named");
}

#[test]
fn the_private_directory_is_removed_when_the_run_ends_as_an_error() {
    let w = World::new("login-error");
    w.fake.answer("tree_open", "default", "", 1);
    let p = private_dir(&w, "r1").display().to_string();
    w.fake.hook("tree_open", "default", &format!("[ -d \"{p}\" ] && touch \"$D/had-private\"\n"));
    w.scripted_approve();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    assert!(w.fake.dir.join("had-private").exists(), "the private directory did not exist while the run was on");
    assert_eq!(w.outcome("r1")["outcome"], "error");
    assert!(!private_dir(&w, "r1").exists(), "the private directory is left after an error");
}

#[test]
fn the_private_directory_is_removed_when_the_run_is_ended_by_sigterm() {
    let w = World::new("login-signal");
    w.fake.answer("tree_run", "1", "", 0);
    w.fake.hook("tree_run", "1", "echo $$ > \"$D/pid\"\nsleep 30\n");
    w.scripted_approve();
    let mut child = w.command(&w.args("r1", 1_000_000)).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let started = std::time::Instant::now();
    while !w.fake.dir.join("pid").exists() {
        assert!(started.elapsed().as_secs() < 20, "the fake tree run did not start");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(private_dir(&w, "r1").join("login").exists(), "the link is there while the run is on");
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
    assert!(!private_dir(&w, "r1").exists(), "the private directory is left after SIGTERM");
}

#[test]
fn a_login_file_is_refused_without_a_runtime_directory_of_the_user_before_anything_is_written() {
    let refused = |tag: &str, setup: &dyn Fn(&mut Command)| {
        let w = World::new(tag);
        w.one_decision_tree();
        w.scripted_approve();
        let mut c = w.command(&w.args("r1", 1_000_000));
        setup(&mut c);
        let o = c.output().unwrap();
        assert_eq!(code(&o), 2, "{tag}: {}", stderr(&o));
        assert!(stderr(&o).contains("XDG_RUNTIME_DIR"), "{tag}: {}", stderr(&o));
        assert!(!stderr(&o).contains("loginhome"), "{tag}: the login path is in the refusal: {}", stderr(&o));
        assert!(!w.run_dir("r1").exists(), "{tag}: a run directory was made");
        assert!(!w.ledger().exists(), "{tag}: the ledger was made");
        assert_eq!(w.fake.calls_so_far(), 0, "{tag}: the binary was called");
    };
    refused("xdg-unset", &|c| {
        c.env_remove("XDG_RUNTIME_DIR");
    });
    refused("xdg-relative", &|c| {
        c.env("XDG_RUNTIME_DIR", "xdg");
    });
    refused("xdg-missing", &|c| {
        c.env("XDG_RUNTIME_DIR", "/nonexistent-runtime-dir");
    });
    // `/` belongs to root: a directory another user owns is no private place.
    if unsafe { libc::geteuid() } != 0 {
        refused("xdg-not-owned", &|c| {
            c.env("XDG_RUNTIME_DIR", "/");
        });
    }
}

#[test]
fn a_private_directory_left_by_a_killed_run_is_refused_by_name_and_is_not_taken_over() {
    let w = World::new("login-stale");
    w.one_decision_tree();
    w.scripted_approve();
    let stale = private_dir(&w, "r1");
    std::fs::create_dir_all(&stale).unwrap();
    std::fs::write(stale.join("keep"), "mine").unwrap();
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(stderr(&o).contains(&stale.display().to_string()) && stderr(&o).contains("exists"), "{}", stderr(&o));
    assert_eq!(std::fs::read_to_string(stale.join("keep")).unwrap(), "mine");
    assert!(!w.run_dir("r1").exists());
}

#[test]
fn a_run_that_names_no_login_file_needs_no_runtime_directory_and_makes_none() {
    let w = World::new("login-none");
    w.one_decision_tree();
    std::fs::write(
        w.root.join("settings.json"),
        json!({"decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [{"approve": true, "principal": "person:owner", "cost_micro_usd": 0, "model_calls": 0}]}}).to_string(),
    )
    .unwrap();
    let mut c = w.command(&w.args("r1", 1_000_000));
    c.env_remove("XDG_RUNTIME_DIR");
    let o = c.output().unwrap();
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(!w.root.join("xdg").join("runbook").exists());
}

/// A hook that leaves in the private directory what a call killed with SIGKILL leaves there: a home
/// of an adapter with a copy of the login, a link to the real login file inside it, and a link to a
/// program, as the adapters make them.
fn leave_a_killed_calls_home(w: &World, run: &str) -> String {
    let p = private_dir(w, run).display().to_string();
    format!(
        "h=\"{p}/login-home.AbCdEf\"; mkdir -p \"$h/.claude\" \"$h/.local/bin\" \"$h/.tmpXyZ/tmp\"\n\
         cp \"{p}/login\" \"$h/.claude/.credentials.json\"\n\
         ln -s \"$(readlink \"{p}/login\")\" \"$h/.tmpXyZ/link-to-the-real-login\"\n\
         ln -s \"$(readlink \"{p}/login\")\" \"$h/.local/bin/claude\"\n\
         mkdir -p \"{p}/config-home.GhIjKl\" && cp \"{p}/login\" \"{p}/config-home.GhIjKl/.credentials.json\"\n\
         touch \"$D/left-a-home\"\n"
    )
}

#[test]
fn a_home_left_in_the_private_directory_by_a_killed_call_is_removed_with_it_and_the_login_file_stays() {
    for (tag, error) in [("login-leftover-end", false), ("login-leftover-error", true)] {
        let w = World::new(tag);
        let login = w.login_path();
        let content = std::fs::read_to_string(&login).unwrap();
        if error {
            w.fake.answer("tree_open", "default", "", 1);
            w.fake.hook("tree_open", "default", &leave_a_killed_calls_home(&w, "r1"));
        } else {
            w.one_decision_tree();
            w.fake.hook("tree_run", "1", &leave_a_killed_calls_home(&w, "r1"));
        }
        w.scripted_approve();
        let o = w.run(&w.args("r1", 1_000_000));
        assert_eq!(code(&o), if error { 1 } else { 0 }, "{tag}: {}", stderr(&o));
        assert!(w.fake.dir.join("left-a-home").exists(), "{tag}: the hook did not run");
        assert!(!private_dir(&w, "r1").exists(), "{tag}: the private directory is left with a home in it");
        assert_eq!(std::fs::read_to_string(&login).unwrap(), content, "{tag}: the login file itself was changed or removed");
        let mut hits = Vec::new();
        grep_tree(&w.run_dir("r1"), &content, &mut hits);
        assert!(hits.is_empty(), "{tag}: the login is in {hits:?}");
    }
}
