//! The three event views asked as `text/event-stream` (story `daemon-skeleton-on-a-runtime-trait`,
//! part C2). A stream is read here from a plain TCP connection, chunk by chunk as it arrives.

use super::commands::Fx;
use runbook::api::{self, Streams};
use runbook::commands::{CloseSession, Daemon, OpenSession, SubmitRun};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Prints the time of a test when it ends.
struct Timed(&'static str, Instant);

fn timed(name: &'static str) -> Timed {
    Timed(name, Instant::now())
}

impl Drop for Timed {
    fn drop(&mut self) {
        println!("TIME {} {:?}", self.0, self.1.elapsed());
    }
}

// ---- a server whose shutdown the test controls ----

struct Srv {
    fx: Fx,
    addr: SocketAddr,
    rt: Option<tokio::runtime::Runtime>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    done: mpsc::Receiver<std::io::Result<()>>,
}

impl Srv {
    fn start(tag: &str, token: Option<&str>, streams: Streams) -> Srv {
        let fx = Fx::new(tag);
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        let listener = rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
        let addr = listener.local_addr().unwrap();
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let (done_tx, done) = mpsc::channel();
        let (daemon, token) = (fx.d().clone(), token.map(String::from));
        rt.spawn(async move {
            let r = api::serve_with(listener, daemon, token.as_deref(), streams, async {
                let _ = stopped.await;
            })
            .await;
            let _ = done_tx.send(r);
        });
        Srv { fx, addr, rt: Some(rt), stop: Some(stop), done }
    }

    fn quiet(tag: &str) -> Srv {
        Srv::start(tag, None, Streams::default())
    }

    /// A short keep-alive interval: a comment line arrives within a few milliseconds.
    fn lively(tag: &str) -> Srv {
        Srv::start(tag, None, Streams { keep_alive: Duration::from_millis(10), ..Streams::default() })
    }

    fn d(&self) -> Daemon {
        self.fx.d().clone()
    }

    /// Fire the shutdown and wait for `serve` to return; the time it took.
    fn shutdown(&mut self) -> Duration {
        let t = Instant::now();
        let _ = self.stop.take().unwrap().send(());
        let r = self.done.recv_timeout(Duration::from_secs(10)).expect("serve did not return within 10 s of the shutdown");
        assert!(r.is_ok(), "{r:?}");
        t.elapsed()
    }

    fn get(&self, path: &str, headers: &[(&str, &str)]) -> super::http::Resp {
        super::http::send(self.addr, "GET", path, headers, None)
    }

    fn page(&self, view: &str, query: &str) -> Vec<Value> {
        let r = self.get(&format!("/daemon/views/{view}?{query}"), &[]);
        assert_eq!(r.status, 200, "{}", r.text());
        r.json()["rows"].as_array().unwrap().clone()
    }

    fn open(&self, path: &str, headers: &[(&str, &str)]) -> Stream {
        Stream::open(self.addr, path, headers)
    }

    /// A live run that holds still until it is cancelled.
    fn hold_run(&self, session: Option<&str>) -> String {
        let run = self.fx.submit_with(SubmitRun { runbook_id: "hold".into(), intake: "i".into(), session_id: session.map(String::from), ..SubmitRun::default() });
        self.fx.wait_state(&run, "Running");
        run
    }

    fn record(&self, run: &str, kind: &str, record: Value) -> u64 {
        let o = self.d().record_run_event(run, kind, record).unwrap();
        assert_eq!(o.name, "recorded", "{o:?}");
        o.published[0].1["position"].as_u64().unwrap()
    }

    fn last_position(&self) -> u64 {
        self.page("daemon-events", "after_position=0").last().map_or(0, |e| e["position"].as_u64().unwrap())
    }
}

impl Drop for Srv {
    fn drop(&mut self) {
        if let Some(rt) = self.rt.take() {
            rt.shutdown_background();
        }
    }
}

// ---- a client that reads a stream as it arrives ----

#[derive(Debug, Clone, PartialEq)]
struct Ev {
    id: String,
    event: String,
    data: String,
}

impl Ev {
    fn json(&self) -> Value {
        serde_json::from_str(&self.data).unwrap_or_else(|e| panic!("data is not one JSON value ({e}): {}", self.data))
    }

    fn position(&self) -> u64 {
        self.id.parse().unwrap()
    }
}

#[derive(Debug)]
enum Item {
    Event(Ev),
    Comment(String),
}

struct Stream {
    tcp: TcpStream,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    raw: Vec<u8>,
    raw_at: usize,
    body: Vec<u8>,
    body_at: usize,
    chunked: bool,
    closed: bool,
}

impl Stream {
    fn open(addr: SocketAddr, path: &str, headers: &[(&str, &str)]) -> Stream {
        let mut tcp = TcpStream::connect(addr).unwrap();
        tcp.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let mut head = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\n");
        if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("accept")) {
            head.push_str("Accept: text/event-stream\r\n");
        }
        for (k, v) in headers {
            head.push_str(&format!("{k}: {v}\r\n"));
        }
        head.push_str("\r\n");
        tcp.write_all(head.as_bytes()).unwrap();
        let mut s = Stream { tcp, status: 0, headers: vec![], raw: vec![], raw_at: 0, body: vec![], body_at: 0, chunked: false, closed: false };
        let split = loop {
            if let Some(i) = s.raw.windows(4).position(|w| w == b"\r\n\r\n") {
                break i;
            }
            assert!(s.fill(), "the connection closed before a response head: {:?}", String::from_utf8_lossy(&s.raw));
        };
        let text = String::from_utf8_lossy(&s.raw[..split]).into_owned();
        let mut lines = text.lines();
        s.status = lines.next().unwrap().split(' ').nth(1).unwrap().parse().unwrap();
        s.headers = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
        s.chunked = s.header("transfer-encoding").is_some_and(|v| v.contains("chunked"));
        s.raw_at = split + 4;
        s
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    /// Read more bytes; false at the end of the connection. A read that waits over the timeout fails the test.
    fn fill(&mut self) -> bool {
        let mut buf = vec![0u8; 64 * 1024];
        match self.tcp.read(&mut buf) {
            Ok(0) => false,
            Ok(n) => {
                self.raw.extend_from_slice(&buf[..n]);
                true
            }
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => false,
            Err(e) => panic!("no data within the read timeout: {e}"),
        }
    }

    /// Move whole chunks from the raw bytes to the body.
    fn dechunk(&mut self) {
        if !self.chunked {
            self.body.extend_from_slice(&self.raw[self.raw_at..]);
            self.raw_at = self.raw.len();
            return;
        }
        loop {
            let rest = &self.raw[self.raw_at..];
            let Some(eol) = rest.windows(2).position(|w| w == b"\r\n") else { return };
            let n = usize::from_str_radix(std::str::from_utf8(&rest[..eol]).unwrap().trim(), 16).unwrap();
            if n == 0 {
                self.closed = true;
                self.raw_at = self.raw.len();
                return;
            }
            if rest.len() < eol + 2 + n + 2 {
                return;
            }
            self.body.extend_from_slice(&rest[eol + 2..eol + 2 + n]);
            self.raw_at += eol + 2 + n + 2;
        }
    }

    /// The next comment or event; `None` when the server closed the stream.
    fn next(&mut self) -> Option<Item> {
        loop {
            self.dechunk();
            let rest = &self.body[self.body_at..];
            if let Some(end) = rest.windows(2).position(|w| w == b"\n\n") {
                let message = String::from_utf8_lossy(&rest[..end]).into_owned();
                self.body_at += end + 2;
                if self.body_at > 4 << 20 {
                    self.body.drain(..self.body_at);
                    self.body_at = 0;
                }
                if self.raw_at > 4 << 20 {
                    self.raw.drain(..self.raw_at);
                    self.raw_at = 0;
                }
                return Some(parse(&message));
            }
            if self.closed || !self.fill() {
                self.dechunk();
                assert!(self.body.len() == self.body_at, "the stream ended inside a message: {:?}", String::from_utf8_lossy(&self.body[self.body_at..]));
                return None;
            }
        }
    }

    /// The next event, skipping comments; `None` at the end of the stream.
    fn event(&mut self) -> Option<Ev> {
        loop {
            match self.next()? {
                Item::Event(e) => return Some(e),
                Item::Comment(_) => {}
            }
        }
    }

    fn events(&mut self, n: usize) -> Vec<Ev> {
        (0..n).map(|i| self.event().unwrap_or_else(|| panic!("the stream ended after {i} of {n} events"))).collect()
    }

    /// Everything up to the end of the stream: its events and its comments.
    fn until_closed(&mut self) -> (Vec<Ev>, Vec<String>) {
        let (mut events, mut comments) = (vec![], vec![]);
        while let Some(item) = self.next() {
            match item {
                Item::Event(e) => events.push(e),
                Item::Comment(c) => comments.push(c),
            }
        }
        (events, comments)
    }

    /// Read until a comment arrives (a keep-alive); the events met on the way.
    fn until_comment(&mut self) -> Vec<Ev> {
        let mut events = vec![];
        loop {
            match self.next().expect("the stream ended while a keep-alive was awaited") {
                Item::Event(e) => events.push(e),
                Item::Comment(_) => return events,
            }
        }
    }
}

fn parse(message: &str) -> Item {
    let (mut id, mut event, mut data, mut comment) = (None, None, vec![], None);
    for line in message.split('\n') {
        if let Some(c) = line.strip_prefix(':') {
            comment = Some(c.trim().to_string());
        } else if let Some(v) = line.strip_prefix("id:") {
            id = Some(v.trim_start().to_string());
        } else if let Some(v) = line.strip_prefix("event:") {
            event = Some(v.trim_start().to_string());
        } else if let Some(v) = line.strip_prefix("data:") {
            data.push(v.strip_prefix(' ').unwrap_or(v).to_string());
        } else {
            panic!("a line that is no SSE field: {line:?} in {message:?}");
        }
    }
    match (comment, id) {
        (Some(c), None) if data.is_empty() => Item::Comment(c),
        (_, Some(id)) => Item::Event(Ev { id, event: event.expect("an event without its name"), data: data.join("\n") }),
        other => panic!("not a comment and not an event: {message:?} ({other:?})"),
    }
}

fn path(view: &str, query: &str) -> String {
    format!("/daemon/views/{view}?{query}")
}

fn positions(events: &[Ev]) -> Vec<u64> {
    events.iter().map(Ev::position).collect()
}

fn page_positions(rows: &[Value]) -> Vec<u64> {
    rows.iter().map(|r| r["position"].as_u64().unwrap()).collect()
}

// ---- 1. what a stream sends ----

#[test]
fn a_stream_sends_every_recorded_event_in_position_order_then_each_new_event() {
    let _t = timed("a_stream_sends_every_recorded_event_in_position_order_then_each_new_event");
    let s = Srv::quiet("sse-basic");
    let open = s.d().open_session(OpenSession { session_id: "s1".into(), client: "test".into() }).unwrap();
    assert_eq!(open.name, "opened");
    let a = s.hold_run(Some("s1"));
    let b = s.hold_run(None);
    s.record(&a, "note", json!({"n": 1}));
    s.record(&b, "other", json!({"n": 2}));
    s.record(&a, "note", json!({"n": 3}));
    let cases = [
        ("events", format!("run_id={a}&after_position=0"), "events"),
        ("session-events", "session_id=s1&after_position=0".to_string(), "session-events"),
        ("daemon-events", "after_position=0".to_string(), "daemon-events"),
    ];
    for (view, query, _) in cases {
        let rows = s.page(view, &query);
        assert!(rows.len() >= 3, "{view}: {rows:?}");
        let mut stream = s.open(&path(view, &query), &[]);
        assert_eq!(stream.status, 200, "{view}");
        assert_eq!(stream.header("content-type"), Some("text/event-stream"), "{view}");
        assert_eq!(stream.header("cache-control"), Some("no-cache"), "{view}");
        let got = stream.events(rows.len());
        for (e, row) in got.iter().zip(&rows) {
            assert_eq!(e.id, row["position"].to_string(), "{view}: the id is the position");
            assert_eq!(e.event, row["kind"].as_str().unwrap(), "{view}: the event is the kind");
            assert_eq!(e.json(), *row, "{view}: the data is the row the page holds");
            assert!(!e.data.contains('\n'), "{view}: one line");
        }
        assert!(page_positions(&rows).windows(2).all(|w| w[0] < w[1]));
        // Then it stays open and sends each new event as it is recorded.
        let p1 = s.record(&a, "later", json!({"n": 4}));
        let next = stream.event().unwrap();
        assert_eq!((next.position(), next.event.as_str()), (p1, "later"), "{view}");
        assert_eq!(next.json()["record"], json!({"n": 4}));
        let p2 = s.record(&a, "later", json!({"n": 5}));
        assert_eq!(stream.event().unwrap().position(), p2, "{view}");
    }
    // The run stream holds the events of its run only, the session stream those of its session.
    let mut only_b = s.open(&path("events", &format!("run_id={b}&after_position=0")), &[]);
    let all_b = s.page("events", &format!("run_id={b}&after_position=0"));
    assert!(only_b.events(all_b.len()).iter().all(|e| e.json()["run_id"] == json!(b)));
}

// ---- 2. resume ----

#[test]
fn a_stream_resumes_after_the_position_of_the_header_or_of_the_parameter_and_the_header_wins() {
    let _t = timed("a_stream_resumes_after_the_position_of_the_header_or_of_the_parameter_and_the_header_wins");
    let s = Srv::quiet("sse-resume");
    let run = s.hold_run(None);
    for i in 0..10 {
        s.record(&run, "note", json!({"i": i}));
    }
    let all = page_positions(&s.page("events", &format!("run_id={run}&after_position=0")));
    assert!(all.len() >= 10);
    let (n3, n7) = (all[2], all[6]);
    let after = |n: u64| all.iter().copied().filter(|p| *p > n).collect::<Vec<_>>();
    let read = |stream: &mut Stream, want: &[u64]| assert_eq!(positions(&stream.events(want.len())), want);

    let q = |n: u64| format!("run_id={run}&after_position={n}");
    // The header alone, with the parameter at 0 (the parameter is required by the page form of the view).
    let h = n3.to_string();
    read(&mut s.open(&path("events", &q(0)), &[("Last-Event-ID", &h)]), &after(n3));
    // The header alone, with no parameter at all.
    read(&mut s.open(&path("events", &format!("run_id={run}")), &[("Last-Event-ID", &h)]), &after(n3));
    // The parameter alone.
    read(&mut s.open(&path("events", &q(n7)), &[]), &after(n7));
    // Neither: everything.
    read(&mut s.open(&path("events", &format!("run_id={run}")), &[]), &after(0));
    // Both: the header wins, whichever is larger.
    let h3 = n3.to_string();
    read(&mut s.open(&path("events", &q(n7)), &[("Last-Event-ID", &h3)]), &after(n3));
    let h7 = n7.to_string();
    read(&mut s.open(&path("events", &q(n3)), &[("Last-Event-ID", &h7)]), &after(n7));
    // The same on the other two views.
    let daemon_all = page_positions(&s.page("daemon-events", "after_position=0"));
    let mid = daemon_all[daemon_all.len() / 2];
    let want: Vec<u64> = daemon_all.iter().copied().filter(|p| *p > mid).collect();
    let hm = mid.to_string();
    read(&mut s.open(&path("daemon-events", "after_position=0"), &[("Last-Event-ID", &hm)]), &want);
    read(&mut s.open(&path("daemon-events", &format!("after_position={mid}")), &[]), &want);
    s.d().open_session(OpenSession { session_id: "sx".into(), client: "t".into() }).unwrap();
    let in_s = s.hold_run(Some("sx"));
    s.record(&in_s, "note", json!({}));
    s.record(&in_s, "note", json!({}));
    let sess = page_positions(&s.page("session-events", "session_id=sx&after_position=0"));
    read(&mut s.open(&path("session-events", "session_id=sx&after_position=0"), &[("Last-Event-ID", &sess[0].to_string())]), &sess[1..]);
    // A reconnect continues exactly where the last event stopped: nothing twice, nothing skipped.
    let mut first = s.open(&path("events", &q(0)), &[]);
    let part = first.events(4);
    drop(first);
    let last = part.last().unwrap().id.clone();
    let mut second = s.open(&path("events", &q(0)), &[("Last-Event-ID", &last)]);
    let rest = second.events(all.len() - 4);
    let mut joined = positions(&part);
    joined.extend(positions(&rest));
    assert_eq!(joined, all);
}

#[test]
fn no_event_is_lost_or_sent_twice_while_a_stream_connects() {
    let _t = timed("no_event_is_lost_or_sent_twice_while_a_stream_connects");
    let s = Srv::quiet("sse-race");
    let run = s.hold_run(None);
    let (rounds, events) = (12usize, 50usize);
    for round in 0..rounds {
        let base = s.last_position();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let recorder = {
            let (d, run, barrier) = (s.d(), run.clone(), barrier.clone());
            std::thread::spawn(move || {
                barrier.wait();
                (0..events)
                    .map(|i| d.record_run_event(&run, "k", json!({"round": round, "i": i})).unwrap().published[0].1["position"].as_u64().unwrap())
                    .collect::<Vec<u64>>()
            })
        };
        // The connection is made at the very moment the recorder starts; the views alternate and the
        // position is given by the parameter or by the header.
        barrier.wait();
        let (view, query) = if round % 2 == 0 { ("daemon-events", format!("after_position={base}")) } else { ("events", format!("run_id={run}&after_position={base}")) };
        let header = base.to_string();
        let headers: Vec<(&str, &str)> = if round % 4 < 2 { vec![] } else { vec![("Last-Event-ID", header.as_str())] };
        let mut stream = s.open(&path(view, &query), &headers);
        let mut got = vec![];
        while got.len() < events {
            let e = stream.event().expect("the stream ended");
            if e.event == "k" {
                got.push(e.position());
            }
        }
        let recorded = recorder.join().unwrap();
        assert_eq!(got, recorded, "round {round}: what was recorded during the connect is what arrived, once and in order");
    }
}

// ---- 3. positions that are not positions ----

#[test]
fn a_last_event_id_that_is_not_a_position_is_refused_400_and_a_position_beyond_the_last_waits() {
    let _t = timed("a_last_event_id_that_is_not_a_position_is_refused_400_and_a_position_beyond_the_last_waits");
    let s = Srv::lively("sse-badid");
    let run = s.hold_run(None);
    let q = format!("run_id={run}&after_position=0");
    for bad in ["abc", "-1", "1.5", "+3", "", " ", "1 2", "0x10", "99999999999999999999999", "٣"] {
        let r = s.get(&path("events", &q), &[("Accept", "text/event-stream"), ("Last-Event-ID", bad)]);
        assert_eq!(r.status, 400, "Last-Event-ID {bad:?}: {}", r.text());
        let body = r.json();
        assert_eq!(body["refused"], json!("bad request"), "{body}");
        assert!(body["message"].as_str().unwrap().contains("Last-Event-ID"), "{body}");
        assert_eq!(r.header("content-type"), Some("application/json"));
    }
    // The parameter is read as strictly when it is the one in use.
    let r = s.get(&path("events", &format!("run_id={run}&after_position=x")), &[("Accept", "text/event-stream")]);
    assert_eq!((r.status, r.json()["refused"].clone()), (400, json!("bad request")), "{}", r.text());
    // The parameter is not read when the header is given: the header wins.
    let ok = s.open(&path("events", &format!("run_id={run}&after_position=x")), &[("Last-Event-ID", "0")]);
    assert_eq!(ok.status, 200);
    // The run is named, as in the page form.
    let r = s.get("/daemon/views/events?after_position=0", &[("Accept", "text/event-stream")]);
    let page = s.get("/daemon/views/events?after_position=0", &[]);
    assert_eq!((r.status, r.text()), (page.status, page.text()));
    assert_eq!(r.status, 400);

    // A position beyond the last recorded one waits: no event, however many are recorded below it.
    let last = s.last_position();
    let beyond = (last + 1000).to_string();
    let mut stream = s.open(&path("events", &q), &[("Last-Event-ID", &beyond)]);
    assert_eq!(stream.status, 200);
    s.record(&run, "below", json!({}));
    assert!(stream.until_comment().is_empty(), "nothing at or below the asked position is sent");
    // The last recorded position itself waits for the next event and sends it.
    let last = s.last_position().to_string();
    let mut stream = s.open(&path("events", &q), &[("Last-Event-ID", &last)]);
    let p = s.record(&run, "next", json!({}));
    let e = stream.event().unwrap();
    assert_eq!((e.position(), e.event.as_str()), (p, "next"));
}

// ---- 4. the end of a stream ----

#[test]
fn a_run_stream_ends_after_its_last_event_when_the_run_is_terminal() {
    let _t = timed("a_run_stream_ends_after_its_last_event_when_the_run_is_terminal");
    let s = Srv::lively("sse-run-end");
    // A live run: the stream stays open until the run is cancelled, then ends after the last event.
    let run = s.hold_run(None);
    s.record(&run, "note", json!({}));
    let q = format!("run_id={run}&after_position=0");
    let mut stream = s.open(&path("events", &q), &[]);
    let seen = stream.until_comment();
    assert!(!seen.is_empty());
    let o = s.fx.cancel(&run);
    assert_eq!(o.name, "cancelled", "{o:?}");
    let (rest, _) = stream.until_closed();
    let mut all = seen;
    all.extend(rest);
    let page = s.page("events", &q);
    assert_eq!(all.iter().map(Ev::json).collect::<Vec<_>>(), page, "every event of the run, the last one included, then the end");
    // A run that ended before the connection: everything, then the end at once.
    let quick = s.fx.submit("quick");
    s.fx.wait_state(&quick, "Ended");
    let (events, _) = s.open(&path("events", &format!("run_id={quick}&after_position=0")), &[]).until_closed();
    assert_eq!(events.iter().map(Ev::json).collect::<Vec<_>>(), s.page("events", &format!("run_id={quick}&after_position=0")));
    assert!(!events.is_empty());
    // A reconnect to an ended run after its last event: no event, the end.
    let last = events.last().unwrap().id.clone();
    let (none, _) = s.open(&path("events", &format!("run_id={quick}&after_position=0")), &[("Last-Event-ID", &last)]).until_closed();
    assert!(none.is_empty());
}

#[test]
fn a_session_stream_ends_when_the_session_is_closed_and_all_its_runs_are_terminal() {
    let _t = timed("a_session_stream_ends_when_the_session_is_closed_and_all_its_runs_are_terminal");
    let s = Srv::lively("sse-session-end");
    s.d().open_session(OpenSession { session_id: "ses".into(), client: "t".into() }).unwrap();
    let (r1, r2) = (s.hold_run(Some("ses")), s.hold_run(Some("ses")));
    let mut stream = s.open(&path("session-events", "session_id=ses&after_position=0"), &[]);
    let mut got = stream.until_comment();
    // Closing the session does not end it while a run is live.
    let o = s.d().close_session(CloseSession { session_id: "ses".into() }).unwrap();
    assert_eq!(o.name, "closed");
    assert_eq!(s.fx.cancel(&r1).name, "cancelled");
    got.extend(stream.until_comment());
    got.extend(stream.until_comment());
    // The last run ends: the stream ends after its last event.
    assert_eq!(s.fx.cancel(&r2).name, "cancelled");
    let (rest, _) = stream.until_closed();
    got.extend(rest);
    let page = s.page("session-events", "session_id=ses&after_position=0");
    assert_eq!(got.iter().map(Ev::json).collect::<Vec<_>>(), page);

    // The other order: the runs end first, the close of the session ends the stream.
    s.d().open_session(OpenSession { session_id: "ses2".into(), client: "t".into() }).unwrap();
    let r3 = s.hold_run(Some("ses2"));
    let mut stream = s.open(&path("session-events", "session_id=ses2&after_position=0"), &[]);
    let mut got = stream.until_comment();
    assert_eq!(s.fx.cancel(&r3).name, "cancelled");
    got.extend(stream.until_comment());
    got.extend(stream.until_comment());
    assert_eq!(s.d().close_session(CloseSession { session_id: "ses2".into() }).unwrap().name, "closed");
    let (rest, _) = stream.until_closed();
    got.extend(rest);
    assert_eq!(got.iter().map(Ev::json).collect::<Vec<_>>(), s.page("session-events", "session_id=ses2&after_position=0"));

    // A session that is closed with every run terminal at the connection: all, then the end.
    let (events, _) = s.open(&path("session-events", "session_id=ses2&after_position=0"), &[]).until_closed();
    assert_eq!(events.iter().map(Ev::json).collect::<Vec<_>>(), s.page("session-events", "session_id=ses2&after_position=0"));
}

#[test]
fn the_daemon_stream_ends_only_at_shutdown() {
    let _t = timed("the_daemon_stream_ends_only_at_shutdown");
    let mut s = Srv::lively("sse-daemon-end");
    s.d().open_session(OpenSession { session_id: "ses".into(), client: "t".into() }).unwrap();
    let run = s.hold_run(Some("ses"));
    let mut stream = s.open(&path("daemon-events", "after_position=0"), &[]);
    let mut other = s.open(&path("daemon-events", "after_position=0"), &[]);
    assert_eq!(s.fx.cancel(&run).name, "cancelled");
    s.d().close_session(CloseSession { session_id: "ses".into() }).unwrap();
    // Every run is terminal and the session is closed; the stream is still open (keep-alives arrive).
    stream.until_comment();
    stream.until_comment();
    let took = s.shutdown();
    assert!(took < Duration::from_secs(2), "serve returned after {took:?}");
    // Both streams end, after what they had not yet delivered.
    stream.until_closed();
    other.until_closed();
}

#[test]
fn sigterm_closes_every_open_stream_and_serve_exits_0_within_2_s() {
    let _t = timed("sigterm_closes_every_open_stream_and_serve_exits_0_within_2_s");
    let dir = super::state::scratch("sse-sigterm");
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_runbook"));
    cmd.arg("serve").arg("--state-dir").arg(dir.join("state")).args(["--listen", "127.0.0.1:0"]);
    cmd.env_remove("RUNBOOK_TOKEN").stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
    let mut child = cmd.spawn().unwrap();
    struct Guard(std::process::Child);
    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let out = child.stdout.take().unwrap();
    let mut guard = Guard(child);
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = std::io::BufRead::read_line(&mut std::io::BufReader::new(out), &mut line);
        let _ = tx.send(line);
    });
    let line = rx.recv_timeout(Duration::from_secs(10)).expect("no first line within 10 s");
    let addr: SocketAddr = line.trim().strip_prefix("runbook: listening on http://").unwrap().parse().unwrap();
    // One run, so that the streams have events, and three streams: one of each view, none of them ended.
    let body = json!({"runbook_id": "sdc", "intake": "hello"}).to_string();
    let r = super::http::send(addr, "POST", "/daemon/commands/submit-run", &[("Content-Type", "application/json")], Some(body.as_bytes()));
    assert_eq!(r.status, 202, "{}", r.text());
    let run = r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_string();
    let mut a = Stream::open(addr, "/daemon/views/daemon-events?after_position=0", &[]);
    let mut b = Stream::open(addr, &format!("/daemon/views/events?run_id={run}&after_position=0"), &[]);
    let c = Stream::open(addr, "/daemon/views/session-events?session_id=nobody-yet&after_position=0", &[]);
    assert_eq!((a.status, b.status, c.status), (200, 200, 200));
    a.events(1);
    b.events(1);
    // A client that never reads does not hold the shutdown either.
    let silent = Stream::open(addr, "/daemon/views/daemon-events?after_position=0", &[]);
    let t = Instant::now();
    let killed = std::process::Command::new("kill").args(["-TERM", &guard.0.id().to_string()]).status().unwrap();
    assert!(killed.success());
    let (ea, _) = a.until_closed();
    let (eb, _) = b.until_closed();
    let _ = (ea, eb);
    let status = loop {
        if let Some(s) = guard.0.try_wait().unwrap() {
            break s;
        }
        assert!(t.elapsed() < Duration::from_secs(2), "serve was still running {:?} after SIGTERM", t.elapsed());
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(status.code(), Some(0));
    println!("exit {:?} after SIGTERM", t.elapsed());
    drop(silent);
}

// ---- 5. keep-alive ----

#[test]
fn a_comment_line_is_sent_as_keep_alive_at_the_injected_interval() {
    let _t = timed("a_comment_line_is_sent_as_keep_alive_at_the_injected_interval");
    assert_eq!(Streams::default().keep_alive, Duration::from_secs(15));
    let s = Srv::lively("sse-keepalive");
    let run = s.hold_run(None);
    let mut stream = s.open(&path("events", &format!("run_id={run}&after_position=0")), &[]);
    let mut comments = 0;
    let mut events = 0;
    while comments < 3 {
        match stream.next().unwrap() {
            Item::Comment(_) => comments += 1,
            Item::Event(_) => events += 1,
        }
    }
    assert!(events >= 1, "the events of the run came first");
    // A keep-alive is no event: the next event after it is the one that is recorded.
    let p = s.record(&run, "after", json!({}));
    assert_eq!(stream.event().unwrap().position(), p);
}

// ---- 6. a slow or gone client ----

/// Events that fill the buffers of a connection that nobody reads: this many of this many bytes.
const BIG_EVENTS: usize = 40;
const BIG_BYTES: usize = 256 * 1024;
/// More than the injected bound, so that they are what a stream falls behind by.
const SMALL_EVENTS: usize = 150;
const TEST_MAX_LAG: usize = 100;

#[test]
fn recording_never_waits_for_a_stream_and_a_client_that_falls_behind_is_disconnected_and_resumes_by_position() {
    let _t = timed("recording_never_waits_for_a_stream_and_a_client_that_falls_behind_is_disconnected_and_resumes_by_position");
    let s = Srv::start("sse-lag", None, Streams { max_lag: TEST_MAX_LAG, ..Streams::default() });
    let run = s.hold_run(None);
    let base = s.last_position();
    let mut stream = s.open(&path("daemon-events", &format!("after_position={base}")), &[]);
    let first = s.record(&run, "first", json!({}));
    assert_eq!(stream.event().unwrap().position(), first, "the stream is live");
    // The client now reads nothing. Big events fill the buffers of the connection, so that the server
    // cannot write; then more events than a stream may fall behind are recorded. Recording must go on.
    let (done_tx, done) = mpsc::channel();
    let recorder = {
        let (d, run) = (s.d(), run.clone());
        std::thread::spawn(move || {
            let t = Instant::now();
            let pad = "x".repeat(BIG_BYTES);
            let mut last = 0;
            for i in 0..BIG_EVENTS {
                last = d.record_run_event(&run, "big", json!({"i": i, "pad": pad})).unwrap().published[0].1["position"].as_u64().unwrap();
            }
            for i in 0..SMALL_EVENTS {
                last = d.record_run_event(&run, "small", json!({"i": i})).unwrap().published[0].1["position"].as_u64().unwrap();
            }
            let _ = done_tx.send((last, t.elapsed()));
        })
    };
    let (last, took) = done.recv_timeout(Duration::from_secs(30)).expect("recording waited for a stream that does not read");
    recorder.join().unwrap();
    println!("recording {} events while a client reads nothing: {took:?}", BIG_EVENTS + SMALL_EVENTS);
    // The client reads again: it receives events in order up to a point, then the server ends the stream.
    let (events, comments) = stream.until_closed();
    assert!(!events.is_empty());
    let got = positions(&events);
    assert!(got.windows(2).all(|w| w[1] == w[0] + 1), "no gap and no repeat before the end");
    let cut = *got.last().unwrap();
    assert!(cut < last, "the stream was ended with {} events behind", last - cut);
    assert!(comments.iter().any(|c| c.contains("Last-Event-ID")), "the end says how to resume: {comments:?}");
    // It resumes by position and gets the rest, once.
    let header = cut.to_string();
    let mut again = s.open(&path("daemon-events", &format!("after_position={base}")), &[("Last-Event-ID", &header)]);
    let mut rest = vec![];
    while rest.last().copied().unwrap_or(cut) < last {
        rest.push(again.event().expect("ended before the last event").position());
    }
    assert_eq!(rest, (cut + 1..=last).collect::<Vec<_>>());
    assert_eq!(Streams::default().max_lag, 1024, "the bound: a live stream may fall 1,024 events behind");
}

// ---- 7. the token ----

#[test]
fn a_token_is_checked_on_a_stream_as_on_every_route() {
    let _t = timed("a_token_is_checked_on_a_stream_as_on_every_route");
    let s = Srv::start("sse-token", Some("s3cret"), Streams::default());
    let q = path("daemon-events", "after_position=0");
    let r = s.get(&q, &[("Accept", "text/event-stream")]);
    assert_eq!(r.status, 401, "{}", r.text());
    assert_eq!(r.header("www-authenticate"), Some("Bearer"));
    let wrong = s.get(&q, &[("Accept", "text/event-stream"), ("Authorization", "Bearer nope")]);
    assert_eq!(wrong.status, 401);
    let ok = s.open(&q, &[("Authorization", "Bearer s3cret")]);
    assert_eq!((ok.status, ok.header("content-type")), (200, Some("text/event-stream")));
}

// ---- 8. an id the daemon does not know ----

#[test]
fn an_unknown_run_or_session_answers_as_the_page_form_does_before_any_stream_starts() {
    let _t = timed("an_unknown_run_or_session_answers_as_the_page_form_does_before_any_stream_starts");
    let s = Srv::quiet("sse-unknown");
    for (view, query) in [("events", "run_id=no-such-run&after_position=0"), ("session-events", "session_id=no-such-session&after_position=0")] {
        let page = s.get(&path(view, query), &[("Accept", "application/json")]);
        assert_eq!((page.status, page.json()), (200, json!({"rows": []})), "{view}");
        // The page holds no row for an unknown id, and so does the stream: it answers 200 and has
        // nothing to send and nothing that can follow, so it ends at once.
        let mut stream = s.open(&path(view, query), &[]);
        assert_eq!(stream.status, page.status, "{view}");
        let (events, _) = stream.until_closed();
        assert!(events.is_empty(), "{view}");
    }
    // A required parameter that is missing is refused as the page form refuses it.
    for view in ["events", "session-events"] {
        let page = s.get(&format!("/daemon/views/{view}?after_position=0"), &[]);
        let stream = s.get(&format!("/daemon/views/{view}?after_position=0"), &[("Accept", "text/event-stream")]);
        assert_eq!((stream.status, stream.text()), (page.status, page.text()), "{view}");
        assert_eq!(page.status, 400);
    }
    // The views that are no event view are not streams: asked as a stream they answer their page.
    let r = s.get("/daemon/views/runs", &[("Accept", "text/event-stream")]);
    assert_eq!((r.status, r.header("content-type")), (200, Some("application/json")));
}
