//! The copy of a run's context into its workspace (`runbook::context_copy`, story the-context-copy-skips-git-and-excluded-
//! paths): `.git` and the exclude list are left out, a run adds names but removes none of the daemon's, and a context over
//! a bound fails the start with a refusal that names the bound and copies nothing.

use super::commands::{config, routes};
use super::state::scratch;
use runbook::commands::{Daemon, Delegate, Fault, Policy, SubmitRun};
use runbook::context_copy::CopyRules;
use runbook::runtime::Confinement;
use runbook::stub::{Script, Step, StubRuntime};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Rig {
    dir: PathBuf,
    d: Daemon,
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.d.stop();
    }
}

fn rig(tag: &str, copy: CopyRules) -> Rig {
    let dir = scratch(tag);
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    std::fs::create_dir_all(dir.join("runbooks")).unwrap();
    let stub = Arc::new(StubRuntime::new());
    stub.script("quick", Script::new(vec![Step::End("done".into())]));
    let policy = Policy { default_confinement: Some(Confinement::Linux), routes: routes(), copy, ..Policy::default() };
    let d = Daemon::start_with(config(&dir), policy, stub).unwrap();
    Rig { dir, d }
}

impl Rig {
    fn ctx(&self) -> PathBuf {
        self.dir.join("ctx/proj")
    }

    fn submit(&self, exclude: &[&str]) -> String {
        let input = SubmitRun {
            runbook_id: "quick".into(),
            intake: "x".into(),
            context_path: Some(self.ctx().to_string_lossy().into_owned()),
            workspace_exclude: exclude.iter().map(|e| (*e).to_owned()).collect(),
            ..SubmitRun::default()
        };
        let o = self.d.submit_run(input).unwrap();
        assert_eq!(o.name, "submitted", "{o:?}");
        o.run_id.unwrap()
    }

    fn wait(&self, run: &str, want: &str) -> Value {
        let until = Instant::now() + Duration::from_secs(20);
        loop {
            let row = self.d.run_detail(run)[0].clone();
            if row["state"] == want {
                return row;
            }
            assert!(Instant::now() < until, "run {run} is {} and not {want}: {row}", row["state"]);
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn workspace(&self, run: &str) -> PathBuf {
        self.dir.join("state/workspaces").join(run)
    }

    /// The record of the run's event `runbook.context-copied`, if it has one.
    fn copied(&self, run: &str) -> Option<Value> {
        self.d.events_after(run, 0).into_iter().find(|e| e["kind"] == "runbook.context-copied").map(|e| e["record"].clone())
    }
}

fn file(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

/// Every path under `root`, relative, sorted; a directory ends in `/`.
fn tree(root: &Path) -> Vec<String> {
    fn walk(root: &Path, at: &Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(at).unwrap() {
            let e = e.unwrap();
            let rel = e.path().strip_prefix(root).unwrap().to_string_lossy().into_owned();
            if e.file_type().unwrap().is_dir() {
                out.push(format!("{rel}/"));
                walk(root, &e.path(), out);
            } else {
                out.push(rel);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

fn files(root: &Path) -> Vec<String> {
    tree(root).into_iter().filter(|p| !p.ends_with('/')).collect()
}

/// Acceptance 1: `.git/`, a `.git` file, `target/`, `a/node_modules/` and a link are left out; `src/lib.rs` is copied.
/// The run's record names the exclude list and counts what it copied and skipped, without listing the skipped paths.
#[test]
fn the_copy_leaves_out_git_the_default_list_at_any_depth_and_links() {
    let r = rig("cc-default", CopyRules::default());
    let ctx = r.ctx();
    file(&ctx, ".git/HEAD", "ref: refs/heads/main\n");
    file(&ctx, ".git/objects/ab/cdef", "object");
    file(&ctx, "a/.git", "gitdir: ../elsewhere\n");
    file(&ctx, "target/debug/app", "binary");
    file(&ctx, "a/node_modules/pkg/index.js", "js");
    file(&ctx, "src/lib.rs", "pub fn f() {}\n");
    let outside = r.dir.join("outside.txt");
    std::fs::write(&outside, "secret").unwrap();
    std::os::unix::fs::symlink(&outside, ctx.join("src/link.rs")).unwrap();
    let run = r.submit(&[]);
    r.wait(&run, "Ended");
    let ws = r.workspace(&run);
    assert_eq!(files(&ws), vec!["src/lib.rs"], "the workspace holds src/lib.rs only: {:?}", tree(&ws));
    assert!(!ws.join(".git").exists() && !ws.join("a/.git").exists() && !ws.join("target").exists() && !ws.join("a/node_modules").exists());
    let record = r.copied(&run).expect("the run records its copy");
    assert_eq!(record["exclude"], json!([".git", "target/", "node_modules/"]), "{record}");
    // copied: a/, src/, src/lib.rs; skipped: .git, a/.git, a/node_modules, target, src/link.rs
    assert_eq!((record["entries"].clone(), record["bytes"].clone(), record["skipped"].clone()), (json!(3), json!(14), json!(5)), "{record}");
    assert!(!record.to_string().contains("link.rs") && !record.to_string().contains("objects"), "skipped entries are counted, not listed: {record}");
}

/// A name matches a whole name only: one that starts with an excluded name, or holds it, is copied.
#[test]
fn a_name_that_only_starts_with_an_excluded_name_is_copied() {
    let r = rig("cc-near", CopyRules::default());
    let ctx = r.ctx();
    for rel in ["targets/a", "target_x/b", "node_modules_x/c", "x/.github/d", "x/.gitignore", "x/my-target/e", ".git2/f"] {
        file(&ctx, rel, "k");
    }
    let run = r.submit(&[]);
    r.wait(&run, "Ended");
    assert_eq!(files(&r.workspace(&run)), vec![".git2/f", "node_modules_x/c", "target_x/b", "targets/a", "x/.github/d", "x/.gitignore", "x/my-target/e"]);
    assert_eq!(r.copied(&run).unwrap()["skipped"], json!(0));
}

/// The daemon's list replaces the default (acceptance 2 at the daemon; the file's key is read in `config.rs`'s case);
/// a run adds its own names and a relative path from the context root (acceptance 3), and cannot remove the daemon's.
#[test]
fn the_daemons_list_replaces_the_default_and_a_run_adds_names_and_paths_but_removes_none() {
    let r = rig("cc-own", CopyRules { exclude: vec!["build/".into()], ..CopyRules::default() });
    let ctx = r.ctx();
    for rel in ["build/out", "target/t", "fixtures/big", "src/fixtures/more", "docs/big/x", "docs/small/y", "other/docs/big/z", "keep"] {
        file(&ctx, rel, "k");
    }
    // `lib/fixtures` is a file: an entry that ends in / leaves out directories only
    file(&ctx, "lib/fixtures", "a file");
    let run = r.submit(&["fixtures/", "docs/big/", "build/"]);
    r.wait(&run, "Ended");
    assert_eq!(files(&r.workspace(&run)), vec!["docs/small/y", "keep", "lib/fixtures", "other/docs/big/z", "target/t"]);
    assert_eq!(r.copied(&run).unwrap()["exclude"], json!([".git", "build/", "fixtures/", "docs/big/"]), "the daemon's first, a repeated name once");
    // a delegation adds its names too, and the daemon's build/ stays left out
    let d = Delegate {
        request: "x".into(),
        model_route: Some("default".into()),
        protocol: Some("quick".into()),
        workspace: Some(ctx.to_string_lossy().into_owned()),
        workspace_exclude: vec!["fixtures/".into()],
        ..Delegate::default()
    };
    let o = r.d.delegate(d).unwrap();
    assert_eq!(o.name, "delegated", "{o:?}");
    let run = o.run_id.unwrap();
    r.wait(&run, "Ended");
    assert_eq!(files(&r.workspace(&run)), vec!["docs/big/x", "docs/small/y", "keep", "lib/fixtures", "other/docs/big/z", "target/t"], "a delegation's fixtures/ is left out besides the daemon's build/");
    assert_eq!(r.copied(&run).unwrap()["exclude"], json!([".git", "build/", "fixtures/"]));
}

/// Acceptance 4: a context over the entry bound, or over the byte bound, ends the start with a refusal naming the bound,
/// and the workspace holds nothing of it.
#[test]
fn a_context_over_a_bound_fails_the_start_naming_the_bound_and_copies_nothing() {
    let r = rig("cc-bound", CopyRules { max_entries: 10, max_bytes: 100, ..CopyRules::default() });
    let ctx = r.ctx();
    for i in 0..12 {
        file(&ctx, &format!("d/f{i:02}"), "k");
    }
    let run = r.submit(&[]);
    let row = r.wait(&run, "FailedToStart");
    let failure = row["failure"].as_str().unwrap_or_default().to_owned();
    assert!(failure.starts_with("context-over-bound: ") && failure.contains("10 entries") && failure.contains("copy_max_entries"), "{row}");
    assert_eq!(tree(&r.workspace(&run)), Vec::<String>::new(), "nothing of the context is copied");
    assert!(r.copied(&run).is_none());

    // the same files under an excluded name are not counted beyond the one entry
    let run = r.submit(&["d/"]);
    r.wait(&run, "Ended");

    std::fs::remove_dir_all(ctx.join("d")).unwrap();
    file(&ctx, "a", "small");
    file(&ctx, "z/big", &"b".repeat(101));
    let run = r.submit(&[]);
    let row = r.wait(&run, "FailedToStart");
    let failure = row["failure"].as_str().unwrap_or_default().to_owned();
    assert!(failure.starts_with("context-over-bound: ") && failure.contains("100 bytes") && failure.contains("copy_max_bytes"), "{row}");
    assert_eq!(tree(&r.workspace(&run)), Vec::<String>::new(), "a file before the one over the bound is not copied either");
}

/// An exclude entry that is no name or relative path is refused before anything is written, naming the member and never
/// the value.
#[test]
fn an_exclude_entry_that_is_no_name_or_relative_path_is_refused_by_the_member() {
    let r = rig("cc-refused", CopyRules::default());
    std::fs::create_dir_all(r.ctx()).unwrap();
    for bad in ["", "/", "/etc/", "../up/", "a/../b", "a//b", "./a", "q\u{0}x", &"n".repeat(201)] {
        let submit = SubmitRun {
            runbook_id: "quick".into(),
            intake: "x".into(),
            context_path: Some(r.ctx().to_string_lossy().into_owned()),
            workspace_exclude: vec!["ok/".into(), bad.to_owned()],
            ..SubmitRun::default()
        };
        let delegate = Delegate { request: "x".into(), protocol: Some("quick".into()), workspace_exclude: vec![bad.to_owned()], ..Delegate::default() };
        for (what, out) in [("submit", r.d.submit_run(submit)), ("delegate", r.d.delegate(delegate))] {
            match out {
                Err(Fault::Invalid(m)) => assert!(m.starts_with("workspace_exclude: ") && (bad.len() < 3 || !m.contains(bad)), "{what} {bad:?}: {m}"),
                other => panic!("{what} {bad:?}: {other:?}"),
            }
        }
    }
    let many: Vec<String> = (0..65).map(|i| format!("n{i}")).collect();
    let out = r.d.submit_run(SubmitRun { runbook_id: "quick".into(), intake: "x".into(), workspace_exclude: many, ..SubmitRun::default() });
    assert!(matches!(&out, Err(Fault::Invalid(m)) if m.contains("more than 64")), "{out:?}");
    assert!(r.d.runs().is_empty(), "a refused submit writes no run");
}

/// Acceptance 2 through the binary: `copy_exclude = ["build/"]` in the daemon's `runbook.toml` leaves out `build/`, and
/// `target/` is copied.
#[test]
fn copy_exclude_of_runbook_toml_leaves_out_its_names_and_copies_target() {
    use super::default_confinement::{live, post, rows};
    let root = scratch("cc-file-ctx");
    let ctx = root.join("proj");
    for rel in ["build/out", "target/t", "src/lib.rs"] {
        file(&ctx, rel, "k");
    }
    let root_arg = root.to_string_lossy().into_owned();
    let l = live("cc-file", &["--context-root", &root_arg, "--default-confinement", "linux"], Some("[delegation]\ncopy_exclude = [\"build/\"]\n"));
    let (status, body) = post(&l, &json!({"runbook_id": runbook::serve::STUB_DECISION, "intake": "x", "context_path": ctx.to_string_lossy()}));
    assert_eq!((status, body["outcome"].clone()), (202, json!("submitted")), "{body}");
    let run = body["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned();
    let until = Instant::now() + Duration::from_secs(20);
    let record = loop {
        let events = rows(&l, &format!("/daemon/views/events?run_id={run}&after_position=0"));
        if let Some(e) = events.iter().find(|e| e["kind"] == "runbook.context-copied") {
            break e["record"].clone();
        }
        assert!(Instant::now() < until, "no runbook.context-copied: {events:?}");
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(record["exclude"], json!([".git", "build/"]), "{record}");
    assert_eq!(files(&l.dir.join("state/workspaces").join(&run)), vec!["src/lib.rs", "target/t"]);
}
