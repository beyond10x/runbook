//! The relay of a daemon's runs end to end (`docs/model-routes.md`, sections 4, 5 and 11): the daemon started as
//! `runbook serve`, its `[gateway]` the one route `default` with the model decider's model as its alias, a run whose
//! decider is `model` on the committed claude-cli adapter, and a stand-in `claude` that calls the address the adapter
//! gives it. Every call goes through the daemon's relay, is booked in the run's spend ledger and in its events, and no
//! process of the run holds the gateway's token.
//!
//! No model is called and nothing leaves loopback: the gateway is a stand-in on a port of the test's own.

use super::default_confinement::rows;
use super::gateway::{claude_calling, claude_cli, entries, gateway_table, holders, home, model_table, serve, serve_capped, submit, until_state, wait_until, write_exe, Home, TOKEN};
use super::relay::{gateway, got};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

const OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nx-llm-gateway-call: gw-call-7\r\nContent-Length: 56\r\nConnection: close\r\n\r\n{\"id\":\"m\",\"usage\":{\"input_tokens\":11,\"output_tokens\":5}}";
const STOPPED: &[u8] = b"HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nContent-Length: 61\r\nConnection: close\r\n\r\n{\"error\":{\"code\":\"model-stopped\",\"message\":\"the pod is off\"}}";

/// The home of [`home`] with the stand-in `claude` replaced by one that calls `roles` in order.
fn home_calling(tag: &str, roles: &str) -> Home {
    let h = home(tag);
    write_exe(&h.home.join("bindings/tools/claude"), &claude_calling(&h.log, roles));
    h
}

/// The lines of the run's spend ledger that are model calls.
fn call_lines(dir: &Path) -> Vec<Value> {
    let ledger: Value = serde_json::from_slice(&std::fs::read(dir.join("state/ledgers/autonomy-demo.json")).unwrap_or_default()).unwrap_or(Value::Null);
    ledger["entries"].as_array().cloned().unwrap_or_default().into_iter().filter(|e| e["what"] == "model call").collect()
}

fn events(l: &super::default_confinement::Live, run: &str, kind: &str) -> Vec<Value> {
    rows(l, &format!("/daemon/views/events?run_id={run}&after_position=0")).into_iter().filter(|e| e["kind"] == kind).map(|e| e["record"].clone()).collect()
}

fn read(log: &Path, name: &str) -> String {
    std::fs::read_to_string(log.join(name)).unwrap_or_default()
}

#[test]
fn a_model_call_of_a_daemon_run_goes_through_the_relay_reaches_the_gateway_with_the_alias_and_books_one_line() {
    let h = home_calling("relay-d-reach", r#""$model""#);
    let gw = gateway(OK);
    // the decider names what one of its calls may cost: a call of unknown cost outside a step books that
    let file = format!("{}{}max_budget_usd = 0.02\n", gateway_table(&gw.base_url, &h.token_file), model_table(Some(&claude_cli())));
    let l = serve("relay-d-reach-d", &h, &file, true);
    let run = submit(&l, "model");
    let end = until_state(&l, &run, &["Ended", "FailedToStart", "Cancelled"]);
    assert_eq!(end["state"], "Ended", "{end}; statuses: {}", read(&h.log, "statuses"));
    let seen = got(&gw);
    assert!(!seen.is_empty(), "the stand-in gateway was never reached");
    for g in &seen {
        assert_eq!(g.body["model"], "a-model", "the gateway got another model than the route's alias: {g:?}");
        assert!(g.head.starts_with("POST /v1/messages HTTP/1.1\r\n") && g.head.contains(&format!("Authorization: Bearer {TOKEN}\r\n")), "{}", g.head);
    }
    // the model process was given the relay's address, never the gateway's
    let env = read(&h.log, "claude-env-all");
    let base = env.lines().find_map(|l| l.strip_prefix("ANTHROPIC_BASE_URL=")).expect("claude was given no base address").to_owned();
    assert!(base.ends_with(&format!("/daemon/relay/{run}")) && !base.contains(gw.base_url.trim_start_matches("http://")), "{base}");
    // one ledger line per call, with the route, the alias, the tokens, the call id and the cost
    let lines = call_lines(&l.dir);
    assert_eq!(lines.len(), seen.len(), "{lines:?}");
    for line in &lines {
        assert_eq!((line["run"].clone(), line["role"].clone(), line["route"].clone(), line["alias"].clone()), (json!(run), json!("default"), json!("default"), json!("a-model")), "{line}");
        assert_eq!((line["input_tokens"].clone(), line["output_tokens"].clone(), line["gateway_call"].clone()), (json!(11), json!(5), json!("gw-call-7")), "{line}");
        assert_eq!((line["cost_known"].clone(), line["micro_usd"].clone()), (json!(false), json!(20_000)), "no price declared: the decider call books what one call of the decider may cost (0.02 USD): {line}");
    }
    let booked = events(&l, &run, "runbook.model-call");
    assert_eq!(booked.len(), seen.len(), "the run's record has no line per call: {booked:?}");
    assert_eq!(booked[0]["alias"], "a-model");
}

#[test]
fn a_role_without_a_route_and_a_call_past_max_llm_calls_are_refused_by_the_relay_and_reach_no_gateway() {
    let h = home_calling("relay-d-refuse", r#"writer "$model" "$model""#);
    let gw = gateway(OK);
    let file = format!("{}{}", gateway_table(&gw.base_url, &h.token_file), model_table(Some(&claude_cli())));
    let l = serve_capped("relay-d-refuse-d", &h, &file, true, "1");
    let run = submit(&l, "model");
    wait_until("the stand-in claude to make its three calls", || read(&h.log, "statuses").lines().count() >= 3);
    let statuses: Vec<String> = read(&h.log, "statuses").lines().map(str::to_owned).collect();
    let bodies: Vec<Value> = read(&h.log, "bodies").lines().map(|b| serde_json::from_str(b).unwrap_or(Value::Null)).collect();
    assert!(statuses[0].contains(" 400 ") && bodies[0]["error"]["code"] == "route-refused", "a role without a route: {} {}", statuses[0], bodies[0]);
    assert!(statuses[1].contains(" 200 "), "the run's own role: {}", statuses[1]);
    assert!(statuses[2].contains(" 429 ") && bodies[2]["error"]["code"] == "cap-reached", "the call past max_llm_calls 1: {} {}", statuses[2], bodies[2]);
    assert_eq!(got(&gw).len(), 1, "a refused call reached the gateway");
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(call_lines(&l.dir).len(), 1, "a refused call was booked: {:?}", call_lines(&l.dir));
    let _ = until_state(&l, &run, &["Ended", "WaitingOnDecision", "Cancelled", "FailedToStart"]);
}

#[test]
fn a_gateway_refusal_fails_the_call_route_unavailable_naming_the_route_the_code_and_the_setting_and_calls_no_other_route() {
    let h = home_calling("relay-d-down", r#""$model""#);
    let gw = gateway(STOPPED);
    let file = format!("{}{}", gateway_table(&gw.base_url, &h.token_file), model_table(Some(&claude_cli())));
    let l = serve("relay-d-down-d", &h, &file, true);
    let run = submit(&l, "model");
    wait_until("the stand-in claude to make its call", || !read(&h.log, "statuses").is_empty());
    wait_until("the failed call to be in the run's record", || !events(&l, &run, "runbook.model-call").is_empty());
    assert!(read(&h.log, "statuses").contains(" 503 "), "{}", read(&h.log, "statuses"));
    assert_eq!(read(&h.log, "bodies").trim(), "{\"error\":{\"code\":\"model-stopped\",\"message\":\"the pod is off\"}}", "the refusal is passed back unchanged");
    let failed = &events(&l, &run, "runbook.model-call")[0];
    assert_eq!((failed["cause"].clone(), failed["route"].clone(), failed["code"].clone(), failed["source"].clone()), (json!("route-unavailable"), json!("default"), json!("model-stopped"), json!("default")), "{failed}");
    assert_eq!(got(&gw).len(), 1, "the call was sent again");
    let line = &call_lines(&l.dir)[0];
    assert_eq!((line["cause"].clone(), line["code"].clone(), line["micro_usd"].clone()), (json!("route-unavailable"), json!("model-stopped"), json!(0)), "{line}");
    let r = super::http::send(l.addr, "POST", "/daemon/commands/cancel-run", &[("Content-Type", "application/json")], Some(json!({"run_id": run}).to_string().as_bytes()));
    assert!(r.status < 300, "cancel-run: {}", r.status);
    until_state(&l, &run, &["Cancelled", "Ended"]);
}

#[test]
fn the_route_file_holds_the_relays_address_and_the_runs_token_and_no_process_of_the_run_holds_the_gateways_token() {
    let h = home("relay-d-file");
    let gw = gateway(OK);
    // a bound program of the run that keeps its environment, then does its work
    let tools = h.home.join("bindings/tools");
    std::fs::rename(tools.join("notes-write.sh"), tools.join("notes-write-real.sh")).unwrap();
    write_exe(&tools.join("notes-write.sh"), &format!("#!/bin/sh\nenv > '{log}/bound-env'\nexec \"$(dirname \"$0\")/notes-write-real.sh\" \"$@\"\n", log = h.log.display()));
    // a model adapter that keeps its environment and what the route file is, and fails: the decision stays open
    let adapter = h.log.join("adapter.sh");
    write_exe(
        &adapter,
        &format!(
            "#!/bin/sh\nenv > '{log}/adapter-env'\nf=\"$RUNBOOK_ROUTE_FILE\"\nstat -c '%a' \"$f\" > '{log}/file-mode'\nstat -c '%a' \"$(dirname \"$f\")\" > '{log}/dir-mode'\nls -A \"$(dirname \"$f\")\" > '{log}/dir-ls'\ncat \"$f\" > '{log}/route-file'\nexit 1\n",
            log = h.log.display()
        ),
    );
    let file = format!("{}{}", gateway_table(&gw.base_url, &h.token_file), model_table(Some(adapter.to_str().unwrap())));
    let l = serve("relay-d-file-d", &h, &file, true);
    let run = submit(&l, "model");
    wait_until("the adapter to run", || h.log.join("route-file").exists());
    std::thread::sleep(Duration::from_millis(300));
    let env = read(&h.log, "adapter-env");
    let file_var = env.lines().find_map(|l| l.strip_prefix("RUNBOOK_ROUTE_FILE=")).expect("the model adapter was given no route file").to_owned();
    assert!(Path::new(&file_var).starts_with(h.xdg.join("runbook")) && file_var.ends_with("/route.json"), "{file_var}");
    let r = |name: &str| read(&h.log, name).trim().to_owned();
    assert_eq!((r("file-mode"), r("dir-mode"), r("dir-ls")), ("600".to_owned(), "700".to_owned(), "route.json".to_owned()));
    let route: Value = serde_json::from_str(&r("route-file")).unwrap();
    assert_eq!(route["format"], "runbook.route/1");
    assert!(route["base_url"].as_str().unwrap().starts_with(&format!("http://{}", l.addr)) && route["base_url"].as_str().unwrap().ends_with(&format!("/daemon/relay/{run}")), "{route}");
    assert_eq!(route["routes"], json!({"default": {"route": "default", "model": "runbook-default"}}), "{route}");
    let run_token = route["token"].as_str().unwrap().to_owned();
    assert!(run_token.len() >= 32 && run_token != TOKEN, "the route file holds no token of the run's own");
    // the bound program of the run was given the same file and nothing else of the route
    let bound = read(&h.log, "bound-env");
    assert_eq!(bound.lines().find_map(|l| l.strip_prefix("RUNBOOK_ROUTE_FILE=")), Some(file_var.as_str()), "a bound program was given no route file: {bound}");
    // no process of the run holds the gateway's token, and no file of the daemon, the run or its private directory
    for (what, text) in [("the adapter", &env), ("a bound program", &bound)] {
        assert!(!text.contains(TOKEN) && !text.contains("SDC_GATEWAY_FILE") && !text.contains("ANTHROPIC_") && !text.contains(&gw.base_url), "{what} holds the gateway: {text}");
    }
    assert!(!r("route-file").contains(TOKEN) && !r("route-file").contains(&gw.base_url));
    assert_eq!(holders(&l.dir, TOKEN), Vec::<PathBuf>::new(), "the token is in a file of the daemon's state");
    assert_eq!(holders(&h.xdg, TOKEN), Vec::<PathBuf>::new(), "the token is in the run's private directory");
    assert!(got(&gw).is_empty(), "the stand-in adapter calls nothing");
    let c = super::http::send(l.addr, "POST", "/daemon/commands/cancel-run", &[("Content-Type", "application/json")], Some(json!({"run_id": run}).to_string().as_bytes()));
    assert!(c.status < 300, "cancel-run: {}", c.status);
    until_state(&l, &run, &["Cancelled"]);
    wait_until("the run's private directory to go", || entries(&h.xdg.join("runbook")).is_empty());
    // the run's token is void once the run ended
    let relay = super::http::send(l.addr, "POST", &format!("/daemon/relay/{run}/v1/messages"), &[("Content-Type", "application/json"), ("Authorization", &format!("Bearer {run_token}"))], Some(b"{\"model\":\"default\"}"));
    assert_eq!(relay.status, 401, "{}", relay.text());
    assert!(got(&gw).is_empty());
}
