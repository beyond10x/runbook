//! `runbook canon`: the subcommands of the `canon` command line that this repository's scripts
//! call (`validate`, `compile`, `check`, `simulate`, `evaluate`), over the linked `b10x-canon`
//! library. Arguments, output and exit statuses are those of the `canon` binary of the same
//! release: 0 success, 1 a document read and rejected or a finding, 2 an unreadable input, 64 a
//! command line that was not understood. The pin is the `b10x-canon` entry of `Cargo.lock`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use b10x_canon::{check, eval, ir, model, simulate, validate};
use clap::Subcommand;

const REJECTED: u8 = 1;
const UNREADABLE: u8 = 2;

/// The `b10x-canon` release this binary links and the commit of its source, read from the lock
/// file the binary was built with.
pub struct Pin {
    pub version: String,
    pub commit: String,
}

pub fn pin() -> Pin {
    let lock = include_str!("../Cargo.lock");
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        if line != "name = \"b10x-canon\"" {
            continue;
        }
        let version = lines.next().and_then(|l| l.strip_prefix("version = \"")).and_then(|l| l.strip_suffix('"')).unwrap_or_default();
        let source = lines.next().unwrap_or_default();
        let commit = source.rsplit('#').next().unwrap_or_default().trim_end_matches('"');
        return Pin { version: version.into(), commit: commit.into() };
    }
    Pin { version: String::new(), commit: String::new() }
}

#[derive(Subcommand)]
pub enum CanonCmd {
    /// Validate a protocol/1 document and the documents it imports.
    Validate {
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        import: Vec<PathBuf>,
    },
    /// Compile a protocol/1 document to its canonical IR on standard output.
    Compile {
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        import: Vec<PathBuf>,
    },
    /// Evaluate a case under a compiled protocol from an evidence directory.
    Evaluate {
        #[arg(long)]
        ir: PathBuf,
        #[arg(long)]
        case: PathBuf,
        #[arg(long)]
        evidence: PathBuf,
        #[arg(long)]
        authority: Option<PathBuf>,
        #[arg(long)]
        decisions: Option<PathBuf>,
        #[arg(long)]
        at: Option<String>,
    },
    /// Check a protocol/1 document over its whole finite state space.
    Check {
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        properties: Option<PathBuf>,
        #[arg(long)]
        import: Vec<PathBuf>,
    },
    /// Walk every state a case can reach from the empty case.
    Simulate {
        #[arg(long)]
        path: PathBuf,
    },
}

/// `runbook canon --version` prints what `canon --version` printed: `canon <version>`.
pub fn version() -> ExitCode {
    println!("canon {}", pin().version);
    ExitCode::SUCCESS
}

pub fn run(cmd: CanonCmd) -> ExitCode {
    match cmd {
        CanonCmd::Validate { path, import } => validate_command(&path, &import),
        CanonCmd::Compile { path, import } => compile_command(&path, &import),
        CanonCmd::Evaluate { ir, case, evidence, authority, decisions, at } => {
            evaluate_command(&ir, &case, &evidence, authority.as_deref(), decisions.as_deref(), at.as_deref())
        }
        CanonCmd::Check { path, properties, import } => check_command(&path, properties.as_deref(), &import),
        CanonCmd::Simulate { path } => simulate_command(&path),
    }
}

fn unreadable(path: &Path, why: impl std::fmt::Display) -> ExitCode {
    eprintln!("error[unreadable]: {}: {why}", model::one_line(&path.display().to_string()));
    ExitCode::from(UNREADABLE)
}

fn read_text(path: &Path) -> std::io::Result<String> {
    let source = std::fs::read_to_string(path)?;
    Ok(match source.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_owned(),
        None => source,
    })
}

fn read_protocol(path: &Path) -> Result<model::Protocol, ExitCode> {
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("error[unreadable]: {path:?}: {error}");
            return Err(ExitCode::from(UNREADABLE));
        }
    };
    let source = source.strip_prefix('\u{feff}').unwrap_or(&source);
    model::parse(source).map_err(|error| {
        eprintln!("error[parse]: {error}");
        ExitCode::from(REJECTED)
    })
}

fn read_imports(paths: &[PathBuf]) -> Result<Vec<model::Protocol>, ExitCode> {
    let mut sorted: Vec<&PathBuf> = paths.iter().collect();
    sorted.sort();
    sorted
        .into_iter()
        .map(|path| {
            let source = read_text(path).map_err(|error| unreadable(path, error))?;
            model::parse(&source).map_err(|error| {
                eprintln!("error[parse]: {}: {error}", model::one_line(&path.display().to_string()));
                ExitCode::from(REJECTED)
            })
        })
        .collect()
}

fn read_composed(path: &Path, imports: &[PathBuf]) -> Result<model::Protocol, ExitCode> {
    let protocol = read_protocol(path)?;
    let imported = read_imports(imports)?;
    ir::compose(&protocol, &imported).map_err(|problems| report(&problems))
}

fn report(problems: &[validate::Problem]) -> ExitCode {
    for problem in problems {
        eprintln!("error[{}]: {problem}", problem.code());
    }
    ExitCode::from(REJECTED)
}

fn compile_command(path: &Path, imports: &[PathBuf]) -> ExitCode {
    let protocol = match read_composed(path, imports) {
        Ok(protocol) => protocol,
        Err(code) => return code,
    };
    match ir::compile(&protocol) {
        Ok(compiled) => {
            print!("{}", compiled.canonical_json());
            ExitCode::SUCCESS
        }
        Err(problems) => report(&problems),
    }
}

fn validate_command(path: &Path, imports: &[PathBuf]) -> ExitCode {
    let protocol = match read_composed(path, imports) {
        Ok(protocol) => protocol,
        Err(code) => return code,
    };
    match validate::validate(&protocol) {
        Ok(()) => {
            println!("valid: protocol `{}` revision {}", model::one_line(protocol.protocol.id.as_str()), protocol.protocol.revision);
            ExitCode::SUCCESS
        }
        Err(problems) => report(&problems),
    }
}

fn check_command(path: &Path, properties_path: Option<&Path>, imports: &[PathBuf]) -> ExitCode {
    let protocol = match read_protocol(path) {
        Ok(protocol) => protocol,
        Err(code) => return code,
    };
    let imported = match read_imports(imports) {
        Ok(imported) => imported,
        Err(code) => return code,
    };
    let properties = match properties_path.map(read_text).transpose() {
        Ok(text) => text,
        Err(error) => return unreadable(properties_path.expect("only a given path is read"), error),
    };
    let compiled = match ir::compose(&protocol, &imported).and_then(|composed| ir::compile(&composed)) {
        Ok(compiled) => compiled,
        Err(problems) => return report(&problems),
    };
    let result = properties
        .as_deref()
        .map(check::read_properties)
        .transpose()
        .and_then(|properties| check::check(&compiled, properties.as_ref()));
    match result {
        Ok(found) => {
            print!("{found}");
            if found.is_clean() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(REJECTED)
            }
        }
        Err(refusal) => {
            eprintln!("error[{}]: {refusal}", refusal.code());
            ExitCode::from(REJECTED)
        }
    }
}

fn simulate_command(path: &Path) -> ExitCode {
    let protocol = match read_protocol(path) {
        Ok(protocol) => protocol,
        Err(code) => return code,
    };
    let compiled = match ir::compose(&protocol, &[]).and_then(|composed| ir::compile(&composed)) {
        Ok(compiled) => compiled,
        Err(problems) => return report(&problems),
    };
    match simulate::simulate(&compiled) {
        Ok(found) => {
            print!("{found}");
            if found.is_clean() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(REJECTED)
            }
        }
        Err(refusal) => {
            eprintln!("error[{}]: {refusal}", refusal.code());
            ExitCode::from(REJECTED)
        }
    }
}

fn is_record_name(name: &std::ffi::OsStr) -> bool {
    Path::new(name).extension().is_some_and(|ext| ext == "yaml" || ext == "json")
}

fn evaluate_command(
    ir_path: &Path,
    case_path: &Path,
    evidence_dir: &Path,
    authority_path: Option<&Path>,
    decisions_path: Option<&Path>,
    at: Option<&str>,
) -> ExitCode {
    let read = |path: &Path| read_text(path).map_err(|error| unreadable(path, error));
    let inputs = (|| {
        let ir = read(ir_path)?;
        let case = read(case_path)?;
        let authority = authority_path.map(read).transpose()?;
        let decisions = decisions_path.map(read).transpose()?;
        let mut names = Vec::new();
        for entry in std::fs::read_dir(evidence_dir).map_err(|e| unreadable(evidence_dir, e))? {
            names.push(entry.map_err(|e| unreadable(evidence_dir, e))?.file_name());
        }
        names.sort();
        let mut evidence = Vec::with_capacity(names.len());
        for name in &names {
            let path = evidence_dir.join(name);
            let metadata = std::fs::metadata(&path).map_err(|e| unreadable(&path, e))?;
            if !metadata.is_file() || !is_record_name(name) {
                return Err(unreadable(
                    &path,
                    "not an evidence record: the evidence directory holds only `*.yaml` and `*.json` files",
                ));
            }
            let text = read(&path)?;
            evidence.push((path, text));
        }
        Ok((ir, case, evidence, authority, decisions))
    })();
    let (ir, case, evidence, authority, decisions) = match inputs {
        Ok(inputs) => inputs,
        Err(code) => return code,
    };
    let decision = eval::read_ir(&ir).map_err(|r| (Vec::new(), r)).and_then(|ir| {
        let case = eval::read_case(&case).map_err(|r| (Vec::new(), r))?;
        let records = evidence
            .iter()
            .map(|(path, text)| eval::read_evidence(text).map_err(|r| (vec![path], r)))
            .collect::<Result<Vec<_>, _>>()?;
        let supplied = eval::Supplied { authority: authority.as_deref(), at, decisions: decisions.as_deref() };
        eval::evaluate_with(&ir, &case, &records, supplied).map_err(|refusal| {
            let files = refusal
                .evidence()
                .iter()
                .flat_map(|cited| {
                    records.iter().zip(&evidence).filter(move |(record, _)| record.id == *cited).map(|(_, (path, _))| path)
                })
                .collect();
            (files, refusal)
        })
    });
    match decision {
        Ok(decision) => {
            print!("{}", eval::render(&decision));
            ExitCode::SUCCESS
        }
        Err((files, refusal)) => {
            if files.is_empty() {
                eprintln!("error[{}]: {refusal}", refusal.code());
            } else {
                let files: Vec<String> = files.iter().map(|path| model::one_line(&path.display().to_string())).collect();
                eprintln!("error[{}]: {}: {refusal}", refusal.code(), files.join(", "));
            }
            ExitCode::from(REJECTED)
        }
    }
}
