//! The security floor: one check, in one middleware, for every route.
//!
//! Every request, on every route (commands, views, event streams, `/mcp`, the UI files, `/ui/login`)
//! passes this order, and the first refusal ends it:
//!
//! 1. Host: on a loopback listener the `Host` header is `127.0.0.1:<port>`, `localhost:<port>`,
//!    `[::1]:<port>` or the listen address itself, else 403 (DNS rebinding). A listener that is not
//!    loopback needs a token and has no such check.
//! 2. Origin: a request with an `Origin` header is 403 unless that is `http://` and the host and port
//!    of the request's `Host` (`null` is refused). A request without `Origin` is not from a page.
//! 3. Token: with a token set, a request without `Authorization: Bearer <token>` gets 401, unknown
//!    routes included. The token is compared in constant time (over the SHA-256 digests of the two
//!    values, so the length of the token is not leaked either) and never logged. `POST /ui/login`
//!    is the one request that carries its token in its body; the handler checks it there.
//! 4. Content type: `POST /daemon/commands/<name>` and `POST /mcp` take `application/json` only,
//!    else 415 (a page cannot send that without a preflight the daemon does not answer).
//!
//! The refusals have constant bodies that name no header value.

use super::refusal;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use sha2::{Digest, Sha256};
use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::sync::Arc;

pub struct Auth {
    digest: Option<[u8; 32]>,
    /// The value of the cookie the login hands a browser: `cookie_value(token)`.
    cookie: Option<String>,
    /// With `--ui`: `POST /ui/login` carries its token in the body, and a read (`GET`, `HEAD`)
    /// without an `Authorization` header may carry the cookie of the login.
    ui: bool,
    /// The address the listener is bound to; known, it turns on the `Host` check of a loopback one.
    listen: Option<SocketAddr>,
}

fn digest(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

fn same(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

/// What the cookie of the login holds: lowercase hex of SHA-256 over `runbook-ui-read/1`, one NUL
/// byte and the token. It is derived, so the cookie does not show the token to another server of the
/// same host, and it is no bearer token: the daemon takes the cookie for reads only.
pub fn cookie_value(token: &str) -> String {
    let mut h = Sha256::new();
    h.update(b"runbook-ui-read/1");
    h.update([0u8]);
    h.update(token.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

impl Auth {
    pub fn new(token: Option<&str>) -> Auth {
        Auth { digest: token.map(digest), cookie: token.map(cookie_value), ui: false, listen: None }
    }

    pub fn with_ui(mut self) -> Auth {
        self.ui = true;
        self
    }

    pub fn with_listen(mut self, listen: SocketAddr) -> Auth {
        self.listen = Some(listen);
        self
    }

    pub fn required(&self) -> bool {
        self.digest.is_some()
    }

    /// The value of the cookie to hand a browser after a login, when a token is required.
    pub fn cookie(&self) -> Option<&str> {
        self.cookie.as_deref()
    }

    /// Whether the value of an `Authorization` header carries the token.
    pub fn admits(&self, authorization: Option<&str>) -> bool {
        let Some(want) = &self.digest else { return true };
        let presented = authorization
            .and_then(|v| v.split_once(' '))
            .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
            .map_or("", |(_, token)| token.trim());
        same(&digest(presented), want) && authorization.is_some()
    }

    /// Whether a cookie value is the one the login hands out; compared in constant time.
    fn admits_cookie(&self, value: &str) -> bool {
        match &self.cookie {
            Some(want) => same(&digest(value), &digest(want)),
            None => true,
        }
    }
}

/// A `host[:port]` as `(lowercase host, port)`, the port 80 when it is left out; `None` for what is
/// not that (an empty host, a path, user information, a port that is no number).
fn authority(s: &str) -> Option<(String, u16)> {
    let s = s.trim().to_ascii_lowercase();
    let (host, port) = if s.starts_with('[') {
        let end = s.find(']')?;
        let rest = &s[end + 1..];
        (&s[..=end], if rest.is_empty() { Some(80) } else { rest.strip_prefix(':').and_then(|p| p.parse::<u16>().ok()) })
    } else {
        match s.rsplit_once(':') {
            Some((h, p)) => (h, p.parse::<u16>().ok()),
            None => (s.as_str(), Some(80)),
        }
    };
    let name = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(host);
    let bracketed = name.len() != host.len();
    let ok = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') || (bracketed && c == ':'));
    if ok { Some((host.to_string(), port?)) } else { None }
}

fn host_of(request: &Request) -> Option<(String, u16)> {
    match request.headers().get(header::HOST) {
        Some(v) => v.to_str().ok().and_then(authority),
        None => request.uri().authority().and_then(|a| authority(a.as_str())),
    }
}

/// Whether `Host` names this loopback listener.
fn host_is_listener(host: Option<&(String, u16)>, listen: &SocketAddr) -> bool {
    let Some((name, port)) = host else { return false };
    if *port != listen.port() {
        return false;
    }
    let literal = match listen.ip() {
        IpAddr::V4(ip) => ip.to_string(),
        IpAddr::V6(ip) => format!("[{ip}]"),
    };
    matches!(name.as_str(), "127.0.0.1" | "localhost" | "[::1]") || *name == literal
}

/// Whether every `Origin` header is `http://` and the host and port of the request.
fn origin_is_host(headers: &HeaderMap, host: Option<&(String, u16)>) -> bool {
    headers.get_all(header::ORIGIN).iter().all(|origin| {
        let Some(rest) = origin.to_str().ok().and_then(|o| o.get(..7).filter(|p| p.eq_ignore_ascii_case("http://")).map(|_| &o[7..])) else { return false };
        !rest.contains(['/', '@', '?', '#']) && authority(rest).is_some() && authority(rest).as_ref() == host
    })
}

fn is_json(headers: &HeaderMap) -> bool {
    let mut kinds = headers.get_all(header::CONTENT_TYPE).iter();
    let one = kinds.next().and_then(|v| v.to_str().ok());
    kinds.next().is_none() && one.is_some_and(|v| v.split(';').next().is_some_and(|m| m.trim().eq_ignore_ascii_case("application/json")))
}

fn takes_json(request: &Request) -> bool {
    let path = request.uri().path();
    *request.method() == Method::POST && (path == crate::mcp::PATH || path.starts_with("/daemon/commands/"))
}

fn cookie_of(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|c| c.trim().split_once('='))
        .find(|(k, _)| *k == crate::ui_embed::COOKIE)
        .map(|(_, v)| v)
}

pub async fn guard(State(auth): State<Arc<Auth>>, request: Request, next: Next) -> Response {
    let host = host_of(&request);
    let loopback = auth.listen.filter(|l| l.ip().is_loopback());
    if loopback.is_some_and(|l| !host_is_listener(host.as_ref(), &l)) || !origin_is_host(request.headers(), host.as_ref()) {
        return refusal(StatusCode::FORBIDDEN, "forbidden", "this request is not for this daemon: its Host or Origin header is refused");
    }
    let login = auth.ui && *request.method() == Method::POST && request.uri().path() == "/ui/login";
    let header = request.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    let admitted = login
        || match header {
            Some(_) => auth.admits(header),
            None => auth.admits(None) || (auth.ui && matches!(*request.method(), Method::GET | Method::HEAD) && cookie_of(request.headers()).is_some_and(|c| auth.admits_cookie(c))),
        };
    if !admitted {
        let mut response = refusal(StatusCode::UNAUTHORIZED, "unauthorized", "a bearer token is required: send Authorization: Bearer <token>");
        response.headers_mut().insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        return response;
    }
    if takes_json(&request) && !is_json(request.headers()) {
        return refusal(StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported media type", "a command is sent with Content-Type: application/json");
    }
    next.run(request).await
}

/// The floor at start: a listen address that is not loopback needs a token.
pub fn check_listen(addr: &SocketAddr, token: Option<&str>) -> Result<(), String> {
    if addr.ip().is_loopback() || token.is_some() {
        return Ok(());
    }
    Err(format!("a token is required to listen on {addr}: it is not a loopback address; give --token-file <file> or set RUNBOOK_TOKEN"))
}

/// The shortest token `runbook serve` starts with, in bytes.
pub const MIN_TOKEN_BYTES: usize = 32;

/// The floor at start for a token the daemon is to check: at least `MIN_TOKEN_BYTES`. The message
/// names the minimum and where a token comes from, and never the token or its length. A client takes
/// the token it was given and does not call this.
pub fn check_token_length(token: Option<&str>) -> Result<(), String> {
    match token {
        Some(t) if t.len() < MIN_TOKEN_BYTES => Err(format!(
            "the token is too short: it must be at least {MIN_TOKEN_BYTES} bytes; put a longer one in the file named by --token-file (see docs/daemon-api.md, \"Running the daemon\")"
        )),
        _ => Ok(()),
    }
}

/// The token from a file (its mode must not give anything to others) or from the environment value.
pub fn load_token(file: Option<&Path>, env: Option<String>) -> Result<Option<String>, String> {
    let raw = match file {
        Some(path) => {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(path).map_err(|e| format!("token file {}: {e}", path.display()))?;
            let mode = meta.permissions().mode() & 0o777;
            if mode & 0o007 != 0 {
                return Err(format!("token file {} has mode {mode:04o}: it must not be accessible to others (chmod 600)", path.display()));
            }
            std::fs::read_to_string(path).map_err(|e| format!("token file {}: {e}", path.display()))?
        }
        None => match env {
            Some(v) => v,
            None => return Ok(None),
        },
    };
    let token = raw.trim_end_matches(['\n', '\r']).to_string();
    if token.is_empty() {
        return Err("the token is empty".to_string());
    }
    Ok(Some(token))
}
