//! `runbook run <runbook>` without `--detach`: one run of a runbook in this process, through the
//! loop of `run_loop.rs`.
//!
//! The options are those the loop needs (`Args`); what the asset root holds for the runbook (the
//! bindings, the protocols, the root protocol) is looked up when its option is not given, by the
//! code `runbook list` and `runbook check` use. Without `--loom` Loom is a library of this process;
//! `--loom` with `--loom-sha256` runs the `b10x-loom` binary as a child process. A missing required
//! option is a usage error that names it (clap, exit 64); a refusal of the run itself is exit 2. The
//! one option that is required only by what the bindings hold, `--spend-ledger` of a run with a model
//! node, is exit 64 too, before anything is written; a run without a model node under `local` that
//! lacks it is refused by name, exit 2.

use super::ports::{refuse_unbuilt, Bugfix, Conformance, OnDivergence, Ports, Refusal, Replay};
use super::{Args, Confinement, EXIT_REFUSED};
use clap::Args as ClapArgs;
use runbook::client::RunArgs;
use std::path::PathBuf;
use std::process::ExitCode;

/// The case id of the root case when `--case` is not given.
pub const DEFAULT_CASE: &str = "IDEA-1";

/// The options of a run in this process; every one conflicts with `--detach`. The ones without a
/// default are required without it.
#[derive(ClapArgs, Clone, Debug)]
pub struct HereOptions {
    /// Name of the run; the run directory is `<runs-dir>/<name>` and must not exist.
    #[arg(long, value_name = "NAME", required_unless_present = "detach", conflicts_with = "detach")]
    pub run: Option<String>,
    /// Directory that holds the run directories.
    #[arg(long, value_name = "DIR", required_unless_present = "detach", conflicts_with = "detach")]
    pub runs_dir: Option<PathBuf>,
    /// The confinement of every bound program: `local` or `substrate`. No default.
    #[arg(long, value_enum, value_name = "LEVEL", required_unless_present = "detach", conflicts_with = "detach")]
    pub confinement: Option<Confinement>,
    /// The run settings (TOML, `docs/loom-run-settings.md`): models, budgets, the decider, the login file's path.
    #[arg(long, value_name = "FILE", required_unless_present = "detach", conflicts_with = "detach")]
    pub settings: Option<PathBuf>,
    /// The spend ledger shared by every run that counts against the same cap. Required when the run has a model node; a run without one opens none.
    #[arg(long, value_name = "FILE", conflicts_with = "detach")]
    pub spend_ledger: Option<PathBuf>,
    /// The spend cap in micro USD over all runs that share the ledger.
    #[arg(long, value_name = "N", required_unless_present = "detach", conflicts_with = "detach")]
    pub cap_micro_usd: Option<u64>,
    /// The most model calls of this run (steps and decisions together).
    #[arg(long, value_name = "N", required_unless_present = "detach", conflicts_with = "detach")]
    pub max_llm_calls: Option<u64>,
    /// The most wall seconds of this run.
    #[arg(long, value_name = "N", required_unless_present = "detach", conflicts_with = "detach")]
    pub max_wall_seconds: Option<u64>,
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
    /// With `--recorded`, the most live calls (no default: none named means no limit; a spent cap stops the run).
    #[arg(long, value_name = "N", conflicts_with = "detach", allow_hyphen_values = true)]
    pub live_cap: Option<String>,
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
fn root_protocol_of(runbook: &str, protocols: &std::path::Path) -> Result<String, String> {
    let entries = crate::list::read(protocols)?;
    let entry = entries.iter().find(|e| e.id == runbook).ok_or_else(|| format!("no runbook {runbook} under {}", protocols.display()))?;
    let revision = entry.revision.ok_or_else(|| format!("protocol/{runbook}.protocol.yaml has no protocol.revision"))?;
    Ok(format!("{}@{revision}", entry.protocol_id.replace('.', "-")))
}

/// The options of the loop from the command line; what is not given comes from the asset root.
pub fn args_of(base: &RunArgs, here: HereOptions) -> Result<Args, Refusal> {
    const USAGE: u8 = 64;
    if let Some(flag) = detach_only(base) {
        return Err((USAGE, format!("{flag} belongs to --detach: a run in this process takes no daemon and no context")));
    }
    // A cap that is named and cannot be read is refused by name, in our words: never read as none.
    let live_cap = here.live_cap.as_deref().map(|c| sdc_tools::named::whole("--live-cap", c)).transpose().map_err(|m| (USAGE, m))?;
    let ports = Ports {
        dry: here.dry,
        replay: Replay {
            recorded: here.recorded,
            on_divergence: here.on_divergence,
            live_adapter: here.live_adapter,
            live_cap,
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
    let missing = |flag: &str| (USAGE, format!("{flag} is required"));
    let intake = runbook::client::read_intake(&base.intake).map_err(|f| (EXIT_REFUSED, f.message))?;
    let assets = if here.bindings.is_none() || here.protocols.is_none() {
        Some(crate::assets::Lookup::from_process().locate().map_err(|e| (EXIT_REFUSED, format!("no asset root for the runbook's bindings and protocols ({e}): name --bindings and --protocols, or set RUNBOOK_HOME")))?)
    } else {
        None
    };
    let protocols = match (&here.protocols, &assets) {
        (Some(p), _) => p.clone(),
        (None, Some(a)) => a.protocol_dir(),
        (None, None) => unreachable!("the asset root is looked up when --protocols is not given"),
    };
    let bindings = match (&here.bindings, &assets) {
        (Some(b), _) => b.clone(),
        (None, Some(a)) => a.root.join("bindings"),
        (None, None) => unreachable!("the asset root is looked up when --bindings is not given"),
    };
    let root_protocol = match here.root_protocol {
        Some(r) => r,
        None => root_protocol_of(super::bugfix::runbook_of(&ports, &base.runbook)?, &protocols).map_err(|e| (EXIT_REFUSED, e))?,
    };
    Ok(Args {
        run: here.run.ok_or_else(|| missing("--run"))?,
        runs_dir: here.runs_dir.ok_or_else(|| missing("--runs-dir"))?,
        loom: here.loom,
        loom_sha256: here.loom_sha256,
        bindings,
        protocols,
        root_protocol,
        case: super::bugfix::case_of(&ports, here.case),
        intake,
        confinement: here.confinement.ok_or_else(|| missing("--confinement"))?,
        settings: here.settings.ok_or_else(|| missing("--settings"))?,
        spend_ledger: here.spend_ledger,
        cap_micro_usd: here.cap_micro_usd.ok_or_else(|| missing("--cap-micro-usd"))?,
        max_llm_calls: here.max_llm_calls.ok_or_else(|| missing("--max-llm-calls"))?,
        max_wall_seconds: here.max_wall_seconds.ok_or_else(|| missing("--max-wall-seconds"))?,
        ports,
    })
}

/// `runbook run <runbook>` without `--detach`.
pub fn command(base: RunArgs, here: HereOptions) -> ExitCode {
    let args = match args_of(&base, here) {
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
            ExitCode::from(refusal.exit_code())
        }
    }
}
