//! `runbook`: the one command for running a runbook. This version has `doctor`, `list` and `check`;
//! `run`, `ui` and `handback` come in later versions and are not stubbed here.

mod assets;
mod canon_cmd;
mod check;
mod doctor;
mod list;
mod loom_run;
mod parity;

use clap::error::ErrorKind;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "runbook", version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("RUNBOOK_COMMIT"), ")"), about = "Run a runbook: a fixed, checked way to handle a case")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Check every program and helper binary a runbook needs; exit 1 when a required one is missing.
    Doctor {
        /// Check only what this runbook needs (sdc, sdc-bugfix).
        #[arg(long = "for", value_name = "RUNBOOK")]
        for_runbook: Option<String>,
        /// Machine-readable output.
        #[arg(long)]
        json: bool,
    },
    /// List the runbooks and child protocols under the asset root's protocol/ directory.
    List {
        /// Machine-readable output.
        #[arg(long)]
        json: bool,
    },
    /// Check the text of a runbook under the asset root (layout, bindings, instruction files, views, cycles list); exit 1 on any finding.
    Check {
        /// Machine-readable output.
        #[arg(long)]
        json: bool,
    },
    /// Run the daemon in the foreground: HTTP and JSON on a runtime that is a stub unless --runtime loom runs through Loom in this process.
    Serve(runbook::serve::ServeArgs),
    /// MCP over standard input and output (newline-delimited JSON-RPC), as a client of a running daemon.
    Mcp(runbook::mcp::stdio::McpArgs),
    /// Run a runbook once in this process, through Loom as a library or a `b10x-loom` child, under a spend ledger and caps; writes a run directory. With `--detach`: submit the run to a daemon and print its id.
    Run(Box<RunCmd>),
    /// Inspect and steer the runs of a daemon: list, show, events, cancel, decisions.
    Runs {
        #[command(subcommand)]
        cmd: runbook::client::RunsCmd,
    },
    /// Answer a decision request of a run on a daemon.
    Decide(runbook::client::DecideArgs),
    /// Run the cycles of a `loom.tree-suite/1` file through Loom's suite runner in this process; exit 1 when a cycle is not as expected, 2 when the suite is refused.
    Test(runbook::test_suite::Args),
    /// Compare a run under the old driver with a run under Loom: summarize, compare, approvals.
    Parity {
        #[command(subcommand)]
        cmd: ParityCmd,
    },
    /// The `canon` subcommands the scripts call (validate, compile, check, simulate, evaluate) over the linked Canon library.
    #[command(args_conflicts_with_subcommands = true)]
    Canon {
        /// Print the linked Canon version, as `canon --version` did.
        #[arg(short = 'V', long)]
        version: bool,
        #[command(subcommand)]
        cmd: Option<canon_cmd::CanonCmd>,
    },
}

/// `runbook run`: the options of the daemon client (`--detach`) and those of a run in this process.
#[derive(clap::Args)]
struct RunCmd {
    #[command(flatten)]
    common: runbook::client::RunArgs,
    #[command(flatten)]
    here: loom_run::here::HereOptions,
}

#[derive(Subcommand)]
enum ParityCmd {
    /// Write the runtime-neutral summary (`runbook.parity-summary/1`) of a finished run: of the old
    /// driver (`--driver` and `--store`) or of Loom (`--loom-status`).
    Summarize {
        /// The run directory the scripts wrote (`runs/<name>`).
        #[arg(long, value_name = "DIR", requires = "store", conflicts_with_all = ["loom_status", "loom_run"], required_unless_present_any = ["loom_status", "loom_run"])]
        driver: Option<PathBuf>,
        /// The case store of the run (the directory that holds the root case).
        #[arg(long, value_name = "DIR", requires = "driver")]
        store: Option<PathBuf>,
        /// A file holding the JSON `b10x-loom tree status` printed (format `loom.tree-status/1`).
        #[arg(long, value_name = "FILE", conflicts_with = "loom_run")]
        loom_status: Option<PathBuf>,
        /// The product tree of a Loom run; the status's `workspace` when not given.
        #[arg(long, value_name = "DIR", requires = "loom_status")]
        product_tree: Option<PathBuf>,
        /// The run directory `runbook run` wrote: its `tree-status.json`, `work/` and the files
        /// that hold what the status does not (`run-settings.jsonl`, `decisions.jsonl`, `protocols/`).
        #[arg(long, value_name = "DIR")]
        loom_run: Option<PathBuf>,
        /// Write the summary here instead of to standard output.
        #[arg(long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
    /// Compare two summaries: exit 0 and one line when equal, exit 1 and one line per difference otherwise.
    Compare {
        /// Print the table of what is compared and how, and exit.
        #[arg(long)]
        rules: bool,
        /// Also compare the sha256 of every product file.
        #[arg(long)]
        product_digests: bool,
        #[arg(required_unless_present = "rules")]
        a: Option<PathBuf>,
        #[arg(required_unless_present = "rules")]
        b: Option<PathBuf>,
    },
    /// Print the person steps of a run in order, as JSON: the `tree approve` calls a second runtime must make.
    Approvals {
        summary: PathBuf,
    },
}

const USAGE_ERROR: u8 = 64;

fn main() -> ExitCode {
    if let Some(path) = assets::Lookup::from_process().path_with_libexec() {
        std::env::set_var("PATH", path);
    }
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            let ok = matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion);
            let _ = e.print();
            return if ok { ExitCode::SUCCESS } else { ExitCode::from(USAGE_ERROR) };
        }
    };
    match cli.cmd {
        Cmd::Doctor { for_runbook, json } => doctor_cmd(for_runbook, json),
        Cmd::List { json } => list_cmd(json),
        Cmd::Check { json } => check_cmd(json),
        Cmd::Run(run) if run.common.detach => runbook::client::run(run.common),
        Cmd::Run(run) => loom_run::here::command(run.common, run.here),
        Cmd::Runs { cmd } => runbook::client::runs(cmd),
        Cmd::Decide(args) => runbook::client::decide(args),
        Cmd::Parity { cmd } => parity_cmd(cmd),
        Cmd::Canon { version: true, .. } => canon_cmd::version(),
        Cmd::Canon { cmd: Some(cmd), .. } => canon_cmd::run(cmd),
        Cmd::Canon { .. } => canon_usage(),
        Cmd::Mcp(args) => runbook::mcp::stdio::run(args),
        Cmd::Serve(mut args) => {
            let assets = assets::Lookup::from_process().locate().ok();
            if args.runbook_dir.is_none() {
                args.runbook_dir = assets.as_ref().map(|a| a.protocol_dir());
            }
            if args.runbook_home.is_none() {
                args.runbook_home = assets.map(|a| a.root);
            }
            runbook::serve::run(args, &list::ROOT_RUNBOOKS)
        }
        Cmd::Test(args) => {
            let protocols = assets::Lookup::from_process().locate().map(|a| a.protocol_dir());
            runbook::test_suite::command(args, protocols)
        }
    }
}

/// `runbook canon` with no subcommand: the reason and the subcommands it has, on standard error.
fn canon_usage() -> ExitCode {
    let mut command = <Cli as clap::CommandFactory>::command();
    let names: Vec<String> = command.find_subcommand_mut("canon").map(|c| c.get_subcommands().map(|s| s.get_name().to_string()).collect()).unwrap_or_default();
    eprintln!("error: runbook canon needs a subcommand: {}
Usage: runbook canon <COMMAND>   (runbook canon --help lists them, runbook canon -V prints the Canon version)", names.join(", "));
    ExitCode::from(USAGE_ERROR)
}

/// Exit 2 is an error (unreadable input); exit 1 is a difference.
fn parity_cmd(cmd: ParityCmd) -> ExitCode {
    let fail = |e: String| {
        eprintln!("runbook parity: {e}");
        ExitCode::from(2)
    };
    match cmd {
        ParityCmd::Summarize { driver, store, loom_status, product_tree, loom_run, output } => {
            let source = match (driver, store, loom_status, loom_run) {
                (Some(run), Some(store), None, None) => parity::Source::Driver { run, store },
                (None, None, Some(status), None) => parity::Source::Loom { status, product_tree },
                (None, None, None, Some(dir)) if product_tree.is_none() => parity::Source::LoomRun { dir },
                _ => return ExitCode::from(USAGE_ERROR),
            };
            let v = match parity::read(&source) {
                Ok(v) => v,
                Err(e) => return fail(e),
            };
            let text = serde_json::to_string_pretty(&v).unwrap() + "\n";
            match output {
                Some(p) => {
                    if let Err(e) = std::fs::write(&p, text) {
                        return fail(format!("{}: {e}", p.display()));
                    }
                    println!("summary: {}", parity::counts(&v));
                }
                None => print!("{text}"),
            }
            ExitCode::SUCCESS
        }
        ParityCmd::Compare { rules: true, .. } => {
            print!("{}", parity::rules_text());
            ExitCode::SUCCESS
        }
        ParityCmd::Compare { product_digests, a: Some(a), b: Some(b), .. } => {
            let (a, b) = match (parity::load(&a), parity::load(&b)) {
                (Ok(a), Ok(b)) => (a, b),
                (Err(e), _) | (_, Err(e)) => return fail(e),
            };
            let diffs = parity::compare(&a, &b, &parity::Options { product_digests });
            let notes = parity::not_recorded_notes(&a, &b);
            if diffs.is_empty() {
                let what = if notes.is_empty() { "equal" } else { "equal on what both sides record" };
                println!("{what}: {}", parity::counts(&a));
                notes.iter().for_each(|n| println!("{n}"));
                println!("parity: {}", if notes.is_empty() { "identical" } else { "equal on what both sides record" });
                ExitCode::SUCCESS
            } else {
                diffs.iter().chain(&notes).for_each(|d| println!("{d}"));
                println!("parity: different");
                ExitCode::from(1)
            }
        }
        ParityCmd::Compare { .. } => ExitCode::from(USAGE_ERROR),
        ParityCmd::Approvals { summary } => match parity::load(&summary) {
            Ok(v) => {
                println!("{}", serde_json::to_string_pretty(&parity::approvals(&v)).unwrap());
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        },
    }
}

fn check_cmd(json: bool) -> ExitCode {
    let assets = match assets::Lookup::from_process().locate() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("runbook check: {e}");
            return ExitCode::from(1);
        }
    };
    let report = check::run(&assets.root);
    if json {
        println!("{}", serde_json::to_string_pretty(&check::json(&report)).unwrap());
    } else {
        print!("{}", check::text(&report));
    }
    if report.ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn doctor_cmd(for_runbook: Option<String>, json: bool) -> ExitCode {
    let only = match &for_runbook {
        None => None,
        Some(id) => match doctor::Scope::runbook(id) {
            Some(s) => Some((id.as_str(), s)),
            None => {
                eprintln!("runbook doctor: unknown runbook '{id}'; startable runbooks: {}", list::ROOT_RUNBOOKS.join(", "));
                return ExitCode::from(USAGE_ERROR);
            }
        },
    };
    let report = doctor::run(&assets::Lookup::from_process(), &|k| std::env::var_os(k), only);
    if json {
        println!("{}", serde_json::to_string_pretty(&doctor::json(&report)).unwrap());
    } else {
        print!("{}", doctor::text(&report));
    }
    if report.ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn list_cmd(json: bool) -> ExitCode {
    let entries = assets::Lookup::from_process().locate().and_then(|a| list::read(&a.protocol_dir()));
    match entries {
        Ok(e) if json => {
            println!("{}", serde_json::to_string_pretty(&list::json(&e)).unwrap());
            ExitCode::SUCCESS
        }
        Ok(e) => {
            print!("{}", list::text(&e));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("runbook list: {e}");
            ExitCode::from(1)
        }
    }
}
