//! `runbook test <suite>`: the cycles of a `loom.tree-suite/1` file, run one after another (or `--workers` beside each other) through
//! Loom's suite runner (`b10x_loom_tree::suite::run_suite`) in this process (`docs/cli.md`). It
//! starts no `b10x-loom` or `canon` process; only the bound programs run, started by
//! Loom's runner, with `PATH` and `LOOM_*` and nothing else of this process's environment.

use b10x_loom_tree::model::{CycleResult, SuiteReport, SuiteRefusalReason};
use b10x_loom_tree::suite::{cycle_line, read_suite, run_suite, workers_served, SuiteRefused, SuiteSettings};
use b10x_loom_tree::{ConfinementAdapter, EngineConfinement, EngineRunner, TestRunner, TreeTime, UNCONFINED_FLAG_REASON};
use clap::{Args as ClapArgs, ValueEnum};
use std::num::NonZeroUsize;
use std::path::PathBuf;
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
    /// Run only these cycles, by name (comma-separated; may be repeated), in suite order. A name that is no cycle of the suite is refused (CycleUnknown) before any cycle.
    #[arg(long, value_name = "NAME[,NAME...]", value_delimiter = ',')]
    pub only: Vec<String>,
    /// How many cycles run beside each other, 1 to 64 (Loom's `workers`). Default 1: one after another. Each cycle has its own directory in the scratch directory and its own 60 s bound; under `local` nothing is confined, so a program can read the other cycles' workspaces. More than 1 under `--confinement substrate` is refused (SubstrateWorkersUnserved).
    #[arg(long, value_name = "N", default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=64))]
    pub workers: u8,
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

fn seconds(result: &CycleResult) -> f64 {
    result.seconds.0.parse::<f64>().unwrap_or(0.0)
}

/// The lines after the cycles: the ten slowest that ran, then the verdict.
pub fn closing_lines(report: &SuiteReport) -> Vec<String> {
    let mut ran: Vec<&CycleResult> = report.cycles.iter().filter(|c| c.run).collect();
    ran.sort_by(|a, b| seconds(b).partial_cmp(&seconds(a)).unwrap_or(std::cmp::Ordering::Equal));
    let mut lines = Vec::new();
    if let Some(only) = &report.only {
        lines.push(format!("selected {} of {} cycles", only.len(), report.suite_cycles));
    }
    lines.push("ten slowest:".to_owned());
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
    let workers = NonZeroUsize::new(usize::from(args.workers)).unwrap_or(NonZeroUsize::MIN);
    if let Err(refused) = workers_served(Some(level.name()), workers) {
        return refused_suite(&refused);
    }
    if let Err(refused) = read_suite(&suite) {
        return refused_suite(&refused);
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
    let mut settings = SuiteSettings::new(protocols, scratch.clone(), TreeTime::Host, runner, Some(level.name().to_owned()));
    settings.workers = workers;
    settings.only = args.only.clone();
    catch_signals();
    let mut print = |result: &CycleResult| println!("{}", clean(&cycle_line(result)));
    let ran = run_suite(&suite, &settings, Some(&mut print), Some(&CANCEL));
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Cli {
        #[command(flatten)]
        args: Args,
    }

    fn parse(extra: &[&str]) -> Result<Args, clap::Error> {
        let mut argv = vec!["runbook-test", "suite.yaml", "--confinement", "local"];
        argv.extend_from_slice(extra);
        Cli::try_parse_from(argv).map(|c| c.args)
    }

    #[test]
    fn cycles_run_one_after_another_unless_workers_says_otherwise() {
        assert_eq!(parse(&[]).unwrap().workers, 1);
        assert_eq!(parse(&["--workers", "4"]).unwrap().workers, 4);
        assert_eq!(parse(&["--workers", "64"]).unwrap().workers, 64);
    }

    #[test]
    fn a_worker_count_outside_one_to_sixty_four_is_refused_by_the_parser() {
        for bad in ["0", "65", "256", "-1", "x"] {
            assert!(parse(&["--workers", bad]).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_selection_line_names_the_chosen_cycles_of_the_suite_only_with_only() {
        let mut report = SuiteReport {
            cycles: Vec::new(),
            expected: 0,
            as_expected: 0,
            failed: Vec::new(),
            pending: 0,
            pending_passing: Vec::new(),
            waiting: Vec::new(),
            not_run: Vec::new(),
            only: None,
            suite_cycles: 54,
        };
        assert!(!closing_lines(&report).iter().any(|l| l.starts_with("selected")));
        report.only = Some(vec!["run-stop".to_owned(), "decider-rework".to_owned()]);
        assert_eq!(closing_lines(&report)[0], "selected 2 of 54 cycles");
    }
}
