//! The `bugfix` port of `runbook run` (`--bugfix <dir>`, `--bugfix-ticket <file>`): a bug-fix case.
//!
//! The root protocol is `sdc-bugfix` and the root case `BUG-1`. Before the tree opens, the
//! `sdc-bugfix` helper runs `seed` in the workspace (the root case's work directory) and writes
//! `bug.yaml`, the code trees `mainline/` and `impl/`, `checks.yaml` and the story there: from a
//! seed directory (`seed --from <dir> --text <report> --story story:bug-<run>`, the report being the
//! intake) or from a ticket (`seed --ticket <file>`; the ticket is the report, the intake is not
//! read, and the code tree is the git repository the ticket names, as `git ls-files` lists it).
//!
//! The ticket's text reaches the work directory only. The runner never reads it: it passes the
//! file's path, prints nothing of the seed on success and, when the seed fails, the last three
//! lines the seed itself printed (they name the ticket by its id and nothing else of it).
//!
//! The bug-fix runbook works on Rust repositories only: the checks a seed writes are Rust's
//! (`docs/bugfix.md`); this port neither widens nor narrows that.

use super::ports::{Bugfix, Ports, Refusal, USAGE};
use super::Args;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The runbook the bug-fix options open.
pub const RUNBOOK: &str = "sdc-bugfix";
/// The root case of a bug fix.
pub const CASE: &str = "BUG-1";
/// The helper that seeds the case.
const HELPER: &str = "sdc-bugfix";

fn given(b: &Bugfix) -> bool {
    b.seed.is_some() || b.ticket.is_some()
}

/// The checks of the options that need no run: one of the two, an existing directory or file. The
/// name is the skeleton's (`ports.rs` calls it for every port); the port is built.
pub fn bugfix_not_built(ports: &Ports) -> Result<(), Refusal> {
    let b = &ports.bugfix;
    if b.seed.is_some() && b.ticket.is_some() {
        return Err((USAGE, "--bugfix and --bugfix-ticket are two ways to seed one case: give one".to_owned()));
    }
    if let Some(dir) = &b.seed {
        if !dir.is_dir() {
            return Err((USAGE, format!("--bugfix needs a seed directory, not '{}'", dir.display())));
        }
    }
    if let Some(file) = &b.ticket {
        if !file.is_file() {
            return Err((USAGE, format!("--bugfix-ticket needs a ticket file (sdc.ticket/1), not '{}'", file.display())));
        }
    }
    Ok(())
}

/// The runbook whose root protocol the run registers: `sdc-bugfix` when a bug-fix option is given
/// (the runbook argument may then be `sdc` or `sdc-bugfix`), else the runbook argument.
pub fn runbook_of<'a>(ports: &Ports, runbook: &'a str) -> Result<&'a str, Refusal> {
    if !given(&ports.bugfix) {
        return Ok(runbook);
    }
    if runbook == "sdc" || runbook == RUNBOOK {
        return Ok(RUNBOOK);
    }
    Err((USAGE, format!("--bugfix and --bugfix-ticket open the bug-fix runbook `{RUNBOOK}`, not `{runbook}`")))
}

/// The root case: `BUG-1` for a bug-fix option when `--case` was left at its default.
pub fn case_of(ports: &Ports, case: String) -> String {
    if given(&ports.bugfix) && case == super::here::DEFAULT_CASE {
        CASE.to_owned()
    } else {
        case
    }
}

/// What is checked before anything is written: the root protocol is the bug-fix one and the helper
/// is found.
pub fn check(args: &Args) -> Result<(), String> {
    if !given(&args.ports.bugfix) {
        return Ok(());
    }
    let (name, _) = args.root_protocol.split_once('@').unwrap_or((args.root_protocol.as_str(), ""));
    if name != RUNBOOK {
        return Err(format!("--bugfix and --bugfix-ticket open the root protocol {RUNBOOK}, not {}", args.root_protocol));
    }
    helper().map(|_| ())
}

/// The helper binary, found as `runbook doctor` finds the others: beside this executable, in a
/// checkout's `driver/target`, then on `PATH`.
fn helper() -> Result<PathBuf, String> {
    let lookup = crate::assets::Lookup::from_process();
    let assets = lookup.locate().ok();
    lookup.helper(HELPER, assets.as_ref()).ok_or_else(|| format!("{HELPER} is not found: build the helper binaries (`cargo build --locked` in driver/) or put {HELPER} on PATH"))
}

/// The text of the report an intake holds: its `text` member when it is a YAML document with one
/// (the idea document), else the intake itself.
fn report_of(intake: &str) -> String {
    if let Ok(serde_yaml::Value::Mapping(m)) = serde_yaml::from_str::<serde_yaml::Value>(intake) {
        if let Some(t) = m.get("text").and_then(serde_yaml::Value::as_str) {
            return t.trim_end().to_owned();
        }
    }
    intake.trim_end().to_owned()
}

/// The arguments of `sdc-bugfix seed` for the options (`workspace`-independent: paths absolute).
fn seed_args(args: &Args) -> Result<Vec<String>, String> {
    let abs = |p: &Path| std::path::absolute(p).map(|p| p.display().to_string()).map_err(|e| format!("{}: {e}", p.display()));
    let b = &args.ports.bugfix;
    let mut v = vec!["seed".to_owned()];
    match (&b.ticket, &b.seed) {
        (Some(ticket), _) => v.extend(["--ticket".to_owned(), abs(ticket)?]),
        (None, Some(dir)) => v.extend(["--from".to_owned(), abs(dir)?, "--text".to_owned(), report_of(&args.intake), "--story".to_owned(), format!("story:bug-{}", args.run)]),
        (None, None) => unreachable!("seed is called for a bug-fix option"),
    }
    Ok(v)
}

/// Runs the seed in `workspace` before the tree opens, with the environment of the bound programs.
/// Nothing is printed on success; a failure is an error that carries the last three lines the seed
/// printed.
pub fn seed(args: &Args, workspace: &Path, env: &[(String, String)]) -> Result<(), String> {
    if !given(&args.ports.bugfix) {
        return Ok(());
    }
    let helper = helper()?;
    let out = Command::new(&helper)
        .args(seed_args(args)?)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .current_dir(workspace)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot start {HELPER}: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let tail = lines[lines.len().saturating_sub(3)..].join(" | ");
    let code = out.status.code().map_or_else(|| "a signal".to_owned(), |c| format!("exit {c}"));
    Err(format!("the seed of the bug-fix case failed ({HELPER} seed: {code}): {tail}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_report_is_the_text_member_of_an_idea_document_or_the_intake() {
        assert_eq!(report_of("text: the pager drops the last page\n"), "the pager drops the last page");
        assert_eq!(report_of("the pager drops the last page\n"), "the pager drops the last page");
        assert_eq!(report_of("- a\n- b\n"), "- a\n- b");
    }

    #[test]
    fn the_case_is_bug_1_only_for_a_bug_fix_option_and_the_default_case() {
        let mut ports = Ports::default();
        assert_eq!(case_of(&ports, "IDEA-1".into()), "IDEA-1");
        ports.bugfix.ticket = Some("t.yaml".into());
        assert_eq!(case_of(&ports, "IDEA-1".into()), "BUG-1");
        assert_eq!(case_of(&ports, "BUG-7".into()), "BUG-7");
    }

    #[test]
    fn the_runbook_follows_the_option() {
        let mut ports = Ports::default();
        assert_eq!(runbook_of(&ports, "sdc"), Ok("sdc"));
        ports.bugfix.seed = Some("d".into());
        assert_eq!(runbook_of(&ports, "sdc"), Ok("sdc-bugfix"));
        assert_eq!(runbook_of(&ports, "sdc-bugfix"), Ok("sdc-bugfix"));
        assert!(runbook_of(&ports, "sdc-story").is_err());
    }
}
