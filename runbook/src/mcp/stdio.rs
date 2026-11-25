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

/// The longest message read: the daemon's own body limit.
const MAX_LINE: usize = crate::api::MAX_BODY_BYTES;

#[derive(Args, Debug, Clone)]
pub struct McpArgs {
    /// The daemon to talk to, `http://host[:port]` (or RUNBOOK_DAEMON). Required.
    #[arg(long, value_name = "URL")]
    pub daemon: Option<String>,
    /// A file holding the daemon's bearer token; its mode must not give anything to others. Also read from RUNBOOK_TOKEN.
    #[arg(long, value_name = "FILE")]
    pub token_file: Option<PathBuf>,
}

/// Serve the messages of `input` to `output` until the input ends.
pub fn serve(be: &dyn Backend, mut input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
    let mut legacy = false;
    let mut line = Vec::new();
    loop {
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
            write_line(&mut output, &serde_json::json!({"jsonrpc": "2.0", "id": null, "error": {"code": rpc::INVALID_REQUEST, "message": format!("a message is at most {MAX_LINE} bytes")}}))?;
            continue;
        }
        let text = line.trim_ascii();
        if text.is_empty() {
            continue;
        }
        // A notification a call sends while it runs (the progress of `delegate`) goes out at once, before the reply; one that
        // cannot be written ends the server as the reply would.
        let mut broken = None;
        let reply = rpc::handle_with(be, text, Wire::Stdio { legacy: &mut legacy }, &mut |note| {
            if broken.is_none() {
                broken = write_line(&mut output, &note).err();
            }
        });
        if let Some(e) = broken {
            return Err(e);
        }
        if let Some(message) = reply.message {
            write_line(&mut output, &message)?;
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
    match serve(&remote, std::io::stdin().lock(), std::io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("runbook mcp: {e}");
            ExitCode::from(1)
        }
    }
}
