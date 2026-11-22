//! `runbook serve`: the daemon in the foreground.

use crate::api::{self, auth};
use crate::commands::{Config, Daemon};
use crate::runtime::Confinement;
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
    /// The weakest confinement level the daemon takes for a run.
    #[arg(long, value_name = "LEVEL", default_value = "linux", value_parser = ["local", "linux"])]
    pub min_confinement: String,
    /// A file holding the bearer token; its mode must not give anything to others. Also read from RUNBOOK_TOKEN.
    #[arg(long, value_name = "FILE")]
    pub token_file: Option<PathBuf>,
    /// Serve the web UI, compiled into this binary, beside the API (`GET /`).
    #[arg(long)]
    pub ui: bool,
    /// Also serve MCP over streamable HTTP at /mcp, behind the same token check.
    #[arg(long)]
    pub mcp: bool,
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

/// Everything that can refuse the start, in order: the token, the listen address against the token
/// (the security floor), then the state directory. Nothing is bound or written before the floor holds.
pub fn prepare(args: &ServeArgs, env: &Env, presets: &[&str]) -> Result<Prepared, String> {
    let token = auth::load_token(args.token_file.as_deref(), env.token.clone())?;
    let addr = args
        .listen
        .to_socket_addrs()
        .map_err(|e| format!("--listen {}: {e}", args.listen))?
        .next()
        .ok_or_else(|| format!("--listen {}: no address", args.listen))?;
    auth::check_listen(&addr, token.as_deref())?;
    let floor = Confinement::parse(&args.min_confinement).ok_or_else(|| format!("--min-confinement {}: not a level", args.min_confinement))?;
    let stub = StubRuntime::new();
    for name in presets {
        stub.script(name, StubRuntime::demo_script());
    }
    stub.script(STUB_DECISION, StubRuntime::decision_script());
    let cfg = Config {
        state_dir: state_dir(args, env)?,
        confinement_floor: floor,
        context_roots: args.context_root.clone(),
        runbook_dir: args.runbook_dir.clone(),
    };
    let daemon = Daemon::start(cfg, Arc::new(stub)).map_err(|e| format!("state directory: {e}"))?;
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
        eprintln!("runbook: this daemon runs the stub runtime: runs are scripted and nothing is executed");
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
