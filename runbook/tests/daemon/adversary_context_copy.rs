//! Attacks on the copy of a run's context (`runbook::context_copy`, story the-context-copy-skips-git-and-excluded-paths):
//! a directory swapped for a link while the copy runs, the bounds at their edge, a copy that fails while it writes, a name
//! that is no UTF-8, the run's own exclude list across a restart and through the HTTP and MCP surfaces.

use super::commands::Fx;
use super::state::scratch;
use runbook::commands::SubmitRun;
use runbook::context_copy::{copy, CopyError, CopyRules};
use serde_json::{json, Value};
use std::ffi::{CString, OsStr};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    if root.exists() {
        walk(root, root, &mut out);
    }
    out.sort();
    out
}

fn c(p: &Path) -> CString {
    CString::new(p.as_os_str().as_bytes()).unwrap()
}

/// The module's promise: "a link inside a context root cannot carry a file from outside it into a run". A directory of the
/// context that is swapped for a link to a directory outside it while the copy runs (a checkout another process changes)
/// must not carry the outside files into the workspace: every directory is opened below the root without following a link.
#[test]
fn adversary_context_copy_a_directory_swapped_for_a_link_carries_no_outside_file() {
    let root = scratch("acc-swap");
    let ctx = root.join("ctx");
    file(&ctx, "a/k.txt", "inside");
    for i in 0..150 {
        file(&ctx, &format!("p{i:03}"), "pad");
    }
    let outside = root.join("outside");
    file(&outside, "k.txt", "OUTSIDE");
    file(&outside, "secret", "OUTSIDE");
    let link = root.join("swap");
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    let (a, l) = (c(&ctx.join("a")), c(&link));
    let stop = Arc::new(AtomicBool::new(false));
    let swapper = {
        let stop = stop.clone();
        std::thread::spawn(move || {
            let mut n = 0u64;
            while !stop.load(Ordering::SeqCst) {
                // ctx/a and the link trade places in one step: ctx/a is a directory or a link to `outside`, never missing
                let r = unsafe { libc::renameat2(libc::AT_FDCWD, a.as_ptr(), libc::AT_FDCWD, l.as_ptr(), libc::RENAME_EXCHANGE) };
                assert_eq!(r, 0, "renameat2: {}", std::io::Error::last_os_error());
                n += 1;
            }
            if n % 2 == 1 {
                unsafe { libc::renameat2(libc::AT_FDCWD, a.as_ptr(), libc::AT_FDCWD, l.as_ptr(), libc::RENAME_EXCHANGE) };
            }
        })
    };
    let until = Instant::now() + Duration::from_secs(1);
    let (mut tries, mut copied, mut leaked) = (0u32, 0u32, None);
    while Instant::now() < until {
        tries += 1;
        let to = root.join("ws").join(tries.to_string());
        if copy(&ctx, &to, &CopyRules::default(), &[]).is_ok() {
            copied += 1;
            let carried: Vec<String> = tree(&to).into_iter().filter(|p| !p.ends_with('/') && std::fs::read(to.join(p)).unwrap() == b"OUTSIDE").collect();
            if !carried.is_empty() {
                leaked = Some(carried);
                break;
            }
        }
        let _ = std::fs::remove_dir_all(&to);
    }
    stop.store(true, Ordering::SeqCst);
    swapper.join().unwrap();
    assert!(leaked.is_none(), "after {tries} copies ({copied} of them Ok) one carried files from outside the context into the workspace: {leaked:?}");
}

/// The bounds at their edge: exactly `copy_max_entries` entries and exactly `copy_max_bytes` bytes are copied; one more of
/// either is refused, and nothing is written.
#[test]
fn adversary_context_copy_the_bounds_hold_at_their_edge() {
    let root = scratch("acc-edge");
    let ctx = root.join("ctx");
    // entries: d/, d/a, d/b, .git (skipped, counted) = 4; bytes: 3 + 7 = 10
    file(&ctx, "d/a", "aaa");
    file(&ctx, "d/b", "bbbbbbb");
    file(&ctx, ".git", "gitdir: x\n");
    let at = |n: u64, b: u64| CopyRules { max_entries: n, max_bytes: b, ..CopyRules::default() };
    let to = root.join("ws-ok");
    let done = copy(&ctx, &to, &at(4, 10), &[]).expect("exactly at both bounds");
    assert_eq!((done.entries, done.bytes, done.skipped), (3, 10, 1));
    assert_eq!(tree(&to), vec!["d/", "d/a", "d/b"]);
    for (rules, key) in [(at(3, 10), "copy_max_entries"), (at(4, 9), "copy_max_bytes")] {
        let to = root.join(format!("ws-{key}"));
        match copy(&ctx, &to, &rules, &[]) {
            Err(CopyError::OverBound { key: k, .. }) => assert_eq!(k, key),
            other => panic!("{key}: {other:?}"),
        }
        assert_eq!(tree(&to), Vec::<String>::new(), "{key}: nothing written");
    }
}

/// A copy that fails while it writes (a file the daemon's user cannot read, after other files were written) removes what it
/// wrote: the workspace holds nothing of the context. The failure names no other path than the one that failed.
#[test]
fn adversary_context_copy_a_copy_that_fails_while_it_writes_leaves_nothing() {
    let root = scratch("acc-fail");
    let ctx = root.join("ctx");
    file(&ctx, "a/one", "1");
    file(&ctx, "b", "2");
    file(&ctx, "c/d/e", "3");
    file(&ctx, "zz", "unreadable");
    std::fs::set_permissions(ctx.join("zz"), std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read(ctx.join("zz")).is_ok() {
        // the test runs with the power to read any file: nothing to observe
        eprintln!("skipped: the test's user reads a file of mode 000");
        return;
    }
    let to = root.join("ws");
    std::fs::create_dir_all(&to).unwrap();
    file(&to, "daemon-own", "kept");
    let out = copy(&ctx, &to, &CopyRules::default(), &[]);
    std::fs::set_permissions(ctx.join("zz"), std::fs::Permissions::from_mode(0o644)).unwrap();
    match &out {
        Err(e @ CopyError::Failed(_)) => {
            let why = e.reason();
            assert!(why.contains("zz") && !why.contains("one") && !why.contains("c/d"), "{why}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(tree(&to), vec!["daemon-own"], "what the copy wrote is removed, what was there before stays");
}

/// A name that is no UTF-8 is copied as its bytes, and is no match for an exclude entry it does not equal; an excluded
/// directory beside it is left out.
#[test]
fn adversary_context_copy_a_name_that_is_no_utf8_is_copied() {
    let root = scratch("acc-utf8");
    let ctx = root.join("ctx");
    std::fs::create_dir_all(ctx.join("target")).unwrap();
    std::fs::write(ctx.join("target/x"), "t").unwrap();
    let odd = OsStr::from_bytes(b"n\xffame");
    std::fs::create_dir_all(ctx.join(odd)).unwrap();
    std::fs::write(ctx.join(odd).join("f"), "k").unwrap();
    let to = root.join("ws");
    let done = copy(&ctx, &to, &CopyRules::default(), &[]).unwrap();
    assert_eq!(std::fs::read(to.join(odd).join("f")).unwrap(), b"k");
    assert!(!to.join("target").exists());
    assert_eq!((done.entries, done.skipped), (2, 1));
}

/// A run with a serial key that waits behind another and is taken up again after a restart keeps its own exclude list: the
/// copy reads it from the run's `runbook.submitted`, which the journal holds.
#[test]
fn adversary_context_copy_a_waiting_run_keeps_its_exclude_list_across_a_restart() {
    let mut fx = Fx::new("acc-restart");
    let ctx = fx.dir.join("ctx/proj");
    file(&ctx, "fixtures/big", "f");
    file(&ctx, "keep", "k");
    let hold = fx.submit_with(SubmitRun { runbook_id: "hold".into(), intake: "x".into(), serial_key: Some("k".into()), ..SubmitRun::default() });
    let run = fx.submit_with(SubmitRun {
        runbook_id: "quick".into(),
        intake: "x".into(),
        serial_key: Some("k".into()),
        context_path: Some(ctx.to_string_lossy().into_owned()),
        workspace_exclude: vec!["fixtures/".into()],
        ..SubmitRun::default()
    });
    fx.wait_state(&hold, "Running");
    fx.wait_state(&run, "WaitingOnKey");
    fx.restart();
    fx.wait_state(&run, "Ended");
    assert_eq!(tree(&fx.dir.join("state/workspaces").join(&run)), vec!["keep"]);
    let record = fx.d().events_after(&run, 0).into_iter().find(|e| e["kind"] == "runbook.context-copied").map(|e| e["record"].clone()).unwrap();
    assert_eq!(record["exclude"], json!([".git", "target/", "node_modules/", "fixtures/"]), "{record}");
}

/// The HTTP surface takes a submit's `workspace_exclude` and the copy applies it; MCP's tools offer the member.
#[test]
fn adversary_context_copy_http_and_mcp_carry_workspace_exclude() {
    use super::default_confinement::{live, post, rows};
    let top = scratch("acc-http-ctx");
    let ctx = top.join("proj");
    for rel in ["fixtures/f", "a/fixtures", "src/lib.rs"] {
        file(&ctx, rel, "k");
    }
    let top_arg = top.to_string_lossy().into_owned();
    let l = live("acc-http", &["--context-root", &top_arg, "--default-confinement", "linux"], None);
    let (status, body) = post(&l, &json!({"runbook_id": runbook::serve::STUB_DECISION, "intake": "x", "context_path": ctx.to_string_lossy(), "workspace_exclude": ["fixtures/"]}));
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
    assert_eq!(record["exclude"], json!([".git", "target/", "node_modules/", "fixtures/"]), "{record}");
    let ws: PathBuf = l.dir.join("state/workspaces").join(&run);
    assert_eq!(tree(&ws).into_iter().filter(|p| !p.ends_with('/')).collect::<Vec<_>>(), vec!["a/fixtures", "src/lib.rs"]);
    let (status, body) = post(&l, &json!({"runbook_id": runbook::serve::STUB_DECISION, "intake": "x", "context_path": ctx.to_string_lossy(), "workspace_exclude": ["../secret-dir"]}));
    assert_eq!(status, 400, "{body}");
    assert!(!body.to_string().contains("secret-dir"), "{body}");
    for tool in ["delegate", "submit_run"] {
        let t: &Value = runbook::mcp::tools::list().iter().find(|t| t["name"] == tool).unwrap_or_else(|| panic!("no tool {tool}"));
        assert!(t["inputSchema"]["properties"].get("workspace_exclude").is_some(), "{tool}: {}", t["inputSchema"]);
    }
}
