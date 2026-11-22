use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::tree::product_files;
use super::*;
use crate::reply::read_json;


fn scratch(name: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-scratch/implement").join(name);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn put(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

fn paths(v: &[TreeFile]) -> Vec<&str> {
    v.iter().map(|f| f.path.as_str()).collect()
}

/// The stub fixture's tree (fixtures/impl): six files, 4,313 bytes.
fn fixture_impl() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/sdc/fixtures/impl")
}

#[test]
fn a_first_story_gets_the_whole_small_tree() {
    let s = select_tree(&fixture_impl(), Path::new("/nonexistent-mainline"), "", INLINE_CAP);
    assert!(s.first_story);
    assert_eq!(paths(&s.inlined), vec!["Cargo.lock", "Cargo.toml", "README.md", "Taskfile.yml", "src/lib.rs", "tests/billing.rs"]);
    assert!(s.listed.is_empty());
    assert_eq!(s.inlined_bytes, 4313);
    assert!(s.inlined.iter().all(|f| f.why == "first story: the whole tree"));
}

#[test]
fn build_output_and_hidden_entries_are_never_candidates() {
    let r = scratch("hidden");
    let t = r.join("impl");
    put(&t, "src/lib.rs", "pub fn a() {}\n");
    put(&t, "target/debug/x", "binary");
    put(&t, ".derived-from", "requirements x");
    put(&t, ".rework", "rework 1");
    put(&t, ".sdc/stub", "x");
    let s = select_tree(&t, &r.join("mainline"), "", INLINE_CAP);
    assert_eq!(paths(&s.inlined), vec!["src/lib.rs"]);
    assert!(s.listed.is_empty());
}

#[test]
fn a_later_story_ranks_its_own_and_named_files_first_and_lists_what_passes_the_cap() {
    let r = scratch("later");
    let (t, m) = (r.join("impl"), r.join("mainline"));
    // Merged by an earlier story: lib.rs (root), big.rs and old.rs untouched.
    for d in [&t, &m] {
        put(d, "Cargo.toml", "[package]\n");
        put(d, "src/lib.rs", "pub mod big;\n");
        put(d, "src/big.rs", &"x".repeat(900));
        put(d, "src/old.rs", &"o".repeat(300));
        put(d, "src/sync.rs", "pub fn sync() {}\n");
    }
    // This story's change: a new file and a changed one; the story names src/sync.rs.
    put(&t, "src/share.rs", &"s".repeat(200));
    put(&t, "src/old.rs", &"n".repeat(300));
    let story = "---\nsummary: share a list. covers todo.list.Share\n---\nExtends src/sync.rs.\n";
    let s = select_tree(&t, &m, story, 1000);
    assert!(!s.first_story);
    assert_eq!(paths(&s.inlined), vec!["src/sync.rs", "src/old.rs", "src/share.rs", "Cargo.toml", "src/lib.rs"]);
    assert_eq!(s.inlined.iter().map(|f| f.why).collect::<Vec<_>>(),
        vec!["named by the story", "this story's change", "this story's change", "crate root", "crate root"]);
    // src/big.rs (900 bytes) would pass the cap of 1000: listed by path and size only.
    assert_eq!(s.listed, vec![TreeFile { path: "src/big.rs".into(), bytes: 900, why: "over the cap" }]);
    assert!(s.inlined_bytes <= 1000);
}

#[test]
fn a_file_that_is_not_text_is_listed() {
    let r = scratch("binary");
    let t = r.join("impl");
    put(&t, "src/lib.rs", "pub fn a() {}\n");
    fs::write(t.join("logo.png"), [0x89u8, 0x50, 0xff, 0xfe]).unwrap();
    let s = select_tree(&t, &r.join("mainline"), "", INLINE_CAP);
    assert_eq!(s.listed, vec![TreeFile { path: "logo.png".into(), bytes: 4, why: "not UTF-8 text" }]);
}

#[test]
fn paths_are_confined_to_the_product_tree() {
    assert_eq!(confine("src/lib.rs"), Ok("src/lib.rs".into()));
    assert_eq!(confine("tests/a_b-1.rs"), Ok("tests/a_b-1.rs".into()));
    for (p, why) in [("../outside.rs", "leaves the product tree"), ("src/../../x", "leaves the product tree"),
        ("/etc/passwd", "an absolute path"), ("C:/x", "an absolute path"), ("src\\lib.rs", "a backslash"),
        ("", "an empty path"), ("src//lib.rs", "not a normal relative path"), ("./src/lib.rs", "not a normal relative path"),
        ("target/debug/app", "build output"), (".sdc/llm/x", "a hidden entry"), ("src/.rework", "a hidden entry"),
        (".derived-from", "a hidden entry")] {
        let e = confine(p).expect_err(p);
        assert!(e.contains(why), "{p}: {e}");
    }
}

#[test]
fn the_writer_refuses_an_escape_and_writes_nothing() {
    let r = scratch("writer");
    let root = r.join("out/impl");
    fs::create_dir_all(&root).unwrap();
    let files = vec![("src/lib.rs".to_string(), "pub fn a() {}\n".to_string()), ("../escape.rs".to_string(), "x".to_string())];
    let e = write_files(&root, &files).unwrap_err();
    assert!(e.contains("../escape.rs") && e.contains("leaves the product tree"), "{e}");
    assert!(!root.join("src/lib.rs").exists(), "one refused path writes nothing");
    assert!(!r.join("out/escape.rs").exists());
    let e = write_files(&root, &[("/tmp/abs.rs".into(), "x".into())]).unwrap_err();
    assert!(e.contains("an absolute path"), "{e}");
}

#[test]
fn the_writer_refuses_a_path_through_a_symlink() {
    let r = scratch("symlink");
    let root = r.join("out/impl");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(r.join("elsewhere")).unwrap();
    std::os::unix::fs::symlink(r.join("elsewhere"), root.join("src")).unwrap();
    let e = write_files(&root, &[("src/lib.rs".into(), "x".into())]).unwrap_err();
    assert!(e.contains("symlink"), "{e}");
    assert!(!r.join("elsewhere/lib.rs").exists());
}

#[test]
fn the_writer_writes_full_files_and_keeps_the_rest() {
    let r = scratch("write");
    let root = r.join("out/impl");
    put(&root, "src/keep.rs", "kept\n");
    put(&root, "src/lib.rs", "old\n");
    let w = write_files(&root, &[("src/lib.rs".into(), "new\n".into()), ("tests/t.rs".into(), "#[test]\nfn t() {}\n".into())]).unwrap();
    assert_eq!(w, vec!["src/lib.rs", "tests/t.rs"]);
    assert_eq!(fs::read_to_string(root.join("src/lib.rs")).unwrap(), "new\n");
    assert_eq!(fs::read_to_string(root.join("src/keep.rs")).unwrap(), "kept\n");
    assert!(root.join("tests/t.rs").is_file());
}

fn reply(files: Value) -> Value {
    json!({"files": files, "note": "built", "findings": [], "remaining": []})
}

#[test]
fn a_valid_reply_is_its_files() {
    let r = check_reply(&reply(json!([{"path": "src/lib.rs", "content": "pub fn a() {}\n"}])), None).unwrap();
    assert_eq!(r.files, vec![("src/lib.rs".to_string(), "pub fn a() {}\n".to_string())]);
    assert_eq!(r.note, "built");
}

#[test]
fn an_invalid_envelope_is_refused_with_its_reasons() {
    let check = |v: Value| check_reply(&v, None).unwrap_err().join("; ");
    assert!(check(json!("done")).contains("not a JSON object"));
    assert!(check(json!({"files": []})).contains("`note` is missing"));
    assert!(check(reply(json!([]))).contains("`files` is empty"));
    assert!(check(reply(json!([{"path": "src/lib.rs"}]))).contains("files[0]: `content` is not a string"));
    assert!(check(reply(json!([{"path": "a.rs", "content": "", "mode": "x"}]))).contains("files[0]: unknown key `mode`"));
    assert!(check(reply(json!([{"path": "a.rs", "content": "1"}, {"path": "a.rs", "content": "2"}]))).contains("files[1]: \"a.rs\" twice"));
    assert!(check(reply(json!([{"path": "../a.rs", "content": "1"}]))).contains("files[0]: path \"../a.rs\": leaves the product tree"));
    let mut v = reply(json!([{"path": "a.rs", "content": "1"}]));
    v["extra"] = json!(1);
    assert!(check(v).contains("unknown key `extra`"));
    let mut v = reply(json!([{"path": "a.rs", "content": "1"}]));
    v["findings"] = json!([{"type": "guess", "cites": [], "says": "", "options": ["a"], "default": "b", "scope_changing": "no"}]);
    let e = check(v);
    for w in ["findings[0]: `type`", "findings[0]: `cites`", "findings[0]: `says`", "findings[0]: `options`", "findings[0]: `default`", "findings[0]: `scope_changing`"] {
        assert!(e.contains(w), "{w} in {e}");
    }
}

#[test]
fn a_continuation_must_account_for_every_path_asked() {
    let asked = vec!["src/a.rs".to_string(), "src/b.rs".to_string()];
    let mut v = reply(json!([{"path": "src/a.rs", "content": "1"}]));
    assert!(check_reply(&v, Some(&asked)).unwrap_err().join("; ").contains("src/b.rs was asked for"));
    v["remaining"] = json!(["src/b.rs"]);
    assert_eq!(check_reply(&v, Some(&asked)).unwrap().remaining, vec!["src/b.rs"]);
    v["remaining"] = json!(["src/a.rs"]);
    assert!(check_reply(&v, Some(&asked)).unwrap_err().join("; ").contains("both written and remaining"));
}

#[test]
fn the_schema_names_every_key_and_requires_them() {
    let s = schema();
    assert_eq!(s["required"], json!(["files", "note", "findings", "remaining"]));
    assert_eq!(s["properties"]["files"]["items"]["required"], json!(["path", "content"]));
    assert_eq!(s["properties"]["files"]["minItems"], 1);
}

/// A story case: the stub fixture's requirements and story, an IR with two commands (one
/// covered), a view of the covered domain, the entity and error they name.
fn case(name: &str) -> PathBuf {
    let c = scratch(name).join("case");
    let w = c.join("work");
    put(&w, "aep/story/invoices.md", "---\nid: story:invoices\nsummary: Invoices. covers billing.invoice.CreateInvoice\n---\n# Invoices\n\n## Acceptance\n\n- an invoice is created\n");
    put(&w, "requirements.yaml", "requirements:\n  - id: r1\n    text: A clerk creates invoices.\n    spec_domains: [billing.invoice]\n  - id: r2\n    text: The customer is emailed.\n    spec_domains: [billing.email]\n");
    let ir = json!({"commands": {
        "billing.invoice.CreateInvoice": {"name": "billing.invoice.CreateInvoice", "domain": "billing.invoice", "naming": {},
            "outcomes": [{"name": "created", "subject": {"entity": "billing.invoice.Invoice"}}, {"name": "refused", "error": "billing.invoice.NoCustomer"}]},
        "billing.email.SendEmail": {"name": "billing.email.SendEmail", "domain": "billing.email", "outcomes": []}},
        "views": {"billing.invoice.OpenInvoices": {"name": "billing.invoice.OpenInvoices", "domain": "billing.invoice", "source": "billing.invoice.Invoice"},
                  "billing.email.Outbox": {"name": "billing.email.Outbox", "domain": "billing.email"}},
        "entities": {"billing.invoice.Invoice": {"name": "billing.invoice.Invoice", "fields": [{"name": "id", "type_ref": {"name": "billing.invoice.InvoiceId"}}]},
                     "billing.email.Email": {"name": "billing.email.Email"}},
        "errors": {"billing.invoice.NoCustomer": {"name": "billing.invoice.NoCustomer"}},
        "types": {"billing.invoice.InvoiceId": {"name": "billing.invoice.InvoiceId"}}, "events": {}});
    put(&w, ".sdc/gate/ir.json", &ir.to_string());
    c
}

#[test]
fn the_prompt_holds_the_story_its_requirements_and_only_its_spec() {
    let c = case("assemble");
    let tree = fixture_impl();
    let a = assemble(&Inputs { case_dir: &c, tree: &tree, story: "invoices", feedback: None, rejection: None,
        rework_limit: None, cap: INLINE_CAP, continuation: None }).unwrap();
    // The spec and requirement checks look at the facts, not at the tree's text (the fixture
    // crate names every billing command).
    let p = &a.prompt[..a.prompt.find("## The product tree").unwrap()];
    assert!(p.contains("covers billing.invoice.CreateInvoice"), "the story");
    assert!(p.contains("r1: A clerk creates invoices.") && !p.contains("r2: The customer is emailed."), "only its requirements");
    assert!(p.contains("- an invoice is created"), "its acceptance lines");
    assert!(p.contains("billing.invoice.CreateInvoice") && !p.contains("billing.email.SendEmail"), "only its commands");
    assert!(p.contains("billing.invoice.OpenInvoices") && !p.contains("billing.email.Outbox"), "the views of its domain");
    assert!(p.contains("billing.invoice.NoCustomer") && p.contains("billing.invoice.InvoiceId"), "what they name");
    assert!(!p.contains("billing.email.Email"), "nothing they do not name");
    assert!(!p.contains("\"naming\""), "empty naming dropped");
    assert!(a.prompt.contains("### impl/src/lib.rs (2300 bytes)"), "the current text of the tree");
    assert!(p.contains("Standard library only"), "the default build rules without build-constraints.yaml");
    assert!(a.prompt.contains("you have no tools, nothing to read"), "nothing to read");
    assert_eq!(a.manifest["tree"]["inlined_bytes"], 4313);
    assert_eq!(a.manifest["tree"]["first_story"], true);
    assert_eq!(a.manifest["requirements"], json!(["r1"]));
    assert_eq!(a.manifest["commands"], json!(["billing.invoice.CreateInvoice"]));
}

#[test]
fn a_rework_prompt_holds_the_rejection_the_feedback_and_the_constraints() {
    let c = case("rework");
    put(&c.join("work"), "build-constraints.yaml", "format: sdc.build-constraints/1\nlanguage: rust\ntext: Rust 2021, std only, no Postgres.\n");
    let tree = fixture_impl();
    let rej = json!({"format": "sdc.rejection/1", "approval": "change.review", "revision": "t1", "count": 1,
        "reasons": ["impl/README.md:1: names no command (violates r1)"]});
    let a = assemble(&Inputs { case_dir: &c, tree: &tree, story: "invoices", feedback: Some("## conformance_run = failed\nassertion failed: fixture-failure\n".into()),
        rejection: Some(rej), rework_limit: Some(2), cap: INLINE_CAP, continuation: None }).unwrap();
    assert!(a.prompt.contains("Rust 2021, std only, no Postgres."));
    assert!(a.prompt.contains("assertion failed: fixture-failure"));
    assert!(a.prompt.contains("1. impl/README.md:1: names no command (violates r1)"));
    assert!(a.prompt.contains("rejection 1 of at most 2"));
    assert_eq!(a.manifest["sections"]["rejection"].as_u64().map(|b| b > 0), Some(true));
}

#[test]
fn a_continuation_names_what_was_written_and_what_is_asked() {
    let c = case("continue");
    let tree = fixture_impl();
    let a = assemble(&Inputs { case_dir: &c, tree: &tree, story: "invoices", feedback: None, rejection: None, rework_limit: None,
        cap: INLINE_CAP, continuation: Some((vec!["src/lib.rs".into()], vec!["tests/billing.rs".into()])) }).unwrap();
    assert!(a.prompt.contains("## Continuation"));
    assert!(a.prompt.contains("Your previous reply wrote: src/lib.rs"));
    assert!(a.prompt.contains("Write exactly these files now, each complete: tests/billing.rs"));
}

/// A call: the case of `case`, an empty out/impl, a request.json, and an adapter (a generated
/// /bin/sh script) that answers request k with replies/<k>.json, cost 0.5 and 10 input tokens.
fn call(name: &str, replies: &[Value]) -> RunArgs {
    let c = case(name);
    let root = c.parent().unwrap().to_path_buf();
    let (cwd, rec, rp) = (root.join("rec/cwd"), root.join("rec"), root.join("replies"));
    fs::create_dir_all(cwd.join("out/impl")).unwrap();
    fs::create_dir_all(&rp).unwrap();
    for (k, r) in replies.iter().enumerate() {
        fs::write(rp.join(format!("{}.json", k + 1)), r.to_string()).unwrap();
    }
    fs::write(rec.join("request.json"), json!({"model": "m", "node": "change.implement", "implement_mode": "request"}).to_string()).unwrap();
    let script = root.join("adapter.sh");
    fs::write(&script, format!("#!/bin/sh\nd=$(dirname \"$1\"); k=${{d##*req-}}\ncp \"{}/$k.json\" \"$d/structured-output.json\" 2>/dev/null\n\
        echo '{{\"adapter\": \"test\", \"model\": \"m\", \"cost_usd\": 0.5, \"usage\": {{\"input_tokens\": 10}}}}'\n", rp.display())).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    RunArgs { rec, cwd, case_dir: c, story: "invoices".into(), adapter: Some(script), rework_limit: None, assemble_only: false, cap: INLINE_CAP }
}

fn file(p: &str, c: &str) -> Value {
    json!({"path": p, "content": c})
}

#[test]
fn run_retries_an_invalid_reply_twice_then_names_the_cause() {
    let bad = json!({"files": [file("../escape.rs", "x")], "note": "", "findings": [], "remaining": []});
    let a = call("run-invalid", &[bad.clone(), bad.clone(), bad]);
    let (code, line) = run(&a);
    assert_eq!(code, 2, "{line}");
    assert!(line.starts_with("implement_reply_invalid after 3 request(s), 2 retries: files[0]: path \"../escape.rs\": leaves the product tree"), "{line}");
    let r = read_json(&a.rec.join("response.json"));
    assert_eq!((r["requests"].as_u64(), r["reply_retries"].as_u64(), r["cause"].as_str()), (Some(3), Some(2), Some("implement_reply_invalid")));
    assert_eq!((r["cost_usd"].as_f64(), r["usage"]["input_tokens"].as_i64()), (Some(1.5), Some(30)));
    assert!(fs::read_to_string(a.rec.join("req-2/prompt.md")).unwrap().contains("The program check refused it: files[0]: path \"../escape.rs\""));
    assert!(product_files(&a.cwd.join("out/impl")).is_empty() && !a.cwd.join("out/escape.rs").exists());
}

#[test]
fn run_accepts_a_reply_after_one_retry() {
    let bad = json!({"files": [file("/abs.rs", "x")], "note": "", "findings": [], "remaining": []});
    let good = json!({"files": [file("src/lib.rs", "pub fn a() {}\n")], "note": "built", "findings": [], "remaining": []});
    let a = call("run-retry", &[bad, good]);
    let (code, line) = run(&a);
    assert_eq!(code, 0, "{line}");
    assert!(line.starts_with("change.implement wrote 1 file(s) in 2 request(s) (1 retries, 0 continuations;"), "{line}");
    assert_eq!(fs::read_to_string(a.cwd.join("out/impl/src/lib.rs")).unwrap(), "pub fn a() {}\n");
    assert_eq!(read_json(&a.rec.join("response.json"))["reply_retries"], 1);
}

#[test]
fn run_continues_by_rule_for_the_remaining_paths() {
    let one = json!({"files": [file("src/lib.rs", "pub mod b;\n")], "note": "part 1", "findings": [], "remaining": ["src/b.rs"]});
    let two = json!({"files": [file("src/b.rs", "pub fn b() {}\n")], "note": "part 2", "findings": [
        {"type": "ambiguity", "cites": ["r1"], "says": "which clerk?", "options": ["any", "one"], "default": "any", "scope_changing": false}], "remaining": []});
    let a = call("run-continue", &[one, two]);
    let (code, line) = run(&a);
    assert_eq!(code, 0, "{line}");
    let r = read_json(&a.rec.join("response.json"));
    assert_eq!((r["requests"].as_u64(), r["continuations"].as_u64(), r["reply_retries"].as_u64()), (Some(2), Some(1), Some(0)));
    let p2 = fs::read_to_string(a.rec.join("req-2/prompt.md")).unwrap();
    assert!(p2.contains("Write exactly these files now, each complete: src/b.rs") && p2.contains("pub mod b;"), "the continuation shows what was written");
    assert!(a.cwd.join("out/impl/src/b.rs").is_file());
    assert!(fs::read_to_string(a.cwd.join("out/findings.yaml")).unwrap().contains("which clerk?"));
    // A reply that leaves files remaining past the cap of continuations is incomplete.
    let more = |n: usize| json!({"files": [file(&format!("src/m{n}.rs"), "")], "note": "", "findings": [], "remaining": [format!("src/m{}.rs", n + 1)]});
    let a = call("run-incomplete", &[more(1), more(2), more(3)]);
    let (code, line) = run(&a);
    assert_eq!(code, 2);
    assert!(line.starts_with("implement_reply_incomplete after 3 request(s): src/m4.rs still remaining after 2 continuations"), "{line}");
}

#[test]
fn pack_is_a_valid_reply_of_the_tree() {
    let v = pack(&fixture_impl(), "stub");
    let r = check_reply(&v, None).unwrap();
    assert_eq!(r.files.len(), 6);
    assert_eq!(r.note, "stub");
}
