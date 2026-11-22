//! `sdc-node`, the Rust program for the seven plain nodes (idea.dissect, questions.write,
//! requirements.normalise, spec.author, test.reproduce, bug.dissect, change.fix) and for the nodes
//! of the planning store and of the review (vision.write, roadmap.write, plan.decompose,
//! context.default, questions.answer, change.review), against the shell code of
//! `bindings/tools/llm-node.sh` that did the same under Loom.
//!
//! The script dispatches these names to the program; `SDC_NODE_SHELL=1` (read only here) keeps
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

/// The nodes of the planning store and of the review.
const STORE_NODES: [&str; 6] = ["vision.write", "roadmap.write", "plan.decompose", "context.default", "questions.answer", "change.review"];

/// The files whose bytes may differ between the script and the program, compared as documents:
/// the stub's findings, and `context.yaml` where context.default merged the decisions of a person
/// (yq writes the merge its own way, the program writes it as a YAML document).
const AS_DOCUMENTS: [&str; 1] = [".sdc/stub-findings."];

fn as_document(key: &str) -> bool {
    key.ends_with(".yaml") && (AS_DOCUMENTS.iter().any(|p| key.starts_with(p)) || key.ends_with("context.yaml"))
}

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
    // diff's own file times (`+++ impl/x<TAB>2026-11-22 16:40:14.797898362 +0100`).
    let t = Regex::new(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d+ [+-]\d{4}").unwrap().replace_all(&t, "<time>");
    Regex::new(r#""pid":\s*\d+"#).unwrap().replace_all(&t, r#""pid":0"#).into_owned()
}

fn files(root: &Path, work: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let mut rel = p.strip_prefix(work).unwrap().display().to_string();
        // aep's lock file is named by a digest of the root it ran in.
        if rel.starts_with(".aep-planning-writer-") {
            rel = ".aep-planning-writer.lock".to_string();
        }
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
    /// The case id (LOOM_CASE): the epic and the story are its second and third part.
    case: &'a str,
}

fn start(node: &str) -> Start<'_> {
    Start { node, extra: vec![], env: vec![], stdin: r#"{"domain":"billing"}"#.to_string(), binding: true, jq_noise: false, case: "IDEA-1.x" }
}

impl<'a> Start<'a> {
    fn case(mut self, case: &'a str) -> Self {
        self.case = case;
        self
    }
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
    let mut a = exec(&sh, true, s, s.case);
    let b = exec(&rs, false, s, s.case);
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
        assert!(as_document(k), "{what}: {k} differs\n--- shell\n{v}\n--- rust\n{}", rust.files[k]);
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
    if node == "plan.decompose" {
        put(&work, ".stub/replies/fixture-stub.yaml", &format!("{node}: [plan-stories.json, stories.json]\n"));
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

/// A rejection without `reasons` leaves the header line alone as the note: no empty comment line
/// follows it (before the shell path chomped the rendered note, it appended `# `).
#[test]
fn the_stub_records_a_rejection_without_reasons_as_its_header_alone_the_same() {
    let without = |root: &Path| {
        rejection_files("requirements.check", "requirements")(root);
        put(root, "rejection.json", &REJECTION.replace(r#""reasons":["The refusal of an empty title is missing.","Say who may cancel."],"#, ""));
    };
    let (_, r) = pair("reject-stub-no-reasons", &start("requirements.normalise").env(&FEEDBACK_ENV).jq_noise(), &without);
    assert!(r.files["requirements.yaml"].ends_with("# rework 2 of the rejection of requirements.check at abc123:\n"), "{r:#?}");
    assert!(!r.files["requirements.yaml"].ends_with("# \n"), "{r:#?}");
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

/// An executable adapter program `adapters/<name>` of the root, a shell script.
fn make_adapter(root: &Path, name: &str, body: &str) {
    let p = root.join("adapters").join(name);
    put(root, &format!("adapters/{name}"), &format!("#!/bin/sh\n{body}"));
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
}

fn adapters(root: &Path) {
    let make = |name: &str, body: &str| make_adapter(root, name, body);
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
fn the_script_hands_its_thirteen_names_to_the_program_unless_sdc_node_shell_is_set() {
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
    for node in NODES.iter().chain(STORE_NODES.iter()) {
        let out = run(false, node);
        assert!(out.starts_with(&format!("program {node} --tools-dir ")) && out.trim_end().ends_with("--model m"), "{node}: {out}");
    }
    assert!(!run(true, "idea.dissect").starts_with("program"), "SDC_NODE_SHELL keeps the script's code");
    assert!(!run(false, "change.implement").starts_with("program"), "a node outside the thirteen stays in the script");
}

// ---- the nodes of the planning store ------------------------------------------------------------

/// An adapter that writes the given files into the call's out/ and answers with a response.
fn writer(root: &Path, name: &str, files: &[(&str, &str)]) {
    let mut body = String::from("cwd=$(jq -r .cwd \"$1\")\n");
    for (path, text) in files {
        body.push_str(&format!("mkdir -p \"$(dirname \"$cwd/out/{path}\")\"\nprintf '%s' '{text}' > \"$cwd/out/{path}\"\n"));
    }
    body.push_str("echo '{\"adapter\":\"test\",\"duration_ms\":1}'\n");
    make_adapter(root, name, &body);
}

const VISION_JSON: &str = r#"{"title":"Invoice billing","summary":"Clerks issue invoices.\n\nThey email them."}"#;
const VISION_MD: &str = "# Invoice billing\n\nThe body.\n";
const ROADMAP_JSON: &str = r#"{"epics":[{"name":"billing","title":"Billing","summary":"invoices and their emails"},{"name":"email","title":"Email","summary":"the mails"}]}"#;
const TWICE_JSON: &str = r#"{"epics":[{"name":"a","title":"A","summary":"s"},{"name":"a","title":"A","summary":"s"}]}"#;

fn store_adapters(root: &Path) {
    writer(root, "vision", &[("vision.json", VISION_JSON), ("vision.md", VISION_MD)]);
    writer(root, "vision-no-md", &[("vision.json", VISION_JSON)]);
    writer(root, "roadmap", &[("roadmap.json", ROADMAP_JSON), ("roadmap.md", "# Roadmap\n\nBilling first.\n")]);
    writer(root, "roadmap-no-md", &[("roadmap.json", ROADMAP_JSON)]);
    writer(root, "roadmap-empty", &[("roadmap.json", r#"{"epics":[]}"#)]);
    writer(root, "roadmap-twice", &[("roadmap.json", TWICE_JSON)]);
}

fn run_aep(work: &Path, args: &[&str]) {
    let o = Command::new("aep").args(args).args(["--root", "."]).current_dir(work).env("PATH", path()).env("AEP_ACTOR", "agent:setup").output().unwrap();
    assert!(o.status.success(), "aep {args:?}: {}", String::from_utf8_lossy(&o.stderr));
}

/// A planning store of aep's own making in work/, with the artifacts `docs` name as `aep new`
/// arguments after `plan artifact new`: the files base_work() lays there are no store.
fn fresh_store(root: &Path, docs: &[&[&str]]) {
    let work = root.join("work");
    fs::remove_dir_all(work.join("aep")).unwrap();
    for d in docs {
        let mut args = vec!["plan", "artifact", "new"];
        args.extend_from_slice(d);
        args.extend(["--store", "aep"]);
        run_aep(&work, &args);
    }
}

const VISION: &[&str] = &["vision", "product", "--title", "Old", "--summary", "old"];
const EPIC: &[&str] = &["epic", "billing", "--title", "Billing", "--summary", "invoices"];

/// A run of a store node on a test adapter that writes its output.
fn store_run(name: &str, node: &str, adapter: &str, setup: &dyn Fn(&Path)) -> Run {
    let arg = format!("{{root}}/adapters/{adapter}");
    let (_, r) = pair(name, &start(node).case("sdc.billing.list").with(&["--adapter", &arg]), &|root| {
        store_adapters(root);
        setup(root);
    });
    r
}

fn not_applied(r: &Run, node: &str, code: i32) {
    assert_eq!(r.code, Some(code), "{r:#?}");
    assert!(r.stdout.starts_with(&format!("llm-node: {node}: the output was not applied (exit {code}):\n")), "{r:#?}");
}

#[test]
fn vision_write_makes_the_vision_the_same() {
    let r = store_run("vision", "vision.write", "vision", &|root| fresh_store(root, &[]));
    assert_eq!(r.code, Some(0), "{r:#?}");
    let v = &r.files["aep/vision/product.md"];
    assert!(v.contains("title: Invoice billing") && v.contains("# Invoice billing\n\nThe body.\n"), "{r:#?}");
}

#[test]
fn vision_write_replaces_the_vision_the_same() {
    let r = store_run("vision-replace", "vision.write", "vision", &|root| fresh_store(root, &[VISION]));
    assert_eq!(r.code, Some(0), "{r:#?}");
    assert!(!r.files["aep/vision/product.md"].contains("title: Old"), "{r:#?}");
}

#[test]
fn vision_write_without_its_text_applies_nothing_the_same() {
    let r = store_run("vision-no-md", "vision.write", "vision-no-md", &|root| fresh_store(root, &[VISION]));
    not_applied(&r, "vision.write", 1);
    assert!(r.files["aep/vision/product.md"].contains("title: Old"), "{r:#?}");
}

#[test]
fn roadmap_write_makes_the_initiative_and_its_epics_the_same() {
    let r = store_run("roadmap", "roadmap.write", "roadmap", &|root| fresh_store(root, &[&["epic", "old", "--title", "Old", "--summary", "old"]]));
    assert_eq!(r.code, Some(0), "{r:#?}");
    assert!(!r.files.contains_key("aep/epic/old.md"), "the epics of the earlier roadmap go: {r:#?}");
    assert!(r.files["aep/initiative/roadmap.md"].contains("summary: 'epics: billing email'") || r.files["aep/initiative/roadmap.md"].contains("epics: billing email"), "{r:#?}");
    assert!(r.files.contains_key("aep/epic/billing.md") && r.files.contains_key("aep/epic/email.md"), "{r:#?}");
}

#[test]
fn roadmap_write_without_its_text_still_makes_the_initiative_the_same() {
    let r = store_run("roadmap-no-md", "roadmap.write", "roadmap-no-md", &|root| fresh_store(root, &[]));
    assert_eq!(r.code, Some(0), "{r:#?}");
}

#[test]
fn roadmap_write_without_epics_applies_nothing_the_same() {
    let r = store_run("roadmap-empty", "roadmap.write", "roadmap-empty", &|root| fresh_store(root, &[]));
    not_applied(&r, "roadmap.write", 1);
}

#[test]
fn roadmap_write_stops_at_an_epic_aep_refuses_the_same() {
    let r = store_run("roadmap-twice", "roadmap.write", "roadmap-twice", &|root| fresh_store(root, &[]));
    not_applied(&r, "roadmap.write", 1);
    assert!(r.files.contains_key("aep/epic/a.md"), "{r:#?}");
}

/// A run of plan.decompose on the fixture adapter, with a store the case sets up.
fn decompose(name: &str, docs: &'static [&'static [&'static str]], more: &dyn Fn(&Path)) -> Run {
    let (_, r) = pair(name, &start("plan.decompose").case("sdc.billing.list"), &|root| {
        settings(root, "plan.decompose");
        fresh_store(root, docs);
        more(root);
    });
    r
}

const STORY: &[&str] = &["story", "old", "--title", "Old", "--summary", "old", "--relate", "delivers:epic:billing", "--relate", "serves:vision:product"];

#[test]
fn plan_decompose_makes_the_stories_and_activates_the_epic_the_same() {
    let r = decompose("decompose", &[VISION, EPIC], &nothing);
    assert_eq!(r.code, Some(0), "{r:#?}");
    assert!(r.files["aep/story/invoices.md"].contains("status: active"), "{r:#?}");
    assert!(r.files["aep/epic/billing.md"].contains("status: active"), "{r:#?}");
}

#[test]
fn plan_decompose_replaces_the_stories_of_its_epic_the_same() {
    let r = decompose("decompose-replace", &[VISION, EPIC, STORY], &nothing);
    assert_eq!(r.code, Some(0), "{r:#?}");
    assert!(!r.files.contains_key("aep/story/old.md"), "{r:#?}");
}

#[test]
fn plan_decompose_leaves_an_active_epic_alone_the_same() {
    let r = decompose("decompose-active", &[VISION, EPIC], &|root| {
        run_aep(&root.join("work"), &["plan", "artifact", "move", "epic:billing", "--to", "active", "--store", "aep", "--executor", "agent:setup"]);
    });
    assert_eq!(r.code, Some(0), "{r:#?}");
}

#[test]
fn plan_decompose_without_its_epic_applies_nothing_the_same() {
    let r = decompose("decompose-no-epic", &[VISION], &nothing);
    not_applied(&r, "plan.decompose", 1);
}

#[test]
fn plan_decompose_lists_the_commands_of_the_compiled_specification_the_same() {
    let (_, r) = pair("decompose-commands", &start("plan.decompose").case("sdc.billing.list").with(&["--adapter", "dry"]), &nothing);
    assert_eq!(r.files[".sdc/llm/001-plan.decompose/cwd/in/commands.txt"], "billing.invoice.CreateInvoice\n", "{r:#?}");
}

const DECIDED: &str = "audience: {value: operators, status: changed}\nplatform: {value: [a, b], status: accepted}\nkind: {value: service, status: default}\n";
const NEW_CONTEXT: &str = "audience: {value: billing clerks, status: default}\nplatform: {value: [x], status: default, why: new}\nkind: {value: service, status: default}\nextra: {value: 1, status: default}\n";

#[test]
fn context_default_keeps_the_decisions_of_a_person_the_same() {
    let (_, r) = pair("context-merge", &start("context.default"), &|root| {
        settings(root, "context.default");
        put(&root.join("work"), "context.yaml", DECIDED);
        put(&root.join("work"), ".stub/context.yaml", NEW_CONTEXT);
    });
    assert_eq!(r.code, Some(0), "{r:#?}");
    let doc: serde_yaml::Value = serde_yaml::from_str(&r.files["context.yaml"]).unwrap();
    assert_eq!(doc["audience"]["value"], "operators", "{r:#?}");
    assert_eq!(doc["platform"]["value"], serde_yaml::from_str::<serde_yaml::Value>("[a, b]").unwrap(), "lists are replaced: {r:#?}");
    assert_eq!(doc["platform"]["why"], "new", "maps are merged: {r:#?}");
    assert_eq!(doc["extra"]["value"], 1, "{r:#?}");
}

#[test]
fn context_default_without_a_decision_takes_the_new_context_as_it_is_the_same() {
    let (_, r) = pair("context-defaults", &start("context.default"), &|root| settings(root, "context.default"));
    assert_eq!(r.code, Some(0), "{r:#?}");
    assert_eq!(r.files["context.yaml"], fs::read_to_string(fx().join("context.yaml")).unwrap(), "{r:#?}");
}

#[test]
fn context_default_without_a_context_applies_nothing_the_same() {
    let (_, r) = pair("context-none", &start("context.default").with(&["--adapter", "true"]), &nothing);
    not_applied(&r, "context.default", 1);
}

const SCOPE_Q: &str = "questions:\n  - id: q3\n    item: d5\n    question: Do invoices cover several currencies?\n    default: no\n    scope_changing: true\n    status: open\n";

#[test]
fn questions_answer_takes_the_answers_the_same() {
    let (_, r) = pair("answer", &start("questions.answer"), &|root| settings(root, "questions.answer"));
    assert_eq!(r.code, Some(0), "{r:#?}");
    assert!(r.files["questions.yaml"].contains("status: accepted"), "{r:#?}");
}

#[test]
fn questions_answer_takes_a_scope_changing_question_that_stayed_the_same() {
    let (_, r) = pair("answer-scope-kept", &start("questions.answer"), &|root| {
        settings(root, "questions.answer");
        put(&root.join("work"), "questions.yaml", SCOPE_Q);
        put(&root.join("work"), ".stub/questions-answered.yaml", &format!("{SCOPE_Q}  - id: q4\n    item: d6\n    question: q\n    default: a\n    scope_changing: false\n    status: accepted\n    answer: a\n"));
    });
    assert_eq!(r.code, Some(0), "{r:#?}");
}

#[test]
fn questions_answer_refuses_a_changed_scope_changing_question_the_same() {
    let (_, r) = pair("answer-scope-changed", &start("questions.answer"), &|root| {
        settings(root, "questions.answer");
        put(&root.join("work"), "questions.yaml", SCOPE_Q);
        put(&root.join("work"), ".stub/questions-answered.yaml", &SCOPE_Q.replace("status: open", "status: accepted\n    answer: yes"));
    });
    not_applied(&r, "questions.answer", 1);
    assert!(r.stdout.contains("questions.answer changed a scope-changing question: not applied"), "{r:#?}");
    assert!(r.files["questions.yaml"] == SCOPE_Q, "the interview stays as it was: {r:#?}");
}

#[test]
fn questions_answer_refuses_a_scope_changing_question_that_lost_a_key_the_same() {
    let (_, r) = pair("answer-scope-order", &start("questions.answer"), &|root| {
        settings(root, "questions.answer");
        put(&root.join("work"), "questions.yaml", SCOPE_Q);
        put(&root.join("work"), ".stub/questions-answered.yaml", &SCOPE_Q.replace("    status: open\n", "").replace("    default: no\n", "    default: no\n    scope_changing: true\n").replace("    scope_changing: true\n    scope_changing: true\n", "    scope_changing: true\n"));
    });
    assert_eq!(r.code, Some(1), "{r:#?}");
}

#[test]
fn questions_answer_without_an_interview_in_the_copy_applies_nothing_the_same() {
    let (_, r) = pair("answer-no-file", &start("questions.answer").with(&["--adapter", "true"]), &|root| fs::remove_file(root.join("work/questions.yaml")).unwrap());
    // The interview is the output's seed: without it the node has nothing to apply.
    assert_eq!(r.code, Some(1), "{r:#?}");
}

// ---- change.review -------------------------------------------------------------------------------

const REVIEW_SETTINGS: &str = "adapter: fixture\nnodes: [change.review]\n";
const REVIEW_REJECTED: &str = "rounds:\n  - decision: rejected\n    summary: \"fixture: the README does not say how to run it\"\n    findings:\n      - {kind: violation, at: \"impl/README.md:1\", violates: r1, says: \"fixture: the README names no command to run the service\"}\n";
const REVIEW_INVALID: &str = "rounds:\n  - decision: rejected\n    summary: fixture\n    findings:\n      - {kind: violation, at: \"impl/README.md:9999\", violates: r1, says: \"fixture: a line that does not exist\"}\n";

/// The story, and the conformance run recorded on the tree under review.
fn review_story(root: &Path) {
    let work = root.join("work");
    put(&work, "aep/story/list.md", "---\nformat: aep.planning-md/3\nid: story:list\nkind: story\nstatus: active\ntitle: List\nsummary: covers billing.invoice.CreateInvoice\n---\n# List\n");
    put(root, "conformance.log", "12 passed\n");
    let o = Command::new(env!("CARGO_BIN_EXE_sdc-reply"))
        .args(["record", "--work", ".", "--kind", "conformance_run", "--tree", "impl", "--log"])
        .arg(root.join("conformance.log"))
        .current_dir(&work)
        .output()
        .unwrap();
    assert!(o.status.success(), "sdc-reply record: {}", String::from_utf8_lossy(&o.stderr));
}

/// Adapters of the review class: they answer `--capabilities` as one that takes a response schema.
fn review_adapters(root: &Path) {
    let make = |name: &str, body: &str| {
        make_adapter(root, name, &format!("if [ \"$1\" = --capabilities ]; then echo '{{\"json_schema\": true}}'; exit 0; fi\n{body}"))
    };
    make("review-exit", "echo '{\"error\":\"the model stopped\"}'\nexit 3\n");
    make("review-api", "echo '{\"terminal_reason\":\"api_error\"}'\nexit 1\n");
    make("review-diverged", "echo '{\"terminal_reason\":\"recording_diverged\"}'\nexit 1\n");
    make("review-silent", "echo '{\"adapter\":\"test\",\"duration_ms\":1}'\n");
    make("review-garbage", "cwd=$(jq -r .cwd \"$1\")\necho 'not json' > \"$cwd/out/review.json\"\necho '{\"adapter\":\"test\",\"duration_ms\":1}'\n");
}

struct Review<'a> {
    settings: &'a str,
    replies: Option<&'a str>,
    extra: Vec<&'a str>,
    env: Vec<(&'a str, &'a str)>,
    story: bool,
}

fn review<'a>(settings: &'a str) -> Review<'a> {
    Review { settings, replies: None, extra: vec![], env: vec![], story: true }
}

impl<'a> Review<'a> {
    fn replies(mut self, r: &'a str) -> Self {
        self.replies = Some(r);
        self
    }
    fn with(mut self, extra: &[&'a str]) -> Self {
        self.extra = extra.to_vec();
        self
    }
    fn env(mut self, env: &[(&'a str, &'a str)]) -> Self {
        self.env = env.to_vec();
        self
    }
    fn run(self, name: &str) -> Run {
        let s = start("change.review").case("sdc.billing.list").with(&self.extra).env(&self.env);
        let (_, r) = pair(name, &s, &|root| {
            if self.story {
                review_story(root);
            }
            review_adapters(root);
            if !self.settings.is_empty() {
                put(&root.join("work"), ".stub/settings.yaml", self.settings);
            }
            if let Some(t) = self.replies {
                put(&root.join("work"), ".stub/replies/review.yaml", t);
            }
        });
        r
    }
}

const REC: &str = ".sdc/llm/001-change.review";

#[test]
fn change_review_approves_the_same() {
    let r = review(REVIEW_SETTINGS).run("review-approved");
    assert_eq!(r.code, Some(0), "{r:#?}");
    assert_eq!(r.stdout, "change.review approved by llm-review:claude-haiku-5-5 (0 outside-the-story note(s))\n", "{r:#?}");
    let req: serde_json::Value = serde_json::from_str(&r.files[&format!("{REC}/request.json")]).unwrap();
    assert_eq!((req["review_attempt"].clone(), req["operations"].clone()), (json!(1), json!(["dir.list", "file.read", "search"])), "{req}");
    assert!(r.files[".sdc/llm/calls.jsonl"].contains(r#""applied_exit":0"#), "{r:#?}");
}

#[test]
fn change_review_rejects_with_the_rejection_document_the_same() {
    let r = review(REVIEW_SETTINGS).replies(REVIEW_REJECTED).run("review-rejected");
    assert_eq!(r.code, Some(1), "{r:#?}");
    assert!(r.stdout.starts_with(r#"{"approval":"change.review","case":"sdc.billing.list""#) && r.stdout.contains(r#""format":"sdc.rejection/1""#), "{r:#?}");
    assert!(r.files.contains_key(".sdc/records/rejections/change.review/001.json"), "{r:#?}");
}

#[test]
fn change_review_asks_again_for_an_invalid_reply_and_records_nothing_the_same() {
    let r = review(REVIEW_SETTINGS).replies(REVIEW_INVALID).run("review-invalid");
    assert_eq!(r.code, Some(2), "{r:#?}");
    assert!(r.stdout.contains("recorded nothing after 3 attempt(s) on adapter"), "{r:#?}");
    assert!(r.files.contains_key(&format!("{REC}/response.try2.json")) && r.files.contains_key(&format!("{REC}/reply.try1.json")), "{r:#?}");
    assert!(r.files[&format!("{REC}/prompt.md")].contains("Your previous reply was refused by the program check:"), "{r:#?}");
    let req: serde_json::Value = serde_json::from_str(&r.files[&format!("{REC}/request.json")]).unwrap();
    assert_eq!(req["review_attempt"], 3, "{req}");
}

#[test]
fn the_number_of_asks_of_the_review_is_sdc_review_retries_the_same() {
    let r = review(REVIEW_SETTINGS).replies(REVIEW_INVALID).env(&[("SDC_REVIEW_RETRIES", "0")]).run("review-no-retry");
    assert_eq!(r.code, Some(2), "{r:#?}");
    assert!(r.stdout.contains("recorded nothing after 1 attempt(s)") && !r.files.contains_key(&format!("{REC}/response.try1.json")), "{r:#?}");
}

#[test]
fn change_review_on_facts_makes_one_request_without_a_tool_the_same() {
    let r = review("adapter: fixture\nnodes: [change.review]\nreview_mode: facts\n").run("review-facts");
    assert_eq!(r.code, Some(0), "{r:#?}");
    let req: serde_json::Value = serde_json::from_str(&r.files[&format!("{REC}/request.json")]).unwrap();
    assert_eq!((req["operations"].clone(), req["max_turns"].clone()), (json!([]), json!(4)), "{req}");
    assert!(r.files[".sdc/llm/calls.jsonl"].contains(r#""input_bytes":"#), "{r:#?}");
    assert!(r.files[&format!("{REC}/prompt.md")].contains("You have no tools and no"), "{r:#?}");
}

#[test]
fn change_review_on_facts_takes_a_file_cited_by_its_path_the_same() {
    let reply = "rounds:\n  - decision: rejected\n    summary: s\n    findings:\n      - {kind: violation, at: impl/README.md, violates: r1, says: the README names no command}\n";
    let r = review("adapter: fixture\nnodes: [change.review]\nreview_mode: facts\n").replies(reply).run("review-facts-rejected");
    assert_eq!(r.code, Some(1), "{r:#?}");
}

#[test]
fn change_review_on_dry_assembles_the_prompt_and_runs_the_stub_the_same() {
    let r = review("").with(&["--adapter", "dry"]).run("review-dry");
    assert_eq!(r.code, Some(0), "{r:#?}");
    assert!(r.files.contains_key(&format!("{REC}/prompt.md")), "{r:#?}");
}

#[test]
fn change_review_on_the_stub_calls_no_model_the_same() {
    let r = review("").run("review-stub");
    assert_eq!(r.code, Some(0), "{r:#?}");
    assert!(!r.files.keys().any(|k| k.starts_with(".sdc/llm/")), "{r:#?}");
}

#[test]
fn a_review_call_that_fails_records_nothing_the_same() {
    let r = review("").env(&[("SDC_REVIEW_ADAPTER", "{root}/adapters/review-exit")]).run("review-exit");
    assert_eq!(r.code, Some(2), "{r:#?}");
    assert!(r.stdout.contains("the review call failed (adapter exit 3)") && r.stdout.contains("after 1 attempt(s)"), "{r:#?}");
}

#[test]
fn a_review_whose_api_ended_exits_75_the_same() {
    let r = review("").env(&[("SDC_REVIEW_ADAPTER", "{root}/adapters/review-api")]).run("review-api");
    assert_eq!(r.code, Some(75), "{r:#?}");
    assert_eq!(r.stdout, "llm-node: change.review could not run: the model's API ended the session; nothing recorded, resume later\n", "{r:#?}");
}

#[test]
fn a_review_whose_recording_diverged_exits_75_the_same() {
    let r = review("").env(&[("SDC_REVIEW_ADAPTER", "{root}/adapters/review-diverged")]).run("review-diverged");
    assert_eq!(r.code, Some(75), "{r:#?}");
}

#[test]
fn a_review_that_wrote_no_reply_is_asked_again_and_records_nothing_the_same() {
    let r = review("").env(&[("SDC_REVIEW_ADAPTER", "{root}/adapters/review-silent")]).run("review-silent");
    assert_eq!(r.code, Some(2), "{r:#?}");
    assert!(r.stdout.contains("no readable reply (review.json: the adapter's structured output, else out/review.json)"), "{r:#?}");
}

#[test]
fn a_review_reply_that_is_no_json_is_unreadable_the_same() {
    let r = review("").env(&[("SDC_REVIEW_ADAPTER", "{root}/adapters/review-garbage")]).run("review-garbage");
    assert_eq!(r.code, Some(2), "{r:#?}");
    assert!(r.stdout.contains("no readable reply"), "{r:#?}");
}

#[test]
fn a_review_without_a_recorded_conformance_run_has_no_inputs_the_same() {
    let mut rv = review(REVIEW_SETTINGS);
    rv.story = false;
    let r = rv.run("review-no-inputs");
    assert_eq!(r.code, Some(2), "{r:#?}");
    assert!(r.stdout.starts_with("llm-node: change.review has no inputs: sdc-reply review-inputs: "), "{r:#?}");
    assert!(!r.files.keys().any(|k| k.ends_with("/request.json")), "{r:#?}");
}

#[test]
fn a_review_adapter_that_takes_no_schema_is_refused_the_same() {
    let r = review("").env(&[("SDC_REVIEW_ADAPTER", "metaharness")]).run("review-refused");
    assert_eq!(r.code, Some(126), "{r:#?}");
    assert!(r.stderr.contains("replies under a response schema, but its adapter metaharness"), "{r:#?}");
}

#[test]
fn a_review_mode_that_is_none_is_refused_the_same() {
    let r = review(REVIEW_SETTINGS).env(&[("SDC_REVIEW_MODE", "bogus")]).run("review-mode");
    assert_eq!(r.code, Some(126), "{r:#?}");
}

// ---- goldens ----------------------------------------------------------------------------------

fn compare(group: &str, name: &str, text: &str) {
    let g = repo().join("instructions/golden").join(group).join(format!("{name}.md"));
    if std::env::var("SDC_BLESS_PROMPTS").is_ok() || (std::env::var("SDC_BLESS_NODE_LOG").is_ok() && group == "node-log") {
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
    let want = [
        "not-called.questions", "not-called.findings", "not-called.derived", "not-applied", "api-ended", "call-exit", "rework.stub", "rework.stub.person",
        "review.no-inputs", "review.call-failed", "review.approved", "review.api", "review.nothing-recorded", "review.invalid", "review.unreadable",
        "answer.scope-changed",
    ];
    assert_eq!(names, want.map(String::from).into_iter().collect(), "a golden no test reads, or a log line with no golden");
}

#[test]
fn the_review_approved_line_is_its_golden_byte_for_byte() {
    let r = review(REVIEW_SETTINGS).run("log-review-approved");
    compare("node-log", "review.approved", &r.stdout);
}

#[test]
fn the_review_no_inputs_line_is_its_golden_byte_for_byte() {
    let mut rv = review(REVIEW_SETTINGS);
    rv.story = false;
    compare("node-log", "review.no-inputs", &rv.run("log-review-no-inputs").stdout);
}

#[test]
fn the_review_api_line_is_its_golden_byte_for_byte() {
    let r = review("").env(&[("SDC_REVIEW_ADAPTER", "{root}/adapters/review-api")]).run("log-review-api");
    compare("node-log", "review.api", &r.stdout);
}

#[test]
fn the_review_nothing_recorded_line_and_the_failed_call_are_their_goldens_byte_for_byte() {
    let r = review("").env(&[("SDC_REVIEW_ADAPTER", "{root}/adapters/review-exit")]).run("log-review-exit");
    compare("node-log", "review.nothing-recorded", &r.stdout);
    compare("node-log", "review.call-failed", &r.files[&format!("{REC}/review-check.err")]);
}

#[test]
fn the_review_invalid_reason_is_its_golden_byte_for_byte() {
    let r = review(REVIEW_SETTINGS).replies(REVIEW_INVALID).run("log-review-invalid");
    compare("node-log", "review.invalid", &r.files[&format!("{REC}/review-check.err")]);
}

#[test]
fn the_review_unreadable_reason_is_its_golden_byte_for_byte() {
    let r = review("").env(&[("SDC_REVIEW_ADAPTER", "{root}/adapters/review-silent")]).run("log-review-silent");
    compare("node-log", "review.unreadable", &r.files[&format!("{REC}/review-check.err")]);
}

#[test]
fn the_scope_changed_line_of_questions_answer_is_its_golden_byte_for_byte() {
    let (_, r) = pair("log-answer-scope", &start("questions.answer"), &|root| {
        settings(root, "questions.answer");
        put(&root.join("work"), "questions.yaml", SCOPE_Q);
        put(&root.join("work"), ".stub/questions-answered.yaml", &SCOPE_Q.replace("status: open", "status: accepted\n    answer: yes"));
    });
    compare("node-log", "answer.scope-changed", &r.files[".sdc/llm/001-questions.answer/apply.log"]);
}

#[test]
fn the_prompt_of_vision_write_is_its_golden_byte_for_byte() {
    golden("vision.write");
}

#[test]
fn the_prompt_of_roadmap_write_is_its_golden_byte_for_byte() {
    golden("roadmap.write");
}

#[test]
fn the_prompt_of_plan_decompose_is_its_golden_byte_for_byte() {
    golden("plan.decompose");
}

#[test]
fn the_prompt_of_context_default_is_its_golden_byte_for_byte() {
    golden("context.default");
}

#[test]
fn the_prompt_of_questions_answer_is_its_golden_byte_for_byte() {
    golden("questions.answer");
}

/// The prompt of the review, the fixture adapter as the review class's.
fn review_prompt(name: &str, more: &dyn Fn(&Path), mode: &str) -> String {
    let adapter = repo().join("bindings/tools/llm-adapter-fixture.sh").display().to_string();
    prompt(name, "change.review", &|root| {
        more(root);
        review_story(root);
    }, &[("SDC_REVIEW_ADAPTER", adapter.as_str()), ("SDC_REVIEW_MODE", mode)])
}

#[test]
fn the_prompt_of_change_review_is_its_golden_byte_for_byte() {
    compare("llm-node", "change.review", &review_prompt("review", &nothing, "session"));
}

#[test]
fn the_prompt_of_change_review_with_a_coverage_document_is_its_golden_byte_for_byte() {
    let coverage = |root: &Path| put(&root.join("work"), "impl/.sdc/coverage.json", "{}\n");
    compare("llm-node", "change.review.coverage", &review_prompt("review-coverage", &coverage, "session"));
}

#[test]
fn the_prompt_of_change_review_on_facts_carries_the_review_input_the_same() {
    // The golden of this prompt (`change.review.facts`) holds a stand-in input; here the script and
    // the program build the real one, and `prompt` asserts they agree.
    let p = review_prompt("review-facts", &nothing, "facts");
    assert!(p.contains("## Requirements this story delivers") && p.contains("### impl/tests/last_page.rs (8 lines)"), "{p}");
}
