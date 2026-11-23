//! The writer of the settings file reads back as what the record held, and a record's value cannot
//! become a member of the file.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const LOOM_SHA: &str = "d80830ea5b1b2c3d4e5f60718293a4b5c6d7e8f90123456789abcdef01234567";
const IDEA: &str = "Bill customers with invoices.";

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("settings-record-text").join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn sha(text: &str) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

fn record(d: &Path, models_writer: &str, decider_model: &str) -> PathBuf {
    let text = format!(
        r#"format: sdc.experiment/1
id: lx
kind: prospective
status: planned
created: 2026-11-22
title: A model run of the cycle through Loom
purpose: Whether a registered record starts a model run through Loom.
inputs:
- {{name: invoices, kind: idea, text: {IDEA}, sha256: {}}}
tree_commit: {COMMIT}
fixed:
  loom.bin: /opt/loom/b10x-loom
  loom.sha256: {LOOM_SHA}
  loom.confinement: local
  loom.cap_micro_usd: 5000000
  models.writer.model: {models_writer}
  models.implement.model: claude-sonnet-5-5
  models.review.model: claude-sonnet-5-5
  models.other.model: claude-sonnet-5-5
  decider.kind: model
  decider.model: {decider_model}
  autonomy: A5
  max_wall_seconds: 3600
variants:
- {{id: low, runner: loom, settings: {{max_llm_calls: 100}}, runs: []}}
runs_per_variant: 1
pass_rule: clean-delivery
decision_rule: {{rule: descriptive, text: the record describes its runs and decides nothing}}
"#,
        sha(IDEA)
    );
    fs::create_dir_all(d.join("experiments")).unwrap();
    fs::write(d.join("experiments/lx.yaml"), text).unwrap();
    d.join("experiments/lx.yaml")
}

fn fake_build(d: &Path) -> PathBuf {
    let b = d.join("b").join(COMMIT);
    fs::create_dir_all(b.join("protocol")).unwrap();
    fs::write(b.join("protocol/sdc.protocol.yaml"), "format: protocol/1\nprotocol:\n  id: sdc\n  revision: 11\n").unwrap();
    fs::create_dir_all(b.join("runbook/src/loom_run")).unwrap();
    fs::write(b.join("runbook/Cargo.toml"), "[package]\nname = \"runbook\"\n").unwrap();
    fs::write(b.join("runbook/src/loom_run/here.rs"), "// runbook run in process\n").unwrap();
    d.join("b")
}

fn print(d: &Path, builds: &Path) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_sdc-experiment"))
        .args(["run", "lx", "--print", "--builds-dir", builds.to_str().unwrap()])
        .current_dir(d)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", "/nonexistent-home")
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
}

fn printed_settings(out: &str) -> String {
    let mut text = String::new();
    let mut on = false;
    for l in out.lines() {
        if l == "  settings:" {
            on = true;
        } else if on && l.is_empty() {
            text.push('\n');
        } else if on {
            match l.strip_prefix("    ") {
                Some(rest) => text.push_str(&format!("{rest}\n")),
                None => break,
            }
        }
    }
    assert!(on, "no settings block in\n{out}");
    text
}

/// A YAML double-quoted scalar of `s`.
fn yaml(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if c.is_control() || c as u32 == 0x2028 || c as u32 == 0x2029 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

#[test]
fn a_text_of_a_record_reads_back_as_written_and_never_becomes_a_member_of_the_file() {
    let odd = [
        "a\nconfinement = \"local\"\ncap_micro_usd = 99999999",
        "line one\n\nline three\n",
        "ends with a newline\n",
        "\n",
        "a\r\nb",
        "tab\there",
        "ctl\u{1}\u{7f}x",
        "\u{2028}sep\u{85}",
        "a.b",
        "[login_file]",
        "1979-05-27",
        "0x10",
        "inf",
        "true",
        "'''\nconfinement = \"x\"",
        "\"\"\"\nconfinement = \"x\"\n\"\"\"",
        "a\"b\\c'd\u{e9}\u{4e2d}",
        "#",
        " ",
    ];
    for (n, value) in odd.iter().enumerate() {
        let d = scratch(&format!("odd-{n}"));
        let builds = fake_build(&d);
        record(&d, &yaml(value), &yaml(value));
        let (code, out) = print(&d, &builds);
        if code != 0 {
            // refused is allowed ("a value that cannot be written is refused, not written wrongly")
            assert!(!out.contains("settings:"), "{value:?}: refused and still printed: {out}");
            continue;
        }
        let text = printed_settings(&out);
        let doc: toml::Table = toml::from_str(&text).unwrap_or_else(|e| panic!("{value:?}: {e}\n{text}"));
        let want = ["format", "run", "autonomy", "max_llm_calls", "max_wall_seconds", "loom_sha256", "confinement", "cap_micro_usd", "decider", "models"];
        let keys: Vec<&str> = doc.keys().map(String::as_str).collect();
        assert_eq!(keys.iter().filter(|k| !want.contains(k)).count(), 0, "{value:?}: a member the writer does not write: {keys:?}\n{text}");
        assert_eq!(doc["confinement"].as_str(), Some("local"), "{value:?}\n{text}");
        assert_eq!(doc["cap_micro_usd"].as_integer(), Some(5_000_000), "{value:?}\n{text}");
        assert_eq!(doc["models"]["writer"]["model"].as_str(), Some(*value), "{value:?}\n{text}");
        assert_eq!(doc["decider"]["model"].as_str(), Some(*value), "{value:?}\n{text}");
    }
}
