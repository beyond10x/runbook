//! Workflow metrics of one recorded run (docs/workflow-metrics.md).
//!
//! ```text
//! sdc-run-metrics <run dir> [--json <file>|-] [--text <file>|-]
//! ```
//!
//! Reads the run directory `scripts/run-trial.sh` writes (the root case `store/IDEA-1/`, or the
//! one case directory under `store/` of a bug-fix run, with every case's `events.jsonl`,
//! `observed.jsonl`, `evidence/`, `ir.json`, `bindings.json`; `llm-calls.jsonl`,
//! `decisions.jsonl`, `rounds.jsonl`, `run-settings.jsonl`, `status.json`, `run-stopped*.json`,
//! `defects.jsonl`, the root's `work/findings.yaml`) and writes `sdc.run-metrics/1`: per node,
//! per edge, per decision class, per stage and per run. Without `--json` and `--text` it prints the
//! text form. Deterministic: no clock reading; the run is named by its directory name. Exit 0, 2 a
//! run directory without a root case `events.jsonl` (or a file that cannot be read), 64 usage.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

#[path = "../run_metrics.rs"]
#[allow(dead_code)]
mod run_metrics;

const USAGE: &str = "usage: sdc-run-metrics <run dir> [--json <file>|-] [--text <file>|-]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut run, mut json_out, mut text_out): (Option<PathBuf>, Option<String>, Option<String>) = (None, None, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--json" | "--text" => {
                let Some(v) = it.next() else {
                    eprintln!("sdc-run-metrics: {a} needs a file or -\n{USAGE}");
                    return ExitCode::from(64);
                };
                if a == "--json" { json_out = Some(v.clone()) } else { text_out = Some(v.clone()) }
            }
            x if x.starts_with("--") => {
                eprintln!("sdc-run-metrics: unknown argument {x}\n{USAGE}");
                return ExitCode::from(64);
            }
            x if run.is_none() => run = Some(PathBuf::from(x)),
            x => {
                eprintln!("sdc-run-metrics: a second run directory {x}\n{USAGE}");
                return ExitCode::from(64);
            }
        }
    }
    let Some(run) = run else {
        eprintln!("{USAGE}");
        return ExitCode::from(64);
    };
    let doc = match run_metrics::load(&run) {
        Ok(d) => d,
        Err(run_metrics::Fail(m)) => {
            eprintln!("sdc-run-metrics: {m}");
            return ExitCode::from(2);
        }
    };
    if json_out.is_none() && text_out.is_none() {
        text_out = Some("-".into());
    }
    let emit = |dest: &str, body: String| -> bool {
        if dest == "-" {
            print!("{body}");
            true
        } else if let Err(e) = fs::write(dest, body) {
            eprintln!("sdc-run-metrics: cannot write {dest}: {e}");
            false
        } else {
            true
        }
    };
    if let Some(j) = &json_out {
        if !emit(j, serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n") {
            return ExitCode::from(1);
        }
    }
    if let Some(t) = &text_out {
        if !emit(t, run_metrics::text(&doc)) {
            return ExitCode::from(1);
        }
    }
    ExitCode::SUCCESS
}
