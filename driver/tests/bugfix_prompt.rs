//! The bug-fix writers' prompts (`bindings/tools/llm-node.sh`, `bug.dissect`,
//! `test.reproduce`, `change.fix`). A run seeded without a ticket keeps its prompt byte for byte
//! (`tests/fixtures/bugfix-prompt/<node>.md`); a run seeded from a ticket (`bug.yaml` carries
//! `ticket`) drops "Standard library only" and carries the ticket's constraints.
//!
//! The adapter is `true`: nothing is called, the prompt is written before the adapter runs.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const NODES: [&str; 3] = ["bug.dissect", "test.reproduce", "change.fix"];

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dst = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &dst);
        } else {
            fs::copy(e.path(), dst).unwrap();
        }
    }
}

/// A case's work/ as it stands before `change.fix` on the pager seed, with `bug_yaml` as the
/// report; each node's prompt.md.
fn prompts(name: &str, bug_yaml: &str) -> Vec<(String, String)> {
    prompts_of("bugfix-pager", "crate", name, bug_yaml)
}

/// The same on the fixture `fixture` whose code tree is `<fixture>/<tree>`. No checks.yaml is in
/// work/: the programs take what the seed would have found (the detection table).
fn prompts_of(fixture: &str, tree: &str, name: &str, bug_yaml: &str) -> Vec<(String, String)> {
    let fx = manifest().join("examples/sdc/fixtures").join(fixture);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bugfix-prompt").join(name);
    let _ = fs::remove_dir_all(&dir);
    let repro: serde_json::Value = serde_json::from_str(&fs::read_to_string(fx.join("stub-repro/repro.json")).unwrap()).unwrap();
    let file = repro["file"].as_str().unwrap();
    let mut out = Vec::new();
    for node in NODES {
        let work = dir.join(node);
        copy_dir(&fx.join(tree), &work.join("mainline"));
        copy_dir(&fx.join(tree), &work.join("impl"));
        copy_dir(&fx.join("stub-repro"), &work.join("repro"));
        let to = work.join("impl").join(file);
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        fs::copy(fx.join("stub-repro").join(Path::new(file).file_name().unwrap()), to).unwrap();
        fs::copy(fx.join("stub-dissection.json"), work.join("bug-dissection.json")).unwrap();
        fs::write(work.join("bug.yaml"), bug_yaml).unwrap();
        let mut c = Command::new("bash");
        c.arg(manifest().join("../bindings/tools/llm-node.sh")).arg(node).current_dir(&work);
        for (k, _) in std::env::vars().filter(|(k, _)| k.starts_with("SDC_")) {
            c.env_remove(k);
        }
        let o = c
            .env("SDC_LLM_ADAPTER", "true")
            .env("SDC_LLM_NODE", llm_of(node))
            .env("SDC_BUGFIX", env!("CARGO_BIN_EXE_sdc-bugfix"))
            .output()
            .unwrap();
        let p = work.join(format!(".sdc/llm/001-{node}/prompt.md"));
        let text = fs::read_to_string(&p).unwrap_or_else(|_| {
            panic!("{node}: no prompt.md; stdout {} stderr {}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
        });
        out.push((node.to_string(), text));
    }
    out
}

#[test]
fn a_seed_run_without_a_ticket_keeps_its_prompts_byte_for_byte() {
    let bug = fs::read_to_string(manifest().join("examples/sdc/fixtures/bugfix-pager/bug.yaml")).unwrap();
    let golden = manifest().join("tests/fixtures/bugfix-prompt");
    for (node, text) in prompts("seed", &bug) {
        let g = golden.join(format!("{node}.md"));
        let want = fs::read_to_string(&g).unwrap_or_else(|_| panic!("no {}", g.display()));
        assert_eq!(text, want, "{node}: the prompt of a run without a ticket changed");
    }
}

#[test]
fn a_ticket_run_drops_standard_library_only_and_carries_the_constraints() {
    let bug = "title: The last partial page is never shown
text: Invented report.
story: story:ticket-pager-7
ticket:
  id: PAGER-7
  kind: bug
constraints:
- Keep the public API of the crate unchanged.
- Add no dependency.
";
    let p = prompts("ticket", bug);
    let get = |n: &str| p.iter().find(|(k, _)| k == n).map(|(_, t)| t.clone()).unwrap();
    let fix = get("change.fix");
    assert!(!fix.contains("Standard library only"), "{fix}");
    assert!(fix.contains("keep the public API.\n"), "{fix}");
    for node in ["test.reproduce", "change.fix"] {
        let t = get(node);
        assert!(t.contains("Constraints of the ticket (in/bug.yaml `constraints`), each one binding:\n- Keep the public API of the crate unchanged.\n- Add no dependency.\n"), "{node}: {t}");
    }
    assert!(!get("bug.dissect").contains("Constraints of the ticket"));
}

#[test]
fn a_javascript_case_s_prompts_quote_its_checks_and_name_no_rust_tool() {
    let bug = fs::read_to_string(manifest().join("examples/sdc/fixtures/bugfix-js/bug.yaml")).unwrap();
    let p = prompts_of("bugfix-js", "repo", "js", &bug);
    let get = |n: &str| p.iter().find(|(k, _)| k == n).map(|(_, t)| t.clone()).unwrap();
    for (node, text) in &p {
        for word in ["Rust", "cargo", "crate", "#[test]", "src/lib.rs", "rustfmt"] {
            assert!(!text.contains(word), "{node} names {word}: {text}");
        }
    }
    let repro = get("test.reproduce");
    assert!(repro.contains(r"^test/[a-z][a-z0-9_-]*\.test\.(c|m)?js$") && repro.contains("- in/checks.yaml\n"), "{repro}");
    assert!(repro.contains("`node --test --test-reporter=tap {file}`"), "{repro}");
    let fix = get("change.fix");
    assert!(fix.contains("  - test: `node --test --test-reporter=tap`\n"), "{fix}");
    assert!(fix.contains("in/impl/test/last_page.test.js") || fix.contains("out/impl/test/last_page.test.js"), "{fix}");
}

// ---------------------------------------------------------------------------------------------
// The instructions of every agent node and of the decider are files under `instructions/`
// (story:instructions-as-files). The prompt each one makes on stub inputs is compared with the
// committed golden under `instructions/golden/`; SDC_BLESS_PROMPTS=1 rewrites the goldens.
// A stand-in `sdc-reply` answers the few calls the tools make before the prompt is written.

const STUB_REPLY: &str = r#"#!/usr/bin/env bash
sub=$1; shift
cwd="" ; schema=no
while [[ $# -gt 0 ]]; do case $1 in --cwd) cwd=$2; shift 2 ;; --schema) schema=yes; shift ;; *) shift ;; esac; done
case $sub in
  adapters) echo '{"review":{"adapter":"true"},"implement":{"adapter":"true"}}' ;;
  review-inputs)
    mkdir -p "$cwd/in"; printf '# Review input: story `list`\n\nstub review input\n' > "$cwd/in/review.md"
    echo "{\"base\":\"mainline\",\"document_bytes\":42,\"coverage\":${STUB_COVERAGE:-false}}" ;;
  schema) echo '{"type":"object"}' ;;
  coverage) if [[ $schema == yes ]]; then echo '{"type":"object","properties":{"requirements":{"type":"array"}}}'; else echo '{"requirements":[{"id":"r1","text":"t"}]}'; fi ;;
  check-review) echo '{"notes":[]}' ;;
  citable) echo '{"artifacts":[],"examples":{"id":"r1","entry":"requirements@1#r1","line":"idea.yaml:1"}}' ;;
  rejections) if [[ -n ${STUB_PRIOR:-} ]]; then echo '[{"revision":"a","reasons":["x"]}]'; else echo '[]'; fi ;;
  later) echo '{"action":"release.publish","claims":["health.required"],"obligations":["o1"]}' ;;
  *) exit 0 ;;
esac
"#;

fn repo() -> PathBuf {
    manifest().parent().unwrap().to_path_buf()
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("instructions").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn stub_reply(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let p = dir.join("sdc-reply-stub");
    fs::write(&p, STUB_REPLY).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    p
}

fn sdc_env_cleared(c: &mut Command) {
    for (k, _) in std::env::vars().filter(|(k, _)| k.starts_with("SDC_") || k.starts_with("STUB_")) {
        c.env_remove(k);
    }
}

/// The binding's `llm` object of an action, as sdc-drive hands it to the tool (SDC_LLM_NODE).
fn llm_of(node: &str) -> String {
    let dir = manifest().join("examples/sdc");
    for e in fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if !p.to_string_lossy().ends_with(".bindings.json") {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        if let Some(l) = v.get(node).and_then(|b| b.get("llm")) {
            return l.to_string();
        }
    }
    panic!("no binding with llm for {node}");
}

fn compare(group: &str, name: &str, text: &str) {
    let g = repo().join("instructions/golden").join(group).join(format!("{name}.md"));
    if std::env::var("SDC_BLESS_PROMPTS").is_ok() {
        fs::create_dir_all(g.parent().unwrap()).unwrap();
        fs::write(&g, text).unwrap();
        return;
    }
    let want = fs::read_to_string(&g).unwrap_or_else(|_| panic!("no golden {}", g.display()));
    assert!(text == want, "{group}/{name}: the prompt differs from {}\n--- got\n{text}\n--- want\n{want}", g.display());
}

/// One work/ for every agent node: the intake and direction files, the specification, the plan, a
/// story, the code tree and the bug files.
fn node_work(name: &str) -> (PathBuf, PathBuf) {
    let dir = scratch(name);
    let work = dir.join("work");
    let fx = manifest().join("examples/sdc/fixtures");
    let pager = fx.join("bugfix-pager");
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
        fs::copy(fx.join(from), work.join(to)).unwrap();
    }
    copy_dir(&fx.join("billing-spec"), &work.join("spec"));
    for (p, t) in [("aep/vision/product.md", "# vision\n"), ("aep/initiative/roadmap.md", "# roadmap\n"),
        ("aep/story/list.md", "---\nid: story:list\n---\n# list\n"), (".sdc/gate/ir.json", r#"{"commands":{"billing.invoice.CreateInvoice":{},"billing.invoice.PayInvoice":{}}}"#)] {
        let q = work.join(p);
        fs::create_dir_all(q.parent().unwrap()).unwrap();
        fs::write(q, t).unwrap();
    }
    (dir, work)
}

fn run_node(case: &str, node: &str, setup: impl Fn(&Path, &Path), env: &[(&str, &str)]) -> String {
    let (dir, work) = node_work(case);
    let reply = stub_reply(&dir);
    setup(&dir, &work);
    if node == "questions.write" {
        // The interview is not rewritten while one exists: the node runs on a work/ without it.
        let _ = fs::remove_file(work.join("questions.yaml"));
    }
    let mut c = Command::new("bash");
    c.arg(manifest().join("../bindings/tools/llm-node.sh")).arg(node).current_dir(&work);
    sdc_env_cleared(&mut c);
    let o = c
        .env("SDC_LLM_ADAPTER", "true")
        .env("SDC_LLM_NODE", llm_of(node))
        .env("SDC_CASE", "sdc.todo.list")
        .env("SDC_ARGS", r#"{"domain":"todo"}"#)
        .env("SDC_REPLY", &reply)
        .env("SDC_BUGFIX", env!("CARGO_BIN_EXE_sdc-bugfix"))
        .envs(env.iter().copied())
        .output()
        .unwrap();
    let p = work.join(format!(".sdc/llm/001-{node}/prompt.md"));
    fs::read_to_string(&p).unwrap_or_else(|_| {
        panic!("{case}: no prompt.md; stdout {} stderr {}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
    })
}

const AGENT_NODES: [&str; 14] = [
    "idea.dissect", "questions.write", "questions.answer", "requirements.normalise", "context.default", "vision.write",
    "roadmap.write", "spec.author", "plan.decompose", "change.implement", "change.review", "bug.dissect", "test.reproduce",
    "change.fix",
];

#[test]
fn the_prompt_of_each_agent_node_is_its_golden_byte_for_byte() {
    let none = |_: &Path, _: &Path| {};
    for node in AGENT_NODES {
        compare("llm-node", node, &run_node(node, node, none, &[]));
    }
    compare("llm-node", "change.implement.coverage", &run_node("impl-cov", "change.implement", none, &[("SDC_IMPLEMENT_COVERAGE", "on")]));
    compare("llm-node", "change.review.facts", &run_node("review-facts", "change.review", none, &[("SDC_REVIEW_MODE", "facts")]));
    compare("llm-node", "change.review.coverage", &run_node("review-cov", "change.review", none, &[("STUB_COVERAGE", "true")]));
    let ticket = |_: &Path, w: &Path| {
        fs::write(w.join("bug.yaml"), "title: t\ntext: Invented report.\nticket:\n  id: PAGER-7\n  kind: bug\nconstraints:\n- Keep the public API of the crate unchanged.\n- Add no dependency.\n").unwrap();
    };
    compare("llm-node", "change.fix.ticket", &run_node("fix-ticket", "change.fix", ticket, &[]));
    compare("llm-node", "test.reproduce.ticket", &run_node("repro-ticket", "test.reproduce", ticket, &[]));
    // A rejected approval of the writer's output: the rework text, with and without the coverage document.
    let rejected = |d: &Path, _: &Path| {
        let rej = d.join("rejection.json");
        fs::write(&rej, r#"{"format":"sdc.rejection/1","approval":"change.approve","revision":"abc123","principal":"person:reviewer","count":1,"reasons":["The refusal of an empty title is missing."],"subject":"change"}"#).unwrap();
        fs::write(d.join("feedback.json"), format!(r#"[{{"kind":"approval","result":"rejected","action":"change.approve","exit":6,"log":"{}"}}]"#, rej.display())).unwrap();
    };
    let fb = |case: &str| format!("{}/instructions/{case}/feedback.json", env!("CARGO_TARGET_TMPDIR"));
    compare("llm-node", "change.implement.rework", &run_node("impl-rework", "change.implement", rejected, &[("SDC_FEEDBACK", &fb("impl-rework"))]));
    compare("llm-node", "change.implement.rework-coverage", &run_node("impl-rework-cov", "change.implement", rejected,
        &[("SDC_FEEDBACK", &fb("impl-rework-cov")), ("SDC_IMPLEMENT_COVERAGE", "on")]));
    // A check that failed on the current revision: in/feedback.md is one more input.
    let failed = |d: &Path, _: &Path| {
        let log = d.join("check.log");
        fs::write(&log, "test result: FAILED. 1 failed\n").unwrap();
        fs::write(d.join("feedback.json"), format!(r#"[{{"kind":"tests","result":"fail","action":"tests.run","exit":1,"log":"{}"}}]"#, log.display())).unwrap();
    };
    compare("llm-node", "change.fix.feedback", &run_node("fix-feedback", "change.fix", failed, &[("SDC_FEEDBACK", &fb("fix-feedback"))]));
}

fn run_decider(case: &str, kind: &str, request: serde_json::Value, env: &[(&str, &str)]) -> String {
    let dir = scratch(case);
    let reply = stub_reply(&dir);
    let case_dir = dir.join("case");
    let work = case_dir.join("work");
    let fx = manifest().join("examples/sdc/fixtures");
    fs::create_dir_all(work.join("aep/vision")).unwrap();
    fs::create_dir_all(work.join("aep/initiative")).unwrap();
    for f in ["idea.yaml", "dissection.yaml", "requirements.yaml", "context.yaml"] {
        fs::copy(fx.join(f), work.join(f)).unwrap();
    }
    fs::copy(fx.join("questions-open.yaml"), work.join("questions.yaml")).unwrap();
    fs::write(work.join("aep/vision/product.md"), "# vision\n").unwrap();
    fs::write(work.join("aep/initiative/roadmap.md"), "# roadmap\n").unwrap();
    fs::write(case_dir.join("case.json"), r#"{"id":"IDEA-1.direction","artifacts":{"goals":{"revision":"g1"}}}"#).unwrap();
    fs::write(case_dir.join("ir.json"), r#"{"actions":{"roadmap.approve":{}},"outcomes":{}}"#).unwrap();
    let rec = dir.join("rec");
    let mut req = request;
    req["case_dir"] = serde_json::json!(case_dir);
    req["record_dir"] = serde_json::json!(rec);
    fs::write(dir.join("request.json"), req.to_string()).unwrap();
    let mut c = Command::new("bash");
    c.arg(manifest().join("../bindings/tools/decider.sh")).arg(kind).arg(dir.join("request.json")).current_dir(&dir);
    sdc_env_cleared(&mut c);
    let o = c.env("SDC_DECIDER_ADAPTER", "true").env("SDC_REPLY", &reply).envs(env.iter().copied()).output().unwrap();
    fs::read_to_string(rec.join("prompt.md")).unwrap_or_else(|_| {
        panic!("{case}: no prompt.md; stdout {} stderr {}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
    })
}

#[test]
fn the_prompt_of_each_decider_is_its_golden_byte_for_byte() {
    use serde_json::json;
    let position = json!({"claims": {"roadmap.required": {"value": "false"}, "health.required": {"value": "unknown"}}, "open_obligations": ["roadmap.required", "o1"], "legitimate_outcomes": []});
    let approval = json!({"type": "approval", "action": "roadmap.approve", "capability": "approve", "subject": "roadmap", "path": "aep/initiative/roadmap.md", "revision": "r7", "position": position});
    let question = json!({"type": "question", "action": "scope.answer", "path": "questions.yaml", "position": position,
        "question": {"id": "q1", "item": "d3", "question": "Which states does an invoice go through?", "options": ["a", "b"], "default": "a", "status": "open"}});
    let finding = |sides: bool| {
        let mut f = json!({"id": "f1", "says": "Two readings.", "options": ["x", "y"], "default": "x", "cites": ["r1"]});
        if sides {
            f["sides"] = json!(["r1", "r2"]);
        }
        json!({"type": "finding", "action": "amend.upstream", "position": position, "finding": f})
    };
    let close = json!({"type": "close", "action": "close", "outcome": "delivered", "position": position});
    let with = |mut v: serde_json::Value, k: &str, x: serde_json::Value| {
        v[k] = x;
        v
    };
    let fin = [("SDC_DECIDER_ESCALATE", "final")];
    let cases: Vec<(&str, &str, serde_json::Value, Vec<(&str, &str)>)> = vec![
        ("approval", "model", approval.clone(), vec![]),
        ("approval-final", "model", approval.clone(), fin.to_vec()),
        ("approval-prior-rejection", "model", with(approval.clone(), "retry", json!({"attempt": 2, "invalid_reason": "cites what does not resolve"})), vec![("STUB_PRIOR", "1")]),
        ("approval-asked-again", "model", with(with(approval.clone(), "note", json!("The run stopped here.")), "refusal", json!({"why": "the check refused it"})), vec![]),
        ("question", "model", question.clone(), vec![]),
        ("question-final", "model", question.clone(), fin.to_vec()),
        ("question-answered", "model", with(question.clone(), "question", json!({"id": "q1", "item": "d3", "question": "Q?", "options": ["a"], "default": "a", "status": "accepted", "answer": "a"})), vec![]),
        ("finding", "model", finding(false), vec![]),
        ("finding-final", "model", finding(false), fin.to_vec()),
        ("finding-rule", "model", finding(true), vec![]),
        ("close", "model", close.clone(), vec![]),
        ("close-final", "model", close.clone(), fin.to_vec()),
        ("requester-approval", "requester", approval.clone(), vec![]),
        ("requester-approval-retry", "requester", with(approval.clone(), "retry", json!({"attempt": 3, "invalid_reason": "no ground"})), vec![]),
    ];
    for (name, kind, req, env) in cases {
        compare("decider", name, &run_decider(name, kind, req, &env));
    }
}

#[test]
fn every_agent_binding_names_its_instruction_file_and_the_tools_hold_no_instruction_text() {
    let root = repo();
    for node in AGENT_NODES {
        let l: serde_json::Value = serde_json::from_str(&llm_of(node)).unwrap();
        let named = l["instructions"].as_object().unwrap_or_else(|| panic!("{node}: the llm object names no `instructions`"));
        for (part, path) in named {
            let path = path.as_str().unwrap();
            assert!(root.join(path).is_file(), "{node}: instructions.{part} names {path}, which is not a file");
        }
        assert!(named.contains_key("task"), "{node}: instructions has no `task`");
    }
    let mut lines = Vec::new();
    let mut stack = vec![root.join("instructions")];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                if p.file_name().unwrap() != "golden" {
                    stack.push(p);
                }
            } else {
                lines.extend(fs::read_to_string(&p).unwrap().lines().map(str::trim).filter(|l| l.len() >= 30).map(String::from).collect::<Vec<_>>());
            }
        }
    }
    assert!(lines.len() > 100, "instructions/ holds too little text: {} long lines", lines.len());
    for tool in ["llm-node.sh", "decider.sh"] {
        let sh = fs::read_to_string(manifest().join("../bindings/tools").join(tool)).unwrap();
        for l in &lines {
            assert!(!sh.contains(l.as_str()), "{tool} still holds instruction text: {l}");
        }
    }
}
