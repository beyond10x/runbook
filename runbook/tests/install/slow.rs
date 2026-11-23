//! The slow lane of the install tests (`task test:install`, not part of `task check`): the prefix that the task
//! made with the real `--release` build (`scripts/install.sh --prefix <dir>`, no `--from-built`), named by
//! `RUNBOOK_INSTALLED_PREFIX`, and the same three commands as the fast lane: `runbook list`, `runbook doctor`
//! and one run with sh programs only.

use std::path::PathBuf;

fn prefix() -> PathBuf {
    let p = std::env::var_os("RUNBOOK_INSTALLED_PREFIX").unwrap_or_else(|| panic!("RUNBOOK_INSTALLED_PREFIX names the prefix the task installed into: run `task test:install`"));
    PathBuf::from(p)
}

#[test]
#[ignore = "slow lane: task test:install builds the release binaries"]
fn the_release_build_installed_lists_the_protocols_from_an_empty_directory() {
    super::list_from(&prefix());
}

#[test]
#[ignore = "slow lane: task test:install builds the release binaries"]
fn the_release_build_installed_has_its_assets_and_helpers_in_the_prefix() {
    super::doctor_from(&prefix());
}

#[test]
#[ignore = "slow lane: task test:install builds the release binaries"]
fn a_run_from_the_release_build_installed_ends_as_from_the_checkout_build() {
    super::run_from(&prefix());
}
