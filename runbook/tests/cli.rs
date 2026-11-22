use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
        let canon = format!("canon {}", canon_pin()["version"].as_str().unwrap());
        let out = match name {
            "yq" => "yq version v4.30.8",
            "jq" => "jq-1.7",
            "canon" => canon.as_str(),
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
const TOOLS: [&str; 4] = [
    "bindings/tools/sdc-tool.sh",
    "bindings/tools/llm-node.sh",
    "bindings/tools/decider.sh",
    "bindings/tools/llm-adapter-claude-cli.sh",
];

const PROGRAMS: [&str; 14] =
    ["canon", "bash", "jq", "yq", "git", "claude", "aep", "ess", "task", "slot", "cargo", "cargo-clippy", "rustfmt", "node"];
const HELPERS: [&str; 9] = [
    "sdc-drive",
    "sdc-findings",
    "sdc-bugfix",
    "sdc-decision-request",
    "sdc-reply",
    "sdc-replay",
    "sdc-check-routes",
    "sdc-unstick",
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
    for want in ["canon", "claude-login", "ess", "task", "slot", "clippy", "rustfmt", "node"] {
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
    w.fakes(&["canon", "bash", "jq", "yq", "git", "claude", "aep", "cargo", "cargo-clippy", "rustfmt"]);
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
    serde_json::from_str(&std::fs::read_to_string(repo().join("driver/canon-pin.json")).unwrap()).unwrap()
}

fn fake_canon_at(p: &Path, version: &str) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, format!("#!/bin/sh\necho 'canon {version}'\n")).unwrap();
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn canon_path(o: &Output) -> String {
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    let c = v["checks"].as_array().unwrap().iter().find(|c| c["name"] == "canon").unwrap().clone();
    c["path"].as_str().unwrap_or_default().to_string()
}

#[test]
fn doctor_finds_the_cached_pinned_canon_and_ignores_a_sibling_checkout() {
    let w = World::new("doctor-canon-cache");
    w.fakes(&PROGRAMS[1..]);
    w.fakes(&HELPERS);
    w.login("{}");
    fake_canon_at(&w.root.join("canon/target/debug/canon"), canon_pin()["version"].as_str().unwrap());
    let o = w.run(&["doctor"], &[]);
    assert_eq!(o.status.code(), Some(1), "{}", stdout(&o));
    assert_eq!(failed(&o), ["canon"]);
    assert!(line_of(&o, "canon").contains("-- fix:"));
    let pin = canon_pin();
    let cached = w.root.join("home/.cache/sdc/canon-simulate").join(&pin["commit"].as_str().unwrap()[..8]).join("bin/canon");
    fake_canon_at(&cached, pin["version"].as_str().unwrap());
    let o = w.run(&["doctor", "--json"], &[]);
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
    assert_eq!(canon_path(&o), cached.to_str().unwrap());
    let o = w.run(&["doctor"], &[]);
    assert!(line_of(&o, "canon").starts_with("pass") && line_of(&o, "canon").contains("(the pin)"), "{}", line_of(&o, "canon"));
}

#[test]
fn doctor_fails_a_canon_whose_version_is_not_the_pin() {
    let w = World::new("doctor-canon-version");
    w.fakes(&PROGRAMS[1..]);
    w.fakes(&HELPERS);
    w.login("{}");
    let wrong = w.root.join("elsewhere/canon");
    fake_canon_at(&wrong, "0.9.0");
    let o = w.run(&["doctor"], &[("CANON", wrong.to_str().unwrap())]);
    assert_eq!(o.status.code(), Some(1), "{}", stdout(&o));
    assert_eq!(failed(&o), ["canon"]);
    let line = line_of(&o, "canon");
    let want = format!("0.9.0, pin is {}", canon_pin()["version"].as_str().unwrap());
    assert!(line.contains(&want) && line.contains("-- fix:"), "{line}");
    fake_canon_at(&wrong, canon_pin()["version"].as_str().unwrap());
    let o = w.run(&["doctor"], &[("CANON", wrong.to_str().unwrap())]);
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
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
    let tool = w.root.join("assets/bindings/tools/decider.sh");
    std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(failed(&w.run(&["doctor"], &[])), ["bindings/tools/decider.sh"]);
    std::fs::remove_file(&tool).unwrap();
    let o = w.run(&["doctor"], &[]);
    assert_eq!(failed(&o), ["bindings/tools/decider.sh"]);
    assert!(line_of(&o, "bindings/tools/decider.sh").contains("-- fix:"));
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
    for later in ["run", "resume", "stop", "ui", "handback", "simulate"] {
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
    std::fs::create_dir_all(co.join("ui")).unwrap();
    std::fs::create_dir_all(co.join("scripts")).unwrap();
    std::fs::copy(r.join("scripts/run-finding-cycles.sh"), co.join("scripts/run-finding-cycles.sh")).unwrap();
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
    assert!(stdout(&o).contains("compared with scripts/run-finding-cycles.sh"), "{}", stdout(&o));
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
    std::fs::set_permissions(co.join("bindings/tools/decider.sh"), std::fs::Permissions::from_mode(0o644)).unwrap();
    let o = check(&w, &co);
    assert_finding(&o, "views/", &["missing", "no such directory"]);
    assert_finding(&o, "bindings/tools/decider.sh", &["missing", "not an executable file"]);
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
fn check_fails_a_duplicate_cycle_id() {
    let w = World::new("check-dup");
    let co = text_copy(&w);
    edit(&co, "cycles/suite.yaml", "- id: finding-agent\n", "- id: stub-cycle\n");
    let o = check(&w, &co);
    assert_finding(&o, "cycles/suite.yaml", &["cycle id stub-cycle appears more than once"]);
}

#[test]
fn check_fails_a_cycle_state_that_differs_from_the_script() {
    let w = World::new("check-state");
    let co = text_copy(&w);
    edit(&co, "cycles/suite.yaml", "- id: stub-cycle\n    state: expected", "- id: stub-cycle\n    state: pending");
    let o = check(&w, &co);
    assert_finding(&o, "cycles/suite.yaml", &["stub-cycle is pending in the suite and expected in scripts/run-finding-cycles.sh"]);
    assert_eq!(findings(&o).len(), 1, "{}", stdout(&o));
}

#[test]
fn check_fails_a_cycle_the_suite_and_the_script_do_not_share() {
    let w = World::new("check-ids");
    let co = text_copy(&w);
    edit(&co, "scripts/run-finding-cycles.sh", "pending=(rework-spent-dropped rework-spent-abandoned)", "pending=(rework-spent-dropped rework-spent-abandoned brand-new-cycle)");
    edit(&co, "cycles/suite.yaml", "- id: finding-agent\n", "- id: finding-agent-renamed\n");
    let o = check(&w, &co);
    assert_finding(&o, "cycles/suite.yaml", &["brand-new-cycle", "not in the suite"]);
    assert_finding(&o, "cycles/suite.yaml", &["finding-agent-renamed", "not in the arrays"]);
    assert_finding(&o, "cycles/suite.yaml", &["cycle finding-agent ", "not in the suite"]);
}

#[test]
fn check_fails_a_missing_fixture_and_a_bad_suite_header() {
    let w = World::new("check-fixture");
    let co = text_copy(&w);
    edit(&co, "cycles/suite.yaml", "fixtures: [driver/examples/sdc/fixtures/finding-spec-agent.yaml]", "fixtures: [driver/examples/sdc/fixtures/no-such-fixture.yaml]");
    assert_finding(&check(&w, &co), "cycles/suite.yaml", &["cycle finding-agent", "driver/examples/sdc/fixtures/no-such-fixture.yaml", "does not exist"]);
    edit(&co, "cycles/suite.yaml", "format: runbook.cycles/1", "format: runbook.cycles/2");
    edit(&co, "cycles/suite.yaml", "- id: stub-cycle\n    state: expected", "- id: stub-cycle\n    state: done");
    let o = check(&w, &co);
    assert_finding(&o, "cycles/suite.yaml", &["runbook.cycles/2"]);
    assert_finding(&o, "cycles/suite.yaml", &["cycle stub-cycle has state \"done\""]);
}

#[test]
fn check_skips_the_comparison_with_the_script_when_it_is_gone_and_says_so() {
    let w = World::new("check-no-script");
    let co = text_copy(&w);
    std::fs::remove_file(co.join("scripts/run-finding-cycles.sh")).unwrap();
    let o = check(&w, &co);
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
    let notes: Vec<String> = stdout(&o).lines().filter(|l| l.starts_with("note: ")).map(String::from).collect();
    assert_eq!(notes, ["note: scripts/run-finding-cycles.sh does not exist: the ids and states of cycles/suite.yaml are not compared with it"]);
}

#[test]
fn check_does_not_compare_an_unlisted_cycle_with_the_script() {
    let w = World::new("check-unlisted");
    let co = text_copy(&w);
    edit(&co, "cycles/suite.yaml", "- id: stub-cycle\n    state: expected", "- id: ran-outside-the-arrays\n    state: unlisted");
    let o = check(&w, &co);
    assert_finding(&o, "cycles/suite.yaml", &["stub-cycle", "not in the suite"]);
    assert_eq!(findings(&o).len(), 1, "{}", stdout(&o));
    assert!(stdout(&o).contains("note: 1 cycles of cycles/suite.yaml are unlisted"), "{}", stdout(&o));
}
