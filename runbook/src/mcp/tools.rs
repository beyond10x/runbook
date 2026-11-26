//! The fifteen tools and their calls. A tool names a route of the command layer: `submit_run`,
//! `cancel_run`, `answer_decision`, `open_session` and `close_session` are the commands of the same
//! names, `list_runs`, `read_run`,
//! `read_events`, `read_open_decisions`, `list_products`, `read_product` and `list_routes` are the views `runs*`, `run`, `events`, `open-decisions`, `products`, `product` and `routes`. The result of a call is the body of the
//! route; nothing is computed here but the choice of view and the existence check of the two reads.
//!
//! `delegate` and `delegation_result` are the command `delegate` and the view `delegation-result`: `delegate` waits for the run's
//! end, reading that view, with a progress notification per step; both answer the view's row with the result text held to
//! `RESULT_CHARS` and a text whose first line names the run. `describe_catalog` is the view `catalog`.

use super::schema;
use super::{refused, Backend};
use crate::api::routes::Answer;
use axum::http::StatusCode;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

/// The key of `_meta` in a tool result that carries the HTTP status of the route's answer.
pub const STATUS_KEY: &str = "runbook/http-status";

/// What a call can fail with before it reaches the command layer: both are JSON-RPC errors
/// (`-32602`), the rest is an answer of the route and a tool execution error.
#[derive(Debug, PartialEq, Eq)]
pub enum Invalid {
    UnknownTool(String),
    Arguments(String),
}

enum Kind {
    Command(&'static str),
    ListRuns,
    ReadRun,
    ReadEvents,
    ReadOpenDecisions,
    ListProducts,
    ReadProduct,
    ListRoutes,
    Delegate,
    DelegationResult,
    DescribeCatalog,
}

struct Tool {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    read_only: bool,
    kind: Kind,
}

const TOOLS: [Tool; 15] = [
    Tool {
        name: "submit_run",
        title: "Submit a run",
        description: "Submit a run of a runbook: the runbook by name, its intake as inline text, optionally a context directory under one of the daemon's context roots, settings and a confinement level (none named: the daemon's default level; a daemon with none refuses the submit by name). Every setting is a string; the caps of the run are settings too (max_wall_seconds, max_llm_calls, cap_micro_usd), each a string of a whole number of 1 or more such as \"50\", at most the daemon's ceiling. The daemon mints the run id and returns it in published[0].payload.run_id of the result; a repeated idempotency_key returns the first run and creates none. A run whose decider kind is model names its model route by model_route (a route name list_routes lists; none named: the daemon's default route) and a route per role by model_routes; an undeclared route, an alias, an address or a token is refused as `route-refused` (HTTP status 422) naming the declared routes.",
        read_only: false,
        kind: Kind::Command("submit-run"),
    },
    Tool {
        name: "list_runs",
        title: "List runs",
        description: "The runs of the daemon, newest first, optionally only those of a session (session_id) or in a state (state), or both. The rows are in the `rows` of the result.",
        read_only: true,
        kind: Kind::ListRuns,
    },
    Tool {
        name: "read_run",
        title: "Read a run",
        description: "One run with everything it was submitted with: state, outcome, intake, settings. A run id the daemon does not know is an error with the HTTP status 404.",
        read_only: true,
        kind: Kind::ReadRun,
    },
    Tool {
        name: "read_events",
        title: "Read the events of a run",
        description: "The events of one run in order, as the runtime recorded them. after_sequence (the run's own number, from 1) returns the events after that sequence number; after_position (the daemon's number over all runs) does the same by position; both default to 0, the whole record. A run id the daemon does not know is an error with the HTTP status 404.",
        read_only: true,
        kind: Kind::ReadEvents,
    },
    Tool {
        name: "read_open_decisions",
        title: "Read the open decisions of a run",
        description: "The decision requests of one run that no one has answered yet, in the `rows` of the result: the request id, the case, the action and the capability that ask, the question, the options and when it opened, and, for a request read from its case, the member `artifact`: the recommended option or answer, the strength, the grounds and whether a free answer is taken. A run with none, or a run id the daemon does not know, has no rows.",
        read_only: true,
        kind: Kind::ReadOpenDecisions,
    },
    Tool {
        name: "answer_decision",
        title: "Answer a decision",
        description: "Answer an open decision request of a run that waits: the run id, the request id (read it with read_open_decisions), the answer and a label for who answers. A request read from its case (it has the member `artifact`) takes the letter of one of its options, alone or followed by a colon and text, or a free answer where it takes one; any other answer is refused as `unknown-option` (HTTP status 422). The label is stored and shown; the daemon's own principal is what the runtime records. The first answer stands.",
        read_only: false,
        kind: Kind::Command("answer-decision"),
    },
    Tool {
        name: "cancel_run",
        title: "Cancel a run",
        description: "Cancel a run that has not ended (Submitted, WaitingForRoute, Running, WaitingOnDecision). It withdraws the run's open decision requests. A run id the daemon does not know is an error with the HTTP status 404.",
        read_only: false,
        kind: Kind::Command("cancel-run"),
    },
    Tool {
        name: "open_session",
        title: "Open a session",
        description: "Open a session: a session_id the client chooses and a client label of at most 200 characters, which is stored and shown and grants nothing. A submit_run with that session_id belongs to the session, and list_runs with it lists those runs. An id the daemon already knows, open or closed, is refused as `session-exists` (HTTP status 422); a label over 200 characters is refused with the HTTP status 400.",
        read_only: false,
        kind: Kind::Command("open-session"),
    },
    Tool {
        name: "close_session",
        title: "Close a session",
        description: "Close an open session: from then on a submit_run with its id is refused as `SessionClosedToRuns` (HTTP status 409). The runs of the session go on; closing it cancels none. An id the daemon does not know is refused as `no-session` (HTTP status 404), a session that is already closed as `wrong-state` (HTTP status 409).",
        read_only: false,
        kind: Kind::Command("close-session"),
    },
    Tool {
        name: "list_products",
        title: "List the products of a run",
        description: "The files a run wrote under its work/ directory, in one row: `files` (each with path, size and modified_at; a path is relative to work/), `hidden` (entries whose name starts with a dot: neither listed nor served), `left_out` (links and files with more than one name: neither listed nor served) and `cut` when the listing stopped at a bound (500 files, 2000 directories, 20000 entries, 6 levels). A run id the daemon does not know is an error with the HTTP status 404.",
        read_only: true,
        kind: Kind::ListProducts,
    },
    Tool {
        name: "read_product",
        title: "Read a product of a run",
        description: "One file of a run's work/ directory, by its path as list_products names it, in one row: `kind` text (with `text`, the first 1048576 bytes as UTF-8) or binary (no text), `size`, `shown`, `truncated` (true when the file holds more than `limit` bytes) and `modified_at`. Refused by name: a path that is empty, absolute or has `..` (400), a name that starts with a dot or a file that is not there (404), a link on the path or a file with more than one link (403), anything that is not a regular file (415), a run id the daemon does not know (404). A refusal holds no content of a file.",
        read_only: true,
        kind: Kind::ReadProduct,
    },
    Tool {
        name: "list_routes",
        title: "List the model routes",
        description: "The model routes the daemon declares, in order of name, in the `rows` of the result: each with `route` (the name a run names), `gateway` (by name), `alias` (the gateway's name of the model), `kind` (managed, hosted or local), `wires` (chat, responses, messages, as the gateway lists them), `status` (ready, not-ready, listed, model-unknown, gateway-not-ready or gateway-unreachable), `phase` when the gateway gave one, and `read_at`. The daemon reads its gateways at most once every 15 seconds, in the background and within 5 seconds, and answers the last read at once; a read sends no model call and starts nothing. No address, token or price is in a row.",
        read_only: true,
        kind: Kind::ListRoutes,
    },
    Tool {
        name: "delegate",
        title: "Delegate agent work",
        description: "Run a piece of agent work on the Runbook daemon, on a named model route: `request` is the whole task as text (at most 1 MiB), `model_route` the route by the name the daemon declares (none: the daemon's default route; a route it does not declare is refused as `route-refused`, listing the declared routes; a route whose gateway is not ready as `route-unavailable`, HTTP status 503). With `protocol` that runbook runs with the request as its intake; without, one agent node does the task and writes result.md. `workspace` is a directory under a daemon context root, copied into the run's workspace. The tool waits for the run's end (at most `wait_seconds`, default 600, at most 3600), sends a progress notification at each step when the call carries a progressToken (over stdio only: the HTTP endpoint sends none), and returns the result: the text starts with one line naming the run, the route, the path, the protocol, the calls and the cost, then, for a run that failed, a line `failed: ` with what failed it (a route that did not serve the run's call is named there with `route-unavailable`), then the run's result.md (at most 40000 characters; `result_truncated` says it was cut); `structuredContent.rows[0]` is the row of delegation_result. Past its wait it returns the run id and its state: read the rest with delegation_result.",
        read_only: false,
        kind: Kind::Delegate,
    },
    Tool {
        name: "delegation_result",
        title: "Read the result of a delegation",
        description: "What a delegation's run has come to, by its run id: the state, the outcome once it ended, what failed it (`failure`: why it failed to start, the runtime's reason, or for the outcome `failed: step <action> <end>` the step whose program failed and how it ended: `exit <n>`, `signal <n>`, `timed out after <s> s` or `ended with no exit`), the text of its result.md (at most 40000 characters here; `result_truncated` says it was cut), its products, the route, the path, the protocol, the calls, the tokens and the cost (`cost_known` false when a call was booked at its reservation). The text starts with the same line as the answer of delegate. A run id the daemon does not know is an error with the HTTP status 404, and so is a run no delegation created.",
        read_only: true,
        kind: Kind::DelegationResult,
    },
    Tool {
        name: "describe_catalog",
        title: "Describe what a delegation may name",
        description: "What a delegate call may name on this daemon, in one row (`rows[0]` of the result): `routes`, each declared model route with `route` (the name a delegation names), `kind`, `alias`, `gateway` (by name), `wires`, `status` and `phase` as list_routes reads them, `max_model_len` and `tool_calling` when the gateway lists them, and the prices per million tokens and the wait the daemon's file declares; `runbooks`, each runbook a delegation may name as its `protocol`, with `title` (its purpose in one line), `description`, `inputs`, its `route_entries` (per role the route that serves it, or the reason it cannot be admitted), `routes_allowed`, `admitted` and the `refusal` of one that is not; `delegation`, the members of delegate (`name`, `required`, `summary`, `max_length`), the `paths` a delegation may take (named, free), the `default_route`, the `classifier_route`, the `confinement` it runs at and the `request_limit` in bytes. No address, token or path is in it.",
        read_only: true,
        kind: Kind::DescribeCatalog,
    },
];

/// The members of the input of `delegate` the handler fills, and the two the MCP server fills (`caller`, `caller_label`):
/// none of them is an argument of the tool.
pub const DELEGATE_FILLED: [&str; 14] = ["run_id", "runbook_id", "confinement", "route", "model_route_alias", "model_route_source", "path", "route_declared", "runbook_known", "context_inside_roots", "daemon_id", "at", "caller", "caller_label"];
/// How long `delegate` waits for the run's end when the call names no `wait_seconds`, and the most it names.
pub const DEFAULT_WAIT_SECONDS: u64 = 600;
pub const MAX_WAIT_SECONDS: u64 = 3600;
/// The most characters of a delegation's result.md a tool answers.
pub const RESULT_CHARS: usize = 40_000;
/// How often `delegate` reads the run while it waits.
const POLL: std::time::Duration = std::time::Duration::from_millis(200);

/// The members of a command's input the HTTP handler fills; a client never sends them.
pub const SUBMIT_FILLED: [&str; 9] = ["run_id", "runbook_known", "context_inside_roots", "daemon_id", "at", "confinement_source", "model_route_alias", "model_route_source", "route_refused"];

/// The members of the input of `answer-decision` the handler fills: the instant, the daemon's own
/// principal and whether the request takes the answer (`answer_offered`), which no tool argument sets.
pub const ANSWER_FILLED: [&str; 3] = ["at", "principal", "answer_offered"];

/// The members of a command's input the daemon defaults when the client leaves them out.
pub const SUBMIT_DEFAULTED: [&str; 1] = ["settings"];

fn input_schema(tool: &Tool) -> Value {
    match &tool.kind {
        Kind::Command("submit-run") => schema::command_input("submit-run", &SUBMIT_FILLED, &SUBMIT_DEFAULTED),
        Kind::Command("answer-decision") => schema::command_input("answer-decision", &ANSWER_FILLED, &[]),
        Kind::Command(command) => schema::command_input(command, &["at"], &[]),
        Kind::ListRuns => schema::object(
            vec![("session_id", schema::view_param("runs-of-session-in-state", "session_id")), ("state", schema::view_param("runs-of-session-in-state", "state"))],
            &[],
        ),
        Kind::ReadRun => schema::object(vec![("run_id", schema::view_param("run", "run_id"))], &["run_id"]),
        Kind::ReadOpenDecisions => schema::object(vec![("run_id", schema::view_param("open-decisions", "run_id"))], &["run_id"]),
        Kind::ListProducts => schema::object(vec![("run_id", schema::view_param("products", "run_id"))], &["run_id"]),
        Kind::ReadProduct => schema::object(vec![("run_id", schema::view_param("product", "run_id")), ("path", schema::view_param("product", "path"))], &["run_id", "path"]),
        Kind::ListRoutes | Kind::DescribeCatalog => schema::object(Vec::new(), &[]),
        Kind::Delegate => {
            let mut s = schema::command_input("delegate", &DELEGATE_FILLED, &SUBMIT_DEFAULTED);
            s["properties"]["wait_seconds"] = json!({"type": "integer", "minimum": 0, "maximum": MAX_WAIT_SECONDS, "description": format!("How long the tool waits for the run's end before it answers the run id and its state. Default {DEFAULT_WAIT_SECONDS}.")});
            s
        }
        Kind::DelegationResult => schema::object(vec![("run_id", schema::view_param("delegation-result", "run_id"))], &["run_id"]),
        Kind::ReadEvents => schema::object(
            vec![
                ("run_id", schema::view_param("events", "run_id")),
                ("after_sequence", json!({"type": "integer", "minimum": 0, "description": "Return the events whose sequence number in the run is above this. Default 0."})),
                ("after_position", {
                    let mut p = schema::view_param("events", "after_position");
                    p["minimum"] = json!(0);
                    p["description"] = json!("Return the events whose position in the daemon is above this. Default 0.");
                    p
                }),
            ],
            &["run_id"],
        ),
    }
}

/// The result of `tools/list`: exactly the fifteen tools, in a fixed order.
pub fn list() -> &'static Vec<Value> {
    static LIST: OnceLock<Vec<Value>> = OnceLock::new();
    LIST.get_or_init(|| {
        TOOLS
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "title": t.title,
                    "description": t.description,
                    "inputSchema": input_schema(t),
                    "annotations": {"readOnlyHint": t.read_only},
                })
            })
            .collect()
    })
}

fn properties(tool: &str) -> Vec<String> {
    list().iter().find(|t| t["name"] == tool).and_then(|t| t["inputSchema"]["properties"].as_object()).map(|p| p.keys().cloned().collect()).unwrap_or_default()
}

/// A member of the arguments as the text of a query parameter: text, or a whole number.
fn query_text(key: &str, v: &Value) -> Result<Option<String>, Invalid> {
    match v {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        Value::Number(n) if n.is_u64() || n.is_i64() => Ok(Some(n.to_string())),
        _ => Err(Invalid::Arguments(format!("the argument {key} must be text or a whole number"))),
    }
}

fn query_of(args: &Map<String, Value>) -> Result<HashMap<String, String>, Invalid> {
    let mut q = HashMap::new();
    for (k, v) in args {
        if let Some(t) = query_text(k, v)? {
            q.insert(k.clone(), t);
        }
    }
    Ok(q)
}

fn no_such_run(run_id: &str) -> Answer {
    refused(StatusCode::NOT_FOUND, "no such run", &format!("the daemon has no run {run_id}"))
}

/// Whether the daemon knows the run, by the view `run`; `Err` is the answer of that view when it failed.
fn known(be: &dyn Backend, run_id: &str) -> Result<bool, Answer> {
    let a = be.view("run", &HashMap::from([("run_id".to_string(), run_id.to_string())]));
    if a.status != StatusCode::OK {
        return Err(a);
    }
    Ok(a.body["rows"].as_array().is_some_and(|r| !r.is_empty()))
}

/// The tool `name` and its arguments, each one the tool takes.
fn arguments(name: &str, arguments: &Value) -> Result<(&'static Tool, Map<String, Value>), Invalid> {
    let tool = TOOLS.iter().find(|t| t.name == name).ok_or_else(|| Invalid::UnknownTool(name.to_string()))?;
    let args = match arguments {
        Value::Null => Map::new(),
        Value::Object(m) => m.clone(),
        _ => return Err(Invalid::Arguments("arguments must be an object".into())),
    };
    let allowed = properties(name);
    if let Some(k) = args.keys().find(|k| !allowed.contains(k)) {
        return Err(Invalid::Arguments(format!("{name} takes no argument {k}; it takes: {}", allowed.join(", "))));
    }
    Ok((tool, args))
}

/// Where `delegate` sends its progress notifications: the client's progress token and the sink of messages.
pub struct Progress<'a> {
    pub token: Value,
    pub send: &'a mut dyn FnMut(Value),
}

/// Call a tool and answer the `result` of its `tools/call`. `progress`: the call's progress token and where its notifications
/// go, when the call carried one.
pub fn run(be: &dyn Backend, name: &str, arguments_given: &Value, progress: Option<Progress>) -> Result<Value, Invalid> {
    let (tool, args) = arguments(name, arguments_given)?;
    match tool.kind {
        Kind::Delegate => delegate(be, args, progress),
        Kind::DelegationResult => {
            let a = be.view("delegation-result", &query_of(&args)?);
            Ok(delegation_answer(&a))
        }
        _ => call(be, name, arguments_given).map(|a| result(&a)),
    }
}

/// `delegate`: the command, then the run read until it ends or the wait is over.
fn delegate(be: &dyn Backend, mut args: Map<String, Value>, mut progress: Option<Progress>) -> Result<Value, Invalid> {
    let wait = match args.remove("wait_seconds") {
        None | Some(Value::Null) => DEFAULT_WAIT_SECONDS,
        Some(v) => v.as_u64().filter(|w| *w <= MAX_WAIT_SECONDS).ok_or_else(|| Invalid::Arguments(format!("wait_seconds must be a whole number from 0 to {MAX_WAIT_SECONDS}")))?,
    };
    // who calls: the server sends it, never the model (the members are no argument of the tool)
    let (client, label) = be.caller();
    if let Some(c) = client {
        args.insert("caller".into(), json!(c));
    }
    if let Some(l) = label {
        args.insert("caller_label".into(), json!(l));
    }
    let a = be.command("delegate", Value::Object(args).to_string().as_bytes());
    let Some(run_id) = a.body["published"][0]["payload"]["run_id"].as_str().filter(|_| a.status == StatusCode::ACCEPTED).map(str::to_owned) else {
        return Ok(result(&a));
    };
    let until = std::time::Instant::now() + std::time::Duration::from_secs(wait);
    let (mut after, mut steps) = (0u64, 0u64);
    loop {
        let r = be.view("delegation-result", &HashMap::from([("run_id".to_owned(), run_id.clone())]));
        let row = match r.body["rows"].get(0) {
            Some(row) if r.status == StatusCode::OK => row.clone(),
            _ => return Ok(result(&r)),
        };
        if let Some(p) = progress.as_mut() {
            let q = HashMap::from([("run_id".to_owned(), run_id.clone()), ("after_position".to_owned(), after.to_string())]);
            let events = be.view("events", &q);
            for e in events.body["rows"].as_array().into_iter().flatten() {
                after = after.max(e["position"].as_u64().unwrap_or(0));
                if e["kind"] == crate::commands::ACTION_STARTED {
                    steps += 1;
                    let step = e["record"]["action"].as_str().or_else(|| e["record"]["case"].as_str()).unwrap_or("a step");
                    let message = format!("step {step} of run {run_id} on route {}: {} call(s), {} micro USD booked so far", text_of(&row["route"]), row["calls"], row["cost_micro_usd"]);
                    (p.send)(json!({"jsonrpc": "2.0", "method": "notifications/progress", "params": {"progressToken": p.token, "progress": steps, "message": message}}));
                }
            }
        }
        let ended = ["Ended", "FailedToStart", "Cancelled"].iter().any(|s| row["run_state"] == *s);
        let now = std::time::Instant::now();
        if ended || now >= until {
            return Ok(delegation_answer(&Answer { status: StatusCode::OK, body: json!({"rows": [row]}), location: None }));
        }
        std::thread::sleep(POLL.min(until - now));
    }
}

/// A member's text, or `?`.
fn text_of(v: &Value) -> &str {
    v.as_str().unwrap_or("?")
}

/// The `result` of `delegate` and `delegation_result` for an answer of the view `delegation-result`: its row with the result text
/// held to `RESULT_CHARS`, and a text that starts with one line naming the run, then a line `failed:` with the row's `failure` when it has one, then the result. A refusal of the view is its
/// answer, as for every tool. A run that failed to start, was cancelled or failed in its runtime is an error.
fn delegation_answer(a: &Answer) -> Value {
    let Some(row) = a.body["rows"].get(0).filter(|_| a.status == StatusCode::OK) else {
        return result(a);
    };
    let mut row = row.clone();
    if let Some(text) = row["result_text"].as_str() {
        if text.chars().count() > RESULT_CHARS {
            row["result_text"] = json!(text.chars().take(RESULT_CHARS).collect::<String>());
            row["result_truncated"] = json!(true);
        }
    }
    let state = text_of(&row["run_state"]).to_owned();
    let outcome = row["outcome"].as_str().map(|o| format!(" ({o})")).unwrap_or_default();
    let cost = if row["cost_known"] == json!(true) { format!("{} micro USD", row["cost_micro_usd"]) } else { format!("{} micro USD booked, the cost of a call not known", row["cost_micro_usd"]) };
    // the last phase the gateway gave of a route that could not answer (a run that waited for its route, or ended `stopped: route`)
    let phase = row["route_phase"].as_str().map(|p| format!(" (phase {p})")).unwrap_or_default();
    let mut head = format!("run {} on route {}{phase}: {state}{outcome}, path {}, protocol {}, {} call(s), {cost}", text_of(&row["run_id"]), text_of(&row["route"]), text_of(&row["path"]), text_of(&row["protocol"]), row["calls"]);
    if let Some(failure) = row["failure"].as_str() {
        head.push_str(&format!("\nfailed: {failure}"));
    }
    let body = match row["result_text"].as_str() {
        Some(text) => text.to_owned(),
        None if state == "WaitingForRoute" => format!("The run has not ended: it waits for its route {} to answer; read its result with delegation_result.", text_of(&row["route"])),
        None if ["Submitted", "Running", "WaitingOnDecision"].contains(&state.as_str()) => "The run has not ended: read its result with delegation_result.".to_owned(),
        None if row["products"].as_array().is_some_and(|p| p.iter().any(|f| f == crate::delegation::RESULT_FILE)) => {
            "The run's result.md is not text: read it with read_product.".to_owned()
        }
        None => "The run wrote no result.md.".to_owned(),
    };
    let failed = matches!(state.as_str(), "FailedToStart" | "Cancelled") || row["outcome"].as_str().is_some_and(failure_outcome);
    json!({
        "content": [{"type": "text", "text": format!("{head}\n\n{body}")}],
        "structuredContent": {"rows": [row]},
        "isError": failed,
        "_meta": {STATUS_KEY: a.status.as_u16()},
    })
}

/// Whether an outcome of an `Ended` run is a failure of the daemon's own, not one the protocol reached: the runtime failed it
/// (`runtime-failure`), the daemon ended it at a stop or a restart (`orphaned`), or its route never could answer (`stopped:
/// route`, and any other `stopped: ...` or `failed...` outcome).
fn failure_outcome(outcome: &str) -> bool {
    outcome == "runtime-failure" || outcome == crate::commands::ORPHANED || outcome.starts_with("stopped:") || outcome.starts_with("failed")
}

/// Call a tool. The answer is the route's.
pub fn call(be: &dyn Backend, name: &str, arguments_given: &Value) -> Result<Answer, Invalid> {
    let (tool, args) = arguments(name, arguments_given)?;
    match &tool.kind {
        Kind::Command(command) => Ok(be.command(command, Value::Object(args).to_string().as_bytes())),
        Kind::ListRuns => {
            let q = query_of(&args)?;
            let view = match (q.contains_key("session_id"), q.contains_key("state")) {
                (true, true) => "runs-of-session-in-state",
                (true, false) => "runs-of-session",
                (false, true) => "runs-in-state",
                (false, false) => "runs",
            };
            Ok(be.view(view, &q))
        }
        Kind::ReadRun => {
            let q = query_of(&args)?;
            let a = be.view("run", &q);
            let empty = a.status == StatusCode::OK && a.body["rows"].as_array().is_some_and(|r| r.is_empty());
            match q.get("run_id") {
                Some(id) if empty => Ok(no_such_run(id)),
                _ => Ok(a),
            }
        }
        Kind::ReadOpenDecisions => Ok(be.view("open-decisions", &query_of(&args)?)),
        Kind::ListProducts => Ok(be.view("products", &query_of(&args)?)),
        Kind::ReadProduct => Ok(be.view("product", &query_of(&args)?)),
        Kind::ListRoutes => Ok(be.view("routes", &HashMap::new())),
        Kind::Delegate => Ok(be.command("delegate", Value::Object(args).to_string().as_bytes())),
        Kind::DelegationResult => Ok(be.view("delegation-result", &query_of(&args)?)),
        Kind::DescribeCatalog => Ok(be.view("catalog", &HashMap::new())),
        Kind::ReadEvents => {
            let mut q = query_of(&args)?;
            let after_sequence = match q.remove("after_sequence") {
                None => 0,
                Some(s) => s.parse::<u64>().map_err(|_| Invalid::Arguments("after_sequence must be a whole number of 0 or more".into()))?,
            };
            q.entry("after_position".to_string()).or_insert_with(|| "0".to_string());
            if let Some(id) = q.get("run_id") {
                match known(be, id) {
                    Ok(true) => {}
                    Ok(false) => return Ok(no_such_run(id)),
                    Err(a) => return Ok(a),
                }
            }
            let mut a = be.view("events", &q);
            if a.status == StatusCode::OK && after_sequence > 0 {
                if let Some(rows) = a.body["rows"].as_array_mut() {
                    rows.retain(|r| r["sequence"].as_u64().is_some_and(|s| s > after_sequence));
                }
            }
            Ok(a)
        }
    }
}

/// The `result` of a `tools/call` for an answer of the route: its body as text and as structured
/// content, an error when the status is 400 or above, and the status in `_meta`.
pub fn result(a: &Answer) -> Value {
    json!({
        "content": [{"type": "text", "text": a.body.to_string()}],
        "structuredContent": a.body,
        "isError": a.status.as_u16() >= 400,
        "_meta": {STATUS_KEY: a.status.as_u16()},
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A run that ended with a result.md that is not text has a result file: the head says so instead of "wrote no result.md".
    #[test]
    fn a_result_md_that_is_not_text_is_named_as_such() {
        let row = json!({"run_id": "r", "run_state": "Ended", "outcome": "delivered", "route": "default", "path": "free", "protocol": "delegation-free", "calls": 1, "cost_micro_usd": 0, "cost_known": true, "result_truncated": false, "products": ["result.md"]});
        let a = delegation_answer(&Answer { status: StatusCode::OK, body: json!({"rows": [row]}), location: None });
        let text = a["content"][0]["text"].as_str().unwrap();
        assert!(!text.contains("wrote no result.md") && text.contains("result.md is not text"), "{text}");
    }
}
