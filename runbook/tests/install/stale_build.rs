//! A release install takes what the build wrote, and nothing older. A stub stands in for cargo.

use super::*;
use crate::build_commit::commit_source;

// ---- what the build made ----

/// A stub `cargo build`: writes a shell program for every `--bin` below `<target-dir>/release/` (below
/// `<target-dir>/<triple>/release/` when `CARGO_BUILD_TARGET` is set, as cargo does) that prints `version`, and reports it
/// the way `--message-format=json-render-diagnostics` does.
fn cargo_stub(version: &str) -> String {
    format!(
        "#!/bin/sh\n\
        to=\n\
        before=\n\
        for a in \"$@\"; do\n\
            [ \"$before\" = --target-dir ] && to=$a\n\
            before=$a\n\
        done\n\
        [ -n \"${{CARGO_BUILD_TARGET:-}}\" ] && to=$to/$CARGO_BUILD_TARGET\n\
        mkdir -p \"$to/release\"\n\
        before=\n\
        for a in \"$@\"; do\n\
            if [ \"$before\" = --bin ]; then\n\
                printf '#!/bin/sh\\necho \"{version}\"\\n' > \"$to/release/$a\"\n\
                chmod 755 \"$to/release/$a\"\n\
                printf '{{\"reason\":\"compiler-artifact\",\"executable\":\"%s\"}}\\n' \"$to/release/$a\"\n\
            fi\n\
            before=$a\n\
        done\n"
    )
}

/// The release install of a fake checkout with the stub cargo on `PATH` and `vars` set.
fn release_install(root: &Path, sandbox: &Path, prefix: &Path, vars: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new("sh");
    cmd.arg(root.join("scripts/install.sh"))
        .arg("--prefix")
        .arg(prefix)
        .env_clear().envs(crate::kept()).envs(NO_SYSTEM_GIT)
        .env("PATH", format!("{}:{SYSTEM_PATH}", sandbox.join("stub-bin").display()))
        .env("HOME", sandbox.join("home"))
        .stdin(Stdio::null());
    for (k, v) in vars {
        cmd.env(k, v);
    }
    cmd.output().unwrap()
}

/// Without `--from-built` the script takes `<crate>/target/release/<name>` after the build. With a triple set
/// (`CARGO_BUILD_TARGET`, or `build.target` of a cargo configuration file) cargo writes to
/// `<target-dir>/<triple>/release/` (checked with the real cargo on a scratch crate), where an older release build of
/// another commit may lie in `target/release`. The script asks cargo which files it built, stops at a file that is not
/// the one it would take and names the cause, and writes nothing. The stub stands in for cargo.
#[test]
fn a_release_install_with_a_cross_triple_is_refused_by_name_and_the_older_release_build_is_not_installed() {
    let root = fake_checkout("fix2-triple", None);
    let (sandbox, prefix) = sandbox_of(&root);
    let head = head12(&root);
    for n in std::iter::once("runbook").chain(HELPERS) {
        write_stub(&root.join(built("release", n).strip_prefix(repo()).unwrap()), "runbook 0.0.1 (000000000000)");
    }
    executable(&sandbox.join("stub-bin/cargo"), &cargo_stub(&stub_version(&head)));
    let o = release_install(&root, &sandbox, &prefix, &[("CARGO_BUILD_TARGET", "x86_64-unknown-linux-gnu")]);
    let version = fs::read_to_string(prefix.join("share/runbook/VERSION")).unwrap_or_default();
    assert!(root.join("watch/target/x86_64-unknown-linux-gnu/release/sdc-watch").is_file(), "the stub built below the triple: {}", err(&o));
    assert!(
        !(code(&o) == 0 && version.contains("000000000000")),
        "exit 0 and VERSION {:?}: the older release build of another commit is installed, HEAD of the checkout is {head}\n{}",
        version.trim(),
        err(&o)
    );
    assert_eq!(code(&o), 1, "{}", err(&o));
    assert!(err(&o).contains("target triple") && err(&o).contains("CARGO_BUILD_TARGET"), "the refusal names the cause: {}", err(&o));
    assert_eq!(listing(&prefix), Vec::<String>::new(), "nothing is written");
}

/// The commit of the binary a build made is compared with HEAD, as it is for `--from-built`: a build whose `runbook`
/// prints another commit stops the install with both commits named, and nothing is written.
#[test]
fn a_release_build_that_prints_another_commit_than_head_is_refused_with_both_commits_and_nothing_is_written() {
    let root = fake_checkout("fix2-built-commit", None);
    let (sandbox, prefix) = sandbox_of(&root);
    let head = head12(&root);
    executable(&sandbox.join("stub-bin/cargo"), &cargo_stub("runbook 0.0.1 (000000000000)"));
    let o = release_install(&root, &sandbox, &prefix, &[]);
    assert_eq!(code(&o), 1, "{}", err(&o));
    assert!(err(&o).contains("000000000000") && err(&o).contains(&head), "both commits are named: {}", err(&o));
    assert_eq!(listing(&prefix), Vec::<String>::new(), "nothing is written");
}

/// The hole the unit names in `build_commit.rs`, and what catches it. The branch HEAD names is moved from another work
/// tree (`git update-ref`): no file the build watches changes, so cargo compiles nothing and the binary keeps the commit
/// it was built from. The script compares that commit with HEAD, stops, names both and writes nothing.
#[test]
fn a_branch_moved_from_another_work_tree_changes_no_watched_file_and_the_install_refuses_the_binary() {
    let root = fake_checkout("moved-elsewhere", None);
    let (sandbox, prefix) = sandbox_of(&root);
    fs::write(root.join("runbook/Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
    git_in(&root, &["add", "runbook/Cargo.toml"]);
    git_in(&root, &["commit", "-q", "-m", "the crate"]);
    let built_from = head12(&root);
    stubs(&root, None, &stub_version(&built_from));
    let branch = git_in(&root, &["symbolic-ref", "HEAD"]);
    let manifest = root.join("runbook");
    let before = commit_source::commit(&manifest);
    assert_eq!(before.value, built_from);
    assert!(!before.watch.is_empty());
    let state = |paths: &[PathBuf]| -> Vec<(PathBuf, Vec<u8>, std::time::SystemTime)> {
        paths.iter().map(|p| (p.clone(), fs::read(p).unwrap(), fs::metadata(p).unwrap().modified().unwrap())).collect()
    };
    let watched = state(&before.watch);

    let other = sandbox.join("other-work-tree");
    git_in(&root, &["worktree", "add", "-q", "-b", "side", other.to_str().unwrap()]);
    fs::write(other.join("protocol/more.txt"), "more\n").unwrap();
    git_in(&other, &["add", "protocol/more.txt"]);
    git_in(&other, &["commit", "-q", "-m", "made in the other work tree"]);
    git_in(&other, &["update-ref", &branch, "side"]);

    let head = head12(&root);
    assert_ne!(head, built_from, "HEAD of the checkout moved");
    assert_eq!(state(&before.watch), watched, "no watched file changed: cargo compiles nothing");
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 1, "{}", err(&o));
    assert!(err(&o).contains(&built_from) && err(&o).contains(&head), "both commits are named: {}", err(&o));
    assert_eq!(listing(&prefix), Vec::<String>::new(), "nothing is written");
}

/// A checkout whose refs are in the reftable format cannot be installed: the build says the commit `unknown` there (it
/// has no file to watch for HEAD), and HEAD is a commit. The script stops, names both and writes nothing.
#[test]
fn a_reftable_checkout_is_refused_because_the_binary_cannot_name_its_commit() {
    let root = fake_tree("reftable");
    let init = Command::new("git").args(["init", "-q", "--ref-format=reftable"]).current_dir(&root).env_remove("GIT_DIR").env_remove("GIT_WORK_TREE").output().unwrap();
    if !init.status.success() {
        eprintln!("this git has no --ref-format=reftable: nothing checked");
        return;
    }
    git_in(&root, &["add", "-A"]);
    git_in(&root, &["commit", "-q", "-m", "a reftable checkout"]);
    stubs(&root, None, &stub_version("unknown"));
    let (_sandbox, prefix) = sandbox_of(&root);
    let head = head12(&root);
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 1, "{}", err(&o));
    assert!(err(&o).contains("unknown") && err(&o).contains(&head), "both are named: {}", err(&o));
    assert_eq!(listing(&prefix), Vec::<String>::new(), "nothing is written");
}
