//! The checkout changes after the build, while the script copies: a link made of a file, of a directory or of a tracked
//! link, and a commit. A stub of `mkdir` or `cp` runs a shell text of the test once, at the call the case names, with
//! the checkout as `$1` (what another process does in the checkout at that moment), and then runs the real program.
//! Fake checkouts and stub programs; no build, no model call, no network; every prefix lies under the target
//! directory.
//!
//! `measurement_*` cases are `#[ignore]`d: each states the outcome a window of the script still gives, names the story
//! that closes it, and fails until that story lands.

use super::*;

/// A stub of `program` for `<sandbox>/stub-bin`: at its `nth` call it runs the shell text `during` once, with the
/// checkout as `$1`, then it runs the real program.
fn hook(sandbox: &Path, root: &Path, program: &str, nth: u32, during: &str) -> String {
    fs::write(sandbox.join("hook.sh"), during).unwrap();
    let s = sandbox.display();
    executable(
        &sandbox.join("stub-bin").join(program),
        &format!(
            "#!/bin/sh\n\
            n=$(cat '{s}/hook-count' 2>/dev/null || echo 0)\n\
            n=$((n + 1))\n\
            echo $n > '{s}/hook-count'\n\
            if [ $n -eq {nth} ]; then /bin/sh '{s}/hook.sh' '{}' >> '{s}/hook.out' 2>&1; fi\n\
            exec /usr/bin/{program} \"$@\"\n",
            root.display()
        ),
    );
    format!("{}:{SYSTEM_PATH}", sandbox.join("stub-bin").display())
}

/// The same, at the first call of `program` whose arguments match the shell pattern `when` (a `case` pattern of
/// `"$*"`): the change is placed at one line of the script, whatever was called before it.
fn hook_when(sandbox: &Path, root: &Path, program: &str, when: &str, during: &str) -> String {
    fs::write(sandbox.join("hook.sh"), during).unwrap();
    let s = sandbox.display();
    executable(
        &sandbox.join("stub-bin").join(program),
        &format!(
            "#!/bin/sh\n\
            case \"$*\" in\n\
            {when}) if [ ! -e '{s}/hook-done' ]; then : > '{s}/hook-done'; /bin/sh '{s}/hook.sh' '{}' >> '{s}/hook.out' 2>&1; fi ;;\n\
            esac\n\
            exec /usr/bin/{program} \"$@\"\n",
            root.display()
        ),
    );
    format!("{}:{SYSTEM_PATH}", sandbox.join("stub-bin").display())
}

const GIT_AS_TEST: &str = "git -c user.name=test -c user.email=test@example.org -c commit.gpgsign=false";

fn version(prefix: &Path) -> String {
    fs::read_to_string(prefix.join("share/runbook/VERSION")).unwrap_or_default().trim().to_owned()
}

/// The files of one directory are copied in one group for each mode, and each group looks at its directory again: the
/// answer of the group before it is not kept. `protocol` has a file of mode 644 and one of mode 755; at the `cp` of the
/// first group the directory becomes a link to a directory outside the checkout. The second group stops the install by
/// the name of its file, the marker stays, and the program of outside is not in the prefix.
#[test]
fn a_directory_made_a_link_between_two_groups_of_its_files_stops_the_install_at_the_second_group() {
    let root = fake_checkout("copy-two-groups", None);
    let (sandbox, prefix) = sandbox_of(&root);
    executable(&root.join("protocol/run.sh"), "#!/bin/sh\necho of the checkout\n");
    git_in(&root, &["add", "protocol/run.sh"]);
    git_in(&root, &["commit", "-q", "-m", "a program"]);
    stubs(&root, None, &stub_version(&head12(&root)));
    fs::create_dir_all(sandbox.join("outside")).unwrap();
    fs::write(sandbox.join("outside/file.txt"), "outside the checkout\n").unwrap();
    executable(&sandbox.join("outside/run.sh"), "#!/bin/sh\necho outside the checkout\n");
    let during = "mv \"$1/protocol\" \"$1/../moved\"\nln -s \"$1/../outside\" \"$1/protocol\"\n";
    let path = hook_when(&sandbox, &root, "cp", "*share/runbook/protocol/", during);
    let o = from_built_with(&root, &prefix, &path, &[]);
    assert!(sandbox.join("hook-done").is_file(), "the directory was changed while the script ran: {}", err(&o));
    assert_eq!(code(&o), 1, "{}", err(&o));
    assert!(err(&o).contains("protocol/run.sh is below protocol, which is a link"), "{}", err(&o));
    assert!(prefix.join("share/runbook/INSTALLING").is_file(), "the marker stays");
    assert!(!prefix.join("share/runbook/protocol/run.sh").exists(), "the program of outside is not copied");
}

/// A tracked link is looked at again before it is written. While the script runs (after its first look at the links,
/// at its first `mkdir`): pointed at a file outside the checkout, it stops the install as a link that leaves the
/// checkout; made a regular file, it stops the install as a link that is not one now. The marker stays and no link is
/// in the prefix.
#[test]
fn a_tracked_link_pointed_outside_or_made_a_file_while_the_script_runs_stops_the_install_by_its_name() {
    for (case, during, said) in [
        ("retargeted", "rm -f \"$1/protocol/lnk\"\nln -s ../../outside.txt \"$1/protocol/lnk\"\n", "protocol/lnk is a link to ../../outside.txt, which leaves the checkout"),
        ("made-a-file", "rm -f \"$1/protocol/lnk\"\necho 'a file now' > \"$1/protocol/lnk\"\n", "protocol/lnk was a link when the list of files was taken and is not one now"),
    ] {
        let root = fake_checkout(&format!("copy-link-{case}"), None);
        let (sandbox, prefix) = sandbox_of(&root);
        std::os::unix::fs::symlink("file.txt", root.join("protocol/lnk")).unwrap();
        git_in(&root, &["add", "protocol/lnk"]);
        git_in(&root, &["commit", "-q", "-m", "a link"]);
        stubs(&root, None, &stub_version(&head12(&root)));
        fs::write(sandbox.join("outside.txt"), "outside the checkout\n").unwrap();
        let path = hook(&sandbox, &root, "mkdir", 1, during);
        let o = from_built_with(&root, &prefix, &path, &[]);
        assert_eq!(code(&o), 1, "{case}: {}", err(&o));
        assert!(err(&o).contains(said), "{case}: {}", err(&o));
        assert!(prefix.join("share/runbook/INSTALLING").is_file(), "{case}: the marker stays");
        assert!(fs::symlink_metadata(prefix.join("share/runbook/protocol/lnk")).is_err(), "{case}: nothing is installed at the link's place");
    }
}

/// HEAD is read before the list is taken and again after the build, and `git status` is asked once, after that. The
/// copy comes later. A commit made in the checkout after those reads and before the files are copied (here: when the
/// script makes `share/runbook`, its first `mkdir` with `--from-built`) gives exit 0 and a prefix whose `VERSION` names
/// the first commit, without ` dirty`, and whose files hold the bytes of the second.
#[test]
#[ignore = "story:install-reads-head-again-after-the-copy"]
fn measurement_a_commit_made_after_the_second_read_of_head_gives_a_prefix_that_names_the_first_commit_with_the_bytes_of_the_second()
{
    let root = fake_checkout("copy-after-second-read", None);
    let (sandbox, prefix) = sandbox_of(&root);
    let first = head12(&root);
    let during = format!(
        "r=$1\n\
        echo 'bytes of the second commit' > \"$r/protocol/file.txt\"\n\
        git -C \"$r\" add protocol/file.txt\n\
        {GIT_AS_TEST} -C \"$r\" commit -q -m 'made while the files are copied'\n"
    );
    let path = hook(&sandbox, &root, "mkdir", 1, &during);
    let o = from_built_with(&root, &prefix, &path, &[]);
    let second = head12(&root);
    assert_ne!(first, second, "the commit was made while the script ran: {}", err(&o));
    let installed = fs::read_to_string(prefix.join("share/runbook/protocol/file.txt")).unwrap_or_default();
    let v = version(&prefix);
    assert!(
        !(code(&o) == 0 && v.contains(&first) && !v.contains("dirty") && installed.contains("second commit")),
        "exit 0 and VERSION {v:?} (the first commit, no mark); share/runbook/protocol/file.txt holds {:?}, the bytes of {second}",
        installed.trim()
    );
}

/// Before each `cp` the files of its group are looked at again; then the script makes the directory below the prefix
/// (`mkdir`, `chmod`) and copies. A file that becomes a link to a file outside the checkout after that look (here: at
/// the `mkdir` of `share/runbook/protocol`) is copied through the link: exit 0, no marker, the bytes of outside.
#[test]
#[ignore = "story:install-copy-does-not-follow-a-late-link"]
fn measurement_a_file_made_a_link_between_the_check_of_its_group_and_its_copy_is_copied_from_outside_the_checkout() {
    let root = fake_checkout("copy-check-to-copy", None);
    let (sandbox, prefix) = sandbox_of(&root);
    fs::write(sandbox.join("outside.txt"), "outside the checkout\n").unwrap();
    let during = "rm -f \"$1/protocol/file.txt\"\nln -s \"$1/../outside.txt\" \"$1/protocol/file.txt\"\n";
    let path = hook_when(&sandbox, &root, "mkdir", "*share/runbook/protocol", during);
    let o = from_built_with(&root, &prefix, &path, &[]);
    assert!(sandbox.join("hook-done").is_file(), "the file was changed while the script ran: {}", err(&o));
    let installed = fs::read_to_string(prefix.join("share/runbook/protocol/file.txt")).unwrap_or_default();
    assert!(
        !installed.contains("outside the checkout"),
        "exit {}, marker {}: share/runbook/protocol/file.txt holds {:?}",
        code(&o),
        if prefix.join("share/runbook/INSTALLING").exists() { "there" } else { "removed" },
        installed.trim()
    );
}
