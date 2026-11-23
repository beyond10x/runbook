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
