//! The readers of the run's two settings records (`settings.json`, `run-settings.jsonl`) read by
//! member name: the same record with its members in another order gives the same result
//! (`docs/loom-run-settings.md`: the order of members follows the settings file of the run).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("settings-record-order").join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().flatten() {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if src.is_dir() {
            copy_tree(&src, &dst);
        } else {
            fs::copy(&src, &dst).unwrap();
        }
    }
}

/// A record the runner's tests hold as the bytes the runner writes (`runbook/tests/fixtures/run-records/`),
/// its clock reading `AT` filled in.
fn golden(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../runbook/tests/fixtures/run-records").join(name);
    fs::read_to_string(&path).unwrap().replace("\"at\":\"AT\"", "\"at\":\"2026-11-22T09:00:00Z\"")
}

/// The JSON text of `v` with the members of every object in the reverse of the order the value holds.
fn reversed(v: &Value) -> String {
    match v {
        Value::Object(m) => format!("{{{}}}", m.iter().rev().map(|(k, x)| format!("{}:{}", serde_json::to_string(k).unwrap(), reversed(x))).collect::<Vec<_>>().join(",")),
        Value::Array(a) => format!("[{}]", a.iter().map(reversed).collect::<Vec<_>>().join(",")),
        other => other.to_string(),
    }
}

fn put(run: &Path, settings: &str, line: &str) {
    fs::write(run.join("settings.json"), settings).unwrap();
    fs::write(run.join("run-settings.jsonl"), format!("{line}\n")).unwrap();
}

/// What each reader of `driver/src` that reads the two records prints for the run: the metrics, the
/// timeline and the finding figures of the run directory.
fn readers(run: &Path) -> Vec<(&'static str, String)> {
    let metrics = Command::new(env!("CARGO_BIN_EXE_sdc-run-metrics")).arg(run).args(["--json", "-"]).output().unwrap();
    let timeline = Command::new(env!("CARGO_BIN_EXE_sdc-run-timeline")).arg(run).args(["--json", "-"]).output().unwrap();
    let kpis = Command::new(env!("CARGO_BIN_EXE_sdc-finding-kpis")).arg(run).arg("--json").output().unwrap();
    [("sdc-run-metrics", metrics), ("sdc-run-timeline", timeline), ("sdc-finding-kpis", kpis)]
        .into_iter()
        .map(|(name, o)| {
            assert_eq!(o.status.code(), Some(0), "{name}: {}", String::from_utf8_lossy(&o.stderr));
            assert!(!o.stdout.is_empty(), "{name} printed nothing");
            (name, String::from_utf8_lossy(&o.stdout).into_owned())
        })
        .collect()
}

#[test]
fn the_readers_of_the_run_records_give_the_same_result_for_the_golden_record_and_for_its_members_in_another_order() {
    let run = scratch("run");
    copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/loom-run"), &run);

    let (settings, line) = (golden("settings.json"), golden("run-settings.jsonl"));
    put(&run, &settings, line.trim_end());
    let in_golden_order = readers(&run);

    let other_settings = reversed(&serde_json::from_str(&settings).unwrap());
    let other_line = reversed(&serde_json::from_str(line.trim_end()).unwrap());
    let before = |text: &str| (text.find("\"case\"").unwrap(), text.find("\"autonomy\"").unwrap());
    assert!(before(&settings).0 > before(&settings).1 && before(&other_settings).0 < before(&other_settings).1, "the second record is not in another order");
    assert!(before(&line).0 > before(&line).1 && before(&other_line).0 < before(&other_line).1, "the second line is not in another order");
    assert_ne!(other_settings, settings.trim_end());
    put(&run, &other_settings, &other_line);
    let in_other_order = readers(&run);

    for ((name, a), (_, b)) in in_golden_order.iter().zip(&in_other_order) {
        assert_eq!(a, b, "{name} reads the record by its order");
    }

    // the readers do read the record: a value that changes is a result that changes
    let changed = line.trim_end().replace("\"max_wall_seconds\":120", "\"max_wall_seconds\":7200").replace("\"adapter\":\"metaharness\"", "\"adapter\":\"another\"");
    assert_ne!(changed, line.trim_end());
    put(&run, &settings, &changed);
    let changed = readers(&run);
    assert!(in_golden_order.iter().zip(&changed).any(|((_, a), (_, b))| a != b), "no reader reads run-settings.jsonl");
}
