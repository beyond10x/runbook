//! The twelve tools and their calls. A tool names a route of the command layer: `submit_run`,
//! `cancel_run`, `answer_decision`, `open_session` and `close_session` are the commands of the same
//! names, `list_runs`, `read_run`,
//! `read_events`, `read_open_decisions`, `list_products`, `read_product` and `list_routes` are the views `runs*`, `run`, `events`, `open-decisions`, `products`, `product` and `routes`. The result of a call is the body of the
//! route; nothing is computed here but the choice of view and the existence check of the two reads.

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
}

struct Tool {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    read_only: bool,
    kind: Kind,
}

const TOOLS: [Tool; 12] = [
    Tool {
        name: "submit_run",
        title: "Submit a run",
        description: "Submit a run of a runbook: the runbook by name, its intake as inline text, optionally a context directory under one of the daemon's context roots, settings and a confinement level (none named: the daemon's default level; a daemon with none refuses the submit by name). Every setting is a string; the caps of the run are settings too (max_wall_seconds, max_llm_calls, cap_micro_usd), each a string of a whole number of 1 or more such as \"50\", at most the daemon's ceiling. The daemon mints the run id and returns it in published[0].payload.run_id of the result; a repeated idempotency_key returns the first run and creates none.",
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
        description: "Cancel a run that has not ended (Submitted, Running, WaitingOnDecision). It withdraws the run's open decision requests. A run id the daemon does not know is an error with the HTTP status 404.",
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
];

/// The members of a command's input the HTTP handler fills; a client never sends them.
pub const SUBMIT_FILLED: [&str; 6] = ["run_id", "runbook_known", "context_inside_roots", "daemon_id", "at", "confinement_source"];

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
        Kind::ListRoutes => schema::object(Vec::new(), &[]),
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

/// The result of `tools/list`: exactly the twelve tools, in a fixed order.
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

/// Call a tool. The answer is the route's.
pub fn call(be: &dyn Backend, name: &str, arguments: &Value) -> Result<Answer, Invalid> {
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
