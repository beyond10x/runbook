//! `runbook serve`: the daemon in the foreground.

use crate::api::{self, auth};
use crate::caps::{CapPolicy, Caps};
use crate::commands::{check_principal, Config, Daemon};
use crate::engine::Engine;
use crate::runtime::{Confinement, Runtime};
use crate::stub::StubRuntime;
use clap::Args;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

pub const DEFAULT_LISTEN: &str = "127.0.0.1:7430";

#[derive(Args, Debug, Clone)]
pub struct ServeArgs {
    /// Address to listen on. Any address that is not loopback needs a token.
    #[arg(long, value_name = "ADDR", default_value = DEFAULT_LISTEN)]
    pub listen: String,
    /// Where the daemon keeps its journal, the record of each run and its workspace
    /// (default `$XDG_STATE_HOME/runbook`, else `~/.local/state/runbook`).
    #[arg(long, value_name = "DIR")]
    pub state_dir: Option<PathBuf>,
    /// A directory a run's context may be copied from; repeatable. None given: no context from a path.
    #[arg(long = "context-root", value_name = "DIR")]
    pub context_root: Vec<PathBuf>,
    /// A directory of runbooks a submit may name besides the presets.
    #[arg(long, value_name = "DIR")]
    pub runbook_dir: Option<PathBuf>,
    /// The runtime runs take: `stub` (scripted, nothing is executed) or `loom` (Loom's library
    /// entry in this process, `docs/engine.md`).
    #[arg(long, value_name = "RUNTIME", default_value = "stub", value_parser = ["loom", "stub"])]
    pub runtime: String,
    /// The runbook home the Loom runtime reads a runbook from: a directory with `protocol/` and
    /// `bindings/`. Default: the asset root (`RUNBOOK_HOME`, an install, or the checkout).
    #[arg(long, value_name = "DIR")]
    pub runbook_home: Option<PathBuf>,
    /// The weakest confinement level the daemon takes for a run.
    #[arg(long, value_name = "LEVEL", default_value = "linux", value_parser = ["local", "linux"])]
    pub min_confinement: String,
    /// A file holding the bearer token; its mode must not give anything to others. Also read from RUNBOOK_TOKEN.
    #[arg(long, value_name = "FILE")]
    pub token_file: Option<PathBuf>,
    /// Who the daemon acts as: `person:<id>` or `agent:<id>`. Required, no default and no environment variable;
    /// fixed for the life of the process. The runtime is given this principal for every answer.
    #[arg(long, value_name = "PRINCIPAL")]
    pub principal: Option<String>,
    /// Serve the web UI, compiled into this binary, beside the API (`GET /`).
    #[arg(long)]
    pub ui: bool,
    /// Also serve MCP over streamable HTTP at /mcp, behind the same token check.
    #[arg(long)]
    pub mcp: bool,
    /// The most wall seconds of a run, when its submit names none. Required with `--runtime loom`; no default.
    #[arg(long, value_name = "SECONDS", value_parser = clap::value_parser!(u64).range(1..))]
    pub max_wall_seconds: Option<u64>,
    /// The most model calls of a run, when its submit names none. Required with `--runtime loom`; no default.
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u64).range(1..))]
    pub max_llm_calls: Option<u64>,
    /// The most spend, micro USD, of the runs of one runbook together, when a submit names none. Required with `--runtime loom`; no default.
    #[arg(long, value_name = "MICRO_USD", value_parser = clap::value_parser!(u64).range(1..))]
    pub cap_micro_usd: Option<u64>,
    /// The most a submit may name as `max_wall_seconds`. Default: `--max-wall-seconds`.
    #[arg(long, value_name = "SECONDS", value_parser = clap::value_parser!(u64).range(1..))]
    pub max_wall_seconds_ceiling: Option<u64>,
    /// The most a submit may name as `max_llm_calls`. Default: `--max-llm-calls`.
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u64).range(1..))]
    pub max_llm_calls_ceiling: Option<u64>,
    /// The most a submit may name as `cap_micro_usd`. Default: `--cap-micro-usd`.
    #[arg(long, value_name = "MICRO_USD", value_parser = clap::value_parser!(u64).range(1..))]
    pub cap_micro_usd_ceiling: Option<u64>,
}

/// The caps policy of the daemon, from its options. Skeleton: not read yet.
pub fn cap_policy(_args: &ServeArgs) -> Result<CapPolicy, String> {
    let unbounded = Caps { max_wall_seconds: u64::MAX, max_llm_calls: u64::MAX, cap_micro_usd: u64::MAX };
    CapPolicy::fixed(unbounded)
}

/// What `serve` reads from the process environment.
#[derive(Default, Clone, Debug)]
pub struct Env {
    pub xdg_state_home: Option<String>,
    pub home: Option<String>,
    pub token: Option<String>,
}

impl Env {
    pub fn from_process() -> Env {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        Env { xdg_state_home: var("XDG_STATE_HOME"), home: var("HOME"), token: var("RUNBOOK_TOKEN") }
    }
}

pub struct Prepared {
    pub addr: SocketAddr,
    pub token: Option<String>,
    pub daemon: Daemon,
}

/// The runbook name of the stub's run that waits on two decisions (`StubRuntime::decision_script`).
pub const STUB_DECISION: &str = "stub-decision";

/// The state directory: `--state-dir`, else `$XDG_STATE_HOME/runbook`, else `~/.local/state/runbook`.
pub fn state_dir(args: &ServeArgs, env: &Env) -> Result<PathBuf, String> {
    if let Some(d) = &args.state_dir {
        return Ok(d.clone());
    }
    match (&env.xdg_state_home, &env.home) {
        (Some(x), _) => Ok(PathBuf::from(x).join("runbook")),
        (None, Some(h)) => Ok(PathBuf::from(h).join(".local/state/runbook")),
        (None, None) => Err("no state directory: give --state-dir, or set XDG_STATE_HOME or HOME".to_string()),
    }
}

/// Everything that can refuse the start, in order: the principal, the token, the listen address against
/// the token (the security floor), then the state directory. Nothing is bound or written before the floor holds.
pub fn prepare(args: &ServeArgs, env: &Env, presets: &[&str]) -> Result<Prepared, String> {
    let principal = args.principal.clone().ok_or("--principal is required: person:<id> or agent:<id>, the principal the daemon acts as (no default)")?;
    check_principal(&principal)?;
    let token = auth::load_token(args.token_file.as_deref(), env.token.clone())?;
    auth::check_token_length(token.as_deref())?;
    let addr = args
        .listen
        .to_socket_addrs()
        .map_err(|e| format!("--listen {}: {e}", args.listen))?
        .next()
        .ok_or_else(|| format!("--listen {}: no address", args.listen))?;
    auth::check_listen(&addr, token.as_deref())?;
    let floor = Confinement::parse(&args.min_confinement).ok_or_else(|| format!("--min-confinement {}: not a level", args.min_confinement))?;
    let state = state_dir(args, env)?;
    let (runtime, runbook_dir): (Arc<dyn Runtime>, Option<PathBuf>) = if args.runtime == "loom" {
        let home = args.runbook_home.clone().ok_or("--runtime loom needs a runbook home: give --runbook-home, set RUNBOOK_HOME, or run inside a checkout")?;
        if !home.join("protocol").is_dir() || !home.join("bindings").is_dir() {
            return Err(format!("--runbook-home {}: holds no protocol/ and bindings/ directories", home.display()));
        }
        (Arc::new(Engine::new(home, state.clone(), cap_policy(args)?)), None)
    } else {
        let stub = StubRuntime::new();
        for name in presets {
            stub.script(name, StubRuntime::demo_script());
        }
        stub.script(STUB_DECISION, StubRuntime::decision_script());
        (Arc::new(stub), args.runbook_dir.clone())
    };
    let cfg = Config { state_dir: state, confinement_floor: floor, context_roots: args.context_root.clone(), runbook_dir, principal };
    let daemon = Daemon::start(cfg, runtime).map_err(|e| format!("state directory: {e}"))?;
    Ok(Prepared { addr, token, daemon })
}

async fn shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};
    let term = signal(SignalKind::terminate());
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = async { match term { Ok(mut t) => { t.recv().await; } Err(_) => std::future::pending().await } } => {}
    }
}

/// Run the daemon until SIGINT or SIGTERM. Exit 1 when the start is refused.
pub fn run(args: ServeArgs, presets: &[&str]) -> ExitCode {
    let prepared = match prepare(&args, &Env::from_process(), presets) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("runbook serve: {e}");
            return ExitCode::from(1);
        }
    };
    let Prepared { addr, token, daemon } = prepared;
    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("runbook serve: {e}");
            return ExitCode::from(1);
        }
    };
    let served = runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| format!("cannot listen on {addr}: {e}"))?;
        let bound = listener.local_addr().map_err(|e| e.to_string())?;
        if args.runtime == "loom" {
            eprintln!("runbook: this daemon runs the Loom runtime: runs execute through Loom in this process");
        } else {
            eprintln!("runbook: this daemon runs the stub runtime: runs are scripted and nothing is executed");
        }
        println!("runbook: listening on http://{bound}");
        let extra = args.mcp.then(|| crate::mcp::router(daemon.clone()));
        api::serve_all(listener, daemon.clone(), token.as_deref(), api::Streams::default(), args.ui, extra, shutdown_signal()).await.map_err(|e| e.to_string())
    });
    daemon.stop();
    match served {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("runbook serve: {e}");
            ExitCode::from(1)
        }
    }
}
