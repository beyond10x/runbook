
//! What `sdc-conform-adapter` says about a product whose output stream ended. The label decides the
//! cause `sdc-conform` records: an event `start` makes the run `product_not_started`, an event
//! `capture` makes it `capture_missing`, the table's fault, and the table node is asked again
//! (`driver/src/conform.rs`, `table_evidence` and the lines before its call).

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value as J};

use super::ess_pin_guard::{executable, stub, table, tmp};

const ADAPTER: &str = env!("CARGO_BIN_EXE_sdc-conform-adapter");

/// One `begin_scenario` sent to a bare adapter whose product is the shell script `product`
/// (`bin/product.sh <port> <data dir>`) and whose table captures `token` from stdout within
/// `timeout_s`: the reply and how long it took.
fn begin_scenario(name: &str, product: &str, timeout_s: u64) -> (J, Duration) {
    use std::io::{BufRead, BufReader, Write};
    let dir = tmp(name);
    let tree = dir.join("tree");
    fs::create_dir_all(tree.join("bin")).unwrap();
    fs::copy(stub(), tree.join("bin/stub-product")).unwrap();
    executable(&tree.join("bin/product.sh"), &format!("#!/bin/sh\n{product}\n"));
    let mut t = table();
    t["start"]["argv"] = json!(["bin/product.sh", "{port}", "{data_dir}"]);
    t["captures"] = json!([{"name": "token", "stream": "stdout", "regex": "board token: (\\S+)", "timeout_s": timeout_s}]);
    let table_file = dir.join("t.json");
    fs::write(&table_file, t.to_string()).unwrap();
    let mut child = Command::new(ADAPTER)
        .arg("--table")
        .arg(&table_file)
        .arg("--tree")
        .arg(&tree)
        .arg("--work")
        .arg(dir.join("adapter-work"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let started = Instant::now();
    let mut stdin = child.stdin.take().unwrap();
    writeln!(stdin, "{}", json!({"id": 1, "op": "begin_scenario", "request": {"scenario": "x"}})).unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let took = started.elapsed();
    drop(stdin);
    child.wait().unwrap();
    (serde_json::from_str(&line).unwrap_or_else(|e| panic!("reply {line:?}: {e}")), took)
}

fn detail(reply: &J) -> String {
    reply["target_error"]["detail"].as_str().unwrap_or("").to_string()
}

/// A product that sends its output to a file of its own and ends a second later with an error did
/// not start: the adapter says so for any exit within `timeout_s` ("the product did not start: the
/// product exited before it answered (exit status: 1)"), as the base commit did after 1.1 s. A
/// fixed wait after the end of the stream would call an exit that comes later a capture the table
/// got wrong. While the product sleeps, the adapter asks `start.ready` on its port; the port is held
/// for the product, so no other server answers there (on a port freed and handed on, another case's
/// product answered in 2 of 5 runs of these modules, and the end was called the table's capture).
/// The case still proves that a product that ends after its output stream closed did not start.
#[test]
fn a_product_that_redirects_its_output_and_ends_a_second_later_did_not_start() {
    let (reply, took) = begin_scenario("p2-redirect-then-exit", "exec > \"$2/server.log\" 2>&1\nsleep 1\necho 'cannot bind'\nexit 1", 20);
    assert_eq!(reply["target_error"]["kind"], "unavailable", "after {took:?}: {reply:#}");
    let d = detail(&reply);
    assert!(d.contains("the product did not start"), "after {took:?} the adapter blames the table's capture for a product that ends with exit status 1: {d:?}");
}

/// The same with the streams closed and the exit 300 ms later: 50 ms past the bound.
#[test]
fn a_product_that_exits_300_ms_after_its_streams_ended_did_not_start() {
    let (reply, took) = begin_scenario("p2-exit-300ms", "exec 1>&- 2>&-\nsleep 0.3\nexit 3", 20);
    assert_eq!(reply["target_error"]["kind"], "unavailable", "after {took:?}: {reply:#}");
    let d = detail(&reply);
    assert!(d.contains("the product did not start"), "after {took:?}: {d:?}");
}

/// A capture is read within `timeout_s` of the start. A product that prints one line that is not
/// UTF-8 (a Latin-1 banner) and then the value has not closed the stream and has printed the value:
/// it is captured. A reader that ends at the first such line misses the value.
#[test]
fn a_capture_printed_after_a_line_that_is_not_utf8_is_read() {
    let (reply, took) = begin_scenario("p2-not-utf8", "printf 'caf\\351 open\\n'\necho 'board token: abc'\nexec bin/stub-product \"$1\" \"$2\" token-quiet", 5);
    assert!(reply["target_error"].is_null(), "after {took:?}: {}", detail(&reply));
}

fn expect_did_not_start(name: &str, product: &str) {
    let (reply, took) = begin_scenario(name, product, 20);
    assert_eq!(reply["target_error"]["kind"], "unavailable", "after {took:?}: {reply:#}");
    let d = detail(&reply);
    assert!(d.contains("the product did not start"), "after {took:?} a product that ended is not a capture the table got wrong: {d:?}");
    assert!(took < Duration::from_secs(10), "an exited product is answered when it exited, not at the capture's bound: {took:?}");
}

/// A product that exits before it prints anything.
#[test]
fn a_product_that_exits_at_once_did_not_start() {
    expect_did_not_start("p2-exit-at-once", "exit 3");
}

/// A product that closes its streams and exits 100 ms, 200 ms, 500 ms later: whatever the delay,
/// the product ended, it did not serve.
#[test]
fn a_product_that_exits_after_its_streams_ended_did_not_start() {
    for ms in [100, 200, 500] {
        expect_did_not_start(&format!("p2-exit-{ms}ms"), &format!("exec 1>&- 2>&-\nsleep 0.{ms:03}\nexit 3"));
    }
}

/// A product that ends by a signal 100 ms after its streams ended, or by SIGKILL at once.
#[test]
fn a_product_that_ends_by_a_signal_did_not_start() {
    expect_did_not_start("p2-term-100ms", "exec 1>&- 2>&-\nsleep 0.1\nkill -TERM $$\nsleep 5");
    expect_did_not_start("p2-kill-at-once", "kill -KILL $$\nsleep 5");
}

/// A product that closed stdout and serves answers `start.ready`: the table's capture is what is
/// wrong, and the adapter says so with the capture's own message, not that the product did not start.
#[test]
fn a_product_that_closed_stdout_and_serves_is_a_capture_the_table_got_wrong() {
    let (reply, took) = begin_scenario("p2-closed-serving", "exec bin/stub-product \"$1\" \"$2\" token-silent", 20);
    assert_eq!(reply["target_error"]["kind"], "unavailable", "after {took:?}: {reply:#}");
    let d = detail(&reply);
    assert!(d.contains("capture token did not appear on stdout"), "after {took:?}: {d:?}");
    assert!(d.contains("closed stdout without printing it"), "after {took:?}: {d:?}");
    assert!(!d.contains("the product did not start"), "{d:?}");
    assert!(took < Duration::from_secs(10), "{took:?}");
}

/// A port of 127.0.0.1 held as `sdc-conform-adapter` holds the port of its product: a socket bound
/// with `SO_REUSEADDR` that never listens. A server that sets `SO_REUSEADDR` (the stub product) may
/// bind the port while the socket is open; the kernel gives it to no other process.
pub(crate) fn held_port() -> (std::os::fd::OwnedFd, u16) {
    use std::os::fd::FromRawFd;
    // SAFETY: socket calls on a descriptor owned from its creation; every pointer points to a
    // local of the size passed with it.
    unsafe {
        let fd = libc::socket(libc::AF_INET, libc::SOCK_STREAM | libc::SOCK_CLOEXEC, 0);
        assert!(fd >= 0, "socket: {}", std::io::Error::last_os_error());
        let owned = std::os::fd::OwnedFd::from_raw_fd(fd);
        let one: libc::c_int = 1;
        assert_eq!(libc::setsockopt(fd, libc::SOL_SOCKET, libc::SO_REUSEADDR, (&one as *const libc::c_int).cast(), std::mem::size_of::<libc::c_int>() as libc::socklen_t), 0);
        let mut addr: libc::sockaddr_in = std::mem::zeroed();
        addr.sin_family = libc::AF_INET as libc::sa_family_t;
        addr.sin_addr.s_addr = u32::from_ne_bytes([127, 0, 0, 1]);
        let mut len = std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;
        assert_eq!(libc::bind(fd, (&addr as *const libc::sockaddr_in).cast(), len), 0, "bind: {}", std::io::Error::last_os_error());
        assert_eq!(libc::getsockname(fd, (&mut addr as *mut libc::sockaddr_in).cast(), &mut len), 0);
        (owned, u16::from_be(addr.sin_port))
    }
}

/// Whether a server that binds 127.0.0.1:`port` without `SO_REUSEADDR`, as the kernel's own choice
/// of a port for another process requires of a port, may bind it now.
fn a_plain_bind_takes(port: u16) -> bool {
    // SAFETY: socket calls on a descriptor owned here and closed before the return; the address
    // is a local of the size passed with it.
    unsafe {
        let fd = libc::socket(libc::AF_INET, libc::SOCK_STREAM | libc::SOCK_CLOEXEC, 0);
        assert!(fd >= 0, "socket: {}", std::io::Error::last_os_error());
        let mut addr: libc::sockaddr_in = std::mem::zeroed();
        addr.sin_family = libc::AF_INET as libc::sa_family_t;
        addr.sin_addr.s_addr = u32::from_ne_bytes([127, 0, 0, 1]);
        addr.sin_port = port.to_be();
        let took = libc::bind(fd, (&addr as *const libc::sockaddr_in).cast(), std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t) == 0;
        libc::close(fd);
        took
    }
}

/// The port the adapter tells its product is held from its choice until the product ends: between
/// the two, before the product binds it, no other server can bind it and the kernel hands it to no
/// other process that asks for a port (2,000 binds to port 0 here). A port that the adapter
/// chose, freed and handed on was taken by the product of another case in about one of two runs
/// of the 58 cases of these two modules (the product then failed with `AddrInUse`, or the other
/// product answered `start.ready` for it). The product that binds the held port serves.
#[test]
fn the_port_a_product_is_told_is_held_until_it_binds_it() {
    let name = "p2-held-port";
    let tree = Path::new(env!("CARGO_TARGET_TMPDIR")).join("ess-pin-guard").join(name).join("tree");
    // The port file of an earlier run is gone before the prober looks.
    let _ = fs::remove_dir_all(tree.parent().unwrap());
    let prober = std::thread::spawn(move || {
        let port_file = tree.join("port");
        let limit = Instant::now() + Duration::from_secs(20);
        let port: u16 = loop {
            if let Some(p) = fs::read_to_string(&port_file).ok().and_then(|t| t.trim().parse().ok()) {
                break p;
            }
            assert!(Instant::now() < limit, "the product did not write its port");
            std::thread::sleep(Duration::from_millis(10));
        };
        let plain = a_plain_bind_takes(port);
        let handed = (0..2000).filter(|_| std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port() == port).count();
        fs::write(tree.join("go"), "").unwrap();
        (port, plain, handed)
    });
    let product = "echo \"$1\" > port.tmp\nmv port.tmp port\nwhile [ ! -f go ]; do sleep 0.01; done\necho 'board token: abc'\nexec bin/stub-product \"$1\" \"$2\" token-quiet";
    let (reply, took) = begin_scenario(name, product, 20);
    let (port, plain, handed) = prober.join().unwrap();
    assert!(!plain, "another server bound port {port} before the product did: the port was freed and handed on");
    assert_eq!(handed, 0, "the kernel handed port {port} to another bind to port 0 {handed} times before the product bound it");
    assert!(reply["target_error"].is_null(), "after {took:?} the product on its held port did not serve: {}", detail(&reply));
}

/// A product that cannot bind a held port (it does not set `SO_REUSEADDR` and says "Address already
/// in use") is started once more on another port that is not held; a product that ends for another
/// reason is started once.
#[test]
fn a_product_that_cannot_bind_the_held_port_is_started_once_more_on_another_port() {
    let starts = |name: &str| -> Vec<String> {
        let file = Path::new(env!("CARGO_TARGET_TMPDIR")).join("ess-pin-guard").join(name).join("tree/starts");
        fs::read_to_string(file).unwrap_or_default().lines().map(String::from).collect()
    };
    let (reply, took) = begin_scenario("p2-refused-port", "echo \"$1\" >> starts\nif [ \"$(wc -l < starts)\" -lt 2 ]; then echo \"bind 127.0.0.1:$1: Address already in use\" >&2; exit 1; fi\nexit 3", 20);
    let ports = starts("p2-refused-port");
    assert_eq!(ports.len(), 2, "after {took:?}: {ports:?} {reply:#}");
    assert_ne!(ports[0], ports[1], "the second start was told the refused port");
    assert!(detail(&reply).contains("exit status: 3"), "the answer is the second start's: {}", detail(&reply));
    let (reply, took) = begin_scenario("p2-other-exit", "echo \"$1\" >> starts\necho 'no such file' >&2\nexit 1", 20);
    assert_eq!(starts("p2-other-exit").len(), 1, "after {took:?}: {reply:#}");
}

fn alive(pid: &str) -> bool {
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(s) => s.rsplit(')').next().is_some_and(|rest| !rest.trim_start().starts_with('Z')),
        Err(_) => false,
    }
}

fn gone_within(pid: &str, limit: Duration) -> bool {
    let end = Instant::now() + limit;
    while alive(pid) {
        if Instant::now() > end {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    true
}

fn grandchild(name: &str) -> String {
    let file = Path::new(env!("CARGO_TARGET_TMPDIR")).join("ess-pin-guard").join(name).join("tree/gc.pid");
    fs::read_to_string(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display())).trim().to_string()
}

/// A product that closed stdout and neither serves nor exits is bounded by the capture's own
/// `timeout_s` (1 s here), with the message of a capture that did not appear in time. The process
/// it left behind does not outlive the adapter.
#[test]
fn a_product_that_closed_stdout_and_neither_serves_nor_exits_is_bounded_by_timeout_s() {
    let (reply, took) = begin_scenario("p2-closed-silent", "exec 1>&-\nsleep 60 &\necho $! > gc.pid\nwait", 1);
    assert_eq!(reply["target_error"]["kind"], "unavailable", "after {took:?}: {reply:#}");
    let d = detail(&reply);
    assert!(d.contains("capture token did not appear on stdout within 1 s"), "after {took:?}: {d:?}");
    assert!(took >= Duration::from_millis(900) && took < Duration::from_secs(8), "{took:?}");
    assert!(gone_within(&grandchild("p2-closed-silent"), Duration::from_secs(5)), "the product's child outlived the adapter");
}

/// A product that ended while a child it started holds its streams open: the streams do not end,
/// the product did; it did not start, and the child does not outlive the adapter.
#[test]
fn a_product_that_ended_while_a_child_holds_its_streams_did_not_start() {
    let name = "p2-grandchild";
    let (reply, took) = begin_scenario(name, "sleep 60 &\necho $! > gc.pid\nexit 2", 20);
    assert_eq!(reply["target_error"]["kind"], "unavailable", "after {took:?}: {reply:#}");
    let d = detail(&reply);
    assert!(d.contains("the product did not start"), "after {took:?}: {d:?}");
    assert!(took < Duration::from_secs(10), "{took:?}");
    assert!(gone_within(&grandchild(name), Duration::from_secs(5)), "the product's child outlived the adapter");
}
