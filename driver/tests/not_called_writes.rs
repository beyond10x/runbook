//! A not-called exit (79) records nothing and writes nothing the next step reads. `sdc-node` copies
//! the retry's feedback document into `work/.sdc/loom-feedback/<digest>/` while it sets up, before
//! it decides not to call the model; the workspace holds that copy after the exit unless the exit
//! removes it.

use serde_json::json;
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
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

fn all_files(base: &Path, dir: &Path, out: &mut BTreeSet<String>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            all_files(base, &p, out);
        } else {
            out.insert(p.strip_prefix(base).unwrap().display().to_string());
        }
    }
}

/// The options of the binding of `spec.author` before `--stub`, and the stub's own arguments.
fn binding(node: &str) -> Vec<String> {
    let doc: serde_yaml::Value = serde_yaml::from_str(&fs::read_to_string(repo().join("bindings/sdc-spec.yaml")).unwrap()).unwrap();
    for a in doc["actions"].as_sequence().unwrap() {
        let argv: Vec<String> = a["executor"]["argv"].as_sequence().map(|s| s.iter().map(|v| v.as_str().unwrap_or_default().to_string()).collect()).unwrap_or_default();
        if argv.len() > 2 && argv[0] == "sdc-node" && argv[1] == node {
            return argv[2..].to_vec();
        }
    }
    panic!("no binding of {node}");
}

#[test]
fn a_not_called_exit_with_a_retry_feedback_document_leaves_the_workspace_as_it_was() {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("not-called-writes");
    let _ = fs::remove_dir_all(&root);
    let work = root.join("work");
    fs::create_dir_all(&work).unwrap();
    let fx = repo().join("driver/examples/sdc/fixtures");
    for (from, to) in [("idea.yaml", "idea.yaml"), ("dissection.yaml", "dissection.yaml"), ("questions-open.yaml", "questions.yaml"), ("requirements.yaml", "requirements.yaml"), ("context.yaml", "context.yaml")] {
        fs::copy(fx.join(from), work.join(to)).unwrap();
    }
    copy_dir(&fx.join("billing-spec"), &work.join("spec"));
    fs::write(
        work.join("findings.yaml"),
        "findings:\n  - id: f1\n    status: needs_person\n    raised_by: llm:spec.author\n",
    )
    .unwrap();
    let doc = json!({"format": "loom.retry-feedback/1", "case": "IDEA-1.x", "action": "x", "records": [{"kind": "spec_validation", "result": "invalid", "subject": "spec", "subject_revision": "r1"}], "logs": []});
    fs::write(root.join("feedback-doc.json"), doc.to_string()).unwrap();

    let mut before = BTreeSet::new();
    all_files(&work, &work, &mut before);

    let rest = binding("spec.author");
    let at = rest.iter().position(|a| a == "--stub").unwrap_or(rest.len());
    let bins = Path::new(env!("CARGO_BIN_EXE_sdc-findings")).parent().unwrap().to_path_buf();
    let mut c = Command::new(env!("CARGO_BIN_EXE_sdc-node"));
    c.arg("spec.author").arg("--tools-dir").arg(repo().join("bindings/tools")).args(&rest[..at]).args(&rest[at..]);
    c.current_dir(&work)
        .env_clear()
        .env("PATH", format!("{}:{}", bins.display(), std::env::var("PATH").unwrap_or_default()))
        .env("LOOM_CASE", "IDEA-1.x")
        .env("LOOM_RETRY_FEEDBACK", root.join("feedback-doc.json"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = c.spawn().unwrap();
    child.stdin.take().unwrap().write_all(br#"{"domain":"billing"}"#).unwrap();
    let o = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&o.stdout).to_string();
    assert_eq!(o.status.code(), Some(79), "not called, the code of exits.not_called: {stdout} {}", String::from_utf8_lossy(&o.stderr));
    assert!(stdout.contains("not called: findings wait for a person"), "{stdout}");

    let mut after = BTreeSet::new();
    all_files(&work, &work, &mut after);
    let made: Vec<&String> = after.difference(&before).collect();
    assert!(made.is_empty(), "a not-called exit writes nothing; the run made {made:?}");
}

/// Every path below `dir`, files, directories and links, without following a link.
fn entries(base: &Path, dir: &Path, out: &mut BTreeSet<String>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        out.insert(p.strip_prefix(base).unwrap().display().to_string());
        let m = fs::symlink_metadata(&p).unwrap();
        if m.is_dir() {
            entries(base, &p, out);
        }
    }
}

/// A workspace where spec.author is not called (a finding waits for a person), and the root.
fn rig(name: &str) -> (PathBuf, PathBuf) {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("not-called-removes").join(name);
    let _ = fs::remove_dir_all(&root);
    let work = root.join("work");
    fs::create_dir_all(&work).unwrap();
    let fx = repo().join("driver/examples/sdc/fixtures");
    for (from, to) in [("idea.yaml", "idea.yaml"), ("dissection.yaml", "dissection.yaml"), ("questions-open.yaml", "questions.yaml"), ("requirements.yaml", "requirements.yaml"), ("context.yaml", "context.yaml")] {
        fs::copy(fx.join(from), work.join(to)).unwrap();
    }
    copy_dir(&fx.join("billing-spec"), &work.join("spec"));
    fs::write(work.join("findings.yaml"), "findings:\n  - id: f1\n    status: needs_person\n    raised_by: llm:spec.author\n").unwrap();
    let doc = json!({"format": "loom.retry-feedback/1", "case": "IDEA-1.x", "action": "x", "records": [{"kind": "spec_validation", "result": "invalid", "subject": "spec", "subject_revision": "r1"}], "logs": [{"blob_sha256": "4f".repeat(32), "tail": "log tail"}]});
    fs::write(root.join("feedback-doc.json"), doc.to_string()).unwrap();
    (root, work)
}

/// Runs spec.author as Loom does; the exit code.
fn run(root: &Path, work: &Path, with_feedback: bool) -> Option<i32> {
    let rest = binding("spec.author");
    let at = rest.iter().position(|a| a == "--stub").unwrap_or(rest.len());
    let bins = Path::new(env!("CARGO_BIN_EXE_sdc-findings")).parent().unwrap().to_path_buf();
    let mut c = Command::new(env!("CARGO_BIN_EXE_sdc-node"));
    c.arg("spec.author").arg("--tools-dir").arg(repo().join("bindings/tools")).args(&rest[..at]).args(&rest[at..]);
    c.current_dir(work)
        .env_clear()
        .env("PATH", format!("{}:{}", bins.display(), std::env::var("PATH").unwrap_or_default()))
        .env("LOOM_CASE", "IDEA-1.x")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if with_feedback {
        c.env("LOOM_RETRY_FEEDBACK", root.join("feedback-doc.json"));
    }
    let mut child = c.spawn().unwrap();
    child.stdin.take().unwrap().write_all(br#"{"domain":"billing"}"#).unwrap();
    child.wait_with_output().unwrap().status.code()
}

#[test]
fn a_not_called_exit_with_no_feedback_document_writes_no_file_and_no_directory() {
    let (root, work) = rig("none");
    let mut before = BTreeSet::new();
    entries(&work, &work, &mut before);
    assert_eq!(run(&root, &work, false), Some(79));
    let mut after = BTreeSet::new();
    entries(&work, &work, &mut after);
    assert_eq!(after, before, "nothing made, nothing removed");
}

#[test]
fn a_feedback_directory_that_existed_before_keeps_its_other_content() {
    let (root, work) = rig("kept-dir");
    fs::create_dir_all(work.join(".sdc/loom-feedback/keep")).unwrap();
    fs::write(work.join(".sdc/loom-feedback/keep/marker"), "mine").unwrap();
    assert_eq!(run(&root, &work, true), Some(79));
    assert_eq!(fs::read_to_string(work.join(".sdc/loom-feedback/keep/marker")).unwrap(), "mine");
    let left: Vec<String> = fs::read_dir(work.join(".sdc/loom-feedback")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(left, ["keep"], "the call's own copy is gone");
}

#[test]
fn an_empty_sdc_directory_that_existed_before_is_still_there() {
    let (root, work) = rig("kept-sdc");
    fs::create_dir_all(work.join(".sdc")).unwrap();
    assert_eq!(run(&root, &work, true), Some(79));
    assert!(work.join(".sdc").is_dir(), ".sdc existed before the run");
    assert_eq!(fs::read_dir(work.join(".sdc")).unwrap().count(), 0);
}

#[test]
fn a_link_at_loom_feedback_that_existed_before_is_not_removed() {
    let (root, work) = rig("link");
    fs::create_dir_all(work.join(".sdc")).unwrap();
    std::os::unix::fs::symlink(root.join("nowhere"), work.join(".sdc/loom-feedback")).unwrap();
    assert_eq!(run(&root, &work, true), Some(79));
    assert!(fs::symlink_metadata(work.join(".sdc/loom-feedback")).is_ok(), "the link was there before the run and is gone after it");
}

#[test]
fn a_link_at_sdc_to_a_directory_outside_keeps_what_was_in_it() {
    let (root, work) = rig("link-out");
    let outside = root.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("precious"), "keep").unwrap();
    std::os::unix::fs::symlink(&outside, work.join(".sdc")).unwrap();
    assert_eq!(run(&root, &work, true), Some(79));
    assert_eq!(fs::read_to_string(outside.join("precious")).unwrap(), "keep");
    assert!(outside.is_dir());
    assert!(fs::symlink_metadata(work.join(".sdc")).unwrap().file_type().is_symlink(), "the link to .sdc is still a link");
}

/// Name, kind, size and modification time of every path below `dir`, links not followed.
fn stat_tree(base: &Path, dir: &Path, out: &mut Vec<(String, bool, u64, std::time::SystemTime)>) {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    paths.sort();
    for p in paths {
        let m = fs::symlink_metadata(&p).unwrap();
        out.push((p.strip_prefix(base).unwrap().display().to_string(), m.is_dir(), m.len(), m.modified().unwrap()));
        if m.is_dir() {
            stat_tree(base, &p, out);
        }
    }
}

#[test]
fn a_not_called_start_with_a_link_at_sdc_writes_nothing_through_it() {
    let (root, work) = rig("link-out-stat");
    let outside = root.join("outside");
    fs::create_dir_all(outside.join("loom-feedback/keep")).unwrap();
    fs::write(outside.join("precious"), "keep").unwrap();
    fs::write(outside.join("loom-feedback/keep/marker"), "mine").unwrap();
    std::os::unix::fs::symlink(&outside, work.join(".sdc")).unwrap();
    let mut before = Vec::new();
    stat_tree(&outside, &outside, &mut before);
    assert_eq!(run(&root, &work, true), Some(79), "a start that is not called writes nothing, so the link at .sdc does not end it");
    let mut after = Vec::new();
    stat_tree(&outside, &outside, &mut after);
    assert_eq!(after, before, "the directory the link names: file list, kinds, sizes and times are as they were");
    assert!(fs::symlink_metadata(work.join(".sdc")).unwrap().file_type().is_symlink());
}
