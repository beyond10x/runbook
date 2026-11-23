//! The slow lane of the install tests (`task test:install`, not part of `task check`): the prefix that the task
//! made with the real `--release` build (`scripts/install.sh --prefix <dir>`, no `--from-built`), named by
//! `RUNBOOK_INSTALLED_PREFIX`, and the same three commands as the fast lane: `runbook list`, `runbook doctor`
//! and one run with sh programs only.

use super::*;

fn prefix() -> PathBuf {
    let p = std::env::var_os("RUNBOOK_INSTALLED_PREFIX").unwrap_or_else(|| panic!("RUNBOOK_INSTALLED_PREFIX names the prefix the task installed into: run `task test:install`"));
    PathBuf::from(p)
}

#[test]
#[ignore = "slow lane: task test:install builds the release binaries"]
fn the_release_build_installed_lists_the_protocols_from_an_empty_directory() {
    super::list_from(&prefix());
}

#[test]
#[ignore = "slow lane: task test:install builds the release binaries"]
fn the_release_build_installed_has_its_assets_and_helpers_in_the_prefix() {
    super::doctor_from(&prefix());
}

#[test]
#[ignore = "slow lane: task test:install builds the release binaries"]
fn a_run_from_the_release_build_installed_ends_as_from_the_checkout_build() {
    super::run_from(&prefix());
}

/// The run the acceptance statement names, `runbook run sdc --dry --intake <file> ...` as `README.md` prints it, from an
/// empty directory with `<prefix>/bin` and the system directories on `PATH`: it reads the installed protocols, bindings,
/// tools and instructions and starts the helpers of `libexec/runbook`, and it ends as the same command of the checkout
/// build with the driver programs on `PATH`. The prefix is the one of the fast lane (`--from-built debug`), so this lane
/// builds no release binary for it; the case needs the three debug builds (`task test:install` makes them). No model
/// call (`--dry`). Both runs at once, with a `PATH` of the system directories and links to `yq`, `aep`, `ess` and `task`
/// where the test's own `PATH` has them. 40 s at the load of 2026-11-23: too near the 60 s bound for the fast lane.
#[test]
#[ignore = "slow lane: about 40 s at load, run by task test:install"]
fn the_dry_run_of_sdc_from_the_prefix_ends_as_the_same_run_of_the_checkout_build() {
    let prefix = shared_prefix();
    let tools = scratch("dry-tools");
    for name in ["yq", "aep", "ess", "task"] {
        let found = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|d| d.join(name))
            .find(|p| p.is_file());
        if let Some(p) = found {
            std::os::unix::fs::symlink(p, tools.join(name)).unwrap();
        }
    }
    let ambient = format!("{}:{SYSTEM_PATH}", tools.display());
    let one = |tag: &str, program: &Path, path: String| -> (Value, PathBuf) {
        let w = scratch(tag);
        fs::write(
            w.join("idea.yaml"),
            "text: a note on tea, kettle and teacup\n",
        )
        .unwrap();
        fs::write(
            w.join("settings.json"),
            "{\"loom\": {\"in_process\": true, \"confinement\": \"local\"}}\n",
        )
        .unwrap();
        let o = Command::new(program)
            .args([
                "run",
                "sdc",
                "--dry",
                "--intake",
                "idea.yaml",
                "--run",
                "trial-dry",
                "--runs-dir",
                "runs",
                "--settings",
                "settings.json",
            ])
            .args([
                "--confinement",
                "local",
                "--spend-ledger",
                "ledger.json",
                "--cap-micro-usd",
                "1000000",
                "--max-llm-calls",
                "200",
                "--max-wall-seconds",
                "50",
            ])
            .env_clear()
            .env("PATH", path)
            .env("HOME", &w)
            .current_dir(&w)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let text = fs::read_to_string(w.join("runs/trial-dry/outcome.json"))
            .unwrap_or_else(|e| panic!("{tag}: no outcome.json: {e}\n{}{}", out(&o), err(&o)));
        (serde_json::from_str(&text).unwrap(), w)
    };
    let helpers = format!(
        "{}:{}",
        repo().join("driver/target/debug").display(),
        repo().join("watch/target/debug").display()
    );
    let installed_program = prefix.join("bin/runbook");
    let ((installed, w_installed), (checkout, w_checkout)) = std::thread::scope(|s| {
        let a = s.spawn(|| {
            one(
                "dry-installed",
                &installed_program,
                format!("{}:{ambient}", prefix.join("bin").display()),
            )
        });
        let b = s.spawn(|| {
            one(
                "dry-checkout",
                Path::new(BIN),
                format!("{helpers}:{ambient}"),
            )
        });
        (a.join().unwrap(), b.join().unwrap())
    });
    assert_eq!(
        comparable(&installed, &w_installed),
        comparable(&checkout, &w_checkout)
    );
    let commands =
        fs::read_to_string(w_installed.join("runs/trial-dry/dry-commands.txt")).unwrap_or_default();
    assert!(
        commands.lines().count() >= 1,
        "the installed run reached no model node: {installed}"
    );
    assert!(installed["steps"].as_u64().unwrap_or(0) >= 2, "{installed}");
}
