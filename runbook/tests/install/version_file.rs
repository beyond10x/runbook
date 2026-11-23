//! What `share/runbook/VERSION` says and which bytes are installed: the commit the binary was built from, and a mark
//! when a tracked file of the copied directories differs from it. Fake checkouts with a git of their own.

use super::*;

fn version_of(prefix: &Path) -> String {
    fs::read_to_string(prefix.join("share/runbook/VERSION")).unwrap()
}

/// A clean checkout gives `VERSION` = what the binary prints; any tracked file of the checkout that differs from HEAD
/// (in a copied directory or outside them) adds ` dirty`, and the bytes installed are those of the work tree, not of
/// HEAD. An untracked file adds no mark.
#[test]
fn version_names_head_and_a_tracked_change_adds_one_word_and_installs_the_work_tree_bytes() {
    let root = fake_checkout("fix2-dirty", None);
    let (sandbox, prefix) = sandbox_of(&root);
    let head = head12(&root);
    let clean = stub_version(&head);
    assert_eq!(code(&from_built(&root, &prefix)), 0);
    assert_eq!(version_of(&prefix), format!("{clean}\n"), "a clean checkout");

    fs::write(root.join("protocol/untracked.txt"), "left by a run\n").unwrap();
    fs::write(root.join("README.md"), "outside the copied directories\n").unwrap();
    git_in(&root, &["add", "README.md"]);
    git_in(&root, &["commit", "-q", "-m", "readme"]);
    stubs(&root, None, &stub_version(&head12(&root)));
    let clean = stub_version(&head12(&root));
    assert_eq!(code(&from_built(&root, &prefix)), 0);
    assert_eq!(version_of(&prefix), format!("{clean}\n"), "an untracked file");
    fs::write(root.join("README.md"), "changed, and still outside\n").unwrap();
    assert_eq!(code(&from_built(&root, &prefix)), 0);
    assert_eq!(version_of(&prefix), format!("{clean} dirty\n"), "a tracked file outside the copied directories differs from HEAD");

    fs::write(root.join("protocol/file.txt"), "edited and not committed\n").unwrap();
    let second = sandbox.join("second/prefix");
    fs::create_dir_all(&second).unwrap();
    fs::create_dir_all(sandbox.join("second/home")).unwrap();
    let o = install(&root.join("scripts/install.sh"), &second, &["--from-built", "debug"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert_eq!(version_of(&second), format!("{clean} dirty\n"), "a tracked file of protocol/ differs from HEAD");
    assert_eq!(fs::read_to_string(second.join("share/runbook/protocol/file.txt")).unwrap(), "edited and not committed\n", "the bytes of the work tree");
    assert!(err(&o).contains("dirty"), "the closing line says it: {}", err(&o));
}

/// `VERSION` names HEAD as the commit the binary was built from and marks a work tree that differs with ` dirty`. The
/// mark covers every tracked file of the checkout, not only the copied directories: with an uncommitted change in a
/// tracked source of the binary (`runbook/src/`, `runbook/Cargo.lock`) the prefix does not say it is HEAD as it stands.
#[test]
fn version_has_a_dirty_mark_when_a_tracked_source_of_the_binary_differs_from_head() {
    let root = fake_checkout("dirty-source", None);
    let (_sandbox, prefix) = sandbox_of(&root);
    fs::create_dir_all(root.join("runbook/src")).unwrap();
    fs::write(root.join("runbook/src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(root.join("runbook/Cargo.lock"), "# the lock file\n").unwrap();
    git_in(&root, &["add", "runbook/src/main.rs", "runbook/Cargo.lock"]);
    git_in(&root, &["commit", "-q", "-m", "the sources of the binary"]);
    stubs(&root, None, &stub_version(&head12(&root)));
    fs::write(root.join("runbook/src/main.rs"), "fn main() { println!(\"changed and not committed\"); }\n").unwrap();
    fs::write(root.join("runbook/Cargo.lock"), "# the lock file, changed\n").unwrap();
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let version = fs::read_to_string(prefix.join("share/runbook/VERSION")).unwrap();
    assert!(
        version.trim().ends_with(" dirty"),
        "runbook/src/main.rs and runbook/Cargo.lock differ from HEAD ({}) and VERSION is {:?}",
        git_in(&root, &["status", "--porcelain", "--untracked-files=no"]).replace('\n', ", "),
        version.trim()
    );
}

fn version(prefix: &Path) -> String {
    fs::read_to_string(prefix.join("share/runbook/VERSION")).unwrap_or_default().trim().to_owned()
}

/// What `git status` calls a difference gives the mark, wherever the file is: a staged change, a mode and a deleted
/// file outside the copied directories. A clean checkout gets none, also when every file was touched since the index
/// was written and when the caller's git configuration asks `git status` for more lines, and an install over a prefix
/// that has the mark takes it away when the checkout is clean again. One checkout, five installs.
#[test]
fn every_difference_git_status_prints_gives_the_mark_and_a_clean_checkout_gets_none() {
    let root = fake_checkout("status-mark", None);
    let (_sandbox, prefix) = sandbox_of(&root);
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(root.join("docs/page.md"), "a page\n").unwrap();
    git_in(&root, &["add", "docs/page.md"]);
    git_in(&root, &["commit", "-q", "-m", "a page"]);
    stubs(&root, None, &stub_version(&head12(&root)));
    for key in ["status.branch", "status.short", "status.showStash", "status.relativePaths"] {
        git_in(&root, &["config", key, "true"]);
    }
    type Change<'a> = (&'a str, &'a dyn Fn(&Path), bool);
    let cases: [Change; 5] = [
        ("clean, touched, configured", &|r| {
            let touched = Command::new("find").args([".", "-path", "./.git", "-prune", "-o", "-type", "f", "-exec", "touch", "{}", "+"]).current_dir(r).output().unwrap();
            assert!(touched.status.success());
        }, false),
        ("staged outside", &|r| {
            fs::write(r.join("docs/page.md"), "changed\n").unwrap();
            git_in(r, &["add", "docs/page.md"]);
        }, true),
        ("mode outside", &|r| fs::set_permissions(r.join("docs/page.md"), fs::Permissions::from_mode(0o755)).unwrap(), true),
        ("deleted outside", &|r| fs::remove_file(r.join("docs/page.md")).unwrap(), true),
        ("clean again", &|_| {}, false),
    ];
    for (case, change, dirty) in cases {
        git_in(&root, &["reset", "-q", "--hard"]);
        change(&root);
        let o = from_built(&root, &prefix);
        assert_eq!(code(&o), 0, "{case}: {}", err(&o));
        assert_eq!(version(&prefix).ends_with(" dirty"), dirty, "{case}: VERSION is {:?}", version(&prefix));
    }
}

/// A file that git is told to leave out of `git status` (`--assume-unchanged`, `--skip-worktree`) and that differs from
/// HEAD in the work tree is installed with the bytes of the work tree, and `VERSION` names HEAD without the mark: the
/// mark is what `git status` prints, and git prints nothing for such a file.
#[test]
#[ignore = "story:install-marks-a-difference-git-status-hides"]
fn measurement_a_copied_file_marked_assume_unchanged_or_skip_worktree_that_differs_from_head_leaves_no_dirty_mark() {
    let mut broken = Vec::new();
    for flag in ["--assume-unchanged", "--skip-worktree"] {
        let root = fake_checkout(&format!("hidden{flag}"), None);
        let (_sandbox, prefix) = sandbox_of(&root);
        git_in(&root, &["update-index", flag, "bindings/file.txt"]);
        fs::write(root.join("bindings/file.txt"), "changed and not committed\n").unwrap();
        let o = from_built(&root, &prefix);
        assert_eq!(code(&o), 0, "{flag}: {}", err(&o));
        let installed = fs::read_to_string(prefix.join("share/runbook/bindings/file.txt")).unwrap();
        let of_head = git_in(&root, &["cat-file", "-p", "HEAD:bindings/file.txt"]);
        if installed.trim() != of_head.trim() && !version(&prefix).ends_with(" dirty") {
            broken.push(format!(
                "{flag}: share/runbook/bindings/file.txt holds {:?}, HEAD holds {:?}, and VERSION is {:?}",
                installed.trim(),
                of_head.trim(),
                version(&prefix)
            ));
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}
