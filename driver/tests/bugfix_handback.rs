//! A bug fix hands back only with a spec reference (`story:a-bug-fix-hands-back-only-with-a-spec-reference`,
//! docs/bugfix.md, "Hand-back: the spec reference").
//!
//! After the gate, `handback.describe` (`sdc-bugfix handback-describe`) writes
//! `handback/mr-description.md`, and `handback.spec-check` runs a check program as
//! `<check> <description file> <kind>` through `sdc-bugfix handback-check -- <check argv>`: one line
//! `spec-first: allowed: ...` with exit 0 or `spec-first: refused: ...` with exit 77; 75, 126 and 127
//! stay program errors. The default check is `sdc-bugfix spec-check`. The protocol's outcome `fixed`
//! requires the claim `spec.referenced`, so a refused reference never reaches hand-back.
//!
//! Stand-in checks are POSIX sh scripts the tests write; no network, no model call.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn yaml(rel: &str) -> serde_yaml::Value {
    serde_yaml::from_str(&fs::read_to_string(root().join(rel)).unwrap()).unwrap()
}

/// A fresh case work directory.
fn work(name: &str) -> PathBuf {
    let w = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bugfix-handback").join(name);
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(&w).unwrap();
    w
}

/// `sdc-bugfix <args>` in `work`: (exit code, stdout, stderr).
fn bugfix(work: &Path, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_sdc-bugfix"));
    c.args(args).current_dir(work);
    for (k, _) in std::env::vars().filter(|(k, _)| k.starts_with("SDC_")) {
        c.env_remove(k);
    }
    let o = c.output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn write_exe(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

// ---- the protocol -------------------------------------------------------------------------------

/// The claims an expression requires true at its top level (`claim:` or every member of `all:`).
fn required_true(v: &serde_yaml::Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(c) = v.get("claim").and_then(|c| c.as_str()) {
        if v.get("is").is_none() {
            out.push(c.to_string());
        }
    }
    for m in v.get("all").and_then(|a| a.as_sequence()).into_iter().flatten() {
        out.extend(required_true(m));
    }
    out
}

/// `fixed` is the hand-back point and needs the reference. No outcome reads a refused reference:
/// Loom ends a case at its first legitimate outcome, so such an outcome would end the case before
/// the refusal sends the description writer back.
#[test]
fn the_outcome_fixed_requires_spec_referenced_beside_the_gate_and_no_outcome_ends_on_a_refusal() {
    let p = yaml("protocol/sdc-bugfix.protocol.yaml");
    let fixed = required_true(&p["outcomes"]["fixed"]["requires"]);
    assert!(fixed.contains(&"gate.passed".to_string()), "{fixed:?}");
    assert!(fixed.contains(&"spec.referenced".to_string()), "fixed is reached without a spec reference: {fixed:?}");
    for (name, o) in p["outcomes"].as_mapping().unwrap() {
        let text = serde_yaml::to_string(&o["requires"]).unwrap();
        if name.as_str() != Some("fixed") {
            assert!(!text.contains("spec.referenced") && !text.contains("spec.checked"), "{name:?} reads the spec reference: {text}");
        }
    }
}

#[test]
fn spec_referenced_holds_only_on_an_ok_spec_reference_about_the_description() {
    let p = yaml("protocol/sdc-bugfix.protocol.yaml");
    let ev = &p["claims"]["spec.referenced"]["true_when"]["evidence"];
    assert_eq!(ev["kind"].as_str(), Some("spec_reference"), "{ev:?}");
    assert_eq!(ev["result"].as_str(), Some("ok"), "{ev:?}");
    assert_eq!(ev["subject"].as_str(), Some("description"), "{ev:?}");
    assert!(p["evidence_kinds"]["spec_reference"].is_mapping());
    assert!(p["artifacts"]["description"].is_mapping());
}

/// Every action that hands back is gated on `spec.referenced`; the two steps that produce it are the
/// only `handback.*` actions that are not. The action that opens the code merge request is later work:
/// it cannot land without the claim.
#[test]
fn every_hand_back_action_requires_spec_referenced() {
    let p = yaml("protocol/sdc-bugfix.protocol.yaml");
    let actions = p["actions"].as_mapping().unwrap();
    let names: Vec<&str> = actions.keys().filter_map(|k| k.as_str()).collect();
    for own in ["handback.describe", "handback.spec-check"] {
        assert!(names.contains(&own), "{own} is not an action: {names:?}");
    }
    let mut gated = 0;
    for (k, a) in actions {
        let name = k.as_str().unwrap();
        if !name.starts_with("handback.") || name == "handback.describe" || name == "handback.spec-check" {
            continue;
        }
        gated += 1;
        assert!(required_true(&a["precondition"]).contains(&"spec.referenced".to_string()), "{name} hands back without spec.referenced");
    }
    // the writer runs only after the gate, the check only on a written description
    assert!(required_true(&actions["handback.describe"]["precondition"]).contains(&"gate.passed".to_string()));
    assert!(required_true(&actions["handback.spec-check"]["precondition"]).contains(&"handback.described".to_string()));
    eprintln!("{gated} later hand-back action(s) checked");
}

// ---- the bindings -------------------------------------------------------------------------------

fn binding<'a>(b: &'a serde_yaml::Value, action: &str) -> &'a serde_yaml::Value {
    b["actions"].as_sequence().unwrap().iter().find(|a| a["action"].as_str() == Some(action)).unwrap_or_else(|| panic!("{action} is not bound"))
}

#[test]
fn the_check_binding_maps_0_to_ok_and_77_to_refused_and_keeps_75_126_127_program_errors() {
    let b = yaml("bindings/sdc-bugfix.yaml");
    assert_eq!(b["artifacts"]["description"].as_str(), Some("handback/mr-description.md"));
    let c = binding(&b, "handback.spec-check");
    let argv: Vec<&str> = c["executor"]["argv"].as_sequence().unwrap().iter().map(|a| a.as_str().unwrap()).collect();
    let sep = argv.iter().position(|a| *a == "--").expect("no `--` before the check's argv");
    assert!(argv[..sep].contains(&"handback-check"), "{argv:?}");
    assert_eq!(&argv[sep + 1..sep + 3], &["sdc-bugfix", "spec-check"], "the default check: {argv:?}");
    let ev = &c["evidence"][0];
    assert_eq!((ev["kind"].as_str(), ev["subject"].as_str(), ev["from"].as_str()), (Some("spec_reference"), Some("description"), Some("exit")));
    let table = ev["results"].as_mapping().unwrap();
    assert_eq!(table.len(), 2, "no `*` row: an exit outside the interface is no result: {table:?}");
    assert_eq!(table.get("0").and_then(|v| v.as_str()), Some("ok"));
    assert_eq!(table.get("77").and_then(|v| v.as_str()), Some("refused"));
    let error: Vec<i64> = c["exits"]["error"].as_sequence().unwrap().iter().map(|v| v.as_i64().unwrap()).collect();
    assert_eq!(error, [75, 126, 127]);
}

#[test]
fn a_refusal_sends_the_description_writer_back_and_its_runs_are_bounded() {
    let b = yaml("bindings/sdc-bugfix.yaml");
    let d = binding(&b, "handback.describe");
    assert_eq!(d["retry_when"]["spec_reference"][0].as_str(), Some("refused"), "{d:?}");
    assert_eq!(d["budget"]["max_runs"].as_i64(), Some(2));
    assert_eq!(d["evidence"][0]["kind"].as_str(), Some("description_written"));
    assert_eq!(d["evidence"][0]["subject"].as_str(), Some("description"));
    let argv: Vec<&str> = d["executor"]["argv"].as_sequence().unwrap().iter().map(|a| a.as_str().unwrap()).collect();
    assert!(argv.contains(&"handback-describe") && !argv.contains(&"sdc-node"), "a program, no model call: {argv:?}");
}

// ---- the default check: sdc-bugfix spec-check ---------------------------------------------------

/// `sdc-bugfix spec-check --spec-dir spec <description> <kind>` on a description of `text`.
fn check(name: &str, text: &str, kind: &str) -> (i32, String) {
    let w = work(&format!("check-{name}"));
    fs::write(w.join("d.md"), text).unwrap();
    let (code, out, err) = bugfix(&w, &["spec-check", "--spec-dir", "spec", "d.md", kind]);
    assert_eq!(out.lines().count(), 1, "one line on stdout: {out:?} {err}");
    (code, out)
}

const SUMMARY: &str = "# Pagination drops the last page\n\n- Where: `src/lib.rs:9`\n- Changed files: `src/lib.rs`\n\n";

#[test]
fn change_passes_with_a_merge_request_of_a_spec_repository() {
    for link in ["https://forge.example.invalid/group/product-spec/-/merge_requests/12", "https://forge.example.invalid/org/specs/pull/7"] {
        let (code, out) = check("change-mr", &format!("{SUMMARY}The contract: {link}.\n"), "change");
        assert_eq!(code, 0, "{out}");
        assert!(out.starts_with("spec-first: allowed: ") && out.contains(link), "{out}");
    }
}

#[test]
fn change_passes_with_a_path_under_the_named_spec_directory() {
    let (code, out) = check("change-path", &format!("{SUMMARY}Specified in spec/pager/pages.md.\n"), "change");
    assert_eq!(code, 0, "{out}");
    assert!(out.starts_with("spec-first: allowed: ") && out.contains("spec/pager/pages.md"), "{out}");
}

#[test]
fn change_is_refused_with_a_code_line_a_code_merge_request_or_spec_says_nothing() {
    for (name, extra) in [
        ("code-line", "See src/lib.rs:9.\n"),
        ("code-mr", "See https://forge.example.invalid/group/product/-/merge_requests/3.\n"),
        ("says-nothing", "The spec says nothing about this.\n"),
        ("outside", "See spec-old/pages.md and ../spec/pages.md.\n"),
    ] {
        let (code, out) = check(&format!("change-{name}"), &format!("{SUMMARY}{extra}"), "change");
        assert_eq!(code, 77, "{name}: {out}");
        assert!(out.starts_with("spec-first: refused: "), "{name}: {out}");
    }
}

#[test]
fn defect_passes_with_a_spec_line_a_line_anchor_or_spec_says_nothing() {
    for (name, extra, quoted) in [
        ("line", "The spec: spec/pager/pages.md:14.\n", "spec/pager/pages.md:14"),
        ("anchor", "The spec: https://forge.example.invalid/org/product-spec/blob/main/pages.md#L14\n", "#L14"),
        ("says-nothing", "The spec says\nnothing about a partial last page.\n", "spec says nothing"),
    ] {
        let (code, out) = check(&format!("defect-{name}"), &format!("{SUMMARY}{extra}"), "defect");
        assert_eq!(code, 0, "{name}: {out}");
        assert!(out.starts_with("spec-first: allowed: ") && out.contains(quoted), "{name}: {out}");
    }
}

#[test]
fn defect_is_refused_without_a_spec_line() {
    for (name, extra) in [
        ("none", "No word on the spec.\n"),
        ("code-line", "See src/lib.rs:9.\n"),
        ("spec-file-no-line", "See spec/pager/pages.md.\n"),
        ("anchor-outside-spec", "See https://forge.example.invalid/org/product/blob/main/src/lib.rs#L9\n"),
    ] {
        let (code, out) = check(&format!("defect-{name}"), &format!("{SUMMARY}{extra}"), "defect");
        assert_eq!(code, 77, "{name}: {out}");
        assert!(out.starts_with("spec-first: refused: "), "{name}: {out}");
    }
}

#[test]
fn a_tool_error_of_the_check_is_77_with_a_reason() {
    let w = work("check-tool-error");
    fs::write(w.join("d.md"), "The spec says nothing.\n").unwrap();
    for (args, why) in [
        (vec!["spec-check", "missing.md", "defect"], "missing.md"),
        (vec!["spec-check", "d.md", "feature"], "kind"),
        (vec!["spec-check", "d.md"], "usage"),
        (vec!["spec-check", "--spec-dir", "/abs", "d.md", "change"], "--spec-dir"),
    ] {
        let (code, out, _) = bugfix(&w, &args);
        assert_eq!(code, 77, "{args:?}: {out}");
        assert_eq!(out.lines().count(), 1, "{args:?}: {out}");
        assert!(out.starts_with("spec-first: refused: ") && out.contains(why), "{args:?}: {out}");
    }
}

// ---- the wrapper: sdc-bugfix handback-check -- <check argv> -------------------------------------

/// A case work directory with a description, `bug.yaml` (with `change_kind` when given) and a
/// stand-in check `check.sh` that records its arguments and prints `line`, then exits `exit`.
fn wrapped(name: &str, change_kind: Option<&str>, line: &str, exit: i32) -> (PathBuf, i32, String, String) {
    let w = work(&format!("wrap-{name}"));
    fs::create_dir_all(w.join("handback")).unwrap();
    fs::write(w.join("handback/mr-description.md"), "a description\n").unwrap();
    let mut bug = "title: t\ntext: x\nstory: story:bug\n".to_string();
    if let Some(k) = change_kind {
        bug.push_str(&format!("change_kind: {k}\n"));
    }
    fs::write(w.join("bug.yaml"), bug).unwrap();
    write_exe(&w.join("check.sh"), &format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > args.txt\nprintf '%s\\n' '{line}'\nexit {exit}\n"));
    let (code, out, err) = bugfix(&w, &["handback-check", "--", "./check.sh", "--rule", "x"]);
    (w, code, out, err)
}

#[test]
fn the_wrapper_appends_the_description_and_the_kind_and_passes_0_and_77() {
    let (w, code, out, err) = wrapped("allowed", Some("change"), "spec-first: allowed: change: fine", 0);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(out, "spec-first: allowed: change: fine\n");
    assert_eq!(fs::read_to_string(w.join("args.txt")).unwrap(), "--rule\nx\nhandback/mr-description.md\nchange\n");
    let (_, code, out, err) = wrapped("refused", None, "spec-first: refused: defect: no spec line", 77);
    assert_eq!(code, 77, "{out}{err}");
    assert_eq!(out, "spec-first: refused: defect: no spec line\n");
}

#[test]
fn the_kind_is_bug_yaml_change_kind_and_defect_by_default() {
    let (w, code, ..) = wrapped("default-kind", None, "spec-first: allowed: defect: ok", 0);
    assert_eq!(code, 0);
    assert!(fs::read_to_string(w.join("args.txt")).unwrap().ends_with("handback/mr-description.md\ndefect\n"));
    let (w, code, out, err) = wrapped("bad-kind", Some("feature"), "spec-first: allowed: x", 0);
    assert_eq!(code, 126, "{out}{err}");
    assert!(err.contains("change_kind"), "{err}");
    assert!(!w.join("args.txt").exists(), "the check ran with no kind");
}

#[test]
fn exits_75_126_127_of_the_check_stay_program_errors() {
    for exit in [75, 126, 127] {
        let (_, code, out, err) = wrapped(&format!("error-{exit}"), None, "spec-first: refused: broke", exit);
        assert_eq!(code, exit, "{out}{err}");
        assert!(err.contains("program error"), "{exit}: {err}");
    }
    // a check that is not there is 127, one that cannot be run 126
    let w = work("wrap-missing");
    fs::write(w.join("bug.yaml"), "text: x\n").unwrap();
    fs::create_dir_all(w.join("handback")).unwrap();
    fs::write(w.join("handback/mr-description.md"), "d\n").unwrap();
    let (code, _, err) = bugfix(&w, &["handback-check", "--", "no-such-check-on-path-8841"]);
    assert_eq!(code, 127, "{err}");
    fs::write(w.join("plain.sh"), "#!/bin/sh\n").unwrap();
    let (code, _, err) = bugfix(&w, &["handback-check", "--", "./plain.sh"]);
    assert_eq!(code, 126, "{err}");
}

#[test]
fn an_exit_or_a_line_outside_the_interface_is_a_program_error() {
    for (name, line, exit) in [
        ("exit-1", "spec-first: refused: one", 1),
        ("exit-3", "spec-first: allowed: three", 3),
        ("ok-with-refused-line", "spec-first: refused: but exit 0", 0),
        ("refused-with-allowed-line", "spec-first: allowed: but exit 77", 77),
        ("no-prefix", "fine", 0),
    ] {
        let (_, code, out, err) = wrapped(name, None, line, exit);
        assert_eq!(code, 126, "{name}: {out}{err}");
        assert!(err.contains("interface"), "{name}: {err}");
    }
}

// ---- the writer: sdc-bugfix handback-describe ---------------------------------------------------

#[test]
fn the_writer_puts_the_fix_summary_and_the_ticket_text_into_the_description_and_prints_no_ticket_text() {
    let w = work("describe");
    let fx = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/sdc/fixtures/bugfix-pager");
    for (d, from) in [("mainline", fx.join("crate")), ("impl", fx.join("stub-fix"))] {
        copy_dir(&from, &w.join(d));
    }
    copy_dir(&fx.join("stub-repro"), &w.join("repro"));
    fs::copy(fx.join("stub-dissection.json"), w.join("bug-dissection.json")).unwrap();
    let marker = "DESCRIBE-TICKET-MARKER-3317";
    fs::write(w.join("bug.yaml"), format!("title: Pagination drops the last page\ntext: An invented report, {marker}. The spec says nothing.\nstory: story:bug\n")).unwrap();
    let (code, out, err) = bugfix(&w, &["handback-describe"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!out.contains(marker) && !err.contains(marker), "the ticket's text is on a stream: {out}{err}");
    let d = fs::read_to_string(w.join("handback/mr-description.md")).unwrap();
    let repro: serde_json::Value = serde_json::from_str(&fs::read_to_string(fx.join("stub-repro/repro.json")).unwrap()).unwrap();
    for want in ["# Pagination drops the last page", marker, "The spec says nothing.", repro["test"].as_str().unwrap(), "src/lib.rs"] {
        assert!(d.contains(want), "no `{want}` in the description:\n{d}");
    }
    // the same inputs write the same bytes
    let (code, ..) = bugfix(&w, &["handback-describe"]);
    assert_eq!(code, 0);
    assert_eq!(fs::read_to_string(w.join("handback/mr-description.md")).unwrap(), d);
    // a missing input is a program error, not a description
    fs::remove_file(w.join("bug.yaml")).unwrap();
    let (code, _, err) = bugfix(&w, &["handback-describe"]);
    assert_eq!(code, 126, "{err}");
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dst = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &dst);
        } else {
            fs::copy(e.path(), dst).unwrap();
        }
    }
}
