//! The integrity of the run's copy of the runbook (`docs/engine.md`, "The run's copy of the runbook"):
//! what the copy takes and refuses, what its digest covers, what a resumed run's pin file may be, the
//! modes and bounds of the copy, and a run the runtime fails on while clients answer.

use super::state::scratch;
use runbook::commands::{AnswerDecision, Config, Daemon, SubmitRun};
use runbook::engine::copy::{copy_dirs, digest, Links};
use runbook::engine::pin::{Pin, COPY_DIR};
use runbook::engine::Engine;
use runbook::runtime::{AnswerFailure, Confinement, DecisionRequest, EventSink, Finish, Recovery, Runtime, RuntimeFailure, RunbookInfo, StartRequest};
use serde_json::Value;
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const PIN_FILE: &str = "runbook-pin.json";
const SECRET: &str = "OUTSIDE-SECRET-BYTES";

fn fresh(tag: &str) -> PathBuf {
    std::fs::canonicalize(scratch(&format!("integrity-{tag}"))).unwrap()
}

/// A runbook home of the pin's own unit tests: protocol `demo` at revision 3.
fn demo_home(tag: &str) -> PathBuf {
    let home = fresh(tag);
    std::fs::create_dir_all(home.join("protocol")).unwrap();
    std::fs::create_dir_all(home.join("bindings/tools")).unwrap();
    std::fs::write(home.join("protocol/demo.protocol.yaml"), "protocol:\n  id: demo\n  revision: 3\n").unwrap();
    std::fs::write(home.join("bindings/demo.yaml"), "protocol: demo@3\n").unwrap();
    std::fs::write(home.join("bindings/tools/run.sh"), "#!/bin/sh\n").unwrap();
    home
}

fn dump(dir: &Path) -> BTreeMap<String, String> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let rel = p.strip_prefix(base).unwrap().to_string_lossy().into_owned();
            let m = std::fs::symlink_metadata(&p).unwrap();
            let mode = m.permissions().mode() & 0o7777;
            if m.file_type().is_symlink() {
                out.insert(rel, format!("L {}", std::fs::read_link(&p).unwrap().display()));
            } else if m.is_dir() {
                out.insert(rel.clone(), format!("D {mode:o}"));
                walk(base, &p, out);
            } else {
                out.insert(rel, format!("F {mode:o} {}", String::from_utf8_lossy(&std::fs::read(&p).unwrap())));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn count_files(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap())
        .map(|e| match e.file_type().unwrap() {
            t if t.is_symlink() => 0,
            t if t.is_dir() => count_files(&e.path()),
            _ => 1,
        })
        .sum()
}

// ---- invariant 2: the copy's modes ----

#[test]
fn copy_has_no_file_writable_by_group_or_others_and_no_setuid_bit() {
    let home = demo_home("modes");
    let tools = home.join("bindings/tools");
    for (name, mode) in [("worldwrite.sh", 0o777), ("setuid.sh", 0o4755), ("groupwrite.yaml", 0o664), ("plain.yaml", 0o644)] {
        std::fs::write(tools.join(name), "x\n").unwrap();
        std::fs::set_permissions(tools.join(name), std::fs::Permissions::from_mode(mode)).unwrap();
    }
    let run = fresh("modes-run");
    let (pin, _) = Pin::take(&home, &run, "demo", None).unwrap();
    let mut bad = Vec::new();
    for (rel, what) in dump(&pin.copy) {
        let Some(rest) = what.strip_prefix("F ") else { continue };
        let mode = u32::from_str_radix(rest.split(' ').next().unwrap(), 8).unwrap();
        if mode & 0o022 != 0 || mode & 0o7000 != 0 {
            bad.push(format!("{rel} {mode:o}"));
        }
    }
    assert!(bad.is_empty(), "files of the copy writable by group or others, or setuid/setgid/sticky: {bad:?}");
    assert!(std::fs::metadata(pin.copy.join("bindings/tools/worldwrite.sh")).unwrap().permissions().mode() & 0o100 != 0, "the executable bit is kept");
}

#[test]
fn copy_modes_are_private_to_its_owner_and_the_executable_bit_stays_with_the_owner() {
    let home = demo_home("modes-exact");
    let tools = home.join("bindings/tools");
    let sources = [("plain.yaml", 0o644, 0o600), ("readonly.yaml", 0o444, 0o600), ("exec.sh", 0o755, 0o700), ("group-exec.sh", 0o610, 0o700), ("other-exec.sh", 0o601, 0o700), ("world.sh", 0o777, 0o700)];
    for (name, mode, _) in sources {
        std::fs::write(tools.join(name), "x\n").unwrap();
        std::fs::set_permissions(tools.join(name), std::fs::Permissions::from_mode(mode)).unwrap();
    }
    std::fs::set_permissions(home.join("bindings/tools"), std::fs::Permissions::from_mode(0o775)).unwrap();
    let run = fresh("modes-exact-run");
    let (pin, _) = Pin::take(&home, &run, "demo", None).unwrap();
    for (name, _, want) in sources {
        let mode = std::fs::metadata(pin.copy.join("bindings/tools").join(name)).unwrap().permissions().mode() & 0o7777;
        assert_eq!(mode, want, "{name}: {mode:o}, not {want:o}");
    }
    for (rel, what) in dump(&pin.copy) {
        if let Some(mode) = what.strip_prefix("D ") {
            assert_eq!(mode, "700", "directory {rel}");
        }
    }
    assert_eq!(std::fs::metadata(&pin.copy).unwrap().permissions().mode() & 0o7777, 0o700);
}

#[test]
fn the_digest_of_the_copy_is_the_digest_of_the_home_whatever_the_group_and_other_bits_say() {
    let home = demo_home("modes-digest");
    let tools = home.join("bindings/tools");
    for (name, mode) in [("a.sh", 0o755), ("b.sh", 0o700), ("c.sh", 0o610), ("d.yaml", 0o664), ("e.yaml", 0o600)] {
        std::fs::write(tools.join(name), "x\n").unwrap();
        std::fs::set_permissions(tools.join(name), std::fs::Permissions::from_mode(mode)).unwrap();
    }
    let run = fresh("modes-digest-run");
    let (pin, _) = Pin::take(&home, &run, "demo", None).unwrap();
    assert_eq!(pin.bindings_sha256, digest(&home.join("bindings")).unwrap());
    // Two homes that differ only in the group and other bits give one digest.
    std::fs::set_permissions(tools.join("a.sh"), std::fs::Permissions::from_mode(0o711)).unwrap();
    std::fs::set_permissions(tools.join("d.yaml"), std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(pin.bindings_sha256, digest(&home.join("bindings")).unwrap());
}

#[test]
fn the_digest_of_a_fixed_tree_has_the_value_it_had_before_the_copy_was_made_private() {
    // The expected value is the sha256 of the documented byte layout, built with `printf` and
    // `sha256sum` (the layout of `docs/engine.md`): a digest recorded by an earlier daemon stays valid.
    let dir = fresh("digest-fixed");
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    std::fs::create_dir(dir.join("e")).unwrap();
    std::os::unix::fs::symlink("a.txt", dir.join("l")).unwrap();
    std::fs::create_dir(dir.join("sub")).unwrap();
    std::fs::write(dir.join("sub/run.sh"), "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(dir.join("sub/run.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(digest(&dir).unwrap(), "c97bd05c1e811f2f39a1c461f31aa0da80e385a88ad03f59a43160184e48b134");
}

#[test]
fn the_digest_of_a_file_larger_than_any_buffer_covers_every_byte() {
    use sha2::{Digest, Sha256};
    let dir = fresh("digest-big");
    let bytes: Vec<u8> = (0..300_000u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
    std::fs::write(dir.join("big.bin"), &bytes).unwrap();
    let mut layout = Sha256::new();
    layout.update(b"F\0big.bin\0");
    layout.update(bytes.len().to_string().as_bytes());
    layout.update([0, 0, 0]);
    layout.update(&bytes);
    layout.update([0]);
    let want: String = layout.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(digest(&dir).unwrap(), want);
    let mut edited = bytes.clone();
    *edited.last_mut().unwrap() ^= 1;
    std::fs::write(dir.join("big.bin"), &edited).unwrap();
    assert_ne!(digest(&dir).unwrap(), want, "the last byte is not in the digest");
}

// ---- invariant 7: a resumed daemon's pin ----

fn taken(tag: &str) -> (PathBuf, PathBuf, Pin) {
    let home = demo_home(tag);
    let run = fresh(&format!("{tag}-run"));
    let (pin, _) = Pin::take(&home, &run, "demo", None).unwrap();
    (home, run, pin)
}

/// A run whose copy stands and whose pin file is not what the submit wrote ends by the error a changed
/// copy has: no path of the run, no byte of the file. `load` runs on a thread: a pin file that is a pipe
/// is refused, never waited on.
fn refused_as_changed(run: &Path, what: &str) {
    let dir = run.to_path_buf();
    let loaded = bounded(5, move || Pin::load(&dir)).unwrap_or_else(|| panic!("{what}: Pin::load did not return within 5 s"));
    let refusal = loaded.expect_err(what);
    assert!(refusal.starts_with("RunbookCopyChanged"), "{what}: {refusal}");
    assert!(!refusal.contains(run.to_str().unwrap()), "{what}: the refusal names a path of the run: {refusal}");
    assert!(!refusal.contains("copy_sha256") && !refusal.contains("demo"), "{what}: the refusal holds a byte of the pin file: {refusal}");
}

#[test]
fn pin_load_refuses_a_run_whose_pin_file_is_gone_while_its_copy_stands() {
    let (_home, run, pin) = taken("pin-gone");
    std::fs::remove_file(run.join(PIN_FILE)).unwrap();
    assert!(pin.copy.is_dir());
    refused_as_changed(&run, "a pin file gone, the copy standing");
}

#[test]
fn pin_load_gives_no_pin_to_a_run_that_has_neither_a_copy_nor_a_pin_file() {
    let run = fresh("pin-before");
    assert_eq!(Pin::load(&run), Ok(None));
    std::fs::create_dir(run.join("tree")).unwrap();
    assert_eq!(Pin::load(&run), Ok(None));
}

#[test]
fn pin_load_refuses_a_pin_file_that_is_not_text() {
    let (_home, run, _pin) = taken("pin-binary");
    std::fs::write(run.join(PIN_FILE), [0xffu8, 0xfe, 0x00, 0x80]).unwrap();
    refused_as_changed(&run, "a pin file that is not text");
}

#[test]
fn pin_load_refuses_a_pin_file_that_is_a_directory() {
    let (_home, run, _pin) = taken("pin-dir");
    std::fs::remove_file(run.join(PIN_FILE)).unwrap();
    std::fs::create_dir(run.join(PIN_FILE)).unwrap();
    refused_as_changed(&run, "a directory in place of the pin file");
}

#[test]
fn pin_load_refuses_a_pin_file_that_is_a_link() {
    let (_home, run, _pin) = taken("pin-link");
    let outside = fresh("pin-link-outside");
    std::fs::rename(run.join(PIN_FILE), outside.join("pin.json")).unwrap();
    std::os::unix::fs::symlink(outside.join("pin.json"), run.join(PIN_FILE)).unwrap();
    refused_as_changed(&run, "a pin file that is a link");
}

#[test]
fn pin_load_refuses_a_pin_file_that_is_a_pipe_without_waiting_on_it() {
    use std::os::unix::ffi::OsStrExt;
    let (_home, run, _pin) = taken("pin-pipe");
    std::fs::remove_file(run.join(PIN_FILE)).unwrap();
    let path = std::ffi::CString::new(run.join(PIN_FILE).as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
    refused_as_changed(&run, "a pin file that is a pipe");
}

#[test]
fn pin_load_refuses_a_pin_file_that_has_a_second_name() {
    let (_home, run, _pin) = taken("pin-second-name");
    std::fs::hard_link(run.join(PIN_FILE), run.join("elsewhere.json")).unwrap();
    refused_as_changed(&run, "a pin file with a second name");
}

#[test]
fn pin_load_refuses_a_copy_that_is_a_link() {
    let (_home, run, pin) = taken("pin-copy-link");
    let outside = fresh("pin-copy-link-outside");
    std::fs::rename(&pin.copy, outside.join("copy")).unwrap();
    std::os::unix::fs::symlink(outside.join("copy"), &pin.copy).unwrap();
    let refusal = Pin::load(&run).unwrap_err();
    assert!(refusal.starts_with("RunbookCopyChanged") && !refusal.contains(run.to_str().unwrap()), "{refusal}");
}

#[test]
fn pin_load_refuses_a_malformed_pin_and_a_digest_the_copy_does_not_have() {
    let (_home, run, pin) = taken("pin-guard");
    assert_eq!(Pin::load(&run).unwrap(), Some(pin.clone()));
    let good = std::fs::read_to_string(run.join(PIN_FILE)).unwrap();
    std::fs::write(run.join(PIN_FILE), "{not json").unwrap();
    assert!(Pin::load(&run).is_err());
    std::fs::write(run.join(PIN_FILE), good.replace(&pin.copy_sha256, &"0".repeat(64))).unwrap();
    assert!(Pin::load(&run).unwrap_err().starts_with("RunbookCopyChanged"));
    std::fs::write(run.join(PIN_FILE), good.replace("\"revision\":3", "\"revision\":\"3\"")).unwrap();
    assert!(Pin::load(&run).is_err());
    std::fs::write(run.join(PIN_FILE), &good).unwrap();
    std::fs::write(pin.copy.join("bindings/tools/run.sh"), "#!/bin/sh\nexit 3\n").unwrap();
    assert!(Pin::load(&run).unwrap_err().starts_with("RunbookCopyChanged"));
}

// ---- invariant 5: the digest's members ----

#[test]
fn the_digest_covers_path_mode_directories_links_and_bytes() {
    let home = demo_home("digest-members");
    let dir = home.join("bindings");
    let base = digest(&dir).unwrap();
    // mode: the executable bit
    std::fs::set_permissions(dir.join("demo.yaml"), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_ne!(base, digest(&dir).unwrap(), "the executable bit is not in the digest");
    std::fs::set_permissions(dir.join("demo.yaml"), std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(base, digest(&dir).unwrap());
    // path: the same bytes under another name, in the same directory and in another one
    std::fs::rename(dir.join("demo.yaml"), dir.join("other.yaml")).unwrap();
    assert_ne!(base, digest(&dir).unwrap(), "a rename is not in the digest");
    std::fs::rename(dir.join("other.yaml"), dir.join("tools/demo.yaml")).unwrap();
    assert_ne!(base, digest(&dir).unwrap(), "a move into a directory is not in the digest");
    std::fs::rename(dir.join("tools/demo.yaml"), dir.join("demo.yaml")).unwrap();
    assert_eq!(base, digest(&dir).unwrap());
    // a directory: an empty one added, nested, then removed
    std::fs::create_dir(dir.join("empty")).unwrap();
    let with_dir = digest(&dir).unwrap();
    assert_ne!(base, with_dir, "an empty directory is not in the digest");
    std::fs::create_dir(dir.join("empty/inner")).unwrap();
    assert_ne!(with_dir, digest(&dir).unwrap(), "a nested empty directory is not in the digest");
    std::fs::remove_dir(dir.join("empty/inner")).unwrap();
    std::fs::remove_dir(dir.join("empty")).unwrap();
    assert_eq!(base, digest(&dir).unwrap());
    // a link swapped for a file with the same bytes, and a link's target
    std::fs::write(dir.join("a.txt"), "protocol: demo@3\n").unwrap();
    std::os::unix::fs::symlink("demo.yaml", dir.join("l")).unwrap();
    let linked = digest(&dir).unwrap();
    std::fs::remove_file(dir.join("l")).unwrap();
    std::fs::write(dir.join("l"), "protocol: demo@3\n").unwrap();
    assert_ne!(linked, digest(&dir).unwrap(), "a link replaced by a file of the same bytes is not in the digest");
    std::fs::remove_file(dir.join("l")).unwrap();
    std::os::unix::fs::symlink("a.txt", dir.join("l")).unwrap();
    assert_ne!(linked, digest(&dir).unwrap(), "a link's target is not in the digest");
    // the bytes, and a length-preserving edit
    std::fs::remove_file(dir.join("l")).unwrap();
    std::fs::remove_file(dir.join("a.txt")).unwrap();
    assert_eq!(base, digest(&dir).unwrap());
    std::fs::write(dir.join("demo.yaml"), "protocol: demo@4\n").unwrap();
    assert_ne!(base, digest(&dir).unwrap());
}

#[test]
fn digest_tells_two_names_apart_that_no_text_can_write() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let (a, b) = (fresh("digest-names-a"), fresh("digest-names-b"));
    std::fs::write(a.join(OsStr::from_bytes(b"x\xff")), "same\n").unwrap();
    std::fs::write(b.join(OsStr::from_bytes(b"x\xfe")), "same\n").unwrap();
    assert_ne!(digest(&a).unwrap(), digest(&b).unwrap(), "two different names have one digest (the name is taken as lossy text)");
}

// ---- invariant 2: the link policy ----

/// What `f` returns, or `None` when it has not returned within `secs`: a case never waits on an entry
/// that blocks (a pipe), it fails.
fn bounded<T: Send + 'static>(secs: u64, f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(secs)).ok()
}

fn within(home: &Path, tag: &str) -> (Result<(), String>, PathBuf) {
    let dst = fresh(&format!("{tag}-dst")).join("bindings");
    let (h, d) = (home.to_path_buf(), dst.clone());
    let made = bounded(5, move || copy_dirs(&[(&h.join("bindings"), &d)], Links::Within(&h)));
    (made.expect("the copy did not return within 5 s: it waits on an entry that is not a regular file"), dst)
}

#[test]
fn every_way_out_of_the_home_is_refused_before_the_first_write_and_names_nothing_outside() {
    use std::os::unix::ffi::OsStrExt;
    let up = "../".repeat(40);
    type Make = Box<dyn Fn(&Path, &Path)>;
    fn mk(f: impl Fn(&Path, &Path) + 'static) -> Make {
        Box::new(f)
    }
    let cases: Vec<(&str, Make)> = vec![
        ("file link", mk(|h, o| std::os::unix::fs::symlink(o.join("secret.txt"), h.join("bindings/tools/l")).unwrap())),
        ("directory link", mk(|h, o| std::os::unix::fs::symlink(o, h.join("bindings/tools/l")).unwrap())),
        (
            "link to a link",
            mk(|h, o| {
                std::os::unix::fs::symlink(o.join("secret.txt"), h.join("bindings/tools/l2")).unwrap();
                std::os::unix::fs::symlink("l2", h.join("bindings/tools/l")).unwrap();
            }),
        ),
        (
            "relative link with ..",
            mk(move |h, o| std::os::unix::fs::symlink(format!("{up}{}", o.join("secret.txt").to_str().unwrap().trim_start_matches('/')), h.join("bindings/tools/l")).unwrap()),
        ),
        ("/proc/self/cwd", mk(|h, _| std::os::unix::fs::symlink("/proc/self/cwd", h.join("bindings/tools/l")).unwrap())),
        ("/proc/self/root", mk(|h, _| std::os::unix::fs::symlink("/proc/self/root", h.join("bindings/tools/l")).unwrap())),
        ("/proc/self/exe", mk(|h, _| std::os::unix::fs::symlink("/proc/self/exe", h.join("bindings/tools/l")).unwrap())),
        ("device", mk(|h, _| std::os::unix::fs::symlink("/dev/null", h.join("bindings/tools/l")).unwrap())),
        (
            "fifo",
            mk(|h, _| {
                let path = std::ffi::CString::new(h.join("bindings/tools/l").as_os_str().as_bytes()).unwrap();
                assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o644) }, 0);
            }),
        ),
    ];
    for (n, (what, make)) in cases.iter().enumerate() {
        let home = demo_home(&format!("escape-{n}"));
        let outside = fresh(&format!("escape-{n}-outside-OUTSIDEDIR"));
        std::fs::write(outside.join("secret.txt"), SECRET).unwrap();
        make(&home, &outside);
        let (refused, dst) = within(&home, &format!("escape-{n}"));
        let reason = refused.expect_err(what);
        assert!(!dst.exists() && !dst.parent().unwrap().join("bindings").exists(), "{what}: a destination was made");
        assert!(!reason.contains("OUTSIDEDIR") && !reason.contains(SECRET), "{what}: the reason names something outside the home: {reason}");
    }
    // a socket
    let home = demo_home("escape-socket");
    // The path of a socket is short: bind through the descriptor of its directory.
    let holder = std::fs::File::open(home.join("bindings/tools")).unwrap();
    let _listener = std::os::unix::net::UnixListener::bind(format!("/proc/self/fd/{}/l", std::os::fd::AsRawFd::as_raw_fd(&holder))).unwrap();
    let (refused, dst) = within(&home, "escape-socket");
    assert!(refused.unwrap_err().contains("not a regular file"));
    assert!(!dst.exists());
}

#[test]
fn a_link_that_returns_into_the_home_by_another_name_and_a_hard_link_are_copied_as_files() {
    let home = demo_home("inside");
    std::os::unix::fs::symlink("../../bindings/demo.yaml", home.join("bindings/tools/back")).unwrap();
    std::fs::hard_link(home.join("bindings/demo.yaml"), home.join("bindings/tools/hard")).unwrap();
    let (copied, dst) = within(&home, "inside");
    copied.unwrap();
    for name in ["tools/back", "tools/hard"] {
        let meta = std::fs::symlink_metadata(dst.join(name)).unwrap();
        assert!(meta.is_file() && !meta.file_type().is_symlink(), "{name}");
        assert_eq!(std::fs::read_to_string(dst.join(name)).unwrap(), "protocol: demo@3\n");
    }
}

#[test]
fn copy_of_links_that_fan_out_is_bounded() {
    // Eleven directories, each holding two links to the next: the home has one regular file more than
    // `demo_home`, and the links lead to 2^11 copies of it.
    let home = demo_home("fan");
    let dir = home.join("bindings");
    let levels = 11;
    for i in 0..=levels {
        std::fs::create_dir(dir.join(format!("d{i}"))).unwrap();
    }
    for i in 0..levels {
        for name in ["a", "b"] {
            std::os::unix::fs::symlink(format!("../d{}", i + 1), dir.join(format!("d{i}/{name}"))).unwrap();
        }
    }
    std::fs::write(dir.join(format!("d{levels}/leaf.txt")), "leaf\n").unwrap();
    let real = count_files(&dir);
    let started = Instant::now();
    let (copied, dst) = within(&home, "fan");
    let took = started.elapsed();
    let made = if copied.is_ok() { count_files(&dst) } else { 0 };
    assert!(copied.is_err() || made <= real * 10, "a home of {real} files was copied as {made} files in {took:?}");
    if let Err(reason) = &copied {
        assert!(!dst.exists(), "a refused copy wrote its destination");
        assert!(!reason.contains(home.to_str().unwrap()), "the refusal names a path: {reason}");
    }
}

#[test]
fn a_copy_over_the_bound_of_bytes_is_refused_before_the_first_write_and_names_no_path() {
    // A sparse file of 16 MiB and one byte: the plan counts its length, nothing is read or written.
    let home = demo_home("bytes");
    let big = std::fs::File::create(home.join("bindings/tools/over.bin")).unwrap();
    big.set_len((16 << 20) + 1).unwrap();
    drop(big);
    let (copied, dst) = within(&home, "bytes");
    let reason = copied.expect_err("a home of 16 MiB and one byte was copied");
    assert!(!dst.exists(), "a refused copy wrote its destination");
    assert!(!reason.contains(home.to_str().unwrap()) && !reason.contains("over.bin"), "the refusal names a path: {reason}");
    // The same home with a file of 15 MiB is within the bound.
    std::fs::File::create(home.join("bindings/tools/over.bin")).unwrap().set_len(15 << 20).unwrap();
    let (copied, dst) = within(&home, "bytes-under");
    copied.unwrap();
    assert_eq!(std::fs::metadata(dst.join("tools/over.bin")).unwrap().len(), 15 << 20);
}

#[test]
fn a_copy_of_exactly_4096_entries_is_made_and_one_entry_more_is_refused() {
    // `bindings/`, `bindings/tools/`, `demo.yaml` and `tools/run.sh` are 4 entries.
    let home = demo_home("entries");
    let tools = home.join("bindings/tools");
    for n in 0..4092 {
        std::fs::write(tools.join(format!("f{n:04}.txt")), "x\n").unwrap();
    }
    let (copied, dst) = within(&home, "entries-at");
    copied.unwrap();
    assert_eq!(count_files(&dst), 4094);
    std::fs::write(tools.join("one-more.txt"), "x\n").unwrap();
    let (copied, dst) = within(&home, "entries-over");
    let reason = copied.expect_err("4097 entries were copied");
    assert!(!dst.exists(), "a refused copy wrote its destination");
    assert!(!reason.contains(home.to_str().unwrap()), "the refusal names a path: {reason}");
}

#[test]
fn copy_takes_nothing_from_outside_when_a_link_is_put_in_after_the_check() {
    let mut leaked = Vec::new();
    for trial in 0..3 {
        let home = demo_home(&format!("race-{trial}"));
        let outside = fresh(&format!("race-{trial}-outside"));
        std::fs::write(outside.join("secret.txt"), SECRET).unwrap();
        let tools = home.join("bindings/tools");
        for n in 0..1500 {
            std::fs::write(tools.join(format!("f{n:04}.txt")), "x\n").unwrap();
        }
        std::fs::write(tools.join("victim.txt"), "inside\n").unwrap();
        let dst = fresh(&format!("race-{trial}-dst")).join("bindings");
        let done = Arc::new(AtomicBool::new(false));
        // The second writer in the home: the first directory of the copy is made after every check.
        let writer = {
            let (dst, victim, secret, done) = (dst.clone(), tools.join("victim.txt"), outside.join("secret.txt"), done.clone());
            std::thread::spawn(move || {
                let limit = Instant::now() + Duration::from_secs(20);
                while !dst.exists() && !done.load(Ordering::SeqCst) && Instant::now() < limit {
                    std::hint::spin_loop();
                }
                let _ = std::fs::remove_file(&victim);
                let _ = std::os::unix::fs::symlink(&secret, &victim);
            })
        };
        let copied = copy_dirs(&[(&home.join("bindings"), &dst)], Links::Within(&home));
        done.store(true, Ordering::SeqCst);
        writer.join().unwrap();
        if copied.is_err() {
            assert!(!dst.exists(), "a refused copy wrote its destination (trial {trial})");
        }
        if copied.is_ok() && std::fs::read_to_string(dst.join("tools/victim.txt")).is_ok_and(|t| t == SECRET) {
            leaked.push(trial);
        }
    }
    assert!(leaked.is_empty(), "a file outside the home is in the copy (trials {leaked:?}): the link was put in after the check and the copy followed it");
}

// ---- invariant 9: `runbook run`'s copy (Links::Keep) as the base made it ----

/// The routine `runbook run` copied its bindings with before the unit (`loom_run/bindings_copy.rs` at 914dffc2), verbatim.
fn base_copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    let entries = std::fs::read_dir(src).map_err(|e| format!("{}: {e}", src.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", src.display()))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let kind = entry.file_type().map_err(|e| format!("{}: {e}", from.display()))?;
        if kind.is_symlink() {
            let target = std::fs::read_link(&from).map_err(|e| format!("{}: {e}", from.display()))?;
            std::os::unix::fs::symlink(&target, &to).map_err(|e| format!("{}: {e}", to.display()))?;
        } else if kind.is_dir() {
            base_copy_dir(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
            let mut permissions = std::fs::metadata(&to).map_err(|e| format!("{}: {e}", to.display()))?.permissions();
            permissions.set_mode(permissions.mode() | 0o200);
            std::fs::set_permissions(&to, permissions).map_err(|e| format!("{}: {e}", to.display()))?;
        }
    }
    Ok(())
}

#[test]
fn keep_copies_byte_for_byte_what_the_base_routine_copied() {
    let src = fresh("keep-src");
    let outside = fresh("keep-outside");
    std::fs::write(outside.join("o.txt"), "outside\n").unwrap();
    std::fs::create_dir_all(src.join("a/b")).unwrap();
    std::fs::create_dir_all(src.join("empty")).unwrap();
    std::fs::create_dir_all(src.join("empty2/inner")).unwrap();
    std::fs::write(src.join("a/b/c.txt"), "c\n").unwrap();
    std::fs::write(src.join("a/run.sh"), "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(src.join("a/run.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(src.join("ro.txt"), "ro\n").unwrap();
    std::fs::set_permissions(src.join("ro.txt"), std::fs::Permissions::from_mode(0o444)).unwrap();
    std::fs::write(src.join("odd.txt"), "odd\n").unwrap();
    std::fs::set_permissions(src.join("odd.txt"), std::fs::Permissions::from_mode(0o4750)).unwrap();
    std::os::unix::fs::symlink("run.sh", src.join("a/ln")).unwrap();
    std::os::unix::fs::symlink("a", src.join("dl")).unwrap();
    std::os::unix::fs::symlink("nowhere", src.join("dang")).unwrap();
    std::os::unix::fs::symlink(outside.join("o.txt"), src.join("out")).unwrap();
    std::os::unix::fs::symlink(&outside, src.join("outdir")).unwrap();
    let (base, head) = (fresh("keep-base").join("x"), fresh("keep-head").join("x"));
    base_copy_dir(&src, &base).unwrap();
    copy_dirs(&[(&src, &head)], Links::Keep).unwrap();
    assert_eq!(dump(&base), dump(&head));
    assert!(dump(&head).contains_key("empty2/inner"));
}

#[test]
fn keep_copies_a_directory_nested_deeper_than_24_levels_as_the_base_did() {
    let src = fresh("keep-deep-src");
    let mut deep = src.clone();
    for _ in 0..30 {
        deep = deep.join("d");
    }
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(deep.join("f.txt"), "deep\n").unwrap();
    let base = fresh("keep-deep-base").join("x");
    base_copy_dir(&src, &base).unwrap();
    let head = fresh("keep-deep-head").join("x");
    let copied = copy_dirs(&[(&src, &head)], Links::Keep);
    assert!(copied.is_ok(), "`runbook run` copied this directory before the unit and now: {copied:?}");
}

// ---- the real engine ----

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

const RUNBOOK: &str = "engine-demo";

struct Real {
    dir: PathBuf,
    home: PathBuf,
    d: Daemon,
}

impl Real {
    fn new(tag: &str, floor: Confinement) -> Real {
        let dir = fresh(tag);
        let home = dir.join("home");
        copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &home);
        let binding = home.join("bindings/engine-draft.yaml");
        let text = std::fs::read_to_string(&binding).unwrap().replace("principal: reviewer", &format!("principal: {}", super::PRINCIPAL));
        std::fs::write(&binding, text).unwrap();
        std::fs::create_dir_all(dir.join("ctx")).unwrap();
        let engine = Arc::new(Engine::with_levels(home.clone(), dir.join("state"), &[Confinement::Local, Confinement::Linux]));
        let cfg = Config { state_dir: dir.join("state"), confinement_floor: floor, context_roots: vec![dir.join("ctx")], runbook_dir: None, principal: super::PRINCIPAL.into() };
        Real { dir, home, d: Daemon::start(cfg, engine).unwrap() }
    }

    fn state(&self) -> PathBuf {
        self.dir.join("state")
    }

    fn submit(&self, id: &str, version: Option<&str>, level: Confinement) -> runbook::commands::Outcome {
        self.d.submit_run(SubmitRun { runbook_id: id.into(), runbook_version: version.map(String::from), intake: "a note on tea".into(), confinement: Some(level), ..SubmitRun::default() }).unwrap()
    }

    fn state_of(&self, run: &str) -> String {
        self.d.run_detail(run)[0]["state"].as_str().unwrap().to_string()
    }

    fn row(&self, run: &str) -> Value {
        self.d.run_detail(run)[0].clone()
    }

    fn runs_made(&self) -> usize {
        std::fs::read_dir(self.state().join("runs")).map(|d| d.count()).unwrap_or(0)
    }

    fn until(&self, what: &str, secs: u64, mut done: impl FnMut(&Real) -> bool) {
        let limit = Instant::now() + Duration::from_secs(secs);
        while !done(self) {
            assert!(Instant::now() < limit, "gave up waiting for {what}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Answer what the run asks until it is over.
    fn settle(&self, run: &str) {
        let mut answered = std::collections::BTreeSet::new();
        self.until(&format!("run {run} to be over"), 50, |me| {
            if matches!(me.state_of(run).as_str(), "Ended" | "Cancelled" | "FailedToStart") {
                return true;
            }
            for d in me.d.open_decisions_of_run(run) {
                let id = d["request_id"].as_str().unwrap().to_string();
                if !answered.insert(id.clone()) {
                    continue;
                }
                let review = d["options"].as_array().unwrap().iter().any(|o| o["answer"].as_str().is_some_and(|a| a.contains("result")));
                let (answer, who) = if review { ("performed", "reviewer") } else { ("approve", "owner") };
                let _ = me.d.answer_decision(AnswerDecision { run_id: run.into(), request_id: id, answer: answer.into(), answered_by: who.into() });
            }
            false
        });
    }
}

impl Drop for Real {
    fn drop(&mut self) {
        self.d.stop();
    }
}

#[test]
fn a_program_that_rewrites_a_later_program_of_its_round_ends_the_run_at_the_next_check() {
    // `docs/engine.md`: at confinement `local` a bound program is unconfined. Inside one round (one
    // `run_tree`) the writer rewrites the check that runs after it, in the run's copy, and the check
    // runs the new bytes; the digest is read before each round and each answer, so the run ends
    // `runtime-failure` at the next one. A check before each program is Loom's, not the engine's.
    let rig = Real::new("window", Confinement::Local);
    let writer = "#!/bin/sh\ndir=\"$(dirname \"$0\")\"\nprintf '#!/bin/sh\\necho rewritten > marker.txt\\nexit 0\\n' > \"$dir/draft-check.sh\"\necho draft >> draft.txt\n";
    std::fs::write(rig.home.join("bindings/tools/draft-write.sh"), writer).unwrap();
    let run = rig.submit(RUNBOOK, None, Confinement::Local).run_id.unwrap();
    rig.settle(&run);
    let row = rig.row(&run);
    assert_eq!(row["outcome"], "runtime-failure", "{row}");
    let failed: Vec<Value> = rig.d.events_after(&run, 0).into_iter().filter(|e| e["kind"] == "runbook.runtime-failed").collect();
    assert_eq!(failed.len(), 1, "{failed:?}");
    assert!(failed[0]["record"]["reason"].as_str().is_some_and(|r| r.starts_with("RunbookCopyChanged")), "{failed:?}");
    let marker = rig.state().join("workspaces").join(&run).join("marker.txt");
    assert!(marker.exists(), "the later program of the round did not run the bytes the earlier one wrote: the documentation says it does; the run: {row}");
}

#[test]
fn a_name_that_is_not_one_of_the_home_is_refused_and_no_run_directory_is_made() {
    let rig = Real::new("names", Confinement::Local);
    // Runbooks that do exist at a nested path and under a hidden name: a name still names none of them.
    for (dir_protocol, dir_bindings, file) in [("protocol/sub", "bindings/sub", "engine-demo"), ("protocol", "bindings", ".hid")] {
        std::fs::create_dir_all(rig.home.join(dir_protocol)).unwrap();
        std::fs::create_dir_all(rig.home.join(dir_bindings)).unwrap();
        std::fs::copy(rig.home.join("protocol/engine-demo.protocol.yaml"), rig.home.join(dir_protocol).join(format!("{file}.protocol.yaml"))).unwrap();
        std::fs::copy(rig.home.join("bindings/engine-demo.yaml"), rig.home.join(dir_bindings).join(format!("{file}.yaml"))).unwrap();
    }
    for id in ["sub/engine-demo", ".hid"] {
        let o = rig.submit(id, None, Confinement::Local);
        assert!(o.is_refusal() && o.name == "no-runbook", "{id:?}: {o:?}");
    }
    let long = "a".repeat(4096);
    let names = [
        long.as_str(),
        "ENGINE-DEMO",
        "Engine-demo",
        "engine-demo\0",
        "engine-demo/",
        "/engine-demo",
        "engine-demo\u{301}",
        "engine\u{2011}demo",
        "\u{ff45}ngine-demo",
        "engine-demo ",
        "engine-demo\n",
        "..",
        "../home/engine-demo",
        "bindings/../engine-demo",
        "engine-demo.protocol.yaml",
    ];
    for id in names {
        let o = rig.submit(id, None, Confinement::Local);
        assert!(o.is_refusal() && o.name == "no-runbook", "{:?}: {o:?}", id.chars().take(20).collect::<String>());
    }
    for version in ["01", "+1", "1 ", "1\0", "1.0", "\u{ff11}", long.as_str()] {
        let o = rig.submit(RUNBOOK, Some(version), Confinement::Local);
        assert!(o.is_refusal() && o.name == "no-runbook", "version {:?}: {o:?}", version.chars().take(20).collect::<String>());
    }
    assert_eq!(rig.runs_made(), 0, "a refused submit made a run directory");
    let ok = rig.submit(RUNBOOK, Some("1"), Confinement::Local);
    assert_eq!(ok.name, "submitted", "{ok:?}");
}

#[test]
fn a_refusal_of_the_copy_names_no_path_and_no_byte_from_outside_the_home() {
    for (n, kind) in ["tool", "protocol", "directory"].iter().enumerate() {
        let rig = Real::new(&format!("reason-{n}"), Confinement::Local);
        let outside = fresh(&format!("reason-{n}-outside-OUTSIDEDIR"));
        std::fs::write(outside.join("secret.txt"), SECRET).unwrap();
        match *kind {
            "tool" => std::os::unix::fs::symlink(outside.join("secret.txt"), rig.home.join("bindings/tools/leak.sh")).unwrap(),
            "protocol" => {
                let file = rig.home.join("protocol/engine-demo.protocol.yaml");
                let text = std::fs::read_to_string(&file).unwrap();
                std::fs::write(outside.join("p.yaml"), format!("{text}\n# {SECRET}\n")).unwrap();
                std::fs::remove_file(&file).unwrap();
                std::os::unix::fs::symlink(outside.join("p.yaml"), &file).unwrap();
            }
            _ => std::os::unix::fs::symlink(&outside, rig.home.join("bindings/tools/leak")).unwrap(),
        }
        let o = rig.submit(RUNBOOK, None, Confinement::Local);
        let run = o.run_id.clone().unwrap_or_default();
        if o.is_refusal() {
            assert!(!format!("{o:?}").contains(SECRET) && !format!("{o:?}").contains("OUTSIDEDIR"), "{kind}: {o:?}");
            continue;
        }
        rig.until("the run to fail to start", 40, |me| me.state_of(&run) == "FailedToStart");
        let seen = format!("{} {:?} {:?}", rig.row(&run), rig.d.events_after(&run, 0), rig.d.decisions_of_run(&run));
        assert!(!seen.contains(SECRET) && !seen.contains("OUTSIDEDIR"), "{kind}: the run's record names something outside the home: {seen}");
        let run_dir = rig.state().join("runs").join(&run);
        assert!(!run_dir.join(COPY_DIR).exists() && !run_dir.join(PIN_FILE).exists() && !run_dir.join("tree").exists(), "{kind}: the run's directory holds something");
    }
}

#[test]
fn below_the_floor_nothing_starts_and_nothing_is_written() {
    let rig = Real::new("floor", Confinement::Linux);
    let o = rig.submit(RUNBOOK, None, Confinement::Local);
    assert!(o.is_refusal() && o.name == "below-floor", "{o:?}");
    assert_eq!(rig.runs_made(), 0);
}

#[test]
fn no_client_visible_record_names_the_path_of_the_copy() {
    let rig = Real::new("paths", Confinement::Local);
    let run = rig.submit(RUNBOOK, None, Confinement::Local).run_id.unwrap();
    rig.settle(&run);
    assert_eq!(rig.row(&run)["outcome"], "delivered", "{}", rig.row(&run));
    let copy = rig.state().join("runs").join(&run).join(COPY_DIR);
    let state = rig.state();
    let mut seen = String::new();
    for e in rig.d.events_after(&run, 0) {
        seen.push_str(&e.to_string());
        seen.push('\n');
    }
    seen.push_str(&format!("{}\n{:?}\n", rig.row(&run), rig.d.decisions_of_run(&run)));
    let at = seen.find(state.to_str().unwrap()).map(|i| seen[i.saturating_sub(120)..(i + 200).min(seen.len())].to_string());
    assert!(!seen.contains(copy.to_str().unwrap()), "a record names the copy: {at:?}");
}

impl Real {
    fn wait_review(&self, run: &str) -> String {
        let mut found = None;
        self.until("the review request", 40, |me| {
            found = me.d.open_decisions_of_run(run).into_iter().find(|d| d["options"].as_array().unwrap().iter().any(|o| o["answer"].as_str().is_some_and(|a| a.contains("result"))));
            found.is_some()
        });
        found.unwrap()["request_id"].as_str().unwrap().to_string()
    }
}

#[test]
fn a_copy_that_cannot_be_read_ends_the_run_without_naming_a_path_of_the_state_directory() {
    let rig = Real::new("unreadable", Confinement::Local);
    let run = rig.submit(RUNBOOK, None, Confinement::Local).run_id.unwrap();
    let review = rig.wait_review(&run);
    let file = rig.state().join("runs").join(&run).join(COPY_DIR).join("bindings/tools/draft-check.sh");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
    let _ = rig.d.answer_decision(AnswerDecision { run_id: run.clone(), request_id: review, answer: "performed".into(), answered_by: "reviewer".into() });
    rig.until("the run to end", 40, |me| me.state_of(&run) == "Ended");
    let events: Vec<Value> = rig.d.events_after(&run, 0).into_iter().filter(|e| e["kind"] == "runbook.runtime-failed").collect();
    assert_eq!(events.len(), 1, "{events:?}");
    let reason = events[0]["record"]["reason"].as_str().unwrap();
    assert!(reason.starts_with("RunbookCopyChanged"), "{reason}");
    assert!(!reason.contains(rig.state().to_str().unwrap()), "the refusal names a path of the state directory: {reason}");
}

#[test]
fn a_bindings_file_that_loom_refuses_at_open_names_no_path_of_the_state_directory() {
    let rig = Real::new("open-refused", Confinement::Local);
    std::fs::write(rig.home.join("bindings/engine-demo.yaml"), "format: loom.bindings/1\nprotocol: engine-demo@9\n").unwrap();
    let run = rig.submit(RUNBOOK, None, Confinement::Local).run_id.unwrap();
    rig.until("the run to fail", 40, |me| matches!(me.state_of(&run).as_str(), "FailedToStart" | "Ended"));
    let seen = format!("{} {:?}", rig.row(&run), rig.d.events_after(&run, 0));
    assert!(seen.contains("FailedToStart") || seen.contains("runtime-failure"), "{seen}");
    assert!(!seen.contains(rig.state().to_str().unwrap()), "the run's record names a path of the state directory: {seen}");
}

#[test]
fn the_pinned_event_is_one_record_of_five_members_before_every_line_of_loom() {
    let rig = Real::new("pinned", Confinement::Local);
    let run = rig.submit(RUNBOOK, None, Confinement::Local).run_id.unwrap();
    rig.settle(&run);
    let events = rig.d.events_after(&run, 0);
    let kinds: Vec<&str> = events.iter().map(|e| e["kind"].as_str().unwrap()).collect();
    let at = kinds.iter().position(|k| *k == "runbook.pinned").expect("no runbook.pinned");
    assert_eq!(kinds.iter().filter(|k| **k == "runbook.pinned").count(), 1);
    assert!(kinds[..at].iter().all(|k| k.starts_with("runbook.")), "a line of Loom comes before the pin: {kinds:?}");
    let members: Vec<&str> = events[at]["record"].as_object().unwrap().keys().map(String::as_str).collect();
    let mut sorted = members.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, ["bindings_sha256", "copy_sha256", "protocol", "revision", "runbook"]);
    assert!(!events[at]["record"].to_string().contains('/'), "{}", events[at]["record"]);
}

// ---- invariant 6: a failed run and a client that answers ----

/// A runtime of one run with two open requests. The first answer makes it fail; with `held` it
/// keeps the failure to itself until `release`, so that a client can answer after the runtime failed
/// and before the daemon heard of it.
struct Dying {
    state: Mutex<DyingState>,
    held: bool,
}

#[derive(Default)]
struct DyingState {
    dead: bool,
    first_answered: bool,
    release: bool,
}

impl Dying {
    fn new(held: bool) -> Arc<Dying> {
        Arc::new(Dying { state: Mutex::new(DyingState::default()), held })
    }

    fn wait_for(&self, what: &str, f: impl Fn(&DyingState) -> bool) {
        let limit = Instant::now() + Duration::from_secs(10);
        while !f(&self.state.lock().unwrap()) {
            assert!(Instant::now() < limit, "gave up waiting for {what}");
            std::thread::yield_now();
        }
    }
}

impl Runtime for Dying {
    fn recover(&self, _run_id: &str) -> Recovery {
        Recovery::Gone
    }
    fn knows_runbook(&self, runbook: &str, version: Option<&str>) -> bool {
        runbook == "book" && version.is_none()
    }
    fn runbooks(&self) -> Vec<RunbookInfo> {
        vec![RunbookInfo { name: "book".into(), version: None }]
    }
    fn kind(&self) -> &'static str {
        "stub"
    }
    fn start(&self, request: StartRequest, sink: &dyn EventSink, _cancel: &AtomicBool) -> Result<Finish, RuntimeFailure> {
        *self.state.lock().unwrap() = DyingState::default();
        sink.started();
        for which in ["a", "b"] {
            sink.decision_requested(DecisionRequest {
                request_id: format!("{}-{which}", request.run_id),
                case_id: "c".into(),
                action: "act".into(),
                capability: "approve".into(),
                question: "q".into(),
                options: vec![],
            });
        }
        self.wait_for("the first answer", |s| s.first_answered);
        // The runtime found its copy changed on the first answer: it fails.
        self.state.lock().unwrap().dead = true;
        if self.held {
            self.wait_for("the release", |s| s.release);
        }
        Err(RuntimeFailure { reason: "RunbookCopyChanged: the copy changed".into() })
    }
    fn answer(&self, _run_id: &str, request_id: &str, _answer: &str, _principal: &str) -> Result<(), AnswerFailure> {
        let mut st = self.state.lock().unwrap();
        if st.dead {
            return Err(AnswerFailure::UnknownRun);
        }
        if request_id.ends_with("-a") {
            st.first_answered = true;
        }
        Ok(())
    }
    fn cancel(&self, _run_id: &str) {}
}

fn dying_daemon(tag: &str, runtime: &Arc<Dying>) -> Daemon {
    let dir = fresh(tag);
    let cfg = Config { state_dir: dir.join("state"), confinement_floor: Confinement::Local, context_roots: vec![], runbook_dir: None, principal: super::PRINCIPAL.into() };
    Daemon::start(cfg, runtime.clone()).unwrap()
}

/// Submits the run and returns its id once both requests are open.
fn submit_waiting(d: &Daemon) -> String {
    let o = d.submit_run(SubmitRun { runbook_id: "book".into(), intake: "x".into(), confinement: Some(Confinement::Local), ..SubmitRun::default() }).unwrap();
    let run = o.run_id.unwrap();
    let limit = Instant::now() + Duration::from_secs(10);
    while d.open_decisions_of_run(&run).len() < 2 && Instant::now() < limit {
        std::thread::yield_now();
    }
    run
}

fn answer_of(d: &Daemon, run: &str, which: &str) -> runbook::commands::Outcome {
    d.answer_decision(AnswerDecision { run_id: run.into(), request_id: format!("{run}-{which}"), answer: "approve".into(), answered_by: "x".into() }).unwrap()
}

/// 60 runs of the race: the client answers the first request while a second client answers the
/// second one at a chosen microsecond. Returns what is wrong with any run: it did not end once,
/// with `runtime-failure` and nothing open, or an answer is neither a refusal nor an acceptance.
fn race(tag: &str) -> Vec<String> {
    let runtime = Dying::new(false);
    let d = dying_daemon(tag, &runtime);
    let mut problems = Vec::new();
    for i in 0..60u64 {
        let run = submit_waiting(&d);
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let second = {
            let (d, run, barrier) = (d.clone(), run.clone(), barrier.clone());
            std::thread::spawn(move || {
                barrier.wait();
                let until = Instant::now() + Duration::from_micros(i * 30);
                while Instant::now() < until {
                    std::hint::spin_loop();
                }
                answer_of(&d, &run, "b")
            })
        };
        barrier.wait();
        let first = answer_of(&d, &run, "a");
        let second = second.join().unwrap();
        let limit = Instant::now() + Duration::from_secs(10);
        while d.run_detail(&run)[0]["state"] != "Ended" && Instant::now() < limit {
            std::thread::yield_now();
        }
        let row = d.run_detail(&run)[0].clone();
        if row["state"] != "Ended" || row["outcome"] != "runtime-failure" {
            problems.push(format!("run {i}: {row}"));
        }
        if !d.open_decisions_of_run(&run).is_empty() {
            problems.push(format!("run {i}: a request is still open"));
        }
        let ended = d.events_after(&run, 0).iter().filter(|e| e["kind"] == "runbook.runtime-failed").count();
        if ended != 1 {
            problems.push(format!("run {i}: {ended} runtime-failed events"));
        }
        for (what, o) in [("first", &first), ("second", &second)] {
            if !o.is_refusal() && o.name != "answered" {
                problems.push(format!("run {i}: {what} answer: {o:?}"));
            }
        }
    }
    d.stop();
    problems
}

#[test]
fn a_run_the_runtime_fails_on_while_it_waits_ends_once_with_nothing_open() {
    let problems = race("race-guard");
    assert!(problems.is_empty(), "{problems:?}");
}

#[test]
fn an_answer_after_the_runtime_has_failed_is_refused_and_leaves_no_record_of_an_acceptance() {
    let runtime = Dying::new(true);
    let d = dying_daemon("answer-late", &runtime);
    let run = submit_waiting(&d);
    let first = answer_of(&d, &run, "a");
    assert_eq!(first.name, "answered", "{first:?}");
    runtime.wait_for("the runtime to fail", |s| s.dead);
    // The runtime has failed and has not told the daemon yet.
    let late = answer_of(&d, &run, "b");
    assert!(late.is_refusal() && late.name == "run-not-waiting", "an answer after the runtime failed was taken: {late:?}");
    runtime.state.lock().unwrap().release = true;
    let limit = Instant::now() + Duration::from_secs(10);
    while d.run_detail(&run)[0]["state"] != "Ended" && Instant::now() < limit {
        std::thread::yield_now();
    }
    let events = d.events_after(&run, 0);
    let kinds: Vec<&str> = events.iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds.iter().filter(|k| **k == "runbook.answer-undeliverable").count(), 0, "{kinds:?}");
    assert_eq!(kinds.iter().filter(|k| **k == "runbook.runtime-failed").count(), 1, "{kinds:?}");
    assert!(d.open_decisions_of_run(&run).is_empty(), "a request is still open");
    assert_eq!(d.run_detail(&run)[0]["outcome"], "runtime-failure");
    d.stop();
}

// ---- the run's directory ----

#[test]
fn a_run_directory_and_its_copy_are_private_to_the_daemons_user() {
    let rig = Real::new("private", Confinement::Local);
    let run = rig.submit(RUNBOOK, None, Confinement::Local).run_id.unwrap();
    let run_dir = rig.state().join("runs").join(&run);
    rig.until("the pin file", 40, |_| run_dir.join(PIN_FILE).is_file());
    assert_eq!(std::fs::metadata(&run_dir).unwrap().permissions().mode() & 0o7777, 0o700, "the run's directory");
    let copy = run_dir.join(COPY_DIR);
    assert_eq!(std::fs::metadata(&copy).unwrap().permissions().mode() & 0o7777, 0o700, "the copy");
    for (rel, what) in dump(&copy) {
        let mode = what.split(' ').nth(1).unwrap();
        let mode = u32::from_str_radix(mode, 8).unwrap();
        if !what.starts_with("L ") {
            assert_eq!(mode & 0o077, 0, "{rel}: {mode:o} is open to group or others");
        }
    }
    rig.settle(&run);
    assert_eq!(rig.row(&run)["outcome"], "delivered", "{}", rig.row(&run));
}
