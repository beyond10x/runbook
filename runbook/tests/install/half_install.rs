//! What an install that stops, or that finds a link at the place of a name it writes, leaves behind: the marker, the
//! order of the last renames, and what `runbook doctor` of such a prefix says. Fake checkouts and stub programs; every
//! prefix lies under the target directory.

use super::*;

// ---- nothing outside the prefix ----

/// The script clears a link at the place of every file it writes (`clear_name`, `rm -f` before `VERSION.new` and
/// `MANIFEST.new`), but not at the place of its marker: `printf ... > "$share/INSTALLING"` follows a link that
/// lies there. A file outside the prefix is overwritten, and a link to a path that does not exist makes that file.
#[test]
fn the_marker_is_written_through_a_link_at_its_place_into_a_file_outside_the_prefix() {
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

/// An install that stops before `share/runbook/protocol/` is written (the asset directories are copied in the order of
/// their names: `bindings`, `cycles`, `driver`, `instructions` come first and hold most of the files) leaves
/// `bin/runbook`, the helpers and the marker. That `runbook` does not count itself as installed (`Lookup::locate`
/// asks for `share/runbook/protocol`), walks up from its working directory, finds the checkout the person installed
/// from, and `runbook doctor` reports the checkout's files with no word about the install that did not finish. Here a
/// stub `cp` fails at its fifth call, as on a full disk.
#[test]
fn doctor_of_an_install_that_stopped_before_protocol_was_copied_reports_the_checkout_and_no_incomplete_install() {
    let sandbox = scratch("fix2-half");
    let prefix = sandbox.join("prefix");
    fs::create_dir_all(&prefix).unwrap();
    fs::create_dir_all(sandbox.join("home")).unwrap();
    let cp = "#!/bin/sh\n\
        n=$(cat \"$FAIL_COUNT\" 2>/dev/null || echo 0)\n\
        n=$((n + 1))\n\
        echo \"$n\" > \"$FAIL_COUNT\"\n\
        if [ \"$n\" -eq 5 ]; then echo 'cp: No space left on device (the stub of the test)' >&2; exit 1; fi\n\
        exec /usr/bin/cp \"$@\"\n";
    executable(&sandbox.join("stub-bin/cp"), cp);
    let o = Command::new("sh")
        .arg(script())
        .arg("--prefix")
        .arg(&prefix)
        .args(["--from-built", "debug"])
        .env_clear()
        .env("PATH", format!("{}:{SYSTEM_PATH}", sandbox.join("stub-bin").display()))
        .env("HOME", sandbox.join("home"))
        .env("FAIL_COUNT", sandbox.join("cp-calls"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(code(&o), 1, "{}{}", out(&o), err(&o));
    assert!(prefix.join("share/runbook/INSTALLING").is_file(), "the marker of the stopped install is there");
    assert!(prefix.join("bin/runbook").is_file() && !prefix.join("share/runbook/protocol").exists(), "the install stopped after bin/runbook and before protocol/: {}", err(&o));
    let path = format!("{}:{SYSTEM_PATH}", prefix.join("bin").display());
    let doctor = Command::new("runbook").arg("doctor").env_clear().env("PATH", path).env("HOME", sandbox.join("home")).current_dir(repo()).stdin(Stdio::null()).output().unwrap();
    let text = out(&doctor);
    let assets = text.lines().find(|l| l.contains("  assets  ")).unwrap_or_default().split("  required").next().unwrap_or_default().trim_end().to_owned();
    assert!(
        text.contains("incomplete install"),
        "runbook doctor of a prefix with share/runbook/INSTALLING, started in the checkout, says nothing of the install that did not finish; its assets line: {assets}"
    );
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
