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

/// The README of the delivered stub tree; the fixture table cites line 2 as its start command.
const README: &str = "Stub chores service.\nRun: bin/stub-product <port> <data dir> <ok|deviate|gone|token|token-silent|nostart|failbuild>\n";

/// An ended run with the fixture specification `spec` and a delivered tree holding the stub product.
fn run_dir(name: &str, spec: &str) -> PathBuf {
    let run = tmp(name).join(name);
    let work = run.join("store/IDEA-1/work");
    copy(&fixtures().join(spec), &work.join("spec"));
    fs::create_dir_all(work.join("published/bin")).unwrap();
    fs::copy(stub(), work.join("published/bin/stub-product")).unwrap();
    fs::write(work.join("published/README.md"), README).unwrap();
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
        .env("SDC_CONFORMANCE_TABLES", "1")
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
    assert_eq!(d["checks"][0], json!({"id": "conformance", "result": "held", "k": 7, "f": 0, "d": 0, "u": 0, "n": 7, "agreement": 7, "tables_counted": 1, "cause": null}), "{c:#}");
    assert_eq!(c["summary"], "7 passed, 0 failed, 0 disputed, 0 not measurable of 7; 1 table");
    assert_eq!(c["tables"][0]["conformance_status"], "passed");
    assert_eq!(c["events_checked_from"], "response_body");
    assert_eq!(c["tables"][0]["source"], "conformance.table");
    assert_eq!(c["tables"][0]["model"], "claude-sonnet-5-5");
    assert_eq!(c["tables"][0]["model_source"], "default");
    assert!(["low", "medium", "high", "xhigh", "max"].contains(&c["tables"][0]["effort"].as_str().unwrap()), "{c:#}");
    assert_eq!(c["tables"][0]["attempts"], 1);
    assert_eq!(c["product"]["tree"], "store/IDEA-1/work/published");
    assert!(run.join("conformance-detail.json").is_file());
    assert!(String::from_utf8_lossy(&o.stdout).contains("conformance: 7 passed, 0 failed, 0 disputed, 0 not measurable of 7; 1 table"), "{}", says(&o));
    let text = fs::read_to_string(run.join("oracle.json")).unwrap();
    assert!(!text.contains(run.to_str().unwrap()), "oracle.json holds an absolute path");
}

#[test]
fn one_deviation_is_contradicted_with_its_expected_and_observed() {
    ess();
    let run = run_dir("deviation", "spec");
    let t = write_table(run.parent().unwrap(), "t.json", &with_mode("gone"));
    let (o, d) = observe(&run, &t, &[("SDC_LLM_MODEL_CONFORMANCE", "claude-opus-5-5"), ("SDC_LLM_EFFORT_CONFORMANCE", "medium")]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    assert_eq!(c["result"], "contradicted", "{c:#}");
    assert_eq!((c["k"].as_u64(), c["f"].as_u64(), c["u"].as_u64(), c["n"].as_u64()), (Some(6), Some(1), Some(0), Some(7)));
    assert_eq!(c["summary"], "6 passed, 1 failed, 0 disputed, 0 not measurable of 7; 1 table");
    assert_eq!(c["tables"][0]["model"], "claude-opus-5-5");
    assert_eq!(c["tables"][0]["effort"], "medium");
    assert_eq!(c["tables"][0]["effort_source"], "SDC_LLM_EFFORT_CONFORMANCE");
    let failed: Vec<&J> = c["scenarios"].as_array().unwrap().iter().filter(|x| x["status"] != "passed").collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["id"], "chores.list.Chore/state/Done/refuses/chores.list.FinishChore");
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
    assert_eq!(d["conformance"]["summary"], "not measured (suite_empty), 0 scenarios");
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
    assert!(c["k"].is_null() && c["f"].is_null() && c["u"].is_null(), "{c:#}");
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
    assert_eq!(c["tables"][0]["attempts"], 3);
    assert!(s(&c["why"]).contains("view chores.list.OpenChores is not covered"), "{c:#}");
    let second = fs::read_to_string(run.join("conformance/table-1/node/002/prompt.md")).unwrap();
    assert!(second.contains("view chores.list.OpenChores is not covered"), "{second}");
    assert!(!run.join("conformance/table-1/node/004").exists());
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
    assert_eq!(d["conformance"]["tables"][0]["attempts"], 2);
    let second = fs::read_to_string(run.join("conformance/table-1/node/002/prompt.md")).unwrap();
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
    let (o, d) = observe(&run, "-", &[("SDC_CONFORMANCE_ADAPTER", "stub")]);
    let c = &d["conformance"];
    assert_eq!(c["cause"], "table_missing");
    assert!(s(&c["why"]).contains("has no model"));
    // A run never measured prints no counts, and its counts are null, not 0.
    assert_eq!(c["summary"], "not measured (table_missing), 7 scenarios", "{c:#}");
    assert!(String::from_utf8_lossy(&o.stdout).contains("conformance: not measured (table_missing), 7 scenarios"), "{}", says(&o));
    assert_eq!(d["checks"][0], json!({"id": "conformance", "result": "unknown", "k": null, "f": null, "d": null, "u": null, "n": 7,
        "agreement": null, "tables_counted": 0, "cause": "table_missing"}));
    assert_eq!(c["measured"], false);
    let (_, d) = observe(&run, "-", &[("ESS_BIN", "/nonexistent/ess")]);
    assert_eq!(d["conformance"]["summary"], "not measured (ess_missing)");
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

fn ir_of(dir: &Path, spec: &str) -> PathBuf {
    let p = dir.join(format!("ir-{spec}.json"));
    let o = Command::new(ess()).args(["compile", "--path"]).arg(fixtures().join(spec)).arg("--out").arg(&p).output().unwrap();
    assert!(o.status.success());
    p
}

fn check(dir: &Path, t: &J) -> (bool, J) {
    check_of(dir, "spec", t)
}

fn check_of(dir: &Path, spec: &str, t: &J) -> (bool, J) {
    check_in(dir, spec, t, README)
}

/// The check against a delivered tree whose README is `readme`.
fn check_in(dir: &Path, spec: &str, t: &J, readme: &str) -> (bool, J) {
    let ir = ir_of(dir, spec);
    let tp = write_table(dir, "check.json", t);
    let tree = dir.join("tree");
    fs::create_dir_all(&tree).unwrap();
    fs::write(tree.join("README.md"), readme).unwrap();
    fs::write(tree.join("PROVENANCE"), "stub bin/stub-product ok\n").unwrap();
    let o = Command::new(BIN).args(["check-table", "--ir"]).arg(&ir).args(["--table", &tp]).arg("--tree").arg(&tree).output().unwrap();
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
    t["commands"][1]["outcomes"][1] = json!({"outcome": "unreachable", "none": {"kind": "not_implemented", "why": "the product never calls a board"}});
    t["commands"][1]["forced"] = json!([]);
    let (ok, v) = check(&dir, &t);
    assert!(ok, "{v}");
    assert_eq!(v["declared_missing"], json!(["outcome chores.list.FinishChore/unreachable (not_implemented): the product never calls a board"]));
    let mut t = table();
    t["commands"][1]["forced"] = json!([]);
    let (ok, v) = check(&dir, &t);
    assert!(!ok && v["problems"].to_string().contains("unreachable is external and needs a forced request"), "{v}");
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

/// The fixture table for the `token` stub: the token printed at start, sent as a header on every
/// request, and bound to the spec's board code when `bind`.
fn token_table(mode: &str, bind: bool) -> J {
    let mut t = with_mode(mode);
    t["captures"] = json!([{"name": "token", "stream": "stdout", "regex": "board token: (\\S+)", "timeout_s": 3}]);
    for c in t["commands"].as_array_mut().unwrap() {
        c["request"]["headers"] = json!({"x-token": "{capture.token}"});
        for f in c["forced"].as_array_mut().into_iter().flatten() {
            f["request"]["headers"] = json!({"x-token": "{capture.token}"});
        }
    }
    t["views"][0]["request"]["headers"] = json!({"x-token": "{capture.token}"});
    if bind {
        t["bindings"] = json!([{"input": "code", "capture": "token"}]);
    }
    t
}

#[test]
fn a_value_the_product_prints_at_start_is_captured_and_bound_to_the_spec_value() {
    ess();
    let run = run_dir("capture", "spec");
    let t = write_table(run.parent().unwrap(), "t.json", &token_table("token", true));
    let (o, d) = observe(&run, &t, &[]);
    assert!(o.status.success(), "{}", says(&o));
    assert_eq!(d["conformance"]["result"], "held", "{:#}", d["conformance"]);
    assert_eq!(d["conformance"]["summary"], "7 passed, 0 failed, 0 disputed, 0 not measurable of 7; 1 table");
    let run = run_dir("capture-unbound", "spec");
    let t = write_table(run.parent().unwrap(), "t.json", &token_table("token", false));
    let (_, d) = observe(&run, &t, &[]);
    let c = &d["conformance"];
    assert_eq!(c["result"], "contradicted", "{c:#}");
    assert_eq!(c["summary"], "6 passed, 1 failed, 0 disputed, 0 not measurable of 7; 1 table");
    assert!(c["scenarios"].as_array().unwrap().iter().any(|x| x["id"] == "chores.list.OpenBoard/outcome/opened" && x["status"] == "failed"), "{c:#}");
}

#[test]
fn a_capture_that_does_not_appear_is_unavailable_and_the_figure_unknown() {
    ess();
    let run = run_dir("capture-missing", "spec");
    let t = write_table(run.parent().unwrap(), "t.json", &token_table("token-silent", true));
    let (o, d) = observe(&run, &t, &[]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    assert_eq!(c["result"], "unknown", "{c:#}");
    assert_eq!(c["cause"], "capture_missing");
    assert!(s(&c["why"]).contains("token"), "{c:#}");
    assert_eq!(c["tables"][0]["conformance_status"], "inconclusive");
    assert_eq!(c["tables"][0]["retried_after_run"], true, "{c:#}");
    assert_eq!(c["tables"][0]["counted"], false);
}

#[test]
fn what_the_interface_cannot_reach_is_not_measurable_and_never_held() {
    ess();
    let mut t = table();
    t["commands"][1]["forced"] = json!([]);
    t["commands"][1]["outcomes"][1] = json!({"outcome": "unreachable",
        "none": {"kind": "not_reachable_over_the_interface", "why": "the board link lives only in the browser"}});
    let run = run_dir("not-measurable", "spec");
    let tp = write_table(run.parent().unwrap(), "t.json", &t);
    let (o, d) = observe(&run, &tp, &[]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    assert_eq!(c["summary"], "6 passed, 0 failed, 0 disputed, 1 not measurable of 7; 1 table, unknown: not_measurable", "{c:#}");
    assert_eq!(c["result"], "unknown");
    assert_eq!(d["checks"][0]["u"], 1);
    assert!(c["scenarios"].as_array().unwrap().iter().any(|x| x["id"] == "chores.list.FinishChore/outcome/unreachable" && x["status"] == "not_measurable"), "{c:#}");
    t["commands"][1]["outcomes"][1]["none"]["kind"] = json!("not_implemented");
    let run = run_dir("not-implemented", "spec");
    let tp = write_table(run.parent().unwrap(), "t.json", &t);
    let (_, d) = observe(&run, &tp, &[]);
    assert_eq!(d["conformance"]["summary"], "6 passed, 1 failed, 0 disputed, 0 not measurable of 7; 1 table", "{:#}", d["conformance"]);
    assert_eq!(d["conformance"]["result"], "contradicted");
}

#[test]
fn a_scenario_with_a_failed_check_fails_even_when_it_also_needs_what_cannot_be_reached() {
    ess();
    let mut t = table();
    t["views"][0] = json!({"view": "chores.list.OpenChores",
        "none": {"kind": "not_reachable_over_the_interface", "why": "the open list is drawn only in the browser"}});
    t["commands"][0]["outcomes"][0]["events"][0]["payload"]["title"] = json!("{response.state}");
    let run = run_dir("failed-and-unreachable", "spec");
    let tp = write_table(run.parent().unwrap(), "t.json", &t);
    let (o, d) = observe(&run, &tp, &[]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    let status = |id: &str| c["scenarios"].as_array().unwrap().iter().find(|x| x["id"] == id).map(|x| x["status"].clone());
    assert_eq!(status("chores.list.AddChore/outcome/added"), Some(json!("failed")), "{c:#}");
    assert_eq!(c["result"], "contradicted");
    assert!(c["failed_checks"].to_string().contains("payload of chores.list.ChoreAdded"), "{c:#}");
}

#[test]
fn the_check_refuses_a_none_without_kind_and_a_capture_never_used() {
    let dir = tmp("check-captures");
    let mut t = table();
    t["views"][0] = json!({"view": "chores.list.OpenChores", "none": "only in the browser"});
    assert!(check(&dir, &t).1["problems"].to_string().contains("none needs a kind"), "{}", check(&dir, &t).1);
    let mut t = table();
    t["captures"] = json!([{"name": "token", "stream": "stdout", "regex": "board token: (\\S+)", "timeout_s": 3}]);
    assert!(check(&dir, &t).1["problems"].to_string().contains("capture token is declared and never used"), "{}", check(&dir, &t).1);
    let mut t = token_table("token", true);
    t["captures"][0]["regex"] = json!("board token: \\S+");
    assert!(check(&dir, &t).1["problems"].to_string().contains("exactly one group"), "{}", check(&dir, &t).1);
    let mut t = token_table("token", true);
    t["captures"][0]["stream"] = json!("stdin");
    assert!(check(&dir, &t).1["problems"].to_string().contains("stdout or stderr"));
    let mut t = token_table("token", true);
    t["bindings"] = json!([{"input": "code", "capture": "other"}]);
    assert!(check(&dir, &t).1["problems"].to_string().contains("no declared capture other"));
    assert!(check(&dir, &token_table("token", true)).0, "{}", check(&dir, &token_table("token", true)).1);
}

/// The fixture table for `spec-optional`: the second view reads the done chores.
fn optional_table() -> J {
    let mut t = table();
    t["views"].as_array_mut().unwrap().push(json!({"view": "chores.list.DoneChores", "request": {"method": "GET", "path": "/chores/done"},
        "rows": "", "fields": {"chore_id": "chore_id", "title": "title"}}));
    t
}

#[test]
fn the_check_refuses_a_forced_request_on_another_route_and_two_views_on_one_request() {
    let dir = tmp("check-routes");
    let mut t = table();
    t["commands"][1]["forced"][0]["request"]["path"] = json!("/board-down");
    let (ok, v) = check(&dir, &t);
    assert!(!ok && v["problems"].to_string().contains("forced[0]: a forced request keeps the method and route of the command's request"), "{v}");
    let mut t = table();
    t["commands"][1]["forced"][0]["request"]["path"] = json!("/chores/00000000-0000-4000-8000-000000000000/finish?board=down");
    assert!(check(&dir, &t).0, "{}", check(&dir, &t).1);
    assert!(check_of(&dir, "spec-optional", &optional_table()).0, "{}", check_of(&dir, "spec-optional", &optional_table()).1);
    let mut t = optional_table();
    t["views"][1]["request"]["path"] = json!("/chores/open");
    let (ok, v) = check_of(&dir, "spec-optional", &t);
    assert!(!ok && v["problems"].to_string().contains("views chores.list.OpenChores and chores.list.DoneChores read the same request"), "{v}");
}

fn figure(t: &J) -> (u64, u64, u64) {
    (t["k"].as_u64().unwrap(), t["f"].as_u64().unwrap(), t["u"].as_u64().unwrap())
}

fn scenario<'a>(c: &'a J, id: &str) -> &'a J {
    c["scenarios"].as_array().unwrap().iter().find(|x| x["id"] == id).unwrap_or_else(|| panic!("no scenario {id}: {c:#}"))
}

#[test]
fn three_tables_are_run_and_a_scenario_passes_when_every_table_that_reaches_it_passes_it() {
    ess();
    let run = run_dir("three-tables", "spec");
    let dir = run.parent().unwrap().to_path_buf();
    let a = write_table(&dir, "a.json", &with_mode("gone"));
    let mut b = with_mode("gone");
    b["commands"][1]["forced"] = json!([]);
    b["commands"][1]["outcomes"][1] = json!({"outcome": "unreachable",
        "none": {"kind": "not_reachable_over_the_interface", "why": "the board lives only in the browser"}});
    let b = write_table(&dir, "b.json", &b);
    let (o, d) = observe(&run, &format!("{a};{b}"), &[("SDC_CONFORMANCE_TABLES", "3")]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    assert_eq!(d["checks"][0], json!({"id": "conformance", "result": "contradicted", "k": 6, "f": 1, "d": 0, "u": 0, "n": 7,
        "agreement": 6, "tables_counted": 3, "cause": null}), "{c:#}");
    assert_eq!(c["summary"], "6 passed, 1 failed, 0 disputed, 0 not measurable of 7; 3 tables agree on 6 of 7");
    let tables = c["tables"].as_array().unwrap();
    assert_eq!(tables.iter().map(figure).collect::<Vec<_>>(), vec![(6, 1, 0), (5, 1, 1), (5, 1, 1)], "{c:#}");
    assert!(tables.iter().all(|t| t["counted"] == true && t["cost_usd"].is_number() && t["n"] == 7), "{c:#}");
    assert_eq!(tables.iter().map(|t| t["slot"].clone()).collect::<Vec<_>>(), vec![json!(1), json!(2), json!(3)]);
    let un = scenario(c, "chores.list.FinishChore/outcome/unreachable");
    assert_eq!((&un["status"], &un["under"]), (&json!("passed"), &json!(["passed", "not_measurable", "not_measurable"])));
    let ws = scenario(c, "chores.list.Chore/state/Done/refuses/chores.list.FinishChore");
    assert_eq!((&ws["status"], &ws["under"]), (&json!("failed"), &json!(["failed", "failed", "failed"])));
    assert!(run.join("conformance/table-3/node/001/prompt.md").is_file());
    assert!(String::from_utf8_lossy(&o.stdout).contains("conformance: 6 passed, 1 failed, 0 disputed, 0 not measurable of 7; 3 tables agree on 6 of 7"), "{}", says(&o));
    let (_, d) = observe(&run, &b, &[("SDC_CONFORMANCE_TABLES", "3")]);
    assert_eq!(d["conformance"]["summary"], "5 passed, 1 failed, 0 disputed, 1 not measurable of 7; 3 tables agree on 7 of 7","{:#}", d["conformance"]);
    assert_eq!(d["conformance"]["result"], "contradicted");
}

#[test]
fn a_table_that_never_signs_in_is_refused_after_its_run_and_asked_again_once() {
    ess();
    let run = run_dir("unsigned", "spec");
    let dir = run.parent().unwrap().to_path_buf();
    let plain = write_table(&dir, "plain.json", &with_mode("token"));
    let good = write_table(&dir, "good.json", &token_table("token", true));
    let (o, d) = observe(&run, &format!("{plain},{good}"), &[]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    assert_eq!(c["result"], "held", "{c:#}");
    let t = &c["tables"][0];
    assert_eq!((&t["retried_after_run"], &t["attempts"], &t["counted"]), (&json!(true), &json!(2), &json!(true)), "{t:#}");
    assert!(t["evidence"].to_string().contains("401"), "{t:#}");
    assert_eq!(t["runs"].as_array().unwrap().len(), 2);
    let second = fs::read_to_string(run.join("conformance/table-1/node/002/prompt.md")).unwrap();
    assert!(second.contains("401") && second.contains("after its run"), "{second}");
    let (_, d) = observe(&run, &plain, &[]);
    let c = &d["conformance"];
    assert_eq!((&c["result"], &c["cause"]), (&json!("unknown"), &json!("table_error")), "{c:#}");
    assert!(c["k"].is_null());
    assert_eq!(c["summary"], "not measured (table_error), 7 scenarios");
    assert_eq!((&c["tables"][0]["counted"], &c["tables"][0]["retried_after_run"]), (&json!(false), &json!(true)));
}

#[test]
fn a_request_naming_an_input_the_scenario_leaves_out_is_the_tables_fault() {
    ess();
    let run = run_dir("optional", "spec-optional");
    let dir = run.parent().unwrap().to_path_buf();
    let mut bad = optional_table();
    bad["commands"][0]["request"]["path"] = json!("/chores?note={input.note}");
    let tables = format!("{},{}", write_table(&dir, "bad.json", &bad), write_table(&dir, "good.json", &optional_table()));
    let (o, d) = observe(&run, &tables, &[]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    assert_eq!(c["result"], "held", "{c:#}");
    let t = &c["tables"][0];
    assert_eq!(t["retried_after_run"], true, "{t:#}");
    assert!(t["evidence"].to_string().contains("{input.note}"), "{t:#}");
    assert_eq!(t["runs"][0]["e"].as_u64().map(|e| e > 0), Some(true), "{t:#}");
}

#[test]
fn several_statuses_may_mean_one_outcome() {
    ess();
    let dir = tmp("check-many");
    let mut many = table();
    many["commands"][1]["outcomes"][0]["status"] = json!([200, 202]);
    assert!(check(&dir, &many).0, "{}", check(&dir, &many).1);
    let mut t = table();
    t["commands"][1]["outcomes"][1]["status"] = json!([503, 409]);
    let (ok, v) = check(&dir, &t);
    assert!(!ok && v["problems"].to_string().contains("status 409 maps to"), "{v}");
    let run = run_dir("many-to-one", "spec");
    let tp = write_table(run.parent().unwrap(), "t.json", &many);
    let (o, d) = observe(&run, &tp, &[]);
    assert!(o.status.success(), "{}", says(&o));
    assert_eq!(d["conformance"]["summary"], "7 passed, 0 failed, 0 disputed, 0 not measurable of 7; 1 table", "{:#}", d["conformance"]);
    assert_eq!(d["conformance"]["result"], "held");
}

#[test]
fn an_id_the_scenario_never_created_is_refused_wrong_state_by_the_adapter() {
    ess();
    let dir = tmp("check-absent");
    let mut t = with_mode("deviate");
    t["commands"][1]["outcomes"][2]["status"] = json!([409, 404]);
    let (ok, v) = check(&dir, &t);
    assert!(!ok && v["problems"].to_string().contains("404 on the wrong_state outcome is the adapter's to decide"), "{v}");
    let run = run_dir("absent", "spec");
    let tp = write_table(run.parent().unwrap(), "t.json", &with_mode("deviate"));
    let (o, d) = observe(&run, &tp, &[]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    assert_eq!(c["summary"], "7 passed, 0 failed, 0 disputed, 0 not measurable of 7; 1 table", "{c:#}");
    let log = fs::read_to_string(run.join("conformance/table-1/run-1/adapter-log.jsonl")).unwrap();
    assert!(log.lines().any(|l| l.contains("\"unknown_instance\"") && l.contains("chores.list.FinishChore/outcome/wrong-state") && l.contains("\"status\":404")), "{log}");
}

#[test]
fn a_404_for_an_entity_the_scenario_created_is_never_wrong_state() {
    ess();
    let run = run_dir("gone", "spec");
    let tp = write_table(run.parent().unwrap(), "t.json", &with_mode("gone"));
    let (o, d) = observe(&run, &tp, &[]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    assert_eq!(c["result"], "contradicted", "{c:#}");
    assert_eq!(scenario(c, "chores.list.Chore/state/Done/refuses/chores.list.FinishChore")["status"], "failed", "{c:#}");
    assert_eq!(scenario(c, "chores.list.FinishChore/outcome/wrong-state")["status"], "passed", "{c:#}");
}

#[test]
fn a_scenario_the_tables_disagree_on_is_disputed_and_the_figure_unknown() {
    ess();
    let run = run_dir("disputed", "spec");
    let dir = run.parent().unwrap().to_path_buf();
    let a = write_table(&dir, "a.json", &table());
    let mut b = table();
    b["commands"][1]["outcomes"][2]["status"] = json!(422);
    let b = write_table(&dir, "b.json", &b);
    let (o, d) = observe(&run, &format!("{a};{b}"), &[("SDC_CONFORMANCE_TABLES", "2")]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    assert_eq!(d["checks"][0], json!({"id": "conformance", "result": "unknown", "k": 5, "f": 0, "d": 2, "u": 0, "n": 7,
        "agreement": 5, "tables_counted": 2, "cause": "disputed"}), "{c:#}");
    assert_eq!(c["summary"], "5 passed, 0 failed, 2 disputed, 0 not measurable of 7; 2 tables agree on 5 of 7, unknown: disputed");
    let ws = scenario(c, "chores.list.Chore/state/Done/refuses/chores.list.FinishChore");
    assert_eq!((&ws["status"], &ws["under"]), (&json!("disputed"), &json!(["passed", "failed"])));
}

/// The README line of the `partner` stub: a board code the operator chooses at start.
const PARTNER: &str = "With a board: `bin/stub-product <port> <data dir> partner <board-code>` (for example B-1)\n";

/// The `partner` stub started with the board code taken from the scenario's `code`.
fn partner_table() -> J {
    let mut t = table();
    t["start"]["argv"] = json!(["bin/stub-product", "{port}", "{data_dir}", "partner", "{arg.board}"]);
    t["start"]["args"] = json!([{"name": "board", "kind": "from_scenario", "field": "code", "when_absent": "B-1"}]);
    t["start"]["from"] = json!([{"file": "README.md", "line": 3}]);
    t
}

#[test]
fn a_start_value_the_readme_leaves_to_the_operator_is_taken_from_the_scenario() {
    ess();
    let dir = tmp("check-start-args");
    let readme = format!("{README}{PARTNER}");
    assert!(check_in(&dir, "spec", &partner_table(), &readme).0, "{}", check_in(&dir, "spec", &partner_table(), &readme).1);
    let mut t = partner_table();
    t["start"]["args"][0]["field"] = json!("nothing");
    let (ok, v) = check_in(&dir, "spec", &t, &readme);
    assert!(!ok && v["problems"].to_string().contains("start.args[0] board: nothing is no input field"), "{v}");
    let mut t = partner_table();
    t["start"]["args"][0]["when_absent"] = json!("B-2");
    let (ok, v) = check_in(&dir, "spec", &t, &readme);
    assert!(!ok && v["problems"].to_string().contains("when_absent B-2 is not on the cited lines"), "{v}");
    let mut t = partner_table();
    t["start"]["argv"][4] = json!("{arg.other}");
    let (ok, v) = check_in(&dir, "spec", &t, &readme);
    assert!(!ok && v["problems"].to_string().contains("{arg.other}"), "{v}");

    let run = run_dir("start-args", "spec");
    fs::write(run.join("store/IDEA-1/work/published/README.md"), &readme).unwrap();
    let tp = write_table(run.parent().unwrap(), "t.json", &partner_table());
    let (o, d) = observe(&run, &tp, &[]);
    assert!(o.status.success(), "{}", says(&o));
    assert_eq!(d["conformance"]["result"], "held", "{:#}", d["conformance"]);
    let mut fixed = partner_table();
    fixed["start"]["argv"][4] = json!("B-1");
    fixed["start"].as_object_mut().unwrap().remove("args");
    let tp = write_table(run.parent().unwrap(), "t.json", &fixed);
    let (_, d) = observe(&run, &tp, &[]);
    let c = &d["conformance"];
    assert_eq!(scenario(c, "chores.list.OpenBoard/outcome/opened")["status"], "failed", "{c:#}");
}

#[test]
fn a_slot_refused_after_its_retry_gets_one_more_session_and_at_most_five() {
    ess();
    let run = run_dir("one-more", "spec");
    let dir = run.parent().unwrap().to_path_buf();
    let plain = write_table(&dir, "plain.json", &with_mode("token"));
    let good = write_table(&dir, "good.json", &token_table("token", true));
    let (o, d) = observe(&run, &format!("{plain},{plain},{good}"), &[]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    assert_eq!(c["result"], "held", "{c:#}");
    let t = &c["tables"][0];
    assert_eq!((&t["attempts"], &t["one_more_session"], &t["counted"]), (&json!(3), &json!(true), &json!(true)), "{t:#}");
    assert_eq!(c["node_sessions"], 3);
    let mut bad = with_mode("token");
    bad["start"].as_object_mut().unwrap().remove("from");
    let bad = write_table(&dir, "bad.json", &bad);
    let (_, d) = observe(&run, &format!("{bad},{bad},{plain},{plain},{plain},{good}"), &[]);
    let t = &d["conformance"]["tables"][0];
    assert_eq!((&t["attempts"], &t["counted"]), (&json!(5), &json!(false)), "{t:#}");
    assert_eq!(d["conformance"]["node_sessions"], 5);
}

#[test]
fn the_check_refuses_a_literal_in_an_event_payload_or_an_error_field() {
    let dir = tmp("check-literal");
    let mut t = table();
    t["commands"][0]["outcomes"][0]["events"][0]["payload"]["title"] = json!("title");
    let (ok, v) = check(&dir, &t);
    assert!(!ok && v["problems"].to_string().contains("payload field title is the literal"), "{v}");
    let mut t = table();
    t["commands"][1]["outcomes"][2]["error"]["fields"]["state"] = json!("Open");
    let (ok, v) = check(&dir, &t);
    assert!(!ok && v["problems"].to_string().contains("error field state is the literal"), "{v}");
}

#[test]
fn a_check_of_a_field_filled_from_the_request_input_is_unverified() {
    ess();
    let run = run_dir("unverified", "spec");
    let tp = write_table(run.parent().unwrap(), "t.json", &table());
    let (o, d) = observe(&run, &tp, &[]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    assert_eq!(c["result"], "held", "{c:#}");
    let un = c["unverified_checks"].as_array().unwrap();
    assert!(un.iter().any(|x| x["scenario"] == "chores.list.FinishChore/outcome/finished" && x["about"] == "payload of chores.list.ChoreFinished"
        && x["fields"] == json!(["chore_id"]) && x["table"] == 1), "{c:#}");
    assert!(!un.iter().any(|x| x["about"] == "payload of chores.list.ChoreAdded"), "{c:#}");
}

#[test]
fn the_start_command_is_one_the_delivered_readme_or_manifest_states() {
    let dir = tmp("check-start");
    let mut t = table();
    t["start"].as_object_mut().unwrap().remove("from");
    let (ok, v) = check(&dir, &t);
    assert!(!ok && v["problems"].to_string().contains("start.from"), "{v}");
    let mut t = table();
    t["start"]["from"] = json!([{"file": "README.md", "line": 1}]);
    let (ok, v) = check(&dir, &t);
    assert!(!ok && v["problems"].to_string().contains("start.from: the cited lines do not contain stub-product"), "{v}");
    let mut t = table();
    t["start"]["from"] = json!([{"file": "PROVENANCE", "line": 1}]);
    let (ok, v) = check(&dir, &t);
    assert!(!ok && v["problems"].to_string().contains("not a README or a manifest"), "{v}");
    let mut t = table();
    t["start"]["from"] = json!([{"file": "README.md", "line": 9}]);
    let (ok, v) = check(&dir, &t);
    assert!(!ok && v["problems"].to_string().contains("README.md has no line 9"), "{v}");
    let mut t = table();
    t["start"]["argv"][2] = json!("{data_dir}/chores.db");
    assert!(check(&dir, &t).0, "an argument holding a placeholder stands for a README value: {}", check(&dir, &t).1);
    let mut t = table();
    t["start"]["argv"][3] = json!("22222222-2222-4222-8222-222222222222");
    t["start"]["from"] = json!([{"file": "README.md", "line": 3}]);
    let partner = format!("{README}With a partner: `bin/stub-product <port> <data dir> <partner-id>`\n");
    assert!(check_in(&dir, "spec", &t, &partner).0, "a value the usage leaves to the operator: {}", check_in(&dir, "spec", &t, &partner).1);
    let mut t = table();
    t["start"]["argv"][3] = json!("bogus");
    let (ok, v) = check(&dir, &t);
    assert!(!ok && v["problems"].to_string().contains("do not contain bogus"), "a value outside the usage's alternatives: {v}");
    let mut t = table();
    t["start"]["argv"] = json!(["cargo", "run", "--", "{port}", "{data_dir}", "ok"]);
    let readme = format!("{README}Or: cargo run -- <port> <data dir> ok\n");
    t["start"]["from"] = json!([{"file": "README.md", "line": 3}]);
    let (ok, v) = check_in(&dir, "spec", &t, &readme);
    assert!(!ok && v["problems"].to_string().contains("start the binary the build made"), "{v}");
    let demo = format!("{README}The mode `deviate` is a demo mode for tests.\nThe header `x-debug` switches on a mock board.\n");
    assert!(check_in(&dir, "spec", &table(), &demo).0, "{}", check_in(&dir, "spec", &table(), &demo).1);
    let (ok, v) = check_in(&dir, "spec", &with_mode("deviate"), &demo);
    assert!(!ok && v["problems"].to_string().contains("start.argv: deviate is what README.md line 3 describes as a test, demo or mock mode"), "{v}");
    let mut t = table();
    t["commands"][0]["request"]["headers"] = json!({"x-debug": "1"});
    let (ok, v) = check_in(&dir, "spec", &t, &demo);
    assert!(!ok && v["problems"].to_string().contains("x-debug is what README.md line 4 describes as a test, demo or mock mode"), "{v}");
}
