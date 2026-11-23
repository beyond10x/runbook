//! What `scripts/install.sh` copies and how, the modes it sets, the lines it prints, the marker of a half install,
//! the check of the commit with `--from-built`, the refused arguments and prefixes, and the layout of `libexec`.
//! Fake checkouts (`fake_checkout`: a git work tree of its own; `fake_archive`: no git) and the shared prefix.

use super::*;

/// A fake checkout, its sandbox and an empty prefix in it.
fn sandbox_of(root: &Path) -> (PathBuf, PathBuf) {
    let sandbox = root.parent().unwrap().to_path_buf();
    let prefix = sandbox.join("prefix");
    fs::create_dir_all(&prefix).unwrap();
    fs::create_dir_all(sandbox.join("home")).unwrap();
    (sandbox, prefix)
}

fn from_built(root: &Path, prefix: &Path) -> Output {
    install(
        &root.join("scripts/install.sh"),
        prefix,
        &["--from-built", "debug"],
    )
}

fn head12(root: &Path) -> String {
    git_in(root, &["rev-parse", "HEAD"])
        .chars()
        .take(12)
        .collect()
}

/// Commits `paths` (added by force) and makes the stubs print the new HEAD, as a build of that HEAD would.
fn recommit(root: &Path, paths: &[&str]) {
    let mut add = vec!["add", "-f", "--"];
    add.extend_from_slice(paths);
    git_in(root, &add);
    git_in(root, &["commit", "-q", "-m", "more"]);
    stubs(root, None, &stub_version(&head12(root)));
}

fn mode(p: &Path) -> u32 {
    fs::symlink_metadata(p).unwrap().permissions().mode() & 0o7777
}

fn manifest(prefix: &Path) -> Vec<String> {
    fs::read_to_string(prefix.join("share/runbook/MANIFEST"))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

// ---- what is copied ----

/// In a git work tree the copy is what git tracks: a tracked dot-file and a tracked link are copied; an untracked file,
/// a dot-file, a link out, a login-looking name, what a run or a build left, and the script itself are not.
#[test]
fn nothing_that_is_not_a_runbook_file_is_copied() {
    let root = fake_checkout("tracked", None);
    let (sandbox, prefix) = sandbox_of(&root);
    fs::write(root.join("driver/examples/sdc/.review-reject"), "").unwrap();
    std::os::unix::fs::symlink("file.txt", root.join("bindings/alias.txt")).unwrap();
    recommit(
        &root,
        &["driver/examples/sdc/.review-reject", "bindings/alias.txt"],
    );
    let outside = sandbox.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.txt"), "outside\n").unwrap();
    std::os::unix::fs::symlink("../../outside", root.join("tools/out")).unwrap();
    let left = [
        "bindings/untracked.yaml",
        "bindings/.hidden",
        "ui/web/.npmrc",
        "tools/.netrc",
        "tools/credentials.json",
        "tools/id_ed25519",
        "bindings/settings.json",
        "bindings/runs/r1/outcome.json",
        "views/target/debug/a",
        "ui/web/node_modules/x/index.js",
        "protocol/.cache/a",
        "instructions/notes.txt~",
        "ui/playwright.config.mjs",
        "ui/notes.txt",
    ];
    for f in left {
        fs::create_dir_all(root.join(f).parent().unwrap()).unwrap();
        fs::write(root.join(f), "left behind\n").unwrap();
    }
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    let share = prefix.join("share/runbook");
    for kept in [
        "protocol/file.txt",
        "bindings/file.txt",
        "bindings/tools/sdc-tool.sh",
        "tools/file.txt",
        "ui/web/file.txt",
        "ui/server.mjs",
        "ui/package.json",
        "driver/examples/sdc/.review-reject",
    ] {
        assert!(
            share.join(kept).is_file(),
            "{kept} is tracked and is not in the prefix"
        );
    }
    assert_eq!(
        fs::read_link(share.join("bindings/alias.txt")).unwrap(),
        Path::new("file.txt"),
        "a tracked link is a link in the prefix"
    );
    let copied: Vec<&str> = left
        .into_iter()
        .chain(["tools/out", "scripts/install.sh"])
        .filter(|f| fs::symlink_metadata(share.join(f)).is_ok())
        .collect();
    assert_eq!(
        copied,
        Vec::<&str>::new(),
        "left out by the rule, and in the prefix"
    );
    let listed = manifest(&prefix);
    for f in [
        "share/runbook/driver/examples/sdc/.review-reject",
        "share/runbook/bindings/alias.txt",
    ] {
        assert!(listed.iter().any(|l| l == f), "{f} is in the MANIFEST");
    }
    assert!(
        !listed.iter().any(|l| l.ends_with("install.sh")
            || l.contains("untracked")
            || l.contains("credentials")),
        "{listed:?}"
    );
    assert_eq!(files_below(&outside).len(), 1);
}

/// Without git the copy is every regular file that is not a dot-file, no link, nothing below `runs/`, `target/`
/// or `node_modules/`, and not the script.
#[test]
fn an_archive_without_git_gives_the_regular_files_that_are_not_dot_files() {
    let root = fake_archive("archive");
    let (sandbox, prefix) = sandbox_of(&root);
    let outside = sandbox.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.txt"), "outside\n").unwrap();
    std::os::unix::fs::symlink("../../outside", root.join("tools/out")).unwrap();
    std::os::unix::fs::symlink("file.txt", root.join("bindings/alias.txt")).unwrap();
    let left = [
        "bindings/.hidden",
        "ui/web/.npmrc",
        "protocol/.cache/a",
        "bindings/runs/r1/outcome.json",
        "views/target/debug/a",
        "ui/web/node_modules/x/index.js",
        "ui/playwright.config.mjs",
        "ui/notes.txt",
    ];
    for f in left {
        fs::create_dir_all(root.join(f).parent().unwrap()).unwrap();
        fs::write(root.join(f), "left behind\n").unwrap();
    }
    fs::write(root.join("bindings/extra.yaml"), "extra: true\n").unwrap();
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    assert!(
        err(&o).contains("no git work tree"),
        "the rule used is said: {}",
        err(&o)
    );
    let share = prefix.join("share/runbook");
    for kept in [
        "protocol/file.txt",
        "bindings/extra.yaml",
        "ui/server.mjs",
        "ui/package.json",
    ] {
        assert!(share.join(kept).is_file(), "{kept}");
    }
    let copied: Vec<&str> = left
        .into_iter()
        .chain(["tools/out", "bindings/alias.txt", "scripts/install.sh"])
        .filter(|f| fs::symlink_metadata(share.join(f)).is_ok())
        .collect();
    assert_eq!(copied, Vec::<&str>::new());
    assert_eq!(
        fs::read_to_string(share.join("VERSION")).unwrap().trim(),
        stub_version("unknown")
    );
}

/// What is copied from the real checkout is exactly what git tracks there (the asset directories, the pages of `ui/`,
/// not the script), and nothing more.
#[test]
fn the_prefix_of_this_checkout_holds_exactly_the_tracked_files_of_the_asset_directories() {
    let prefix = shared_prefix();
    let o = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--",
            "protocol",
            "scripts",
            "driver/examples/sdc",
            "bindings",
            "instructions",
            "views",
            "cycles",
            "tools",
            "ui",
        ])
        .current_dir(repo())
        .output()
        .unwrap();
    let mut want: BTreeSet<String> = BTreeSet::new();
    for f in String::from_utf8(o.stdout)
        .unwrap()
        .split('\0')
        .filter(|f| !f.is_empty())
    {
        let page = f.strip_prefix("ui/").is_some_and(|r| {
            !r.contains('/')
                && (r.ends_with(".mjs") && !r.starts_with("playwright") || r == "package.json")
        });
        let in_dir = !f.starts_with("ui/") || f.starts_with("ui/web/");
        if (in_dir || page) && f != "scripts/install.sh" {
            want.insert(format!("share/runbook/{f}"));
        }
    }
    let have: BTreeSet<String> = manifest(&prefix)
        .into_iter()
        .filter(|l| {
            l.starts_with("share/runbook/") && !l.ends_with("/VERSION") && !l.ends_with("/MANIFEST")
        })
        .collect();
    let missing: Vec<&String> = want.difference(&have).take(5).collect();
    let extra: Vec<&String> = have.difference(&want).take(5).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "tracked and not in the prefix: {missing:?}; in the prefix and not tracked: {extra:?}"
    );
    for f in &want {
        assert!(
            fs::symlink_metadata(prefix.join(f)).is_ok(),
            "{f} is in the MANIFEST and not in the prefix"
        );
    }
}

// ---- modes ----

/// The mode comes from the index, never from the work tree and never from the umask: a tracked 100755 file whose
/// work-tree copy lost its bit is executable, a tracked 100644 file that is world-writable or set-id is 644, and every
/// directory is 755, though the install runs under `umask 077`.
#[test]
fn modes_come_from_the_index_whatever_the_work_tree_or_the_umask_say() {
    let root = fake_checkout("modes-index", None);
    let (_sandbox, prefix) = sandbox_of(&root);
    fs::set_permissions(
        root.join("bindings/tools/sdc-tool.sh"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    fs::set_permissions(
        root.join("protocol/file.txt"),
        fs::Permissions::from_mode(0o4777),
    )
    .unwrap();
    fs::set_permissions(
        root.join("views/file.txt"),
        fs::Permissions::from_mode(0o666),
    )
    .unwrap();
    let o = Command::new("sh")
        .args(["-c", "umask 077 && exec sh \"$0\" \"$@\""])
        .arg(root.join("scripts/install.sh"))
        .arg("--prefix")
        .arg(&prefix)
        .args(["--from-built", "debug"])
        .env_clear().envs(NO_SYSTEM_GIT)
        .env("PATH", SYSTEM_PATH)
        .env("HOME", prefix.parent().unwrap().join("home"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    let m = |rel: &str| mode(&prefix.join(rel));
    assert_eq!(m("share/runbook/bindings/tools/sdc-tool.sh"), 0o755);
    assert_eq!(
        m("share/runbook/protocol/file.txt"),
        0o644,
        "set-id and world-writable bits are not carried"
    );
    assert_eq!(m("share/runbook/views/file.txt"), 0o644);
    assert_eq!(m("bin/runbook"), 0o755);
    assert_eq!(m("libexec/runbook/sdc-node"), 0o755);
    for l in listing(&prefix) {
        if let Some(dir) = l.strip_suffix('/') {
            assert_eq!(m(dir), 0o755, "directory {dir}");
        }
    }
}

/// Without git the owner's execute bit decides: 755 or 644, whatever else the work tree says.
#[test]
fn an_archive_gets_755_for_an_owner_executable_file_and_644_for_the_others() {
    let root = fake_archive("modes-archive");
    let (_sandbox, prefix) = sandbox_of(&root);
    fs::set_permissions(
        root.join("bindings/tools/sdc-tool.sh"),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    fs::set_permissions(
        root.join("protocol/file.txt"),
        fs::Permissions::from_mode(0o664),
    )
    .unwrap();
    fs::set_permissions(
        root.join("views/file.txt"),
        fs::Permissions::from_mode(0o2666),
    )
    .unwrap();
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    let m = |rel: &str| mode(&prefix.join("share/runbook").join(rel));
    assert_eq!(
        (
            m("bindings/tools/sdc-tool.sh"),
            m("protocol/file.txt"),
            m("views/file.txt")
        ),
        (0o755, 0o644, 0o644)
    );
}

/// Every file with the mode 100755 in the index of this checkout is executable in the prefix (the adapters and
/// tools the bindings start), and no other file is: 755 and 644 as git has them.
#[test]
fn every_file_executable_in_the_index_is_executable_in_the_prefix_and_no_other() {
    let prefix = shared_prefix();
    let o = Command::new("git")
        .args(["ls-files", "-s", "-z"])
        .current_dir(repo())
        .output()
        .unwrap();
    let text = String::from_utf8(o.stdout).unwrap();
    let modes: std::collections::BTreeMap<&str, &str> = text
        .split('\0')
        .filter_map(|e| {
            e.split_once('\t')
                .map(|(meta, path)| (path, meta.split(' ').next().unwrap()))
        })
        .collect();
    let mut executable = 0;
    for rel in manifest(&prefix) {
        let Some(path) = rel.strip_prefix("share/runbook/") else {
            continue;
        };
        let Some(git_mode) = modes.get(path) else {
            continue;
        };
        if *git_mode == "120000" {
            continue;
        }
        let want = if *git_mode == "100755" { 0o755 } else { 0o644 };
        executable += usize::from(want == 0o755);
        assert_eq!(
            mode(&prefix.join(&rel)),
            want,
            "{rel} is {git_mode} in the index"
        );
    }
    assert!(
        executable >= 2,
        "the checkout has executable tools: {executable}"
    );
}

// ---- the printed lines ----

/// The first directory of `PATH` after a shell has run the line the script printed for it, or `None` when the shell is
/// not installed.
fn first_path_entry(shell: &str, line: &str, home: &Path) -> Option<String> {
    let command = line
        .split_once("):")
        .unwrap_or_else(|| panic!("no label in {line}"))
        .1
        .trim();
    let script = match shell {
        "fish" => format!("{command}; printf '%s\\n' $fish_user_paths[1]"),
        _ => format!("{command}\nprintf '%s\\n' \"${{PATH%%:*}}\""),
    };
    let mut cmd = Command::new(shell);
    match shell {
        "fish" => cmd.args(["--no-config", "-c"]),
        "zsh" => cmd.args(["-f", "-c"]),
        _ => cmd.args(["--norc", "--noprofile", "-c"]),
    };
    let o = match cmd
        .arg(&script)
        .env_clear()
        .env("PATH", SYSTEM_PATH)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .stdin(Stdio::null())
        .output()
    {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => panic!("{shell}: {e}"),
    };
    assert_eq!(code(&o), 0, "{shell} on {line}: {}{}", out(&o), err(&o));
    Some(out(&o).trim_end_matches('\n').to_owned())
}

/// The three lines name one directory, `<prefix>/bin`, for the shell they are for, whatever characters the prefix
/// holds: a space, `"`, `$`, a backquote, `'` and `\`. Evaluated by every one of the three shells installed here (a
/// shell that is not installed is named in the output and skipped).
#[test]
fn the_printed_lines_give_one_directory_in_bash_zsh_and_fish_for_a_prefix_with_odd_characters() {
    let root = fake_checkout("lines", None);
    let (sandbox, _) = sandbox_of(&root);
    let mut skipped = BTreeSet::new();
    for name in [
        "my prefix",
        "dq\"x",
        "dollar$x",
        "back`tick",
        "sq'x",
        "back\\slash",
        "plain",
    ] {
        let prefix = sandbox.join(name);
        fs::create_dir_all(&prefix).unwrap();
        let o = from_built(&root, &prefix);
        assert_eq!(code(&o), 0, "{name}: {}{}", out(&o), err(&o));
        let said = out(&o);
        let want = format!("{}/bin", prefix.display());
        for (label, shell) in [("bash", "bash"), ("zsh", "zsh"), ("fish", "fish")] {
            let line = said
                .lines()
                .find(|l| l.trim_start().starts_with(label))
                .unwrap_or_else(|| panic!("{name}: no {label} line:\n{said}"));
            if label != "fish" {
                assert!(
                    line.contains("export PATH=\"") && line.trim_end().ends_with(":$PATH\""),
                    "{name}: the {label} line exports PATH: {line}"
                );
            }
            match first_path_entry(shell, line, &sandbox.join("home")) {
                Some(first) => assert_eq!(
                    first, want,
                    "{name}: the {label} line gives {first:?}: {line}"
                ),
                None => {
                    skipped.insert(shell);
                }
            }
        }
    }
    for shell in &skipped {
        println!("skipped: {shell} is not installed here");
    }
}

// ---- the marker of a half install ----

/// An install that stops (here: a directory where a file goes) leaves `share/runbook/INSTALLING` and says so; the next
/// install over it finishes the job and removes the marker; so does one over a marker a killed install left.
#[test]
fn an_install_that_stops_leaves_the_marker_and_the_next_install_finishes_the_job() {
    let root = fake_checkout("marker", None);
    let (_sandbox, prefix) = sandbox_of(&root);
    let marker = prefix.join("share/runbook/INSTALLING");
    let obstacle = prefix.join("share/runbook/protocol/file.txt");
    fs::create_dir_all(&obstacle).unwrap();
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 1, "{}{}", out(&o), err(&o));
    assert!(marker.is_file(), "the marker stays");
    assert!(err(&o).contains("did not finish"), "{}", err(&o));
    assert!(
        !prefix.join("share/runbook/MANIFEST").exists()
            && !prefix.join("share/runbook/VERSION").exists()
    );
    fs::remove_dir(&obstacle).unwrap();
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    assert!(!marker.exists(), "the marker is the last thing removed");
    assert!(
        prefix.join("share/runbook/MANIFEST").is_file()
            && prefix.join("share/runbook/VERSION").is_file()
    );
    assert!(!manifest(&prefix).iter().any(|l| l.ends_with("INSTALLING")));
    fs::write(&marker, "left by a killed install\n").unwrap();
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    assert!(!marker.exists());
}

/// `runbook doctor` of an installed layout with the marker says `incomplete install`, in one line, and exits 1; the same
/// layout without it passes (`the_installed_doctor_finds...`). The binary and the files are those of the shared prefix (a hard link
/// and links), in a sandbox prefix of its own.
#[test]
fn doctor_of_a_prefix_with_the_marker_says_incomplete_install_and_fails() {
    let shared = shared_prefix();
    let sandbox = scratch("doctor-marker");
    let prefix = sandbox.join("prefix");
    fs::create_dir_all(prefix.join("bin")).unwrap();
    fs::create_dir_all(prefix.join("share/runbook")).unwrap();
    fs::hard_link(shared.join("bin/runbook"), prefix.join("bin/runbook")).unwrap();
    std::os::unix::fs::symlink(shared.join("libexec"), prefix.join("libexec")).unwrap();
    for e in fs::read_dir(shared.join("share/runbook"))
        .unwrap()
        .flatten()
    {
        std::os::unix::fs::symlink(e.path(), prefix.join("share/runbook").join(e.file_name()))
            .unwrap();
    }
    let doctor = |p: &Path| from_prefix(p, &sandbox, &["doctor", "--for", "sdc"]);
    let install_line = |o: &Output| {
        out(o)
            .lines()
            .find(|l| {
                l.split("  ")
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .nth(1)
                    == Some("install")
            })
            .unwrap_or_else(|| panic!("no install line:\n{}", out(o)))
            .to_owned()
    };
    fs::write(prefix.join("share/runbook/INSTALLING"), "").unwrap();
    let o = doctor(&prefix);
    let line = install_line(&o);
    assert!(
        line.starts_with("fail")
            && line.contains("incomplete install")
            && line.contains("INSTALLING"),
        "{line}"
    );
    assert_eq!(code(&o), 1, "{}", out(&o));
}

// ---- --from-built checks the commit ----

/// With `--from-built` the commit `runbook --version` prints is HEAD of the checkout, or the script refuses with both
/// values, before it writes anything. `unknown` on one side only is refused too; `unknown` on both is not (an archive).
#[test]
fn from_built_refuses_a_binary_of_another_commit_and_names_both_commits() {
    let root = fake_checkout("commit-git", None);
    let (sandbox, prefix) = sandbox_of(&root);
    let head = head12(&root);
    for (built_says, case) in [
        ("0123456789ab", "another commit"),
        ("unknown", "unknown against a commit"),
    ] {
        write_stub(
            &root.join(built("debug", "runbook").strip_prefix(repo()).unwrap()),
            &stub_version(built_says),
        );
        let before = listing(&sandbox);
        let o = from_built(&root, &prefix);
        assert_eq!(code(&o), 1, "{case}: {}{}", out(&o), err(&o));
        let said = err(&o);
        assert_eq!(said.lines().count(), 1, "{case}: one line: {said}");
        assert!(
            said.contains(built_says) && said.contains(&head),
            "{case}: both values: {said}"
        );
        assert_eq!(
            listing(&sandbox),
            before,
            "{case}: the refusal wrote something"
        );
    }
    let archive = fake_archive("commit-archive");
    let (_sandbox, prefix) = sandbox_of(&archive);
    write_stub(
        &archive.join(built("debug", "runbook").strip_prefix(repo()).unwrap()),
        &stub_version("0123456789ab"),
    );
    let o = from_built(&archive, &prefix);
    assert_eq!(code(&o), 1, "{}{}", out(&o), err(&o));
    assert!(
        err(&o).contains("0123456789ab") && err(&o).contains("unknown"),
        "{}",
        err(&o)
    );
    stubs(&archive, None, &stub_version("unknown"));
    let o = from_built(&archive, &prefix);
    assert_eq!(
        code(&o),
        0,
        "unknown on both sides is allowed: {}{}",
        out(&o),
        err(&o)
    );
}

// ---- refused arguments and prefixes ----

#[test]
fn an_empty_value_and_a_repeated_option_are_refused_as_usage_errors() {
    let w = scratch("refuse-values");
    let cases: Vec<(Vec<&str>, &str)> = vec![
        (vec!["--prefix", "", "--from-built", "debug"], "empty"),
        (vec!["--prefix", "/x", "--from-built", ""], "empty"),
        (vec!["--prefix", "/x", "--prefix", "/y"], "twice"),
        (
            vec![
                "--prefix",
                "/x",
                "--from-built",
                "debug",
                "--from-built",
                "release",
            ],
            "twice",
        ),
    ];
    for (args, word) in cases {
        let o = Command::new("sh")
            .arg(script())
            .args(&args)
            .env_clear()
            .env("PATH", SYSTEM_PATH)
            .env("HOME", &w)
            .current_dir(&w)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(code(&o), 2, "{args:?}: {}{}", out(&o), err(&o));
        let first = err(&o).lines().next().unwrap_or_default().to_owned();
        assert!(
            first.contains(word),
            "{args:?}: the first line says {word:?}: {first}"
        );
        assert!(err(&o).contains("usage:"), "{args:?}: {}", err(&o));
        assert!(
            listing(&w).is_empty(),
            "{args:?}: a refusal wrote something"
        );
    }
}

#[test]
fn a_prefix_with_a_newline_in_its_name_or_inside_a_copied_directory_is_refused_and_nothing_is_written(
) {
    let root = fake_checkout("refuse-prefix", None);
    let (sandbox, _) = sandbox_of(&root);
    let with_newline = sandbox.join("pre\nfix");
    fs::create_dir_all(&with_newline).unwrap();
    let ends_with_newline = sandbox.join("prefix-2\n");
    fs::create_dir_all(&ends_with_newline).unwrap();
    let mut inside = Vec::new();
    for d in [
        "protocol",
        "bindings/tools",
        "ui/web",
        "driver/examples/sdc",
        "ui",
    ] {
        let p = root.join(d).join("p");
        fs::create_dir_all(&p).unwrap();
        inside.push(p);
    }
    let before = listing(&sandbox);
    for (prefix, word) in [(with_newline, "newline"), (ends_with_newline, "newline")]
        .into_iter()
        .chain(inside.into_iter().map(|p| (p, "inside")))
    {
        let o = from_built(&root, &prefix);
        assert_eq!(code(&o), 2, "{}: {}{}", prefix.display(), out(&o), err(&o));
        assert!(
            err(&o).lines().next().unwrap_or_default().contains(word),
            "{}: {}",
            prefix.display(),
            err(&o)
        );
        assert_eq!(
            listing(&sandbox),
            before,
            "{}: the refusal wrote something",
            prefix.display()
        );
    }
}

// ---- the layout of libexec ----

/// The helpers are in `<prefix>/libexec/runbook`, a directory of this product alone: `libexec` holds nothing else.
#[test]
fn the_helpers_are_in_libexec_runbook_and_libexec_holds_nothing_else() {
    let prefix = shared_prefix();
    let names = |d: &Path| -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(d)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    };
    assert_eq!(names(&prefix.join("libexec")), ["runbook"]);
    assert_eq!(names(&prefix.join("libexec/runbook")), HELPERS);
    assert_eq!(names(&prefix.join("bin")), ["runbook"]);
}
