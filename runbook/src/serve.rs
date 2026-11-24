//! `runbook serve`: the daemon in the foreground.

use crate::api::{self, auth};
use crate::commands::{check_principal, Config, Daemon};
use crate::engine::Engine;
use crate::runtime::{Confinement, Runtime};
use crate::stub::StubRuntime;
use clap::Args;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

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
    /// The most seconds a drain takes. On SIGTERM or SIGINT the daemon refuses new runs and waits for the
    /// live ones to end; a second signal, or this many seconds, ends it at once and the live runs end as
    /// orphaned. Default: no bound.
    #[arg(long, value_name = "SECONDS")]
    pub drain_seconds: Option<u64>,
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
        (Arc::new(Engine::new(home, state.clone())), None)
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

/// SIGTERM and SIGINT as one stream of stop signals. Both handlers are installed once, so a second signal
/// is seen while the drain waits. A handler that cannot be installed never fires.
struct StopSignals {
    term: Option<tokio::signal::unix::Signal>,
    int: Option<tokio::signal::unix::Signal>,
}

async fn received(s: &mut Option<tokio::signal::unix::Signal>) {
    if let Some(s) = s {
        if s.recv().await.is_some() {
            return;
        }
    }
    std::future::pending::<()>().await
}

impl StopSignals {
    fn install() -> StopSignals {
        use tokio::signal::unix::{signal, SignalKind};
        StopSignals { term: signal(SignalKind::terminate()).ok(), int: signal(SignalKind::interrupt()).ok() }
    }

    async fn next(&mut self) {
        tokio::select! {
            () = received(&mut self.term) => {}
            () = received(&mut self.int) => {}
        }
    }
}

/// How the daemon stops. The first signal begins the drain (`Daemon::begin_drain`): a submit is refused,
/// reads and decisions are served, and the live runs go on. The future ends, and with it the server, when no
/// run is live (`daemon.drained`), or at once on a second signal or when `bound` has run out; then every
/// live run ends as orphaned with the event that the daemon was stopped while it ran.
async fn stop_after_drain(daemon: Daemon, bound: Option<Duration>, mut signals: StopSignals) {
    signals.next().await;
    let live = match daemon.begin_drain() {
        Ok(live) => live,
        Err(e) => {
            eprintln!("runbook serve: the journal could not record the drain: {e:?}");
            daemon.orphan_live_runs();
            return;
        }
    };
    if !live.is_empty() {
        eprintln!("runbook: draining: {} live run(s) go on to their end and no new run is taken; a second signal stops at once", live.len());
    }
    let mut changes = daemon.subscribe();
    let deadline = bound.map(|b| tokio::time::Instant::now() + b);
    loop {
        if daemon.live_runs().is_empty() {
            if let Err(e) = daemon.finish_drain() {
                eprintln!("runbook serve: the journal could not record the end of the drain: {e:?}");
            }
            return;
        }
        let bound_over = async {
            match deadline {
                Some(at) => tokio::time::sleep_until(at).await,
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            _ = changes.changed() => {}
            () = signals.next() => {
                eprintln!("runbook: stopped at once on a second signal: the live runs end as orphaned");
                break;
            }
            () = bound_over => {
                eprintln!("runbook: stopped at once: the drain bound ran out: the live runs end as orphaned");
                break;
            }
        }
    }
    daemon.orphan_live_runs();
}

/// Run the daemon until SIGINT or SIGTERM and then the end of its drain. Exit 1 when the start is refused.
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
        // Before the address is announced: a signal from then on is a stop, never the default death.
        let signals = StopSignals::install();
        let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| format!("cannot listen on {addr}: {e}"))?;
        let bound = listener.local_addr().map_err(|e| e.to_string())?;
        if args.runtime == "loom" {
            eprintln!("runbook: this daemon runs the Loom runtime: runs execute through Loom in this process");
        } else {
            eprintln!("runbook: this daemon runs the stub runtime: runs are scripted and nothing is executed");
        }
        println!("runbook: listening on http://{bound}");
        let extra = args.mcp.then(|| crate::mcp::router(daemon.clone()));
        let stopping = stop_after_drain(daemon.clone(), args.drain_seconds.map(Duration::from_secs), signals);
        api::serve_all(listener, daemon.clone(), token.as_deref(), api::Streams::default(), args.ui, extra, stopping).await.map_err(|e| e.to_string())
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
