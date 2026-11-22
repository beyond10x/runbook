//! The bug-fix writers' prompts (the program `sdc-node`, nodes `bug.dissect`,
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
        let o = start_node(node, &work);
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

fn repo() -> PathBuf {
    manifest().parent().unwrap().to_path_buf()
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("instructions").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn sdc_env_cleared(c: &mut Command) {
    for (k, _) in std::env::vars().filter(|(k, _)| k.starts_with("SDC_") || k.starts_with("STUB_") || k.starts_with("LOOM_")) {
        c.env_remove(k);
    }
}

/// `sdc-node <node>` as Loom starts it, in `work`: the tools directory of the repository, `LOOM_CASE`
/// set, no other `SDC_*` or `LOOM_*` variable, `sdc-bugfix` and `sdc-findings` on `PATH`.
fn node_command(node: &str, work: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_sdc-node"));
    c.arg(node).arg("--tools-dir").arg(repo().join("bindings/tools")).current_dir(work);
    sdc_env_cleared(&mut c);
    let bins = Path::new(env!("CARGO_BIN_EXE_sdc-bugfix")).parent().unwrap().display().to_string();
    c.env("PATH", format!("{bins}:{}", std::env::var("PATH").unwrap_or_default())).env("LOOM_CASE", "IDEA-1.x");
    c
}

/// `c` with `{}` on standard input, as Loom gives a node with no arguments.
fn with_arguments(mut c: Command) -> std::process::Output {
    use std::io::Write;
    c.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = c.spawn().unwrap();
    child.stdin.take().unwrap().write_all(b"{}").unwrap();
    child.wait_with_output().unwrap()
}

/// One start of `sdc-node` for `node` in `work` on the adapter `true`, with the instruction files the
/// binding names: the prompt is written before the adapter runs.
fn start_node(node: &str, work: &Path) -> std::process::Output {
    let llm: serde_json::Value = serde_json::from_str(&llm_of(node)).unwrap();
    let mut c = node_command(node, work);
    c.args(["--adapter", "true", "--instructions", &llm["instructions"].to_string()]);
    with_arguments(c)
}

/// The binding's `llm` object of an action, as the binding gives it.
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

const AGENT_NODES: [&str; 14] = [
    "idea.dissect", "questions.write", "questions.answer", "requirements.normalise", "context.default", "vision.write",
    "roadmap.write", "spec.author", "plan.decompose", "change.implement", "change.review", "bug.dissect", "test.reproduce",
    "change.fix",
];

/// The goldens that `sdc_node.rs` compares in its own cases (a ticket, no gate, feedback, the review
/// on a coverage document, the implementer sent back or asked again, ...).
const VARIANT_GOLDENS: [&str; 11] = [
    "change.fix.ticket", "test.reproduce.ticket", "change.fix.no-gate", "change.fix.feedback", "change.review.coverage",
    "change.review.facts", "change.review.retry", "change.implement.coverage", "change.implement.rework",
    "change.implement.rework-coverage", "change.implement.coverage-retry",
];

/// Each agent node's prompt, written by the program `sdc-node` under Loom (one start per node, on
/// the adapter `true`), is its golden byte for byte. A golden no test reads is a failure.
#[test]
fn the_prompt_of_each_agent_node_is_its_golden_byte_for_byte() {
    for node in AGENT_NODES {
        compare("llm-node", node, &super::sdc_node::loom_prompt(node));
    }
    if std::env::var("SDC_BLESS_PROMPTS").is_err() {
        let cases = include_str!("sdc_node.rs");
        for name in VARIANT_GOLDENS {
            assert!(cases.contains(&format!("\"llm-node\", \"{name}\"")), "no case of sdc_node.rs reads the golden {name}");
        }
        let seen: std::collections::BTreeSet<String> = AGENT_NODES.iter().chain(VARIANT_GOLDENS.iter()).map(|n| n.to_string()).collect();
        assert_eq!(seen, golden_names("llm-node"), "a golden no test reads, or a prompt with no golden");
    }
}

/// The names of the goldens of one group on disk.
fn golden_names(group: &str) -> std::collections::BTreeSet<String> {
    fs::read_dir(repo().join("instructions/golden").join(group))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().trim_end_matches(".md").to_string())
        .collect()
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
    for e in fs::read_dir(manifest().join("../bindings/tools")).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_none_or(|x| x != "sh") {
            continue;
        }
        let sh = fs::read_to_string(&p).unwrap();
        for l in &lines {
            assert!(!sh.contains(l.as_str()), "{} still holds instruction text: {l}", p.display());
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
        "feedback",
        "requester-request",
        "node-log",
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
        let instructions = serde_json::json!({"task": path}).to_string();
        let mut c = node_command("idea.dissect", &work);
        c.args(["--adapter", "true", "--instructions", &instructions, "--stub", "stub-file", "dissection.yaml", "dissection.yaml"]);
        let o = with_arguments(c);
        let err = String::from_utf8_lossy(&o.stderr);
        assert_eq!(o.status.code(), Some(126), "{path}: stderr {err}");
        assert!(err.contains(path) && err.contains("instructions/<...>.md"), "{path}: stderr {err}");
    }
}

// ---------------------------------------------------------------------------------------------
// The model instructions of the Rust binaries (story:rust-prompts-as-instruction-files):
// `sdc-reply implement-run` (SDC_IMPLEMENT_MODE=request) and `sdc-conform` (the table node). No
// model is called: `implement-run --assemble-only` and an adapter script write the prompts,
// `sdc-conform prompt` prints its own.

fn bin_out(bin: &str, args: &[&str]) -> String {
    let o = Command::new(bin).args(args).output().unwrap();
    assert!(o.status.success(), "{bin} {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap()
}

fn names(of: &[&str]) -> std::collections::BTreeSet<String> {
    of.iter().map(|s| s.to_string()).collect()
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

/// A story that covers a command the specification lacks, over an empty product tree: no
/// requirement traces to it, the specification names it missing, impl/ is empty.
fn implement_sparse(name: &str) -> (PathBuf, PathBuf, PathBuf, Vec<&'static str>) {
    let (case, rec, cwd) = implement_call(name);
    fs::write(case.join("work/aep/story/invoices.md"), "---\nid: story:invoices\nsummary: Elsewhere. covers other.thing.Nothing\n---\n# Elsewhere\n\n## Acceptance\n\n- nothing is created\n").unwrap();
    fs::remove_dir_all(cwd.join("out/impl")).unwrap();
    fs::create_dir_all(cwd.join("out/impl")).unwrap();
    (case, rec, cwd, vec!["--assemble-only"])
}

/// A tiny product tree past a cap of 20 bytes: `a.txt` is inlined, `b.txt` is over the cap and
/// `blob.bin` is not text. `full` adds the build constraints, the findings, the feedback and a
/// rejection, so that every section of the prompt is there.
fn implement_listed(name: &str, full: bool) -> (PathBuf, PathBuf, PathBuf, Vec<&'static str>) {
    let (case, rec, cwd) = implement_call(name);
    fs::remove_dir_all(cwd.join("out/impl")).unwrap();
    fs::create_dir_all(cwd.join("out/impl")).unwrap();
    fs::write(cwd.join("out/impl/a.txt"), "aa\n").unwrap();
    fs::write(cwd.join("out/impl/b.txt"), "bb bb bb bb bb bb bb bb bb bb\n").unwrap();
    fs::write(cwd.join("out/impl/blob.bin"), [0xffu8, 0xfe, 0, 1]).unwrap();
    if full {
        for (p, t) in [("build-constraints.yaml", "format: sdc.build-constraints/1\nlanguage: rust\ntext: Rust 2021, std only, no Postgres.\n"),
            ("findings.yaml", "findings:\n- id: f1\n  status: resolved\n")] {
            fs::write(case.join("work").join(p), t).unwrap();
        }
        fs::create_dir_all(cwd.join("in")).unwrap();
        fs::write(cwd.join("in/feedback.md"), "## conformance_run = failed\nassertion failed: fixture-failure\n").unwrap();
        fs::write(cwd.join("in/rejection.json"), r#"{"format":"sdc.rejection/1","approval":"change.review","revision":"t1","principal":"person:reviewer","count":1,"reasons":["the refusal of an empty customer is missing"]}"#).unwrap();
        return (case, rec, cwd, vec!["--assemble-only", "--max-bytes", "20", "--rework-limit", "2"]);
    }
    (case, rec, cwd, vec!["--assemble-only", "--max-bytes", "20"])
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
    // A story whose command the specification lacks, over an empty product tree.
    let (case, rec, cwd, extra) = implement_sparse("implement-sparse");
    implement_run(&case, &rec, &cwd, &extra);
    compare("implement", "request.sparse", &fs::read_to_string(rec.join("prompt.md")).unwrap());
    // A tree past the cap: files listed without text, one of them not UTF-8.
    let (case, rec, cwd, extra) = implement_listed("implement-listed", false);
    implement_run(&case, &rec, &cwd, &extra);
    compare("implement", "request.listed", &fs::read_to_string(rec.join("prompt.md")).unwrap());
    if std::env::var("SDC_BLESS_PROMPTS").is_err() {
        let want = ["request", "request.rework", "request.continuation", "request.retry", "request.retry-no-reply", "request.retry-failed",
            "request.sparse", "request.listed"];
        assert_eq!(golden_names("implement"), names(&want));
    }
}

// ---------------------------------------------------------------------------------------------
// The shared renderer of the Rust binaries (driver/src/instructions.rs), the same rules as
// `render` of the shell script that `sdc-node` replaced.

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
    assert!(instructions::render("instructions/decider/close.md", &[("outcome", "kept")]).unwrap().contains("kept"));
}

#[test]
fn the_rust_binaries_hold_no_instruction_text() {
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
    let implement = ["mod", "tree", "reply", "assemble", "run"].map(|f| format!("src/reply/implement/{f}.rs"));
    let implement_text = implement.iter().map(|f| fs::read_to_string(manifest().join(f)).unwrap()).collect::<Vec<_>>().join("\n");
    for (src, text) in [("src/conform.rs", fs::read_to_string(manifest().join("src/conform.rs")).unwrap()),
        ("src/reply/implement/", implement_text)] {
        let text = text.replace("\\\"", "\"");
        for l in &lines {
            assert!(!text.contains(l.as_str()), "{src} still holds instruction text: {l}");
        }
        assert!(text.contains("instructions::render") || text.contains("text(\""), "{src} reads no instruction file");
    }
    // `sdc-node` and the runner (`runbook/src/loom_run/`, the decider and its sub-modules among them)
    // took over from `llm-node.sh` and `decider.sh`, the two programs this guard held before: every
    // Rust source of them is held to the same rule (see `guarded_sources`).
    let (sources, refused) = guarded_sources();
    assert!(refused.is_empty(), "files the guard cannot cut into non-test code: {refused:?}");
    assert!(lines.len() > 100 && sources.len() > 8, "found {} lines and {} sources", lines.len(), sources.len());
    for (p, text) in &sources {
        if let Some(l) = first_instruction_line_in(text, &lines) {
            panic!("{} holds instruction text: {l}", p.display());
        }
    }
    // The scan can fail: a source of the set with one instruction line pasted into a string constant.
    for (p, text) in [&sources[0], sources.iter().find(|(p, _)| p.ends_with("loom_run/decider.rs")).expect("the decider's module root is scanned")] {
        let mutant = format!("{text}\nconst PASTED: &str = \"{}\";\n", lines[0].replace('"', "\\\""));
        assert!(first_instruction_line_in(&mutant, &lines).is_some(), "the scan misses a line pasted into {}", p.display());
    }
    // The lines above only find a copy of a line of a file. A heading or a note of a few words is
    // found in what the binary renders: the request-mode prompt of three calls (the specification
    // lacks the story's command and impl/ is empty; the tree is past the cap; every section).
    for (name, case) in [("sparse", implement_sparse("words-sparse")), ("listed", implement_listed("words-listed", false)), ("full", implement_listed("words-full", true))] {
        let (case, rec, cwd, extra) = case;
        implement_run(&case, &rec, &cwd, &extra);
        let prompt = fs::read_to_string(rec.join("prompt.md")).unwrap();
        let stray = words_outside_files_and_inputs(&prompt, &case, &cwd);
        assert!(stray.is_empty(), "the request-mode prompt of the {name} call holds words that are in no instruction file and no input: {stray:?}");
    }
}

/// The words of a request-mode prompt that are neither in an instruction file nor in an input:
/// the text of every `change.implement.request.*.md` (the parts between its `{{name}}`
/// placeholders) is cut out of the prompt, and every word of the call's inputs (the files of the
/// case's work/, of the call's in/ and of the product tree, and their paths) is allowed.
fn words_outside_files_and_inputs(prompt: &str, case: &Path, cwd: &Path) -> Vec<String> {
    let placeholder = regex::Regex::new(r"\{\{[a-z_]+\}\}").unwrap();
    let word = regex::Regex::new(r"[A-Za-z]+").unwrap();
    let mut parts = Vec::new();
    for e in fs::read_dir(repo().join("instructions")).unwrap() {
        let p = e.unwrap().path();
        if p.file_name().unwrap().to_string_lossy().starts_with("change.implement.request.") {
            let text = fs::read_to_string(&p).unwrap();
            parts.extend(placeholder.split(text.trim_end_matches('\n')).map(String::from));
        }
    }
    parts.retain(|s| word.is_match(s));
    parts.sort_by_key(|s| std::cmp::Reverse(s.len()));
    let mut left = prompt.to_string();
    for part in &parts {
        left = left.replace(part.as_str(), "\u{1}");
    }
    let mut allowed = std::collections::BTreeSet::new();
    for root in [case.join("work"), cwd.join("in"), cwd.join("out/impl")] {
        let mut stack = vec![root.clone()];
        while let Some(d) = stack.pop() {
            for e in fs::read_dir(&d).into_iter().flatten() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                let text = fs::read_to_string(&p).unwrap_or_default();
                let rel = p.strip_prefix(&root).unwrap().display().to_string();
                for w in word.find_iter(&text).chain(word.find_iter(&rel)) {
                    allowed.insert(w.as_str().to_string());
                }
            }
        }
    }
    let mut stray: Vec<String> = word.find_iter(&left).map(|m| m.as_str().to_string()).filter(|w| !allowed.contains(w)).collect();
    stray.sort();
    stray.dedup();
    stray
}

fn first_instruction_line_in(text: &str, lines: &[String]) -> Option<String> {
    let text = text.replace("\\\"", "\"");
    lines.iter().find(|l| text.contains(l.as_str())).cloned()
}


/// Test code may hold instruction text on purpose (a fixture of what a prompt says). The rule: a
/// file named `tests.rs`, `golden.rs` or `*_tests.rs` is test code, and so is an inline
/// `#[cfg(test)]` module (the attribute line followed by a `mod name {` line).
fn is_test_file(p: &Path) -> bool {
    let n = p.file_name().unwrap().to_string_lossy();
    n == "tests.rs" || n == "golden.rs" || n.ends_with("_tests.rs")
}

/// The offset of the brace that closes the first `{` of `text`, counting braces and leaving out
/// those in line and block comments, string literals (raw ones too) and char literals. Not handled:
/// nested block comments, and braces that a macro body makes unbalanced. Enough for the files of
/// the scanned directories; a file it misreads fails the guard instead of passing it.
fn closing_brace(text: &str) -> Option<usize> {
    let b = text.as_bytes();
    let (mut i, mut depth) = (0, 0usize);
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
                continue;
            }
            b'r' if matches!(b.get(i + 1), Some(b'"' | b'#')) && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) => {
                let (mut j, mut hashes) = (i + 1, 0);
                while b.get(j) == Some(&b'#') {
                    hashes += 1;
                    j += 1;
                }
                if b.get(j) == Some(&b'"') {
                    let end = format!("\"{}", "#".repeat(hashes));
                    i = text[j + 1..].find(&end).map_or(b.len(), |k| j + 1 + k + end.len());
                    continue;
                }
            }
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
                continue;
            }
            b'\'' => {
                if b.get(i + 1) == Some(&b'\\') {
                    i += 3;
                    while i < b.len() && b[i] != b'\'' {
                        i += 1;
                    }
                    i += 1;
                    continue;
                }
                if let Some(c) = text[i + 1..].chars().next() {
                    if text[i + 1 + c.len_utf8()..].starts_with('\'') {
                        i += 2 + c.len_utf8();
                        continue;
                    }
                }
            }
            b'{' => depth += 1,
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// The text before an inline `#[cfg(test)]` module. A file with any item after the closing brace
/// of that module is an error naming what follows: the cut would drop non-test code with it.
fn non_test_text(text: &str) -> Result<String, String> {
    let lines: Vec<&str> = text.lines().collect();
    let cut = (0..lines.len().saturating_sub(1))
        .find(|&i| lines[i].trim() == "#[cfg(test)]" && lines[i + 1].trim_start().starts_with("mod ") && lines[i + 1].trim_end().ends_with('{'));
    let Some(i) = cut else { return Ok(lines.join("\n")) };
    let module = lines[i + 1..].join("\n");
    let end = closing_brace(&module).ok_or("the inline test module is not closed (or its braces are misread)")?;
    let after = module[end + 1..].trim();
    if !after.is_empty() {
        return Err(format!("code after the inline test module: {}", after.lines().next().unwrap_or_default()));
    }
    Ok(lines[..i].join("\n"))
}

/// Every `.rs` file under `dir` that is not test code, with its non-test text, found by walking;
/// a file `non_test_text` refuses is named in `refused`.
fn program_sources(dir: &Path, out: &mut Vec<(PathBuf, String)>, refused: &mut Vec<String>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            program_sources(&p, out, refused);
        } else if p.extension().is_some_and(|x| x == "rs") && !is_test_file(&p) {
            match non_test_text(&fs::read_to_string(&p).unwrap()) {
                Ok(text) => out.push((p.clone(), text)),
                Err(e) => refused.push(format!("{}: {e}", p.display())),
            }
        }
    }
}

/// The Rust sources of the programs that replaced `llm-node.sh` and `decider.sh`: `sdc-node` (its
/// file and its directory) and the runner's `loom_run` module tree, which holds the decider (its
/// module root `decider.rs` and `decider/`), and the files the cut refused. A new file is in the
/// set without an edit here.
fn guarded_sources() -> (Vec<(PathBuf, String)>, Vec<String>) {
    let (mut sources, mut refused) = (Vec::new(), Vec::new());
    let node = manifest().join("src/bin/sdc-node.rs");
    sources.push((node.clone(), fs::read_to_string(&node).unwrap()));
    program_sources(&manifest().join("src/bin/sdc-node"), &mut sources, &mut refused);
    program_sources(&repo().join("runbook/src/loom_run"), &mut sources, &mut refused);
    (sources, refused)
}

/// The set the guard scans takes in the decider's module root and its sub-modules, and leaves out
/// its test code.
#[test]
fn the_guard_scans_the_decider_module_root_and_leaves_out_test_code() {
    let (sources, refused) = guarded_sources();
    assert!(refused.is_empty(), "{refused:?}");
    let scanned = |rel: &str| sources.iter().any(|(p, _)| *p == repo().join(rel));
    assert!(scanned("runbook/src/loom_run/decider.rs"), "the decider's module root is not scanned");
    assert!(scanned("runbook/src/loom_run/decider/requester.rs"), "a sub-module of the decider is not scanned");
    assert!(!scanned("runbook/src/loom_run/decider/tests.rs") && !scanned("runbook/src/loom_run/decider/golden.rs"), "test code is scanned");
    assert_eq!(non_test_text("a\n#[cfg(test)]\nmod t {\nb\n}\n").unwrap(), "a");
    assert_eq!(non_test_text("a\n#[cfg(test)]\nmod t;\nb\n").unwrap(), "a\n#[cfg(test)]\nmod t;\nb");
}

/// The cut counts braces from the inline test module's opening one, leaves out the braces of
/// strings, chars and comments, and refuses a file with an item after the module.
#[test]
fn a_file_with_code_after_its_inline_test_module_fails_the_guard() {
    let tricky = "x\n#[cfg(test)]\nmod tests {\n    const A: &str = \"}\";\n    const R: &str = r#\"} \" }\"#;\n    const C: char = '}';\n    const E: char = '\\'';\n    // }\n    /* } */\n    fn f<'a>(_: &'a str) {}\n}\n";
    assert_eq!(non_test_text(tricky).unwrap(), "x");
    assert_eq!(non_test_text(&format!("{tricky}\n")).unwrap(), "x");
    let after = format!("{tricky}fn after_the_test_module() {{}}\n");
    let e = non_test_text(&after).unwrap_err();
    assert!(e.contains("code after the inline test module: fn after_the_test_module"), "{e}");
    assert!(non_test_text("#[cfg(test)]\nmod tests {\nfn f() {\n").unwrap_err().contains("not closed"));
    // Every real file of the scanned directories that has such a module has nothing after it.
    for rel in ["runbook/src/loom_run/budget.rs", "runbook/src/loom_run/bugfix.rs", "runbook/src/loom_run/replay.rs"] {
        let real = fs::read_to_string(repo().join(rel)).unwrap();
        assert!(non_test_text(&real).is_ok(), "{rel} is refused");
        assert!(non_test_text(&format!("{real}\nfn after() {{}}\n")).is_err(), "{rel} with an item appended passes");
    }
}

fn pasted_instruction_line() -> (Vec<String>, String) {
    let mut lines = Vec::new();
    for e in fs::read_dir(repo().join("instructions/decider")).unwrap() {
        let p = e.unwrap().path();
        if p.is_file() {
            lines.extend(fs::read_to_string(&p).unwrap().lines().map(str::trim).filter(|l| l.len() >= 30).map(String::from));
        }
    }
    let one = lines[0].replace('"', "\\\"");
    (lines, format!("\nconst PASTED: &str = \"{one}\";\n"))
}

/// The real text of files of the set with a line pasted at its end, through the guard's own cut:
/// decider.rs, another `loom_run` file, a decider sub-module, two `sdc-node` files.
#[test]
fn a_line_pasted_into_a_scanned_file_is_found() {
    let (lines, paste) = pasted_instruction_line();
    let (sources, _) = guarded_sources();
    for rel in ["runbook/src/loom_run/decider.rs", "runbook/src/loom_run/ledger.rs", "runbook/src/loom_run/decider/prompt.rs", "driver/src/bin/sdc-node/call.rs", "driver/src/bin/sdc-node.rs"] {
        let path = repo().join(rel);
        assert!(sources.iter().any(|(p, _)| *p == path), "{rel} is not scanned");
        let real = fs::read_to_string(&path).unwrap();
        let cut = if rel.ends_with("sdc-node.rs") { format!("{real}{paste}") } else { non_test_text(&format!("{real}{paste}")).unwrap() };
        assert!(first_instruction_line_in(&cut, &lines).is_some(), "a line pasted at the end of {rel} is not found");
    }
}

/// A new file in a walked directory, and one in a sub-directory, is scanned without an edit of the
/// guard, and one with code after its inline test module is refused. The directory is the test's
/// own, under the target directory: nothing is written into the source tree.
#[test]
fn a_new_file_in_a_walked_directory_is_scanned() {
    let (lines, paste) = pasted_instruction_line();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("guard-walk");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("sub")).unwrap();
    fs::write(dir.join("new.rs"), format!("//! new{paste}")).unwrap();
    fs::write(dir.join("sub/new_sub.rs"), format!("//! new{paste}")).unwrap();
    fs::write(dir.join("new_tests.rs"), format!("//! test code{paste}")).unwrap();
    fs::write(dir.join("late.rs"), "#[cfg(test)]\nmod tests {\n}\nfn late() {}\n").unwrap();
    let (mut sources, mut refused) = (Vec::new(), Vec::new());
    program_sources(&dir, &mut sources, &mut refused);
    assert_eq!(sources.len(), 2, "{:?}", sources.iter().map(|s| &s.0).collect::<Vec<_>>());
    for (p, text) in &sources {
        assert!(first_instruction_line_in(text, &lines).is_some(), "the line pasted into the new file {} is not found", p.display());
    }
    assert!(refused.len() == 1 && refused[0].contains("late.rs") && refused[0].contains("code after the inline test module"), "{refused:?}");
    fs::remove_dir_all(&dir).unwrap();
}

/// Every file the rule leaves out by its name is test code: its parent declares it behind
/// `#[cfg(test)]`.
#[test]
fn every_file_excluded_by_name_is_declared_behind_cfg_test() {
    let mut files = Vec::new();
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        for e in fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") && is_test_file(&p) {
                out.push(p);
            }
        }
    }
    walk(&repo().join("runbook/src/loom_run"), &mut files);
    walk(&manifest().join("src/bin/sdc-node"), &mut files);
    assert!(!files.is_empty());
    let mut bad = Vec::new();
    for f in files {
        let name = f.file_stem().unwrap().to_string_lossy().to_string();
        let dir = f.parent().unwrap();
        let parent = [dir.with_extension("rs"), dir.join("mod.rs")].into_iter().find(|p| p.is_file()).expect("a parent module file");
        let text = fs::read_to_string(&parent).unwrap();
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        if !lines.windows(2).any(|w| w[0] == "#[cfg(test)]" && w[1] == format!("mod {name};")) {
            bad.push(f.display().to_string());
        }
    }
    assert!(bad.is_empty(), "excluded by name and not declared behind cfg(test): {bad:?}");
}
