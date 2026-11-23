//! What the build of `runbook` watches to notice that HEAD moved: nothing that changes when it did not.

use super::*;


fn repository_with(tag: &str, init: &[&str]) -> Option<PathBuf> {
    let repo = scratch(tag);
    fs::create_dir_all(repo.join("runbook")).unwrap();
    fs::write(repo.join("runbook/Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
    let o = Command::new("git").args(["-c", "init.defaultBranch=main"]).args(init).current_dir(&repo).env_remove("GIT_DIR").env_remove("GIT_WORK_TREE").env_remove("GIT_INDEX_FILE").output().unwrap();
    if !o.status.success() {
        return None;
    }
    git(&repo, &["add", "runbook/Cargo.toml"]);
    git(&repo, &["commit", "-q", "-m", "first"]);
    Some(repo)
}

/// The build watches the whole directory `refs/heads` and `packed-refs` of the common repository. They change when any
/// branch of the repository moves, in any worktree, and when refs are packed: cargo then runs `build.rs` again and
/// compiles the package (seen with the real cargo on a scratch crate), though HEAD of this checkout is where it was.
#[test]
fn what_the_build_watches_changes_when_another_branch_moves_and_head_does_not() {
    let repo = repository_with("fix2-other-branch", &["init", "-q"]).unwrap();
    let linked = scratch("fix2-other-branch-linked").join("tree");
    git(&repo, &["worktree", "add", "-q", "-b", "side", linked.to_str().unwrap(), "HEAD"]);
    let manifest = linked.join("runbook");
    let mut moved = Vec::new();
    let steps: [(&str, &dyn Fn()); 3] = [
        ("a branch made in the main worktree", &|| {
            git(&repo, &["branch", "another"]);
        }),
        ("a commit on main in the main worktree", &|| {
            fs::write(repo.join("note.txt"), "x").unwrap();
            git(&repo, &["add", "note.txt"]);
            git(&repo, &["commit", "-q", "-m", "main moves"]);
        }),
        ("git pack-refs --all", &|| {
            git(&repo, &["pack-refs", "--all"]);
        }),
    ];
    for (what, step) in steps {
        let c = commit_source::commit(&manifest);
        let head = head12(&linked);
        assert_eq!(c.value, head);
        let before = newest(&c.watch);
        later();
        step();
        assert_eq!(head12(&linked), head, "{what}: HEAD of the linked worktree did not move");
        if newest(&c.watch) != before {
            moved.push(what);
        }
    }
    assert!(moved.is_empty(), "HEAD did not move and the watched paths changed (cargo compiles the package again) after: {moved:?}");
}

/// In a repository with the reftable format `HEAD` is a fixed stub and `refs/heads` a fixed file: no watched path
/// changes when HEAD moves, the build script is not run again, and `runbook --version` keeps the commit of the
/// first build.
#[test]
fn in_a_reftable_repository_a_commit_changes_nothing_the_build_watches() {
    let Some(repo) = repository_with("fix2-reftable", &["init", "-q", "--ref-format=reftable"]) else {
        eprintln!("this git has no --ref-format=reftable: nothing checked");
        return;
    };
    let manifest = repo.join("runbook");
    let c = commit_source::commit(&manifest);
    assert_eq!(c.value, head12(&repo));
    let before = newest(&c.watch);
    later();
    fs::write(repo.join("note.txt"), "x").unwrap();
    git(&repo, &["add", "note.txt"]);
    git(&repo, &["commit", "-q", "-m", "second"]);
    assert_ne!(head12(&repo), c.value, "HEAD moved");
    assert!(newest(&c.watch) > before, "HEAD moved from {} to {} and no watched path changed: {:?}", c.value, head12(&repo), c.watch);
}
