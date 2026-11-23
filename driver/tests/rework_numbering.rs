//! The note the stub of `sdc-node` writes into the artifact a person's approval rejected, from the
//! second rejection of the same approval on: "rework 2 of the rejection of <action>".
//!
//! Loom hands a bound program only the retry records about the revision the case holds for the
//! subject now (`retry_records` of `b10x-loom-bindings`: a record about an earlier revision is
//! skipped), and a rework makes a new revision. So the document of the second rework holds one
//! record, the second, as the first rework's did. The stub counts the rejections it has numbered
//! in its workspace (`.sdc/rework-seen`, the evidence ids), so the same reasons twice still leave
//! another artifact and Loom makes a revision.

use serde_json::json;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo() -> PathBuf {
    manifest().parent().unwrap().to_path_buf()
}

fn fx() -> PathBuf {
    manifest().join("examples/sdc/fixtures")
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let p = e.unwrap().path();
        let t = to.join(p.file_name().unwrap());
        if p.is_dir() {
            copy_dir(&p, &t);
        } else {
            fs::copy(&p, &t).unwrap();
        }
    }
}

/// The options of `requirements.normalise` in `bindings/*.yaml` after the action name.
fn binding(node: &str) -> Vec<String> {
    for e in fs::read_dir(repo().join("bindings")).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_none_or(|x| x != "yaml") {
            continue;
        }
        let doc: serde_yaml::Value = serde_yaml::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        for a in doc.get("actions").and_then(|a| a.as_sequence()).into_iter().flatten() {
            let argv: Vec<String> = a
                .get("executor")
                .and_then(|x| x.get("argv"))
                .and_then(|x| x.as_sequence())
                .map(|s| s.iter().map(|v| v.as_str().unwrap_or_default().to_string()).collect())
                .unwrap_or_default();
            if argv.len() > 2 && argv[0] == "sdc-node" && argv[1] == node {
                return argv[2..].to_vec();
            }
        }
    }
    panic!("no binding runs {node} through sdc-node");
}

/// One case's workspace, kept across the reworks of the case as it is under Loom.
struct Case {
    root: PathBuf,
    runs: usize,
}

impl Case {
    fn new(tag: &str) -> Case {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("rework-numbering").join(tag);
        let _ = fs::remove_dir_all(&root);
        let work = root.join("work");
        fs::create_dir_all(&work).unwrap();
        let pager = fx().join("bugfix-pager");
        copy_dir(&pager.join("crate"), &work.join("mainline"));
        copy_dir(&pager.join("crate"), &work.join("impl"));
        for (from, to) in [("idea.yaml", "idea.yaml"), ("dissection.yaml", "dissection.yaml"), ("questions-open.yaml", "questions.yaml"), ("requirements.yaml", "requirements.yaml"), ("context.yaml", "context.yaml")] {
            fs::copy(fx().join(from), work.join(to)).unwrap();
        }
        copy_dir(&fx().join("billing-spec"), &work.join("spec"));
        Case { root, runs: 0 }
    }

    /// `requirements.yaml` after one start of the stub of `requirements.normalise` with the retry
    /// feedback document `doc`, kept as Loom keeps one: a file of its own for each document.
    fn start(&mut self, doc: &str) -> String {
        self.runs += 1;
        let work = self.root.join("work");
        let name = format!("feedback-doc-{}.json", self.runs);
        fs::write(self.root.join(&name), doc).unwrap();
        let bins = Path::new(env!("CARGO_BIN_EXE_sdc-findings")).parent().unwrap();
        let path = format!("{}:{}", bins.display(), std::env::var("PATH").unwrap_or_default());
        let rest = binding("requirements.normalise");
        let mut c = Command::new(env!("CARGO_BIN_EXE_sdc-node"));
        c.arg("requirements.normalise").arg("--tools-dir").arg(repo().join("bindings/tools")).args(&rest);
        c.current_dir(&work).env_clear().env("PATH", path).env("LOOM_CASE", "IDEA-1.x").env("LOOM_RETRY_FEEDBACK", self.root.join(&name));
        c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = c.spawn().unwrap();
        child.stdin.take().unwrap().write_all(br#"{"domain":"billing"}"#).unwrap();
        let o = child.wait_with_output().unwrap();
        assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
        fs::read_to_string(work.join("requirements.yaml")).unwrap()
    }
}

const REASONS: &str = "The requirements lack the named risks.\n";

fn blob() -> String {
    "5e".repeat(32)
}

/// A record of the retry feedback document as Loom writes it.
fn record(id: &str, revision: &str, result: &str, action: &str) -> serde_json::Value {
    json!({"evidence_id": id, "kind": "requirements_approval", "subject": "requirements", "subject_revision": revision, "result": result, "blob_sha256": blob(), "action": action, "exit": null})
}

fn document(records: &[serde_json::Value]) -> String {
    json!({"format": "loom.retry-feedback/1", "case": "IDEA-1.x", "action": "x", "records": records, "logs": [{"blob_sha256": blob(), "tail": REASONS}]}).to_string()
}

fn note(n: &str) -> String {
    format!("# rework{n} of the rejection of requirements.check:\n# - The requirements lack the named risks.\n")
}

/// What Loom hands the writer after each rejection of the same approval: one record, the one about
/// the revision the case holds now (a rework makes a new revision, so the earlier rejection is
/// about an earlier revision and is not in the document). The second rework has to leave another
/// artifact than the first, or Loom makes no revision and the tree has no step to take.
#[test]
fn the_second_rework_of_the_same_reasons_differs_from_the_first_when_loom_gives_one_record() {
    let mut case = Case::new("second-differs");
    let first = case.start(&document(&[record("e1", "r1", "rejected", "requirements.check")]));
    let second = case.start(&document(&[record("e2", "r2", "rejected", "requirements.check")]));
    assert_ne!(first, second, "the second rework left the artifact the first one left:\n{second}");
    assert!(first.ends_with(&note("")), "{first}");
    assert!(second.ends_with(&note(" 2")), "{second}");
}

#[test]
fn each_rework_of_the_same_reasons_is_numbered_by_the_rejections_before_it() {
    let mut case = Case::new("each-numbered");
    for (n, (id, revision)) in [("e1", "r1"), ("e2", "r2"), ("e3", "r3"), ("e4", "r4")].into_iter().enumerate() {
        let text = case.start(&document(&[record(id, revision, "rejected", "requirements.check")]));
        let nth = if n == 0 { String::new() } else { format!(" {}", n + 1) };
        assert!(text.ends_with(&note(&nth)), "rework {}: {text}", n + 1);
    }
}

/// A second start of the writer for the rejection it has numbered (the run is retried before the
/// artifact changes) is the same rework: the same document gives the same note.
#[test]
fn a_start_again_on_the_same_rejection_writes_the_same_note() {
    let mut case = Case::new("same-rework");
    let doc = document(&[record("e1", "r1", "rejected", "requirements.check")]);
    let first = case.start(&doc);
    let again = case.start(&doc);
    assert_eq!(first, again);
    assert!(again.ends_with(&note("")), "{again}");
}

/// Two records of one document (the records an earlier rejection of the same revision left) are two
/// rejections, and a later document that holds one of them again does not count it twice.
#[test]
fn a_rejection_in_two_documents_is_counted_once() {
    let mut case = Case::new("counted-once");
    let first = case.start(&document(&[record("e1", "r1", "rejected", "requirements.check"), record("e2", "r1", "rejected", "requirements.check")]));
    assert!(first.ends_with(&note(" 2")), "{first}");
    let second = case.start(&document(&[record("e2", "r1", "rejected", "requirements.check"), record("e3", "r2", "rejected", "requirements.check")]));
    assert!(second.ends_with(&note(" 3")), "{second}");
}

/// A record that is no rejection of this approval by a person does not count as one: another
/// action's rejection, and a result that is no rejection of this action.
#[test]
fn a_record_that_is_not_a_rejection_of_this_approval_is_not_counted() {
    let mut case = Case::new("not-counted");
    let first = case.start(&document(&[record("e1", "r1", "rejected", "requirements.check")]));
    assert!(first.ends_with(&note("")), "{first}");
    let second = case.start(&document(&[
        record("e3", "r2", "rejected", "tests.run"),
        record("e2", "r2", "rejected", "requirements.check"),
        record("e4", "r2", "approved", "requirements.check"),
    ]));
    assert!(second.ends_with(&note(" 2")), "{second}");
}
