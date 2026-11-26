//! A serial key (`story:a-run-waits-for-its-serial-key`): a submit or a delegation that names `serial_key` waits while a
//! live run with the same key that was submitted before it has not ended; runs with another key or none are not held.
//! In process over the stub runtime: `hold` runs until it is cancelled, `quick` ends at once.

use super::commands::Fx;
use runbook::commands::{Delegate, Fault, SubmitRun};
use runbook::stub::{Script, Step};
use serde_json::Value;

fn keyed(book: &str, key: Option<&str>) -> SubmitRun {
    SubmitRun {
        runbook_id: book.into(),
        intake: "x".into(),
        serial_key: key.map(str::to_owned),
        ..SubmitRun::default()
    }
}

fn key_wait(fx: &Fx, run: &str) -> Value {
    fx.d().run_detail(run)[0]
        .get("key_wait")
        .cloned()
        .unwrap_or(Value::Null)
}

fn behind(key: &str, run: &str) -> Value {
    Value::String(format!("waiting on key {key} behind run {run}"))
}

#[test]
fn runs_with_one_key_run_one_at_a_time_in_submit_order_and_a_cancel_of_the_holder_starts_the_next()
{
    let fx = Fx::new("serial-order");
    let a = fx.submit_with(keyed("hold", Some("k")));
    let b = fx.submit_with(keyed("hold", Some("k")));
    let c = fx.submit_with(keyed("quick", Some("k")));
    fx.wait_state(&a, "Running");
    fx.wait_state(&b, "WaitingOnKey");
    fx.wait_state(&c, "WaitingOnKey");
    assert_eq!(key_wait(&fx, &b), behind("k", &a));
    assert_eq!(key_wait(&fx, &c), behind("k", &b));
    assert_eq!(fx.d().run_detail(&b)[0]["serial_key"], "k");
    assert!(key_wait(&fx, &a).is_null(), "the holder waits on nothing");

    assert_eq!(fx.cancel(&a).name, "cancelled");
    fx.wait_state(&b, "Running");
    assert!(key_wait(&fx, &b).is_null());
    assert_eq!(
        fx.state_of(&c),
        "WaitingOnKey",
        "the third waits while the second runs"
    );
    assert_eq!(key_wait(&fx, &c), behind("k", &b));

    assert_eq!(fx.cancel(&b).name, "cancelled");
    fx.wait_state(&c, "Ended");
    let kinds: Vec<String> = fx
        .d()
        .events_after(&c, 0)
        .iter()
        .map(|e| e["kind"].as_str().unwrap().to_owned())
        .collect();
    assert!(
        kinds.contains(&"runbook.waiting-on-key".to_owned())
            && kinds.contains(&"runbook.key-taken".to_owned()),
        "{kinds:?}"
    );
}

#[test]
fn runs_with_two_keys_and_a_run_with_none_are_not_held() {
    let fx = Fx::new("serial-two");
    let a = fx.submit_with(keyed("hold", Some("a")));
    let b = fx.submit_with(keyed("hold", Some("b")));
    let none = fx.submit_with(keyed("hold", None));
    for run in [&a, &b, &none] {
        fx.wait_state(run, "Running");
    }
}

#[test]
fn a_cancel_of_a_waiting_run_takes_it_out_of_the_queue() {
    let fx = Fx::new("serial-cancel-waiting");
    let a = fx.submit_with(keyed("hold", Some("k")));
    let b = fx.submit_with(keyed("hold", Some("k")));
    let c = fx.submit_with(keyed("hold", Some("k")));
    fx.wait_state(&a, "Running");
    fx.wait_state(&c, "WaitingOnKey");
    assert_eq!(fx.cancel(&b).name, "cancelled");
    assert_eq!(fx.state_of(&b), "Cancelled");
    assert_eq!(
        key_wait(&fx, &c),
        behind("k", &a),
        "the run behind the cancelled one waits on the run ahead of it"
    );
    assert_eq!(fx.cancel(&a).name, "cancelled");
    fx.wait_state(&c, "Running");
}

#[test]
fn a_run_that_fails_to_start_frees_the_key() {
    let fx = Fx::new("serial-fail");
    fx.stub.script(
        "fails",
        Script::new(vec![Step::FailToStart("no start".into())]),
    );
    let a = fx.submit_with(keyed("hold", Some("k")));
    let b = fx.submit_with(keyed("fails", Some("k")));
    let c = fx.submit_with(keyed("quick", Some("k")));
    fx.wait_state(&a, "Running");
    fx.wait_state(&c, "WaitingOnKey");
    fx.cancel(&a);
    fx.wait_state(&b, "FailedToStart");
    fx.wait_state(&c, "Ended");
}

#[test]
fn the_queue_of_a_key_holds_across_a_restart() {
    let mut fx = Fx::new("serial-restart");
    let a = fx.submit_with(keyed("hold", Some("k")));
    let b = fx.submit_with(keyed("hold", Some("k")));
    let c = fx.submit_with(keyed("quick", Some("k")));
    fx.wait_state(&a, "Running");
    fx.wait_state(&b, "WaitingOnKey");
    fx.wait_state(&c, "WaitingOnKey");
    fx.restart();
    let row = &fx.d().run_detail(&a)[0];
    assert_eq!(
        (row["state"].as_str(), row["outcome"].as_str()),
        (Some("Ended"), Some("orphaned")),
        "the holder's runtime is gone"
    );
    fx.wait_state(&b, "Running");
    assert_eq!(
        fx.state_of(&c),
        "WaitingOnKey",
        "the waiting runs keep their order across the restart"
    );
    assert_eq!(key_wait(&fx, &c), behind("k", &b));
    fx.cancel(&b);
    fx.wait_state(&c, "Ended");
}

#[test]
fn a_delegation_waits_for_its_key_too() {
    let fx = Fx::new("serial-delegate");
    let delegate = |key: &str| {
        let o = fx
            .d()
            .delegate(Delegate {
                request: "x".into(),
                model_route: Some("default".into()),
                protocol: Some("hold".into()),
                serial_key: Some(key.into()),
                ..Delegate::default()
            })
            .unwrap();
        assert_eq!(o.name, "delegated", "{o:?}");
        o.run_id.unwrap()
    };
    let first = delegate("k");
    let second = delegate("k");
    let run = fx.submit_with(keyed("quick", Some("k")));
    fx.wait_state(&first, "Running");
    fx.wait_state(&second, "WaitingOnKey");
    assert_eq!(key_wait(&fx, &second), behind("k", &first));
    assert_eq!(
        key_wait(&fx, &run),
        behind("k", &second),
        "a submit and a delegation share one queue per key"
    );
}

#[test]
fn a_key_is_one_to_200_characters_and_a_refused_one_writes_nothing() {
    let fx = Fx::new("serial-bound");
    for bad in [String::new(), "k".repeat(201)] {
        let r = fx.d().submit_run(keyed("quick", Some(&bad)));
        assert!(
            matches!(r, Err(Fault::Invalid(ref m)) if m.contains("serial_key")),
            "{r:?}"
        );
        let r = fx.d().delegate(Delegate {
            request: "x".into(),
            model_route: Some("default".into()),
            protocol: Some("quick".into()),
            serial_key: Some(bad.clone()),
            ..Delegate::default()
        });
        assert!(
            matches!(r, Err(Fault::Invalid(ref m)) if m.contains("serial_key")),
            "{r:?}"
        );
    }
    assert!(fx.d().runs().is_empty(), "a refused key makes no run");
    let run = fx.submit_with(keyed("quick", Some(&"k".repeat(200))));
    fx.wait_state(&run, "Ended");
}
