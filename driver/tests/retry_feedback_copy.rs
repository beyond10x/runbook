//! The copy of the retry feedback that `sdc-node` writes under `.sdc/loom-feedback/<digest>/`: a link at
//! any of its files is replaced and never followed, a not-called start writes nothing through a link,
//! the copy exists when `find_rejection` reads it, and a start again over a leftover ends 0.

use serde_json::json;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

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

/// The options after the action name of the binding that runs `node` through `sdc-node`.
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

fn blob() -> String {
    "5e".repeat(32)
}

fn digest16(doc: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(doc.as_bytes()).iter().take(8).map(|x| format!("{x:02x}")).collect()
}

fn document(with_records: bool) -> String {
    let rec = json!({"evidence_id": "e1", "kind": "requirements_approval", "subject": "requirements", "subject_revision": "r1", "result": "rejected", "blob_sha256": blob(), "action": "requirements.check", "exit": null});
    let mut d = json!({"format": "loom.retry-feedback/1", "case": "IDEA-1.x", "action": "x", "logs": [{"blob_sha256": blob(), "tail": "The requirements lack the named risks.\n"}]});
    if with_records {
        d["records"] = json!([rec]);
    }
    d.to_string()
}

struct Rig {
    root: PathBuf,
    work: PathBuf,
}

impl Rig {
    fn new(tag: &str) -> Rig {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("retry-feedback-copy").join(tag);
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
        Rig { root, work }
    }

    fn feedback_dir(&self, doc: &str) -> PathBuf {
        self.work.join(".sdc/loom-feedback").join(digest16(doc))
    }

    /// One start of `node` with the document `doc` (the rest of the binding after the name, `stdin` the arguments).
    fn start(&self, node: &str, doc: &str) -> Output {
        let name = "feedback-doc.json";
        fs::write(self.root.join(name), doc).unwrap();
        let bins = Path::new(env!("CARGO_BIN_EXE_sdc-findings")).parent().unwrap();
        let path = format!("{}:{}", bins.display(), std::env::var("PATH").unwrap_or_default());
        let rest = binding(node);
        let mut c = Command::new(env!("CARGO_BIN_EXE_sdc-node"));
        c.arg(node).arg("--tools-dir").arg(repo().join("bindings/tools")).args(&rest);
        c.current_dir(&self.work).env_clear().env("PATH", path).env("LOOM_CASE", "IDEA-1.x").env("LOOM_RETRY_FEEDBACK", self.root.join(name));
        c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = c.spawn().unwrap();
        child.stdin.take().unwrap().write_all(br#"{"domain":"billing"}"#).unwrap();
        child.wait_with_output().unwrap()
    }
}

fn listing(dir: &Path) -> Vec<(String, bool, u64, std::time::SystemTime)> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, bool, u64, std::time::SystemTime)>) {
        let mut paths: Vec<PathBuf> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
        paths.sort();
        for p in paths {
            let m = fs::symlink_metadata(&p).unwrap();
            out.push((p.strip_prefix(base).unwrap().display().to_string(), m.is_dir(), m.len(), m.modified().unwrap()));
            if m.is_dir() {
                walk(base, &p, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out
}

/// Guard (the log files of the copy are not written with `fs::write`): a link at a log file of the
/// copy is never followed.
#[test]
fn a_link_at_a_log_file_of_the_copy_leaves_the_file_it_names_unchanged() {
    let rig = Rig::new("blob-link");
    let doc = document(true);
    let outside = rig.root.join("outside-file");
    fs::write(&outside, "mine\n").unwrap();
    let dir = rig.feedback_dir(&doc);
    fs::create_dir_all(&dir).unwrap();
    std::os::unix::fs::symlink(&outside, dir.join(blob())).unwrap();
    let o = rig.start("requirements.normalise", &doc);
    assert_eq!(fs::read_to_string(&outside).unwrap(), "mine\n", "the start wrote through the link at a log file (exit {:?})", o.status.code());
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
}

/// Guard (a link is removed whatever it names, not only a regular file): the
/// sentence "a link at one of them is removed, never followed" is whole: after the start the copy is
/// a regular file holding the copy of this start.
#[test]
fn a_link_at_feedback_json_is_replaced_by_the_copy_of_this_start() {
    let rig = Rig::new("feedback-link-replaced");
    let doc = document(true);
    let outside = rig.root.join("outside-file");
    fs::write(&outside, "mine\n").unwrap();
    let dir = rig.feedback_dir(&doc);
    fs::create_dir_all(&dir).unwrap();
    std::os::unix::fs::symlink(&outside, dir.join("feedback.json")).unwrap();
    let o = rig.start("requirements.normalise", &doc);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(fs::read_to_string(&outside).unwrap(), "mine\n");
    let m = fs::symlink_metadata(dir.join("feedback.json")).unwrap();
    assert!(m.file_type().is_file(), "feedback.json is still a link after the start");
    assert!(fs::read_to_string(dir.join("feedback.json")).unwrap().contains("requirements_approval"));
}

/// Guard (a truncating open in place of `create_new` would survive it, because the file
/// is removed first): a start that writes through a second name of the file would show here. A file of the copy with a
/// second name is replaced, the other name is unchanged.
#[test]
fn a_second_name_of_feedback_json_keeps_its_bytes() {
    let rig = Rig::new("feedback-hard");
    let doc = document(true);
    let outside = rig.root.join("outside-file");
    fs::write(&outside, "mine\n").unwrap();
    let dir = rig.feedback_dir(&doc);
    fs::create_dir_all(&dir).unwrap();
    fs::hard_link(&outside, dir.join("feedback.json")).unwrap();
    let o = rig.start("requirements.normalise", &doc);
    assert_eq!(fs::read_to_string(&outside).unwrap(), "mine\n", "the start wrote through a second name (exit {:?})", o.status.code());
}

/// Guard (the copy is not written before the not-called exit of `questions.write`): a document
/// without records leaves `questions.write` not called (exit 79) when `questions.yaml` is written;
/// the workspace, and the directory a link at `.sdc/loom-feedback` names, are as they were.
#[test]
fn a_not_called_questions_write_start_writes_nothing_through_a_link_at_loom_feedback() {
    let rig = Rig::new("qw-not-called");
    let outside = rig.root.join("outside");
    fs::create_dir_all(outside.join("keep")).unwrap();
    fs::write(outside.join("keep/marker"), "mine").unwrap();
    fs::create_dir_all(rig.work.join(".sdc")).unwrap();
    std::os::unix::fs::symlink(&outside, rig.work.join(".sdc/loom-feedback")).unwrap();
    let before = listing(&outside);
    let before_work = listing(&rig.work);
    let o = rig.start("questions.write", &document(false));
    assert_eq!(o.status.code(), Some(79), "{} {}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    assert_eq!(listing(&outside), before, "the directory the link names");
    assert_eq!(listing(&rig.work), before_work, "the workspace");
}

/// Guard (a start that is not called, `spec.author` waiting for a person, with a link at the directory of
/// the copy): nothing is written through it (stat level) and the exit is 79.
#[test]
fn a_not_called_start_with_a_link_at_the_directory_of_the_copy_writes_nothing_through_it() {
    let rig = Rig::new("nc-copy-link");
    fs::write(rig.work.join("findings.yaml"), "findings:\n  - id: f1\n    status: needs_person\n    raised_by: llm:spec.author\n").unwrap();
    let doc = document(true);
    let outside = rig.root.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("precious"), "keep").unwrap();
    fs::create_dir_all(rig.work.join(".sdc/loom-feedback")).unwrap();
    std::os::unix::fs::symlink(&outside, rig.feedback_dir(&doc)).unwrap();
    let before = listing(&outside);
    let o = rig.start("spec.author", &doc);
    assert_eq!(o.status.code(), Some(79), "{} {}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    assert_eq!(listing(&outside), before);
    assert!(fs::symlink_metadata(rig.feedback_dir(&doc)).unwrap().file_type().is_symlink());
}

/// A healthy second start of the same node with the document of the first and a leftover of an ended
/// start (the directory of the copy with only a log file in it) ends 0, not 126.
#[test]
fn a_start_again_over_the_leftover_of_an_ended_start_ends_0() {
    let rig = Rig::new("leftover");
    let doc = document(true);
    let dir = rig.feedback_dir(&doc);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(blob()), "old tail").unwrap();
    let o = rig.start("requirements.normalise", &doc);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(fs::read_to_string(dir.join(blob())).unwrap(), "The requirements lack the named risks.\n");
    let o = rig.start("requirements.normalise", &doc);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
}

/// Guard (the copy is written before `find_rejection`): `find_rejection` reads the copy's
/// `feedback.json` and the log file the record names, so an approval's rejection document that Loom's
/// feedback carries as the tail of a log reaches the stub only if the copy is written before it.
#[test]
fn a_rejection_document_in_a_log_tail_reaches_the_stub_through_the_copy() {
    let rig = Rig::new("rejection-through-copy");
    let rejection = r#"{"format":"sdc.rejection/1","approval":"requirements.check","revision":"abc123","principal":"person:reviewer","count":2,"reasons":["The refusal of an empty title is missing.","Say who may cancel."],"subject":"requirements"}"#;
    let rec = json!({"evidence_id": "e1", "kind": "requirements_approval", "subject": "requirements", "subject_revision": "abc123", "result": "rejected", "blob_sha256": blob(), "action": "requirements.check", "exit": 6});
    let doc = json!({"format": "loom.retry-feedback/1", "case": "IDEA-1.x", "action": "x", "records": [rec], "logs": [{"blob_sha256": blob(), "tail": rejection}]}).to_string();
    let o = rig.start("requirements.normalise", &doc);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let text = fs::read_to_string(rig.work.join("requirements.yaml")).unwrap();
    assert!(text.contains("# rework 2 of the rejection of requirements.check at abc123:\n# - The refusal of an empty title is missing.\n# - Say who may cancel.\n"), "{text}");
}

/// The refusal for a link at `.sdc` of a called start: one line, no path outside the work directory,
/// no byte of a file; and nothing written.
#[test]
fn a_called_start_with_a_link_at_sdc_ends_126_with_one_line_and_no_outside_path() {
    let rig = Rig::new("called-sdc-link");
    let outside = rig.root.join("outside-secret-dir");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret"), "SECRETBYTES").unwrap();
    std::os::unix::fs::symlink(&outside, rig.work.join(".sdc")).unwrap();
    let before = listing(&outside);
    let o = rig.start("requirements.normalise", &document(true));
    assert_eq!(o.status.code(), Some(126));
    let err = String::from_utf8_lossy(&o.stderr).to_string();
    assert_eq!(err.trim_end().lines().count(), 1, "{err}");
    assert!(!err.contains(&outside.display().to_string()) && !err.contains("SECRETBYTES") && !err.contains("outside-secret-dir"), "{err}");
    assert_eq!(listing(&outside), before);
}
