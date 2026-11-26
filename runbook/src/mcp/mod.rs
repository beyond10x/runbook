//! MCP, the third face of the command layer (`crate::commands`): the run commands as tools.
//!
//! HTTP, MCP and the CLI call the same functions. A tool is a thin name for a route of
//! `crate::api::routes`: its call is `routes::command_answer` or `routes::view_answer` and its result
//! carries the JSON body of that route unchanged. The input schema of a tool is derived from the
//! generated OpenAPI document of the daemon (`schema`), never typed a second time.
//!
//! Two transports, one dispatcher (`rpc`):
//! - stdio: `runbook mcp`, newline-delimited JSON-RPC on standard input and output (`stdio`). It is a
//!   client of the daemon named by `--daemon` or `RUNBOOK_DAEMON` (`remote`).
//! - streamable HTTP: `runbook serve --mcp`, one POST path (`http`), behind the daemon's one token check.
//!
//! The revision is named in `docs/mcp.md`; the constants of it are in `rpc`.

pub mod http;
pub mod remote;
pub mod rpc;
pub mod schema;
pub mod stdio;
pub mod tools;

use crate::api::routes::{self, Answer};
use crate::commands::Daemon;
use axum::http::StatusCode;
use serde_json::json;
use std::collections::HashMap;

/// The path of the MCP endpoint of `runbook serve --mcp`.
pub const PATH: &str = "/mcp";

/// What a tool needs of the daemon: the two calls a route is (a command with a JSON body, a view
/// with a query). The daemon of this process is `Local`; a daemon named by URL is `remote::Remote`.
pub trait Backend {
    fn command(&self, name: &str, body: &[u8]) -> Answer;
    fn view(&self, name: &str, query: &HashMap<String, String>) -> Answer;
    /// Who calls through this server, sent with a delegation (`docs/model-routes.md`, Part two, "What is recorded"): the MCP
    /// client's name (`clientInfo.name` of `initialize`) and the session's label (`RUNBOOK_CALLER`). None in process.
    fn caller(&self) -> (Option<String>, Option<String>) {
        (None, None)
    }
    /// The MCP client named itself in `initialize`.
    fn client_named(&self, _name: &str) {}
}

/// The daemon in this process: the routes' own functions.
pub struct Local(pub Daemon);

impl Backend for Local {
    fn command(&self, name: &str, body: &[u8]) -> Answer {
        routes::command_answer(name, &self.0, body)
    }

    fn view(&self, name: &str, query: &HashMap<String, String>) -> Answer {
        routes::view_answer(name, &self.0, query)
    }
}

/// An answer that is no outcome of the specification, in the body shape of the HTTP routes.
pub fn refused(status: StatusCode, what: &str, message: &str) -> Answer {
    Answer { status, body: json!({"refused": what, "message": message}), location: None }
}

/// The router of the MCP endpoint of a daemon in this process. The caller puts it behind the token
/// check (`api::serve_extra` does).
pub fn router(daemon: Daemon) -> axum::Router {
    http::router(std::sync::Arc::new(Local(daemon)))
}
