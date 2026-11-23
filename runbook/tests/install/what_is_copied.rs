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

// ---- a checkout whose git cannot be asked ----

/// A checkout with a `.git` whose git cannot be asked is never copied like an archive: the script stops with exit 1 and
/// writes nothing, and the message gives the reason and the remedy. Three ways there: the checkout belongs to another
/// user than the one who installs (`GIT_TEST_ASSUME_DIFFERENT_OWNER` is git's own switch for that state), `git` is not
/// on `PATH`, and the `.git` is a file that names a repository that does not exist. Files git does not track lie in the
/// checkout (a settings file, a key, a note of a run).
#[test]
fn a_git_checkout_whose_git_cannot_be_asked_is_refused_with_the_reason_and_what_to_do_and_nothing_is_written() {
    for (case, said) in [("owner", "another owner"), ("nogit", "git is not on PATH"), ("brokengit", "cannot be asked")] {
        let root = fake_checkout(&format!("fix2-ask-{case}"), None);
        let (sandbox, prefix) = sandbox_of(&root);
        fs::write(root.join("bindings/settings.json"), "{\"login_file\": \"/somewhere/login.json\"}\n").unwrap();
        fs::write(root.join("tools/id_ed25519"), "a key\n").unwrap();
        stubs(&root, None, &stub_version("unknown"));
        let mut cmd = Command::new("sh");
        cmd.arg(root.join("scripts/install.sh")).arg("--prefix").arg(&prefix).args(["--from-built", "debug"]).env_clear().env("HOME", sandbox.join("home")).stdin(Stdio::null());
        match case {
            "owner" => {
                cmd.env("PATH", SYSTEM_PATH).env("GIT_TEST_ASSUME_DIFFERENT_OWNER", "1");
            }
            "nogit" => {
                let tools = sandbox.join("tools-without-git");
                fs::create_dir_all(&tools).unwrap();
                for t in ["sh", "awk", "sed", "sort", "tr", "cut", "cat", "find", "cp", "chmod", "mkdir", "rm", "rmdir", "mv", "ln", "readlink", "dirname", "wc"] {
                    std::os::unix::fs::symlink(Path::new("/usr/bin").join(t), tools.join(t)).unwrap();
                }
                cmd.env("PATH", &tools);
            }
            _ => {
                fs::remove_dir_all(root.join(".git")).unwrap();
                fs::write(root.join(".git"), "gitdir: ./no-such-repository\n").unwrap();
                cmd.env("PATH", SYSTEM_PATH);
            }
        }
        let o = cmd.output().unwrap();
        assert_eq!(code(&o), 1, "{case}: {}", err(&o));
        assert!(err(&o).contains("nothing was written") || err(&o).contains("install from a source archive"), "{case}: no remedy in: {}", err(&o));
        assert!(err(&o).contains(said), "{case}: the reason is not named: {}", err(&o));
        assert_eq!(listing(&prefix), Vec::<String>::new(), "{case}: nothing is written");
    }
}

/// A source archive has no `.git` and is copied as before, also when it is unpacked inside another git work tree.
#[test]
fn a_tree_without_a_dot_git_is_copied_as_an_archive_inside_another_repository_too() {
    let outer = fake_checkout("fix2-archive-in-repository", None);
    let inner = outer.join("unpacked");
    fs::create_dir_all(&inner).unwrap();
    let archive = fake_archive("fix2-archive-copy");
    let status = Command::new("cp").arg("-a").arg(format!("{}/.", archive.display())).arg(&inner).status().unwrap();
    assert!(status.success());
    let sandbox = scratch("fix2-archive-prefix");
    let prefix = sandbox.join("prefix");
    fs::create_dir_all(&prefix).unwrap();
    fs::create_dir_all(sandbox.join("home")).unwrap();
    let o = install(&inner.join("scripts/install.sh"), &prefix, &["--from-built", "debug"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(prefix.join("share/runbook/protocol/file.txt").is_file());
}

// ---- links ----

fn commit_link(root: &Path, name: &str, target: &str) {
    std::os::unix::fs::symlink(target, root.join(name)).unwrap();
    git_in(root, &["add", name]);
    git_in(root, &["commit", "-q", "-m", "a link"]);
    stubs(root, None, &stub_version(&head12(root)));
}

/// A link is installed only when its target stays inside the checkout, is copied too, and has `..` at its start only:
/// each of these is refused before anything is written, with the link and its target named.
#[test]
fn a_tracked_link_to_a_file_that_is_not_copied_or_with_a_dot_dot_in_its_middle_is_refused_before_anything_is_written() {
    for (case, target, said) in [
        ("not-copied", "../README.md", "not among the files that are copied"),
        ("dangling", "no-such-file.txt", "not among the files that are copied"),
        ("middle", "../scripts/../protocol/file.txt", "allowed at the start of a link target only"),
        ("above-and-back", "../../../../x/y", "leaves the checkout"),
        ("absolute", "/etc/hostname", "absolute path"),
    ] {
        let root = fake_checkout(&format!("fix2-link-{case}"), None);
        let (_, prefix) = sandbox_of(&root);
        fs::write(root.join("README.md"), "not copied\n").unwrap();
        git_in(&root, &["add", "README.md"]);
        commit_link(&root, "protocol/lnk", target);
        let o = from_built(&root, &prefix);
        assert_eq!(code(&o), 1, "{case}: {}", err(&o));
        assert!(err(&o).contains("protocol/lnk") && err(&o).contains(target), "{case}: the link is not named: {}", err(&o));
        assert!(err(&o).contains(said), "{case}: the refusal gives the reason {said:?}: {}", err(&o));
        assert_eq!(listing(&prefix), Vec::<String>::new(), "{case}: nothing is written");
    }
}

/// A link to a file and a link to a directory that are copied too are copied as links with their targets as written.
#[test]
fn a_tracked_link_to_a_copied_file_and_to_a_copied_directory_is_installed_as_a_link() {
    let root = fake_checkout("fix2-link-good", None);
    let (_, prefix) = sandbox_of(&root);
    std::os::unix::fs::symlink("file.txt", root.join("protocol/same-directory")).unwrap();
    std::os::unix::fs::symlink("../scripts", root.join("protocol/to-a-directory")).unwrap();
    std::os::unix::fs::symlink("../scripts/file.txt", root.join("protocol/to-a-file")).unwrap();
    git_in(&root, &["add", "protocol"]);
    git_in(&root, &["commit", "-q", "-m", "links"]);
    stubs(&root, None, &stub_version(&head12(&root)));
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let share = prefix.join("share/runbook/protocol");
    for (name, target) in [("same-directory", "file.txt"), ("to-a-directory", "../scripts"), ("to-a-file", "../scripts/file.txt")] {
        assert_eq!(fs::read_link(share.join(name)).unwrap(), Path::new(target), "{name}");
    }
    assert_eq!(fs::read_to_string(share.join("to-a-file")).unwrap(), "scripts\n");
    assert!(share.join("to-a-directory/file.txt").is_file());
}

/// A `.git` whose repository names another work tree (`core.worktree`) is not the work tree of this checkout: git's
/// top is that other directory, and the script stops with the two names and writes nothing.
#[test]
fn a_checkout_whose_git_names_another_work_tree_is_refused_and_nothing_is_written() {
    let root = fake_checkout("fix2-other-top", None);
    let (sandbox, prefix) = sandbox_of(&root);
    let elsewhere = sandbox.join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    git_in(&root, &["config", "core.worktree", elsewhere.to_str().unwrap()]);
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 1, "{}", err(&o));
    assert!(err(&o).contains("top of its work tree"), "{}", err(&o));
    assert_eq!(listing(&prefix), Vec::<String>::new(), "nothing is written");
}

// ---- what is copied ----

/// `git ls-files -z` is turned into lines, so a tracked name with a newline in it becomes two entries: its first line,
/// taken for a tracked file, and the rest, skipped with a message. With an untracked file of the first line's name
/// beside it the install copies that file and ends with exit 0.
#[test]
fn a_tracked_name_with_a_newline_does_not_let_an_untracked_file_of_its_first_line_into_the_prefix() {
    let root = fake_checkout("adv3-newline-name", None);
    let (_sandbox, prefix) = sandbox_of(&root);
    fs::write(root.join("protocol/notes\nof-a-run.txt"), "tracked\n").unwrap();
    git_in(&root, &["add", "-A", "protocol"]);
    git_in(&root, &["commit", "-q", "-m", "a name with a newline"]);
    stubs(&root, None, &stub_version(&head12(&root)));
    fs::write(root.join("protocol/notes"), "left by a person, not tracked\n").unwrap();
    let o = from_built(&root, &prefix);
    assert!(
        !prefix.join("share/runbook/protocol/notes").exists(),
        "exit {}: the untracked protocol/notes is in the prefix\n{}",
        code(&o),
        err(&o)
    );
}

/// A tracked file that is a link in the work tree is refused (`what_is_copied.rs`). The check reads the last part of
/// the path only: with a tracked directory replaced by a link to a directory outside the checkout, the file of the same
/// name there is copied (`VERSION` then ends in ` dirty`, because git sees the tracked file as deleted).
#[test]
fn a_tracked_directory_that_is_a_link_in_the_work_tree_is_not_copied_from_outside_the_checkout() {
    let root = fake_checkout("adv3-directory-link", None);
    let (sandbox, prefix) = sandbox_of(&root);
    fs::create_dir_all(root.join("protocol/sub")).unwrap();
    fs::write(root.join("protocol/sub/x.txt"), "tracked\n").unwrap();
    git_in(&root, &["add", "-A", "protocol"]);
    git_in(&root, &["commit", "-q", "-m", "a directory"]);
    stubs(&root, None, &stub_version(&head12(&root)));
    fs::create_dir_all(sandbox.join("outside")).unwrap();
    fs::write(sandbox.join("outside/x.txt"), "a login outside the checkout\n").unwrap();
    fs::remove_dir_all(root.join("protocol/sub")).unwrap();
    std::os::unix::fs::symlink(sandbox.join("outside"), root.join("protocol/sub")).unwrap();
    let o = from_built(&root, &prefix);
    let copied = fs::read_to_string(prefix.join("share/runbook/protocol/sub/x.txt")).unwrap_or_default();
    assert!(
        !copied.contains("a login outside the checkout"),
        "exit {}: share/runbook/protocol/sub/x.txt holds the bytes of a file outside the checkout ({}): {copied:?}",
        code(&o),
        fs::read_to_string(prefix.join("share/runbook/VERSION")).unwrap_or_default().trim()
    );
}

/// A submodule in a copied directory is an entry that is neither a file nor a link: it is named and not copied.
#[test]
fn a_submodule_in_a_copied_directory_is_named_and_not_copied() {
    let root = fake_checkout("adv3-submodule", None);
    let (_sandbox, prefix) = sandbox_of(&root);
    let commit = git_in(&root, &["rev-parse", "HEAD"]);
    fs::create_dir_all(root.join("protocol/sub")).unwrap();
    git_in(&root, &["update-index", "--add", "--cacheinfo", &format!("160000,{commit},protocol/sub")]);
    git_in(&root, &["commit", "-q", "-m", "a submodule"]);
    stubs(&root, None, &stub_version(&head12(&root)));
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(err(&o).contains("not copying protocol/sub"), "the submodule is named: {}", err(&o));
    assert!(!prefix.join("share/runbook/protocol/sub").exists());
}

/// A tab in a tracked name, and a tab or a newline in the target of a tracked link, are refused by name before anything
/// is written: the list of files is lines with tabs between the fields.
#[test]
fn a_tab_in_a_tracked_name_and_a_tab_or_newline_in_a_link_target_are_refused_by_name_and_nothing_is_written() {
    for (case, name, target, word) in [
        ("tab-name", "protocol/a\tb.txt", None, "has a tab in its name"),
        ("tab-target", "protocol/lnk", Some("file.txt\tx"), "whose target has a tab or a newline"),
        ("newline-target", "protocol/lnk", Some("file.txt\nbindings/file.txt"), "whose target has a tab or a newline"),
    ] {
        let root = fake_checkout(&format!("adv3-{case}"), None);
        let (_sandbox, prefix) = sandbox_of(&root);
        match target {
            Some(t) => std::os::unix::fs::symlink(t, root.join(name)).unwrap(),
            None => fs::write(root.join(name), "x\n").unwrap(),
        }
        git_in(&root, &["add", "-A", "protocol"]);
        git_in(&root, &["commit", "-q", "-m", case]);
        stubs(&root, None, &stub_version(&head12(&root)));
        let o = from_built(&root, &prefix);
        assert_eq!(code(&o), 1, "{case}: {}", err(&o));
        assert!(err(&o).contains(word), "{case}: {}", err(&o));
        assert_eq!(listing(&prefix), Vec::<String>::new(), "{case}: nothing is written");
    }
}
