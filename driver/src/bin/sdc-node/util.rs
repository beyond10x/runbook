//! Small helpers the port of `llm-node.sh` needs: exits, output sinks, child processes, copies,
//! the UTC clock.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// A program error whose message is already printed; the value is the exit code.
#[derive(Debug)]
pub struct Exit(pub i32);

/// A step that can end the program.
pub type R<T> = Result<T, Exit>;

/// Prints `msg` on standard error and returns the exit.
pub fn die(msg: impl AsRef<str>, code: i32) -> Exit {
    let _ = writeln!(io::stderr(), "{}", msg.as_ref());
    Exit(code)
}

/// Where the output of a step goes: the process's own streams, or one log file for both.
pub struct Out {
    log: Option<File>,
}

impl Out {
    /// Standard output and standard error of the process.
    pub fn inherit() -> Out {
        Out { log: None }
    }

    /// The file `path`, truncated (`> path 2>&1`).
    pub fn truncate(path: &Path) -> Out {
        Out { log: File::create(path).ok() }
    }

    /// The file `path`, appended to (`>> path 2>&1`); created when absent.
    pub fn append(path: &Path) -> Out {
        Out { log: OpenOptions::new().create(true).append(true).open(path).ok() }
    }

    /// One line on standard output, or in the log.
    pub fn line(&mut self, bytes: &[u8]) {
        let mut b = bytes.to_vec();
        b.push(b'\n');
        match &mut self.log {
            Some(f) => {
                let _ = f.write_all(&b);
            }
            None => {
                let _ = io::stdout().write_all(&b);
            }
        }
    }

    /// One line on standard error, or in the log.
    pub fn err_line(&mut self, text: &str) {
        match &mut self.log {
            Some(f) => {
                let _ = writeln!(f, "{text}");
            }
            None => {
                let _ = writeln!(io::stderr(), "{text}");
            }
        }
    }

    fn stdio(&self) -> (Stdio, Stdio) {
        match &self.log {
            Some(f) => match (f.try_clone(), f.try_clone()) {
                (Ok(a), Ok(b)) => (Stdio::from(a), Stdio::from(b)),
                _ => (Stdio::null(), Stdio::null()),
            },
            None => (Stdio::inherit(), Stdio::inherit()),
        }
    }
}

/// The exit code of a finished child as bash reports it: a signal is 128 plus its number.
pub fn code_of(status: std::process::ExitStatus) -> i32 {
    status.code().unwrap_or_else(|| 128 + status.signal().unwrap_or(0))
}

/// Starts `cmd` with standard input closed and its output in `out`; its exit code. A program that
/// cannot be started is 127 (absent) or 126, with a line on `out`'s error stream.
pub fn run_cmd(cmd: &mut Command, out: &mut Out) -> i32 {
    let (o, e) = out.stdio();
    cmd.stdin(Stdio::null()).stdout(o).stderr(e);
    match cmd.status() {
        Ok(s) => code_of(s),
        Err(err) => {
            out.err_line(&format!("llm-node: {}: {err}", cmd.get_program().to_string_lossy()));
            if err.kind() == io::ErrorKind::NotFound { 127 } else { 126 }
        }
    }
}

/// Starts `cmd` with its standard output in the file `stdout` (truncated) and its standard error
/// inherited; its exit code.
pub fn run_to_file(cmd: &mut Command, stdout: &Path) -> i32 {
    let o = match File::create(stdout) {
        Ok(f) => Stdio::from(f),
        Err(_) => Stdio::null(),
    };
    cmd.stdin(Stdio::null()).stdout(o).stderr(Stdio::inherit());
    match cmd.status() {
        Ok(s) => code_of(s),
        Err(err) => {
            let _ = writeln!(io::stderr(), "llm-node: {}: {err}", cmd.get_program().to_string_lossy());
            if err.kind() == io::ErrorKind::NotFound { 127 } else { 126 }
        }
    }
}

/// `cp -r src dst` where `dst` does not exist: a symbolic link stays a link.
pub fn cp_r(src: &Path, dst: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(src)?;
    if meta.file_type().is_symlink() {
        std::os::unix::fs::symlink(fs::read_link(src)?, dst)
    } else if meta.is_dir() {
        fs::create_dir_all(dst)?;
        let mut first = Ok(());
        for e in fs::read_dir(src)? {
            let e = e?;
            if let Err(err) = cp_r(&e.path(), &dst.join(e.file_name())) {
                if first.is_ok() {
                    first = Err(err);
                }
            }
        }
        first
    } else {
        fs::copy(src, dst).map(|_| ())
    }
}

/// `cp -r src/. dst/`: the contents of the directory `src` into the directory `dst`.
pub fn cp_contents(src: &Path, dst: &Path) -> io::Result<()> {
    let mut first = Ok(());
    for e in fs::read_dir(src)? {
        let e = e?;
        if let Err(err) = cp_r(&e.path(), &dst.join(e.file_name())) {
            if first.is_ok() {
                first = Err(err);
            }
        }
    }
    first
}

/// `rm -rf path`.
pub fn rm_rf(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(_) => Ok(()),
    }
}

/// `command -v name` on PATH.
pub fn which(name: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0))
}

/// Whether `path` is an executable regular file (`[[ -x ]]` and not a directory).
pub fn is_exec(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// The first 16 hex digits of the SHA-256 of the file (`sha256sum < f | cut -c1-16`); empty when
/// the file cannot be read.
pub fn sha16(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    match fs::read(path) {
        Ok(b) => Sha256::digest(&b).iter().take(8).map(|x| format!("{x:02x}")).collect(),
        Err(_) => String::new(),
    }
}

/// `date -u +%Y-%m-%dT%H:%M:%SZ`, or with `.%3N` milliseconds.
pub fn now_utc(millis: bool) -> String {
    let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = d.as_secs() as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let (h, m, s) = (rem / 3600, rem % 3600 / 60, rem % 60);
    if millis {
        format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}.{:03}Z", d.subsec_millis())
    } else {
        format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
    }
}

/// Command substitution: the trailing newlines are dropped.
pub fn chomp(s: &str) -> &str {
    s.trim_end_matches('\n')
}

/// `[[ -n ${NAME:-} ]]`: the variable is set and not empty.
pub fn env_set(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// Starts `cmd` with standard input closed, its standard error in `out` and its standard output
/// captured; the exit code and the bytes of standard output.
pub fn run_capture(cmd: &mut Command, out: &mut Out) -> (i32, Vec<u8>) {
    let (_, e) = out.stdio();
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(e);
    match cmd.output() {
        Ok(o) => (code_of(o.status), o.stdout),
        Err(err) => {
            out.err_line(&format!("llm-node: {}: {err}", cmd.get_program().to_string_lossy()));
            (if err.kind() == io::ErrorKind::NotFound { 127 } else { 126 }, Vec::new())
        }
    }
}

/// Starts `cmd` with standard input closed, its standard output in the file `stdout` (truncated)
/// and its standard error appended to the file `stderr`; its exit code.
pub fn run_split(cmd: &mut Command, stdout: &Path, stderr: &Path) -> i32 {
    let o = File::create(stdout).map_or_else(|_| Stdio::null(), Stdio::from);
    let log = OpenOptions::new().create(true).append(true).open(stderr).ok();
    let e = log.as_ref().and_then(|f| f.try_clone().ok()).map_or_else(Stdio::null, Stdio::from);
    cmd.stdin(Stdio::null()).stdout(o).stderr(e);
    match cmd.status() {
        Ok(s) => code_of(s),
        Err(err) => {
            if let Some(mut f) = log {
                let _ = writeln!(f, "llm-node: {}: {err}", cmd.get_program().to_string_lossy());
            }
            if err.kind() == io::ErrorKind::NotFound { 127 } else { 126 }
        }
    }
}
