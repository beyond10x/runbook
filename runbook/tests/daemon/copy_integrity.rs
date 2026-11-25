//! The integrity of the run's copy of the runbook (`docs/engine.md`, "The run's copy of the runbook"):
//! what the copy takes and refuses, what its digest covers, what a resumed run's pin file may be, the
//! modes and bounds of the copy, and a run the runtime fails on while clients answer.

use super::state::scratch;
use runbook::commands::{AnswerDecision, CancelRun, Config, Daemon, SubmitRun};
use runbook::engine::copy::{copy_dirs, digest, Links, MAX_BYTES, MAX_ENTRIES};
use runbook::engine::pin::{Pin, COPY_DIR};
use runbook::engine::Engine;
use runbook::runtime::{AnswerFailure, Confinement, DecisionRequest, EventSink, Finish, Recovery, Runtime, RuntimeFailure, RunbookInfo, StartRequest};
use serde_json::Value;
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
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

fn load(run: &Path) -> Result<Option<Pin>, String> {
    let dir = run.to_path_buf();
    bounded(5, move || Pin::load(&dir)).expect("Pin::load did not return within 5 s")
}

fn must_end_as_changed(run: &Path, what: &str, bad: &mut Vec<String>) {
    match load(run) {
        Err(e) if e.starts_with("RunbookCopyChanged") && !e.contains(run.to_str().unwrap()) => {}
        other => bad.push(format!("{what}: {:?}", other.map(|p| p.map(|p| p.runbook)))),
    }
}

fn variant(n: &mut u32, what: &str, bad: &mut Vec<String>, edit: impl FnOnce(&Path, &Pin, &str)) {
    *n += 1;
    let (_home, run, pin) = taken(&format!("pm{n}"));
    let good = std::fs::read_to_string(run.join(PIN_FILE)).unwrap();
    edit(&run, &pin, &good);
    must_end_as_changed(&run, what, bad);
}

#[test]
fn pin_load_ends_every_run_whose_pin_is_not_what_the_submit_wrote() {
    let mut bad = Vec::new();
    let mut n = 0u32;
    let b = &mut bad;
    variant(&mut n, "pin file stands, the copy is gone", b, |_, pin, _| std::fs::remove_dir_all(&pin.copy).unwrap());
    variant(&mut n, "pin file stands, the copy is a regular file", b, |_, pin, _| {
        std::fs::remove_dir_all(&pin.copy).unwrap();
        std::fs::write(&pin.copy, "x").unwrap();
    });
    variant(&mut n, "pin file stands, the copy is a dangling link", b, |_, pin, _| {
        std::fs::remove_dir_all(&pin.copy).unwrap();
        std::os::unix::fs::symlink("nowhere", &pin.copy).unwrap();
    });
    variant(&mut n, "pin file gone, the copy is a dangling link", b, |run, pin, _| {
        std::fs::remove_file(run.join(PIN_FILE)).unwrap();
        std::fs::remove_dir_all(&pin.copy).unwrap();
        std::os::unix::fs::symlink("nowhere", &pin.copy).unwrap();
    });
    variant(&mut n, "no copy, the pin file is a dangling link", b, |run, pin, _| {
        std::fs::remove_dir_all(&pin.copy).unwrap();
        std::fs::remove_file(run.join(PIN_FILE)).unwrap();
        std::os::unix::fs::symlink("nowhere", run.join(PIN_FILE)).unwrap();
    });
    variant(&mut n, "the pin file is a link to a regular file in the run", b, |run, _, _| {
        std::fs::rename(run.join(PIN_FILE), run.join("real.json")).unwrap();
        std::os::unix::fs::symlink("real.json", run.join(PIN_FILE)).unwrap();
    });
    variant(&mut n, "the pin file cannot be read (mode 000)", b, |run, _, _| {
        std::fs::set_permissions(run.join(PIN_FILE), std::fs::Permissions::from_mode(0o000)).unwrap();
    });
    variant(&mut n, "an empty pin file", b, |run, _, _| std::fs::write(run.join(PIN_FILE), "").unwrap());
    variant(&mut n, "a pin file cut in half", b, |run, _, good| std::fs::write(run.join(PIN_FILE), &good[..good.len() / 2]).unwrap());
    variant(&mut n, "a pin file of exactly 64 KiB of garbage", b, |run, _, _| std::fs::write(run.join(PIN_FILE), vec![b'x'; 65536]).unwrap());
    variant(&mut n, "a pin file of 64 KiB and one byte of garbage", b, |run, _, _| std::fs::write(run.join(PIN_FILE), vec![b'x'; 65537]).unwrap());
    variant(&mut n, "a valid pin file padded to 64 KiB and one byte", b, |run, _, good| {
        let mut text = good.to_string();
        while text.len() < 65537 {
            text.push(' ');
        }
        std::fs::write(run.join(PIN_FILE), text).unwrap();
    });
    variant(&mut n, "a valid pin file with a NUL after it", b, |run, _, good| std::fs::write(run.join(PIN_FILE), format!("{good}\0")).unwrap());
    for shape in ["[]", "null", "\"x\"", "123", "{}", "true", "[{\"runbook\":\"demo\"}]"] {
        variant(&mut n, &format!("valid JSON of the wrong shape {shape}"), b, |run, _, _| std::fs::write(run.join(PIN_FILE), shape).unwrap());
    }
    for key in ["runbook", "protocol", "revision", "bindings_sha256", "copy_sha256"] {
        variant(&mut n, &format!("member {key} removed"), b, |run, _, good| {
            let mut doc: Value = serde_json::from_str(good).unwrap();
            doc.as_object_mut().unwrap().remove(key);
            std::fs::write(run.join(PIN_FILE), doc.to_string()).unwrap();
        });
    }
    for (key, value) in [
        ("runbook", Value::from(1)),
        ("protocol", Value::Null),
        ("revision", Value::from("3")),
        ("revision", Value::from(3.5)),
        ("bindings_sha256", Value::from(7)),
        ("copy_sha256", Value::from(1)),
        ("copy_sha256", Value::from(vec!["x"])),
    ] {
        variant(&mut n, &format!("member {key} of the wrong type {value}"), b, |run, _, good| {
            let mut doc: Value = serde_json::from_str(good).unwrap();
            doc[key] = value;
            std::fs::write(run.join(PIN_FILE), doc.to_string()).unwrap();
        });
    }
    variant(&mut n, "the digest in upper case", b, |run, pin, good| {
        let upper = pin.copy_sha256.to_uppercase();
        assert_ne!(upper, pin.copy_sha256, "the digest has no letter");
        std::fs::write(run.join(PIN_FILE), good.replace(&pin.copy_sha256, &upper)).unwrap();
    });
    variant(&mut n, "the digest with a space after it", b, |run, pin, good| {
        std::fs::write(run.join(PIN_FILE), good.replace(&pin.copy_sha256, &format!("{} ", pin.copy_sha256))).unwrap();
    });
    variant(&mut n, "the digest of the bindings given for the copy", b, |run, pin, good| {
        std::fs::write(run.join(PIN_FILE), good.replace(&pin.copy_sha256, &pin.bindings_sha256)).unwrap();
    });
    assert!(bad.is_empty(), "{} variants went on or were not refused as a changed copy:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn pin_load_runs_before_the_unit_and_a_missing_directory_are_no_pin_and_the_size_bound_is_exact() {
    let run = fresh("pm-before");
    assert_eq!(Pin::load(&run), Ok(None));
    assert_eq!(Pin::load(&run.join("not-there")), Ok(None));
    std::fs::create_dir(run.join("tree")).unwrap();
    std::fs::write(run.join("tree/manifest"), "x").unwrap();
    assert_eq!(Pin::load(&run), Ok(None));
    // A valid pin file padded to exactly 64 KiB is read; one byte more is refused (the bound is the
    // length, inclusive).
    let (_home, run, _pin) = taken("pm-size");
    let good = std::fs::read_to_string(run.join(PIN_FILE)).unwrap();
    let mut text = good.clone();
    while text.len() < 65536 {
        text.push(' ');
    }
    std::fs::write(run.join(PIN_FILE), &text).unwrap();
    assert!(load(&run).is_ok_and(|p| p.is_some()), "a valid pin file of exactly 65536 bytes was refused");
    text.push(' ');
    std::fs::write(run.join(PIN_FILE), &text).unwrap();
    assert!(load(&run).is_err(), "a pin file of 65537 bytes was read");
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
    // What this case holds is a count, not a time: a copy of MAX_ENTRIES entries is made whole and a
    // copy of one more is refused before its first write, naming no path. Every entry here is a
    // directory or a regular file, so the copy has nothing to wait on, and it is called without the
    // 5 s bound of `within`, whose subject (a copy never waits on an entry that blocks) the cases
    // with a pipe or a socket hold; under that bound this case wrote and copied 8,000 files and
    // missed it at a build node load of 60.
    assert_eq!(MAX_ENTRIES, 4096);
    // `bindings/`, `bindings/tools/`, `demo.yaml` and `tools/run.sh` are 4 entries.
    let home = demo_home("entries");
    let tools = home.join("bindings/tools");
    for n in 0..4092 {
        std::fs::write(tools.join(format!("f{n:04}.txt")), "x\n").unwrap();
    }
    let copy = |tag: &str| {
        let dst = fresh(&format!("{tag}-dst")).join("bindings");
        (copy_dirs(&[(&home.join("bindings"), &dst)], Links::Within(&home)), dst)
    };
    let (copied, dst) = copy("entries-at");
    copied.unwrap();
    assert_eq!(count_files(&dst), 4094);
    std::fs::write(tools.join("one-more.txt"), "x\n").unwrap();
    let (copied, dst) = copy("entries-over");
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

#[test]
fn a_job_directory_that_is_itself_a_link_out_of_the_home_is_refused_and_nothing_is_written() {
    for (n, via_dir_link) in [false, true].into_iter().enumerate() {
        let home = demo_home(&format!("toplink-{n}"));
        let outside = fresh(&format!("toplink-{n}-outside"));
        std::fs::create_dir_all(outside.join("d")).unwrap();
        std::fs::write(outside.join("d/victim.txt"), SECRET).unwrap();
        std::fs::remove_dir_all(home.join("bindings")).unwrap();
        let target = if via_dir_link { outside.join("d") } else { outside.clone() };
        std::os::unix::fs::symlink(&target, home.join("bindings")).unwrap();
        let out = fresh(&format!("toplink-{n}-out"));
        let (h, src, dst) = (home.clone(), home.join("bindings"), out.join("bindings"));
        let d2 = dst.clone();
        let made = bounded(20, move || copy_dirs(&[(&src, &d2)], Links::Within(&h))).expect("copy_dirs did not return");
        let reason = made.expect_err("a job directory that links out of the home was copied");
        assert!(!dst.exists(), "variant {n}: a destination was written for a refused job");
        assert!(!reason.contains(SECRET), "variant {n}: {reason}");
    }
}

fn job(home: &Path, name: &str, files: usize, size: u64) -> PathBuf {
    let dir = home.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    for n in 0..files {
        let f = std::fs::File::create(dir.join(format!("f{n:05}"))).unwrap();
        if size > 0 {
            f.set_len(size).unwrap();
        }
    }
    dir
}

fn copy_two(home: &Path, tag: &str) -> (Result<(), String>, PathBuf, PathBuf) {
    let out = fresh(&format!("{tag}-out"));
    let (da, db) = (out.join("a"), out.join("b"));
    let (h, a, b, x, y) = (home.to_path_buf(), home.join("a"), home.join("b"), da.clone(), db.clone());
    let made = bounded(30, move || copy_dirs(&[(&a, &x), (&b, &y)], Links::Within(&h))).expect("the copy did not return within 30 s");
    (made, da, db)
}

#[test]
fn the_bounds_count_across_all_jobs_and_a_home_at_a_bound_exactly_is_accepted() {
    let per_job = MAX_ENTRIES / 2 - 1;
    // Entries: two jobs of a directory and `per_job` files are exactly MAX_ENTRIES.
    let home = fresh("bound-entries");
    job(&home, "a", per_job, 0);
    job(&home, "b", per_job, 0);
    let (made, da, db) = copy_two(&home, "bound-entries-at");
    made.unwrap();
    assert!(da.is_dir() && db.is_dir());
    job(&home, "b", per_job + 1, 0);
    let (made, da, db) = copy_two(&home, "bound-entries-over");
    let reason = made.expect_err("MAX_ENTRIES + 1 entries over two jobs were copied");
    assert!(!da.exists() && !db.exists(), "a refused copy wrote a destination");
    assert!(!reason.contains(home.to_str().unwrap()) && !reason.contains("f0"), "the refusal names a path: {reason}");
    // Bytes: the lengths over two jobs, at the bound and one byte over.
    let home = fresh("bound-bytes");
    job(&home, "a", 1, MAX_BYTES / 2);
    job(&home, "b", 1, MAX_BYTES / 2);
    let (made, _, _) = copy_two(&home, "bound-bytes-at");
    made.unwrap();
    std::fs::File::create(home.join("b/f00000")).unwrap().set_len(MAX_BYTES / 2 + 1).unwrap();
    let (made, da, db) = copy_two(&home, "bound-bytes-over");
    let reason = made.expect_err("MAX_BYTES + 1 bytes over two jobs were copied");
    assert!(!da.exists() && !db.exists(), "a refused copy wrote a destination");
    assert!(!reason.contains(home.to_str().unwrap()) && !reason.contains("f0"), "the refusal names a path: {reason}");
    // One file of exactly MAX_BYTES.
    let home = fresh("bound-bytes-one");
    job(&home, "a", 1, MAX_BYTES);
    job(&home, "b", 0, 0);
    let (made, _, _) = copy_two(&home, "bound-bytes-one-at");
    made.unwrap();
}

#[test]
fn a_directory_nested_exactly_24_levels_is_copied_and_25_is_refused_naming_no_outside_path() {
    let nest = |levels: usize, tag: &str| {
        let home = demo_home(tag);
        let mut deep = home.join("bindings");
        for _ in 0..levels {
            deep = deep.join("d");
        }
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(deep.join("f.txt"), "x\n").unwrap();
        let dst = fresh(&format!("{tag}-dst")).join("bindings");
        let (h, d) = (home.clone(), dst.clone());
        let made = bounded(30, move || copy_dirs(&[(&h.join("bindings"), &d)], Links::Within(&h))).unwrap();
        (made, dst)
    };
    let (made, _) = nest(23, "depth-23");
    made.unwrap();
    let (made, _) = nest(24, "depth-24");
    assert!(made.is_ok(), "a directory nested 24 levels below bindings/ was refused: {made:?}");
    let (made25, dst25) = nest(25, "depth-25");
    assert!(made25.is_err() && !dst25.exists(), "a directory nested 25 levels was copied");
    let reason = made25.unwrap_err();
    assert!(!reason.contains("depth-25"), "the refusal names a path of the home: {reason}");
}

fn swap_attempt(variant: &str, attempt: u32) -> (Result<(), String>, bool, PathBuf, PathBuf) {
    let tag = format!("swap-{variant}-{attempt}");
    let home = fresh(&tag);
    let outside = fresh(&format!("{tag}-outside"));
    std::fs::write(outside.join("secret.txt"), SECRET).unwrap();
    let a = home.join("a");
    std::fs::create_dir_all(&a).unwrap();
    for n in 0..3000 {
        std::fs::write(a.join(format!("f{n:04}.txt")), "x\n").unwrap();
    }
    let b = home.join("b");
    std::fs::create_dir_all(&b).unwrap();
    std::fs::write(b.join("victim.txt"), "inside\n").unwrap();
    let out = fresh(&format!("{tag}-out"));
    let (da, db) = (out.join("a"), out.join("b"));
    let done = Arc::new(AtomicBool::new(false));
    let writer = {
        let (da, home, outside, done, variant) = (da.clone(), home.clone(), outside.clone(), done.clone(), variant.to_string());
        std::thread::spawn(move || {
            let limit = Instant::now() + Duration::from_secs(20);
            while !da.exists() && !done.load(std::sync::atomic::Ordering::SeqCst) && Instant::now() < limit {
                std::hint::spin_loop();
            }
            let victim = home.join("b/victim.txt");
            let replace = |text: &str| {
                std::fs::write(home.join("b/new.tmp"), text).unwrap();
                std::fs::rename(home.join("b/new.tmp"), &victim).unwrap();
            };
            match variant.as_str() {
                "link" => {
                    std::fs::remove_file(&victim).unwrap();
                    std::os::unix::fs::symlink(outside.join("secret.txt"), &victim).unwrap();
                }
                "longer" => replace("inside, and longer\n"),
                "shorter" => replace("in\n"),
                "same-length" => replace("INSIDE\n"),
                "append" => {
                    use std::io::Write;
                    std::fs::OpenOptions::new().append(true).open(&victim).unwrap().write_all(b"more\n").unwrap();
                }
                "truncate" => std::fs::write(&victim, "").unwrap(),
                "dirlink" => {
                    std::fs::create_dir_all(outside.join("dir")).unwrap();
                    std::fs::write(outside.join("dir/victim.txt"), SECRET).unwrap();
                    std::fs::rename(home.join("b"), home.join("b.orig")).unwrap();
                    std::os::unix::fs::symlink(outside.join("dir"), home.join("b")).unwrap();
                }
                other => panic!("{other}"),
            }
        })
    };
    let (h, x, y, a2, b2) = (home.clone(), da.clone(), db.clone(), a.clone(), b.clone());
    let made = bounded(60, move || copy_dirs(&[(&a2, &x), (&b2, &y)], Links::Within(&h))).expect("the copy did not return within 60 s");
    done.store(true, std::sync::atomic::Ordering::SeqCst);
    writer.join().unwrap();
    let leaked = std::fs::read_to_string(db.join("victim.txt")).is_ok_and(|t| t.contains(SECRET));
    (made, leaked, da, db)
}

#[test]
fn a_source_that_is_not_the_file_the_plan_saw_refuses_the_whole_copy_and_leaves_nothing() {
    let mut bad = Vec::new();
    for variant in ["link", "longer", "shorter", "same-length", "append", "truncate", "dirlink"] {
        let mut landed = None;
        for attempt in 0..8 {
            let (made, leaked, da, db) = swap_attempt(variant, attempt);
            if leaked {
                bad.push(format!("{variant}: a file outside the home is in the copy (attempt {attempt})"));
                landed = Some(attempt);
                break;
            }
            if made.is_err() {
                if da.exists() || db.exists() {
                    bad.push(format!("{variant}: the copy refused and left a destination (attempt {attempt})"));
                }
                landed = Some(attempt);
                break;
            }
        }
        match landed {
            Some(_) => {}
            None => bad.push(format!("{variant}: the swap never landed in 8 attempts (inconclusive)")),
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
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

#[test]
fn keep_copies_more_entries_and_more_bytes_than_the_daemons_copy_takes_as_the_base_did() {
    // `docs/engine.md`: `runbook run` has no bound of entries, of bytes or of depth. One entry over the
    // bound of entries and a sparse file one byte over the bound of bytes are copied as they are.
    let src = fresh("keep-bounds-src");
    for n in 0..=MAX_ENTRIES {
        std::fs::write(src.join(format!("f{n:05}.txt")), "x\n").unwrap();
    }
    std::fs::File::create(src.join("big.bin")).unwrap().set_len(MAX_BYTES + 1).unwrap();
    let head = fresh("keep-bounds-head").join("x");
    let copied = copy_dirs(&[(&src, &head)], Links::Keep);
    assert!(copied.is_ok(), "`runbook run` copied this directory before the unit and now: {copied:?}");
    assert_eq!(count_files(&head), MAX_ENTRIES + 2);
    assert_eq!(std::fs::metadata(head.join("big.bin")).unwrap().len(), MAX_BYTES + 1);
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
        Real::with(tag, floor, |engine| engine)
    }

    /// The daemon runs `wrap(engine)` as its runtime.
    fn with(tag: &str, floor: Confinement, wrap: impl FnOnce(Arc<Engine>) -> Arc<dyn Runtime>) -> Real {
        let dir = fresh(tag);
        let home = dir.join("home");
        copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &home);
        let binding = home.join("bindings/engine-draft.yaml");
        let text = std::fs::read_to_string(&binding).unwrap().replace("principal: reviewer", &format!("principal: {}", super::PRINCIPAL));
        std::fs::write(&binding, text).unwrap();
        std::fs::create_dir_all(dir.join("ctx")).unwrap();
        let engine = Arc::new(Engine::with_levels(home.clone(), dir.join("state"), &[Confinement::Local, Confinement::Linux], crate::daemon::policy()));
        let cfg = Config { state_dir: dir.join("state"), confinement_floor: floor, context_roots: vec![dir.join("ctx")], runbook_dir: None, principal: super::PRINCIPAL.into() };
        Real { dir, home, d: Daemon::start(cfg, wrap(engine)).unwrap() }
    }

    fn state(&self) -> PathBuf {
        self.dir.join("state")
    }

    fn submit(&self, id: &str, version: Option<&str>, level: Confinement) -> runbook::commands::Outcome {
        self.d.submit_run(SubmitRun { runbook_id: id.into(), runbook_version: version.map(String::from), intake: "a note on tea".into(), confinement: Some(level), ..SubmitRun::default() }).unwrap()
    }

    fn submit_intake(&self, intake: &str) -> runbook::commands::Outcome {
        self.d.submit_run(SubmitRun { runbook_id: RUNBOOK.into(), intake: intake.into(), confinement: Some(Confinement::Local), ..SubmitRun::default() }).unwrap()
    }

    fn submit_demo(&self) -> runbook::commands::Outcome {
        self.submit_intake("a note on tea")
    }

    fn state_of(&self, run: &str) -> String {
        self.d.run_detail(run)[0]["state"].as_str().unwrap().to_string()
    }

    fn row(&self, run: &str) -> Value {
        self.d.run_detail(run)[0].clone()
    }

    fn over(&self, run: &str) -> bool {
        matches!(self.state_of(run).as_str(), "Ended" | "Cancelled" | "FailedToStart")
    }

    fn seen(&self, run: &str) -> String {
        format!("{} {:?} {:?}", self.row(run), self.d.events_after(run, 0), self.d.decisions_of_run(run))
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
fn a_refused_submit_leaves_an_empty_run_directory_and_touches_nothing_else() {
    let rig = Real::new("refused-submit", Confinement::Local);
    let first = rig.submit_demo().run_id.unwrap();
    let first_dir = rig.state().join("runs").join(&first);
    rig.until("the first run's pin file", 40, |_| first_dir.join(PIN_FILE).is_file());
    let pin_before = std::fs::read(first_dir.join(PIN_FILE)).unwrap();
    std::fs::write(rig.state().join("runs/decoy.txt"), "decoy").unwrap();
    let mut bad = Vec::new();
    let tools = rig.home.join("bindings/tools");
    // A file the plan sees and the write cannot open: the refusal comes in the write phase, after the
    // first directory is made.
    std::fs::write(tools.join("locked.sh"), "x").unwrap();
    std::fs::set_permissions(tools.join("locked.sh"), std::fs::Permissions::from_mode(0o000)).unwrap();
    let locked = rig.submit_demo().run_id.unwrap();
    rig.until("the locked run to be over", 40, |me| me.over(&locked));
    std::fs::remove_file(tools.join("locked.sh")).unwrap();
    // A home over the bound of entries.
    for n in 0..(MAX_ENTRIES + 10) {
        std::fs::write(tools.join(format!("f{n:05}.txt")), "x\n").unwrap();
    }
    let over = rig.submit_demo().run_id.unwrap();
    rig.until("the over-bound run to be over", 40, |me| me.over(&over));
    for (what, run) in [("write phase", &locked), ("bound", &over)] {
        let state = rig.state_of(run);
        if state != "FailedToStart" {
            bad.push(format!("{what}: the run is {state}"));
        }
        let dir = rig.state().join("runs").join(run);
        let left: Vec<String> = std::fs::read_dir(&dir).map(|d| d.map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
        if !left.is_empty() {
            bad.push(format!("{what}: the run directory holds {left:?}"));
        }
        let seen = rig.seen(run);
        if seen.contains(rig.state().to_str().unwrap()) || seen.contains(rig.home.to_str().unwrap()) || seen.contains("locked.sh") || seen.contains("f0000") {
            bad.push(format!("{what}: the record names a path: {seen}"));
        }
    }
    // The first run's copy, its pin file and the decoy are as they were.
    assert_eq!(std::fs::read(first_dir.join(PIN_FILE)).unwrap(), pin_before);
    assert!(first_dir.join(COPY_DIR).join("bindings/engine-demo.yaml").is_file());
    assert_eq!(std::fs::read_to_string(rig.state().join("runs/decoy.txt")).unwrap(), "decoy");
    let _ = rig.d.cancel_run(CancelRun { run_id: first.clone() });
    rig.until("the first run to end", 40, |me| me.over(&first));
    assert!(bad.is_empty(), "{bad:?}");
}

#[test]
fn a_refusal_or_a_failure_at_the_start_names_no_path_of_the_state_directory_or_the_home() {
    let kinds = ["link-out", "root-bindings-not-yaml", "root-bindings-not-yaml-no-intake", "protocol-not-yaml", "unreadable-tool", "over-the-bound", "too-deep", "child-bindings-missing"];
    let mut bad = Vec::new();
    for (n, kind) in kinds.iter().enumerate() {
        let rig = Real::new(&format!("names-{n}"), Confinement::Local);
        let outside = fresh(&format!("names-{n}-outside"));
        std::fs::write(outside.join("secret.txt"), SECRET).unwrap();
        let tools = rig.home.join("bindings/tools");
        let mut intake = "a note on tea";
        match *kind {
            "link-out" => std::os::unix::fs::symlink(outside.join("secret.txt"), tools.join("leak.sh")).unwrap(),
            "root-bindings-not-yaml" => std::fs::write(rig.home.join("bindings/engine-demo.yaml"), "a: [unclosed\n").unwrap(),
            "root-bindings-not-yaml-no-intake" => {
                std::fs::write(rig.home.join("bindings/engine-demo.yaml"), "a: [unclosed\n").unwrap();
                intake = "";
            }
            "protocol-not-yaml" => std::fs::write(rig.home.join("protocol/zz.protocol.yaml"), "protocol: [unclosed\n").unwrap(),
            "unreadable-tool" => {
                std::fs::write(tools.join("locked.sh"), "x").unwrap();
                std::fs::set_permissions(tools.join("locked.sh"), std::fs::Permissions::from_mode(0o000)).unwrap();
            }
            "over-the-bound" => {
                for i in 0..(MAX_ENTRIES + 10) {
                    std::fs::write(tools.join(format!("f{i:05}.txt")), "x\n").unwrap();
                }
            }
            "too-deep" => {
                let mut deep = tools.clone();
                for _ in 0..30 {
                    deep = deep.join("d");
                }
                std::fs::create_dir_all(&deep).unwrap();
            }
            "child-bindings-missing" => std::fs::remove_file(rig.home.join("bindings/engine-draft.yaml")).unwrap(),
            other => panic!("{other}"),
        }
        let o = rig.submit_intake(intake);
        let seen = if o.is_refusal() {
            format!("{o:?}")
        } else {
            let run = o.run_id.clone().unwrap();
            rig.until("the run to be over", 40, |me| me.over(&run));
            rig.seen(&run)
        };
        let reason = seen.find("reason").map(|i| seen[i..(i + 260).min(seen.len())].to_string()).unwrap_or_else(|| seen.chars().take(260).collect());
        if seen.contains(rig.state().to_str().unwrap()) {
            bad.push(format!("{kind}: names a path of the state directory: {reason}"));
        } else if seen.contains(rig.home.to_str().unwrap()) {
            bad.push(format!("{kind}: names a path of the home: {reason}"));
        }
        assert!(!seen.contains(SECRET) && !seen.contains(outside.to_str().unwrap()), "{kind}: names something outside: {seen}");
    }
    assert!(bad.is_empty(), "{} of {} refusals name a path:\n{}", bad.len(), kinds.len(), bad.join("\n"));
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
/// and before the daemon heard of it. A healthy one takes both answers and delivers; `delivered` lists
/// the requests whose answer the runtime took.
struct Dying {
    state: Mutex<DyingState>,
    held: bool,
    fails: bool,
}

#[derive(Default)]
struct DyingState {
    dead: bool,
    first_answered: bool,
    release: bool,
    delivered: Vec<String>,
}

impl Dying {
    fn new(held: bool) -> Arc<Dying> {
        Arc::new(Dying { state: Mutex::new(DyingState::default()), held, fails: true })
    }

    fn healthy() -> Arc<Dying> {
        Arc::new(Dying { state: Mutex::new(DyingState::default()), held: false, fails: false })
    }

    fn delivered(&self) -> Vec<String> {
        self.state.lock().unwrap().delivered.clone()
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
                artifact: None,
            });
        }
        if !self.fails {
            self.wait_for("both answers", |s| s.delivered.len() >= 2);
            sink.resumed();
            return Ok(Finish::Outcome("delivered".into()));
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
        st.delivered.push(request_id.to_string());
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

fn answer_by(d: &Daemon, run: &str, which: &str, who: &str) -> runbook::commands::Outcome {
    d.answer_decision(AnswerDecision { run_id: run.into(), request_id: format!("{run}-{which}"), answer: "approve".into(), answered_by: who.into() }).unwrap()
}

fn ended(d: &Daemon, run: &str) {
    let limit = Instant::now() + Duration::from_secs(10);
    while d.run_detail(run)[0]["state"] != "Ended" && Instant::now() < limit {
        std::thread::yield_now();
    }
}

fn decision(d: &Daemon, run: &str, which: &str) -> Value {
    d.decisions_of_run(run).into_iter().find(|x| x["request_id"] == format!("{run}-{which}")).unwrap()
}

#[test]
fn a_late_answer_is_refused_whoever_sends_it_and_leaves_no_acceptance_and_no_record() {
    let runtime = Dying::new(true);
    let d = dying_daemon("late", &runtime);
    let run = submit_waiting(&d);
    assert_eq!(answer_by(&d, &run, "a", "alice").name, "answered");
    runtime.wait_for("the runtime to fail", |s| s.dead);
    for who in ["bob", "alice", "agent:test-daemon"] {
        let late = answer_by(&d, &run, "b", who);
        assert!(late.is_refusal() && late.name == "run-not-waiting", "{who}: {late:?}");
    }
    let again_a = answer_by(&d, &run, "a", "bob");
    assert!(again_a.is_refusal() && again_a.name == "already-answered", "{again_a:?}");
    let unknown = d.answer_decision(AnswerDecision { run_id: run.clone(), request_id: "nope".into(), answer: "approve".into(), answered_by: "x".into() }).unwrap();
    assert!(unknown.is_refusal() && unknown.name == "no-request", "{unknown:?}");
    let b = decision(&d, &run, "b");
    assert_eq!(b["state"], "Open", "{b}");
    assert!(b.get("answer").is_none() && b.get("answered_by").is_none() && b.get("principal").is_none(), "an acceptance is on the record: {b}");
    runtime.state.lock().unwrap().release = true;
    ended(&d, &run);
    assert_eq!(d.run_detail(&run)[0]["outcome"], "runtime-failure");
    let b = decision(&d, &run, "b");
    assert_eq!(b["state"], "Withdrawn", "{b}");
    assert!(b.get("answer").is_none() && b.get("principal").is_none(), "an acceptance is on the record: {b}");
    let kinds: Vec<String> = d.events_after(&run, 0).iter().map(|e| e["kind"].as_str().unwrap().to_string()).collect();
    assert_eq!(kinds.iter().filter(|k| *k == "runbook.runtime-failed").count(), 1, "{kinds:?}");
    assert_eq!(kinds.iter().filter(|k| *k == "runbook.answer-undeliverable").count(), 0, "{kinds:?}");
    assert!(d.open_decisions_of_run(&run).is_empty());
    assert_eq!(runtime.delivered(), vec![format!("{run}-a")]);
    let after = answer_by(&d, &run, "b", "bob");
    assert!(after.is_refusal() && after.name == "withdrawn", "an answer after the end: {after:?}");
    let after_a = answer_by(&d, &run, "a", "bob");
    assert!(after_a.is_refusal() && after_a.name == "already-answered", "{after_a:?}");
    d.stop();
}

#[test]
fn a_healthy_waiting_run_takes_both_answers_and_a_second_answer_is_refused() {
    let runtime = Dying::healthy();
    let d = dying_daemon("healthy", &runtime);
    let run = submit_waiting(&d);
    let a = answer_by(&d, &run, "a", "alice");
    let again = answer_by(&d, &run, "a", "bob");
    let b = answer_by(&d, &run, "b", "carol");
    assert_eq!((a.name, b.name), ("answered", "answered"), "{a:?} {b:?}");
    assert!(again.is_refusal() && again.name == "already-answered", "{again:?}");
    ended(&d, &run);
    assert_eq!(d.run_detail(&run)[0]["outcome"], "delivered");
    for which in ["a", "b"] {
        let x = decision(&d, &run, which);
        assert_eq!(x["state"], "Answered", "{x}");
        assert_eq!(x["principal"], super::PRINCIPAL, "{x}");
    }
    assert_eq!(runtime.delivered().len(), 2);
    d.stop();
}

#[test]
fn two_hundred_races_accept_no_answer_the_runtime_did_not_take() {
    let runtime = Dying::new(false);
    let d = dying_daemon("race-200", &runtime);
    let mut problems = Vec::new();
    for i in 0..200u64 {
        let run = submit_waiting(&d);
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let second = {
            let (d, run, barrier) = (d.clone(), run.clone(), barrier.clone());
            std::thread::spawn(move || {
                barrier.wait();
                let until = Instant::now() + Duration::from_micros(i * 10);
                while Instant::now() < until {
                    std::hint::spin_loop();
                }
                answer_by(&d, &run, "b", "bob")
            })
        };
        barrier.wait();
        let first = answer_by(&d, &run, "a", "alice");
        let second = second.join().unwrap();
        ended(&d, &run);
        let row = d.run_detail(&run)[0].clone();
        if row["state"] != "Ended" || row["outcome"] != "runtime-failure" {
            problems.push(format!("run {i}: {row}"));
        }
        let events = d.events_after(&run, 0);
        let count = |kind: &str| events.iter().filter(|e| e["kind"] == kind).count();
        if count("runbook.runtime-failed") != 1 || count("runbook.answer-undeliverable") != 0 {
            problems.push(format!("run {i}: {} runtime-failed, {} answer-undeliverable", count("runbook.runtime-failed"), count("runbook.answer-undeliverable")));
        }
        if !d.open_decisions_of_run(&run).is_empty() {
            problems.push(format!("run {i}: a request is still open"));
        }
        for (which, o) in [("a", &first), ("b", &second)] {
            let accepted = o.name == "answered";
            if !accepted && !o.is_refusal() {
                problems.push(format!("run {i}: answer {which}: {o:?}"));
            }
            let state = decision(&d, &run, which)["state"].clone();
            if accepted != (state == "Answered") {
                problems.push(format!("run {i}: answer {which} {} but the decision is {state}", o.name));
            }
        }
    }
    d.stop();
    assert!(problems.is_empty(), "{} problems: {:?}", problems.len(), &problems[..problems.len().min(8)]);
}

#[test]
fn an_answer_the_daemon_cannot_record_is_not_delivered_to_the_run() {
    let runtime = Dying::healthy();
    let dir = fresh("unrecordable");
    let cfg = Config { state_dir: dir.join("state"), confinement_floor: Confinement::Local, context_roots: vec![], runbook_dir: None, principal: super::PRINCIPAL.into() };
    let d = Daemon::start(cfg, runtime.clone()).unwrap();
    let run = submit_waiting(&d);
    let size = std::fs::metadata(dir.join("state/journal.jsonl")).unwrap().len();
    // The journal cannot grow by a line: the file-size limit of the process is a few bytes above it.
    unsafe {
        libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
        let limit = libc::rlimit { rlim_cur: size + 5, rlim_max: libc::RLIM_INFINITY };
        assert_eq!(libc::setrlimit(libc::RLIMIT_FSIZE, &limit), 0);
    }
    let answered = d.answer_decision(AnswerDecision { run_id: run.clone(), request_id: format!("{run}-a"), answer: "approve".into(), answered_by: "alice".into() });
    unsafe {
        let limit = libc::rlimit { rlim_cur: libc::RLIM_INFINITY, rlim_max: libc::RLIM_INFINITY };
        assert_eq!(libc::setrlimit(libc::RLIMIT_FSIZE, &limit), 0);
    }
    assert!(answered.is_err(), "the answer was recorded although the journal could not grow: {answered:?}");
    assert!(runtime.delivered().is_empty(), "the client was told the answer failed, and the run took it: delivered {:?}", runtime.delivered());
    // The journal takes lines again: the same answer is accepted, the run is handed each answer once and
    // ends, and no half-written line of the failed answer stands in the journal.
    let again = answer_by(&d, &run, "a", "alice");
    assert_eq!(again.name, "answered", "{again:?}");
    let second = answer_by(&d, &run, "b", "alice");
    assert_eq!(second.name, "answered", "{second:?}");
    ended(&d, &run);
    assert_eq!(runtime.delivered(), vec![format!("{run}-a"), format!("{run}-b")]);
    d.stop();
    drop(d);
    let cfg = Config { state_dir: dir.join("state"), confinement_floor: Confinement::Local, context_roots: vec![], runbook_dir: None, principal: super::PRINCIPAL.into() };
    let reopened = Daemon::start(cfg, Dying::healthy()).expect("the journal of the daemon reads again");
    assert_eq!(decision(&reopened, &run, "a")["state"], "Answered");
    reopened.stop();
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

struct Umask(libc::mode_t);

impl Umask {
    fn set(mask: libc::mode_t) -> Umask {
        Umask(unsafe { libc::umask(mask) })
    }
}

impl Drop for Umask {
    fn drop(&mut self) {
        unsafe { libc::umask(self.0) };
    }
}

fn modes_of(dir: &Path, out: &mut Vec<(String, bool, u32)>, base: &Path) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let m = std::fs::symlink_metadata(&p).unwrap();
        let rel = p.strip_prefix(base).unwrap().display().to_string();
        assert!(!m.file_type().is_symlink(), "{rel} is a link");
        out.push((rel, m.is_dir(), m.permissions().mode() & 0o7777));
        if m.is_dir() {
            modes_of(&p, out, base);
        }
    }
}

#[test]
fn the_copy_and_the_run_directory_are_private_under_every_umask_and_no_special_bit_survives() {
    let mut bad = Vec::new();
    for (n, mask) in [0o000u32, 0o022, 0o077].into_iter().enumerate() {
        let _umask = Umask::set(mask);
        let rig = Real::new(&format!("umask-{n}"), Confinement::Local);
        let tools = rig.home.join("bindings/tools");
        let sources: [(&str, u32, u32); 6] = [("a.yaml", 0o666, 0o600), ("b.sh", 0o777, 0o700), ("c.sh", 0o2755, 0o700), ("d.yaml", 0o1644, 0o600), ("e.sh", 0o4711, 0o700), ("f.yaml", 0o444, 0o600)];
        for (name, mode, _) in sources {
            std::fs::write(tools.join(name), "x\n").unwrap();
            std::fs::set_permissions(tools.join(name), std::fs::Permissions::from_mode(mode)).unwrap();
        }
        for (name, mode) in [("sub1", 0o2777u32), ("sub2", 0o1777), ("sub3", 0o777)] {
            std::fs::create_dir_all(rig.home.join("bindings").join(name)).unwrap();
            std::fs::set_permissions(rig.home.join("bindings").join(name), std::fs::Permissions::from_mode(mode)).unwrap();
        }
        let run = rig.submit_demo().run_id.unwrap();
        let run_dir = rig.state().join("runs").join(&run);
        rig.until("the pin file", 40, |_| run_dir.join(PIN_FILE).is_file());
        assert!(run_dir.is_dir() && run_dir.join(PIN_FILE).is_file(), "umask {mask:o}: run dir {} pin {}; record: {}", run_dir.is_dir(), run_dir.join(PIN_FILE).is_file(), rig.seen(&run));
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o7777;
        if mode(&run_dir) != 0o700 {
            bad.push(format!("umask {mask:o}: runs/<run> is {:o}", mode(&run_dir)));
        }
        if mode(&run_dir.join(PIN_FILE)) != 0o600 {
            bad.push(format!("umask {mask:o}: the pin file is {:o}", mode(&run_dir.join(PIN_FILE))));
        }
        let copy = run_dir.join(COPY_DIR);
        assert!(copy.is_dir(), "umask {mask:o}: no copy; record: {}", rig.seen(&run));
        let mut all = vec![(String::new(), true, mode(&copy))];
        modes_of(&copy, &mut all, &copy);
        for (rel, is_dir, m) in &all {
            if *is_dir && *m != 0o700 {
                bad.push(format!("umask {mask:o}: directory {rel:?} is {m:o}"));
            }
            if !*is_dir && *m != 0o600 && *m != 0o700 {
                bad.push(format!("umask {mask:o}: file {rel:?} is {m:o}"));
            }
        }
        for (name, _, want) in sources {
            let at = copy.join("bindings/tools").join(name);
            assert!(at.exists(), "umask {mask:o}: {name} missing from the copy; tools holds {:?}; home tools holds {:?}; record: {}", std::fs::read_dir(copy.join("bindings/tools")).map(|r| r.map(|e| e.unwrap().file_name()).collect::<Vec<_>>()), std::fs::read_dir(&tools).map(|r| r.map(|e| e.unwrap().file_name()).collect::<Vec<_>>()), rig.seen(&run));
            let got = mode(&at);
            if got != want {
                bad.push(format!("umask {mask:o}: {name} is {got:o}, not {want:o}"));
            }
        }
        let _ = rig.d.cancel_run(CancelRun { run_id: run.clone() });
        rig.until("the run to end", 40, |me| me.over(&run));
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// The engine as the daemon's runtime, with a hook on the run's log lines: when the root case of the run
/// logs that it terminated (the last line of the last round, read before the final check of the copy),
/// the hook changes the run's copy.
struct ChangesTheCopyAtTheEnd(Arc<Engine>);

struct HookedSink<'a> {
    inner: &'a dyn EventSink,
    copy: PathBuf,
}

impl EventSink for HookedSink<'_> {
    fn steps(&self) -> Option<Arc<dyn runbook::runtime::StartTarget>> {
        self.inner.steps()
    }
    fn started(&self) {
        self.inner.started()
    }
    fn record(&self, kind: &str, record: Value) {
        let last = kind == "Terminated" && record["reason"] == "delivered";
        self.inner.record(kind, record);
        if last {
            std::fs::write(self.copy.join("bindings/tools/added-after-the-last-round.txt"), "x\n").unwrap();
        }
    }
    fn decision_requested(&self, request: DecisionRequest) {
        self.inner.decision_requested(request)
    }
    fn decision_answered(&self, request_id: &str, answer: &str, by: &str, principal: &str) -> bool {
        self.inner.decision_answered(request_id, answer, by, principal)
    }
    fn resumed(&self) {
        self.inner.resumed()
    }
}

impl Runtime for ChangesTheCopyAtTheEnd {
    fn recover(&self, run_id: &str) -> Recovery {
        self.0.recover(run_id)
    }
    fn knows_runbook(&self, runbook: &str, version: Option<&str>) -> bool {
        self.0.knows_runbook(runbook, version)
    }
    fn runbooks(&self) -> Vec<RunbookInfo> {
        self.0.runbooks()
    }
    fn kind(&self) -> &'static str {
        self.0.kind()
    }
    fn start(&self, request: StartRequest, sink: &dyn EventSink, cancel: &AtomicBool) -> Result<Finish, RuntimeFailure> {
        let hooked = HookedSink { inner: sink, copy: request.state_dir.join(COPY_DIR) };
        self.0.start(request, &hooked, cancel)
    }
    fn answer(&self, run_id: &str, request_id: &str, answer: &str, principal: &str) -> Result<(), AnswerFailure> {
        self.0.answer(run_id, request_id, answer, principal)
    }
    fn cancel(&self, run_id: &str) {
        self.0.cancel(run_id)
    }
}

#[test]
fn a_copy_changed_after_the_last_round_ends_the_run_and_is_not_delivered() {
    let rig = Real::with("after-last-round", Confinement::Local, |engine| Arc::new(ChangesTheCopyAtTheEnd(engine)));
    let run = rig.submit(RUNBOOK, None, Confinement::Local).run_id.unwrap();
    rig.settle(&run);
    let row = rig.row(&run);
    assert_eq!(row["outcome"], "runtime-failure", "a run whose copy changed after its last round was delivered: {row}");
    let failed: Vec<Value> = rig.d.events_after(&run, 0).into_iter().filter(|e| e["kind"] == "runbook.runtime-failed").collect();
    assert_eq!(failed.len(), 1, "{failed:?}");
    assert!(failed[0]["record"]["reason"].as_str().is_some_and(|r| r.starts_with("RunbookCopyChanged")), "{failed:?}");
}

struct AnswersAtTheEnd {
    engine: Arc<Engine>,
    daemon: Arc<OnceLock<Daemon>>,
    tried: Arc<Mutex<Vec<(String, String)>>>,
}

struct AnsweringSink<'a> {
    inner: &'a dyn EventSink,
    copy: PathBuf,
    run: String,
    daemon: Arc<OnceLock<Daemon>>,
    tried: Arc<Mutex<Vec<(String, String)>>>,
}

impl EventSink for AnsweringSink<'_> {
    fn steps(&self) -> Option<Arc<dyn runbook::runtime::StartTarget>> {
        self.inner.steps()
    }
    fn started(&self) {
        self.inner.started()
    }
    fn record(&self, kind: &str, record: Value) {
        let last = kind == "Terminated" && record["reason"] == "delivered";
        self.inner.record(kind, record);
        if last {
            std::fs::write(self.copy.join("bindings/tools/added-after-the-last-round.txt"), "x\n").unwrap();
            let d = self.daemon.get().unwrap();
            for x in d.decisions_of_run(&self.run) {
                let id = x["request_id"].as_str().unwrap().to_string();
                let o = d.answer_decision(AnswerDecision { run_id: self.run.clone(), request_id: id.clone(), answer: "approve".into(), answered_by: "late".into() }).unwrap();
                self.tried.lock().unwrap().push((id, if o.is_refusal() { o.name.to_string() } else { format!("ACCEPTED {}", o.name) }));
            }
        }
    }
    fn decision_requested(&self, request: DecisionRequest) {
        self.inner.decision_requested(request)
    }
    fn decision_answered(&self, request_id: &str, answer: &str, by: &str, principal: &str) -> bool {
        self.inner.decision_answered(request_id, answer, by, principal)
    }
    fn resumed(&self) {
        self.inner.resumed()
    }
}

impl Runtime for AnswersAtTheEnd {
    fn recover(&self, run_id: &str) -> Recovery {
        self.engine.recover(run_id)
    }
    fn knows_runbook(&self, runbook: &str, version: Option<&str>) -> bool {
        self.engine.knows_runbook(runbook, version)
    }
    fn runbooks(&self) -> Vec<RunbookInfo> {
        self.engine.runbooks()
    }
    fn kind(&self) -> &'static str {
        self.engine.kind()
    }
    fn start(&self, request: StartRequest, sink: &dyn EventSink, cancel: &AtomicBool) -> Result<Finish, RuntimeFailure> {
        let hooked = AnsweringSink { inner: sink, copy: request.state_dir.join(COPY_DIR), run: request.run_id.clone(), daemon: self.daemon.clone(), tried: self.tried.clone() };
        self.engine.start(request, &hooked, cancel)
    }
    fn answer(&self, run_id: &str, request_id: &str, answer: &str, principal: &str) -> Result<(), AnswerFailure> {
        self.engine.answer(run_id, request_id, answer, principal)
    }
    fn cancel(&self, run_id: &str) {
        self.engine.cancel(run_id)
    }
}

#[test]
fn an_answer_while_the_last_check_runs_is_refused_and_the_copy_changed_at_the_end_is_not_delivered() {
    let (cell, tried) = (Arc::new(OnceLock::new()), Arc::new(Mutex::new(Vec::new())));
    let (c2, t2) = (cell.clone(), tried.clone());
    let rig = Real::with("last-check", Confinement::Local, move |engine| Arc::new(AnswersAtTheEnd { engine, daemon: c2, tried: t2 }));
    cell.set(rig.d.clone()).ok();
    let run = rig.submit_demo().run_id.unwrap();
    rig.settle(&run);
    let row = rig.row(&run);
    assert_eq!(row["outcome"], "runtime-failure", "{row}");
    let tried = tried.lock().unwrap().clone();
    assert!(!tried.is_empty(), "the hook made no attempt");
    assert!(tried.iter().all(|(_, name)| !name.starts_with("ACCEPTED")), "an answer was accepted while the last check ran: {tried:?}");
    let failed: Vec<Value> = rig.d.events_after(&run, 0).into_iter().filter(|e| e["kind"] == "runbook.runtime-failed").collect();
    assert_eq!(failed.len(), 1, "{failed:?}");
}

#[test]
fn no_hook_that_changes_a_copy_or_a_run_is_reachable_from_the_product() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut hits = Vec::new();
    fn walk(dir: &Path, hits: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, hits);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let text = std::fs::read_to_string(&p).unwrap();
                for (n, line) in text.lines().enumerate() {
                    let low = line.to_lowercase();
                    if (low.contains("hook") || low.contains("added-after") || low.contains("env::var(\"runbook_test") || low.contains("cfg(feature")) && !line.trim_start().starts_with("//") {
                        hits.push(format!("{}:{}: {}", p.display(), n + 1, line.trim()));
                    }
                }
            }
        }
    }
    walk(&src, &mut hits);
    assert!(hits.is_empty(), "{hits:#?}");
}

#[test]
fn no_record_of_a_run_names_a_path_into_the_runs_directory() {
    let rig = Real::new("records", Confinement::Local);
    let run = rig.submit_demo().run_id.unwrap();
    rig.settle(&run);
    assert_eq!(rig.row(&run)["outcome"], "delivered", "{}", rig.row(&run));
    let seen = rig.seen(&run);
    let runs = rig.state().join("runs");
    assert!(!seen.contains(runs.to_str().unwrap()), "a record names a path under the runs directory");
    assert!(!seen.contains(rig.home.to_str().unwrap()), "a record names a path of the home");
}

/// What leaves the engine toward a client names no path of the state directory and no path of the
/// runbook home, whatever the names of those directories are.
mod names_of_paths {
    use super::super::state::scratch;
    use runbook::commands::{AnswerDecision, CancelRun, Config, Daemon, SubmitRun};
    use runbook::engine::Engine;
    use super::journal_before_hand_over::config;
    use runbook::runtime::Confinement;
    use serde_json::Value;
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    const RUNBOOK: &str = "engine-demo";

    fn fresh(tag: &str) -> PathBuf {
        std::fs::canonicalize(scratch(&format!("names-of-paths-{tag}"))).unwrap()
    }

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

    /// A daemon over the real engine whose state directory and runbook home have the names given. `token` is a
    /// piece of the path of both that nothing a client reads may hold.
    struct Rig {
        dir: PathBuf,
        home: PathBuf,
        state: PathBuf,
        ctx: PathBuf,
        d: Daemon,
        token: String,
    }

    /// A leaf of a path: the bytes, and whether the path goes through a link to a directory of its own.
    struct Leaf {
        bytes: &'static [u8],
        link: bool,
    }

    const fn leaf(bytes: &'static [u8]) -> Leaf {
        Leaf { bytes, link: false }
    }

    impl Rig {
        fn new(tag: &str, state: &Leaf, home: &Leaf) -> Rig {
            let dir = fresh(tag);
            let place = |leaf: &Leaf, real: &str| -> PathBuf {
                let named = dir.join(OsStr::from_bytes(leaf.bytes));
                if leaf.link {
                    let target = dir.join(real);
                    std::fs::create_dir_all(&target).unwrap();
                    std::os::unix::fs::symlink(&target, &named).unwrap();
                }
                named
            };
            let home_path = place(home, "real-home");
            let state_path = place(state, "real-state");
            copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &home_path);
            let binding = home_path.join("bindings/engine-draft.yaml");
            let text = std::fs::read_to_string(&binding).unwrap().replace("principal: reviewer", &format!("principal: {}", super::super::PRINCIPAL));
            std::fs::write(&binding, text).unwrap();
            let ctx = fresh(&format!("{tag}-ctx"));
            let engine = Arc::new(Engine::with_levels(home_path.clone(), state_path.clone(), &[Confinement::Local, Confinement::Linux], crate::daemon::policy()));
            let cfg = Config { state_dir: state_path.clone(), confinement_floor: Confinement::Local, context_roots: vec![ctx.clone()], runbook_dir: None, principal: super::super::PRINCIPAL.into() };
            let token = dir.file_name().unwrap().to_string_lossy().into_owned();
            Rig { dir, home: home_path, state: state_path, ctx, d: Daemon::start(cfg, engine).unwrap(), token }
        }

        fn row(&self, run: &str) -> Value {
            self.d.run_detail(run)[0].clone()
        }

        fn state_of(&self, run: &str) -> String {
            self.row(run)["state"].as_str().unwrap().to_string()
        }

        fn over(&self, run: &str) -> bool {
            matches!(self.state_of(run).as_str(), "Ended" | "Cancelled" | "FailedToStart")
        }

        fn until(&self, what: &str, secs: u64, mut done: impl FnMut(&Rig) -> bool) {
            let limit = Instant::now() + Duration::from_secs(secs);
            while !done(self) {
                assert!(Instant::now() < limit, "gave up waiting for {what}");
                std::thread::sleep(Duration::from_millis(10));
            }
        }

        fn seen(&self, run: &str) -> String {
            format!("{} {:?} {:?}", self.row(run), self.d.events_after(run, 0), self.d.decisions_of_run(run))
        }

        /// Answers what the run asks until it is over or `secs` have passed (then the run is cancelled).
        fn settle(&self, run: &str, secs: u64) {
            let mut answered = std::collections::BTreeSet::new();
            let limit = Instant::now() + Duration::from_secs(secs);
            while !self.over(run) {
                if Instant::now() > limit {
                    let _ = self.d.cancel_run(CancelRun { run_id: run.into() });
                    break;
                }
                for d in self.d.open_decisions_of_run(run) {
                    let id = d["request_id"].as_str().unwrap().to_string();
                    if !answered.insert(id.clone()) {
                        continue;
                    }
                    let review = d["options"].as_array().unwrap().iter().any(|o| o["answer"].as_str().is_some_and(|a| a.contains("result")));
                    let (answer, who) = if review { ("performed", "reviewer") } else { ("approve", "owner") };
                    let _ = self.d.answer_decision(AnswerDecision { run_id: run.into(), request_id: id, answer: answer.into(), answered_by: who.into() });
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }

        /// What a client reads of one submit that fails in the way `kind` says.
        fn client_reads(&self, kind: &str) -> String {
            self.client_reads_as_texts(kind).0
        }

        /// What a client reads of one submit that fails in the way `kind` says: once as one text of the
        /// Debug form of the records, once as the plain texts the records hold.
        fn client_reads_as_texts(&self, kind: &str) -> (String, Vec<String>) {
            let mut context = None;
            match kind {
                "protocol-not-yaml" => std::fs::write(self.home.join("protocol/zz.protocol.yaml"), "protocol: [unclosed\n").unwrap(),
                "idea-is-a-directory" => {
                    std::fs::create_dir_all(self.ctx.join("c/idea.yaml")).unwrap();
                    context = Some(self.ctx.join("c").to_str().unwrap().to_string());
                }
                "link-out" => {
                    let outside = self.dir.join("outside-file");
                    std::fs::write(&outside, "x").unwrap();
                    std::os::unix::fs::symlink(&outside, self.home.join("bindings/tools/leak.sh")).unwrap();
                }
                other => panic!("{other}"),
            }
            let o = self.d.submit_run(SubmitRun { runbook_id: RUNBOOK.into(), intake: "a note on tea".into(), confinement: Some(Confinement::Local), context_path: context, ..SubmitRun::default() }).unwrap();
            let mut texts = Vec::new();
            if o.is_refusal() {
                texts.extend(o.error.map(str::to_string));
                texts.extend(o.payload.iter().flat_map(strings_of));
                texts.extend(o.published.iter().flat_map(|(name, v)| std::iter::once(name.to_string()).chain(strings_of(v))));
                return (format!("{o:?}"), texts);
            }
            let run = o.run_id.clone().unwrap();
            self.until("the run to be over", 40, |me| me.over(&run));
            texts.extend(strings_of(&self.row(&run)));
            texts.extend(self.d.events_after(&run, 0).iter().flat_map(strings_of));
            texts.extend(self.d.decisions_of_run(&run).iter().flat_map(strings_of));
            (self.seen(&run), texts)
        }
    }

    /// Every key and every text a value holds, as they are and not escaped.
    fn strings_of(value: &Value) -> Vec<String> {
        match value {
            Value::String(s) => vec![s.clone()],
            Value::Array(items) => items.iter().flat_map(strings_of).collect(),
            Value::Object(map) => map.iter().flat_map(|(k, v)| std::iter::once(k.clone()).chain(strings_of(v))).collect(),
            _ => vec![],
        }
    }

    impl Drop for Rig {
        fn drop(&mut self) {
            self.d.stop();
        }
    }

    /// What in `seen` names the rig's paths, or nothing.
    fn leaked(rig: &Rig, kind: &str, seen: &str) -> Option<String> {
        let at = seen.find(rig.token.as_str())?;
        let from = seen[..at].char_indices().rev().nth(60).map_or(0, |(i, _)| i);
        let to = (at + 160).min(seen.len());
        let to = (to..=seen.len()).find(|i| seen.is_char_boundary(*i)).unwrap_or(seen.len());
        Some(format!("{kind}: the token of the path is in what a client reads: ...{}...", &seen[from..to]))
    }

    fn names_leave_nothing(tag: &str, state: &Leaf, home: &Leaf) -> Vec<String> {
        let mut bad = Vec::new();
        for kind in ["protocol-not-yaml", "idea-is-a-directory", "link-out"] {
            let rig = Rig::new(&format!("{tag}-{kind}"), state, home);
            let seen = rig.client_reads(kind);
            if let Some(why) = leaked(&rig, kind, &seen) {
                bad.push(why);
            }
        }
        bad
    }

    fn assert_clean(what: &str, bad: Vec<String>) {
        assert!(bad.is_empty(), "{what}: {} of 3 failures name the path:\n{}", bad.len(), bad.join("\n"));
    }

    /// Every way a text can spell `path`: as the file system names it (lossy where it is not UTF-8), with
    /// each control character replaced by a space, and as Rust's `Debug` escapes it, of each of those.
    fn forms_of(path: &Path) -> Vec<String> {
        let mut forms = spellings_of(path);
        if let Ok(real) = std::fs::canonicalize(path) {
            forms.extend(spellings_of(&real));
        }
        forms.sort();
        forms.dedup();
        forms
    }

    fn spellings_of(path: &Path) -> Vec<String> {
        let escaped = |s: &str| {
            let quoted = format!("{s:?}");
            quoted[1..quoted.len() - 1].to_string()
        };
        let lossy = path.to_string_lossy().into_owned();
        let cleaned: String = lossy.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
        let debug_path = format!("{path:?}");
        let mut forms = vec![lossy.clone(), cleaned.clone(), escaped(&lossy), escaped(&cleaned), debug_path[1..debug_path.len() - 1].to_string()];
        let escaped_then_cleaned: String = escaped(&lossy).chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
        forms.push(escaped_then_cleaned);
        forms
    }

    #[test]
    fn no_form_of_the_state_directory_or_the_home_reaches_a_client_whatever_the_names_are() {
        let table: [(&str, Leaf, Leaf); 14] = [
            ("tab", leaf(b"st\tate"), leaf(b"ho\tme")),
            ("newline", leaf(b"st\nate"), leaf(b"ho\nme")),
            ("carriage return", leaf(b"st\rate"), leaf(b"ho\rme")),
            ("c1 control", leaf("st\u{85}ate".as_bytes()), leaf("ho\u{9b}me".as_bytes())),
            ("delete", leaf(b"st\x7fate"), leaf(b"ho\x7fme")),
            ("quote", leaf(b"st\"ate"), leaf(b"ho\"me")),
            ("backslash", leaf(b"st\\ate"), leaf(b"ho\\me")),
            ("combining mark", leaf("st\u{301}ate".as_bytes()), leaf("ho\u{301}me".as_bytes())),
            ("space", leaf(b"st ate  dir"), leaf(b"ho me x")),
            ("not utf-8", leaf(b"st\xFFate"), leaf(b"ho\xFEme")),
            ("prefix of another", leaf(b"st"), leaf(b"st-home")),
            ("another is its prefix", leaf(b"h-state"), leaf(b"h")),
            ("linked directories", Leaf { bytes: b"link-state", link: true }, Leaf { bytes: b"link-home", link: true }),
            ("tab and quote below a link", Leaf { bytes: b"l\t\"k-state", link: true }, Leaf { bytes: b"l\t\"k-home", link: true }),
        ];
        let mut bad = Vec::new();
        for (n, (what, state, home)) in table.iter().enumerate() {
            for kind in ["protocol-not-yaml", "idea-is-a-directory", "link-out"] {
                let rig = Rig::new(&format!("forms-{n}-{kind}"), state, home);
                let (seen, texts) = rig.client_reads_as_texts(kind);
                if let Some(why) = leaked(&rig, &format!("{what}/{kind}"), &seen) {
                    bad.push(why);
                }
                for (dir, name) in [(&rig.state, "state directory"), (&rig.home, "home")] {
                    for form in forms_of(dir) {
                        if let Some(text) = texts.iter().find(|t| t.contains(form.as_str())) {
                            bad.push(format!("{what}/{kind}: the {name} as {form:?} is in the text {:?}", text.chars().take(240).collect::<String>()));
                        }
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{} of {} failures name a path:\n{}", bad.len(), table.len() * 3, bad.join("\n"));
    }

    #[test]
    fn a_state_directory_with_a_tab_in_its_name_leaves_no_path_in_a_failure() {
        assert_clean("tab", names_leave_nothing("tab", &leaf(b"st\tate"), &leaf(b"home")));
    }

    #[test]
    fn a_state_directory_with_a_newline_in_its_name_leaves_no_path_in_a_failure() {
        assert_clean("newline", names_leave_nothing("nl", &leaf(b"st\nate"), &leaf(b"home")));
    }

    #[test]
    fn a_state_directory_with_a_quote_and_a_backslash_in_its_name_leaves_no_path_in_a_failure() {
        assert_clean("quote and backslash", names_leave_nothing("quote", &leaf(b"st\"at\\e"), &leaf(b"home")));
    }

    #[test]
    fn a_state_directory_with_a_combining_mark_in_its_name_leaves_no_path_in_a_failure() {
        assert_clean("combining mark", names_leave_nothing("comb", &leaf("st\u{301}ate".as_bytes()), &leaf(b"home")));
    }

    #[test]
    fn a_state_directory_and_a_home_that_are_not_utf8_leave_no_path_in_a_failure() {
        assert_clean("not UTF-8", names_leave_nothing("bytes", &leaf(b"st\xFFate"), &leaf(b"ho\xFEme")));
    }

    #[test]
    fn a_home_given_as_a_relative_name_leaves_the_words_of_a_reason_as_they_are() {
        // `runbook serve --runbook-home home`: the engine's home is the relative path `home`. What the engine
        // takes out of a text is the path, not every word that spells it.
        let dir = fresh("relative");
        let home = dir.join("home");
        copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &home);
        let binding = home.join("bindings/engine-draft.yaml");
        let text = std::fs::read_to_string(&binding).unwrap().replace("principal: reviewer", &format!("principal: {}", super::super::PRINCIPAL));
        std::fs::write(&binding, text).unwrap();
        let outside = dir.join("outside-file");
        std::fs::write(&outside, "x").unwrap();
        std::os::unix::fs::symlink(&outside, home.join("bindings/tools/leak.sh")).unwrap();
        std::env::set_current_dir(&dir).unwrap();
        let engine = Arc::new(Engine::with_levels(PathBuf::from("home"), dir.join("state"), &[Confinement::Local, Confinement::Linux], crate::daemon::policy()));
        let d = Daemon::start(config(&dir), engine).unwrap();
        let run = d.submit_run(SubmitRun { runbook_id: RUNBOOK.into(), intake: "a note on tea".into(), confinement: Some(Confinement::Local), ..SubmitRun::default() }).unwrap().run_id.unwrap();
        let limit = Instant::now() + Duration::from_secs(40);
        while !matches!(d.run_detail(&run)[0]["state"].as_str(), Some("FailedToStart" | "Ended")) {
            assert!(Instant::now() < limit, "the run did not end");
            std::thread::sleep(Duration::from_millis(10));
        }
        let seen = format!("{} {:?}", d.run_detail(&run)[0], d.events_after(&run, 0));
        d.stop();
        assert!(seen.contains("a link that leaves the runbook home"), "a word of the reason was taken for the path: {seen}");
    }

    #[test]
    fn a_home_with_a_tab_and_a_space_and_a_state_directory_with_spaces_leave_no_path_in_a_failure() {
        assert_clean("spaces", names_leave_nothing("space", &leaf(b"st ate  dir"), &leaf(b"ho\tme x")));
    }

    #[test]
    fn a_state_directory_and_a_home_reached_through_links_leave_no_path_in_a_failure() {
        assert_clean("links", names_leave_nothing("link", &Leaf { bytes: b"link-state", link: true }, &Leaf { bytes: b"link-home", link: true }));
    }

    #[test]
    fn a_path_that_is_a_prefix_of_another_leaves_no_path_in_a_failure() {
        assert_clean("prefix", names_leave_nothing("prefix-a", &leaf(b"st"), &leaf(b"st-home")));
        assert_clean("prefix reversed", names_leave_nothing("prefix-b", &leaf(b"h-state"), &leaf(b"h")));
        assert_clean("home inside state", names_leave_nothing("nested", &leaf(b"st"), &leaf(b"st/home")));
        assert_clean("state inside home", names_leave_nothing("nested-b", &leaf(b"home/state"), &leaf(b"home")));
    }

    #[test]
    fn a_program_that_names_its_own_paths_leaves_no_path_in_what_the_run_reports() {
        let rig = Rig::new("program", &leaf(b"state"), &leaf(b"home"));
        let say = "#!/bin/sh\necho \"program $0 in $PWD, workspace $PWD.\" >&2\necho \"out $0 in $PWD.\"\n";
        std::fs::write(rig.home.join("bindings/tools/draft-check.sh"), format!("{say}[ -s draft.txt ]\n")).unwrap();
        let run = rig.d.submit_run(SubmitRun { runbook_id: RUNBOOK.into(), intake: "a note on tea".into(), confinement: Some(Confinement::Local), ..SubmitRun::default() }).unwrap().run_id.unwrap();
        rig.settle(&run, 20);
        let seen = rig.seen(&run);
        if let Some(why) = leaked(&rig, "program", &seen) {
            panic!("{why}");
        }
    }
}

/// The daemon hands an answer to a run only after its journal holds the line, and takes the line back
/// when the run refuses the answer.
mod journal_before_hand_over {
    use super::super::state::scratch;
    use runbook::commands::{AnswerDecision, Config, Daemon, SubmitRun};
    use runbook::runtime::{AnswerFailure, Confinement, DecisionRequest, EventSink, Finish, Recovery, Runtime, RuntimeFailure, RunbookInfo, StartRequest};
    use serde_json::Value;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    fn fresh(tag: &str) -> PathBuf {
        std::fs::canonicalize(scratch(&format!("journal-{tag}"))).unwrap()
    }

    // ---- the journal line comes before the run is handed the answer ----

    /// A runtime of one run with two requests. The first answer makes it fail (it keeps the failure to itself
    /// until `release`, so that a late answer reaches the daemon after the runtime has failed); `kill` ends the
    /// process the moment the daemon hands it an answer.
    struct Gate {
        state: Mutex<GateState>,
        kill: bool,
    }

    #[derive(Default)]
    struct GateState {
        dead: bool,
        first: bool,
        release: bool,
        delivered: Vec<String>,
    }

    impl Gate {
        fn new(kill: bool) -> Arc<Gate> {
            Arc::new(Gate { state: Mutex::new(GateState::default()), kill })
        }

        fn wait_for(&self, what: &str, f: impl Fn(&GateState) -> bool) {
            let limit = Instant::now() + Duration::from_secs(20);
            while !f(&self.state.lock().unwrap()) {
                assert!(Instant::now() < limit, "gave up waiting for {what}");
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }

    impl Runtime for Gate {
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
        fn start(&self, request: StartRequest, sink: &dyn EventSink, cancel: &AtomicBool) -> Result<Finish, RuntimeFailure> {
            sink.started();
            for which in ["a", "b"] {
                sink.decision_requested(DecisionRequest { request_id: format!("{}-{which}", request.run_id), case_id: "c".into(), action: "act".into(), capability: "approve".into(), question: "q".into(), options: vec![], artifact: None });
            }
            if self.kill {
                while !cancel.load(std::sync::atomic::Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(5));
                }
                return Ok(Finish::Cancelled);
            }
            self.wait_for("the first answer", |s| s.first);
            self.state.lock().unwrap().dead = true;
            self.wait_for("the release", |s| s.release);
            Err(RuntimeFailure { reason: "RunbookCopyChanged: the copy changed".into() })
        }
        fn answer(&self, _run_id: &str, request_id: &str, _answer: &str, _principal: &str) -> Result<(), AnswerFailure> {
            if self.kill {
                unsafe {
                    libc::kill(libc::getpid(), libc::SIGKILL);
                }
                std::thread::sleep(Duration::from_secs(30));
            }
            let mut st = self.state.lock().unwrap();
            if st.dead {
                return Err(AnswerFailure::UnknownRun);
            }
            st.delivered.push(request_id.to_string());
            if request_id.ends_with("-a") {
                st.first = true;
            }
            Ok(())
        }
        fn cancel(&self, _run_id: &str) {}
    }

    pub(super) fn config(dir: &Path) -> Config {
        Config { state_dir: dir.join("state"), confinement_floor: Confinement::Local, context_roots: vec![], runbook_dir: None, principal: super::super::PRINCIPAL.into() }
    }

    fn submit_waiting(d: &Daemon) -> String {
        let run = d.submit_run(SubmitRun { runbook_id: "book".into(), intake: "x".into(), confinement: Some(Confinement::Local), ..SubmitRun::default() }).unwrap().run_id.unwrap();
        let limit = Instant::now() + Duration::from_secs(20);
        while d.open_decisions_of_run(&run).len() < 2 {
            assert!(Instant::now() < limit, "the run did not open its requests");
            std::thread::sleep(Duration::from_millis(2));
        }
        run
    }

    fn answer(d: &Daemon, run: &str, which: &str) -> runbook::commands::Outcome {
        d.answer_decision(AnswerDecision { run_id: run.into(), request_id: format!("{run}-{which}"), answer: "approve".into(), answered_by: "x".into() }).unwrap()
    }

    fn decision(d: &Daemon, run: &str, which: &str) -> Value {
        d.decisions_of_run(run).into_iter().find(|x| x["request_id"] == format!("{run}-{which}")).unwrap()
    }

    fn lines_for(dir: &Path, needle: &str) -> usize {
        let text = std::fs::read_to_string(dir.join("state/journal.jsonl")).unwrap();
        text.lines().filter(|l| l.contains("\"fact\":\"decision_answered\"") && l.contains(needle)).count()
    }

    #[test]
    fn an_answer_refused_as_late_is_not_in_the_journal_when_the_daemon_starts_again() {
        let dir = fresh("late-journal");
        let gate = Gate::new(false);
        let d = Daemon::start(config(&dir), gate.clone()).unwrap();
        let run = submit_waiting(&d);
        assert_eq!(answer(&d, &run, "a").name, "answered");
        gate.wait_for("the runtime to fail", |s| s.dead);
        let late = answer(&d, &run, "b");
        assert!(late.is_refusal() && late.name == "run-not-waiting", "{late:?}");
        gate.state.lock().unwrap().release = true;
        let limit = Instant::now() + Duration::from_secs(20);
        while d.run_detail(&run)[0]["state"] != "Ended" {
            assert!(Instant::now() < limit, "the run did not end");
            std::thread::sleep(Duration::from_millis(2));
        }
        d.stop();
        drop(d);
        assert_eq!(lines_for(&dir, &format!("{run}-a")), 1, "the first answer is on the journal once");
        assert_eq!(lines_for(&dir, &format!("{run}-b")), 0, "the journal holds the line of an answer that was refused");
        let reopened = Daemon::start(config(&dir), Gate::new(false)).unwrap();
        assert_eq!(decision(&reopened, &run, "a")["state"], "Answered");
        assert_ne!(decision(&reopened, &run, "b")["state"], "Answered", "a daemon that starts again holds an answer that was refused");
        reopened.stop();
    }

    const KILL_DIR: &str = "RUNBOOK_KILL_DIR";

    #[test]
    fn a_kill_between_the_journal_line_and_the_hand_over_leaves_the_answer_on_the_journal_once() {
        if let Some(dir) = std::env::var_os(KILL_DIR) {
            // The child: the daemon hands the first answer to the runtime, which ends the process.
            let d = Daemon::start(config(Path::new(&dir)), Gate::new(true)).unwrap();
            let run = submit_waiting(&d);
            std::fs::write(Path::new(&dir).join("run-id"), &run).unwrap();
            let _ = answer(&d, &run, "a");
            std::thread::sleep(Duration::from_secs(30));
            return;
        }
        let dir = fresh("kill");
        let exe = std::env::current_exe().unwrap();
        let name = "daemon::copy_integrity::journal_before_hand_over::a_kill_between_the_journal_line_and_the_hand_over_leaves_the_answer_on_the_journal_once";
        let status = std::process::Command::new(exe).args(["--exact", name, "--nocapture", "--test-threads=1"]).env(KILL_DIR, &dir).env("RUNBOOK_NO_CONFIG", "1").status().unwrap();
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(status.signal(), Some(libc::SIGKILL), "the child was not killed at the hand-over: {status:?}");
        let run = std::fs::read_to_string(dir.join("run-id")).unwrap();
        let journal = std::fs::read(dir.join("state/journal.jsonl")).unwrap();
        assert_eq!(journal.last(), Some(&b'\n'), "the journal ends in a line that is not whole");
        assert_eq!(lines_for(&dir, &format!("{run}-a")), 1, "the answer the run was being handed is on the journal once");
        let d = Daemon::start(config(&dir), Gate::new(false)).unwrap();
        assert_eq!(decision(&d, &run, "a")["state"], "Answered", "the answer the journal holds is not recorded after the restart");
        let row = d.run_detail(&run)[0].clone();
        assert_eq!((row["state"].as_str(), row["outcome"].as_str()), (Some("Ended"), Some("orphaned")), "{row}");
        assert!(d.open_decisions_of_run(&run).is_empty(), "a request is still open after the restart");
        d.stop();
    }

    #[test]
    fn two_clients_answering_two_requests_leave_one_journal_line_each_and_a_whole_journal() {
        let dir = fresh("two-lines");
        let gate = Gate::new(false);
        let d = Daemon::start(config(&dir), gate.clone()).unwrap();
        let mut problems = Vec::new();
        for i in 0..40u64 {
            let run = submit_waiting(&d);
            let barrier = Arc::new(std::sync::Barrier::new(2));
            let second = {
                let (d, run, barrier) = (d.clone(), run.clone(), barrier.clone());
                std::thread::spawn(move || {
                    barrier.wait();
                    let until = Instant::now() + Duration::from_micros(i * 15);
                    while Instant::now() < until {
                        std::hint::spin_loop();
                    }
                    answer(&d, &run, "b")
                })
            };
            barrier.wait();
            let first = answer(&d, &run, "a");
            let second = second.join().unwrap();
            let state = |w: &str| decision(&d, &run, w)["state"].clone();
            for (w, o) in [("a", &first), ("b", &second)] {
                let accepted = o.name == "answered";
                let on_journal = lines_for(&dir, &format!("{run}-{w}"));
                if on_journal > 1 || accepted != (on_journal == 1) || accepted != (state(w) == "Answered") {
                    problems.push(format!("run {i} answer {w}: {} journal lines {on_journal}, decision {}", o.name, state(w)));
                }
            }
            gate.state.lock().unwrap().release = true;
            let limit = Instant::now() + Duration::from_secs(20);
            while d.run_detail(&run)[0]["state"] != "Ended" {
                assert!(Instant::now() < limit, "the run did not end");
                std::thread::sleep(Duration::from_millis(1));
            }
            *gate.state.lock().unwrap() = GateState::default();
        }
        d.stop();
        drop(d);
        let text = std::fs::read_to_string(dir.join("state/journal.jsonl")).unwrap();
        assert!(text.ends_with('\n') && text.lines().all(|l| serde_json::from_str::<Value>(l).is_ok()), "the journal holds a line that is not whole");
        assert!(problems.is_empty(), "{} problems: {:?}", problems.len(), &problems[..problems.len().min(6)]);
    }
}

/// A state directory (and a runbook home) given as a relative path, as `runbook serve --state-dir rel`
/// gives it: every path the engine passes to Loom is absolute, and nothing a client reads names a path.
/// These cases change the process's directory to their own scratch directory (each runs in a process of
/// its own under the test runner).
mod relative_state_directory {
    use super::super::state::scratch;
    use runbook::commands::{Config, Daemon, SubmitRun};
    use runbook::engine::Engine;
    use runbook::runtime::{Confinement, DecisionRequest, EventSink, Runtime, StartRequest};
    use serde_json::Value;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    const RUNBOOK: &str = "engine-demo";

    struct Quiet;

    impl EventSink for Quiet {
        fn steps(&self) -> Option<Arc<dyn runbook::runtime::StartTarget>> {
            None
        }
        fn started(&self) {}
        fn record(&self, _kind: &str, _record: Value) {}
        fn decision_requested(&self, _request: DecisionRequest) {}
        fn decision_answered(&self, _request_id: &str, _answer: &str, _by: &str, _principal: &str) -> bool {
            false
        }
        fn resumed(&self) {}
    }

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

    fn strings_of(value: &Value) -> Vec<String> {
        match value {
            Value::String(s) => vec![s.clone()],
            Value::Array(items) => items.iter().flat_map(strings_of).collect(),
            Value::Object(map) => map.iter().flat_map(|(k, v)| std::iter::once(k.clone()).chain(strings_of(v))).collect(),
            _ => vec![],
        }
    }

    /// A scratch directory holding the fixture runbook as `home`, which becomes the process's directory.
    fn workdir(tag: &str) -> PathBuf {
        let dir = std::fs::canonicalize(scratch(&format!("relative-state-{tag}"))).unwrap();
        let home = dir.join("rel-home-dir");
        copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &home);
        let binding = home.join("bindings/engine-draft.yaml");
        let text = std::fs::read_to_string(&binding).unwrap().replace("principal: reviewer", &format!("principal: {}", super::super::PRINCIPAL));
        std::fs::write(&binding, text).unwrap();
        std::env::set_current_dir(&dir).unwrap();
        dir
    }

    fn until_waiting_or_over(d: &Daemon, run: &str) -> String {
        let limit = Instant::now() + Duration::from_secs(40);
        loop {
            let state = d.run_detail(run)[0]["state"].as_str().unwrap().to_string();
            if matches!(state.as_str(), "FailedToStart" | "Ended" | "Cancelled" | "WaitingOnDecision") {
                return state;
            }
            assert!(Instant::now() < limit, "the run did not wait or end: {}", d.run_detail(run)[0]);
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn an_ordinary_run_under_a_state_directory_given_as_a_relative_path_names_no_path() {
        workdir("ordinary");
        let engine = Arc::new(Engine::with_levels(PathBuf::from("rel-home-dir"), PathBuf::from("rel-state-dir"), &[Confinement::Local, Confinement::Linux], crate::daemon::policy()));
        let cfg = Config { state_dir: PathBuf::from("rel-state-dir"), confinement_floor: Confinement::Local, context_roots: vec![], runbook_dir: None, principal: super::super::PRINCIPAL.into() };
        let d = Daemon::start(cfg, engine).unwrap();
        let run = d.submit_run(SubmitRun { runbook_id: RUNBOOK.into(), intake: "a note on tea".into(), confinement: Some(Confinement::Local), ..SubmitRun::default() }).unwrap().run_id.unwrap();
        until_waiting_or_over(&d, &run);
        let mut texts = strings_of(&d.run_detail(&run)[0]);
        texts.extend(d.events_after(&run, 0).iter().flat_map(strings_of));
        texts.extend(d.decisions_of_run(&run).iter().flat_map(strings_of));
        d.stop();
        let bad: Vec<String> = texts.iter().filter(|t| t.contains("rel-state-dir") || t.contains("rel-home-dir")).map(|t| t.chars().take(260).collect()).collect();
        assert!(bad.is_empty(), "the directory as the operator gave it is in {} texts: {bad:?}", bad.len());
    }

    #[test]
    fn a_store_that_is_not_a_directory_names_no_path_under_a_relative_state_directory() {
        let dir = workdir("store");
        let engine = Engine::with_levels(PathBuf::from("rel-home-dir"), PathBuf::from("rel-state-dir"), &[Confinement::Local], crate::daemon::policy());
        let run_dir = PathBuf::from("rel-state-dir/runs/r1");
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::write(run_dir.join("tree"), "not a directory").unwrap();
        let request = StartRequest {
            run_id: "r1".into(),
            runbook: RUNBOOK.into(),
            version: None,
            intake: String::new(),
            context_dir: None,
            settings: Default::default(),
            confinement: Confinement::Local,
            state_dir: run_dir,
            workspace_dir: PathBuf::from("rel-state-dir/workspaces/r1"),
            terms: Default::default(),
        };
        let reason = engine.start(request, &Quiet, &AtomicBool::new(false)).expect_err("the start did not fail").reason;
        assert!(reason.contains("tree"), "the start did not fail at the store as the case says: {reason}");
        assert!(!reason.contains("rel-state-dir") && !reason.contains(dir.to_str().unwrap()), "a path of the state directory is in the reason: {reason}");
    }

    #[test]
    fn a_run_under_a_state_directory_and_a_home_given_as_relative_paths_starts_and_waits_on_its_first_decision() {
        let dir = workdir("serve");
        let args = runbook::serve::ServeArgs {
            listen: "127.0.0.1:0".into(),
            state_dir: Some(PathBuf::from("rel-state-dir")),
            context_root: vec![],
            runbook_dir: None,
            runtime: "loom".into(),
            runbook_home: Some(PathBuf::from("rel-home-dir")),
            runbooks: vec![],
            min_confinement: "local".into(),
            default_confinement: None,
            file_confinement: None,
            autonomy: None,
            decider: None,
            arbiter: None,
            routes: Default::default(),
            route_table: None,
            token_file: None,
            principal: Some(super::super::PRINCIPAL.into()),
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
        let prepared = runbook::serve::prepare(&args, &runbook::serve::Env::default(), &[]).unwrap();
        let d = prepared.daemon;
        let run = d.submit_run(SubmitRun { runbook_id: RUNBOOK.into(), intake: "a note on tea".into(), confinement: Some(Confinement::Local), ..SubmitRun::default() }).unwrap().run_id.unwrap();
        let state = until_waiting_or_over(&d, &run);
        let row = d.run_detail(&run)[0].clone();
        let open = d.open_decisions_of_run(&run);
        d.stop();
        assert_eq!(state, "WaitingOnDecision", "the run did not wait on a decision: {row}");
        assert!(!open.is_empty(), "the run waits on no decision");
        assert!(dir.join("rel-state-dir/runs").join(&run).join("tree").is_dir(), "the run's store is not below the state directory as given");
    }
}
