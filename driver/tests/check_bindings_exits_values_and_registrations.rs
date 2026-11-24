//! Values the shape check of `scripts/check-bindings.sh --exits <root>` must not let through to a jq error (an argv of false, a result key with a final newline), and the registrations step of the full check for a protocol file it cannot read.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const GOOD: &str = "format: loom.bindings/1\nprotocol: example@1\nactions:\n  - action: a.one\n    executor: {kind: program, argv: [tools/x.sh]}\n    exits: {error: [75, 126, 127]}\n";
const HDR: &str = "format: loom.bindings/1\nprotocol: example@1\nactions:\n";

fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/check-bindings.sh")
}

fn root_in(name: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("check-bindings-values-and-registrations").join(name);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("bindings")).unwrap();
    root
}

fn run(root: &Path) -> (Option<i32>, String, String) {
    let o = Command::new("bash").arg(script()).arg("--exits").arg(root).output().unwrap();
    (o.status.code(), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn refused_by_name(name: &str, body: &str) {
    let root = root_in(name);
    fs::write(root.join("bindings/good.yaml"), GOOD).unwrap();
    fs::write(root.join("bindings/bad.yaml"), format!("{HDR}{body}")).unwrap();
    let (code, out, err) = run(&root);
    assert_eq!(code, Some(1), "{name}: {out}\n{err}");
    assert!(out.contains("bindings/bad.yaml b.two:") && !format!("{out}{err}").contains("jq:"), "{name}: {out}\n{err}");
}

#[test]
fn an_argv_of_false_is_refused_by_name_and_not_by_a_jq_error() {
    refused_by_name("argv-false", "  - action: b.two\n    executor: {kind: program, argv: false}\n    exits: {error: [75, 126, 127]}\n");
}

#[test]
fn a_result_key_with_a_trailing_newline_is_refused_by_name_and_not_by_a_jq_error() {
    refused_by_name(
        "key-newline",
        "  - action: b.two\n    executor: {kind: program, argv: [x]}\n    exits: {error: [75, 126, 127]}\n    evidence: [{kind: k, from: exit, results: {\"1\\n\": a}}]\n",
    );
}

fn tree_for_registrations(name: &str) -> PathBuf {
    let root = root_in(name);
    let from = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    fs::create_dir_all(root.join("protocol/compiled")).unwrap();
    fs::create_dir_all(root.join("driver/examples/sdc")).unwrap();
    fs::create_dir_all(root.join("scripts")).unwrap();
    for e in fs::read_dir(from.join("protocol")).unwrap() {
        let p = e.unwrap().path();
        if p.to_string_lossy().ends_with(".protocol.yaml") {
            fs::copy(&p, root.join("protocol").join(p.file_name().unwrap())).unwrap();
        }
    }
    fs::copy(from.join("bindings/PENDING.md"), root.join("bindings/PENDING.md")).unwrap();
    fs::copy(from.join("bindings/sdc-story.yaml"), root.join("bindings/sdc-story.yaml")).unwrap();
    fs::copy(from.join("driver/examples/sdc/sdc-story.bindings.json"), root.join("driver/examples/sdc/sdc-story.bindings.json")).unwrap();
    fs::copy(from.join("protocol/compiled/sdc-story.ir.json"), root.join("protocol/compiled/sdc-story.ir.json")).unwrap();
    std::os::unix::fs::symlink(from.join("instructions").canonicalize().unwrap(), root.join("instructions")).unwrap();
    std::os::unix::fs::symlink(from.join("bindings/tools").canonicalize().unwrap(), root.join("bindings/tools")).unwrap();
    let text = fs::read_to_string(script()).unwrap();
    let cut = text.split("# The answers file of").next().unwrap().replacen("\ncheck_exits\n", "\n", 1);
    fs::write(root.join("scripts/check-bindings.sh"), format!("{cut}exit $fail\n")).unwrap();
    root
}

#[test]
fn the_registrations_step_fails_the_full_check_for_a_protocol_file_it_cannot_read() {
    let root = tree_for_registrations("registrations-protocol-file");
    let full = |root: &Path| {
        let o = Command::new("bash").arg(root.join("scripts/check-bindings.sh")).output().unwrap();
        (o.status.code(), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
    };
    let (code, out, err) = full(&root);
    assert!(out.contains("pass registrations") && code == Some(0), "the fixture is not a clean tree: {out}\n{err}");
    fs::write(root.join("protocol/zzz.protocol.yaml"), "this: is: [not\n  yaml {{{\n").unwrap();
    let (code, out, err) = full(&root);
    assert_eq!(code, Some(1), "{out}\n{err}");
    assert!(out.contains("FAIL registrations") && !out.contains("pass registrations"), "{out}\n{err}");
}
