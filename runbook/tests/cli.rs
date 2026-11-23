mod daemon;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

mod linked_canon;
mod canon_cmd;
mod ess_digest;
mod exit_classes;
mod loom_run;
mod reply_lib;
mod test_suite;

const BIN: &str = env!("CARGO_BIN_EXE_runbook");

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/repo")
}

struct World {
    root: PathBuf,
}

impl World {
    fn new(tag: &str) -> World {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for d in ["bin", "home"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        for d in LAYOUT_DIRS {
            std::fs::create_dir_all(root.join("assets").join(d)).unwrap();
        }
        std::fs::write(root.join("assets/bindings/sdc.yaml"), "").unwrap();
        for t in TOOLS {
            let p = root.join("assets").join(t);
            std::fs::write(&p, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        World { root }
    }

    fn fake(&self, name: &str) {
        let out = match name {
            "yq" => "yq version v4.30.8",
            "jq" => "jq-1.7",
            _ => "1.2.3",
        };
        let p = self.root.join("bin").join(name);
        std::fs::write(&p, format!("#!/bin/sh\necho '{out}'\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn fakes(&self, names: &[&str]) {
        names.iter().for_each(|n| self.fake(n));
    }

    /// A login file of the claude CLI under the world's HOME.
    fn login(&self, content: &str) {
        std::fs::create_dir_all(self.root.join("home/.claude")).unwrap();
        std::fs::write(self.root.join("home/.claude/.credentials.json"), content).unwrap();
    }

    /// `runbook <args>` with nothing in its environment but a PATH of the fake programs, an empty
    /// HOME and the asset root of this world.
    fn run(&self, args: &[&str], extra: &[(&str, &str)]) -> Output {
        let mut c = Command::new(BIN);
        c.args(args)
            .env_clear()
            .env("PATH", self.root.join("bin"))
            .env("HOME", self.root.join("home"))
            .env("RUNBOOK_HOME", self.root.join("assets"))
            .current_dir(&self.root);
        for (k, v) in extra {
            c.env(k, v);
        }
        c.output().unwrap()
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

const LAYOUT_DIRS: [&str; 9] =
    ["protocol", "scripts", "driver/examples/sdc", "bindings", "bindings/tools", "instructions", "views", "cycles", "ui"];
const TOOLS: [&str; 2] = [
    "bindings/tools/sdc-tool.sh",
    "bindings/tools/llm-adapter-claude-cli.sh",
];

const PROGRAMS: [&str; 13] =
    ["bash", "jq", "yq", "git", "claude", "aep", "ess", "task", "slot", "cargo", "cargo-clippy", "rustfmt", "node"];
const HELPERS: [&str; 8] = [
    "sdc-node",
    "sdc-findings",
    "sdc-bugfix",
    "sdc-decision-request",
    "sdc-reply",
    "sdc-replay",
    "sdc-check-routes",
    "sdc-conform",
];

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn fields(l: &str) -> Vec<&str> {
    l.split("  ").map(str::trim).filter(|s| !s.is_empty()).collect()
}

/// (status, name, detail) of each check line of `runbook doctor`.
fn checks(o: &Output) -> Vec<(String, String, String)> {
    stdout(o)
        .lines()
        .filter(|l| !l.starts_with("doctor"))
        .map(|l| {
            let f = fields(l);
            (f[0].to_string(), f[1].to_string(), f[2].to_string())
        })
        .collect()
}

fn line_of(o: &Output, name: &str) -> String {
    stdout(o).lines().find(|l| fields(l).get(1) == Some(&name)).unwrap_or_else(|| panic!("no {name} line in\n{}", stdout(o))).to_string()
}

fn failed(o: &Output) -> Vec<String> {
    checks(o).into_iter().filter(|(s, _, _)| s == "fail").map(|(_, n, _)| n).collect()
}

#[test]
fn doctor_prints_one_fail_line_per_missing_program_and_exits_1() {
    let w = World::new("doctor-missing");
    let present = ["bash", "jq", "yq", "git", "claude", "aep", "cargo"];
    w.fakes(&present);
    w.fakes(&HELPERS);
    let o = w.run(&["doctor"], &[]);
    assert_eq!(o.status.code(), Some(1), "{}", stdout(&o));
    let out = stdout(&o);
    let missing = failed(&o);
    for want in ["claude-login", "ess", "task", "slot", "clippy", "rustfmt", "node"] {
        assert_eq!(missing.iter().filter(|m| *m == want).count(), 1, "{want} in\n{out}");
        let line = line_of(&o, want);
        assert!(line.starts_with("fail") && line.contains("missing") && line.contains("required") && line.contains("-- fix:"), "{line}");
    }
    for ok in present {
        assert!(line_of(&o, ok).starts_with("pass"), "{ok} not passed\n{out}");
    }
    assert!(out.contains("rustup component add clippy"), "{out}");
}

#[test]
fn doctor_exits_0_when_everything_required_is_there() {
    let w = World::new("doctor-all");
    w.fakes(&PROGRAMS);
    w.fakes(&HELPERS);
    w.login("{}");
    let o = w.run(&["doctor"], &[]);
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
    let out = stdout(&o);
    let required: Vec<&str> = out.lines().filter(|l| l.contains("  required  ")).collect();
    assert!(required.len() > 20, "{out}");
    for l in required {
        assert!(l.starts_with("pass") && !l.contains("-- fix:"), "{l}");
    }
    assert!(out.lines().filter(|l| !l.starts_with("doctor")).all(|l| l.starts_with("pass") || l.starts_with("warn")), "{out}");
    assert!(out.contains("everything required is there"), "{out}");
    assert!(out.contains("v4.30.8"), "{out}");
    for p in ["jq", "yq"] {
        assert!(line_of(&o, p).starts_with("pass"));
    }
}

#[test]
fn a_login_file_counts_and_is_not_read() {
    let w = World::new("doctor-login");
    w.fakes(&PROGRAMS);
    w.fakes(&HELPERS);
    assert_eq!(w.run(&["doctor"], &[]).status.code(), Some(1));
    w.login("secret-token-value");
    let o = w.run(&["doctor"], &[]);
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
    assert!(!stdout(&o).contains("secret-token-value"));
}

#[test]
fn a_python_yq_is_not_the_yq() {
    let w = World::new("doctor-yq");
    w.fakes(&PROGRAMS);
    w.fakes(&HELPERS);
    w.login("{}");
    std::fs::write(w.root.join("bin/yq"), "#!/bin/sh\necho 'yq 3.4.3'\n").unwrap();
    let o = w.run(&["doctor"], &[]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(failed(&o), ["yq"]);
}

#[test]
fn doctor_for_one_runbook_checks_only_what_it_needs() {
    let w = World::new("doctor-for");
    w.fakes(&["bash", "jq", "yq", "git", "claude", "aep", "cargo", "cargo-clippy", "rustfmt"]);
    w.fakes(&HELPERS);
    w.login("{}");
    let o = w.run(&["doctor", "--for", "sdc-bugfix"], &[]);
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
    let names: Vec<String> = checks(&o).into_iter().map(|(_, n, _)| n).collect();
    for absent in ["ess", "task", "node"] {
        assert!(!names.iter().any(|n| n == absent), "{absent} listed for sdc-bugfix");
    }
    let slot = line_of(&o, "slot");
    assert!(slot.starts_with("warn") && slot.contains("optional"), "{slot}");
    let o = w.run(&["doctor", "--for", "sdc"], &[]);
    assert_eq!(o.status.code(), Some(1));
    let names = failed(&o);
    assert!(["ess", "task", "slot"].iter().all(|n| names.iter().any(|m| m == n)), "{names:?}");
    assert!(!names.iter().any(|n| n == "clippy" || n == "node"));
    let o = w.run(&["doctor", "--for", "sdc-story"], &[]);
    assert_eq!(o.status.code(), Some(64));
}

#[test]
fn doctor_json_reports_every_check() {
    let w = World::new("doctor-json");
    w.fakes(&["jq"]);
    let o = w.run(&["doctor", "--json"], &[]);
    assert_eq!(o.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["ok"], false);
    let jq = v["checks"].as_array().unwrap().iter().find(|c| c["name"] == "jq").unwrap();
    assert_eq!((jq["found"].as_bool(), jq["version"].as_str(), jq["required"].as_bool()), (Some(true), Some("1.7"), Some(true)));
    let git = v["checks"].as_array().unwrap().iter().find(|c| c["name"] == "git").unwrap();
    assert_eq!((git["found"].as_bool(), git["install"].is_string()), (Some(false), true));
}

fn canon_pin() -> serde_json::Value {
    let lock = std::fs::read_to_string(repo().join("runbook/Cargo.lock")).unwrap();
    let entry = &lock[lock.find("name = \"b10x-canon\"\n").unwrap()..];
    let mut lines = entry.lines().skip(1);
    let version = lines.next().unwrap().trim_start_matches("version = \"").trim_end_matches('"');
    let commit = lines.next().unwrap().rsplit('#').next().unwrap().trim_end_matches('"');
    serde_json::json!({"version": version, "commit": commit})
}

#[test]
fn doctor_reports_missing_assets_with_what_to_do() {
    let w = World::new("doctor-no-assets");
    let o = w.run(&["doctor"], &[("RUNBOOK_HOME", w.root.join("home").to_str().unwrap())]);
    assert_eq!(o.status.code(), Some(1));
    let line = line_of(&o, "assets");
    assert!(line.starts_with("fail") && line.contains("no protocol/ directory") && line.contains("-- fix:"), "{line}");
}

fn world_with_everything(tag: &str) -> World {
    let w = World::new(tag);
    w.fakes(&PROGRAMS);
    w.fakes(&HELPERS);
    w.login("{}");
    w
}

#[test]
fn doctor_reports_every_directory_of_the_text_layout_present_in_this_checkout() {
    let w = World::new("doctor-layout-checkout");
    let o = w.run(&["doctor"], &[("RUNBOOK_HOME", repo().to_str().unwrap())]);
    for name in ["protocol/", "scripts/", "driver/examples/sdc/", "bindings/", "bindings/*.yaml", "bindings/tools/", "instructions/", "views/", "cycles/", "ui/"] {
        let line = line_of(&o, name);
        assert!(line.starts_with("pass") && line.contains("required"), "{line}");
    }
    for tool in TOOLS {
        assert!(line_of(&o, tool).starts_with("pass"), "{}", line_of(&o, tool));
    }
}

#[test]
fn doctor_names_the_directory_of_the_text_layout_that_is_missing() {
    let w = world_with_everything("doctor-layout-fake");
    assert_eq!(w.run(&["doctor"], &[]).status.code(), Some(0));
    for dir in ["instructions", "views", "cycles", "bindings/tools"] {
        let at = w.root.join("assets").join(dir);
        let away = w.root.join("away");
        std::fs::rename(&at, &away).unwrap();
        let o = w.run(&["doctor"], &[]);
        assert_eq!(o.status.code(), Some(1), "{dir}\n{}", stdout(&o));
        let line = line_of(&o, &format!("{dir}/"));
        assert!(line.starts_with("fail") && line.contains("missing") && line.contains("-- fix:"), "{line}");
        std::fs::rename(&away, &at).unwrap();
    }
    std::fs::remove_file(w.root.join("assets/bindings/sdc.yaml")).unwrap();
    let o = w.run(&["doctor"], &[]);
    assert_eq!(failed(&o), ["bindings/*.yaml"], "{}", stdout(&o));
}

#[test]
fn doctor_on_a_copy_of_this_checkout_without_instructions_names_it() {
    let w = World::new("doctor-layout-copy");
    let copy = w.root.join("copy");
    std::fs::create_dir_all(&copy).unwrap();
    for d in LAYOUT_DIRS.iter().map(|d| d.split('/').next().unwrap()).collect::<std::collections::BTreeSet<_>>() {
        std::os::unix::fs::symlink(repo().join(d), copy.join(d)).unwrap();
    }
    let o = w.run(&["doctor"], &[("RUNBOOK_HOME", copy.to_str().unwrap())]);
    assert!(line_of(&o, "instructions/").starts_with("pass"));
    std::fs::rename(copy.join("instructions"), copy.join("instructions.away")).unwrap();
    let o = w.run(&["doctor"], &[("RUNBOOK_HOME", copy.to_str().unwrap())]);
    let line = line_of(&o, "instructions/");
    assert!(line.starts_with("fail") && line.contains("missing") && line.contains("-- fix:"), "{line}");
}

#[test]
fn doctor_checks_the_tools_at_bindings_tools_and_that_they_run() {
    let w = world_with_everything("doctor-tools");
    for tool in TOOLS {
        let line = line_of(&w.run(&["doctor"], &[]), tool);
        assert!(line.starts_with("pass") && line.contains("required"), "{line}");
    }
    let tool = w.root.join("assets/bindings/tools/llm-adapter-claude-cli.sh");
    std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(failed(&w.run(&["doctor"], &[])), ["bindings/tools/llm-adapter-claude-cli.sh"]);
    std::fs::remove_file(&tool).unwrap();
    let o = w.run(&["doctor"], &[]);
    assert_eq!(failed(&o), ["bindings/tools/llm-adapter-claude-cli.sh"]);
    assert!(line_of(&o, "bindings/tools/llm-adapter-claude-cli.sh").contains("-- fix:"));
}

#[test]
fn doctor_cites_no_line_numbers() {
    let w = World::new("doctor-no-lines");
    let o = w.run(&["doctor"], &[]);
    for l in stdout(&o).lines() {
        let cites = l.split(|c: char| c.is_whitespace() || c == ',' || c == '(').any(|t| {
            let t = t.trim_end_matches([')', ';']);
            [".sh", ".rs", ".mjs", ".yaml", ".json"].iter().any(|e| t.split_once(&format!("{e}:")).is_some_and(|(_, n)| n.starts_with(|c: char| c.is_ascii_digit())))
        });
        assert!(!cites, "{l}");
    }
}

#[test]
fn list_reads_the_fixture_protocols_without_starting_anything() {
    let w = World::new("list-fixture");
    let o = w.run(&["list"], &[("RUNBOOK_HOME", fixture().to_str().unwrap())]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let rows: Vec<String> = stdout(&o).lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).collect();
    assert_eq!(
        rows,
        [
            "sdc runbook Fixture root runbook that starts child cases",
            "sdc-bugfix runbook Fixture bugfix runbook",
            "sdc-story child Fixture child protocol for one story",
        ]
    );
    let mut entries: Vec<String> = std::fs::read_dir(&w.root).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    entries.sort();
    assert_eq!(entries, ["assets", "bin", "home"], "list wrote nothing into the working directory");
}

#[test]
fn list_without_a_protocol_directory_fails() {
    let w = World::new("list-none");
    let o = w.run(&["list"], &[("RUNBOOK_HOME", w.root.join("home").to_str().unwrap())]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("no protocol/ directory"));
}

#[test]
fn list_shows_the_nine_protocols_of_this_repository() {
    let w = World::new("list");
    let root = repo();
    let o = Command::new(BIN).arg("list").env("RUNBOOK_HOME", &root).current_dir(&w.root).output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    let rows: Vec<Vec<String>> = stdout(&o).lines().map(|l| l.split_whitespace().map(String::from).collect()).collect();
    assert_eq!(rows.len(), 9, "{}", stdout(&o));
    let kind = |id: &str| rows.iter().find(|r| r[0] == id).unwrap_or_else(|| panic!("{id} missing"))[1].clone();
    assert_eq!(kind("sdc"), "runbook");
    assert_eq!(kind("sdc-bugfix"), "runbook");
    for child in ["sdc-intake", "sdc-direction", "sdc-epic", "sdc-spec", "sdc-plan", "sdc-story", "sdc-release"] {
        assert_eq!(kind(child), "child");
    }
    assert!(rows.iter().all(|r| r.len() > 2), "every row has a title");

    let o = Command::new(BIN).args(["list", "--json"]).env("RUNBOOK_HOME", &root).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    let a = v.as_array().unwrap();
    assert_eq!(a.len(), 9);
    let startable: Vec<&str> = a.iter().filter(|e| e["startable"] == true).map(|e| e["id"].as_str().unwrap()).collect();
    assert_eq!(startable, ["sdc", "sdc-bugfix"]);
    assert!(a.iter().all(|e| !e["title"].as_str().unwrap().is_empty()));
}

#[test]
fn list_finds_a_checkout_from_the_working_directory() {
    let o = Command::new(BIN).arg("list").env_remove("RUNBOOK_HOME").current_dir(repo().join("docs")).output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(stdout(&o).lines().count(), 9);
}

#[test]
fn version_is_the_repository_version() {
    let manifest = std::fs::read_to_string(repo().join("driver/Cargo.toml")).unwrap();
    let want = manifest.lines().find_map(|l| l.strip_prefix("version = \"").and_then(|r| r.strip_suffix('"'))).unwrap();
    let o = Command::new(BIN).arg("--version").output().unwrap();
    assert_eq!(stdout(&o).trim(), format!("runbook {want}"));
}

#[test]
fn version_unknown_subcommand_and_help() {
    let o = Command::new(BIN).arg("--version").output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(stdout(&o).trim(), format!("runbook {}", env!("CARGO_PKG_VERSION")));
    let o = Command::new(BIN).arg("frobnicate").output().unwrap();
    assert_eq!(o.status.code(), Some(64));
    assert!(String::from_utf8_lossy(&o.stderr).contains("Usage:"));
    let o = Command::new(BIN).arg("help").output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    let h = stdout(&o);
    assert!(h.contains("doctor") && h.contains("list") && h.contains("check"), "{h}");
    for later in ["resume", "stop", "ui", "handback", "simulate"] {
        assert!(!h.lines().any(|l| l.trim_start().starts_with(&format!("{later} "))), "{later} listed\n{h}");
        assert_eq!(Command::new(BIN).arg(later).output().unwrap().status.code(), Some(64), "{later}");
    }
    assert_eq!(Command::new(BIN).output().unwrap().status.code(), Some(64));
}

fn copy_dir(from: &Path, to: &Path, skip: &[&str]) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let name = e.file_name();
        let (src, dst) = (e.path(), to.join(&name));
        if src.is_dir() {
            if !skip.contains(&name.to_string_lossy().as_ref()) {
                copy_dir(&src, &dst, &[]);
            }
        } else {
            std::fs::copy(&src, &dst).unwrap();
        }
    }
}

/// A scratch copy of the text of this checkout: what `runbook check` reads. The fixtures the
/// cycles name are linked, not copied; `ui/` is an empty directory.
fn text_copy(w: &World) -> PathBuf {
    let co = w.root.join("co");
    let r = repo();
    for d in ["protocol", "bindings", "views", "cycles"] {
        copy_dir(&r.join(d), &co.join(d), &[]);
    }
    copy_dir(&r.join("instructions"), &co.join("instructions"), &["golden"]);
    copy_dir(&r.join("driver/src"), &co.join("driver/src"), &[]);
    copy_dir(&r.join("runbook/src"), &co.join("runbook/src"), &[]);
    std::fs::create_dir_all(co.join("ui")).unwrap();
    std::fs::create_dir_all(co.join("scripts")).unwrap();
    std::fs::create_dir_all(co.join("docs")).unwrap();
    std::fs::copy(r.join("docs/cycles-suite.md"), co.join("docs/cycles-suite.md")).unwrap();
    std::fs::create_dir_all(co.join("driver/examples/sdc")).unwrap();
    for e in std::fs::read_dir(r.join("driver/examples/sdc")).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if n.ends_with(".bindings.json") {
            std::fs::copy(e.path(), co.join("driver/examples/sdc").join(&n)).unwrap();
        }
    }
    std::os::unix::fs::symlink(r.join("driver/examples/sdc/fixtures"), co.join("driver/examples/sdc/fixtures")).unwrap();
    std::os::unix::fs::symlink(r.join("decision"), co.join("decision")).unwrap();
    co
}

/// Replace the first `from` of `file` under `co` by `to`; the text must be there.
fn edit(co: &Path, file: &str, from: &str, to: &str) {
    let p = co.join(file);
    let t = std::fs::read_to_string(&p).unwrap();
    assert!(t.contains(from), "{file} has no {from:?}");
    std::fs::write(&p, t.replacen(from, to, 1)).unwrap();
}

fn check(w: &World, co: &Path) -> Output {
    w.run(&["check"], &[("RUNBOOK_HOME", co.to_str().unwrap())])
}

/// The finding lines of `runbook check`: every line but `note:` and the closing `check:` line.
fn findings(o: &Output) -> Vec<String> {
    stdout(o).lines().filter(|l| !l.starts_with("note: ") && !l.starts_with("check: ")).map(String::from).collect()
}

/// The check fails, and a finding names `file` and holds every text of `has`, with a fix.
fn assert_finding(o: &Output, file: &str, has: &[&str]) {
    assert_eq!(o.status.code(), Some(1), "{}", stdout(o));
    let hit = findings(o).into_iter().find(|l| l.starts_with(&format!("{file}: ")) && has.iter().all(|h| l.contains(h)));
    let hit = hit.unwrap_or_else(|| panic!("no finding of {file} with {has:?}\n{}", stdout(o)));
    assert!(hit.contains("; fix: "), "{hit}");
}

#[test]
fn check_passes_on_this_checkout() {
    let w = World::new("check-checkout");
    let o = w.run(&["check"], &[("RUNBOOK_HOME", repo().to_str().unwrap())]);
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
    assert!(stdout(&o).starts_with("check: ok ("), "{}", stdout(&o));
    assert!(stdout(&o).contains("compared with docs/cycles-suite.md"), "{}", stdout(&o));
}

#[test]
fn check_passes_on_a_clean_copy_and_json_says_so() {
    let w = World::new("check-clean");
    let co = text_copy(&w);
    let o = w.run(&["check", "--json"], &[("RUNBOOK_HOME", co.to_str().unwrap())]);
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
    let j: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(j["ok"], true);
    assert_eq!(j["findings"].as_array().unwrap().len(), 0);
    assert!(j["summary"].as_str().unwrap().contains("compared with"), "{j}");
}

#[test]
fn check_json_lists_each_finding_with_file_what_and_fix() {
    let w = World::new("check-json");
    let co = text_copy(&w);
    edit(&co, "views/sdc.view.json", "runbook.view/1", "runbook.view/9");
    let o = w.run(&["check", "--json"], &[("RUNBOOK_HOME", co.to_str().unwrap())]);
    assert_eq!(o.status.code(), Some(1));
    let j: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(j["ok"], false);
    let f = &j["findings"][0];
    assert_eq!((f["check"].as_str(), f["file"].as_str()), (Some("views"), Some("views/sdc.view.json")));
    assert!(f["what"].as_str().unwrap().contains("runbook.view/9") && !f["fix"].as_str().unwrap().is_empty(), "{f}");
}

#[test]
fn check_without_assets_fails_and_says_why() {
    let w = World::new("check-no-assets");
    let o = w.run(&["check"], &[("RUNBOOK_HOME", w.root.join("home").to_str().unwrap())]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("RUNBOOK_HOME"), "{:?}", o);
}

#[test]
fn check_names_a_part_of_the_layout_that_is_missing() {
    let w = World::new("check-layout");
    let co = text_copy(&w);
    std::fs::remove_dir_all(co.join("views")).unwrap();
    std::fs::set_permissions(co.join("bindings/tools/llm-adapter-claude-cli.sh"), std::fs::Permissions::from_mode(0o644)).unwrap();
    let o = check(&w, &co);
    assert_finding(&o, "views/", &["missing", "no such directory"]);
    assert_finding(&o, "bindings/tools/llm-adapter-claude-cli.sh", &["missing", "not an executable file"]);
}

#[test]
fn check_fails_a_binding_that_names_a_missing_instruction_file() {
    let w = World::new("check-instr-missing");
    let co = text_copy(&w);
    edit(&co, "bindings/sdc-intake.yaml", "instructions/idea.dissect.md", "instructions/no-such-file.md");
    let o = check(&w, &co);
    assert_finding(&o, "bindings/sdc-intake.yaml", &["idea.dissect", "instructions/no-such-file.md", "existing"]);
    edit(&co, "bindings/sdc-intake.yaml", "instructions/no-such-file.md", "instructions/../README.md");
    assert_finding(&check(&w, &co), "bindings/sdc-intake.yaml", &["instructions/../README.md", "without .."]);
}

#[test]
fn check_fails_a_json_binding_that_names_a_missing_instruction_file() {
    let w = World::new("check-instr-missing-json");
    let co = text_copy(&w);
    edit(&co, "driver/examples/sdc/sdc-intake.bindings.json", "instructions/idea.dissect.md", "instructions/no-such-file.md");
    assert_finding(&check(&w, &co), "driver/examples/sdc/sdc-intake.bindings.json", &["idea.dissect", "instructions/no-such-file.md"]);
}

#[test]
fn check_fails_two_forms_that_name_different_files() {
    let w = World::new("check-forms-differ");
    let co = text_copy(&w);
    edit(&co, "driver/examples/sdc/sdc-intake.bindings.json", "instructions/idea.dissect.md", "instructions/questions.write.md");
    let o = check(&w, &co);
    assert_finding(&o, "bindings/sdc-intake.yaml", &["idea.dissect", "differs from llm.instructions", "sdc-intake.bindings.json"]);
    assert_eq!(findings(&o).len(), 1, "{}", stdout(&o));
}

#[test]
fn check_fails_a_binding_pair_with_one_form_missing_and_an_unreadable_one() {
    let w = World::new("check-pair");
    let co = text_copy(&w);
    std::fs::remove_file(co.join("bindings/sdc-plan.yaml")).unwrap();
    std::fs::write(co.join("bindings/sdc-epic.yaml"), "actions: [unclosed\n").unwrap();
    std::fs::write(co.join("driver/examples/sdc/sdc-spec.bindings.json"), "{").unwrap();
    let o = check(&w, &co);
    assert_finding(&o, "driver/examples/sdc/sdc-plan.bindings.json", &["no bindings/sdc-plan.yaml"]);
    assert_finding(&o, "bindings/sdc-epic.yaml", &["does not parse"]);
    assert_finding(&o, "driver/examples/sdc/sdc-spec.bindings.json", &["does not parse"]);
}

#[test]
fn check_fails_a_binding_whose_program_is_not_an_executable_tool() {
    let w = World::new("check-tool");
    let co = text_copy(&w);
    edit(&co, "bindings/sdc-intake.yaml", "tools/sdc-tool.sh, check-idea", "tools/no-such-tool.sh, check-idea");
    assert_finding(&check(&w, &co), "bindings/sdc-intake.yaml", &["idea.check", "tools/no-such-tool.sh", "not an executable bindings/tools/<file>"]);
    edit(&co, "bindings/sdc-intake.yaml", "tools/no-such-tool.sh, check-idea", "../tools/sdc-tool.sh, check-idea");
    assert_finding(&check(&w, &co), "bindings/sdc-intake.yaml", &["../tools/sdc-tool.sh", "a path with .."]);
}

#[test]
fn check_fails_an_instruction_file_with_a_bad_placeholder() {
    let w = World::new("check-placeholder");
    let co = text_copy(&w);
    let p = co.join("instructions/idea.dissect.md");
    let t = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, format!("{t}\nGiven {{{{Bad-Name}}}} and {{{{good_name}}}}.\n")).unwrap();
    let o = check(&w, &co);
    assert_finding(&o, "instructions/idea.dissect.md", &["{{Bad-Name}}", "{{[a-z_]+}}"]);
    assert_eq!(findings(&o).len(), 1, "{}", stdout(&o));
}

#[test]
fn check_fails_an_instruction_file_nothing_names() {
    let w = World::new("check-unreferenced");
    let co = text_copy(&w);
    std::fs::write(co.join("instructions/orphan.md"), "Nothing names this file.\n").unwrap();
    std::fs::write(co.join("instructions/decider/orphan-two.md"), "Nor this one.\n").unwrap();
    let o = check(&w, &co);
    assert_finding(&o, "instructions/orphan.md", &["no binding, program or other instruction file names it"]);
    assert_finding(&o, "instructions/decider/orphan-two.md", &["names it"]);
    assert_eq!(findings(&o).len(), 2, "{}", stdout(&o));
}

#[test]
fn check_counts_a_name_in_a_rust_source_of_the_driver() {
    let w = World::new("check-rust-names");
    let co = text_copy(&w);
    std::fs::write(co.join("instructions/rust-only.md"), "Only a Rust binary reads this file.\n").unwrap();
    std::fs::write(co.join("instructions/rust-near.md"), "Only a longer name is in the Rust source.\n").unwrap();
    let bin = co.join("driver/src/bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(
        bin.join("reader.rs"),
        "fn main() {\n    let _ = render(\"instructions/rust-only.md\");\n    let _ = render(\"instructions/rust-near.md2\");\n}\n",
    )
    .unwrap();
    let o = check(&w, &co);
    assert_finding(&o, "instructions/rust-near.md", &["no binding, program or other instruction file names it"]);
    assert_eq!(findings(&o).len(), 1, "{}", stdout(&o));
    std::fs::remove_file(bin.join("reader.rs")).unwrap();
    let o = check(&w, &co);
    assert_finding(&o, "instructions/rust-only.md", &["names it"]);
    assert_finding(&o, "instructions/rust-near.md", &["names it"]);
    assert_eq!(findings(&o).len(), 2, "{}", stdout(&o));
}

#[test]
fn check_fails_a_view_with_another_format_id() {
    let w = World::new("check-view");
    let co = text_copy(&w);
    edit(&co, "views/sdc.view.json", "runbook.view/1", "runbook.view/2");
    let o = check(&w, &co);
    assert_finding(&o, "views/sdc.view.json", &["runbook.view/2", "runbook.view/1"]);
    std::fs::write(co.join("views/sdc.view.json"), "[]").unwrap();
    assert_finding(&check(&w, &co), "views/sdc.view.json", &["not a JSON object"]);
    std::fs::write(co.join("views/other.view.json"), "{\"phases\": {}}").unwrap();
    assert_finding(&check(&w, &co), "views/other.view.json", &["no \"format\" member"]);
}

#[test]
fn check_accepts_a_view_product_and_refuses_one_that_is_not_a_path_inside_the_run() {
    let w = World::new("check-view-product");
    let co = text_copy(&w);
    let view = |product: &str| format!("{{\"format\": \"runbook.view/1\", \"product\": {product}}}");
    std::fs::write(co.join("views/other.view.json"), view("\"work/example/example.md\"")).unwrap();
    let o = check(&w, &co);
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
    for (product, says) in [
        ("5", "must be a string"),
        ("[\"work/a.md\"]", "must be a string"),
        ("\"\"", "is empty"),
        ("\"/etc/hostname\"", "is absolute"),
        ("\"../outside.md\"", "leaves the run"),
        ("\"work/../../outside.md\"", "leaves the run"),
        ("\"work/.cache/token.md\"", "starts with a dot"),
    ] {
        std::fs::write(co.join("views/other.view.json"), view(product)).unwrap();
        assert_finding(&check(&w, &co), "views/other.view.json", &["product", says]);
    }
}

#[test]
fn check_fails_a_duplicate_cycle_name() {
    let w = World::new("check-dup");
    let co = text_copy(&w);
    edit(&co, "cycles/tree-suite.yaml", "- name: finding-agent\n", "- name: stub-cycle\n");
    let o = check(&w, &co);
    assert_finding(&o, "cycles/tree-suite.yaml", &["cycle name stub-cycle appears more than once"]);
}

#[test]
fn check_fails_a_row_whose_state_differs_from_the_suite_file() {
    let w = World::new("check-state");
    let co = text_copy(&w);
    edit(&co, "docs/cycles-suite.md", "| stub-cycle | converted |", "| stub-cycle | pending |");
    let o = check(&w, &co);
    assert_finding(&o, "docs/cycles-suite.md", &["stub-cycle is pending in the table and converted in cycles/tree-suite.yaml"]);
    assert_eq!(findings(&o).len(), 1, "{}", stdout(&o));
    edit(&co, "docs/cycles-suite.md", "| stub-cycle | pending |", "| stub-cycle | held by a Rust test |");
    let o = check(&w, &co);
    assert_finding(&o, "docs/cycles-suite.md", &["stub-cycle is a cycle of cycles/tree-suite.yaml and its row says held by a Rust test"]);
}

#[test]
fn check_fails_a_cycle_the_suite_file_the_list_and_the_table_do_not_share() {
    let w = World::new("check-ids");
    let co = text_copy(&w);
    let names = std::fs::read_to_string(co.join("cycles/old-suite-names.txt")).unwrap();
    std::fs::write(co.join("cycles/old-suite-names.txt"), format!("{names}brand-new-cycle\n")).unwrap();
    edit(&co, "cycles/tree-suite.yaml", "- name: finding-agent\n", "- name: finding-agent-renamed\n");
    let o = check(&w, &co);
    assert_finding(&o, "docs/cycles-suite.md", &["entry brand-new-cycle of cycles/old-suite-names.txt has no row"]);
    assert_finding(&o, "cycles/tree-suite.yaml", &["cycle finding-agent-renamed is not an entry of cycles/old-suite-names.txt"]);
    assert_finding(&o, "docs/cycles-suite.md", &["entry finding-agent is converted in the table and not there in cycles/tree-suite.yaml"]);
    edit(&co, "docs/cycles-suite.md", "| stub-cycle | converted |", "| stub-cycle-twice | converted |");
    assert_finding(&check(&w, &co), "docs/cycles-suite.md", &["entry stub-cycle-twice is not in cycles/old-suite-names.txt"]);
}

#[test]
fn check_fails_a_missing_seed_and_a_bad_suite_header() {
    let w = World::new("check-seed");
    let co = text_copy(&w);
    edit(&co, "cycles/tree-suite.yaml", "from: seeds/idea.yaml", "from: seeds/no-such-seed.yaml");
    assert_finding(&check(&w, &co), "cycles/tree-suite.yaml", &["cycle defaults", "seeds/no-such-seed.yaml", "does not exist"]);
    edit(&co, "cycles/tree-suite.yaml", "format: loom.tree-suite/1", "format: loom.tree-suite/2");
    let o = check(&w, &co);
    assert_finding(&o, "cycles/tree-suite.yaml", &["loom.tree-suite/2"]);
}

#[test]
fn check_skips_the_comparison_with_the_table_when_it_is_gone_and_says_so() {
    let w = World::new("check-no-table");
    let co = text_copy(&w);
    std::fs::remove_file(co.join("docs/cycles-suite.md")).unwrap();
    let o = check(&w, &co);
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
    let notes: Vec<String> = stdout(&o).lines().filter(|l| l.starts_with("note: ")).map(String::from).collect();
    assert_eq!(notes, ["note: docs/cycles-suite.md does not exist: the entries of cycles/old-suite-names.txt are not compared with it"]);
}

#[test]
fn check_fails_when_the_list_of_old_entries_is_gone() {
    let w = World::new("check-no-names");
    let co = text_copy(&w);
    std::fs::remove_file(co.join("cycles/old-suite-names.txt")).unwrap();
    assert_finding(&check(&w, &co), "cycles/old-suite-names.txt", &["missing"]);
}

// ---- runbook parity: synthetic stores of the old driver, written by the test itself

use serde_json::{json, Value};

#[derive(Clone)]
struct Knobs {
    prefix: &'static str,
    rev: &'static str,
    second_rev: &'static str,
    check_result: &'static str,
    swap: bool,
    drop_b_check: bool,
    extra_file: bool,
    extra_path: Option<&'static str>,
    file_a: String,
    answer: &'static str,
}

fn base() -> Knobs {
    Knobs {
        prefix: "R1",
        rev: "tD1",
        second_rev: "tD1",
        check_result: "passed",
        swap: false,
        drop_b_check: false,
        extra_file: false,
        extra_path: None,
        file_a: "hello".into(),
        answer: "approved",
    }
}

struct Synth {
    root: PathBuf,
}

impl Synth {
    fn new(tag: &str) -> Synth {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("parity-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        Synth { root }
    }

    fn put(&self, rel: &str, text: &str) {
        let p = self.root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    fn put_json(&self, rel: &str, v: &Value) {
        self.put(rel, &serde_json::to_string(v).unwrap());
    }

    fn put_events(&self, rel: &str, events: &[Value]) {
        let lines: Vec<String> = events
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let mut e = e.clone();
                e["seq"] = json!(i);
                e["at"] = e.get("at").cloned().unwrap_or(json!("2026-11-20T12:00:00Z"));
                serde_json::to_string(&e).unwrap()
            })
            .collect();
        self.put(rel, &(lines.join("\n") + "\n"));
    }

    /// A finished run of the old driver in two cases: the root `<p>` and its child `<p>.direction`.
    fn driver_run(&self, k: &Knobs) -> (PathBuf, PathBuf) {
        let _ = std::fs::remove_dir_all(self.root.join("store"));
        let (p, c) = (k.prefix, format!("{}.direction", k.prefix));
        let r = format!("store/{p}");
        let d = format!("{r}/children/{c}");
        self.put(
            "run/run-settings.jsonl",
            r#"{"format":"sdc.run-settings/1","at":"2026-11-20T12:00:00Z","decider":{"kind":"person"},"adapter":"stub","stand_in":true,"stand_in_principal":"person:stand-in"}"#,
        );
        // the child: direction
        self.put_json(
            &format!("{d}/case.json"),
            &json!({"id": c, "protocol": "sdc.direction", "revision": "c4", "termination": "passed", "artifacts": {"doc": {"revision": k.rev}}}),
        );
        self.put_json(&format!("{d}/ir.json"), &json!({"actions": {"roadmap.approve": {"requires": [{"capability": "owner.approve"}]}}}));
        self.put_json(&format!("{d}/evaluations/0001.json"), &json!({"obligations": [{"id": "x.required", "status": "discharged"}]}));
        let a = json!({"format": "canon-evidence/1", "id": "e0002-check.run.a_check", "kind": "a_check", "subject": "doc",
            "subject_revision": k.rev, "result": k.check_result, "producer": "p"});
        let b = json!({"format": "canon-evidence/1", "id": "e0002-check.run.b_check", "kind": "b_check", "subject": "doc",
            "subject_revision": k.second_rev, "result": "passed", "upstream_revisions": {"doc": k.rev}});
        let ap = json!({"format": "canon-evidence/1", "id": "e0009-roadmap.approve", "kind": "roadmap_approval", "subject": "roadmap",
            "subject_revision": "tR1", "result": k.answer, "producer": "person:stand-in"});
        self.put_json(&format!("{d}/evidence/e0002-check.run.a_check.json"), &a);
        let mut ids = vec!["e0002-check.run.a_check"];
        if !k.drop_b_check {
            self.put_json(&format!("{d}/evidence/e0002-check.run.b_check.json"), &b);
            ids.push("e0002-check.run.b_check");
        }
        self.put_json(&format!("{d}/evidence/e0009-roadmap.approve.json"), &ap);
        if k.swap {
            ids.reverse();
        }
        self.put_events(
            &format!("{d}/events.jsonl"),
            &[
                json!({"event": "opened", "protocol": {"id": "sdc.direction", "revision": 2}, "ir": "irdigest-direction"}),
                json!({"event": "action_started", "action": "check.run", "argv": ["sh"], "retry_of": []}),
                json!({"event": "action_finished", "action": "check.run", "exit": "1", "evidence": [], "principal": null}),
                json!({"event": "action_started", "action": "check.run", "argv": ["sh"], "retry_of": ["a_check"]}),
                json!({"event": "action_finished", "action": "check.run", "exit": "0", "evidence": ids, "principal": null}),
                json!({"event": "action_started", "action": "roadmap.approve", "argv": ["true"], "retry_of": []}),
                json!({"event": "action_finished", "action": "roadmap.approve", "exit": "0", "evidence": ["e0009-roadmap.approve"],
                    "principal": "person:stand-in", "at": "2026-11-20T12:03:00Z"}),
            ],
        );
        // the root
        self.put_json(
            &format!("{r}/case.json"),
            &json!({"id": p, "protocol": "sdc", "revision": "c2", "termination": "operating", "artifacts": {"idea": {"revision": "tI1"}}}),
        );
        self.put_json(&format!("{r}/ir.json"), &json!({"actions": {"final.approve": {"requires": [{"capability": "release.approve"}]}}}));
        self.put_json(&format!("{r}/evaluations/0001.json"), &json!({"obligations": [{"id": "direction.required", "status": "open"}]}));
        self.put_json(
            &format!("{r}/evidence/e0003-direction.open.direction.json"),
            &json!({"format": "canon-evidence/1", "id": "e0003-direction.open.direction", "kind": "case_outcome", "subject": "idea",
                "subject_revision": "tI1", "child_case": c, "result": "passed"}),
        );
        self.put_json(
            &format!("{r}/evidence/e0003-direction.open.json"),
            &json!({"format": "canon-evidence/1", "id": "e0003-direction.open", "kind": "direction_opened", "subject": "idea", "subject_revision": "tI1"}),
        );
        self.put_json(&format!("{r}/evidence/e0009-roadmap.approve.json"), &ap);
        self.put_json(
            &format!("{r}/evidence/e0012-final.approve.json"),
            &json!({"format": "canon-evidence/1", "id": "e0012-final.approve", "kind": "final_approval", "subject": "idea",
                "subject_revision": "tI1", "result": "approved"}),
        );
        self.put_events(
            &format!("{r}/events.jsonl"),
            &[
                json!({"event": "opened", "protocol": {"id": "sdc", "revision": 4}, "ir": "irdigest-root"}),
                json!({"event": "action_started", "action": "direction.open", "argv": [], "retry_of": []}),
                json!({"event": "child_opened", "child_case": c, "evidence": "e0003-direction.open.direction"}),
                json!({"event": "action_finished", "action": "direction.open", "exit": "0", "evidence": ["e0003-direction.open"], "principal": null}),
                json!({"event": "action_started", "action": "roadmap.read", "copy": {"kind": "roadmap_approval", "from": "sdc.direction"}}),
                json!({"event": "action_finished", "action": "roadmap.read", "exit": "0", "evidence": ["e0009-roadmap.approve"], "principal": null}),
                json!({"event": "action_started", "action": "final.approve", "argv": ["true"], "retry_of": []}),
                json!({"event": "action_finished", "action": "final.approve", "exit": "0", "evidence": ["e0012-final.approve"],
                    "principal": "person:stand-in", "at": "2026-11-20T12:05:00Z"}),
                json!({"event": "authority_recorded", "capability": "publish.approve", "decision": "granted", "principal": "person:stand-in",
                    "at": "2026-11-20T12:06:00Z"}),
                json!({"event": "evaluated"}),
                json!({"event": "action_started", "action": "release.publish", "argv": [], "retry_of": []}),
                json!({"event": "action_finished", "action": "release.publish", "exit": "0", "evidence": [], "principal": null}),
            ],
        );
        // the product
        self.put(&format!("{r}/work/a.txt"), &k.file_a);
        self.put(&format!("{r}/work/sub/b.txt"), "b");
        for excluded in ["target/x.o", "node_modules/y.js", ".git/z", "sub/target/q", ".aep-planning-writer-0f.lock"] {
            self.put(&format!("{r}/work/{excluded}"), "left out");
        }
        if k.extra_file {
            self.put(&format!("{r}/work/extra.txt"), "extra");
        }
        if let Some(p) = k.extra_path {
            self.put(&format!("{r}/work/{p}"), "x");
        }
        (self.root.join("run"), self.root.join("store"))
    }

    fn summary(&self, name: &str, k: &Knobs) -> PathBuf {
        let (run, store) = self.driver_run(k);
        let out = self.root.join(name);
        let o = Command::new(BIN)
            .args(["parity", "summarize", "--driver"])
            .arg(&run)
            .arg("--store")
            .arg(&store)
            .arg("--output")
            .arg(&out)
            .output()
            .unwrap();
        assert_eq!(o.status.code(), Some(0), "{}{}", stdout(&o), String::from_utf8_lossy(&o.stderr));
        out
    }
}

impl Drop for Synth {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn parity(args: &[&str]) -> Output {
    Command::new(BIN).arg("parity").args(args).output().unwrap()
}

fn path_str(p: &Path) -> &str {
    p.to_str().unwrap()
}

fn read_summary(p: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// `compare` of the base against a variant: (exit code, output lines).
fn compare_with(tag: &str, k: Knobs, extra: &[&str]) -> (i32, Vec<String>) {
    let s = Synth::new(tag);
    let a = s.summary("a.json", &base());
    let b = s.summary("b.json", &k);
    let mut args = vec!["compare"];
    args.extend(extra);
    args.extend([path_str(&a), path_str(&b)]);
    let o = parity(&args);
    (o.status.code().unwrap(), stdout(&o).lines().filter(|l| !l.starts_with("parity: ")).map(String::from).collect())
}

#[test]
fn parity_summarize_reads_a_two_case_store() {
    let s = Synth::new("summarize");
    let f = s.summary("a.json", &base());
    let text = std::fs::read_to_string(&f).unwrap();
    assert!(!text.contains(env!("CARGO_TARGET_TMPDIR")), "an absolute path in the summary");
    assert!(!text.contains("2026-11-20"), "a time in the summary");
    let v = read_summary(&f);
    assert_eq!(v["format"], "runbook.parity-summary/1");
    assert_eq!(v["source"]["run_settings"]["stand_in_principal"], "person:stand-in");
    let cases = v["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 2);
    let (root, child) = (&cases[0], &cases[1]);
    assert_eq!((root["id"].as_str(), root["link"].as_str(), root["parent"].as_str()), (Some("R1"), None, None));
    assert_eq!(
        (child["id"].as_str(), child["link"].as_str(), child["parent"].as_str()),
        (Some("R1.direction"), Some("direction"), Some("R1"))
    );
    assert_eq!(
        (root["protocol"].as_str(), root["protocol_revision"].as_i64(), root["outcome"].as_str()),
        (Some("sdc"), Some(4), Some("operating"))
    );
    assert_eq!(root["open_obligations"], json!(["direction.required"]));
    assert_eq!(child["open_obligations"], json!([]));
    assert_eq!(child["stop"], Value::Null);
    let ev = |c: &Value| -> Vec<String> {
        c["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| format!("{} {} {}", e["index"], e["kind"].as_str().unwrap(), e["action"].as_str().unwrap_or("-")))
            .collect()
    };
    assert_eq!(
        ev(root),
        ["0 case_outcome direction.open", "1 direction_opened direction.open", "2 roadmap_approval roadmap.read", "3 final_approval final.approve"]
    );
    assert_eq!(ev(child), ["0 a_check check.run", "1 b_check check.run", "2 roadmap_approval roadmap.approve"]);
    assert_eq!(child["evidence"][1]["upstream_revisions"], json!({"doc": "tD1"}));
    assert_eq!(child["evidence"][0]["result"], "passed");
    assert_eq!(root["evidence"][1]["result"], Value::Null);
    assert_eq!(
        child["program_runs"],
        json!([{"action": "check.run", "attempt": 1, "exit": 1, "retry_followed": true},
               {"action": "check.run", "attempt": 2, "exit": 0, "retry_followed": false}])
    );
    assert_eq!(root["program_runs"], json!([]));
    assert_eq!(
        child["person_steps"],
        json!([{"order": 0, "action": "roadmap.approve", "answer": "approved", "actor": "person:stand-in", "capability": "owner.approve"}])
    );
    assert_eq!(
        root["person_steps"],
        json!([{"order": 1, "action": "final.approve", "answer": "approved", "actor": "person:stand-in", "capability": "release.approve"},
               {"order": 2, "action": null, "answer": "granted", "actor": "person:stand-in", "capability": "publish.approve"}])
    );
    assert_eq!(
        v["copies"],
        json!([{"action": "roadmap.read", "kind": "roadmap_approval", "from_protocol": "sdc.direction", "from": "R1.direction", "to": "R1",
                "subject": "roadmap", "subject_revision": "tR1"}])
    );
    assert_eq!(v["decision_requests"], json!([]));
    assert_eq!(v["not_recorded"], json!(["decision_requests[].question", "decision_requests[].options"]));
    assert_eq!(
        v["protocols"],
        json!([{"id": "sdc", "revision": 4, "digest": "irdigest-root"}, {"id": "sdc.direction", "revision": 2, "digest": "irdigest-direction"}])
    );
    let files: Vec<&str> = v["product_tree"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap()).collect();
    assert_eq!(files, ["a.txt", "sub/b.txt"]);
}

#[test]
fn parity_summarize_prints_to_stdout_without_output_and_refuses_a_missing_store() {
    let s = Synth::new("stdout");
    let (run, store) = s.driver_run(&base());
    let o = parity(&["summarize", "--driver", path_str(&run), "--store", path_str(&store)]);
    assert_eq!(o.status.code(), Some(0));
    let v: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(v["format"], "runbook.parity-summary/1");
    let o = parity(&["summarize", "--driver", path_str(&run), "--store", path_str(&s.root.join("nothing"))]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("nothing"));
}

#[test]
fn parity_summary_digests_are_sha256() {
    let s = Synth::new("sha");
    let mut k = base();
    k.file_a = "abc".into();
    let v = read_summary(&s.summary("a.json", &k));
    assert_eq!(v["product_tree"][0]["sha256"], "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    k.file_a = "a".repeat(1_000_000);
    let v = read_summary(&s.summary("b.json", &k));
    assert_eq!(v["product_tree"][0]["sha256"], "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0");
}

#[test]
fn parity_compare_of_equal_summaries_prints_one_line_and_exits_0() {
    let (code, lines) = compare_with("equal", base(), &["--product-digests"]);
    assert_eq!(code, 0, "{lines:?}");
    assert_eq!(lines, ["equal: 2 cases, 7 evidence records, 2 program runs, 3 person steps, 2 product files"]);
}

#[test]
fn parity_compare_names_generated_values_by_pattern_not_by_value() {
    let mut k = base();
    k.prefix = "Q9";
    k.rev = "tZZ";
    k.second_rev = "tZZ";
    let (code, lines) = compare_with("renamed", k, &["--product-digests"]);
    assert_eq!((code, lines.len()), (0, 1), "{lines:?}");
}

#[test]
fn parity_compare_detects_another_result() {
    let mut k = base();
    k.check_result = "failed";
    let (code, lines) = compare_with("result", k, &[]);
    assert_eq!(code, 1);
    assert_eq!(lines, [r#"cases[1].evidence[0].result: a="passed" b="failed""#]);
}

#[test]
fn parity_compare_detects_another_order() {
    let mut k = base();
    k.swap = true;
    let (code, lines) = compare_with("order", k, &[]);
    assert_eq!(code, 1);
    assert!(lines.contains(&r#"cases[1].evidence[0].kind: a="a_check" b="b_check""#.to_string()), "{lines:?}");
    assert!(lines.contains(&r#"cases[1].evidence[1].kind: a="b_check" b="a_check""#.to_string()), "{lines:?}");
}

#[test]
fn parity_compare_detects_a_missing_record() {
    let mut k = base();
    k.drop_b_check = true;
    let (code, lines) = compare_with("missing", k, &[]);
    assert_eq!(code, 1);
    assert!(lines.iter().any(|l| l.starts_with("cases[1].evidence[2]: a={") && l.ends_with("b=(absent)")), "{lines:?}");
}

#[test]
fn parity_compare_detects_another_revision_pattern() {
    let mut k = base();
    k.second_rev = "tD2";
    let (code, lines) = compare_with("pattern", k, &[]);
    assert_eq!(code, 1);
    assert!(lines.contains(&r#"cases[1].evidence[1].subject_revision: a="r3" b="r4""#.to_string()), "{lines:?}");
}

#[test]
fn parity_compare_detects_another_product_file() {
    let mut k = base();
    k.extra_file = true;
    let (code, lines) = compare_with("file", k, &[]);
    assert_eq!(code, 1);
    assert_eq!(lines, ["product_tree[extra.txt]: a=(absent) b=present"]);
}

#[test]
fn parity_compare_reads_file_digests_only_with_the_flag() {
    let mut k = base();
    k.file_a = "another".into();
    let (code, lines) = compare_with("digest-off", k.clone(), &[]);
    assert_eq!((code, lines.len()), (0, 1), "{lines:?}");
    let (code, lines) = compare_with("digest-on", k, &["--product-digests"]);
    assert_eq!(code, 1);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("product_tree[a.txt].sha256: a="), "{lines:?}");
}

#[test]
fn parity_compare_reads_a_wall_clock_stamp_in_a_product_file_as_no_difference() {
    let s = Synth::new("stamp");
    let mut k = base();
    k.file_a = "moved at: 2026-11-22T01:55:36Z by x".into();
    let a = s.summary("a.json", &k);
    k.file_a = "moved at: 2026-11-22T02:02:10Z by x".into();
    let b = s.summary("b.json", &k);
    let o = parity(&["compare", "--product-digests", path_str(&a), path_str(&b)]);
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
    k.file_a = "moved at: 2026-11-22T02:02:10Z by y".into();
    let c = s.summary("c.json", &k);
    let o = parity(&["compare", "--product-digests", path_str(&a), path_str(&c)]);
    assert_eq!(o.status.code(), Some(1), "{}", stdout(&o));
}

#[test]
fn parity_compare_detects_another_person_answer() {
    let mut k = base();
    k.answer = "rejected";
    let (code, lines) = compare_with("answer", k, &[]);
    assert_eq!(code, 1);
    assert!(lines.contains(&r#"cases[1].person_steps[0].answer: a="approved" b="rejected""#.to_string()), "{lines:?}");
}

#[test]
fn parity_compare_refuses_what_is_no_summary() {
    let s = Synth::new("refuse");
    s.put("x.json", r#"{"format": "other/1"}"#);
    let x = s.root.join("x.json");
    let o = parity(&["compare", path_str(&x), path_str(&x)]);
    assert_eq!(o.status.code(), Some(2));
}

#[test]
fn parity_rules_table_covers_every_member_of_a_summary() {
    let o = parity(&["compare", "--rules"]);
    assert_eq!(o.status.code(), Some(0));
    let table = stdout(&o);
    let rows: Vec<&str> = table.lines().skip(1).filter_map(|l| l.split(" | ").next()).collect();
    assert!(table.contains("source | never compared") && table.contains("product_tree[].sha256 | equal by value only with --product-digests"));
    let s = Synth::new("rules");
    let mut have = std::collections::BTreeSet::new();
    summary_paths("", &read_summary(&s.summary("a.json", &base())), &mut have);
    assert!(have.contains("cases[].evidence[].upstream_revisions.*"));
    for m in &have {
        let covered = rows.contains(&m.as_str()) || ["source.", "not_recorded[]"].iter().any(|p| m.starts_with(p));
        assert!(covered, "member {m} has no row in the rules table");
    }
}

#[test]
fn parity_approvals_lists_the_person_steps_in_order() {
    let s = Synth::new("approvals");
    let a = s.summary("a.json", &base());
    let o = parity(&["approvals", path_str(&a)]);
    assert_eq!(o.status.code(), Some(0));
    let v: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(
        v,
        json!([
            {"path": "direction", "case": "direction", "action": "roadmap.approve", "capability": "owner.approve", "answer": "approved", "actor": "person:stand-in"},
            {"path": ".", "case": null, "action": "final.approve", "capability": "release.approve", "answer": "approved", "actor": "person:stand-in"},
            {"path": ".", "case": null, "action": null, "capability": "publish.approve", "answer": "granted", "actor": "person:stand-in"}
        ])
    );
}

/// `summarize` over the base run after `damage` changed a file of it: (exit code, stderr).
fn summarize_damaged(tag: &str, damage: impl Fn(&Synth)) -> (Option<i32>, String) {
    let s = Synth::new(tag);
    let (run, store) = s.driver_run(&base());
    damage(&s);
    let o = parity(&["summarize", "--driver", path_str(&run), "--store", path_str(&store)]);
    (o.status.code(), String::from_utf8_lossy(&o.stderr).into_owned())
}

const CHILD: &str = "store/R1/children/R1.direction";

#[test]
fn parity_summarize_refuses_a_truncated_event_file() {
    let (code, err) = summarize_damaged("trunc", |s| {
        let p = s.root.join(format!("{CHILD}/events.jsonl"));
        let t = std::fs::read_to_string(&p).unwrap();
        std::fs::write(&p, &t[..t.len() - 40]).unwrap();
    });
    assert_eq!(code, Some(2), "{err}");
    assert!(err.contains("R1.direction/events.jsonl"), "{err}");
}

#[test]
fn parity_summarize_refuses_a_malformed_evidence_record() {
    let (code, err) = summarize_damaged("evidence", |s| s.put(&format!("{CHILD}/evidence/e0002-check.run.b_check.json"), "{\"kind\":"));
    assert_eq!(code, Some(2), "{err}");
    assert!(err.contains("e0002-check.run.b_check.json"), "{err}");
}

#[test]
fn parity_summarize_refuses_a_store_with_a_member_missing() {
    for member in [format!("{CHILD}/events.jsonl"), "store/R1/ir.json".to_string(), format!("{CHILD}/evaluations/0001.json"), format!("{CHILD}/evidence/e0009-roadmap.approve.json")] {
        let (code, err) = summarize_damaged("missing", |s| std::fs::remove_file(s.root.join(&member)).unwrap());
        assert_eq!(code, Some(2), "{member}: {err}");
        let name = member.rsplit('/').next().unwrap();
        let hint = if name == "0001.json" { "evaluations" } else { name };
        assert!(err.contains(hint), "{member}: {err}");
    }
    let (code, err) = summarize_damaged("nobindings", |s| {
        s.put_json(&format!("{CHILD}/case.json"), &json!({"id": "R1.direction", "protocol": "sdc.direction", "revision": "c4"}));
    });
    assert_eq!(code, Some(2), "{err}");
    assert!(err.contains("bindings.json"), "{err}");
}

#[test]
fn parity_summarize_refuses_a_malformed_evaluation() {
    let (code, err) = summarize_damaged("badeval", |s| s.put(&format!("{CHILD}/evaluations/0001.json"), "[1,"));
    assert_eq!(code, Some(2), "{err}");
    assert!(err.contains("0001.json"), "{err}");
}

/// The protocols the fast-lane fixture holds: one without a gate probe and one with. The script
/// starts about 20 processes per protocol, and a test is slow when the pool is loaded because of
/// its process count, so the fixture is two protocols, not the nine of the repository.
const LANE_PROTOCOLS: [&str; 2] = ["sdc", "sdc-epic"];

/// A scratch copy of what `scripts/check-protocols.sh` reads for `LANE_PROTOCOLS` (their sources,
/// their pinned simulate reports and their entries of the committed ledger, byte for byte), and a
/// stub `runbook` that validates and compiles anything: the fast lane takes every result from the
/// ledger and runs no real check.
fn protocol_tree(tag: &str) -> (PathBuf, PathBuf) {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (src, dst) = (repo(), root.clone());
    std::fs::create_dir_all(dst.join("scripts")).unwrap();
    for f in ["check-protocols.sh", "evaluate-cases.sh"] {
        std::fs::copy(src.join("scripts").join(f), dst.join("scripts").join(f)).unwrap();
    }
    std::fs::create_dir_all(dst.join("protocol/simulate")).unwrap();
    std::fs::copy(src.join("protocol/simulate/CANON_REV"), dst.join("protocol/simulate/CANON_REV")).unwrap();
    for n in LANE_PROTOCOLS {
        std::fs::copy(src.join(format!("protocol/{n}.protocol.yaml")), dst.join(format!("protocol/{n}.protocol.yaml"))).unwrap();
        std::fs::copy(src.join(format!("protocol/simulate/{n}.txt")), dst.join(format!("protocol/simulate/{n}.txt"))).unwrap();
    }
    let mut ledger: Value = serde_json::from_str(&std::fs::read_to_string(src.join("protocol/check-ledger.json")).unwrap()).unwrap();
    ledger["protocols"].as_object_mut().unwrap().retain(|k, _| LANE_PROTOCOLS.contains(&k.as_str()));
    assert_eq!(ledger["protocols"].as_object().unwrap().len(), LANE_PROTOCOLS.len());
    std::fs::write(root.join("protocol/check-ledger.json"), serde_json::to_string_pretty(&ledger).unwrap() + "\n").unwrap();
    std::fs::create_dir_all(root.join("runbook")).unwrap();
    std::fs::copy(repo().join("runbook/Cargo.lock"), root.join("runbook/Cargo.lock")).unwrap();
    let stub = root.join("runbook-stub");
    std::fs::write(&stub, "#!/bin/sh\ncase \"$2\" in compile) echo '{}';; esac\n").unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    (root, stub)
}

fn check_protocols(root: &Path, stub: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut c = Command::new("bash");
    c.arg(root.join("scripts/check-protocols.sh")).args(args).env("RUNBOOK_BIN", stub);
    for (k, v) in env {
        c.env(k, v);
    }
    c.output().unwrap()
}

#[test]
fn protocol_fast_lane_passes_on_the_committed_ledger() {
    let (root, stub) = protocol_tree("proto-green");
    let o = check_protocols(&root, &stub, &[], &[]);
    assert_eq!(o.status.code(), Some(0), "{}{}", stdout(&o), String::from_utf8_lossy(&o.stderr));
    assert!(stdout(&o).contains("lane: fast"));
}

#[test]
fn protocol_fast_lane_fails_on_a_hand_edited_ledger_result() {
    let (root, stub) = protocol_tree("proto-edit");
    let ledger = root.join("protocol/check-ledger.json");
    let text = std::fs::read_to_string(&ledger).unwrap();
    let edited = text.replacen("0 findings", "7 findings", 1);
    assert_ne!(text, edited);
    std::fs::write(&ledger, edited).unwrap();
    let o = check_protocols(&root, &stub, &[], &[]);
    assert_eq!(o.status.code(), Some(1), "{}", stdout(&o));
    assert!(stdout(&o).contains("edited by hand"), "{}", stdout(&o));
    let simulate = text.replacen("terminable 54", "terminable 55", 1);
    assert_ne!(text, simulate);
    std::fs::write(&ledger, simulate).unwrap();
    assert_eq!(check_protocols(&root, &stub, &[], &[]).status.code(), Some(1));
}

#[test]
fn protocol_record_and_repin_refuse_simulate_off() {
    let (root, stub) = protocol_tree("proto-off");
    for flag in ["--record", "--repin"] {
        let o = check_protocols(&root, &stub, &[flag], &[("SDC_SIMULATE", "off")]);
        assert_eq!(o.status.code(), Some(2), "{flag}: {}", stdout(&o));
        assert!(stdout(&o).contains("SDC_SIMULATE=off"), "{flag}: {}", stdout(&o));
    }
}

// ---- runbook parity: Loom's side, read from a hand-written `tree status` document
//
// `fixtures/parity/` holds a small tree under made-up ids: `loom-status.json` follows Loom's
// encoder member for member (a parent with two children, one approval with an action, one grant
// without, one retried run, one copied record), `driver-summary.json` is the driver-side summary
// of the same tree, `loom-summary.golden.json` is what the reader must write, and `product/` is
// the product tree. No test starts a process of Loom or of the driver.

/// Every member path of a summary, as the rules table names them.
fn summary_paths(tpl: &str, v: &Value, out: &mut std::collections::BTreeSet<String>) {
    match v {
        Value::Object(m) => m.iter().for_each(|(k, x)| {
            let t = if tpl.is_empty() {
                k.clone()
            } else if tpl.ends_with("upstream_revisions") {
                format!("{tpl}.*")
            } else {
                format!("{tpl}.{k}")
            };
            summary_paths(&t, x, out)
        }),
        Value::Array(l) => l.iter().for_each(|x| summary_paths(&format!("{tpl}[]"), x, out)),
        _ => {
            out.insert(tpl.to_string());
        }
    }
}

fn pfx() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parity")
}

/// Each test of this section ends within 5 s and says how long it took (`--nocapture`).
struct Timed(&'static str, std::time::Instant);

impl Timed {
    fn start(name: &'static str) -> Timed {
        Timed(name, std::time::Instant::now())
    }
}

impl Drop for Timed {
    fn drop(&mut self) {
        let took = self.1.elapsed();
        eprintln!("time {}: {took:?}", self.0);
        if !std::thread::panicking() {
            assert!(took < std::time::Duration::from_secs(5), "{} took {took:?}", self.0);
        }
    }
}

fn pfx_json(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(pfx().join(name)).unwrap()).unwrap()
}

/// The status of the fixture after `edit`, written to a scratch file named by `tag`.
fn loom_status_file(s: &Synth, tag: &str, edit: impl Fn(&mut Value)) -> PathBuf {
    let mut v = pfx_json("loom-status.json");
    edit(&mut v);
    s.put_json(&format!("{tag}.status.json"), &v);
    s.root.join(format!("{tag}.status.json"))
}

fn loom_summarize(status: &Path, extra: &[&str]) -> Output {
    let tree = pfx().join("product");
    let mut args = vec!["summarize", "--loom-status", path_str(status)];
    if !extra.contains(&"--product-tree") {
        args.extend(["--product-tree", path_str(&tree)]);
    }
    args.extend(extra);
    parity(&args)
}

/// The Loom-side summary of the fixture after `edit`, as a file named by `tag`.
fn loom_summary_file(s: &Synth, tag: &str, edit: impl Fn(&mut Value)) -> PathBuf {
    let status = loom_status_file(s, tag, edit);
    let out = s.root.join(format!("{tag}.summary.json"));
    let o = loom_summarize(&status, &["--output", path_str(&out)]);
    assert_eq!(o.status.code(), Some(0), "{}{}", stdout(&o), String::from_utf8_lossy(&o.stderr));
    out
}

/// The case of a status by id.
fn case_mut<'a>(v: &'a mut Value, id: &str) -> &'a mut Value {
    v["cases"].as_array_mut().unwrap().iter_mut().find(|c| c["case"] == id).unwrap()
}

#[test]
fn parity_loom_summary_is_the_golden_file() {
    let _t = Timed::start("parity_loom_summary_is_the_golden_file");
    let s = Synth::new("loom-golden");
    let o = loom_summarize(&pfx().join("loom-status.json"), &[]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let golden = std::fs::read_to_string(pfx().join("loom-summary.golden.json")).unwrap();
    assert_eq!(stdout(&o), golden);
    assert!(!golden.contains("2026-01-01") && !golden.contains("store/"), "a time or a path of the status in the summary");
    let out = s.root.join("o.json");
    let o = loom_summarize(&pfx().join("loom-status.json"), &["--output", path_str(&out)]);
    assert_eq!(stdout(&o).trim(), "summary: 3 cases, 9 evidence records, 5 program runs, 2 person steps, 2 product files");
}

#[test]
fn parity_loom_summary_lists_what_the_status_does_not_record() {
    let _t = Timed::start("parity_loom_summary_lists_what_the_status_does_not_record");
    let v = pfx_json("loom-summary.golden.json");
    // The status alone holds no run settings, no decision and no protocol digest; a copied record's
    // action, subject and revision are the record of that kind in the target case.
    assert_eq!(v["not_recorded"], json!(["source.run_settings", "decision_requests", "protocols[].digest"]));
    assert_eq!(v["source"], json!({"runtime": "loom", "status_version": "0.0.0", "run_settings": null}));
    assert_eq!(v["decision_requests"], json!([]));
    let copy = &v["copies"][0];
    assert_eq!((&copy["action"], &copy["subject"], &copy["subject_revision"]), (&json!("approval.copy"), &json!("route"), &json!("rev-r2")));
    assert_eq!(v["protocols"][0]["digest"], Value::Null);
    let table = stdout(&parity(&["compare", "--rules"]));
    for m in v["not_recorded"].as_array().unwrap() {
        let row = table.lines().find(|l| l.starts_with(&format!("{} | ", m.as_str().unwrap()))).unwrap();
        assert!(row.contains(" | (not recorded) | ") || row.contains(" | run directory: "), "{row}");
    }
}

#[test]
fn parity_a_copy_whose_record_is_not_in_the_target_case_is_an_error_that_names_the_case() {
    let _t = Timed::start("parity_a_copy_whose_record_is_not_in_the_target_case_is_an_error_that_names_the_case");
    let s = Synth::new("loom-copy-missing");
    let status = loom_status_file(&s, "m", |v| {
        case_mut(v, "root")["evidence"].as_array_mut().unwrap().retain(|e| e["kind"] != "route_approval");
    });
    let o = loom_summarize(&status, &[]);
    assert_eq!(o.status.code(), Some(2), "{}", stdout(&o));
    let err = String::from_utf8_lossy(&o.stderr).to_string();
    assert!(err.contains("case root") && err.contains("route_approval") && err.contains("copies[0]"), "{err}");
}

/// A Loom run directory as `runbook run` leaves it, built from the fixtures: the status, the
/// product tree (`work`), the settings line, the decisions it answered and the protocol files it copied.
fn loom_run_dir(s: &Synth, tag: &str) -> PathBuf {
    let dir = s.root.join(tag);
    std::fs::create_dir_all(dir.join("protocols")).unwrap();
    std::fs::copy(pfx().join("loom-status.json"), dir.join("tree-status.json")).unwrap();
    copy_dir(&pfx().join("product"), &dir.join("work"), &[]);
    std::fs::write(
        dir.join("run-settings.jsonl"),
        "{\"format\":\"sdc.run-settings/1\",\"leg\":\"loom\",\"decider\":{\"kind\":\"scripted\",\"principal\":\"person:owner\"},\"max_llm_calls\":100}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("decisions.jsonl"),
        "{\"case\":\"root.first\",\"type\":\"approval\",\"kind\":\"ApproveAction\",\"principal\":\"person:owner\"}\n{\"case\":\"root.second\",\"type\":\"approval\",\"kind\":\"Approve\",\"principal\":\"person:owner\"}\n",
    )
    .unwrap();
    for (id, rev) in [("tour", 3), ("tour.leg", 2)] {
        std::fs::write(dir.join("protocols").join(format!("{}.protocol.yaml", id.replace('.', "-"))), format!("format: protocol/1\nprotocol:\n  id: {id}\n  revision: {rev}\n")).unwrap();
    }
    dir
}

#[test]
fn parity_a_loom_run_directory_records_what_the_status_does_not_and_compares_identical() {
    let _t = Timed::start("parity_a_loom_run_directory_records_what_the_status_does_not_and_compares_identical");
    let s = Synth::new("loom-run-dir");
    let dir = loom_run_dir(&s, "run");
    let out = s.root.join("b.json");
    let o = parity(&["summarize", "--loom-run", path_str(&dir), "--output", path_str(&out)]);
    assert_eq!(o.status.code(), Some(0), "{}{}", stdout(&o), String::from_utf8_lossy(&o.stderr));
    let v = read_summary(&out);
    assert_eq!(v["not_recorded"], json!([]));
    assert_eq!(v["source"]["run_settings"], json!({"decider": "scripted", "adapter": null, "stand_in": true, "stand_in_principal": "person:owner"}));
    assert_eq!(v["decision_requests"], json!([]));
    let digest = |name: &str| -> String { use sha2::{Digest, Sha256}; Sha256::digest(std::fs::read(dir.join("protocols").join(name)).unwrap()).iter().map(|b| format!("{b:02x}")).collect() };
    assert_eq!(v["protocols"][0]["digest"], json!(digest("tour.protocol.yaml")));
    assert_eq!(v["protocols"][1]["digest"], json!(digest("tour-leg.protocol.yaml")));
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(!text.contains(env!("CARGO_TARGET_TMPDIR")), "an absolute path in the summary");
    // The driver's fixture summary and this one: nothing differs and nothing is unrecorded.
    for (x, y) in [(pfx().join("driver-summary.json"), out.clone()), (out.clone(), pfx().join("driver-summary.json"))] {
        let o = parity(&["compare", "--product-digests", path_str(&x), path_str(&y)]);
        assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
        let lines: Vec<String> = stdout(&o).lines().map(String::from).collect();
        assert_eq!(lines, ["equal: 3 cases, 9 evidence records, 5 program runs, 2 person steps, 2 product files", "parity: identical"]);
    }
}

#[test]
fn parity_a_loom_run_directory_that_misses_a_file_or_holds_a_decision_with_no_rule_is_refused_by_name() {
    let _t = Timed::start("parity_a_loom_run_directory_that_misses_a_file_or_holds_a_decision_with_no_rule_is_refused_by_name");
    let s = Synth::new("loom-run-dir-bad");
    let refused = |tag: &str, edit: &dyn Fn(&Path), needle: &str| {
        let dir = loom_run_dir(&s, tag);
        edit(&dir);
        let o = parity(&["summarize", "--loom-run", path_str(&dir)]);
        assert_eq!(o.status.code(), Some(2), "{tag}: {}", stdout(&o));
        let err = String::from_utf8_lossy(&o.stderr).to_string();
        assert!(err.contains(needle), "{tag}: {needle} not named: {err}");
    };
    refused("no-settings", &|d| std::fs::remove_file(d.join("run-settings.jsonl")).unwrap(), "run-settings.jsonl");
    refused("no-status", &|d| std::fs::remove_file(d.join("tree-status.json")).unwrap(), "tree-status.json");
    refused("no-protocol", &|d| std::fs::remove_file(d.join("protocols/tour-leg.protocol.yaml")).unwrap(), "tour-leg");
    refused(
        "question",
        &|d| std::fs::write(d.join("decisions.jsonl"), "{\"case\":\"root\",\"type\":\"approval\",\"kind\":\"Answer\"}\n").unwrap(),
        "decisions.jsonl:1",
    );
    // `--loom-run` is a source of its own.
    let dir = loom_run_dir(&s, "usage");
    for other in [["--loom-status", "x"], ["--product-tree", "x"], ["--driver", "x"]] {
        let mut args = vec!["summarize", "--loom-run", path_str(&dir)];
        args.extend(other);
        assert_eq!(parity(&args).status.code(), Some(64), "{other:?}");
    }
}

#[test]
fn parity_a_loom_run_directory_with_the_deciders_answers_to_a_question_and_a_finding_is_read() {
    let _t = Timed::start("parity_a_loom_run_directory_with_the_deciders_answers_to_a_question_and_a_finding_is_read");
    let s = Synth::new("loom-run-dir-answers");
    let dir = loom_run_dir(&s, "run");
    std::fs::write(
        dir.join("decisions.jsonl"),
        "{\"case\":\"root\",\"type\":\"question\",\"class\":\"scope_changing_answer\",\"principal\":\"decider:model\"}\n{\"case\":\"root\",\"type\":\"finding\",\"class\":\"finding\",\"principal\":\"decider:model\"}\n",
    )
    .unwrap();
    let o = parity(&["summarize", "--loom-run", path_str(&dir)]);
    assert_eq!(o.status.code(), Some(0), "{}{}", stdout(&o), String::from_utf8_lossy(&o.stderr));
}

#[test]
fn parity_compare_ends_with_a_line_that_says_how_the_two_summaries_stand() {
    let _t = Timed::start("parity_compare_ends_with_a_line_that_says_how_the_two_summaries_stand");
    let s = Synth::new("closing-line");
    let b = loom_summary_file(&s, "b", |_| {});
    let a = pfx().join("driver-summary.json");
    let o = parity(&["compare", path_str(&a), path_str(&b)]);
    assert_eq!((o.status.code(), stdout(&o).lines().last()), (Some(0), Some("parity: equal on what both sides record")));
    let o = parity(&["compare", path_str(&a), path_str(&a)]);
    assert_eq!((o.status.code(), stdout(&o).lines().last()), (Some(0), Some("parity: identical")));
    let b = loom_summary_file(&s, "c", |v| case_mut(v, "root.second")["outcome"] = json!("failed"));
    let o = parity(&["compare", path_str(&a), path_str(&b)]);
    assert_eq!((o.status.code(), stdout(&o).lines().last()), (Some(1), Some("parity: different")));
}

#[test]
fn parity_loom_summary_maps_the_words_and_counts_the_attempts() {
    let _t = Timed::start("parity_loom_summary_maps_the_words_and_counts_the_attempts");
    let v = pfx_json("loom-summary.golden.json");
    let first = &v["cases"][1];
    assert_eq!((first["id"].as_str(), first["link"].as_str(), first["parent"].as_str()), (Some("root.first"), Some("first"), Some("root")));
    assert_eq!(first["protocol"], "tour-leg");
    assert_eq!(first["protocol_revision"], 2);
    // Loom's attempt of the second check is 1 (a run that retries nothing); the summary counts the runs of the action
    let runs: Vec<(String, i64, i64, bool)> = first["program_runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["action"].as_str().unwrap().to_string(), r["attempt"].as_i64().unwrap(), r["exit"].as_i64().unwrap(), r["retry_followed"].as_bool().unwrap()))
        .collect();
    assert_eq!(
        runs,
        [("route.write".into(), 1, 0, true), ("route.check".into(), 1, 1, false), ("route.write".into(), 2, 0, false), ("route.check".into(), 2, 0, false)]
    );
    // Allow with an action is the result of the action's record; Allow without one is granted
    assert_eq!(first["person_steps"][0]["answer"], "approved");
    assert_eq!(v["cases"][2]["person_steps"][0]["answer"], "granted");
    assert_eq!(v["cases"][2]["person_steps"][0]["action"], Value::Null);
    // a case_outcome record takes the action of the step that opened the child
    assert_eq!(v["cases"][0]["evidence"][0]["action"], "first.open");
    assert_eq!(v["cases"][0]["evidence"][3]["action"], "second.open");
}

#[test]
fn parity_loom_and_driver_summaries_of_one_tree_compare_equal() {
    let _t = Timed::start("parity_loom_and_driver_summaries_of_one_tree_compare_equal");
    let s = Synth::new("loom-equal");
    let b = loom_summary_file(&s, "b", |_| {});
    let a = pfx().join("driver-summary.json");
    for (x, y, by) in [(&a, &b, "b"), (&b, &a, "a")] {
        let o = parity(&["compare", "--product-digests", path_str(x), path_str(y)]);
        assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
        let lines: Vec<String> = stdout(&o).lines().map(String::from).collect();
        assert_eq!(lines[0], "equal on what both sides record: 3 cases, 9 evidence records, 5 program runs, 2 person steps, 2 product files");
        // the driver lists the question and the options of a decision request, Loom the whole list
        let mut expect: Vec<String> = ["source.run_settings", "decision_requests", "protocols[].digest"]
            .iter()
            .map(|m| format!("not recorded on one side: {m} (not recorded by {by})"))
            .collect();
        expect.push("parity: equal on what both sides record".to_owned());
        assert_eq!(lines[1..], expect[..]);
    }
}

#[test]
fn parity_a_member_one_side_does_not_record_is_no_difference_of_values() {
    let _t = Timed::start("parity_a_member_one_side_does_not_record_is_no_difference_of_values");
    let s = Synth::new("loom-notrec");
    let b = loom_summary_file(&s, "b", |_| {});
    let mut d = pfx_json("driver-summary.json");
    d["source"]["run_settings"] = json!({"decider": "another"});
    d["decision_requests"] = json!([{"id": "q1", "case": "T-1", "question": null, "options": [], "answer": "yes", "who": "person:stand-in"}]);
    d["protocols"][0]["digest"] = json!("another");
    s.put_json("d.json", &d);
    let o = parity(&["compare", path_str(&s.root.join("d.json")), path_str(&b)]);
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
    assert!(stdout(&o).contains("not recorded on one side: protocols[].digest (not recorded by b)"), "{}", stdout(&o));
    // what both record is still compared: a copy's subject is recorded on both sides now
    d["copies"][0]["subject"] = json!("another");
    s.put_json("d.json", &d);
    let o = parity(&["compare", path_str(&s.root.join("d.json")), path_str(&b)]);
    assert_eq!(o.status.code(), Some(1));
    assert!(stdout(&o).lines().any(|l| l == r#"copies[0].subject: a="another" b="route""#), "{}", stdout(&o));
    d["copies"][0]["subject"] = json!("route");
    d["copies"][0]["kind"] = json!("another_kind");
    s.put_json("d.json", &d);
    let o = parity(&["compare", path_str(&s.root.join("d.json")), path_str(&b)]);
    assert_eq!(o.status.code(), Some(1));
    assert!(stdout(&o).lines().any(|l| l == r#"copies[0].kind: a="another_kind" b="route_approval""#), "{}", stdout(&o));
}

/// Exit code and the lines of `compare` of the driver fixture against the Loom fixture after `edit`,
/// without the notes of members not recorded.
fn compare_loom_variant(tag: &str, edit: impl Fn(&mut Value)) -> (i32, Vec<String>) {
    let s = Synth::new(tag);
    let b = loom_summary_file(&s, "b", edit);
    let o = parity(&["compare", "--product-digests", path_str(&pfx().join("driver-summary.json")), path_str(&b)]);
    let lines = stdout(&o).lines().filter(|l| !l.starts_with("not recorded on one side") && !l.starts_with("parity: ")).map(String::from).collect();
    (o.status.code().unwrap(), lines)
}

#[test]
fn parity_loom_red_a_record_missing_on_the_loom_side() {
    let _t = Timed::start("parity_loom_red_a_record_missing_on_the_loom_side");
    let (code, lines) = compare_loom_variant("loom-red-record", |v| {
        case_mut(v, "root.second")["evidence"].as_array_mut().unwrap().pop();
    });
    assert_eq!(code, 1, "{lines:?}");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].starts_with("cases[2].evidence[0]: a={") && lines[0].ends_with("b=(absent)"), "{lines:?}");
    assert!(lines[0].contains("ticket_issued"), "{lines:?}");
}

#[test]
fn parity_loom_red_another_outcome_of_one_child() {
    let _t = Timed::start("parity_loom_red_another_outcome_of_one_child");
    let (code, lines) = compare_loom_variant("loom-red-outcome", |v| case_mut(v, "root.second")["outcome"] = json!("failed"));
    assert_eq!(code, 1, "{lines:?}");
    assert_eq!(lines, [r#"cases[2].outcome: a="passed" b="failed""#]);
}

#[test]
fn parity_loom_red_a_denial_against_the_drivers_approval() {
    let _t = Timed::start("parity_loom_red_a_denial_against_the_drivers_approval");
    let (code, lines) = compare_loom_variant("loom-red-deny", |v| {
        let a = &mut case_mut(v, "root.first")["approvals"][0];
        a["answer"] = json!("Deny");
        a.as_object_mut().unwrap().remove("action");
    });
    assert_eq!(code, 1, "{lines:?}");
    assert!(lines.contains(&r#"cases[1].person_steps[0].answer: a="approved" b="denied""#.to_string()), "{lines:?}");
    assert!(lines.contains(&r#"cases[1].person_steps[0].action: a="route.approve" b=null"#.to_string()), "{lines:?}");
}

#[test]
fn parity_loom_stops_are_read_in_the_words_of_the_summary() {
    let _t = Timed::start("parity_loom_stops_are_read_in_the_words_of_the_summary");
    let s = Synth::new("loom-stop");
    for (stop, cause) in [("AwaitingPerson", "awaiting_person"), ("AwaitingAuthority", "awaiting_authority"), ("Stuck", "stopped"), ("BudgetSpent", "budget_spent")] {
        let f = loom_summary_file(&s, "w", |v| {
            let c = case_mut(v, "root.second");
            c.as_object_mut().unwrap().remove("outcome");
            c["stop"] = json!(stop);
        });
        let v: Value = read_summary(&f);
        assert_eq!(v["cases"][2]["outcome"], Value::Null);
        assert_eq!(v["cases"][2]["stop"], json!({"cause": cause}), "{stop}");
    }
    // the last stop of a case that went on is old: an outcome wins; no stop on a running case is null
    let f = loom_summary_file(&s, "old", |v| case_mut(v, "root.second")["stop"] = json!("AwaitingPerson"));
    assert_eq!(read_summary(&f)["cases"][2]["stop"], Value::Null);
    let f = loom_summary_file(&s, "run", |v| {
        case_mut(v, "root.second").as_object_mut().unwrap().remove("outcome");
    });
    let v = read_summary(&f);
    assert_eq!((&v["cases"][2]["outcome"], &v["cases"][2]["stop"]), (&Value::Null, &Value::Null));
}

#[test]
fn parity_loom_orders_cases_below_their_parent_and_person_steps_by_instant() {
    let _t = Timed::start("parity_loom_orders_cases_below_their_parent_and_person_steps_by_instant");
    let s = Synth::new("loom-order");
    let f = loom_summary_file(&s, "o", |v| {
        let mut deep = case_mut(v, "root.second").clone();
        deep["case"] = json!("root.first.deep");
        deep["parent"] = json!("root.first");
        deep["name"] = json!("deep");
        deep["approvals"] = json!([]);
        v["cases"].as_array_mut().unwrap().push(deep);
        case_mut(v, "root.first")["approvals"][0]["at"] = json!("2026-01-01T00:00:09Z");
    });
    let v = read_summary(&f);
    let ids: Vec<&str> = v["cases"].as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["root", "root.first", "root.first.deep", "root.second"]);
    assert_eq!(v["cases"][1]["person_steps"][0]["order"], 1);
    assert_eq!(v["cases"][3]["person_steps"][0]["order"], 0);
}

#[test]
fn parity_loom_reader_refuses_a_status_it_cannot_read_by_name() {
    let _t = Timed::start("parity_loom_reader_refuses_a_status_it_cannot_read_by_name");
    let s = Synth::new("loom-refuse");
    let refuse = |tag: &str, edit: &dyn Fn(&mut Value), words: &[&str]| {
        let status = loom_status_file(&s, tag, edit);
        let o = loom_summarize(&status, &[]);
        let err = String::from_utf8_lossy(&o.stderr).into_owned();
        assert_eq!(o.status.code(), Some(2), "{tag}: {}{err}", stdout(&o));
        assert!(stdout(&o).is_empty(), "{tag}: a summary was written: {}", stdout(&o));
        for w in words {
            assert!(err.contains(w), "{tag}: {err} lacks {w}");
        }
    };
    let remove = |v: &mut Value, name: &str| drop(v.as_object_mut().unwrap().remove(name));
    refuse("fmt", &|v| v["format"] = json!("loom.tree-status/2"), &["loom.tree-status/2", "loom.tree-status/1"]);
    refuse("nofmt", &|v| remove(v, "format"), &["no format"]);
    refuse("name", &|v| remove(case_mut(v, "root.first"), "name"), &["case root.first", "name"]);
    refuse("parent", &|v| remove(case_mut(v, "root.second"), "parent"), &["case root.second", "parent"]);
    refuse("lost", &|v| case_mut(v, "root.second")["parent"] = json!("root.nowhere"), &["case root.second", "parent", "root.nowhere"]);
    refuse("protocol_id", &|v| remove(case_mut(v, "root.second"), "protocol_id"), &["case root.second", "protocol_id"]);
    refuse("revision", &|v| case_mut(v, "root.second")["protocol_revision"] = json!("two"), &["case root.second", "protocol_revision", "two"]);
    refuse("kind", &|v| remove(&mut case_mut(v, "root.first")["evidence"][1], "kind"), &["case root.first", "evidence[1]", "kind"]);
    refuse("exit", &|v| remove(&mut case_mut(v, "root.first")["runs"][0], "exit"), &["case root.first", "runs[0]", "exit"]);
    refuse("capability", &|v| remove(&mut case_mut(v, "root.first")["approvals"][0], "capability"), &["case root.first", "approvals[0]", "capability"]);
    refuse("answer", &|v| case_mut(v, "root.first")["approvals"][0]["answer"] = json!("Maybe"), &["case root.first", "approvals[0]", "answer", "Maybe"]);
    refuse(
        "stop",
        &|v| {
            let c = case_mut(v, "root.second");
            c.as_object_mut().unwrap().remove("outcome");
            c["stop"] = json!("Elsewhere");
        },
        &["case root.second", "stop", "Elsewhere"],
    );
    refuse("noresult", &|v| case_mut(v, "root.first")["evidence"].as_array_mut().unwrap().truncate(2), &["case root.first", "approvals[0]", "route.approve"]);
    refuse("copy", &|v| case_mut(v, "root")["copies"][0]["source_case"] = json!("root.third"), &["case root", "copies", "root.third"]);
    refuse("outcome-action", &|v| case_mut(v, "root")["evidence"].as_array_mut().unwrap().truncate(4), &["case root", "evidence[3]", "action"]);
    refuse("cases", &|v| remove(v, "cases"), &["cases"]);
    // a product tree that is not there
    let o = loom_summarize(&pfx().join("loom-status.json"), &["--product-tree", path_str(&s.root.join("nothing"))]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("nothing"));
    // the status's own workspace is the tree when none is given, and it is not there
    let o = parity(&["summarize", "--loom-status", path_str(&pfx().join("loom-status.json"))]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("workspace"), "{}", String::from_utf8_lossy(&o.stderr));
    // a file that is no JSON
    s.put("bad.json", "{\"format\":");
    let o = loom_summarize(&s.root.join("bad.json"), &[]);
    assert_eq!(o.status.code(), Some(2));
}

#[test]
fn parity_summarize_takes_one_source() {
    let _t = Timed::start("parity_summarize_takes_one_source");
    let st = pfx().join("loom-status.json");
    let o = parity(&["summarize", "--loom-status", path_str(&st), "--driver", "x", "--store", "y"]);
    assert_eq!(o.status.code(), Some(64));
    let o = parity(&["summarize"]);
    assert_eq!(o.status.code(), Some(64));
    let o = parity(&["summarize", "--product-tree", "x"]);
    assert_eq!(o.status.code(), Some(64));
}

#[test]
fn parity_rules_table_and_the_docs_table_say_the_same() {
    let _t = Timed::start("parity_rules_table_and_the_docs_table_say_the_same");
    let table = stdout(&parity(&["compare", "--rules"]));
    let docs = std::fs::read_to_string(repo().join("docs/parity.md")).unwrap();
    let mut rows = 0;
    for l in table.lines().skip(1).filter(|l| l.matches(" | ").count() == 4) {
        let c: Vec<&str> = l.split(" | ").collect();
        let line = docs.lines().find(|d| d.starts_with(&format!("| `{}` | ", c[0]))).unwrap_or_else(|| panic!("no row of {} in docs/parity.md", c[0]));
        assert!(line.ends_with(&format!(" | {} | {} |", c[3], c[4])), "the docs row of {} differs from the table: {line}", c[0]);
        rows += 1;
    }
    assert!(rows > 40, "{rows} rows");
    // every member of both summaries has a row
    let mut have = std::collections::BTreeSet::new();
    summary_paths("", &pfx_json("loom-summary.golden.json"), &mut have);
    summary_paths("", &pfx_json("driver-summary.json"), &mut have);
    let rows: Vec<&str> = table.lines().skip(1).filter_map(|l| l.split(" | ").next()).collect();
    for m in &have {
        let covered = rows.contains(&m.as_str()) || ["source.", "not_recorded[]"].iter().any(|p| m.starts_with(p));
        assert!(covered, "member {m} has no row in the rules table");
    }
}

// ---- runbook parity: the registration stem of a protocol id, and the feedback copies of Loom

/// A one-case summary whose protocol id is spelled `id` in the case, the copy and the protocol list.
fn mini_summary(id: &str) -> Value {
    json!({
        "format": "runbook.parity-summary/1",
        "source": {"runtime": "driver", "run_settings": null},
        "not_recorded": [],
        "cases": [{"id": "c", "link": null, "parent": null, "protocol": id, "protocol_revision": 1, "outcome": null,
            "stop": null, "open_obligations": [], "evidence": [], "program_runs": [], "person_steps": []}],
        "copies": [{"action": "a", "kind": "k", "from_protocol": id, "from": "c", "to": "c", "subject": "s", "subject_revision": "r"}],
        "decision_requests": [],
        "protocols": [{"id": id, "revision": 1, "digest": "d"}],
        "product_tree": [],
    })
}

fn compare_mini(tag: &str, a: &str, b: &str) -> (i32, Vec<String>) {
    let s = Synth::new(tag);
    s.put_json("a.json", &mini_summary(a));
    s.put_json("b.json", &mini_summary(b));
    let o = parity(&["compare", path_str(&s.root.join("a.json")), path_str(&s.root.join("b.json"))]);
    (o.status.code().unwrap(), stdout(&o).lines().filter(|l| !l.starts_with("parity: ")).map(String::from).collect())
}

#[test]
fn parity_compare_reads_a_protocol_id_by_its_registration_stem() {
    let _t = Timed::start("parity_compare_reads_a_protocol_id_by_its_registration_stem");
    for (a, b) in [("sdc.intake", "sdc-intake"), ("sdc-intake", "sdc_intake"), ("sdc.intake", "sdc.intake")] {
        let (code, lines) = compare_mini("stem-equal", a, b);
        assert_eq!((code, lines.len()), (0, 1), "{a} {b}: {lines:?}");
    }
    let table = stdout(&parity(&["compare", "--rules"]));
    for m in ["cases[].protocol", "copies[].from_protocol", "protocols[].id"] {
        let row = table.lines().find(|l| l.starts_with(&format!("{m} | "))).unwrap();
        assert!(row.contains("registration stem"), "{row}");
    }
}

#[test]
fn parity_compare_still_tells_two_protocol_ids_apart() {
    let _t = Timed::start("parity_compare_still_tells_two_protocol_ids_apart");
    let (code, lines) = compare_mini("stem-differ", "sdc.intake", "sdc.plan");
    assert_eq!(code, 1, "{lines:?}");
    assert_eq!(
        lines,
        [
            r#"cases[0].protocol: a="sdc.intake" b="sdc.plan""#,
            r#"copies[0].from_protocol: a="sdc.intake" b="sdc.plan""#,
            r#"protocols[0].id: a="sdc.intake" b="sdc.plan""#,
        ]
    );
}

#[test]
fn parity_summary_leaves_out_the_feedback_copies_of_loom() {
    let _t = Timed::start("parity_summary_leaves_out_the_feedback_copies_of_loom");
    let mut k = base();
    k.extra_path = Some(".sdc/loom-feedback/0123456789abcdef/feedback.json");
    let (code, lines) = compare_with("feedback-equal", k, &["--product-digests"]);
    assert_eq!(code, 0, "{lines:?}");
    assert_eq!(lines, ["equal: 2 cases, 7 evidence records, 2 program runs, 3 person steps, 2 product files"]);
    let mut k = base();
    k.extra_path = Some(".sdc/other/feedback.json");
    let (code, lines) = compare_with("feedback-other", k, &[]);
    assert_eq!(code, 1, "{lines:?}");
    assert_eq!(lines, ["product_tree[.sdc/other/feedback.json]: a=(absent) b=present"]);
}

#[test]
fn parity_loom_product_tree_leaves_out_the_feedback_copies_too() {
    let _t = Timed::start("parity_loom_product_tree_leaves_out_the_feedback_copies_too");
    let s = Synth::new("loom-feedback");
    s.put("tree/a.txt", "a");
    s.put("tree/.sdc/loom-feedback/0123456789abcdef/log.txt", "tail");
    s.put("tree/.sdc/keep/x", "x");
    s.put("tree/.sdc/records/conformance_run/0123456789abcdef.log", "test b ... ok\nfinished in 0.02s\n");
    let out = s.root.join("o.json");
    let o = loom_summarize(&pfx().join("loom-status.json"), &["--product-tree", path_str(&s.root.join("tree")), "--output", path_str(&out)]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let paths: Vec<Value> = read_summary(&out)["product_tree"].as_array().unwrap().iter().map(|e| e["path"].clone()).collect();
    assert_eq!(paths, [json!(".sdc/keep/x"), json!("a.txt")]);
    let table = stdout(&parity(&["compare", "--rules"]));
    assert!(table.lines().any(|l| l.starts_with("product_tree[.sdc/loom-feedback/**] | never compared | ")), "{table}");
    assert!(table.lines().any(|l| l.starts_with("product_tree[.sdc/records/**] | never compared | ")), "{table}");
}

// ---- runbook parity: the revision a child's `case_outcome` record carries in its parent

/// A one-case summary of three records of the subject `doc`: a check, the `case_outcome` record of
/// a child opened by `child.open`, and a second check. `outcome_rev` is the revision the outcome
/// record carries, `outcome_up` its upstream revision, `check_rev` the revision of the first check.
fn outcome_summary(check_rev: &str, outcome_rev: &str, outcome_up: &str) -> Value {
    let mut v = mini_summary("p");
    v["copies"] = json!([]);
    let rec = |i: usize, kind: &str, action: &str, rev: &str, up: Value| {
        json!({"index": i, "kind": kind, "result": null, "subject": "doc", "subject_revision": rev,
            "upstream_revisions": up, "action": action})
    };
    v["cases"][0]["evidence"] = json!([
        rec(0, "a_check", "check.run", check_rev, json!({})),
        rec(1, "case_outcome", "child.open", outcome_rev, json!({"doc": outcome_up})),
        rec(2, "b_check", "check.run", "tNEW", json!({})),
    ]);
    v
}

fn compare_summaries(tag: &str, a: &Value, b: &Value) -> (i32, Vec<String>) {
    let s = Synth::new(tag);
    s.put_json("a.json", a);
    s.put_json("b.json", b);
    let o = parity(&["compare", path_str(&s.root.join("a.json")), path_str(&s.root.join("b.json"))]);
    (o.status.code().unwrap(), stdout(&o).lines().filter(|l| !l.starts_with("parity: ")).map(String::from).collect())
}

#[test]
fn parity_compare_leaves_the_revision_of_a_case_outcome_record_out() {
    let _t = Timed::start("parity_compare_leaves_the_revision_of_a_case_outcome_record_out");
    // the driver keeps the revision of the opening, Loom the one the child ended with, which a
    // later record of the parent also holds: counted, the labels of both records would differ
    let a = outcome_summary("tOLD", "tOLD2", "u1");
    let b = outcome_summary("tOLD", "tNEW", "u2");
    let (code, lines) = compare_summaries("outcome-equal", &a, &b);
    assert_eq!((code, lines.len()), (0, 1), "{lines:?}");
    let table = stdout(&parity(&["compare", "--rules"]));
    for m in ["cases[].evidence[case_outcome].subject_revision", "cases[].evidence[case_outcome].upstream_revisions.*"] {
        let row = table.lines().find(|l| l.starts_with(&format!("{m} | never compared | "))).unwrap_or_else(|| panic!("no row {m}"));
        assert!(row.contains("opening") && row.contains("termination"), "{row}");
    }
}

#[test]
fn parity_compare_still_tells_the_revision_of_another_record_apart() {
    let _t = Timed::start("parity_compare_still_tells_the_revision_of_another_record_apart");
    // the first check holds the revision of the second on one side only: the pattern differs
    let a = outcome_summary("tOLD", "tOLD2", "u1");
    let b = outcome_summary("tNEW", "tOLD2", "u1");
    let (code, lines) = compare_summaries("outcome-other", &a, &b);
    assert_eq!(code, 1, "{lines:?}");
    assert_eq!(lines, [r#"cases[0].evidence[2].subject_revision: a="r2" b="r1""#]);
}

/// `docs/cycles-suite.md` says every entry is in one of four states, each in exactly one spelling: a
/// row with any other word, a free-text variant included, fails the check and the finding names it.
#[test]
fn check_fails_a_row_whose_state_is_none_of_the_four() {
    let w = World::new("check-row-state-word");
    let co = text_copy(&w);
    edit(&co, "docs/cycles-suite.md", "| pool-wait | dropped with the driver |", "| pool-wait | banana |");
    let o = check(&w, &co);
    assert_finding(&o, "docs/cycles-suite.md", &["pool-wait", "banana"]);
    edit(&co, "docs/cycles-suite.md", "| pool-wait | banana |", "| pool-wait | held in part by a Rust test |");
    let o = check(&w, &co);
    assert_finding(&o, "docs/cycles-suite.md", &["pool-wait", "held in part by a Rust test"]);
    edit(&co, "docs/cycles-suite.md", "| pool-wait | held in part by a Rust test |", "| pool-wait | dropped with the driver |");
    assert_eq!(check(&w, &co).status.code(), Some(0));
}

/// The near misses of a state word. Each is a finding, none passes.
#[test]
fn check_fails_every_near_miss_of_a_state_word() {
    let w = World::new("adv-check-state-near-miss");
    let co = text_copy(&w);
    let mut cur = "| pool-wait | dropped with the driver |".to_string();
    for bad in [
        "Dropped with the driver",
        "DROPPED WITH THE DRIVER",
        "dropped with the driver.",
        "dropped with the driver (all of it)",
        "**dropped with the driver**",
        "`dropped with the driver`",
        "dropped  with the driver",
        "dropped\u{a0}with the driver",
        "",
        "dropped with the driver, held by a Rust test",
    ] {
        let next = format!("| pool-wait | {bad} |");
        edit(&co, "docs/cycles-suite.md", &cur, &next);
        let o = check(&w, &co);
        assert_eq!(o.status.code(), Some(1), "state {bad:?} passed:\n{}", stdout(&o));
        cur = next;
    }
}

/// A row cut to one cell has no state, so the entry has no row.
#[test]
fn check_fails_a_row_cut_to_one_cell() {
    let w = World::new("adv-check-state-one-cell");
    let co = text_copy(&w);
    let p = co.join("docs/cycles-suite.md");
    let t = std::fs::read_to_string(&p).unwrap();
    let cut: Vec<String> = t.lines().map(|l| if l.starts_with("| pool-wait |") { "| pool-wait |".to_string() } else { l.to_string() }).collect();
    std::fs::write(&p, cut.join("\n") + "\n").unwrap();
    let o = check(&w, &co);
    assert_finding(&o, "docs/cycles-suite.md", &["pool-wait", "no row"]);
}
