//! `runbook`: the one command for running a runbook. This version has `doctor` and `list`;
//! `run`, `ui` and `handback` come in later versions and are not stubbed here.

mod assets;
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
