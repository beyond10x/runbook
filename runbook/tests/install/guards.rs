//! Adversary pass 1 of the unit `install-script-lays-out-a-prefix`. Untracked; not the implementor's.
//!
//! `red_*`: fails on the unit as committed (`6a71b257`), for the reason its name states.
//! `guard_*`: passes on the unit and fails when the guard it names is taken out of `scripts/install.sh`; no
//! committed case holds that guard. `ADV1_SCRIPT=<file>` runs the cases of a fake checkout on that copy of the
//! script instead of the tree's (a mutated copy in the adversary's scratch directory).

use super::*;

/// A fake checkout with the script under test, its sandbox and an empty prefix in it.
fn subject(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
    let root = fake_checkout(tag, None);
    if let Some(other) = std::env::var_os("ADV1_SCRIPT") {
        fs::copy(other, root.join("scripts/install.sh")).unwrap();
    }
    let sandbox = root.parent().unwrap().to_path_buf();
    let prefix = sandbox.join("prefix");
    fs::create_dir_all(&prefix).unwrap();
    fs::create_dir_all(sandbox.join("home")).unwrap();
    (root, sandbox, prefix)
}

fn from_built(root: &Path, prefix: &Path) -> Output {
    install(&root.join("scripts/install.sh"), prefix, &["--from-built", "debug"])
}

fn mode(p: &Path) -> u32 {
    fs::symlink_metadata(p).unwrap().permissions().mode() & 0o7777
}

// ---- red ----

/// `Lookup::path_with_libexec` with no `PATH` (or an empty one) joins `libexec` with one empty entry: the programs
/// of the run get `PATH=<prefix>/libexec:`. An empty entry is the working directory, and the directories a shell
/// searches without a `PATH` are gone, so the same run behaves otherwise than from the checkout build.
#[test]
fn red_an_installed_run_started_without_path_gives_its_programs_the_working_directory_as_path_and_no_system_directory() {
    let prefix = shared_prefix();
    let writer = "#!/bin/sh\n\
        printf '%s\\n' \"${PATH-<unset>}\" > path.txt\n\
        printf '#!/bin/sh\\necho ran-from-the-working-directory\\n' > planted\n\
        /bin/chmod +x planted\n\
        (planted 2>/dev/null || echo not-found) > planted.txt\n\
        (command -v ls 2>/dev/null || echo not-found) > ls.txt\n\
        echo \"kettle, teacup, saucer\" > note.txt\n";
    let run = |tag: &str, program: PathBuf| -> (String, String, String) {
        let w = world(tag);
        fs::write(w.join("bindings/tools/note-write.sh"), writer).unwrap();
        let with = |cwd: &Path, args: &[&str]| Command::new(&program).args(args).env_clear().env("HOME", cwd).current_dir(cwd).stdin(Stdio::null()).output().unwrap();
        run_example(&with, &w);
        let read = |n: &str| fs::read_to_string(w.join("runs/r1/work").join(n)).unwrap_or_default().trim_end().to_owned();
        (read("path.txt"), read("planted.txt"), read("ls.txt"))
    };
    let (path_checkout, planted_checkout, ls_checkout) = run("adv1-nopath-checkout", PathBuf::from(BIN));
    assert_eq!(planted_checkout, "not-found", "the checkout build: a file of the working directory is not started by its bare name (PATH {path_checkout:?})");
    assert_ne!(ls_checkout, "not-found", "the checkout build: a bound program finds ls (PATH {path_checkout:?})");
    let (path, planted, ls) = run("adv1-nopath-installed", prefix.join("bin/runbook"));
    assert_eq!(
        (path.split(':').filter(|d| d.is_empty()).count(), planted.as_str(), ls.as_str()),
        (0, "not-found", ls_checkout.as_str()),
        "installed, no PATH: the PATH of a bound program is {path:?} (an empty entry is the working directory); \
         a file the program wrote into its working directory, started by its bare name: {planted:?}; ls: {ls:?} (checkout build: {ls_checkout:?})"
    );
}

/// `clean_path` splits the path with `set -- ${rel%/*}` while pathname expansion is on: a directory name of the old
/// MANIFEST that is a pattern (`[p]rotocol`) is replaced by the name it matches in the working directory
/// (`protocol`, a directory of every checkout), the link check looks at that other name, and the file behind the
/// link is removed. The script is started from the checkout, as `README.md` prints the command.
#[test]
fn red_a_manifest_path_through_a_link_whose_name_is_a_pattern_is_removed_outside_the_prefix() {
    let (root, sandbox, prefix) = subject("adv1-pattern");
    assert_eq!(code(&from_built(&root, &prefix)), 0);
    let outside = sandbox.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("a.txt"), "outside\n").unwrap();
    std::os::unix::fs::symlink(&outside, prefix.join("share/runbook/[p]rotocol")).unwrap();
    let manifest_path = prefix.join("share/runbook/MANIFEST");
    let mut manifest = fs::read_to_string(&manifest_path).unwrap();
    manifest.push_str("share/runbook/[p]rotocol/a.txt\n");
    fs::write(&manifest_path, manifest).unwrap();
    let o = Command::new("sh")
        .arg("scripts/install.sh")
        .arg("--prefix")
        .arg(&prefix)
        .args(["--from-built", "debug"])
        .env_clear()
        .env("PATH", SYSTEM_PATH)
        .env("HOME", sandbox.join("home"))
        .current_dir(&root)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    assert!(outside.join("a.txt").is_file(), "a file outside the prefix was removed through a link:\n{}", err(&o));
}

/// The script's comment: the copy leaves out "any login or settings file". Its filter is `.env*`, `*credential*`
/// and `settings.local.json`; the committed case asserts no name that starts with `settings`, on a tree that holds
/// none.
#[test]
fn red_a_login_file_and_a_settings_file_of_the_checkout_are_copied_into_the_prefix() {
    let (root, _sandbox, prefix) = subject("adv1-login");
    fs::write(root.join("ui/web/.npmrc"), "//registry.npmjs.org/:_authToken=SECRET\n").unwrap();
    fs::write(root.join("tools/.netrc"), "machine example.invalid login a password SECRET\n").unwrap();
    fs::write(root.join("bindings/settings.json"), "{\"login_file\": \"/somewhere/login.json\"}\n").unwrap();
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    let share = prefix.join("share/runbook");
    assert!(share.join("tools/file.txt").is_file(), "the plain file of tools/ is copied");
    let copied: Vec<&str> = ["ui/web/.npmrc", "tools/.netrc", "bindings/settings.json"].into_iter().filter(|f| share.join(f).exists()).collect();
    assert_eq!(copied, Vec::<&str>::new(), "login and settings files of the checkout are in the prefix");
}

/// `cp -p` carries the mode of the source over: a checkout made under umask 002 (664 files, 775 programs) gives a
/// prefix whose helpers on `PATH` and whose bound tools are group-writable, whatever the umask of the install.
#[test]
fn red_a_group_writable_file_of_the_checkout_is_group_writable_in_the_prefix() {
    let (root, _sandbox, prefix) = subject("adv1-modes");
    fs::set_permissions(root.join("protocol/file.txt"), fs::Permissions::from_mode(0o664)).unwrap();
    fs::set_permissions(root.join("bindings/tools/sdc-tool.sh"), fs::Permissions::from_mode(0o775)).unwrap();
    fs::set_permissions(root.join("driver/target/debug/sdc-node"), fs::Permissions::from_mode(0o775)).unwrap();
    let o = Command::new("sh")
        .args(["-c", "umask 022 && exec sh \"$0\" \"$@\""])
        .arg(root.join("scripts/install.sh"))
        .arg("--prefix")
        .arg(&prefix)
        .args(["--from-built", "debug"])
        .env_clear()
        .env("PATH", SYSTEM_PATH)
        .env("HOME", prefix.parent().unwrap().join("home"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    assert_eq!(mode(&prefix.join("libexec/sdc-node")) & 0o111, 0o111, "the helper is executable");
    let writable: Vec<String> = files_below(&prefix).into_iter().filter(|rel| mode(&prefix.join(rel)) & 0o022 != 0).map(|rel| format!("{rel} {:o}", mode(&prefix.join(&rel)))).collect();
    assert_eq!(writable, Vec::<String>::new(), "group- or world-writable files in a prefix installed under umask 022");
}

/// The three lines the script prints are what `README.md` tells the reader to add to a profile. The bash and zsh
/// lines quote the directory; the fish line does not, and fish splits an unquoted argument at a space.
#[test]
fn red_the_fish_line_of_a_prefix_with_a_space_names_two_directories() {
    let (root, sandbox, _) = subject("adv1-fish");
    let prefix = sandbox.join("my prefix");
    fs::create_dir_all(&prefix).unwrap();
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    let said = out(&o);
    let bash = said.lines().find(|l| l.trim_start().starts_with("bash")).unwrap_or_else(|| panic!("no bash line: {said}"));
    assert!(bash.ends_with(&format!("export PATH=\"{}/bin:$PATH\"", prefix.display())), "{bash}");
    let fish = said.lines().find(|l| l.trim_start().starts_with("fish")).unwrap_or_else(|| panic!("no fish line: {said}"));
    let argument = fish.split("fish_add_path ").nth(1).unwrap_or_else(|| panic!("no fish_add_path: {fish}"));
    let dir = format!("{}/bin", prefix.display());
    assert!(argument == format!("'{dir}'") || argument == format!("\"{dir}\""), "the fish line names the directory unquoted: {fish}");
}

/// Without `--from-built` the script unsets `CARGO_TARGET_DIR` and then takes `<crate>/target/release/<name>`. With
/// `build.target-dir` set another way (`CARGO_BUILD_TARGET_DIR`, or a cargo configuration file above the checkout
/// or in the cargo home) cargo builds elsewhere and exits 0, and the script installs whatever older release build
/// lies in the checkout. The stub stands in for that cargo: it writes every `--bin` it is asked for into
/// `$CARGO_BUILD_TARGET_DIR/release` (checked with the real cargo on a scratch crate: the build lands there and the
/// crate's own `target/` keeps its old file).
#[test]
fn red_a_release_install_takes_an_old_binary_when_cargo_builds_into_another_target_directory() {
    let (root, sandbox, prefix) = subject("adv1-stale");
    for n in std::iter::once("runbook").chain(HELPERS) {
        let p = built("release", n);
        let p = root.join(p.strip_prefix(repo()).unwrap());
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, "#!/bin/sh\necho \"runbook 0.0.1 (000000000000)\"\n").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let stub_dir = sandbox.join("stub-bin");
    fs::create_dir_all(&stub_dir).unwrap();
    let cargo = "#!/bin/sh\n\
        to=$CARGO_BUILD_TARGET_DIR/release\n\
        mkdir -p \"$to\"\n\
        before=\n\
        for a in \"$@\"; do\n\
            if [ \"$before\" = --bin ]; then\n\
                printf '#!/bin/sh\\necho \"runbook 9.9.9 (0123456789ab)\"\\n' > \"$to/$a\"\n\
                chmod 755 \"$to/$a\"\n\
            fi\n\
            before=$a\n\
        done\n";
    fs::write(stub_dir.join("cargo"), cargo).unwrap();
    fs::set_permissions(stub_dir.join("cargo"), fs::Permissions::from_mode(0o755)).unwrap();
    let o = Command::new("sh")
        .arg(root.join("scripts/install.sh"))
        .arg("--prefix")
        .arg(&prefix)
        .env_clear()
        .env("PATH", format!("{}:{SYSTEM_PATH}", stub_dir.display()))
        .env("HOME", sandbox.join("home"))
        .env("CARGO_BUILD_TARGET_DIR", sandbox.join("elsewhere"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(sandbox.join("elsewhere/release/runbook").is_file(), "the stub cargo built into the other target directory: {}", err(&o));
    let version = fs::read_to_string(prefix.join("share/runbook/VERSION")).unwrap_or_default();
    assert!(
        code(&o) != 0 || version.trim() == "runbook 9.9.9 (0123456789ab)",
        "exit {} and VERSION {:?}: the script installed the release build that lay in the checkout, not the one this run built\n{}",
        code(&o),
        version.trim(),
        err(&o)
    );
}

// ---- guards no committed case holds ----

/// `clear_name`: a link at the place of a file the script writes is replaced, never written through.
#[test]
fn guard_a_link_at_the_place_of_a_file_is_replaced_and_never_written_through() {
    let (root, sandbox, prefix) = subject("adv1-link-at-file");
    let victim = sandbox.join("victim.txt");
    fs::write(&victim, "victim\n").unwrap();
    let places = ["bin/runbook", "libexec/sdc-node", "share/runbook/protocol/file.txt"];
    for place in places {
        fs::create_dir_all(prefix.join(place).parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&victim, prefix.join(place)).unwrap();
    }
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    assert_eq!(fs::read_to_string(&victim).unwrap(), "victim\n", "the script wrote through a link out of the prefix");
    for place in places {
        assert!(fs::symlink_metadata(prefix.join(place)).unwrap().is_file(), "{place} is a file of its own now");
    }
    assert_eq!(fs::read_to_string(prefix.join("share/runbook/protocol/file.txt")).unwrap(), "protocol\n");
}

/// The `bin/* | libexec/* | share/runbook/*` filter of the removal, and the removal of the directories it emptied.
#[test]
fn guard_an_old_manifest_line_outside_bin_libexec_and_share_runbook_is_not_removed_and_an_emptied_directory_is() {
    let (root, _sandbox, prefix) = subject("adv1-outside-three");
    assert_eq!(code(&from_built(&root, &prefix)), 0);
    fs::create_dir_all(prefix.join("etc")).unwrap();
    fs::create_dir_all(prefix.join("share/runbook/gone/deep")).unwrap();
    fs::create_dir_all(prefix.join("share/other")).unwrap();
    for f in ["notes.txt", "etc/conf", "share/other/theirs.txt", "share/runbook/gone/deep/old.txt"] {
        fs::write(prefix.join(f), "kept?\n").unwrap();
    }
    let manifest_path = prefix.join("share/runbook/MANIFEST");
    let mut manifest = fs::read_to_string(&manifest_path).unwrap();
    manifest.push_str("notes.txt\netc/conf\nshare/other/theirs.txt\nshare/runbook/gone/deep/old.txt\n");
    fs::write(&manifest_path, manifest).unwrap();
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    for kept in ["notes.txt", "etc/conf", "share/other/theirs.txt"] {
        assert!(prefix.join(kept).is_file(), "{kept} was removed: it is outside bin, libexec and share/runbook\n{}", err(&o));
    }
    assert!(!prefix.join("share/runbook/gone").exists(), "the file the old MANIFEST listed and its emptied directories are gone");
}

/// Two installs of one tree write the same bytes, and they are these: the list is fixed, sorted in the C locale.
#[test]
fn guard_two_installs_write_the_same_manifest_and_version_and_the_manifest_is_this_list() {
    let (root, sandbox, prefix) = subject("adv1-same-bytes");
    let second = sandbox.join("prefix-2");
    fs::create_dir_all(&second).unwrap();
    assert_eq!(code(&from_built(&root, &prefix)), 0);
    let o = Command::new("sh")
        .arg(root.join("scripts/install.sh"))
        .arg("--prefix")
        .arg(&second)
        .args(["--from-built", "debug"])
        .env_clear()
        .env("PATH", SYSTEM_PATH)
        .env("HOME", sandbox.join("home"))
        .env("LC_ALL", "en_US.UTF-8")
        .env("LANG", "en_US.UTF-8")
        .current_dir(&sandbox)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    let read = |p: &Path, n: &str| fs::read_to_string(p.join("share/runbook").join(n)).unwrap();
    assert_eq!(read(&prefix, "VERSION"), "runbook 9.9.9 (0123456789ab)\n");
    assert_eq!(read(&prefix, "VERSION"), read(&second, "VERSION"));
    assert_eq!(read(&prefix, "MANIFEST"), read(&second, "MANIFEST"));
    let want = [
        "bin/runbook",
        "libexec/sdc-answers",
        "libexec/sdc-bugfix",
        "libexec/sdc-check-routes",
        "libexec/sdc-conform",
        "libexec/sdc-conform-adapter",
        "libexec/sdc-decision-request",
        "libexec/sdc-findings",
        "libexec/sdc-node",
        "libexec/sdc-replay",
        "libexec/sdc-reply",
        "libexec/sdc-watch",
        "share/runbook/MANIFEST",
        "share/runbook/VERSION",
        "share/runbook/bindings/file.txt",
        "share/runbook/bindings/tools/file.txt",
        "share/runbook/bindings/tools/sdc-tool.sh",
        "share/runbook/cycles/file.txt",
        "share/runbook/driver/examples/sdc/file.txt",
        "share/runbook/instructions/file.txt",
        "share/runbook/protocol/file.txt",
        "share/runbook/scripts/file.txt",
        "share/runbook/scripts/install.sh",
        "share/runbook/tools/file.txt",
        "share/runbook/ui/package.json",
        "share/runbook/ui/server.mjs",
        "share/runbook/ui/web/file.txt",
        "share/runbook/views/file.txt",
    ];
    assert_eq!(read(&prefix, "MANIFEST").lines().collect::<Vec<_>>(), want);
}

/// The filter of the copy, on a checkout that holds what it names: the committed case
/// (`nothing_that_is_not_a_runbook_file_is_copied`) reads the real tree, which holds none of these.
#[test]
fn guard_what_a_run_or_a_build_left_in_the_asset_directories_is_not_copied() {
    let (root, _sandbox, prefix) = subject("adv1-filter");
    let left = [
        "bindings/.env.local",
        "bindings/credentials.json",
        "bindings/My-Credentials.txt",
        "bindings/settings.local.json",
        "bindings/runs/r1/outcome.json",
        "tools/target/debug/a",
        "ui/web/node_modules/x/index.js",
        "protocol/.git/config",
        "views/.cache/a",
    ];
    for f in left {
        fs::create_dir_all(root.join(f).parent().unwrap()).unwrap();
        fs::write(root.join(f), "left behind\n").unwrap();
    }
    fs::write(root.join("bindings/kept.yaml"), "kept: true\n").unwrap();
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    let share = prefix.join("share/runbook");
    assert_eq!(fs::read_to_string(share.join("bindings/kept.yaml")).unwrap(), "kept: true\n");
    let copied: Vec<&str> = left.into_iter().filter(|f| share.join(f).exists()).collect();
    assert_eq!(copied, Vec::<&str>::new());
    let manifest = fs::read_to_string(share.join("MANIFEST")).unwrap();
    assert!(manifest.lines().any(|l| l == "share/runbook/bindings/kept.yaml"));
    assert!(!manifest.lines().any(|l| left.iter().any(|f| l.ends_with(f))), "{manifest}");
}

/// The check of `bin`, `libexec`, `share` and `share/runbook` before anything is written.
#[test]
fn guard_a_prefix_whose_libexec_is_a_link_is_refused_before_anything_is_written() {
    let (root, sandbox, prefix) = subject("adv1-top-link");
    let outside = sandbox.join("outside");
    fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, prefix.join("libexec")).unwrap();
    let before = listing(&sandbox);
    let o = from_built(&root, &prefix);
    assert_ne!(code(&o), 0, "{}{}", out(&o), err(&o));
    assert!(err(&o).contains("symbolic link"), "{}", err(&o));
    assert_eq!(listing(&sandbox), before, "the refused install wrote something");
}

/// The third `~/.cargo` check: the cargo directory is a link, and the prefix is named by the place it points to.
#[test]
fn guard_a_prefix_inside_the_directory_the_cargo_link_points_to_is_refused() {
    let (root, sandbox, _) = subject("adv1-cargo-link");
    let real = sandbox.join("disk/cargo-home");
    fs::create_dir_all(real.join("bin")).unwrap();
    std::os::unix::fs::symlink(&real, sandbox.join("home/.cargo")).unwrap();
    let before = listing(&sandbox);
    let o = Command::new("sh")
        .arg(root.join("scripts/install.sh"))
        .arg("--prefix")
        .arg(real.join("bin"))
        .args(["--from-built", "debug"])
        .env_clear()
        .env("PATH", SYSTEM_PATH)
        .env("HOME", sandbox.join("home"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(code(&o), 2, "{}{}", out(&o), err(&o));
    assert!(err(&o).contains(".cargo"), "{}", err(&o));
    assert_eq!(listing(&sandbox), before, "the refusal wrote something");
}

/// The run the acceptance statement names, `runbook run sdc --dry --intake <file> ...` as `README.md` prints it,
/// from an empty directory: from the prefix it reads the installed protocols, bindings, tools and instructions and
/// starts the helpers of `libexec`; it ends as the same command of the checkout build with the driver programs on
/// `PATH`. The committed run case uses a runbook of its own (`--protocols`, `--bindings`), so nothing of
/// `share/runbook` is run by it. No model call (`--dry`). Both runs at once, with a `PATH` of the system
/// directories and links to `yq`, `aep`, `ess` and `task` where the test's own `PATH` has them (with the whole
/// `PATH` of the test the two runs, one after the other, ran past 60 s on 2026-11-23). 40 s at the load of that
/// day: too near the 60 s bound for the fast lane, so it is ignored there (`--run-ignored only` runs it).
#[test]
#[ignore = "40 s at load (2026-11-23): a slow-lane case, not one of the fast lane"]
fn guard_the_dry_run_of_sdc_from_the_prefix_ends_as_the_same_run_of_the_checkout_build() {
    let prefix = shared_prefix();
    let tools = scratch("adv1-dry-tools");
    for name in ["yq", "aep", "ess", "task"] {
        let found = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).map(|d| d.join(name)).find(|p| p.is_file());
        if let Some(p) = found {
            std::os::unix::fs::symlink(p, tools.join(name)).unwrap();
        }
    }
    let ambient = format!("{}:{SYSTEM_PATH}", tools.display());
    let one = |tag: &str, program: &Path, path: String| -> (Value, PathBuf) {
        let w = scratch(tag);
        fs::write(w.join("idea.yaml"), "text: a note on tea, kettle and teacup\n").unwrap();
        fs::write(w.join("settings.json"), "{\"loom\": {\"in_process\": true, \"confinement\": \"local\"}}\n").unwrap();
        let o = Command::new(program)
            .args(["run", "sdc", "--dry", "--intake", "idea.yaml", "--run", "trial-dry", "--runs-dir", "runs", "--settings", "settings.json"])
            .args(["--confinement", "local", "--spend-ledger", "ledger.json", "--cap-micro-usd", "1000000", "--max-llm-calls", "200", "--max-wall-seconds", "50"])
            .env_clear()
            .env("PATH", path)
            .env("HOME", &w)
            .current_dir(&w)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let text = fs::read_to_string(w.join("runs/trial-dry/outcome.json")).unwrap_or_else(|e| panic!("{tag}: no outcome.json: {e}\n{}{}", out(&o), err(&o)));
        (serde_json::from_str(&text).unwrap(), w)
    };
    let helpers = format!("{}:{}", repo().join("driver/target/debug").display(), repo().join("watch/target/debug").display());
    let installed_program = prefix.join("bin/runbook");
    let ((installed, w_installed), (checkout, w_checkout)) = std::thread::scope(|s| {
        let a = s.spawn(|| one("adv1-dry-installed", &installed_program, format!("{}:{ambient}", prefix.join("bin").display())));
        let b = s.spawn(|| one("adv1-dry-checkout", Path::new(BIN), format!("{helpers}:{ambient}")));
        (a.join().unwrap(), b.join().unwrap())
    });
    assert_eq!(comparable(&installed, &w_installed), comparable(&checkout, &w_checkout));
    let commands = fs::read_to_string(w_installed.join("runs/trial-dry/dry-commands.txt")).unwrap_or_default();
    assert!(commands.lines().count() >= 1, "the installed run reached no model node: {installed}");
    assert!(installed["steps"].as_u64().unwrap_or(0) >= 2, "{installed}");
}
