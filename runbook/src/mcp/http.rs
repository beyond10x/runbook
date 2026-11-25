//! The MCP endpoint of `runbook serve --mcp`: streamable HTTP, one path (`super::PATH`), POST only.
//!
//! Each POST is one JSON-RPC request or notification; a request is answered with one JSON object
//! (`application/json`), a notification with 202 and no body. No stream, no session: GET and DELETE
//! are 405, an `Mcp-Session-Id` or `Last-Event-ID` header is ignored. The endpoint is mounted behind
//! the daemon's one guard (`api::auth::guard`): a request without the token is refused 401, one with a
//! foreign `Host` or `Origin` 403, one without `Content-Type: application/json` 415, before this code
//! runs.

use super::rpc::{self, Headers, Wire};
use super::{Backend, PATH};
use crate::api::{json_response, MAX_BODY_BYTES};
use axum::body::{Body, Bytes};
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::post;
use axum::Router;
use serde_json::json;
use std::sync::{Arc, OnceLock};

/// The most MCP calls of the HTTP endpoint answered at once on the blocking pool.
pub const MAX_WAITING: usize = 32;

fn waiting() -> &'static tokio::sync::Semaphore {
    static WAITING: OnceLock<tokio::sync::Semaphore> = OnceLock::new();
    WAITING.get_or_init(|| tokio::sync::Semaphore::new(MAX_WAITING))
}

type Shared = Arc<dyn Backend + Send + Sync>;

pub fn router(backend: Shared) -> Router {
    Router::new().route(PATH, post(post_message).fallback(method_not_allowed)).with_state(backend)
}

async fn method_not_allowed() -> Response {
    let mut r = json_response(StatusCode::METHOD_NOT_ALLOWED, &json!({"refused": "method not allowed", "message": "the MCP endpoint takes POST only: it has no stream and no session"}));
    r.headers_mut().insert(header::ALLOW, header::HeaderValue::from_static("POST"));
    r
}

fn text(h: &HeaderMap, name: &str) -> Option<String> {
    h.get(name).and_then(|v| v.to_str().ok()).map(str::to_string)
}

async fn post_message(State(backend): State<Shared>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let Ok(bytes): Result<Bytes, _> = axum::body::to_bytes(body, MAX_BODY_BYTES).await else {
        return json_response(StatusCode::PAYLOAD_TOO_LARGE, &json!({"refused": "request body too large", "message": format!("the body is at most {MAX_BODY_BYTES} bytes")}));
    };
    let headers = Headers { protocol_version: text(&parts.headers, "mcp-protocol-version"), method: text(&parts.headers, "mcp-method"), name: text(&parts.headers, "mcp-name") };
    // A call may wait (`delegate` waits for its run's end): off the workers, and at most `MAX_WAITING` at once on the blocking
    // pool, which the relay's admit and book share; the next one waits here, on no thread. The endpoint has no stream, so a
    // notification a call sends is dropped.
    let Ok(_permit) = waiting().acquire().await else {
        return json_response(StatusCode::INTERNAL_SERVER_ERROR, &json!({"refused": "internal error", "message": "the MCP call ended without an answer"}));
    };
    let Ok(reply) = tokio::task::spawn_blocking(move || rpc::handle(backend.as_ref(), &bytes, Wire::Http(&headers))).await else {
        return json_response(StatusCode::INTERNAL_SERVER_ERROR, &json!({"refused": "internal error", "message": "the MCP call ended without an answer"}));
    };
    match reply.message {
        Some(m) => json_response(StatusCode::from_u16(reply.status).unwrap_or(StatusCode::OK), &m),
        None => Response::builder().status(StatusCode::ACCEPTED).body(Body::empty()).unwrap_or_default(),
    }
}
