//! Timeline of one recorded run (docs/llm-is-not-the-runtime.md).
//!
//! ```text
//! sdc-run-timeline <run dir> [--json <file>|-] [--text <file>|-] [--check]
//! sdc-run-timeline --check-doc <timeline.json>
//! ```
//!
//! Reads an SDC run directory (the case tree under `store/`, `llm-calls.jsonl` and `llm/`,
//! `decisions.jsonl` and `decider/`, `rounds.jsonl`, `run-settings.jsonl`, `status.json`) or a
//! free-running agent's run directory (`free-agent.json`, `events.jsonl`, `received.jsonl`) and
//! writes `sdc.run-timeline/1`: settings, case tree, totals and spans (program, model, decider,
//! rule, tool, idle), sanitized (no prompt, reply, file content or absolute path). Without
//! `--json` and `--text` it prints the totals line. `--check` also holds the totals to the sum of
//! the spans and, for an SDC run, the cost to `sdc-run-metrics`' run cost; `--check-doc` holds a
//! written document's totals to its spans. Deterministic. Exit 0, 1 a failed check, 2 a run
//! directory or document that cannot be read, 64 usage.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

#[path = "../run_metrics.rs"]
#[allow(dead_code)]
mod run_metrics;
#[path = "../run_timeline.rs"]
mod run_timeline;

const USAGE: &str = "usage: sdc-run-timeline <run dir> [--json <file>|-] [--text <file>|-] [--check]
       sdc-run-timeline --check-doc <timeline.json>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut run, mut json_out, mut text_out, mut check, mut check_doc): (Option<PathBuf>, Option<String>, Option<String>, bool, Option<String>) =
        (None, None, None, false, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--json" | "--text" | "--check-doc" => {
                let Some(v) = it.next() else {
                    eprintln!("sdc-run-timeline: {a} needs a value\n{USAGE}");
                    return ExitCode::from(64);
                };
                match a.as_str() {
                    "--json" => json_out = Some(v.clone()),
                    "--text" => text_out = Some(v.clone()),
                    _ => check_doc = Some(v.clone()),
                }
            }
            "--check" => check = true,
            x if x.starts_with("--") => {
                eprintln!("sdc-run-timeline: unknown argument {x}\n{USAGE}");
                return ExitCode::from(64);
            }
            x if run.is_none() => run = Some(PathBuf::from(x)),
            x => {
                eprintln!("sdc-run-timeline: a second run directory {x}\n{USAGE}");
                return ExitCode::from(64);
            }
        }
    }
    if let Some(f) = check_doc {
        let doc: serde_json::Value = match fs::read_to_string(&f).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string())) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("sdc-run-timeline: cannot read {f}: {e}");
                return ExitCode::from(2);
            }
        };
        let bad = run_timeline::check(&doc);
        for b in &bad {
            eprintln!("sdc-run-timeline: {f}: {b}");
        }
        return if bad.is_empty() { ExitCode::SUCCESS } else { ExitCode::from(1) };
    }
    let Some(run) = run else {
        eprintln!("{USAGE}");
        return ExitCode::from(64);
    };
    let doc = match run_timeline::load(&run) {
        Ok(d) => d,
        Err(run_metrics::Fail(m)) => {
            eprintln!("sdc-run-timeline: {m}");
            return ExitCode::from(2);
        }
    };
    let mut failed = false;
    if check {
        for b in run_timeline::check(&doc) {
            eprintln!("sdc-run-timeline: {b}");
            failed = true;
        }
        if doc["kind"] == "sdc" {
            match run_metrics::load(&run) {
                Ok(m) => {
                    let (a, b) = (doc["totals"]["cost_usd"].as_f64(), m["metrics"]["cost_usd"].as_f64());
                    if !matches!((a, b), (Some(a), Some(b)) if (a - b).abs() < 1e-6) {
                        eprintln!("sdc-run-timeline: totals.cost_usd is {a:?} but sdc-run-metrics gives {b:?}");
                        failed = true;
                    }
                }
                Err(run_metrics::Fail(m)) => {
                    eprintln!("sdc-run-timeline: sdc-run-metrics cannot read the run: {m}");
                    failed = true;
                }
            }
        }
    }
    if json_out.is_none() && text_out.is_none() {
        text_out = Some("-".into());
    }
    let emit = |dest: &str, body: String| -> bool {
        if dest == "-" {
            print!("{body}");
            true
        } else if let Err(e) = fs::write(dest, body) {
            eprintln!("sdc-run-timeline: cannot write {dest}: {e}");
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
        if !emit(t, run_timeline::text(&doc)) {
            return ExitCode::from(1);
        }
    }
    if failed { ExitCode::from(1) } else { ExitCode::SUCCESS }
}
