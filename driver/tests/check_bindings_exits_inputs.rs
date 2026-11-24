//! Cases for `scripts/check-bindings.sh --exits <root>`: a script lying in the root, the
//! per-binding `--pool-wait` rule and the refusals by name. The script is the one of the tree
//! (`../scripts/check-bindings.sh` at run time of the test binary).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const POOL_FILES: [&str; 3] = ["sdc-story.yaml", "sdc-release.yaml", "sdc-bugfix.yaml"];
const GOOD: &str = "format: loom.bindings/1\nprotocol: example@1\nactions:\n  - action: a.one\n    executor: {kind: program, argv: [tools/x.sh]}\n    exits: {error: [75, 126, 127]}\n";

fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/check-bindings.sh")
}

fn root_in(name: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("check-bindings-adv2").join(name);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("bindings")).unwrap();
    root
}

struct Said {
    code: Option<i32>,
    out: String,
    err: String,
}

fn said(o: Output) -> Said {
    Said { code: o.status.code(), out: String::from_utf8_lossy(&o.stdout).into_owned(), err: String::from_utf8_lossy(&o.stderr).into_owned() }
}

fn run(root: &Path) -> Said {
    said(Command::new("bash").arg(script()).arg("--exits").arg(root).output().unwrap())
}

/// A tree whose `bindings/` is this repository's less the three pool files, with a copy of the script in `scripts/`.
fn script_tree(name: &str) -> PathBuf {
    let root = root_in(name);
    let from = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bindings");
    for e in fs::read_dir(from).unwrap() {
        let p = e.unwrap().path();
        let file = p.file_name().unwrap().to_string_lossy().into_owned();
        if p.extension().is_some_and(|x| x == "yaml") && !POOL_FILES.contains(&file.as_str()) {
            fs::copy(&p, root.join("bindings").join(&file)).unwrap();
        }
    }
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::copy(script(), root.join("scripts/check-bindings.sh")).unwrap();
    root
}

#[test]
fn a_tree_with_a_copy_of_the_script_is_another_runbook_for_a_relative_start_a_dot_root_and_a_linked_tree() {
    let root = script_tree("located");
    let want = |s: &Said, how: &str| {
        assert_eq!((s.code, s.out.as_str()), (Some(0), "pass exits\n"), "{how}: {}", s.err);
    };
    for dot in [".", "./"] {
        let o = Command::new("bash").current_dir(&root).arg("scripts/check-bindings.sh").arg("--exits").arg(dot).output().unwrap();
        want(&said(o), dot);
    }
    let o = Command::new("bash").current_dir(root.join("scripts")).arg("check-bindings.sh").arg("--exits").arg("..").output().unwrap();
    want(&said(o), "..");
    let link = root.parent().unwrap().join("located-link");
    let _ = fs::remove_file(&link);
    std::os::unix::fs::symlink(&root, &link).unwrap();
    let o = Command::new("bash").arg(link.join("scripts/check-bindings.sh")).arg("--exits").arg(&link).output().unwrap();
    want(&said(o), "link/link");
    let o = Command::new("bash").arg(root.join("scripts/check-bindings.sh")).arg("--exits").arg(&link).output().unwrap();
    want(&said(o), "script/link");
    let o = Command::new("bash").arg(link.join("scripts/check-bindings.sh")).arg("--exits").arg(&root).output().unwrap();
    want(&said(o), "link/root");
}

/// The acceptance of the story, with the script lying in the root: a root with program bindings that carry
/// `exits.error` and none of this repository's pool bindings exits 0 and prints no line about the build pool.
#[test]
fn a_runbook_that_holds_a_copy_of_the_script_is_not_asked_for_the_pool_bindings() {
    let root = root_in("vendored");
    fs::write(root.join("bindings/example.yaml"), GOOD).unwrap();
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::copy(script(), root.join("scripts/check-bindings.sh")).unwrap();
    let s = said(Command::new("bash").arg(root.join("scripts/check-bindings.sh")).arg("--exits").arg(&root).output().unwrap());
    assert_eq!((s.code, s.out.as_str()), (Some(0), "pass exits\n"), "{}", s.err);
}

#[test]
fn a_root_whose_path_has_glob_brackets_is_not_refused_for_having_no_bindings() {
    let root = root_in("brackets[1]");
    fs::write(root.join("bindings/example.yaml"), GOOD).unwrap();
    let s = run(&root);
    assert_eq!((s.code, s.out.as_str()), (Some(0), "pass exits\n"), "{}", s.err);
}

fn with_one_action(name: &str, argv: &str, codes: &str) -> Said {
    let root = root_in(name);
    let doc = format!("format: loom.bindings/1\nprotocol: example@1\nactions:\n  - action: a.one\n    executor: {{kind: program, argv: {argv}}}\n    exits: {{error: {codes}}}\n");
    fs::write(root.join("bindings/example.yaml"), doc).unwrap();
    run(&root)
}

#[test]
fn only_a_whole_pool_wait_element_of_the_argv_makes_the_five_codes_the_want() {
    let three = "[75, 126, 127]";
    let five = "[69, 70, 75, 126, 127]";
    let cases = [
        ("part-of-word", r#"[tools/x.sh, "x--pool-wait"]"#, three, 0),
        ("equals-form", r#"[tools/x.sh, "--pool-wait=600"]"#, three, 0),
        ("inside-a-quoted-argument", r#"[bash, -c, "slot --pool-wait 5 run"]"#, three, 0),
        ("later-element", r#"[tools/x.sh, a, b, c, d, --pool-wait, "5"]"#, five, 0),
        ("twice", r#"[tools/x.sh, --pool-wait, "5", --pool-wait, "6"]"#, five, 0),
        ("five-without", "[tools/x.sh]", five, 0),
        ("three-with", r#"[tools/x.sh, --pool-wait, "5"]"#, three, 1),
        ("five-with-part-of-word", r#"[tools/x.sh, "x--pool-wait"]"#, five, 0),
    ];
    for (name, argv, codes, want) in cases {
        let s = with_one_action(&format!("pw-{name}"), argv, codes);
        assert_eq!(s.code, Some(want), "{name}: {}\n{}", s.out, s.err);
        assert!(!s.out.contains("not list") && !s.out.contains("build pool"), "{name}: {}\n{}", s.out, s.err);
    }
}

#[test]
fn an_empty_file_beside_a_good_one_is_refused_by_name() {
    let root = root_in("empty-beside");
    fs::write(root.join("bindings/good.yaml"), GOOD).unwrap();
    fs::write(root.join("bindings/empty.yaml"), "").unwrap();
    let s = run(&root);
    assert_eq!(s.code, Some(1), "{}\n{}", s.out, s.err);
    assert!(s.out.contains("bindings/empty.yaml"), "{}\n{}", s.out, s.err);
}

#[test]
fn a_shape_that_the_check_cannot_read_is_refused_by_name_and_not_by_a_jq_error() {
    let shapes = [
        ("executor-string", "  - action: b.two\n    executor: nope\n"),
        ("argv-string", "  - action: b.two\n    executor: {kind: program, argv: nope}\n"),
        ("action-number", "  - 1\n"),
    ];
    for (name, bad) in shapes {
        let root = root_in(&format!("shape-{name}"));
        fs::write(root.join("bindings/good.yaml"), GOOD).unwrap();
        fs::write(root.join("bindings/bad.yaml"), format!("format: loom.bindings/1\nactions:\n{bad}")).unwrap();
        let s = run(&root);
        assert_eq!(s.code, Some(1), "{name}: {}\n{}", s.out, s.err);
        assert!(s.out.contains("bindings/bad.yaml") && !s.err.contains("jq:"), "{name}: {}\n{}", s.out, s.err);
    }
}
