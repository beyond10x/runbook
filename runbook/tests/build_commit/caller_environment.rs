//! What the environment of the caller does not change in the commit a build is made from.

use super::*;

/// `GIT_CEILING_DIRECTORIES` at the repository root tells git not to look in the root for a repository, so a git run in
/// `runbook/` finds none and the commit was `unknown`. The calls drop it, with the other variables that point git
/// elsewhere. The variable is set in this process only, for a repository of this test alone.
#[test]
fn a_ceiling_directory_at_the_repository_root_does_not_make_the_commit_unknown() {
    let repo = repository("ceiling");
    let manifest = repo.join("runbook");
    std::env::set_var("GIT_CEILING_DIRECTORIES", &repo);
    let c = commit_source::commit(&manifest);
    std::env::remove_var("GIT_CEILING_DIRECTORIES");
    assert_eq!(c.value, head12(&repo));
    assert!(!c.watch.is_empty(), "{:?}", c.watch);
}

/// `git commit` sets `GIT_INDEX_FILE` for the hooks it runs. A build started from a hook (a pre-commit check that runs
/// `cargo build`) with an index that does not hold `runbook/Cargo.toml` gave the commit `unknown` if the variable
/// reached git: the calls drop it. The variable is set in this process only, for a repository of this test alone.
#[test]
fn an_index_file_of_the_callers_environment_does_not_make_the_commit_unknown() {
    let repo = repository("index-file");
    let manifest = repo.join("runbook");
    std::env::set_var("GIT_INDEX_FILE", repo.join("an-index-of-a-hook"));
    let c = commit_source::commit(&manifest);
    std::env::remove_var("GIT_INDEX_FILE");
    assert_eq!(c.value, head12(&repo));
    assert!(!c.watch.is_empty(), "{:?}", c.watch);
}
