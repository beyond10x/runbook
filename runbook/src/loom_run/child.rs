//! The `b10x-loom` child: an environment the runner builds itself, one call at a time, ended by
//! a signal or by the wall-time cap.

use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// What one call of `b10x-loom` left.
#[derive(Debug)]
pub struct ChildResult {
    /// The exit code; `None` when the child was ended by a signal.
    pub code: Option<i32>,
    /// What it printed on standard output.
    pub stdout: Vec<u8>,
    /// What it printed on standard error.
    pub stderr: Vec<u8>,
    /// Why the runner ended the child early, if it did.
    pub ended_by: Option<EndedBy>,
}

/// Why the runner ended a running child.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndedBy {
    /// SIGTERM or SIGINT reached the runner.
    Signal,
    /// The wall-time deadline passed.
    WallTime,
}

/// The environment of every `b10x-loom` child, built by the runner: `PATH` (the runner's own,
/// `/usr/bin:/bin` when it has none), `HOME` and `XDG_DATA_HOME` inside the run directory, and
/// `RUNBOOK_NO_CONFIG`: a start of `runbook` from inside a run reads no `runbook.toml` (`config.rs`),
/// whatever a program of the run wrote into the tree. Nothing else is passed.
pub fn child_env(run_dir: &Path) -> Vec<(String, String)> {
    let path = std::env::var("PATH").ok().filter(|p| !p.is_empty()).unwrap_or_else(|| "/usr/bin:/bin".to_owned());
    let home = run_dir.join("home");
    vec![
        ("PATH".to_owned(), path),
        ("HOME".to_owned(), home.display().to_string()),
        ("XDG_DATA_HOME".to_owned(), home.join("data").display().to_string()),
        (crate::config::ENV_NO_CONFIG.to_owned(), "1".to_owned()),
    ]
}

fn pump(mut r: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = r.read_to_end(&mut buf);
        buf
    })
}

fn signal_group(pid: u32, sig: i32) {
    // SAFETY: kill(2) with a process group id of a child this function started.
    unsafe {
        libc::kill(-(pid as i32), sig);
    }
}

/// Runs `loom args...` with exactly `env` (cleared first), working directory `cwd`; waits for it,
/// ending its process group when `cancel` is set (SIGTERM, then SIGKILL after 3 s) or when
/// `deadline_secs` (seconds since the Unix epoch on `now`) has passed.
pub fn run(
    loom: &Path,
    args: &[String],
    env: &[(String, String)],
    cwd: &Path,
    cancel: &AtomicBool,
    deadline_secs: u64,
    now: &dyn Fn() -> u64,
) -> Result<ChildResult, String> {
    let mut child = Command::new(loom)
        .args(args)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|e| format!("cannot start b10x-loom: {e}"))?;
    let pid = child.id();
    let out = pump(child.stdout.take().ok_or("no stdout pipe")?);
    let err = pump(child.stderr.take().ok_or("no stderr pipe")?);
    let mut ended_by = None;
    let status = loop {
        match child.try_wait().map_err(|e| format!("cannot wait for b10x-loom: {e}"))? {
            Some(status) => break status,
            None => {
                if cancel.load(Ordering::SeqCst) {
                    ended_by = Some(EndedBy::Signal);
                } else if now() >= deadline_secs {
                    ended_by = Some(EndedBy::WallTime);
                }
                if ended_by.is_some() {
                    signal_group(pid, libc::SIGTERM);
                    let start = Instant::now();
                    loop {
                        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
                            break;
                        }
                        if start.elapsed() > Duration::from_secs(3) {
                            signal_group(pid, libc::SIGKILL);
                            let _ = child.wait();
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    signal_group(pid, libc::SIGKILL);
                    break child.wait().map_err(|e| e.to_string())?;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    };
    Ok(ChildResult {
        code: status.code(),
        stdout: out.join().unwrap_or_default(),
        stderr: err.join().unwrap_or_default(),
        ended_by,
    })
}
