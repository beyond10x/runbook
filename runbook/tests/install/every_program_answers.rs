//! Every program the script installs answers `--version` with its name and the version of the checkout, with no input,
//! no environment and no file of a run: the check of a release archive starts each one so in a plain `alpine`
//! container (`.github/workflows/release.yml`).

use super::*;

#[test]
fn every_program_of_the_prefix_answers_version_with_its_name_and_the_version() {
    let prefix = shared_prefix();
    let version = env!("CARGO_PKG_VERSION");
    let cwd = scratch("answers-version");
    let mut helpers: Vec<PathBuf> = fs::read_dir(prefix.join("libexec/runbook")).unwrap().flatten().map(|e| e.path()).collect();
    helpers.sort();
    assert_eq!(helpers.len(), HELPERS.len(), "the helpers of the prefix: {helpers:?}");
    let mut bad = vec![];
    for p in std::iter::once(prefix.join("bin/runbook")).chain(helpers) {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let o = Command::new(&p).arg("--version").env_clear().env("PATH", SYSTEM_PATH).env("HOME", &cwd).current_dir(&cwd).stdin(Stdio::null()).output().unwrap();
        let said = out(&o);
        let want = format!("{name} {version}");
        let first = said.lines().next().unwrap_or_default();
        if code(&o) != 0 || !(first == want || first.starts_with(&format!("{want} ("))) {
            bad.push(format!("{name} --version: exit {}, stdout {said:?}, stderr {:?}; want {want:?}", code(&o), err(&o)));
        }
    }
    let left: Vec<String> = fs::read_dir(&cwd).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert!(left.is_empty(), "--version wrote into the working directory: {left:?}");
    assert!(bad.is_empty(), "{bad:#?}");
}
