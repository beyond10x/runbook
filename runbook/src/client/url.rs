//! The daemon's URL: `http://host[:port]` only. The client speaks plain HTTP (the crate holds no TLS
//! client), so `https://` is refused; a URL with user info is refused without echoing it.

use super::exit::Failure;
use std::net::IpAddr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub host: String,
    pub port: u16,
    pub loopback: bool,
}

impl Target {
    /// The value of the `Host` header.
    pub fn authority(&self) -> String {
        let host = if self.host.contains(':') { format!("[{}]", self.host) } else { self.host.clone() };
        format!("{host}:{}", self.port)
    }
}

pub fn parse(url: &str) -> Result<Target, Failure> {
    let lower = url.to_ascii_lowercase();
    let rest = if lower.starts_with("http://") {
        &url["http://".len()..]
    } else if lower.starts_with("https://") {
        return Err(Failure::usage(
            "--daemon: https is not supported, this client speaks plain http; use an http:// URL on loopback (a tunnel such as ssh -L reaches a remote daemon)",
        ));
    } else {
        return Err(Failure::usage("--daemon: the URL must start with http://, as http://127.0.0.1:7430"));
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    if authority.contains('@') {
        return Err(Failure::usage("--daemon: the URL has user info in it; a token comes from --token-file, never from the URL"));
    }
    if !(tail.is_empty() || tail == "/") {
        return Err(Failure::usage("--daemon: the URL names a path, query or fragment; the daemon is at its root: http://host:port"));
    }
    let (host, port) = split_host_port(authority)?;
    if host.is_empty() {
        return Err(Failure::usage("--daemon: the URL has no host"));
    }
    let valid = host.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':'));
    if !valid {
        return Err(Failure::usage("--daemon: the host holds a character a host name does not"));
    }
    let loopback = host.eq_ignore_ascii_case("localhost") || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback());
    Ok(Target { host, port, loopback })
}

fn split_host_port(authority: &str) -> Result<(String, u16), Failure> {
    let bad_port = || Failure::usage("--daemon: the port is not a number from 1 to 65535");
    let (host, port) = if let Some(inner) = authority.strip_prefix('[') {
        let (host, after) = inner.split_once(']').ok_or_else(|| Failure::usage("--daemon: an IPv6 host needs its closing ]"))?;
        match after {
            "" => (host, None),
            _ => (host, Some(after.strip_prefix(':').ok_or_else(|| Failure::usage("--daemon: text after the ] of the host"))?)),
        }
    } else {
        match authority.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        }
    };
    let port = match port {
        None => 80,
        Some(p) => p.parse::<u16>().ok().filter(|p| *p != 0).ok_or_else(bad_port)?,
    };
    Ok((host.to_string(), port))
}
