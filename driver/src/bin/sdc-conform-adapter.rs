//! sdc-conform-adapter: an `ess-conformance-target/1` adapter that drives a delivered product's
//! HTTP JSON surface through a checked route table (`sdc.conformance-table/1`, docs/conformance.md).
//!
//! ```text
//! sdc-conform-adapter --table <table.json> --tree <delivered tree copy> --work <dir>
//!                     [--log <file.jsonl>] [--start-timeout <s>] [--request-timeout <s>]
//! ```
//!
//! `ess verify conform run --target process -- sdc-conform-adapter …` starts it. Each scenario gets
//! a fresh product process, started from the table's `start.argv` in the tree with a free port on
//! 127.0.0.1 and an empty data directory under `--work`, and killed (its whole process group) at
//! `end_scenario`. The product gets no environment but `PATH`, `LANG`, `HOST`, `PORT`, and `HOME`
//! and `TMPDIR` set to the scenario's directory: no credential of the caller reaches it. Its stdout
//! and stderr go to `product.log`; the table's `captures` are read from them (one regex group, within
//! `timeout_s` of the start; a capture that does not appear answers `begin_scenario` `unavailable`)
//! and are filled into paths, headers and bodies as `{capture.<name>}`. A `binding` replaces an input
//! field of that name by its capture and maps the captured value back to the spec value in events,
//! error fields and view rows. The adapter reports what the product answered, never a verdict: a
//! status the table does not map is no declared outcome. Events are built from the response body by the
//! table's payload templates (the product publishes none an HTTP client could observe). A product
//! that does not start is answered `unavailable` (the scenario ends `error`), a command or view the
//! table declares `none` `unsupported`. One JSON line per start, unmapped status and unfillable
//! template goes to `--log`.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value as J};

#[path = "../conformance.rs"]
#[allow(dead_code)]
mod conformance;

use conformance::{at, fill, fill_path, fill_text, http, none_of, read_json, s, sha256_bytes, Ctx, Reply};

const SPEAKS: &str = "ess-conformance-target/1";

struct Product {
    child: Child,
    port: u16,
}

impl Drop for Product {
    fn drop(&mut self) {
        let pid = self.child.id();
        let _ = Command::new("kill").args(["-s", "KILL", "--", &format!("-{pid}")]).stderr(Stdio::null()).status();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Adapter {
    table: J,
    name: String,
    tree: PathBuf,
    work: PathBuf,
    log: Option<File>,
    start_timeout: Duration,
    request_timeout: Duration,
    product: Option<Product>,
    scenario: String,
    scenarios: usize,
    events: Vec<J>,
    forced: BTreeMap<String, (String, u64)>,
    writes: u64,
    captured: J,
    reverse: BTreeMap<String, J>,
}

enum Answer {
    Ok(J),
    Unsupported(String),
    Unavailable(String),
}

/// A filled request: path, body, headers.
type Filled = (String, Option<J>, Vec<(String, String)>);

enum Fail {
    Start(String),
    Capture(String, String),
}

/// Copies one output stream of the product, line by line, into `product.log` and a buffer the
/// captures are read from.
fn pump(r: impl Read + Send + 'static, log: Arc<Mutex<File>>, buf: Arc<Mutex<String>>) {
    std::thread::spawn(move || {
        for line in BufReader::new(r).lines().map_while(Result::ok) {
            if let Ok(mut f) = log.lock() {
                let _ = writeln!(f, "{line}");
            }
            if let Ok(mut b) = buf.lock() {
                if b.len() < 1 << 20 {
                    b.push_str(&line);
                    b.push('\n');
                }
            }
        }
    });
}

fn free_port() -> Result<u16, String> {
    let l = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("no free port: {e}"))?;
    l.local_addr().map(|a| a.port()).map_err(|e| e.to_string())
}

impl Adapter {
    fn note(&mut self, v: J) {
        if let Some(f) = self.log.as_mut() {
            let mut v = v;
            v["scenario"] = json!(self.scenario);
            let _ = writeln!(f, "{v}");
        }
    }

    fn start(&mut self) -> Result<(), Fail> {
        self.product = None;
        self.captured = json!({});
        self.reverse.clear();
        self.scenarios += 1;
        let dir = self.work.join(format!("{:04}", self.scenarios));
        let data = dir.join("data");
        fs::create_dir_all(&data).map_err(|e| Fail::Start(format!("{}: {e}", data.display())))?;
        let port = free_port().map_err(Fail::Start)?;
        let argv: Vec<String> = self.table["start"]["argv"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|a| {
                s(a).replace("{host}", "127.0.0.1")
                    .replace("{port}", &port.to_string())
                    .replace("{data_dir}", &data.to_string_lossy())
                    .replace("{data_file}", &data.join("data").to_string_lossy())
            })
            .collect();
        let Some(prog) = argv.first() else { return Err(Fail::Start("start.argv is empty".into())) };
        let prog = if prog.contains('/') { self.tree.join(prog) } else { PathBuf::from(prog) };
        let log = Arc::new(Mutex::new(File::create(dir.join("product.log")).map_err(|e| Fail::Start(e.to_string()))?));
        let mut child = Command::new(&prog)
            .args(&argv[1..])
            .current_dir(&self.tree)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("LANG", "C.UTF-8")
            .env("HOME", &dir)
            .env("TMPDIR", &dir)
            .env("HOST", "127.0.0.1")
            .env("PORT", port.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()
            .map_err(|e| Fail::Start(format!("cannot start {}: {e}", prog.display())))?;
        let spawned = Instant::now();
        let streams = [Arc::new(Mutex::new(String::new())), Arc::new(Mutex::new(String::new()))];
        if let Some(o) = child.stdout.take() {
            pump(o, log.clone(), streams[0].clone());
        }
        if let Some(e) = child.stderr.take() {
            pump(e, log.clone(), streams[1].clone());
        }
        let mut product = Product { child, port };
        let exited = |product: &mut Product| -> Option<String> {
            let st = product.child.try_wait().ok().flatten()?;
            std::thread::sleep(Duration::from_millis(100));
            let tail = fs::read_to_string(dir.join("product.log")).unwrap_or_default();
            let tail: String = tail.lines().rev().take(5).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" | ");
            Some(format!("the product exited before it answered ({st}): {tail}"))
        };
        let mut captured = serde_json::Map::new();
        for c in self.table["captures"].as_array().cloned().unwrap_or_default() {
            let (name, stream) = (s(&c["name"]).to_string(), s(&c["stream"]).to_string());
            let re = regex::Regex::new(s(&c["regex"])).map_err(|e| Fail::Start(format!("capture {name}: {e}")))?;
            let buf = &streams[usize::from(stream == "stderr")];
            let secs = c["timeout_s"].as_u64().unwrap_or(5);
            let deadline = spawned + Duration::from_secs(secs);
            loop {
                let found = re.captures(&buf.lock().map(|b| b.clone()).unwrap_or_default()).and_then(|m| m.get(1).map(|g| g.as_str().to_string()));
                if let Some(v) = found {
                    captured.insert(name.clone(), J::String(v));
                    break;
                }
                if let Some(why) = exited(&mut product) {
                    return Err(Fail::Start(why));
                }
                if Instant::now() > deadline {
                    return Err(Fail::Capture(name.clone(), format!("capture {name} did not appear on {stream} within {secs} s")));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        self.captured = J::Object(captured);
        self.product = Some(product);
        let ready = self.table["start"]["ready"].clone();
        let deadline = spawned + self.start_timeout;
        loop {
            if let Some(why) = self.product.as_mut().and_then(exited) {
                self.product = None;
                return Err(Fail::Start(why));
            }
            if self.send_with(&ready, &BTreeMap::new(), Duration::from_millis(500)).is_ok() {
                break;
            }
            if Instant::now() > deadline {
                self.product = None;
                return Err(Fail::Start(format!("the product did not answer {} {} within {} s", s(&ready["method"]), s(&ready["path"]), self.start_timeout.as_secs())));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    }

    fn send(&self, r: &J, ctx: &Ctx) -> Result<Reply, String> {
        self.send_with(r, ctx, self.request_timeout)
    }

    /// One request of the table, filled from `ctx` and the scenario's captures.
    fn send_with(&self, r: &J, ctx: &Ctx, timeout: Duration) -> Result<Reply, String> {
        let filled = self.fill_request(r, ctx)?;
        self.send_filled(r, filled, timeout)
    }

    /// Path, body and headers of a table request, filled; an error names the placeholder that
    /// has no value (the table's fault when the scenario does not carry it).
    fn fill_request(&self, r: &J, ctx: &Ctx) -> Result<Filled, String> {
        let mut ctx = ctx.clone();
        ctx.insert("capture", self.captured.clone());
        let path = fill_path(s(&r["path"]), &ctx)?;
        let body = match r.get("body") {
            None | Some(J::Null) => None,
            Some(b) => Some(fill(b, &ctx)?),
        };
        let mut headers = vec![];
        for (k, v) in r["headers"].as_object().into_iter().flatten() {
            headers.push((k.clone(), fill_text(s(v), &ctx)?));
        }
        Ok((path, body, headers))
    }

    fn send_filled(&self, r: &J, (path, body, headers): Filled, timeout: Duration) -> Result<Reply, String> {
        let port = self.product.as_ref().map(|p| p.port).ok_or("no product is running")?;
        http(port, s(&r["method"]), &path, body.as_ref(), &headers, timeout)
    }

    /// A command's or view's request sent; one that cannot be filled is logged `request_unfillable`.
    fn send_logged(&mut self, about: (&str, &str), r: &J, ctx: &Ctx) -> Result<Reply, String> {
        let filled = match self.fill_request(r, ctx) {
            Ok(f) => f,
            Err(why) => {
                let mut v = json!({"event": "request_unfillable", "why": why});
                v[about.0] = json!(about.1);
                self.note(v);
                return Err(why);
            }
        };
        let path = filled.0.clone();
        let reply = self.send_filled(r, filled, self.request_timeout);
        let mut v = json!({"event": "request", "method": s(&r["method"]), "path": path,
            "status": reply.as_ref().ok().map(|x| x.status), "error": reply.as_ref().err()});
        v[about.0] = json!(about.1);
        self.note(v);
        reply
    }

    /// The scenario's input with every bound field replaced by its captured value; each replaced
    /// spec value is remembered so that what the product answers maps back to it.
    fn bind(&mut self, input: &J) -> J {
        let mut input = input.clone();
        for b in self.table["bindings"].as_array().cloned().unwrap_or_default() {
            let (field, cap) = (s(&b["input"]), s(&b["capture"]));
            let Some(v) = self.captured.get(cap).cloned() else { continue };
            if let Some(slot) = input.as_object_mut().and_then(|m| m.get_mut(field)) {
                if *slot != v {
                    self.reverse.insert(v.to_string(), slot.clone());
                }
                *slot = v;
            }
        }
        input
    }

    /// A value the product answered with its captured values mapped back to the spec values.
    fn unbind(&self, v: J) -> J {
        match v {
            J::Array(a) => J::Array(a.into_iter().map(|x| self.unbind(x)).collect()),
            J::Object(m) => J::Object(m.into_iter().map(|(k, x)| (k, self.unbind(x))).collect()),
            x => self.reverse.get(&x.to_string()).cloned().unwrap_or(x),
        }
    }

    fn command_entry(&self, name: &str) -> Option<J> {
        self.table["commands"].as_array()?.iter().find(|c| s(&c["command"]) == name).cloned()
    }

    fn execute(&mut self, req: &J) -> Answer {
        let name = s(&req["command"]).to_string();
        let Some(c) = self.command_entry(&name) else { return Answer::Unsupported(format!("the route table has no entry for {name}")) };
        if let Some((kind, why)) = none_of(&c) {
            return Answer::Unsupported(format!("the product offers no way to run {name} ({kind}): {why}"));
        }
        let mut request = c["request"].clone();
        if let Some((outcome, left)) = self.forced.get(&name).cloned() {
            if let Some(f) = c["forced"].as_array().into_iter().flatten().find(|f| s(&f["outcome"]) == outcome) {
                request = f["request"].clone();
            }
            if left <= 1 { self.forced.remove(&name); } else { self.forced.insert(name.clone(), (outcome, left - 1)); }
        }
        let mut ctx: Ctx = BTreeMap::new();
        let input = self.bind(&req["input"]);
        ctx.insert("input", input);
        let reply = match self.send_logged(("command", &name), &request, &ctx) {
            Ok(r) => r,
            Err(e) => return Answer::Unavailable(format!("{name}: {} {}: {e}", s(&request["method"]), s(&request["path"]))),
        };
        self.writes += 1;
        let body = reply.json();
        ctx.insert("response", body.clone());
        let matched = c["outcomes"].as_array().into_iter().flatten().find(|o| {
            conformance::statuses(o).is_some_and(|st| st.contains(&i64::from(reply.status)))
                && (!o["when_body"].is_object() || at(&body, s(&o["when_body"]["path"])) == Some(&o["when_body"]["equals"]))
        }).cloned();
        let mut result = json!({"outcome": null, "error": null, "consistency": self.writes.to_string(), "direct_events": []});
        let Some(o) = matched else {
            self.note(json!({"event": "unmapped_status", "command": name, "status": reply.status, "body": reply.body.chars().take(300).collect::<String>()}));
            return Answer::Ok(result);
        };
        if o["lookup"].is_object() {
            match self.send(&o["lookup"], &ctx) {
                Ok(r) => { ctx.insert("lookup", r.json()); }
                Err(e) => self.note(json!({"event": "lookup_failed", "command": name, "why": e})),
            }
        }
        result["outcome"] = json!({"command": name, "outcome": o["outcome"]});
        let mut events = vec![];
        for e in o["events"].as_array().into_iter().flatten() {
            match fill(&e["payload"], &ctx) {
                Ok(p) => events.push(json!({"event": e["event"], "payload": self.unbind(p)})),
                Err(why) => self.note(json!({"event": "fill_failed", "command": name, "built": e["event"], "why": why})),
            }
        }
        if o["error"].is_object() {
            let mut fields = serde_json::Map::new();
            for (k, t) in o["error"]["fields"].as_object().into_iter().flatten() {
                let v = fill(t, &ctx).unwrap_or_else(|why| {
                    self.note(json!({"event": "fill_failed", "command": name, "field": k, "why": why}));
                    J::Null
                });
                fields.insert(k.clone(), self.unbind(v));
            }
            result["error"] = json!({"error": o["error"]["error"], "fields": fields});
        }
        self.events.extend(events.iter().cloned());
        result["direct_events"] = J::Array(events);
        Answer::Ok(result)
    }

    fn query(&mut self, req: &J) -> Answer {
        let name = s(&req["view"]).to_string();
        let Some(v) = self.table["views"].as_array().into_iter().flatten().find(|v| s(&v["view"]) == name).cloned() else {
            return Answer::Unsupported(format!("the route table has no entry for {name}"));
        };
        if let Some((kind, why)) = none_of(&v) {
            return Answer::Unsupported(format!("the product offers no way to read {name} ({kind}): {why}"));
        }
        let mut ctx: Ctx = BTreeMap::new();
        let params = self.bind(&req.get("params").cloned().unwrap_or(json!({})));
        ctx.insert("params", params);
        let reply = match self.send_logged(("view", &name), &v["request"], &ctx) {
            Ok(r) => r,
            Err(e) => return Answer::Unavailable(format!("{name}: {e}")),
        };
        let body = reply.json();
        let rows = at(&body, s(&v["rows"])).and_then(J::as_array).cloned();
        let Some(rows) = rows.filter(|_| (200..300).contains(&reply.status)) else {
            self.note(json!({"event": "view_unreadable", "view": name, "status": reply.status, "body": reply.body.chars().take(300).collect::<String>()}));
            return Answer::Ok(json!({"rows": []}));
        };
        let fields = v["fields"].as_object().cloned().unwrap_or_default();
        let rows: Vec<J> = rows.iter().map(|r| {
            self.unbind(J::Object(fields.iter().filter_map(|(k, p)| at(r, s(p)).map(|x| (k.clone(), x.clone()))).collect()))
        }).collect();
        Answer::Ok(json!({"rows": rows}))
    }

    fn answer(&mut self, op: &str, req: &J) -> Answer {
        match op {
            "handshake" => Answer::Ok(json!({"speaks": SPEAKS, "implementation": {"name": self.name, "version": env!("CARGO_PKG_VERSION")}})),
            "begin_scenario" => {
                self.scenario = s(&req["scenario"]).to_string();
                self.events.clear();
                self.forced.clear();
                self.writes = 0;
                match self.start() {
                    Ok(()) => {
                        self.note(json!({"event": "start", "ok": true}));
                        Answer::Ok(J::Null)
                    }
                    Err(Fail::Start(why)) => {
                        self.note(json!({"event": "start", "ok": false, "why": why}));
                        Answer::Unavailable(format!("the product did not start: {why}"))
                    }
                    Err(Fail::Capture(name, why)) => {
                        self.product = None;
                        self.note(json!({"event": "capture", "ok": false, "name": name, "why": why}));
                        Answer::Unavailable(why)
                    }
                }
            }
            "end_scenario" => {
                self.product = None;
                Answer::Ok(J::Null)
            }
            "execute_command" => self.execute(req),
            "query_view" => self.query(req),
            "observe_events" => {
                let e = s(&req["event"]);
                Answer::Ok(J::Array(self.events.iter().filter(|x| s(&x["event"]) == e).cloned().collect()))
            }
            "configure_external_outcome" | "configure_external_outcome_repeatedly" => {
                let cmd = s(&req["force"]["command"]).to_string();
                let outcome = s(&req["force"]["outcome"]).to_string();
                let has = self.command_entry(&cmd).is_some_and(|c| c["forced"].as_array().into_iter().flatten().any(|f| s(&f["outcome"]) == outcome));
                if !has {
                    return Answer::Unsupported(format!("the route table gives no request that makes {cmd} take {outcome}"));
                }
                self.forced.insert(cmd, (outcome, req["times"].as_u64().unwrap_or(1)));
                Answer::Ok(J::Null)
            }
            _ => Answer::Unsupported(format!("this adapter does not implement {op}")),
        }
    }
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(table), Some(tree), Some(work)) = (arg(&args, "--table"), arg(&args, "--tree"), arg(&args, "--work")) else {
        eprintln!("usage: sdc-conform-adapter --table <table.json> --tree <dir> --work <dir> [--log <file>] [--start-timeout <s>] [--request-timeout <s>]");
        return ExitCode::from(64);
    };
    let t = match read_json(Path::new(&table)) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("sdc-conform-adapter: {e}");
            return ExitCode::from(2);
        }
    };
    let secs = |n: &str, d: u64| Duration::from_secs(arg(&args, n).and_then(|x| x.parse().ok()).unwrap_or(d));
    let log = arg(&args, "--log").and_then(|p| OpenOptions::new().create(true).append(true).open(p).ok());
    let mut a = Adapter {
        name: format!("sdc-conform-adapter table {}", &sha256_bytes(t.to_string().as_bytes())[..12]),
        table: t,
        tree: PathBuf::from(tree),
        work: PathBuf::from(work),
        log,
        start_timeout: secs("--start-timeout", 20),
        request_timeout: secs("--request-timeout", 10),
        product: None,
        scenario: String::new(),
        scenarios: 0,
        events: vec![],
        forced: BTreeMap::new(),
        writes: 0,
        captured: json!({}),
        reverse: BTreeMap::new(),
    };
    let _ = fs::create_dir_all(&a.work);
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines().map_while(Result::ok) {
        let Ok(msg) = serde_json::from_str::<J>(&line) else { continue };
        let op = s(&msg["op"]).to_string();
        let req = msg.get("request").cloned().unwrap_or(J::Null);
        let reply = match a.answer(&op, &req) {
            Answer::Ok(v) => json!({"id": msg["id"], "ok": v}),
            Answer::Unsupported(why) => json!({"id": msg["id"], "target_error": {"kind": "unsupported", "observation": op, "why": why}}),
            Answer::Unavailable(why) => json!({"id": msg["id"], "target_error": {"kind": "unavailable", "operation": op, "detail": why}}),
        };
        if writeln!(out, "{reply}").and_then(|_| out.flush()).is_err() {
            break;
        }
    }
    a.product = None;
    ExitCode::SUCCESS
}
