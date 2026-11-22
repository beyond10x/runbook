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
        for (k, _) in std::env::vars().filter(|(k, _)| k.starts_with("SDC_") || k.starts_with("LOOM_")) {
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
  check-review) if [[ -n ${STUB_REVIEW_FAIL:-} ]]; then echo "the reply names no decision" >&2; exit 2; fi; echo '{"notes":[]}' ;;
  check-coverage) if [[ -n ${STUB_COVERAGE_FAIL:-} ]]; then echo "requirement r1 is not answered" >&2; exit 2; fi ;;
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
    for (k, _) in std::env::vars().filter(|(k, _)| k.starts_with("SDC_") || k.starts_with("STUB_") || k.starts_with("LOOM_")) {
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
    let work = run_node_in(case, node, setup, env);
    let p = work.join(format!(".sdc/llm/001-{node}/prompt.md"));
    fs::read_to_string(&p).unwrap_or_else(|_| panic!("{case}: no prompt.md at {}", p.display()))
}

/// Runs the node and returns its work/: the record of the call is under `.sdc/llm/001-<node>/`.
fn run_node_in(case: &str, node: &str, setup: impl Fn(&Path, &Path), env: &[(&str, &str)]) -> PathBuf {
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
    assert!(p.is_file(), "{case}: no prompt.md; stdout {} stderr {}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    work
}

const AGENT_NODES: [&str; 14] = [
    "idea.dissect", "questions.write", "questions.answer", "requirements.normalise", "context.default", "vision.write",
    "roadmap.write", "spec.author", "plan.decompose", "change.implement", "change.review", "bug.dissect", "test.reproduce",
    "change.fix",
];

#[test]
fn the_prompt_of_each_agent_node_is_its_golden_byte_for_byte() {
    let none = |_: &Path, _: &Path| {};
    let mut seen: Vec<String> = Vec::new();
    let mut cmp = |name: &str, text: &str| {
        compare("llm-node", name, text);
        seen.push(name.to_string());
    };
    for node in AGENT_NODES {
        cmp(node, &run_node(node, node, none, &[]));
    }
    cmp("change.implement.coverage", &run_node("impl-cov", "change.implement", none, &[("SDC_IMPLEMENT_COVERAGE", "on")]));
    cmp("change.review.facts", &run_node("review-facts", "change.review", none, &[("SDC_REVIEW_MODE", "facts")]));
    cmp("change.review.coverage", &run_node("review-cov", "change.review", none, &[("STUB_COVERAGE", "true")]));
    let ticket = |_: &Path, w: &Path| {
        fs::write(w.join("bug.yaml"), "title: t\ntext: Invented report.\nticket:\n  id: PAGER-7\n  kind: bug\nconstraints:\n- Keep the public API of the crate unchanged.\n- Add no dependency.\n").unwrap();
    };
    cmp("change.fix.ticket", &run_node("fix-ticket", "change.fix", ticket, &[]));
    cmp("test.reproduce.ticket", &run_node("repro-ticket", "test.reproduce", ticket, &[]));
    // A rejected approval of the writer's output: the rework text, with and without the coverage document.
    let rejected = |d: &Path, _: &Path| {
        let rej = d.join("rejection.json");
        fs::write(&rej, r#"{"format":"sdc.rejection/1","approval":"change.approve","revision":"abc123","principal":"person:reviewer","count":1,"reasons":["The refusal of an empty title is missing."],"subject":"change"}"#).unwrap();
        fs::write(d.join("feedback.json"), format!(r#"[{{"kind":"approval","result":"rejected","action":"change.approve","exit":6,"log":"{}"}}]"#, rej.display())).unwrap();
    };
    let fb = |case: &str| format!("{}/instructions/{case}/feedback.json", env!("CARGO_TARGET_TMPDIR"));
    cmp("change.implement.rework", &run_node("impl-rework", "change.implement", rejected, &[("SDC_FEEDBACK", &fb("impl-rework"))]));
    cmp("change.implement.rework-coverage", &run_node("impl-rework-cov", "change.implement", rejected,
        &[("SDC_FEEDBACK", &fb("impl-rework-cov")), ("SDC_IMPLEMENT_COVERAGE", "on")]));
    // A check that failed on the current revision: in/feedback.md is one more input.
    let failed = |d: &Path, _: &Path| {
        let log = d.join("check.log");
        fs::write(&log, "test result: FAILED. 1 failed\n").unwrap();
        fs::write(d.join("feedback.json"), format!(r#"[{{"kind":"tests","result":"fail","action":"tests.run","exit":1,"log":"{}"}}]"#, log.display())).unwrap();
    };
    cmp("change.fix.feedback", &run_node("fix-feedback", "change.fix", failed, &[("SDC_FEEDBACK", &fb("fix-feedback"))]));
    // The retry texts: a review reply and a coverage document the program check refused, asked again.
    cmp("change.review.retry", &run_node("review-retry", "change.review", none, &[("STUB_REVIEW_FAIL", "1"), ("SDC_REVIEW_RETRIES", "1")]));
    cmp("change.implement.coverage-retry", &run_node("impl-cov-retry", "change.implement", none,
        &[("SDC_IMPLEMENT_COVERAGE", "on"), ("STUB_COVERAGE_FAIL", "1"), ("SDC_COVERAGE_RETRIES", "1")]));
    // The writer's own last run failed: its log is the feedback, and the note names why.
    let own_failure = |d: &Path, _: &Path| {
        let case_dir = d.join("case");
        fs::create_dir_all(case_dir.join("blobs")).unwrap();
        fs::write(case_dir.join("blobs/run1"), "error: the writer stopped\n").unwrap();
        fs::write(case_dir.join("events.jsonl"), "{\"event\":\"action_finished\",\"action\":\"change.fix\",\"exit\":1,\"log\":\"run1\"}\n").unwrap();
    };
    let case_dir = format!("{}/instructions/fix-own-failure/case", env!("CARGO_TARGET_TMPDIR"));
    cmp("change.fix.feedback-run", &run_node("fix-own-failure", "change.fix", own_failure, &[("SDC_CASE_DIR", &case_dir)]));
    // A case whose checks.yaml names no gate command.
    let no_gate = |_: &Path, w: &Path| {
        fs::write(w.join("checks.yaml"), "format: sdc.checks/1\ntest: [cargo, test]\nrepro: [cargo, test]\ntest_file: '^tests/[a-z_]+\\.rs$'\n").unwrap();
    };
    cmp("change.fix.no-gate", &run_node("fix-no-gate", "change.fix", no_gate, &[]));
    if std::env::var("SDC_BLESS_PROMPTS").is_err() {
        assert_eq!(seen.iter().cloned().collect::<std::collections::BTreeSet<_>>(), golden_names("llm-node"), "a golden no test reads, or a prompt with no golden");
    }
}

/// The names of the goldens of one group on disk.
fn golden_names(group: &str) -> std::collections::BTreeSet<String> {
    fs::read_dir(repo().join("instructions/golden").join(group))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().trim_end_matches(".md").to_string())
        .collect()
}

fn run_decider(case: &str, kind: &str, request: serde_json::Value, env: &[(&str, &str)]) -> String {
    let rec = run_decider_in(case, kind, request, env);
    fs::read_to_string(rec.join("prompt.md")).unwrap()
}

/// Runs the decider and returns the record directory; the requester's `in/` is `<record>/cwd/in/`.
fn run_decider_in(case: &str, kind: &str, request: serde_json::Value, env: &[(&str, &str)]) -> PathBuf {
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
    assert!(
        rec.join("prompt.md").is_file(),
        "{case}: no prompt.md; stdout {} stderr {}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    rec
}

/// A decider case: its golden name, the kind of decider, the request and the environment.
type DeciderCase<'a> = (&'a str, &'a str, serde_json::Value, Vec<(&'a str, &'a str)>);

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
    let cases: Vec<DeciderCase> = vec![
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
    let seen: std::collections::BTreeSet<String> = cases.iter().map(|c| c.0.to_string()).collect();
    for (name, kind, req, env) in cases {
        compare("decider", name, &run_decider(name, kind, req, &env));
    }
    if std::env::var("SDC_BLESS_PROMPTS").is_err() {
        assert_eq!(seen, golden_names("decider"), "a golden no test reads, or a prompt with no golden");
    }
}

/// What the two tools write for the model besides the prompt: `in/feedback.md` of a writer sent
/// back (the headings, the fences and the line for a check with no output) and the requester's
/// `in/request.json` for an approval and a close (its question and option texts). Held byte for
/// byte against `instructions/golden/feedback/` and `instructions/golden/requester-request/`.
#[test]
fn the_feedback_file_and_the_requester_request_are_their_golden_byte_for_byte() {
    use serde_json::json;
    let tmp = env!("CARGO_TARGET_TMPDIR");
    let feedback_of = |work: &Path| fs::read_to_string(work.join(".sdc/llm/001-change.fix/cwd/in/feedback.md")).unwrap();
    let long_log: String = (1..=90).map(|i| format!("slot: job {i} queued\nrunning line {i}\n")).collect();
    let checks = |d: &Path| {
        let rej = d.join("rejection.json");
        fs::write(&rej, r#"{"format":"sdc.rejection/1","approval":"change.approve","revision":"abc123","principal":"person:reviewer","count":1,"reasons":["The refusal of an empty title is missing."],"subject":"change"}"#).unwrap();
        fs::write(d.join("tests.log"), &long_log).unwrap();
        fs::write(d.join("quiet.log"), "slot: job 3 queued\nslot: job 3 running\n").unwrap();
        let records = json!([
            {"kind": "tests", "result": "fail", "action": "tests.run", "exit": 1, "log": d.join("tests.log")},
            {"kind": "approval", "result": "rejected", "action": "change.approve", "exit": 6, "log": rej},
            {"kind": "lint", "result": "invalid", "action": null, "exit": null, "log": d.join("quiet.log")},
            {"kind": "gate", "result": "fail", "action": "gate.run", "exit": 2, "log": d.join("missing.log")},
        ]);
        fs::write(d.join("feedback.json"), records.to_string()).unwrap();
    };
    let own_failure = |d: &Path| {
        let case_dir = d.join("case");
        fs::create_dir_all(case_dir.join("blobs")).unwrap();
        let log: String = (1..=100).map(|i| format!("slot: job {i} queued\nwriter line {i}\n")).collect();
        fs::write(case_dir.join("blobs/run1"), log).unwrap();
        fs::write(case_dir.join("events.jsonl"), "{\"event\":\"action_finished\",\"action\":\"change.fix\",\"exit\":3,\"log\":\"run1\"}\n").unwrap();
    };
    let fb = |case: &str| format!("{tmp}/instructions/{case}/feedback.json");
    let cd = |case: &str| format!("{tmp}/instructions/{case}/case");
    compare(
        "feedback",
        "checks",
        &feedback_of(&run_node_in("fb-checks", "change.fix", |d, _| checks(d), &[("SDC_FEEDBACK", &fb("fb-checks"))])),
    );
    compare(
        "feedback",
        "own-failure",
        &feedback_of(&run_node_in("fb-own", "change.fix", |d, _| own_failure(d), &[("SDC_CASE_DIR", &cd("fb-own"))])),
    );
    compare(
        "feedback",
        "checks-and-own-failure",
        &feedback_of(&run_node_in(
            "fb-both",
            "change.fix",
            |d, _| {
                checks(d);
                own_failure(d);
            },
            &[("SDC_FEEDBACK", &fb("fb-both")), ("SDC_CASE_DIR", &cd("fb-both"))],
        )),
    );
    let position = json!({"claims": {}, "open_obligations": [], "legitimate_outcomes": []});
    let approval = json!({"type": "approval", "action": "roadmap.approve", "capability": "approve", "subject": "roadmap", "path": "aep/initiative/roadmap.md", "revision": "r7", "class": "approval", "position": position});
    let close = json!({"type": "close", "action": "close", "outcome": "delivered", "class": "close", "position": position});
    let bare = json!({"type": "approval", "action": "plan.approve", "subject": "plan", "position": position});
    for (name, req) in [("approval", approval), ("approval-no-revision", bare), ("close", close)] {
        let rec = run_decider_in(&format!("req-{name}"), "requester", req, &[]);
        compare("requester-request", name, &fs::read_to_string(rec.join("cwd/in/request.json")).unwrap());
    }
    if std::env::var("SDC_BLESS_PROMPTS").is_err() {
        assert_eq!(golden_names("feedback"), ["checks", "checks-and-own-failure", "own-failure"].map(String::from).into_iter().collect());
        assert_eq!(golden_names("requester-request"), ["approval", "approval-no-revision", "close"].map(String::from).into_iter().collect());
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

/// Every instruction file is reached by a golden: each line of its text outside the `{{name}}`
/// placeholders (a line of 12 characters or more) is in some golden prompt.
#[test]
fn every_instruction_file_is_reached_by_a_golden() {
    let root = repo().join("instructions");
    let mut goldens = String::new();
    for group in [
        "llm-node",
        "decider",
        "implement",
        "conform",
        "unstick",
        "feedback",
        "requester-request",
    ] {
        for e in fs::read_dir(root.join("golden").join(group)).unwrap() {
            goldens.push_str(&fs::read_to_string(e.unwrap().path()).unwrap());
        }
    }
    let placeholder = regex::Regex::new(r"\{\{[a-z_]+\}\}").unwrap();
    let mut stack = vec![root.clone()];
    let mut unreached = Vec::new();
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                if p.file_name().unwrap() != "golden" {
                    stack.push(p);
                }
                continue;
            }
            let text = fs::read_to_string(&p).unwrap();
            let missing: Vec<&str> = placeholder
                .split(&text)
                .flat_map(str::lines)
                .map(str::trim)
                .filter(|l| l.len() >= 12 && !goldens.contains(l))
                .collect();
            if !missing.is_empty() {
                unreached.push(format!("{}: {:?}", p.strip_prefix(&root).unwrap().display(), missing));
            }
        }
    }
    assert!(unreached.is_empty(), "instruction files no golden reaches:\n{}", unreached.join("\n"));
}

/// A binding or `--instructions` value that leaves `instructions/<...>.md` is refused (exit 126)
/// and the file is not read.
#[test]
fn an_instruction_path_outside_instructions_is_refused() {
    for (i, path) in ["../../x.md", "/etc/hostname.md", "instructions/../README.md", "instructions/idea.dissect.txt"].iter().enumerate() {
        let dir = scratch(&format!("path-{i}"));
        let work = dir.join("work");
        fs::create_dir_all(&work).unwrap();
        let mut c = Command::new("bash");
        c.arg(manifest().join("../bindings/tools/llm-node.sh")).arg("idea.dissect").current_dir(&work);
        sdc_env_cleared(&mut c);
        let node = serde_json::json!({"instructions": {"task": path}, "stub": ["stub-file", "dissection.yaml", "dissection.yaml"]});
        let o = c.env("SDC_LLM_ADAPTER", "true").env("SDC_LLM_NODE", node.to_string()).output().unwrap();
        let err = String::from_utf8_lossy(&o.stderr);
        assert_eq!(o.status.code(), Some(126), "{path}: stderr {err}");
        assert!(err.contains(path) && err.contains("instructions/<...>.md"), "{path}: stderr {err}");
    }
}

// ---------------------------------------------------------------------------------------------
// The model instructions of the three Rust binaries (story:rust-prompts-as-instruction-files):
// `sdc-reply implement-run` (SDC_IMPLEMENT_MODE=request), `sdc-conform` (the table node) and
// `sdc-unstick`. No model is called: `implement-run --assemble-only` and an adapter script write
// the prompts, `sdc-unstick prompt` and `sdc-conform prompt` print theirs.

fn bin_out(bin: &str, args: &[&str]) -> String {
    let o = Command::new(bin).args(args).output().unwrap();
    assert!(o.status.success(), "{bin} {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap()
}

fn names(of: &[&str]) -> std::collections::BTreeSet<String> {
    of.iter().map(|s| s.to_string()).collect()
}

#[test]
fn the_prompt_of_sdc_unstick_is_its_golden_byte_for_byte() {
    let dir = scratch("unstick");
    for (name, f) in [("stop.json", r#"{"case_dir":"/nonexistent","cause":"c","reason":"r"}"#), ("moves.json", "[]"), ("decision.json", "{}")] {
        fs::write(dir.join(name), f).unwrap();
    }
    for (name, extra) in [("first", vec![]), ("retry", vec!["--attempt", "2", "--invalid", "move x is not one of the ids in moves"])] {
        let rec = dir.join(name);
        let mut args: Vec<String> = ["prompt", "--stop", "stop.json", "--moves", "moves.json", "--decision", "decision.json", "--record"]
            .iter().map(|s| s.to_string()).collect();
        args.push(rec.display().to_string());
        args.extend(extra.iter().map(|s| s.to_string()));
        let o = Command::new(env!("CARGO_BIN_EXE_sdc-unstick")).args(&args).current_dir(&dir).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        compare("unstick", name, &fs::read_to_string(rec.join("prompt.md")).unwrap());
    }
    if std::env::var("SDC_BLESS_PROMPTS").is_err() {
        assert_eq!(golden_names("unstick"), names(&["first", "retry"]));
    }
}

#[test]
fn the_prompt_of_the_conformance_table_node_is_its_golden_byte_for_byte() {
    let bin = env!("CARGO_BIN_EXE_sdc-conform");
    compare("conform", "first", &bin_out(bin, &["prompt"]));
    compare("conform", "refused", &bin_out(bin, &["prompt", "--attempt", "2", "--refused", "view chores.list.OpenChores is not covered", "--refused", "start.from names no README line"]));
    compare("conform", "run-fault", &bin_out(bin, &["prompt", "--attempt", "3", "--run-fault", "the product answered 401 to every request", "--run-fault", "a capture never appeared"]));
    if std::env::var("SDC_BLESS_PROMPTS").is_err() {
        assert_eq!(golden_names("conform"), names(&["first", "refused", "run-fault"]));
    }
}

/// A story case for `sdc-reply implement-run`: story, requirements, a compiled spec with two
/// commands (one covered), the product tree of the stub fixture in out/impl.
fn implement_call(name: &str) -> (PathBuf, PathBuf, PathBuf) {
    let root = scratch(name);
    let (case, rec, cwd) = (root.join("case"), root.join("rec"), root.join("rec/cwd"));
    let w = case.join("work");
    for (p, t) in [
        ("aep/story/invoices.md", "---\nid: story:invoices\nsummary: Invoices. covers billing.invoice.CreateInvoice\n---\n# Invoices\n\n## Acceptance\n\n- an invoice is created\n"),
        ("requirements.yaml", "requirements:\n  - id: r1\n    text: A clerk creates invoices.\n    spec_domains: [billing.invoice]\n  - id: r2\n    text: The customer is emailed.\n    spec_domains: [billing.email]\n"),
    ] {
        fs::create_dir_all(w.join(p).parent().unwrap()).unwrap();
        fs::write(w.join(p), t).unwrap();
    }
    let ir = serde_json::json!({"commands": {
        "billing.invoice.CreateInvoice": {"name": "billing.invoice.CreateInvoice", "domain": "billing.invoice", "naming": {},
            "outcomes": [{"name": "created", "subject": {"entity": "billing.invoice.Invoice"}}, {"name": "refused", "error": "billing.invoice.NoCustomer"}]},
        "billing.email.SendEmail": {"name": "billing.email.SendEmail", "domain": "billing.email", "outcomes": []}},
        "views": {"billing.invoice.OpenInvoices": {"name": "billing.invoice.OpenInvoices", "domain": "billing.invoice", "source": "billing.invoice.Invoice"}},
        "entities": {"billing.invoice.Invoice": {"name": "billing.invoice.Invoice", "fields": [{"name": "id", "type_ref": {"name": "billing.invoice.InvoiceId"}}]}},
        "errors": {"billing.invoice.NoCustomer": {"name": "billing.invoice.NoCustomer"}},
        "types": {"billing.invoice.InvoiceId": {"name": "billing.invoice.InvoiceId"}}, "events": {}});
    fs::create_dir_all(w.join(".sdc/gate")).unwrap();
    fs::write(w.join(".sdc/gate/ir.json"), ir.to_string()).unwrap();
    copy_dir(&manifest().join("examples/sdc/fixtures/impl"), &cwd.join("out/impl"));
    fs::write(rec.join("request.json"), r#"{"model":"m","node":"change.implement","implement_mode":"request"}"#).unwrap();
    (case, rec, cwd)
}

/// An adapter that answers request k with `replies/<k>.json` (if any), prints a response and
/// exits with `exit`.
fn implement_adapter(rec: &Path, replies: &[&str], exit: i32) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let rp = rec.parent().unwrap().join("replies");
    fs::create_dir_all(&rp).unwrap();
    for (k, r) in replies.iter().enumerate() {
        fs::write(rp.join(format!("{}.json", k + 1)), r).unwrap();
    }
    let script = rec.parent().unwrap().join("adapter.sh");
    fs::write(&script, format!("#!/bin/sh\nd=$(dirname \"$1\"); k=${{d##*req-}}\ncp \"{}/$k.json\" \"$d/structured-output.json\" 2>/dev/null\necho '{{\"adapter\": \"test\", \"subtype\": \"error_max_turns\"}}'\nexit {exit}\n", rp.display())).unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    script
}

fn implement_run(case: &Path, rec: &Path, cwd: &Path, extra: &[&str]) {
    let o = Command::new(env!("CARGO_BIN_EXE_sdc-reply"))
        .args(["implement-run", "--rec"]).arg(rec).arg("--cwd").arg(cwd).arg("--case-dir").arg(case)
        .args(["--story", "invoices"]).args(extra).output().unwrap();
    assert!(!String::from_utf8_lossy(&o.stdout).contains("implement_inputs_missing"), "{}", String::from_utf8_lossy(&o.stdout));
}

#[test]
fn the_prompts_of_implement_request_mode_are_their_goldens_byte_for_byte() {
    let take = |rec: &Path, req: usize| fs::read_to_string(rec.join(format!("req-{req}/prompt.md"))).unwrap();
    // The first request, without build constraints, findings or feedback.
    let (case, rec, cwd) = implement_call("implement-first");
    implement_run(&case, &rec, &cwd, &["--assemble-only"]);
    compare("implement", "request", &fs::read_to_string(rec.join("prompt.md")).unwrap());
    // A rework: build constraints, the run's findings, the feedback and the rejection.
    let (case, rec, cwd) = implement_call("implement-rework");
    for (p, t) in [("build-constraints.yaml", "format: sdc.build-constraints/1\nlanguage: rust\ntext: Rust 2021, std only, no Postgres.\n"),
        ("findings.yaml", "findings:\n- id: f1\n  status: resolved\n")] {
        fs::write(case.join("work").join(p), t).unwrap();
    }
    fs::create_dir_all(cwd.join("in")).unwrap();
    fs::write(cwd.join("in/feedback.md"), "## conformance_run = failed\nassertion failed: fixture-failure\n").unwrap();
    fs::write(cwd.join("in/rejection.json"), r#"{"format":"sdc.rejection/1","approval":"change.review","revision":"t1","principal":"person:reviewer","count":1,"reasons":["impl/README.md:1: names no command (violates r1)","the refusal of an empty customer is missing"]}"#).unwrap();
    implement_run(&case, &rec, &cwd, &["--assemble-only", "--rework-limit", "2"]);
    compare("implement", "request.rework", &fs::read_to_string(rec.join("prompt.md")).unwrap());
    // A reply that leaves a file remaining: the second request is the continuation.
    let (case, rec, cwd) = implement_call("implement-continuation");
    let one = r#"{"files":[{"path":"src/lib.rs","content":"pub mod b;\n"}],"note":"part 1","findings":[],"remaining":["tests/billing.rs"]}"#;
    let adapter = implement_adapter(&rec, &[one], 0);
    implement_run(&case, &rec, &cwd, &["--adapter", adapter.to_str().unwrap()]);
    compare("implement", "request.continuation", &take(&rec, 2));
    // A reply the program check refused, one that is no reply, and a request that failed: the retry text of each.
    let bad = r#"{"files":[{"path":"../escape.rs","content":"x"}],"note":"","findings":[],"remaining":[]}"#;
    for (name, replies, exit) in [("request.retry", vec![bad], 0), ("request.retry-no-reply", vec![], 0), ("request.retry-failed", vec![], 3)] {
        let (case, rec, cwd) = implement_call(&format!("implement-{name}"));
        let adapter = implement_adapter(&rec, &replies, exit);
        implement_run(&case, &rec, &cwd, &["--adapter", adapter.to_str().unwrap()]);
        compare("implement", name, &take(&rec, 2));
    }
    if std::env::var("SDC_BLESS_PROMPTS").is_err() {
        let want = ["request", "request.rework", "request.continuation", "request.retry", "request.retry-no-reply", "request.retry-failed"];
        assert_eq!(golden_names("implement"), names(&want));
    }
}

// ---------------------------------------------------------------------------------------------
// The shared renderer of the Rust binaries (driver/src/instructions.rs), the same rules as
// `render` of llm-node.sh.

#[path = "../src/instructions.rs"]
#[allow(dead_code)]
mod instructions;

#[test]
fn the_rust_renderer_refuses_a_path_outside_instructions_and_fills_in_one_pass() {
    let root = scratch("render");
    fs::create_dir_all(root.join("instructions/sub")).unwrap();
    fs::write(root.join("README.md"), "outside {{x}}\n").unwrap();
    fs::write(root.join("instructions/a.md"), "a {{x}} b {{y}}\n\n\n").unwrap();
    fs::write(root.join("instructions/sub/b.md"), "no placeholder\n").unwrap();
    fs::write(root.join("instructions/c.md"), "needs {{missing_value}} and {{x}}").unwrap();
    // A path that is not instructions/<...>.md, is absolute or has a `..` segment is refused, and the file is not read.
    for bad in ["../README.md", "instructions/../README.md", "instructions/sub/../../README.md", "/etc/hostname.md", "README.md",
        "instructions/a.txt", "instructions", "other/instructions/a.md", "instructions/.."] {
        let e = instructions::render_in(&root, bad, &[("x", "1")]).unwrap_err();
        assert!(e.contains(bad) && e.contains("instructions/<...>.md"), "{bad}: {e}");
        assert_eq!(instructions::check_path(bad), Err(e), "{bad}");
    }
    // Every placeholder in one pass: a value is not searched again; trailing newlines of the file are dropped.
    assert_eq!(instructions::render_in(&root, "instructions/a.md", &[("x", "{{y}}"), ("y", "2")]).unwrap(), "a {{y}} b 2");
    assert_eq!(instructions::render_in(&root, "instructions/sub/b.md", &[]).unwrap(), "no placeholder");
    // A placeholder with no value is an error that names it; so is a missing file.
    let e = instructions::render_in(&root, "instructions/c.md", &[("x", "1")]).unwrap_err();
    assert!(e.contains("instructions/c.md") && e.contains("{{missing_value}}"), "{e}");
    let e = instructions::render_in(&root, "instructions/none.md", &[]).unwrap_err();
    assert!(e.contains("instructions/none.md") && e.contains("missing"), "{e}");
    // The repository's own files are found from the binary's place.
    assert!(instructions::render("instructions/run.unstick.closing.md", &[]).unwrap().starts_with("When the file is written"));
}

#[test]
fn the_three_rust_binaries_hold_no_instruction_text() {
    let mut lines = Vec::new();
    let mut stack = vec![repo().join("instructions")];
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
    for src in ["src/bin/sdc-conform.rs", "src/bin/sdc-unstick.rs", "src/bin/sdc-reply/implement.rs"] {
        let text = fs::read_to_string(manifest().join(src)).unwrap().replace("\\\"", "\"");
        for l in &lines {
            assert!(!text.contains(l.as_str()), "{src} still holds instruction text: {l}");
        }
        assert!(text.contains("instructions::render") || text.contains("text(\""), "{src} reads no instruction file");
    }
}
