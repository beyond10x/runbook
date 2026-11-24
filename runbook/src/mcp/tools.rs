//! The seven tools and their calls. A tool names a route of the command layer: `submit_run`,
//! `cancel_run` and `answer_decision` are the commands of the same names, `list_runs`, `read_run`,
//! `read_events` and `read_open_decisions` are the views `runs*`, `run`, `events` and `open-decisions`. The result of a call is the body of the
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
}

struct Tool {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    read_only: bool,
    kind: Kind,
}

const TOOLS: [Tool; 7] = [
    Tool {
        name: "submit_run",
        title: "Submit a run",
        description: "Submit a run of a runbook: the runbook by name, its intake as inline text, optionally a context directory under one of the daemon's context roots, settings and a confinement level. Every setting is a string; the caps of the run are settings too (max_wall_seconds, max_llm_calls, cap_micro_usd), each a string of a whole number of 1 or more such as \"50\", at most the daemon's ceiling. The daemon mints the run id and returns it in published[0].payload.run_id of the result; a repeated idempotency_key returns the first run and creates none.",
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
        description: "The decision requests of one run that no one has answered yet, in the `rows` of the result: the request id, the case, the action and the capability that ask, the question, the options and when it opened. A run with none, or a run id the daemon does not know, has no rows.",
        read_only: true,
        kind: Kind::ReadOpenDecisions,
    },
    Tool {
        name: "answer_decision",
        title: "Answer a decision",
        description: "Answer an open decision request of a run that waits: the run id, the request id (read it with read_open_decisions), the answer and a label for who answers. The label is stored and shown; the daemon's own principal is what the runtime records. The first answer stands.",
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
];

/// The members of a command's input the HTTP handler fills; a client never sends them.
pub const SUBMIT_FILLED: [&str; 5] = ["run_id", "runbook_known", "context_inside_roots", "daemon_id", "at"];

/// The members of the input of `answer-decision` the handler fills: the instant and the daemon's own
/// principal, which no tool argument sets.
pub const ANSWER_FILLED: [&str; 2] = ["at", "principal"];

/// The members of a command's input the daemon defaults when the client leaves them out.
pub const SUBMIT_DEFAULTED: [&str; 2] = ["settings", "confinement"];

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

/// The result of `tools/list`: exactly the seven tools, in a fixed order.
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
