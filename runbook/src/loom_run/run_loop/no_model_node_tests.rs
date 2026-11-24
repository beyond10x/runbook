//! The decision of the opening "model node or not". It gates the login link only (`model_node.rs`):
//! no cap, level or option refuses or limits a run by it. No model call and no network request.

use super::tests::{args, world};
use crate::loom_run::model_node::{decide, ModelNode};

#[test]
fn a_re_drive_a_replay_setting_conformance_and_a_decider_that_may_call_a_model_count_at_the_opening() {
    use crate::loom_run::decide::{RefusingDecider, ScriptedDecider};
    let (root, f) = world("p-decide");
    let bindings = root.join("bindings");
    for level in [crate::loom_run::Confinement::Local, crate::loom_run::Confinement::Substrate] {
        let mut plain = args(&root, &f, 600);
        plain.confinement = level;
        assert_eq!(decide(&plain, &serde_json::json!({}), &RefusingDecider, &bindings), ModelNode::Without, "nothing in this run names a model, at any level: {}", level.as_arg());
    }
    let plain = args(&root, &f, 600);
    let mut redrive = args(&root, &f, 600);
    redrive.ports.replay.recorded = Some(root.join("recording"));
    assert!(decide(&redrive, &serde_json::json!({}), &RefusingDecider, &bindings).has_model_node(), "a re-drive can go live");
    assert!(decide(&plain, &serde_json::json!({"replay": {"live_adapter": "x"}}), &RefusingDecider, &bindings).has_model_node(), "a replay setting can go live");
    let mut observe = args(&root, &f, 600);
    observe.ports.conformance = crate::loom_run::ports::Conformance::Observe;
    assert!(decide(&observe, &serde_json::json!({}), &RefusingDecider, &bindings).has_model_node(), "the observation starts a model");
    let reserving = ScriptedDecider::new(vec![], 5);
    assert!(decide(&plain, &serde_json::json!({}), &reserving, &bindings).has_model_node(), "a decider that reserves an amount");
    assert!(decide(&plain, &serde_json::json!({}), &RefusingDecider, &root.join("absent")).has_model_node(), "bindings that cannot be read");
}

#[test]
fn the_decision_of_the_opening_does_not_depend_on_a_cap() {
    use crate::loom_run::decide::RefusingDecider;
    let (root, f) = world("p-decide-caps");
    let bindings = root.join("bindings");
    let mut none = args(&root, &f, 600);
    none.spend_ledger = None;
    none.cap_micro_usd = None;
    none.max_llm_calls = None;
    let mut zero = none.clone();
    zero.cap_micro_usd = Some(0);
    zero.max_llm_calls = Some(0);
    for a in [none, zero, args(&root, &f, 600)] {
        assert_eq!(decide(&a, &serde_json::json!({}), &RefusingDecider, &bindings), ModelNode::Without);
    }
}
