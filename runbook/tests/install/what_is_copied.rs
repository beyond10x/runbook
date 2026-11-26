//! What is copied: only the files git tracks, and no link that leaves the directory it is copied from. Fake checkouts;
//! every prefix lies under the target directory.

use super::*;

// ---- only tracked files ----

/// A checkout with a `.git` whose git cannot be asked gives no untracked file to the prefix (a settings file, a key, a
/// note of a run): the install stops or copies none of them. Two ways there: the checkout belongs to another user than
/// the one who installs (a root shell without `SUDO_UID`, `doas`, a container with a mounted checkout: git refuses a
/// repository of another owner; `GIT_TEST_ASSUME_DIFFERENT_OWNER` is git's own switch for that state), and `git` is not
/// on `PATH`. The built files print the commit `unknown`, as a build under the same condition does (`build_commit.rs`
/// asks the same git).
#[test]
fn a_git_checkout_whose_git_cannot_be_asked_never_gives_its_untracked_files_to_the_prefix() {
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
        cmd.arg(root.join("scripts/install.sh")).arg("--prefix").arg(&prefix).args(["--from-built", "debug"]).env_clear().envs(crate::kept()).envs(NO_SYSTEM_GIT).env("HOME", sandbox.join("home")).stdin(Stdio::null());
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

/// A tracked link whose target climbs out of the checkout and names a directory `r` on its way back
/// (`../../../../r/x`) is not installed as a link that points outside the prefix: the install stops, or the link in the
/// prefix does not read a file of `<prefix>/../r`.
#[test]
fn a_tracked_link_whose_target_passes_a_directory_named_r_is_not_installed_pointing_out_of_the_prefix() {
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
        cmd.arg(root.join("scripts/install.sh")).arg("--prefix").arg(&prefix).args(["--from-built", "debug"]).env_clear().envs(crate::kept()).envs(NO_SYSTEM_GIT).env("HOME", sandbox.join("home")).stdin(Stdio::null());
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

/// A machine whose system git configuration trusts every directory (`safe.directory = *`: the forge's runner image)
/// answers for a checkout of another owner, so the install goes on as for any checkout: exit 0, the tracked files are
/// copied and the untracked ones (a settings file, a key) are not. The refusal above holds only where git refuses.
#[test]
fn a_checkout_of_another_owner_that_the_system_git_configuration_trusts_is_installed_with_its_tracked_files_only() {
    let root = fake_checkout("fix2-owner-trusted", None);
    let (sandbox, prefix) = sandbox_of(&root);
    fs::write(root.join("bindings/settings.json"), "{\"login_file\": \"/somewhere/login.json\"}\n").unwrap();
    fs::write(root.join("tools/id_ed25519"), "a key\n").unwrap();
    stubs(&root, None, &stub_version(&head12(&root)));
    let system = sandbox.join("system-gitconfig");
    fs::write(&system, "[safe]\n\tdirectory = *\n").unwrap();
    let o = Command::new("sh")
        .arg(root.join("scripts/install.sh"))
        .arg("--prefix")
        .arg(&prefix)
        .args(["--from-built", "debug"])
        .env_clear().envs(crate::kept())
        .env("HOME", sandbox.join("home"))
        .env("PATH", SYSTEM_PATH)
        .env("GIT_TEST_ASSUME_DIFFERENT_OWNER", "1")
        .env("GIT_CONFIG_SYSTEM", &system)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(code(&o), 0, "{}", err(&o));
    let share = prefix.join("share/runbook");
    assert!(share.join("protocol/file.txt").is_file(), "a tracked file is copied");
    for untracked in ["bindings/settings.json", "tools/id_ed25519"] {
        assert!(!share.join(untracked).exists(), "{untracked} is not tracked and is not in the prefix");
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

/// `git ls-files -z` is read as lines, and a tracked name with a newline in it must not become two entries (its first
/// line taken for a tracked file): with an untracked file of the first line's name beside it, that file is never copied.
#[test]
fn a_tracked_name_with_a_newline_does_not_let_an_untracked_file_of_its_first_line_into_the_prefix() {
    let root = fake_checkout("newline-name", None);
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

/// A tracked file that is a link in the work tree is refused (above). So is a tracked directory replaced by a link to a
/// directory outside the checkout: the file of the same name there is never copied.
#[test]
fn a_tracked_directory_that_is_a_link_in_the_work_tree_is_not_copied_from_outside_the_checkout() {
    let root = fake_checkout("directory-link", None);
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
    let root = fake_checkout("submodule", None);
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
        let root = fake_checkout(case, None);
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

/// A tracked directory that is a link in the work tree is refused by the name of a file below it, before anything is
/// written: the prefix holds nothing.
#[test]
fn a_tracked_directory_that_is_a_link_in_the_work_tree_is_refused_by_name_and_nothing_is_written() {
    let root = fake_checkout("directory-link-named", None);
    let (sandbox, prefix) = sandbox_of(&root);
    fs::create_dir_all(root.join("protocol/sub")).unwrap();
    fs::write(root.join("protocol/sub/x.txt"), "tracked\n").unwrap();
    git_in(&root, &["add", "-A", "protocol"]);
    git_in(&root, &["commit", "-q", "-m", "a directory"]);
    stubs(&root, None, &stub_version(&head12(&root)));
    fs::create_dir_all(sandbox.join("outside")).unwrap();
    fs::write(sandbox.join("outside/x.txt"), "outside\n").unwrap();
    fs::remove_dir_all(root.join("protocol/sub")).unwrap();
    std::os::unix::fs::symlink(sandbox.join("outside"), root.join("protocol/sub")).unwrap();
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 1, "{}", err(&o));
    assert!(err(&o).contains("protocol/sub/x.txt") && err(&o).contains("protocol/sub"), "{}", err(&o));
    assert_eq!(listing(&prefix), Vec::<String>::new(), "nothing is written");
}

/// A tracked name with a newline is refused by name (as a control character), as a tab is, and nothing is written.
#[test]
fn a_tracked_name_with_a_newline_is_refused_by_name_and_nothing_is_written() {
    let root = fake_checkout("newline-named", None);
    let (_sandbox, prefix) = sandbox_of(&root);
    fs::write(root.join("protocol/notes\nof-a-run.txt"), "tracked\n").unwrap();
    git_in(&root, &["add", "-A", "protocol"]);
    git_in(&root, &["commit", "-q", "-m", "a name with a newline"]);
    stubs(&root, None, &stub_version(&head12(&root)));
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 1, "{}", err(&o));
    assert!(err(&o).contains("has a control character in its name"), "{}", err(&o));
    assert!(err(&o).contains("protocol/notes"), "the name is given: {}", err(&o));
    assert_eq!(listing(&prefix), Vec::<String>::new(), "nothing is written");
}

/// A tracked name with the byte 0x01 (the byte the list uses for a newline) is refused by name with the same word as a
/// newline, a control character, and nothing is written.
#[test]
fn a_tracked_name_with_the_byte_0x01_is_refused_as_a_control_character_and_nothing_is_written() {
    let root = fake_checkout("soh-named", None);
    let (_sandbox, prefix) = sandbox_of(&root);
    fs::write(root.join("protocol/notes\u{1}of-a-run.txt"), "tracked\n").unwrap();
    git_in(&root, &["add", "-A", "protocol"]);
    git_in(&root, &["commit", "-q", "-m", "a name with the byte 0x01"]);
    stubs(&root, None, &stub_version(&head12(&root)));
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 1, "{}", err(&o));
    assert!(err(&o).contains("has a control character in its name"), "{}", err(&o));
    assert!(err(&o).contains("protocol/notes"), "the name is given: {}", err(&o));
    assert_eq!(listing(&prefix), Vec::<String>::new(), "nothing is written");
}

/// A sparse checkout leaves tracked files out of the work tree. The install stops with the name of the first one and
/// says sparse, so that the reason is known; nothing is written.
#[test]
fn a_sparse_checkout_is_refused_by_the_name_of_a_missing_file_and_says_sparse() {
    let root = fake_checkout("sparse", None);
    let (_sandbox, prefix) = sandbox_of(&root);
    git_in(&root, &["sparse-checkout", "set", "--no-cone", "/protocol/", "/bindings/", "/instructions/", "/views/", "/cycles/", "/ui/", "/driver/", "/scripts/"]);
    assert!(!root.join("tools/file.txt").exists(), "the sparse checkout left tools/ out");
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 1, "{}", err(&o));
    assert!(err(&o).contains("tools/file.txt"), "{}", err(&o));
    assert!(err(&o).contains("sparse"), "{}", err(&o));
    assert_eq!(listing(&prefix), Vec::<String>::new(), "nothing is written");
}

// ---- links, names and refusals of the list ----

/// A directory above a tracked file that is a link in the work tree is refused at every depth: the directory right
/// above the file, two and three above it, the copied directory itself, and a directory above a copied directory
/// (`driver`). Nothing is written.
#[test]
fn a_link_at_any_depth_above_a_tracked_file_is_refused_by_the_name_of_the_link_and_nothing_is_written() {
    for (case, link, file) in [
        ("right-above", "protocol/a/b/c", "protocol/a/b/c/x.txt"),
        ("two-above", "protocol/a/b", "protocol/a/b/c/x.txt"),
        ("three-above", "protocol/a", "protocol/a/b/c/x.txt"),
        ("the-copied-directory", "protocol", "protocol/"),
        ("above-a-copied-directory", "driver", "driver/examples/sdc/file.txt"),
    ] {
        let root = fake_checkout(&format!("depth-link-{case}"), None);
        let (sandbox, prefix) = sandbox_of(&root);
        fs::create_dir_all(root.join("protocol/a/b/c")).unwrap();
        fs::write(root.join("protocol/a/b/c/x.txt"), "tracked\n").unwrap();
        git_in(&root, &["add", "-A", "protocol"]);
        git_in(&root, &["commit", "-q", "-m", "a deep directory"]);
        stubs(&root, None, &stub_version(&head12(&root)));
        let moved = sandbox.join("moved-outside");
        fs::rename(root.join(link), &moved).unwrap();
        std::os::unix::fs::symlink(&moved, root.join(link)).unwrap();
        // the stubs of driver/ went with it: the link answers for them, as it would for a person
        let o = from_built(&root, &prefix);
        assert_eq!(code(&o), 1, "{case}: {}", err(&o));
        assert!(
            err(&o).contains(&format!("is tracked below {link}, which is a link")) && err(&o).contains(file),
            "{case}: the link and a file below it are named: {}",
            err(&o)
        );
        assert_eq!(listing(&prefix), Vec::<String>::new(), "{case}: nothing is written");
    }
}

/// A tracked file that is deleted by hand in a checkout that is not sparse is refused by name, and the message does not
/// call the checkout sparse. A built `runbook` of another commit is refused with the two commits and the file to touch
/// (after a branch was moved from another work tree cargo compiles nothing until an input of the build changes).
#[test]
fn a_missing_file_of_a_full_checkout_is_not_called_sparse_and_a_binary_of_another_commit_is_told_what_to_touch() {
    let root = fake_checkout("missing-not-sparse", None);
    let (_sandbox, prefix) = sandbox_of(&root);
    fs::remove_file(root.join("protocol/file.txt")).unwrap();
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 1, "{}", err(&o));
    assert!(err(&o).contains("protocol/file.txt is tracked and is not in the work tree as a file"), "{}", err(&o));
    assert!(!err(&o).contains("sparse"), "{}", err(&o));
    assert_eq!(listing(&prefix), Vec::<String>::new(), "nothing is written");

    let root = fake_checkout("binary-of-other-commit", None);
    let (_sandbox, prefix) = sandbox_of(&root);
    stubs(&root, None, &stub_version("0123456789ab"));
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 1, "{}", err(&o));
    assert!(err(&o).contains("says commit 0123456789ab") && err(&o).contains(&head12(&root)), "{}", err(&o));
    assert!(err(&o).contains("touch runbook/build.rs"), "{}", err(&o));
    assert_eq!(listing(&prefix), Vec::<String>::new(), "nothing is written");
}

/// Names a shell loop mishandles are copied under their own names, and a second install over the prefix removes none
/// of them: a leading dash, `-n`, `-e`, a space at either end, a pattern, a backslash, quotes, a command substitution,
/// a byte that is not UTF-8, and directories of such names.
#[test]
fn names_a_shell_loop_mishandles_are_installed_under_their_own_names_and_kept_by_a_second_install() {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    let root = fake_checkout("odd-names", None);
    let (_sandbox, prefix) = sandbox_of(&root);
    let names: Vec<Vec<u8>> = [
        "-rf", "--help", "-n", "-e", "a b", " lead", "trail ", "glob*", "[x]", "?", "back\\slash", "back\\n", "quote\"q", "single'q",
        "$(touch pwned)", "`touch pwned`", ";semi", "&amp", "a|b", "#hash", "~", "!bang", "%s", "...", "..x", "x..",
        "-d/in", "a dir/in", "*/in", "[y]/in", "dir\\back/in", " lead dir/in", "trail dir /in",
    ]
    .iter()
    .map(|n| n.as_bytes().to_vec())
    .chain([b"latin\xe4".to_vec(), b"bel\x07".to_vec()])
    .collect();
    for n in &names {
        let p = root.join("protocol").join(std::ffi::OsStr::from_bytes(n));
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, n).unwrap();
    }
    git_in(&root, &["add", "-A", "protocol"]);
    git_in(&root, &["commit", "-q", "-m", "odd names"]);
    stubs(&root, None, &stub_version(&head12(&root)));
    for round in ["first", "second"] {
        let o = from_built(&root, &prefix);
        assert_eq!(code(&o), 0, "{round}: {}", err(&o));
        assert!(!err(&o).contains("removed "), "{round}: {}", err(&o));
        for n in &names {
            let p = prefix.join("share/runbook/protocol").join(std::ffi::OsString::from_vec(n.clone()));
            assert_eq!(fs::read(&p).ok().as_deref(), Some(n.as_slice()), "{round}: {:?}", String::from_utf8_lossy(n));
        }
    }
    assert!(!root.join("pwned").exists() && !prefix.join("pwned").exists() && !Path::new("pwned").exists(), "no name was run as a command");
}

/// A tracked link is written with `ln -s -- "$target" <place>`. A target that begins with a dash (a tracked file of that
/// name lies beside the link, so the target passes every check) is a name, not an option: the link is in the prefix
/// with that target, and nothing is left in the working directory of the caller. Started from an empty directory of
/// its own.
#[test]
fn a_tracked_link_whose_target_begins_with_a_dash_is_installed_with_that_target_and_leaves_nothing_in_the_working_directory() {
    let root = fake_checkout("dash-target", None);
    let (sandbox, prefix) = sandbox_of(&root);
    fs::write(root.join("protocol/-n"), "the file the link names\n").unwrap();
    std::os::unix::fs::symlink("-n", root.join("protocol/lnk")).unwrap();
    git_in(&root, &["add", "-A", "protocol"]);
    git_in(&root, &["commit", "-q", "-m", "a link to a name with a dash"]);
    stubs(&root, None, &stub_version(&head12(&root)));
    let cwd = sandbox.join("cwd");
    fs::create_dir_all(&cwd).unwrap();
    let o = Command::new("sh")
        .arg(root.join("scripts/install.sh"))
        .arg("--prefix")
        .arg(&prefix)
        .args(["--from-built", "debug"])
        .env_clear().envs(crate::kept()).envs(NO_SYSTEM_GIT)
        .env("PATH", SYSTEM_PATH)
        .env("HOME", sandbox.join("home"))
        .current_dir(&cwd)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let left = listing(&cwd);
    let at = prefix.join("share/runbook/protocol/lnk");
    let installed = fs::read_link(&at).map(|t| t.display().to_string()).unwrap_or_else(|_| "nothing".to_owned());
    let listed = fs::read_to_string(prefix.join("share/runbook/MANIFEST")).unwrap_or_default().lines().any(|l| l == "share/runbook/protocol/lnk");
    assert!(
        left.is_empty() && (code(&o) != 0 || installed == "-n"),
        "exit {}; at share/runbook/protocol/lnk: {installed}; MANIFEST lists it: {listed}; left in the working directory: {left:?}",
        code(&o)
    );
}
