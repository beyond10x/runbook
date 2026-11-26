//! Cases of the gateway beside `gateway.rs` and `gateway_hardening.rs`: the token file's kinds, the signals, the
//! modes under a umask of 000, the docs. Invented strings only; no model, no network.

use super::core::{stderr, toml_text, World};
use serde_json::{json, Value};
use std::os::unix::fs::{symlink, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TOKEN: &str = "gwk-token-3a7f91c0d5e2";
const BASE: &str = "http://127.0.0.1:8080";
const LEAK: &str = "GWK-CONTENT-MUST-NOT-COME-BACK";

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
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn dir_of(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gwk-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The runner on the settings of the world, ended within 20 s (else killed): its exit code and its standard error.
fn runs_to_the_end(w: &World, tag: &str) -> (i32, String) {
    use std::io::Read;
    let mut child = w.command(&w.args("r1", 1_000_000)).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap();
    let status = ended_within(&mut child, 20).unwrap_or_else(|| panic!("{tag}: the runner did not end within 20 s"));
    let mut said = String::new();
    child.stderr.take().unwrap().read_to_string(&mut said).unwrap();
    (status.code().unwrap_or(-1), said)
}

fn wait_for(path: &Path, secs: u64, what: &str) {
    let started = Instant::now();
    while !path.exists() {
        assert!(started.elapsed().as_secs() < secs, "{what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn signal(name: &str, pid: u32) {
    assert!(std::process::Command::new("kill").args([format!("-{name}"), pid.to_string()]).status().unwrap().success());
}

// ---- E: signals ----

#[test]
fn a_hangup_that_the_caller_ignores_does_not_end_the_run() {
    let w = World::new("gwk-nohup");
    w.fake.answer("tree_run", "1", "", 0);
    w.fake.hook("tree_run", "1", "echo $$ > \"$D/pid\"\nsleep 30\n");
    settings(&w, json!({"base_url": BASE, "token": TOKEN}));
    let mut cmd = w.command(&w.args("r1", 1_000_000));
    // What `nohup` does before it runs the command: SIGHUP is ignored and the ignore is inherited through exec.
    unsafe {
        cmd.pre_exec(|| {
            libc::signal(libc::SIGHUP, libc::SIG_IGN);
            Ok(())
        });
    }
    let mut child = cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    wait_for(&w.fake.dir.join("pid"), 20, "the fake tree run did not start");
    signal("HUP", child.id());
    // A runner that takes a SIGHUP ends its run, the start included, in about 0.1 s (`the_private_directory_is_gone_after_sighup_too`
    // in `gateway_hardening.rs`); one second is ten times that.
    std::thread::sleep(Duration::from_secs(1));
    let still_running = child.try_wait().unwrap().is_none();
    // SIGTERM first: the runner ends the process group of its fake, the `sleep` included. A kill of the hook's shell
    // first left the `sleep` holding the runner's pipes, and the runner waited for them until it was killed 8 s later.
    let _ = std::process::Command::new("kill").args(["-TERM", &child.id().to_string()]).stderr(Stdio::null()).status();
    let _ = ended_within(&mut child, 8);
    if let Ok(pid) = std::fs::read_to_string(w.fake.dir.join("pid")) {
        let _ = std::process::Command::new("kill").args(["-KILL", pid.trim()]).status();
    }
    assert!(still_running, "the run ended on a SIGHUP that its caller had ignored (as nohup does)");
}

#[test]
fn guard_three_signals_in_a_row_leave_no_private_directory_and_the_runner_ends() {
    let w = World::new("gwk-three");
    w.fake.answer("tree_run", "1", "", 0);
    w.fake.hook("tree_run", "1", "echo $$ > \"$D/pid\"\nsleep 30\n");
    settings(&w, json!({"base_url": BASE, "token": TOKEN}));
    let mut child = w.command(&w.args("r1", 1_000_000)).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    wait_for(&w.fake.dir.join("pid"), 20, "the fake tree run did not start");
    assert!(private_dir(&w, "r1").join("gateway").exists());
    for name in ["HUP", "TERM", "INT", "HUP", "TERM"] {
        signal(name, child.id());
        std::thread::sleep(Duration::from_millis(15));
    }
    let ended = ended_within(&mut child, 10);
    if let Ok(pid) = std::fs::read_to_string(w.fake.dir.join("pid")) {
        let _ = std::process::Command::new("kill").args(["-KILL", pid.trim()]).status();
    }
    assert!(ended.is_some(), "the runner did not end after five signals");
    assert!(!private_dir(&w, "r1").exists(), "the private directory is left after several signals");
}

// ---- C: the kinds of token file ----

fn put(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

/// Runs the runner on `path` as the token file; `Ok` is exit 0, `Err` the refusal's exit code and what it printed.
fn token_file_run(tag: &str, path: &Path) -> (i32, String) {
    let w = World::new(&format!("gwk-{tag}"));
    w.one_decision_tree();
    settings(&w, json!({"base_url": BASE, "token_file": path.display().to_string()}));
    let (exit, said) = runs_to_the_end(&w, tag);
    if exit != 0 {
        assert!(!w.run_dir("r1").exists() && !private_dir(&w, "r1").exists(), "{tag}: a run directory exists after the refusal");
    }
    (exit, said)
}

#[test]
fn a_token_file_that_holds_one_good_line_is_taken_in_each_of_its_forms() {
    let dir = dir_of("accept");
    let real = put(&dir, "real", format!("{TOKEN}\n").as_bytes());
    let sub = dir.join("sub");
    std::fs::create_dir(&sub).unwrap();
    std::fs::hard_link(&real, dir.join("second-name")).unwrap();
    let real_dir = dir.join("real-dir");
    std::fs::create_dir(&real_dir).unwrap();
    put(&real_dir, "tok", format!("{TOKEN}\n").as_bytes());
    symlink(&real_dir, dir.join("linked-dir")).unwrap();
    let ro = put(&dir, "read-only", format!("{TOKEN}\n").as_bytes());
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o400)).unwrap();
    let forms: Vec<(&str, PathBuf)> = vec![
        ("plain", real.clone()),
        ("no end of line", put(&dir, "bare", TOKEN.as_bytes())),
        ("crlf", put(&dir, "crlf", format!("{TOKEN}\r\n").as_bytes())),
        ("a second name of the file", dir.join("second-name")),
        ("eight bytes", put(&dir, "eight", b"abcd1234\n")),
        ("8192 bytes", put(&dir, "max", format!("{}\n", "x".repeat(8192)).as_bytes())),
        ("dot-dot in the path", sub.join("../real")),
        ("a link in a parent component", dir.join("linked-dir/tok")),
        ("mode 0400", ro),
    ];
    let mut wrong = Vec::new();
    for (what, path) in &forms {
        let (exit, said) = token_file_run(&what.replace(' ', "-"), path);
        if exit != 0 {
            wrong.push(format!("{what}: exit {exit}: {}", said.lines().next().unwrap_or("")));
        }
    }
    assert_eq!(wrong, Vec::<String>::new(), "a good token file was refused");
}

#[test]
fn a_token_file_that_is_no_good_line_is_refused_by_name_only_and_without_a_hang() {
    let dir = dir_of("refuse");
    let real = put(&dir, "real", format!("{TOKEN}\n").as_bytes());
    symlink(&real, dir.join("link")).unwrap();
    symlink(dir.join("link"), dir.join("link-to-link")).unwrap();
    symlink(dir.join("absent"), dir.join("dangling")).unwrap();
    let folder = dir.join("folder");
    std::fs::create_dir(&folder).unwrap();
    let fifo = dir.join("fifo");
    assert!(std::process::Command::new("mkfifo").arg(&fifo).status().unwrap().success());
    let none = put(&dir, "mode-000", format!("{TOKEN}\n").as_bytes());
    std::fs::set_permissions(&none, std::fs::Permissions::from_mode(0o000)).unwrap();
    let sparse = dir.join("sparse");
    std::fs::File::create(&sparse).unwrap().set_len(100 << 20).unwrap();
    let leak = |tail: &str| format!("{LEAK}{tail}");
    let forms: Vec<(&str, PathBuf)> = vec![
        ("seven bytes", put(&dir, "seven", b"abcd123\n")),
        ("8193 bytes", put(&dir, "over", format!("{}\n", "x".repeat(8193)).as_bytes())),
        ("8210 bytes", put(&dir, "over2", "x".repeat(8210).as_bytes())),
        ("20000 bytes", put(&dir, "large", leak(&"x".repeat(20_000)).as_bytes())),
        ("100 MB of NUL", sparse),
        ("empty", put(&dir, "empty", b"")),
        ("one end of line", put(&dir, "nl", b"\n")),
        ("two lines", put(&dir, "two", leak("\nsecond\n").as_bytes())),
        ("an empty second line", put(&dir, "blank", leak("\n\n").as_bytes())),
        ("a trailing blank", put(&dir, "trail", leak(" \n").as_bytes())),
        ("a leading blank", put(&dir, "lead", leak("").replace("GWK", " GWK").as_bytes())),
        ("a byte order mark", put(&dir, "bom", [b"\xef\xbb\xbf".as_slice(), leak("\n").as_bytes()].concat().as_slice())),
        ("a NUL inside", put(&dir, "nul", leak("\0tail\n").as_bytes())),
        ("a tab inside", put(&dir, "tab", leak("\ttail\n").as_bytes())),
        ("a character over ASCII", put(&dir, "wide", leak("\u{e9}tail\n").as_bytes())),
        ("bytes that are no UTF-8", put(&dir, "bad", [leak("").as_bytes(), &[0xff, 0xfe, b'\n']].concat().as_slice())),
        ("a lone carriage return", put(&dir, "cr", leak("\r").as_bytes())),
        ("a link", dir.join("link")),
        ("a link to a link", dir.join("link-to-link")),
        ("a dangling link", dir.join("dangling")),
        ("a directory", folder),
        ("a FIFO", fifo),
        ("/dev/null", PathBuf::from("/dev/null")),
        ("/dev/zero", PathBuf::from("/dev/zero")),
        ("/dev/full", PathBuf::from("/dev/full")),
        ("the runner's own environment", PathBuf::from("/proc/self/environ")),
        ("the runner's own argv", PathBuf::from("/proc/self/cmdline")),
        ("mode 000", none),
        ("a missing file", dir.join("absent")),
    ];
    assert!(forms.len() >= 28);
    let mut wrong = Vec::new();
    for (what, path) in &forms {
        let started = Instant::now();
        let (exit, said) = token_file_run(&what.replace([' ', '\'', '/'], "-"), path);
        let named = said.contains("gateway.token_file");
        let leaked = said.contains(LEAK) || said.contains(TOKEN) || said.contains(&path.display().to_string()) || said.contains("marker-secret-value");
        if exit != 2 || !named || leaked {
            wrong.push(format!("{what}: exit {exit}, named {named}, leaked {leaked}: {}", said.lines().next().unwrap_or("")));
        }
        assert!(started.elapsed().as_secs() < 20, "{what} took {:?}", started.elapsed());
    }
    assert_eq!(wrong, Vec::<String>::new(), "a token file that is no good line was not refused by name only");
}

#[test]
fn a_token_file_that_grows_while_it_is_read_ends_the_runner_and_leaks_nothing() {
    let dir = dir_of("grow");
    let file = put(&dir, "grows", TOKEN.as_bytes());
    let stop = Arc::new(AtomicBool::new(false));
    let writer = {
        let (stop, file) = (stop.clone(), file.clone());
        std::thread::spawn(move || {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().append(true).open(file).unwrap();
            while !stop.load(Ordering::SeqCst) {
                let _ = f.write_all(&[b'x'; 1024]);
                std::thread::sleep(Duration::from_millis(1));
            }
        })
    };
    let (exit, said) = token_file_run("grows", &file);
    stop.store(true, Ordering::SeqCst);
    writer.join().unwrap();
    assert!(exit == 0 || exit == 2, "exit {exit}: {said}");
    assert!(!said.contains(TOKEN));
}

#[test]
fn the_docs_say_which_component_of_token_file_is_not_followed() {
    // `O_NOFOLLOW` refuses a link as the last component only: a link in a directory of the path is followed, and
    // the page says "without following a link". The sentence has to say which component.
    let doc = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("docs/loom-run-settings.md")).unwrap();
    let start = doc.find("## `[gateway]`").expect("the page has the [gateway] section");
    let section = &doc[start..];
    let section = &section[..section[3..].find("\n## ").map_or(section.len(), |i| i + 3)].to_lowercase();
    assert!(
        ["last component", "final component", "last part", "parent", "the file itself", "directory in the path", "directory of the path"].iter().any(|w| section.contains(w)),
        "the section says a token_file is opened without following a link and never says that a link in a directory of the path is followed"
    );
}

#[test]
fn the_docs_say_that_an_ignored_hangup_stays_ignored_what_the_filter_covers_and_the_bound_of_an_address() {
    let doc = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("docs/loom-run-settings.md")).unwrap();
    let start = doc.find("## `[gateway]`").expect("the page has the [gateway] section");
    let section = &doc[start..];
    let section = section[..section[3..].find("\n## ").map_or(section.len(), |i| i + 3)].split_whitespace().collect::<Vec<_>>().join(" ");
    for sentence in [
        "A SIGHUP that the caller ignored (a run under `nohup`) stays ignored and does not end the run.",
        "The filter of item 3 covers what `claude` prints to its two streams and the files made from it; a file that `claude` writes by its own tools is not covered.",
        "at most 2048 bytes",
        "a link as the last component, a FIFO",
    ] {
        assert!(section.contains(sentence), "the [gateway] section lacks: {sentence}");
    }
}

// ---- F: modes under a umask of 000 ----

#[test]
fn guard_the_directories_and_the_file_never_have_a_wider_mode_under_umask_000_at_any_sampled_moment() {
    let w = World::new("gwk-umask");
    let xdg = w.root.join("xdg");
    let probe = format!(
        "echo $$ > \"$D/pid\"\nstat -c '%a %n' '{x}/runbook' '{x}/runbook/r1' '{x}/runbook/r1/gateway' > \"$D/modes\"\nsleep 1\n",
        x = xdg.display()
    );
    w.fake.answer("tree_run", "1", "", 0);
    w.fake.hook("tree_run", "1", &probe);
    settings(&w, json!({"base_url": BASE, "token": TOKEN}));
    let mut cmd = w.command(&w.args("r1", 1_000_000));
    unsafe {
        cmd.pre_exec(|| {
            libc::umask(0);
            Ok(())
        });
    }
    let mut child = cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let seen: Arc<Mutex<Vec<String>>> = Arc::default();
    let sampler = {
        let (stop, seen, xdg) = (stop.clone(), seen.clone(), xdg.clone());
        std::thread::spawn(move || {
            let paths = [(xdg.join("runbook"), 0o700u32), (xdg.join("runbook/r1"), 0o700), (xdg.join("runbook/r1/gateway"), 0o600)];
            let mut found = 0u32;
            while !stop.load(Ordering::SeqCst) {
                for (p, max) in &paths {
                    if let Ok(m) = std::fs::symlink_metadata(p) {
                        found += 1;
                        let mode = m.permissions().mode() & 0o7777;
                        if mode & !max != 0 {
                            seen.lock().unwrap().push(format!("{} had mode {mode:o}", p.display()));
                        }
                    }
                }
            }
            seen.lock().unwrap().push(format!("(sampled {found} times)"));
        })
    };
    let ended = ended_within(&mut child, 25);
    stop.store(true, Ordering::SeqCst);
    sampler.join().unwrap();
    if let Ok(pid) = std::fs::read_to_string(w.fake.dir.join("pid")) {
        let _ = std::process::Command::new("kill").args(["-KILL", pid.trim()]).status();
    }
    assert!(ended.is_some());
    let seen = seen.lock().unwrap().clone();
    let wide: Vec<&String> = seen.iter().filter(|s| !s.starts_with('(')).collect();
    assert!(wide.is_empty(), "a mode wider than the contract was sampled: {wide:?}");
    let modes = std::fs::read_to_string(w.fake.dir.join("modes")).expect("the probe did not run");
    let mut lines: Vec<&str> = modes.lines().map(|l| l.split_whitespace().next().unwrap()).collect();
    lines.sort();
    assert_eq!(lines, ["600", "700", "700"], "{modes}");
}

#[test]
fn guard_an_unknown_confinement_is_refused_before_anything_is_read_and_prints_no_token() {
    let w = World::new("gwk-confinement");
    w.one_decision_tree();
    settings(&w, json!({"base_url": BASE, "token": TOKEN}));
    let mut args = w.args("r1", 1_000_000);
    let i = args.iter().position(|a| a == "--confinement").unwrap();
    args[i + 1] = "loopback".to_owned();
    let o = w.run(&args);
    assert!(matches!(o.status.code(), Some(2) | Some(64)), "{}", stderr(&o));
    assert!(!stderr(&o).contains(TOKEN));
    assert!(!w.run_dir("r1").exists() && !private_dir(&w, "r1").exists());
}

// ---- G: the sentences of the round ----

#[test]
fn guard_the_docs_of_the_gateway_use_only_the_loopback_example_and_the_placeholder_token() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let mut found = Vec::new();
    for (file, from) in [("docs/loom-run-settings.md", "## `[gateway]`"), ("docs/loom-run.md", "### The gateway"), ("CHANGELOG.md", "Model programs reach a gateway")] {
        let text = std::fs::read_to_string(root.join(file)).unwrap();
        let start = text.find(from).unwrap_or_else(|| panic!("{file} has no {from}"));
        let part = &text[start..];
        let part = &part[..part[from.len()..].find("\n#").map_or(part.len(), |i| i + from.len())];
        for scheme in ["http://", "https://"] {
            for (i, _) in part.match_indices(scheme) {
                let rest = &part[i + scheme.len()..];
                let host: String = rest.chars().take_while(|c| !c.is_whitespace() && !"`'\"),;".contains(*c)).collect();
                if !host.is_empty() && host != "127.0.0.1:8080" {
                    found.push(format!("{file}: {scheme}{host}"));
                }
            }
        }
        for (i, _) in part.match_indices("token = \"") {
            let value: String = part[i + 9..].chars().take_while(|c| *c != '"').collect();
            if value != "placeholder" {
                found.push(format!("{file}: token = \"{value}\""));
            }
        }
    }
    assert_eq!(found, Vec::<String>::new(), "a docs sentence of the gateway names an address or a token that is not the example's");
}
