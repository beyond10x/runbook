//! `runbook`: the one command for running a runbook. This version has `doctor`, `list` and `check`;
//! `run`, `ui` and `handback` come in later versions and are not stubbed here.

mod assets;
mod canon_cmd;
mod catalog;
mod check;
mod config;
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
    #[command(flatten)]
    source: config::Source,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Check every program and helper binary a runbook needs; exit 1 when a required one is missing.
    Doctor {
        /// Check only what this runbook needs: a name `runbook list` prints as a runbook.
        #[arg(long = "for", value_name = "RUNBOOK")]
        for_runbook: Option<String>,
        /// Machine-readable output.
        #[arg(long)]
        json: bool,
    },
    /// List the runbooks and child protocols of every runbook directory (`runbooks` of runbook.toml, then the built-in set): name, kind, version, directory, title.
    List {
        /// Machine-readable output.
        #[arg(long)]
        json: bool,
    },
    /// Check the text of every runbook directory (layout, bindings, instruction files, views; the cycles list of the built-in set); exit 1 on any finding.
    Check {
        /// Check only the directory of this runbook: a name `runbook list` prints.
        #[arg(long = "for", value_name = "RUNBOOK")]
        for_runbook: Option<String>,
        /// Machine-readable output.
        #[arg(long)]
        json: bool,
    },
    /// Run the daemon in the foreground: HTTP and JSON on a runtime that is a stub unless --runtime loom runs through Loom in this process.
    Serve(Box<runbook::serve::ServeArgs>),
    /// MCP over standard input and output (newline-delimited JSON-RPC), as a client of a running daemon.
    Mcp(runbook::mcp::stdio::McpArgs),
    /// Run a runbook once in this process, through Loom as a library or a `b10x-loom` child, under a spend ledger and caps; writes a run directory. With `--detach`: submit the run to a daemon and print its id.
    Run(Box<RunCmd>),
    /// Print every effective value of `runbook run` with where it comes from: an option, a settings file, `runbook.toml` or a built-in default. Opens no login file.
    Config(config::ValueOptions),
    /// Write a starter `runbook.toml` (mode 600) into the working directory; refuses to overwrite one.
    Init,
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
        Cmd::Doctor { for_runbook, json } => doctor_cmd(&cli.source, for_runbook, json),
        Cmd::List { json } => list_cmd(&cli.source, json),
        Cmd::Check { for_runbook, json } => check_cmd(&cli.source, for_runbook, json),
        Cmd::Run(run) if run.common.detach => match run.here.values.first_given() {
            Some(flag) => {
                eprintln!("runbook run: {flag} belongs to a run in this process and conflicts with --detach");
                ExitCode::from(USAGE_ERROR)
            }
            None => runbook::client::run(run.common),
        },
        Cmd::Run(run) => loom_run::here::command(run.common, run.here, &cli.source),
        Cmd::Config(options) => match config::show(&cli.source, &options) {
            Ok(text) => {
                print!("{text}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("runbook config: {e}");
                ExitCode::from(loom_run::EXIT_REFUSED)
            }
        },
        Cmd::Init => match config::init(None) {
            Ok(path) => {
                println!("wrote {}", path.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("runbook init: {e}");
                ExitCode::from(loom_run::EXIT_REFUSED)
            }
        },
        Cmd::Runs { cmd } => runbook::client::runs(cmd),
        Cmd::Decide(args) => runbook::client::decide(args),
        Cmd::Parity { cmd } => parity_cmd(cmd),
        Cmd::Canon { version: true, .. } => canon_cmd::version(),
        Cmd::Canon { cmd: Some(cmd), .. } => canon_cmd::run(cmd),
        Cmd::Canon { .. } => canon_usage(),
        Cmd::Mcp(args) => runbook::mcp::stdio::run(args),
        Cmd::Serve(mut args) => {
            let world = match catalog::open(&cli.source) {
                Ok(w) => w,
                Err((code, e)) => {
                    eprintln!("runbook serve: {e}");
                    return ExitCode::from(code);
                }
            };
            args.runbooks = world.configured.clone();
            match config::load(&cli.source) {
                Ok(file) => {
                    args.file_confinement = file.as_ref().and_then(|f| f.confinement).map(loom_run::library::level_of);
                    if let Some(file) = file.filter(|f| !f.deciders.is_empty()) {
                        let limits = loom_run::arbiter::Limits {
                            login_file: file.members.get(loom_run::settings::LOGIN_KEY).and_then(|v| v.as_str()).map(str::to_owned),
                            cap_micro_usd: file.cap_micro_usd,
                            max_llm_calls: file.max_llm_calls,
                        };
                        match loom_run::arbiter::DaemonArbiter::new(file.deciders, limits) {
                            Ok(arbiter) => args.arbiter = Some(std::sync::Arc::new(arbiter)),
                            Err(e) => {
                                eprintln!("runbook serve: {e}");
                                return ExitCode::from(2);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("runbook serve: {e}");
                    return ExitCode::from(2);
                }
            }
            if args.runbook_dir.is_none() {
                args.runbook_dir = world.assets.as_ref().map(|a| a.protocol_dir());
            }
            let presets: Vec<String> = world.startable().iter().map(|s| s.to_string()).collect();
            if args.runbook_home.is_none() {
                args.runbook_home = world.assets.map(|a| a.root);
            }
            let presets: Vec<&str> = presets.iter().map(String::as_str).collect();
            runbook::serve::run(*args, &presets)
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

fn check_cmd(source: &config::Source, for_runbook: Option<String>, json: bool) -> ExitCode {
    let world = match catalog::open(source).and_then(catalog::World::need_some) {
        Ok(w) => w,
        Err((code, e)) => {
            eprintln!("runbook check: {e}");
            return ExitCode::from(code);
        }
    };
    let built_in = world.assets.as_ref().map(|a| a.root.clone());
    let reports: Vec<check::Report> = match &for_runbook {
        None => {
            let mut all: Vec<check::Report> = world.configured.iter().map(|d| check::run_dir(d)).collect();
            all.extend(built_in.as_deref().map(check::run));
            all
        }
        Some(id) => match runbook::runbooks::find(&world.entries, id) {
            Some(e) if e.built_in => vec![check::run(&e.dir)],
            Some(e) => vec![check::run_dir(&e.dir)],
            None => {
                eprintln!("runbook check: unknown runbook '{id}'; runbooks: {}", world.startable().join(", "));
                return ExitCode::from(USAGE_ERROR);
            }
        },
    };
    let ok = reports.iter().all(check::Report::ok);
    if json && reports.len() == 1 {
        println!("{}", serde_json::to_string_pretty(&check::json(&reports[0])).unwrap());
    } else if json {
        let all: Vec<_> = reports.iter().map(check::json).collect();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "ok": ok, "directories": all })).unwrap());
    } else {
        for r in &reports {
            if reports.len() > 1 {
                println!("directory {}:", r.root.display());
            }
            print!("{}", check::text(r));
        }
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn doctor_cmd(source: &config::Source, for_runbook: Option<String>, json: bool) -> ExitCode {
    let mut directory = None;
    let only = match &for_runbook {
        None => None,
        Some(id) => {
            let world = match catalog::open(source).and_then(catalog::World::need_some) {
                Ok(w) => w,
                Err((code, e)) => {
                    eprintln!("runbook doctor: {e}");
                    return ExitCode::from(code);
                }
            };
            match runbook::runbooks::find(&world.entries, id).filter(|e| e.startable) {
                Some(e) => {
                    if !e.built_in {
                        directory = Some(assets::Assets { root: e.dir.clone(), source: assets::Source::Configured });
                    }
                    Some((id.as_str(), doctor::Scope::runbook(id)))
                }
                None => {
                    eprintln!("runbook doctor: unknown runbook '{id}'; startable runbooks: {}", world.startable().join(", "));
                    return ExitCode::from(USAGE_ERROR);
                }
            }
        }
    };
    let report = doctor::run(&assets::Lookup::from_process(), &|k| std::env::var_os(k), only, directory);
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

fn list_cmd(source: &config::Source, json: bool) -> ExitCode {
    match catalog::open(source).and_then(catalog::World::need_some) {
        Ok(w) if json => {
            println!("{}", serde_json::to_string_pretty(&list::json(&w.entries)).unwrap());
            ExitCode::SUCCESS
        }
        Ok(w) => {
            print!("{}", list::text(&w.entries));
            ExitCode::SUCCESS
        }
        Err((code, e)) => {
            eprintln!("runbook list: {e}");
            ExitCode::from(code)
        }
    }
}
