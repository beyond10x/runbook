//! A daemon named by URL, as a `Backend`: each call is the HTTP request of the route it stands for
//! (`POST /daemon/commands/<name>`, `GET /daemon/views/<name>?<query>`), sent with the bearer token
//! when there is one, over plain HTTP/1.1 on a fresh connection. The answer is the route's own, so a
//! tool result over stdio is byte for byte the body the route gave.
//!
//! Only `http://`: there is no TLS client in this binary. A token sent to a host that is not
//! loopback travels in clear; `Remote::parse` reports that to the caller (`clear_token`).

use super::{refused, Backend};
use crate::api::routes::{self, Answer};
use axum::http::StatusCode;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(30);
/// The most of an answer that is read.
const MAX_ANSWER: u64 = 64 * 1024 * 1024;

pub struct Remote {
    authority: String,
    token: Option<String>,
    /// The session's label (`RUNBOOK_CALLER`), sent with a delegation.
    label: Option<String>,
    /// The MCP client's name, once `initialize` gave it.
    client: std::sync::Mutex<Option<String>>,
}

impl Remote {
    /// `http://host[:port]` with nothing after it but an optional `/`. The token is never part of an error.
    pub fn parse(url: &str, token: Option<String>) -> Result<Remote, String> {
        let rest = url.strip_prefix("http://").ok_or_else(|| format!("daemon URL {url:?}: only http:// is supported (a daemon on this machine or a tunnel to one)"))?;
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        if authority.is_empty() || authority.contains('@') || !path.is_empty() {
            return Err(format!("daemon URL {url:?}: give http://host[:port], with no user, no path and no query"));
        }
        let authority = if authority.contains(':') && !authority.ends_with(']') { authority.to_string() } else { format!("{authority}:80") };
        Ok(Remote { authority, token, label: None, client: std::sync::Mutex::new(None) })
    }

    /// The session's label this server sends with a delegation (`RUNBOOK_CALLER`).
    pub fn with_label(mut self, label: Option<String>) -> Remote {
        self.label = label;
        self
    }

    /// Whether a token goes to a host that is not loopback, over a connection that is not encrypted.
    pub fn clear_token(&self) -> bool {
        let host = self.authority.rsplit_once(':').map_or(self.authority.as_str(), |(h, _)| h).trim_matches(['[', ']']);
        self.token.is_some() && !(host == "localhost" || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback()))
    }

    fn send(&self, method: &str, target: &str, body: Option<&[u8]>) -> Answer {
        match self.exchange(method, target, body) {
            Ok(a) => a,
            Err(e) => refused(StatusCode::BAD_GATEWAY, "daemon unreachable", &format!("{} did not answer {method} {target}: {e}", self.authority)),
        }
    }

    fn exchange(&self, method: &str, target: &str, body: Option<&[u8]>) -> Result<Answer, String> {
        let addr = self.authority.to_socket_addrs().map_err(|e| e.to_string())?.next().ok_or("no address")?;
        let mut s = TcpStream::connect_timeout(&addr, TIMEOUT).map_err(|e| e.to_string())?;
        s.set_read_timeout(Some(TIMEOUT)).and_then(|()| s.set_write_timeout(Some(TIMEOUT))).map_err(|e| e.to_string())?;
        let mut head = format!("{method} {target} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nAccept: application/json\r\n", self.authority);
        if let Some(t) = &self.token {
            head.push_str(&format!("Authorization: Bearer {t}\r\n"));
        }
        if let Some(b) = body {
            head.push_str(&format!("Content-Type: application/json\r\nContent-Length: {}\r\n", b.len()));
        }
        head.push_str("\r\n");
        s.write_all(head.as_bytes()).and_then(|()| body.map_or(Ok(()), |b| s.write_all(b))).map_err(|e| e.to_string())?;
        let mut raw = Vec::new();
        s.take(MAX_ANSWER).read_to_end(&mut raw).map_err(|e| e.to_string())?;
        parse_answer(&raw)
    }
}

fn parse_answer(raw: &[u8]) -> Result<Answer, String> {
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or("the answer has no head")?;
    let head = String::from_utf8_lossy(&raw[..split]);
    let mut lines = head.lines();
    let status: u16 = lines.next().and_then(|l| l.split(' ').nth(1)).and_then(|s| s.parse().ok()).ok_or("the answer has no status")?;
    let headers: Vec<(String, String)> = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string())).collect();
    let header = |name: &str| headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str());
    let mut body = raw[split + 4..].to_vec();
    if header("transfer-encoding").is_some_and(|v| v.contains("chunked")) {
        body = dechunk(&body).ok_or("the chunked answer is cut short")?;
    }
    let status = StatusCode::from_u16(status).map_err(|e| e.to_string())?;
    let location = header("location").map(str::to_string);
    let body = serde_json::from_slice::<Value>(&body)
        .unwrap_or_else(|_| serde_json::json!({"refused": "not a daemon answer", "message": format!("status {} with a body that is not JSON", status.as_u16())}));
    Ok(Answer { status, body, location })
}

fn dechunk(mut rest: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let eol = rest.windows(2).position(|w| w == b"\r\n")?;
        let n = usize::from_str_radix(std::str::from_utf8(&rest[..eol]).ok()?.split(';').next()?.trim(), 16).ok()?;
        if n == 0 {
            return Some(out);
        }
        out.extend_from_slice(rest.get(eol + 2..eol + 2 + n)?);
        rest = rest.get(eol + 2 + n + 2..)?;
    }
}

fn encode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

impl Backend for Remote {
    fn caller(&self) -> (Option<String>, Option<String>) {
        (self.client.lock().unwrap_or_else(|e| e.into_inner()).clone(), self.label.clone())
    }

    fn client_named(&self, name: &str) {
        *self.client.lock().unwrap_or_else(|e| e.into_inner()) = Some(name.to_owned());
    }

    fn command(&self, name: &str, body: &[u8]) -> Answer {
        self.send("POST", &routes::command_path(name), Some(body))
    }

    fn view(&self, name: &str, query: &HashMap<String, String>) -> Answer {
        let mut pairs: Vec<_> = query.iter().collect();
        pairs.sort();
        let q: Vec<String> = pairs.iter().map(|(k, v)| format!("{}={}", encode(k), encode(v))).collect();
        let target = if q.is_empty() { routes::view_path(name) } else { format!("{}?{}", routes::view_path(name), q.join("&")) };
        self.send("GET", &target, None)
    }
}
