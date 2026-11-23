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

/// A branch made in another work tree, a commit on another branch of the repository and `git pack-refs --all` move no
/// HEAD of this work tree: none of the paths the build watches changes, so cargo does not compile the package again
/// (seen with the real cargo on a scratch crate). Each state is a step on a linked work tree whose HEAD stays.
#[test]
fn the_watched_paths_stay_as_they_are_when_another_branch_moves_and_head_does_not() {
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

/// A repository with the reftable format keeps `HEAD` as a fixed stub and its log inside the tables, so no file shows
/// that HEAD moved. The commit there is never a stale one: it is `unknown` (and nothing is watched), or it is HEAD and a
/// watched path changes when HEAD moves. `scripts/install.sh` refuses a binary that says `unknown` for a checkout
/// whose HEAD is a commit.
#[test]
fn a_reftable_repository_never_gives_a_stale_commit() {
    let Some(repo) = repository_with("fix2-reftable", &["init", "-q", "--ref-format=reftable"]) else {
        eprintln!("this git has no --ref-format=reftable: nothing checked");
        return;
    };
    let manifest = repo.join("runbook");
    let c = commit_source::commit(&manifest);
    let before = newest(&c.watch);
    later();
    fs::write(repo.join("note.txt"), "x").unwrap();
    git(&repo, &["add", "note.txt"]);
    git(&repo, &["commit", "-q", "-m", "second"]);
    let head = head12(&repo);
    let after = commit_source::commit(&manifest);
    assert_eq!(after.value, "unknown", "the commit of a reftable repository is unknown (it was {} and HEAD is {head})", c.value);
    assert!(c.watch.is_empty() && after.watch.is_empty(), "nothing is watched: {:?} {:?}", c.watch, after.watch);
    assert_eq!(newest(&after.watch), before);
}

/// What the build watches in an ordinary repository is `HEAD` and its reflog and nothing of `refs/`: a branch, a pack of
/// refs or another work tree's commit cannot touch them.
#[test]
fn the_build_watches_head_and_its_log_and_no_ref() {
    let repo = repository_with("fix2-watch-set", &["init", "-q"]).unwrap();
    let c = commit_source::commit(&repo.join("runbook"));
    let names: Vec<String> = c.watch.iter().map(|p| p.canonicalize().unwrap().strip_prefix(repo.join(".git").canonicalize().unwrap()).unwrap().display().to_string()).collect();
    assert_eq!(names, ["HEAD", "logs/HEAD"]);
}

/// Without a reflog (`core.logAllRefUpdates=false`) the old set stands in: `refs/heads` (a commit writes its loose ref there, also
/// after `git pack-refs` emptied it) and `packed-refs` once it exists. Every watched path exists after the pack.
#[test]
fn without_a_reflog_the_ref_directory_is_watched_and_after_pack_refs_the_packed_file_too() {
    let repo = scratch("fix2-no-reflog");
    fs::create_dir_all(repo.join("runbook")).unwrap();
    fs::write(repo.join("runbook/Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "core.logAllRefUpdates", "false"]);
    git(&repo, &["add", "runbook/Cargo.toml"]);
    git(&repo, &["commit", "-q", "-m", "first"]);
    let manifest = repo.join("runbook");
    let c = commit_source::commit(&manifest);
    let names: Vec<String> = c.watch.iter().map(|p| p.canonicalize().unwrap().strip_prefix(repo.join(".git").canonicalize().unwrap()).unwrap().display().to_string()).collect();
    assert_eq!(names, ["HEAD", "refs/heads"], "no logs/HEAD without a reflog");
    let before = newest(&c.watch);
    later();
    commit_more(&repo, "b");
    assert!(newest(&c.watch) > before, "a commit changes the ref directory");
    git(&repo, &["pack-refs", "--all"]);
    let packed = commit_source::commit(&manifest);
    assert_eq!(packed.value, head12(&repo));
    assert!(packed.watch.iter().all(|p| p.exists()), "{:?}", packed.watch);
    assert!(packed.watch.iter().any(|p| p.ends_with("packed-refs")), "{:?}", packed.watch);
    let before = newest(&packed.watch);
    later();
    commit_more(&repo, "c");
    assert!(newest(&packed.watch) > before, "a commit on a packed branch writes its loose ref again");
}
