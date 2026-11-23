//! What the caller's environment does not change: the files an install writes are the same with `CDPATH`, a
//! start file of the shell, `GLOBIGNORE`, `POSIXLY_CORRECT` and the locale set as with a clean environment.
//! Fake checkouts; every prefix lies under the target directory.

use super::*;

/// The script is started by a relative path from the checkout's root, as a person does (`sh scripts/install.sh`), by
/// `sh` and by `bash` (which reads `BASH_ENV` only when `POSIXLY_CORRECT` is not set). `CDPATH` names a directory that has a
/// `scripts` of its own: `cd scripts/..` of an unreset shell goes there and prints the directory it went to. The start
/// file sets `IFS` and `GLOBIGNORE`. The same files come out as without the variables.
#[test]
fn cdpath_a_start_file_globignore_posixly_correct_and_the_locale_change_nothing_of_what_is_installed() {
    let root = fake_checkout("fix2-env", None);
    let (sandbox, plain) = sandbox_of(&root);
    assert_eq!(code(&from_built(&root, &plain)), 0);
    let manifest = fs::read_to_string(plain.join("share/runbook/MANIFEST")).unwrap();
    let version = fs::read_to_string(plain.join("share/runbook/VERSION")).unwrap();

    let decoy = sandbox.join("decoy");
    fs::create_dir_all(decoy.join("scripts")).unwrap();
    let start_file = sandbox.join("start.sh");
    fs::write(&start_file, "IFS=:\nGLOBIGNORE='*'\necho start file ran >&2\n").unwrap();
    for (shell, posix) in [("sh", true), ("bash", false)] {
        let home = sandbox.join(format!("{shell}/home"));
        let prefix = sandbox.join(format!("{shell}/prefix"));
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&prefix).unwrap();
        let o = Command::new(shell)
            .arg("scripts/install.sh")
            .arg("--prefix")
            .arg(&prefix)
            .args(["--from-built", "debug"])
            .current_dir(&root)
            .env_clear()
            .env("PATH", SYSTEM_PATH)
            .env("HOME", &home)
            .env("CDPATH", &decoy)
            .env("ENV", &start_file)
            .env("BASH_ENV", &start_file)
            .env("GLOBIGNORE", "*")
            .envs(posix.then_some(("POSIXLY_CORRECT", "1")))
            .env("LC_ALL", "de_DE.UTF-8")
            .env("LANG", "de_DE.UTF-8")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(code(&o), 0, "{shell}: {}", err(&o));
        assert_eq!(fs::read_to_string(prefix.join("share/runbook/MANIFEST")).unwrap(), manifest, "{shell}");
        assert_eq!(fs::read_to_string(prefix.join("share/runbook/VERSION")).unwrap(), version, "{shell}");
        assert_eq!(files_below(&prefix), files_below(&plain), "{shell}");
    }
}

/// The script sets `LC_ALL=C` for itself. Without that line a name with a byte that is not UTF-8 (a Latin-1 `ä`) is cut
/// in two by `awk` under a UTF-8 locale of the caller and the install stops at `cp`. The committed case sets a locale
/// and has only ASCII names, which every locale reads alike.
#[test]
fn a_name_that_is_not_utf8_installs_the_same_under_a_utf8_locale_of_the_caller() {
    use std::os::unix::ffi::OsStrExt;
    let locales = Command::new("locale").arg("-a").output().map(|o| out(&o)).unwrap_or_default();
    let Some(locale) = ["de_DE.utf8", "en_US.utf8"].into_iter().find(|l| locales.lines().any(|have| have == *l)) else {
        eprintln!("no UTF-8 locale with rules of its own on this host ({locales:?}): nothing to hold");
        return;
    };
    let root = fake_checkout("adv3-locale", None);
    let (sandbox, plain) = sandbox_of(&root);
    let name = std::ffi::OsStr::from_bytes(b"b\xe4r.txt");
    fs::write(root.join("protocol").join(name), "latin-1\n").unwrap();
    fs::write(root.join("protocol/Zebra.txt"), "upper\n").unwrap();
    fs::write(root.join("protocol/apple.txt"), "lower\n").unwrap();
    git_in(&root, &["add", "-A", "protocol"]);
    git_in(&root, &["commit", "-q", "-m", "names a locale reads differently"]);
    stubs(&root, None, &stub_version(&head12(&root)));
    assert_eq!(code(&from_built(&root, &plain)), 0);
    let under = sandbox.join("under-locale");
    fs::create_dir_all(&under).unwrap();
    let o = Command::new("sh")
        .arg(root.join("scripts/install.sh"))
        .arg("--prefix")
        .arg(&under)
        .args(["--from-built", "debug"])
        .env_clear()
        .env("PATH", SYSTEM_PATH)
        .env("HOME", sandbox.join("home"))
        .env("LC_ALL", locale)
        .env("LANG", locale)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(code(&o), 0, "{locale}: {}", err(&o));
    assert_eq!(fs::read(under.join("share/runbook/protocol").join(name)).unwrap(), b"latin-1\n");
    assert_eq!(
        fs::read(under.join("share/runbook/MANIFEST")).unwrap(),
        fs::read(plain.join("share/runbook/MANIFEST")).unwrap(),
        "{locale}: the same MANIFEST as without a locale"
    );
}
