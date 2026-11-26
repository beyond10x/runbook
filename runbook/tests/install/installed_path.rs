//! The `PATH` an installed runbook gives the programs it starts.

use super::*;

/// `path_with_libexec` changes nothing when the helpers' directory is on `PATH` already, wherever it stands: with a
/// directory in front of it that holds a program of a helper's name, a bound program starts that one.
#[test]
fn a_program_of_a_helpers_name_earlier_on_path_is_started_when_the_helpers_directory_is_on_path_already() {
    let prefix = shared_prefix();
    let w = world("fix2-path-order");
    let other = w.join("other-bin");
    executable(&other.join("sdc-reply"), "#!/bin/sh\necho not the helper of the prefix\n");
    let path = format!("{}:{}:{}:{SYSTEM_PATH}", other.display(), prefix.join("libexec/runbook").display(), prefix.join("bin").display());
    let with = |cwd: &Path, args: &[&str]| Command::new(prefix.join("bin/runbook")).args(args).env_clear().envs(crate::kept()).env("PATH", &path).env("HOME", cwd).env("RUNBOOK_NO_CONFIG", "1").current_dir(cwd).stdin(Stdio::null()).output().unwrap();
    let (_, found) = run_example(&with, &w);
    assert!(found.ends_with("libexec/runbook/sdc-reply"), "a bound program of an installed run finds sdc-reply at {found:?}");
}
