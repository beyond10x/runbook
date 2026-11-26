//! The CLI as a client of the daemon: `runbook run --detach`, `runbook runs ...`, `runbook decide`, `runbook delegate`,
//! `runbook catalog`.
//!
//! Each command is one call of a route of the daemon (`api`), with the input types of the server's own
//! routes, and prints what the route returns: text lines by default, the route's JSON with `--json`.
//! `runbook run` without `--detach` is not a client: the binary runs the case in its own process
//! (`loom_run/here.rs`) and only the options and the intake reader are shared. The client opens no
//! session: `SubmitRun` takes none, and a run without one belongs to the daemon alone.
//!
//! The token is read from a file (`--token-file`, `RUNBOOK_TOKEN_FILE`) whose mode gives nothing to
//! group or others; it is never taken from argv or a plain environment value, and `http` never lets it
//! out of a text.

pub mod api;
pub mod exit;
pub mod http;
pub mod sse;
pub mod url;

use crate::commands::{AnswerDecision, CancelRun, Delegate, SubmitRun, MAX_INTAKE_BYTES};
use api::Api;
use clap::{Args, Subcommand};
use exit::Failure;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

const RESUME_BACKOFF: Duration = Duration::from_millis(100);

/// Where the daemon is and how to prove who asks.
#[derive(Args, Debug, Clone)]
pub struct Remote {
    /// The daemon's URL, `http://host:port` (plain HTTP only); also read from RUNBOOK_DAEMON.
    #[arg(long, value_name = "URL")]
    pub daemon: Option<String>,
    /// A file holding the bearer token, mode 600 (also read from RUNBOOK_TOKEN_FILE); the token is
    /// taken from nothing else.
    #[arg(long, value_name = "FILE")]
    pub token_file: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct RunArgs {
    /// The runbook to run, by name (`sdc`, `sdc-bugfix`, or one under the daemon's runbook directory).
    pub runbook: String,
    /// The intake: a file, or `-` for standard input.
    #[arg(long, value_name = "FILE|-")]
    pub intake: String,
    /// The context directory; the daemon copies it into the run's workspace and needs it under one of its context roots.
    #[arg(long, value_name = "DIR")]
    pub context: Option<PathBuf>,
    /// A setting of the run, `KEY=VALUE`; repeatable, each key once. With `--detach` it goes to the daemon; in process it is a condition of the replay (`--recorded`).
    #[arg(long = "set", value_name = "KEY=VALUE")]
    pub set: Vec<String>,
    /// The model route of the run, by the name `runbook.toml` (`[routes.<name>]`) or the daemon declares; without it the route policy's default. In process, a route that cannot answer now is refused.
    #[arg(long = "model-route", value_name = "NAME")]
    pub model_route: Option<String>,
    /// Submit the run to the daemon, print its id and return at once.
    #[arg(long)]
    pub detach: bool,
    /// Print what the route returns as JSON.
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub remote: Remote,
}

#[derive(Subcommand, Debug)]
pub enum RunsCmd {
    /// List the daemon's runs, newest first.
    List {
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        remote: Remote,
    },
    /// Show one run with its open decision requests.
    Show {
        run: String,
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        remote: Remote,
    },
    /// Print the events of a run and follow the stream until the run ends.
    Events {
        run: String,
        /// Print what exists and return, even when the run is live.
        #[arg(long)]
        no_follow: bool,
        /// One JSON row per line instead of the lines of the stream.
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        remote: Remote,
    },
    /// Cancel a run that has not ended.
    Cancel {
        run: String,
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        remote: Remote,
    },
    /// List the open decision requests of a run: the request ids `decide` takes.
    Decisions {
        run: String,
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        remote: Remote,
    },
}

#[derive(Args, Debug)]
pub struct DecideArgs {
    pub run: String,
    /// The request's id, as `runs show` and `runs decisions` print it.
    pub request: String,
    /// The answer: the letter of one of the request's options.
    pub answer: String,
    /// Who answers, recorded with the answer (default: $USER, else `cli`).
    #[arg(long, value_name = "NAME")]
    pub by: Option<String>,
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub remote: Remote,
}

#[derive(Args, Debug)]
pub struct DelegateArgs {
    /// The request: a file holding the work asked for, as text, or `-` for standard input.
    #[arg(value_name = "REQUEST_FILE|-")]
    pub request: String,
    /// The model route of the run, by the name the daemon declares it under.
    #[arg(long, value_name = "ROUTE")]
    pub route: Option<String>,
    /// The runbook to run with the request as its intake; without one the daemon's free run does the work.
    #[arg(long, value_name = "RUNBOOK")]
    pub protocol: Option<String>,
    /// A directory the daemon copies into the run's workspace; it must lie under one of the daemon's context roots.
    #[arg(long, value_name = "DIR")]
    pub workspace: Option<PathBuf>,
    /// A setting of the run, `KEY=VALUE`; repeatable, each key once.
    #[arg(long = "set", value_name = "KEY=VALUE")]
    pub set: Vec<String>,
    /// Print what the route returns as JSON.
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub remote: Remote,
}

#[derive(Args, Debug)]
pub struct CatalogArgs {
    /// Print what the route returns as JSON.
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub remote: Remote,
}

/// `runbook catalog`: one call of `GET /daemon/views/catalog`; prints a line per route, per runbook (with the refusal of
/// one whose route entries cannot be admitted) and of the delegation's terms.
pub fn catalog(args: CatalogArgs) -> ExitCode {
    done("catalog", catalog_print(args))
}

pub fn run(args: RunArgs) -> ExitCode {
    done("run", run_detached(args))
}

/// `runbook delegate`: one call of `POST /daemon/commands/delegate`; prints the run's id (read its result with
/// `GET /daemon/views/delegation-result`).
pub fn delegate(args: DelegateArgs) -> ExitCode {
    done("delegate", delegate_run(args))
}

pub fn runs(cmd: RunsCmd) -> ExitCode {
    match cmd {
        RunsCmd::List { json, remote } => done("runs list", list(&remote, json)),
        RunsCmd::Show { run, json, remote } => done("runs show", show(&remote, &run, json)),
        RunsCmd::Events { run, no_follow, json, remote } => done("runs events", events(&remote, &run, no_follow, json)),
        RunsCmd::Cancel { run, json, remote } => done("runs cancel", cancel(&remote, &run, json)),
        RunsCmd::Decisions { run, json, remote } => done("runs decisions", decisions(&remote, &run, json)),
    }
}

pub fn decide(args: DecideArgs) -> ExitCode {
    done("decide", decide_run(args))
}

fn done(command: &str, result: Result<(), Failure>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(f) => {
            if !f.message.is_empty() {
                eprintln!("runbook {command}: {}", f.message);
            }
            ExitCode::from(f.code)
        }
    }
}

fn say(text: &str) -> Result<(), Failure> {
    let mut out = io::stdout().lock();
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => Ok(()),
        // A closed pipe (`| head`) ends the command quietly.
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Err(Failure::new(0, "")),
        Err(e) => Err(Failure::new(exit::OTHER, format!("cannot write to standard output: {e}"))),
    }
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// The token from its file: the mode is read from the opened file, and group or others may hold no bit.
pub fn load_token(path: &Path) -> Result<http::Token, Failure> {
    use std::os::unix::fs::PermissionsExt;
    let config = |why: String| Failure::new(exit::CONFIG, format!("token file {}: {why}", path.display()));
    let mut file = std::fs::File::open(path).map_err(|e| config(e.to_string()))?;
    let meta = file.metadata().map_err(|e| config(e.to_string()))?;
    if !meta.is_file() {
        return Err(config("not a regular file".to_string()));
    }
    let mode = meta.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(config(format!("has mode {mode:04o}: it must not be readable by group or others (chmod 600)")));
    }
    let mut raw = String::new();
    (&mut file).take(8 * 1024).read_to_string(&mut raw).map_err(|e| config(e.to_string()))?;
    let token = raw.trim_end_matches(['\n', '\r']);
    if token.is_empty() {
        return Err(config("is empty".to_string()));
    }
    if token.chars().any(char::is_control) {
        return Err(config("holds a control character".to_string()));
    }
    Ok(http::Token::new(token.to_string()))
}

fn connect(remote: &Remote) -> Result<Api, Failure> {
    let url = remote.daemon.clone().or_else(|| env("RUNBOOK_DAEMON")).ok_or_else(|| Failure::usage("a daemon is needed: give --daemon <url> or set RUNBOOK_DAEMON"))?;
    let target = url::parse(&url)?;
    let token_file = remote.token_file.clone().or_else(|| env("RUNBOOK_TOKEN_FILE").map(PathBuf::from));
    let token = token_file.as_deref().map(load_token).transpose()?;
    if token.is_some() && !target.loopback {
        eprintln!("runbook: warning: the token goes over plain http to {}: use a tunnel (ssh -L) or a network you trust", target.host);
    }
    Ok(Api { client: http::Client::new(target, token) })
}

// ---- run --detach ----

fn run_detached(args: RunArgs) -> Result<(), Failure> {
    let settings = settings_of(&args.set)?;
    let api = connect(&args.remote)?;
    let intake = read_intake(&args.intake)?;
    let context_path = match &args.context {
        None => None,
        Some(dir) => Some(absolute_dir("--context", dir)?),
    };
    let input = SubmitRun { runbook_id: args.runbook, intake, context_path, settings, model_route: args.model_route, ..SubmitRun::default() };
    let reply = api.command("submit-run", &input)?;
    if args.json {
        return say(&format!("{}\n", reply.body.trim_end()));
    }
    let run_id = run_id_of(&reply).ok_or_else(|| Failure::new(exit::OTHER, "the answer of submit-run names no run"))?;
    say(&format!("{run_id}\n"))
}

fn settings_of(set: &[String]) -> Result<BTreeMap<String, String>, Failure> {
    let mut settings = BTreeMap::new();
    for item in set {
        let (key, value) = item.split_once('=').filter(|(k, _)| !k.is_empty()).ok_or_else(|| Failure::usage(format!("--set {item}: a setting is KEY=VALUE")))?;
        if settings.insert(key.to_string(), value.to_string()).is_some() {
            return Err(Failure::usage(format!("--set {key}: the key is given twice")));
        }
    }
    Ok(settings)
}

pub fn read_intake(source: &str) -> Result<String, Failure> {
    read_text("--intake", "the intake", source)
}

/// The text of a file, or of standard input for `-`, at most `MAX_INTAKE_BYTES`: `flag` and `what` name it in a failure.
fn read_text(flag: &str, what: &str, source: &str) -> Result<String, Failure> {
    let mut bytes = Vec::new();
    let limit = MAX_INTAKE_BYTES as u64 + 1;
    let read = if source == "-" { io::stdin().lock().take(limit).read_to_end(&mut bytes) } else { std::fs::File::open(source).and_then(|f| f.take(limit).read_to_end(&mut bytes)) };
    read.map_err(|e| Failure::new(exit::NOT_FOUND, format!("{flag} {source}: {e}")))?;
    if bytes.len() > MAX_INTAKE_BYTES {
        return Err(Failure::new(exit::REFUSED, format!("IntakeTooLarge: {what} is over {MAX_INTAKE_BYTES} bytes")));
    }
    String::from_utf8(bytes).map_err(|_| Failure::new(exit::REFUSED, format!("{what} is not UTF-8 text")))
}

fn absolute_dir(flag: &str, dir: &Path) -> Result<String, Failure> {
    let missing = |why: String| Failure::new(exit::NOT_FOUND, format!("{flag} {}: {why}", dir.display()));
    let full = std::fs::canonicalize(dir).map_err(|e| missing(e.to_string()))?;
    if !full.is_dir() {
        return Err(missing("not a directory".to_string()));
    }
    full.to_str().map(String::from).ok_or_else(|| Failure::new(exit::REFUSED, format!("{flag}: the path is not UTF-8 text")))
}

// ---- delegate ----

fn delegate_run(args: DelegateArgs) -> Result<(), Failure> {
    let settings = settings_of(&args.set)?;
    let api = connect(&args.remote)?;
    let request = read_text("the request", "the request", &args.request)?;
    let workspace = match &args.workspace {
        None => None,
        Some(dir) => Some(absolute_dir("--workspace", dir)?),
    };
    let input = Delegate { request, model_route: args.route, protocol: args.protocol, workspace, settings, ..Delegate::default() };
    let reply = api.command("delegate", &input)?;
    if args.json {
        return say(&format!("{}\n", reply.body.trim_end()));
    }
    let run_id = run_id_of(&reply).ok_or_else(|| Failure::new(exit::OTHER, "the answer of delegate names no run"))?;
    say(&format!("{run_id}\n"))
}

// ---- catalog ----

fn catalog_print(args: CatalogArgs) -> Result<(), Failure> {
    let reply = connect(&args.remote)?.view("catalog", &[])?;
    if args.json {
        return say(&format!("{}\n", reply.body.trim_end()));
    }
    let rows = Api::rows(&reply)?;
    let row = rows.first().ok_or_else(|| Failure::new(exit::OTHER, "the view catalog answered no row"))?;
    say(&catalog_text(row))
}

/// The text of a row of the view `catalog`: a line per route, per runbook and per route entry that is not admitted, and one
/// of the delegation's terms.
fn catalog_text(row: &Value) -> String {
    let list = |v: &Value| v.as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).filter(|s| !s.is_empty()).unwrap_or_else(|| "-".to_owned());
    let mut text = String::new();
    for r in row["routes"].as_array().into_iter().flatten() {
        let mut line = format!("route {}  {}  alias {}  {}", field(r, "route"), field(r, "kind"), field(r, "alias"), field(r, "status"));
        if let Some(p) = r["phase"].as_str() {
            line.push_str(&format!(" ({p})"));
        }
        line.push_str(&format!("  wires {}", list(&r["wires"])));
        if let Some(n) = r["max_model_len"].as_u64() {
            line.push_str(&format!("  context {n}"));
        }
        match (r["price_in_micro_usd_per_mtok"].as_u64(), r["price_out_micro_usd_per_mtok"].as_u64()) {
            (None, None) => line.push_str("  no price"),
            (i, o) => line.push_str(&format!("  micro USD per Mtok in {} out {}", i.map_or("-".to_owned(), |n| n.to_string()), o.map_or("-".to_owned(), |n| n.to_string()))),
        }
        text.push_str(&format!("{line}\n"));
    }
    for b in row["runbooks"].as_array().into_iter().flatten() {
        let state = if b["admitted"] == Value::Bool(true) { "admitted".to_owned() } else { format!("refused: {}", field(b, "refusal")) };
        text.push_str(&format!("runbook {}  {}  inputs {}  {state}\n", field(b, "runbook_id"), field(b, "title"), list(&b["inputs"])));
        for e in b["route_entries"].as_array().into_iter().flatten().filter(|e| e["admitted"] == Value::Bool(true)) {
            text.push_str(&format!("  role {}: route {}\n", field(e, "role"), field(e, "route")));
        }
    }
    let d = &row["delegation"];
    text.push_str(&format!(
        "delegation  paths {}  default route {}  classifier route {}  confinement {}  request at most {} bytes\n",
        list(&d["paths"]),
        field(d, "default_route"),
        field(d, "classifier_route"),
        field(d, "confinement"),
        d["request_limit"]
    ));
    text
}

/// The run a submit created: the `run_id` of the event it published, else the `Location` header.
fn run_id_of(reply: &http::Reply) -> Option<String> {
    let body: Value = serde_json::from_str(&reply.body).ok()?;
    if let Some(id) = body.pointer("/published/0/payload/run_id").and_then(Value::as_str) {
        return Some(id.to_string());
    }
    reply.header("location")?.split_once("run_id=").map(|(_, id)| id.to_string())
}

// ---- runs ----

fn list(remote: &Remote, json: bool) -> Result<(), Failure> {
    let reply = connect(remote)?.view("runs", &[])?;
    if json {
        return say(&format!("{}\n", reply.body.trim_end()));
    }
    let text: String = Api::rows(&reply)?.iter().map(|r| format!("{}\n", run_line(r))).collect();
    say(&text)
}

fn field<'a>(row: &'a Value, key: &str) -> &'a str {
    row.get(key).and_then(Value::as_str).unwrap_or("-")
}

fn run_line(row: &Value) -> String {
    let mut line = format!("{}  {}  {}  {}", field(row, "run_id"), field(row, "state"), field(row, "runbook_id"), field(row, "submitted_at"));
    for key in ["outcome", "failure"] {
        if let Some(v) = row.get(key).and_then(Value::as_str) {
            line.push_str(&format!("  {key}={v}"));
        }
    }
    line
}

fn show(remote: &Remote, run: &str, json: bool) -> Result<(), Failure> {
    let api = connect(remote)?;
    let reply = api.view("run", &[("run_id", run)])?;
    let rows = Api::rows(&reply)?;
    let Some(row) = rows.first() else { return Err(no_such_run(run)) };
    if json {
        return say(&format!("{}\n", reply.body.trim_end()));
    }
    let mut text = String::new();
    for (key, value) in row.as_object().into_iter().flatten() {
        match value {
            Value::String(s) => text.push_str(&format!("{key}: {s}\n")),
            other => text.push_str(&format!("{key}: {other}\n")),
        }
    }
    for d in Api::rows(&api.view("open-decisions", &[("run_id", run)])?)? {
        text.push_str(&format!("open decision {}", decision_text(&d)));
    }
    say(&text)
}

fn no_such_run(run: &str) -> Failure {
    Failure::new(exit::NOT_FOUND, format!("no such run: {run}"))
}

/// `<request id>: <question>`, then one line per option.
fn decision_text(row: &Value) -> String {
    let mut text = format!("{}: {}\n", field(row, "request_id"), field(row, "question"));
    for option in row.get("options").and_then(Value::as_array).into_iter().flatten() {
        match (option.get("letter").and_then(Value::as_str), option.get("does").and_then(Value::as_str)) {
            (Some(letter), Some(does)) => text.push_str(&format!("  {letter}: {does}\n")),
            _ => text.push_str(&format!("  {option}\n")),
        }
    }
    text
}

fn decisions(remote: &Remote, run: &str, json: bool) -> Result<(), Failure> {
    let api = connect(remote)?;
    let reply = api.view("open-decisions", &[("run_id", run)])?;
    if json {
        return say(&format!("{}\n", reply.body.trim_end()));
    }
    let text: String = Api::rows(&reply)?.iter().map(decision_text).collect();
    say(&text)
}

fn events(remote: &Remote, run: &str, no_follow: bool, json: bool) -> Result<(), Failure> {
    let api = connect(remote)?;
    if Api::rows(&api.view("run", &[("run_id", run)])?)?.is_empty() {
        return Err(no_such_run(run));
    }
    if no_follow {
        let rows = Api::rows(&api.view("events", &[("run_id", run), ("after_position", "0")])?)?;
        let text: String = rows.iter().map(|r| if json { format!("{r}\n") } else { sse::row_text(r) }).collect();
        return say(&text);
    }
    sse::follow(&api, run, json, &mut io::stdout().lock(), RESUME_BACKOFF)
}

fn cancel(remote: &Remote, run: &str, json: bool) -> Result<(), Failure> {
    let reply = connect(remote)?.command("cancel-run", &CancelRun { run_id: run.to_string() })?;
    answered(&reply, run, json)
}

fn decide_run(args: DecideArgs) -> Result<(), Failure> {
    let by = args.by.or_else(|| env("USER")).unwrap_or_else(|| "cli".to_string());
    let input = AnswerDecision { run_id: args.run, request_id: args.request.clone(), answer: args.answer, answered_by: by };
    let reply = connect(&args.remote)?.command("answer-decision", &input)?;
    answered(&reply, &args.request, args.json)
}

/// The outcome of a command and the id it was about, or the route's body.
fn answered(reply: &http::Reply, id: &str, json: bool) -> Result<(), Failure> {
    if json {
        return say(&format!("{}\n", reply.body.trim_end()));
    }
    let body: Value = serde_json::from_str(&reply.body).map_err(|e| Failure::new(exit::OTHER, format!("the answer is not JSON: {e}")))?;
    say(&format!("{} {id}\n", body.get("outcome").and_then(Value::as_str).unwrap_or("done")))
}
