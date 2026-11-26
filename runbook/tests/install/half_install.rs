//! What an install that stops, or that finds a link at the place of a name it writes, leaves behind: the marker, the
//! order of the last renames, and what `runbook doctor` of such a prefix says. Fake checkouts and stub programs; every
//! prefix lies under the target directory.

use super::*;

// ---- nothing outside the prefix ----

/// A link at the place of the marker (`share/runbook/INSTALLING`), to a file outside the prefix or to a path that does
/// not exist, is not written through: the file outside keeps its bytes and no file is made outside the prefix.
#[test]
fn the_marker_is_not_written_through_a_link_at_its_place_into_a_file_outside_the_prefix() {
    let root = fake_checkout("fix2-marker-link", None);
    let (sandbox, prefix) = sandbox_of(&root);
    assert_eq!(code(&from_built(&root, &prefix)), 0);
    let outside = sandbox.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("keep.txt"), "precious\n").unwrap();
    let marker = prefix.join("share/runbook/INSTALLING");
    std::os::unix::fs::symlink(outside.join("keep.txt"), &marker).unwrap();
    let o = from_built(&root, &prefix);
    let kept = fs::read_to_string(outside.join("keep.txt")).unwrap();
    let _ = fs::remove_file(&marker);
    std::os::unix::fs::symlink(outside.join("made-by-the-install.txt"), &marker).unwrap();
    let o2 = from_built(&root, &prefix);
    assert_eq!(
        (kept.as_str(), files_below(&outside).into_iter().collect::<Vec<_>>()),
        ("precious\n", vec!["keep.txt".to_owned()]),
        "the script wrote outside the prefix through a link at share/runbook/INSTALLING (exit {} and {}):\n{}{}",
        code(&o),
        code(&o2),
        err(&o),
        err(&o2)
    );
}
// ---- the installed binary finds its own files ----

/// The prefix of an install that stopped before `share/runbook/protocol/` was written (the asset directories are
/// copied in the order of their names: `bindings`, `cycles`, `driver`, `instructions` come first): it has
/// `bin/runbook`, the helpers and the marker. Made once per run of the tests; a stub `cp` fails at its fifth call, as on
/// a full disk.
fn stopped_prefix() -> PathBuf {
    shared_prefix_of("stopped", &|partial| {
        let cp = "#!/bin/sh\n\
            n=$(cat \"$FAIL_COUNT\" 2>/dev/null || echo 0)\n\
            n=$((n + 1))\n\
            echo \"$n\" > \"$FAIL_COUNT\"\n\
            if [ \"$n\" -eq 5 ]; then echo 'cp: No space left on device (the stub of the test)' >&2; exit 1; fi\n\
            exec /usr/bin/cp \"$@\"\n";
        executable(&partial.join("stub-bin/cp"), cp);
        let prefix = partial.join("prefix");
        let o = Command::new("sh")
            .arg(script())
            .arg("--prefix")
            .arg(&prefix)
            .args(["--from-built", "debug"])
            .env_clear().envs(crate::kept()).envs(NO_SYSTEM_GIT)
            .env("PATH", format!("{}:{SYSTEM_PATH}", partial.join("stub-bin").display()))
            .env("HOME", partial.join("home"))
            .env("FAIL_COUNT", partial.join("cp-calls"))
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(code(&o), 1, "{}{}", out(&o), err(&o));
        assert!(prefix.join("share/runbook/INSTALLING").is_file(), "the marker of the stopped install is there");
        assert!(prefix.join("bin/runbook").is_file() && !prefix.join("share/runbook/protocol").exists(), "the install stopped after bin/runbook and before protocol/: {}", err(&o));
    })
}

/// A directory that is a checkout to `runbook` (`protocol/` and `driver/examples/sdc/`) with nothing else in it: the
/// working directory of a run that would find a checkout if the prefix did not claim its place. It is no copy of this
/// repository, so a program that reads `rust-toolchain.toml` in the working directory finds none.
fn bare_checkout(tag: &str) -> PathBuf {
    let dir = scratch(tag);
    fs::create_dir_all(dir.join("protocol")).unwrap();
    fs::create_dir_all(dir.join("driver/examples/sdc")).unwrap();
    dir
}

/// `runbook doctor` of a stopped install, started in the checkout the person installed from: the install line is
/// there, it fails and says `incomplete install`, and the assets line is not the checkout's.
#[test]
fn doctor_of_an_install_that_stopped_before_protocol_was_copied_says_incomplete_install_in_a_checkout() {
    let prefix = stopped_prefix();
    let o = from_prefix(&prefix, &bare_checkout("stopped-doctor"), &["doctor"]);
    let text = out(&o);
    assert_ne!(code(&o), 0, "{text}");
    let line = |name: &str| text.lines().find(|l| l.split("  ").map(str::trim).filter(|s| !s.is_empty()).nth(1) == Some(name)).unwrap_or_else(|| panic!("no {name} line:\n{text}")).to_owned();
    let install = line("install");
    assert!(install.contains("incomplete install") && install.contains("INSTALLING"), "{install}");
    let assets = line("assets");
    assert!(assets.contains("incomplete install"), "the assets line is not the stopped prefix's: {assets}");
}

/// The same from an empty directory, and `list` and `check` of the stopped prefix: each is refused with the words
/// `incomplete install` and the marker's name, in the checkout and outside it.
#[test]
fn list_and_check_of_an_install_that_stopped_are_refused_by_name_in_every_directory() {
    let prefix = stopped_prefix();
    let elsewhere = scratch("stopped-elsewhere");
    for cwd in [bare_checkout("stopped-list"), elsewhere.clone()] {
        for args in [&["list"][..], &["check"], &["doctor"]] {
            let o = from_prefix(&prefix, &cwd, args);
            let said = format!("{}{}", out(&o), err(&o));
            assert_ne!(code(&o), 0, "{args:?} in {cwd:?} succeeded: {said}");
            assert!(said.contains("incomplete install"), "{args:?} in {cwd:?}: {said}");
        }
    }
}

/// A prefix without a marker and without `protocol/` (the install was never started there, or a person removed what
/// it wrote) is also its own: refused by name, never a checkout.
#[test]
fn a_prefix_with_a_share_directory_and_no_protocol_is_refused_and_never_falls_back_to_the_checkout() {
    let sandbox = scratch("share-without-protocol");
    let prefix = sandbox.join("prefix");
    fs::create_dir_all(prefix.join("bin")).unwrap();
    fs::create_dir_all(prefix.join("share/runbook")).unwrap();
    // a hard link, not a copy: a copy of a program that is written while another thread starts a program can fail with
    // "text file busy" (the file is open for writing in a child that has not yet run its program)
    fs::hard_link(BIN, prefix.join("bin/runbook")).unwrap();
    let o = from_prefix(&prefix, &bare_checkout("share-without-protocol-checkout"), &["list"]);
    assert_ne!(code(&o), 0, "{}", out(&o));
    assert!(err(&o).contains("incomplete install"), "{}", err(&o));
}

/// An old MANIFEST that lists the marker, `VERSION.new` or `MANIFEST.new` removes none of them: the install keeps those
/// names for itself (the marker is what says the install did not finish), and says so.
#[test]
fn a_manifest_line_that_names_the_marker_or_a_new_file_removes_nothing_and_the_marker_stays_after_a_failed_rename() {
    let root = fake_checkout("fix2-manifest-marker", None);
    let (sandbox, prefix) = sandbox_of(&root);
    assert_eq!(code(&from_built(&root, &prefix)), 0);
    let manifest_path = prefix.join("share/runbook/MANIFEST");
    let mut manifest = fs::read_to_string(&manifest_path).unwrap();
    manifest.push_str("share/runbook/INSTALLING\nshare/runbook/VERSION.new\nshare/runbook/MANIFEST.new\n");
    fs::write(&manifest_path, manifest).unwrap();
    executable(&sandbox.join("stub-bin/mv"), "#!/bin/sh\necho 'mv: no space left on device' >&2\nexit 1\n");
    let o = from_built_with(&root, &prefix, &format!("{}:{SYSTEM_PATH}", sandbox.join("stub-bin").display()), &[]);
    assert_ne!(code(&o), 0);
    assert!(prefix.join("share/runbook/INSTALLING").is_file(), "the marker was removed by a line of the old MANIFEST: {}", err(&o));
    assert!(err(&o).contains("not removing share/runbook/INSTALLING"), "{}", err(&o));
}

/// Links at the places of `VERSION.new` and `MANIFEST.new` are removed, not written through; a prefix whose
/// `libexec/runbook` is a link is refused before anything is written.
#[test]
fn links_at_the_new_files_and_at_libexec_runbook_are_never_written_through() {
    let root = fake_checkout("fix2-g-new-links", None);
    let (sandbox, prefix) = sandbox_of(&root);
    assert_eq!(code(&from_built(&root, &prefix)), 0);
    let outside = sandbox.join("outside");
    fs::create_dir_all(outside.join("dir")).unwrap();
    for n in ["VERSION.new", "MANIFEST.new"] {
        fs::write(outside.join(n), "precious\n").unwrap();
        std::os::unix::fs::symlink(outside.join(n), prefix.join("share/runbook").join(n)).unwrap();
    }
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 0, "{}", err(&o));
    for n in ["VERSION.new", "MANIFEST.new"] {
        assert_eq!(fs::read_to_string(outside.join(n)).unwrap(), "precious\n", "{n} was written through a link");
    }

    let linked = sandbox.join("linked/prefix");
    fs::create_dir_all(linked.join("libexec")).unwrap();
    fs::create_dir_all(sandbox.join("linked/home")).unwrap();
    std::os::unix::fs::symlink(outside.join("dir"), linked.join("libexec/runbook")).unwrap();
    let o = from_built(&root, &linked);
    assert_ne!(code(&o), 0, "a prefix whose libexec/runbook is a link was accepted");
    assert_eq!(files_below(&outside.join("dir")), BTreeSet::new(), "{}", err(&o));
    assert!(!linked.join("share").exists() && !linked.join("bin").exists(), "{:?}", listing(&linked));
}
/// The marker is removed after `VERSION` and `MANIFEST` are in place: an install over an older one whose `mv` fails
/// (a full disk) keeps the marker.
#[test]
fn an_install_whose_last_rename_fails_keeps_the_marker() {
    let root = fake_checkout("fix2-g-mv", None);
    let (sandbox, prefix) = sandbox_of(&root);
    assert_eq!(code(&from_built(&root, &prefix)), 0);
    executable(&sandbox.join("stub-bin/mv"), "#!/bin/sh\necho 'mv: no space left on device' >&2\nexit 1\n");
    let o = from_built_with(&root, &prefix, &format!("{}:{SYSTEM_PATH}", sandbox.join("stub-bin").display()), &[]);
    assert_ne!(code(&o), 0);
    assert!(prefix.join("share/runbook/INSTALLING").is_file(), "the marker is gone and the install did not finish: {}", err(&o));
}

/// A line of the old MANIFEST that names the marker removes nothing, spelled `share/runbook/INSTALLING` or with two
/// slashes: a spelling the script does not write is never compared as text with the names the install keeps for itself.
/// The install then stops (a stub `cp` fails, as on a full disk) and the marker is still there.
#[test]
fn an_old_manifest_line_that_spells_the_marker_with_two_slashes_does_not_remove_it() {
    let root = fake_checkout("manifest-slashes", None);
    let (sandbox, prefix) = sandbox_of(&root);
    assert_eq!(code(&from_built(&root, &prefix)), 0);
    let manifest = prefix.join("share/runbook/MANIFEST");
    let mut lines = fs::read_to_string(&manifest).unwrap();
    lines.push_str("share/runbook//INSTALLING\n");
    fs::write(&manifest, lines).unwrap();
    executable(&sandbox.join("stub-bin/cp"), "#!/bin/sh\necho 'cp: no space left on device' >&2\nexit 1\n");
    let o = from_built_with(&root, &prefix, &format!("{}:{SYSTEM_PATH}", sandbox.join("stub-bin").display()), &[]);
    assert_ne!(code(&o), 0);
    assert!(prefix.join("share/runbook/INSTALLING").is_file(), "the install did not finish and the marker is gone: {}", err(&o));
}

/// A line of the old MANIFEST that is not in the form this script writes (an empty part, `.` or `..` as a part, a
/// leading `/`) removes nothing and is named on standard error, with the reason; a line in the form is still removed.
#[test]
fn an_old_manifest_line_that_is_not_in_the_form_the_script_writes_removes_nothing_and_is_named() {
    let root = fake_checkout("manifest-form", None);
    let (_sandbox, prefix) = sandbox_of(&root);
    assert_eq!(code(&from_built(&root, &prefix)), 0);
    fs::write(prefix.join("share/runbook/protocol/dropped.txt"), "from an older install\n").unwrap();
    let odd = [
        "share/runbook//INSTALLING",
        "share/runbook/protocol//file.txt",
        "share/runbook/./protocol/file.txt",
        "share/runbook/protocol/../protocol/file.txt",
        "/share/runbook/protocol/file.txt",
        "share/runbook/protocol/",
    ];
    let manifest = prefix.join("share/runbook/MANIFEST");
    let mut text = fs::read_to_string(&manifest).unwrap();
    text.push_str("share/runbook/protocol/dropped.txt\n");
    for line in odd {
        text.push_str(line);
        text.push('\n');
    }
    fs::write(&manifest, text).unwrap();
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 0, "{}", err(&o));
    for line in odd {
        assert!(
            err(&o).lines().any(|l| l.contains(&format!("not removing {line}:")) && l.contains("form")),
            "{line} is named with the reason: {}",
            err(&o)
        );
        assert!(!err(&o).contains(&format!("removed {line} ")), "{line} removed something: {}", err(&o));
    }
    assert!(err(&o).contains("removed share/runbook/protocol/dropped.txt"), "a line in the form is still removed: {}", err(&o));
    assert!(!prefix.join("share/runbook/protocol/dropped.txt").exists());
    assert!(!prefix.join("share/runbook/INSTALLING").exists(), "the install finished");
}

// ---- lines of an old MANIFEST ----

/// Lines of an old MANIFEST in the form the script writes that name no file of the prefix remove nothing: a space or a
/// carriage return after the marker's name, a pattern, a tab, a directory. The install then stops (a stub `cp` fails)
/// and the marker, `VERSION` and a file beside them that a pattern would match are still there.
#[test]
fn lines_of_an_old_manifest_that_only_look_like_the_markers_name_or_a_pattern_remove_nothing() {
    let root = fake_checkout("manifest-lines", None);
    let (sandbox, prefix) = sandbox_of(&root);
    assert_eq!(code(&from_built(&root, &prefix)), 0);
    fs::write(prefix.join("share/runbook/kept.txt"), "kept\n").unwrap();
    let manifest = prefix.join("share/runbook/MANIFEST");
    let mut text = fs::read_to_string(&manifest).unwrap();
    for line in ["share/runbook/INSTALLING ", " share/runbook/INSTALLING", "share/runbook/INSTALLING\r", "share/runbook/*", "share/runbook/?ERSION", "share/runbook/a\tb", "share/runbook/protocol", "bin", "libexec/runbook", "-rf", "bin/-rf"] {
        text.push_str(line);
        text.push('\n');
    }
    fs::write(&manifest, text).unwrap();
    executable(&sandbox.join("stub-bin/cp"), "#!/bin/sh\necho 'cp: no space left on device' >&2\nexit 1\n");
    let o = from_built_with(&root, &prefix, &format!("{}:{SYSTEM_PATH}", sandbox.join("stub-bin").display()), &[]);
    assert_ne!(code(&o), 0);
    assert!(!err(&o).contains("removed "), "{}", err(&o));
    for kept in ["share/runbook/INSTALLING", "share/runbook/VERSION", "share/runbook/kept.txt", "share/runbook/protocol/file.txt", "bin/runbook"] {
        assert!(prefix.join(kept).is_file(), "{kept}: {}", err(&o));
    }
}
