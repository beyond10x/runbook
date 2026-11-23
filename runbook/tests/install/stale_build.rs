//! A release install takes what the build wrote, and nothing older. A stub stands in for cargo.

use super::*;

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
        .env_clear()
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
