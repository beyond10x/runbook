//! The web UI inside the binary: the files of `ui/web/` are compiled in (`build.rs` lists the
//! directory) and `runbook serve --ui` serves them beside the API.
//!
//! A request is looked up by its path exactly as it was sent: a path that is not in the list (a
//! directory, `..`, an encoded form of either) answers 404, and nothing is read from the disk at
//! run time. The token rule is the daemon's own: the same middleware and the same `Auth::admits`
//! guard these routes and the API. A browser cannot send an `Authorization` header with a page
//! load, so it hands the token over once in the body of a form (`POST /ui/login`, never in a URL,
//! so no log holds it) and keeps a value derived from it in an `HttpOnly`, `SameSite=Strict`
//! cookie (`auth::cookie_value`: not the token), which the middleware accepts for reads (`GET`,
//! `HEAD`) only. The login route sits behind the same middleware as every other route.

use crate::api::auth::Auth;
use crate::api::refusal;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

include!(concat!(env!("OUT_DIR"), "/ui_files.rs"));

/// The name of the cookie that carries the token a browser handed over.
pub const COOKIE: &str = "runbook_ui";

/// The embedded files: their paths below `ui/web/`, sorted.
pub fn paths() -> Vec<&'static str> {
    FILES.iter().map(|(p, _)| *p).collect()
}

/// How many files are embedded and how many bytes they hold.
pub fn size() -> (usize, usize) {
    (FILES.len(), FILES.iter().map(|(_, b)| b.len()).sum())
}

/// The SHA-256 of some bytes, in lowercase hex.
pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// The source of a content security policy for an inline script: `sha256-<base64 of the digest>`.
pub fn csp_hash(script: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let sum = Sha256::digest(script);
    let mut out = String::from("sha256-");
    for chunk in sum.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ABC[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

struct Entry {
    bytes: &'static [u8],
    content_type: &'static str,
    etag: HeaderValue,
}

struct Site {
    entries: HashMap<&'static str, Entry>,
    csp: HeaderValue,
}

/// The inline scripts of a page (`<script>` without attributes), each as its CSP hash source.
fn inline_script_hashes(html: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(html);
    text.split("<script>").skip(1).filter_map(|t| t.split("</script>").next()).map(|s| csp_hash(s.as_bytes())).collect()
}

/// The policy of every embedded file: scripts, styles, fonts and connections from this origin only.
/// `index.html` has one inline script, allowed by its hash. The page's own code writes `style="..."`
/// attributes into its markup, so attributes (and only attributes) may carry inline styles; a
/// `<style>` element may not. Images are this origin's, and `data:` for the arrow in `style.css`.
fn policy(index: &[u8]) -> String {
    let hashes: String = inline_script_hashes(index).iter().map(|h| format!(" '{h}'")).collect();
    format!(
        "default-src 'none'; script-src 'self'{hashes}; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'"
    )
}

fn site() -> &'static Site {
    static SITE: OnceLock<Site> = OnceLock::new();
    SITE.get_or_init(|| {
        let mut entries = HashMap::new();
        for (path, bytes) in FILES {
            let etag = HeaderValue::from_str(&format!("\"{}\"", digest(bytes))).unwrap();
            entries.insert(*path, Entry { bytes, content_type: content_type(path), etag });
        }
        let index = FILES.iter().find(|(p, _)| *p == "index.html").map_or(&[][..], |(_, b)| b);
        Site { entries, csp: HeaderValue::from_str(&policy(index)).unwrap() }
    })
}

fn secure(response: &mut Response, csp: &HeaderValue) {
    let h = response.headers_mut();
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(header::CONTENT_SECURITY_POLICY, csp.clone());
    h.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    // how the page knows the daemon served it (`ui/web/source.js`)
    h.insert("server-timing", HeaderValue::from_static("runbookd"));
}

/// The router's fallback with `--ui`: an embedded file, else the 404 of any unknown route.
pub async fn fallback(request: Request) -> Response {
    let site = site();
    let path = request.uri().path();
    let key = if path == "/" { "index.html" } else { path.strip_prefix('/').unwrap_or("") };
    let Some(entry) = site.entries.get(key) else {
        return refusal(StatusCode::NOT_FOUND, "not found", "no such route");
    };
    if request.method() != Method::GET && request.method() != Method::HEAD {
        return refusal(StatusCode::METHOD_NOT_ALLOWED, "method not allowed", "commands are POST, views and files are GET");
    }
    let matches = request
        .headers()
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .any(|t| t.trim() == "*" || t.trim().trim_start_matches("W/").as_bytes() == entry.etag.as_bytes());
    let mut response = if matches {
        let mut r = Response::new(Body::empty());
        *r.status_mut() = StatusCode::NOT_MODIFIED;
        r
    } else {
        let mut r = Response::new(Body::from(entry.bytes));
        r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static(entry.content_type));
        r
    };
    let h = response.headers_mut();
    h.insert(header::ETAG, entry.etag.clone());
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, no-cache"));
    secure(&mut response, &site.csp);
    response
}

// ---- the token, for a browser ----

const LOGIN_PAGE: &str = "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><title>Runbook: token</title></head>\n<body><h1>Runbook</h1>\n<p>This daemon needs its token.</p>\n<form method=\"post\" action=\"/ui/login\"><label>Token <input type=\"password\" name=\"token\" autocomplete=\"off\" autofocus></label> <button type=\"submit\">Open</button></form>\n</body></html>\n";

fn login_page(status: StatusCode, note: &str) -> Response {
    let body = if note.is_empty() { LOGIN_PAGE.to_string() } else { LOGIN_PAGE.replace("<p>This daemon needs its token.</p>", &format!("<p>{note}</p>")) };
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    let h = response.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'none'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'"));
    h.insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
    response
}

/// The page load of a browser that holds no token is answered 401 with a form for it; every other
/// 401 stays the JSON refusal of the security floor.
pub async fn login_form(request: Request, next: Next) -> Response {
    let browser = request.method() == Method::GET
        && request.uri().path() == "/"
        && request.headers().get_all(header::ACCEPT).iter().filter_map(|v| v.to_str().ok()).any(|v| v.contains("text/html"));
    let response = next.run(request).await;
    if browser && response.status() == StatusCode::UNAUTHORIZED {
        return login_page(StatusCode::UNAUTHORIZED, "");
    }
    response
}

fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' => {
                out.push(u8::from_str_radix(s.get(i + 1..i + 3)?, 16).ok()?);
                i += 2;
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8(out).ok()
}

const MAX_LOGIN_BYTES: usize = 4096;

/// `POST /ui/login`: the token in the body of a form, checked by `Auth::admits` like a bearer token
/// on any other route. The answer is a redirect to `/` with the cookie, or 401 with the form again.
pub async fn login(State(auth): State<Arc<Auth>>, request: Request) -> Response {
    let Ok(body) = axum::body::to_bytes(request.into_body(), MAX_LOGIN_BYTES).await else {
        return refusal(StatusCode::PAYLOAD_TOO_LARGE, "request body too large", "the form is small");
    };
    let text = String::from_utf8_lossy(&body);
    let token = text.split('&').filter_map(|kv| kv.split_once('=')).find(|(k, _)| *k == "token").and_then(|(_, v)| percent_decode(v)).unwrap_or_default();
    if !auth.admits(Some(&format!("Bearer {token}"))) {
        return login_page(StatusCode::UNAUTHORIZED, "That token was refused.");
    }
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::SEE_OTHER;
    let h = response.headers_mut();
    h.insert(header::LOCATION, HeaderValue::from_static("/"));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if let Some(value) = auth.cookie() {
        if let Ok(v) = HeaderValue::from_str(&format!("{COOKIE}={value}; HttpOnly; SameSite=Strict; Path=/")) {
            h.insert(header::SET_COOKIE, v);
        }
    }
    response
}
