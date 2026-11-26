//! A runbook declares the routes it needs and the daemon admits them (`docs/model-routes.md`, section 2, D-1; story
//! `protocol-declares-the-routes-it-needs`). The runbook's file `bindings/<runbook>.routes.yaml` (`runbook.routes/1`)
//! names per role a daemon route or states requirements, and may narrow the routes that serve it in `routes_allowed`. At
//! submit (and at a delegation that names the runbook as its protocol) the daemon admits each entry: a named route must
//! be declared; requirements are met by the run's route or the daemon's default when it meets them, else by the one
//! declared route that does; a route outside `routes_allowed` is refused. The precedence is the run's word, then the
//! protocol's, then `route_policy.default`. A refusal is `422 route-refused` naming the runbook, the role and the route
//! or the requirement, and nothing is written.
//!
//! No model is called and nothing leaves loopback: the gateways are stand-ins on ports of the test's own, the model
//! decider's adapter is the committed claude-cli adapter and `claude` a stand-in that calls the relay.

use super::default_confinement::{live_env, post, rows, Live};
use super::gateway::{claude_calling, claude_cli, entries, home, model_table, until_state, write_exe, Home};
use super::http::send;
use super::openapi;
use super::relay::{gateway_listing, got, Gateway};
use serde_json::{json, Value};
use std::path::Path;

const OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nx-llm-gateway-call: gw-call-9\r\nContent-Length: 56\r\nConnection: close\r\n\r\n{\"id\":\"m\",\"usage\":{\"input_tokens\":13,\"output_tokens\":4}}";
const RULE: &str = "[deciders.rule]\nverdict = \"approve\"\n";
const RUNBOOK: &str = "autonomy-demo";

/// A stand-in gateway that is ready, lists the aliases of the three routes (wires messages and chat, no context length)
/// and answers every call with [`OK`].
fn gateway() -> Gateway {
    gateway_listing(OK, &["alias-a", "alias-b", "alias-c"])
}

/// Three routes on two gateways: `a` (hosted, priced) and `c` (local, free) on the gateway `one`, `b` (hosted, priced) on
/// `two`, and the default route `c`. A hosted route without prices would book the reservation of each call and spend the
/// run's cap.
fn file(one: &str, two: &str, token_file: &Path) -> String {
    let tf = token_file.display();
    format!(
        "[gateways.one]\nbase_url = \"{one}\"\ntoken_file = \"{tf}\"\n[gateways.two]\nbase_url = \"{two}\"\ntoken_file = \"{tf}\"\n\
         [routes.a]\ngateway = \"one\"\nmodel = \"alias-a\"\nkind = \"hosted\"\nprice_in_micro_usd_per_mtok = 1000000\nprice_out_micro_usd_per_mtok = 2000000\n\
         [routes.b]\ngateway = \"two\"\nmodel = \"alias-b\"\nkind = \"hosted\"\nprice_in_micro_usd_per_mtok = 1000000\nprice_out_micro_usd_per_mtok = 2000000\n\
         [routes.c]\ngateway = \"one\"\nmodel = \"alias-c\"\nkind = \"local\"\n\
         [route_policy]\ndefault = \"c\"\n{}{RULE}",
        model_table(Some(&claude_cli()))
    )
}

/// The daemon on the home `h` at the default level `local`, so it takes a delegation too.
fn serve(tag: &str, h: &Home, one: &Gateway, two: &Gateway) -> Live {
    let args = ["--runtime", "loom", "--runbook-home", h.home.to_str().unwrap(), "--min-confinement", "local", "--default-confinement", "local", "--max-wall-seconds", "3600", "--max-llm-calls", "10000", "--cap-micro-usd", "1000000000"];
    live_env(tag, &args, Some(&file(&one.base_url, &two.base_url, &h.token_file)), &[("XDG_RUNTIME_DIR", Some(h.xdg.to_str().unwrap()))])
}

/// The runbook's route entries file: `text`, written over the last one (the daemon reads it at each submit).
fn routes(h: &Home, text: &str) {
    std::fs::write(h.home.join(format!("bindings/{RUNBOOK}.routes.yaml")), format!("format: runbook.routes/1\n{text}")).unwrap();
}

fn submit_with(l: &Live, extra: Value) -> (u16, Value) {
    let mut body = json!({"runbook_id": RUNBOOK, "intake": "a note on tea", "confinement": "local", "autonomy": "A5", "decider": "model"});
    for (k, v) in extra.as_object().unwrap() {
        body[k] = v.clone();
    }
    post(l, &body)
}

fn submitted(l: &Live, extra: Value) -> String {
    let (status, body) = submit_with(l, extra.clone());
    assert_eq!((status, body["outcome"].clone()), (202, json!("submitted")), "{extra}: {body}");
    body["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned()
}

/// The submit is refused `422 route-refused` listing the declared routes, with a reason that names the runbook and each
/// of `says`; the reason.
fn refused(l: &Live, extra: Value, says: &[&str]) -> String {
    let (status, body) = submit_with(l, extra.clone());
    refusal(&extra, status, &body, says)
}

fn refusal(asked: &Value, status: u16, body: &Value, says: &[&str]) -> String {
    assert_eq!((status, body["outcome"].clone(), body["error"].clone()), (422, json!("route-refused"), json!("sdc.daemon.RouteRefused")), "{asked}: {body}");
    assert_eq!(body["payload"]["declared"], json!(["a", "b", "c"]), "{asked}: {body}");
    let reason = body["payload"]["reason"].as_str().unwrap_or_default().to_owned();
    for word in std::iter::once(&RUNBOOK).chain(says) {
        assert!(reason.contains(word), "{asked}: the reason does not say {word:?}: {reason}");
    }
    reason
}

fn row(l: &Live, run: &str) -> Value {
    rows(l, &format!("/daemon/views/run?run_id={run}"))[0].clone()
}

fn models(g: &Gateway) -> Vec<String> {
    got(g).iter().map(|x| x.body["model"].as_str().unwrap_or("").to_owned()).collect()
}

/// The run ended (its calls are all made).
fn ended(l: &Live, run: &str) {
    let end = until_state(l, run, &["Ended", "FailedToStart", "Cancelled"]);
    assert_eq!(end["state"], "Ended", "{end}");
}

fn runs(l: &Live) -> usize {
    rows(l, "/daemon/views/runs").len()
}

#[test]
fn a_role_naming_a_declared_route_is_admitted_and_reaches_it_and_an_undeclared_one_is_refused_naming_runbook_and_role() {
    let h = home("pr-named");
    write_exe(&h.home.join("bindings/tools/claude"), &claude_calling(&h.log, r#"writer "$model""#));
    let (one, two) = (gateway(), gateway());
    routes(&h, "model_routes:\n  writer: {route: b}\n");
    let l = serve("pr-named-d", &h, &one, &two);

    // the run names no route: the role writer takes the protocol's route b, the role default the daemon's default c
    let run = submitted(&l, json!({}));
    ended(&l, &run);
    let (on_one, on_two) = (models(&one), models(&two));
    assert!(!on_two.is_empty() && on_two.iter().all(|m| m == "alias-b") && on_one.len() == on_two.len() && on_one.iter().all(|m| m == "alias-c"), "{on_one:?} {on_two:?}");
    let r = row(&l, &run);
    assert_eq!((r["model_route"].clone(), r["model_route_source"].clone(), r["model_routes"].clone()), (json!("c"), json!("default"), json!({"writer": "b"})), "{r}");
    let lines: Vec<Value> = rows(&l, &format!("/daemon/views/events?run_id={run}&after_position=0")).into_iter().filter(|e| e["kind"] == "runbook.model-call").map(|e| e["record"].clone()).collect();
    let said: Vec<_> = lines.iter().map(|x| (x["role"].clone(), x["route"].clone(), x["source"].clone())).collect();
    let pair = [(json!("writer"), json!("b"), json!("protocol")), (json!("default"), json!("c"), json!("default"))];
    assert!(!said.is_empty() && said.chunks(2).all(|c| c == pair), "{said:?}");
    let sources = &openapi::doc()["components"]["schemas"]["sdc.daemon.RouteSource"]["enum"];
    assert_eq!(sources, &json!(["run", "protocol", "default"]), "the document names every setting that chooses a route");

    // a route the daemon does not declare is refused naming the runbook, the role and the route; nothing is written
    routes(&h, "model_routes:\n  writer: {route: gpt}\n");
    let before = (runs(&l), got(&one).len(), got(&two).len(), entries(&l.dir.join("state/runs")).len());
    refused(&l, json!({}), &["role writer", "gpt", "declared: a, b, c"]);
    refused(&l, json!({"model_route": "a"}), &["role writer", "gpt"]);
    // an entry that is not one (an address in its place) is refused by its role and never echoed
    routes(&h, "model_routes:\n  writer: {route: \"http://10.1.2.3:9/v1\"}\n");
    let reason = refused(&l, json!({}), &["role writer"]);
    assert!(!reason.contains("10.1.2.3"), "{reason}");
    assert_eq!(before, (runs(&l), got(&one).len(), got(&two).len(), entries(&l.dir.join("state/runs")).len()), "a refused submit wrote a run or called a route");

    // the run's word comes before the protocol's
    routes(&h, "model_routes:\n  writer: {route: b}\n");
    let run = submitted(&l, json!({"model_routes": {"writer": "a"}}));
    assert_eq!(row(&l, &run)["model_routes"], json!({"writer": "a"}));
}

#[test]
fn requirements_met_by_one_route_are_admitted_and_met_by_none_or_several_are_refused_naming_the_requirement() {
    let h = home("pr-needs");
    let (one, two) = (gateway(), gateway());
    let l = serve("pr-needs-d", &h, &one, &two);

    // kind hosted: a and b meet it, and the daemon's default c does not, so with no route of the run's two routes do and
    // the role is refused
    let before = (runs(&l), got(&one).len(), got(&two).len());
    routes(&h, "model_routes:\n  writer: {needs: {kind: hosted}}\n");
    refused(&l, json!({}), &["role writer", "several routes meet kind hosted", "a, b"]);

    // none meets kind managed, nor a context length the gateway does not list
    routes(&h, "model_routes:\n  writer: {needs: {kind: managed}}\n");
    refused(&l, json!({}), &["role writer", "kind managed"]);
    routes(&h, "model_routes:\n  writer: {needs: {wire: messages, min_context: 100000}}\n");
    refused(&l, json!({}), &["role writer", "min_context 100000"]);
    // a route the run names for the role must meet its requirements
    routes(&h, "model_routes:\n  writer: {needs: {kind: local}}\n");
    refused(&l, json!({"model_routes": {"writer": "b"}}), &["role writer", "the route b", "kind local"]);
    assert_eq!(before, (runs(&l), got(&one).len(), got(&two).len()), "a refused submit wrote a run or called a route");

    // kind hosted with the run's route b: b meets it and serves the role
    routes(&h, "model_routes:\n  writer: {needs: {kind: hosted}}\n");
    let run = submitted(&l, json!({"model_route": "b"}));
    assert_eq!(row(&l, &run)["model_routes"], json!({"writer": "b"}));
    // kind local: only c meets it; the run's route a does not, so the one route that does serves the role
    routes(&h, "model_routes:\n  writer: {needs: {kind: local, wire: messages}}\n");
    let run = submitted(&l, json!({"model_route": "a"}));
    let r = row(&l, &run);
    assert_eq!((r["model_route"].clone(), r["model_routes"].clone()), (json!("a"), json!({"writer": "c"})), "{r}");
}

#[test]
fn a_route_outside_routes_allowed_is_refused_at_submit_and_at_a_delegation_naming_the_runbook() {
    let h = home("pr-allowed");
    let (one, two) = (gateway(), gateway());
    let l = serve("pr-allowed-d", &h, &one, &two);

    routes(&h, "routes_allowed: [a, c]\n");
    let before = (runs(&l), got(&two).len());
    refused(&l, json!({"model_route": "b"}), &["the route b", "routes_allowed (a, c)"]);
    refused(&l, json!({"model_routes": {"writer": "b"}}), &["model_routes.writer", "the route b", "routes_allowed"]);
    let (status, body) = delegate(&l, &json!({"request": "a note on tea", "protocol": RUNBOOK, "model_route": "b"}));
    refusal(&json!("delegate"), status, &body, &["the route b", "routes_allowed"]);
    // an entry that names a route outside routes_allowed is refused, whatever the run names
    routes(&h, "model_routes:\n  writer: {route: b}\nroutes_allowed: [a, c]\n");
    refused(&l, json!({"model_route": "a"}), &["role writer", "the route b", "routes_allowed"]);
    assert_eq!(before, (runs(&l), got(&two).len()), "a refused submit wrote a run or called a route");

    // the default route c is allowed: a run that names none is admitted on it
    routes(&h, "routes_allowed: [a, c]\n");
    let run = submitted(&l, json!({}));
    assert_eq!((row(&l, &run)["model_route"].clone(), row(&l, &run)["model_route_source"].clone()), (json!("c"), json!("default")));
    let (status, body) = delegate(&l, &json!({"request": "a note on tea", "protocol": RUNBOOK, "model_route": "a"}));
    assert_eq!((status, body["outcome"].clone()), (202, json!("delegated")), "{body}");

    // the default route c is not allowed: a run that names none is refused naming it and the setting that chose it
    routes(&h, "routes_allowed: [a]\n");
    let before = runs(&l);
    refused(&l, json!({}), &["the route c", "route_policy.default", "routes_allowed (a)"]);
    assert_eq!(runs(&l), before);
    let run = submitted(&l, json!({"model_route": "a"}));
    assert_eq!(row(&l, &run)["model_route"], json!("a"));
}

fn delegate(l: &Live, body: &Value) -> (u16, Value) {
    let r = send(l.addr, "POST", "/daemon/commands/delegate", &[("Content-Type", "application/json")], Some(body.to_string().as_bytes()));
    (r.status, r.json())
}
