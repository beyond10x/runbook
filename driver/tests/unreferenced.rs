//! `sdc-check-unreferenced` on small fixture trees written to a temporary directory and given to
//! the tool with `--files-from`, so no test starts git. Each rule has a case that it refers to and
//! a case that it does not.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

struct Tree {
    root: PathBuf,
    list: PathBuf,
}

fn tree(name: &str, files: &[(&str, &str)]) -> Tree {
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("unreferenced").join(name);
    let _ = fs::remove_dir_all(&base);
    let root = base.join("tree");
    fs::create_dir_all(&root).unwrap();
    let mut names = String::new();
    for (p, text) in files {
        let at = root.join(p);
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        fs::write(&at, text).unwrap();
        names.push_str(p);
        names.push('\n');
    }
    let list = base.join("files.txt");
    fs::write(&list, names).unwrap();
    Tree { root, list }
}

struct Out {
    code: i32,
    out: String,
    err: String,
}

impl Out {
    fn lines(&self) -> Vec<&str> {
        self.out.lines().collect()
    }
}

fn run(t: &Tree, args: &[&str]) -> Out {
    let o = Command::new(env!("CARGO_BIN_EXE_sdc-check-unreferenced"))
        .arg("--root")
        .arg(&t.root)
        .arg("--files-from")
        .arg(&t.list)
        .args(args)
        .output()
        .unwrap();
    Out { code: o.status.code().unwrap(), out: String::from_utf8_lossy(&o.stdout).into_owned(), err: String::from_utf8_lossy(&o.stderr).into_owned() }
}

#[test]
fn a_file_named_by_its_path_is_referred_to_and_a_near_miss_is_not() {
    let t = tree(
        "path",
        &[
            ("Taskfile.yml", "run: scripts/a.sh\nalso: ./scripts/b.sh\nold: scripts/c.sh.bak\nother: scripts/d.sh2\n"),
            ("scripts/a.sh", ""),
            ("scripts/b.sh", ""),
            ("scripts/c.sh", ""),
            ("scripts/d.sh", ""),
        ],
    );
    let o = run(&t, &[]);
    assert_eq!(o.code, 1, "{}", o.err);
    assert_eq!(o.lines(), ["scripts/c.sh", "scripts/d.sh"]);
}

#[test]
fn an_empty_list_is_exit_zero() {
    let t = tree("empty", &[("Taskfile.yml", "run: scripts/a.sh\n"), ("scripts/a.sh", "")]);
    let o = run(&t, &[]);
    assert_eq!((o.code, o.out.as_str()), (0, ""));
}

#[test]
fn a_file_does_not_refer_to_itself_and_files_outside_the_scope_are_never_listed() {
    let t = tree("self", &[("scripts/a.sh", "scripts/a.sh\n"), ("docs/notes.md", "nobody names this\n")]);
    let o = run(&t, &[]);
    assert_eq!(o.lines(), ["scripts/a.sh"]);
}

#[test]
fn a_path_relative_to_the_directory_of_the_referrer_or_to_its_crate_refers() {
    let t = tree(
        "relative",
        &[
            ("bindings/tools/x.sh", "read answers.yaml; source ../sdc.yaml; \"$here/../tools/y.json\"\n"),
            ("bindings/tools/answers.yaml", ""),
            ("bindings/tools/spare.yaml", ""),
            ("bindings/sdc.yaml", ""),
            ("bindings/tools/y.json", ""),
            ("bindings/outside.yaml", ""),
            ("driver/Cargo.toml", "[package]\n"),
            ("driver/tests/t.rs", "let p = \"tests/fixtures/f/a.json\";\n"),
            ("driver/tests/fixtures/f/a.json", ""),
            ("driver/tests/fixtures/f/b.json", ""),
            ("driver/tests/fixtures/g/a.json", ""),
        ],
    );
    let o = run(&t, &["--keep", "none.txt"]);
    assert_eq!(o.code, 2, "an explicit keep file must exist");
    let o = run(&t, &[]);
    assert_eq!(
        o.lines(),
        ["bindings/outside.yaml", "bindings/tools/spare.yaml", "bindings/tools/x.sh", "driver/tests/fixtures/f/b.json", "driver/tests/fixtures/g/a.json"]
    );
}

#[test]
fn a_directory_plus_a_name_below_it_refers_and_the_name_alone_in_another_directory_does_not() {
    let t = tree(
        "dirname",
        &[
            ("scripts/run.sh", "d=instructions/shared\ncat \"$d/closing.md\"\nsub=instructions/decider/inner\ncat \"$sub/leaf/deep.md\"\n"),
            ("instructions/shared/closing.md", ""),
            ("instructions/shared/other.md", ""),
            ("instructions/elsewhere/closing.md", ""),
            ("instructions/decider/inner/leaf/deep.md", ""),
            ("Taskfile.yml", "x: scripts/run.sh\n"),
        ],
    );
    let o = run(&t, &[]);
    assert_eq!(o.lines(), ["instructions/elsewhere/closing.md", "instructions/shared/other.md"]);
}

#[test]
fn a_mod_declaration_refers_to_its_file_and_to_a_nested_one() {
    let t = tree(
        "mods",
        &[
            ("Taskfile.yml", "x: watch/src/main.rs\n"),
            ("watch/src/main.rs", "mod a;\npub mod b;\n// mod ghost;\n"),
            ("watch/src/a.rs", "mod d;\n"),
            ("watch/src/a/d.rs", ""),
            ("watch/src/b/mod.rs", ""),
            ("watch/src/c.rs", ""),
            ("watch/src/ghost.rs", ""),
        ],
    );
    let o = run(&t, &[]);
    assert_eq!(o.lines(), ["watch/src/c.rs", "watch/src/ghost.rs"]);
}

#[test]
fn cargo_targets_refer_unless_the_manifest_turns_discovery_off() {
    let t = tree(
        "cargo",
        &[
            ("goals/Cargo.toml", "[package]\nname = \"g\"\n"),
            ("goals/src/main.rs", ""),
            ("goals/src/bin/extra.rs", ""),
            ("goals/tests/check.rs", ""),
            ("goals/tests/notes.txt", ""),
            ("watch/Cargo.toml", "[package]\nautotests = false\nautobins=false\n"),
            ("watch/src/main.rs", ""),
            ("watch/src/bin/extra.rs", ""),
            ("watch/tests/check.rs", ""),
        ],
    );
    let o = run(&t, &[]);
    assert_eq!(o.lines(), ["goals/tests/notes.txt", "watch/src/bin/extra.rs", "watch/tests/check.rs"]);
}

#[test]
fn the_files_a_tool_reads_by_convention_are_never_listed() {
    let t = tree("convention", &[("goals/Cargo.lock", ""), ("eval/quality/package.json", ""), ("eval/quality/package-lock.json", "")]);
    let o = run(&t, &[]);
    assert_eq!((o.code, o.out.as_str()), (0, ""));
}

#[test]
fn planning_history_and_the_data_files_do_not_count_as_referrers() {
    let t = tree(
        "history",
        &[
            (".engineering/planning/story/x.md", "scripts/a.sh scripts/b.sh scripts/c.sh scripts/d.sh\n"),
            ("CHANGELOG.md", "scripts/a.sh\n"),
            ("docs/releases/1.md", "scripts/b.sh\n"),
            ("experiments/r.yaml", "scripts/c.sh\n"),
            ("docs/live.md", "scripts/d.sh\n"),
            ("scripts/a.sh", ""),
            ("scripts/b.sh", ""),
            ("scripts/c.sh", ""),
            ("scripts/d.sh", ""),
        ],
    );
    let o = run(&t, &[]);
    assert_eq!(o.lines(), ["scripts/a.sh", "scripts/b.sh", "scripts/c.sh"]);
}

#[test]
fn a_directory_read_as_a_whole_refers_to_every_file_below_it() {
    let t = tree(
        "whole",
        &[
            ("scripts/walk.sh", "find \"$root/cases\" -name expected.json\n"),
            ("Taskfile.yml", "x: scripts/walk.sh\n"),
            ("cases/one/case.yaml", ""),
            ("cases/one/work/notes.md", ""),
            ("cases/two/case.yaml", ""),
            ("tools/free.sh", ""),
            ("whole.txt", "cases scripts/walk.sh the script walks cases/ with find\n"),
        ],
    );
    let o = run(&t, &["--whole", "whole.txt"]);
    assert_eq!(o.lines(), ["tools/free.sh"]);
    let o = run(&t, &[]);
    assert_eq!(o.lines().len(), 4, "without the whole file the three cases files and the tool are listed: {:?}", o.lines());
}

#[test]
fn a_whole_line_that_is_wrong_is_refused() {
    let t = tree(
        "whole-refused",
        &[
            ("scripts/walk.sh", "find cases\n"),
            ("Taskfile.yml", "x: scripts/walk.sh\n"),
            ("cases/one/case.yaml", ""),
            ("docs/notes.md", "cases\n"),
        ],
    );
    for (line, reason) in [
        ("nowhere scripts/walk.sh the directory is gone", "has no tracked file"),
        ("cases scripts/gone.sh the referrer is gone", "is not a referrer"),
        ("cases tools/none.sh the referrer was never there", "is not a referrer"),
        ("cases scripts/walk.sh", "needs all three"),
    ] {
        fs::write(t.root.join("whole.txt"), format!("{line}\n")).unwrap();
        let o = run(&t, &["--whole", "whole.txt"]);
        assert_eq!(o.code, 2, "{line}: {}", o.out);
        assert!(o.err.contains(reason), "{line}: {}", o.err);
    }
    fs::write(t.root.join("whole.txt"), "cases docs/notes.md prose that names the directory is a referrer too\n").unwrap();
    let o = run(&t, &["--whole", "whole.txt"]);
    assert_eq!((o.code, o.out.as_str()), (0, ""), "{}", o.err);
    let t2 = tree("whole-no-name", &[("scripts/walk.sh", "find stuff\n"), ("Taskfile.yml", "x: scripts/walk.sh\n"), ("cases/one/case.yaml", "")]);
    fs::write(t2.root.join("whole.txt"), "cases scripts/walk.sh the script never names cases\n").unwrap();
    let o = run(&t2, &["--whole", "whole.txt"]);
    assert_eq!(o.code, 2);
    assert!(o.err.contains("has no token with the directory's own name"), "{}", o.err);
}

#[test]
fn a_kept_file_is_not_listed_and_a_bad_keep_line_is_refused() {
    let files: &[(&str, &str)] = &[("Taskfile.yml", "x: scripts/used.sh\n"), ("scripts/used.sh", ""), ("scripts/dead.sh", ""), ("scripts/dead2.sh", "")];
    let t = tree("keep", files);
    fs::write(t.root.join("keep.txt"), "scripts/dead.sh no caller yet\n").unwrap();
    let o = run(&t, &["--keep", "keep.txt"]);
    assert_eq!(o.lines(), ["scripts/dead2.sh"]);
    for (line, why) in [
        ("scripts/gone.sh the file was deleted", "is not a tracked file in scope"),
        ("scripts/used.sh it is called", "is referred to now"),
        ("scripts/dead.sh", "needs a reason"),
        ("Taskfile.yml out of scope", "is not a tracked file in scope"),
    ] {
        fs::write(t.root.join("keep.txt"), format!("{line}\n")).unwrap();
        let o = run(&t, &["--keep", "keep.txt"]);
        assert_eq!(o.code, 2, "{line}: {}", o.out);
        assert!(o.err.contains(why), "{line}: {}", o.err);
    }
    fs::write(t.root.join("keep.txt"), "scripts/dead.sh once\nscripts/dead.sh twice\n").unwrap();
    let o = run(&t, &["--keep", "keep.txt"]);
    assert_eq!(o.code, 2);
    assert!(o.err.contains("is listed twice"), "{}", o.err);
}

#[test]
fn the_keep_file_does_not_make_a_file_referred_to() {
    let t = tree("keep-not-a-ref", &[("scripts/dead.sh", ""), ("driver-keep.txt", "scripts/dead.sh a reason\n")]);
    let o = run(&t, &["--keep", "driver-keep.txt"]);
    assert_eq!((o.code, o.out.as_str()), (0, ""), "{}", o.err);
}

fn without_tree(name: &str) -> Tree {
    tree(
        name,
        &[
            ("Taskfile.yml", "x: scripts/a.sh\ny: scripts/b.sh\nz: scripts/z.sh\n"),
            ("scripts/a.sh", "tools/only-a.sh tools/both.sh\n"),
            ("scripts/b.sh", "tools/both.sh tools/only-b.sh\n"),
            ("scripts/z.sh", ""),
            ("tools/only-a.sh", ""),
            ("tools/only-b.sh", ""),
            ("tools/both.sh", ""),
            ("tools/dead-already.sh", ""),
        ],
    )
}

#[test]
fn without_lists_what_only_the_deleted_files_refer_to_and_names_them() {
    let t = without_tree("without");
    let o = run(&t, &["--without", "scripts/a.sh"]);
    assert_eq!((o.code, o.lines()), (0, vec!["tools/only-a.sh\tscripts/a.sh"]));
    let o = run(&t, &["--without", "scripts/a.sh", "scripts/b.sh"]);
    assert_eq!(
        o.lines(),
        ["tools/both.sh\tscripts/a.sh,scripts/b.sh", "tools/only-a.sh\tscripts/a.sh", "tools/only-b.sh\tscripts/b.sh"]
    );
}

#[test]
fn without_does_not_list_a_file_that_was_dead_before_or_the_deleted_files_themselves() {
    let t = without_tree("without-dead");
    let o = run(&t, &["--without", "scripts/a.sh", "scripts/b.sh"]);
    assert!(!o.out.contains("dead-already"), "{}", o.out);
    assert!(!o.out.contains("scripts/z.sh"), "{}", o.out);
    assert!(!o.out.lines().any(|l| l.starts_with("scripts/a.sh") || l.starts_with("scripts/b.sh")), "{}", o.out);
}

#[test]
fn without_a_whole_directory_rule_counts_when_the_referrer_is_deleted() {
    let t = tree(
        "without-whole",
        &[
            ("scripts/walk.sh", "find cases\n"),
            ("Taskfile.yml", "x: scripts/walk.sh\n"),
            ("cases/one/case.yaml", ""),
            ("whole.txt", "cases scripts/walk.sh walked by find\n"),
        ],
    );
    let o = run(&t, &["--whole", "whole.txt", "--without", "scripts/walk.sh"]);
    assert_eq!(o.lines(), ["cases/one/case.yaml\tscripts/walk.sh"]);
}

#[test]
fn without_a_path_that_is_not_tracked_is_an_error() {
    let t = without_tree("without-bad");
    let o = run(&t, &["--without", "scripts/none.sh"]);
    assert_eq!(o.code, 2);
    assert!(o.err.contains("not a tracked file"), "{}", o.err);
}

#[test]
fn json_output_has_the_path_and_the_naming_files() {
    let t = without_tree("json");
    let o = run(&t, &["--format", "json", "--without", "scripts/a.sh"]);
    let v: serde_json::Value = serde_json::from_str(&o.out).unwrap();
    assert_eq!(v, serde_json::json!([{"path": "tools/only-a.sh", "named_by": ["scripts/a.sh"]}]));
    let o = run(&t, &["--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&o.out).unwrap();
    assert_eq!(v, serde_json::json!([{"path": "tools/dead-already.sh", "named_by": []}]));
    assert_eq!(o.code, 1);
}

#[test]
fn typescript_imports_without_an_extension_refer() {
    let t = tree(
        "ts",
        &[
            ("eval/quality/main.ts", "import {a} from './adapters';\nimport b from './blind.mjs';\n"),
            ("eval/quality/adapters.ts", ""),
            ("eval/quality/blind.mjs", ""),
            ("eval/quality/lonely.ts", ""),
            ("Taskfile.yml", "x: eval/quality/main.ts\n"),
        ],
    );
    let o = run(&t, &[]);
    assert_eq!(o.lines(), ["eval/quality/lonely.ts"]);
}

#[test]
fn the_help_names_the_rules_and_the_left_out_directories() {
    let o = Command::new(env!("CARGO_BIN_EXE_sdc-check-unreferenced")).arg("--help").output().unwrap();
    let h = String::from_utf8_lossy(&o.stdout);
    for want in ["SCOPE", "Left out, with the reason", "docs/", "ui/", "--without", "--keep", "--whole", "mod", "cargo", "whole"] {
        assert!(h.contains(want), "--help lacks {want}: {h}");
    }
}
