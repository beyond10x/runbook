//! What is copied: only the files git tracks, and no link that leaves the directory it is copied from. Fake checkouts;
//! every prefix lies under the target directory.

use super::*;

// ---- only tracked files ----

/// A checkout with a `.git` whose git cannot be asked is taken for a source archive: every regular file that is no
/// dot-file is copied, tracked or not. Two ways there: the checkout belongs to another user than the one who installs
/// (a root shell without `SUDO_UID`, `doas`, a container with a mounted checkout: git refuses a repository of another owner;
/// `GIT_TEST_ASSUME_DIFFERENT_OWNER` is git's own switch for that state), and `git` is not on `PATH`. The built files
/// print the commit `unknown`, as a build under the same condition does (`build_commit.rs` asks the same git).
#[test]
fn a_git_checkout_whose_git_cannot_be_asked_is_copied_like_an_archive_with_its_untracked_files() {
    let mut broken = Vec::new();
    for case in ["another owner (a root shell, a container)", "no git on PATH"] {
        let root = fake_checkout(&format!("fix2-nogit-{}", &case[..2]), None);
        let (sandbox, prefix) = sandbox_of(&root);
        assert!(root.join(".git").is_dir());
        fs::write(root.join("bindings/settings.json"), "{\"login_file\": \"/somewhere/login.json\"}\n").unwrap();
        fs::write(root.join("tools/id_ed25519"), "a key\n").unwrap();
        fs::write(root.join("protocol/notes-of-a-run.txt"), "left by a person\n").unwrap();
        stubs(&root, None, &stub_version("unknown"));
        let mut cmd = Command::new("sh");
        cmd.arg(root.join("scripts/install.sh")).arg("--prefix").arg(&prefix).args(["--from-built", "debug"]).env_clear().env("HOME", sandbox.join("home")).stdin(Stdio::null());
        if case.starts_with("another") {
            cmd.env("PATH", SYSTEM_PATH).env("GIT_TEST_ASSUME_DIFFERENT_OWNER", "1");
        } else {
            let tools = sandbox.join("tools-without-git");
            fs::create_dir_all(&tools).unwrap();
            for t in ["sh", "awk", "sed", "sort", "tr", "cut", "cat", "find", "cp", "chmod", "mkdir", "rm", "rmdir", "mv", "ln", "readlink", "dirname", "wc"] {
                std::os::unix::fs::symlink(Path::new("/usr/bin").join(t), tools.join(t)).unwrap();
            }
            cmd.env("PATH", &tools);
        }
        let o = cmd.output().unwrap();
        let share = prefix.join("share/runbook");
        let copied: Vec<&str> = ["bindings/settings.json", "tools/id_ed25519", "protocol/notes-of-a-run.txt"].into_iter().filter(|f| share.join(f).exists()).collect();
        if code(&o) == 0 && !copied.is_empty() {
            broken.push(format!("{case}: exit 0, untracked files of a git checkout in the prefix: {copied:?}\n{}", err(&o)));
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}

/// The check of a tracked link's target walks the path under a made-up root `/r/` and accepts what stays below it. `..`
/// above that root is dropped, so a target that climbs out and names a directory `r` on its way back passes, and the
/// link is installed as it is: in the prefix it points outside the prefix.
#[test]
fn a_tracked_link_whose_target_passes_a_directory_named_r_is_installed_pointing_out_of_the_prefix() {
    let root = fake_checkout("fix2-link-r", None);
    let (sandbox, prefix) = sandbox_of(&root);
    std::os::unix::fs::symlink("../../../../r/x", root.join("protocol/lnk")).unwrap();
    git_in(&root, &["add", "protocol/lnk"]);
    git_in(&root, &["commit", "-q", "-m", "a link"]);
    stubs(&root, None, &stub_version(&head12(&root)));
    fs::create_dir_all(sandbox.join("r")).unwrap();
    fs::write(sandbox.join("r/x"), "outside the prefix\n").unwrap();
    let o = from_built(&root, &prefix);
    let through = fs::read_to_string(prefix.join("share/runbook/protocol/lnk")).unwrap_or_default();
    assert!(
        !(code(&o) == 0 && through == "outside the prefix\n"),
        "exit 0, and share/runbook/protocol/lnk -> ../../../../r/x reads <prefix>/../r/x: {through:?}"
    );
}
/// `GIT_DIR`, `GIT_WORK_TREE` and `GIT_INDEX_FILE` of the caller's environment name another repository and an index
/// that does not exist: the install copies the same files as without them. Without `unset GIT_DIR GIT_WORK_TREE
/// GIT_INDEX_FILE` in the script the list of tracked files comes from the other repository.
#[test]
fn git_variables_of_the_environment_change_nothing_of_what_is_copied() {
    let root = fake_checkout("fix2-g-gitenv", None);
    let (sandbox, prefix) = sandbox_of(&root);
    fs::write(root.join("protocol/untracked.txt"), "left by a run\n").unwrap();
    let other = sandbox.join("other");
    fs::create_dir_all(other.join("protocol")).unwrap();
    fs::write(other.join("protocol/untracked.txt"), "tracked in the other repository\n").unwrap();
    git_in(&other, &["init", "-q"]);
    git_in(&other, &["add", "-A"]);
    git_in(&other, &["commit", "-q", "-m", "another repository"]);
    assert_eq!(code(&from_built(&root, &prefix)), 0);
    let plain = fs::read_to_string(prefix.join("share/runbook/MANIFEST")).unwrap();
    let second = sandbox.join("second/prefix");
    fs::create_dir_all(&second).unwrap();
    fs::create_dir_all(sandbox.join("second/home")).unwrap();
    let o = from_built_with(&root, &second, SYSTEM_PATH, &[("GIT_DIR", &other.join(".git")), ("GIT_WORK_TREE", &other), ("GIT_INDEX_FILE", &sandbox.join("no-such-index"))]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert_eq!(fs::read_to_string(second.join("share/runbook/MANIFEST")).unwrap(), plain);
    assert!(!plain.contains("untracked.txt"));
}
/// A tracked link whose target leaves the directory it is copied from is refused before anything is written.
#[test]
fn a_tracked_link_that_leaves_its_directory_is_refused_and_nothing_is_written() {
    let root = fake_checkout("fix2-g-link-out", None);
    let (sandbox, prefix) = sandbox_of(&root);
    fs::write(sandbox.join("outside.txt"), "outside the checkout\n").unwrap();
    std::os::unix::fs::symlink("../../outside.txt", root.join("protocol/lnk")).unwrap();
    git_in(&root, &["add", "protocol/lnk"]);
    git_in(&root, &["commit", "-q", "-m", "a link that leaves"]);
    stubs(&root, None, &stub_version(&head12(&root)));
    let o = from_built(&root, &prefix);
    assert_ne!(code(&o), 0, "a link to ../../outside.txt was accepted");
    assert_eq!(listing(&prefix), Vec::<String>::new(), "{}", err(&o));
}
/// A tracked file that is a link in the work tree (the index still says file) is not copied through the link.
#[test]
fn a_tracked_file_that_is_a_link_in_the_work_tree_is_refused_and_nothing_is_written() {
    let root = fake_checkout("fix2-g-file-link", None);
    let (sandbox, prefix) = sandbox_of(&root);
    fs::write(sandbox.join("login.json"), "{\"token\": \"a login\"}\n").unwrap();
    fs::remove_file(root.join("protocol/file.txt")).unwrap();
    std::os::unix::fs::symlink(sandbox.join("login.json"), root.join("protocol/file.txt")).unwrap();
    let o = from_built(&root, &prefix);
    assert_ne!(code(&o), 0, "the bytes of a file outside the checkout were copied through a link");
    assert_eq!(listing(&prefix), Vec::<String>::new(), "{}", err(&o));
}
