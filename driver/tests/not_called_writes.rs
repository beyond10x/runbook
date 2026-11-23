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
