//! Scripted cycles of `sdc-conform observe` (docs/conformance.md) with a stub product and a stub
//! table node (`examples/conformance-stub-product.rs`), and the route table check. They need the
//! pinned ess (driver/ess-pin.json): `sdc-conform ess-install` once, or `ESS_BIN`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{json, Value as J};

const BIN: &str = env!("CARGO_BIN_EXE_sdc-conform");

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/conformance")
}

fn stub() -> PathBuf {
    let p = Path::new(BIN).parent().unwrap().join("examples/conformance-stub-product");
    assert!(p.is_file(), "{} is missing: cargo test builds the examples", p.display());
    p
}

fn ess() -> PathBuf {
    let o = Command::new(BIN).arg("ess-path").output().unwrap();
    assert!(o.status.success(), "the pinned ess is not installed (run `sdc-conform ess-install` or set ESS_BIN): {}", String::from_utf8_lossy(&o.stderr));
    PathBuf::from(String::from_utf8_lossy(&o.stdout).trim())
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().flatten() {
        let d = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() { copy(&e.path(), &d) } else { fs::copy(e.path(), &d).unwrap(); }
    }
}

fn tmp(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("conformance").join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// An ended run with the fixture specification `spec` and a delivered tree holding the stub product.
fn run_dir(name: &str, spec: &str) -> PathBuf {
    let run = tmp(name).join(name);
    let work = run.join("store/IDEA-1/work");
    copy(&fixtures().join(spec), &work.join("spec"));
    fs::create_dir_all(work.join("published/bin")).unwrap();
    fs::copy(stub(), work.join("published/bin/stub-product")).unwrap();
    fs::write(work.join("published/README.md"), "Stub chores service: bin/stub-product <port> <data dir>.\n").unwrap();
    fs::write(work.join("published/PROVENANCE"), "stub\n").unwrap();
    run
}

fn table() -> J {
    serde_json::from_str(&fs::read_to_string(fixtures().join("table.json")).unwrap()).unwrap()
}

fn write_table(dir: &Path, name: &str, t: &J) -> String {
    let p = dir.join(name);
    fs::write(&p, t.to_string()).unwrap();
    p.display().to_string()
}

fn with_mode(mode: &str) -> J {
    let mut t = table();
    t["start"]["argv"][3] = json!(mode);
    t
}

fn observe(run: &Path, tables: &str, extra: &[(&str, &str)]) -> (Output, J) {
    let mut c = Command::new(BIN);
    c.arg("observe").arg(run)
        .env("SDC_CONFORMANCE_ADAPTER", stub())
        .env("SDC_STUB_TABLE_NODE", tables)
        .env("SDC_CONFORMANCE_TARGET_TIMEOUT", "10")
        .env_remove("SDC_LLM_MODEL_CONFORMANCE")
        .env_remove("SDC_LLM_EFFORT_CONFORMANCE")
        .env_remove("SDC_LLM_EFFORT_OTHER")
        .env_remove("SDC_LLM_EFFORT");
    for (k, v) in extra {
        c.env(k, v);
    }
    let o = c.output().unwrap();
    let oracle = fs::read_to_string(run.join("oracle.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(J::Null);
    (o, oracle)
}

fn says(o: &Output) -> String {
    format!("stdout {} stderr {}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

#[test]
fn every_scenario_passes_and_the_check_holds() {
    ess();
    let run = run_dir("all-pass", "spec");
    let t = write_table(run.parent().unwrap(), "t.json", &table());
    let (o, d) = observe(&run, &t, &[]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    assert_eq!(d["format"], "sdc.oracle/1");
    assert_eq!(d["checks"][0], json!({"id": "conformance", "result": "held", "k": 6, "n": 6, "cause": null}), "{c:#}");
    assert_eq!(c["summary"], "6 of 6 scenarios");
    assert_eq!(c["conformance_status"], "passed");
    assert_eq!(c["events_checked_from"], "response_body");
    assert_eq!(c["table"]["source"], "conformance.table");
    assert_eq!(c["table"]["model"], "claude-sonnet-5-5");
    assert_eq!(c["table"]["model_source"], "default");
    assert!(["low", "medium", "high", "xhigh", "max"].contains(&c["table"]["effort"].as_str().unwrap()), "{c:#}");
    assert_eq!(c["table"]["attempts"], 1);
    assert_eq!(c["product"]["tree"], "store/IDEA-1/work/published");
    assert!(run.join("conformance-detail.json").is_file());
    assert!(String::from_utf8_lossy(&o.stdout).contains("conformance: 6 of 6 scenarios"), "{}", says(&o));
    let text = fs::read_to_string(run.join("oracle.json")).unwrap();
    assert!(!text.contains(run.to_str().unwrap()), "oracle.json holds an absolute path");
}

#[test]
fn one_deviation_is_contradicted_with_its_expected_and_observed() {
    ess();
    let run = run_dir("deviation", "spec");
    let t = write_table(run.parent().unwrap(), "t.json", &with_mode("deviate"));
    let (o, d) = observe(&run, &t, &[("SDC_LLM_MODEL_CONFORMANCE", "claude-opus-5-5"), ("SDC_LLM_EFFORT_CONFORMANCE", "medium")]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    assert_eq!(c["result"], "contradicted", "{c:#}");
    assert_eq!((c["k"].as_u64(), c["n"].as_u64()), (Some(5), Some(6)));
    assert_eq!(c["summary"], "5 of 6 scenarios");
    assert_eq!(c["table"]["model"], "claude-opus-5-5");
    assert_eq!(c["table"]["effort"], "medium");
    assert_eq!(c["table"]["effort_source"], "SDC_LLM_EFFORT_CONFORMANCE");
    let failed: Vec<&J> = c["scenarios"].as_array().unwrap().iter().filter(|x| x["status"] != "passed").collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["id"], "chores.list.FinishChore/outcome/wrong-state");
    let checks = c["failed_checks"].as_array().unwrap();
    assert!(checks.iter().any(|x| x["expected"].to_string().contains("wrong-state") && x["observed"].is_array()), "{c:#}");
    assert!(c["adapter_notes"].to_string().contains("unmapped_status"), "{c:#}");
}

#[test]
fn an_empty_suite_is_unknown_never_a_pass() {
    ess();
    let run = run_dir("empty", "spec-empty");
    let t = write_table(run.parent().unwrap(), "t.json", &table());
    let (o, d) = observe(&run, &t, &[]);
    assert!(o.status.success(), "{}", says(&o));
    assert_eq!(d["checks"][0]["result"], "unknown");
    assert_eq!(d["conformance"]["cause"], "suite_empty");
    assert_eq!(d["conformance"]["n"], 0);
    assert_eq!(d["conformance"]["summary"], "0 of 0 scenarios, unknown: suite_empty");
}

#[test]
fn a_product_that_does_not_start_is_unknown() {
    ess();
    let run = run_dir("nostart", "spec");
    let t = write_table(run.parent().unwrap(), "t.json", &with_mode("nostart"));
    let (o, d) = observe(&run, &t, &[]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    assert_eq!(c["result"], "unknown", "{c:#}");
    assert_eq!(c["cause"], "product_not_started");
    assert_eq!(c["k"], 0);
    assert!(s(&c["why"]).contains("exited before it answered"), "{c:#}");
}

fn s(v: &J) -> &str {
    v.as_str().unwrap_or("")
}

#[test]
fn a_refused_table_is_asked_again_twice_then_unknown() {
    ess();
    let run = run_dir("refused", "spec");
    let mut t = table();
    t["views"] = json!([]);
    let t = write_table(run.parent().unwrap(), "t.json", &t);
    let (o, d) = observe(&run, &t, &[]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    assert_eq!(c["result"], "unknown");
    assert_eq!(c["cause"], "table_refused");
    assert_eq!(c["table"]["attempts"], 3);
    assert!(s(&c["why"]).contains("view chores.list.OpenChores is not covered"), "{c:#}");
    let second = fs::read_to_string(run.join("conformance/table-node/002/prompt.md")).unwrap();
    assert!(second.contains("view chores.list.OpenChores is not covered"), "{second}");
    assert!(!run.join("conformance/table-node/004").exists());
}

#[test]
fn a_table_fixed_on_the_retry_is_used() {
    ess();
    let run = run_dir("retry", "spec");
    let mut bad = table();
    bad["commands"][1]["outcomes"][2]["status"] = json!(200);
    let dir = run.parent().unwrap().to_path_buf();
    let tables = format!("{},{}", write_table(&dir, "bad.json", &bad), write_table(&dir, "good.json", &table()));
    let (o, d) = observe(&run, &tables, &[]);
    assert!(o.status.success(), "{}", says(&o));
    assert_eq!(d["conformance"]["result"], "held", "{:#}", d["conformance"]);
    assert_eq!(d["conformance"]["table"]["attempts"], 2);
    let second = fs::read_to_string(run.join("conformance/table-node/002/prompt.md")).unwrap();
    assert!(second.contains("without a when_body discriminator"), "{second}");
}

#[test]
fn a_product_that_does_not_build_is_unknown() {
    ess();
    let run = run_dir("nobuild", "spec");
    let mut t = table();
    t["build"] = json!({"argv": ["bin/stub-product", "0", "x", "failbuild"]});
    let t = write_table(run.parent().unwrap(), "t.json", &t);
    let (o, d) = observe(&run, &t, &[]);
    assert!(o.status.success(), "{}", says(&o));
    assert_eq!(d["conformance"]["cause"], "product_build_failed", "{:#}", d["conformance"]);
    assert!(s(&d["conformance"]["why"]).contains("could not compile"));
}

#[test]
fn a_missing_table_and_a_model_less_adapter_are_unknown() {
    ess();
    let run = run_dir("notable", "spec");
    let (o, d) = observe(&run, "-", &[]);
    assert!(o.status.success(), "{}", says(&o));
    assert_eq!(d["conformance"]["cause"], "table_missing", "{:#}", d["conformance"]);
    let (_, d) = observe(&run, "-", &[("SDC_CONFORMANCE_ADAPTER", "stub")]);
    assert_eq!(d["conformance"]["cause"], "table_missing");
    assert!(s(&d["conformance"]["why"]).contains("has no model"));
}

#[test]
fn an_ess_binary_that_is_not_the_pinned_one_is_refused() {
    let run = run_dir("pin", "spec");
    let t = write_table(run.parent().unwrap(), "t.json", &table());
    let stub = stub();
    let (o, d) = observe(&run, &t, &[("ESS_BIN", stub.to_str().unwrap())]);
    assert!(o.status.success(), "{}", says(&o));
    assert_eq!(d["conformance"]["result"], "unknown");
    assert_eq!(d["conformance"]["cause"], "ess_pin_mismatch", "{:#}", d["conformance"]);
    let (_, d) = observe(&run, &t, &[("ESS_BIN", "/nonexistent/ess")]);
    assert_eq!(d["conformance"]["cause"], "ess_missing");
}

#[test]
fn a_run_without_a_delivered_tree_is_unknown() {
    ess();
    let run = run_dir("nodelivery", "spec");
    fs::remove_dir_all(run.join("store/IDEA-1/work/published")).unwrap();
    let (_, d) = observe(&run, "-", &[]);
    assert_eq!(d["conformance"]["cause"], "no_delivered_tree", "{:#}", d["conformance"]);
}

fn ir(dir: &Path) -> PathBuf {
    let p = dir.join("ir.json");
    let o = Command::new(ess()).args(["compile", "--path"]).arg(fixtures().join("spec")).arg("--out").arg(&p).output().unwrap();
    assert!(o.status.success());
    p
}

fn check(dir: &Path, t: &J) -> (bool, J) {
    let ir = ir(dir);
    let tp = write_table(dir, "check.json", t);
    let o = Command::new(BIN).args(["check-table", "--ir"]).arg(&ir).args(["--table", &tp]).output().unwrap();
    let v: J = serde_json::from_slice(&o.stdout).unwrap_or(J::Null);
    (o.status.success(), v)
}

#[test]
fn the_check_refuses_a_status_mapped_to_two_outcomes_unless_each_has_a_body_discriminator() {
    let dir = tmp("check-disc");
    assert_eq!(check(&dir, &table()), (true, json!({"accepted": true, "problems": [], "declared_missing": []})));
    let mut t = table();
    t["commands"][1]["outcomes"][1]["status"] = json!(409);
    let (ok, v) = check(&dir, &t);
    assert!(!ok);
    assert!(v["problems"].to_string().contains("status 409 maps to"), "{v}");
    t["commands"][1]["outcomes"][1]["when_body"] = json!({"path": "error", "equals": "board unreachable"});
    t["commands"][1]["outcomes"][2]["when_body"] = json!({"path": "error", "equals": "already done"});
    assert!(check(&dir, &t).0, "{}", check(&dir, &t).1);
}

#[test]
fn the_check_refuses_what_does_not_cover_the_specification_and_counts_declared_gaps() {
    let dir = tmp("check-cover");
    let mut t = table();
    t["commands"][1]["outcomes"].as_array_mut().unwrap().remove(1);
    let (ok, v) = check(&dir, &t);
    assert!(!ok && v["problems"].to_string().contains("outcome unreachable is not covered"), "{v}");
    let mut t = table();
    t["commands"][1]["outcomes"][1] = json!({"outcome": "unreachable", "none": "the product never calls a board"});
    t["commands"][1]["forced"] = json!([]);
    let (ok, v) = check(&dir, &t);
    assert!(ok, "{v}");
    assert_eq!(v["declared_missing"], json!(["outcome chores.list.FinishChore/unreachable: the product never calls a board"]));
    let mut t = table();
    t["commands"][0]["outcomes"][0]["events"][0]["payload"] = json!({"title": "{response.title}"});
    assert!(check(&dir, &t).1["problems"].to_string().contains("payload fields"));
    let mut t = table();
    t["start"]["argv"][0] = json!("/usr/bin/env");
    assert!(check(&dir, &t).1["problems"].to_string().contains("relative path inside the delivered tree"));
    let mut t = table();
    t["start"]["argv"] = json!(["bin/stub-product", "8080"]);
    assert!(check(&dir, &t).1["problems"].to_string().contains("never passes {port}"));
}
