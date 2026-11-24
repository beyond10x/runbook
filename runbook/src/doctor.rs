//! `runbook doctor`: every program and helper binary a runbook needs, one line each.
//!
//! The table below is the list of prerequisites, each with the place in the code that makes the
//! program necessary. A login or an API key is only checked for existence; its value is never read.

use crate::assets::{Assets, Lookup, Part, LAYOUT};
use serde_json::{json, Value};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scope {
    Sdc,
    Bugfix,
    Ui,
    /// A runbook of a configured directory: what every runbook needs is required, what one of the
    /// two built-in runbooks needs more is optional.
    Other,
}

impl Scope {
    /// The scope of a startable runbook by name: the two built-in runbooks have their own table
    /// entries (the programs their bindings run), any other runbook the common ones.
    pub fn runbook(id: &str) -> Scope {
        match id {
            "sdc" => Scope::Sdc,
            "sdc-bugfix" => Scope::Bugfix,
            _ => Scope::Other,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    Required,
    Optional,
}

enum Kind {
    Assets,
    /// A program on `PATH`; `bin` is the file to look for, `args` print its version.
    Program(&'static str, &'static [&'static str]),
    /// The Canon this binary is linked with (`runbook canon`): nothing is looked up, it is there on every host.
    Canon,
    /// The Go yq, major version 4: its `--version` prints `v4.x` (the Python yq prints `3.x`).
    Yq,
    /// One of the helper binaries (`sdc-*`).
    Helper,
    /// A Claude login file or `ANTHROPIC_API_KEY`.
    Login,
}

struct Spec {
    name: &'static str,
    kind: Kind,
    needs: &'static [(Scope, Level)],
    by: &'static str,
    hint: &'static str,
}

use Level::{Optional as Opt, Required as Req};
use Scope::{Bugfix as B, Sdc as S, Ui as U};

const RUN: &[(Scope, Level)] = &[(S, Req), (B, Req)];
const RUN_UI: &[(Scope, Level)] = &[(S, Req), (B, Req), (U, Req)];
const RUN_OPT: &[(Scope, Level)] = &[(S, Opt), (B, Opt)];
const ANY_OPT: &[(Scope, Level)] = &[(S, Opt), (B, Opt), (U, Opt)];

const BUILD_HELPERS: &str = "build them: cd driver && cargo build --release, or install a runbook release";
const SHELL_TOOLS: &str = "install it with your package manager";
const B10X: &str = "install it from the beyond10x agent plugins (github.com/beyond10x/agentplugins)";

const fn program(bin: &'static str) -> Kind {
    Kind::Program(bin, &["--version"])
}

#[rustfmt::skip]
const SPECS: &[Spec] = &[
    Spec { name: "assets", kind: Kind::Assets, needs: RUN_UI, by: "every runbook: the directories of the text layout, each checked on its own line below", hint: "set RUNBOOK_HOME, install them at <prefix>/share/runbook, or run inside a checkout" },
    Spec { name: "canon", kind: Kind::Canon, needs: RUN, by: "every runbook: `runbook canon` compiles and evaluates the protocols with the Canon this binary is linked with", hint: "" },
    Spec { name: "bash", kind: program("bash"), needs: RUN, by: "every runbook: the bindings' tools are bash scripts (bindings/tools)", hint: SHELL_TOOLS },
    Spec { name: "jq", kind: program("jq"), needs: RUN_UI, by: "every runbook and the ui (bindings/tools/sdc-tool.sh)", hint: SHELL_TOOLS },
    Spec { name: "yq", kind: Kind::Yq, needs: RUN_UI, by: "every runbook and the ui, the Go yq v4 (bindings/tools/sdc-tool.sh and ui/decisions.mjs)", hint: "install the Go yq v4 (its --version prints v4.x), not the Python yq" },
    Spec { name: "git", kind: program("git"), needs: RUN, by: "every runbook: run records name the commit; sdc-replay and sdc-experiment use git", hint: SHELL_TOOLS },
    Spec { name: "claude", kind: program("claude"), needs: RUN, by: "every runbook: the model steps run the claude CLI (bindings/tools/llm-adapter-claude-cli.sh)", hint: "npm install -g @anthropic-ai/claude-code" },
    Spec { name: "claude-login", kind: Kind::Login, needs: RUN, by: "every runbook: a login file or ANTHROPIC_API_KEY, as bindings/tools/llm-adapter-claude-cli.sh reads them", hint: "log in once with `claude`, or export ANTHROPIC_API_KEY" },
    Spec { name: "aep", kind: program("aep"), needs: RUN, by: "every runbook: the story step and the plan nodes (driver/src/bin/sdc-bugfix.rs, bindings/tools/sdc-tool.sh)", hint: B10X },
    Spec { name: "ess", kind: program("ess"), needs: &[(S, Req)], by: "sdc: the spec and story protocols (bindings/tools/sdc-tool.sh)", hint: B10X },
    Spec { name: "task", kind: program("task"), needs: &[(S, Req)], by: "sdc: the story gate runs `task check` in the implementation (bindings/tools/sdc-tool.sh)", hint: "install go-task (taskfile.dev/installation)" },
    Spec { name: "slot", kind: program("slot"), needs: &[(S, Req), (B, Opt)], by: "sdc: the story gate (bindings/tools/sdc-tool.sh); sdc-bugfix: builds go through it when present (driver/src/bin/sdc-bugfix.rs)", hint: B10X },
    Spec { name: "cargo", kind: program("cargo"), needs: RUN, by: "sdc: the story's interim gate (bindings/tools/sdc-tool.sh); sdc-bugfix: the code tree's gate (driver/src/bin/sdc-bugfix.rs)", hint: "install Rust with rustup (rustup.rs)" },
    Spec { name: "clippy", kind: program("cargo-clippy"), needs: &[(B, Req)], by: "sdc-bugfix: the gate's lint step (driver/src/bin/sdc-bugfix.rs)", hint: "rustup component add clippy" },
    Spec { name: "rustfmt", kind: program("rustfmt"), needs: &[(B, Req)], by: "sdc-bugfix: the gate's format step (driver/src/bin/sdc-bugfix.rs)", hint: "rustup component add rustfmt" },
    Spec { name: "node", kind: program("node"), needs: &[(U, Req)], by: "the ui only (ui/server.mjs)", hint: "install Node.js" },
    Spec { name: "sdc-findings", kind: Kind::Helper, needs: RUN, by: "every runbook: late findings (bindings/tools/sdc-tool.sh, sdc-node)", hint: BUILD_HELPERS },
    Spec { name: "sdc-bugfix", kind: Kind::Helper, needs: RUN, by: "every runbook: the bugfix steps (bindings/tools/sdc-tool.sh)", hint: BUILD_HELPERS },
    Spec { name: "sdc-decision-request", kind: Kind::Helper, needs: RUN, by: "every runbook: bindings/tools/sdc-tool.sh, the requester decider of runbook run", hint: BUILD_HELPERS },
    Spec { name: "sdc-reply", kind: Kind::Helper, needs: RUN, by: "every runbook: sdc-node, bindings/tools/sdc-tool.sh", hint: BUILD_HELPERS },
    Spec { name: "sdc-replay", kind: Kind::Helper, needs: RUN, by: "every runbook: runbook run --recorded", hint: BUILD_HELPERS },
    Spec { name: "sdc-check-routes", kind: Kind::Helper, needs: RUN, by: "every runbook: scripts/check-bindings.sh", hint: BUILD_HELPERS },
    Spec { name: "sdc-conform", kind: Kind::Helper, needs: RUN, by: "every runbook: bindings/tools/sdc-tool.sh, runbook run --conformance", hint: BUILD_HELPERS },
    Spec { name: "sdc-experiment", kind: Kind::Helper, needs: RUN_OPT, by: "optional: experiment records and runs under systemd", hint: BUILD_HELPERS },
    Spec { name: "sdc-run-metrics", kind: Kind::Helper, needs: RUN_OPT, by: "optional: run metrics", hint: BUILD_HELPERS },
    Spec { name: "sdc-run-timeline", kind: Kind::Helper, needs: RUN_OPT, by: "optional: run timeline", hint: BUILD_HELPERS },
    Spec { name: "sdc-run-compare", kind: Kind::Helper, needs: RUN_OPT, by: "optional: compare two runs", hint: BUILD_HELPERS },
    Spec { name: "sdc-finding-kpis", kind: Kind::Helper, needs: RUN_OPT, by: "optional: finding KPIs", hint: BUILD_HELPERS },
    Spec { name: "sdc-conform-adapter", kind: Kind::Helper, needs: RUN_OPT, by: "optional: conformance adapter", hint: BUILD_HELPERS },
    Spec { name: "sdc-protocol-site", kind: Kind::Helper, needs: RUN_OPT, by: "optional: the documentation site's data", hint: BUILD_HELPERS },
    Spec { name: "sdc-free-agent", kind: Kind::Helper, needs: RUN_OPT, by: "optional: free-agent runs", hint: BUILD_HELPERS },
    Spec { name: "sdc-check-unreferenced", kind: Kind::Helper, needs: ANY_OPT, by: "optional: the check for files nothing refers to (task check:unreferenced)", hint: BUILD_HELPERS },
    Spec { name: "sdc-recorded-pool", kind: Kind::Helper, needs: ANY_OPT, by: "optional: the recorded build and test results of the cycles suite (task test:cycles-loom, docs/cycles-suite.md)", hint: BUILD_HELPERS },
    Spec { name: "sdc-answers", kind: Kind::Helper, needs: RUN_OPT, by: "optional: runbook run with a decider at A5 (bindings/tools/answers.yaml)", hint: BUILD_HELPERS },
    Spec { name: "sdc-node", kind: Kind::Helper, needs: RUN, by: "every runbook: the model-step nodes of the bindings, found on PATH", hint: BUILD_HELPERS },
    Spec { name: "sdc-watch", kind: Kind::Helper, needs: ANY_OPT, by: "optional: the file-change record of a run; SDC_WATCH=0 turns it off", hint: BUILD_HELPERS },
    Spec { name: "systemd-run", kind: program("systemd-run"), needs: ANY_OPT, by: "optional: sdc-experiment starts runs as transient units", hint: "needs systemd" },
    Spec { name: "curl", kind: program("curl"), needs: ANY_OPT, by: "optional: sdc-conform downloads artifacts", hint: SHELL_TOOLS },
    Spec { name: "tar", kind: program("tar"), needs: ANY_OPT, by: "optional: sdc-conform unpacks artifacts", hint: SHELL_TOOLS },
];

/// The names of the helper binaries the table checks.
#[cfg(test)]
fn helper_names() -> Vec<&'static str> {
    SPECS.iter().filter(|s| matches!(s.kind, Kind::Helper)).map(|s| s.name).collect()
}

#[derive(Debug, Clone)]
pub struct Check {
    pub name: &'static str,
    pub found: bool,
    pub required: bool,
    /// The version, the path of a helper, or `missing`.
    pub detail: String,
    pub path: Option<PathBuf>,
    pub by: &'static str,
    pub hint: &'static str,
}

pub struct Report {
    pub scope: Option<String>,
    pub assets: Option<Assets>,
    pub checks: Vec<Check>,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.checks.iter().all(|c| c.found || !c.required)
    }
}

/// `directory`: the runbook directory the check is about when it is not the built-in set; its parts
/// are what the layout lines ask for, and the helpers are still looked up as for the built-in set.
pub fn run(l: &Lookup, var: &dyn Fn(&str) -> Option<OsString>, only: Option<(&str, Scope)>, directory: Option<Assets>) -> Report {
    let other = matches!(only, Some((_, Scope::Other)));
    let scopes: Vec<Scope> = match only {
        Some((_, Scope::Other)) => vec![S, B],
        Some((_, s)) => vec![s],
        None => vec![S, B, U],
    };
    let located = l.locate();
    let found_assets = located.as_ref().ok().cloned();
    let assets = directory.or_else(|| found_assets.clone());
    let layout: Vec<&Part> = match assets.as_ref().map(|a| a.source) {
        Some(crate::assets::Source::Configured) => LAYOUT.iter().filter(|p| crate::assets::DIRECTORY_PARTS.contains(&p.name)).collect(),
        _ => LAYOUT.iter().collect(),
    };
    // A binary of an installed prefix reports that prefix's install line whether or not its assets can be read:
    // an install that stopped has no protocol/ and must not look like a checkout's doctor.
    let own_prefix = match l.home {
        Some(_) => None,
        None => l.own_share().map(|root| Assets {
            root,
            source: crate::assets::Source::Installed,
        }),
    };
    let mut checks = Vec::new();
    for spec in SPECS {
        let levels: Vec<Level> = spec.needs.iter().filter(|(s, _)| scopes.contains(s)).map(|(_, v)| *v).collect();
        if levels.is_empty() {
            continue;
        }
        let required = if other { levels.len() == scopes.len() && levels.iter().all(|v| *v == Req) } else { levels.contains(&Req) };
        let (found, detail, path) = match (&spec.kind, &assets, &located) {
            (Kind::Assets, Some(a), _) => (true, format!("{} ({})", a.root.display(), a.source.as_str()), Some(a.root.clone())),
            (Kind::Assets, None, Err(e)) => missing(e),
            _ => probe(spec, l, var, found_assets.as_ref()),
        };
        checks.push(Check { name: spec.name, found, required, detail, path, by: spec.by, hint: spec.hint });
        if matches!(spec.kind, Kind::Assets) {
            checks.extend(own_prefix.as_ref().and_then(install_check));
            checks.extend(layout.iter().map(|part| layout_check(part, assets.as_ref())));
        }
    }
    Report { scope: only.map(|(id, _)| id.to_string()), assets, checks }
}

const INSTALL_HINT: &str =
    "run scripts/install.sh --prefix <prefix> again over this prefix: it finishes the install";

/// How many names of missing files the `install` line spells out.
const NAMED_MISSING: usize = 3;

/// What an installed prefix lacks, or `None` when it is whole: the `share/runbook/INSTALLING` marker of an
/// install that did not finish is there, `VERSION` or `MANIFEST` is missing, or a file `MANIFEST` names is
/// missing. Existence only: nothing is hashed.
fn incomplete(share: &Path) -> Option<String> {
    if share.join("INSTALLING").symlink_metadata().is_ok() {
        return Some("share/runbook/INSTALLING is there: an install did not finish".to_string());
    }
    let absent: Vec<&str> = ["VERSION", "MANIFEST"]
        .into_iter()
        .filter(|n| !share.join(n).is_file())
        .collect();
    if !absent.is_empty() {
        return Some(format!(
            "share/runbook/{} is missing",
            absent.join(" and share/runbook/")
        ));
    }
    let prefix = share.join("..").join("..");
    let manifest = std::fs::read_to_string(share.join("MANIFEST")).ok()?;
    let gone: Vec<&str> = manifest
        .lines()
        .filter(|l| !l.is_empty() && !prefix.join(l).exists())
        .collect();
    if gone.is_empty() {
        return None;
    }
    let first: Vec<&str> = gone.iter().take(NAMED_MISSING).copied().collect();
    let more = if gone.len() > NAMED_MISSING {
        ", ..."
    } else {
        ""
    };
    Some(format!(
        "{} of the files MANIFEST names are missing: {}{more}",
        gone.len(),
        first.join(", ")
    ))
}

/// The `install` line, only for assets of an installed prefix: `incomplete install: <what is missing>`, or the
/// number of files `MANIFEST` names.
fn install_check(a: &Assets) -> Option<Check> {
    if a.source != crate::assets::Source::Installed {
        return None;
    }
    let (found, detail) = match incomplete(&a.root) {
        Some(what) => (false, format!("incomplete install: {what}")),
        None => {
            let n = std::fs::read_to_string(a.root.join("MANIFEST"))
                .map(|m| m.lines().filter(|l| !l.is_empty()).count())
                .unwrap_or(0);
            (
                true,
                format!("whole ({n} files named by MANIFEST are there)"),
            )
        }
    };
    Some(Check {
        name: "install",
        found,
        required: true,
        detail,
        path: None,
        by: "every runbook run from an installed prefix: the files the install script wrote",
        hint: INSTALL_HINT,
    })
}

const LAYOUT_HINT: &str ="restore it in the asset root: it holds every directory of the text layout, as the repository does";

/// One part of the text layout, found under the asset root or `missing` by name. Every part is
/// required, as the `assets` line is (every runbook and the ui need all of them).
fn layout_check(part: &Part, assets: Option<&Assets>) -> Check {
    let (found, detail, path) = match assets {
        None => missing("no assets"),
        Some(a) if part.present(&a.root) => (true, a.root.join(part.path).display().to_string(), Some(a.root.join(part.path))),
        Some(_) => missing(part.absent_reason()),
    };
    Check { name: part.name, found, required: true, detail, path, by: part.by, hint: LAYOUT_HINT }
}

fn missing(why: &str) -> (bool, String, Option<PathBuf>) {
    let d = if why.is_empty() { "missing".to_string() } else { format!("missing ({why})") };
    (false, d, None)
}

fn probe(spec: &Spec, l: &Lookup, var: &dyn Fn(&str) -> Option<OsString>, assets: Option<&Assets>) -> (bool, String, Option<PathBuf>) {
    match &spec.kind {
        Kind::Assets => match l.locate() {
            Ok(a) => (true, format!("{} ({})", a.root.display(), a.source.as_str()), Some(a.root)),
            Err(e) => missing(&e),
        },
        Kind::Helper => match l.helper(spec.name, assets) {
            Some(p) => (true, p.display().to_string(), Some(p)),
            None => missing(""),
        },
        Kind::Login => login(var),
        Kind::Canon => (true, format!("{} (linked, revision {})", CANON_PIN.version, CANON_PIN.commit.get(..8).unwrap_or("")), None),
        Kind::Yq => match l.which("yq") {
            None => missing(""),
            Some(p) => {
                let out = version_output(&p, &["--version"]).unwrap_or_default();
                let v = extract_version(&out);
                if v.starts_with("v4") {
                    (true, v, Some(p))
                } else {
                    missing("found a yq that is not the Go yq v4")
                }
            }
        },
        Kind::Program(bin, args) => match l.which(bin) {
            Some(p) => with_version(p, args),
            None => missing(""),
        },
    }
}

/// The Canon release this binary links: the `b10x-canon` entry of `runbook/Cargo.lock`
/// (`canon_cmd::pin`), which is the one pin there is.
type CanonPin = crate::canon_cmd::Pin;

static CANON_PIN: std::sync::LazyLock<CanonPin> = std::sync::LazyLock::new(crate::canon_cmd::pin);

fn with_version(p: PathBuf, args: &[&str]) -> (bool, String, Option<PathBuf>) {
    let v = version_output(&p, args).map(|o| extract_version(&o)).filter(|v| !v.is_empty()).unwrap_or_else(|| "found".into());
    (true, v, Some(p))
}

/// The API key variable the claude CLI adapter accepts in place of a login file.
const API_KEY_VAR: &str = "ANTHROPIC_API_KEY";

/// Existence only: the login file or the key is never opened or printed.
fn login(var: &dyn Fn(&str) -> Option<OsString>) -> (bool, String, Option<PathBuf>) {
    let set = |k: &str| var(k).filter(|v| !v.is_empty());
    if set(API_KEY_VAR).is_some() {
        return (true, format!("{API_KEY_VAR} is set"), None);
    }
    let file = set("SDC_CLAUDE_CREDENTIALS").map(PathBuf::from).or_else(|| {
        let dir = set("CLAUDE_CONFIG_DIR").map(PathBuf::from).or_else(|| set("HOME").map(|h| PathBuf::from(h).join(".claude")))?;
        Some(dir.join(".credentials.json"))
    });
    match file {
        Some(f) if f.is_file() => (true, "login file present".into(), Some(f)),
        _ => missing("no login file and no ANTHROPIC_API_KEY"),
    }
}

/// stdout and stderr of `path args`, or `None` when it does not start, fails or takes over 5 s.
fn version_output(path: &Path, args: &[&str]) -> Option<String> {
    let mut child = Command::new(path).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait().ok()? {
            Some(st) if st.success() => break,
            Some(_) => return None,
            None if start.elapsed() > Duration::from_secs(5) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }
    let out = child.wait_with_output().ok()?;
    Some(format!("{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

/// The first version-looking token of the first non-empty line (`1.2.3`, `v4.30.8`, `jq-1.8.2`),
/// else that line cut to 40 characters.
pub fn extract_version(out: &str) -> String {
    let Some(line) = out.lines().map(str::trim).find(|l| !l.is_empty()) else { return String::new() };
    for tok in line.split_whitespace() {
        let t = tok.trim_matches(|c: char| c == ',' || c == '(' || c == ')');
        let tail = t.trim_start_matches(|c: char| c.is_ascii_lowercase() || c == '-');
        let starts_digit = |s: &str| s.chars().next().is_some_and(|c| c.is_ascii_digit());
        if starts_digit(t) || (t.starts_with('v') && starts_digit(&t[1..])) {
            return t.to_string();
        }
        if t.contains('-') && starts_digit(tail) && t.len() > tail.len() && t[..t.len() - tail.len()].ends_with('-') {
            return tail.to_string();
        }
    }
    line.chars().take(40).collect()
}

pub fn text(r: &Report) -> String {
    let w1 = r.checks.iter().map(|c| c.name.len()).max().unwrap_or(0);
    let w2 = r.checks.iter().map(|c| c.detail.chars().count()).max().unwrap_or(0).min(60);
    let mut s = String::new();
    for c in &r.checks {
        let level = if c.required { "required" } else { "optional" };
        let status = match (c.found, c.required) {
            (true, _) => "pass",
            (false, true) => "fail",
            (false, false) => "warn",
        };
        s.push_str(&format!("{status}  {:<w1$}  {:<w2$}  {level}  {}", c.name, c.detail, c.by));
        if !c.found {
            s.push_str(&format!("  -- fix: {}", c.hint));
        }
        s.push('\n');
    }
    let bad = r.checks.iter().filter(|c| !c.found && c.required).count();
    let opt = r.checks.iter().filter(|c| !c.found && !c.required).count();
    let scope = r.scope.as_deref().map(|s| format!(" for {s}")).unwrap_or_default();
    if bad == 0 {
        s.push_str(&format!("doctor{scope}: everything required is there ({opt} optional absent)\n"));
    } else {
        s.push_str(&format!("doctor{scope}: {bad} required absent, {opt} optional absent\n"));
    }
    s
}

pub fn json(r: &Report) -> Value {
    json!({
        "ok": r.ok(),
        "for": r.scope,
        "assets": r.assets.as_ref().map(|a| json!({"root": a.root, "source": a.source.as_str()})),
        "checks": r.checks.iter().map(|c| json!({
            "name": c.name,
            "found": c.found,
            "required": c.required,
            "version": if c.found { Value::String(c.detail.clone()) } else { Value::Null },
            "detail": c.detail,
            "path": c.path,
            "needed_by": c.by,
            "install": if c.found { Value::Null } else { Value::String(c.hint.into()) },
        })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_picked_out_of_the_usual_lines() {
        for (line, want) in [
            ("jq-1.8.2", "1.8.2"),
            ("yq (https://example.org/yq/) version v4.30.8", "v4.30.8"),
            ("git version 2.56.0", "2.56.0"),
            ("GNU bash, version 5.3.20(1)-release (x86_64-pc-linux-gnu)", "5.3.20(1)-release"),
            ("tar (GNU tar) 1.35", "1.35"),
            ("2.1.330 (Claude Code)", "2.1.330"),
            ("task 3.54.0+b5aef1c2", "3.54.0+b5aef1c2"),
            ("no digits here at all", "no digits here at all"),
        ] {
            assert_eq!(extract_version(line), want, "{line}");
        }
    }

    #[test]
    fn the_helper_table_matches_the_driver_binaries() {
        let manifest = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../driver/Cargo.toml")).unwrap();
        let mut in_bin = false;
        let mut want: Vec<String> = Vec::new();
        for l in manifest.lines() {
            if l.starts_with('[') {
                in_bin = l == "[[bin]]";
            } else if in_bin {
                if let Some(n) = l.strip_prefix("name = \"").and_then(|r| r.strip_suffix('"')) {
                    want.push(n.into());
                }
            }
        }
        want.push("sdc-watch".into());
        want.sort();
        want.dedup();
        let mut have: Vec<String> = helper_names().into_iter().map(String::from).collect();
        have.sort();
        assert_eq!(have, want);
    }

    fn whole_prefix(t: &crate::assets::testdir::Dir) -> Assets {
        let share = t.dir("prefix/share/runbook");
        t.dir("prefix/share/runbook/protocol");
        t.dir("prefix/bin");
        for f in [
            "bin/runbook",
            "share/runbook/VERSION",
            "share/runbook/MANIFEST",
        ] {
            std::fs::write(t.path().join("prefix").join(f), "x\n").unwrap();
        }
        std::fs::write(
            share.join("MANIFEST"),
            "bin/runbook\nshare/runbook/VERSION\nshare/runbook/MANIFEST\n",
        )
        .unwrap();
        Assets {
            root: share,
            source: crate::assets::Source::Installed,
        }
    }

    #[test]
    fn a_whole_prefix_passes_the_install_line_and_one_that_is_not_installed_has_none() {
        let t = crate::assets::testdir::Dir::new("doctor-whole");
        let a = whole_prefix(&t);
        let c = install_check(&a).expect("an installed prefix has the line");
        assert!(c.found && c.required, "{}", c.detail);
        let checkout = Assets {
            root: a.root.clone(),
            source: crate::assets::Source::Checkout,
        };
        assert!(install_check(&checkout).is_none());
    }

    /// The `install` line of a prefix that is not whole: failed, required, one line, `incomplete install: <what>`.
    fn incomplete_line(a: &Assets) -> String {
        let c = install_check(a).unwrap();
        assert!(!c.found && c.required, "{}", c.detail);
        assert!(!c.detail.contains('\n'), "one line: {}", c.detail);
        assert!(c.detail.starts_with("incomplete install: "), "{}", c.detail);
        c.detail
    }

    #[test]
    fn the_marker_of_an_install_that_did_not_finish_is_an_incomplete_install() {
        let t = crate::assets::testdir::Dir::new("doctor-marker");
        let a = whole_prefix(&t);
        std::fs::write(a.root.join("INSTALLING"), "").unwrap();
        assert!(incomplete_line(&a).contains("INSTALLING"));
        std::fs::remove_file(a.root.join("INSTALLING")).unwrap();
        assert!(install_check(&a).unwrap().found);
    }

    #[test]
    fn a_missing_version_file_is_an_incomplete_install() {
        let t = crate::assets::testdir::Dir::new("doctor-no-version");
        let a = whole_prefix(&t);
        std::fs::remove_file(a.root.join("VERSION")).unwrap();
        let d = incomplete_line(&a);
        assert!(d.contains("VERSION") && !d.contains("MANIFEST"), "{d}");
    }

    #[test]
    fn a_missing_manifest_is_an_incomplete_install() {
        let t = crate::assets::testdir::Dir::new("doctor-no-manifest");
        let a = whole_prefix(&t);
        std::fs::remove_file(a.root.join("MANIFEST")).unwrap();
        let d = incomplete_line(&a);
        assert!(d.contains("MANIFEST") && !d.contains("VERSION"), "{d}");
    }

    #[test]
    fn a_file_the_manifest_names_that_is_missing_is_an_incomplete_install_and_is_named() {
        let t = crate::assets::testdir::Dir::new("doctor-gone-file");
        let a = whole_prefix(&t);
        let listed = "bin/runbook\nshare/runbook/VERSION\nshare/runbook/MANIFEST\nbin/gone\nshare/runbook/protocol/gone.yaml\n";
        std::fs::write(a.root.join("MANIFEST"), listed).unwrap();
        let d = incomplete_line(&a);
        assert!(
            d.contains("bin/gone")
                && d.contains("share/runbook/protocol/gone.yaml")
                && d.contains("2 of"),
            "{d}"
        );
        std::fs::remove_file(t.path().join("prefix/bin/runbook")).unwrap();
        assert!(
            incomplete_line(&a).contains("bin/runbook"),
            "a listed file that was there is missing now"
        );
        std::fs::write(
            a.root.join("MANIFEST"),
            "bin/a\nbin/b\nbin/c\nbin/d\nbin/e\n",
        )
        .unwrap();
        let d = incomplete_line(&a);
        assert!(
            d.contains("5 of") && d.contains("bin/a") && !d.contains("bin/d") && d.ends_with("..."),
            "at most three are named: {d}"
        );
    }

    /// A binary of a prefix whose install stopped before `protocol/` was copied: started in a checkout, its doctor
    /// still reports that prefix, says `incomplete install` and fails, and its assets are not the checkout's.
    #[test]
    fn doctor_of_a_prefix_that_stopped_before_protocol_says_incomplete_install_in_a_checkout() {
        let t = crate::assets::testdir::Dir::new("doctor-stopped");
        let checkout = t.dir("co");
        std::fs::create_dir_all(checkout.join("protocol")).unwrap();
        std::fs::create_dir_all(checkout.join("driver/examples/sdc")).unwrap();
        t.dir("prefix/share/runbook");
        std::fs::write(
            t.path().join("prefix/share/runbook/INSTALLING"),
            "started\n",
        )
        .unwrap();
        let l = Lookup {
            home: None,
            exe: Some(t.dir("prefix/bin").join("runbook")),
            cwd: Some(checkout),
            path: None,
            ceiling: Some(t.path().to_path_buf()),
        };
        let r = run(&l, &|_| None, None, None);
        assert!(!r.ok(), "the doctor of a stopped install fails");
        assert!(r.assets.is_none(), "{:?}", r.assets);
        let install = r.checks.iter().find(|c| c.name == "install").unwrap();
        assert!(
            !install.found
                && install.required
                && install.detail.starts_with("incomplete install: ")
                && install.detail.contains("INSTALLING"),
            "{}",
            install.detail
        );
        let assets = r.checks.iter().find(|c| c.name == "assets").unwrap();
        assert!(
            !assets.found && assets.detail.contains("incomplete install"),
            "{}",
            assets.detail
        );
        std::fs::remove_file(t.path().join("prefix/share/runbook/INSTALLING")).unwrap();
        let r = run(&l, &|_| None, None, None);
        let install = r.checks.iter().find(|c| c.name == "install").unwrap();
        assert!(
            !install.found && install.detail.contains("VERSION"),
            "no marker, no protocol/, no VERSION: still an incomplete install ({})",
            install.detail
        );
    }

    #[test]
    fn login_checks_existence_only() {
        let none = |_: &str| None;
        assert!(!login(&none).0);
        let key = |k: &str| (k == API_KEY_VAR).then(|| OsString::from("secret-value"));
        let (ok, detail, path) = login(&key);
        assert!(ok && path.is_none() && !detail.contains("secret-value"));
    }
}
