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
    assert!(h.contains("doctor") && h.contains("list"), "{h}");
    for later in ["run", "resume", "stop", "ui", "handback", "check", "simulate"] {
        assert!(!h.lines().any(|l| l.trim_start().starts_with(&format!("{later} "))), "{later} listed\n{h}");
        assert_eq!(Command::new(BIN).arg(later).output().unwrap().status.code(), Some(64), "{later}");
    }
    assert_eq!(Command::new(BIN).output().unwrap().status.code(), Some(64));
}
