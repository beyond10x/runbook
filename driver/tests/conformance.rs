//! Scripted cycles of `sdc-conform observe` (docs/conformance.md) with a stub product and a stub
//! table node (`examples/conformance-stub-product.rs`), and the route table check. They need the
//! pinned ess (driver/ess-pin.json): `sdc-conform ess-install` once, or `ESS_BIN`.

use std::collections::BTreeMap;
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

/// Runs of `sdc-conform ess-path` in this process.
static ESS_PATH_RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// The pinned ess, resolved once per process (each test is one process under cargo-nextest).
fn ess() -> PathBuf {
    static ESS: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    ESS.get_or_init(|| {
        ESS_PATH_RUNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let o = Command::new(BIN).arg("ess-path").output().unwrap();
        assert!(o.status.success(), "the pinned ess is not installed (run `sdc-conform ess-install` or set ESS_BIN): {}", String::from_utf8_lossy(&o.stderr));
        PathBuf::from(String::from_utf8_lossy(&o.stdout).trim())
    })
    .clone()
}

/// The precondition of a test that runs the pinned ess through `sdc-conform`: the binary is
/// installed where `sdc-conform` looks (`ESS_BIN`, else the cache path). `sdc-conform` checks the
/// pin against the binary's bytes at each start; a test that does not need the path asks only this
/// and does not start `sdc-conform` for it.
fn installed() {
    let pin: J = serde_json::from_str(&fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("ess-pin.json")).unwrap()).unwrap();
    let path = crate::ess_pin_guard::installed_ess(&pin);
    assert!(path.is_file(), "the pinned ess is not installed (run `sdc-conform ess-install` or set ESS_BIN): {}", path.display());
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
const README: &str = "Stub chores service.\nRun: bin/stub-product <port> <data dir> <ok|deviate|gone|token|token-silent|session|nostart|failbuild>\n";

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
    installed();
    let run = run_dir("all-pass", "spec");
    let t = write_table(run.parent().unwrap(), "t.json", &table());
    let (o, d) = observe(&run, &t, &[]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    assert_eq!(d["format"], "sdc.oracle/1");
    assert_eq!(d["checks"][0], json!({"id": "conformance", "result": "held", "k": 7, "f": 0, "p": 0, "d": 0, "u": 0, "n": 7, "agreement": 7, "tables_counted": 1, "cause": null}), "{c:#}");
    assert_eq!(c["summary"], "spec scenarios proven passed: 7 of 7 (lower bound); 0 failed; 0 not measurable; 1 table (held)");
    assert_eq!(c["tables"][0]["conformance_status"], "passed");
    assert_eq!(c["events_checked_from"], "response_body");
    assert_eq!(c["tables"][0]["source"], "conformance.table");
    assert_eq!(c["tables"][0]["model"], "claude-sonnet-5-5");
    assert_eq!(c["tables"][0]["model_source"], "default");
    assert!(["low", "medium", "high", "xhigh", "max"].contains(&c["tables"][0]["effort"].as_str().unwrap()), "{c:#}");
    assert_eq!(c["tables"][0]["attempts"], 1);
    assert_eq!(c["product"]["tree"], "store/IDEA-1/work/published");
    assert!(run.join("conformance-detail.json").is_file());
    assert!(String::from_utf8_lossy(&o.stdout).contains("conformance: spec scenarios proven passed: 7 of 7 (lower bound); 0 failed; 0 not measurable; 1 table (held)"), "{}", says(&o));
    let text = fs::read_to_string(run.join("oracle.json")).unwrap();
    assert!(!text.contains(run.to_str().unwrap()), "oracle.json holds an absolute path");
}

#[test]
fn one_deviation_is_contradicted_with_its_expected_and_observed() {
    installed();
    let run = run_dir("deviation", "spec");
    let t = write_table(run.parent().unwrap(), "t.json", &with_mode("gone"));
    let (o, d) = observe(&run, &t, &[("SDC_LLM_MODEL_CONFORMANCE", "claude-opus-5-5"), ("SDC_LLM_EFFORT_CONFORMANCE", "medium")]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    assert_eq!(c["result"], "contradicted", "{c:#}");
    assert_eq!((c["k"].as_u64(), c["f"].as_u64(), c["u"].as_u64(), c["n"].as_u64()), (Some(6), Some(1), Some(0), Some(7)));
    assert_eq!(c["summary"], "spec scenarios proven passed: 6 of 7 (lower bound); 1 failed; 0 not measurable; 1 table");
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
    installed();
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
    installed();
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
    installed();
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

/// A money budget of the table node is optional: with `SDC_CONFORMANCE_MAX_BUDGET_USD` not set the
/// request of the node carries no `max_budget_usd`; one that is set is in the request as it is. The
/// turn bound is not money and is always there.
#[test]
fn the_table_node_is_asked_with_a_money_budget_only_when_one_is_named() {
    installed();
    for (tag, named, want) in [("none", None, None), ("named", Some("0.5"), Some(0.5))] {
        let run = run_dir(&format!("budget-{tag}"), "spec");
        let mut t = table();
        t["views"] = json!([]);
        let t = write_table(run.parent().unwrap(), "t.json", &t);
        let extra: Vec<(&str, &str)> = named.map(|n| ("SDC_CONFORMANCE_MAX_BUDGET_USD", n)).into_iter().collect();
        let (o, _) = observe(&run, &t, &extra);
        assert!(o.status.success(), "{tag}: {}", says(&o));
        let request: J = serde_json::from_str(&fs::read_to_string(run.join("conformance/table-1/node/001/request.json")).unwrap()).unwrap();
        assert_eq!(request.get("max_budget_usd").and_then(J::as_f64), want, "{tag}: {request:#}");
        assert_eq!(request.get("max_budget_usd").is_some(), want.is_some(), "{tag}: {request:#}");
        assert_eq!(request["max_turns"], 40, "{tag}: {request:#}");
    }
}

#[test]
fn a_table_fixed_on_the_retry_is_used() {
    installed();
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
    installed();
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
    installed();
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
    assert_eq!(d["checks"][0], json!({"id": "conformance", "result": "unknown", "k": null, "f": null, "p": null, "d": null, "u": null, "n": 7,
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
    installed();
    let run = run_dir("nodelivery", "spec");
    fs::remove_dir_all(run.join("store/IDEA-1/work/published")).unwrap();
    let (_, d) = observe(&run, "-", &[]);
    assert_eq!(d["conformance"]["cause"], "no_delivered_tree", "{:#}", d["conformance"]);
}

/// Runs of `ess compile` per output file in this process (the test of `ir_of` reads them).
static COMPILES: std::sync::Mutex<BTreeMap<PathBuf, u32>> = std::sync::Mutex::new(BTreeMap::new());

fn ir_of(dir: &Path, spec: &str) -> PathBuf {
    let p = dir.join(format!("ir-{spec}.json"));
    if p.is_file() {
        return p;
    }
    *COMPILES.lock().unwrap().entry(p.clone()).or_default() += 1;
    let o = Command::new(ess()).args(["compile", "--path"]).arg(fixtures().join(spec)).arg("--out").arg(&p).output().unwrap();
    assert!(o.status.success());
    p
}

/// A test that checks one table many times starts the pinned ess once: one `sdc-conform ess-path`
/// (a digest and a `--version`) and one `ess compile` of the fixture, not one of each per check.
#[test]
fn the_pinned_ess_and_the_compiled_fixture_are_made_once_for_many_checks() {
    let dir = tmp("check-once");
    for _ in 0..4 {
        assert!(check(&dir, &table()).0);
    }
    assert_eq!(ESS_PATH_RUNS.load(std::sync::atomic::Ordering::SeqCst), 1, "runs of `sdc-conform ess-path`");
    assert_eq!(COMPILES.lock().unwrap().get(&dir.join("ir-spec.json")).copied(), Some(1), "runs of `ess compile`");
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
    installed();
    let run = run_dir("capture", "spec");
    let t = write_table(run.parent().unwrap(), "t.json", &token_table("token", true));
    let (o, d) = observe(&run, &t, &[]);
    assert!(o.status.success(), "{}", says(&o));
    assert_eq!(d["conformance"]["result"], "held", "{:#}", d["conformance"]);
    assert_eq!(d["conformance"]["summary"], "spec scenarios proven passed: 7 of 7 (lower bound); 0 failed; 0 not measurable; 1 table (held)");
    let run = run_dir("capture-unbound", "spec");
    let t = write_table(run.parent().unwrap(), "t.json", &token_table("token", false));
    let (_, d) = observe(&run, &t, &[]);
    let c = &d["conformance"];
    assert_eq!(c["result"], "contradicted", "{c:#}");
    assert_eq!(c["summary"], "spec scenarios proven passed: 6 of 7 (lower bound); 1 failed; 0 not measurable; 1 table");
    assert!(c["scenarios"].as_array().unwrap().iter().any(|x| x["id"] == "chores.list.OpenBoard/outcome/opened" && x["status"] == "failed"), "{c:#}");
}

const ADAPTER: &str = env!("CARGO_BIN_EXE_sdc-conform-adapter");

/// One `begin_scenario` sent to a bare `sdc-conform-adapter` whose table captures `token` from
/// stdout within `timeout_s`, for the stub product in `mode`: the `unavailable` detail the adapter
/// answered, and how long the answer took.
fn begin_scenario_detail(name: &str, mode: &str, timeout_s: u64) -> (String, std::time::Duration) {
    use std::io::{BufRead, BufReader, Write};
    let run = run_dir(name, "spec");
    let work = run.join("store/IDEA-1/work");
    let mut t = token_table(mode, true);
    t["captures"][0]["timeout_s"] = json!(timeout_s);
    let table = write_table(run.parent().unwrap(), "t.json", &t);
    let mut child = Command::new(ADAPTER)
        .args(["--table", &table, "--tree"])
        .arg(work.join("published"))
        .arg("--work")
        .arg(run.parent().unwrap().join("adapter-work"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let started = std::time::Instant::now();
    let mut stdin = child.stdin.take().unwrap();
    writeln!(stdin, "{}", json!({"id": 1, "op": "begin_scenario", "request": {"scenario": "x"}})).unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let took = started.elapsed();
    drop(stdin);
    child.wait().unwrap();
    let reply: J = serde_json::from_str(&line).unwrap_or_else(|e| panic!("reply {line:?}: {e}"));
    assert_eq!(reply["target_error"]["kind"], "unavailable", "{reply:#}");
    (s(&reply["target_error"]["detail"]).to_string(), took)
}

/// A product that closed the stream the capture is read from can never print it: the adapter says
/// so at once and does not wait out `timeout_s` (20 s here; 7 scenarios x 3 runs made 21 s of
/// waiting at 1 s in the test below).
#[test]
fn a_capture_on_a_stream_the_product_closed_is_missing_at_once() {
    let (detail, took) = begin_scenario_detail("capture-closed", "token-silent", 20);
    assert!(detail.contains("capture token did not appear on stdout") && detail.contains("closed"), "{detail}");
    assert!(took < std::time::Duration::from_secs(10), "answered after {took:?}, the timeout is 20 s");
}

/// A product that keeps the stream open and prints nothing is waited for until `timeout_s`.
#[test]
fn a_capture_that_does_not_appear_on_an_open_stream_waits_its_timeout() {
    let (detail, took) = begin_scenario_detail("capture-open", "token-quiet", 1);
    assert!(detail.contains("capture token did not appear on stdout within 1 s"), "{detail}");
    assert!(took >= std::time::Duration::from_secs(1), "answered after {took:?}, before the timeout");
}

#[test]
fn a_capture_that_does_not_appear_is_unavailable_and_the_figure_unknown() {
    installed();
    let run = run_dir("capture-missing", "spec");
    let mut silent = token_table("token-silent", true);
    // the product never prints the token and closes its output: each scenario of the 3 runs ends
    // at once (`a_capture_on_a_stream_the_product_closed_is_missing_at_once`)
    silent["captures"][0]["timeout_s"] = json!(1);
    let t = write_table(run.parent().unwrap(), "t.json", &silent);
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
    installed();
    let mut t = table();
    t["commands"][1]["forced"] = json!([]);
    t["commands"][1]["outcomes"][1] = json!({"outcome": "unreachable",
        "none": {"kind": "not_reachable_over_the_interface", "why": "the board link lives only in the browser"}});
    let run = run_dir("not-measurable", "spec");
    let tp = write_table(run.parent().unwrap(), "t.json", &t);
    let (o, d) = observe(&run, &tp, &[]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    assert_eq!(c["summary"], "spec scenarios proven passed: 6 of 7 (lower bound); 0 failed; 1 not measurable; 1 table", "{c:#}");
    assert_eq!(c["result"], "unknown");
    assert_eq!(d["checks"][0]["u"], 1);
    assert!(c["scenarios"].as_array().unwrap().iter().any(|x| x["id"] == "chores.list.FinishChore/outcome/unreachable" && x["status"] == "not_measurable"), "{c:#}");
    t["commands"][1]["outcomes"][1]["none"]["kind"] = json!("not_implemented");
    let run = run_dir("not-implemented", "spec");
    let tp = write_table(run.parent().unwrap(), "t.json", &t);
    let (_, d) = observe(&run, &tp, &[]);
    assert_eq!(d["conformance"]["summary"], "spec scenarios proven passed: 6 of 7 (lower bound); 1 failed; 0 not measurable; 1 table", "{:#}", d["conformance"]);
    assert_eq!(d["conformance"]["result"], "contradicted");
}

#[test]
fn a_scenario_with_a_failed_check_fails_even_when_it_also_needs_what_cannot_be_reached() {
    installed();
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
    installed();
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
    assert_eq!(d["checks"][0], json!({"id": "conformance", "result": "contradicted", "k": 6, "f": 1, "p": 0, "d": 0, "u": 0, "n": 7,
        "agreement": 6, "tables_counted": 3, "cause": null}), "{c:#}");
    assert_eq!(c["summary"], "spec scenarios proven passed: 6 of 7 (lower bound); 1 failed; 0 not measurable; tables agree on 6 of 7");
    let tables = c["tables"].as_array().unwrap();
    assert_eq!(tables.iter().map(figure).collect::<Vec<_>>(), vec![(6, 1, 0), (5, 1, 1), (5, 1, 1)], "{c:#}");
    assert!(tables.iter().all(|t| t["counted"] == true && t["cost_usd"].is_number() && t["n"] == 7), "{c:#}");
    assert_eq!(tables.iter().map(|t| t["slot"].clone()).collect::<Vec<_>>(), vec![json!(1), json!(2), json!(3)]);
    let un = scenario(c, "chores.list.FinishChore/outcome/unreachable");
    assert_eq!((&un["status"], &un["under"]), (&json!("passed"), &json!(["passed", "not_measurable", "not_measurable"])));
    let ws = scenario(c, "chores.list.Chore/state/Done/refuses/chores.list.FinishChore");
    assert_eq!((&ws["status"], &ws["under"]), (&json!("failed"), &json!(["failed", "failed", "failed"])));
    assert!(run.join("conformance/table-3/node/001/prompt.md").is_file());
    assert!(String::from_utf8_lossy(&o.stdout).contains("conformance: spec scenarios proven passed: 6 of 7 (lower bound); 1 failed; 0 not measurable; tables agree on 6 of 7"), "{}", says(&o));
    let (_, d) = observe(&run, &b, &[("SDC_CONFORMANCE_TABLES", "3")]);
    assert_eq!(d["conformance"]["summary"], "spec scenarios proven passed: 5 of 7 (lower bound); 1 failed; 1 not measurable; tables agree on 7 of 7","{:#}", d["conformance"]);
    assert_eq!(d["conformance"]["result"], "contradicted");
}

#[test]
fn a_table_that_never_signs_in_is_refused_after_its_run_and_asked_again_once() {
    installed();
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
    installed();
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
    installed();
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
    assert_eq!(d["conformance"]["summary"], "spec scenarios proven passed: 7 of 7 (lower bound); 0 failed; 0 not measurable; 1 table (held)", "{:#}", d["conformance"]);
    assert_eq!(d["conformance"]["result"], "held");
}

#[test]
fn an_id_the_scenario_never_created_is_refused_wrong_state_by_the_adapter() {
    installed();
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
    assert_eq!(c["summary"], "spec scenarios proven passed: 7 of 7 (lower bound); 0 failed; 0 not measurable; 1 table (held)", "{c:#}");
    let log = fs::read_to_string(run.join("conformance/table-1/run-1/adapter-log.jsonl")).unwrap();
    assert!(log.lines().any(|l| l.contains("\"unknown_instance\"") && l.contains("chores.list.FinishChore/outcome/wrong-state") && l.contains("\"status\":404")), "{log}");
}

#[test]
fn a_404_for_an_entity_the_scenario_created_is_never_wrong_state() {
    installed();
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
    installed();
    let run = run_dir("disputed", "spec");
    let dir = run.parent().unwrap().to_path_buf();
    let a = write_table(&dir, "a.json", &table());
    let mut b = table();
    b["commands"][1]["outcomes"][2]["status"] = json!(422);
    let b = write_table(&dir, "b.json", &b);
    let (o, d) = observe(&run, &format!("{a};{b}"), &[("SDC_CONFORMANCE_TABLES", "2")]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    assert_eq!(d["checks"][0], json!({"id": "conformance", "result": "unknown", "k": 5, "f": 0, "p": 0, "d": 2, "u": 0, "n": 7,
        "agreement": 5, "tables_counted": 2, "cause": "disputed"}), "{c:#}");
    assert_eq!(c["summary"], "spec scenarios proven passed: 5 of 7 (lower bound); 0 failed; 2 disputed; 0 not measurable; tables agree on 5 of 7");
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
    installed();
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
    installed();
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
    installed();
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
fn a_scenario_one_table_saw_fail_is_failed_though_the_others_declare_it_unreachable() {
    installed();
    let run = run_dir("failed-over-unreachable", "spec");
    let dir = run.parent().unwrap().to_path_buf();
    let mut a = table();
    a["commands"][1]["outcomes"][1]["status"] = json!(500);
    let a = write_table(&dir, "a.json", &a);
    let mut b = table();
    b["commands"][1]["forced"] = json!([]);
    b["commands"][1]["outcomes"][1] = json!({"outcome": "unreachable",
        "none": {"kind": "not_reachable_over_the_interface", "why": "the board lives only in the browser"}});
    let b = write_table(&dir, "b.json", &b);
    let (o, d) = observe(&run, &format!("{a};{b}"), &[("SDC_CONFORMANCE_TABLES", "3")]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    let un = scenario(c, "chores.list.FinishChore/outcome/unreachable");
    assert_eq!((&un["status"], &un["under"]), (&json!("failed"), &json!(["failed", "not_measurable", "not_measurable"])), "{c:#}");
    assert_eq!(c["summary"], "spec scenarios proven passed: 6 of 7 (lower bound); 1 failed; 0 not measurable; tables agree on 6 of 7");
    assert_eq!(c["result"], "contradicted");
}

#[test]
fn a_sign_in_the_specification_does_not_model_fails_the_refused_scenarios_as_a_precondition() {
    installed();
    let run = run_dir("precondition", "spec");
    let dir = run.parent().unwrap().to_path_buf();
    let (o, d) = observe(&run, &write_table(&dir, "t.json", &with_mode("session")), &[]);
    assert!(o.status.success(), "{}", says(&o));
    let c = &d["conformance"];
    assert_eq!(c["summary"], "spec scenarios proven passed: 1 of 7 (lower bound); 6 failed, of which 6 refused for a precondition the specification does not model; 0 not measurable; 1 table", "{c:#}");
    assert_eq!((&c["result"], &d["checks"][0]["p"]), (&json!("contradicted"), &json!(6)));
    let added = scenario(c, "chores.list.AddChore/outcome/added");
    assert_eq!((&added["status"], &added["cause"]), (&json!("failed"), &json!("precondition_not_in_spec")), "{c:#}");
    assert_eq!(scenario(c, "chores.list.OpenBoard/outcome/opened")["status"], "passed");
    let t = &c["tables"][0];
    assert_eq!((&t["counted"], &t["retried_after_run"], &t["cause"]), (&json!(true), &json!(true), &J::Null), "{t:#}");
    assert_eq!(t["precondition_not_in_spec"].as_array().map(Vec::len), Some(6), "{t:#}");
    assert!(String::from_utf8_lossy(&o.stdout).contains("6 failed, of which 6 refused for a precondition"), "{}", says(&o));
    // A product that refuses every request wants a key the table did not send: not measured.
    let (_, d) = observe(&run, &write_table(&dir, "plain.json", &with_mode("token")), &[]);
    assert_eq!((&d["conformance"]["result"], &d["conformance"]["cause"]), (&json!("unknown"), &json!("table_error")), "{:#}", d["conformance"]);
}

#[test]
fn a_stored_observation_is_recomputed_without_a_node_and_summarized_without_product_text() {
    installed();
    let run = run_dir("recompute", "spec");
    let dir = run.parent().unwrap().to_path_buf();
    let (o, d) = observe(&run, &write_table(&dir, "t.json", &with_mode("session")), &[]);
    assert!(o.status.success(), "{}", says(&o));
    let before = d["conformance"]["summary"].clone();
    // The record as an observation before the precondition rule would have left it.
    let mut old = d.clone();
    old["conformance"]["tables"][0]["counted"] = json!(false);
    old["conformance"]["tables"][0]["cause"] = json!("table_error");
    old["conformance"]["tables"][0]["why"] = json!("refused after its run: the product answered 401");
    old["conformance"]["summary"] = json!("not measured (table_error), 7 scenarios");
    fs::write(run.join("oracle.json"), old.to_string()).unwrap();
    let o = Command::new(BIN).arg("recompute").arg(&run).env_remove("SDC_STUB_TABLE_NODE").output().unwrap();
    assert!(o.status.success(), "{}", says(&o));
    let d: J = serde_json::from_str(&fs::read_to_string(run.join("oracle.json")).unwrap()).unwrap();
    let c = &d["conformance"];
    assert_eq!(c["summary"], before, "{c:#}");
    assert_eq!(c["recomputed"]["previous_summary"], "not measured (table_error), 7 scenarios");
    assert_eq!((&c["tables"][0]["attempts"], &c["node_sessions"]), (&old["conformance"]["tables"][0]["attempts"], &old["conformance"]["node_sessions"]), "{c:#}");
    let o = Command::new(BIN).arg("summary").arg(&run).output().unwrap();
    assert!(o.status.success(), "{}", says(&o));
    let sum: J = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!((&sum["format"], &sum["k"], &sum["f"], &sum["p"], &sum["n"]), (&json!("sdc.conformance-summary/1"), &json!(1), &json!(6), &json!(6), &json!(7)));
    assert_eq!(sum["scenario_causes"], json!({"failed:precondition_not_in_spec": 6}));
    let text = sum.to_string();
    assert!(!text.contains("NotSignedIn") && !text.contains("chores.list") && !text.contains("\"why\""), "{text}");
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

/// Whether process `pid` still runs (a zombie does not: nothing of it serves any more).
fn alive(pid: &str) -> bool {
    fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|s| s.rsplit(')').next().is_some_and(|t| t.trim_start().split(' ').next() != Some("Z")))
}

/// The product of a killed adapter does not stay: its parent (a shell here, standing for the
/// adapter) is killed with SIGKILL, so no destructor runs, and the stub must be gone within 5 s.
/// The stub's port is held until the case ends, as the adapter holds it: a port freed and handed on
/// was bound by another case's product first in one of 50 runs of these modules ("the stub did not
/// start", `AddrInUse`).
#[test]
fn the_stub_product_ends_when_its_parent_is_killed() {
    use std::io::{BufRead, BufReader};
    let dir = tmp("orphan-stub");
    let (_held, port) = super::capture_fate::held_port();
    let port = port.to_string();
    let mut parent = Command::new("sh")
        .args(["-c", "\"$0\" \"$@\" & echo $!; wait"]).arg(stub()).args([port.as_str(), dir.to_str().unwrap(), "ok"])
        .stdout(std::process::Stdio::piped()).spawn().unwrap();
    let mut pid = String::new();
    BufReader::new(parent.stdout.take().unwrap()).read_line(&mut pid).unwrap();
    let pid = pid.trim().to_string();
    let listening = |port: &str| std::net::TcpStream::connect(("127.0.0.1", port.parse::<u16>().unwrap())).is_ok();
    let up = std::time::Instant::now();
    while !listening(&port) && up.elapsed().as_secs() < 5 {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(listening(&port) && alive(&pid), "the stub did not start");
    parent.kill().unwrap();
    parent.wait().unwrap();
    let killed = std::time::Instant::now();
    while alive(&pid) && killed.elapsed().as_secs() < 5 {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let left = alive(&pid);
    if left {
        let _ = Command::new("kill").args(["-s", "KILL", "--", &pid]).status();
    }
    assert!(!left, "the stub product {pid} still runs 5 s after its parent was killed");
}

/// Integrity check of the money budget: a `SDC_CONFORMANCE_MAX_BUDGET_USD` that is SET and cannot be read
/// as a finite number must never reach the table node as "no budget" (no `max_budget_usd` member, or a
/// `null` one, which the adapters read as "no cap"). The run may refuse; the node may be started only with
/// a finite number in its request.
fn budget_set_but_unreadable(tag: &str, value: &str) {
    installed();
    let run = run_dir(&format!("budget-set-{tag}"), "spec");
    let mut t = table();
    t["views"] = json!([]);
    let t = write_table(run.parent().unwrap(), "t.json", &t);
    let (_o, _) = observe(&run, &t, &[("SDC_CONFORMANCE_MAX_BUDGET_USD", value)]);
    let path = run.join("conformance/table-1/node/001/request.json");
    let Ok(text) = fs::read_to_string(&path) else { return };
    let request: J = serde_json::from_str(&text).unwrap();
    let budget = request.get("max_budget_usd").and_then(J::as_f64);
    assert!(
        budget.is_some_and(f64::is_finite),
        "SDC_CONFORMANCE_MAX_BUDGET_USD={value:?} was set; the node was started with max_budget_usd {:?} (no budget)",
        request.get("max_budget_usd")
    );
}

#[test]
fn conformance_budget_set_to_text_is_not_no_budget() {
    budget_set_but_unreadable("text", "abc");
}

#[test]
fn conformance_budget_set_with_a_unit_is_not_no_budget() {
    budget_set_but_unreadable("unit", "2usd");
}

#[test]
fn conformance_budget_set_with_a_space_is_not_no_budget() {
    budget_set_but_unreadable("space", " 2");
}

#[test]
fn conformance_budget_set_to_nan_is_not_no_budget() {
    budget_set_but_unreadable("nan", "NaN");
}

#[test]
fn conformance_budget_set_to_inf_is_not_no_budget() {
    budget_set_but_unreadable("inf", "inf");
}

#[test]
fn conformance_budget_set_beyond_f64_is_not_no_budget() {
    budget_set_but_unreadable("overflow", "1e400");
}

/// `observe` with the budget variable given as raw bytes (an environment value need not be UTF-8).
fn observe_budget_bytes(tag: &str, value: &std::ffi::OsStr) -> (Output, PathBuf) {
    installed();
    let run = run_dir(&format!("budget-set-{tag}"), "spec");
    let mut t = table();
    t["views"] = json!([]);
    let t = write_table(run.parent().unwrap(), "t.json", &t);
    let mut c = Command::new(BIN);
    c.arg("observe").arg(&run)
        .env("SDC_CONFORMANCE_ADAPTER", stub())
        .env("SDC_STUB_TABLE_NODE", &t)
        .env("SDC_CONFORMANCE_TARGET_TIMEOUT", "10")
        .env("SDC_CONFORMANCE_TABLES", "1")
        .env_remove("SDC_LLM_MODEL_CONFORMANCE")
        .env_remove("SDC_LLM_EFFORT_CONFORMANCE")
        .env_remove("SDC_LLM_EFFORT_OTHER")
        .env_remove("SDC_LLM_EFFORT")
        .env("SDC_CONFORMANCE_MAX_BUDGET_USD", value);
    (c.output().unwrap(), run)
}

/// The `max_budget_usd` of the table node's request, `None` when the observation wrote no request.
fn requested_budget(run: &Path) -> Option<J> {
    let text = fs::read_to_string(run.join("conformance/table-1/node/001/request.json")).ok()?;
    let request: J = serde_json::from_str(&text).unwrap();
    Some(request.get("max_budget_usd").cloned().unwrap_or(J::Null))
}

/// A named budget whose bytes are no UTF-8 text cannot be read as a number: it is refused, never
/// read as unset, and no byte of it is shown.
#[test]
fn conformance_budget_set_to_bytes_that_are_no_text_is_refused_by_name() {
    use std::os::unix::ffi::OsStrExt as _;
    let (o, run) = observe_budget_bytes("bytes", std::ffi::OsStr::from_bytes(b"2\xff"));
    assert_eq!(requested_budget(&run), None, "the node was started: {}", says(&o));
    let oracle = fs::read_to_string(run.join("oracle.json")).unwrap();
    assert!(oracle.contains("SDC_CONFORMANCE_MAX_BUDGET_USD") && oracle.contains("table_missing"), "{oracle}");
}

/// A named value is the node's budget as named, or the observation is refused with no request written:
/// 0 is a budget of 0, below 0 is refused (the lower bound), and so is a hex number.
/// One start per row: 3 rows, one of them a started node.
#[test]
fn conformance_budget_set_is_the_named_number_or_refused() {
    let mut bad = vec![];
    for (tag, value, equals) in [("zero", "0", Some(0.0)), ("neg", "-1", None), ("hex", "0x10", None)] {
        let (o, run) = observe_budget_bytes(&format!("t-{tag}"), std::ffi::OsStr::new(value));
        match (requested_budget(&run), equals) {
            (None, None) => {}
            (Some(g), Some(n)) if g.as_f64() == Some(n) => {}
            (got, _) => bad.push(format!("{value:?}: request max_budget_usd {got:?}, named {equals:?}; {}", says(&o))),
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// A refusal of the variable's value cuts the value at 40 bytes: no output and nothing under the run
/// holds more of a long value (a pasted secret) than that, and the variable is named.
#[test]
fn conformance_budget_refusal_cuts_the_value_at_40_bytes() {
    let value = format!("sk-ant-api03-{}", "Q".repeat(100));
    let (o, run) = observe_budget_bytes("cut", std::ffi::OsStr::new(&value));
    let mut seen = says(&o);
    let mut files = vec![run];
    while let Some(d) = files.pop() {
        for e in fs::read_dir(&d).into_iter().flatten().flatten() {
            if e.path().is_dir() {
                files.push(e.path());
            } else {
                seen.push_str(&String::from_utf8_lossy(&fs::read(e.path()).unwrap_or_default()));
            }
        }
    }
    assert!(seen.contains("SDC_CONFORMANCE_MAX_BUDGET_USD"), "the refusal names the variable: {seen}");
    assert!(!seen.contains(&value[..41]), "more than 40 bytes of the value are in the output or the run");
}

/// An empty variable names no budget: the request has no `max_budget_usd`. `-0` is a budget of 0 with a
/// positive sign: the request's number is `+0`, never `-0.0` (an adapter passes the text on as written).
/// One start per row: 2 rows.
#[test]
fn conformance_budget_empty_is_no_budget_and_minus_zero_is_a_request_of_plus_zero() {
    let mut bad = vec![];
    for (tag, value, want) in [("empty", "", None), ("negzero", "-0", Some(0.0))] {
        let (o, run) = observe_budget_bytes(&format!("e-{tag}"), std::ffi::OsStr::new(value));
        let ok = match (requested_budget(&run), want) {
            (Some(g), None) => g.is_null(),
            (Some(g), Some(n)) => g.as_f64() == Some(n) && g.as_f64().is_some_and(f64::is_sign_positive),
            (None, _) => false,
        };
        if !ok {
            bad.push(format!("{value:?}: request max_budget_usd {:?}, want {want:?}; {}", requested_budget(&run), says(&o)));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// The order of the observation: the pin check runs first (it starts the pinned binary with `--version`),
/// then the budget variable is read, then `ess compile` runs. A refused variable starts no command of
/// `ess` but the pin check; a good one is followed by `compile`.
#[test]
fn conformance_budget_refused_starts_no_ess_command_but_the_pin_check() {
    installed();
    let mut bad = vec![];
    for (tag, value, compiles) in [("neg", "-1", false), ("text", "abc", false), ("good", "2", true)] {
        let run = run_dir(&format!("order-{tag}"), "spec");
        let dir = run.parent().unwrap().to_path_buf();
        let log = dir.join("ess-calls.txt");
        let body = format!("#!/bin/sh\necho \"$@\" >> {}\n[ \"$1\" = \"--version\" ] && echo 'ess 9.9.9'\nexit 1\n", log.display());
        let ess = dir.join("ess");
        crate::ess_pin_guard::executable(&ess, &body);
        let want = sdc_tools::conformance::sha256_bytes(body.as_bytes());
        let pin = dir.join("pin.json");
        fs::write(&pin, json!({"format": "sdc.ess-pin/1", "version": "9.9.9", "platforms": {sdc_tools::conformance::target_triple(&J::Null): {"binary_sha256": want}}}).to_string()).unwrap();
        let mut t = table();
        t["views"] = json!([]);
        let t = write_table(&dir, "t.json", &t);
        let o = Command::new(BIN)
            .arg("observe")
            .arg(&run)
            .arg("--pin")
            .arg(&pin)
            .env("ESS_BIN", &ess)
            .env("SDC_CONFORMANCE_ADAPTER", stub())
            .env("SDC_STUB_TABLE_NODE", &t)
            .env("SDC_CONFORMANCE_TARGET_TIMEOUT", "10")
            .env("SDC_CONFORMANCE_TABLES", "1")
            .env("SDC_CONFORMANCE_MAX_BUDGET_USD", value)
            .output()
            .unwrap();
        let calls = fs::read_to_string(&log).unwrap_or_default();
        let lines: Vec<&str> = calls.lines().collect();
        if lines.first() != Some(&"--version") {
            bad.push(format!("{tag}: the pin check did not run first: {lines:?}; {}", says(&o)));
        }
        let compiled = lines.iter().any(|l| l.starts_with("compile"));
        if compiled != compiles {
            bad.push(format!("{tag} {value}: ess compile ran {compiled}, want {compiles}: {lines:?}"));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// The platform of a pinned tool: a musl build takes `<arch>-unknown-linux-musl` when the pin names that platform and
/// `<arch>-unknown-linux-gnu` when it does not (the ess pin names no musl archive); a gnu build takes
/// `-linux-gnu` whatever the pin names, and macOS its `-apple-darwin`. This test binary is a gnu build, so
/// `target_triple` of a pin that names both platforms says `-linux-gnu`.
#[test]
fn target_triple_takes_the_musl_platform_only_when_the_pin_names_it() {
    use sdc_tools::conformance::{platform_for, target_triple};
    let both = json!({"platforms": {"x86_64-unknown-linux-gnu": {}, "x86_64-unknown-linux-musl": {}}});
    let gnu_only = json!({"platforms": {"x86_64-unknown-linux-gnu": {}}});
    let musl_only = json!({"platforms": {"x86_64-unknown-linux-musl": {}}});
    let mut bad = vec![];
    for (arch, os, musl, pin, want) in [
        ("x86_64", "linux", true, &both, "x86_64-unknown-linux-musl"),
        ("x86_64", "linux", true, &musl_only, "x86_64-unknown-linux-musl"),
        ("x86_64", "linux", true, &gnu_only, "x86_64-unknown-linux-gnu"),
        ("x86_64", "linux", true, &J::Null, "x86_64-unknown-linux-gnu"),
        ("aarch64", "linux", true, &both, "aarch64-unknown-linux-gnu"),
        ("x86_64", "linux", false, &both, "x86_64-unknown-linux-gnu"),
        ("x86_64", "linux", false, &musl_only, "x86_64-unknown-linux-gnu"),
        ("aarch64", "macos", false, &both, "aarch64-apple-darwin"),
    ] {
        let got = platform_for(arch, os, musl, pin);
        if got != want {
            bad.push(format!("{arch} {os} musl={musl} pin={pin}: {got}, want {want}"));
        }
    }
    if cfg!(all(target_os = "linux", target_env = "gnu")) {
        let host = format!("{}-unknown-linux-gnu", std::env::consts::ARCH);
        let got = target_triple(&both);
        if got != host {
            bad.push(format!("target_triple of this gnu build with a pin that names musl: {got}, want {host}"));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}
