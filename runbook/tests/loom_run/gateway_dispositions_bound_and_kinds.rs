//! Cases of the gateway beside `gateway.rs`, `gateway_hardening.rs` and `gateway_signals_and_files.rs`: the signal
//! dispositions of the runner, an ignored hangup over a whole run, the address bound in the settings reader, the
//! kinds of token file the sentences name, the `O_NONBLOCK` literal of the driver's reader. Invented strings only;
//! no model, no network.

use super::core::{toml_text, World};
use serde_json::{json, Value};
use std::os::unix::fs::symlink;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

const TOKEN: &str = "gwd-token-4b1d9e7a3c52";
const BASE: &str = "http://127.0.0.1:8080";
const LEAK: &str = "GWD-CONTENT-MUST-NOT-COME-BACK";

fn private_dir(w: &World, run: &str) -> PathBuf {
    w.root.join("xdg").join("runbook").join(run)
}

fn settings(w: &World, gateway: Value) {
    let scripted = json!({"kind": "scripted", "max_call_micro_usd": 0, "answers": [{"approve": true, "principal": "person:owner", "cost_micro_usd": 0, "model_calls": 0}]});
    std::fs::write(w.root.join("settings.toml"), toml_text(&json!({"decider": scripted, "gateway": gateway}))).unwrap();
}

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
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for(path: &Path, secs: u64, what: &str) {
    let started = Instant::now();
    while !path.exists() {
        assert!(started.elapsed().as_secs() < secs, "{what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn signal(name: &str, pid: u32) {
    let _ = std::process::Command::new("kill").args([format!("-{name}"), pid.to_string()]).stderr(Stdio::null()).status();
}

fn kill_hook_pid(w: &World) {
    if let Ok(pid) = std::fs::read_to_string(w.fake.dir.join("pid")) {
        let _ = std::process::Command::new("kill").args(["-KILL", pid.trim()]).status();
    }
}

/// How the caller sets the three signals before it starts the runner.
#[derive(Clone, Copy)]
struct Caller {
    hup: bool,
    int: bool,
    term: bool,
}

fn start_runner(w: &World, caller: Caller) -> Child {
    let mut cmd = w.command(&w.args("r1", 1_000_000));
    // What `nohup` does before it runs the command: the signal is ignored and the ignore is inherited through exec.
    unsafe {
        cmd.pre_exec(move || {
            for (set, sig) in [(caller.hup, libc::SIGHUP), (caller.int, libc::SIGINT), (caller.term, libc::SIGTERM)] {
                if set {
                    libc::signal(sig, libc::SIG_IGN);
                }
            }
            Ok(())
        });
    }
    cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap()
}

/// The `SigIgn` and `SigCgt` masks of a process, from `/proc/<pid>/status`.
fn masks(pid: u32) -> (u64, u64) {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap();
    let get = |key: &str| status.lines().find_map(|l| l.strip_prefix(key)).map(|v| u64::from_str_radix(v.trim(), 16).unwrap()).unwrap();
    (get("SigIgn:"), get("SigCgt:"))
}

const HUP: u64 = 1 << 0;
const INT: u64 = 1 << 1;
const TERM: u64 = 1 << 14;

fn runner_in_a_run(tag: &str, caller: Caller) -> ((u64, u64), World, Child) {
    let w = World::new(tag);
    w.fake.answer("tree_run", "1", "", 0);
    w.fake.hook("tree_run", "1", "echo $$ > \"$D/pid\"\nsleep 30\n");
    settings(&w, json!({"base_url": BASE, "token": TOKEN}));
    let child = start_runner(&w, caller);
    wait_for(&w.fake.dir.join("pid"), 20, "the fake tree run did not start");
    let m = masks(child.id());
    (m, w, child)
}

/// Ends the runner with SIGTERM, which ends the process group of its fake (the shell and its `sleep`), then kills the
/// hook's shell should it still be there. The other order left the `sleep` holding the runner's pipes, and the runner
/// waited for them until this function killed it 8 s later.
fn finish(w: &World, mut child: Child) {
    let _ = std::process::Command::new("kill").args(["-TERM", &child.id().to_string()]).stderr(Stdio::null()).status();
    let _ = ended_within(&mut child, 8);
    kill_hook_pid(w);
}

// ---- the dispositions, read from the kernel, in a run ----

#[test]
fn the_runner_catches_hup_int_and_term_when_the_caller_ignored_none() {
    let ((ign, cgt), w, child) = runner_in_a_run("gwd-disp-default", Caller { hup: false, int: false, term: false });
    finish(&w, child);
    assert_eq!(cgt & (HUP | INT | TERM), HUP | INT | TERM, "SigCgt {cgt:x}");
    assert_eq!(ign & (HUP | INT | TERM), 0, "SigIgn {ign:x}");
}

#[test]
fn the_runner_leaves_an_ignored_hup_ignored_and_catches_int_and_term() {
    let ((ign, cgt), w, child) = runner_in_a_run("gwd-disp-nohup", Caller { hup: true, int: false, term: false });
    finish(&w, child);
    assert_eq!(ign & HUP, HUP, "SigIgn {ign:x}: SIGHUP is not ignored");
    assert_eq!(cgt & HUP, 0, "SigCgt {cgt:x}: SIGHUP has a handler");
    assert_eq!(cgt & (INT | TERM), INT | TERM, "SigCgt {cgt:x}");
}

#[test]
fn the_runner_catches_int_and_term_whatever_the_caller_ignored_as_before_the_round() {
    // A caller that ignores SIGINT (a shell that starts a job in the background) and SIGTERM: the runner still
    // removes its private directory on both, as it did before the round.
    let ((ign, cgt), w, child) = runner_in_a_run("gwd-disp-all", Caller { hup: true, int: true, term: true });
    finish(&w, child);
    assert_eq!(cgt & (INT | TERM), INT | TERM, "SigCgt {cgt:x}");
    assert_eq!(ign & (INT | TERM), 0, "SigIgn {ign:x}");
    assert_eq!(ign & HUP, HUP);
}

#[test]
fn an_ignored_hup_sent_twice_leaves_the_run_and_its_private_directory_and_a_term_then_removes_it() {
    let (_, w, mut child) = runner_in_a_run("gwd-nohup-run", Caller { hup: true, int: false, term: false });
    let gateway = private_dir(&w, "r1").join("gateway");
    assert!(gateway.exists());
    signal("HUP", child.id());
    std::thread::sleep(Duration::from_millis(300));
    signal("HUP", child.id());
    // A runner that takes a SIGHUP ends in about 0.1 s (`gateway_hardening.rs`); one second is ten times that.
    std::thread::sleep(Duration::from_secs(1));
    let running = child.try_wait().unwrap().is_none();
    let present = gateway.exists();
    signal("TERM", child.id());
    // The runner ends the program it started itself; the stand-in is killed only after the end (or the bound).
    let ended = ended_within(&mut child, 10);
    kill_hook_pid(&w);
    assert!(running, "the run ended on an ignored SIGHUP");
    assert!(present, "the private directory went on an ignored SIGHUP");
    assert!(ended.is_some(), "the runner did not end on SIGTERM");
    assert!(!private_dir(&w, "r1").exists(), "the private directory is left after SIGTERM");
}

#[test]
fn a_hup_at_any_moment_of_the_start_never_leaves_a_private_directory() {
    // The window between reading the disposition and setting the handler, and the start before it: a SIGHUP that
    // is not ignored ends the runner at once (nothing exists yet) or sets the flag (the directory goes with the run).
    for (i, delay_ms) in [0u64, 1, 2, 3, 5, 8, 12, 20, 30, 45].iter().enumerate() {
        let w = World::new(&format!("gwd-race-{i}"));
        w.one_decision_tree();
        settings(&w, json!({"base_url": BASE, "token": TOKEN}));
        let mut child = start_runner(&w, Caller { hup: false, int: false, term: false });
        std::thread::sleep(Duration::from_millis(*delay_ms));
        signal("HUP", child.id());
        assert!(ended_within(&mut child, 20).is_some(), "delay {delay_ms} ms: the runner did not end");
        assert!(!private_dir(&w, "r1").exists(), "delay {delay_ms} ms: the private directory is left");
    }
}

// ---- the address bound in the settings reader ----

fn inline_run(tag: &str, base: &str) -> (i32, String) {
    use std::io::Read;
    let w = World::new(&format!("gwd-{tag}"));
    w.one_decision_tree();
    settings(&w, json!({"base_url": base, "token": TOKEN}));
    let mut child = w.command(&w.args("r1", 1_000_000)).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap();
    let status = ended_within(&mut child, 20).unwrap_or_else(|| panic!("{tag}: the runner did not end within 20 s"));
    let mut said = String::new();
    child.stderr.take().unwrap().read_to_string(&mut said).unwrap();
    if status.code() != Some(0) {
        assert!(!w.run_dir("r1").exists() && !private_dir(&w, "r1").exists(), "{tag}: a directory exists after the refusal");
    }
    (status.code().unwrap_or(-1), said)
}

#[test]
fn the_settings_reader_takes_an_address_of_2048_bytes_and_refuses_2049_and_multibyte_ones_by_name_only() {
    let ascii = |n: usize| format!("http://h/{}", "p".repeat(n - 9));
    let (exit, said) = inline_run("addr-2048", &ascii(2048));
    assert_eq!(exit, 0, "2048 bytes: {said}");
    let mut wrong = Vec::new();
    let two_byte = {
        let mut s = String::from("http://h/");
        s.push_str(&"\u{e9}".repeat(1019));
        s.push('p');
        s
    };
    assert_eq!(two_byte.len(), 2048);
    for (what, base) in [("2049 ASCII bytes", ascii(2049)), ("20,000 ASCII bytes", ascii(20_000)), ("2048 bytes with two-byte characters", two_byte)] {
        let (exit, said) = inline_run("addr-refused", &base);
        let named = said.contains("gateway.base_url");
        let leaked = said.contains("pppp") || said.contains('\u{e9}') || said.contains(TOKEN);
        if exit != 2 || !named || leaked {
            wrong.push(format!("{what}: exit {exit}, named {named}, leaked {leaked}"));
        }
    }
    assert_eq!(wrong, Vec::<String>::new());
}

// ---- the kinds of token file the sentences name ----

fn token_file_run(tag: &str, path: &Path) -> (i32, String) {
    use std::io::Read;
    let w = World::new(&format!("gwd-{tag}"));
    w.one_decision_tree();
    settings(&w, json!({"base_url": BASE, "token_file": path.display().to_string()}));
    let mut child = w.command(&w.args("r1", 1_000_000)).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap();
    let status = ended_within(&mut child, 20).unwrap_or_else(|| panic!("{tag}: the runner did not end within 20 s"));
    let mut said = String::new();
    child.stderr.take().unwrap().read_to_string(&mut said).unwrap();
    if status.code() != Some(0) {
        assert!(!w.run_dir("r1").exists() && !private_dir(&w, "r1").exists(), "{tag}: a directory exists after the refusal");
    }
    (status.code().unwrap_or(-1), said)
}

#[test]
fn a_socket_a_link_to_a_fifo_a_link_to_a_directory_a_link_through_a_trailing_slash_and_a_device_are_refused_by_name_only() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gwd-kinds-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // A socket is bound by a path of at most 107 bytes: bind through the directory's descriptor, the socket file is
    // then found at its long path as any file.
    let dir_handle = std::fs::File::open(&dir).unwrap();
    let socket = dir.join("socket");
    let _listener = std::os::unix::net::UnixListener::bind(format!("/proc/self/fd/{}/socket", std::os::fd::AsRawFd::as_raw_fd(&dir_handle))).unwrap();
    let fifo = dir.join("fifo");
    assert!(std::process::Command::new("mkfifo").arg(&fifo).status().unwrap().success());
    symlink(&fifo, dir.join("link-to-fifo")).unwrap();
    let folder = dir.join("folder");
    std::fs::create_dir(&folder).unwrap();
    symlink(&folder, dir.join("link-to-folder")).unwrap();
    let real = dir.join("real");
    std::fs::write(&real, format!("{LEAK}\n")).unwrap();
    symlink(&real, dir.join("link")).unwrap();
    let forms: Vec<(&str, PathBuf)> = vec![
        ("a socket", socket),
        ("a link to a FIFO", dir.join("link-to-fifo")),
        ("a link to a directory", dir.join("link-to-folder")),
        ("a file with a slash after it", PathBuf::from(format!("{}/", real.display()))),
        ("a link with a slash after it", PathBuf::from(format!("{}/", dir.join("link").display()))),
        ("a link with a dot after it", dir.join("link/.")),
        ("/dev/null", PathBuf::from("/dev/null")),
        ("/dev/zero", PathBuf::from("/dev/zero")),
        ("a link in /proc", PathBuf::from("/proc/self/exe")),
    ];
    let mut wrong = Vec::new();
    for (what, path) in &forms {
        let started = Instant::now();
        let (exit, said) = token_file_run(&what.replace([' ', '/'], "-"), path);
        let named = said.contains("gateway.token_file");
        let leaked = said.contains(LEAK) || said.contains(&path.display().to_string());
        if exit != 2 || !named || leaked {
            wrong.push(format!("{what}: exit {exit}, named {named}, leaked {leaked}: {}", said.lines().next().unwrap_or("")));
        }
        assert!(started.elapsed().as_secs() < 20, "{what} took {:?}", started.elapsed());
    }
    assert_eq!(wrong, Vec::<String>::new(), "a token file of the wrong kind was not refused by name only");
}

#[test]
fn the_nonblock_literal_of_the_driver_reader_is_the_platforms_own_and_the_reader_asks_the_descriptor() {
    let src = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("driver/src/gateway.rs")).unwrap();
    let flat = src.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(libc::O_NONBLOCK, 0o4000, "this platform's O_NONBLOCK");
    assert!(flat.contains("target_os = \"linux\", target_os = \"android\"))] const NONBLOCK: i32 = 0o4000;"), "the Linux literal changed");
    assert!(flat.contains("not(any(target_os = \"linux\", target_os = \"android\")))] const NONBLOCK: i32 = 0x4;"), "the other literal changed");
    assert!(flat.contains(".custom_flags(NONBLOCK)") && flat.contains(".metadata()") && flat.contains(".is_file()"));
}
