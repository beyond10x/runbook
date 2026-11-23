//! The commit `runbook --version` prints (`build_commit.rs`, used by `build.rs`) in every state a checkout can be
//! in: loose refs (a fresh `git fetch` checkout), packed refs (after `git pack-refs` or `git gc`), a linked
//! worktree, a detached HEAD, a tree outside git and a source tree unpacked inside another repository.
//!
//! Two things are held. Every path the build script is told to watch exists, because cargo rebuilds the package
//! on every call for a `rerun-if-changed` path that is missing. And a move of HEAD (a commit, an amend, a
//! switch, a detach, a commit after a pack) changes the newest time among the paths that were watched before it,
//! which is what cargo compares.

#[allow(dead_code)]
#[path = "../../build_commit.rs"]
mod commit_source;

mod other_refs;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

type Move<'a> = (&'a str, Box<dyn Fn() + 'a>);

fn scratch(tag: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("build-commit-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// `git <args>` in `dir`, with an identity and no signing of its own; the trimmed output.
fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.org",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8_lossy(&o.stdout).trim().to_owned()
}

/// A repository whose `runbook/Cargo.toml` is tracked, with one commit and loose refs only.
fn repository(tag: &str) -> PathBuf {
    let repo = scratch(tag);
    fs::create_dir_all(repo.join("runbook")).unwrap();
    fs::write(repo.join("runbook/Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["add", "runbook/Cargo.toml"]);
    git(&repo, &["commit", "-q", "-m", "first"]);
    repo
}

fn commit_more(dir: &Path, name: &str) {
    fs::write(dir.join(name), name).unwrap();
    git(dir, &["add", name]);
    git(dir, &["commit", "-q", "-m", name]);
}

/// The newest modification time among the paths and everything below the directories among them: what cargo
/// compares with the time of the last build.
fn newest(paths: &[PathBuf]) -> SystemTime {
    fn walk(p: &Path, best: &mut SystemTime) {
        let Ok(m) = fs::symlink_metadata(p) else {
            return;
        };
        *best = (*best).max(m.modified().unwrap());
        if m.is_dir() {
            for e in fs::read_dir(p).unwrap().flatten() {
                walk(&e.path(), best);
            }
        }
    }
    let mut best = SystemTime::UNIX_EPOCH;
    for p in paths {
        walk(p, &mut best);
    }
    best
}

fn head12(dir: &Path) -> String {
    git(dir, &["rev-parse", "HEAD"]).chars().take(12).collect()
}

fn later() {
    std::thread::sleep(Duration::from_millis(60));
}

/// The commit is HEAD, and every watched path exists and includes `HEAD` and the refs of the branch.
fn assert_watches_what_exists(manifest: &Path, state: &str) -> commit_source::Commit {
    let c = commit_source::commit(manifest);
    assert_eq!(c.value, head12(manifest), "{state}: the value is HEAD");
    assert!(
        c.watch.len() >= 2,
        "{state}: HEAD and refs/heads are watched: {:?}",
        c.watch
    );
    for p in &c.watch {
        assert!(
            p.exists(),
            "{state}: {} is watched and does not exist: cargo would rebuild on every call",
            p.display()
        );
    }
    c
}

#[test]
fn every_watched_path_exists_with_loose_refs_packed_refs_a_linked_worktree_and_a_detached_head() {
    let repo = repository("exists");
    let manifest = repo.join("runbook");
    let c = assert_watches_what_exists(&manifest, "loose refs");
    assert!(
        !repo.join(".git/packed-refs").exists(),
        "the fixture starts without packed-refs"
    );
    assert!(
        !c.watch.iter().any(|p| p.ends_with("packed-refs")),
        "packed-refs is not watched where it is not there"
    );

    git(&repo, &["pack-refs", "--all"]);
    assert!(repo.join(".git/packed-refs").exists());
    assert!(
        !repo.join(".git/refs/heads/main").exists(),
        "pack-refs removed the loose ref"
    );
    let c = assert_watches_what_exists(&manifest, "packed refs");
    assert!(
        !c.watch.iter().any(|p| p.ends_with("packed-refs")),
        "packed-refs is not watched while HEAD has a reflog: a pack changes no HEAD"
    );

    let linked = scratch("exists-linked").join("tree");
    git(
        &repo,
        &[
            "worktree",
            "add",
            "-q",
            linked.to_str().unwrap(),
            "-b",
            "other",
        ],
    );
    let c = assert_watches_what_exists(&linked.join("runbook"), "linked worktree");
    assert!(
        c.watch
            .iter()
            .any(|p| p.components().any(|k| k.as_os_str() == "worktrees")),
        "the worktree's own HEAD is watched: {:?}",
        c.watch
    );

    git(&repo, &["checkout", "-q", "--detach"]);
    assert_watches_what_exists(&manifest, "detached HEAD");
}

#[test]
fn a_move_of_head_changes_what_was_watched_before_it_in_every_state() {
    let repo = repository("moves");
    let manifest = repo.join("runbook");
    let moves: Vec<Move> = vec![
        (
            "a commit with loose refs",
            Box::new(|| commit_more(&repo, "a")),
        ),
        (
            "an amend",
            Box::new(|| {
                git(&repo, &["commit", "-q", "--amend", "-m", "amended"]);
            }),
        ),
        (
            "a commit after pack-refs",
            Box::new(|| {
                git(&repo, &["pack-refs", "--all"]);
                later();
                commit_more(&repo, "b");
            }),
        ),
        (
            "a second commit on the packed branch",
            Box::new(|| commit_more(&repo, "c")),
        ),
        (
            "a switch to a new branch and a commit",
            Box::new(|| {
                git(&repo, &["switch", "-q", "-c", "feature/x"]);
                commit_more(&repo, "d");
            }),
        ),
        (
            "a detach",
            Box::new(|| {
                git(&repo, &["checkout", "-q", "--detach", "HEAD~1"]);
            }),
        ),
        (
            "a commit on the detached HEAD",
            Box::new(|| commit_more(&repo, "e")),
        ),
        (
            "a switch back to a branch",
            Box::new(|| {
                git(&repo, &["switch", "-q", "main"]);
            }),
        ),
    ];
    for (what, act) in moves {
        let before = commit_source::commit(&manifest);
        let t0 = newest(&before.watch);
        later();
        act();
        let after = commit_source::commit(&manifest);
        assert_ne!(after.value, before.value, "{what}: HEAD moved");
        assert!(
            newest(&before.watch) > t0,
            "{what}: nothing the build watched changed, so cargo would keep the old commit ({:?})",
            before.watch
        );
        assert!(
            after.watch.iter().all(|p| p.exists()),
            "{what}: {:?}",
            after.watch
        );
    }

    let linked = scratch("moves-linked").join("tree");
    git(
        &repo,
        &[
            "worktree",
            "add",
            "-q",
            linked.to_str().unwrap(),
            "-b",
            "other",
        ],
    );
    let before = commit_source::commit(&linked.join("runbook"));
    let t0 = newest(&before.watch);
    later();
    commit_more(&linked, "f");
    assert_ne!(
        commit_source::commit(&linked.join("runbook")).value,
        before.value
    );
    assert!(
        newest(&before.watch) > t0,
        "a commit in a linked worktree changes nothing watched: {:?}",
        before.watch
    );
}

#[test]
fn a_tree_outside_git_or_unpacked_inside_another_repository_gives_unknown_and_watches_nothing() {
    let outside = scratch("outside");
    fs::create_dir_all(outside.join("runbook")).unwrap();
    fs::write(outside.join("runbook/Cargo.toml"), "").unwrap();
    let c = commit_source::commit(&outside.join("runbook"));
    assert_eq!(
        (c.value.as_str(), c.watch.len()),
        ("unknown", 0),
        "outside git"
    );

    let other = repository("enclosing");
    fs::create_dir_all(other.join("unpacked/runbook")).unwrap();
    fs::write(other.join("unpacked/runbook/Cargo.toml"), "").unwrap();
    let c = commit_source::commit(&other.join("unpacked/runbook"));
    assert_eq!(
        (c.value.as_str(), c.watch.len()),
        ("unknown", 0),
        "a tree whose Cargo.toml the enclosing repository does not track"
    );

    let tracked_elsewhere = other.join("runbook");
    assert_eq!(
        commit_source::commit(&tracked_elsewhere).value,
        head12(&other),
        "a tracked one is that repository's commit"
    );
}

#[test]
fn git_environment_variables_cannot_point_the_commit_at_another_repository() {
    let other = repository("env-other");
    let outside = scratch("env-outside");
    fs::create_dir_all(outside.join("runbook")).unwrap();
    fs::write(outside.join("runbook/Cargo.toml"), "").unwrap();
    // nextest runs every test in a process of its own, so the variables stay in this one
    std::env::set_var("GIT_DIR", other.join(".git"));
    std::env::set_var("GIT_WORK_TREE", &other);
    std::env::set_var("GIT_INDEX_FILE", other.join(".git/index"));
    let c = commit_source::commit(&outside.join("runbook"));
    let tracked = commit_source::commit(&other.join("runbook"));
    std::env::remove_var("GIT_DIR");
    std::env::remove_var("GIT_WORK_TREE");
    std::env::remove_var("GIT_INDEX_FILE");
    assert_eq!(c.value, "unknown", "GIT_DIR of another repository");
    assert_eq!(tracked.value, head12(&other));
}
