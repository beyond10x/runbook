//! A small blocking HTTP/1.1 client over `std::net::TcpStream` for a loopback or plain-HTTP daemon.
//! One request per connection (`Connection: close`). The bearer token is added here and nowhere else;
//! every text that leaves this module has the token replaced, so no error, body or JSON can carry it.

use super::exit::{self, Failure};
use super::url::Target;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REPLY_TIMEOUT: Duration = Duration::from_secs(30);
/// Three keep-alive intervals of the daemon (15 s): a stream silent for longer is taken as dropped.
const STREAM_TIMEOUT: Duration = Duration::from_secs(45);
const MAX_HEAD_BYTES: u64 = 64 * 1024;
const MAX_REPLY_BYTES: u64 = 32 * 1024 * 1024;

/// A bearer token. It has no `Display`, and its `Debug` shows nothing.
#[derive(Clone)]
pub struct Token(String);

impl Token {
    pub fn new(value: String) -> Token {
        Token(value)
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(..)")
    }
}

#[derive(Debug, Clone)]
pub struct Client {
    target: Target,
    token: Option<Token>,
    reply_timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Reply {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// The body of a response that is being read: a stream of events.
pub struct Body {
    reader: BufReader<Box<dyn Read>>,
    token: Option<Token>,
}

impl Body {
    /// One line without its end of line; `None` at the clean end of the body; an error when the
    /// connection broke, ended inside a chunk, or the line is over `limit` bytes.
    pub fn line(&mut self, limit: u64) -> io::Result<Option<String>> {
        let mut raw = Vec::new();
        let n = (&mut self.reader).take(limit + 1).read_until(b'\n', &mut raw)?;
        if n == 0 {
            return Ok(None);
        }
        if raw.last() != Some(&b'\n') {
            return if raw.len() as u64 > limit {
                Err(io::Error::new(io::ErrorKind::InvalidData, "a line of the stream is longer than the client reads"))
            } else {
                Ok(None)
            };
        }
        raw.pop();
        if raw.last() == Some(&b'\r') {
            raw.pop();
        }
        Ok(Some(scrub(&String::from_utf8_lossy(&raw), self.token.as_ref())))
    }
}

impl Read for Body {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reader.read(buf)
    }
}

fn scrub(text: &str, token: Option<&Token>) -> String {
    match token {
        Some(Token(t)) if !t.is_empty() => text.replace(t.as_str(), "[token]"),
        _ => text.to_string(),
    }
}

fn unreachable(target: &Target, why: &str) -> Failure {
    Failure::new(exit::UNREACHABLE, format!("cannot reach the daemon at {}:{}: {why}", target.host, target.port))
}

fn why(e: &io::Error) -> String {
    match e.kind() {
        io::ErrorKind::ConnectionRefused => "connection refused".to_string(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => "timed out".to_string(),
        _ => e.to_string(),
    }
}

impl Client {
    pub fn new(target: Target, token: Option<Token>) -> Client {
        Client { target, token, reply_timeout: REPLY_TIMEOUT }
    }

    /// The client with `timeout` as the most a read or a write of [`Client::request`] waits (the daemon's own
    /// default is 30 s).
    pub fn with_reply_timeout(mut self, timeout: Duration) -> Client {
        self.reply_timeout = timeout;
        self
    }

    pub fn target(&self) -> &Target {
        &self.target
    }

    pub fn has_token(&self) -> bool {
        self.token.is_some()
    }

    fn connect(&self, read_timeout: Duration) -> Result<TcpStream, Failure> {
        let addrs = (self.target.host.as_str(), self.target.port).to_socket_addrs().map_err(|e| unreachable(&self.target, &why(&e)))?;
        let mut last = None;
        for addr in addrs {
            match TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) {
                Ok(s) => {
                    s.set_read_timeout(Some(read_timeout)).ok();
                    s.set_write_timeout(Some(self.reply_timeout)).ok();
                    s.set_nodelay(true).ok();
                    return Ok(s);
                }
                Err(e) => last = Some(e),
            }
        }
        Err(unreachable(&self.target, &last.map_or("no address".to_string(), |e| why(&e))))
    }

    fn send(&self, s: &mut TcpStream, method: &str, path: &str, headers: &[(&str, String)], body: Option<&[u8]>) -> Result<(), Failure> {
        let mut head = format!("{method} {path} HTTP/1.1\r\nHost: {}\r\nUser-Agent: runbook/{}\r\nConnection: close\r\n", self.target.authority(), env!("CARGO_PKG_VERSION"));
        if let Some(Token(t)) = &self.token {
            head.push_str(&format!("Authorization: Bearer {t}\r\n"));
        }
        for (k, v) in headers {
            head.push_str(&format!("{k}: {v}\r\n"));
        }
        if let Some(b) = body {
            head.push_str(&format!("Content-Type: application/json\r\nContent-Length: {}\r\n", b.len()));
        }
        head.push_str("\r\n");
        let sent = s.write_all(head.as_bytes()).and_then(|()| body.map_or(Ok(()), |b| s.write_all(b))).and_then(|()| s.flush());
        sent.map_err(|e| unreachable(&self.target, &why(&e)))
    }

    /// The status, the headers and the body of the answer, whole.
    pub fn request(&self, method: &str, path: &str, body: Option<&[u8]>) -> Result<Reply, Failure> {
        let mut s = self.connect(self.reply_timeout)?;
        self.send(&mut s, method, path, &[("Accept", "application/json".to_string())], body)?;
        let (status, headers, mut body) = self.read_response(s)?;
        let mut text = Vec::new();
        (&mut body).take(MAX_REPLY_BYTES + 1).read_to_end(&mut text).map_err(|e| unreachable(&self.target, &why(&e)))?;
        if text.len() as u64 > MAX_REPLY_BYTES {
            return Err(Failure::new(exit::OTHER, "the answer of the daemon is larger than the client reads"));
        }
        Ok(Reply { status, headers, body: scrub(&String::from_utf8_lossy(&text), self.token.as_ref()) })
    }

    /// `GET path` as an event stream, resumed after `last_event_id` when given. An answer that is not
    /// 200 is returned whole as `Err(Ok(reply))` so the caller can map it.
    pub fn stream(&self, path: &str, last_event_id: Option<u64>) -> Result<Result<Body, Reply>, Failure> {
        let mut s = self.connect(STREAM_TIMEOUT)?;
        let mut headers = vec![("Accept", "text/event-stream".to_string())];
        if let Some(id) = last_event_id {
            headers.push(("Last-Event-ID", id.to_string()));
        }
        self.send(&mut s, "GET", path, &headers, None)?;
        let (status, response_headers, mut body) = self.read_response(s)?;
        if status == 200 {
            return Ok(Ok(body));
        }
        let mut text = Vec::new();
        let _ = (&mut body.reader).take(MAX_REPLY_BYTES).read_to_end(&mut text);
        Ok(Err(Reply { status, headers: response_headers, body: scrub(&String::from_utf8_lossy(&text), self.token.as_ref()) }))
    }

    fn read_response(&self, s: TcpStream) -> Result<Response, Failure> {
        let mut r = BufReader::new(s);
        let broken = |e: io::Error| unreachable(&self.target, &why(&e));
        let mut head = Vec::new();
        loop {
            let mut line = Vec::new();
            let n = (&mut r).take(MAX_HEAD_BYTES).read_until(b'\n', &mut line).map_err(broken)?;
            if n == 0 {
                return Err(unreachable(&self.target, "the connection closed before an answer"));
            }
            let text = String::from_utf8_lossy(&line).trim_end_matches(['\r', '\n']).to_string();
            if text.is_empty() {
                break;
            }
            head.push(text);
            if head.iter().map(String::len).sum::<usize>() as u64 > MAX_HEAD_BYTES {
                return Err(Failure::new(exit::OTHER, "the head of the answer is too long"));
            }
        }
        let status = head
            .first()
            .and_then(|l| l.strip_prefix("HTTP/1."))
            .and_then(|l| l.split(' ').nth(1))
            .and_then(|c| c.parse::<u16>().ok())
            .ok_or_else(|| Failure::new(exit::OTHER, "the answer is not HTTP/1.x"))?;
        let headers: Vec<(String, String)> = head.iter().skip(1).filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
        let find = |name: &str| headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str());
        let reader: Box<dyn Read> = if find("transfer-encoding").is_some_and(|v| v.to_ascii_lowercase().contains("chunked")) {
            Box::new(Chunked { r, left: 0, done: false })
        } else if let Some(n) = find("content-length").and_then(|v| v.parse::<u64>().ok()) {
            Box::new(Exact { r, left: n })
        } else {
            Box::new(r)
        };
        Ok((status, headers, Body { reader: BufReader::new(reader), token: self.token.clone() }))
    }
}

/// The status, the headers and the body of an answer that is being read.
type Response = (u16, Vec<(String, String)>, Body);

/// A body with a `Content-Length`: ending early is an error, not an end.
struct Exact<R> {
    r: R,
    left: u64,
}

impl<R: Read> Read for Exact<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.left == 0 || buf.is_empty() {
            return Ok(0);
        }
        let n = buf.len().min(self.left as usize);
        let got = self.r.read(&mut buf[..n])?;
        if got == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        self.left -= got as u64;
        Ok(got)
    }
}

/// A chunked body. Its end is the zero chunk and nothing else: a connection that ends anywhere else
/// is an `UnexpectedEof`, which is how the client tells a dropped stream from a closed one.
struct Chunked<R> {
    r: R,
    left: u64,
    done: bool,
}

impl<R: BufRead> Chunked<R> {
    fn line(&mut self) -> io::Result<String> {
        let mut raw = Vec::new();
        let n = (&mut self.r).take(1024).read_until(b'\n', &mut raw)?;
        if n == 0 || raw.last() != Some(&b'\n') {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        Ok(String::from_utf8_lossy(&raw).trim_end_matches(['\r', '\n']).to_string())
    }
}

impl<R: BufRead> Read for Chunked<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.done || buf.is_empty() {
            return Ok(0);
        }
        if self.left == 0 {
            let size_line = self.line()?;
            let size = size_line.split(';').next().unwrap_or("").trim();
            let size = u64::from_str_radix(size, 16).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "a chunk size is not hexadecimal"))?;
            if size == 0 {
                while !self.line()?.is_empty() {}
                self.done = true;
                return Ok(0);
            }
            self.left = size;
        }
        let n = buf.len().min(self.left as usize);
        let got = self.r.read(&mut buf[..n])?;
        if got == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        self.left -= got as u64;
        if self.left == 0 {
            self.line()?;
        }
        Ok(got)
    }
}
