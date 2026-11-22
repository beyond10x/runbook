//! `runbook runs events`: the run's event stream. The text is the lines `curl -N` of the route gives
//! (`id:`, `event:`, `data:` and a blank line per event; the `: keep-alive` comments a client ignores
//! are left out). A dropped connection is resumed with `Last-Event-ID`, and an event whose position was
//! printed is never printed again.

use super::api::{view_path, Api};
use super::exit::{self, Failure};
use super::http::Body;
use serde_json::Value;
use std::io::{self, Write};
use std::time::Duration;

/// The longest line the client reads from a stream (an event's `data:` is one line).
const MAX_LINE: u64 = 16 * 1024 * 1024;
/// Failed attempts in a row (no connection, or a connection that gave no event) before the follow gives up.
const MAX_ATTEMPTS: u32 = 5;
const RUN_VIEW: &str = "run";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Event {
    pub id: Option<u64>,
    /// The field lines as received, comments left out.
    pub lines: Vec<String>,
    pub data: String,
}

impl Event {
    /// The lines of the event as `curl` shows them, each ending in a newline, then one blank line.
    pub fn text(&self) -> String {
        let mut out: String = self.lines.iter().map(|l| format!("{l}\n")).collect();
        out.push('\n');
        out
    }
}

/// How the stream of one connection ended.
#[derive(Debug)]
pub enum End {
    /// The daemon closed it after its last chunk.
    Closed,
    /// The connection broke, timed out or sent what is not a stream.
    Dropped(String),
}

/// The next event of a body, or how it ended.
pub fn next_event(body: &mut Body) -> Result<Event, End> {
    let mut event = Event::default();
    loop {
        let line = match body.line(MAX_LINE) {
            Ok(Some(l)) => l,
            Ok(None) => return Err(End::Closed),
            Err(e) => return Err(End::Dropped(e.to_string())),
        };
        if line.is_empty() {
            if event.lines.is_empty() {
                continue;
            }
            return Ok(event);
        }
        if line.starts_with(':') {
            continue;
        }
        let (field, value) = line.split_once(':').map_or((line.as_str(), ""), |(f, v)| (f, v.strip_prefix(' ').unwrap_or(v)));
        match field {
            "id" => event.id = value.bytes().all(|b| b.is_ascii_digit()).then(|| value.parse().ok()).flatten(),
            "data" => {
                if !event.data.is_empty() {
                    event.data.push('\n');
                }
                event.data.push_str(value);
            }
            _ => {}
        }
        event.lines.push(line);
    }
}

/// An event as the page of the `events` view holds it, written as the stream writes it.
pub fn row_text(row: &Value) -> String {
    let position = row.get("position").map_or(String::new(), |p| p.to_string());
    let kind = row.get("kind").and_then(Value::as_str).unwrap_or("").replace(['\r', '\n'], " ");
    format!("id: {position}\nevent: {kind}\ndata: {row}\n\n")
}

fn finish(e: io::Error) -> Failure {
    // A closed pipe (`| head`) ends the command quietly.
    if e.kind() == io::ErrorKind::BrokenPipe {
        Failure::new(0, "")
    } else {
        Failure::new(exit::OTHER, format!("cannot write to standard output: {e}"))
    }
}

fn is_terminal(run_row: &Value) -> bool {
    matches!(run_row.get("state").and_then(Value::as_str), Some("Ended" | "Cancelled" | "FailedToStart"))
}

/// Follow the run's stream to its end, writing each event once.
pub fn follow(api: &Api, run: &str, json: bool, out: &mut dyn Write, backoff: Duration) -> Result<(), Failure> {
    let path = view_path("events", &[("run_id", run)]);
    let mut last: Option<u64> = None;
    let mut attempts = 0u32;
    loop {
        let progressed = last;
        let connection = api.client.stream(&path, last);
        let end = match connection {
            Err(f) => {
                attempts += 1;
                if attempts >= MAX_ATTEMPTS {
                    return Err(f);
                }
                std::thread::sleep(backoff * attempts);
                continue;
            }
            Ok(Err(reply)) => return Err(api.failure(&reply)),
            Ok(Ok(mut body)) => loop {
                match next_event(&mut body) {
                    Ok(event) => {
                        if let (Some(id), Some(seen)) = (event.id, last) {
                            if id <= seen {
                                continue;
                            }
                        }
                        let written = if json {
                            writeln!(out, "{}", event.data)
                        } else {
                            write!(out, "{}", event.text())
                        };
                        written.and_then(|()| out.flush()).map_err(finish)?;
                        if event.id.is_some() {
                            last = event.id;
                        }
                    }
                    Err(end) => break end,
                }
            },
        };
        if last != progressed {
            attempts = 0;
        }
        if matches!(end, End::Closed) {
            // The daemon closes a run's stream when the run is over and its last event is sent; it also
            // closes one that fell behind or at shutdown, so the run says whether this is the end.
            let rows = Api::rows(&api.view(RUN_VIEW, &[("run_id", run)])?)?;
            match rows.first() {
                Some(row) if is_terminal(row) => return Ok(()),
                Some(_) => {}
                None => return Err(Failure::new(exit::NOT_FOUND, format!("no such run: {run}"))),
            }
        }
        if last == progressed {
            attempts += 1;
            if attempts >= MAX_ATTEMPTS {
                let why = match end {
                    End::Dropped(why) => why,
                    End::Closed => "the daemon closed the stream of a run that has not ended".to_string(),
                };
                return Err(Failure::new(exit::UNREACHABLE, format!("the event stream of run {run} gave no event in {MAX_ATTEMPTS} attempts: {why}")));
            }
        }
        if last == progressed {
            std::thread::sleep(backoff * attempts);
        }
    }
}
