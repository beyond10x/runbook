//! `runbook run <runbook>` without `--detach`: one run of a runbook in this process, through the
//! loop of `run_loop.rs`.
//!
//! The options are those the loop needs (`Args`). The values that have a default (the run, the directory
//! of the runs, the spend ledger, the caps, the confinement, the settings) come from an option, else
//! from `runbook.toml` (`config.rs`), else from a built-in default where a wrong value does no harm;
//! what the asset root holds for the runbook (the bindings, the protocols, the root protocol) is looked
//! up when its option is not given, by the code `runbook list` and `runbook check` use. Without
//! `--loom` Loom is a library of this process; `--loom` with `--loom-sha256` runs the `b10x-loom` binary
//! as a child process. A value that has no default and is not stated (the confinement, the wall time,
//! the two caps) is a usage error that names the option and the line to add (exit 64); a refusal of the
//! run itself, and of `runbook.toml`, is exit 2.

use super::ports::{refuse_unbuilt, Bugfix, Conformance, OnDivergence, Ports, Refusal, Replay};
use super::{Args, EXIT_REFUSED};
use crate::config::{self, ValueOptions};
use clap::Args as ClapArgs;
use runbook::client::RunArgs;
use runbook::runbooks::{self, Entry};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The case id of the root case when `--case` is not given.
pub const DEFAULT_CASE: &str = "IDEA-1";

/// The options of a run in this process; every one conflicts with `--detach` (the value options are
/// checked against it by `main`).
#[derive(ClapArgs, Clone, Debug)]
pub struct HereOptions {
    #[command(flatten)]
    pub values: ValueOptions,
    /// The `b10x-loom` binary, run as a child process; with `--loom-sha256`. Without both, Loom is
    /// a library of this process.
    #[arg(long, value_name = "FILE", conflicts_with = "detach")]
    pub loom: Option<PathBuf>,
    /// The sha256 (hex) the `--loom` file must have; a different one is refused before anything is written.
    #[arg(long, value_name = "HEX", conflicts_with = "detach")]
    pub loom_sha256: Option<String>,
    /// The `bindings/` directory (its yaml files are copied, never edited); default: `bindings/` of the asset root.
    #[arg(long, value_name = "DIR", conflicts_with = "detach")]
    pub bindings: Option<PathBuf>,
    /// The directory of the protocol files (`*.protocol.yaml`) to register; default: `protocol/` of the asset root.
    #[arg(long, value_name = "DIR", conflicts_with = "detach")]
    pub protocols: Option<PathBuf>,
    /// The root protocol, `name@revision`; default: the file of the runbook under the protocols directory.
    #[arg(long, value_name = "NAME@REVISION", conflicts_with = "detach")]
    pub root_protocol: Option<String>,
    /// The id of the root case.
    #[arg(long, value_name = "ID", default_value = DEFAULT_CASE, conflicts_with = "detach")]
    pub case: String,
    /// The stub cycle: every model node on the adapter dry, print every model command, run none, approve person steps by script.
    #[arg(long, conflicts_with = "detach")]
    pub dry: bool,
    /// Re-drive a recorded run (a run directory or a recording), answering each model call and each decision with the recorded reply; `--set KEY=VALUE` sets its conditions (`max_runs.<node>`, `timeout.<node>`).
    #[arg(long, value_name = "DIR", conflicts_with = "detach")]
    pub recorded: Option<PathBuf>,
    /// With `--recorded`, what a call with no recorded reply does: stop (exit 11), take the nearest reply, or go live.
    #[arg(long, value_enum, value_name = "POLICY", conflicts_with = "detach")]
    pub on_divergence: Option<OnDivergence>,
    /// With `--recorded`, the adapter of a live call: `dry`, `metaharness`, `claude-cli` or a path.
    #[arg(long, value_name = "ADAPTER", conflicts_with = "detach")]
    pub live_adapter: Option<String>,
    /// With `--recorded`, the most live calls (default 5; a spent cap stops the run).
    #[arg(long, value_name = "N", conflicts_with = "detach")]
    pub live_cap: Option<u64>,
    /// With `--recorded`, a fault to inject (`reject@<action>#<try>=<reason>`, `timeout@<node>#<try>`, `check-fail@<action>#<try>`); repeatable.
    #[arg(long, value_name = "FAULT", conflicts_with = "detach")]
    pub inject: Vec<String>,
    /// With `--recorded`, the actions whose recorded results are taken as they are.
    #[arg(long, value_name = "ACTIONS", conflicts_with = "detach")]
    pub take_recorded: Option<String>,
    /// With `--recorded`, the commit of the recorded run's source tree.
    #[arg(long, value_name = "COMMIT", conflicts_with = "detach")]
    pub source_tree: Option<String>,
    /// Open a bug-fix case from a seed directory.
    #[arg(long, value_name = "DIR", conflicts_with = "detach")]
    pub bugfix: Option<PathBuf>,
    /// Open a bug-fix case from a ticket file (`sdc.ticket/1`).
    #[arg(long, value_name = "FILE", conflicts_with = "detach")]
    pub bugfix_ticket: Option<PathBuf>,
    /// The requester's goals document, copied unchanged to `work/goals.yaml` of the run.
    #[arg(long, value_name = "FILE", conflicts_with = "detach")]
    pub goals: Option<PathBuf>,
    /// Record every file change of the run beside it (`fs/changes.jsonl`).
    #[arg(long, conflicts_with = "detach")]
    pub watch: bool,
    /// Hold the delivered product to the specification's suite after a close: `off` (default) or `observe`.
    #[arg(long, value_enum, value_name = "MODE", default_value = "off", conflicts_with = "detach")]
    pub conformance: Conformance,
}

/// The detach-only options a run in this process does not take: the first one given, named.
/// `--set` is not among them: in process it is a condition of the replay (`--recorded`).
fn detach_only(base: &RunArgs) -> Option<&'static str> {
    if base.context.is_some() {
        Some("--context")
    } else if base.json {
        Some("--json")
    } else if base.remote.daemon.is_some() {
        Some("--daemon")
    } else if base.remote.token_file.is_some() {
        Some("--token-file")
    } else {
        None
    }
}

/// The root protocol of the runbook as the loop takes it: `<protocol id with . as ->@<revision>`.
fn root_protocol_of(entry: &Entry) -> Result<String, String> {
    let revision = entry.revision.ok_or_else(|| format!("protocol/{}.protocol.yaml has no protocol.revision", entry.id))?;
    Ok(format!("{}@{revision}", entry.protocol_id.replace('.', "-")))
}

/// What the caller named of the three places a runbook's files and root are.
struct Named<'a> {
    protocols: Option<&'a Path>,
    bindings: Option<&'a Path>,
    root_protocol: Option<&'a str>,
}

/// The protocols, the bindings and the root protocol of a runbook; the root protocol is `None` when
/// `--root-protocol` names it.
struct Located {
    protocols: PathBuf,
    bindings: PathBuf,
    root_protocol: Option<String>,
}

/// Resolves `runbook` through the one function `list` uses (`runbook::runbooks::load`): the directories
/// of `runbook.toml`, then the built-in set. `--protocols` and `--bindings` name their directories
/// themselves; the root protocol is that of the file named `runbook` in the protocols directory in use.
fn locate(runbook: &str, named: Named<'_>, file: Option<&config::File>) -> Result<Located, Refusal> {
    let refuse = |e: String| (EXIT_REFUSED, e);
    let from_files = |protocols: &Path| -> Result<String, Refusal> {
        let entries = runbooks::read_protocols(protocols).map_err(refuse)?;
        let entry = entries.iter().find(|e| e.id == runbook).ok_or_else(|| refuse(format!("no runbook {runbook} under {}", protocols.display())))?;
        root_protocol_of(entry).map_err(refuse)
    };
    if let (Some(protocols), Some(bindings)) = (named.protocols, named.bindings) {
        let root_protocol = if named.root_protocol.is_some() { None } else { Some(from_files(protocols)?) };
        return Ok(Located { protocols: protocols.to_path_buf(), bindings: bindings.to_path_buf(), root_protocol });
    }
    let world = crate::catalog::open_with(file.cloned(), &crate::assets::Lookup::from_process()).map_err(|(_, e)| refuse(e))?;
    let found = runbooks::find(&world.entries, runbook);
    let dir = match (found, world.assets.as_ref(), named.root_protocol) {
        (Some(e), _, _) => e.dir.clone(),
        (None, Some(a), Some(_)) => a.root.clone(),
        (None, None, _) if world.configured.is_empty() => {
            let why = world.missing.clone().unwrap_or_default();
            return Err(refuse(format!("no asset root for the runbook's bindings and protocols ({why}): name --bindings and --protocols, or set RUNBOOK_HOME")));
        }
        (None, _, _) => return Err(refuse(format!("no runbook {runbook} in {}", world.searched()))),
    };
    let protocols = named.protocols.map(Path::to_path_buf).unwrap_or_else(|| dir.join("protocol"));
    let bindings = named.bindings.map(Path::to_path_buf).unwrap_or_else(|| dir.join("bindings"));
    let root_protocol = match (named.root_protocol, named.protocols, found) {
        (Some(_), _, _) => None,
        (None, Some(p), _) => Some(from_files(p)?),
        (None, None, Some(e)) => Some(root_protocol_of(e).map_err(refuse)?),
        (None, None, None) => return Err(refuse(format!("no runbook {runbook} in {}", world.searched()))),
    };
    Ok(Located { protocols, bindings, root_protocol })
}


/// The refusal of a value that is not stated: the option, the key and a line to add, and the file.
fn not_stated(flag: &str, line: &str, no_default: bool, file: Option<&std::path::Path>) -> Refusal {
    let default = if no_default { " and has no built-in default" } else { "" };
    let place = match file {
        Some(f) => f.display().to_string(),
        None => "a runbook.toml (no file was found; `runbook init` writes one)".to_owned(),
    };
    (64, format!("{flag} is required{default}: give the option, or add the line `{line}` to {place}"))
}

/// The options of the loop from the command line and `runbook.toml`; what is not given comes from the
/// asset root. The second part is the run's name when it was generated.
pub fn args_of(base: &RunArgs, here: HereOptions, file: Option<&config::File>) -> Result<(Args, Option<String>), Refusal> {
    const USAGE: u8 = 64;
    if let Some(flag) = detach_only(base) {
        return Err((USAGE, format!("{flag} belongs to --detach: a run in this process takes no daemon and no context")));
    }
    let ports = Ports {
        dry: here.dry,
        replay: Replay {
            recorded: here.recorded,
            on_divergence: here.on_divergence,
            live_adapter: here.live_adapter,
            live_cap: here.live_cap,
            inject: here.inject,
            set: base.set.clone(),
            take_recorded: here.take_recorded,
            source_tree: here.source_tree,
        },
        bugfix: Bugfix { seed: here.bugfix, ticket: here.bugfix_ticket },
        goals: here.goals,
        watch: here.watch,
        conformance: here.conformance,
    };
    let intake = runbook::client::read_intake(&base.intake).map_err(|f| (EXIT_REFUSED, f.message))?;
    let named = Named { protocols: here.protocols.as_deref(), bindings: here.bindings.as_deref(), root_protocol: here.root_protocol.as_deref() };
    let Located { protocols, bindings, root_protocol } = locate(super::bugfix::runbook_of(&ports, &base.runbook)?, named, file)?;
    let root_protocol = root_protocol.or(here.root_protocol).ok_or_else(|| (EXIT_REFUSED, "no root protocol".to_owned()))?;
    let cwd = std::env::current_dir().map_err(|e| (EXIT_REFUSED, format!("the working directory: {e}")))?;
    let values = config::resolve(&here.values, file, &cwd);
    let at = values.file.as_deref();
    let confinement = values.confinement.clone().map(|(c, _)| c).ok_or_else(|| not_stated("--confinement", "confinement = \"<local or substrate>\"", true, at))?;
    let max_wall_seconds = values.max_wall_seconds.clone().map(|(n, _)| n).ok_or_else(|| not_stated("--max-wall-seconds", "max_wall_seconds = <seconds>", true, at))?;
    let cap_micro_usd = values.cap_micro_usd.clone().map(|(n, _)| n).ok_or_else(|| not_stated("--cap-micro-usd", "cap_micro_usd = <micro USD>", false, at))?;
    let max_llm_calls = values.max_llm_calls.clone().map(|(n, _)| n).ok_or_else(|| not_stated("--max-llm-calls", "max_llm_calls = <calls>", false, at))?;
    let (run, generated) = match values.run {
        Some((name, _)) => (name, None),
        None => {
            let name = config::generated_run_name(&base.runbook);
            (name.clone(), Some(name))
        }
    };
    Ok((
        Args {
            run,
            runs_dir: values.runs_dir.0,
            loom: here.loom,
            loom_sha256: here.loom_sha256,
            bindings,
            protocols,
            root_protocol,
            case: super::bugfix::case_of(&ports, here.case),
            intake,
            confinement,
            settings: values.settings,
            defaults: values.members,
            spend_ledger: values.spend_ledger.0,
            cap_micro_usd,
            max_llm_calls,
            max_wall_seconds,
            ports,
            config: file.and_then(config::File::noted),
        },
        generated,
    ))
}

/// `runbook run <runbook>` without `--detach`.
pub fn command(base: RunArgs, here: HereOptions, source: &config::Source) -> ExitCode {
    let file = match config::load(source) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("runbook run: {e}");
            return ExitCode::from(EXIT_REFUSED);
        }
    };
    // Before anything starts, and before a refusal that names the file: where the values come from.
    if let Some(said) = file.as_ref().and_then(config::File::said) {
        eprintln!("runbook run: {said}");
    }
    let (args, generated) = match args_of(&base, here, file.as_ref()) {
        Ok(a) => a,
        Err((code, line)) => {
            eprintln!("runbook run: {line}");
            return ExitCode::from(code);
        }
    };
    if let Err((code, line)) = refuse_unbuilt(&args.ports) {
        eprintln!("runbook run: {line}");
        return ExitCode::from(code);
    }
    if let Some(name) = generated {
        eprintln!("runbook run: run {name} (generated), directory {}", args.runs_dir.join(&name).display());
    }
    match super::run_loop::run(&args) {
        Ok(outcome) => {
            match outcome.reason() {
                Some(reason) => eprintln!("runbook run: {}: {reason}", outcome.label()),
                None => eprintln!("runbook run: {}", outcome.label()),
            }
            ExitCode::from(outcome.exit_code())
        }
        Err(refusal) => {
            eprintln!("runbook run: refused: {refusal}");
            ExitCode::from(EXIT_REFUSED)
        }
    }
}
