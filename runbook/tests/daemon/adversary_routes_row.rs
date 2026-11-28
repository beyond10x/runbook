//! Adversary case of the story `the-routes-row-says-whether-a-new-run-would-join-a-wake`: the member
//! `wake_joinable_until` of a route row of the view `routes` read against what the ESS summary and `docs/daemon-api.md`
//! promise a client.
//!
//! In process, on the daemon's test clock, with the stub runtime and the stand-in gateway of `route_wake.rs` on a
//! loopback port of the test's own: no model call, nothing leaves loopback.

use super::route_wake::{answer, daemon_in, route_row, submit, until_records, until_state, wake_on, Gateway};
use runbook::clock::SetClock;
use runbook::commands::SubmitRun;
use runbook::runtime::Autonomy;
use serde_json::json;
use std::sync::Arc;

/// `wake_joinable_until` (ESS `sdc.daemon.ModelRoute`, `docs/daemon-api.md` "The day's figures of a route"): "a run
/// submitted before this instant joins the wake". A run submitted at 10:05, inside the window the row names (until
/// 10:10), behind a serial key another live run holds, takes its key at 10:10 and sends a wake of its own: the join is
/// decided when the run's thread wakes the route, not at the submit the row speaks of.
#[test]
fn adversary_routes_row_a_run_submitted_before_wake_joinable_until_joins_the_wake() {
    let gw = Gateway::start("stopped", (202, json!({"model": "qwen", "phase": "starting"})));
    let dir = super::state::scratch("adv-row-join-keyed");
    let clock = Arc::new(SetClock::at("2026-11-26T10:00:00Z").unwrap());
    let d = daemon_in(&dir, &gw, wake_on(500, 100), clock.clone());
    // the key holder: a run on the hosted route that waits on one decision
    let o = d.submit_run(SubmitRun { runbook_id: "held".into(), intake: "x".into(), autonomy: Some(Autonomy::A5), decider: Some("model".into()), model_route: Some("claude".into()), serial_key: Some("k".into()), ..SubmitRun::default() }).unwrap();
    let holder = o.run_id.unwrap();
    until_state(&d, &holder, "WaitingOnDecision");

    let first = submit(&d, "qwen");
    until_records(&d, &first, "runbook.route-wake-answered");
    clock.set("2026-11-26T10:05:00Z");
    let row = route_row(&d, "qwen");
    assert_eq!((row["wake_joinable"].clone(), row["wake_joinable_until"].clone()), (json!(true), json!("2026-11-26T10:10:00.000Z")), "{row}");

    // submitted now, before wake_joinable_until
    let o = d.submit_run(SubmitRun { runbook_id: "quick".into(), intake: "x".into(), autonomy: Some(Autonomy::A5), decider: Some("model".into()), model_route: Some("qwen".into()), serial_key: Some("k".into()), ..SubmitRun::default() }).unwrap();
    let keyed = o.run_id.unwrap();
    until_state(&d, &keyed, "WaitingOnKey");
    clock.set("2026-11-26T10:10:00Z");
    answer(&d, &holder);
    let wake = until_records(&d, &keyed, "runbook.route-wake");
    assert_eq!(wake[0]["reason"], "joined a wake sent within the wait", "a run submitted before wake_joinable_until did not join the wake the row named: {wake:?}");
    gw.no_call();
}
