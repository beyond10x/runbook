//! sdc-recorded-pool: the stand-in `slot` of the cycles suite (`task test:cycles-loom`,
//! docs/cycles-suite.md, "Recorded build and test results"). The suite's task puts a link named
//! `slot` to this program first on the PATH of `runbook test`, which Loom gives every bound
//! program, so the build and test commands that `sdc-tool.sh` (`pool_run`) and `sdc-bugfix` start
//! through the pool are answered here and no cycle builds anything.
//!
//!   slot [slot options] [run] [--] <command> <args>...
//!
//! The command's answer is a recording, `<recordings>/<digest>.json` (format `sdc.recorded-pool/1`:
//! `argv`, `exit`, `stdout`, `stderr`), where the digest is the sha256 of the command's argv and of
//! the tree it runs in (the working directory: every file's path, kind and bytes; `target/`, `.git`
//! and the CARGO_TARGET_DIR inside it left out). A recording prints its stdout and stderr, with `{dir}` read
//! as the working directory and `{target}` as CARGO_TARGET_DIR, and exits with its exit. A command
//! with no recording prints one line that starts `error:` and names the digest, and exits 127 (a
//! program error, never a test result). `slot status` and `slot cancel` print nothing: no job waits.
//!
//! Record mode (`task test:cycles-record`): when the PATH directory this program was found through
//! holds a file named `record`, a command with no recording runs through the next `slot` on PATH
//! (with HOME from the user's passwd entry when the caller has none: Loom gives a program none), in a
//! copy of the tree under the repository's `target/cycles-recording/`, which the pool syncs. What
//! it printed is written as the recording: slot's own `slot: ` lines dropped, durations (`in 0.68s`)
//! written `in 0.00s`, the copy's path written `{dir}` and its target `{target}`. A recording that
//! would still name the home directory is not written. Then it is replayed as above.
//!
//! The recordings are `cycles/recorded/` of the repository whose `driver/target/` holds this
//! program, and the copies go under its `target/cycles-recording/`; SDC_RECORDINGS and
//! SDC_RECORDING_WORK name others (the tests of `driver/tests/recorded_pool.rs`).
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use regex::Regex;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const FORMAT: &str = "sdc.recorded-pool/1";
/// A command this program cannot answer: a program error to `sdc-tool.sh` and `sdc-bugfix`.
const NOT_ANSWERED: u8 = 127;
/// slot's options that take a value (as the host's `slot` wrapper reads them).
const VALUED: [&str; 6] = ["--timeout", "--tokens", "--priority", "--fetch", "--memory-max", "--label"];

type R<T> = Result<T, String>;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = command(&args);
    match cmd.first().map(String::as_str) {
        None => {
            eprintln!("error: recorded pool: no command (slot [options] [run] [--] <command> <args>...)");
            return ExitCode::from(2);
        }
        Some("status" | "cancel") => return ExitCode::SUCCESS,
        _ => {}
    }
    match answer(&args, &cmd) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("error: recorded pool: {}", e.replace('\n', " "));
            ExitCode::from(NOT_ANSWERED)
        }
    }
}

/// The command after slot's own options, `run` and `--`.
fn command(args: &[String]) -> Vec<String> {
    let mut i = 0;
    let mut run = false;
    while i < args.len() {
        let a = args[i].as_str();
        if VALUED.contains(&a) {
            i += 2;
        } else if a == "--" {
            i += 1;
            break;
        } else if a == "run" && !run {
            run = true;
            i += 1;
        } else if a.starts_with('-') {
            i += 1;
        } else {
            break;
        }
    }
    args.get(i..).unwrap_or_default().to_vec()
}

fn answer(args: &[String], cmd: &[String]) -> R<u8> {
    let cwd = std::env::current_dir().map_err(|e| format!("the working directory: {e}"))?;
    let target = std::env::var_os("CARGO_TARGET_DIR").filter(|v| !v.is_empty()).map(PathBuf::from);
    let key = digest(cmd, &cwd, target.as_deref())?;
    let recordings = match std::env::var_os("SDC_RECORDINGS").filter(|v| !v.is_empty()) {
        Some(d) => PathBuf::from(d),
        None => root()?.join("cycles/recorded"),
    };
    let file = recordings.join(format!("{key}.json"));
    if !file.exists() {
        let Some(real) = recording_pool()? else {
            return Err(format!(
                "no recording {key} for `{}` in {}: no recorded result for this command on this tree; record it with `task test:cycles-record` (docs/cycles-suite.md)",
                cmd.join(" "),
                cwd.display()
            ));
        };
        record(args, cmd, &cwd, target.as_deref(), &key, &real, &file)?;
    }
    replay(&file, &cwd, target.as_deref())
}

/// The repository: the nearest directory above this program that holds `cycles/tree-suite.yaml`.
fn root() -> R<PathBuf> {
    let exe = std::env::current_exe().map_err(|e| format!("this program's path: {e}"))?;
    exe.ancestors()
        .find(|d| d.join("cycles/tree-suite.yaml").is_file())
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("no repository above {} (a directory with cycles/tree-suite.yaml)", exe.display()))
}

/// In record mode, the `slot` to run a command through: the next one on PATH after the directory
/// this program was found through, when that directory holds a file named `record`. None otherwise.
fn recording_pool() -> R<Option<PathBuf>> {
    let me = std::env::current_exe().and_then(fs::canonicalize).map_err(|e| format!("this program's path: {e}"))?;
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut found = false;
    for d in std::env::split_paths(&path) {
        let s = d.join("slot");
        let Ok(m) = fs::metadata(&s) else { continue };
        if !m.is_file() || m.permissions().mode() & 0o111 == 0 {
            continue;
        }
        let mine = fs::canonicalize(&s).is_ok_and(|c| c == me);
        if !found {
            if !mine {
                return Ok(None);
            }
            if !d.join("record").is_file() {
                return Ok(None);
            }
            found = true;
        } else if !mine {
            return Ok(Some(s));
        }
    }
    if found {
        Err("record mode: no other `slot` on PATH after this one".into())
    } else {
        Ok(None)
    }
}

/// The sha256 (hex) of the command and of the tree `dir`, `target/` and `skip` left out.
fn digest(cmd: &[String], dir: &Path, skip: Option<&Path>) -> R<String> {
    let mut h = Sha256::new();
    h.update(FORMAT.as_bytes());
    h.update([0]);
    for a in cmd {
        h.update(a.as_bytes());
        h.update([0]);
    }
    h.update([1]);
    let mut files = Vec::new();
    walk(dir, dir, skip, &mut files)?;
    files.sort();
    for rel in files {
        let p = dir.join(OsStr::from_bytes(&rel));
        let m = fs::symlink_metadata(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        if m.file_type().is_symlink() {
            let to = fs::read_link(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            h.update(b"l");
            h.update(&rel);
            h.update([0]);
            h.update(to.as_os_str().as_bytes());
            h.update([0]);
        } else {
            let bytes = fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            h.update(if m.mode() & 0o111 != 0 { b"x" } else { b"f" });
            h.update(&rel);
            h.update([0]);
            h.update((bytes.len() as u64).to_le_bytes());
            h.update(&bytes);
        }
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Whether `p` is left out of the tree `top`: its `target/`, its `.git` (the repository a bug-fix
/// check tree carries, docs/bugfix.md: its index and objects differ with every copy), and `skip`.
fn left_out(top: &Path, p: &Path, skip: Option<&Path>) -> bool {
    p == top.join("target") || p == top.join(".git") || skip.is_some_and(|s| s == p)
}

/// The paths of the files (and links) below `d`, relative to `top`, as bytes.
fn walk(top: &Path, d: &Path, skip: Option<&Path>, out: &mut Vec<Vec<u8>>) -> R<()> {
    for e in fs::read_dir(d).map_err(|e| format!("{}: {e}", d.display()))? {
        let e = e.map_err(|e| format!("{}: {e}", d.display()))?;
        let p = e.path();
        if left_out(top, &p, skip) {
            continue;
        }
        let t = e.file_type().map_err(|e| format!("{}: {e}", p.display()))?;
        if t.is_dir() {
            walk(top, &p, skip, out)?;
        } else {
            out.push(p.strip_prefix(top).unwrap_or(&p).as_os_str().as_bytes().to_vec());
        }
    }
    Ok(())
}

fn copy_tree(top: &Path, from: &Path, to: &Path, skip: Option<&Path>) -> R<()> {
    fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for e in fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let e = e.map_err(|e| format!("{}: {e}", from.display()))?;
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if left_out(top, &src, skip) {
            continue;
        }
        let t = e.file_type().map_err(|e| format!("{}: {e}", src.display()))?;
        if t.is_symlink() {
            let link = fs::read_link(&src).map_err(|e| format!("{}: {e}", src.display()))?;
            std::os::unix::fs::symlink(link, &dst).map_err(|e| format!("{}: {e}", dst.display()))?;
        } else if t.is_dir() {
            copy_tree(top, &src, &dst, skip)?;
        } else {
            fs::copy(&src, &dst).map_err(|e| format!("{}: {e}", src.display()))?;
        }
    }
    Ok(())
}

/// The caller's HOME, else the home directory of the user's passwd entry.
fn home() -> Option<String> {
    if let Some(h) = std::env::var("HOME").ok().filter(|h| !h.is_empty()) {
        return Some(h);
    }
    let uid = fs::metadata("/proc/self").ok()?.uid().to_string();
    let entry = |line: &str| {
        let f: Vec<&str> = line.trim_end().split(':').collect();
        (f.len() > 5 && f[2] == uid && f[5].starts_with('/')).then(|| f[5].to_owned())
    };
    // /etc/passwd first (the caller's PATH may hold no getent), then getent for a directory service.
    if let Some(h) = fs::read_to_string("/etc/passwd").ok().and_then(|t| t.lines().find_map(entry)) {
        return Some(h);
    }
    let o = Command::new("getent").args(["passwd", &uid]).stdin(Stdio::null()).output().ok()?;
    entry(&String::from_utf8(o.stdout).ok()?)
}

/// What a command printed, as a recording holds it.
fn normalized(text: &str, dir: &Path, target: Option<&Path>) -> String {
    let durations = Regex::new(r"\bin [0-9]+(\.[0-9]+)?s\b").unwrap();
    let mut out = String::new();
    for line in text.split_inclusive('\n') {
        if line.starts_with("slot: ") {
            continue;
        }
        let mut l = line.to_owned();
        if let Some(t) = target {
            l = l.replace(&t.display().to_string(), "{target}");
        }
        l = l.replace(&dir.display().to_string(), "{dir}");
        out.push_str(&durations.replace_all(&l, "in 0.00s"));
    }
    out
}

fn record(args: &[String], cmd: &[String], cwd: &Path, target: Option<&Path>, key: &str, real: &Path, file: &Path) -> R<()> {
    let work_root = match std::env::var_os("SDC_RECORDING_WORK").filter(|v| !v.is_empty()) {
        Some(d) => PathBuf::from(d),
        None => root()?.join("target/cycles-recording"),
    };
    let work = work_root.join(key);
    let _ = fs::remove_dir_all(&work);
    let inside = target.and_then(|t| t.strip_prefix(cwd).ok()).map(Path::to_path_buf);
    copy_tree(cwd, cwd, &work.join("tree"), target)?;
    let copy = fs::canonicalize(work.join("tree")).map_err(|e| format!("{}: {e}", work.display()))?;
    let rec_target = match (target, inside) {
        (Some(_), Some(rel)) => Some(copy.join(rel)),
        (Some(_), None) => Some(work.join("target")),
        (None, _) => None,
    };
    let mut c = Command::new(real);
    c.args(args).current_dir(&copy).stdin(Stdio::null());
    if let Some(t) = &rec_target {
        c.env("CARGO_TARGET_DIR", t);
    }
    let home = home();
    if std::env::var("HOME").map_or(true, |h| h.is_empty()) {
        if let Some(h) = &home {
            c.env("HOME", h);
        }
    }
    let o = c.output().map_err(|e| format!("record mode: {} could not start: {e}", real.display()))?;
    let _ = fs::remove_dir_all(&work);
    let Some(code) = o.status.code() else {
        return Err(format!("record mode: `{}` ended on a signal; nothing recorded", cmd.join(" ")));
    };
    let stdout = normalized(&String::from_utf8_lossy(&o.stdout), &copy, rec_target.as_deref());
    let stderr = normalized(&String::from_utf8_lossy(&o.stderr), &copy, rec_target.as_deref());
    if let Some(h) = home.filter(|h| h.len() > 1) {
        if let Some(l) = stdout.lines().chain(stderr.lines()).find(|l| l.contains(&format!("{h}/"))) {
            return Err(format!("record mode: the output of `{}` names the home directory; not recorded: {l}", cmd.join(" ")));
        }
    }
    let rec = json!({"format": FORMAT, "argv": cmd, "exit": code, "stdout": stdout, "stderr": stderr});
    let dir = file.parent().ok_or("the recordings directory")?;
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let tmp = dir.join(format!(".{key}.json.tmp"));
    fs::write(&tmp, serde_json::to_string_pretty(&rec).unwrap() + "\n").map_err(|e| format!("{}: {e}", tmp.display()))?;
    fs::rename(&tmp, file).map_err(|e| format!("{}: {e}", file.display()))
}

fn replay(file: &Path, cwd: &Path, target: Option<&Path>) -> R<u8> {
    let text = fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let rec: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
    if rec["format"] != FORMAT {
        return Err(format!("{}: not {FORMAT}", file.display()));
    }
    let code = rec["exit"].as_u64().filter(|c| *c <= 255).ok_or_else(|| format!("{}: no exit 0 to 255", file.display()))? as u8;
    let dir = cwd.display().to_string();
    let tgt = target.map_or_else(|| cwd.join("target"), Path::to_path_buf).display().to_string();
    let fill = |k: &str| rec[k].as_str().unwrap_or("").replace("{target}", &tgt).replace("{dir}", &dir);
    let mut o = std::io::stdout().lock();
    let _ = o.write_all(fill("stdout").as_bytes());
    let _ = o.flush();
    let mut e = std::io::stderr().lock();
    let _ = e.write_all(fill("stderr").as_bytes());
    let _ = e.flush();
    Ok(code)
}
