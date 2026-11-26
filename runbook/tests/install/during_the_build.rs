//! The checkout changes while the build runs: a commit, a branch switch, a tracked file replaced by a link, and a link
//! at the place of the build log. A stub `cargo build` runs a shell file of the test once, in the checkout, before it
//! "builds". Fake checkouts; no real build, no network; every prefix lies under the target directory.

use super::*;

/// A stub `cargo build` like the one of `stale_build.rs`, with one addition: before the first crate is "built" it runs
/// the shell file `$DURING` once with the checkout as its argument, which is what another process does in the checkout
/// while the build runs. Every program it writes prints HEAD as it is at that moment, as a build of `runbook` does
/// whose build script runs after the change. With `$DURING_LAST` set the file runs instead after the programs of the
/// crate `runbook` are written, which print HEAD as it was before: a change after the binary was built.
const CARGO_DURING: &str = r#"#!/bin/sh
to=
before=
for a in "$@"; do
    [ "$before" = --target-dir ] && to=$a
    before=$a
done
root=${to%/*/target}
late=
case " $* " in *" runbook "*) [ -n "${DURING_LAST:-}" ] && late=1 ;; esac
if [ -n "${DURING:-}" ] && [ -z "${DURING_LAST:-}" ] && [ ! -e "$root/../during-done" ]; then
    : > "$root/../during-done"
    sh "$DURING" "$root" >&2
fi
commit=$(git -C "$root" rev-parse HEAD | cut -c1-12)
mkdir -p "$to/release"
before=
for a in "$@"; do
    if [ "$before" = --bin ]; then
        printf '#!/bin/sh\necho "runbook 9.9.9 (%s)"\n' "$commit" > "$to/release/$a"
        chmod 755 "$to/release/$a"
        printf '{"reason":"compiler-artifact","executable":"%s"}\n' "$to/release/$a"
    fi
    before=$a
done
if [ -n "$late" ] && [ -n "${DURING:-}" ]; then
    sh "$DURING" "$root" >&2
fi
"#;

/// The release install of a fake checkout with the stub above on `PATH`; `during` is the shell text run in the checkout
/// while the first crate builds.
fn release_install_during(root: &Path, sandbox: &Path, prefix: &Path, during: &str) -> Output {
    release_install_during_at(root, sandbox, prefix, during, false)
}

/// The same; with `last` the text runs after the binary of `runbook`, the last crate, is built.
fn release_install_during_at(root: &Path, sandbox: &Path, prefix: &Path, during: &str, last: bool) -> Output {
    executable(&sandbox.join("stub-bin/cargo"), CARGO_DURING);
    fs::write(sandbox.join("during.sh"), during).unwrap();
    let mut cmd = Command::new("sh");
    cmd.arg(root.join("scripts/install.sh"))
        .arg("--prefix")
        .arg(prefix)
        .env_clear().envs(crate::kept()).envs(NO_SYSTEM_GIT)
        .env("PATH", format!("{}:{SYSTEM_PATH}", sandbox.join("stub-bin").display()))
        .env("HOME", sandbox.join("home"))
        .env("DURING", sandbox.join("during.sh"))
        .stdin(Stdio::null());
    if last {
        cmd.env("DURING_LAST", "1");
    }
    cmd.output().unwrap()
}

const GIT_AS_TEST: &str = "git -c user.name=test -c user.email=test@example.org -c commit.gpgsign=false";

// ---- the checkout changes while the build runs ----

/// The script lists the files to copy and their modes before the build and copies after it. A commit made in the
/// checkout while `watch/` or `driver/` builds (minutes of a release build) builds `runbook` from the new HEAD: the
/// prefix must never name the new HEAD in `VERSION`, without ` dirty`, with the file list and the modes of the old one
/// (a file the new HEAD tracks missing, a file it tracks as executable at 644).
#[test]
fn a_commit_made_while_the_build_runs_never_gives_a_prefix_that_names_the_new_head_with_the_files_of_the_old_one() {
    let root = fake_checkout("commit-during", None);
    let (sandbox, prefix) = sandbox_of(&root);
    let before = head12(&root);
    let during = format!(
        "r=$1\n\
        echo 'new in HEAD' > \"$r/protocol/new-in-head.txt\"\n\
        chmod 755 \"$r/tools/file.txt\"\n\
        git -C \"$r\" add protocol/new-in-head.txt tools/file.txt\n\
        {GIT_AS_TEST} -C \"$r\" commit -q -m 'made while the build runs'\n"
    );
    let o = release_install_during(&root, &sandbox, &prefix, &during);
    let head = head12(&root);
    assert_ne!(before, head, "the commit was made while the build ran: {}", err(&o));
    let version = fs::read_to_string(prefix.join("share/runbook/VERSION")).unwrap_or_default();
    let share = prefix.join("share/runbook");
    let missing = !share.join("protocol/new-in-head.txt").exists();
    let mode = fs::metadata(share.join("tools/file.txt")).map(|m| m.permissions().mode() & 0o777).unwrap_or(0);
    assert!(
        !(code(&o) == 0 && version.contains(&head) && !version.contains("dirty") && (missing || mode != 0o755)),
        "exit 0 and VERSION {:?} (HEAD moved from {before} to {head} during the build): protocol/new-in-head.txt, tracked by {head}, in the prefix: {}; tools/file.txt, 100755 in {head}, has mode {mode:o}",
        version.trim(),
        !missing
    );
}

/// The same window, another change: a tracked file is replaced by a link to a file outside the checkout while the build
/// runs. The check "is in the work tree as a file" is made again right before the copy: the bytes of the file outside
/// the checkout are never in the prefix.
#[test]
fn a_tracked_file_that_becomes_a_link_while_the_build_runs_is_not_copied_from_outside_the_checkout() {
    let root = fake_checkout("link-during", None);
    let (sandbox, prefix) = sandbox_of(&root);
    fs::write(sandbox.join("login.json"), "{\"token\": \"a login outside the checkout\"}\n").unwrap();
    let during = "rm -f \"$1/protocol/file.txt\"\nln -s \"$1/../login.json\" \"$1/protocol/file.txt\"\n";
    let o = release_install_during(&root, &sandbox, &prefix, during);
    let copied = fs::read_to_string(prefix.join("share/runbook/protocol/file.txt")).unwrap_or_default();
    assert!(
        !copied.contains("a login outside the checkout"),
        "exit {}: share/runbook/protocol/file.txt holds the bytes of a file outside the checkout: {copied:?}",
        code(&o)
    );
}

/// The build writes what cargo reports to `<crate>/target/install-build.json`. A link left at that name is removed
/// first, not written through.
#[test]
fn a_link_at_the_place_of_the_build_log_is_not_written_through() {
    let root = fake_checkout("build-log", None);
    let (sandbox, prefix) = sandbox_of(&root);
    let outside = sandbox.join("outside.txt");
    fs::write(&outside, "kept\n").unwrap();
    for krate in ["watch", "driver", "runbook"] {
        fs::create_dir_all(root.join(krate).join("target")).unwrap();
        let log = root.join(krate).join("target/install-build.json");
        let _ = fs::remove_file(&log);
        std::os::unix::fs::symlink(&outside, &log).unwrap();
    }
    let o = release_install_during(&root, &sandbox, &prefix, "");
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert_eq!(fs::read_to_string(&outside).unwrap(), "kept\n", "the file the links pointed at");
}

/// The same window as above, and the rule the script keeps: the commit it read before it took the list of files is the
/// commit it installs. HEAD read again after the build is another: the install stops with exit 1, names both commits,
/// and writes nothing.
#[test]
fn a_commit_made_while_the_build_runs_stops_the_install_naming_both_commits_and_writes_nothing() {
    let root = fake_checkout("during-commit-named", None);
    let (sandbox, prefix) = sandbox_of(&root);
    let before = head12(&root);
    let during = format!(
        "r=$1\n\
        echo 'new in HEAD' > \"$r/protocol/new-in-head.txt\"\n\
        git -C \"$r\" add protocol/new-in-head.txt\n\
        {GIT_AS_TEST} -C \"$r\" commit -q -m 'made while the build runs'\n"
    );
    let o = release_install_during(&root, &sandbox, &prefix, &during);
    let head = head12(&root);
    assert_ne!(before, head, "the commit was made while the build ran: {}", err(&o));
    assert_eq!(code(&o), 1, "{}", err(&o));
    assert!(err(&o).contains(&before) && err(&o).contains(&head), "both commits are named: {}", err(&o));
    assert_eq!(listing(&prefix), Vec::<String>::new(), "nothing is written");
}

/// A tracked file that is a link when it is about to be copied, and was a file when the list was taken, stops the
/// install with its name. The marker stays: the install had begun.
#[test]
fn a_file_that_changed_kind_while_the_build_runs_stops_the_install_with_its_name_and_the_marker_stays() {
    let root = fake_checkout("during-kind-named", None);
    let (sandbox, prefix) = sandbox_of(&root);
    fs::write(sandbox.join("outside.txt"), "outside\n").unwrap();
    let during = "rm -f \"$1/protocol/file.txt\"\nln -s \"$1/../outside.txt\" \"$1/protocol/file.txt\"\n";
    let o = release_install_during(&root, &sandbox, &prefix, during);
    assert_eq!(code(&o), 1, "{}", err(&o));
    assert!(err(&o).contains("protocol/file.txt"), "the file is named: {}", err(&o));
    assert!(prefix.join("share/runbook/INSTALLING").is_file(), "the marker stays: {}", err(&o));
    assert!(!prefix.join("share/runbook/protocol/file.txt").exists(), "nothing is copied from outside the checkout");
}

/// The same for a directory above a tracked file: replaced by a link while the build runs, the file below it is named
/// and not copied.
#[test]
fn a_directory_that_became_a_link_while_the_build_runs_stops_the_install_with_the_file_below_it_and_the_marker_stays() {
    let root = fake_checkout("during-directory-named", None);
    let (sandbox, prefix) = sandbox_of(&root);
    fs::create_dir_all(root.join("protocol/sub")).unwrap();
    fs::write(root.join("protocol/sub/x.txt"), "tracked\n").unwrap();
    git_in(&root, &["add", "-A", "protocol"]);
    git_in(&root, &["commit", "-q", "-m", "a directory"]);
    fs::create_dir_all(sandbox.join("outside")).unwrap();
    fs::write(sandbox.join("outside/x.txt"), "outside\n").unwrap();
    let during = "rm -rf \"$1/protocol/sub\"\nln -s \"$1/../outside\" \"$1/protocol/sub\"\n";
    let o = release_install_during(&root, &sandbox, &prefix, during);
    assert_eq!(code(&o), 1, "{}", err(&o));
    assert!(err(&o).contains("protocol/sub/x.txt"), "the file below the link is named: {}", err(&o));
    assert!(prefix.join("share/runbook/INSTALLING").is_file(), "the marker stays: {}", err(&o));
    assert!(!prefix.join("share/runbook/protocol/sub/x.txt").exists(), "nothing is copied from outside the checkout");
}

/// A commit made after the binary of `runbook` was built: the binary prints the commit it was built from, which is the
/// commit the script read first, and HEAD is another. Only the second read of HEAD sees it: the install stops with both
/// commits named and writes nothing.
#[test]
fn a_commit_made_after_the_binary_was_built_stops_the_install_naming_both_commits_and_writes_nothing() {
    let root = fake_checkout("after-the-binary", None);
    let (sandbox, prefix) = sandbox_of(&root);
    let before = head12(&root);
    let during = format!(
        "r=$1\n\
        echo 'new in HEAD' > \"$r/protocol/new-in-head.txt\"\n\
        git -C \"$r\" add protocol/new-in-head.txt\n\
        {GIT_AS_TEST} -C \"$r\" commit -q -m 'made after the binary was built'\n"
    );
    let o = release_install_during_at(&root, &sandbox, &prefix, &during, true);
    let head = head12(&root);
    assert_ne!(before, head, "the commit was made after the binary was built: {}", err(&o));
    assert_eq!(code(&o), 1, "{}", err(&o));
    assert!(err(&o).contains(&before) && err(&o).contains(&head), "both commits are named: {}", err(&o));
    assert_eq!(listing(&prefix), Vec::<String>::new(), "nothing is written");
}
