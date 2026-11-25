//! The JSON-RPC 2.0 layer of MCP, the same on both transports.
//!
//! Revision implemented: the MCP specification `2026-07-28` (the current one), the "modern" era:
//! stateless requests that carry `_meta["io.modelcontextprotocol/protocolVersion"]` and
//! `...clientCapabilities`, `server/discover`, results with `resultType`, no `initialize`. The server
//! is also "dual-era" as that revision's Versioning page allows: a client of the "legacy" revisions
//! `2025-11-25` and `2025-06-18` opens with `initialize` and `notifications/initialized` and is
//! answered in those revisions' shape. The era is chosen per request by how the client opens.
//!
//! Methods: `initialize` (legacy), `server/discover` (modern), `ping`, `tools/list`, `tools/call`,
//! the notification `notifications/initialized` and any other notification (accepted, no reply).

use super::tools::{self, Invalid};
use super::Backend;
use serde_json::{json, Map, Value};

pub const MODERN: &str = "2026-07-28";
/// The legacy revisions that are answered, newest first.
pub const LEGACY: [&str; 2] = ["2025-11-25", "2025-06-18"];
/// Every protocol version this server speaks, as `server/discover` and the unsupported-version error list them.
pub const SUPPORTED: [&str; 3] = [MODERN, LEGACY[0], LEGACY[1]];

pub const SERVER_NAME: &str = "runbook";
const INSTRUCTIONS: &str = "Runs of a runbook on the Runbook daemon: submit_run, list_runs, read_run, read_events, read_open_decisions, answer_decision, cancel_run, open_session, close_session, list_products, read_product, list_routes, and agent work on a named model route: delegate, delegation_result. Each result is the JSON body the daemon's HTTP route of the same name answers (delegate and delegation_result: the row of the view delegation-result, with a text that starts with one line naming the run); isError is true and _meta[\"runbook/http-status\"] is the HTTP status when the route refused.";

pub const META_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
pub const META_CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";
pub const META_SERVER_INFO: &str = "io.modelcontextprotocol/serverInfo";

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;
pub const HEADER_MISMATCH: i64 = -32020;
pub const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32022;

/// The headers of a streamable HTTP request the dispatcher validates against the body.
#[derive(Default, Debug, Clone)]
pub struct Headers {
    pub protocol_version: Option<String>,
    pub method: Option<String>,
    pub name: Option<String>,
}

/// Where a message came from. On stdio the legacy handshake is state of the process; on HTTP there
/// is no state, and a legacy request names its revision in the `MCP-Protocol-Version` header.
pub enum Wire<'a> {
    Stdio { legacy: &'a mut bool },
    Http(&'a Headers),
}

/// What to send back: the HTTP status (stdio ignores it) and the message, if any.
#[derive(Debug)]
pub struct Reply {
    pub status: u16,
    pub message: Option<Value>,
}

fn error(id: Value, code: i64, message: &str, data: Option<Value>) -> Value {
    let mut e = json!({"code": code, "message": message});
    if let Some(d) = data {
        e["data"] = d;
    }
    json!({"jsonrpc": "2.0", "id": id, "error": e})
}

/// The HTTP status of a JSON-RPC error: 400 for a request the server cannot take as sent, 404 for a
/// method it does not have, 500 for its own failure.
fn status_of(code: i64) -> u16 {
    match code {
        PARSE_ERROR | INVALID_REQUEST | INVALID_PARAMS | HEADER_MISMATCH | UNSUPPORTED_PROTOCOL_VERSION => 400,
        METHOD_NOT_FOUND => 404,
        INTERNAL_ERROR => 500,
        _ => 200,
    }
}

fn fail(id: Value, code: i64, message: &str, data: Option<Value>) -> Reply {
    Reply { status: status_of(code), message: Some(error(id, code, message, data)) }
}

fn ok(id: Value, result: Value) -> Reply {
    Reply { status: 200, message: Some(json!({"jsonrpc": "2.0", "id": id, "result": result})) }
}

fn server_info() -> Value {
    json!({"name": SERVER_NAME, "title": "Runbook", "version": env!("CARGO_PKG_VERSION")})
}

/// The text of an `Mcp-Name` header value, decoded when it is in the base64 sentinel form.
fn decode_header(value: &str) -> Option<String> {
    match value.strip_prefix("=?base64?").and_then(|v| v.strip_suffix("?=")) {
        Some(b64) => String::from_utf8(base64_decode(b64)?).ok(),
        None => Some(value.to_string()),
    }
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.trim_end_matches('=').bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Handle one message: the bytes of a line (stdio) or a POST body (HTTP). Notifications a call would send are dropped.
pub fn handle(be: &dyn Backend, input: &[u8], wire: Wire) -> Reply {
    handle_with(be, input, wire, &mut |_| {})
}

/// `handle`, with the notifications a call sends while it runs (the progress of `delegate`) given to `notify`, each before the reply.
pub fn handle_with(be: &dyn Backend, input: &[u8], mut wire: Wire, notify: &mut dyn FnMut(Value)) -> Reply {
    let msg: Value = match serde_json::from_slice(input) {
        Ok(v) => v,
        Err(e) => return fail(Value::Null, PARSE_ERROR, &format!("not JSON: {}", crate::commands::out_of_range_member(input, &e).unwrap_or_else(|| e.to_string())), None),
    };
    let Some(obj) = msg.as_object() else {
        return fail(Value::Null, INVALID_REQUEST, "a message is one JSON object (batches are not part of this revision)", None);
    };
    let id = obj.get("id").cloned();
    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        // A response from the client: no request of this server is ever outstanding.
        return match wire {
            Wire::Http(_) => fail(id.unwrap_or(Value::Null), INVALID_REQUEST, "the client sends requests and notifications, not responses", None),
            Wire::Stdio { .. } => Reply { status: 202, message: None },
        };
    };
    let Some(id) = id else {
        return Reply { status: 202, message: None };
    };
    if !(id.is_string() || id.is_i64() || id.is_u64()) {
        return fail(Value::Null, INVALID_REQUEST, "the id of a request is a string or an integer", None);
    }
    let params = obj.get("params").cloned().unwrap_or_else(|| json!({}));
    if !params.is_object() {
        return fail(id, INVALID_PARAMS, "params must be an object", None);
    }
    request(be, &mut wire, id, method, &params, notify)
}

enum Era {
    Modern,
    Legacy,
}

fn era(wire: &Wire, id: &Value, method: &str, params: &Value) -> Result<Era, Reply> {
    let meta = params.get("_meta").and_then(Value::as_object);
    match meta.and_then(|m| m.get(META_VERSION)) {
        Some(Value::String(v)) if v == MODERN => {
            if !meta.is_some_and(|m| m.get(META_CAPABILITIES).is_some_and(Value::is_object)) {
                return Err(fail(id.clone(), INVALID_PARAMS, &format!("a request needs _meta[\"{META_CAPABILITIES}\"], an object"), None));
            }
            Ok(Era::Modern)
        }
        Some(Value::String(v)) if LEGACY.contains(&v.as_str()) => Ok(Era::Legacy),
        Some(other) => Err(fail(
            id.clone(),
            UNSUPPORTED_PROTOCOL_VERSION,
            "Unsupported protocol version",
            Some(json!({"supported": SUPPORTED, "requested": other})),
        )),
        None => {
            let legacy_session = match wire {
                Wire::Stdio { legacy } => **legacy,
                Wire::Http(h) => h.protocol_version.as_deref().is_some_and(|v| LEGACY.contains(&v)),
            };
            if legacy_session || method == "ping" {
                Ok(Era::Legacy)
            } else {
                Err(fail(
                    id.clone(),
                    INVALID_PARAMS,
                    &format!("a request needs _meta[\"{META_VERSION}\"] (revision {MODERN}); a client of an earlier revision sends initialize first"),
                    None,
                ))
            }
        }
    }
}

/// The checks of a modern request on HTTP: the headers equal the body.
fn check_headers(h: &Headers, id: &Value, method: &str, params: &Value) -> Result<(), Reply> {
    let mismatch = |what: String| Err(fail(id.clone(), HEADER_MISMATCH, &format!("Header mismatch: {what}"), None));
    match h.protocol_version.as_deref() {
        Some(v) if v == MODERN => {}
        Some(v) => return mismatch(format!("MCP-Protocol-Version {v:?} does not match the request's {MODERN:?}")),
        None => return mismatch("MCP-Protocol-Version is missing".into()),
    }
    match h.method.as_deref() {
        Some(m) if m == method => {}
        Some(m) => return mismatch(format!("Mcp-Method {m:?} does not match the method {method:?}")),
        None => return mismatch("Mcp-Method is missing".into()),
    }
    if method == "tools/call" {
        let want = params.get("name").and_then(Value::as_str).unwrap_or_default();
        match h.name.as_deref().map(decode_header) {
            Some(Some(n)) if n == want => {}
            Some(_) => return mismatch("Mcp-Name does not match the tool name".into()),
            None => return mismatch("Mcp-Name is missing".into()),
        }
    }
    Ok(())
}

fn request(be: &dyn Backend, wire: &mut Wire, id: Value, method: &str, params: &Value, notify: &mut dyn FnMut(Value)) -> Reply {
    if method == "initialize" {
        return initialize(wire, id, params);
    }
    let era = match era(wire, &id, method, params) {
        Ok(e) => e,
        Err(r) => return r,
    };
    if let (Era::Modern, Wire::Http(h)) = (&era, &*wire) {
        if let Err(r) = check_headers(h, &id, method, params) {
            return r;
        }
    }
    let result = match (method, &era) {
        ("ping", _) => json!({}),
        ("server/discover", Era::Modern) => json!({
            "supportedVersions": SUPPORTED,
            "capabilities": {"tools": {}},
            "instructions": INSTRUCTIONS,
        }),
        ("tools/list", _) => json!({"tools": tools::list()}),
        ("tools/call", _) => match tools_call(be, params, notify) {
            Ok(r) => r,
            Err(Invalid::UnknownTool(n)) => return fail(id, INVALID_PARAMS, &format!("Unknown tool: {n}"), None),
            Err(Invalid::Arguments(m)) => return fail(id, INVALID_PARAMS, &m, None),
        },
        _ => return fail(id, METHOD_NOT_FOUND, &format!("Method not found: {method}"), None),
    };
    ok(id, decorate(result, &era))
}

/// A modern result says it is complete and who answered; a legacy result is bare.
fn decorate(mut result: Value, era: &Era) -> Value {
    if let (Era::Modern, Some(m)) = (era, result.as_object_mut()) {
        m.insert("resultType".into(), json!("complete"));
        let meta = m.entry("_meta").or_insert_with(|| Value::Object(Map::new()));
        if let Some(meta) = meta.as_object_mut() {
            meta.insert(META_SERVER_INFO.into(), server_info());
        }
    }
    result
}

fn tools_call(be: &dyn Backend, params: &Value, notify: &mut dyn FnMut(Value)) -> Result<Value, Invalid> {
    let name = params.get("name").and_then(Value::as_str).ok_or_else(|| Invalid::Arguments("tools/call needs params.name, a string".into()))?;
    let arguments = params.get("arguments").unwrap_or(&Value::Null);
    // A progress token is a string or an integer (`_meta.progressToken`); without one the call sends no progress.
    let token = params.get("_meta").and_then(|m| m.get("progressToken")).filter(|t| t.is_string() || t.is_i64() || t.is_u64()).cloned();
    let progress = token.map(|token| tools::Progress { token, send: notify });
    tools::run(be, name, arguments, progress)
}

fn initialize(wire: &mut Wire, id: Value, params: &Value) -> Reply {
    let Some(asked) = params.get("protocolVersion").and_then(Value::as_str) else {
        return fail(id, INVALID_PARAMS, "initialize needs params.protocolVersion, a string", None);
    };
    // The client's revision when this server answers it, else the newest legacy one: the client
    // then decides whether it can speak that.
    let version = LEGACY.iter().find(|v| **v == asked).unwrap_or(&LEGACY[0]);
    if let Wire::Stdio { legacy } = wire {
        **legacy = true;
    }
    ok(
        id,
        json!({
            "protocolVersion": version,
            "capabilities": {"tools": {"listChanged": false}},
            "serverInfo": server_info(),
            "instructions": INSTRUCTIONS,
        }),
    )
}
