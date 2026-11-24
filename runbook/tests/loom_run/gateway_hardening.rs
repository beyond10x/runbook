//! Cases of the contract of `docs/loom-run-settings.md` ("`[gateway]`"): the token file, the signals that end a run,
//! the docs. Invented strings only; no model, no network.

use super::core::{code, stderr, toml_text, World};
use serde_json::{json, Value};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

const TOKEN: &str = "review-token-91c0d7e2ab35";
const BASE: &str = "http://127.0.0.1:8080";

fn private_dir(w: &World, run: &str) -> PathBuf {
    w.root.join("xdg").join("runbook").join(run)
}

fn settings(w: &World, gateway: Value) {
    let scripted = json!({"kind": "scripted", "max_call_micro_usd": 0, "answers": [{"approve": true, "principal": "person:owner", "cost_micro_usd": 0, "model_calls": 0}]});
    std::fs::write(w.root.join("settings.toml"), toml_text(&json!({"decider": scripted, "gateway": gateway}))).unwrap();
}

/// Waits for the runner for `secs`; a runner that is still there is killed and the case says so.
fn ended_within(child: &mut Child, secs: u64) -> Option<std::process::ExitStatus> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return Some(status);
        }
        if start.elapsed().as_secs() >= secs {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn token_files_dir(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("review-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_token_file_that_is_a_fifo_is_refused_by_name_and_the_runner_does_not_hang() {
    let w = World::new("review-fifo");
    w.one_decision_tree();
    let dir = token_files_dir("fifo");
    let fifo = dir.join("token-pipe");
    assert!(std::process::Command::new("mkfifo").arg(&fifo).status().unwrap().success());
    settings(&w, json!({"base_url": BASE, "token_file": fifo.display().to_string()}));
    let mut child = w.command(&w.args("r1", 1_000_000)).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap();
    let status = ended_within(&mut child, 15).expect("the runner did not end within 15 s on a token_file that is a FIFO (it was killed)");
    assert_eq!(status.code(), Some(2));
    assert!(!w.run_dir("r1").exists() && !private_dir(&w, "r1").exists());
}

#[test]
fn a_token_file_that_is_a_link_is_refused_by_name_and_not_followed() {
    let w = World::new("review-link");
    w.one_decision_tree();
    let dir = token_files_dir("link");
    let real = dir.join("real-holder");
    std::fs::write(&real, format!("{TOKEN}\n")).unwrap();
    let link = dir.join("link-holder");
    symlink(&real, &link).unwrap();
    settings(&w, json!({"base_url": BASE, "token_file": link.display().to_string()}));
    let o = w.run(&w.args("r1", 1_000_000));
    let said = stderr(&o);
    assert_eq!(code(&o), 2, "a token_file that is a link was accepted: {said}");
    assert!(said.contains("gateway.token_file") && !said.contains(TOKEN) && !said.contains("link-holder"), "{said}");
    assert!(!w.run_dir("r1").exists() && !private_dir(&w, "r1").exists());
}

#[test]
fn the_private_directory_is_gone_after_sighup_too() {
    let w = World::new("review-hup");
    w.fake.answer("tree_run", "1", "", 0);
    w.fake.hook("tree_run", "1", "echo $$ > \"$D/pid\"\nsleep 30\n");
    settings(&w, json!({"base_url": BASE, "token": TOKEN}));
    let mut child = w.command(&w.args("r1", 1_000_000)).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let started = Instant::now();
    while !w.fake.dir.join("pid").exists() {
        assert!(started.elapsed().as_secs() < 20, "the fake tree run did not start");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(private_dir(&w, "r1").join("gateway").exists());
    assert!(std::process::Command::new("kill").args(["-HUP", &child.id().to_string()]).status().unwrap().success());
    let ended = ended_within(&mut child, 8);
    if let Ok(pid) = std::fs::read_to_string(w.fake.dir.join("pid")) {
        let _ = std::process::Command::new("kill").args(["-KILL", pid.trim()]).status();
    }
    assert!(ended.is_some(), "the runner did not end after SIGHUP");
    assert!(!private_dir(&w, "r1").join("gateway").exists(), "the gateway file with the token is left in the runtime directory after SIGHUP");
}

#[test]
fn the_docs_say_what_a_plain_http_address_that_is_not_loopback_does_with_the_token() {
    let doc = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("docs/loom-run-settings.md")).unwrap();
    let start = doc.find("## `[gateway]`").expect("the page has the [gateway] section");
    let section = &doc[start..];
    let section = &section[..section[3..].find("\n## ").map_or(section.len(), |i| i + 3)];
    let lower = section.to_lowercase();
    assert!(
        ["clear text", "cleartext", "plain text", "plaintext", "unencrypted", "not encrypted", "in the clear"].iter().any(|w| lower.contains(w)),
        "base_url accepts http:// at a host that is not loopback and the section never says the token then travels in clear text"
    );
}

/// The runner on the settings of the world, ended within 15 s (a runner still there is killed and the case says so);
/// its exit code and what it printed on standard error.
fn runs_to_the_end(w: &World, tag: &str) -> (i32, String) {
    use std::io::Read;
    let mut child = w.command(&w.args("r1", 1_000_000)).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap();
    let status = ended_within(&mut child, 15).unwrap_or_else(|| panic!("{tag}: the runner did not end within 15 s"));
    let mut said = String::new();
    child.stderr.take().unwrap().read_to_string(&mut said).unwrap();
    (status.code().unwrap_or(-1), said)
}

#[test]
fn guard_a_token_file_that_is_unreadable_too_large_not_text_or_has_a_stray_cr_is_refused_by_name_only() {
    let dir = token_files_dir("faults");
    let unreadable = dir.join("unreadable-holder");
    std::fs::write(&unreadable, format!("{TOKEN}\n")).unwrap();
    std::fs::set_permissions(&unreadable, std::os::unix::fs::PermissionsExt::from_mode(0o000_u32)).unwrap();
    let large = dir.join("large-holder");
    std::fs::write(&large, "x".repeat(20_000)).unwrap();
    let binary = dir.join("binary-holder");
    std::fs::write(&binary, [0xffu8, 0xfe, 0x41, 0x0a]).unwrap();
    let stray = dir.join("stray-cr-holder");
    std::fs::write(&stray, format!("{TOKEN}\rsecond\n")).unwrap();
    for (tag, path) in [("unreadable", &unreadable), ("large", &large), ("binary", &binary), ("stray-cr", &stray)] {
        let w = World::new(&format!("review-fault-{tag}"));
        w.one_decision_tree();
        settings(&w, json!({"base_url": BASE, "token_file": path.display().to_string()}));
        let (exit, said) = runs_to_the_end(&w, tag);
        assert_eq!(exit, 2, "{tag}: {said}");
        assert!(said.contains("gateway.token_file") && !said.contains(TOKEN) && !said.contains(&path.display().to_string()) && !said.contains("xxxxxxxx"), "{tag}: {said}");
        assert!(!w.run_dir("r1").exists() && !private_dir(&w, "r1").exists(), "{tag}");
    }
}

#[test]
fn a_token_file_that_is_a_directory_a_socket_or_a_device_is_refused_by_name_only() {
    use std::os::fd::AsRawFd;
    let dir = token_files_dir("kinds");
    let folder = dir.join("folder-holder");
    std::fs::create_dir(&folder).unwrap();
    // The path of a socket is short enough for `bind` through the directory's descriptor.
    let holder = std::fs::File::open(&dir).unwrap();
    let _listener = std::os::unix::net::UnixListener::bind(format!("/proc/self/fd/{}/socket-holder", holder.as_raw_fd())).unwrap();
    let socket = dir.join("socket-holder");
    for (tag, path) in [("directory", folder), ("socket", socket), ("device", PathBuf::from("/dev/zero"))] {
        let w = World::new(&format!("review-kind-{tag}"));
        w.one_decision_tree();
        settings(&w, json!({"base_url": BASE, "token_file": path.display().to_string()}));
        let (exit, said) = runs_to_the_end(&w, tag);
        assert_eq!(exit, 2, "{tag}: {said}");
        assert!(said.contains("gateway.token_file") && !said.contains(&path.display().to_string()), "{tag}: {said}");
        assert!(!w.run_dir("r1").exists() && !private_dir(&w, "r1").exists(), "{tag}");
    }
}

#[test]
fn a_token_of_fewer_than_8_bytes_is_refused_by_name_only_inline_and_in_a_file_and_one_of_8_is_taken() {
    let dir = token_files_dir("short");
    let file = dir.join("short-holder");
    std::fs::write(&file, "short12\n").unwrap();
    for (tag, table, member) in [
        ("inline", json!({"base_url": BASE, "token": "short12"}), "gateway.token"),
        ("file", json!({"base_url": BASE, "token_file": file.display().to_string()}), "gateway.token_file"),
    ] {
        let w = World::new(&format!("review-short-{tag}"));
        w.one_decision_tree();
        settings(&w, table);
        let (exit, said) = runs_to_the_end(&w, tag);
        assert_eq!(exit, 2, "{tag}: {said}");
        assert!(said.contains(member) && !said.contains("short12"), "{tag}: {said}");
        assert!(!w.run_dir("r1").exists() && !private_dir(&w, "r1").exists(), "{tag}");
    }
    let w = World::new("review-short-eight");
    w.one_decision_tree();
    settings(&w, json!({"base_url": BASE, "token": "short123"}));
    let (exit, said) = runs_to_the_end(&w, "eight");
    assert_eq!(exit, 0, "{said}");
}

#[test]
fn guard_a_private_directory_that_exists_is_not_started_over_and_is_left_as_it_was() {
    let w = World::new("review-exists");
    w.one_decision_tree();
    std::fs::create_dir_all(private_dir(&w, "r1")).unwrap();
    std::fs::write(private_dir(&w, "r1").join("keep"), "mine").unwrap();
    settings(&w, json!({"base_url": BASE, "token": TOKEN}));
    let (exit, said) = runs_to_the_end(&w, "exists");
    assert_ne!(exit, 0, "{said}");
    assert!(!said.contains(TOKEN), "{said}");
    assert_eq!(std::fs::read_to_string(private_dir(&w, "r1").join("keep")).unwrap(), "mine");
    assert!(!private_dir(&w, "r1").join("gateway").exists(), "a gateway file was written into a directory that was there");
    assert!(!w.run_dir("r1").exists());
    assert_eq!(w.fake.calls_so_far(), 0);
}

#[test]
fn guard_an_empty_table_and_a_file_address_are_refused_by_name_before_a_run_directory_exists() {
    for (tag, table) in [("empty", json!({})), ("file-scheme", json!({"base_url": "file:///etc/hosts", "token": TOKEN})), ("only-token-file", json!({"token_file": "/absolute/holder"}))] {
        let w = World::new(&format!("review-shape-{tag}"));
        w.one_decision_tree();
        settings(&w, table);
        let (exit, said) = runs_to_the_end(&w, tag);
        assert_eq!(exit, 2, "{tag}: {said}");
        assert!(said.contains("gateway") && !said.contains(TOKEN) && !said.contains("/etc/hosts") && !said.contains("/absolute/holder"), "{tag}: {said}");
        assert!(!w.run_dir("r1").exists() && !private_dir(&w, "r1").exists(), "{tag}");
        assert_eq!(w.fake.calls_so_far(), 0, "{tag}");
    }
}
