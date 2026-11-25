//! HTTP with JSON on tokio and axum: one face of the command layer (`crate::commands`).
//!
//! Routes are exactly those of `ess/generated/openapi/openapi/runbookd.yaml`; the status codes and
//! body shapes of the commands are the document's (they come from `commands::OUTCOMES`). Beside them:
//! 400 for a body or query the command layer cannot take (the document has none), 401 from the
//! security floor, 404 for any other path, 405 for a wrong method, 413 for a body over the limit.
//! An event view asked with `Accept: text/event-stream` is a stream (`sse`).

pub mod auth;
pub mod routes;
pub mod sse;

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
use std::future::{Future, IntoFuture};
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

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
    streams: Streams,
    closing: watch::Receiver<bool>,
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
    if routes::EVENT_VIEWS.contains(&name) && routes::wants_stream(request.headers()) {
        return sse::answer(name, &app.daemon, &query, request.headers(), &app.streams, app.closing.clone());
    }
    if routes::WAITING_VIEWS.contains(&name) {
        // a view that may wait on the network waits off the workers that answer every other request
        let daemon = app.daemon.clone();
        return tokio::task::spawn_blocking(move || routes::view(name, &daemon, &query))
            .await
            .unwrap_or_else(|_| refusal(StatusCode::INTERNAL_SERVER_ERROR, "internal error", "the view failed"));
    }
    routes::view(name, &app.daemon, &query)
}

async fn not_found() -> Response {
    refusal(StatusCode::NOT_FOUND, "not found", "no such route")
}

async fn method_not_allowed() -> Response {
    refusal(StatusCode::METHOD_NOT_ALLOWED, "method not allowed", "commands are POST, views are GET")
}

/// The header every answer carries while the daemon drains: the count of live runs it waits for. The
/// views and commands are the specification's, so a read says "draining" here and not in a row.
pub const DRAINING_HEADER: &str = "x-runbook-draining";

async fn drain_header(State(daemon): State<Daemon>, request: Request, next: middleware::Next) -> Response {
    let mut response = next.run(request).await;
    let live = daemon.read(|s| s.draining().then(|| s.live_run_ids().len()));
    if let Some(live) = live {
        if let Ok(v) = HeaderValue::from_str(&live.to_string()) {
            response.headers_mut().insert(DRAINING_HEADER, v);
        }
    }
    response
}

/// The router of the daemon. With a token, the one middleware that checks it wraps every route, the
/// fallback included.
pub fn router(daemon: Daemon, token: Option<&str>) -> Router {
    router_with(daemon, token, Streams::default(), watch::channel(false).1)
}

/// The router with the behaviour of streams given and the signal that closes them.
pub fn router_with(daemon: Daemon, token: Option<&str>, streams: Streams, closing: watch::Receiver<bool>) -> Router {
    router_full(daemon, token, streams, closing, false, None, None)
}

/// The whole router: the API, the embedded UI beside it when `ui` is set (`crate::ui_embed`: its
/// files are the fallback and `POST /ui/login` is a route), and the routes of `extra` (the MCP
/// endpoint). One middleware (`auth::guard`) wraps all of them, the fallback included. `listen` is
/// the address the listener is bound to; with it the guard checks `Host` on a loopback listener.
pub fn router_full(daemon: Daemon, token: Option<&str>, streams: Streams, closing: watch::Receiver<bool>, ui: bool, extra: Option<Router>, listen: Option<SocketAddr>) -> Router {
    let drain_daemon = daemon.clone();
    let app = App { daemon, streams, closing };
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
    let mut auth = Auth::new(token);
    if ui {
        auth = auth.with_ui();
    }
    if let Some(listen) = listen {
        auth = auth.with_listen(listen);
    }
    let auth = Arc::new(auth);
    let router = router.method_not_allowed_fallback(method_not_allowed);
    let router = if ui { router.fallback(crate::ui_embed::fallback) } else { router.fallback(not_found) };
    let mut router = router.with_state(app);
    if let Some(extra) = extra {
        router = router.merge(extra);
    }
    if ui {
        router = router.merge(Router::new().route("/ui/login", post(crate::ui_embed::login)).with_state(auth.clone()));
    }
    let router = router.layer(middleware::from_fn_with_state(auth, auth::guard));
    let router = if ui { router.layer(middleware::from_fn(crate::ui_embed::login_form)) } else { router };
    // Outermost, so that an answer of the guard (401, 403) or of the login form carries the header too.
    router.layer(middleware::from_fn_with_state(drain_daemon, drain_header))
}

/// How streams behave: the interval of the keep-alive comment and the most events a live stream may
/// fall behind before it is closed (it resumes by position).
#[derive(Clone, Debug)]
pub struct Streams {
    pub keep_alive: Duration,
    pub max_lag: usize,
}

impl Default for Streams {
    fn default() -> Streams {
        Streams { keep_alive: Duration::from_secs(15), max_lag: 1024 }
    }
}

/// How long the connections get to finish after the shutdown began; then `serve` returns.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(1);

/// Serve until `shutdown` completes: every open stream is closed at once, the other requests are
/// finished, and a connection that does not finish within a second (a client that does not read) no
/// longer holds `serve` back.
pub async fn serve(listener: tokio::net::TcpListener, daemon: Daemon, token: Option<&str>, shutdown: impl Future<Output = ()> + Send + 'static) -> io::Result<()> {
    serve_with(listener, daemon, token, Streams::default(), shutdown).await
}

/// `serve` with the behaviour of streams given.
pub async fn serve_with(
    listener: tokio::net::TcpListener,
    daemon: Daemon,
    token: Option<&str>,
    streams: Streams,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    serve_all(listener, daemon, token, streams, false, None, shutdown).await
}

/// `serve` with the embedded UI beside the API when `ui` is set.
pub async fn serve_ui(listener: tokio::net::TcpListener, daemon: Daemon, token: Option<&str>, ui: bool, shutdown: impl Future<Output = ()> + Send + 'static) -> io::Result<()> {
    serve_all(listener, daemon, token, Streams::default(), ui, None, shutdown).await
}

/// `serve_with` and the routes of `extra`: the MCP endpoint (`crate::mcp::router`) is such a router.
pub async fn serve_extra(
    listener: tokio::net::TcpListener,
    daemon: Daemon,
    token: Option<&str>,
    streams: Streams,
    extra: Option<Router>,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    serve_all(listener, daemon, token, streams, false, extra, shutdown).await
}

/// Everything `runbook serve` can mount: the API, the UI when `ui` is set and the routes of
/// `extra`, all behind the one guard (`auth::guard`), which knows the address of the listener.
pub async fn serve_all(
    listener: tokio::net::TcpListener,
    daemon: Daemon,
    token: Option<&str>,
    streams: Streams,
    ui: bool,
    extra: Option<Router>,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    let (closing_tx, closing) = watch::channel(false);
    let router = router_full(daemon, token, streams, closing.clone(), ui, extra, listener.local_addr().ok());
    let serving = axum::serve(listener, router).with_graceful_shutdown(async move {
        shutdown.await;
        let _ = closing_tx.send(true);
    });
    let mut serving = std::pin::pin!(serving.into_future());
    tokio::select! {
        served = &mut serving => served,
        () = async { sse::closed(closing).await; tokio::time::sleep(SHUTDOWN_GRACE).await } => Ok(()),
    }
}
