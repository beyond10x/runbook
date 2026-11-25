//! `runbook serve`: the daemon in the foreground.

use crate::api::{self, auth};
use crate::caps::{CapPolicy, Caps};
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
    /// The runbook directories of `runbook.toml` (`runbooks`), looked up before the runbook home. Not an option: the binary fills it from the file.
    #[arg(skip)]
    pub runbooks: Vec<PathBuf>,
    /// The weakest confinement level the daemon takes for a run.
    #[arg(long, value_name = "LEVEL", default_value = "linux", value_parser = ["local", "linux"])]
    pub min_confinement: String,
    /// The level of a submit that names none; never below the floor, or the daemon does not start. Else the
    /// `confinement` of `runbook.toml` (`local` is the level `local`, `substrate` the level `linux`). With neither, a submit that names no level is refused by name.
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
    /// The model routes `runbook.toml` declares (`[gateways.*]`, `[routes.*]`, `[route_policy]`), their token files
    /// read. Not an option: the binary fills it from the file.
    #[arg(skip)]
    pub routes: Arc<crate::routes::RouteTable>,
    /// The route table of the runs whose decider kind is `model` (`crate::relay`): the daemon serves the relay and
    /// such a run's model calls go through it. Not an option: the binary fills it from the `[gateway]` of the file.
    #[arg(skip)]
    pub route_table: Option<crate::relay::RouteTable>,
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
    /// The most spend, micro USD, of one run, when its submit names none: each run has its own cap, and the spend of other runs of the runbook never reduces it. Required with `--runtime loom`; no default.
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
    /// The most seconds a drain takes. On SIGTERM or SIGINT the daemon refuses new runs and waits for the
    /// live ones to end; a second signal, or this many seconds, ends it at once and the live runs end as
    /// orphaned. Default: no bound.
    #[arg(long, value_name = "SECONDS")]
    pub drain_seconds: Option<u64>,
}

/// The caps policy of the daemon, from its options: the three caps are required and have no default;
/// each ceiling is the cap itself unless it is given. A missing cap is refused naming its option.
pub fn cap_policy(args: &ServeArgs) -> Result<CapPolicy, String> {
    let given = [("--max-wall-seconds", args.max_wall_seconds), ("--max-llm-calls", args.max_llm_calls), ("--cap-micro-usd", args.cap_micro_usd)];
    let missing: Vec<&str> = given.iter().filter(|(_, v)| v.is_none()).map(|(k, _)| *k).collect();
    let (Some(wall), Some(calls), Some(spend)) = (args.max_wall_seconds, args.max_llm_calls, args.cap_micro_usd) else {
        return Err(format!("{} required with --runtime loom: a daemon run is never unbound, and a cap has no default", missing.join(", ")));
    };
    let defaults = Caps { max_wall_seconds: wall, max_llm_calls: calls, cap_micro_usd: spend };
    let ceilings = Caps {
        max_wall_seconds: args.max_wall_seconds_ceiling.unwrap_or(wall),
        max_llm_calls: args.max_llm_calls_ceiling.unwrap_or(calls),
        cap_micro_usd: args.cap_micro_usd_ceiling.unwrap_or(spend),
    };
    CapPolicy::new(defaults, ceilings)
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
    /// The relay of the runs on a route, when the daemon has a route table: served beside the API.
    pub relay: Option<Arc<crate::relay::Relay>>,
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
    let state = std::path::absolute(state_dir(args, env)?).map_err(|_| "the state directory cannot be made absolute: the daemon has no current directory".to_string())?;
    let mut relay = None;
    let (runtime, runbook_dir): (Arc<dyn Runtime>, Option<PathBuf>) = if args.runtime == "loom" {
        let home = args.runbook_home.clone().ok_or("--runtime loom needs a runbook home: give --runbook-home, set RUNBOOK_HOME, or run inside a checkout")?;
        let home = std::path::absolute(home).map_err(|_| "the runbook home cannot be made absolute: the daemon has no current directory".to_string())?;
        if !home.join("protocol").is_dir() || !home.join("bindings").is_dir() {
            return Err(format!("--runbook-home {}: holds no protocol/ and bindings/ directories", home.display()));
        }
        let caps = cap_policy(args)?;
        crate::runbooks::load(&args.runbooks, Some(&home))?;
        let mut engine = Engine::new(home, state.clone(), caps).with_runbook_dirs(args.runbooks.clone());
        if let Some(arbiter) = &args.arbiter {
            engine = engine.with_arbiter(arbiter.clone());
        }
        if let Some(table) = args.route_table.clone().filter(|t| !t.is_empty()) {
            let r = crate::relay::Relay::new();
            engine = engine.with_routes(r.clone(), table);
            relay = Some(r);
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
    let policy = Policy { default_confinement, autonomy, decider: args.decider.clone(), kinds, routes: args.routes.clone() };
    let daemon = Daemon::start_with(cfg, policy, runtime).map_err(|e| format!("state directory: {e}"))?;
    Ok(Prepared { addr, token, daemon, relay })
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

/// A line on standard error that never fails the caller: standard error may be a closed pipe (a `| tee`
/// that ended), and a write to it must not stop the drain.
fn note(line: std::fmt::Arguments) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr(), "{line}");
}

/// How the daemon stops. The first signal begins the drain (`Daemon::begin_drain`): a submit is refused,
/// reads and decisions are served, and the live runs go on. The future ends, and with it the server, when no
/// run is live (`daemon.drained`), or at once on a second signal or when `bound` has run out; then every
/// live run ends as orphaned with the event that the daemon was stopped while it ran. It gives the number of
/// runs it ended as orphaned.
async fn stop_after_drain(daemon: Daemon, bound: Option<Duration>, mut signals: StopSignals) -> usize {
    signals.next().await;
    let live = match daemon.begin_drain() {
        Ok(live) => live,
        Err(e) => {
            note(format_args!("runbook serve: the journal could not record the drain: {e:?}"));
            return daemon.orphan_live_runs();
        }
    };
    if !live.is_empty() {
        note(format_args!("runbook: draining: {} live run(s) go on to their end and no new run is taken; a second signal stops at once", live.len()));
    }
    let mut changes = daemon.subscribe();
    // A bound that overflows an instant is no bound.
    let deadline = bound.and_then(|b| tokio::time::Instant::now().checked_add(b));
    loop {
        if daemon.live_runs().is_empty() {
            if let Err(e) = daemon.finish_drain() {
                note(format_args!("runbook serve: the journal could not record the end of the drain: {e:?}"));
            }
            return 0;
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
                note(format_args!("runbook: stopped at once on a second signal: the live runs end as orphaned"));
                break;
            }
            () = bound_over => {
                note(format_args!("runbook: stopped at once: the drain bound ran out: the live runs end as orphaned"));
                break;
            }
        }
    }
    daemon.orphan_live_runs()
}

/// How the stop ended: the runs it left orphaned, and whether the stop future itself failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopEnd {
    pub orphaned: usize,
    pub failed: bool,
}

/// Run `stop` (it gives the number of runs it orphaned) on a task of its own. A panic in it is caught: every
/// live run is ended as orphaned with the reason that the daemon was stopped while it ran, and the end says
/// the stop failed. A stop that dies must not leave the process holding runs it will not wait for.
pub async fn guarded_stop(daemon: &Daemon, stop: impl std::future::Future<Output = usize> + Send + 'static) -> StopEnd {
    match tokio::spawn(stop).await {
        Ok(orphaned) => StopEnd { orphaned, failed: false },
        Err(e) => {
            note(format_args!("runbook serve: the stop failed ({e}): the live runs end as orphaned"));
            StopEnd { orphaned: daemon.orphan_live_runs(), failed: true }
        }
    }
}

/// The exit code of `runbook serve` when it was stopped while at least one run was live and each such run
/// ended as orphaned (a second signal, `--drain-seconds`, or a stop that failed). 0 is a stop that left no
/// run behind; 1 is a start that was refused or a server that failed; 64 and 2 are the usage errors.
pub const EXIT_ORPHANED: u8 = 75;

/// The code `serve` exits with, given how it ended.
fn exit_code(served: &Result<(), String>, end: StopEnd) -> ExitCode {
    match served {
        Err(_) => ExitCode::from(1),
        Ok(()) if end.orphaned > 0 => ExitCode::from(EXIT_ORPHANED),
        Ok(()) if end.failed => ExitCode::from(1),
        Ok(()) => ExitCode::SUCCESS,
    }
}

/// Run the daemon until SIGINT or SIGTERM and then the end of its drain. Exit 1 when the start is refused,
/// `EXIT_ORPHANED` when the stop left a live run orphaned, else 0.
pub fn run(args: ServeArgs, presets: &[&str]) -> ExitCode {
    let prepared = match prepare(&args, &Env::from_process(), presets) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("runbook serve: {e}");
            return ExitCode::from(1);
        }
    };
    let Prepared { addr, token, daemon, relay } = prepared;
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
            note(format_args!("runbook: this daemon runs the Loom runtime: runs execute through Loom in this process"));
        } else {
            note(format_args!("runbook: this daemon runs the stub runtime: runs are scripted and nothing is executed"));
        }
        // The relay of the runs on a route answers on this listener when it is loopback: its address goes into every
        // route file. On any other listener it is not served, and a run on a route does not start.
        if let Some(relay) = &relay {
            relay.set_listen(bound);
            if !relay.serves() {
                note(format_args!("runbook: the relay of model calls serves a loopback listener only: a run whose decider is a model does not start on {bound}"));
            }
        }
        println!("runbook: listening on http://{bound}");
        let mut extra = args.mcp.then(|| crate::mcp::router(daemon.clone()));
        if let Some(relay) = relay.as_ref().filter(|r| r.serves()) {
            extra = Some(match extra {
                Some(mcp) => mcp.merge(relay.router()),
                None => relay.router(),
            });
        }
        let end = Arc::new(std::sync::Mutex::new(StopEnd { orphaned: 0, failed: false }));
        let stopping = {
            let (daemon, end) = (daemon.clone(), end.clone());
            let bound = args.drain_seconds.map(Duration::from_secs);
            async move {
                let how = guarded_stop(&daemon, stop_after_drain(daemon.clone(), bound, signals)).await;
                *end.lock().unwrap_or_else(|p| p.into_inner()) = how;
            }
        };
        let served = api::serve_all(listener, daemon.clone(), token.as_deref(), api::Streams::default(), args.ui, extra, stopping).await.map_err(|e| e.to_string());
        let how = *end.lock().unwrap_or_else(|p| p.into_inner());
        Ok::<_, String>((served, how))
    });
    daemon.stop();
    match served {
        Ok((served, how)) => {
            if let Err(e) = &served {
                note(format_args!("runbook serve: {e}"));
            }
            exit_code(&served, how)
        }
        Err(e) => {
            note(format_args!("runbook serve: {e}"));
            ExitCode::from(1)
        }
    }
}
