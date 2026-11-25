//! A run names a model route (`docs/model-routes.md`, section 2; story `run-names-a-model-route`): a submit names a
//! route of the daemon's `runbook.toml` by name in `model_route` (and per role in `model_routes`), else the run takes
//! `route_policy.default`; the run's calls go through the daemon's relay to that route's gateway with its alias, and
//! the run's record names the route, the alias and the setting that chose it. A route that is not declared, an
//! address, a token or an alias in its place, a route for a run that makes no model call, and a model run with no
//! route on a daemon with no default are refused `422 route-refused` before anything is written, listing the declared
//! routes.
//!
//! No model is called and nothing leaves loopback: the gateways are stand-ins on ports of the test's own, the model
//! decider's adapter is the committed claude-cli adapter and `claude` a stand-in that calls the relay.

use super::default_confinement::{post, rows, Live};
use super::gateway::{claude_calling, claude_cli, entries, home, model_table, serve, until_state, wait_until, write_exe, Home, TOKEN};
use super::relay::{gateway_listing, got, Gateway};
use serde_json::{json, Value};
use std::path::Path;

const OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nx-llm-gateway-call: gw-call-9\r\nContent-Length: 56\r\nConnection: close\r\n\r\n{\"id\":\"m\",\"usage\":{\"input_tokens\":13,\"output_tokens\":4}}";
const STOPPED: &[u8] = b"HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nContent-Length: 61\r\nConnection: close\r\n\r\n{\"error\":{\"code\":\"model-stopped\",\"message\":\"the pod is off\"}}";
const RULE: &str = "[deciders.rule]\nverdict = \"approve\"\n";

/// A stand-in gateway that answers every call with `answer`, is ready and lists the aliases of the three routes: the
/// daemon asks a declared route's gateway before a run starts (story `route-readiness-before-a-run`).
fn gateway(answer: &'static [u8]) -> Gateway {
    gateway_listing(answer, &["alias-a", "alias-b", "alias-c"])
}

/// Three routes on two gateways: `a` and `c` on the gateway `one`, `b` on `two`; `policy` is the `[route_policy]`
/// table's body (empty: none).
fn routes_file(one: &str, two: &str, token_file: &Path, policy: &str) -> String {
    let tf = token_file.display();
    let mut file = format!(
        "[gateways.one]\nbase_url = \"{one}\"\ntoken_file = \"{tf}\"\n[gateways.two]\nbase_url = \"{two}\"\ntoken_file = \"{tf}\"\n\
         [routes.a]\ngateway = \"one\"\nmodel = \"alias-a\"\nkind = \"hosted\"\nprice_in_micro_usd_per_mtok = 1000000\nprice_out_micro_usd_per_mtok = 2000000\n\
         [routes.b]\ngateway = \"two\"\nmodel = \"alias-b\"\nkind = \"hosted\"\nprice_out_micro_usd_per_mtok = 1000000\n\
         [routes.c]\ngateway = \"one\"\nmodel = \"alias-c\"\nkind = \"local\"\n"
    );
    if !policy.is_empty() {
        file.push_str(&format!("[route_policy]\n{policy}\n"));
    }
    file.push_str(&model_table(Some(&claude_cli())));
    file.push_str(RULE);
    file
}

fn home_calling(tag: &str, roles: &str) -> Home {
    let h = home(tag);
    write_exe(&h.home.join("bindings/tools/claude"), &claude_calling(&h.log, roles));
    h
}

fn submit_with(l: &Live, kind: &str, extra: Value) -> (u16, Value) {
    let mut body = json!({"runbook_id": "autonomy-demo", "intake": "a note on tea", "confinement": "local", "autonomy": "A5", "decider": kind});
    for (k, v) in extra.as_object().unwrap() {
        body[k] = v.clone();
    }
    post(l, &body)
}

fn submitted(l: &Live, kind: &str, extra: Value) -> String {
    let (status, body) = submit_with(l, kind, extra);
    assert_eq!((status, body["outcome"].clone()), (202, json!("submitted")), "{body}");
    body["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned()
}

fn row(l: &Live, run: &str) -> Value {
    rows(l, &format!("/daemon/views/run?run_id={run}"))[0].clone()
}

fn events(l: &Live, run: &str, kind: &str) -> Vec<Value> {
    rows(l, &format!("/daemon/views/events?run_id={run}&after_position=0")).into_iter().filter(|e| e["kind"] == kind).map(|e| e["record"].clone()).collect()
}

fn models(g: &Gateway) -> Vec<String> {
    got(g).iter().map(|x| x.body["model"].as_str().unwrap_or("").to_owned()).collect()
}

fn ended(l: &Live, run: &str) {
    let end = until_state(l, run, &["Ended", "FailedToStart", "Cancelled"]);
    assert_eq!(end["state"], "Ended", "{end}");
}

#[test]
fn runs_naming_a_and_b_and_none_reach_the_alias_of_a_of_b_and_of_the_default_route_and_the_record_says_which() {
    let h = home_calling("rr-reach", r#""$model""#);
    let (one, two) = (gateway(OK), gateway(OK));
    let l = serve("rr-reach-d", &h, &routes_file(&one.base_url, &two.base_url, &h.token_file, "default = \"c\""), true);

    let run_a = submitted(&l, "model", json!({"model_route": "a"}));
    ended(&l, &run_a);
    assert!(!models(&one).is_empty() && models(&one).iter().all(|m| m == "alias-a") && got(&two).is_empty(), "route a: {:?} {:?}", models(&one), models(&two));
    for g in got(&one) {
        assert!(g.head.contains(&format!("Authorization: Bearer {TOKEN}\r\n")), "{}", g.head);
    }
    let r = row(&l, &run_a);
    assert_eq!((r["model_route"].clone(), r["model_route_alias"].clone(), r["model_route_source"].clone()), (json!("a"), json!("alias-a"), json!("run")), "{r}");
    let line = &events(&l, &run_a, "runbook.model-call")[0];
    assert_eq!((line["route"].clone(), line["alias"].clone(), line["source"].clone(), line["input_tokens"].clone(), line["cost_known"].clone(), line["micro_usd"].clone()), (json!("a"), json!("alias-a"), json!("run"), json!(13), json!(true), json!(21)), "{line}");
    let first = &events(&l, &run_a, "runbook.submitted")[0];
    assert_eq!((first["model_route"].clone(), first["model_route_alias"].clone(), first["model_route_source"].clone()), (json!("a"), json!("alias-a"), json!("run")), "{first}");

    let before = got(&one).len();
    let run_b = submitted(&l, "model", json!({"model_route": "b"}));
    ended(&l, &run_b);
    assert!(!models(&two).is_empty() && models(&two).iter().all(|m| m == "alias-b") && got(&one).len() == before, "route b: {:?} {:?}", models(&one), models(&two));
    assert_eq!(row(&l, &run_b)["model_route_source"], "run");

    let before_two = got(&two).len();
    let run_d = submitted(&l, "model", json!({}));
    ended(&l, &run_d);
    assert!(models(&one)[before..].iter().all(|m| m == "alias-c") && got(&one).len() > before && got(&two).len() == before_two, "the default: {:?}", models(&one));
    let r = row(&l, &run_d);
    assert_eq!((r["model_route"].clone(), r["model_route_alias"].clone(), r["model_route_source"].clone()), (json!("c"), json!("alias-c"), json!("default")), "{r}");
    let line = &events(&l, &run_d, "runbook.model-call")[0];
    assert_eq!((line["route"].clone(), line["source"].clone(), line["micro_usd"].clone()), (json!("c"), json!("default"), json!(0)), "a local route without prices costs 0: {line}");
}

#[test]
fn a_role_named_in_model_routes_reaches_its_own_routes_alias_and_the_role_default_the_runs_route() {
    let h = home_calling("rr-roles", r#"writer "$model""#);
    let (one, two) = (gateway(OK), gateway(OK));
    let l = serve("rr-roles-d", &h, &routes_file(&one.base_url, &two.base_url, &h.token_file, ""), true);
    let run = submitted(&l, "model", json!({"model_route": "a", "model_routes": {"writer": "b"}}));
    ended(&l, &run);
    // each decision of the run asks the role writer, then the role default: one call to each gateway per decision
    let (a, b) = (models(&one), models(&two));
    assert!(!a.is_empty() && a.len() == b.len() && a.iter().all(|m| m == "alias-a") && b.iter().all(|m| m == "alias-b"), "{a:?} {b:?}");
    let r = row(&l, &run);
    assert_eq!((r["model_route"].clone(), r["model_routes"].clone()), (json!("a"), json!({"writer": "b"})), "{r}");
    let lines = events(&l, &run, "runbook.model-call");
    let pair = [(json!("writer"), json!("b"), json!("run")), (json!("default"), json!("a"), json!("run"))];
    let said: Vec<_> = lines.iter().map(|x| (x["role"].clone(), x["route"].clone(), x["source"].clone())).collect();
    assert!(said.len() == 2 * a.len() && said.chunks(2).all(|c| c == pair), "{said:?}");
}

/// The runs the daemon holds and the run directories in its state.
fn nothing_written(l: &Live) {
    assert_eq!(rows(l, "/daemon/views/runs"), Vec::<Value>::new(), "a refused submit made a run");
    assert_eq!(entries(&l.dir.join("state/runs")), Vec::<String>::new(), "a refused submit wrote a run directory");
}

fn refused(l: &Live, kind: &str, extra: Value, says: &str, never: &[&str]) {
    let (status, body) = submit_with(l, kind, extra.clone());
    assert_eq!((status, body["outcome"].clone(), body["error"].clone()), (422, json!("route-refused"), json!("sdc.daemon.RouteRefused")), "{extra}: {body}");
    assert_eq!(body["payload"]["declared"], json!(["a", "b", "c"]), "{extra}: {body}");
    let reason = body["payload"]["reason"].as_str().unwrap_or_default();
    assert!(reason.contains(says), "{extra}: the reason does not say {says:?}: {reason}");
    for value in never {
        assert!(!body.to_string().contains(value), "{extra}: the refusal holds the value {value:?}: {body}");
    }
}

#[test]
fn an_undeclared_route_an_address_a_token_or_an_alias_in_its_place_are_refused_by_name_listing_the_declared_routes_and_nothing_is_written() {
    let h = home("rr-refuse");
    let (one, two) = (gateway(OK), gateway(OK));
    let l = serve("rr-refuse-d", &h, &routes_file(&one.base_url, &two.base_url, &h.token_file, "default = \"c\""), true);
    refused(&l, "model", json!({"model_route": "no-such-route"}), "declares no route", &["no-such-route"]);
    refused(&l, "model", json!({"model_route": "http://127.0.0.1:9/v1"}), "an address", &["127.0.0.1:9"]);
    refused(&l, "model", json!({"model_route": "alias-b"}), "an alias", &["alias-b"]);
    refused(&l, "model", json!({"model_route": "sk-ant-api03-0123456789abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnop"}), "a token", &["sk-ant-api03"]);
    refused(&l, "model", json!({"model_routes": {"writer": "nope-route"}}), "model_routes.writer", &["nope-route"]);
    refused(&l, "model", json!({"model_routes": {"a b/c": "a"}}), "a role", &["a b/c"]);
    refused(&l, "model", json!({"model_route": ""}), "empty", &[]);
    // the gateway's own members are refused by name too, before the body is read as a submit
    for member in ["token", "base_url", "token_file"] {
        let (status, body) = submit_with(&l, "model", json!({member: "secret-value-31"}));
        assert_eq!(status, 400, "{member}: {body}");
        assert!(!body.to_string().contains("secret-value-31"), "{body}");
    }
    nothing_written(&l);
    assert!(got(&one).is_empty() && got(&two).is_empty());
}

#[test]
fn a_run_that_makes_no_model_call_and_names_a_route_is_refused_and_one_that_names_none_takes_no_default() {
    let h = home("rr-rule");
    let (one, two) = (gateway(OK), gateway(OK));
    let l = serve("rr-rule-d", &h, &routes_file(&one.base_url, &two.base_url, &h.token_file, "default = \"c\""), true);
    refused(&l, "rule", json!({"model_route": "a"}), "makes no model call", &[]);
    refused(&l, "rule", json!({"model_routes": {"writer": "a"}}), "makes no model call", &[]);
    refused(&l, "model", json!({"autonomy": "A0", "model_route": "a"}), "makes no model call", &[]);
    nothing_written(&l);
    let run = submitted(&l, "rule", json!({}));
    ended(&l, &run);
    let r = row(&l, &run);
    assert!(r.get("model_route").is_none() && r.get("model_route_source").is_none(), "the default was applied to a run that makes no model call: {r}");
    assert!(got(&one).is_empty() && got(&two).is_empty() && entries(&h.xdg).is_empty());
}

#[test]
fn a_model_run_that_names_no_route_on_a_daemon_without_a_default_is_refused_with_the_list() {
    let h = home("rr-nodefault");
    let (one, two) = (gateway(OK), gateway(OK));
    let l = serve("rr-nodefault-d", &h, &routes_file(&one.base_url, &two.base_url, &h.token_file, ""), true);
    refused(&l, "model", json!({}), "route_policy.default", &[]);
    nothing_written(&l);
}

#[test]
fn a_route_that_is_down_fails_the_call_naming_the_route_the_code_and_the_setting_and_no_other_route_is_called() {
    let h = home_calling("rr-down", r#""$model""#);
    let (one, two) = (gateway(STOPPED), gateway(OK));
    let l = serve("rr-down-d", &h, &routes_file(&one.base_url, &two.base_url, &h.token_file, "default = \"b\""), true);
    let run = submitted(&l, "model", json!({"model_route": "a"}));
    wait_until("the failed call to be in the run's record", || !events(&l, &run, "runbook.model-call").is_empty());
    let failed = &events(&l, &run, "runbook.model-call")[0];
    assert_eq!(
        (failed["cause"].clone(), failed["route"].clone(), failed["code"].clone(), failed["source"].clone(), failed["micro_usd"].clone()),
        (json!("route-unavailable"), json!("a"), json!("model-stopped"), json!("run"), json!(0)),
        "{failed}"
    );
    assert_eq!((got(&one).len(), got(&two).len()), (1, 0), "the call was sent again, or to the default route");
    let c = super::http::send(l.addr, "POST", "/daemon/commands/cancel-run", &[("Content-Type", "application/json")], Some(json!({"run_id": run}).to_string().as_bytes()));
    assert!(c.status < 300, "cancel-run: {}", c.status);
    until_state(&l, &run, &["Cancelled", "Ended"]);
}

#[test]
fn a_daemon_with_only_a_gateway_table_runs_a_model_run_on_its_interim_route_default_and_refuses_another_name() {
    let h = home_calling("rr-lone", r#""$model""#);
    let gw = gateway(OK);
    let file = format!("{}{}", super::gateway::gateway_table(&gw.base_url, &h.token_file), model_table(Some(&claude_cli())));
    let l = serve("rr-lone-d", &h, &file, true);
    let (status, body) = submit_with(&l, "model", json!({"model_route": "qwen"}));
    assert_eq!((status, body["outcome"].clone(), body["payload"]["declared"].clone()), (422, json!("route-refused"), json!(["default"])), "{body}");
    let named = submitted(&l, "model", json!({"model_route": "default"}));
    ended(&l, &named);
    assert_eq!(row(&l, &named)["model_route_source"], "run");
    let run = submitted(&l, "model", json!({}));
    ended(&l, &run);
    assert!(!got(&gw).is_empty() && models(&gw).iter().all(|m| m == "a-model"), "{:?}", models(&gw));
    let r = row(&l, &run);
    assert_eq!((r["model_route"].clone(), r["model_route_alias"].clone(), r["model_route_source"].clone()), (json!("default"), json!("a-model"), json!("default")), "{r}");
}

/// `ess/domains/daemon.yaml`: the entity `Run` has the relation `model_route` (`references`, cardinality one) to
/// `ModelRoute` via `model_route`, and the view `routes` lists the `ModelRoute` rows. A run's `model_route` names a row
/// of that view.
#[test]
fn the_model_route_of_a_run_on_a_lone_gateway_is_a_route_the_view_routes_lists() {
    let h = home_calling("rr-lone-ref", r#""$model""#);
    let gw = gateway(OK);
    let file = format!("{}{}", super::gateway::gateway_table(&gw.base_url, &h.token_file), model_table(Some(&claude_cli())));
    let l = serve("rr-lone-ref-d", &h, &file, true);
    let run = submitted(&l, "model", json!({}));
    ended(&l, &run);
    let r = row(&l, &run);
    let listed: Vec<Value> = rows(&l, "/daemon/views/routes").iter().map(|x| x["route"].clone()).collect();
    assert!(listed.contains(&r["model_route"]), "the run references the route {} and the view routes lists {listed:?}", r["model_route"]);
}
