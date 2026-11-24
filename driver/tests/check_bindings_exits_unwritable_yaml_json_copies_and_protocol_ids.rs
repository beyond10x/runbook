//! Inputs of `scripts/check-bindings.sh` that the table of the value test does not vary: YAML that `yq` cannot write as JSON, the JSON copies
//! of the bindings, and protocol files without a text id. Each case is red on `main` (5fc1cfe4) as well and is `#[ignore]`d as a measurement
//! until the story check-bindings-refuses-every-file-by-name is built.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/check-bindings.sh")
}

fn root_in(name: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("check-bindings-unwritable-json-protocol").join(name);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("bindings")).unwrap();
    root
}

fn said(o: &std::process::Output) -> (String, String) {
    (String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// A tree for the full check cut before its answers file: this repository's bindings, protocol files, instructions and the
/// compiled protocol and JSON bindings of `sdc-story`.
fn full_tree(name: &str) -> PathBuf {
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
    for e in fs::read_dir(from.join("bindings")).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "yaml") || p.file_name().is_some_and(|n| n == "PENDING.md") {
            fs::copy(&p, root.join("bindings").join(p.file_name().unwrap())).unwrap();
        }
    }
    fs::copy(from.join("driver/examples/sdc/sdc-story.bindings.json"), root.join("driver/examples/sdc/sdc-story.bindings.json")).unwrap();
    fs::copy(from.join("protocol/compiled/sdc-story.ir.json"), root.join("protocol/compiled/sdc-story.ir.json")).unwrap();
    std::os::unix::fs::symlink(from.join("instructions").canonicalize().unwrap(), root.join("instructions")).unwrap();
    std::os::unix::fs::symlink(from.join("bindings/tools").canonicalize().unwrap(), root.join("bindings/tools")).unwrap();
    let text = fs::read_to_string(script()).unwrap();
    let cut = text.split("# The answers file of").next().unwrap();
    fs::write(root.join("scripts/check-bindings.sh"), format!("{cut}exit $fail\n")).unwrap();
    root
}

fn full(root: &Path) -> (Option<i32>, String, String) {
    let o = Command::new("bash").arg(root.join("scripts/check-bindings.sh")).output().unwrap();
    let (out, err) = said(&o);
    (o.status.code(), out, err)
}

fn clean(root: &Path) {
    let (code, out, err) = full(root);
    assert!(code == Some(0) && out.contains("pass registrations"), "the fixture is not a clean tree: {out}\n{err}");
}

/// Valid YAML that yq cannot write as JSON (`.inf`, a key that is a list) ends the check with a line of yq that names no file and
/// no `FAIL` line.
#[test]
#[ignore = "measurement: fails until story check-bindings-refuses-every-file-by-name is built"]
fn yaml_that_yq_cannot_write_as_json_is_refused_by_a_line_that_names_the_file() {
    let forms = [
        ("inf", "  - action: x\n    executor: {kind: program, argv: [x]}\n    exits: {error: [75, 126, 127]}\n    handoff: .inf\n"),
        ("list-key", "  - action: x\n    ? [a, b]\n    : 4\n    executor: {kind: program, argv: [x]}\n    exits: {error: [75, 126, 127]}\n"),
    ];
    for (name, body) in forms {
        let root = root_in(&format!("yq-{name}"));
        fs::write(root.join("bindings/bad.yaml"), format!("format: loom.bindings/1\nactions:\n{body}")).unwrap();
        let o = Command::new("bash").arg(script()).arg("--exits").arg(&root).output().unwrap();
        let (out, err) = said(&o);
        assert_eq!(o.status.code(), Some(1), "{name}: {out}\n{err}");
        assert!(out.contains("FAIL") && out.contains("bindings/bad.yaml"), "{name}: stdout holds no line that names the file: {out:?}\n{err}");
    }
}

/// The JSON copy of the bindings (`driver/examples/sdc/<name>.bindings.json`) is read by the full check; a member of the wrong type
/// is a raw jq error and exit 5.
#[test]
#[ignore = "measurement: fails until story check-bindings-refuses-every-file-by-name is built"]
fn a_json_binding_member_of_the_wrong_type_is_refused_by_a_line_that_names_the_file() {
    for (name, member, value) in [
        ("argv-false", "argv", serde_json::json!(false)),
        ("llm-string", "llm", serde_json::json!("x")),
        ("results-string", "results", serde_json::json!("x")),
        ("copy-number", "copy", serde_json::json!(1)),
    ] {
        let root = full_tree(&format!("json-{name}"));
        clean(&root);
        let path = root.join("driver/examples/sdc/sdc-story.bindings.json");
        let mut doc: serde_json::Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        doc["spec.gate"][member] = value;
        fs::write(&path, doc.to_string()).unwrap();
        let (code, out, err) = full(&root);
        assert_eq!(code, Some(1), "{name}: {out}\n{err}");
        assert!(out.contains("sdc-story.bindings.json") && !err.contains("jq:"), "{name}: {out}\n{err}");
    }
}

/// A protocol file the registrations step reads without `protocol.id` or `protocol.revision` of a text and a number is a raw jq error
/// and exit 5.
#[test]
#[ignore = "measurement: fails until story check-bindings-refuses-every-file-by-name is built"]
fn a_protocol_file_without_an_id_is_refused_by_a_line_that_names_the_file() {
    for (name, text) in [("no-protocol", "a: b\n"), ("id-number", "protocol: {id: 5, revision: 1}\n"), ("no-id", "protocol: {revision: 1}\n")] {
        let root = full_tree(&format!("protocol-{name}"));
        clean(&root);
        fs::write(root.join("protocol/zzz.protocol.yaml"), text).unwrap();
        let (code, out, err) = full(&root);
        assert_eq!(code, Some(1), "{name}: {out}\n{err}");
        assert!(out.contains("protocol/zzz.protocol.yaml") && !err.contains("jq:"), "{name}: {out}\n{err}");
    }
}
