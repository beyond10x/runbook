//! Attacks on the context roots of `runbook serve` (story `context-roots-are-keys-of-runbook-toml`; `docs/daemon-api.md`,
//! "Context roots"): a root that exists is never changed and one with a group or other write bit stops the start; a
//! root that cannot be read stops the start with the OS error, exit 2; the start log says `created with mode 0700` only
//! for a root it created.

use super::state::scratch;
use super::PRINCIPAL;
use std::io::{BufRead, BufReader, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

const BIN: &str = env!("CARGO_BIN_EXE_runbook");

fn set_mode(path: &Path, mode: u32) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o7777
}

/// How a start of `runbook serve` ended: whether it listened, its exit code when it stopped by itself, and its
/// standard error.
struct Start {
    listened: bool,
    code: Option<i32>,
    err: String,
}

/// `runbook serve` with `file` as `<dir>/conf/runbook.toml`, started in `<dir>/elsewhere`. It is stopped as soon as it
/// listens; a start that is refused ends by itself.
fn serve_once(dir: &Path, file: &str, args: &[&str]) -> Start {
    std::fs::create_dir_all(dir.join("conf")).unwrap();
    std::fs::create_dir_all(dir.join("elsewhere")).unwrap();
    let config = dir.join("conf/runbook.toml");
    std::fs::write(&config, file).unwrap();
    let mut cmd = Command::new(BIN);
    cmd.current_dir(dir.join("elsewhere")).arg("serve").args(["--principal", PRINCIPAL, "--listen", "127.0.0.1:0", "--min-confinement", "local"]);
    cmd.arg("--state-dir").arg(dir.join("state")).args(args);
    cmd.env_remove("RUNBOOK_TOKEN").env("RUNBOOK_CONFIG", &config).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    let out = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = BufReader::new(out).read_line(&mut line);
        let _ = tx.send(line);
    });
    let line = rx.recv_timeout(Duration::from_secs(10)).unwrap_or_default();
    let listened = line.starts_with("runbook: listening on");
    if listened {
        let _ = child.kill();
    }
    let status = child.wait().unwrap();
    let mut err = String::new();
    child.stderr.take().unwrap().read_to_string(&mut err).unwrap();
    Start { listened, code: if listened { None } else { status.code() }, err }
}

/// A root named through a directory that does not exist and `..` (`missing/../shared`) is the existing directory
/// `shared`. With mode 0777 it is writable by group and others: the start must stop, exit 2, naming the mode, as it does
/// for `shared` named directly. The start log must not say the root was created with mode 0700.
#[test]
fn a_world_writable_root_named_through_a_missing_directory_and_dotdot_stops_the_start() {
    let dir = scratch("adv-ctxroot-dotdot");
    let shared = dir.join("conf/shared");
    std::fs::create_dir_all(&shared).unwrap();
    set_mode(&shared, 0o777);
    let s = serve_once(&dir, "context_roots = [\"missing/../shared\"]\n", &[]);
    assert_eq!(mode(&shared), 0o777);
    assert!(
        !s.listened && s.code == Some(2) && s.err.contains("is writable by group or others (mode 0777)"),
        "the existing root {} of mode 0777 was taken (listened {}, code {:?}); standard error:\n{}",
        shared.display(),
        s.listened,
        s.code,
        s.err
    );
}

/// A root that exists and cannot be read (mode 0000, owned by the daemon's user) stops the start with the OS error,
/// naming the key and the path, exit 2 (`docs/daemon-api.md`: "and so does a root that cannot be read or created,
/// with the OS error"). A user that reads any directory has nothing to show.
#[test]
fn a_root_that_cannot_be_read_stops_the_start_with_the_os_error() {
    let dir = scratch("adv-ctxroot-unreadable");
    let root = dir.join("sealed");
    std::fs::create_dir_all(&root).unwrap();
    set_mode(&root, 0o000);
    if std::fs::read_dir(&root).is_ok() {
        set_mode(&root, 0o700);
        return;
    }
    let s = serve_once(&dir, &format!("context_roots = [\"{}\"]\n", root.display()), &[]);
    set_mode(&root, 0o700);
    assert!(
        !s.listened && s.code == Some(2) && s.err.contains(&format!("context_roots {}: ", root.display())) && s.err.contains("Permission denied"),
        "a root that cannot be read was taken (listened {}, code {:?}); standard error:\n{}",
        s.listened,
        s.code,
        s.err
    );
}

/// Held, and kills the mutant `mode & 0o022` -> `mode & 0o002` that the suite let live (it showed 0777 only): a root
/// with the group write bit alone (0770) or the other write bit alone (0702) stops the start, exit 2, naming its mode.
#[test]
fn a_root_with_only_the_group_or_only_the_other_write_bit_stops_the_start() {
    let dir = scratch("adv-ctxroot-onebit");
    for m in [0o770, 0o702, 0o2770] {
        let root = dir.join(format!("root-{m:o}"));
        std::fs::create_dir_all(&root).unwrap();
        set_mode(&root, m);
        let s = serve_once(&dir, "", &["--context-root", root.to_str().unwrap()]);
        assert!(!s.listened && s.code == Some(2) && s.err.contains(&format!("(mode {m:04o})")), "mode {m:04o}: listened {}, code {:?}: {}", s.listened, s.code, s.err);
        assert_eq!(mode(&root), m, "the mode of an existing root was changed");
    }
}

/// Held: a root that is a symbolic link to a directory of mode 0777 is judged by the directory it names and stops the
/// start, exit 2; a dangling link is refused, exit 2, and nothing is created where it points.
#[test]
fn a_root_that_is_a_link_to_a_world_writable_directory_or_a_dangling_link_stops_the_start() {
    let dir = scratch("adv-ctxroot-links");
    let shared = dir.join("shared");
    std::fs::create_dir_all(&shared).unwrap();
    set_mode(&shared, 0o777);
    let link = dir.join("link-to-shared");
    std::os::unix::fs::symlink(&shared, &link).unwrap();
    let s = serve_once(&dir, &format!("context_roots = [\"{}\"]\n", link.display()), &[]);
    assert!(!s.listened && s.code == Some(2) && s.err.contains("(mode 0777)"), "listened {}, code {:?}: {}", s.listened, s.code, s.err);
    let target = dir.join("nowhere/ctx");
    let dangling = dir.join("dangling");
    std::os::unix::fs::symlink(&target, &dangling).unwrap();
    let s = serve_once(&dir, "", &["--context-root", dangling.to_str().unwrap()]);
    assert!(!s.listened && s.code == Some(2) && s.err.contains(&format!("--context-root {}: ", dangling.display())), "listened {}, code {:?}: {}", s.listened, s.code, s.err);
    assert!(!dir.join("nowhere").exists(), "a dangling link's target was created");
    assert!(!dir.join("state").exists(), "the daemon wrote its state before it refused a root");
}
