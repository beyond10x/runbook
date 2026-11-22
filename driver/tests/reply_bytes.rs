//! The `sdc-reply` binary prints the bytes of the expected files in `runbook/tests/fixtures/reply/expected/`;
//! `runbook/tests/reply_lib/` checks the library, linked into `runbook`, against the same files.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn fixtures() -> PathBuf {
    repo().join("runbook/tests/fixtures/reply")
}

fn print(args: &[&str]) -> String {
    let o = Command::new(env!("CARGO_BIN_EXE_sdc-reply")).args(args).env_remove("LOOM_CASE").output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap()
}

fn expected(name: &str) -> String {
    std::fs::read_to_string(fixtures().join("expected").join(name)).unwrap()
}

#[test]
fn the_binary_prints_the_expected_bytes() {
    let (call, case) = (fixtures().join("call"), fixtures().join("case"));
    let (call, case) = (call.to_str().unwrap(), case.to_str().unwrap());
    assert_eq!(print(&["citable", "--cwd", call, "--case-dir", case]), expected("citable.json"));
    assert_eq!(print(&["schema", "--cwd", call, "--case-dir", case, "--type", "approval"]), expected("schema-approval.json"));
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("reply-bytes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let dir = root.join("case");
    std::fs::create_dir_all(dir.join("evidence")).unwrap();
    std::fs::copy(repo().join("protocol/compiled/sdc-direction.ir.json"), dir.join("ir.json")).unwrap();
    std::fs::copy(repo().join("driver/examples/sdc/sdc-direction.bindings.json"), dir.join("bindings.json")).unwrap();
    std::fs::write(
        dir.join("case.json"),
        r#"{"id":"IDEA-1.x","protocol":"sdc.direction","artifacts":{"context":{"revision":"t1"},"requirements":{"revision":"r1"}}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("evidence/e0004-context.check.json"),
        r#"{"format":"canon-evidence/1","id":"e0004-context.check","kind":"context_defaults","result":"valid","subject":"context","subject_revision":"t1","upstream_revisions":null}"#,
    )
    .unwrap();
    let request = root.join("request.json");
    std::fs::write(
        &request,
        format!(
            r#"{{"type":"approval","case_dir":"{}","action":"context.approve","subject":"context","revision":"t1","position":{{"claims":{{"context.valid":{{"value":"true"}},"context.decided":{{"value":"unknown"}}}},"open_obligations":[]}}}}"#,
            dir.display()
        ),
    )
    .unwrap();
    let ends = repo().join("driver/examples/sdc/rule-ends.json");
    assert_eq!(
        print(&["rule-approval", "--request", request.to_str().unwrap(), "--ends", ends.to_str().unwrap()]),
        expected("rule-approval.json")
    );
    let _ = std::fs::remove_dir_all(&root);
}
