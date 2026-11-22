//! `tools/decision-grounds.sh`, the grounding check that stands before `sdc.decision.Recommend`
//! (`ess/domains/decision.yaml`): a recommended letter the request does not offer is refused, so
//! the check is withheld and no letter reaches the stored recommendation. The proposals here are
//! written by hand with invented text.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::{json, Value};

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("decision-grounds").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn option(letter: &str) -> Value {
    json!({"letter": letter, "does": "Do it.", "cost": "None.",
           "consequence": {"admits": [], "blocks": []}})
}

/// The verdict the tool prints for a proposal of `recommended` on a request offering `offered`,
/// whose one ground cites a line of a file the test writes.
fn verdict(name: &str, offered: &[&str], recommended: &str) -> Value {
    let dir = scratch(name);
    fs::write(dir.join("notes.md"), "first line\nsecond line\n").unwrap();
    let request = json!({
        "request_id": "DR-T-1", "case_id": "T", "unblocks": {"kind": "approval", "action": "a.approve"},
        "options": offered.iter().map(|l| option(l)).collect::<Vec<_>>(),
        "opened_at": "2026-11-22T09:00:00Z",
    });
    let proposal = json!({
        "request": request,
        "proposal": {
            "recommended_option": recommended, "strength": "strong", "default_on_silence": true,
            "proposed_by": "rule",
            "grounds": [{"source": "policy",
                         "citation": {"form": "path_line", "value": {"path": "notes.md", "line": 1}},
                         "supports": recommended, "how": "The first line says so."}],
        },
    });
    let file = dir.join("proposal.json");
    fs::write(&file, serde_json::to_vec(&proposal).unwrap()).unwrap();
    let out = Command::new("bash")
        .arg(manifest().join("../tools/decision-grounds.sh"))
        .arg(&file)
        .arg("--case")
        .arg(&dir)
        .arg("--root")
        .arg(&dir)
        .env("TMPDIR", &dir)
        .output()
        .unwrap();
    let parsed: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!("no verdict (exit {:?}): {e}: {}", out.status.code(), String::from_utf8_lossy(&out.stderr))
    });
    parsed["recommend"].clone()
}

#[test]
fn a_recommended_letter_the_request_offers_is_verified() {
    let v = verdict("offered", &["A", "B"], "A");
    assert_eq!(v["check_verdict"], "verified", "{v}");
}

#[test]
fn a_recommended_letter_the_request_does_not_offer_is_refused_and_named() {
    let v = verdict("unoffered", &["A", "B"], "Z");
    assert_eq!(v["check_verdict"], "refused", "{v}");
    let report = v["check_report"].as_str().unwrap();
    assert!(report.contains("option Z is not an option of the request"), "{report}");
}
