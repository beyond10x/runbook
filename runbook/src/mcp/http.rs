//! The MCP endpoint of `runbook serve --mcp`: streamable HTTP, one path (`super::PATH`), POST only.
//!
//! Each POST is one JSON-RPC request or notification; a request is answered with one JSON object
//! (`application/json`), a notification with 202 and no body. No stream, no session: GET and DELETE
//! are 405, an `Mcp-Session-Id` or `Last-Event-ID` header is ignored. The endpoint is mounted behind
//! the daemon's one token check (`api::guarded`), so a request without the token is refused 401 before
//! this code runs. An `Origin` header that is not the host the request was sent to is 403 (DNS
//! rebinding).

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
use std::sync::Arc;

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

/// Whether the `Origin` is the host the request was sent to.
fn origin_ok(h: &HeaderMap) -> bool {
    let Some(origin) = h.get(header::ORIGIN) else { return true };
    let origin = origin.to_str().unwrap_or_default();
    let authority = origin.split_once("://").map_or(origin, |(_, rest)| rest).trim_end_matches('/');
    text(h, "host").is_some_and(|host| host.eq_ignore_ascii_case(authority))
}

async fn post_message(State(backend): State<Shared>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    if !origin_ok(&parts.headers) {
        return json_response(StatusCode::FORBIDDEN, &json!({"jsonrpc": "2.0", "error": {"code": rpc::INVALID_REQUEST, "message": "Origin is not the host of this request"}}));
    }
    let Ok(bytes): Result<Bytes, _> = axum::body::to_bytes(body, MAX_BODY_BYTES).await else {
        return json_response(StatusCode::PAYLOAD_TOO_LARGE, &json!({"refused": "request body too large", "message": format!("the body is at most {MAX_BODY_BYTES} bytes")}));
    };
    let headers = Headers { protocol_version: text(&parts.headers, "mcp-protocol-version"), method: text(&parts.headers, "mcp-method"), name: text(&parts.headers, "mcp-name") };
    let reply = rpc::handle(backend.as_ref(), &bytes, Wire::Http(&headers));
    match reply.message {
        Some(m) => json_response(StatusCode::from_u16(reply.status).unwrap_or(StatusCode::OK), &m),
        None => Response::builder().status(StatusCode::ACCEPTED).body(Body::empty()).unwrap_or_default(),
    }
}
