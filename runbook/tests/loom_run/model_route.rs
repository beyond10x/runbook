//! `runbook run --model-route <name>` (`docs/model-routes.md`, section 2 and section 3, point 7): the run reads the
//! routes of its `runbook.toml`, takes the route the option names (else `route_policy.default`), and reaches that
//! route's gateway with the route's alias as the model of every model node and of a model decider. Before anything
//! is written it asks the gateway whether the route can answer (`GET /ready`, `GET /v1/models`, and the status of the
//! route's model); a route that cannot is refused naming the route, its phase and the setting that chose it, exit 2.
//! `runbook run` does not wait for a route.
//!
//! No model is called and nothing leaves loopback: the gateway is a stand-in on a port of the test's own that answers
//! the three `GET` routes and nothing else; Loom is the fake binary of the world and `sdc-node` and `claude` are
//! stand-ins.

use super::core::{code, stderr, toml_text, World};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TOKEN: &str = "gw-route-run-6a1f0c93e2";

/// A gateway on a loopback port: `/ready` answers `ready`, `/v1/models` lists `models`, and `/v1/status` answers
/// `status` (none: 404). Every request's first line is kept.
struct Standin {
    base_url: String,
    lines: Arc<Mutex<Vec<String>>>,
}

fn standin(ready: u16, models: Value, status: Option<Value>) -> Standin {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let lines = Arc::new(Mutex::new(Vec::new()));
    let kept = lines.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            let mut raw = Vec::new();
            let mut byte = [0u8; 1];
            while !raw.ends_with(b"\r\n\r\n") && raw.len() < 64 * 1024 {
                match stream.read(&mut byte) {
                    Ok(1) => raw.push(byte[0]),
                    _ => break,
                }
            }
            let head = String::from_utf8_lossy(&raw).into_owned();
            let line = head.lines().next().unwrap_or_default().to_owned();
            kept.lock().unwrap().push(line.clone());
            let (code, body) = match line.split_whitespace().nth(1).unwrap_or("") {
                "/ready" => (ready, json!({"ready": ready == 200})),
                "/v1/models" => (200, models.clone()),
                "/v1/status" => status.clone().map_or((404, json!({"error": {"code": "not-found"}})), |s| (200, s)),
                _ => (404, json!({"error": {"code": "not-found"}})),
            };
            let body = body.to_string();
            let _ = stream.write_all(format!("HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
        }
    });
    Standin { base_url, lines }
}

fn listed(alias: &str) -> Value {
    json!({"object": "list", "data": [{"id": alias, "wires": ["messages", "chat"]}]})
}

fn status_of(alias: &str, phase: &str, callable: bool) -> Value {
    json!({"format": "llm-gateway.status/1", "models": [{"model": alias, "kind": "managed", "wires": ["chat"], "phase": phase, "callable": callable}]})
}

/// A world whose `runbook.toml` declares the route `qwen` (a managed model on `gw`) and the route `claude` (hosted on
/// the same gateway), with `policy` as the body of its `[route_policy]` table (empty: none), and whose settings file
/// has a scripted decider and no login.
fn routed(tag: &str, gw: &Standin, policy: &str, settings: Value) -> World {
    let w = World::new(tag);
    w.one_decision_tree();
    let token_file = w.root.join("secret/gateway-token");
    std::fs::create_dir_all(token_file.parent().unwrap()).unwrap();
    std::fs::write(&token_file, format!("{TOKEN}\n")).unwrap();
    std::fs::set_permissions(&token_file, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut file = format!(
        "[gateways.local]\nbase_url = \"{}\"\ntoken_file = \"{}\"\n[routes.qwen]\ngateway = \"local\"\nmodel = \"qwen-alias\"\nkind = \"managed\"\n[routes.claude]\ngateway = \"local\"\nmodel = \"claude-alias\"\nkind = \"hosted\"\n",
        gw.base_url,
        token_file.display()
    );
    if !policy.is_empty() {
        file.push_str(&format!("[route_policy]\n{policy}\n"));
    }
    std::fs::write(w.root.join("runbook.toml"), file).unwrap();
    let mut v = json!({"decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [{"approve": true, "principal": "person:owner", "cost_micro_usd": 0, "model_calls": 0}]}});
    for (k, x) in settings.as_object().unwrap() {
        v[k] = x.clone();
    }
    std::fs::write(w.root.join("settings.toml"), toml_text(&v)).unwrap();
    w
}

fn args(w: &World, route: Option<&str>) -> Vec<String> {
    let mut a = w.args("r1", 1_000_000);
    a.extend(["--config".to_owned(), w.root.join("runbook.toml").display().to_string()]);
    if let Some(r) = route {
        a.extend(["--model-route".to_owned(), r.to_owned()]);
    }
    a
}

fn private_dir(w: &World) -> PathBuf {
    w.root.join("xdg").join("runbook").join("r1")
}

/// A refused run: exit 2, every word of `says` in the refusal, nothing written, Loom never called, no token shown.
fn refused(w: &World, args: &[String], says: &[&str]) {
    let o = w.run(args);
    let said = stderr(&o);
    assert_eq!(code(&o), 2, "{said}");
    for s in says {
        assert!(said.contains(s), "the refusal does not say {s:?}: {said}");
    }
    assert!(!said.contains(TOKEN) && !String::from_utf8_lossy(&o.stdout).contains(TOKEN), "{said}");
    assert!(!w.run_dir("r1").exists(), "a run directory was made");
    assert!(!private_dir(w).exists(), "a private directory was made");
    assert_eq!(w.fake.calls_so_far(), 0, "Loom was called");
}

#[test]
fn a_named_route_that_is_ready_gives_the_run_its_gateway_and_its_alias_as_the_model_of_every_model_node() {
    let gw = standin(200, listed("qwen-alias"), Some(status_of("qwen-alias", "ready", true)));
    let w = routed("route-run-ready", &gw, "", json!({}));
    let p = private_dir(&w).display().to_string();
    w.fake.hook("tree_run", "1", &format!("cat \"{p}/gateway\" > \"$D/gateway-file\"\n"));
    let o = w.run(&args(&w, Some("qwen")));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let file = std::fs::read_to_string(w.fake.dir.join("gateway-file")).expect("the run had no gateway file while it ran");
    assert!(file.lines().any(|l| l == format!("base_url={}", gw.base_url)) && file.lines().any(|l| l == format!("token={TOKEN}")), "{file}");
    let copy = std::fs::read_to_string(w.run_dir("r1/bindings/sdc.yaml")).unwrap();
    assert!(copy.contains("qwen-alias") && copy.contains("--gateway-file") && !copy.contains(TOKEN), "{copy}");
    let asked: Vec<String> = gw.lines.lock().unwrap().clone();
    assert!(asked.iter().all(|l| l.starts_with("GET ")), "the check sent more than reads: {asked:?}");
    assert!(asked.iter().any(|l| l.starts_with("GET /ready ")) && asked.iter().any(|l| l.starts_with("GET /v1/models ")), "{asked:?}");
    assert!(!String::from_utf8_lossy(&o.stdout).contains(TOKEN) && !stderr(&o).contains(TOKEN));
    let recorded: Value = serde_json::from_str(&std::fs::read_to_string(w.run_dir("r1/settings.json")).unwrap()).unwrap();
    assert_eq!(recorded["gateway"], "named", "{recorded}");
}

#[test]
fn a_managed_route_that_is_not_callable_is_refused_naming_the_route_its_phase_and_the_option_that_chose_it() {
    let gw = standin(200, listed("qwen-alias"), Some(status_of("qwen-alias", "stopped", false)));
    let w = routed("route-run-stopped", &gw, "", json!({}));
    refused(&w, &args(&w, Some("qwen")), &["qwen", "stopped", "--model-route"]);
}

#[test]
fn the_default_of_the_file_chooses_the_route_of_a_run_that_names_none_and_its_refusal_says_so() {
    let gw = standin(200, listed("qwen-alias"), Some(status_of("qwen-alias", "starting", false)));
    let w = routed("route-run-default", &gw, "default = \"qwen\"", json!({}));
    refused(&w, &args(&w, None), &["qwen", "starting", "route_policy.default"]);
}

#[test]
fn a_gateway_that_is_not_ready_or_does_not_serve_the_alias_refuses_the_run() {
    let gw = standin(503, listed("qwen-alias"), None);
    let w = routed("route-run-notready", &gw, "", json!({}));
    refused(&w, &args(&w, Some("claude")), &["claude", "gateway not ready", "--model-route"]);
    let gw = standin(200, listed("another-model"), None);
    let w = routed("route-run-unlisted", &gw, "", json!({}));
    refused(&w, &args(&w, Some("claude")), &["claude", "claude-alias", "--model-route"]);
}

#[test]
fn an_undeclared_route_and_a_route_beside_a_gateway_of_the_settings_are_refused_by_name() {
    let gw = standin(200, listed("qwen-alias"), Some(status_of("qwen-alias", "ready", true)));
    let w = routed("route-run-undeclared", &gw, "", json!({}));
    refused(&w, &args(&w, Some("nope")), &["--model-route", "claude, qwen"]);
    let w = routed("route-run-both", &gw, "", json!({"gateway": {"base_url": "http://127.0.0.1:9", "token": "settings-token-77"}}));
    refused(&w, &args(&w, Some("qwen")), &["gateway", "--model-route"]);
    let w = routed("route-run-model", &gw, "", json!({"models": {"writer": {"model": "another-model"}}}));
    refused(&w, &args(&w, Some("qwen")), &["models.writer.model", "--model-route"]);
    assert!(gw.lines.lock().unwrap().is_empty(), "a refused name or model read the gateway");
}
