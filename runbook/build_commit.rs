//! The commit a build is made from (`runbook --version`) and the files whose change means that the commit moved.
//! Used by `build.rs`, and by `tests/build_commit/mod.rs`, which feeds it repositories in the states a checkout can be in.

use std::path::{Path, PathBuf};

pub struct Commit {
    /// The first 12 characters of HEAD, or `unknown`.
    pub value: String,
    /// Files and directories that exist now and change when HEAD moves: `build.rs` prints a `rerun-if-changed`
    /// line for each. A path that does not exist is never in the list: cargo rebuilds the package on every call
    /// for a `rerun-if-changed` path that is missing.
    pub watch: Vec<PathBuf>,
}

/// What `git` prints for `args` in `dir`, or `None` outside a git checkout, without git, or on any error. The
/// environment cannot point git at another repository: `GIT_DIR`, `GIT_WORK_TREE` and `GIT_INDEX_FILE` are removed.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// The commit of the repository that tracks this crate, or `unknown`: the value counts only when `Cargo.toml` of
/// the manifest directory is tracked there, so a source tree unpacked inside another repository gives `unknown`.
/// A dirty tree is not marked (it would cost a rebuild on every edit): the value is HEAD of the build.
///
/// How a move of HEAD is noticed, whatever state the repository is in: `HEAD` (a switch, a detached commit), the
/// directory `refs/heads` (a commit on a branch writes its loose ref there, also after `git pack-refs`, which
/// empties it) and `packed-refs` when the file exists. All three are named by `git rev-parse --git-path`, so in a
/// linked worktree they are that worktree's `HEAD` and the common repository's refs. A tree outside git, or one
/// where `Cargo.toml` is not tracked, watches nothing: a `.git` that appears later is not noticed until another
/// input changes, because the place it would appear in does not exist, and a path that does not exist makes cargo
/// rebuild on every call.
pub fn commit(manifest: &Path) -> Commit {
    let unknown = Commit {
        value: "unknown".to_owned(),
        watch: Vec::new(),
    };
    if git(manifest, &["ls-files", "--error-unmatch", "Cargo.toml"]).is_none() {
        return unknown;
    }
    let watch: Vec<PathBuf> = ["HEAD", "refs/heads", "packed-refs"]
        .iter()
        .filter_map(|name| git(manifest, &["rev-parse", "--git-path", name]))
        .map(|p| manifest.join(p))
        .filter(|p| p.exists())
        .collect();
    match git(manifest, &["rev-parse", "HEAD"]) {
        Some(head) => Commit {
            value: head.chars().take(12).collect(),
            watch,
        },
        None => Commit { watch, ..unknown },
    }
}
