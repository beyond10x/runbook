//! Event streams: the three event views asked with `Accept: text/event-stream`.
//!
//! A stream is the view's page read again and again from the daemon's state: it holds a position, reads
//! the events after it from the state (never from a queue), sends them, and waits on the state's
//! version (`State::subscribe`) when there is nothing more. A notification that is missed or comes
//! twice changes nothing: the next read finds what there is.
//!
//! Recording an event never waits for a stream: it bumps a version and returns. A stream is pulled by
//! its connection, so a client that reads slowly holds back its own stream and nothing else. A live
//! stream that finds more than `Streams::max_lag` events waiting for it is ended, and the client resumes
//! by position.

use super::{refusal, Streams};
use crate::commands::{event_rows, Daemon};
use crate::state::{RunEvent, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures_util::stream;
use std::collections::{HashMap, VecDeque};
use std::convert::Infallible;
use tokio::sync::watch;

/// Events read from the state in one go while a stream replays what was recorded before it connected.
const REPLAY_BATCH: usize = 256;

#[derive(Clone)]
pub enum Source {
    Run(String),
    Session(String),
    Daemon,
}

impl Source {
    fn filter(&self) -> (Option<&str>, Option<&str>) {
        match self {
            Source::Run(r) => (Some(r), None),
            Source::Session(s) => (None, Some(s)),
            Source::Daemon => (None, None),
        }
    }

    /// Whether nothing more can come: the run is terminal, the session is closed with every run
    /// terminal, or the id is unknown (as the page form, which holds no row for it). The daemon's own
    /// stream ends at shutdown only.
    fn finished(&self, st: &State) -> bool {
        match self {
            Source::Run(id) => st.run(id).is_none_or(|r| !r.state.is_live()),
            Source::Session(id) => match st.session(id) {
                None => true,
                Some(s) => s.state == crate::state::SessionState::Closed && st.runs().iter().all(|r| r.session_id.as_deref() != Some(id.as_str()) || !r.state.is_live()),
            },
            Source::Daemon => false,
        }
    }
}

/// A position as a client writes it: digits only, and one a 64-bit number holds.
fn parse_position(raw: &str) -> Option<u64> {
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    raw.parse().ok()
}

fn bad(message: String) -> Response {
    refusal(StatusCode::BAD_REQUEST, "bad request", &message)
}

/// Where a stream starts: the `Last-Event-ID` header when it is given, else `after_position`, else 0.
fn start_after(query: &HashMap<String, String>, headers: &HeaderMap) -> Result<u64, String> {
    let mut ids = headers.get_all("last-event-id").iter();
    if let Some(first) = ids.next() {
        let parsed = (ids.next().is_none()).then(|| first.to_str().ok().and_then(parse_position)).flatten();
        return parsed.ok_or_else(|| "Last-Event-ID must be a position, an integer of 0 or more, as the id of an event; it is given once".to_string());
    }
    match query.get("after_position") {
        None => Ok(0),
        Some(raw) => parse_position(raw).ok_or_else(|| format!("after_position must be an integer of 0 or more, not {raw:?}")),
    }
}

fn required<'a>(query: &'a HashMap<String, String>, key: &str) -> Result<&'a str, String> {
    query.get(key).map(String::as_str).ok_or_else(|| format!("the query parameter {key} is required"))
}

/// The answer to an event view asked as a stream, or the refusal the page form gives.
pub fn answer(view: &str, daemon: &Daemon, query: &HashMap<String, String>, headers: &HeaderMap, streams: &Streams, closing: watch::Receiver<bool>) -> Response {
    let source = match view {
        "events" => required(query, "run_id").map(|r| Source::Run(r.to_string())),
        "session-events" => required(query, "session_id").map(|s| Source::Session(s.to_string())),
        _ => Ok(Source::Daemon),
    };
    let source = match source {
        Ok(s) => s,
        Err(m) => return bad(m),
    };
    let after = match start_after(query, headers) {
        Ok(a) => a,
        Err(m) => return bad(m),
    };
    let feed = Feed::new(daemon, source, after, streams.max_lag, closing);
    let body = stream::unfold(feed, |mut feed| async move { feed.next().await.map(|event| (Ok::<Event, Infallible>(event), feed)) });
    Sse::new(body).keep_alive(KeepAlive::new().interval(streams.keep_alive).text("keep-alive")).into_response()
}

enum Step {
    Events,
    Lagged,
    Wait,
    End,
}

pub struct Feed {
    daemon: Daemon,
    source: Source,
    /// Events with a position above this one are still to be sent (or skipped as not of this stream).
    cursor: u64,
    /// The last position at the connection: what lies at or below it is replay, which may be any size.
    replay_until: u64,
    max_lag: usize,
    version: watch::Receiver<u64>,
    closing: watch::Receiver<bool>,
    out: VecDeque<Event>,
    done: bool,
}

/// The name of an event as SSE carries it: one line.
fn one_line(name: &str) -> String {
    name.replace(['\r', '\n'], " ")
}

fn sse_event(position: u64, kind: &str, row: &serde_json::Value) -> Event {
    Event::default().id(position.to_string()).event(one_line(kind)).data(row.to_string())
}

impl Feed {
    pub fn new(daemon: &Daemon, source: Source, after: u64, max_lag: usize, closing: watch::Receiver<bool>) -> Feed {
        // The version is taken before the state is read, in the same hold of its lock as the position.
        let (version, replay_until) = daemon.read(|st| (st.subscribe(), st.last_position()));
        Feed { daemon: daemon.clone(), source, cursor: after, replay_until, max_lag, version, closing, out: VecDeque::new(), done: false }
    }

    /// One read of the state: queue what is after the cursor, or say what to do instead.
    fn step(&mut self, st: &State) -> Step {
        let replaying = self.cursor < self.replay_until;
        let limit = if replaying { REPLAY_BATCH } else { self.max_lag.saturating_add(1) };
        let (run, session) = self.source.filter();
        let page = st.events_page(run, session, self.cursor, limit);
        if !replaying && page.events.len() > self.max_lag {
            return Step::Lagged;
        }
        self.cursor = self.cursor.max(page.scanned);
        if !page.events.is_empty() {
            let rows = event_rows(page.events.clone());
            for (e, row) in page.events.iter().zip(&rows) {
                let e: &RunEvent = e;
                self.out.push_back(sse_event(e.position, &e.kind, row));
            }
            return Step::Events;
        }
        if self.source.finished(st) {
            Step::End
        } else {
            Step::Wait
        }
    }

    /// The next event of the stream; `None` when the stream is over.
    pub async fn next(&mut self) -> Option<Event> {
        loop {
            if *self.closing.borrow() {
                return None;
            }
            if let Some(event) = self.out.pop_front() {
                return Some(event);
            }
            if self.done {
                return None;
            }
            // Mark the version seen before the state is read: a change after this wakes the wait below.
            self.version.borrow_and_update();
            let daemon = self.daemon.clone();
            match daemon.read(|st| self.step(st)) {
                Step::Events => {}
                Step::End => self.done = true,
                Step::Lagged => {
                    self.done = true;
                    self.out.push_back(Event::default().comment(format!("the stream fell more than {} events behind and is closed: reconnect with Last-Event-ID: {}", self.max_lag, self.cursor)));
                }
                Step::Wait => {
                    tokio::select! {
                        changed = self.version.changed() => {
                            if changed.is_err() {
                                self.done = true;
                            }
                        }
                        () = closed(self.closing.clone()) => return None,
                    }
                }
            }
        }
    }
}

/// Completes when the daemon starts to shut down; never when the sender is gone.
pub async fn closed(mut closing: watch::Receiver<bool>) {
    loop {
        if *closing.borrow() {
            return;
        }
        if closing.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}
