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
    assert_eq!(out, "FAIL exits\n  bindings/example.yaml bare.action: exits.error is [], want [75,126,127] or [69,70,75,126,127]\n", "{err}");
}

/// Loom 0.61.0: a run killed at its timeout matches only a `timeout` row of a `from: exit` table, never
/// `*` (`story:wildcard-result-row-does-not-match-timeout`). The key `timeout` is a value Loom reads beside
/// the exit codes, so the check takes it as it takes `*`: no shape line, no `jq` error, the exits step passes.
#[test]
fn a_timeout_row_of_a_result_table_read_from_the_exit_passes_as_loom_reads_it() {
    let doc = "format: loom.bindings/1\nprotocol: example@1\nactions:\n  - action: run.action\n    executor: {kind: program, argv: [tools/x.sh]}\n    exits: {error: [75, 126, 127]}\n    evidence:\n      - {kind: k, subject: s, producer: p, from: exit, results: {\"0\": passed, timeout: failed, \"*\": failed}}\n";
    let o = outside_root("timeout-row-passes", doc);
    let (out, err) = said(&o);
    assert_eq!((o.status.code(), out.as_str()), (Some(0), "pass exits\n"), "{err}");
    assert!(!err.contains("jq:"), "{err}");
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

#[test]
fn a_root_with_a_space_or_a_glob_character_in_its_path_is_read() {
    for name in ["with space", "star*", "question?", "brace{a,b}", "tilde~"] {
        let o = outside_root(name, "format: loom.bindings/1\nprotocol: example@1\nactions:\n  - action: a.one\n    executor: {kind: program, argv: [tools/x.sh]}\n    exits: {error: [75, 126, 127]}\n");
        let (out, err) = said(&o);
        assert_eq!((o.status.code(), out.as_str()), (Some(0), "pass exits\n"), "{name}: {err}");
    }
}

fn one_action(name: &str, argv: &str, codes: &str) -> (Option<i32>, String, String) {
    let doc = format!("format: loom.bindings/1\nprotocol: example@1\nactions:\n  - action: a.one\n    executor: {{kind: program, argv: {argv}}}\n    exits: {{error: {codes}}}\n");
    let o = outside_root(name, &doc);
    let (out, err) = said(&o);
    (o.status.code(), out, err)
}

#[test]
fn a_binding_of_another_runbook_without_the_flag_may_list_the_five_codes_for_the_environment_variable() {
    let (code, out, err) = one_action("other-runbook-five-no-flag", "[tools/x.sh]", "[69, 70, 75, 126, 127]");
    assert_eq!((code, out.as_str()), (Some(0), "pass exits\n"), "{err}");
}

#[test]
fn a_binding_of_another_runbook_without_the_flag_and_with_other_codes_gets_the_line_naming_both_lists() {
    let (code, out, err) = one_action("other-runbook-other-codes", "[tools/x.sh]", "[75, 126]");
    assert_eq!(
        (code, out.as_str()),
        (Some(1), "FAIL exits\n  bindings/example.yaml a.one: exits.error is [75,126], want [75,126,127] or [69,70,75,126,127]\n"),
        "{err}"
    );
}

#[test]
fn a_dangling_link_named_yaml_is_refused_by_one_line_naming_it() {
    let root = root_in("dangling-alone");
    fs::write(root.join("bindings/good.yaml"), GOOD).unwrap();
    std::os::unix::fs::symlink("nowhere", root.join("bindings/dangling.yaml")).unwrap();
    let o = run_exits(&root);
    let (out, err) = said(&o);
    assert_eq!((o.status.code(), out.as_str()), (Some(1), "FAIL exits\n  bindings/dangling.yaml: a link that points at nothing\n"), "{err}");
}

/// A link named like one of the three files counts as holding it: the pool rule applies, and nothing provides the
/// five bindings, so the output is the refusal of the link and the five pool lines.
#[test]
fn a_dangling_link_named_like_a_pool_file_is_refused_and_the_pool_rule_applies() {
    let root = root_in("dangling-pool-name");
    fs::write(root.join("bindings/good.yaml"), GOOD).unwrap();
    std::os::unix::fs::symlink("nowhere", root.join("bindings/sdc-story.yaml")).unwrap();
    let o = run_exits(&root);
    let (out, err) = said(&o);
    let pool = "a binding that waits for the build pool is not in bindings/";
    let want = format!(
        "FAIL exits\n  bindings/sdc-story.yaml: a link that points at nothing\n  bindings/sdc-story.yaml:tests.run: {pool}\n  bindings/sdc-release.yaml:acceptance.run: {pool}\n  bindings/sdc-bugfix.yaml:repro.run: {pool}\n  bindings/sdc-bugfix.yaml:tests.run: {pool}\n  bindings/sdc-bugfix.yaml:gate.run: {pool}\n"
    );
    assert_eq!((o.status.code(), out.as_str()), (Some(1), want.as_str()), "{err}");
}

/// A tree for the full check, cut before its cargo steps (at the answers file): this repository's bindings, protocol
/// files and instructions, and the compiled protocol and JSON bindings of `sdc-story`.
fn full_check_tree(name: &str) -> PathBuf {
    let root = own_root(name);
    let from = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    fs::create_dir_all(root.join("protocol")).unwrap();
    for e in fs::read_dir(from.join("protocol")).unwrap() {
        let p = e.unwrap().path();
        if p.to_string_lossy().ends_with(".protocol.yaml") {
            fs::copy(&p, root.join("protocol").join(p.file_name().unwrap())).unwrap();
        }
    }
    fs::copy(from.join("bindings/PENDING.md"), root.join("bindings/PENDING.md")).unwrap();
    for (dir, file) in [("driver/examples/sdc", "sdc-story.bindings.json"), ("protocol/compiled", "sdc-story.ir.json")] {
        fs::create_dir_all(root.join(dir)).unwrap();
        fs::copy(from.join(dir).join(file), root.join(dir).join(file)).unwrap();
    }
    std::os::unix::fs::symlink(from.join("instructions").canonicalize().unwrap(), root.join("instructions")).unwrap();
    std::os::unix::fs::symlink(from.join("bindings/tools").canonicalize().unwrap(), root.join("bindings/tools")).unwrap();
    let text = fs::read_to_string(script()).unwrap();
    let cut = text.split("# The answers file of").next().unwrap();
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::write(root.join("scripts/check-bindings.sh"), format!("{cut}exit $fail\n")).unwrap();
    root
}

fn full_check(root: &std::path::Path) -> (Option<i32>, String, String) {
    let o = Command::new("bash").arg(root.join("scripts/check-bindings.sh")).output().unwrap();
    (o.status.code(), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The registrations step reads `bindings/*.yaml` through yq, as the exits step does and before it. To show the
/// registrations step alone, the call of the exits step is cut out of the copy of the script (the line `check_exits`
/// at the start of a line). A clean tree is the control; then a file in `bindings/` that is no YAML must fail the
/// check: its reader was a process substitution, whose failure was lost, so the step stayed green with yq's message
/// printed.
#[test]
fn the_registrations_step_alone_fails_the_full_check_for_a_bindings_file_it_cannot_read() {
    let root = full_check_tree("full-registrations");
    let script_copy = root.join("scripts/check-bindings.sh");
    let text = fs::read_to_string(&script_copy).unwrap();
    let no_exits = text.replacen("\ncheck_exits\n", "\n", 1);
    assert_ne!(text, no_exits);
    fs::write(&script_copy, no_exits).unwrap();
    let (code, out, err) = full_check(&root);
    assert!(out.contains("pass registrations") && !out.contains("exits") && code == Some(0), "the fixture is not a clean tree: {out}\n{err}");
    fs::write(root.join("bindings/extra.yaml"), "this: is: [not\n  yaml {{{\n").unwrap();
    let (code, out, err) = full_check(&root);
    assert_eq!(code, Some(1), "{out}\n{err}");
    assert!(out.contains("FAIL registrations") && !out.contains("pass registrations"), "{out}\n{err}");
    assert!(err.contains("bindings/extra.yaml"), "{out}\n{err}");
}

#[test]
fn an_executor_that_is_present_and_null_is_refused_by_one_line() {
    let root = root_in("executor-null-line");
    fs::write(root.join("bindings/bad.yaml"), "format: loom.bindings/1\nactions:\n  - action: b.two\n    executor:\n").unwrap();
    let o = run_exits(&root);
    let (out, err) = said(&o);
    assert_eq!((o.status.code(), out.as_str()), (Some(1), "FAIL exits\n  bindings/bad.yaml b.two: executor is not a mapping\n"), "{err}");
}

#[test]
fn evidence_of_the_wrong_type_is_refused_by_one_line_naming_the_file_and_the_action() {
    let shapes = [
        ("map", "{a: b}", "evidence is not a list"),
        ("number", "[1]", "an evidence entry is not a mapping"),
        ("null-entry", "[null]", "an evidence entry is not a mapping"),
        ("results-string", "[{kind: k, from: exit, results: nope}]", "results of an evidence entry is not a mapping"),
        ("results-key", "[{kind: k, from: exit, results: {ok: pass}}]", "a result key of an evidence entry read from the exit is no exit code"),
    ];
    for (name, ev, line) in shapes {
        let root = root_in(&format!("evidence-{name}"));
        fs::write(root.join("bindings/good.yaml"), GOOD).unwrap();
        let doc = format!("format: loom.bindings/1\nactions:\n  - action: b.two\n    executor: {{kind: program, argv: [x]}}\n    exits: {{error: [75, 126, 127]}}\n    evidence: {ev}\n");
        fs::write(root.join("bindings/bad.yaml"), doc).unwrap();
        let o = run_exits(&root);
        let (out, err) = said(&o);
        assert_eq!((o.status.code(), out.as_str()), (Some(1), format!("FAIL exits\n  bindings/bad.yaml b.two: {line}\n").as_str()), "{name}: {err}");
    }
}

/// The full check reads the evidence of the bindings file of a protocol in its per-protocol loop, before the exits
/// step: the shape is refused there by one line under `FAIL <name>`, and the other steps go on.
#[test]
fn the_full_check_refuses_evidence_of_the_wrong_type_by_one_line_under_the_protocol() {
    let root = full_check_tree("full-evidence");
    let file = root.join("bindings/sdc-story.yaml");
    let text = fs::read_to_string(&file).unwrap();
    let bad = text.replacen("    evidence:\n      - kind:", "    evidence: nope\n    extra:\n      - kind:", 1);
    assert_ne!(text, bad);
    fs::write(&file, bad).unwrap();
    let (code, out, err) = full_check(&root);
    assert_eq!(code, Some(1), "{out}\n{err}");
    assert!(out.contains("FAIL sdc-story\n  bindings/sdc-story.yaml ") && out.contains(": evidence is not a list\n"), "{out}\n{err}");
    assert!(!err.contains("jq:"), "{out}\n{err}");
}

const GOOD: &str = "format: loom.bindings/1\nprotocol: example@1\nactions:\n  - action: a.one\n    executor: {kind: program, argv: [tools/x.sh]}\n    exits: {error: [75, 126, 127]}\n";

#[test]
fn an_empty_file_is_refused_by_one_line_naming_it_alone_and_beside_others() {
    for (name, beside) in [("empty-alone", false), ("empty-beside", true)] {
        let root = root_in(name);
        fs::write(root.join("bindings/empty.yaml"), "").unwrap();
        if beside {
            fs::write(root.join("bindings/good.yaml"), GOOD).unwrap();
        }
        let o = run_exits(&root);
        let (out, err) = said(&o);
        assert_eq!((o.status.code(), out.as_str()), (Some(1), "FAIL exits\n  bindings/empty.yaml: the file is empty\n"), "{name}: {err}");
    }
}

#[test]
fn an_entry_or_a_member_of_the_wrong_type_is_refused_by_one_line_naming_the_file_and_the_action() {
    let shapes = [
        ("number", "  - 1\n", "bindings/bad.yaml: an entry of actions is not a mapping"),
        ("null", "  - null\n", "bindings/bad.yaml: an entry of actions is not a mapping"),
        ("no-name", "  - {executor: {kind: program}}\n", "bindings/bad.yaml: an entry of actions has no action name"),
        ("executor-string", "  - action: b.two\n    executor: nope\n", "bindings/bad.yaml b.two: executor is not a mapping"),
        ("argv-string", "  - action: b.two\n    executor: {kind: program, argv: nope}\n", "bindings/bad.yaml b.two: executor.argv is not a list"),
    ];
    for (name, bad, line) in shapes {
        let root = root_in(&format!("shape-{name}"));
        fs::write(root.join("bindings/good.yaml"), GOOD).unwrap();
        fs::write(root.join("bindings/bad.yaml"), format!("format: loom.bindings/1\nactions:\n{bad}")).unwrap();
        let o = run_exits(&root);
        let (out, err) = said(&o);
        assert_eq!((o.status.code(), out.as_str()), (Some(1), format!("FAIL exits\n  {line}\n").as_str()), "{name}: {err}");
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
fn a_tree_that_holds_a_copy_of_the_script_and_the_named_files_emptied_is_another_runbook_with_three_empty_files() {
    let o = script_tree("script-tree-emptied", |dir, f| fs::write(dir.join(f), "").unwrap());
    let (out, err) = said(&o);
    let want: String = ["sdc-bugfix.yaml", "sdc-release.yaml", "sdc-story.yaml"].iter().map(|f| format!("  bindings/{f}: the file is empty\n")).collect();
    assert_eq!((o.status.code(), out.as_str()), (Some(1), format!("FAIL exits\n{want}").as_str()), "{err}");
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

/// The route entries of a runbook, `bindings/<id>.routes.yaml` (`runbook.routes/1`, `docs/daemon-api.md`, "The
/// catalog"): Runbook's own file beside the bindings, not a Loom bindings file.
const ROUTES: &str = "format: runbook.routes/1\nmodel_routes:\n  default: {route: fast}\n  review: {needs: {wire: messages}}\nroutes_allowed: [fast]\n";

fn run_step(step: &str, root: &PathBuf) -> (Option<i32>, String, String) {
    let o = Command::new("bash").arg(script()).arg(step).arg(root).output().unwrap();
    (o.status.code(), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// A runbook of another repository with its route entries beside its bindings: the exits step and the budget step pass.
#[test]
fn a_route_file_beside_the_bindings_is_left_out_and_the_exits_and_budget_steps_pass() {
    let root = root_in("route-file-passes");
    fs::write(root.join("bindings/example.yaml"), GOOD).unwrap();
    fs::write(root.join("bindings/example.routes.yaml"), ROUTES).unwrap();
    for (step, pass) in [("--exits", "pass exits\n"), ("--budget", "pass budget\n")] {
        let (code, out, err) = run_step(step, &root);
        assert_eq!((code, out.as_str()), (Some(0), pass), "{step}: {err}");
        assert!(!err.contains("routes"), "{step}: {err}");
    }
}

/// A file named as the route entries of a runbook that the daemon would not read as them is refused by the exits step,
/// one line naming it; the bindings file beside it is checked as before.
#[test]
fn a_file_named_as_route_entries_that_is_not_one_is_refused_by_one_line_naming_it() {
    let lead = "bindings/example.routes.yaml: a file of this name holds the route entries of runbook example";
    let cases: [(&str, &str, String); 5] = [
        ("route-file-other-format", "format: loom.bindings/1\nactions: []\n", format!("{lead}, and its format is not runbook.routes/1")),
        ("route-file-no-format", "model_routes:\n  default: {route: fast}\n", format!("{lead}, and its format is not runbook.routes/1")),
        ("route-file-empty", "", format!("{lead}, and its format is not runbook.routes/1")),
        ("route-file-list", "- format: runbook.routes/1\n", format!("{lead}, and its format is not runbook.routes/1")),
        ("route-file-no-yaml", "format: [runbook\n  routes {{\n", format!("{lead}, and it is not YAML")),
    ];
    for (name, text, line) in cases {
        let root = root_in(name);
        fs::write(root.join("bindings/example.yaml"), GOOD).unwrap();
        fs::write(root.join("bindings/example.routes.yaml"), text).unwrap();
        let (code, out, err) = run_step("--exits", &root);
        assert_eq!((code, out.as_str()), (Some(1), format!("FAIL exits\n  {line}\n").as_str()), "{name}: {err}");
    }
    for (name, target) in [("route-file-link", "real.txt"), ("route-file-dangling", "nowhere")] {
        let root = root_in(name);
        fs::write(root.join("bindings/example.yaml"), GOOD).unwrap();
        fs::write(root.join("bindings/real.txt"), ROUTES).unwrap();
        std::os::unix::fs::symlink(target, root.join("bindings/example.routes.yaml")).unwrap();
        let (code, out, err) = run_step("--exits", &root);
        let want = format!("FAIL exits\n  {lead}, and it is a link, which the daemon does not follow\n");
        assert_eq!((code, out.as_str()), (Some(1), want.as_str()), "{name}: {err}");
    }
}

/// The full check of this repository's tree with a route file beside a protocol's bindings: the registrations step
/// reads no protocol for it, and every step passes.
#[test]
fn the_full_check_leaves_a_route_file_out_of_the_registrations() {
    let root = full_check_tree("full-route-file");
    fs::write(root.join("bindings/sdc-story.routes.yaml"), ROUTES).unwrap();
    let (code, out, err) = full_check(&root);
    assert_eq!(code, Some(0), "{out}\n{err}");
    for line in ["pass exits", "pass budget", "pass registrations"] {
        assert!(out.contains(line), "{line}: {out}\n{err}");
    }
    assert!(!out.contains("routes.yaml"), "{out}\n{err}");
}

/// Adversary: two files of the route-entries name that the daemon does not read as route entries
/// (`runbook::admission::read` refuses a file over `FILE_LIMIT`, 64 KiB; `admission::parse` takes the text with
/// `serde_yaml::from_str`, which refuses a stream of two documents, "not YAML"). Each has `format: runbook.routes/1`
/// as its first document, and the exits step passes it: its yq reads every document and its jq keeps the first.
#[test]
fn adversary_a_route_file_the_daemon_does_not_read_as_two_documents_or_over_64_kib_is_refused() {
    let lead = "bindings/example.routes.yaml: a file of this name holds the route entries of runbook example, and";
    let big = format!("{ROUTES}{}", "# padding of a comment that the daemon still counts against its 64 KiB read\n".repeat(1000));
    assert!(big.len() > 64 * 1024);
    for (name, text) in [("adv-route-two-documents", format!("{ROUTES}---\n")), ("adv-route-over-64-kib", big)] {
        let root = root_in(name);
        fs::write(root.join("bindings/example.yaml"), GOOD).unwrap();
        fs::write(root.join("bindings/example.routes.yaml"), text).unwrap();
        let (code, out, err) = run_step("--exits", &root);
        assert!(code == Some(1) && out.starts_with("FAIL exits\n") && out.contains(lead), "{name}: exit {code:?}\n{out}{err}");
    }
}

/// Adversary: a route file of 418 bytes whose aliases expand to about 47 MB of JSON (six levels of ten). The daemon
/// reads route files through `serde_yaml`, whose repetition limit refuses alias expansion past 100 jumps per event; the
/// exits step's `route_refusal` runs `yq -o=json '.'`, which expands every alias, so one such file holds the exits step
/// for about 16 s on this host (seven levels: about 160 s and 470 MB; eight: past every budget). The budget step
/// (`refused_document`) counts aliases unexpanded for the same reason. The bound here is 5 s.
#[test]
fn adversary_an_alias_expansion_route_file_does_not_hold_the_exits_step() {
    let mut text = String::from("format: runbook.routes/1\na0: &a0 [x, x, x, x, x, x, x, x, x, x]\n");
    for i in 1..=6 {
        let p = format!("*a{}", i - 1);
        text.push_str(&format!("a{i}: &a{i} [{}]\n", [p.as_str(); 10].join(", ")));
    }
    let root = root_in("adv-route-alias-expansion");
    fs::write(root.join("bindings/example.yaml"), GOOD).unwrap();
    fs::write(root.join("bindings/example.routes.yaml"), &text).unwrap();
    let start = std::time::Instant::now();
    let (code, out, err) = run_step("--exits", &root);
    let took = start.elapsed();
    assert!(took < std::time::Duration::from_secs(5), "the exits step took {took:?} on a {} byte route file: exit {code:?}\n{out}{err}", text.len());
}

/// A directory of the route-entries name: the daemon refuses it as no regular file (`admission::read`), and the exits
/// step says the same.
#[test]
fn a_directory_named_as_route_entries_is_refused_as_no_regular_file() {
    let root = root_in("route-file-directory");
    fs::write(root.join("bindings/example.yaml"), GOOD).unwrap();
    fs::create_dir(root.join("bindings/example.routes.yaml")).unwrap();
    let (code, out, err) = run_step("--exits", &root);
    let want = "FAIL exits\n  bindings/example.routes.yaml: a file of this name holds the route entries of runbook example, and it is not a regular file\n";
    assert_eq!((code, out.as_str()), (Some(1), want), "{err}");
}

/// A root whose `bindings/` holds route entries and no bindings file: one line, naming `bindings/` relative to the
/// root, says that no bindings file is there besides the route entries.
#[test]
fn a_root_with_route_entries_only_is_refused_by_one_line_naming_bindings_relatively() {
    let root = root_in("route-file-only");
    fs::write(root.join("bindings/example.routes.yaml"), ROUTES).unwrap();
    let (code, out, err) = run_step("--exits", &root);
    let want = "FAIL exits\n  bindings/: no bindings file besides the route entries (<id>.routes.yaml)\n";
    assert_eq!((code, out.as_str()), (Some(1), want), "{err}");
}
