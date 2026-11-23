//! `runbook serve`: the daemon in the foreground.

use crate::api::{self, auth};
use crate::commands::{check_default_confinement, check_principal, check_terms, Config, Daemon, Policy};
use crate::engine::arbiter::Arbiter;
use crate::engine::Engine;
use crate::runtime::{Autonomy, Confinement, Runtime, Terms};
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
    /// The runbook directories of `runbook.toml` (`runbooks`), looked up before the runbook home. Not an option: the binary fills it from the file.
    #[arg(skip)]
    pub runbooks: Vec<PathBuf>,
    /// The weakest confinement level the daemon takes for a run.
    #[arg(long, value_name = "LEVEL", default_value = "linux", value_parser = ["local", "linux"])]
    pub min_confinement: String,
    /// The level of a submit that names none; never below the floor, or the daemon does not start. Else the
    /// `confinement` of `runbook.toml` (`local` is the level `local`, `substrate` the level `linux`), else linux.
    #[arg(long, value_name = "LEVEL", value_parser = ["local", "linux"])]
    pub default_confinement: Option<String>,
    /// The `confinement` of `runbook.toml`, as a level. Not an option: the binary fills it from the file.
    #[arg(skip)]
    pub file_confinement: Option<Confinement>,
    /// The autonomy level of a submit that names none: what the decider of the run takes over from a person.
    /// None given: a submit that names none has no level and every decision of its run waits for a client.
    /// It needs a decider kind (`--decider`), or the daemon does not start.
    #[arg(long, value_name = "LEVEL", value_parser = ["A0", "A1", "A2", "A3", "A4", "A5"])]
    pub autonomy: Option<String>,
    /// The decider kind of a submit that names none: one of the kinds `[deciders.<kind>]` of `runbook.toml`
    /// configures (`rule`, `model`); a name that is not one of them and the daemon does not start.
    #[arg(long, value_name = "KIND")]
    pub decider: Option<String>,
    /// The deciders `runbook.toml` configures (`[deciders.<kind>]`). Not an option: the binary fills it from the file.
    #[arg(skip)]
    pub arbiter: Option<Arc<dyn Arbiter>>,
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
    let default_confinement = match (&args.default_confinement, args.file_confinement) {
        (Some(level), _) => Some(Confinement::parse(level).ok_or_else(|| format!("--default-confinement {level}: not a level"))?),
        (None, file) => file,
    };
    if let Some(default) = default_confinement {
        check_default_confinement(floor, default)?;
    }
    // The deciders exist for the engine only: the stub runtime has none, so with it no level and no kind is configured.
    let kinds = if args.runtime == "loom" { args.arbiter.as_ref().map(|a| a.kinds()).unwrap_or_default() } else { Vec::new() };
    let autonomy = args.autonomy.as_deref().map(|a| Autonomy::parse(a).ok_or_else(|| format!("--autonomy {a}: not a level"))).transpose()?;
    check_terms(&Terms { autonomy, decider: args.decider.clone() }, &kinds).map_err(|e| format!("the default decider of the daemon (--autonomy, --decider): {e}"))?;
    let state = state_dir(args, env)?;
    let (runtime, runbook_dir): (Arc<dyn Runtime>, Option<PathBuf>) = if args.runtime == "loom" {
        let home = args.runbook_home.clone().ok_or("--runtime loom needs a runbook home: give --runbook-home, set RUNBOOK_HOME, or run inside a checkout")?;
        if !home.join("protocol").is_dir() || !home.join("bindings").is_dir() {
            return Err(format!("--runbook-home {}: holds no protocol/ and bindings/ directories", home.display()));
        }
        crate::runbooks::load(&args.runbooks, Some(&home))?;
        let mut engine = Engine::new(home, state.clone()).with_runbook_dirs(args.runbooks.clone());
        if let Some(arbiter) = &args.arbiter {
            engine = engine.with_arbiter(arbiter.clone());
        }
        (Arc::new(engine), None)
    } else {
        let stub = StubRuntime::new();
        for name in presets {
            stub.script(name, StubRuntime::demo_script());
        }
        stub.script(STUB_DECISION, StubRuntime::decision_script());
        (Arc::new(stub), args.runbook_dir.clone())
    };
    let cfg = Config { state_dir: state, confinement_floor: floor, context_roots: args.context_root.clone(), runbook_dir, principal };
    let policy = Policy { default_confinement, autonomy, decider: args.decider.clone(), kinds };
    let daemon = Daemon::start_with(cfg, policy, runtime).map_err(|e| format!("state directory: {e}"))?;
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
