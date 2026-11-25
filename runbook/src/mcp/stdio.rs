//! `runbook mcp`: MCP over stdio, newline-delimited JSON-RPC 2.0, as a client of a daemon.
//!
//! The server reads standard input only and writes only MCP messages to standard output; what a person
//! should read (a refusal to start, a warning) goes to standard error. The daemon is named by
//! `--daemon <url>` or `RUNBOOK_DAEMON`; with none named the command refuses to start, because one
//! daemon owns the state of the runs and a second process on its directory would not be that daemon.
//! The token is read from `--token-file` or `RUNBOOK_TOKEN`, never from the command line.

use super::remote::Remote;
use super::rpc::{self, Wire};
use super::Backend;
use crate::api::auth;
use clap::Args;
use std::io::{BufRead, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// The longest message read: the daemon's own body limit.
const MAX_LINE: usize = crate::api::MAX_BODY_BYTES;
/// The most `tools/call` requests answered side by side; one more is answered in the read loop, which then reads nothing
/// until it is answered.
pub const MAX_CALLS: usize = 16;

#[derive(Args, Debug, Clone)]
pub struct McpArgs {
    /// The daemon to talk to, `http://host[:port]` (or RUNBOOK_DAEMON). Required.
    #[arg(long, value_name = "URL")]
    pub daemon: Option<String>,
    /// A file holding the daemon's bearer token; its mode must not give anything to others. Also read from RUNBOOK_TOKEN.
    #[arg(long, value_name = "FILE")]
    pub token_file: Option<PathBuf>,
}

/// Serve the messages of `input` to `output` until the input ends, and every call taken has been answered. A `tools/call`
/// is answered on a thread of its own (`delegate` waits for its run's end), so the calls of one session run side by side;
/// the notifications of a call go out before its reply, each message whole.
pub fn serve(be: &(dyn Backend + Sync), input: impl BufRead, output: impl Write + Send) -> std::io::Result<()> {
    let output = Mutex::new(output);
    let broken: Mutex<Option<std::io::Error>> = Mutex::new(None);
    let calls = AtomicUsize::new(0);
    let read = std::thread::scope(|scope| read_loop(be, input, &output, &broken, &calls, scope));
    let failed = lock(&broken).take();
    match failed {
        Some(e) => Err(e),
        None => read,
    }
}

/// Write `message` unless a write failed before; a failure is kept for `serve` to return.
fn send(output: &Mutex<impl Write>, broken: &Mutex<Option<std::io::Error>>, message: &serde_json::Value) {
    let mut failed = lock(broken);
    if failed.is_none() {
        *failed = write_line(&mut *lock(output), message).err();
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Whether a message is one request of `tools/call` (a batch or anything else is answered in the read loop).
fn is_call(text: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(text).is_ok_and(|m| m["method"] == "tools/call" && m.get("id").is_some())
}

fn read_loop<'scope, 'env, W: Write + Send>(
    be: &'env (dyn Backend + Sync),
    mut input: impl BufRead,
    output: &'env Mutex<W>,
    broken: &'env Mutex<Option<std::io::Error>>,
    calls: &'env AtomicUsize,
    scope: &'scope std::thread::Scope<'scope, 'env>,
) -> std::io::Result<()> {
    let mut legacy = false;
    let mut line = Vec::new();
    loop {
        if lock(broken).is_some() {
            return Ok(());
        }
        line.clear();
        let n = (&mut input).take(MAX_LINE as u64 + 1).read_until(b'\n', &mut line)?;
        if n == 0 {
            return Ok(());
        }
        let complete = line.ends_with(b"\n");
        if !complete && line.len() > MAX_LINE {
            // Too long: drop the rest of the line and say so.
            let mut sink = Vec::new();
            while !sink.ends_with(b"\n") {
                sink.clear();
                if (&mut input).take(1 << 16).read_until(b'\n', &mut sink)? == 0 {
                    break;
                }
            }
            send(output, broken, &serde_json::json!({"jsonrpc": "2.0", "id": null, "error": {"code": rpc::INVALID_REQUEST, "message": format!("a message is at most {MAX_LINE} bytes")}}));
            continue;
        }
        let text = line.trim_ascii();
        if text.is_empty() {
            continue;
        }
        // A notification a call sends while it runs (the progress of `delegate`) goes out at once, before the reply; one that
        // cannot be written ends the server as the reply would. A call reads the handshake's era and never changes it.
        let call = is_call(text);
        if call && calls.fetch_add(1, Ordering::SeqCst) < MAX_CALLS {
            let (text, mut era) = (text.to_vec(), legacy);
            scope.spawn(move || {
                let reply = rpc::handle_with(be, &text, Wire::Stdio { legacy: &mut era }, &mut |note| send(output, broken, &note));
                if let Some(message) = reply.message {
                    send(output, broken, &message);
                }
                calls.fetch_sub(1, Ordering::SeqCst);
            });
            continue;
        }
        if call {
            calls.fetch_sub(1, Ordering::SeqCst);
        }
        let reply = rpc::handle_with(be, text, Wire::Stdio { legacy: &mut legacy }, &mut |note| send(output, broken, &note));
        if let Some(message) = reply.message {
            send(output, broken, &message);
        }
    }
}

fn write_line(output: &mut impl Write, message: &serde_json::Value) -> std::io::Result<()> {
    // `to_string` has no newline in it: a string's newline is escaped.
    output.write_all(message.to_string().as_bytes())?;
    output.write_all(b"\n")?;
    output.flush()
}

pub fn run(args: McpArgs) -> ExitCode {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let Some(url) = args.daemon.or_else(|| env("RUNBOOK_DAEMON")) else {
        eprintln!("runbook mcp: no daemon named: give --daemon <url> or set RUNBOOK_DAEMON (start one with `runbook serve`)");
        return ExitCode::from(1);
    };
    let remote = auth::load_token(args.token_file.as_deref(), env("RUNBOOK_TOKEN")).and_then(|token| Remote::parse(&url, token));
    let remote = match remote {
        Ok(r) => r,
        Err(e) => {
            eprintln!("runbook mcp: {e}");
            return ExitCode::from(1);
        }
    };
    if remote.clear_token() {
        eprintln!("runbook mcp: the token goes to {url} over plain HTTP, and that host is not this machine");
    }
    match serve(&remote, std::io::stdin().lock(), std::io::stdout()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("runbook mcp: {e}");
            ExitCode::from(1)
        }
    }
}
