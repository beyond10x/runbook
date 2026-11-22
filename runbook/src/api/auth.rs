//! The security floor: one check, in one middleware, for every route.
//!
//! With a token set, a request without `Authorization: Bearer <token>` gets 401 on every route,
//! unknown routes included. The token is compared in constant time (over the SHA-256 digests of the
//! two values, so the length of the token is not leaked either) and never logged.

use super::refusal;
use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

pub struct Auth {
    digest: Option<[u8; 32]>,
    /// With `--ui`: a read (`GET`, `HEAD`) without an `Authorization` header may carry the token in
    /// the cookie a browser got from the login form (`crate::ui_embed`); `admits` checks it the same way.
    ui_cookie: bool,
}

fn digest(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

impl Auth {
    pub fn new(token: Option<&str>) -> Auth {
        Auth { digest: token.map(digest), ui_cookie: false }
    }

    pub fn with_ui_cookie(mut self) -> Auth {
        self.ui_cookie = true;
        self
    }

    pub fn required(&self) -> bool {
        self.digest.is_some()
    }

    /// Whether the value of an `Authorization` header carries the token.
    pub fn admits(&self, authorization: Option<&str>) -> bool {
        let Some(want) = &self.digest else { return true };
        let presented = authorization
            .and_then(|v| v.split_once(' '))
            .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
            .map_or("", |(_, token)| token.trim());
        let got = digest(presented);
        let mut diff = u8::from(authorization.is_none());
        for (a, b) in got.iter().zip(want) {
            diff |= a ^ b;
        }
        diff == 0
    }
}

pub async fn guard(State(auth): State<Arc<Auth>>, request: Request, next: Next) -> Response {
    let header = request.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    let cookie = match header {
        None if auth.ui_cookie && matches!(*request.method(), Method::GET | Method::HEAD) => crate::ui_embed::cookie_token(request.headers()).map(|t| format!("Bearer {t}")),
        _ => None,
    };
    if auth.admits(cookie.as_deref().or(header)) {
        return next.run(request).await;
    }
    let mut response = refusal(StatusCode::UNAUTHORIZED, "unauthorized", "a bearer token is required: send Authorization: Bearer <token>");
    response.headers_mut().insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
    response
}

/// The floor at start: a listen address that is not loopback needs a token.
pub fn check_listen(addr: &SocketAddr, token: Option<&str>) -> Result<(), String> {
    if addr.ip().is_loopback() || token.is_some() {
        return Ok(());
    }
    Err(format!("a token is required to listen on {addr}: it is not a loopback address; give --token-file <file> or set RUNBOOK_TOKEN"))
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
