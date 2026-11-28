//! The catalog (`docs/model-routes.md`, section 10): `GET /daemon/views/catalog`, the MCP tool `describe_catalog` and
//! `runbook catalog` list what a delegation may name: the declared model routes (kind, alias, wires, status, phase,
//! prices), the runbooks (purpose, inputs, the admission of their route entries, `bindings/<runbook>.routes.yaml`) and
//! the terms of a delegation (its members, its paths, the default and classifier routes).
//!
//! The daemon is `runbook serve` with the Loom runtime on a runbook home of the test's own; no run is started. Its
//! gateway is a stand-in on a loopback port of the test's own that answers `GET` requests from a table: no model is
//! called and nothing leaves loopback.

use super::default_confinement::{live, rows, Live};
use super::model_routes::{token_file, Standin, TOKEN};
use super::openapi;
use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_runbook");

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A runbook of the home: its protocol (`description`), its bindings (placing the idea when `idea`) and, when given, its
/// route entries file.
fn runbook(home: &Path, name: &str, revision: u64, description: &str, idea: bool, routes: Option<&str>) {
    write(&home.join(format!("protocol/{name}.protocol.yaml")), &format!("format: protocol/1\nprotocol:\n  id: {name}\n  revision: {revision}\n  description: >-\n    {description}\n"));
    let artifacts = if idea { "artifacts:\n  idea: idea.yaml\n  result: result.md\n" } else { "artifacts:\n  result: result.md\n" };
    write(&home.join(format!("bindings/{name}.yaml")), &format!("format: loom.bindings/1\nprotocol: {name}@{revision}\nprogram_root: bindings\n{artifacts}actions: []\n"));
    if let Some(text) = routes {
        write(&home.join(format!("bindings/{name}.routes.yaml")), text);
    }
}

/// Today's gateway with the context length and the tool calling of its two models: the hosted `claude` (messages) and
/// the managed `qwen` (chat, responses), whose pod is stopped.
fn gateway() -> Standin {
    Standin::start(vec![
        ("GET /ready", 200, json!({"status": "ready"})),
        (
            "GET /v1/models",
            200,
            json!({"object": "list", "data": [
                {"id": "claude", "object": "model", "wires": ["messages"], "max_model_len": 200000, "tool_calling": true},
                {"id": "qwen", "object": "model", "wires": ["chat", "responses"], "max_model_len": 32768},
            ]}),
        ),
        ("GET /v1/models/qwen/status", 200, json!({"model": "qwen", "phase": "stopped"})),
    ])
}

/// The daemon's file: the gateway `default` with a token file, the routes `qwen` (managed, free, a wait) and `claude`
/// (hosted, priced), and the policy that names `claude` for both.
fn file(base_url: &str, token_file: &Path) -> String {
    format!(
        "[gateways.default]\nbase_url = \"{base_url}\"\ntoken_file = \"{}\"\n\n[routes.qwen]\ngateway = \"default\"\nmodel = \"qwen\"\nkind = \"managed\"\nstart_wait_seconds = 600\nprice_in_micro_usd_per_mtok = 0\nprice_out_micro_usd_per_mtok = 0\n\n[routes.claude]\ngateway = \"default\"\nmodel = \"claude\"\nkind = \"hosted\"\nprice_in_micro_usd_per_mtok = 3000000\nprice_out_micro_usd_per_mtok = 15000000\n\n[route_policy]\ndefault = \"claude\"\nclassifier = \"claude\"\n",
        token_file.display()
    )
}

/// `runbook serve --runtime loom` on the home `home` with the file of [`file`] and the caps a loom daemon needs.
fn daemon(tag: &str, gw: &Standin, home: &Path) -> Live {
    let dir = super::state::scratch(&format!("{tag}-files"));
    let args = ["--runtime", "loom", "--runbook-home", home.to_str().unwrap(), "--min-confinement", "local", "--default-confinement", "local", "--max-wall-seconds", "3600", "--max-llm-calls", "100", "--cap-micro-usd", "1000000000"];
    live(tag, &args, Some(&file(&gw.base_url, &token_file(&dir))))
}

/// The one row of the view, checked against the document, with what must never be in it.
fn catalog(l: &Live, gw: &Standin, home: &Path) -> Value {
    let rows = rows(l, "/daemon/views/catalog");
    assert_eq!(rows.len(), 1, "{rows:?}");
    let row = rows[0].clone();
    openapi::check_row("Catalog", &row);
    for r in row["routes"].as_array().unwrap() {
        members_of("CatalogRoute", r);
    }
    for b in row["runbooks"].as_array().unwrap() {
        members_of("CatalogRunbook", b);
        b["route_entries"].as_array().unwrap().iter().for_each(|e| members_of("RouteEntry", e));
    }
    members_of("DelegationTerms", &row["delegation"]);
    row["delegation"]["members"].as_array().unwrap().iter().for_each(|m| members_of("DelegateMember", m));
    let text = row.to_string();
    for value in [TOKEN, gw.base_url.as_str(), "127.0.0.1", "gateway-token", home.to_str().unwrap(), l.dir.to_str().unwrap()] {
        assert!(!text.contains(value), "{value} is in the catalog: {text}");
    }
    row
}

/// A member of the row is the document's struct `name`: it has every required member and no other.
fn members_of(name: &str, value: &Value) {
    let s = &openapi::doc()["components"]["schemas"][format!("sdc.daemon.{name}")];
    let props = s["properties"].as_object().unwrap_or_else(|| panic!("no schema {name}"));
    let obj = value.as_object().unwrap_or_else(|| panic!("{name}: not an object: {value}"));
    for r in s["required"].as_array().into_iter().flatten() {
        assert!(obj.contains_key(r.as_str().unwrap()), "{name}: missing {r} in {value}");
    }
    for k in obj.keys() {
        assert!(props.contains_key(k), "{name}: the document has no member {k}: {value}");
    }
}

/// A row without its instant of the read, which every route has.
fn without_read_at(route: &Value) -> Value {
    let mut r = route.clone();
    assert!(r["read_at"].as_str().is_some_and(|t| t.ends_with('Z')), "a route has no instant of its read: {r}");
    r.as_object_mut().unwrap().remove("read_at");
    r
}

#[test]
fn the_catalog_of_a_daemon_with_two_routes_and_two_runbooks_lists_both_of_each_with_their_fields() {
    let gw = gateway();
    let home = super::state::scratch("catalog-two-home");
    runbook(
        &home,
        "alpha",
        3,
        "Answer a question in one file. The answer is written to result.md.",
        true,
        Some("format: runbook.routes/1\nmodel_routes:\n  writer: {route: qwen}\n  review: {needs: {wire: messages, min_context: 100000, tool_calling: true}}\n"),
    );
    runbook(&home, "beta", 1, "Review a change", false, None);
    let l = daemon("catalog-two", &gw, &home);
    let row = catalog(&l, &gw, &home);

    let routes: Vec<Value> = row["routes"].as_array().unwrap().iter().map(without_read_at).collect();
    assert_eq!(
        routes,
        vec![
            json!({"route": "claude", "gateway": "default", "alias": "claude", "kind": "hosted", "wires": ["messages"], "status": "listed", "max_model_len": 200000, "tool_calling": true, "price_in_micro_usd_per_mtok": 3000000, "price_out_micro_usd_per_mtok": 15000000, "daily_booked_micro_usd": 0, "daily_llm_calls": 0}),
            json!({"route": "qwen", "gateway": "default", "alias": "qwen", "kind": "managed", "wires": ["chat", "responses"], "status": "not-ready", "phase": "stopped", "max_model_len": 32768, "price_in_micro_usd_per_mtok": 0, "price_out_micro_usd_per_mtok": 0, "start_wait_seconds": 600, "daily_booked_micro_usd": 0, "daily_llm_calls": 0}),
        ]
    );

    assert_eq!(
        row["runbooks"],
        json!([
            {
                "runbook_id": "alpha",
                "runbook_version": "3",
                "title": "Answer a question in one file",
                "description": "Answer a question in one file. The answer is written to result.md.",
                "inputs": ["idea"],
                "route_entries": [
                    {"role": "review", "route": "claude", "needs": {"wire": "messages", "min_context": 100000, "tool_calling": true}, "admitted": true},
                    {"role": "writer", "route": "qwen", "admitted": true},
                ],
                "admitted": true,
            },
            {"runbook_id": "beta", "runbook_version": "1", "title": "Review a change", "description": "Review a change", "inputs": [], "route_entries": [], "admitted": true},
        ])
    );

    let d = &row["delegation"];
    let members: Vec<(String, bool)> = d["members"].as_array().unwrap().iter().map(|m| (m["name"].as_str().unwrap().to_owned(), m["required"].as_bool().unwrap())).collect();
    assert_eq!(
        members,
        [("request", true), ("model_route", false), ("protocol", false), ("workspace", false), ("workspace_exclude", false), ("settings", false), ("session_id", false), ("idempotency_key", false), ("serial_key", false), ("repository", false)].map(|(n, r)| (n.to_owned(), r)),
        "{d}"
    );
    let key = d["members"].as_array().unwrap().iter().find(|m| m["name"] == "idempotency_key").unwrap();
    assert_eq!(key["max_length"], 200, "{key}");
    let serial = d["members"].as_array().unwrap().iter().find(|m| m["name"] == "serial_key").unwrap();
    assert_eq!(serial["max_length"], 200, "{serial}");
    assert!(d["members"][0]["summary"].as_str().is_some_and(|s| s.contains("1 MiB")), "{d}");
    assert_eq!(
        (d["paths"].clone(), d["default_route"].clone(), d["classifier_route"].clone(), d["confinement"].clone(), d["request_limit"].clone()),
        (json!(["named", "free", "classified", "generated"]), json!("claude"), json!("claude"), json!("local"), json!(1048576)),
        "{d}"
    );

    // the MCP tool answers the same row
    let mcp = runbook::mcp::remote::Remote::parse(&format!("http://{}", l.addr), None).unwrap();
    let tool = runbook::mcp::tools::call(&mcp, "describe_catalog", &json!({})).unwrap();
    assert_eq!(tool.status, 200);
    assert_eq!(tool.body["rows"], json!([row]));
    assert!(runbook::mcp::tools::call(&mcp, "describe_catalog", &json!({"route": "qwen"})).is_err(), "describe_catalog takes no argument");
}

#[test]
fn a_runbook_whose_route_entries_cannot_be_admitted_is_listed_with_the_reason() {
    let gw = gateway();
    let home = super::state::scratch("catalog-refused-home");
    // a route the daemon does not declare, needs no route meets, and a named route outside routes_allowed
    runbook(
        &home,
        "gamma",
        2,
        "Write a report",
        true,
        Some("format: runbook.routes/1\nmodel_routes:\n  writer: {route: gpt}\n  review: {needs: {wire: embeddings}}\n  helper: {route: claude}\n  reader: {needs: {wire: chat}}\nroutes_allowed: [qwen]\n"),
    );
    // a file that is not one of route entries
    runbook(&home, "delta", 1, "Sort the inbox", true, Some("format: loom.bindings/1\nmodel_routes: []\n"));
    runbook(&home, "epsilon", 1, "Plan a release", true, None);
    let l = daemon("catalog-refused", &gw, &home);
    let row = catalog(&l, &gw, &home);
    let by = |id: &str| row["runbooks"].as_array().unwrap().iter().find(|r| r["runbook_id"] == id).unwrap_or_else(|| panic!("{id} is not listed: {row}")).clone();

    let gamma = by("gamma");
    assert_eq!(gamma["admitted"], false, "{gamma}");
    assert_eq!(gamma["routes_allowed"], json!(["qwen"]), "{gamma}");
    let entry = |role: &str| gamma["route_entries"].as_array().unwrap().iter().find(|e| e["role"] == role).unwrap().clone();
    let reason = |role: &str| entry(role)["reason"].as_str().unwrap_or("").to_owned();
    assert_eq!(gamma["route_entries"].as_array().unwrap().iter().map(|e| e["role"].as_str().unwrap()).collect::<Vec<_>>(), ["helper", "reader", "review", "writer"]);
    assert_eq!((entry("writer")["admitted"].clone(), entry("writer").get("route").cloned()), (json!(false), None), "{gamma}");
    assert!(reason("writer").contains("writer") && reason("writer").contains("gpt") && reason("writer").contains("declared: claude, qwen"), "{gamma}");
    assert_eq!(entry("review")["admitted"], false, "{gamma}");
    assert!(reason("review").contains("review") && reason("review").contains("wire embeddings"), "{gamma}");
    assert_eq!(entry("helper")["admitted"], false, "{gamma}");
    assert!(reason("helper").contains("claude") && reason("helper").contains("routes_allowed"), "{gamma}");
    assert_eq!((entry("reader")["admitted"].clone(), entry("reader")["route"].clone()), (json!(true), json!("qwen")), "the default route is not allowed, the one allowed route meets the needs: {gamma}");
    let refusal = gamma["refusal"].as_str().unwrap();
    for role in ["writer", "review", "helper"] {
        assert!(refusal.contains(&reason(role)), "the refusal names every reason: {refusal}");
    }

    let delta = by("delta");
    assert_eq!((delta["admitted"].clone(), delta["route_entries"].clone()), (json!(false), json!([])), "{delta}");
    assert!(delta["refusal"].as_str().is_some_and(|r| r.contains("bindings/delta.routes.yaml") && r.contains("runbook.routes/1")), "{delta}");

    let epsilon = by("epsilon");
    assert_eq!((epsilon["admitted"].clone(), epsilon.get("refusal").cloned()), (json!(true), None), "{epsilon}");

    // the command line prints the same: its JSON is the view's, its text names each runbook's refusal
    let cli = |json: bool| {
        let mut c = Command::new(BIN);
        c.args(["catalog", "--daemon", &format!("http://{}", l.addr)]).env("RUNBOOK_NO_CONFIG", "1").env_remove("RUNBOOK_DAEMON").env_remove("RUNBOOK_TOKEN_FILE").stdin(Stdio::null());
        if json {
            c.arg("--json");
        }
        let o = c.output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap()
    };
    let printed: Value = serde_json::from_str(&cli(true)).unwrap();
    assert_eq!(printed["rows"], json!([row]));
    let text = cli(false);
    for line in ["route claude", "route qwen", "runbook epsilon", "admitted", refusal, delta["refusal"].as_str().unwrap()] {
        assert!(text.contains(line), "{line:?} is not printed:\n{text}");
    }
}
