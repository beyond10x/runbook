//! Adversary cases of wave 3 of the model routes: the role names of the route file and the relay, the precedence of a
//! runbook's `needs`, the cost of a model call of unknown cost that ends after its step, and the record of a delegation
//! whose runbook the classifier picked.
//!
//! No model is called and nothing leaves loopback: the gateways are stand-ins on ports of the test's own.

use super::default_confinement::rows;
use super::delegate::{daemon_at, until_ended, ROUTE};
use super::gateway::{claude_cli, model_table, until_state, write_exe};
use super::http::send;
use super::relay::gateway_answering;
use runbook::admission::{for_run, parse, Asked, By, Facts};
use runbook::relay::{role_model, ResolvedRoute, RouteTable};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nx-llm-gateway-call: gw-call-9\r\nContent-Length: 56\r\nConnection: close\r\n\r\n{\"id\":\"m\",\"usage\":{\"input_tokens\":11,\"output_tokens\":5}}";

fn resolved(name: &str, alias: &str) -> ResolvedRoute {
    ResolvedRoute {
        name: name.to_owned(),
        base_url: "http://127.0.0.1:9".to_owned(),
        token: None,
        alias: alias.to_owned(),
        kind: "hosted".to_owned(),
        price_in_micro_usd_per_mtok: None,
        price_out_micro_usd_per_mtok: None,
    }
}

// ---- G3: the route file's model of a role ----

/// `docs/model-routes.md` section 11 and `docs/daemon-api.md` ("The relay of a run's model calls"): the route file gives
/// each role the model a client sends for it, `runbook-<role>`, and the relay takes that model for the role. A role is 1
/// to 64 letters, digits, `.`, `_` or `-` (a submit's `model_routes`, a runbook's route entries), so `x` and `runbook-x`
/// are two roles of one run. The claude-cli adapter of the role `x` sends the file's model of `x`, `runbook-x`: that call
/// must reach the route of `x`, not the route of the role `runbook-x`.
#[test]
fn adversary_wave3_the_route_files_model_of_a_role_reaches_that_role_when_another_role_is_named_runbook_role() {
    assert!(runbook::routes::is_name("x") && runbook::routes::is_name("runbook-x"), "both are role names a submit may give");
    let table = RouteTable::default().with("x", resolved("qwen", "qwen-alias"), "run").with("runbook-x", resolved("claude", "claude-alias"), "run");
    let sent = role_model("x");
    assert_eq!(sent, "runbook-x", "the route file's model of the role x");
    let of = |model: &str| table.named(model).map(|(role, route, _)| (role.to_owned(), route.name.clone()));
    // the role runbook-x by its own file model reaches its route
    assert_eq!(of(&role_model("runbook-x")), Some(("runbook-x".to_owned(), "claude".to_owned())));
    assert_eq!(
        of(&sent),
        Some(("x".to_owned(), "qwen".to_owned())),
        "the model the route file gives the role x reached another role's route"
    );
    assert_eq!(of(&format!("{sent}[1m]")), Some(("x".to_owned(), "qwen".to_owned())), "with the suffix Claude Code adds");
}

// ---- protocol-declares-the-routes-it-needs: the precedence of `needs` ----

fn facts(route: &str, kind: &str) -> Facts {
    Facts { route: route.to_owned(), kind: kind.to_owned(), wires: vec!["messages".to_owned()], max_model_len: None, tool_calling: None }
}

/// `docs/model-routes.md` section 2: "requirements are met by the run's or the policy's route when it meets them, else by
/// the one declared route that meets them"; `docs/daemon-api.md` ("The route of a run") refuses needs that "several do
/// and neither the run's route nor the default is one of them". The run names the route `c` (local), which misses the
/// role's `kind: hosted`; the daemon's default `b` meets it, and so does `a`. The default is one of the routes that meet
/// the needs, so the role takes it and the submit is not refused.
#[test]
fn adversary_wave3_needs_the_runs_route_misses_are_met_by_the_daemon_default_when_several_routes_meet_them() {
    let routes = [facts("a", "hosted"), facts("b", "hosted"), facts("c", "local")];
    let declared = parse("r", "format: runbook.routes/1\nmodel_routes:\n  w: {needs: {kind: hosted}}\n").unwrap();
    let none = BTreeMap::new();
    let chosen = for_run("r", &declared, &routes, Asked { route: Some("c"), roles: &none, default: Some("b") });
    let want: BTreeMap<String, (String, By)> = [("default".to_owned(), ("c".to_owned(), By::Run)), ("w".to_owned(), ("b".to_owned(), By::Protocol))].into_iter().collect();
    assert_eq!(chosen, Ok(want), "the daemon's default meets the role's needs and the role did not take it");
}

// ---- G1: a model call of unknown cost that ends after its step ----

/// A runbook of one bound program, `late.write`: it reserves 0.0123 USD (12,300 micro USD), calls the relay through the
/// run's route file (role `default`) and waits for the answer; it is killed at its timeout of 1 s. With `agent_call`
/// it first writes the call directory an agent node's adapter writes before it starts its model client
/// (`.sdc/llm/<n>-<action>/request.json`), whose cost line it would write after the answer. Its process id goes to
/// `pid_file`, for the gateway of [`gateway_after_the_program`].
fn late_runbook_with(home: &std::path::Path, agent_call: bool, pid_file: &std::path::Path) {
    let protocol = "format: protocol/1\nprotocol:\n  id: late-demo\n  revision: 1\n  description: >-\n    An adversary case's runbook: one program whose model call outlasts its timeout.\nartifacts:\n  idea: {}\n  result: {}\nevidence_kinds:\n  result_written: {}\nclaims:\n  result.written:\n    true_when:\n      evidence: {kind: result_written, subject: result, result: written}\nactions:\n  late.write:\n    effect: write\n    may_produce:\n      - evidence: result_written\noutcomes:\n  answered:\n    requires: {claim: result.written}\n";
    let bindings = "format: loom.bindings/1\nprotocol: late-demo@1\nprogram_root: bindings\nartifacts:\n  idea: idea.yaml\n  result: result.md\nactions:\n  - action: late.write\n    executor:\n      kind: program\n      argv: [tools/late.sh, --max-budget-usd, \"0.0123\"]\n    timeout_seconds: 1\n    evidence:\n      - kind: result_written\n        subject: result\n        producer: late\n        from: exit\n        results: {\"0\": written, timeout: failed, \"*\": failed}\n";
    std::fs::write(home.join("protocol/late-demo.protocol.yaml"), protocol).unwrap();
    std::fs::write(home.join("bindings/late-demo.yaml"), bindings).unwrap();
    let call_dir = if agent_call { "mkdir -p .sdc/llm/001-late.write && printf '{}\\n' > .sdc/llm/001-late.write/request.json\n" } else { "" };
    let program = format!("#!/bin/bash\necho $$ > '{}'\n", pid_file.display()) + call_dir + r#"f=$RUNBOOK_ROUTE_FILE
base=$(jq -r .base_url "$f") && token=$(jq -r .token "$f") || exit 9
rest=${base#http://}
hostport=${rest%%/*}
path=/${rest#*/}
body='{"model":"default","max_tokens":8,"messages":[{"role":"user","content":"hi"}]}'
exec 3<>"/dev/tcp/${hostport%:*}/${hostport##*:}" || exit 7
printf 'POST %s/v1/messages HTTP/1.0\r\nHost: %s\r\nAuthorization: Bearer %s\r\nContent-Type: application/json\r\nContent-Length: %s\r\n\r\n%s' "$path" "$hostport" "$token" "${#body}" "$body" >&3
answer=$(cat <&3)
printf 'answered\n' > result.md
"#;
    write_exe(&home.join("bindings/tools/late.sh"), &program);
}

fn late_runbook(home: &std::path::Path, pid_file: &std::path::Path) {
    late_runbook_with(home, false, pid_file);
}

/// Whether the program whose process id `pid_file` holds has not ended yet: no file yet, or a process that is neither gone
/// nor a zombie.
fn program_lives(pid_file: &std::path::Path) -> bool {
    let Ok(pid) = std::fs::read_to_string(pid_file) else { return true };
    match std::fs::read_to_string(format!("/proc/{}/stat", pid.trim())) {
        Ok(stat) => stat.rfind(')').and_then(|at| stat[at + 1..].split_whitespace().next()).is_some_and(|state| state != "Z"),
        Err(_) => false,
    }
}

/// A stand-in gateway that lists the aliases of the routes and answers each model call [`OK`] once the program whose
/// process id `pid_file` holds has ended, at most 20 s after the call came: the program is killed at its timeout of 1 s
/// while its call is in flight, and the answer comes after that.
fn gateway_after_the_program(pid_file: std::path::PathBuf) -> super::relay::Gateway {
    gateway_answering(
        move |g| {
            if g.head.starts_with("POST ") {
                let limit = Instant::now() + Duration::from_secs(20);
                while program_lives(&pid_file) && Instant::now() < limit {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
            OK.to_vec()
        },
        &["a-model", "b-model"],
    )
}

/// Submits a run of `late-demo` on the route `default` and waits for its end: its row and its events.
fn late_run(l: &super::default_confinement::Live) -> (Value, Vec<Value>) {
    let r = send(l.addr, "POST", "/daemon/commands/submit-run", &[("Content-Type", "application/json")], Some(json!({"runbook_id": "late-demo", "intake": "red or blue", "model_route": "default"}).to_string().as_bytes()));
    assert_eq!(r.status, 202, "{}", r.text());
    let run = r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned();
    let end = until_state(l, &run, &["Ended", "FailedToStart", "Cancelled"]);
    (end, rows(l, &format!("/daemon/views/events?run_id={run}&after_position=0")))
}

/// G2 of the wave: a run whose bound program failed (killed at its timeout among them) and left the tree with nothing to
/// do ends `failed: step <action> <end>`; G1: a step whose calls the relay booked releases its reservation. An agent node
/// writes its call directory before its model client starts and its cost line after the answer; killed at its timeout
/// while its call to the relay is in flight, it leaves the directory without the line (the shipped runbooks' agent nodes
/// have timeouts of 300 to 1,500 s). The relay books that call when the gateway answers. The step still failed at its
/// timeout: the run ends naming it, not `runtime-failure` for a cost record the relay's line replaces on a route.
#[test]
fn adversary_wave3_an_agent_step_killed_at_its_timeout_with_its_call_in_flight_ends_naming_the_step() {
    let pid_file = super::state::scratch("adv3-late-agent-pid").join("late.pid");
    let gw = gateway_after_the_program(pid_file.clone());
    let (h, l, _) = daemon_at("adv3-late-agent", &gw.base_url, &format!("{}{ROUTE}", model_table(Some(&claude_cli()))));
    late_runbook_with(&h.home, true, &pid_file);
    let (end, events) = late_run(&l);
    let why: Vec<Value> = events.iter().filter(|e| e["kind"] == "runbook.runtime-failed" || e["kind"] == "runbook.step-failed").map(|e| e["record"]["reason"].clone()).collect();
    let booked: Vec<Value> = events.iter().filter(|e| e["kind"] == "runbook.cost-booked").map(|e| e["record"].clone()).collect();
    assert_eq!(end["model_route"], "default", "{end}");
    assert_eq!(
        (end["state"].clone(), end["outcome"].clone()),
        (json!("Ended"), json!("failed: step late.write timed out after 1 s")),
        "the step killed at its timeout does not end the run naming it: reasons {why:?}, bookings {booked:?}"
    );
}

/// `docs/daemon-api.md` ("The relay of a run's model calls"), the cost of a call: "a declared price per million tokens,
/// else `cost_known: false` and the reservation: the step's largest, else what one call of the decider may cost, at most
/// what the cap has left"; G1 of the wave: a step whose calls the relay booked releases its reservation, so the relay's
/// line is the call's cost. The route `default` declares no price. The step `late.write` holds 12,300 micro USD and its
/// one call reaches the gateway, which answers after the program was killed at its timeout: the relay books that call
/// once (its line names the step), and its cost is the step's reservation, not the decider's nor what the run's cap of
/// 1,000,000,000 micro USD has left.
#[test]
fn adversary_wave3_a_call_of_unknown_cost_that_ends_after_its_step_was_killed_books_the_steps_reservation() {
    let pid_file = super::state::scratch("adv3-late-pid").join("late.pid");
    let gw = gateway_after_the_program(pid_file.clone());
    let (h, l, _) = daemon_at("adv3-late", &gw.base_url, &format!("{}{ROUTE}", model_table(Some(&claude_cli()))));
    late_runbook(&h.home, &pid_file);
    let r = send(l.addr, "POST", "/daemon/commands/submit-run", &[("Content-Type", "application/json")], Some(json!({"runbook_id": "late-demo", "intake": "red or blue", "model_route": "default"}).to_string().as_bytes()));
    assert_eq!(r.status, 202, "{}", r.text());
    let run = r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned();
    let end = until_state(&l, &run, &["Ended", "FailedToStart", "Cancelled"]);
    let events = rows(&l, &format!("/daemon/views/events?run_id={run}&after_position=0"));
    let calls: Vec<Value> = events.iter().filter(|e| e["kind"] == "runbook.model-call").map(|e| e["record"].clone()).collect();
    assert_eq!(end["model_route"], "default", "{end}");
    assert_eq!(calls.len(), 1, "the late call is not booked once as an event of the run: {end} {events:?}");
    assert_eq!((calls[0]["step"].clone(), calls[0]["cost_known"].clone()), (json!("late.write"), json!(false)), "{:?}", calls[0]);
    assert_eq!(calls[0]["micro_usd"], 12_300, "the call of the step late.write was not booked at the step's reservation: {:?}", calls[0]);
}

// ---- a classified delegation: the run record names its route ----

/// The routes `default` (alias `a-model`, the daemon's default) and `other` (alias `b-model`) on the lone gateway, and the
/// classifier route `classify` (alias `c-model`).
const ROUTES_AND_CLASSIFY: &str = "[routes.default]\ngateway = \"default\"\nmodel = \"a-model\"\nkind = \"hosted\"\n[routes.other]\ngateway = \"default\"\nmodel = \"b-model\"\nkind = \"hosted\"\n[routes.classify]\ngateway = \"default\"\nmodel = \"c-model\"\nkind = \"hosted\"\n[route_policy]\ndefault = \"default\"\nclassifier = \"classify\"\n";

fn messages(text: &str) -> Vec<u8> {
    let body = json!({"id": "m", "type": "message", "role": "assistant", "content": [{"type": "text", "text": text}], "usage": {"input_tokens": 7, "output_tokens": 3}}).to_string();
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).into_bytes()
}

fn delegated_run(l: &super::default_confinement::Live, body: &Value) -> String {
    let r = send(l.addr, "POST", "/daemon/commands/delegate", &[("Content-Type", "application/json")], Some(body.to_string().as_bytes()));
    assert_eq!(r.status, 202, "{}", r.text());
    r.json()["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned()
}

fn route_of(l: &super::default_confinement::Live, run: &str) -> (Value, Value, Value) {
    let r = rows(l, &format!("/daemon/views/run?run_id={run}"))[0].clone();
    (r["model_route"].clone(), r["model_route_alias"].clone(), r["model_route_source"].clone())
}

/// `docs/model-routes.md` section 6: "The run record names the route, the alias and the route's source (run, protocol or
/// default)"; the CHANGELOG entry of this wave: a delegation whose runbook the classifier picks "takes that runbook's
/// route entries ... as a delegation that names the runbook does". The runbook `answer-demo` gives the role `default` the
/// route `other`. A delegation that names it runs on `other` and its record says so, source `protocol`. A delegation the
/// classifier sends to the same runbook makes its calls on `other` too: its record must name that route, not the daemon's
/// default it did not take.
#[test]
fn adversary_wave3_a_classified_delegation_records_the_route_its_runbook_gave_it() {
    let classified = json!({"protocol": "answer-demo", "confidence": 0.9, "inputs": {}}).to_string();
    let gw = gateway_answering(move |g| if g.body["model"] == "c-model" { messages(&classified) } else { OK.to_vec() }, &["a-model", "b-model", "c-model"]);
    let (h, l, _) = daemon_at("adv3-classified", &gw.base_url, ROUTES_AND_CLASSIFY);
    std::fs::write(h.home.join("bindings/answer-demo.routes.yaml"), "format: runbook.routes/1\nmodel_routes:\n  default: {route: other}\n").unwrap();

    let named = delegated_run(&l, &json!({"request": "green or black", "protocol": "answer-demo"}));
    assert_eq!(until_ended(&l, &named)["outcome"], "answered");
    assert_eq!(route_of(&l, &named), (json!("other"), json!("b-model"), json!("protocol")), "the delegation naming the runbook");

    let run = delegated_run(&l, &json!({"request": "green or black"}));
    let end = until_ended(&l, &run);
    assert_eq!((end["outcome"].clone(), end["path"].clone()), (json!("answered"), json!("classified")), "{end}");
    let calls: Vec<(Value, Value)> = rows(&l, &format!("/daemon/views/events?run_id={run}&after_position=0"))
        .into_iter()
        .filter(|e| e["kind"] == "runbook.model-call" && e["record"]["role"] == "default")
        .map(|e| (e["record"]["route"].clone(), e["record"]["source"].clone()))
        .collect();
    assert_eq!(calls, vec![(json!("other"), json!("protocol"))], "the classified run's call of the role default");
    assert_eq!(route_of(&l, &run), (json!("other"), json!("b-model"), json!("protocol")), "the run record names a route the run's calls did not take");
}
