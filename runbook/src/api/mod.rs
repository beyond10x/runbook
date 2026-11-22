//! HTTP with JSON on tokio and axum: one face of the command layer (`crate::commands`).
//!
//! Routes are exactly those of `ess/generated/openapi/openapi/runbookd.yaml`; the status codes and
//! body shapes of the commands are the document's (they come from `commands::OUTCOMES`). Beside them:
//! 400 for a body or query the command layer cannot take (the document has none), 401 from the
//! security floor, 404 for any other path, 405 for a wrong method, 406 for an event view asked as a
//! stream (streams are the next part of the story), 413 for a body over the limit.

pub mod auth;
pub mod routes;

use crate::commands::{Daemon, MAX_INTAKE_BYTES};
use auth::Auth;
use axum::body::Body;
use axum::extract::{Query, Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{middleware, Router};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::sync::Arc;

/// The largest body the daemon reads: what the largest valid submit can need, a little over. The
/// intake takes at most 1 MiB of bytes, and JSON writes a control character as six bytes
/// (`\u0001`), so a valid intake needs at most six times that; 64 KiB are left for the other members.
pub const MAX_BODY_BYTES: usize = MAX_INTAKE_BYTES * 6 + 64 * 1024;

pub fn json_response(status: StatusCode, body: &Value) -> Response {
    let mut response = Response::new(Body::from(serde_json::to_vec(body).unwrap_or_default()));
    *response.status_mut() = status;
    response.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
}

/// An answer that is no outcome of the specification: `{"refused": <what>, "message": <why>}`.
pub fn refusal(status: StatusCode, what: &str, message: &str) -> Response {
    json_response(status, &json!({"refused": what, "message": message}))
}

#[derive(Clone)]
struct App {
    daemon: Daemon,
}

/// The body, or `None` when it is over the limit: refused on the declared length before any of it is
/// read, and cut off at the limit while it streams in when no length is declared.
async fn read_body(request: Request) -> Option<Vec<u8>> {
    let declared = request.headers().get(header::CONTENT_LENGTH).and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<usize>().ok());
    if declared.is_some_and(|n| n > MAX_BODY_BYTES) {
        return None;
    }
    axum::body::to_bytes(request.into_body(), MAX_BODY_BYTES).await.ok().map(|b| b.to_vec())
}

async fn command_route(app: App, name: &'static str, request: Request) -> Response {
    match read_body(request).await {
        Some(body) => routes::command(name, &app.daemon, &body),
        None => refusal(StatusCode::PAYLOAD_TOO_LARGE, "request body too large", &format!("the body is at most {MAX_BODY_BYTES} bytes")),
    }
}

async fn view_route(app: App, name: &'static str, query: HashMap<String, String>, request: Request) -> Response {
    routes::view(name, &app.daemon, &query, request.headers())
}

async fn not_found() -> Response {
    refusal(StatusCode::NOT_FOUND, "not found", "no such route")
}

async fn method_not_allowed() -> Response {
    refusal(StatusCode::METHOD_NOT_ALLOWED, "method not allowed", "commands are POST, views are GET")
}

/// The router of the daemon. With a token, the one middleware that checks it wraps every route, the
/// fallback included.
pub fn router(daemon: Daemon, token: Option<&str>) -> Router {
    let app = App { daemon };
    let mut router = Router::new();
    for name in routes::COMMANDS {
        router = router.route(
            &routes::command_path(name),
            post(move |State(app): State<App>, request: Request| command_route(app, name, request)),
        );
    }
    for name in routes::VIEWS {
        router = router.route(
            &routes::view_path(name),
            get(move |State(app): State<App>, Query(query): Query<HashMap<String, String>>, request: Request| view_route(app, name, query, request)),
        );
    }
    router
        .method_not_allowed_fallback(method_not_allowed)
        .fallback(not_found)
        .with_state(app)
        .layer(middleware::from_fn_with_state(Arc::new(Auth::new(token)), auth::guard))
}

/// How streams behave: the keep-alive interval and the most events a live stream may fall behind.
#[derive(Clone, Debug)]
pub struct Streams {
    pub keep_alive: std::time::Duration,
    pub max_lag: usize,
}

impl Default for Streams {
    fn default() -> Streams {
        Streams { keep_alive: std::time::Duration::from_secs(15), max_lag: 1024 }
    }
}

/// `serve` with the behaviour of streams given.
pub async fn serve_with(
    listener: tokio::net::TcpListener,
    daemon: Daemon,
    token: Option<&str>,
    _streams: Streams,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    serve(listener, daemon, token, shutdown).await
}

/// Serve until `shutdown` completes.
pub async fn serve(listener: tokio::net::TcpListener, daemon: Daemon, token: Option<&str>, shutdown: impl Future<Output = ()> + Send + 'static) -> io::Result<()> {
    axum::serve(listener, router(daemon, token)).with_graceful_shutdown(shutdown).await
}
