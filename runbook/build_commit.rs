//! The commit a build is made from (`runbook --version`) and the files whose change means that the commit moved.
//! Used by `build.rs`, and by `tests/build_commit/mod.rs`, which feeds it repositories in the states a checkout can be in.

use std::path::{Path, PathBuf};

pub struct Commit {
    /// The first 12 characters of HEAD, or `unknown`.
    pub value: String,
    /// Files that exist now and change when HEAD moves and at no other time: `build.rs` prints a
    /// `rerun-if-changed` line for each. A path that does not exist is never in the list: cargo rebuilds the package
    /// on every call for a `rerun-if-changed` path that is missing.
    pub watch: Vec<PathBuf>,
}

/// The variables that point git at another repository, index, object store or ceiling than the one of the directory
/// it is run in: none of them reaches the git calls below.
const GIT_ENVIRONMENT: [&str; 12] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_NAMESPACE",
    "GIT_CEILING_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_PARAMETERS",
    "GIT_PREFIX",
];

/// What `git` prints for `args` in `dir`, or `None` outside a git checkout, without git, or on any error.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = std::process::Command::new("git");
    cmd.args(args).current_dir(dir);
    for name in GIT_ENVIRONMENT {
        cmd.env_remove(name);
    }
    let out = cmd.output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// The path git names for `name` of this work tree (`HEAD` and `logs/HEAD` are the linked work tree's own), when
/// it exists.
fn existing(manifest: &Path, name: &str) -> Option<PathBuf> {
    git(manifest, &["rev-parse", "--git-path", name])
        .map(|p| manifest.join(p))
        .filter(|p| p.exists())
}

/// The commit of the repository that tracks this crate, or `unknown`: the value counts only when `Cargo.toml` of
/// the manifest directory is tracked there, so a source tree unpacked inside another repository that does not track
/// it gives `unknown`. A dirty tree is not marked (it would cost a rebuild on every edit): the value is HEAD of the
/// build.
///
/// How a move of HEAD is noticed: two files of this work tree change when, and only when, its HEAD moves. `HEAD`
/// (a switch, a detached commit) and `logs/HEAD`, the reflog of HEAD, which every commit, amend, reset and checkout
/// of this work tree appends to, in a linked work tree too (it has its own). A branch made elsewhere, a commit on
/// another branch in another work tree and `git pack-refs` change neither, so none of them rebuilds the crate.
/// Without a reflog (`core.logAllRefUpdates=false`) the directory `refs/heads` and `packed-refs` stand in, as before:
/// there a branch made elsewhere or a pack costs a rebuild, which is the price of having no log. A move of the branch
/// HEAD names made from another work tree (`git update-ref`, `git push .`) appends to no reflog of this one and is not
/// noticed; `scripts/install.sh` compares the commit of the binary with HEAD and stops on the difference.
///
/// A repository with the reftable format keeps no such files (`HEAD` is a stub and the log is inside the tables):
/// its commit is `unknown` and nothing is watched, so a binary built there says `unknown` and `scripts/install.sh`
/// refuses it for the checkout's real HEAD. A tree outside git, or one where `Cargo.toml` is not tracked, watches
/// nothing either: a `.git` that appears later is not noticed until another input changes, because the place it
/// would appear in does not exist, and a path that does not exist makes cargo rebuild on every call.
pub fn commit(manifest: &Path) -> Commit {
    let unknown = Commit {
        value: "unknown".to_owned(),
        watch: Vec::new(),
    };
    if git(manifest, &["ls-files", "--error-unmatch", "Cargo.toml"]).is_none() {
        return unknown;
    }
    if git(manifest, &["rev-parse", "--show-ref-format"]).as_deref() == Some("reftable") {
        return unknown;
    }
    let mut watch: Vec<PathBuf> = existing(manifest, "HEAD").into_iter().collect();
    match existing(manifest, "logs/HEAD") {
        Some(log) => watch.push(log),
        None => watch.extend(["refs/heads", "packed-refs"].iter().filter_map(|name| existing(manifest, name))),
    }
    match git(manifest, &["rev-parse", "HEAD"]) {
        Some(head) => Commit {
            value: head.chars().take(12).collect(),
            watch,
        },
        None => Commit { watch, ..unknown },
    }
}
