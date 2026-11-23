//! The pin of the `ess` binary (`docs/conformance.md`, "The ess pin"): a binary whose bytes differ
//! from the pin is refused, whatever else is on the host, and the check does not wait on anything
//! that is not the binary itself. The helpers are shared with the capture cases beside this file.

use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value as J};
use sha2::{Digest, Sha256};

const BIN: &str = env!("CARGO_BIN_EXE_sdc-conform");

pub(crate) fn tmp(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("ess-pin-guard").join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn sha_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn says(o: &Output) -> String {
    format!("exit {:?} stdout {} stderr {}", o.status.code(), String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

pub(crate) fn executable(path: &Path, body: &str) {
    fs::write(path, body).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn triple() -> String {
    let arch = std::env::consts::ARCH;
    if cfg!(target_os = "macos") { format!("{arch}-apple-darwin") } else { format!("{arch}-unknown-linux-gnu") }
}

/// A pin of version 9.9.9 that names `want` as the binary's digest.
fn pin_of(dir: &Path, want: &str) -> PathBuf {
    let pin = dir.join("pin.json");
    fs::write(&pin, json!({"format": "sdc.ess-pin/1", "version": "9.9.9", "platforms": {triple(): {"binary_sha256": want}}}).to_string()).unwrap();
    pin
}

fn ess_path(pin: &Path, bin: &Path, cache: &Path) -> Command {
    let mut c = Command::new(BIN);
    c.arg("ess-path").arg("--pin").arg(pin).env("ESS_BIN", bin).env("SDC_ESS_DIGEST_CACHE", cache);
    c
}

/// The stamp `binary_sha256` records for a file: size, mtime, ctime, inode, device. All of it is
/// what `stat` shows to anyone who can see the file.
fn stamp(path: &Path) -> String {
    let m = fs::metadata(path).unwrap();
    format!("{} {}.{:09} {}.{:09} {} {}", m.len(), m.mtime(), m.mtime_nsec(), m.ctime(), m.ctime_nsec(), m.ino(), m.dev())
}

/// A cache entry written by hand, not by `sdc-conform`: it names `sha` for the file at `path` as
/// that file is now.
fn entry_by_hand(cache: &Path, path: &Path, sha: &str) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    fs::create_dir_all(cache).unwrap();
    let entry = cache.join(&sha_hex(path.as_os_str().as_bytes())[..32]);
    fs::write(&entry, format!("ess-digest/1 {sha} 9999999999000000000\n{}\n{}\n", stamp(path), path.display())).unwrap();
    entry
}

const PINNED: &str = "#!/bin/sh\necho 'ess 9.9.9'\n";
const OTHER: &str = "#!/bin/sh\necho 'ess 9.9.9' # not the pinned bytes\n";

// ---- no file stands in for the hash of the binary ----------------------------------------------

/// The invariant of the pin: a binary whose bytes differ from the pin is never accepted. The binary
/// below is refused with no file beside it; a record of its digest, written by hand in the format a
/// digest cache once used and in the directory a variable names (the stamp is what `stat` prints),
/// must not change that. Nothing reads the variable any more: the case fails when something does.
#[test]
fn a_record_written_by_hand_in_a_named_directory_does_not_make_another_binary_the_pinned_one() {
    let dir = tmp("entry-by-hand");
    let (bin, cache) = (dir.join("ess"), dir.join("cache"));
    executable(&bin, OTHER);
    let want = sha_hex(PINNED.as_bytes());
    let pin = pin_of(&dir, &want);

    let o = ess_path(&pin, &bin, &dir.join("empty-cache")).output().unwrap();
    assert!(!o.status.success() && String::from_utf8_lossy(&o.stderr).contains("ess_pin_mismatch"), "the control, no entry: {}", says(&o));

    entry_by_hand(&cache, &bin, &want);
    let o = ess_path(&pin, &bin, &cache).output().unwrap();
    assert!(
        !o.status.success(),
        "a binary with sha256 {} was accepted as the pin {want} from a cache entry written by hand: {}",
        sha_hex(&fs::read(&bin).unwrap()),
        says(&o)
    );
}

/// The same entry in the default place: no `SDC_ESS_DIGEST_CACHE`, `$HOME/.cache/sdc/ess-digest`.
#[test]
fn a_record_written_by_hand_below_home_does_not_make_another_binary_the_pinned_one() {
    let dir = tmp("entry-by-hand-home");
    let bin = dir.join("ess");
    executable(&bin, OTHER);
    let want = sha_hex(PINNED.as_bytes());
    let pin = pin_of(&dir, &want);
    let home = dir.join("home");
    entry_by_hand(&home.join(".cache/sdc/ess-digest"), &bin, &want);
    let o = Command::new(BIN)
        .arg("ess-path")
        .arg("--pin")
        .arg(&pin)
        .env("ESS_BIN", &bin)
        .env("HOME", &home)
        .env_remove("SDC_ESS_DIGEST_CACHE")
        .env_remove("XDG_CACHE_HOME")
        .output()
        .unwrap();
    assert!(!o.status.success(), "another binary was accepted as the pin from an entry below $HOME/.cache/sdc/ess-digest: {}", says(&o));
}

/// A FIFO where a record of the binary's digest was once kept: `sdc-conform ess-path` reads no
/// such file, so it answers (here: refuses the binary, which is not the pin) and does not wait on
/// it. The case takes 5 s when it fails.
#[test]
fn a_fifo_where_a_record_was_once_kept_does_not_stop_the_pin_check() {
    use std::os::unix::ffi::OsStrExt;
    let dir = tmp("fifo-entry");
    let (bin, cache) = (dir.join("ess"), dir.join("cache"));
    executable(&bin, OTHER);
    let pin = pin_of(&dir, &sha_hex(PINNED.as_bytes()));
    fs::create_dir_all(&cache).unwrap();
    let entry = cache.join(&sha_hex(bin.as_os_str().as_bytes())[..32]);
    assert!(Command::new("mkfifo").arg(&entry).status().unwrap().success());

    let mut child = ess_path(&pin, &bin, &cache).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let started = Instant::now();
    let ended = loop {
        if let Some(st) = child.try_wait().unwrap() {
            break Some(st);
        }
        if started.elapsed() > Duration::from_secs(5) {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(ended.is_some(), "sdc-conform ess-path did not end within 5 s: it waits on the cache entry, a FIFO");
    assert_eq!(ended.unwrap().code(), Some(1), "the binary is not the pin");
}

/// The bytes decide, every time: the pin's binary is accepted; a binary of another size at the same
/// path is refused; one of the same size and the old mtime put back is refused; the pin's binary put
/// back is accepted. The refusal names the digest of the bytes the file has now.
#[test]
fn a_changed_binary_is_refused_and_the_pin_binary_put_back_is_accepted() {
    let dir = tmp("swap");
    let (bin, nowhere) = (dir.join("ess"), dir.join("nowhere"));
    let good = "#!/bin/sh\necho 'ess 9.9.9'\n";
    let want = sha_hex(good.as_bytes());
    let pin = pin_of(&dir, &want);
    executable(&bin, good);
    let good_mtime = fs::metadata(&bin).unwrap().modified().unwrap();
    std::thread::sleep(Duration::from_millis(250));

    let o = ess_path(&pin, &bin, &nowhere).output().unwrap();
    assert!(o.status.success(), "{}", says(&o));
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), bin.to_str().unwrap());

    let longer = "#!/bin/sh\necho 'ess 9.9.9'\n# swapped\n";
    executable(&bin, longer);
    let o = ess_path(&pin, &bin, &nowhere).output().unwrap();
    let err = String::from_utf8_lossy(&o.stderr).to_string();
    assert!(!o.status.success() && err.contains("ess_pin_mismatch") && err.contains(&sha_hex(longer.as_bytes())), "{}", says(&o));

    let same_size = "#!/bin/sh\necho 'ess 9.9.8'\n";
    assert_eq!(same_size.len(), good.len());
    executable(&bin, same_size);
    fs::File::options().write(true).open(&bin).unwrap().set_modified(good_mtime).unwrap();
    let o = ess_path(&pin, &bin, &nowhere).output().unwrap();
    let err = String::from_utf8_lossy(&o.stderr).to_string();
    assert!(!o.status.success() && err.contains("ess_pin_mismatch") && err.contains(&sha_hex(same_size.as_bytes())), "{}", says(&o));

    executable(&bin, good);
    let o = ess_path(&pin, &bin, &nowhere).output().unwrap();
    assert!(o.status.success(), "{}", says(&o));
}

/// `sdc-conform ess-path` writes nothing below the home directory: the home directory below is
/// empty afterwards (the step stays inside the tree and its scratch).
#[test]
fn the_pin_check_writes_nothing_below_the_home_directory() {
    let dir = tmp("writes-nothing");
    let (bin, home) = (dir.join("ess"), dir.join("home"));
    executable(&bin, PINNED);
    let pin = pin_of(&dir, &sha_hex(PINNED.as_bytes()));
    fs::create_dir_all(&home).unwrap();
    std::thread::sleep(Duration::from_millis(250));
    for _ in 0..2 {
        let o = Command::new(BIN).arg("ess-path").arg("--pin").arg(&pin).env("ESS_BIN", &bin).env("HOME", &home).env_remove("XDG_CACHE_HOME").output().unwrap();
        assert!(o.status.success(), "{}", says(&o));
    }
    let left: Vec<PathBuf> = fs::read_dir(&home).unwrap().flatten().map(|e| e.path()).collect();
    assert!(left.is_empty(), "the check wrote below the home directory: {left:?}");
}

// ---- what a model-written build step can reach: the files `sdc-conform` reads -----------------

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/conformance")
}

pub(crate) fn stub() -> PathBuf {
    let p = Path::new(BIN).parent().unwrap().join("examples/conformance-stub-product");
    assert!(p.is_file(), "{} is missing: cargo build --examples builds it", p.display());
    p
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().flatten() {
        let d = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy(&e.path(), &d)
        } else {
            fs::copy(e.path(), &d).unwrap();
        }
    }
}

pub(crate) fn table() -> J {
    serde_json::from_str(&fs::read_to_string(fixtures().join("table.json")).unwrap()).unwrap()
}

/// The installed pinned ess, found as `resolve_ess` finds it (`ESS_BIN`, else the cache path), with
/// no digest taken here: `sdc-conform observe` below takes it.
pub(crate) fn installed_ess(pin: &J) -> PathBuf {
    let var = |n: &str| std::env::var_os(n).filter(|v| !v.is_empty()).map(PathBuf::from);
    var("ESS_BIN").unwrap_or_else(|| {
        var("SDC_ESS_CACHE")
            .or_else(|| var("XDG_CACHE_HOME").map(|d| d.join("sdc/ess")))
            .unwrap_or_else(|| var("HOME").unwrap().join(".cache/sdc/ess"))
            .join(pin["version"].as_str().unwrap())
            .join(triple())
            .join("ess")
    })
}

/// `sdc-conform observe` runs the build step of the delivered tree, code a model wrote, as the
/// operator's user and with the operator's `HOME` (`docs/conformance.md`, "Limits";
/// `driver/src/conform.rs`, `build_product`). Before the digest cache, a build step that replaced
/// the pinned ess was found out by the next `sdc-conform`: the digest was taken at every start.
/// Here the build step of one observation puts another program in the binary's place and writes
/// the entry for it where `sdc-conform` looks by default, `$HOME/.cache/sdc/ess-digest`; the next
/// `sdc-conform ess-path` must still refuse that program. `HOME` is a directory of this test, so
/// the host's cache is not touched; the installed ess is only read, through a symbolic link.
/// One digest of the installed binary and one observation: 3 s on an idle host, about 20 s at a
/// load average of 160.
#[test]
fn a_build_step_of_an_observed_product_does_not_make_the_next_run_accept_another_ess() {
    let dir = tmp("build-step");
    let pin: J = serde_json::from_str(&fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("ess-pin.json")).unwrap()).unwrap();
    let pinned = pin["platforms"][triple()]["binary_sha256"].as_str().unwrap().to_string();
    let real = installed_ess(&pin);
    assert!(real.is_file(), "the pinned ess is not installed (run `sdc-conform ess-install` or set ESS_BIN): {}", real.display());

    let (ess, home) = (dir.join("ess"), dir.join("home"));
    std::os::unix::fs::symlink(&real, &ess).unwrap();
    fs::create_dir_all(&home).unwrap();

    let run = dir.join("run");
    let work = run.join("store/IDEA-1/work");
    copy(&fixtures().join("spec"), &work.join("spec"));
    fs::create_dir_all(work.join("published/bin")).unwrap();
    fs::copy(stub(), work.join("published/bin/stub-product")).unwrap();
    fs::write(work.join("published/README.md"), "Stub chores service.\nRun: bin/stub-product <port> <data dir> <ok>\n").unwrap();
    fs::write(work.join("published/PROVENANCE"), "stub\n").unwrap();
    // The delivered tree's build step. It needs a shell, `stat` and `sha256sum`, the path of the
    // binary and the digest the pin names (both are in the repository and in `sdc-conform`'s own
    // messages), and `$HOME`, which `sdc-conform` hands it.
    let marker = dir.join("build-step-ran").display().to_string();
    assert!(!Path::new(&marker).exists(), "the marker is there before the build step ran");
    let build = format!(
        "#!/bin/sh\nset -eu\n: > '{marker}'\ness='{ess}'\nreal=$(readlink -f \"$ess\")\nrm \"$ess\"\n\
         printf '#!/bin/sh\\necho \"$1 $2 $3\" >> \"%s.ran\"\\nexec \"%s\" \"$@\"\\n' \"$ess\" \"$real\" > \"$ess\"\nchmod 755 \"$ess\"\n\
         cache=\"$HOME/.cache/sdc/ess-digest\"\nmkdir -p \"$cache\"\nkey=$(printf '%s' \"$ess\" | sha256sum | cut -c1-32)\n\
         printf 'ess-digest/1 {pinned} 9999999999000000000\\n%s\\n%s\\n' \"$(stat -c '%s %.9Y %.9Z %i %d' \"$ess\")\" \"$ess\" > \"$cache/$key\"\n",
        ess = ess.display()
    );
    executable(&work.join("published/bin/build.sh"), &build);
    let mut t = table();
    t["build"] = json!({"argv": ["bin/build.sh"]});
    let table_file = dir.join("t.json");
    fs::write(&table_file, t.to_string()).unwrap();

    let conform = |home: &Path| {
        let mut c = Command::new(BIN);
        c.env("ESS_BIN", &ess).env("HOME", home).env_remove("SDC_ESS_DIGEST_CACHE").env_remove("XDG_CACHE_HOME");
        c
    };
    let o = conform(&home)
        .arg("observe")
        .arg(&run)
        .env("SDC_CONFORMANCE_ADAPTER", stub())
        .env("SDC_STUB_TABLE_NODE", &table_file)
        .env("SDC_CONFORMANCE_TARGET_TIMEOUT", "10")
        .env("SDC_CONFORMANCE_TABLES", "1")
        .env_remove("SDC_LLM_MODEL_CONFORMANCE")
        .env_remove("SDC_LLM_EFFORT_CONFORMANCE")
        .env_remove("SDC_LLM_EFFORT_OTHER")
        .env_remove("SDC_LLM_EFFORT")
        .output()
        .unwrap();
    assert!(o.status.success(), "the observation: {}", says(&o));
    assert!(Path::new(&marker).exists(), "the build step did not run: {}", says(&o));
    let after = sha_hex(&fs::read(&ess).unwrap());

    // the control: the same program with no cache entry (another, empty HOME) is refused, as every
    // start refused it before the cache
    let o = conform(&dir.join("empty-home")).arg("ess-path").output().unwrap();
    assert!(!o.status.success() && String::from_utf8_lossy(&o.stderr).contains("ess_pin_mismatch"), "the control, no entry: {}", says(&o));

    let o = conform(&home).arg("ess-path").output().unwrap();
    assert!(
        !o.status.success(),
        "after the build step of an observed product, a program with sha256 {after} is accepted as the pinned ess ({pinned}): {}",
        says(&o)
    );
}
