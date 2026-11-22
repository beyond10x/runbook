//! No producer id of a binding, a protocol, an example or a view names the deleted runtime: the
//! runner is `runbook run`, so its evidence has the producer `runbook` (or `runbook.<x>`). The
//! text is read from the tree; no program is started.

use std::fs;
use std::path::{Path, PathBuf};

const OLD: &str = "sdc-drive";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn names_old_producer(line: &str) -> bool {
    let Some(at) = line.find("producer") else { return false };
    line[at..].contains(OLD)
}

#[test]
fn no_producer_id_names_the_deleted_runtime() {
    let r = root();
    let mut all = Vec::new();
    for d in ["bindings", "protocol", "driver/examples", "views", "website/data"] {
        files(&r.join(d), &mut all);
    }
    all.sort();
    let mut found = Vec::new();
    for p in &all {
        let Ok(text) = fs::read_to_string(p) else { continue };
        for (i, l) in text.lines().enumerate() {
            if names_old_producer(l) {
                found.push(format!("{}:{}: {}", p.strip_prefix(&r).unwrap().display(), i + 1, l.trim()));
            }
        }
    }
    assert!(all.len() > 20, "the scan reached {} files, expected the whole tree", all.len());
    assert!(found.is_empty(), "producer ids that name `{OLD}`:\n{}", found.join("\n"));
}

#[test]
fn the_scan_sees_a_producer_line() {
    assert!(names_old_producer("    \"producer\": \"sdc-drive.merge\","));
    assert!(names_old_producer("        producer: sdc-drive"));
    assert!(!names_old_producer("        producer: runbook.merge"));
    assert!(!names_old_producer("  \"note\": \"the old sdc-drive\""));
}
