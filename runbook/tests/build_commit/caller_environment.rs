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
