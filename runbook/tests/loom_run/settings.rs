//! The one settings format, `runbook.loom-run-settings/1` (`docs/loom-run-settings.md`), and what a
//! run does with the file `sdc-experiment run` writes today (unit R4, commit 970db1f7, function
//! `loom_settings_json`; its fixture is `fixed()` in `driver/tests/experiment_loom.rs`).

use super::core::{code, stderr, World};
use super::decider::r4_settings;
use serde_json::{json, Value};

fn write(w: &World, v: &Value) {
    std::fs::write(w.root.join("settings.json"), serde_json::to_string_pretty(v).unwrap()).unwrap();
}

/// Runs with `settings` and asserts a refusal (exit 2, nothing written, no call) that names `needle`.
fn refused(tag: &str, mutate: impl Fn(&mut Value), needle: &str) {
    let w = World::new(tag);
    let mut v = r4_settings(&w, 5_000_000);
    mutate(&mut v);
    write(&w, &v);
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 2, "{needle}: {}", stderr(&o));
    assert!(stderr(&o).contains(needle), "{needle} not named: {}", stderr(&o));
    assert!(!w.run_dir("r1").exists(), "{needle}: a run directory was made");
    assert_eq!(w.fake.calls_so_far(), 0, "{needle}: the binary was called");
    assert!(!w.ledger().exists(), "{needle}: the ledger was made");
}

#[test]
fn the_file_r4_writes_today_is_refused_for_the_decider_kind_it_does_not_name() {
    refused("r4-today", |_| {}, "decider.kind");
}

#[test]
fn the_file_r4_writes_today_with_a_kind_still_stops_at_the_first_decision_for_a_person() {
    // Everything R4 writes is accepted (format, run, loom{..., login_file}, models, the two caps, the
    // decider's principal and model); with `decider.kind` and no `autonomy` the decider is built and
    // the run stops at the first decision: the model decider answers only at autonomy A5.
    let w = World::new("r4-kind");
    std::fs::create_dir_all(w.root.join("bindings/tools")).unwrap();
    std::fs::write(w.root.join("bindings/tools/decider.sh"), "#!/bin/sh\n").unwrap();
    std::fs::create_dir_all(w.root.join("instructions/decider")).unwrap();
    std::fs::write(w.root.join("instructions/decider/x.md"), "x\n").unwrap();
    std::fs::write(w.root.join("sdc-reply"), "x").unwrap();
    let mut v = r4_settings(&w, 5_000_000);
    v["decider"]["kind"] = json!("model");
    v["decider"]["without_claims"] = json!(true);
    v["decider"]["reply_bin"] = json!(w.root.join("sdc-reply").display().to_string());
    write(&w, &v);
    w.one_decision_tree();
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 8, "{}", stderr(&o));
    let reason = w.outcome("r1")["reason"].as_str().unwrap().to_owned();
    assert!(reason.contains("no autonomy level named"), "{reason}");
}

#[test]
fn a_member_that_repeats_an_option_must_agree_with_it_and_is_refused_by_name() {
    refused("sha", |v| v["loom"]["sha256"] = json!("00".repeat(32)), "loom.sha256");
    refused("conf", |v| v["loom"]["confinement"] = json!("substrate"), "loom.confinement");
    refused("cap", |v| v["loom"]["cap_micro_usd"] = json!(1), "loom.cap_micro_usd");
    refused("calls", |v| v["max_llm_calls"] = json!(150), "max_llm_calls");
    refused("wall", |v| v["max_wall_seconds"] = json!(3600), "max_wall_seconds");
    refused("calls-null", |v| v["max_llm_calls"] = Value::Null, "max_llm_calls");
    refused("run", |v| v["run"] = json!("another"), "is not the run named by --run");
    refused("format", |v| v["format"] = json!("runbook.loom-run-settings/2"), "is not runbook.loom-run-settings/1");
    refused("bin", |v| v["loom"]["bin"] = json!("/opt/loom/b10x-loom"), "loom.bin");
    refused("loom-key", |v| v["loom"]["extra"] = json!(1), "loom.extra");
}

#[test]
fn the_login_file_is_given_once_and_the_deciders_program_is_the_runners() {
    refused(
        "login-twice",
        |v| v["login_file"] = json!("/somewhere/else"),
        "login_file and loom.login_file",
    );
    refused(
        "program",
        |v| {
            v["decider"]["kind"] = json!("model");
            v["decider"]["program"] = json!("/opt/decider.sh");
        },
        "decider.program",
    );
    refused("principal", |v| {
        v["decider"]["kind"] = json!("model");
        v["decider"]["principal"] = json!("person:owner");
    }, "decider.principal");
}
