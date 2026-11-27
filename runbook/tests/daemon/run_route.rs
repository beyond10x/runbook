//! A run names a model route (`docs/model-routes.md`, section 2; story `run-names-a-model-route`): a submit names a
//! route of the daemon's `runbook.toml` by name in `model_route` (and per role in `model_routes`), else the run takes
//! `route_policy.default`; the run's calls go through the daemon's relay to that route's gateway with its alias, and
//! the run's record names the route, the alias and the setting that chose it. A run takes a route only when it has a
//! model step: its decider is a model above A0, or its runbook runs an agent node or a program whose per-call budget is
//! above 0 (its bound programs get the route file; story `the-default-route-wakes-for-runs-without-a-model-call`). A
//! route that is not declared, an address, a token or an alias in its place, a route for a run that makes no model call
//! (no model step), and a model run with no route on a daemon with no default are refused `422 route-refused` before
//! anything is written, listing the declared routes.
//!
//! No model is called and nothing leaves loopback: the gateways are stand-ins on ports of the test's own, the model
//! decider's adapter is the committed claude-cli adapter and `claude` a stand-in that calls the relay.

use super::default_confinement::{post, rows, Live};
use super::gateway::{claude_calling, claude_cli, entries, home, model_table, serve, until_state, wait_until, write_exe, Home, TOKEN};
use super::relay::{gateway_listing, gateway_with_status, got, Gateway};
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
    // the runbook's spend ledger books each call of the run as one line naming route a, with the call's tokens and the
    // cost of a's declared prices (13 input tokens at 1 USD and 4 output tokens at 2 USD per million: 21 micro USD), and
    // nothing else of the run
    let ledger: Value = serde_json::from_slice(&std::fs::read(l.dir.join("state/ledgers/autonomy-demo.json")).unwrap()).unwrap();
    let booked: Vec<&Value> = ledger["entries"].as_array().into_iter().flatten().filter(|e| e["run"] == run_a.as_str()).collect();
    assert!(!booked.is_empty(), "the ledger has no line of the run: {ledger}");
    for e in &booked {
        assert_eq!(
            (e["what"].clone(), e["route"].clone(), e["alias"].clone(), e["input_tokens"].clone(), e["output_tokens"].clone(), e["cost_known"].clone(), e["micro_usd"].clone()),
            (json!("model call"), json!("a"), json!("alias-a"), json!(13), json!(4), json!(true), json!(21)),
            "{e}"
        );
    }
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

/// A runbook of the test's home that runs no program and has no agent node: its one action is a person's.
fn person_runbook(h: &Home) {
    let protocol = "format: protocol/1\nprotocol:\n  id: person-demo\n  revision: 1\n  description: >-\n    A route test's runbook: a person approves the idea; no program runs.\nartifacts:\n  idea: {}\nevidence_kinds:\n  idea_review: {}\nclaims:\n  idea.approved:\n    true_when:\n      evidence: {kind: idea_review, subject: idea, result: approved}\nactions:\n  idea.review:\n    effect: write\n    may_produce:\n      - evidence: idea_review\noutcomes:\n  approved:\n    requires: {claim: idea.approved}\n";
    let bindings = "format: loom.bindings/1\nprotocol: person-demo@1\nprogram_root: bindings\nartifacts:\n  idea: idea.yaml\nactions:\n  - action: idea.review\n    executor:\n      kind: person\n      principal: reviewer\n    evidence:\n      - kind: idea_review\n        subject: idea\n        producer: reviewer\n        from: effect\n        results: {performed: approved, rejected: rejected}\n";
    std::fs::write(h.home.join("protocol/person-demo.protocol.yaml"), protocol).unwrap();
    std::fs::write(h.home.join("bindings/person-demo.yaml"), bindings).unwrap();
}

/// The program of `notes.write` for a run whose model calls come from its bound programs, not its decider: it writes
/// one line to `log/route-files` (`set` when `RUNBOOK_ROUTE_FILE` names a file, else `none`), and with the file it
/// sends one Messages call for the role `default` to the relay the file names, then writes the notes.
fn program_calling(log: &Path) -> String {
    format!(
        r#"#!/bin/bash
log='{log}'
f=${{RUNBOOK_ROUTE_FILE-}}
if [ -z "$f" ]; then echo none >> "$log/route-files"; echo notes >> notes.txt; exit 0; fi
echo set >> "$log/route-files"
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

fn route_files(h: &Home) -> Vec<String> {
    std::fs::read_to_string(h.log.join("route-files")).unwrap_or_default().lines().map(str::to_owned).collect()
}

/// Point 1 and the rule that stays of point 2: a run whose runbook runs no program (no bound program, no agent node) and
/// whose decider is not a model above A0 may not take a route. A submit for it that names one is refused, and the
/// reason names the member that named it; the default route is not applied to it.
#[test]
fn a_run_whose_runbook_runs_no_program_and_whose_decider_is_no_model_is_refused_naming_the_member_and_takes_no_default() {
    let h = home("rr-rule");
    person_runbook(&h);
    let (one, two) = (gateway(OK), gateway(OK));
    let l = serve("rr-rule-d", &h, &routes_file(&one.base_url, &two.base_url, &h.token_file, "default = \"c\""), true);
    let person = |extra: Value| {
        let mut extra = extra;
        extra["runbook_id"] = json!("person-demo");
        extra
    };
    refused(&l, "rule", person(json!({"model_route": "a"})), "model_route", &[]);
    refused(&l, "rule", person(json!({"model_route": "a"})), "makes no model call", &[]);
    refused(&l, "rule", person(json!({"model_routes": {"writer": "a"}})), "model_routes.writer", &[]);
    refused(&l, "model", person(json!({"autonomy": "A0", "model_route": "a"})), "model_route", &[]);
    nothing_written(&l);
    let run = submitted(&l, "rule", person(json!({})));
    let r = row(&l, &run);
    assert!(r.get("model_route").is_none() && r.get("model_route_source").is_none(), "the default was applied to a run that makes no model call: {r}");
    assert!(got(&one).is_empty() && got(&two).is_empty() && entries(&h.xdg).is_empty());
}

/// Point 2: a run's model calls may come from its bound programs, not from its decider. A run whose decider is a rule
/// takes the route it names, else the default route; its bound program gets `RUNBOOK_ROUTE_FILE` and its call reaches
/// the route's gateway through the relay with the route's alias, booked with the setting that chose it.
#[test]
fn a_rule_run_whose_runbook_runs_programs_takes_its_named_route_or_the_default_and_its_programs_get_the_route_file() {
    let h = home("rr-programs");
    write_exe(&h.home.join("bindings/tools/notes-write.sh"), &program_calling(&h.log));
    let (one, two) = (gateway(OK), gateway(OK));
    let l = serve("rr-programs-d", &h, &routes_file(&one.base_url, &two.base_url, &h.token_file, "default = \"c\""), true);

    let named = submitted(&l, "rule", json!({"model_route": "a"}));
    ended(&l, &named);
    let r = row(&l, &named);
    assert_eq!((r["model_route"].clone(), r["model_route_alias"].clone(), r["model_route_source"].clone()), (json!("a"), json!("alias-a"), json!("run")), "{r}");
    assert!(!route_files(&h).is_empty() && route_files(&h).iter().all(|s| s == "set"), "the bound program got no route file: {:?}", route_files(&h));
    assert!(!models(&one).is_empty() && models(&one).iter().all(|m| m == "alias-a") && got(&two).is_empty(), "route a: {:?} {:?}", models(&one), models(&two));
    let line = &events(&l, &named, "runbook.model-call")[0];
    assert_eq!((line["route"].clone(), line["step"].clone(), line["source"].clone()), (json!("a"), json!("notes.write"), json!("run")), "{line}");

    let before = got(&one).len();
    let defaulted = submitted(&l, "rule", json!({}));
    ended(&l, &defaulted);
    let r = row(&l, &defaulted);
    assert_eq!((r["model_route"].clone(), r["model_route_alias"].clone(), r["model_route_source"].clone()), (json!("c"), json!("alias-c"), json!("default")), "{r}");
    assert!(got(&one).len() > before && models(&one)[before..].iter().all(|m| m == "alias-c"), "the default route: {:?}", models(&one));
}

/// Point 2, a run with no route: on a daemon that declares routes and no default, a rule run that names none is not
/// refused and takes no route; its bound program gets no `RUNBOOK_ROUTE_FILE` and no call reaches a gateway.
#[test]
fn a_rule_run_that_names_no_route_on_a_daemon_without_a_default_takes_none_and_its_programs_get_no_route_file() {
    let h = home("rr-noroute");
    write_exe(&h.home.join("bindings/tools/notes-write.sh"), &program_calling(&h.log));
    let (one, two) = (gateway(OK), gateway(OK));
    let l = serve("rr-noroute-d", &h, &routes_file(&one.base_url, &two.base_url, &h.token_file, ""), true);
    let run = submitted(&l, "rule", json!({}));
    ended(&l, &run);
    let r = row(&l, &run);
    assert!(r.get("model_route").is_none(), "{r}");
    assert!(!route_files(&h).is_empty() && route_files(&h).iter().all(|s| s == "none"), "a run with no route handed a route file: {:?}", route_files(&h));
    assert!(got(&one).is_empty() && got(&two).is_empty());
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

/// A runtime whose runbooks run a program that calls a model and whose relay serves runs or not
/// (`Runtime::serves_routes`): it keeps the start request of each run and ends it.
struct Programs {
    serves: bool,
    started: std::sync::Mutex<Vec<runbook::runtime::StartRequest>>,
}

impl runbook::runtime::Runtime for Programs {
    fn recover(&self, _run_id: &str) -> runbook::runtime::Recovery {
        runbook::runtime::Recovery::Gone
    }
    fn knows_runbook(&self, _runbook: &str, _version: Option<&str>) -> bool {
        true
    }
    fn runbooks(&self) -> Vec<runbook::runtime::RunbookInfo> {
        Vec::new()
    }
    fn kind(&self) -> &'static str {
        "stub"
    }
    fn program_steps(&self, _runbook: &str, _version: Option<&str>) -> runbook::runtime::ProgramSteps {
        runbook::runtime::ProgramSteps::ModelCall
    }
    fn serves_routes(&self) -> bool {
        self.serves
    }
    fn start(&self, request: runbook::runtime::StartRequest, sink: &dyn runbook::runtime::EventSink, _cancel: &std::sync::atomic::AtomicBool) -> Result<runbook::runtime::Finish, runbook::runtime::RuntimeFailure> {
        self.started.lock().unwrap().push(request);
        sink.started();
        Ok(runbook::runtime::Finish::Outcome("done".into()))
    }
    fn answer(&self, _run_id: &str, _request_id: &str, _answer: &str, _principal: &str) -> Result<(), runbook::runtime::AnswerFailure> {
        Ok(())
    }
    fn cancel(&self, _run_id: &str) {}
}

/// A run whose model calls come from its programs only and that names no route takes the daemon's declared default
/// only where the relay serves runs: on a daemon whose listener is not loopback a run on a route does not start, so
/// such a run takes no route there and runs as before routes. A route it names is its own word and is kept.
#[test]
fn a_program_run_takes_the_declared_default_only_where_the_relay_serves_runs() {
    use runbook::routes::{RouteTable, Tables};
    let gw = gateway_listing(OK, &["alias-c"]);
    let gateways = json!({"g": {"base_url": gw.base_url}});
    let declared = json!({"c": {"gateway": "g", "model": "alias-c", "kind": "hosted"}});
    let policy = json!({"default": "c"});
    let routes = std::sync::Arc::new(RouteTable::declare(&Tables { gateways: Some(&gateways), routes: Some(&declared), route_policy: Some(&policy), ..Tables::default() }).unwrap());
    let take = |serves: bool, named: Option<&str>| {
        let dir = super::state::scratch(&format!("rr-serves-{serves}-{}", named.unwrap_or("none")));
        std::fs::create_dir_all(dir.join("ctx")).unwrap();
        let rt = std::sync::Arc::new(Programs { serves, started: std::sync::Mutex::new(Vec::new()) });
        let policy = runbook::commands::Policy { default_confinement: Some(runbook::runtime::Confinement::Linux), routes: routes.clone(), ..runbook::commands::Policy::default() };
        let d = runbook::commands::Daemon::start_with(super::commands::config(&dir), policy, rt.clone()).unwrap();
        let out = d.submit_run(runbook::commands::SubmitRun { runbook_id: "any".into(), intake: "x".into(), model_route: named.map(str::to_owned), ..runbook::commands::SubmitRun::default() }).unwrap_or_else(|f| panic!("{f:?}"));
        assert_eq!(out.body()["outcome"], "submitted", "{}", out.body());
        let limit = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while rt.started.lock().unwrap().is_empty() {
            assert!(std::time::Instant::now() < limit, "the run did not start");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let started = rt.started.lock().unwrap();
        started[0].routes.as_ref().map(|t| t.get("default").map(|(r, _)| r.name.clone()))
    };
    assert_eq!(take(true, None), Some(Some("c".to_owned())), "a program run on a daemon whose relay serves runs takes the declared default");
    assert_eq!(take(false, None), None, "a program run on a daemon whose relay serves no run took the default route, so it would not start");
    assert_eq!(take(false, Some("c")), Some(Some("c".to_owned())), "a route the run names is its own word");
}

/// What the stand-in of a stopped managed model answers to a wake, and to any other request it keeps.
const WOKE: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 20\r\nConnection: close\r\n\r\n{\"phase\":\"starting\"}";
const WAKE_M: &str = "POST /v1/models/alias-m/wake";

/// A stand-in gateway of the managed route `m` (alias `alias-m`): it is ready, lists the alias, says in `GET /v1/status`
/// that the model is stopped, and keeps every other request (a wake, a call) and answers it 200.
fn stopped_gateway() -> Gateway {
    gateway_with_status(|_| WOKE.to_vec(), &["alias-m"], json!({"models": [{"model": "alias-m", "callable": false, "phase": "stopped"}]}))
}

/// A daemon file whose one route `m` is managed, waits 600 s for its model, is woken at a submit within a daily wake cap,
/// and is `route_policy.default`.
fn waking_default(gw: &Gateway, token_file: &Path) -> String {
    format!(
        "[gateways.one]\nbase_url = \"{}\"\ntoken_file = \"{}\"\n[routes.m]\ngateway = \"one\"\nmodel = \"alias-m\"\nkind = \"managed\"\nstart_wait_seconds = 600\nwake_on_submit = true\nwake_daily_cap_micro_usd = 5000000\nwake_cost_micro_usd = 1000\n[route_policy]\ndefault = \"m\"\n{}{RULE}",
        gw.base_url,
        token_file.display(),
        model_table(Some(&claude_cli()))
    )
}

/// The request lines the gateway kept: every request but its reads.
fn kept_lines(g: &Gateway) -> Vec<String> {
    got(g).iter().map(|x| x.head.lines().next().unwrap_or("").rsplit_once(' ').map(|(l, _)| l.to_owned()).unwrap_or_default()).collect()
}

fn route_and_source(r: &Value) -> (Value, Value) {
    (r.get("model_route").cloned().unwrap_or(Value::Null), r.get("model_route_source").cloned().unwrap_or(Value::Null))
}

/// A run whose decider or one of whose programs calls a model is on the route `m` by the default, waits for it, and its
/// one wake reaches the gateway.
fn woke_by_default(l: &Live, gw: &Gateway, run: &str) {
    let r = until_state(l, run, &["WaitingForRoute"]);
    assert_eq!(route_and_source(&r), (json!("m"), json!("default")), "{r}");
    wait_until("the wake of the route m", || kept_lines(gw).iter().any(|x| x == WAKE_M));
    assert_eq!(kept_lines(gw), vec![WAKE_M.to_owned()], "one wake and no call");
}

/// A home whose runbook `autonomy-demo` has no model step: every bound program of it names the per-call budget 0.
fn home_without_a_model_step(tag: &str) -> Home {
    let h = home(tag);
    let bindings = h.home.join("bindings/autonomy-demo.yaml");
    let text = std::fs::read_to_string(&bindings).unwrap();
    assert!(text.contains("--max-budget-usd, \"0.01\""), "the fixture's one budget moved: {text}");
    std::fs::write(&bindings, text.replace("\"0.01\"", "\"0\"")).unwrap();
    h
}

/// The run took no route (its record names none), booked and sent no wake, waited for no route and ran to its end, and
/// nothing reached the gateway.
fn took_no_route_and_ended(l: &Live, gw: &Gateway, run: &str) {
    assert_eq!(route_and_source(&row(l, run)), (Value::Null, Value::Null), "a route went to a run with no model step");
    ended(l, run);
    for kind in ["runbook.route-wake", "runbook.route-wake-answered", "runbook.waiting-for-route"] {
        assert_eq!(events(l, run, kind), Vec::<Value>::new(), "{kind}");
    }
    assert_eq!(kept_lines(gw), Vec::<String>::new(), "the gateway was sent a wake or a call");
}

/// Story `the-default-route-wakes-for-runs-without-a-model-call`, point 1 of the brief: a runbook whose bound programs all
/// name the per-call budget 0 has no model step. Under a `route_policy.default` that names a stopped managed route with
/// `wake_on_submit`, its run with a rule decider takes no route, books and sends no wake, waits for no route and runs to
/// its end. Point 2: the same runbook under a model decider has a model step, takes the default and wakes it.
#[test]
fn a_run_with_no_model_step_takes_no_default_route_and_wakes_nothing_and_runs_to_its_end() {
    let h = home_without_a_model_step("rr-quiet");
    let gw = stopped_gateway();
    let l = serve("rr-quiet-d", &h, &waking_default(&gw, &h.token_file), true);

    let run = submitted(&l, "rule", json!({}));
    took_no_route_and_ended(&l, &gw, &run);
    assert_eq!(events(&l, &run, "runbook.submitted")[0].get("model_route_not_taken"), None, "the run named no route");

    let model = submitted(&l, "model", json!({}));
    woke_by_default(&l, &gw, &model);
}

/// Point 3 of the brief, as the coordinator decided it: a run with no model step that names a route (its own
/// `model_route` and a role's) is admitted, takes no route and wakes nothing, as one that names none; its first event
/// `runbook.submitted` names the routes the submit asked for under `model_route_not_taken` and says why. A runbook with
/// no program keeps its refusal (`a_run_whose_runbook_runs_no_program_and_whose_decider_is_no_model_is_refused_...`).
#[test]
fn a_run_with_no_model_step_that_names_a_route_is_admitted_takes_none_wakes_nothing_and_its_first_event_says_why() {
    let h = home_without_a_model_step("rr-quiet-named");
    let gw = stopped_gateway();
    let l = serve("rr-quiet-named-d", &h, &waking_default(&gw, &h.token_file), true);

    let run = submitted(&l, "rule", json!({"model_route": "m", "model_routes": {"writer": "m"}}));
    took_no_route_and_ended(&l, &gw, &run);
    let first = &events(&l, &run, "runbook.submitted")[0];
    assert_eq!(first.get("model_route"), None, "{first}");
    let not_taken = &first["model_route_not_taken"];
    assert_eq!((not_taken["model_route"].clone(), not_taken["model_routes"].clone()), (json!("m"), json!({"writer": "m"})), "{first}");
    let reason = not_taken["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("makes no model call"), "{first}");
}

/// Point 2 of the brief: a runbook whose bound program names a per-call budget above 0 has a model step. Its run with a
/// rule decider takes the default route as before and wakes it.
#[test]
fn a_rule_run_whose_program_names_a_budget_above_0_takes_the_default_route_and_wakes_it() {
    let h = home("rr-woken");
    let gw = stopped_gateway();
    let l = serve("rr-woken-d", &h, &waking_default(&gw, &h.token_file), true);
    let run = submitted(&l, "rule", json!({}));
    woke_by_default(&l, &gw, &run);
}

// ---- adversary of the unit: the rule "a program whose per-call budget, read as the run reserves it, is above 0" ----

/// Adversary: a `handoff_when` check is a program of the run. Loom starts it through the runner every bound program
/// starts through (`b10x-loom-bindings`, `handoff.rs`), so the daemon reserves its budget as it starts
/// (`budget::Reserves::of`, `engine/gate.rs`, `Reserving::run_child`) and, on a route, hands it the route file
/// (`engine/route.rs`, `RouteEnv::run_child`). A check that names `--max-budget-usd 0.2` reserves 0.2 USD: by the rule of
/// `docs/model-routes.md`, section 2 ("a program whose per-call budget, read as the run reserves it, is above 0") the
/// run has a model step, takes the default route and wakes it. Every other program of the runbook names 0 (the file's
/// member), so only the check makes the model step.
#[test]
fn adversary_a_rule_run_whose_handoff_check_reserves_a_budget_above_0_takes_the_default_route() {
    let h = home_without_a_model_step("rr-adv-handoff");
    let bindings = h.home.join("bindings/autonomy-demo.yaml");
    let text = std::fs::read_to_string(&bindings).unwrap();
    let writer = "      argv: [tools/notes-write.sh, --max-budget-usd, \"0\"]\n";
    assert!(text.contains(writer) && text.contains("program_root: bindings\n"), "{text}");
    let text = text
        .replace("program_root: bindings\n", "program_root: bindings\nmax_budget_usd: 0\n")
        .replace(writer, &format!("{writer}    handoff: Write the notes by hand.\n    handoff_when: [tools/judge.sh, --max-budget-usd, \"0.2\"]\n"));
    std::fs::write(&bindings, text).unwrap();
    write_exe(&h.home.join("bindings/tools/judge.sh"), "#!/bin/sh\nexit 1\n");
    // the premise: the run reserves 0.2 USD for the check as it starts it
    let reserves = runbook::budget::reserves(&bindings).unwrap();
    assert_eq!(reserves.of("tools/judge.sh", &["--max-budget-usd".to_owned(), "0.2".to_owned()]), 200_000, "the check's reservation");
    let gw = stopped_gateway();
    let l = serve("rr-adv-handoff-d", &h, &waking_default(&gw, &h.token_file), true);
    let run = submitted(&l, "rule", json!({}));
    assert_eq!(route_and_source(&row(&l, &run)), (json!("m"), json!("default")), "a run whose handoff check reserves 0.2 USD took no route: the check starts with no route file");
}

/// A bindings directory of the adversary's own with the files `files` (name, text).
fn adversary_bindings(tag: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
    let dir = super::state::scratch(&format!("rr-adv-{tag}"));
    for (name, text) in files {
        std::fs::write(dir.join(name), text).unwrap();
    }
    dir.join(files[0].0)
}

/// Adversary: "an agent node calls a model whatever its budget" (`budget::tests`, the unit's own case) holds for
/// `executor.argv` only. The agent node `sdc-node` as an `arguments_from` program or as a `handoff_when` check of a
/// runbook whose budgets are 0 makes no model step, though Loom starts both through the run's runner (they get the
/// route file on a route) and `runbook run` counts both forms as a model node
/// (`tests/loom_run/no_model_node_guards.rs`, `forms_of_the_copy_that_must_count_as_a_model_node`).
#[test]
fn adversary_an_agent_node_in_arguments_from_or_handoff_when_is_a_model_step_whatever_its_budget() {
    let child = "format: loom.bindings/1\nmax_budget_usd: 0\nactions:\n  - action: c\n    executor: {kind: program, argv: [tools/c.sh]}\n";
    let forms = [
        ("arguments_from", "format: loom.bindings/1\nmax_budget_usd: 0\nactions:\n  - action: k.open\n    executor: {kind: open, name: k, bindings: child.yaml}\n    arguments_from: [sdc-node, list]\n"),
        ("handoff_when", "format: loom.bindings/1\nmax_budget_usd: 0\nactions:\n  - action: a\n    executor: {kind: program, argv: [tools/a.sh]}\n    handoff: by hand\n    handoff_when: [sdc-node, check]\n"),
    ];
    let missed: Vec<&str> = forms.iter().filter(|(tag, root)| !runbook::budget::calls_a_model(&adversary_bindings(&format!("node-{tag}"), &[("root.yaml", root), ("child.yaml", child)]))).map(|(tag, _)| *tag).collect();
    assert!(missed.is_empty(), "an agent node that made no model step, in: {missed:?}");
}

/// Adversary: `docs/model-routes.md`, section 2, and `Runtime::program_steps` say bindings that cannot be read count as
/// a model step. A child bindings file an `open` action names that is no YAML is skipped (`budget::walk`), so the
/// runbook counts as calling no model.
#[test]
fn adversary_bindings_whose_child_file_cannot_be_read_count_as_a_model_step() {
    let root = "format: loom.bindings/1\nmax_budget_usd: 0\nactions:\n  - action: a\n    executor: {kind: program, argv: [tools/a.sh]}\n  - action: k.open\n    executor: {kind: open, name: k, bindings: child.yaml}\n";
    let path = adversary_bindings("unreadable-child", &[("root.yaml", root), ("child.yaml", "format: loom.bindings/1\nactions: [\n")]);
    assert!(runbook::budget::calls_a_model(&path), "a runbook whose child bindings cannot be read counted as calling no model");
}
