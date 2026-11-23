//! What `scripts/install.sh --help` says about what the script does and does not do. Each sentence is held by a case
//! of its own elsewhere in this directory; these cases hold that the sentence is in the text.

use super::*;

fn help() -> String {
    let o = Command::new("sh").arg(script()).arg("--help").env_clear().env("PATH", SYSTEM_PATH).stdin(Stdio::null()).output().unwrap();
    assert_eq!(code(&o), 0);
    out(&o).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The marker: `share` and `share/runbook` are made first, then the marker, then every other change. A stop in the first
/// step leaves two empty directories and no marker (`half_install.rs`).
#[test]
fn the_help_says_the_directories_come_first_then_the_marker_then_every_other_change() {
    let text = help();
    assert!(text.contains("share and share/runbook are made first, then the marker, then every other change"), "{text}");
    assert!(!text.contains("from before the first change"), "{text}");
}

/// What an install from an archive lacks, a branch moved from another work tree, and a reftable checkout
/// (`rules.rs`, `during_the_build.rs`, `stale_build.rs`).
#[test]
fn the_help_names_what_an_archive_install_lacks_a_branch_moved_elsewhere_and_a_reftable_checkout() {
    let text = help();
    assert!(text.contains("install from an archive copies no link and no dot file and skips runs/, target/ and node_modules/, so it lacks files a git checkout has"), "{text}");
    assert!(text.contains("branch was moved from another work tree a release install is refused until an input of the build changes"), "{text}");
    assert!(text.contains("reftable checkout cannot be installed"), "{text}");
}

/// The mark covers the checkout, and a sparse checkout is named as such.
#[test]
fn the_help_says_dirty_covers_every_tracked_file_of_the_checkout_and_names_a_sparse_checkout() {
    let text = help();
    assert!(text.contains("VERSION ends in \" dirty\" when any tracked file of the checkout differs from HEAD (index or work tree)"), "{text}");
    assert!(text.contains("a sparse checkout"), "{text}");
}
