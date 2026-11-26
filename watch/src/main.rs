//! sdc-watch: records every effective file change in an SDC run directory, attributed to the
//! driver action and the agent tool call that caused it. Record format: docs/fs-changes.md.

mod attrib;
mod held;
mod layout;
mod store;
mod timefmt;
mod watch;

use clap::{Parser, Subcommand};
use std::fs;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "sdc-watch", version, about = "Record every effective file change of an SDC run, with its attribution")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Watch until SIGTERM or SIGINT; append to <out>/changes.jsonl.
    Run {
        run_dir: PathBuf,
        /// Output dir (default <run-dir>/fs).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Kind of run directory (default: a Loom run when `settings.json` is in it).
        #[arg(long, value_enum)]
        kind: Option<Kind>,
        /// Stop, as on SIGTERM, when the process with this id is no longer the parent: the
        /// watcher does not outlive the run that started it, also when that is killed.
        #[arg(long, value_name = "PID")]
        parent: Option<i32>,
    },
    /// Write <out>/snapshot.json and the blobs of every tracked file, once; records nothing.
    Scan {
        run_dir: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        /// Kind of run directory (default: a Loom run when `settings.json` is in it).
        #[arg(long, value_enum)]
        kind: Option<Kind>,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum Kind {
    /// A run of the removed driver (`store/<case>/work`).
    Driver,
    /// A run of `runbook run` (`work/`, `rounds.jsonl`).
    Loom,
}

/// Fixes the layout for the process: the kind given, else the one the files tell.
fn set_kind(run_dir: &Path, kind: Option<Kind>) {
    let loom = match kind {
        Some(Kind::Loom) => true,
        Some(Kind::Driver) => false,
        None => run_dir.join(layout::LOOM_MARK).is_file(),
    };
    layout::set_kind(if loom { layout::Kind::Loom } else { layout::Kind::Driver });
}

/// How often a watcher with a parent looks whether the parent is still there.
const PARENT_POLL_MS: i32 = 200;

/// Arranges that SIGTERM comes when the parent dies; false when it is already gone.
fn die_with_parent(parent: i32) -> bool {
    // SAFETY: prctl with PR_SET_PDEATHSIG takes the signal number and no pointer.
    unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM as libc::c_ulong) };
    unsafe { libc::getppid() == parent }
}

/// The run directory and the output directory, and the output directory held: it is opened once here, without following a
/// link at its name, and every record, blob and the pid record are made, read and removed through that descriptor.
fn dirs(run_dir: &Path, out: Option<PathBuf>) -> Result<(PathBuf, PathBuf, held::Held), String> {
    let run = fs::canonicalize(run_dir).map_err(|e| format!("{}: {e}", run_dir.display()))?;
    let out = out.unwrap_or_else(|| run.join("fs"));
    fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let out = fs::canonicalize(&out).map_err(|e| e.to_string())?;
    let held = held::Held::open(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok((run, out, held))
}

/// How many bytes of the pid record are read.
const PID_LIMIT: u64 = 64;

/// The pid record of a running watcher, in the output directory.
const PID_FILE: &str = "sdc-watch.pid";

fn alive(pid: i32) -> bool {
    pid > 0 && unsafe { libc::kill(pid, 0) } == 0
}

fn run(run_dir: &Path, out: Option<PathBuf>, kind: Option<Kind>, parent: Option<i32>) -> Result<(), String> {
    set_kind(run_dir, kind);
    let (run, out, out_dir) = dirs(run_dir, out)?;
    let pid_file = out.join(PID_FILE);
    let recorded = out_dir.read(PID_FILE, PID_LIMIT).and_then(|b| String::from_utf8(b).ok()).and_then(|s| s.trim().parse::<i32>().ok());
    if let Some(pid) = recorded {
        if alive(pid) && pid != std::process::id() as i32 {
            return Err(format!("already watched by pid {pid} ({})", pid_file.display()));
        }
    }
    let (mut sig_r, sig_w) = UnixStream::pair().map_err(|e| e.to_string())?;
    sig_r.set_nonblocking(true).map_err(|e| e.to_string())?;
    for s in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        let w = sig_w.try_clone().map_err(|e| e.to_string())?;
        signal_hook::low_level::pipe::register(s, w).map_err(|e| e.to_string())?;
        signal_hook::flag::register(s, store::stop_flag()).map_err(|e| e.to_string())?;
    }
    if let Some(p) = parent {
        if !die_with_parent(p) {
            return Err(format!("the parent {p} is already gone"));
        }
    }
    let mut w = watch::Watcher::start(run.clone(), out.clone(), out_dir.try_clone().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let (files, watches) = w.tracked();
    store::write_through(&out_dir, PID_FILE, format!("{}\n", std::process::id()).as_bytes()).map_err(|e| format!("{}: {e}", pid_file.display()))?;
    eprintln!("sdc-watch: watching {} ({files} files, {watches} dirs), record {}", run.display(),
        out.join("changes.jsonl").display());
    loop {
        let mut timeout = match w.next_wake() {
            None => -1,
            Some(t) => t.saturating_duration_since(Instant::now()).as_millis().min(i32::MAX as u128) as i32,
        };
        if let Some(p) = parent {
            if unsafe { libc::getppid() } != p {
                break;
            }
            if !(0..=PARENT_POLL_MS).contains(&timeout) {
                timeout = PARENT_POLL_MS;
            }
        }
        let mut fds = [
            libc::pollfd { fd: w.fd(), events: libc::POLLIN, revents: 0 },
            libc::pollfd { fd: sig_r.as_raw_fd(), events: libc::POLLIN, revents: 0 },
        ];
        let n = unsafe { libc::poll(fds.as_mut_ptr(), 2, timeout) };
        if n < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            return Err(format!("poll: {}", std::io::Error::last_os_error()));
        }
        if fds[1].revents != 0 {
            let mut b = [0u8; 16];
            let _ = sig_r.read(&mut b);
            break;
        }
        if fds[0].revents != 0 {
            w.drain();
        }
        w.flush(false);
        w.write(false);
        w.late();
    }
    w.finish();
    let _ = out_dir.remove(PID_FILE);
    eprintln!("sdc-watch: stopped; {} changes recorded this session, next seq {}", w.written, w.seq);
    Ok(())
}

fn scan(run_dir: &Path, out: Option<PathBuf>, kind: Option<Kind>) -> Result<(), String> {
    set_kind(run_dir, kind);
    let (run, out, out_dir) = dirs(run_dir, out)?;
    let blobs = store::Blobs::new(out_dir.sub("blobs").map_err(|e| format!("{}: {e}", out.join("blobs").display()))?);
    let (_, seq) = store::replay(&out_dir, &run, &blobs).map_err(|e| e.to_string())?;
    let snap = watch::snapshot(&run, &out, &blobs);
    store::write_snapshot(&out_dir, &run, &snap, seq).map_err(|e| e.to_string())?;
    println!("{} files, snapshot {}", snap.len(), out.join("snapshot.json").display());
    Ok(())
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let r = match cli.cmd {
        Cmd::Run { run_dir, out, kind, parent } => run(&run_dir, out, kind, parent),
        Cmd::Scan { run_dir, out, kind } => scan(&run_dir, out, kind),
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sdc-watch: {e}");
            ExitCode::FAILURE
        }
    }
}
