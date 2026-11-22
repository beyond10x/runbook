//! `runbook test <suite>`: the cycles of a `loom.tree-suite/1` file, run one after another through
//! Loom's suite runner (`b10x_loom_tree::suite::run_suite`) in this process (`docs/cli.md`). It
//! starts no `b10x-loom` or `canon` process; only the bound programs run, started by
//! Loom's runner, with `PATH` and `LOOM_*` and nothing else of this process's environment.

use b10x_loom_tree::model::{CycleResult, SuiteReport, SuiteRefusalReason};
use b10x_loom_tree::suite::{cycle_line, read_suite, run_suite, SuiteRefused, SuiteSettings};
use b10x_loom_tree::{ConfinementAdapter, EngineConfinement, EngineRunner, TestRunner, TreeTime, UNCONFINED_FLAG_REASON};
use clap::{Args as ClapArgs, ValueEnum};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// The exit code when every cycle that ran is as expected.
pub const EXIT_AS_EXPECTED: u8 = 0;
/// The exit code when a cycle that ran, and is not pending, is not as expected, or the run was cancelled.
pub const EXIT_FAILED: u8 = 1;
/// The exit code when the command line, the suite, the protocols, a seed, the scratch directory or the confinement level was refused before any cycle.
pub const EXIT_REFUSED: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Level {
    /// Unconfined, declared insecure: Loom's `local`.
    Local,
    /// Substrate's linux adapter. Loom's suite runner refuses it today.
    Substrate,
}

impl Level {
    fn name(self) -> &'static str {
        match self {
            Level::Local => "local",
            Level::Substrate => "substrate",
        }
    }
}

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// The `loom.tree-suite/1` file; its seeds, bindings and suite sources are paths below its directory.
    pub suite: PathBuf,
    /// The directory the cycles work in: absent or empty. Default: a new directory under `$XDG_RUNTIME_DIR/runbook/`, else under the state directory, removed when every cycle is as expected and kept, named, otherwise. A directory named here is never removed.
    #[arg(long, value_name = "DIR")]
    pub scratch: Option<PathBuf>,
    /// Run only these cycles, by name (comma-separated; may be repeated).
    #[arg(long, value_name = "NAME[,NAME...]", value_delimiter = ',')]
    pub only: Vec<String>,
    /// The confinement every bound program starts under: local or substrate. No default; passed on unchanged.
    #[arg(long, value_enum, required = true, value_name = "LEVEL")]
    pub confinement: Option<Level>,
}

/// Set by SIGINT and SIGTERM: the suite ends the cycle that runs before its next step.
static CANCEL: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: libc::c_int) {
    CANCEL.store(true, Ordering::SeqCst);
}

fn catch_signals() {
    for signal in [libc::SIGINT, libc::SIGTERM] {
        // SAFETY: the handler only stores into an atomic, which is async-signal-safe.
        unsafe {
            libc::signal(signal, on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t);
        }
    }
}

fn clean(text: &str) -> String {
    text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect()
}

fn refuse(what: impl std::fmt::Display) -> ExitCode {
    eprintln!("runbook test: refused: {}", clean(&what.to_string()));
    ExitCode::from(EXIT_REFUSED)
}

fn refused_suite(refused: &SuiteRefused) -> ExitCode {
    refuse(refused)
}

/// The runner of `level`, as Loom's `tree` commands open it (`crates/loom-cli/src/tree.rs`, `tree_runner`).
fn runner(level: Level) -> Result<Arc<dyn TestRunner>, String> {
    match level {
        Level::Substrate => Err(
            "ConfinementRefused: CgroupUndelegated: runbook test holds no cgroup root, which the substrate level needs; \
             Loom's suite runner takes none today. Use --confinement local to run the bound programs unconfined."
                .to_owned(),
        ),
        Level::Local => EngineRunner::open(&EngineConfinement { adapter: ConfinementAdapter::Local, local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()), cgroup_root: None })
            .map(|r| Arc::new(r) as Arc<dyn TestRunner>)
            .map_err(|e| format!("ConfinementRefused: {e}")),
    }
}

/// A new, absent directory for the cycles: under `$XDG_RUNTIME_DIR/runbook/`, else under `$XDG_STATE_HOME/runbook/`, else `$HOME/.local/state/runbook/`. Never under `/tmp`.
pub fn default_scratch(env: &dyn Fn(&str) -> Option<PathBuf>, pid: u32) -> Result<PathBuf, String> {
    let base = match (env("XDG_RUNTIME_DIR"), env("XDG_STATE_HOME"), env("HOME")) {
        (Some(runtime), _, _) if runtime.is_absolute() => runtime.join("runbook"),
        (_, Some(state), _) if state.is_absolute() => state.join("runbook"),
        (_, _, Some(home)) if home.is_absolute() => home.join(".local/state/runbook"),
        _ => return Err("no directory for the cycles: give --scratch, or set XDG_RUNTIME_DIR, XDG_STATE_HOME or HOME".to_owned()),
    };
    let seconds = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    Ok(base.join(format!("test-{seconds}-{pid}")))
}

/// `only` as a copy of the suite next to the original, holding only the named cycles, removed when dropped. Loom's runner reads a seed, a binding and a suite source relative to the suite file's own directory, so the copy has to lie there.
struct Filtered {
    path: PathBuf,
}

impl Filtered {
    fn make(suite: &Path, only: &BTreeSet<String>, pid: u32) -> Result<Filtered, String> {
        let text = std::fs::read_to_string(suite).map_err(|e| format!("{}: {e}", suite.display()))?;
        let mut document: serde_yaml::Value = serde_yaml::from_str(&text).map_err(|e| format!("{}: {e}", suite.display()))?;
        let cycles = document.get_mut("cycles").and_then(|c| c.as_sequence_mut()).ok_or_else(|| format!("{}: no list of cycles", suite.display()))?;
        cycles.retain(|c| c.get("name").and_then(|n| n.as_str()).is_some_and(|n| only.contains(n)));
        let dir = suite.parent().unwrap_or(Path::new("."));
        let path = dir.join(format!(".runbook-test-only-{pid}.suite.yaml"));
        let body = serde_yaml::to_string(&document).map_err(|e| e.to_string())?;
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        std::io::Write::write_all(&mut file, body.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Filtered { path })
    }
}

impl Drop for Filtered {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn seconds(result: &CycleResult) -> f64 {
    result.seconds.0.parse::<f64>().unwrap_or(0.0)
}

/// The lines after the cycles: the ten slowest that ran, then the verdict.
pub fn closing_lines(report: &SuiteReport) -> Vec<String> {
    let mut ran: Vec<&CycleResult> = report.cycles.iter().filter(|c| c.run).collect();
    ran.sort_by(|a, b| seconds(b).partial_cmp(&seconds(a)).unwrap_or(std::cmp::Ordering::Equal));
    let mut lines = vec!["ten slowest:".to_owned()];
    lines.extend(ran.iter().take(10).map(|c| format!("  {:>7.1} s  {}", seconds(c), c.name)));
    lines.push(format!("{} of {} as expected, {} pending", report.as_expected, report.expected, report.pending));
    if !report.failed.is_empty() {
        lines.push(format!("failed: {}", report.failed.join(", ")));
    }
    lines
}

/// Runs the command and returns its exit code. `protocols` is the directory of the runbook's protocol files (the asset root's `protocol/`).
pub fn command(args: Args, protocols: Result<PathBuf, String>) -> ExitCode {
    let Some(level) = args.confinement else { return refuse("--confinement is required") };
    let suite = match std::path::absolute(&args.suite) {
        Ok(p) => p,
        Err(e) => return refuse(format!("{}: {e}", args.suite.display())),
    };
    let protocols = match protocols {
        Ok(p) => p,
        Err(e) => return refuse(e),
    };
    let decoded = match read_suite(&suite) {
        Ok(d) => d,
        Err(refused) => return refused_suite(&refused),
    };
    let only: BTreeSet<String> = args.only.iter().cloned().collect();
    let names: BTreeSet<&str> = decoded.cycles.iter().map(|c| c.name.as_str()).collect();
    if let Some(unknown) = only.iter().find(|n| !names.contains(n.as_str())) {
        return refuse(format!("--only names no cycle {unknown:?} in {}", suite.display()));
    }
    let runner = match runner(level) {
        Ok(r) => r,
        Err(e) => return refuse(e),
    };
    let pid = std::process::id();
    let (scratch, default) = match &args.scratch {
        Some(p) => match std::path::absolute(p) {
            Ok(p) => (p, false),
            Err(e) => return refuse(format!("{}: {e}", p.display())),
        },
        None => match default_scratch(&|k| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from), pid) {
            Ok(p) => (p, true),
            Err(e) => return refuse(e),
        },
    };
    let filtered = if only.is_empty() {
        None
    } else {
        match Filtered::make(&suite, &only, pid) {
            Ok(f) => Some(f),
            Err(e) => return refuse(e),
        }
    };
    let to_run = filtered.as_ref().map_or(suite.as_path(), |f| f.path.as_path());
    let settings = SuiteSettings::new(protocols, scratch.clone(), TreeTime::Host, runner, Some(level.name().to_owned()));
    catch_signals();
    let mut print = |result: &CycleResult| println!("{}", clean(&cycle_line(result)));
    let ran = run_suite(to_run, &settings, Some(&mut print), Some(&CANCEL));
    drop(filtered);
    let report = match ran {
        Ok(r) => r,
        Err(refused) => {
            if refused.reason() != SuiteRefusalReason::ScratchNotEmpty && default {
                let _ = std::fs::remove_dir(&scratch);
            }
            return refused_suite(&refused);
        }
    };
    for line in closing_lines(&report) {
        println!("{line}");
    }
    let cancelled = CANCEL.load(Ordering::SeqCst);
    if cancelled {
        println!("cancelled: the cycles after the last line above did not run");
    }
    let code = if report.failed.is_empty() && !cancelled { EXIT_AS_EXPECTED } else { EXIT_FAILED };
    if default {
        if code == EXIT_AS_EXPECTED {
            let _ = std::fs::remove_dir_all(&scratch);
        } else {
            println!("scratch kept: {}", scratch.display());
        }
    }
    ExitCode::from(code)
}
