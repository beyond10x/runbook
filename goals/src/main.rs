//! sdc-goals: checks, digests and resolves `sdc.goals/1` documents (docs/requester.md § 2).

mod check;

use clap::{Parser, Subcommand, ValueEnum};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "sdc-goals",
    about = "Check, digest and resolve sdc.goals/1 goals documents"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, ValueEnum)]
enum Format {
    Text,
    Json,
}

#[derive(Subcommand)]
enum Cmd {
    /// Validate goals documents; with several files, also the rules across their layers.
    /// Findings go to stderr, one per line: `<file>:<id>: <code>: <message>`.
    /// Exit 0 valid, 1 findings, 2 usage or unreadable file.
    Check {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// text: findings on stderr only; json: also a report on stdout.
        #[arg(long, value_enum, default_value = "text")]
        format: Format,
    },
    /// Print the digest of one entry, `sha256:<hex>`: what a ground citation
    /// `goals@<rev>#<id>` carries. Exit 1 when the entry is not in the file.
    Digest { file: PathBuf, id: String },
    /// Print as JSON the entries of the given layers that apply at an instant, each with its
    /// layer and digest. Checks the files first; exit 1 with findings and no output.
    Resolve {
        /// YYYY-MM-DD or YYYY-MM-DDTHH:MM:SSZ; no clock is read.
        #[arg(long, value_parser = parse_instant)]
        at: String,
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// Check one ground that cites a goals entry, `goals@<revision>#<entry id>`, against the
    /// given (current) layers: the entry exists, its digest is the cited one, it is inside its
    /// validity window at the instant and no applying lower-layer entry overrides it. A cited
    /// revision other than --current is a note (`revision_moved`), not a refusal. Prints a JSON
    /// report; each refusal also goes to stderr as `<cite>: <code>: <message>`.
    /// Exit 0 the ground holds, 1 refused, 2 usage or unreadable file.
    Ground {
        /// YYYY-MM-DD or YYYY-MM-DDTHH:MM:SSZ; no clock is read.
        #[arg(long, value_parser = parse_instant)]
        at: String,
        /// goals@<revision>#<entry id>
        #[arg(long, value_parser = parse_cite)]
        cite: String,
        /// The current revision of the goals artifact (the case snapshot's); a cited revision
        /// that differs is reported as the note `revision_moved`.
        #[arg(long)]
        current: String,
        /// The entry digest the ground carries, sha256:<hex>; absent: not compared.
        #[arg(long)]
        digest: Option<String>,
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
}

fn parse_cite(s: &str) -> Result<String, String> {
    check::parse_cite(s)
        .map(|_| s.to_string())
        .ok_or_else(|| format!("`{s}` is not goals@<revision>#<layer>/<name>"))
}

fn parse_instant(s: &str) -> Result<String, String> {
    check::instant(s).ok_or_else(|| format!("`{s}` is not YYYY-MM-DD or YYYY-MM-DDTHH:MM:SSZ"))
}

fn main() -> ExitCode {
    match Cli::parse().cmd {
        Cmd::Check { files, format } => run_check(&files, format),
        Cmd::Digest { file, id } => run_digest(&file, &id),
        Cmd::Resolve { at, files } => run_resolve(&at, &files),
        Cmd::Ground {
            at,
            cite,
            current,
            digest,
            files,
        } => run_ground(&at, &cite, &current, digest.as_deref(), &files),
    }
}

fn run_ground(
    at: &str,
    cite: &str,
    current: &str,
    digest: Option<&str>,
    files: &[PathBuf],
) -> ExitCode {
    let docs = match read_all(files) {
        Ok(d) => d,
        Err(c) => return c,
    };
    let (revision, id) = check::parse_cite(cite).expect("validated by clap");
    let report = check::check(&docs);
    let (entry, refusals, moved) = if report.findings.is_empty() {
        let g = check::ground(&report, revision, current, id, digest, at);
        (g.entry, g.refusals, g.revision_moved)
    } else {
        let f = &report.findings[0];
        let message = format!(
            "the goals layers have {} finding(s), first {}:{}: {}: {}",
            report.findings.len(),
            f.file,
            f.id,
            f.code,
            f.message
        );
        (
            None,
            vec![check::Refusal {
                code: "invalid_goals",
                message,
            }],
            revision != current,
        )
    };
    for r in &refusals {
        eprintln!("{cite}: {}: {}", r.code, r.message);
    }
    let notes: Vec<Value> = if moved {
        vec![json!({
            "code": "revision_moved",
            "cited": revision,
            "current": current,
            "message": format!("{id} is cited at goals@{revision}; the current revision is {current} and the entry is checked there"),
        })]
    } else {
        Vec::new()
    };
    let out = json!({
        "cite": cite,
        "revision": revision,
        "current": current,
        "entry": id,
        "at": at,
        "holds": refusals.is_empty(),
        "digest": entry.map(check::Entry::digest),
        "layer": entry.map(|e| e.layer),
        "file": entry.map(|e| e.file.as_str()),
        "refusals": refusals.iter().map(|r| json!({"code": r.code, "message": r.message})).collect::<Vec<_>>(),
        "notes": notes,
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
    ExitCode::from(u8::from(!refusals.is_empty()))
}

fn read_all(files: &[PathBuf]) -> Result<Vec<(String, String)>, ExitCode> {
    let mut docs = Vec::new();
    for p in files {
        match fs::read_to_string(p) {
            Ok(t) => docs.push((p.display().to_string(), t)),
            Err(e) => {
                eprintln!("sdc-goals: cannot read {}: {e}", p.display());
                return Err(ExitCode::from(2));
            }
        }
    }
    Ok(docs)
}

/// The entry's fields as parsed, with its layer and digest added.
fn entry_json(e: &check::Entry, with_file: bool) -> Value {
    let mut v = e.raw.clone();
    if let Some(m) = v.as_object_mut() {
        if with_file {
            m.insert("file".into(), json!(e.file));
        }
        m.insert("layer".into(), json!(e.layer));
        m.insert("digest".into(), json!(e.digest()));
    }
    v
}

fn print_findings(report: &check::Report) {
    for f in &report.findings {
        eprintln!("{}:{}: {}: {}", f.file, f.id, f.code, f.message);
    }
}

fn run_check(files: &[PathBuf], format: Format) -> ExitCode {
    let docs = match read_all(files) {
        Ok(d) => d,
        Err(c) => return c,
    };
    let report = check::check(&docs);
    print_findings(&report);
    if let Format::Json = format {
        let findings: Vec<Value> = report
            .findings
            .iter()
            .map(|f| json!({"file": f.file, "id": f.id, "code": f.code, "message": f.message}))
            .collect();
        let out = json!({
            "valid": report.findings.is_empty(),
            "files": docs.iter().map(|(n, _)| n).collect::<Vec<_>>(),
            "entries": report.entries.iter().map(|e| entry_json(e, true)).collect::<Vec<_>>(),
            "findings": findings,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    }
    ExitCode::from(u8::from(!report.findings.is_empty()))
}

fn run_digest(file: &PathBuf, id: &str) -> ExitCode {
    let docs = match read_all(std::slice::from_ref(file)) {
        Ok(d) => d,
        Err(c) => return c,
    };
    match check::raw_entry(&docs[0].1, id) {
        Some(raw) => {
            println!("{}", check::digest_of(&raw));
            ExitCode::SUCCESS
        }
        None => {
            eprintln!("{}:{id}: no_entry: no entry with this id", docs[0].0);
            ExitCode::from(1)
        }
    }
}

fn run_resolve(at: &str, files: &[PathBuf]) -> ExitCode {
    let docs = match read_all(files) {
        Ok(d) => d,
        Err(c) => return c,
    };
    let report = check::check(&docs);
    if !report.findings.is_empty() {
        print_findings(&report);
        return ExitCode::from(1);
    }
    let entries: Vec<Value> = check::resolve(&report.entries, at)
        .into_iter()
        .map(|e| entry_json(e, false))
        .collect();
    let out = json!({"format": check::FORMAT, "at": at, "entries": entries});
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
    ExitCode::SUCCESS
}
