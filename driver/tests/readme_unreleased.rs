//! The status line of `README.md` and the `[Unreleased]` section of `CHANGELOG.md` say the same thing: a README
//! whose line of the latest release (the first `- New in`) says "nothing is unreleased" goes with an empty
//! `[Unreleased]` section. The lines of older releases keep the words they had.

use std::fs;
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

#[test]
fn the_readme_says_nothing_is_unreleased_only_when_the_changelog_has_nothing_unreleased() {
    let changelog = fs::read_to_string(repo().join("CHANGELOG.md")).unwrap();
    let unreleased = changelog.split("\n## [Unreleased]\n").nth(1).unwrap().split("\n## [").next().unwrap();
    let entries = unreleased.lines().filter(|l| l.starts_with("- ")).count();
    let readme = fs::read_to_string(repo().join("README.md")).unwrap();
    let latest = readme.lines().find(|l| l.starts_with("- New in ")).expect("README.md has a line `- New in <version>`");
    assert!(
        entries == 0 || !latest.contains("(nothing is unreleased)"),
        "CHANGELOG.md [Unreleased] holds {entries} entries, README.md says: {latest}"
    );
}
