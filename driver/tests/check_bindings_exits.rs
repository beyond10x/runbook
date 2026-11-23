//! The `exits` check of `scripts/check-bindings.sh` (`--exits <root>` runs it alone over
//! `<root>/bindings/*.yaml`). Loom reads two classes in an `exits` member, `error` and `not_called`
//! (`const EXITS` of its bindings decoder), each a list of exit codes: another class, or a class that
//! is not a list, is refused by a named line, not passed and not a raw `jq` error.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/check-bindings.sh")
}

fn action(name: &str, exits: &str) -> String {
    format!("  - action: {name}\n    executor: {{kind: program, argv: [tools/x.sh]}}\n    exits: {exits}\n")
}

/// The check over a root whose one bindings file holds a good action and `bad`.
fn check(name: &str, bad: &str) -> Output {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("check-bindings-exits").join(name);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("bindings")).unwrap();
    let doc = format!("format: loom.bindings/1\nactions:\n{}{bad}", action("good.action", "{error: [75, 126, 127]}"));
    fs::write(root.join("bindings/fixture.yaml"), doc).unwrap();
    Command::new("bash").arg(script()).arg("--exits").arg(&root).output().unwrap()
}

fn said(o: &Output) -> (String, String) {
    (String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

#[test]
fn a_class_that_loom_does_not_read_is_refused_by_name() {
    let o = check("unknown-class", &action("warn.action", "{error: [75, 126, 127], warn: [1]}"));
    let (out, err) = said(&o);
    assert_eq!(o.status.code(), Some(1), "{out}\n{err}");
    assert!(out.contains("FAIL exits"), "{out}\n{err}");
    assert!(out.contains("bindings/fixture.yaml warn.action: exits.warn is not an exit class, Loom reads error and not_called"), "{out}\n{err}");
    assert!(!out.contains("good.action") && !err.contains("jq:"), "{out}\n{err}");
}

#[test]
fn a_class_that_is_not_a_list_is_refused_by_name_and_not_by_a_jq_error() {
    for (member, exits) in [("error", "{error: 75}"), ("not_called", "{error: [75, 126, 127], not_called: 79}")] {
        let o = check(&format!("not-a-list-{member}"), &action("number.action", exits));
        let (out, err) = said(&o);
        assert_eq!(o.status.code(), Some(1), "{member}: {out}\n{err}");
        assert!(out.contains(&format!("bindings/fixture.yaml number.action: exits.{member} is not a list of exit codes")), "{member}: {out}\n{err}");
        assert!(!out.contains("good.action") && !err.contains("jq:"), "{member}: {out}\n{err}");
    }
}

#[test]
fn an_exits_member_that_is_not_a_mapping_is_refused_by_name() {
    let o = check("not-a-mapping", &action("list.action", "[75, 126, 127]"));
    let (out, err) = said(&o);
    assert_eq!(o.status.code(), Some(1), "{out}\n{err}");
    assert!(out.contains("bindings/fixture.yaml list.action: exits is not a mapping of exit classes"), "{out}\n{err}");
    assert!(!out.contains("good.action") && !err.contains("jq:"), "{out}\n{err}");
}

fn root_in(name: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("check-bindings-exits").join(name);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("bindings")).unwrap();
    root
}

fn run_exits(root: &PathBuf) -> Output {
    Command::new("bash").arg(script()).arg("--exits").arg(root).output().unwrap()
}

/// A runbook of another repository: one bindings file, none of this repository's pool bindings.
fn outside_root(name: &str, doc: &str) -> Output {
    let root = root_in(name);
    fs::write(root.join("bindings/example.yaml"), doc).unwrap();
    run_exits(&root)
}

#[test]
fn a_runbook_of_another_repository_is_not_asked_for_the_pool_bindings() {
    let doc = format!(
        "format: loom.bindings/1\nprotocol: example@1\nactions:\n{}{}",
        action("first.action", "{error: [75, 126, 127]}"),
        action("second.action", "{error: [75, 126, 127]}")
    );
    let o = outside_root("other-runbook-passes", &doc);
    let (out, err) = said(&o);
    assert_eq!(o.status.code(), Some(0), "{out}\n{err}");
    assert!(out.contains("pass exits"), "{out}\n{err}");
    assert!(!out.contains("build pool"), "{out}\n{err}");
}

#[test]
fn a_binding_of_another_runbook_without_exits_error_is_the_one_line_named() {
    let doc = format!(
        "format: loom.bindings/1\nprotocol: example@1\nactions:\n{}  - action: bare.action\n    executor: {{kind: program, argv: [tools/x.sh]}}\n",
        action("first.action", "{error: [75, 126, 127]}")
    );
    let o = outside_root("other-runbook-one-line", &doc);
    let (out, err) = said(&o);
    assert_eq!(o.status.code(), Some(1), "{out}\n{err}");
    assert_eq!(out, "FAIL exits\n  bindings/example.yaml bare.action: exits.error is [], want [75,126,127]\n", "{err}");
}

fn waiting_action(name: &str, exits: &str) -> String {
    format!("  - action: {name}\n    executor: {{kind: program, argv: [tools/x.sh, --pool-wait, \"600\"]}}\n    exits: {exits}\n")
}

#[test]
fn a_binding_of_another_runbook_that_waits_for_the_pool_wants_the_five_codes_and_passes_with_them() {
    let doc = format!(
        "format: loom.bindings/1\nprotocol: example@1\nactions:\n{}{}",
        action("plain.action", "{error: [75, 126, 127]}"),
        waiting_action("waits.action", "{error: [69, 70, 75, 126, 127]}")
    );
    let o = outside_root("other-runbook-pool-wait-passes", &doc);
    let (out, err) = said(&o);
    assert_eq!(o.status.code(), Some(0), "{out}\n{err}");
    assert_eq!(out, "pass exits\n", "{err}");
}

#[test]
fn a_binding_of_another_runbook_that_waits_for_the_pool_with_three_codes_gets_one_line_naming_the_list() {
    let doc = format!(
        "format: loom.bindings/1\nprotocol: example@1\nactions:\n{}",
        waiting_action("waits.action", "{error: [75, 126, 127]}")
    );
    let o = outside_root("other-runbook-pool-wait-short", &doc);
    let (out, err) = said(&o);
    assert_eq!(o.status.code(), Some(1), "{out}\n{err}");
    assert_eq!(out, "FAIL exits\n  bindings/example.yaml waits.action: exits.error is [75,126,127], want [69,70,75,126,127]\n", "{err}");
}

/// A copy of this repository's `bindings/*.yaml` as a root: the root of this repository.
fn own_root(name: &str) -> PathBuf {
    let root = root_in(name);
    let from = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bindings");
    for e in fs::read_dir(from).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "yaml") {
            fs::copy(&p, root.join("bindings").join(p.file_name().unwrap())).unwrap();
        }
    }
    root
}

#[test]
fn this_repository_passes_and_a_removed_pool_binding_is_still_reported() {
    let root = own_root("own-root-removed");
    let o = run_exits(&root);
    let (out, err) = said(&o);
    assert_eq!(o.status.code(), Some(0), "{out}\n{err}");
    fs::remove_file(root.join("bindings/sdc-release.yaml")).unwrap();
    let o = run_exits(&root);
    let (out, err) = said(&o);
    assert_eq!(o.status.code(), Some(1), "{out}\n{err}");
    assert!(
        out.contains("bindings/sdc-release.yaml:acceptance.run: a binding that waits for the build pool is not in bindings/"),
        "{out}\n{err}"
    );
}

#[test]
fn a_root_without_bindings_files_is_refused_by_one_line_naming_the_root() {
    for (name, with_dir) in [("no-bindings-dir", false), ("empty-bindings-dir", true)] {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("check-bindings-exits").join(name);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(if with_dir { root.join("bindings") } else { root.clone() }).unwrap();
        let o = run_exits(&root);
        let (out, err) = said(&o);
        let root = root.canonicalize().unwrap();
        assert_eq!(o.status.code(), Some(1), "{name}: {out}\n{err}");
        assert_eq!(out, format!("FAIL exits\n  {}: no bindings/*.yaml\n", root.display()), "{name}: {err}");
        assert!(err.is_empty(), "{name}: {err}");
    }
}

const POOL_FILES: [&str; 3] = ["sdc-story.yaml", "sdc-release.yaml", "sdc-bugfix.yaml"];

#[test]
fn the_three_files_the_pool_list_names_exist_in_this_repository() {
    for f in POOL_FILES {
        assert!(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bindings").join(f).is_file(), "{f}");
    }
}

#[test]
fn this_repository_itself_passes_the_exits_check() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let o = run_exits(&root);
    let (out, err) = said(&o);
    assert_eq!(o.status.code(), Some(0), "{out}\n{err}");
    assert_eq!(out, "pass exits\n", "{err}");
}

/// A tree in which a copy of the script lies in `scripts/`: the repository the script lies in is that tree. Its
/// `bindings/` is this repository's, less the three files the pool list names, which `leave` writes or omits.
fn script_tree(name: &str, leave: impl Fn(&std::path::Path, &str)) -> Output {
    let root = own_root(name);
    for f in POOL_FILES {
        fs::remove_file(root.join("bindings").join(f)).unwrap();
        leave(&root.join("bindings"), f);
    }
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::copy(script(), root.join("scripts/check-bindings.sh")).unwrap();
    Command::new("bash").arg(root.join("scripts/check-bindings.sh")).arg("--exits").arg(&root).output().unwrap()
}

#[test]
fn a_tree_that_holds_a_copy_of_the_script_and_none_of_the_named_files_is_another_runbook() {
    let o = script_tree("script-tree-removed", |_, _| {});
    let (out, err) = said(&o);
    assert_eq!((o.status.code(), out.as_str()), (Some(0), "pass exits\n"), "{err}");
}

#[test]
fn a_tree_that_holds_a_copy_of_the_script_and_the_named_files_emptied_is_another_runbook() {
    let o = script_tree("script-tree-emptied", |dir, f| fs::write(dir.join(f), "").unwrap());
    let (out, err) = said(&o);
    assert_eq!((o.status.code(), out.as_str()), (Some(0), "pass exits\n"), "{err}");
}

/// The full check, cut before its cargo steps (at the registrations), over a tree that has the compiled protocol and
/// the JSON bindings of `sdc-story` and the YAML bindings of this repository less `sdc-story.yaml`.
#[test]
fn the_full_check_fails_when_a_bindings_file_of_a_protocol_is_missing() {
    let root = own_root("full-check-missing");
    fs::remove_file(root.join("bindings/sdc-story.yaml")).unwrap();
    let from = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    for (dir, file) in [("driver/examples/sdc", "sdc-story.bindings.json"), ("protocol/compiled", "sdc-story.ir.json")] {
        fs::create_dir_all(root.join(dir)).unwrap();
        fs::copy(from.join(dir).join(file), root.join(dir).join(file)).unwrap();
    }
    let text = fs::read_to_string(script()).unwrap();
    let cut = text.split("# The registrations of bindings/*.yaml").next().unwrap();
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::write(root.join("scripts/check-bindings.sh"), format!("{cut}exit $fail\n")).unwrap();
    let o = Command::new("bash").arg(root.join("scripts/check-bindings.sh")).output().unwrap();
    let (out, err) = said(&o);
    assert_eq!(o.status.code(), Some(1), "{out}\n{err}");
    assert!(out.contains("FAIL sdc-story\n  no bindings/sdc-story.yaml\n"), "{out}\n{err}");
}

#[test]
fn this_repository_reports_a_pool_binding_without_pool_wait() {
    let root = own_root("own-root-no-pool-wait");
    let file = root.join("bindings/sdc-release.yaml");
    let text = fs::read_to_string(&file).unwrap();
    let cut = text.replace(", --pool-wait, \"1200\"]", "]");
    assert_ne!(text, cut);
    fs::write(&file, cut).unwrap();
    let o = run_exits(&root);
    let (out, err) = said(&o);
    assert_eq!(o.status.code(), Some(1), "{out}\n{err}");
    assert!(
        out.contains("bindings/sdc-release.yaml acceptance.run: a binding that waits for the build pool has no --pool-wait in its argv"),
        "{out}\n{err}"
    );
}
