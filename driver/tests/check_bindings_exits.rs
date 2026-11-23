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
