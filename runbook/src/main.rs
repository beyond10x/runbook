//! `runbook`: the one command for running a runbook. This version has `doctor`, `list` and `check`;
//! `run`, `ui` and `handback` come in later versions and are not stubbed here.

mod assets;
mod check;
mod doctor;
mod list;

use clap::error::ErrorKind;
use clap::{Parser, Subcommand};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "runbook", version, about = "Run a runbook: a fixed, checked way to handle a case")]
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
}

const USAGE_ERROR: u8 = 64;

fn main() -> ExitCode {
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
