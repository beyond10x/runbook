//! Cases for `scripts/check-bindings.sh`: links, shapes and path spellings that reach `--exits <root>` and the full check. The script is the one of the tree.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const GOOD: &str = "format: loom.bindings/1\nprotocol: example@1\nactions:\n  - action: a.one\n    executor: {kind: program, argv: [tools/x.sh]}\n    exits: {error: [75, 126, 127]}\n";
const HDR: &str = "format: loom.bindings/1\nprotocol: example@1\nactions:\n";

fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/check-bindings.sh")
}

fn root_in(name: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("check-bindings-links-and-shapes").join(name);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("bindings")).unwrap();
    root
}

fn run_in(cwd: Option<&Path>, root: &str) -> (Option<i32>, String, String) {
    let mut c = Command::new("bash");
    if let Some(d) = cwd {
        c.current_dir(d);
    }
    let o = c.arg(script()).arg("--exits").arg(root).output().unwrap();
    (o.status.code(), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn run(root: &Path) -> (Option<i32>, String, String) {
    run_in(None, root.to_str().unwrap())
}

#[test]
fn a_dangling_link_in_bindings_is_refused_by_name_and_not_skipped() {
    let root = root_in("dangling-beside");
    fs::write(root.join("bindings/good.yaml"), GOOD).unwrap();
    std::os::unix::fs::symlink("nowhere", root.join("bindings/dangling.yaml")).unwrap();
    let (code, out, err) = run(&root);
    assert_eq!(code, Some(1), "{out}\n{err}");
    assert!(format!("{out}{err}").contains("bindings/dangling.yaml"), "{out}\n{err}");
}

#[test]
fn an_executor_that_is_null_is_refused_like_one_that_is_no_mapping() {
    let root = root_in("executor-null");
    fs::write(root.join("bindings/good.yaml"), GOOD).unwrap();
    fs::write(root.join("bindings/bad.yaml"), format!("{HDR}  - action: b.two\n    executor:\n")).unwrap();
    let (code, out, err) = run(&root);
    assert_eq!(code, Some(1), "{out}\n{err}");
    assert!(out.contains("bindings/bad.yaml b.two: executor is not a mapping"), "{out}\n{err}");
}

#[test]
fn evidence_of_a_shape_the_check_cannot_read_is_refused_by_name_and_not_by_a_jq_error() {
    let shapes = [
        ("map", "evidence: {a: b}"),
        ("number", "evidence: [1]"),
        ("non-numeric-result-key", "evidence: [{kind: k, from: exit, results: {ok: pass}}]"),
        ("results-string", "evidence: [{kind: k, from: exit, results: nope}]"),
    ];
    for (name, ev) in shapes {
        let root = root_in(&format!("evidence-{name}"));
        fs::write(root.join("bindings/good.yaml"), GOOD).unwrap();
        let doc = format!("{HDR}  - action: b.two\n    executor: {{kind: program, argv: [x]}}\n    exits: {{error: [75, 126, 127]}}\n    {ev}\n");
        fs::write(root.join("bindings/bad.yaml"), doc).unwrap();
        let (code, out, err) = run(&root);
        assert_eq!(code, Some(1), "{name}: {out}\n{err}");
        assert!(out.contains("bindings/bad.yaml") && !err.contains("jq:"), "{name}: {out}\n{err}");
    }
}

#[test]
fn a_root_given_as_a_relative_path_that_starts_with_a_dash_is_read() {
    let parent = root_in("dash-parent");
    let root = parent.join("-dash");
    fs::create_dir_all(root.join("bindings")).unwrap();
    fs::write(root.join("bindings/example.yaml"), GOOD).unwrap();
    let (code, out, err) = run_in(Some(&parent), "-dash");
    assert_eq!((code, out.as_str()), (Some(0), "pass exits\n"), "{err}");
}

/// The full check without `--exits`, cut before its cargo steps (at the answers file), over a tree with this
/// repository's bindings, the protocol files and the compiled protocol and JSON bindings of `sdc-story` and a
/// dangling link `extra.yaml` in `bindings/`: the base stops at the exits step with exit 1.
#[test]
fn the_full_check_fails_for_a_dangling_link_in_bindings() {
    let root = root_in("full-dangling");
    let from = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    for e in fs::read_dir(from.join("bindings")).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "yaml" || x == "md") {
            fs::copy(&p, root.join("bindings").join(p.file_name().unwrap())).unwrap();
        }
    }
    fs::create_dir_all(root.join("protocol")).unwrap();
    for e in fs::read_dir(from.join("protocol")).unwrap() {
        let p = e.unwrap().path();
        if p.to_string_lossy().ends_with(".protocol.yaml") {
            fs::copy(&p, root.join("protocol").join(p.file_name().unwrap())).unwrap();
        }
    }
    for (dir, file) in [("driver/examples/sdc", "sdc-story.bindings.json"), ("protocol/compiled", "sdc-story.ir.json")] {
        fs::create_dir_all(root.join(dir)).unwrap();
        fs::copy(from.join(dir).join(file), root.join(dir).join(file)).unwrap();
    }
    std::os::unix::fs::symlink(from.join("instructions").canonicalize().unwrap(), root.join("instructions")).unwrap();
    std::os::unix::fs::symlink(from.join("bindings/tools").canonicalize().unwrap(), root.join("bindings/tools")).unwrap();
    std::os::unix::fs::symlink("nowhere", root.join("bindings/extra.yaml")).unwrap();
    let text = fs::read_to_string(script()).unwrap();
    let cut = text.split("# The answers file of").next().unwrap();
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::write(root.join("scripts/check-bindings.sh"), format!("{cut}exit $fail\n")).unwrap();
    let o = Command::new("bash").arg(root.join("scripts/check-bindings.sh")).output().unwrap();
    let (out, err) = (String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned());
    assert!(out.contains("pass sdc-story") && out.contains("pass registrations"), "the fixture is not a clean tree: {out}\n{err}");
    assert_ne!(o.status.code(), Some(0), "{out}\n{err}");
}

/// The page: a refused file leaves the other files checked, and text that is no YAML names the file in yq's message.
#[test]
fn a_refused_file_leaves_the_other_files_checked_and_non_yaml_names_the_file() {
    let root = root_in("others-checked");
    fs::write(root.join("bindings/a-empty.yaml"), "").unwrap();
    fs::write(root.join("bindings/b-bare.yaml"), format!("{HDR}  - action: bare.action\n    executor: {{kind: program, argv: [x]}}\n")).unwrap();
    let (code, out, err) = run(&root);
    assert_eq!(
        (code, out.as_str()),
        (Some(1), "FAIL exits\n  bindings/a-empty.yaml: the file is empty\n  bindings/b-bare.yaml bare.action: exits.error is [], want [75,126,127] or [69,70,75,126,127]\n"),
        "{err}"
    );
    let root = root_in("non-yaml");
    fs::write(root.join("bindings/good.yaml"), GOOD).unwrap();
    fs::write(root.join("bindings/text.yaml"), "this: is: [not\n  yaml {{{\n").unwrap();
    let (code, out, err) = run(&root);
    assert_eq!(code, Some(1), "{out}\n{err}");
    assert!(format!("{out}{err}").contains("bindings/text.yaml"), "{out}\n{err}");
}

/// What another runbook gets when it holds a file that has one of the three names: the five pool lines.
#[test]
fn a_runbook_of_another_repository_with_a_file_named_sdc_story_yaml_gets_the_five_pool_lines() {
    let root = root_in("named-own");
    fs::write(root.join("bindings/sdc-story.yaml"), GOOD).unwrap();
    let (code, out, err) = run(&root);
    let want = "FAIL exits\n  bindings/sdc-story.yaml:tests.run: a binding that waits for the build pool is not in bindings/\n  bindings/sdc-release.yaml:acceptance.run: a binding that waits for the build pool is not in bindings/\n  bindings/sdc-bugfix.yaml:repro.run: a binding that waits for the build pool is not in bindings/\n  bindings/sdc-bugfix.yaml:tests.run: a binding that waits for the build pool is not in bindings/\n  bindings/sdc-bugfix.yaml:gate.run: a binding that waits for the build pool is not in bindings/\n";
    assert_eq!((code, out.as_str()), (Some(1), want), "{err}");
}

/// The files the script's pool list names are the files the pin and the page name.
#[test]
fn the_files_the_script_names_for_the_pool_list_are_the_three_the_page_names() {
    let text = fs::read_to_string(script()).unwrap();
    let mut named: Vec<String> = Vec::new();
    for part in text.split("\"bindings/").skip(1) {
        if let Some((file, rest)) = part.split_once(".yaml:") {
            if rest.starts_with(|c: char| c.is_ascii_lowercase()) {
                let f = format!("{file}.yaml");
                if !named.contains(&f) {
                    named.push(f);
                }
            }
        }
    }
    named.sort();
    assert_eq!(named, ["sdc-bugfix.yaml", "sdc-release.yaml", "sdc-story.yaml"]);
    let page = fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../docs/loom-bindings.md")).unwrap();
    for f in &named {
        assert!(page.contains(&format!("`{f}`")), "{f}");
        assert!(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bindings").join(f).is_file(), "{f}");
    }
}

fn one(name: &str, argv: &str, codes: &str) -> (Option<i32>, String, String) {
    let root = root_in(name);
    let doc = format!("{HDR}  - action: a.one\n    executor: {{kind: program, argv: {argv}}}\n    exits: {{error: {codes}}}\n");
    fs::write(root.join("bindings/example.yaml"), doc).unwrap();
    run(&root)
}

/// For another runbook: the flag as a whole argv element wants the five codes; without it the three or the five.
#[test]
fn another_runbook_every_flag_spelling_times_every_code_list() {
    let spellings: [(&str, &str, bool); 6] = [
        ("flag", r#"[x, --pool-wait, "5"]"#, true),
        ("equals", r#"[x, "--pool-wait=5"]"#, false),
        ("longer-word", r#"[x, "no--pool-waits"]"#, false),
        ("quoted-in-string", r#"[sh, -c, "x --pool-wait 5"]"#, false),
        ("absent", "[x]", false),
        ("as-argv0", r#"["--pool-wait"]"#, true),
    ];
    let lists: [(&str, &str); 7] = [
        ("three", "[75, 126, 127]"),
        ("five", "[69, 70, 75, 126, 127]"),
        ("subset", "[75, 126]"),
        ("superset", "[69, 70, 75, 126, 127, 128]"),
        ("reordered-five", "[127, 126, 75, 70, 69]"),
        ("reordered-three", "[127, 75, 126]"),
        ("empty", "[]"),
    ];
    let mut wrong = Vec::new();
    for (sn, argv, flag) in spellings {
        for (ln, codes) in lists {
            let (code, out, err) = one(&format!("m-{sn}-{ln}"), argv, codes);
            let five = ln == "five" || ln == "reordered-five";
            let three = ln == "three" || ln == "reordered-three";
            let ok = if flag { five } else { five || three };
            let got_ok = code == Some(0) && out == "pass exits\n";
            let got_one_line = code == Some(1) && out.lines().count() == 2 && out.contains("bindings/example.yaml a.one: exits.error is ");
            if !(if ok { got_ok } else { got_one_line }) {
                wrong.push(format!("{sn} x {ln}: expected {} got {code:?} {out:?} {err:?}", if ok { "pass" } else { "one line" }));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// A flag in a nested executor member is not the argv of the binding.
#[test]
fn a_flag_in_a_nested_member_of_the_executor_does_not_make_the_five_codes_the_want() {
    let root = root_in("nested");
    let doc = format!("{HDR}  - action: a.one\n    executor: {{kind: program, argv: [x], fallback: {{argv: [--pool-wait, \"5\"]}}}}\n    exits: {{error: [75, 126, 127]}}\n");
    fs::write(root.join("bindings/example.yaml"), doc).unwrap();
    let (code, out, err) = run(&root);
    assert_eq!((code, out.as_str()), (Some(0), "pass exits\n"), "{err}");
}
