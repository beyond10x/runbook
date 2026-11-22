//! `sdc-node`, the Rust program for the seven plain nodes (idea.dissect, questions.write,
//! requirements.normalise, spec.author, test.reproduce, bug.dissect, change.fix), against the
//! shell code of `bindings/tools/llm-node.sh` that did the same under Loom.
//!
//! The script dispatches the seven names to the program; `SDC_NODE_SHELL=1` (read only here) keeps
//! its own code, so both run on the same input in two roots and must give the same exit code,
//! standard output, standard error and the same files in the workspace. A YAML file whose bytes
//! differ (the one a YAML writer lays out its own way) is compared as a parsed document, and the
//! test names it on standard error. The Loom-mode prompts are held byte for byte against
//! `instructions/golden/llm-node/`, the step log lines against `instructions/golden/node-log/`
//! (SDC_BLESS_PROMPTS=1 rewrites the goldens).

use regex::Regex;
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const NODES: [&str; 7] = ["idea.dissect", "questions.write", "requirements.normalise", "spec.author", "test.reproduce", "bug.dissect", "change.fix"];

/// The files whose bytes may differ between the script and the program, compared as documents.
const AS_DOCUMENTS: [&str; 1] = [".sdc/stub-findings."];

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo() -> PathBuf {
    manifest().parent().unwrap().to_path_buf()
}

fn fx() -> PathBuf {
    manifest().join("examples/sdc/fixtures")
}

/// This process's PATH with the driver crate's binaries first: `sdc-node`, `sdc-findings` and
/// `sdc-reply` come from PATH under Loom.
fn path() -> String {
    let bins = Path::new(env!("CARGO_BIN_EXE_sdc-findings")).parent().unwrap();
    format!("{}:{}", bins.display(), std::env::var("PATH").unwrap_or_default())
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

fn put(base: &Path, rel: &str, text: &str) {
    let p = base.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

/// The argv of the node's action in `bindings/*.yaml` after the action name: the options, then
/// `--stub` and the stub's own arguments.
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
            if argv.len() > 2 && argv[0] == "tools/llm-node.sh" && argv[1] == node {
                return argv[2..].to_vec();
            }
        }
    }
    panic!("no binding runs {node} through llm-node.sh");
}

/// One work/ for every node: the intake and direction files, the specification, a story, the code
/// tree and the bug files.
fn base_work(work: &Path, node: &str) {
    let pager = fx().join("bugfix-pager");
    copy_dir(&pager.join("crate"), &work.join("mainline"));
    copy_dir(&pager.join("crate"), &work.join("impl"));
    copy_dir(&pager.join("stub-repro"), &work.join("repro"));
    let repro: serde_json::Value = serde_json::from_str(&fs::read_to_string(pager.join("stub-repro/repro.json")).unwrap()).unwrap();
    let file = repro["file"].as_str().unwrap();
    let to = work.join("impl").join(file);
    fs::create_dir_all(to.parent().unwrap()).unwrap();
    fs::copy(pager.join("stub-repro").join(Path::new(file).file_name().unwrap()), to).unwrap();
    fs::copy(pager.join("stub-dissection.json"), work.join("bug-dissection.json")).unwrap();
    fs::copy(pager.join("bug.yaml"), work.join("bug.yaml")).unwrap();
    for (from, to) in [("idea.yaml", "idea.yaml"), ("dissection.yaml", "dissection.yaml"), ("questions-open.yaml", "questions.yaml"),
        ("requirements.yaml", "requirements.yaml"), ("context.yaml", "context.yaml")] {
        fs::copy(fx().join(from), work.join(to)).unwrap();
    }
    copy_dir(&fx().join("billing-spec"), &work.join("spec"));
    for (p, t) in [("aep/vision/product.md", "# vision\n"), ("aep/initiative/roadmap.md", "# roadmap\n"),
        ("aep/story/list.md", "---\nid: story:list\n---\n# list\n"), (".sdc/gate/ir.json", r#"{"commands":{"billing.invoice.CreateInvoice":{}}}"#)] {
        put(work, p, t);
    }
    if node == "questions.write" {
        // The interview is not rewritten while one exists: the node runs on a work/ without it.
        fs::remove_file(work.join("questions.yaml")).unwrap();
    }
}

#[derive(Debug)]
struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
    files: BTreeMap<String, String>,
}

/// Text with the run's root and the instants and process ids it wrote made comparable.
fn norm(root: &Path, text: &str) -> String {
    let t = text.replace(&root.display().to_string(), "<root>");
    let t = Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z").unwrap().replace_all(&t, "<instant>");
    Regex::new(r#""pid":\s*\d+"#).unwrap().replace_all(&t, r#""pid":0"#).into_owned()
}

fn files(root: &Path, work: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let rel = p.strip_prefix(work).unwrap().display().to_string();
        if p.is_dir() {
            files(root, work, &p, out);
        } else {
            out.insert(rel, norm(root, &String::from_utf8_lossy(&fs::read(&p).unwrap())));
        }
    }
}

/// What a run is given.
struct Start<'a> {
    node: &'a str,
    /// Options after the binding's own, before `--stub`; `{root}` is the run's root.
    extra: Vec<String>,
    /// Environment variables besides PATH and LOOM_CASE; `{root}` as above.
    env: Vec<(String, String)>,
    stdin: String,
    /// Whether the binding's own options and stub argv are given.
    binding: bool,
    /// The script's own jq complains on standard error before it refuses a bad option value or
    /// reads a document without the field it iterates; the program does not reproduce those lines,
    /// so a case that has them compares standard error without the lines `jq: error`.
    jq_noise: bool,
}

fn start(node: &str) -> Start<'_> {
    Start { node, extra: vec![], env: vec![], stdin: r#"{"domain":"billing"}"#.to_string(), binding: true, jq_noise: false }
}

impl<'a> Start<'a> {
    fn jq_noise(mut self) -> Self {
        self.jq_noise = true;
        self
    }
    fn with(mut self, extra: &[&str]) -> Self {
        self.extra = extra.iter().map(|s| s.to_string()).collect();
        self
    }
    fn env(mut self, env: &[(&str, &str)]) -> Self {
        self.env = env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        self
    }
}

fn exec(root: &Path, shell: bool, s: &Start, case: &str) -> Run {
    let sub = |t: &str| t.replace("{root}", &root.display().to_string());
    let rest = if s.binding { binding(s.node) } else { vec![] };
    let at = rest.iter().position(|a| a == "--stub").unwrap_or(rest.len());
    let mut argv: Vec<String> = vec![s.node.to_string()];
    argv.extend_from_slice(&rest[..at]);
    argv.extend(s.extra.iter().map(|a| sub(a)));
    argv.extend_from_slice(&rest[at..]);
    let work = root.join("work");
    let mut c = Command::new(repo().join("bindings/tools/llm-node.sh"));
    c.args(&argv).current_dir(&work).env_clear().env("PATH", path()).env("LOOM_CASE", case);
    if shell {
        c.env("SDC_NODE_SHELL", "1");
    }
    for (k, v) in &s.env {
        c.env(k, sub(v));
    }
    c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = c.spawn().unwrap();
    child.stdin.take().unwrap().write_all(s.stdin.as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    let mut f = BTreeMap::new();
    files(root, &work, &work, &mut f);
    Run { code: o.status.code(), stdout: norm(root, &String::from_utf8_lossy(&o.stdout)), stderr: norm(root, &String::from_utf8_lossy(&o.stderr)), files: f }
}

fn roots(name: &str) -> (PathBuf, PathBuf) {
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("node-parity").join(name);
    let _ = fs::remove_dir_all(&base);
    let (s, r) = (base.join("shell"), base.join("rust"));
    fs::create_dir_all(s.join("work")).unwrap();
    fs::create_dir_all(r.join("work")).unwrap();
    (s, r)
}

/// The script and the program on the same input; `setup` gets each root (its `work/` has the base
/// files) and writes what the case adds.
fn pair(name: &str, s: &Start, setup: &dyn Fn(&Path)) -> (Run, Run) {
    let (sh, rs) = roots(name);
    for r in [&sh, &rs] {
        base_work(&r.join("work"), s.node);
        setup(r);
    }
    let mut a = exec(&sh, true, s, "IDEA-1.x");
    let b = exec(&rs, false, s, "IDEA-1.x");
    if s.jq_noise {
        a.stderr = a.stderr.lines().filter(|l| !l.starts_with("jq: error")).map(|l| format!("{l}\n")).collect();
    }
    same(name, &a, &b);
    (a, b)
}

fn nothing(_: &Path) {}

/// Equal exit code, standard output, standard error and files; a YAML file in `AS_DOCUMENTS`
/// whose bytes differ must parse to the same document, and is named.
fn same(what: &str, shell: &Run, rust: &Run) {
    assert_eq!(shell.code, rust.code, "{what}: exit\nshell {shell:#?}\nrust {rust:#?}");
    assert_eq!(shell.stdout, rust.stdout, "{what}: stdout");
    assert_eq!(shell.stderr, rust.stderr, "{what}: stderr");
    assert_eq!(shell.files.keys().collect::<Vec<_>>(), rust.files.keys().collect::<Vec<_>>(), "{what}: the files written");
    for (k, v) in &shell.files {
        if v == &rust.files[k] {
            continue;
        }
        assert!(k.ends_with(".yaml") && AS_DOCUMENTS.iter().any(|p| k.starts_with(p)), "{what}: {k} differs\n--- shell\n{v}\n--- rust\n{}", rust.files[k]);
        let doc = |t: &str| serde_yaml::from_str::<serde_yaml::Value>(t).unwrap();
        assert_eq!(doc(v), doc(&rust.files[k]), "{what}: {k} as a document");
        eprintln!("compared as parsed documents: {what}: {k}");
    }
}

fn settings(root: &Path, node: &str) {
    let work = root.join("work");
    put(&work, ".stub/settings.yaml", &format!("adapter: fixture\nnodes: [{node}]\n"));
    if node == "test.reproduce" {
        put(&work, ".stub/replies/fixture-stub.yaml", &format!("{node}: [stub-tree, bugfix-pager/stub-repro, repro]\n"));
    }
}

fn on_dry(node: &str) {
    let (_, r) = pair(&format!("dry-{node}"), &start(node).with(&["--adapter", "dry"]), &nothing);
    assert_eq!(r.code, Some(0), "{node}: {r:#?}");
    assert!(r.files.contains_key(&format!(".sdc/llm/001-{node}/prompt.md")) && r.stderr.contains("would run: metaharness"), "{node}: {r:#?}");
}

fn on_fixture(node: &str) {
    let (_, r) = pair(&format!("fixture-{node}"), &start(node), &|root| settings(root, node));
    assert_eq!(r.code, Some(0), "{node}: {r:#?}");
    assert!(r.files.contains_key(&format!(".sdc/llm/001-{node}/response.json")), "{node}: {r:#?}");
}

fn on_stub(node: &str) {
    let (_, r) = pair(&format!("stub-{node}"), &start(node), &nothing);
    assert_eq!(r.code, Some(0), "{node}: {r:#?}");
    assert!(!r.files.keys().any(|k| k.starts_with(".sdc/llm/")), "{node}: the stub makes no model call: {r:#?}");
}

/// An adapter that ends well and writes nothing: a node whose output tree starts as a copy of the
/// input (spec.author, change.fix) applies it, the others have no output to apply (exit 1).
fn on_empty_output(node: &str) {
    let (_, r) = pair(&format!("true-{node}"), &start(node).with(&["--adapter", "true"]), &nothing);
    let want = if matches!(node, "spec.author" | "change.fix") { 0 } else { 1 };
    assert_eq!(r.code, Some(want), "{node}: {r:#?}");
    assert_eq!(r.stdout.contains("the output was not applied (exit 1):"), want == 1, "{node}: {r:#?}");
}

macro_rules! each_node {
    ($kind:ident) => {
        mod $kind {
            #[test]
            fn idea_dissect() {
                super::$kind("idea.dissect")
            }
            #[test]
            fn questions_write() {
                super::$kind("questions.write")
            }
            #[test]
            fn requirements_normalise() {
                super::$kind("requirements.normalise")
            }
            #[test]
            fn spec_author() {
                super::$kind("spec.author")
            }
            #[test]
            fn test_reproduce() {
                super::$kind("test.reproduce")
            }
            #[test]
            fn bug_dissect() {
                super::$kind("bug.dissect")
            }
            #[test]
            fn change_fix() {
                super::$kind("change.fix")
            }
        }
    };
}

each_node!(on_dry);
each_node!(on_fixture);
each_node!(on_stub);
each_node!(on_empty_output);

fn ticket(root: &Path) {
    put(&root.join("work"), "bug.yaml", "title: t\ntext: Invented report.\nticket:\n  id: PAGER-7\n  kind: bug\nconstraints:\n- Keep the public API of the crate unchanged.\n- Add no dependency.\n");
}

#[test]
fn a_ticket_and_its_constraints_reach_test_reproduce_the_same() {
    let (_, r) = pair("ticket-test.reproduce", &start("test.reproduce").with(&["--adapter", "dry"]), &ticket);
    assert!(r.files[".sdc/llm/001-test.reproduce/prompt.md"].contains("Add no dependency."), "{r:#?}");
}

#[test]
fn a_ticket_and_its_constraints_reach_change_fix_the_same() {
    let (_, r) = pair("ticket-change.fix", &start("change.fix").with(&["--adapter", "dry"]), &ticket);
    assert!(r.files[".sdc/llm/001-change.fix/prompt.md"].contains("Add no dependency."), "{r:#?}");
}

#[test]
fn a_ticket_reaches_bug_dissect_the_same() {
    pair("ticket-bug.dissect", &start("bug.dissect").with(&["--adapter", "dry"]), &ticket);
}

fn retry_doc(records: serde_json::Value, logs: serde_json::Value) -> String {
    json!({"format": "loom.retry-feedback/1", "case": "IDEA-1.x", "action": "x", "records": records, "logs": logs}).to_string()
}

const RETRY_ENV: [(&str, &str); 1] = [("LOOM_RETRY_FEEDBACK", "{root}/feedback-doc.json")];

fn retried(node: &str) {
    let blob = "4f".repeat(32);
    let log = "check: scope-changing question lacks a key\nq1: lacks excerpt\n";
    let failed = retry_doc(
        json!([{"evidence_id": "e1", "kind": "tests", "subject": "change", "subject_revision": "r1", "result": "fail", "producer": "tests.run", "blob_sha256": blob, "action": "tests.run", "exit": 1},
            {"evidence_id": "e2", "kind": "lint", "subject": "change", "subject_revision": "r1", "result": "invalid"}]),
        json!([{"blob_sha256": blob, "tail": log}, {"blob_sha256": "../x", "tail": "not a log"}]),
    );
    let (_, r) = pair(&format!("retry-dry-{node}"), &start(node).with(&["--adapter", "dry"]).env(&RETRY_ENV), &|root| put(root, "feedback-doc.json", &failed));
    assert_eq!(r.code, Some(0), "{node}: {r:#?}");
    let fb = &r.files[&format!(".sdc/llm/001-{node}/cwd/in/feedback.md")];
    assert!(fb.contains("q1: lacks excerpt") && fb.contains("## tests = fail"), "{node}: {fb}");
}

#[test]
fn a_retry_feedback_document_reaches_questions_write_the_same() {
    retried("questions.write");
}

#[test]
fn a_retry_feedback_document_reaches_change_fix_the_same() {
    retried("change.fix");
}

#[test]
fn a_retry_feedback_document_reaches_spec_author_the_same() {
    retried("spec.author");
}

fn person_rejection() -> String {
    let reasons = "The requirements lack the named risks.\nName the failure modes of a missing connection.\n";
    let blob = "5e".repeat(32);
    retry_doc(
        json!([{"evidence_id": "e1", "kind": "requirements_approval", "subject": "requirements", "subject_revision": "r2", "result": "rejected", "blob_sha256": blob, "action": "requirements.check", "exit": null}]),
        json!([{"blob_sha256": blob, "tail": reasons}]),
    )
}

/// The stub on a person's rejection records the reasons in the rejected artifact.
#[test]
fn a_persons_rejection_reaches_the_stub_the_same() {
    let doc = person_rejection();
    let (_, r) = pair("retry-person", &start("requirements.normalise").env(&RETRY_ENV), &|root| put(root, "feedback-doc.json", &doc));
    assert_eq!(r.code, Some(0), "{r:#?}");
    assert!(r.files["requirements.yaml"].ends_with("# rework of the rejection of requirements.check:\n# - The requirements lack the named risks.\n# - Name the failure modes of a missing connection.\n"), "{r:#?}");
}

#[test]
fn a_retry_feedback_document_of_another_format_is_a_program_error_the_same() {
    let (_, r) = pair("retry-bad", &start("change.fix").env(&RETRY_ENV), &|root| put(root, "feedback-doc.json", "{\"format\":\"other\"}"));
    assert_eq!(r.code, Some(126), "{r:#?}");
    assert!(r.stderr.contains("names no loom.retry-feedback/1 document"), "{r:#?}");
}

#[test]
fn a_retry_feedback_document_without_records_is_no_feedback_the_same() {
    let doc = json!({"format": "loom.retry-feedback/1"}).to_string();
    let (_, r) = pair("retry-no-records", &start("change.fix").with(&["--adapter", "dry"]).env(&RETRY_ENV).jq_noise(), &|root| put(root, "feedback-doc.json", &doc));
    assert_eq!(r.code, Some(0), "{r:#?}");
}

const REJECTION: &str = r#"{"format":"sdc.rejection/1","approval":"requirements.check","revision":"abc123","principal":"person:reviewer","count":2,"reasons":["The refusal of an empty title is missing.","Say who may cancel."],"subject":"requirements"}"#;

const FEEDBACK_ENV: [(&str, &str); 1] = [("SDC_FEEDBACK", "{root}/feedback.json")];

/// A rejected approval of the writer's output, as the driver's feedback file carries it.
fn rejection_files(approval: &'static str, subject: &'static str) -> impl Fn(&Path) {
    move |root: &Path| {
        put(root, "rejection.json", &REJECTION.replace("requirements.check", approval).replace("\"requirements\"", &format!("\"{subject}\"")));
        put(root, "feedback.json", &json!([{"kind": "approval", "result": "rejected", "action": approval, "exit": 6, "log": format!("{}/rejection.json", root.display())}]).to_string());
    }
}

/// On dry the writer reads it as in/rejection.json and the rework text.
fn rejected_on_dry(node: &str, approval: &'static str, subject: &'static str) {
    let (_, r) = pair(&format!("reject-dry-{node}"), &start(node).with(&["--adapter", "dry"]).env(&FEEDBACK_ENV), &rejection_files(approval, subject));
    assert_eq!(r.code, Some(0), "{node}: {r:#?}");
    assert!(r.files.contains_key(&format!(".sdc/llm/001-{node}/cwd/in/rejection.json")), "{node}: {r:#?}");
}

#[test]
fn a_rejection_reaches_requirements_normalise_the_same() {
    rejected_on_dry("requirements.normalise", "requirements.check", "requirements");
}

#[test]
fn a_rejection_reaches_spec_author_the_same() {
    rejected_on_dry("spec.author", "spec.review", "spec");
}

#[test]
fn a_rejection_reaches_change_fix_the_same() {
    rejected_on_dry("change.fix", "tests.run", "change");
}

/// On the stub the rework is recorded in the rejected artifact: a YAML file.
#[test]
fn the_stub_records_a_rejection_in_a_yaml_file_the_same() {
    let (_, r) = pair("reject-stub-yaml", &start("requirements.normalise").env(&FEEDBACK_ENV), &rejection_files("requirements.check", "requirements"));
    assert!(r.files["requirements.yaml"].contains("# rework 2 of the rejection of requirements.check at abc123:\n# - The refusal of an empty title is missing.\n# - Say who may cancel.\n"), "{r:#?}");
}

/// ... and in a tree.
#[test]
fn the_stub_records_a_rejection_in_a_tree_the_same() {
    let (_, r) = pair("reject-stub-tree", &start("spec.author").env(&FEEDBACK_ENV), &rejection_files("spec.review", "spec"));
    assert!(r.files["spec/.rework"].starts_with("rework 2 of the rejection of spec.review at abc123:\n- The refusal"), "{r:#?}");
}

/// The text of the rework note the stub appended: from its first line to the end of the file.
fn note(file: &str) -> String {
    file[file.find("# rework").expect("a rework note")..].to_string()
}

const FINDING: &str = "findings:\n  - id: f1\n    status: needs_person\n    raised_by: llm:spec.author\n  - id: f2\n    status: needs_person\n    raised_by: llm:requirements.normalise\n    decision: B\n";

const DERIVED_FINDINGS: &str = "findings:\n  - id: f1\n    status: resolved\n    raised_by: llm:spec.author\n    resolution:\n      default_taken: true\n";

fn derived_by_default(root: &Path) {
    let w = root.join("work");
    put(&w, "spec/.derived-from", "requirements 0000000000000000 resolved \n");
    put(&w, "findings.yaml", DERIVED_FINDINGS);
}

#[test]
fn a_finding_that_waits_for_a_person_stops_the_node_the_same() {
    let (_, r) = pair("findings-wait", &start("spec.author"), &|root| put(&root.join("work"), "findings.yaml", FINDING));
    assert_eq!(r.code, Some(0), "{r:#?}");
    assert_eq!(r.stdout, "llm-node: spec.author not called: findings wait for a person (f1 f2)\n");
}

#[test]
fn a_decided_finding_does_not_stop_the_node_the_same() {
    let (_, r) = pair("findings-decided", &start("spec.author"), &|root| put(&root.join("work"), "findings.yaml", "findings:\n  - id: f1\n    status: needs_person\n    decision: A\n"));
    assert!(r.files["spec/.derived-from"].starts_with("requirements "), "{r:#?}");
}

#[test]
fn a_derived_tree_is_renewed_when_every_decision_is_a_default_the_node_raised_the_same() {
    let (_, r) = pair("findings-derived", &start("spec.author").with(&["--adapter", "dry"]), &derived_by_default);
    assert_eq!(r.stdout, "llm-node: spec.author not called: the decisions since spec/ was written are the defaults it raised and follows (f1); spec/ marked as derived from the amended requirements\n");
    assert!(!r.files.keys().any(|k| k.starts_with(".sdc/llm/")), "{r:#?}");
}

#[test]
fn a_decision_of_a_person_calls_the_node_the_same() {
    let person = |root: &Path| {
        let w = root.join("work");
        put(&w, "spec/.derived-from", "requirements 0000000000000000 resolved \n");
        put(&w, "findings.yaml", &DERIVED_FINDINGS.replace("true", "false"));
    };
    let (_, r) = pair("findings-person-decision", &start("spec.author").with(&["--adapter", "dry"]), &person);
    assert!(r.files.contains_key(".sdc/llm/001-spec.author/prompt.md"), "{r:#?}");
}

#[test]
fn the_stub_raises_findings_by_rounds_the_same() {
    let rounds = |root: &Path| {
        put(
            &root.join("work"),
            ".stub/finding.yaml",
            "node: spec.author\nrounds:\n  - - type: missing_decision\n      cites: [r1]\n      says: r1 does not say whether a cancelled invoice keeps its number\n      options: [it keeps its number, its number is freed]\n      default: it keeps its number\n      scope_changing: false\n  - []\n",
        );
    };
    let (_, r) = pair("findings-rounds", &start("spec.author"), &rounds);
    assert_eq!(r.code, Some(0), "{r:#?}");
    assert!(r.files[".sdc/stub-round.spec.author"] == "1\n" && r.files.contains_key(".sdc/stub-findings.spec.author.yaml"), "{r:#?}");
}

#[test]
fn the_stub_raises_a_fixture_finding_the_same() {
    let one = |root: &Path| put(&root.join("work"), ".stub/finding.yaml", "node: requirements.normalise\nfindings:\n  - type: missing_decision\n    cites: [r1]\n    says: s\n    options: [a, b]\n    default: a\n    scope_changing: false\n");
    let (_, r) = pair("findings-fixture", &start("requirements.normalise"), &one);
    assert!(r.files.contains_key("findings.yaml"), "{r:#?}");
}

fn have_questions(root: &Path) {
    fs::copy(fx().join("questions-open.yaml"), root.join("work/questions.yaml")).unwrap();
}

#[test]
fn questions_write_is_not_called_while_questions_exist_the_same() {
    let (_, r) = pair("questions-guard", &start("questions.write"), &have_questions);
    assert_eq!(r.stdout, "llm-node: questions.write not called: questions.yaml exists and no check found it invalid\n");
}

#[test]
fn questions_write_is_called_again_when_a_check_sent_it_back_the_same() {
    let invalid = |root: &Path| {
        have_questions(root);
        put(root, "feedback.json", "[]");
    };
    let (_, r) = pair("questions-called", &start("questions.write").env(&FEEDBACK_ENV), &invalid);
    assert!(!r.stdout.contains("not called"), "{r:#?}");
}

fn adapters(root: &Path) {
    let make = |name: &str, body: &str| {
        let p = root.join("adapters").join(name);
        put(root, &format!("adapters/{name}"), &format!("#!/bin/sh\n{body}"));
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    };
    make("api-error", "printf '{\"terminal_reason\":\"api_error\",\"api_error_status\":529}\\n'\nexit 1\n");
    make("api-error-bare", "printf '{\"terminal_reason\":\"api_error\"}\\n'\nexit 1\n");
    make("exits", "printf '{\"error\":\"the model stopped\"}\\n'\nexit 3\n");
    make("silent", "exit 4\n");
}

/// A run that must end with `code`, the same by script and program; its record.
fn ends(name: &str, node: &str, extra: &[&str], code: i32) -> Run {
    let (s, r) = pair(&format!("fail-{name}"), &start(node).with(extra), &adapters);
    assert_eq!(r.code, Some(code), "{name}: {s:#?}");
    r
}

#[test]
fn an_api_error_with_a_status_ends_the_same() {
    let r = ends("api-error", "change.fix", &["--adapter", "{root}/adapters/api-error"], 75);
    assert!(r.stdout.contains("(api_error 529)"), "{r:#?}");
}

#[test]
fn an_api_error_without_a_status_ends_the_same() {
    let r = ends("api-error-bare", "change.fix", &["--adapter", "{root}/adapters/api-error-bare"], 75);
    assert!(r.stdout.contains("(api_error);"), "{r:#?}");
}

#[test]
fn an_adapter_that_exits_with_a_reason_ends_the_same() {
    let r = ends("exits", "spec.author", &["--adapter", "{root}/adapters/exits"], 3);
    assert!(r.stdout.contains("the model call exited 3: the model stopped"), "{r:#?}");
}

#[test]
fn an_adapter_that_exits_without_a_reason_ends_the_same() {
    let r = ends("silent", "idea.dissect", &["--adapter", "{root}/adapters/silent"], 4);
    assert!(r.stdout.contains("the model call exited 4: no reason given"), "{r:#?}");
}

fn refused(name: &str, extra: &[&str], code: i32, want: &str) {
    let (_, r) = pair(&format!("fail-{name}"), &start("idea.dissect").with(extra).jq_noise(), &nothing);
    assert_eq!(r.code, Some(code), "{name}: {r:#?}");
    assert!(r.stderr.contains(want), "{name}: {r:#?}");
}

#[test]
fn a_max_turns_that_is_no_number_is_refused_the_same() {
    refused("turns", &["--max-turns", "many"], 64, "--max-turns takes a number");
}

#[test]
fn a_max_budget_that_is_no_number_is_refused_the_same() {
    refused("budget", &["--max-budget-usd", "lots"], 64, "--max-budget-usd takes a number");
}

#[test]
fn instructions_that_are_no_json_object_are_refused_the_same() {
    refused("json", &["--instructions", "{not json"], 64, "--instructions takes a JSON object");
}

#[test]
fn an_unknown_argument_is_refused_the_same() {
    refused("unknown", &["--nonsense"], 64, "unknown argument --nonsense");
}

#[test]
fn an_option_without_a_value_is_refused_the_same() {
    let mut s = start("idea.dissect").with(&["--model"]);
    s.binding = false;
    let (_, r) = pair("fail-no-value", &s, &nothing);
    assert_eq!(r.code, Some(64), "{r:#?}");
    assert_eq!(r.stderr, "llm-node: --model takes a value\n");
}

#[test]
fn an_instruction_path_outside_instructions_is_refused_the_same() {
    refused("outside", &["--adapter", "dry", "--instructions", r#"{"task":"../README.md"}"#], 126, "is not instructions/<...>.md");
}

#[test]
fn a_missing_instruction_file_is_refused_the_same() {
    refused("missing", &["--adapter", "dry", "--instructions", r#"{"task":"instructions/none.md"}"#], 126, "instruction file instructions/none.md is missing");
}

#[test]
fn standard_input_that_is_no_json_object_is_refused_the_same() {
    let mut s = start("idea.dissect");
    s.stdin = "[1]".into();
    let (sh, rs) = roots("fail-stdin");
    for r in [&sh, &rs] {
        base_work(&r.join("work"), "idea.dissect");
    }
    let (a, b) = (exec(&sh, true, &s, "IDEA-1.x"), exec(&rs, false, &s, "IDEA-1.x"));
    same("stdin", &a, &b);
    assert_eq!(b.code, Some(64), "{b:#?}");
}

#[test]
fn a_stub_setting_that_does_not_exist_is_refused_the_same() {
    let (_, r) = pair("fail-setting", &start("idea.dissect"), &|root| put(&root.join("work"), ".stub/settings.yaml", "bogus: 1\n"));
    assert_eq!(r.code, Some(64), "{r:#?}");
}

#[test]
fn the_script_hands_the_seven_names_to_the_program_unless_sdc_node_shell_is_set() {
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("node-parity/dispatch");
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(base.join("bin")).unwrap();
    fs::create_dir_all(base.join("work")).unwrap();
    let fake = base.join("bin/sdc-node");
    fs::write(&fake, "#!/bin/sh\necho \"program $*\"\n").unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    let run = |shell: bool, node: &str| {
        let mut c = Command::new(repo().join("bindings/tools/llm-node.sh"));
        c.args([node, "--model", "m"]).current_dir(base.join("work")).env_clear().env("PATH", format!("{}:{}", base.join("bin").display(), path())).env("LOOM_CASE", "IDEA-1.x");
        if shell {
            c.env("SDC_NODE_SHELL", "1");
        }
        let o = c.stdin(Stdio::null()).output().unwrap();
        String::from_utf8_lossy(&o.stdout).into_owned()
    };
    for node in NODES {
        let out = run(false, node);
        assert!(out.starts_with(&format!("program {node} --tools-dir ")) && out.trim_end().ends_with("--model m"), "{node}: {out}");
    }
    assert!(!run(true, "idea.dissect").starts_with("program"), "SDC_NODE_SHELL keeps the script's code");
    assert!(!run(false, "context.default").starts_with("program"), "a node outside the seven stays in the script");
}

// ---- goldens ----------------------------------------------------------------------------------

fn compare(group: &str, name: &str, text: &str) {
    let g = repo().join("instructions/golden").join(group).join(format!("{name}.md"));
    if std::env::var("SDC_BLESS_NODE_LOG").is_ok() && group == "node-log" {
        fs::create_dir_all(g.parent().unwrap()).unwrap();
        fs::write(&g, text).unwrap();
        return;
    }
    let want = fs::read_to_string(&g).unwrap_or_else(|_| panic!("no golden {}", g.display()));
    assert!(text == want, "{group}/{name}: differs from {}\n--- got\n{text}\n--- want\n{want}", g.display());
}

/// The prompt of one Loom-mode run with the adapter `true`, by the program and by the script.
fn prompt(name: &str, node: &str, setup: &dyn Fn(&Path), env: &[(&str, &str)]) -> String {
    let (sh, rs) = roots(&format!("golden-{name}"));
    let mut s = start(node).with(&["--adapter", "true"]).env(env);
    s.stdin = r#"{"domain":"todo"}"#.into();
    let mut got = Vec::new();
    for (root, shell) in [(&rs, false), (&sh, true)] {
        base_work(&root.join("work"), node);
        setup(root);
        exec(root, shell, &s, "sdc.todo.list");
        let p = root.join(format!("work/.sdc/llm/001-{node}/prompt.md"));
        got.push(fs::read_to_string(&p).unwrap_or_else(|_| panic!("{name}: no prompt.md at {}", p.display())));
    }
    assert!(got[0] == got[1], "{name}: the script and the program wrote different prompts");
    got.remove(0)
}

fn golden(node: &str) {
    compare("llm-node", node, &prompt(node, node, &nothing, &[]));
}

#[test]
fn the_prompt_of_idea_dissect_is_its_golden_byte_for_byte() {
    golden("idea.dissect");
}

#[test]
fn the_prompt_of_questions_write_is_its_golden_byte_for_byte() {
    golden("questions.write");
}

#[test]
fn the_prompt_of_requirements_normalise_is_its_golden_byte_for_byte() {
    golden("requirements.normalise");
}

#[test]
fn the_prompt_of_spec_author_is_its_golden_byte_for_byte() {
    golden("spec.author");
}

#[test]
fn the_prompt_of_test_reproduce_is_its_golden_byte_for_byte() {
    golden("test.reproduce");
}

#[test]
fn the_prompt_of_bug_dissect_is_its_golden_byte_for_byte() {
    golden("bug.dissect");
}

#[test]
fn the_prompt_of_change_fix_is_its_golden_byte_for_byte() {
    golden("change.fix");
}

#[test]
fn the_prompt_of_change_fix_with_a_ticket_is_its_golden_byte_for_byte() {
    compare("llm-node", "change.fix.ticket", &prompt("fix-ticket", "change.fix", &ticket, &[]));
}

#[test]
fn the_prompt_of_test_reproduce_with_a_ticket_is_its_golden_byte_for_byte() {
    compare("llm-node", "test.reproduce.ticket", &prompt("repro-ticket", "test.reproduce", &ticket, &[]));
}

#[test]
fn the_prompt_of_change_fix_without_a_gate_is_its_golden_byte_for_byte() {
    let no_gate = |root: &Path| put(&root.join("work"), "checks.yaml", "format: sdc.checks/1\ntest: [cargo, test]\nrepro: [cargo, test]\ntest_file: '^tests/[a-z_]+\\.rs$'\n");
    compare("llm-node", "change.fix.no-gate", &prompt("fix-no-gate", "change.fix", &no_gate, &[]));
}

/// A check that failed on the current revision: Loom's loom.retry-feedback/1 document.
#[test]
fn the_prompt_of_change_fix_sent_back_is_its_golden_byte_for_byte() {
    let blob = "7a".repeat(32);
    let failed = retry_doc(
        json!([{"evidence_id": "e1", "kind": "tests", "subject": "change", "subject_revision": "r1", "result": "fail", "blob_sha256": blob, "action": "tests.run", "exit": 1}]),
        json!([{"blob_sha256": blob, "tail": "test result: FAILED. 1 failed\n"}]),
    );
    let doc = |root: &Path| put(root, "feedback-doc.json", &failed);
    compare("llm-node", "change.fix.feedback", &prompt("fix-feedback", "change.fix", &doc, &RETRY_ENV));
}

// ---- the step log lines -----------------------------------------------------------------------

#[test]
fn the_not_called_line_of_questions_write_is_its_golden_byte_for_byte() {
    let (_, r) = pair("log-guard", &start("questions.write"), &have_questions);
    compare("node-log", "not-called.questions", &r.stdout);
}

#[test]
fn the_not_called_line_for_waiting_findings_is_its_golden_byte_for_byte() {
    let (_, r) = pair("log-findings", &start("spec.author"), &|root| put(&root.join("work"), "findings.yaml", FINDING));
    compare("node-log", "not-called.findings", &r.stdout);
}

#[test]
fn the_not_called_line_for_a_derived_tree_is_its_golden_byte_for_byte() {
    let (_, r) = pair("log-derived", &start("spec.author").with(&["--adapter", "dry"]), &derived_by_default);
    compare("node-log", "not-called.derived", &r.stdout);
}

#[test]
fn the_not_applied_line_is_its_golden_byte_for_byte() {
    let (_, r) = pair("log-not-applied", &start("bug.dissect").with(&["--adapter", "true"]), &nothing);
    compare("node-log", "not-applied", &r.stdout);
}

#[test]
fn the_api_ended_line_is_its_golden_byte_for_byte() {
    let (_, r) = pair("log-api", &start("change.fix").with(&["--adapter", "{root}/adapters/api-error"]), &adapters);
    compare("node-log", "api-ended", &r.stdout);
}

#[test]
fn the_call_exit_line_is_its_golden_byte_for_byte() {
    let (_, r) = pair("log-exit", &start("spec.author").with(&["--adapter", "{root}/adapters/exits"]), &adapters);
    compare("node-log", "call-exit", &r.stdout);
}

#[test]
fn the_rework_note_of_the_stub_is_its_golden_byte_for_byte() {
    let (_, r) = pair("log-rework", &start("requirements.normalise").env(&FEEDBACK_ENV), &rejection_files("requirements.check", "requirements"));
    compare("node-log", "rework.stub", &note(&r.files["requirements.yaml"]));
}

#[test]
fn the_rework_note_of_the_stub_for_a_person_is_its_golden_byte_for_byte() {
    let doc = person_rejection();
    let (_, r) = pair("log-rework-person", &start("requirements.normalise").env(&RETRY_ENV), &|root| put(root, "feedback-doc.json", &doc));
    compare("node-log", "rework.stub.person", &note(&r.files["requirements.yaml"]));
}

#[test]
fn every_step_log_golden_is_read_by_a_test() {
    let names: std::collections::BTreeSet<String> = fs::read_dir(repo().join("instructions/golden/node-log"))
        .unwrap()
        .map(|e| e.unwrap().path().file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    let want = ["not-called.questions", "not-called.findings", "not-called.derived", "not-applied", "api-ended", "call-exit", "rework.stub", "rework.stub.person"];
    assert_eq!(names, want.map(String::from).into_iter().collect(), "a golden no test reads, or a log line with no golden");
}
