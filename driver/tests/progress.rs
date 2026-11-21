//! `sdc-experiment progress` and the stale check of `check --all` on a small repository written by
//! the test: one record with two variants, its run summaries and a release table
//! (docs/improving.md, `driver/src/bin/sdc-experiment/progress.rs`).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::json;
use sha2::{Digest, Sha256};

const IDEA: &str = "A to-do list web app";

fn sha(s: &str) -> String {
    Sha256::digest(s.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

fn exp(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sdc-experiment")).args(args).current_dir(root).output().unwrap()
}

fn say(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

fn summary(run: &str, writer: &str, clean: bool, cost: f64, wall: f64) -> String {
    json!({"format": "sdc.run-metrics/1", "run": run, "nodes": [], "edges": [], "stages": [],
        "metrics": {"finished": true, "closed": true, "delivered": clean, "clean": clean, "cost_usd": cost, "wall_s": wall,
            "end": if clean { "closed: operating" } else { "closed: not_delivered" }},
        "settings": {"models.writer.model": writer, "decider.model": "h"}}).to_string()
}

fn repo(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("progress").join(name);
    let _ = fs::remove_dir_all(&root);
    let e = root.join("experiments");
    fs::create_dir_all(e.join("e1/runs")).unwrap();
    fs::write(e.join("e1.yaml"), format!("format: sdc.experiment/1\nid: e1\nkind: retrospective\nstatus: complete\ncreated: 2026-11-21\ntitle: t\npurpose: p\n\
inputs:\n- {{name: todo, kind: idea, text: \"{IDEA}\", sha256: {}}}\ntree_commit: acd6f12\nfixed: {{decider.model: h}}\n\
variants:\n- {{id: a, settings: {{models.writer.model: h}}, runs: [h1, h2]}}\n- {{id: b, settings: {{models.writer.model: s}}, runs: [s1]}}\n\
runs_per_variant: 2\npass_rule: clean-delivery\ndecision_rule: {{rule: descriptive, text: t}}\n", sha(IDEA))).unwrap();
    for (r, w, c, cost, wall) in [("h1", "h", true, 1.0, 1200.0), ("h2", "h", false, 0.5, 600.0), ("s1", "s", true, 2.0, 300.0)] {
        fs::write(e.join(format!("e1/runs/{r}.json")), summary(r, w, c, cost, wall)).unwrap();
    }
    fs::write(e.join("releases.yaml"), format!("format: sdc.releases/1\ninput: {{name: todo, sha256: {}, description: a to-do app}}\nsettings:\n\
- {{id: haiku, label: All Haiku, note: n, match: {{models.writer.model: h, decider.model: h}}}}\n\
- {{id: mixed, label: Sonnet work, note: n, match: {{models.writer.model: s, decider.model: h}}}}\n\
entries:\n- {{id: t1, kind: pre-release, short: batch, date: 2026-11-21, tree: acd6f12, experiments: [e1], changes: [{{commit: acd6f12, line: a fix, effect: unmeasured}}]}}\n\
- {{id: 0.1.0, kind: release, short: notes, version: 0.1.0, date: 2026-11-21, tree: null, tree_note: not tagged, experiments: [], changes: [{{commit: 8b3abd4, line: notes, effect: none}}]}}\n", sha(IDEA))).unwrap();
    root
}

#[test]
fn the_ledger_is_stale_when_the_table_changes_and_current_after_progress() {
    let root = repo("stale");
    let o = exp(&root, &["report", "e1"]);
    assert!(o.status.success(), "{}", say(&o));
    assert!(say(&o).contains("progress.svg"), "report writes the ledger too: {}", say(&o));
    let o = exp(&root, &["check", "--all"]);
    assert!(o.status.success(), "{}", say(&o));
    let md = fs::read_to_string(root.join("docs/progress.md")).unwrap();
    assert!(md.contains("| All Haiku | `e1` | a | 1 of 2 | none given, n 2 < 3 | 1.00 [1.00, 1.00] (1) | 1200 [1200, 1200] (1) |"), "{md}");
    let svg = fs::read_to_string(root.join("docs/img/progress.svg")).unwrap();
    assert!(svg.contains(">2 of 3<") && svg.contains("$1.00 · 20 min"), "{svg}");

    // Red: a change line edited in the table, nothing regenerated.
    let t = root.join("experiments/releases.yaml");
    fs::write(&t, fs::read_to_string(&t).unwrap().replace("line: a fix", "line: another fix")).unwrap();
    let o = exp(&root, &["check", "--all"]);
    assert_eq!(o.status.code(), Some(1), "{}", say(&o));
    for f in ["docs/progress.md: stale", "docs/progress.json: stale"] {
        assert!(say(&o).contains(f), "{}", say(&o));
    }
    // Red: the image alone edited by hand.
    exp(&root, &["progress"]);
    let img = root.join("docs/img/progress.svg");
    fs::write(&img, fs::read_to_string(&img).unwrap().replace("2 of 3", "3 of 3")).unwrap();
    let o = exp(&root, &["check", "--all"]);
    assert_eq!(o.status.code(), Some(1), "{}", say(&o));
    assert!(say(&o).contains("progress.svg: stale"), "{}", say(&o));

    // Green again.
    let o = exp(&root, &["progress"]);
    assert!(o.status.success(), "{}", say(&o));
    let o = exp(&root, &["check", "--all"]);
    assert!(o.status.success(), "{}", say(&o));
}

#[test]
fn the_image_is_the_same_bytes_from_two_copies_of_the_inputs() {
    let (a, b) = (repo("bytes-a"), repo("bytes-b"));
    for r in [&a, &b] {
        let o = exp(r, &["progress"]);
        assert!(o.status.success(), "{}", say(&o));
    }
    for f in ["docs/img/progress.svg", "docs/progress.json", "docs/progress.md"] {
        assert_eq!(fs::read(a.join(f)).unwrap(), fs::read(b.join(f)).unwrap(), "{f}");
    }
}

#[test]
fn a_table_problem_writes_nothing_and_fails() {
    let root = repo("bad");
    let t = root.join("experiments/releases.yaml");
    fs::write(&t, fs::read_to_string(&t).unwrap().replace("effect: none", "effect: maybe")).unwrap();
    let o = exp(&root, &["progress"]);
    assert_eq!(o.status.code(), Some(1), "{}", say(&o));
    assert!(say(&o).contains("`effect` must be measured, unmeasured or none"), "{}", say(&o));
    assert!(!root.join("docs/progress.md").exists());
}

/// D149: the LLM decider is `decider.kind: model`; `haiku` is its old name. A record that says
/// haiku checks against a run that recorded model and the other way round, and a release table
/// setting matches variants by either name.
#[test]
fn decider_kind_haiku_and_model_are_one_setting() {
    let root = repo("decider-kind");
    let e = root.join("experiments");
    let rec = e.join("e1.yaml");
    fs::write(&rec, fs::read_to_string(&rec).unwrap().replace("fixed: {decider.model: h}", "fixed: {decider.model: h, decider.kind: haiku}")).unwrap();
    for (r, kind) in [("h1", "model"), ("h2", "haiku"), ("s1", "model")] {
        let p = e.join(format!("e1/runs/{r}.json"));
        let mut d: serde_json::Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        d["settings"]["decider.kind"] = json!(kind);
        fs::write(&p, d.to_string()).unwrap();
    }
    let t = e.join("releases.yaml");
    fs::write(&t, fs::read_to_string(&t).unwrap().replace("match: {models.writer.model: h, decider.model: h}",
        "match: {models.writer.model: h, decider.model: h, decider.kind: model}")).unwrap();
    let o = exp(&root, &["check", "experiments/e1.yaml"]);
    assert!(o.status.success(), "{}", say(&o));
    assert!(!say(&o).contains("decider.kind"), "{}", say(&o));
    let o = exp(&root, &["progress"]);
    assert!(o.status.success(), "{}", say(&o));
    let md = fs::read_to_string(root.join("docs/progress.md")).unwrap();
    assert!(md.contains("| All Haiku | `e1` | a |"), "{md}");
}
