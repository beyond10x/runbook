//! The pin of the `aep` binary (`driver/aep-pin.json`, docs/forge-gate.md): the driver tests use aep
//! as the planning store of the programs they start, and the forge has no aep on `PATH`. The forge
//! and the local gate both install the pinned release (`sdc-conform aep-install`) and
//! `task test:driver` puts its directory first on the `PATH` of the tests (`sdc-conform aep-path`),
//! ahead of the trap `aep` of `scripts/host-tool-traps`. The host's aep is never used.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{json, Value as J};
use sha2::{Digest, Sha256};

const BIN: &str = env!("CARGO_BIN_EXE_sdc-conform");

fn tmp(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("aep-pin").join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn sha_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn says(o: &Output) -> String {
    format!("exit {:?} stdout {} stderr {}", o.status.code(), String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

fn triple() -> String {
    let arch = std::env::consts::ARCH;
    if cfg!(target_os = "macos") { format!("{arch}-apple-darwin") } else { format!("{arch}-unknown-linux-gnu") }
}

/// A release of aep 9.9.9 for this host in `dir`: the archive `aep-9.9.9-<triple>.tar.gz` with
/// the member `aep-9.9.9-<triple>/aep`, a script that says `aep 9.9.9`. Returns (archive, pin).
fn release(dir: &Path) -> (PathBuf, PathBuf) {
    let t = triple();
    let top = format!("aep-9.9.9-{t}");
    let src = dir.join("src");
    fs::create_dir_all(src.join(&top)).unwrap();
    let body = "#!/bin/sh\necho 'aep 9.9.9'\n";
    let bin = src.join(&top).join("aep");
    fs::write(&bin, body).unwrap();
    fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
    let archive = dir.join(format!("{top}.tar.gz"));
    let st = Command::new("tar").arg("-czf").arg(&archive).arg("-C").arg(&src).arg(&top).status().unwrap();
    assert!(st.success());
    let pin = dir.join("aep-pin.json");
    let platform = json!({
        "asset": format!("{top}.tar.gz"),
        "archive_sha256": sha_hex(&fs::read(&archive).unwrap()),
        "member": format!("{top}/aep"),
        "binary_sha256": sha_hex(body.as_bytes()),
    });
    fs::write(&pin, json!({"format": "sdc.aep-pin/1", "version": "9.9.9", "platforms": {t: platform}}).to_string()).unwrap();
    (archive, pin)
}

fn conform(args: &[&str], pin: &Path, cache: &Path) -> Output {
    Command::new(BIN).args(args).arg("--pin").arg(pin).env("SDC_AEP_CACHE", cache).env_remove("XDG_CACHE_HOME").output().unwrap()
}

/// The pin names one release of aep: its version, the commit of its tag and, for each platform,
/// the archive and the binary inside it by their digests.
#[test]
fn the_repository_pins_one_release_of_aep() {
    let pin: J = serde_json::from_str(&fs::read_to_string(repo().join("driver/aep-pin.json")).unwrap()).unwrap();
    assert_eq!(pin["format"], "sdc.aep-pin/1");
    let version = pin["version"].as_str().unwrap();
    assert!(!version.is_empty() && version.split('.').all(|p| p.parse::<u32>().is_ok()), "{version}");
    let commit = pin["commit"].as_str().unwrap();
    assert!(commit.len() == 40 && commit.chars().all(|c| c.is_ascii_hexdigit()), "{commit}");
    assert_eq!(pin["release"].as_str().unwrap(), format!("https://github.com/beyond10x/aep/releases/tag/{version}"));
    let platforms = pin["platforms"].as_object().unwrap();
    assert!(platforms.contains_key("x86_64-unknown-linux-gnu"), "the forge's runner: {platforms:?}");
    for (t, p) in platforms {
        assert_eq!(p["asset"].as_str().unwrap(), format!("aep-{version}-{t}.tar.gz"));
        assert_eq!(p["member"].as_str().unwrap(), format!("aep-{version}-{t}/aep"));
        for k in ["archive_sha256", "binary_sha256"] {
            let h = p[k].as_str().unwrap();
            assert!(h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit()), "{t} {k}: {h}");
        }
    }
}

/// A static musl build of `sdc-conform` (the musl release archive, for a host without glibc) takes the pin's
/// `<arch>-unknown-linux-musl` archive of aep for both Linux architectures, and a gnu build the `-linux-gnu` one
/// (`conformance::platform_for`): a pin that names no musl archive gives a musl build the gnu aep, which needs glibc.
#[test]
fn a_musl_build_takes_the_pinned_musl_aep() {
    use sdc_tools::conformance::platform_for;
    let pin: J = serde_json::from_str(&fs::read_to_string(repo().join("driver/aep-pin.json")).unwrap()).unwrap();
    let mut bad = vec![];
    for arch in ["x86_64", "aarch64"] {
        for (musl, libc) in [(true, "musl"), (false, "gnu")] {
            let want = format!("{arch}-unknown-linux-{libc}");
            let got = platform_for(arch, "linux", musl, &pin);
            if got != want {
                bad.push(format!("{arch} musl={musl}: {got}, want {want}"));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// `aep-install` unpacks the pinned binary from the archive into the cache, and `aep-path` names
/// that binary: the directory `task test:driver` puts first on the `PATH` of the tests.
#[test]
fn the_installed_pinned_aep_is_the_one_aep_path_names() {
    let d = tmp("install");
    let (archive, pin) = release(&d);
    let cache = d.join("cache");
    let want = cache.join("9.9.9").join(triple()).join("aep");
    let o = conform(&["aep-install", "--archive", archive.to_str().unwrap()], &pin, &cache);
    assert!(o.status.success(), "{}", says(&o));
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), want.display().to_string());
    let o = conform(&["aep-path"], &pin, &cache);
    assert!(o.status.success(), "{}", says(&o));
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), want.display().to_string());
    let o = Command::new(&want).arg("--version").output().unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "aep 9.9.9");
}

/// Without the pinned binary `aep-path` exits 1 and says how to install it: the driver step stops
/// there, it does not fall back to an aep found elsewhere.
#[test]
fn without_the_pinned_aep_aep_path_fails_with_the_install_command() {
    let d = tmp("missing");
    let (_, pin) = release(&d);
    let o = conform(&["aep-path"], &pin, &d.join("cache"));
    assert_eq!(o.status.code(), Some(1), "{}", says(&o));
    assert!(o.stdout.is_empty(), "{}", says(&o));
    assert!(String::from_utf8_lossy(&o.stderr).contains("run `sdc-conform aep-install`"), "{}", says(&o));
}

/// An archive whose digest is not the pinned one installs nothing.
#[test]
fn an_archive_with_another_digest_installs_nothing() {
    let d = tmp("archive-digest");
    let (archive, pin) = release(&d);
    let mut doc: J = serde_json::from_str(&fs::read_to_string(&pin).unwrap()).unwrap();
    doc["platforms"][triple()]["archive_sha256"] = json!("0".repeat(64));
    fs::write(&pin, doc.to_string()).unwrap();
    let cache = d.join("cache");
    let o = conform(&["aep-install", "--archive", archive.to_str().unwrap()], &pin, &cache);
    assert_eq!(o.status.code(), Some(1), "{}", says(&o));
    assert!(String::from_utf8_lossy(&o.stderr).contains("the pin 0000"), "{}", says(&o));
    assert!(!cache.join("9.9.9").join(triple()).join("aep").exists());
}

/// A binary in the cache whose bytes or version are not the pinned ones is refused by `aep-path`.
#[test]
fn a_cached_binary_that_is_not_the_pinned_one_is_refused() {
    let d = tmp("changed");
    let (archive, pin) = release(&d);
    let cache = d.join("cache");
    let o = conform(&["aep-install", "--archive", archive.to_str().unwrap()], &pin, &cache);
    assert!(o.status.success(), "{}", says(&o));
    let bin = cache.join("9.9.9").join(triple()).join("aep");
    fs::write(&bin, "#!/bin/sh\necho 'aep 9.9.8'\n").unwrap();
    let o = conform(&["aep-path"], &pin, &cache);
    assert_eq!(o.status.code(), Some(1), "{}", says(&o));
    assert!(String::from_utf8_lossy(&o.stderr).contains("has sha256"), "{}", says(&o));

    let body = "#!/bin/sh\necho 'aep 9.9.8'\n";
    let mut doc: J = serde_json::from_str(&fs::read_to_string(&pin).unwrap()).unwrap();
    doc["platforms"][triple()]["binary_sha256"] = json!(sha_hex(body.as_bytes()));
    fs::write(&pin, doc.to_string()).unwrap();
    let o = conform(&["aep-path"], &pin, &cache);
    assert_eq!(o.status.code(), Some(1), "{}", says(&o));
    assert!(String::from_utf8_lossy(&o.stderr).contains("the pin is 9.9.9"), "{}", says(&o));
}

/// The forge installs the pinned aep for a code change before its driver step, and the driver
/// step takes the PATH directory of its tests from `aep-path`, ahead of the trap `aep`.
#[test]
fn the_forge_and_the_local_gate_give_the_driver_tests_the_pinned_aep() {
    let wf = fs::read_to_string(repo().join(".github/workflows/pages.yml")).unwrap();
    let steps: Vec<&str> = wf.split("\n      - ").collect();
    let install = steps.iter().position(|s| s.contains("--bin sdc-conform -- aep-install")).expect("a forge step runs `sdc-conform aep-install`");
    let driver = steps.iter().position(|s| s.contains("run: task test:driver")).expect("a forge step runs `task test:driver`");
    assert!(install < driver, "the pinned aep is installed before the driver step");
    assert!(steps[install].contains("if: steps.changes.outputs.class == 'code'"), "{}", steps[install]);

    let trap = fs::read_to_string(repo().join("scripts/host-tool-traps/aep")).unwrap();
    assert!(trap.contains("exit 127"), "{trap}");
    let tasks = fs::read_to_string(repo().join("Taskfile.yml")).unwrap();
    let task = tasks.split("\n  test:driver:\n").nth(1).unwrap().split("\n  test:runbook:\n").next().unwrap();
    assert!(task.contains("--bin sdc-conform -- aep-path"), "{task}");
    assert!(task.contains("PATH=\"$aep_dir:$trap_dir:$PATH\""), "{task}");
}
