//! Adversary cases of the story `the-routes-row-says-whether-a-new-run-would-join-a-wake`: the member
//! `wake_joinable_until` of a route row of the view `routes` read against what the ESS summary and `docs/daemon-api.md`
//! promise a client. The join is decided when a run wakes the route, not at its submit (decision D19 of the unit's
//! coordinator), and a run behind a serial key wakes the route only once it holds the key.
//!
//! In process, on the daemon's test clock, with the stub runtime and the stand-in gateway of `route_wake.rs` on a
//! loopback port of the test's own: no model call, nothing leaves loopback.

use super::route_wake::{answer, daemon_in, records, route_row, submit, until_records, until_state, wake_on, Gateway};
use runbook::clock::SetClock;
use runbook::commands::{Daemon, SubmitRun};
use runbook::runtime::Autonomy;
use serde_json::{json, Value};
use std::sync::Arc;

/// A daemon whose key `k` is held by a run on the hosted route that waits on one decision, a wake of `qwen` taken at
/// 10:00 that the row names joinable until 10:10, and a run of the key on `qwen` submitted at 10:05, inside that window,
/// that waits for the key: the daemon, its clock, the stand-in, the holder and the keyed run.
fn keyed_inside_the_window(tag: &str) -> (Daemon, Arc<SetClock>, Gateway, String, String) {
    let gw = Gateway::start("stopped", (202, json!({"model": "qwen", "phase": "starting"})));
    let dir = super::state::scratch(tag);
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    let d = daemon_in(&dir, &gw, wake_on(500, 100), clock.clone());
    let o = d.submit_run(SubmitRun { runbook_id: "held".into(), intake: "x".into(), autonomy: Some(Autonomy::A5), decider: Some("model".into()), model_route: Some("claude".into()), serial_key: Some("k".into()), ..SubmitRun::default() }).unwrap();
    let holder = o.run_id.unwrap();
    until_state(&d, &holder, "WaitingOnDecision");

    let first = submit(&d, "qwen");
    until_records(&d, &first, "runbook.route-wake-answered");
    clock.set("2026-11-26T10:05:00Z");
    let row = route_row(&d, "qwen");
    assert_eq!((row["wake_joinable"].clone(), row["wake_joinable_until"].clone()), (json!(true), json!("2026-11-26T10:10:00.000Z")), "{row}");

    let o = d.submit_run(SubmitRun { runbook_id: "quick".into(), intake: "x".into(), autonomy: Some(Autonomy::A5), decider: Some("model".into()), model_route: Some("qwen".into()), serial_key: Some("k".into()), ..SubmitRun::default() }).unwrap();
    let keyed = o.run_id.unwrap();
    until_state(&d, &keyed, "WaitingOnKey");
    assert!(records(&d, &keyed, "runbook.route-wake").is_empty(), "a run waiting for its key woke the route");
    (d, clock, gw, holder, keyed)
}

/// A run submitted before `wake_joinable_until` behind a serial key that it gets at that instant wakes the route then,
/// at or after the end of the window: it joins nothing and sends and books its own wake.
#[test]
fn adversary_routes_row_a_keyed_run_submitted_inside_the_window_that_gets_its_key_at_wake_joinable_until_sends_its_own_wake() {
    let (d, clock, gw, holder, keyed) = keyed_inside_the_window("adv-row-join-keyed-late");
    clock.set("2026-11-26T10:10:00Z");
    answer(&d, &holder);
    let wake = until_records(&d, &keyed, "runbook.route-wake");
    assert_eq!(wake, vec![json!({"route": "qwen", "sent": true, "day": "2026-11-26", "cost_micro_usd": 100, "booked_micro_usd": 200, "cap_micro_usd": 500})], "the run joined a wake past its window");
    assert_eq!(gw.wakes(), 2);
    gw.no_call();
}

/// The same run getting its key before `wake_joinable_until` wakes the route inside the window and joins the wake the
/// row named.
#[test]
fn adversary_routes_row_a_keyed_run_that_gets_its_key_before_wake_joinable_until_joins_the_wake() {
    let (d, clock, gw, holder, keyed) = keyed_inside_the_window("adv-row-join-keyed-early");
    let sent_at: Value = route_row(&d, "qwen")["wake_joinable_sent_at"].clone();
    clock.set("2026-11-26T10:07:00Z");
    answer(&d, &holder);
    let wake = until_records(&d, &keyed, "runbook.route-wake");
    assert_eq!((wake[0]["reason"].clone(), wake[0]["joined_at"].clone()), (json!("joined a wake sent within the wait"), sent_at), "{wake:?}");
    assert_eq!(gw.wakes(), 1);
    gw.no_call();
}
