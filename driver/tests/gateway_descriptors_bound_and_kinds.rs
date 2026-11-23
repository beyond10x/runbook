//! Cases of the gateway beside `gateway.rs`, `gateway_hardening.rs` and `gateway_readers_and_filter.rs`: the
//! descriptors of the adapter, the address bound at the byte, the Rust reader on every kind of file.
//! Invented strings only; `claude` is a stand-in in POSIX sh; no model, no network. Every wait has a bound.

use std::fs;
use std::io::Write;
use std::os::unix::fs::{symlink, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const TOKEN: &str = "gwd-token-9c41e7d2a6b8";
const BASE: &str = "http://127.0.0.1:8080";
const RESULT: &str = r#"{"type":"result","subtype":"success","is_error":false,"num_turns":1,"total_cost_usd":0,"duration_ms":1,"usage":{}}"#;
const O_NONBLOCK_LINUX: i32 = 0o4000;

fn tools() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../bindings/tools")
}

struct Case {
    base: PathBuf,
}

impl Case {
    fn new(name: &str) -> Case {
        let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gwd-gateway").join(name);
        let _ = fs::remove_dir_all(&base);
        for d in ["work", "bin", "log", "private"] {
            fs::create_dir_all(base.join(d)).unwrap();
        }
        fs::set_permissions(base.join("private"), fs::Permissions::from_mode(0o700)).unwrap();
        Case { base }
    }

    fn program(&self, name: &str, text: &str) {
        let p = self.base.join("bin").join(name);
        fs::write(&p, text).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn request(&self) -> PathBuf {
        let work = self.base.join("work");
        fs::write(work.join("prompt.md"), "prompt\n").unwrap();
        fs::create_dir_all(work.join("cwd")).unwrap();
        let request = serde_json::json!({"record_dir": work.join("rec"), "prompt_file": work.join("prompt.md"), "cwd": work.join("cwd"),
            "model": "claude-haiku-5-5", "max_turns": 1, "max_budget_usd": 0.1, "operations": ["file.read"], "node": "idea.dissect", "attempt": 1});
        fs::write(work.join("request.json"), request.to_string()).unwrap();
        work.join("request.json")
    }

    /// The adapter with a gateway file (`gateway`) or with an API key and no gateway.
    fn command(&self, gateway: bool) -> Command {
        let path = format!("{}:{}", self.base.join("bin").display(), std::env::var("PATH").unwrap_or_default());
        let mut c = Command::new(tools().join("llm-adapter-claude-cli.sh"));
        c.arg(self.request()).env_clear().env("PATH", path);
        if gateway {
            let p = self.base.join("private/gateway");
            fs::write(&p, format!("base_url={BASE}\ntoken={TOKEN}\n")).unwrap();
            fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
            c.env("SDC_GATEWAY_FILE", p);
        } else {
            c.env("ANTHROPIC_API_KEY", "gwd-invented-api-key-value");
        }
        c.current_dir(self.base.join("work")).stdin(Stdio::null());
        c
    }

    /// `c` run for at most 40 s, its two streams in files (no pipe a process left behind could hold open).
    fn bounded(&self, mut c: Command) -> (std::process::Output, Duration) {
        let (out, err) = (self.base.join("log/bounded-stdout"), self.base.join("log/bounded-stderr"));
        c.stdout(fs::File::create(&out).unwrap()).stderr(fs::File::create(&err).unwrap());
        let mut child = c.spawn().unwrap();
        let start = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if start.elapsed().as_secs() >= 40 {
                let _ = child.kill();
                let _ = child.wait();
                panic!("the command did not end within 40 s");
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        let took = start.elapsed();
        (std::process::Output { status, stdout: fs::read(&out).unwrap(), stderr: fs::read(&err).unwrap() }, took)
    }
}

fn said(o: &std::process::Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

fn gw(base: &str, token: &str) -> Vec<u8> {
    format!("base_url={base}\ntoken={token}\n").into_bytes()
}

fn shell_reads(case: &Case, path: &Path, lc_all: &str) -> (bool, String) {
    let mut c = Command::new("bash");
    c.args(["-c", "source \"$1\" && sdc_gateway_read \"$2\"", "_"]).arg(tools().join("gateway.sh")).arg(path).env_clear().env("PATH", std::env::var("PATH").unwrap_or_default()).env("LC_ALL", lc_all);
    let (o, _) = case.bounded(c);
    (o.status.success(), said(&o))
}

// ---- every descriptor of the adapter that is not 0, 1, 2 stays out of claude and what it starts ----

fn descriptors_of_claude(gateway: bool, tag: &str) -> String {
    let case = Case::new(tag);
    let log = case.base.join("log/fds");
    // The glob lists the descriptors of the shell once, before any redirect of the loop's body is opened.
    let list = |who: &str| format!("for f in /proc/self/fd/*; do echo \"{who} ${{f##*/}} $(readlink \"$f\")\" >> '{}'; done", log.display());
    case.program("claude", &format!("#!/bin/sh\n{}\nsh -c '{}'\necho '{RESULT}'\n", list("claude"), list("child").replace('\'', "'\\''")));
    let (o, _) = case.bounded(case.command(gateway));
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    fs::read_to_string(&log).expect("claude did not start")
}

fn extra_descriptors(listing: &str) -> Vec<String> {
    listing
        .lines()
        .filter(|l| {
            let mut w = l.splitn(3, ' ');
            let (_, fd, target) = (w.next(), w.next().unwrap_or(""), w.next().unwrap_or(""));
            // 0, 1 and 2 are the call's own; a descriptor on a directory of /proc is the glob's own handle.
            // `readlink` is a process of its own: a descriptor it does not hold (the shell's script as 255, the handle of
            // the glob's directory) is not inherited by a program the shell starts, and its target is empty.
            !matches!(fd, "0" | "1" | "2") && !target.is_empty()
        })
        .map(str::to_owned)
        .collect()
}

#[test]
fn claude_and_its_child_hold_no_descriptor_but_0_1_2_with_a_gateway() {
    let listing = descriptors_of_claude(true, "fds-gateway");
    assert!(listing.contains("claude 1 ") && listing.contains("child 1 "), "the stand-in did not list: {listing}");
    assert_eq!(extra_descriptors(&listing), Vec::<String>::new(), "claude or its child holds a descriptor beyond 0, 1, 2");
}

#[test]
fn claude_and_its_child_hold_no_descriptor_but_0_1_2_without_a_gateway() {
    let listing = descriptors_of_claude(false, "fds-key");
    assert!(listing.contains("claude 1 ") && listing.contains("child 1 "), "the stand-in did not list: {listing}");
    assert_eq!(extra_descriptors(&listing), Vec::<String>::new(), "claude or its child holds a descriptor beyond 0, 1, 2 (no gateway)");
}

// ---- the address bound, at the byte, with multibyte characters, in both readers and both locales ----

fn padded(prefix: &str, ch: &str, bytes: usize) -> String {
    // `prefix`, then as many `ch` as fit, then ASCII to make exactly `bytes` bytes.
    let n = (bytes - prefix.len()) / ch.len();
    let mut s = format!("{prefix}{}", ch.repeat(n));
    while s.len() < bytes {
        s.push('p');
    }
    assert_eq!(s.len(), bytes);
    s
}

#[test]
fn the_two_readers_take_2048_ascii_bytes_and_refuse_everything_else_at_the_bound_in_both_locales() {
    let case = Case::new("bound");
    let file = case.base.join("work/gateway");
    let max_token = "t".repeat(8192);
    let mut wrong = Vec::new();
    let mut forms: Vec<(String, String, bool)> = Vec::new();
    for n in [2047usize, 2048, 2049, 2050] {
        forms.push((format!("{n} ASCII bytes"), padded("http://h/", "p", n), n <= 2048));
        forms.push((format!("{n} bytes, https with an IPv6 host"), padded("https://[::1]:8080/", "p", n), n <= 2048));
        forms.push((format!("{n} bytes with two-byte characters"), padded("http://h/", "\u{e9}", n), false));
        forms.push((format!("{n} bytes with three-byte characters"), padded("http://h/", "\u{20ac}", n), false));
        forms.push((format!("{n} bytes with four-byte characters"), padded("http://h/", "\u{1f600}", n), false));
    }
    // 2048 characters that are more than 2048 bytes, and 1024 two-byte characters that are exactly 2048 bytes.
    forms.push(("2048 two-byte characters".into(), format!("http://h/{}", "\u{e9}".repeat(2039)), false));
    forms.push(("1019 two-byte characters at 2047 bytes".into(), padded("http://h/", "\u{e9}", 2047), false));
    for (what, base, taken) in &forms {
        fs::write(&file, gw(base, &max_token)).unwrap();
        let rust = sdc_tools::gateway::read(&file).is_ok();
        let c = shell_reads(&case, &file, "C").0;
        let utf8 = shell_reads(&case, &file, "en_US.utf8").0;
        let posix = shell_reads(&case, &file, "POSIX").0;
        if rust != *taken || c != *taken || utf8 != *taken || posix != *taken {
            wrong.push(format!("{what}: expected {taken}, rust={rust} shell(C)={c} shell(en_US.utf8)={utf8} shell(POSIX)={posix}"));
        }
    }
    assert_eq!(wrong, Vec::<String>::new(), "the readers differ from the bound of 2048 bytes");
}

#[test]
fn neither_refusal_of_a_long_or_multibyte_address_prints_a_byte_of_it() {
    let case = Case::new("bound-print");
    let file = case.base.join("work/gateway");
    for base in [padded("http://h/", "p", 2049), padded("http://h/", "\u{e9}", 2048), padded("http://h/", "p", 20_000)] {
        fs::write(&file, gw(&base, TOKEN)).unwrap();
        let rust = sdc_tools::gateway::read(&file).unwrap_err();
        assert!(!rust.contains("pppp") && !rust.contains('\u{e9}') && !rust.contains(TOKEN), "{rust}");
        for locale in ["C", "en_US.utf8"] {
            let (ok, said) = shell_reads(&case, &file, locale);
            assert!(!ok);
            assert!(!said.contains("pppp") && !said.contains('\u{e9}') && !said.contains(TOKEN) && !said.is_empty(), "{locale}: {said}");
        }
        // The adapter ends before claude starts, naming no value.
        case.program("claude", "#!/bin/sh\necho claude-started > \"$0.started\"\n");
        let c = case.command(true);
        fs::write(case.base.join("private/gateway"), gw(&base, TOKEN)).unwrap();
        let (o, _) = case.bounded(c);
        assert_eq!(o.status.code(), Some(127));
        let said = said(&o);
        assert!(!said.contains("pppp") && !said.contains('\u{e9}') && !said.contains(TOKEN), "{said}");
        assert!(!case.base.join("bin/claude.started").exists(), "claude started on a refused gateway file");
    }
}

// ---- the Rust reader on every kind of file: refused, no byte read, no wait ----

fn rchar() -> u64 {
    let io = fs::read_to_string("/proc/thread-self/io").unwrap();
    io.lines().find_map(|l| l.strip_prefix("rchar: ")).unwrap().trim().parse().unwrap()
}

/// `read(path)` on a thread of its own, ended or not within 5 s: whether it is `Ok`, and the bytes it read.
fn read_bounded(path: PathBuf) -> Result<(bool, u64), std::sync::mpsc::RecvTimeoutError> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        // Reading `/proc/thread-self/io` is itself counted by the next reading: one reading's own bytes are taken off.
        let first = rchar();
        let before = rchar();
        let own = before - first;
        let ok = sdc_tools::gateway::read(&path).is_ok();
        let after = rchar();
        let _ = tx.send((ok, (after - before).saturating_sub(own)));
    });
    rx.recv_timeout(Duration::from_secs(5))
}

#[test]
fn the_rust_reader_refuses_a_fifo_a_socket_a_directory_a_device_and_a_link_to_a_fifo_without_a_byte_and_without_a_wait() {
    let case = Case::new("kinds");
    let dir = case.base.join("work");
    let good = format!("base_url={BASE}\ntoken={TOKEN}\n");
    // A FIFO that holds a good gateway text, with a reader and a writer already open: only the kind can refuse it.
    let fifo = dir.join("fifo");
    assert!(Command::new("mkfifo").arg(&fifo).status().unwrap().success());
    let _hold = fs::OpenOptions::new().read(true).custom_flags(O_NONBLOCK_LINUX).open(&fifo).unwrap();
    let mut writer = fs::OpenOptions::new().write(true).custom_flags(O_NONBLOCK_LINUX).open(&fifo).unwrap();
    writer.write_all(good.as_bytes()).unwrap();
    // A FIFO with no peer at all.
    let lonely = dir.join("lonely");
    assert!(Command::new("mkfifo").arg(&lonely).status().unwrap().success());
    // A socket is bound by a path of at most 107 bytes: bind through the directory's descriptor, the socket file is
    // then found at its long path as any file.
    let dir_handle = fs::File::open(&dir).unwrap();
    let socket = dir.join("sock");
    let _listener = std::os::unix::net::UnixListener::bind(format!("/proc/self/fd/{}/sock", std::os::fd::AsRawFd::as_raw_fd(&dir_handle))).unwrap();
    let link = dir.join("link-to-fifo");
    symlink(&fifo, &link).unwrap();
    let folder = dir.join("folder");
    fs::create_dir(&folder).unwrap();
    let mut wrong = Vec::new();
    for (what, path) in [
        ("a FIFO holding a good text", fifo),
        ("a FIFO with no peer", lonely),
        ("a socket", socket),
        ("a directory", folder),
        ("/dev/null", PathBuf::from("/dev/null")),
        ("/dev/zero", PathBuf::from("/dev/zero")),
        ("a link to a FIFO holding a good text", link),
    ] {
        match read_bounded(path) {
            Ok((false, bytes)) if bytes <= 3 => {} // a reading of the counter may grow by a digit; the file text is 61 bytes
            Ok((ok, bytes)) => wrong.push(format!("{what}: ok={ok}, {bytes} bytes read")),
            Err(_) => wrong.push(format!("{what}: no end within 5 s")),
        }
    }
    assert_eq!(wrong, Vec::<String>::new(), "the Rust reader took, read or waited on a file that is no regular file");
}

#[test]
fn the_rust_reader_still_takes_a_regular_file_and_a_link_to_one_as_the_shell_reader_does() {
    let case = Case::new("regular");
    let dir = case.base.join("work");
    let real = dir.join("gateway");
    fs::write(&real, gw(BASE, TOKEN)).unwrap();
    let link = dir.join("link");
    symlink(&real, &link).unwrap();
    for (what, path) in [("a regular file", &real), ("a link to a regular file", &link)] {
        let rust = sdc_tools::gateway::read(path).is_ok();
        let shell = shell_reads(&case, path, "C").0;
        assert_eq!((rust, shell), (true, true), "{what}");
    }
}
