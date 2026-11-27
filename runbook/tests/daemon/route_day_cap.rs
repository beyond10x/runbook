//! A daily cap on a route of any kind (story `any-route-kind-takes-a-daily-cap`; `docs/model-routes.md`, section 5;
//! `docs/daemon-api.md`, "The day's figures of a route"): `daily_cap_micro_usd` and `daily_max_llm_calls` on a managed,
//! hosted or local route. The relay books each call's cost against the route's day (the UTC date of the daemon's clock),
//! and a call past the day's figure is refused `429 cap-reached` with `cap_key` `route.<name>.<key>` before it is sent.
//! The figures are rebuilt from the spend ledgers when the daemon starts, and the view `routes` answers them.
//!
//! The rule of the spend cap: a call is sent while the day's booked spend of the route is below the cap; its cost is
//! booked when its answer has ended, before its caller sees that end, so no call is sent once the booked figure has
//! reached the cap. The rule of a route with no declared price: its calls' cost is not known, so `daily_cap_micro_usd`
//! is refused on it at start and its calls are capped by `daily_max_llm_calls`.
//!
//! In process on a test clock with a stand-in gateway on loopback that answers 1000 input tokens a call; the run's
//! outcome at the cap runs `runbook serve`. No model is called and nothing leaves loopback.

use super::default_confinement::{post, rows, Live};
use super::gateway::{home, serve, until_state, write_exe, Home};
use super::http::{send, Resp};
use super::relay::{gateway_answering, got, Gateway};
use runbook::clock::SetClock;
use runbook::commands::{Daemon, Policy};
use runbook::ledger::Ledger;
use runbook::relay::{Booking, Relay, ResolvedRoute, Ticket};
use runbook::routes::{RouteTable, Tables};
use runbook::runtime::Confinement;
use runbook::stub::StubRuntime;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const GATEWAY_TOKEN: &str = "gw-day-cap-71c4e0b2a9d3";
const DAEMON_TOKEN: &str = "day-cap-daemon-token-0123456789-abcdefgh";
const MORNING: &str = "2026-11-28T09:00:00Z";
/// The four members of a row of the view `routes` this story adds.
const DAILY: [&str; 4] = ["daily_booked_micro_usd", "daily_cap_micro_usd", "daily_llm_calls", "daily_max_llm_calls"];

/// A stand-in gateway that answers every call 200 with 1000 input tokens and no output (1000 micro USD at 1 USD per
/// million input tokens), and is ready and lists the aliases of the routes below.
fn gateway() -> Gateway {
    let body = r#"{"id":"m","content":[],"usage":{"input_tokens":1000,"output_tokens":0}}"#;
    let answer = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).into_bytes();
    gateway_answering(move |_| answer.clone(), &["alias-h", "alias-l", "alias-n", "alias-m"])
}

/// The routes of the in-process cases: `h` hosted at 1 USD per million input tokens with a daily cap of 3000 micro USD,
/// `l` local without prices and at most 2 calls a day, `n` hosted without prices and at most 1 call a day, `m` managed
/// at 1 USD per million input tokens with both figures, and `free` hosted with neither.
fn routes() -> Value {
    json!({
        "h": {"gateway": "gw", "model": "alias-h", "kind": "hosted", "price_in_micro_usd_per_mtok": 1_000_000, "daily_cap_micro_usd": 3000},
        "l": {"gateway": "gw", "model": "alias-l", "kind": "local", "daily_max_llm_calls": 2},
        "n": {"gateway": "gw", "model": "alias-n", "kind": "hosted", "daily_max_llm_calls": 1},
        "m": {"gateway": "gw", "model": "alias-m", "kind": "managed", "price_in_micro_usd_per_mtok": 1_000_000, "daily_cap_micro_usd": 5000, "daily_max_llm_calls": 4},
        "free": {"gateway": "gw", "model": "alias-h", "kind": "hosted", "price_in_micro_usd_per_mtok": 1_000_000},
    })
}

/// The run's side of the relay: no cap of its own, a reservation of 7000 micro USD for a call of unknown cost, and its
/// lines booked into the spend ledger `<state>/ledgers/<runbook>.json` as a daemon run books them.
struct Book {
    ledger: Ledger,
    run: String,
    capped: Mutex<Vec<(String, String)>>,
}

impl Booking for Book {
    fn admit(&self) -> Result<(), String> {
        Ok(())
    }
    fn reservation_micro_usd(&self) -> u64 {
        7_000
    }
    fn step(&self) -> Option<String> {
        Some("notes.write".to_owned())
    }
    fn book(&self, line: Value, micro_usd: u64) -> Result<(), String> {
        self.ledger.book_line(&self.run, &line, micro_usd)
    }
    fn route_capped(&self, route: &str, key: &str, _detail: &str) {
        self.capped.lock().unwrap().push((route.to_owned(), key.to_owned()));
    }
}

/// A daemon on a test clock with the routes of `routes` on one stand-in gateway, and its relay mounted beside its API as
/// `runbook serve` mounts it, counting the routes' days through the daemon.
struct Rig {
    dir: PathBuf,
    clock: Arc<SetClock>,
    table: RouteTable,
    d: Daemon,
    relay: Arc<Relay>,
    addr: SocketAddr,
    _rt: tokio::runtime::Runtime,
}

fn table_of(gw: &Gateway, dir: &Path, routes: &Value) -> RouteTable {
    let token_file = dir.join("gateway-token");
    std::fs::write(&token_file, format!("{GATEWAY_TOKEN}\n")).unwrap();
    let gateways = json!({"gw": {"base_url": gw.base_url, "token_file": token_file.display().to_string()}});
    let mut table = RouteTable::declare(&Tables { gateways: Some(&gateways), routes: Some(routes), ..Tables::default() }).unwrap();
    table.read_tokens().unwrap();
    table
}

impl Rig {
    fn new(tag: &str, gw: &Gateway) -> Rig {
        let dir = super::state::scratch(tag);
        Rig::on(dir, gw, Arc::new(SetClock::at(MORNING).unwrap()))
    }

    /// A daemon on the state of `dir`, as one started again there.
    fn on(dir: PathBuf, gw: &Gateway, clock: Arc<SetClock>) -> Rig {
        std::fs::create_dir_all(dir.join("ctx")).unwrap();
        std::fs::create_dir_all(dir.join("runbooks")).unwrap();
        let table = table_of(gw, &dir, &routes());
        let policy = Policy { default_confinement: Some(Confinement::Linux), routes: Arc::new(table.clone()), ..Policy::default() };
        let d = Daemon::start_full(super::commands::config(&dir), policy, Arc::new(StubRuntime::new()), clock.clone()).unwrap();
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        let listener = rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
        let addr = listener.local_addr().unwrap();
        let relay = Relay::new();
        relay.set_listen(addr);
        relay.count_days(d.route_days());
        let (daemon, extra) = (d.clone(), relay.router());
        rt.spawn(async move {
            let _ = runbook::api::serve_extra(listener, daemon, Some(DAEMON_TOKEN), runbook::api::Streams::default(), Some(extra), std::future::pending()).await;
        });
        Rig { dir, clock, table, d, relay, addr, _rt: rt }
    }

    /// The run `run` on the relay with its role `default` on the route `route`, booked into the ledger of the runbook
    /// `rb`.
    fn open(&self, run: &str, route: &str) -> (Ticket, Arc<Book>) {
        let resolved = ResolvedRoute::from(self.table.resolve(route).unwrap());
        let ticket = self.relay.open(run, runbook::relay::RouteTable::default().with("default", resolved, "run")).unwrap();
        let ledgers = self.dir.join("state/ledgers");
        std::fs::create_dir_all(&ledgers).unwrap();
        let book = Arc::new(Book { ledger: Ledger::open(&ledgers.join("rb.json")).unwrap(), run: run.to_owned(), capped: Mutex::new(Vec::new()) });
        ticket.take_calls(book.clone());
        (ticket, book)
    }

    fn call(&self, ticket: &Ticket) -> Resp {
        let path = format!("{}/v1/messages", ticket.base_url().strip_prefix(&format!("http://{}", self.addr)).unwrap());
        let body = json!({"model": "default", "max_tokens": 8, "messages": [{"role": "user", "content": "hi"}]});
        send(self.addr, "POST", &path, &[("Content-Type", "application/json"), ("Authorization", &format!("Bearer {}", ticket.token())), ("anthropic-version", "2023-06-01")], Some(body.to_string().as_bytes()))
    }

    /// The row of `route` in the view `routes`.
    fn row(&self, route: &str) -> Value {
        self.d.routes().into_iter().find(|r| r["route"] == route).unwrap_or_else(|| panic!("no row of {route}"))
    }

    /// The model-call lines of the ledger of the runbook `rb`.
    fn lines(&self) -> Vec<Value> {
        let ledger: Value = serde_json::from_slice(&std::fs::read(self.dir.join("state/ledgers/rb.json")).unwrap_or_default()).unwrap_or(Value::Null);
        ledger["entries"].as_array().cloned().unwrap_or_default().into_iter().filter(|e| e["what"] == "model call").collect()
    }
}

/// The daily figures of a row: the four members, null where absent.
fn daily(row: &Value) -> [Value; 4] {
    DAILY.map(|k| row.get(k).cloned().unwrap_or(Value::Null))
}

fn refused_with(r: &Resp, key: &str) {
    assert_eq!(r.status, 429, "{}", r.text());
    let body = r.json();
    assert_eq!((body["error"]["code"].clone(), body["error"]["cap_key"].clone()), (json!("cap-reached"), json!(key)), "{body}");
}

/// Acceptance 1 and 6, and the view: a hosted route with `daily_cap_micro_usd = 3000` and calls of 1000 each sends the
/// third call and refuses the fourth, naming the route's key; the stand-in saw 3 calls.
#[test]
fn a_hosted_route_with_a_daily_cap_of_3000_sends_three_calls_of_1000_and_refuses_the_fourth_naming_its_key() {
    let gw = gateway();
    let r = Rig::new("day-cap-hosted", &gw);
    let (ticket, book) = r.open("run-h", "h");
    for n in 1..=3 {
        let a = r.call(&ticket);
        assert_eq!(a.status, 200, "call {n}: {}", a.text());
    }
    let fourth = r.call(&ticket);
    refused_with(&fourth, "route.h.daily_cap_micro_usd");
    assert!(fourth.json()["error"]["message"].as_str().unwrap_or("").contains("route h"), "{}", fourth.text());
    assert_eq!(got(&gw).len(), 3, "the stand-in saw another count of calls");
    assert_eq!(*book.capped.lock().unwrap(), [("h".to_owned(), "route.h.daily_cap_micro_usd".to_owned())], "the run was not told its route's cap was reached");
    // each line carries the day it was sent on, the key the daemon's day figures read
    let lines = r.lines();
    assert_eq!(lines.len(), 3, "{lines:?}");
    for line in &lines {
        assert_eq!((line["route"].clone(), line["day"].clone(), line["micro_usd"].clone()), (json!("h"), json!("2026-11-28"), json!(1000)), "{line}");
    }
    // the view answers the day's figures of every route: the booked spend and the calls, and the caps where declared
    assert_eq!(daily(&r.row("h")), [json!(3000), json!(3000), json!(3), Value::Null]);
    assert_eq!(daily(&r.row("free")), [json!(0), Value::Null, json!(0), Value::Null]);
    super::openapi::check_row("Routes", &r.row("h"));
    // the catalog and the MCP tool list_routes answer the view's figures
    let catalog = r.d.catalog();
    let in_catalog = catalog[0]["routes"].as_array().unwrap().iter().find(|row| row["route"] == "h").cloned().unwrap_or_default();
    assert_eq!(daily(&in_catalog), [json!(3000), json!(3000), json!(3), Value::Null], "{in_catalog}");
    let tool = runbook::mcp::tools::call(&runbook::mcp::Local(r.d.clone()), "list_routes", &json!({})).unwrap();
    let h = tool.body["rows"].as_array().unwrap().iter().find(|row| row["route"] == "h").cloned().unwrap_or_default();
    assert_eq!(daily(&h), [json!(3000), json!(3000), json!(3), Value::Null], "{}", tool.body);
}

/// Acceptance 2: after 00:00 UTC on the test clock a call on the same route is sent, and the new day starts at 0.
#[test]
fn after_midnight_utc_on_the_daemons_clock_a_call_on_the_capped_route_is_sent_again() {
    let gw = gateway();
    let r = Rig::new("day-cap-midnight", &gw);
    let (ticket, _) = r.open("run-mid", "h");
    for _ in 0..3 {
        assert_eq!(r.call(&ticket).status, 200);
    }
    r.clock.set("2026-11-28T23:59:59.999Z");
    refused_with(&r.call(&ticket), "route.h.daily_cap_micro_usd");
    r.clock.set("2026-11-29T00:00:00Z");
    let next = r.call(&ticket);
    assert_eq!(next.status, 200, "{}", next.text());
    assert_eq!(got(&gw).len(), 4);
    assert_eq!(daily(&r.row("h")), [json!(1000), json!(3000), json!(1), Value::Null]);
    assert_eq!(r.lines().last().map(|l| l["day"].clone()), Some(json!("2026-11-29")));
}

/// Acceptance 3 and the no-price rule: a local route with `daily_max_llm_calls = 2` refuses the third call of the day; a
/// hosted route with no price is counted by its calls, and its booked figure is what the ledger booked (the reservation).
#[test]
fn a_local_route_with_daily_max_llm_calls_2_refuses_the_third_call_and_a_route_without_a_price_is_counted_by_its_calls() {
    let gw = gateway();
    let r = Rig::new("day-cap-local", &gw);
    let (local, book) = r.open("run-l", "l");
    assert_eq!(r.call(&local).status, 200);
    assert_eq!(r.call(&local).status, 200);
    refused_with(&r.call(&local), "route.l.daily_max_llm_calls");
    assert_eq!(got(&gw).len(), 2);
    assert_eq!(*book.capped.lock().unwrap(), [("l".to_owned(), "route.l.daily_max_llm_calls".to_owned())]);
    assert_eq!(daily(&r.row("l")), [json!(0), Value::Null, json!(2), json!(2)], "a local route without prices books 0");
    let (unpriced, _) = r.open("run-n", "n");
    assert_eq!(r.call(&unpriced).status, 200);
    refused_with(&r.call(&unpriced), "route.n.daily_max_llm_calls");
    assert_eq!(got(&gw).len(), 3);
    assert_eq!(daily(&r.row("n")), [json!(7000), Value::Null, json!(1), json!(1)], "a call of unknown cost books its reservation");
    // a managed route takes both keys: the first figure reached refuses
    let (managed, _) = r.open("run-m", "m");
    for _ in 0..4 {
        assert_eq!(r.call(&managed).status, 200);
    }
    refused_with(&r.call(&managed), "route.m.daily_max_llm_calls");
    assert_eq!(daily(&r.row("m")), [json!(4000), json!(5000), json!(4), json!(4)]);
}

/// Acceptance 4: two runs on one route share its day figure.
#[test]
fn two_runs_on_one_route_share_its_day_figure() {
    let gw = gateway();
    let r = Rig::new("day-cap-shared", &gw);
    let (a, book_a) = r.open("run-a", "h");
    let (b, book_b) = r.open("run-b", "h");
    assert_eq!(r.call(&a).status, 200);
    assert_eq!(r.call(&a).status, 200);
    assert_eq!(r.call(&b).status, 200);
    refused_with(&r.call(&b), "route.h.daily_cap_micro_usd");
    refused_with(&r.call(&a), "route.h.daily_cap_micro_usd");
    assert_eq!(got(&gw).len(), 3);
    assert_eq!((book_a.capped.lock().unwrap().len(), book_b.capped.lock().unwrap().len()), (1, 1));
    // another route's figure is its own
    let (free, _) = r.open("run-free", "free");
    assert_eq!(r.call(&free).status, 200);
}

/// Acceptance 5: a daemon started again the same day keeps the figure, rebuilt from the spend ledger; the next day of
/// its clock starts at 0.
#[test]
fn a_daemon_started_again_the_same_day_keeps_the_routes_figure_rebuilt_from_the_ledger() {
    let gw = gateway();
    let r = Rig::new("day-cap-restart", &gw);
    let (ticket, _) = r.open("run-1", "h");
    for _ in 0..3 {
        assert_eq!(r.call(&ticket).status, 200);
    }
    let (local, _) = r.open("run-2", "l");
    assert_eq!(r.call(&local).status, 200);
    drop(ticket);
    drop(local);
    let (dir, clock) = (r.dir.clone(), r.clock.clone());
    r.d.stop();
    drop(r);
    clock.set("2026-11-28T18:00:00Z");
    let again = Rig::on(dir.clone(), &gw, clock.clone());
    assert_eq!(daily(&again.row("h")), [json!(3000), json!(3000), json!(3), Value::Null]);
    assert_eq!(daily(&again.row("l")), [json!(0), Value::Null, json!(1), json!(2)]);
    let (ticket, _) = again.open("run-3", "h");
    refused_with(&again.call(&ticket), "route.h.daily_cap_micro_usd");
    assert_eq!(got(&gw).len(), 4, "a daemon started again sent a call past the day's cap");
    drop(ticket);
    again.d.stop();
    drop(again);
    clock.set("2026-11-29T00:00:01Z");
    let next_day = Rig::on(dir, &gw, clock);
    assert_eq!(daily(&next_day.row("h")), [json!(0), json!(3000), json!(0), Value::Null]);
}

/// `runbook serve` on `file`: its status and standard error, and whether it wrote its state directory.
fn serve_refused(tag: &str, file: &str) -> (Option<i32>, String, bool) {
    let dir = super::state::scratch(tag);
    std::fs::write(dir.join("runbook.toml"), file).unwrap();
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_runbook"));
    cmd.arg("serve").args(["--principal", super::PRINCIPAL, "--listen", "127.0.0.1:0"]).arg("--state-dir").arg(dir.join("state"));
    cmd.env_remove("RUNBOOK_TOKEN").env("RUNBOOK_CONFIG", dir.join("runbook.toml"));
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let (status, _, err) = super::http::exits_within(cmd, Duration::from_secs(10));
    (status, err, dir.join("state").exists())
}

/// Acceptance 6 and the keys at start: `wake_daily_cap_micro_usd` on a hosted route is still refused, and
/// `daily_cap_micro_usd` on it is taken; 0, a number that is not whole and a text are refused at start by name, and so is
/// `daily_cap_micro_usd` on a route that declares no price.
#[test]
fn the_daily_keys_are_taken_on_every_kind_and_0_a_non_integer_or_a_spend_cap_without_a_price_is_refused_at_start_by_name() {
    let route = |kind: &str, extra: &str| format!("[gateways.gw]\nbase_url = \"http://127.0.0.1:9\"\n[routes.r1]\ngateway = \"gw\"\nmodel = \"m\"\nkind = \"{kind}\"\n{extra}");
    let priced = "price_in_micro_usd_per_mtok = 1000000\n";
    let cases = [
        (route("hosted", &format!("{priced}wake_daily_cap_micro_usd = 3000\n")), "routes.r1.wake_daily_cap_micro_usd", "a key of a managed route only"),
        (route("hosted", &format!("{priced}daily_cap_micro_usd = 0\n")), "routes.r1.daily_cap_micro_usd", "above 0"),
        (route("local", "daily_max_llm_calls = 0\n"), "routes.r1.daily_max_llm_calls", "above 0"),
        (route("hosted", &format!("{priced}daily_cap_micro_usd = 1.5\n")), "routes.r1.daily_cap_micro_usd", "whole number"),
        (route("managed", "daily_max_llm_calls = 2.0\n"), "routes.r1.daily_max_llm_calls", "whole number"),
        (route("hosted", &format!("{priced}daily_cap_micro_usd = \"3000\"\n")), "routes.r1.daily_cap_micro_usd", "whole number"),
        (route("local", "daily_max_llm_calls = -1\n"), "routes.r1.daily_max_llm_calls", "whole number"),
        (route("hosted", "daily_cap_micro_usd = 3000\n"), "routes.r1.daily_cap_micro_usd", "declares no price"),
        (route("local", "daily_cap_micro_usd = 3000\n"), "routes.r1.daily_cap_micro_usd", "daily_max_llm_calls"),
    ];
    for (n, (file, key, why)) in cases.iter().enumerate() {
        let (status, err, wrote) = serve_refused(&format!("day-cap-refused-{n}"), file);
        assert_eq!(status, Some(2), "case {n}: the daemon started: {err}");
        assert!(err.contains(key) && err.contains(why), "case {n}: the refusal does not name {key} and {why}: {err}");
        assert!(!wrote, "case {n}: the state directory was written");
    }
    // taken: the spend cap on a priced route of each kind, the call cap on any route
    let gateways = json!({"gw": {"base_url": "http://127.0.0.1:9"}});
    for (kind, extra) in [("hosted", json!({"price_in_micro_usd_per_mtok": 1, "daily_cap_micro_usd": 3000})), ("local", json!({"price_out_micro_usd_per_mtok": 1, "daily_cap_micro_usd": 1})), ("managed", json!({"price_in_micro_usd_per_mtok": 1, "daily_cap_micro_usd": 3000, "daily_max_llm_calls": 9})), ("hosted", json!({"daily_max_llm_calls": 1}))] {
        let mut r1 = json!({"gateway": "gw", "model": "m", "kind": kind});
        r1.as_object_mut().unwrap().extend(extra.as_object().cloned().unwrap());
        let routes = json!({"r1": r1});
        assert!(RouteTable::declare(&Tables { gateways: Some(&gateways), routes: Some(&routes), ..Tables::default() }).is_ok(), "{kind} {extra}");
    }
}

/// The OpenAPI document declares the four figures as optional integers of a row of the view `routes` and of a route of
/// the catalog.
#[test]
fn the_openapi_document_declares_the_daily_figures_of_a_route_row_and_of_a_catalog_route() {
    let doc = super::openapi::doc();
    for schema in ["sdc.daemon.Routes.Row", "sdc.daemon.CatalogRoute"] {
        let s = &doc["components"]["schemas"][schema];
        for member in DAILY {
            assert_eq!(s["properties"][member]["type"], "integer", "{schema}.{member}: {}", s["properties"][member]);
        }
        let required: Vec<&str> = s["required"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        assert!(DAILY.iter().all(|k| !required.contains(k)), "{schema} requires a figure: {required:?}");
    }
}

/// The program of `notes.write`: one Messages call for the role `default` to the relay its route file names; it writes
/// the status it got to `log/program-statuses` and the notes, whatever the answer.
fn program_calling(log: &Path) -> String {
    format!(
        r#"#!/bin/bash
log='{log}'
f=${{RUNBOOK_ROUTE_FILE-}}
[ -n "$f" ] || exit 9
base=$(jq -r .base_url "$f") && token=$(jq -r .token "$f") || exit 9
rest=${{base#http://}}
hostport=${{rest%%/*}}
path=/${{rest#*/}}
body='{{"model":"default","max_tokens":8,"messages":[{{"role":"user","content":"hi"}}]}}'
exec 3<>"/dev/tcp/${{hostport%:*}}/${{hostport##*:}}" || exit 7
printf 'POST %s/v1/messages HTTP/1.0\r\nHost: %s\r\nAuthorization: Bearer %s\r\nContent-Type: application/json\r\nContent-Length: %s\r\n\r\n%s' "$path" "$hostport" "$token" "${{#body}}" "$body" >&3
answer=$(cat <&3)
exec 3<&-
printf '%s\n' "${{answer%%$'\r'*}}" >> "$log/program-statuses"
echo notes >> notes.txt
"#,
        log = log.display()
    )
}

fn submit_rule(l: &Live) -> String {
    let (status, body) = post(l, &json!({"runbook_id": "autonomy-demo", "intake": "a note on tea", "confinement": "local", "autonomy": "A5", "decider": "rule", "model_route": "a"}));
    assert_eq!((status, body["outcome"].clone()), (202, json!("submitted")), "{body}");
    body["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned()
}

fn statuses(h: &Home) -> Vec<String> {
    std::fs::read_to_string(h.log.join("program-statuses")).unwrap_or_default().lines().map(str::to_owned).collect()
}

/// The run at the route's cap ends as a run at a cap ends (`stopped: llm calls` for the calls, `stopped: spend cap` for
/// the spend), and its event `runbook.cap-reached` names the route and the key.
#[test]
fn a_run_whose_call_the_routes_daily_cap_refused_ends_as_a_run_at_a_cap_naming_the_route_and_the_key() {
    let h = home("day-cap-run");
    write_exe(&h.home.join("bindings/tools/notes-write.sh"), &program_calling(&h.log));
    let body = r#"{"id":"m","usage":{"input_tokens":7,"output_tokens":0}}"#;
    let answer = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).into_bytes();
    let gw = gateway_answering(move |_| answer.clone(), &["alias-a"]);
    let tf = h.token_file.display();
    let file = format!(
        "[gateways.one]\nbase_url = \"{}\"\ntoken_file = \"{tf}\"\n[routes.a]\ngateway = \"one\"\nmodel = \"alias-a\"\nkind = \"hosted\"\nprice_in_micro_usd_per_mtok = 1000000\ndaily_max_llm_calls = 1\n[deciders.rule]\nverdict = \"approve\"\n",
        gw.base_url
    );
    let l = serve("day-cap-run-d", &h, &file, true);
    let first = submit_rule(&l);
    let end = until_state(&l, &first, &["Ended", "FailedToStart", "Cancelled"]);
    assert_eq!(end["state"], "Ended", "{end}");
    assert_ne!(end["outcome"], "stopped: llm calls", "{end}");
    let second = submit_rule(&l);
    let end = until_state(&l, &second, &["Ended", "FailedToStart", "Cancelled"]);
    assert_eq!((end["state"].clone(), end["outcome"].clone()), (json!("Ended"), json!("stopped: llm calls")), "{end}");
    let s = statuses(&h);
    assert!(s.len() == 2 && s[0].contains(" 200 ") && s[1].contains(" 429 "), "{s:?}");
    assert_eq!(got(&gw).len(), 1, "the call past the route's cap reached the gateway");
    let capped: Vec<Value> = rows(&l, &format!("/daemon/views/events?run_id={second}&after_position=0")).into_iter().filter(|e| e["kind"] == "runbook.cap-reached").map(|e| e["record"].clone()).collect();
    assert_eq!(capped.len(), 1, "{capped:?}");
    assert_eq!((capped[0]["cap"].clone(), capped[0]["route"].clone(), capped[0]["cap_key"].clone()), (json!("llm calls"), json!("a"), json!("route.a.daily_max_llm_calls")), "{}", capped[0]);
    let row = rows(&l, "/daemon/views/routes").into_iter().find(|r| r["route"] == "a").unwrap_or_default();
    assert_eq!(daily(&row), [json!(7), Value::Null, json!(1), json!(1)], "{row}");
}

/// The run's side of the relay with a cap of calls of its own: the calls past it are refused by the run.
struct Capped {
    left: Mutex<u64>,
}

impl Booking for Capped {
    fn admit(&self) -> Result<(), String> {
        let mut left = self.left.lock().unwrap();
        if *left == 0 {
            return Err("llm calls: the run's cap".to_owned());
        }
        *left -= 1;
        Ok(())
    }
    fn reservation_micro_usd(&self) -> u64 {
        7_000
    }
    fn step(&self) -> Option<String> {
        None
    }
    fn book(&self, _line: Value, _micro_usd: u64) -> Result<(), String> {
        Ok(())
    }
}

/// A call the run's own caps refuse was never sent: it gives back the count its route's day took for it, so another
/// run on the route still has the day's calls.
#[test]
fn a_call_the_runs_own_cap_refuses_gives_back_its_count_of_the_routes_day() {
    let gw = gateway();
    let r = Rig::new("day-cap-run-refused", &gw);
    let resolved = ResolvedRoute::from(r.table.resolve("l").unwrap());
    let ticket = r.relay.open("run-out", runbook::relay::RouteTable::default().with("default", resolved, "run")).unwrap();
    ticket.take_calls(Arc::new(Capped { left: Mutex::new(0) }));
    for _ in 0..3 {
        let refused = r.call(&ticket);
        assert_eq!(refused.status, 429, "{}", refused.text());
        assert!(refused.json()["error"].get("cap_key").is_none(), "the run's cap is no route's: {}", refused.text());
    }
    assert_eq!(daily(&r.row("l")), [json!(0), Value::Null, json!(0), json!(2)], "a call the run refused counts on its route");
    let (other, _) = r.open("run-in", "l");
    assert_eq!(r.call(&other).status, 200);
    assert_eq!(r.call(&other).status, 200);
    refused_with(&r.call(&other), "route.l.daily_max_llm_calls");
    assert_eq!(got(&gw).len(), 2);
}

/// A ledger the daemon cannot read when it starts leaves the day's figures unknown: a call on a route with a daily cap
/// is refused naming its key and the cause, and a route without one still sends.
#[test]
fn a_ledger_that_cannot_be_read_at_start_refuses_the_calls_of_a_capped_route_and_none_other() {
    let gw = gateway();
    let dir = super::state::scratch("day-cap-broken-ledger");
    std::fs::create_dir_all(dir.join("state/ledgers")).unwrap();
    std::fs::write(dir.join("state/ledgers/other.json"), "{not a ledger").unwrap();
    let r = Rig::on(dir, &gw, Arc::new(SetClock::at(MORNING).unwrap()));
    let (capped, book) = r.open("run-c", "h");
    let refused = r.call(&capped);
    refused_with(&refused, "route.h.daily_cap_micro_usd");
    assert!(refused.text().contains("could not be rebuilt"), "{}", refused.text());
    assert_eq!(book.capped.lock().unwrap().len(), 1);
    let (free, _) = r.open("run-f", "free");
    assert_eq!(r.call(&free).status, 200);
    assert_eq!(got(&gw).len(), 1);
}
