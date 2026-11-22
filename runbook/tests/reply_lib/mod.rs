//! The `sdc_tools` library, called from `runbook`'s build (serde_json `preserve_order` on), gives the bytes
//! the `sdc-reply` binary prints (driver build, sorted keys): a citable set, a decider schema and a rule
//! approval on a hand-written fixture (`tests/fixtures/reply/`). The expected files are the binary's output;
//! `driver/tests/reply_bytes.rs` checks the binary against the same files.

use super::repo;
use sdc_tools::reply::citations::{citable_set, ctx};
use sdc_tools::reply::records::Facts;
use sdc_tools::reply::rules::rule_approval;
use sdc_tools::reply::schemas::decider_schema;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reply")
}

fn expected(name: &str) -> String {
    std::fs::read_to_string(fixtures().join("expected").join(name)).unwrap()
}

fn bytes(v: &Value) -> String {
    format!("{v}\n")
}

#[test]
fn the_library_prints_the_bytes_the_binary_prints() {
    let c = ctx(&fixtures().join("call"), &Facts::open(&fixtures().join("case")));
    assert_eq!(bytes(&citable_set(&c)), expected("citable.json"));
    assert_eq!(bytes(&decider_schema(&c, "approval", false, &[], false)), expected("schema-approval.json"));
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("reply-lib-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let case = root.join("case");
    std::fs::create_dir_all(case.join("evidence")).unwrap();
    let repo = repo();
    std::fs::copy(repo.join("protocol/compiled/sdc-direction.ir.json"), case.join("ir.json")).unwrap();
    std::fs::copy(repo.join("driver/examples/sdc/sdc-direction.bindings.json"), case.join("bindings.json")).unwrap();
    std::fs::write(
        case.join("case.json"),
        r#"{"id":"IDEA-1.x","protocol":"sdc.direction","artifacts":{"context":{"revision":"t1"},"requirements":{"revision":"r1"}}}"#,
    )
    .unwrap();
    std::fs::write(
        case.join("evidence/e0004-context.check.json"),
        r#"{"format":"canon-evidence/1","id":"e0004-context.check","kind":"context_defaults","result":"valid","subject":"context","subject_revision":"t1","upstream_revisions":null}"#,
    )
    .unwrap();
    let request = json!({
        "type": "approval", "case_dir": case.to_str().unwrap(), "action": "context.approve", "subject": "context", "revision": "t1",
        "position": {"claims": {"context.valid": {"value": "true"}, "context.decided": {"value": "unknown"}}, "open_obligations": []},
    });
    let ends: Value = serde_json::from_str(&std::fs::read_to_string(repo.join("driver/examples/sdc/rule-ends.json")).unwrap()).unwrap();
    assert_eq!(bytes(&rule_approval(&request, &ends)), expected("rule-approval.json"));
    let _ = std::fs::remove_dir_all(&root);
}
