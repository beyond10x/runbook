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

/// The `PT_INTERP` program header of an ELF file, the dynamic loader a program asks for: `Ok(None)` for a static
/// program. Reads a 64-bit little-endian ELF, the binaries of both Linux platforms of the pin.
fn elf_interp(bytes: &[u8]) -> Result<Option<String>, String> {
    if bytes.len() < 64 || &bytes[..4] != b"\x7fELF" {
        return Err("not an ELF file".into());
    }
    if bytes[4] != 2 || bytes[5] != 1 {
        return Err(format!("not a 64-bit little-endian ELF (class {}, data {})", bytes[4], bytes[5]));
    }
    let u16_at = |o: usize| u16::from_le_bytes([bytes[o], bytes[o + 1]]) as usize;
    let u64_at = |o: usize| u64::from_le_bytes(bytes[o..o + 8].try_into().unwrap()) as usize;
    let (phoff, phentsize, phnum) = (u64_at(0x20), u16_at(0x36), u16_at(0x38));
    for i in 0..phnum {
        let h = i.checked_mul(phentsize).and_then(|x| x.checked_add(phoff)).filter(|h| h.checked_add(0x28).is_some_and(|e| e <= bytes.len()));
        let Some(h) = h else {
            return Err(format!("program header {i} lies past the end of the file"));
        };
        if u32::from_le_bytes(bytes[h..h + 4].try_into().unwrap()) == 3 {
            let (off, size) = (u64_at(h + 0x08), u64_at(h + 0x20));
            let raw = off.checked_add(size).and_then(|end| bytes.get(off..end)).ok_or("PT_INTERP lies past the end of the file")?;
            return Ok(Some(String::from_utf8_lossy(raw).trim_end_matches('\0').to_string()));
        }
    }
    Ok(None)
}

/// The aep the driver tests run is static: the first `aep` on the `PATH` of the tests (`task test:driver` puts the
/// directory of `sdc-conform aep-path` first, built for `<arch>-unknown-linux-musl`) has no `PT_INTERP`, so it needs
/// no glibc and runs on a host whose glibc is older than the 2.39 the gnu aep of the pin needs. A gnu aep, the trap
/// `aep` of `scripts/host-tool-traps` and no aep at all fail it.
#[cfg(target_os = "linux")]
#[test]
fn the_aep_the_driver_tests_run_is_static() {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let aep = std::env::split_paths(&path)
        .map(|d| d.join("aep"))
        .find(|p| fs::metadata(p).map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false))
        .expect("no aep on the PATH of the tests: run them with `task test:driver`");
    let bytes = fs::read(&aep).unwrap();
    match elf_interp(&bytes) {
        Ok(None) => {}
        Ok(Some(interp)) => panic!("{} asks for the loader {interp}: the driver tests run a dynamic aep, not the static musl one", aep.display()),
        Err(e) => panic!("{}: {e}: the driver tests run no aep binary of the pin (run them with `task test:driver`)", aep.display()),
    }
}

/// `elf_interp` finds the loader of a dynamic program and none in a static one.
#[cfg(target_os = "linux")]
#[test]
fn elf_interp_tells_a_static_program_from_a_dynamic_one() {
    let interp = elf_interp(&fs::read("/bin/sh").unwrap()).unwrap();
    assert!(interp.as_deref().is_some_and(|i| i.contains("ld")), "{interp:?}");
    let mut header = vec![0u8; 64];
    header[..4].copy_from_slice(b"\x7fELF");
    header[4] = 2;
    header[5] = 1;
    assert_eq!(elf_interp(&header).unwrap(), None);
    assert!(elf_interp(b"#!/bin/sh\nexit 127\n").is_err());
}

/// The forge runs the local gate's driver step for a code change. That step depends on `deps:driver-aep`, the one
/// network step: on Linux it adds the musl rustup target and installs the pinned aep with a musl `sdc-conform`, each
/// only when missing. The step itself downloads nothing: it takes the PATH directory of its tests from `aep-path` of
/// the same musl `sdc-conform`, ahead of the trap `aep`, and stops with the install command when it finds none. The
/// forge's own earlier step `aep-install` installs the gnu aep, which the tests no longer run.
#[test]
fn the_forge_and_the_local_gate_give_the_driver_tests_the_pinned_aep() {
    let wf = fs::read_to_string(repo().join(".github/workflows/pages.yml")).unwrap();
    let steps: Vec<&str> = wf.split("\n      - ").collect();
    let driver = steps.iter().position(|s| s.contains("run: task test:driver")).expect("a forge step runs `task test:driver`");
    assert!(steps[driver].contains("if: steps.changes.outputs.class == 'code'"), "{}", steps[driver]);

    let trap = fs::read_to_string(repo().join("scripts/host-tool-traps/aep")).unwrap();
    assert!(trap.contains("exit 127"), "{trap}");
    let tasks = fs::read_to_string(repo().join("Taskfile.yml")).unwrap();
    let block = |name: &str| -> String {
        let head = format!("\n  {name}:\n");
        let rest = tasks.split(head.as_str()).nth(1).unwrap_or_else(|| panic!("Taskfile.yml has no task {name}"));
        let lines: Vec<&str> = rest.lines().take_while(|l| !(l.starts_with("  ") && !l.starts_with("   "))).collect();
        format!("\n{}\n", lines.join("\n"))
    };
    let deps = block("deps:driver-aep");
    assert!(deps.contains("musl=$(uname -m)-unknown-linux-musl"), "the deps task takes the musl target of this host: {deps}");
    assert!(deps.contains("rustup target add \"$musl\""), "the deps task adds the musl target where it is missing: {deps}");
    assert!(deps.contains("--target $musl --bin sdc-conform --"), "the deps task runs a musl sdc-conform: {deps}");
    assert!(deps.contains("$conform aep-install ||"), "the deps task installs the pinned aep with it: {deps}");
    assert!(deps.contains("$conform aep-path"), "the deps task checks the installed aep against the pin: {deps}");

    let task = block("test:driver");
    assert!(task.contains("\n    deps: [deps:driver-aep]\n"), "test:driver depends on the deps task: {task}");
    assert!(task.contains("musl=$(uname -m)-unknown-linux-musl"), "the step takes the musl target of this host: {task}");
    assert!(task.contains("--target $musl --bin sdc-conform --"), "the step runs a musl sdc-conform: {task}");
    assert!(task.contains("$conform aep-path"), "{task}");
    assert!(task.contains("task deps:driver-aep"), "the step stops with the install command: {task}");
    for net in ["rustup target add", "aep-install ||", "aep-install\n", "curl"] {
        assert!(!task.contains(net), "the test step makes no network request ({net:?}): {task}");
    }
    assert!(task.contains("PATH=\"$aep_dir:$trap_dir:$PATH\""), "{task}");
}

// ---- adversary cases --------------------------------------------------------------------------

/// A 64-bit little-endian ELF header with one program header of `phentsize` bytes at `phoff`.
fn elf_header(phoff: u64, phnum: u16) -> Vec<u8> {
    let mut h = vec![0u8; 64];
    h[..4].copy_from_slice(b"\x7fELF");
    h[4] = 2;
    h[5] = 1;
    h[0x20..0x28].copy_from_slice(&phoff.to_le_bytes());
    h[0x36..0x38].copy_from_slice(&0x38u16.to_le_bytes());
    h[0x38..0x3a].copy_from_slice(&phnum.to_le_bytes());
    h
}

/// A malformed ELF whose program header offset lies past the end of the address space is refused with an error,
/// the way `elf_interp` refuses one past the end of the file, not with a panic.
#[test]
fn elf_interp_refuses_a_program_header_offset_past_the_address_space() {
    let h = elf_header(u64::MAX - 0x10, 1);
    let got = std::panic::catch_unwind(|| elf_interp(&h));
    assert!(matches!(got, Ok(Err(_))), "elf_interp panicked or accepted a program header offset of u64::MAX - 0x10: {:?}", got.as_ref().map_err(|_| "panic"));
}

/// A malformed ELF whose `PT_INTERP` offset lies past the end of the address space is refused with an error, not a
/// panic.
#[test]
fn elf_interp_refuses_an_interp_offset_past_the_address_space() {
    let mut f = elf_header(64, 1);
    let mut ph = vec![0u8; 0x38];
    ph[..4].copy_from_slice(&3u32.to_le_bytes());
    ph[0x08..0x10].copy_from_slice(&u64::MAX.to_le_bytes());
    ph[0x20..0x28].copy_from_slice(&2u64.to_le_bytes());
    f.extend(ph);
    let got = std::panic::catch_unwind(|| elf_interp(&f));
    assert!(matches!(got, Ok(Err(_))), "elf_interp panicked or accepted a PT_INTERP offset of u64::MAX: {:?}", got.as_ref().map_err(|_| "panic"));
}

/// `task test:driver` now runs `aep-install` itself when `aep-path` finds no pinned aep, so the gates of two
/// checkouts on one host that start after a pin change install into the one cache (`~/.cache/sdc/aep/<version>/
/// <triple>/`) at the same time. Every one of them succeeds and leaves the pinned binary.
#[test]
fn installs_into_one_cache_at_the_same_time_all_succeed() {
    let d = tmp("concurrent");
    let t = triple();
    let top = format!("aep-9.9.9-{t}");
    let src = d.join("src");
    fs::create_dir_all(src.join(&top)).unwrap();
    let mut body = b"#!/bin/sh\necho 'aep 9.9.9'\nexit 0\n".to_vec();
    body.extend(std::iter::repeat_n(b'#', 2 << 20));
    body.push(b'\n');
    let bin = src.join(&top).join("aep");
    fs::write(&bin, &body).unwrap();
    fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
    let archive = d.join(format!("{top}.tar.gz"));
    assert!(Command::new("tar").arg("-czf").arg(&archive).arg("-C").arg(&src).arg(&top).status().unwrap().success());
    let pin = d.join("aep-pin.json");
    let platform = json!({
        "asset": format!("{top}.tar.gz"),
        "archive_sha256": sha_hex(&fs::read(&archive).unwrap()),
        "member": format!("{top}/aep"),
        "binary_sha256": sha_hex(&body),
    });
    fs::write(&pin, json!({"format": "sdc.aep-pin/1", "version": "9.9.9", "platforms": {t: platform}}).to_string()).unwrap();
    let mut failed = vec![];
    for round in 0..3 {
        let cache = d.join(format!("cache-{round}"));
        let kids: Vec<_> = (0..4)
            .map(|_| {
                Command::new(BIN)
                    .args(["aep-install", "--archive", archive.to_str().unwrap()])
                    .arg("--pin")
                    .arg(&pin)
                    .env("SDC_AEP_CACHE", &cache)
                    .env_remove("XDG_CACHE_HOME")
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .unwrap()
            })
            .collect();
        for k in kids {
            let o = k.wait_with_output().unwrap();
            if !o.status.success() {
                failed.push(format!("round {round}: {}", says(&o)));
            }
        }
    }
    assert!(failed.is_empty(), "{} of 12 installs (3 rounds of 4 at once) failed: {failed:#?}", failed.len());
}
