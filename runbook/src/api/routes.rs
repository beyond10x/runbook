//! The routes of the OpenAPI document `ess/generated/openapi/openapi/runbookd.yaml`, each a thin call
//! into the command layer: a command is `POST /daemon/commands/<name>` with a JSON body, a view is
//! `GET /daemon/views/<name>` with its parameters in the query.

use super::json_response;
use crate::commands::{AnswerDecision, CancelRun, CloseSession, Daemon, Fault, OpenSession, Outcome, SubmitRun};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::collections::HashMap;

/// The client commands, by wire name.
pub const COMMANDS: [&str; 5] = ["open-session", "close-session", "submit-run", "cancel-run", "answer-decision"];

/// The views, by wire name.
pub const VIEWS: [&str; 13] = [
    "runs",
    "runs-of-session",
    "runs-in-state",
    "runs-of-session-in-state",
    "run",
    "open-decisions",
    "decisions",
    "events",
    "session-events",
    "daemon-events",
    "sessions",
    "configuration",
    "runbooks",
];

/// The views that page events by position: asked with `Accept: text/event-stream` they are streams.
pub const EVENT_VIEWS: [&str; 3] = ["events", "session-events", "daemon-events"];

pub fn command_path(name: &str) -> String {
    format!("/daemon/commands/{name}")
}

pub fn view_path(name: &str) -> String {
    format!("/daemon/views/{name}")
}

fn parse<T: DeserializeOwned>(body: &[u8]) -> Result<T, Fault> {
    serde_json::from_slice(body).map_err(|e| Fault::Invalid(format!("the body is not a valid input of the command: {}", crate::commands::out_of_range_member(body, &e).unwrap_or_else(|| e.to_string()))))
}

/// What a route answers before it is an HTTP response: the status, the JSON body and the `Location`
/// of a submit. The MCP tools (`crate::mcp`) call `command_answer` and `view_answer` and carry this
/// body unchanged.
pub struct Answer {
    pub status: StatusCode,
    pub body: Value,
    pub location: Option<String>,
}

impl Answer {
    pub fn into_response(self) -> Response {
        let mut response = json_response(self.status, &self.body);
        if let Some(v) = self.location.and_then(|l| HeaderValue::from_str(&l).ok()) {
            response.headers_mut().insert(header::LOCATION, v);
        }
        response
    }
}

fn refused(status: StatusCode, what: &str, message: &str) -> Answer {
    Answer { status, body: json!({"refused": what, "message": message}), location: None }
}

fn fault(f: Fault) -> Answer {
    match f {
        Fault::Invalid(message) => refused(StatusCode::BAD_REQUEST, "bad request", &message),
        Fault::Internal(message) => refused(StatusCode::INTERNAL_SERVER_ERROR, "internal error", &message),
        Fault::Draining { live_runs } => {
            let message = format!("the daemon is draining: it got its stop signal, takes no new run and waits for {live_runs} live run(s) to end; submit again to the daemon that starts next");
            Answer { status: StatusCode::SERVICE_UNAVAILABLE, body: json!({"refused": "draining", "message": message, "live_runs": live_runs}), location: None }
        }
    }
}

fn outcome(o: Outcome) -> Answer {
    // The answer of a submit names the run in the event it publishes; a replay has no run in its
    // body, so the first run is also named in the `Location`.
    let location = o.run_id.as_ref().map(|run_id| format!("{}?run_id={run_id}", view_path("run")));
    Answer { status: StatusCode::from_u16(o.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), body: o.body(), location }
}

pub fn command(name: &str, daemon: &Daemon, body: &[u8]) -> Response {
    command_answer(name, daemon, body).into_response()
}

pub fn command_answer(name: &str, daemon: &Daemon, body: &[u8]) -> Answer {
    let reply = match name {
        "open-session" => parse::<OpenSession>(body).and_then(|i| daemon.open_session(i)),
        "close-session" => parse::<CloseSession>(body).and_then(|i| daemon.close_session(i)),
        "submit-run" => parse::<SubmitRun>(body).and_then(|i| daemon.submit_run(i)),
        "cancel-run" => parse::<CancelRun>(body).and_then(|i| daemon.cancel_run(i)),
        "answer-decision" => parse::<AnswerDecision>(body).and_then(|i| daemon.answer_decision(i)),
        other => unreachable!("no command {other}"),
    };
    match reply {
        Ok(o) => outcome(o),
        Err(f) => fault(f),
    }
}

fn text<'a>(q: &'a HashMap<String, String>, key: &str) -> Result<&'a str, Fault> {
    q.get(key).map(String::as_str).ok_or_else(|| Fault::Invalid(format!("the query parameter {key} is required")))
}

fn position(q: &HashMap<String, String>) -> Result<u64, Fault> {
    let raw = text(q, "after_position")?;
    raw.parse().map_err(|_| Fault::Invalid(format!("after_position must be an integer of 0 or more, not {raw:?}")))
}

/// Whether the request asks for `text/event-stream` among its media types.
pub fn wants_stream(headers: &HeaderMap) -> bool {
    headers
        .get_all(header::ACCEPT)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .any(|m| m.split(';').next().is_some_and(|t| t.trim().eq_ignore_ascii_case("text/event-stream")))
}

/// The page form of a view. An event view asked as a stream is `sse::answer`, chosen before this.
pub fn view(name: &str, daemon: &Daemon, query: &HashMap<String, String>) -> Response {
    view_answer(name, daemon, query).into_response()
}

pub fn view_answer(name: &str, daemon: &Daemon, query: &HashMap<String, String>) -> Answer {
    let rows: Result<Vec<Value>, Fault> = (|| {
        Ok(match name {
            "runs" => daemon.runs(),
            "runs-of-session" => daemon.runs_of_session(text(query, "session_id")?),
            "runs-in-state" => daemon.runs_in_state(text(query, "state")?)?,
            "runs-of-session-in-state" => daemon.runs_of_session_in_state(text(query, "session_id")?, text(query, "state")?)?,
            "run" => daemon.run_detail(text(query, "run_id")?),
            "open-decisions" => daemon.open_decisions_of_run(text(query, "run_id")?),
            "decisions" => daemon.decisions_of_run(text(query, "run_id")?),
            "events" => daemon.events_after(text(query, "run_id")?, position(query)?),
            "session-events" => daemon.events_of_session_after(text(query, "session_id")?, position(query)?),
            "daemon-events" => daemon.events_of_daemon_after(position(query)?),
            "sessions" => daemon.sessions(),
            "configuration" => daemon.daemon_configuration(),
            "runbooks" => daemon.runbooks(),
            other => unreachable!("no view {other}"),
        })
    })();
    match rows {
        Ok(rows) => Answer { status: StatusCode::OK, body: json!({"rows": rows}), location: None },
        Err(f) => fault(f),
    }
}
