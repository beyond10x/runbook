//! `sdc-check-names`: no line a change adds names a person, an organisation or a session. The refused
//! word of these tests is a session name the test builds from its parts at run time, so no source file
//! holds it; its digest is the first line of `driver/refused-names.txt`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The session name of `driver/refused-names.txt`'s first line, built from its parts.
fn refused_word() -> String {
    ["host", "operator"].join("-")
}

fn scratch(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("names").join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
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
    fn holds(&self, word: &str) -> bool {
        let w = word.to_lowercase();
        self.out.to_lowercase().contains(&w) || self.err.to_lowercase().contains(&w)
    }
}

fn check(root: &Path, args: &[&str]) -> Out {
    let o = Command::new(env!("CARGO_BIN_EXE_sdc-check-names")).arg("--root").arg(root).args(args).output().unwrap();
    Out {
        code: o.status.code().unwrap(),
        out: String::from_utf8_lossy(&o.stdout).into_owned(),
        err: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

/// Writes the files and the list `--files-from` reads; the full mode then needs no git.
fn tree(name: &str, files: &[(&str, String)]) -> (PathBuf, PathBuf) {
    let base = scratch(name);
    let root = base.join("tree");
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
    (root, list)
}

/// git in a fixture repository, with no hook, signing or identity of the machine's.
fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .args(["-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false",
            "-c", "core.hooksPath=/dev/null", "-c", "init.defaultBranch=main"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

#[test]
fn a_line_with_a_refused_word_fails_and_a_clean_line_passes() {
    let w = refused_word();
    let (root, list) = tree(
        "full-clean",
        &[("notes.md", "the operator session asked at 10:16:06Z\nhost and operator are two words\n".into())],
    );
    let o = check(&root, &["--full", "--files-from", list.to_str().unwrap()]);
    assert_eq!((o.code, o.out.as_str()), (0, ""), "{}", o.err);
    assert!(o.err.contains("2 lines"), "the summary counts the lines it checked: {}", o.err);

    let (root, list) = tree(
        "full-refused",
        &[
            ("clean.md", "nothing to see\n".into()),
            (
                "docs/notes.md",
                format!(
                    "the operator\n- `{w}` asked at 10:16:06Z\ncorrelation: \"{w}-2026-11-23-d671\"\n{}, 10:35:46Z\n",
                    w.to_uppercase()
                ),
            ),
        ],
    );
    let o = check(&root, &["--full", "--files-from", list.to_str().unwrap()]);
    assert_eq!(o.code, 1, "{}", o.err);
    assert_eq!(
        o.lines(),
        [
            "docs/notes.md:2: a refused session name (sha256 105ddfc5a2fa)",
            "docs/notes.md:3: a refused session name (sha256 105ddfc5a2fa)",
            "docs/notes.md:4: a refused session name (sha256 105ddfc5a2fa)",
        ]
    );
    assert!(!o.holds(&w), "a finding never prints the word: {}{}", o.out, o.err);
}

#[test]
fn a_change_is_checked_on_the_lines_it_adds_only() {
    let w = refused_word();
    let repo = scratch("diff").join("repo");
    fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "--quiet"]);
    fs::write(repo.join("a.md"), format!("old line naming {w}\n")).unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "--quiet", "-m", "base"]);
    let base = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["switch", "--quiet", "-c", "change"]);
    fs::write(repo.join("a.md"), format!("old line naming {w}\nthe operator asked\n")).unwrap();
    git(&repo, &["commit", "--quiet", "-am", "change"]);

    let o = check(&repo, &["--base", &base]);
    assert_eq!((o.code, o.out.as_str()), (0, ""), "an old line is not the change's: {}", o.err);
    assert!(o.err.contains("1 line"), "{}", o.err);

    fs::write(repo.join("a.md"), format!("old line naming {w}\nthe operator asked\nand {w} said\n")).unwrap();
    let o = check(&repo, &["--base", &base]);
    assert_eq!(o.code, 1, "{}", o.err);
    assert_eq!(o.lines(), ["a.md:3: a refused session name (sha256 105ddfc5a2fa)"]);
    assert!(!o.holds(&w), "{}{}", o.out, o.err);

    fs::write(repo.join(".gitignore"), "out/\n").unwrap();
    fs::create_dir_all(repo.join("out")).unwrap();
    fs::write(repo.join("out/log.txt"), format!("{w}\n")).unwrap();
    fs::write(repo.join("new.md"), format!("a new file\nnot yet added, naming {w}\n")).unwrap();
    let o = check(&repo, &["--base", &base]);
    assert_eq!(o.code, 1, "{}", o.err);
    assert_eq!(
        o.lines(),
        ["a.md:3: a refused session name (sha256 105ddfc5a2fa)", "new.md:2: a refused session name (sha256 105ddfc5a2fa)"],
        "a file git does not track yet is added whole; an ignored one is not checked"
    );
}

#[test]
fn without_origin_main_a_merge_head_is_checked_against_its_first_parent() {
    let w = refused_word();
    let repo = scratch("merge").join("repo");
    fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "--quiet"]);
    fs::write(repo.join("a.md"), "base\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "--quiet", "-m", "base"]);
    git(&repo, &["switch", "--quiet", "-c", "side"]);
    fs::write(repo.join("b.md"), format!("clean\n{w} here\n")).unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "--quiet", "-m", "side"]);
    git(&repo, &["switch", "--quiet", "main"]);
    fs::write(repo.join("c.md"), "main moved on\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "--quiet", "-m", "main"]);
    git(&repo, &["merge", "--quiet", "--no-ff", "-m", "merge", "side"]);

    let o = check(&repo, &[]);
    assert_eq!(o.code, 1, "{}", o.err);
    assert_eq!(o.lines(), ["b.md:2: a refused session name (sha256 105ddfc5a2fa)"]);
    assert!(o.err.contains("first parent"), "the base it took is named: {}", o.err);
}

/// The default mode on this repository: what the branch adds to `origin/main` (the merge base), or on
/// the forge's pull-request checkout (a merge, no `origin/main`) what the merge adds to its first parent.
#[test]
fn this_repository_adds_no_refused_word() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let o = check(root, &[]);
    eprintln!("{}", o.err);
    assert_eq!((o.code, o.out.as_str()), (0, ""), "{}", o.err);
}
