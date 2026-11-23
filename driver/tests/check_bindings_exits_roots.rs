//! Cases for `scripts/check-bindings.sh --exits <root>`: the pool list of this repository
//! and the roots that are not this repository. `ADV_CHECK_BINDINGS` names another copy of the script
//! (a mutant in the scratch directory); without it the script of the tree runs.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

const POOL_FILES: [&str; 3] = ["sdc-story.yaml", "sdc-release.yaml", "sdc-bugfix.yaml"];

fn script() -> PathBuf {
    match std::env::var_os("ADV_CHECK_BINDINGS") {
        Some(p) => PathBuf::from(p),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/check-bindings.sh"),
    }
}

fn root_in(name: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("check-bindings-adv").join(name);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("bindings")).unwrap();
    root
}

fn run(root: &std::path::Path) -> (Option<i32>, String, String) {
    let o: Output = Command::new("bash").arg(script()).arg("--exits").arg(root).output().unwrap();
    (o.status.code(), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// This repository's `bindings/*.yaml` copied into the root, less the files `skip` says to leave out.
fn own_bindings(name: &str, skip: impl Fn(&str) -> bool) -> PathBuf {
    let root = root_in(name);
    let from = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bindings");
    for e in fs::read_dir(from).unwrap() {
        let p = e.unwrap().path();
        let file = p.file_name().unwrap().to_string_lossy().into_owned();
        if p.extension().is_some_and(|x| x == "yaml") && !skip(&file) {
            fs::copy(&p, root.join("bindings").join(&file)).unwrap();
        }
    }
    root
}

const GOOD: &str = "format: loom.bindings/1\nprotocol: example@1\nactions:\n  - action: a.one\n    executor: {kind: program, argv: [tools/x.sh]}\n    exits: {error: [75, 126, 127]}\n";

#[test]
fn this_repository_without_the_three_named_files_is_still_reported() {
    let root = own_bindings("own-without-three", |f| POOL_FILES.contains(&f));
    let (code, out, err) = run(&root);
    assert_eq!(code, Some(1), "{out}\n{err}");
    for p in ["sdc-story.yaml:tests.run", "sdc-release.yaml:acceptance.run", "sdc-bugfix.yaml:gate.run"] {
        assert!(out.contains(&format!("bindings/{p}: a binding that waits for the build pool is not in bindings/")), "{out}\n{err}");
    }
}

#[test]
fn this_repository_with_the_three_named_files_empty_is_still_reported() {
    let root = own_bindings("own-three-empty", |f| POOL_FILES.contains(&f));
    for f in POOL_FILES {
        fs::write(root.join("bindings").join(f), "").unwrap();
    }
    let (code, out, err) = run(&root);
    assert_eq!(code, Some(1), "{out}\n{err}");
    assert!(out.contains("a binding that waits for the build pool is not in bindings/"), "{out}\n{err}");
}

#[test]
fn a_root_holding_only_one_named_file_gets_the_pool_list() {
    for keep in POOL_FILES {
        let root = own_bindings(&format!("only-{keep}"), |f| f != keep);
        let (code, out, err) = run(&root);
        assert_eq!(code, Some(1), "{keep}: {out}\n{err}");
        let other = POOL_FILES.iter().find(|f| **f != keep).unwrap();
        assert!(out.contains(&format!("bindings/{other}:")) && out.contains("a binding that waits for the build pool is not in bindings/"), "{keep}: {out}\n{err}");
    }
}

#[test]
fn a_file_that_is_not_a_mapping_is_refused_by_name() {
    let root = root_in("scalar");
    fs::write(root.join("bindings/good.yaml"), GOOD).unwrap();
    fs::write(root.join("bindings/scalar.yaml"), "hello\n").unwrap();
    let (code, out, err) = run(&root);
    assert_eq!(code, Some(1), "{out}\n{err}");
    assert!(out.contains("bindings/scalar.yaml"), "{out}\n{err}");
}

#[test]
fn a_file_without_actions_is_refused_by_name_and_not_by_a_jq_error() {
    let root = root_in("no-actions");
    fs::write(root.join("bindings/good.yaml"), GOOD).unwrap();
    fs::write(root.join("bindings/header.yaml"), "format: loom.bindings/1\nprotocol: example@1\n").unwrap();
    let (code, out, err) = run(&root);
    assert_eq!(code, Some(1), "{out}\n{err}");
    assert!(out.contains("bindings/header.yaml") && !err.contains("jq:"), "{out}\n{err}");
}

#[test]
fn another_runbook_in_a_directory_with_a_space_and_a_trailing_slash_passes() {
    let root = root_in("with space");
    fs::write(root.join("bindings/example.yaml"), GOOD).unwrap();
    let (code, out, err) = run(&root);
    assert_eq!((code, out.as_str()), (Some(0), "pass exits\n"), "{err}");
    let mut slashed = root.clone().into_os_string();
    slashed.push("/");
    let (code, out, err) = run(&PathBuf::from(slashed));
    assert_eq!((code, out.as_str()), (Some(0), "pass exits\n"), "{err}");
}
