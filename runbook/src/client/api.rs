//! The routes of the daemon as the client calls them. The paths and the input types are the ones
//! the server's routes are built from (`crate::api::routes`, `crate::commands`): the client has no
//! second copy of a body shape. The answers of views are rows of JSON, as the server builds them.

use super::exit::{self, Failure};
use super::http::{Client, Reply};
use crate::api::routes;
use serde::Serialize;
use serde_json::Value;

pub struct Api {
    pub client: Client,
}

/// A percent-encoded query value: everything but the unreserved characters.
pub fn encode(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub fn view_path(name: &str, query: &[(&str, &str)]) -> String {
    let mut path = routes::view_path(name);
    for (i, (k, v)) in query.iter().enumerate() {
        path.push(if i == 0 { '?' } else { '&' });
        path.push_str(&format!("{k}={}", encode(v)));
    }
    path
}

impl Api {
    /// A command: its input is the type the server's route reads. `Err` for any answer that is not 2xx.
    pub fn command<T: Serialize>(&self, name: &str, input: &T) -> Result<Reply, Failure> {
        assert!(routes::COMMANDS.contains(&name), "no command {name}");
        let body = serde_json::to_vec(input).map_err(|e| Failure::new(exit::OTHER, format!("the input of {name} cannot be written: {e}")))?;
        let reply = self.client.request("POST", &routes::command_path(name), Some(&body))?;
        self.checked(reply)
    }

    pub fn view(&self, name: &str, query: &[(&str, &str)]) -> Result<Reply, Failure> {
        assert!(routes::VIEWS.contains(&name), "no view {name}");
        let reply = self.client.request("GET", &view_path(name, query), None)?;
        self.checked(reply)
    }

    pub fn checked(&self, reply: Reply) -> Result<Reply, Failure> {
        if reply.ok() {
            Ok(reply)
        } else {
            Err(self.failure(&reply))
        }
    }

    /// The rows of a view's answer.
    pub fn rows(reply: &Reply) -> Result<Vec<Value>, Failure> {
        let v: Value = serde_json::from_str(&reply.body).map_err(|e| Failure::new(exit::OTHER, format!("the answer is not JSON: {e}")))?;
        match v.get("rows").and_then(Value::as_array) {
            Some(rows) => Ok(rows.clone()),
            None => Err(Failure::new(exit::OTHER, "the answer of the view has no rows member")),
        }
    }

    /// An answer that is not 2xx as a failure: the exit code of its status and one line with the
    /// error's code and message as the body gives them.
    pub fn failure(&self, reply: &Reply) -> Failure {
        let code = match reply.status {
            400 | 413 | 422 => exit::REFUSED,
            401 | 403 => exit::DENIED,
            404 => exit::NOT_FOUND,
            409 => exit::CONFLICT,
            _ => exit::OTHER,
        };
        let (name, message) = describe(&reply.body, reply.status);
        let mut line = format!("{} {name}: {message}", reply.status);
        if reply.status == 401 && !self.client.has_token() {
            line.push_str(" (no token is configured: give --token-file <file> or set RUNBOOK_TOKEN_FILE)");
        }
        Failure::new(code, line)
    }
}

/// The error's code and message: `error` and the outcome with its payload for a refusal of the
/// specification, `refused` and `message` for an answer outside it, else the status and the body.
fn describe(body: &str, status: u16) -> (String, String) {
    let parsed: Option<Value> = serde_json::from_str(body).ok();
    if let Some(v) = &parsed {
        if let (Some(code), Some(message)) = (v.get("refused").and_then(Value::as_str), v.get("message").and_then(Value::as_str)) {
            return (code.to_string(), message.to_string());
        }
        if let Some(code) = v.get("error").and_then(Value::as_str) {
            let outcome = v.get("outcome").and_then(Value::as_str).unwrap_or("refused");
            let message = match v.get("payload") {
                Some(p) if !p.is_null() => format!("{outcome} {p}"),
                _ => outcome.to_string(),
            };
            return (code.to_string(), message);
        }
    }
    let text: String = body.trim().chars().take(200).collect();
    (format!("http-{status}"), if text.is_empty() { "no body".to_string() } else { text })
}
