//! A release install takes what the build wrote, and nothing older. A stub stands in for cargo.

use super::*;

// ---- a stale binary ----

/// Without `--from-built` the script takes `<crate>/target/release/<name>` after the build. With a triple set
/// (`CARGO_BUILD_TARGET`, or `build.target` of a cargo configuration file) cargo writes to
/// `<target-dir>/<triple>/release/` (checked with the real cargo on a scratch crate), and the script installs the
/// older release build that lies in `target/release`, with no check of its commit: the check `--from-built` makes is
/// not made after a build. The stub stands in for cargo; the older build prints another commit than HEAD.
#[test]
fn a_release_install_with_a_cross_triple_installs_the_older_release_build_of_another_commit() {
    let root = fake_checkout("fix2-triple", None);
    let (sandbox, prefix) = sandbox_of(&root);
    let head = head12(&root);
    for n in std::iter::once("runbook").chain(HELPERS) {
        write_stub(&root.join(built("release", n).strip_prefix(repo()).unwrap()), "runbook 0.0.1 (000000000000)");
    }
    let cargo = format!(
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
        done\n",
        version = stub_version(&head)
    );
    executable(&sandbox.join("stub-bin/cargo"), &cargo);
    let o = Command::new("sh")
        .arg(root.join("scripts/install.sh"))
        .arg("--prefix")
        .arg(&prefix)
        .env_clear()
        .env("PATH", format!("{}:{SYSTEM_PATH}", sandbox.join("stub-bin").display()))
        .env("HOME", sandbox.join("home"))
        .env("CARGO_BUILD_TARGET", "x86_64-unknown-linux-gnu")
        .stdin(Stdio::null())
        .output()
        .unwrap();
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
