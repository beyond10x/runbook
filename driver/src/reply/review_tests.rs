use std::fs;
use std::path::PathBuf;

use serde_json::{json, Value};

use super::records::*;
use super::review::*;
use super::schemas::*;
use super::*;

/// runs/trial-3-haiku-3's requirements and plan: tasks covers todo.task, sync todo.sync;
/// nothing covers todo.access (r2, r3), and the spec has no traces.json.
fn haiku3() -> (Vec<Value>, Vec<String>, Vec<String>) {
    let r = |id: &str, d: &str| json!({"id": id, "text": id, "spec_domains": [d]});
    let reqs = vec![r("r1", "todo.task"), r("r2", "todo.access"), r("r3", "todo.access"),
                    r("r4", "todo.sync"), r("r5", "todo.sync"), r("r6", "todo.task")];
    let tasks = ["todo.task.AddTask", "todo.task.CompleteTask", "todo.task.ReopenTask"].map(String::from).to_vec();
    (reqs, tasks, vec!["todo.sync.SyncChanges".to_string()])
}

fn ids(v: &[Value]) -> Vec<&str> {
    v.iter().filter_map(|e| e["id"].as_str()).collect()
}

#[test]
fn scope_without_traces_uses_spec_domains() {
    let (reqs, tasks, sync) = haiku3();
    let all = vec![tasks.clone(), sync.clone()];
    let (mine, unc) = review_scope(&reqs, &Value::Null, &Value::Null, &tasks, &all);
    assert_eq!((ids(&mine), ids(&unc)), (vec!["r1", "r6"], vec!["r2", "r3"]));
    let (mine, _) = review_scope(&reqs, &Value::Null, &Value::Null, &sync, &all);
    assert_eq!(ids(&mine), vec!["r4", "r5"]);
}

#[test]
fn scope_with_traces_counts_commands_and_same_domain_views() {
    let (mut reqs, tasks, sync) = haiku3();
    reqs[3]["behaviour"] = json!("constraint");
    let traces = json!({"r1": ["todo.task.AddTask"], "r2": ["todo.task.TaskList"], "r3": ["todo.access.Member"],
                        "r5": ["todo.sync.SyncChanges"], "r6": ["todo.task.AddTask"]});
    let commands = json!({"todo.task.AddTask": {}, "todo.sync.SyncChanges": {}});
    let (mine, unc) = review_scope(&reqs, &traces, &commands, &tasks, &[tasks.clone(), sync.clone()]);
    // r2's view is in todo.task (tasks' domain); r3's actor is in no covered domain; r4 is a constraint.
    assert_eq!((ids(&mine), ids(&unc)), (vec!["r1", "r2", "r6"], vec!["r3"]));
}

#[test]
fn story_summary_and_covers() {
    let t = "---\nformat: x\nid: story:tasks\nsummary: A user adds a task. covers todo.task.AddTask todo.task.CompleteTask\n---\n# T\n";
    let (at, s) = story_summary(t).unwrap();
    assert_eq!(at, 4);
    assert_eq!(covers(&s), vec!["todo.task.AddTask", "todo.task.CompleteTask"]);
}

/// A call dir and case dir: the change tree with a three-line README, one story requirement r1,
/// one uncovered requirement r3, README.md changed in the diff.
fn call(name: &str) -> (PathBuf, PathBuf) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-scratch/check-review").join(name);
    let _ = fs::remove_dir_all(&root);
    let (cwd, case) = (root.join("call"), root.join("case"));
    fs::create_dir_all(cwd.join("in/impl")).unwrap();
    fs::create_dir_all(case.join("work")).unwrap();
    fs::write(cwd.join("in/impl/README.md"), "# App\nrun it\nend\n").unwrap();
    fs::write(cwd.join("in/change.diff"), "+++ impl/README.md\t2026\n").unwrap();
    fs::write(cwd.join("in/scope.json"), json!({"covers": ["todo.task.AddTask"], "requirements": [{"id": "r1"}],
        "acceptance": [{"cite": "aep/story/tasks.md:4"}]}).to_string()).unwrap();
    fs::write(cwd.join("in/uncovered.json"), json!({"requirements": [{"id": "r3"}]}).to_string()).unwrap();
    fs::write(cwd.join("in/listing.json"), json!({"files": [{"path": "src/server.rs", "bytes": 60}, {"path": "public/index.html", "bytes": 28}]}).to_string()).unwrap();
    fs::write(case.join("case.json"), json!({"id": "IDEA-1.app.tasks", "artifacts": {"change": {"revision": "t1"}}}).to_string()).unwrap();
    (cwd, case)
}

fn finding(kind: &str, at: &str, v: &str) -> Value {
    json!({"kind": kind, "at": at, "violates": v, "says": "it does not"})
}

#[test]
fn a_rejection_with_a_resolvable_finding_is_the_rejection() {
    let (cwd, case) = call("reject");
    let reply = json!({"decision": "rejected", "summary": "s", "findings": [finding("violation", "impl/README.md:2", "r1"),
        finding("outside", "impl/README.md:1", "r3")]});
    let (ok, v) = check_review(&cwd, &Facts::driver(&case), &reply, "llm-review:m").unwrap();
    assert!(!ok);
    assert_eq!(v["format"], "sdc.rejection/1");
    assert_eq!(v["revision"], "t1");
    assert_eq!(v["count"], 1);
    assert_eq!(v["reasons"], json!(["impl/README.md:2: it does not (violates r1)"]));
    assert_eq!(v["notes"], json!(["impl/README.md:1: it does not (outside this story: r3)"]));
    assert_eq!(v["citations"][0]["in_diff"], true);
}

#[test]
fn an_invalid_reply_is_never_a_decision() {
    let (cwd, case) = call("invalid");
    let check = |reply: Value| check_review(&cwd, &Facts::driver(&case), &reply, "p").unwrap_err().join("; ");
    // A line the file does not have, a file outside the change tree, a requirement outside the story.
    assert!(check(json!({"decision": "rejected", "findings": [finding("violation", "impl/README.md:9", "r1")]})).contains("lines 9-9 outside impl/README.md"));
    assert!(check(json!({"decision": "approved", "findings": [finding("violation", "in/scope.json:1", "r1")]})).contains("not a line of the change tree"));
    assert!(check(json!({"decision": "rejected", "findings": [finding("violation", "impl/README.md:1", "r3")]})).contains("not a requirement, acceptance line or command of this story"));
    // A rejection with only an `outside` finding, or none, names no violation.
    assert!(check(json!({"decision": "rejected", "findings": [finding("outside", "impl/README.md:1", "r3")]})).contains("names no violation"));
    assert!(check(json!({"decision": "rejected", "findings": []})).contains("names no violation"));
    assert!(check(json!({"decision": "maybe", "findings": []})).contains("neither approved nor rejected"));
    assert!(check(Value::Null).contains("neither approved nor rejected"));
}

#[test]
fn an_approval_may_carry_outside_notes() {
    let (cwd, case) = call("approve");
    let reply = json!({"decision": "approved", "summary": "s", "findings": [finding("outside", "impl/README.md:1", "r3")]});
    let (ok, v) = check_review(&cwd, &Facts::driver(&case), &reply, "p").unwrap();
    assert!(ok);
    assert_eq!(v["notes"].as_array().unwrap().len(), 1);
}

// ---- review-inputs ---------------------------------------------------------------------------

/// A case of two stories (tasks covers todo.task.AddTask, sync covers todo.sync.SyncChanges) and
/// three requirements (r1 tasks, r2 sync, r3 nobody). The change under review edits
/// `src/lib.rs` and adds `src/new.rs`; `src/untouched.rs` and `README.md` are not in the diff.
fn review_case(name: &str, tests_log: &str) -> (PathBuf, PathBuf) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-scratch/review-inputs").join(name);
    let _ = fs::remove_dir_all(&root);
    let (cwd, case) = (root.join("call"), root.join("case"));
    let w = case.join("work");
    for d in ["aep/story", "impl/src", "impl/public", "mainline/src", "mainline/public", ".sdc/gate", "../evidence", "../blobs"] {
        fs::create_dir_all(w.join(d)).unwrap();
    }
    let put = |p: PathBuf, t: &str| fs::write(p, t).unwrap();
    put(w.join("aep/story/tasks.md"), "---\nid: story:tasks\nsummary: Tasks. covers todo.task.AddTask\n---\n# T\n\n## Acceptance\n\nAdding a task shows it in the list.\n");
    put(w.join("aep/story/sync.md"), "---\nid: story:sync\nsummary: Sync. covers todo.sync.SyncChanges\n---\n# S\n");
    put(w.join("requirements.yaml"), "requirements:\n  - {id: r1, text: R1 TASKS TEXT, spec_domains: [todo.task]}\n  - {id: r2, text: R2 SYNC TEXT, spec_domains: [todo.sync]}\n  - {id: r3, text: R3 NOBODY TEXT, spec_domains: [todo.access]}\n");
    put(w.join(".sdc/gate/ir.json"), &json!({"commands": {"todo.task.AddTask": {"name": "AddTask", "outcomes": ["added"]},
        "todo.sync.SyncChanges": {"name": "SyncChanges", "outcomes": ["SYNC_ONLY_OUTCOME"]}}}).to_string());
    put(w.join("mainline/src/lib.rs"), "pub fn add() {}\n");
    put(w.join("impl/src/lib.rs"), "pub fn add() {}\npub fn remove() {}\n");
    put(w.join("impl/src/new.rs"), "pub fn brand_new() {}\n");
    for d in ["mainline", "impl"] {
        put(w.join(d).join("src/untouched.rs"), "pub fn untouched_fn() {\n    let UNTOUCHED_BODY = 1;\n}\n");
        put(w.join(d).join("README.md"), "UNTOUCHED_README\n");
        put(w.join(d).join("src/server.rs"), "use std::io;\npub struct Server;\npub fn serve() {\n    router.get(\"/api/tasks\");\n}\nfn private_helper() {}\n");
        put(w.join(d).join("public/index.html"), "<html>UNTOUCHED_HTML</html>\n");
    }
    put(case.join("case.json"), &json!({"id": "IDEA-1.app.tasks", "artifacts": {"change": {"revision": "t1"}, "spec": {"revision": "s1"}}}).to_string());
    put(case.join("evidence/e1.json"), &json!({"kind": "conformance_run", "subject_revision": "t1"}).to_string());
    put(case.join("blobs/tests.log"), tests_log);
    put(case.join("events.jsonl"), &(json!({"event": "action_finished", "evidence": ["e1"], "log": "tests.log"}).to_string() + "\n"));
    (cwd, case)
}

const PASSING_LOG: &str = "billing v3 against reference 0.96.0 — passed\n  passed todo.task.AddTask/outcome/added\n  passed todo.task.AddTask/outcome/refused\n\
    2 scenarios: 2 passed, 0 failed, 0 error, 0 unsupported\n\nrunning 2 tests\ntest tests::add ... ok\ntest tests::remove ... ok\n\n\
    test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n";

#[test]
fn the_review_input_holds_the_touched_files_and_no_other() {
    let (cwd, case) = review_case("touched", PASSING_LOG);
    let v = review_inputs(&cwd, &Facts::driver(&case), "tasks").unwrap();
    let doc = fs::read_to_string(cwd.join("in/review.md")).unwrap();
    assert_eq!(v["changed_files"], json!(["src/lib.rs", "src/new.rs"]));
    assert!(doc.contains("### impl/src/lib.rs") && doc.contains("pub fn remove()"), "{doc}");
    assert!(doc.contains("### impl/src/new.rs") && doc.contains("brand_new"), "{doc}");
    // A file outside the diff is in neither the document nor in/.
    assert!(!doc.contains("UNTOUCHED_BODY") && !doc.contains("UNTOUCHED_README") && !doc.contains("UNTOUCHED_HTML"), "{doc}");
    assert!(!cwd.join("in/impl/src/untouched.rs").exists() && !cwd.join("in/impl/README.md").exists());
    assert!(cwd.join("in/impl/src/lib.rs").is_file() && cwd.join("in/impl/src/new.rs").is_file());
    // Line numbers, and a new file's lines once (not again in the diff).
    assert!(doc.contains("  2  pub fn remove() {}"), "{doc}");
    assert_eq!(doc.matches("brand_new").count(), 1, "{doc}");
    assert!(doc.contains("+pub fn remove() {}"), "the diff of the edited file stays: {doc}");
    assert_eq!(v["document_bytes"], doc.len());
}

#[test]
fn the_review_input_holds_the_stories_requirements_and_commands_only() {
    let (cwd, case) = review_case("scope", PASSING_LOG);
    let v = review_inputs(&cwd, &Facts::driver(&case), "tasks").unwrap();
    let doc = fs::read_to_string(cwd.join("in/review.md")).unwrap();
    assert_eq!(v["requirements"], json!(["r1"]));
    assert_eq!(v["uncovered"], json!(["r3"]));
    // The story's requirement and acceptance line; the uncovered one, apart; the other story's, absent.
    assert!(doc.contains("- r1: R1 TASKS TEXT") && doc.contains("Adding a task shows it in the list."), "{doc}");
    assert!(doc.contains("- r3: R3 NOBODY TEXT"), "{doc}");
    assert!(!doc.contains("R2 SYNC TEXT") && !doc.contains("SYNC_ONLY_OUTCOME") && !doc.contains("todo.sync"), "{doc}");
    assert!(doc.contains("- todo.task.AddTask: {") && doc.contains("\"outcomes\""), "{doc}");
    // Every id the schema lets a finding name is in the document.
    let (inside, outside) = review_targets(&cwd.join("in"));
    for id in inside.iter().filter(|i| !i.contains(':')).chain(outside.iter()) {
        assert!(doc.contains(id.as_str()), "{id} missing from {doc}");
    }
    assert!(inside.contains(&"aep/story/tasks.md:9".to_string()) && doc.contains("aep/story/tasks.md:9"), "{doc}");
}

#[test]
fn the_review_input_holds_the_test_summary_not_the_log() {
    let (cwd, case) = review_case("tests", PASSING_LOG);
    review_inputs(&cwd, &Facts::driver(&case), "tasks").unwrap();
    let doc = fs::read_to_string(cwd.join("in/review.md")).unwrap();
    assert!(doc.contains("2 scenarios: 2 passed, 0 failed, 0 error, 0 unsupported"), "{doc}");
    assert!(doc.contains("unit tests: 2 passed, 0 failed, 0 ignored") && doc.contains("failing: none"), "{doc}");
    assert!(!doc.contains("passed todo.task.AddTask/outcome/added") && !doc.contains("test tests::add"), "{doc}");
    assert!(!cwd.join("in/tests.txt").exists() && !cwd.join("in/gate.txt").exists());
    let failing = PASSING_LOG.replace("  passed todo.task.AddTask/outcome/refused", "  failed todo.task.AddTask/outcome/refused")
        .replace("test tests::remove ... ok", "test tests::remove ... FAILED");
    let (cwd, case) = review_case("tests-failing", &failing);
    review_inputs(&cwd, &Facts::driver(&case), "tasks").unwrap();
    let doc = fs::read_to_string(cwd.join("in/review.md")).unwrap();
    assert!(doc.contains("failing: todo.task.AddTask/outcome/refused, tests::remove"), "{doc}");
}

#[test]
fn the_build_constraints_are_in_the_document_when_the_tree_has_them() {
    let (cwd, case) = review_case("no-constraints", PASSING_LOG);
    assert_eq!(review_inputs(&cwd, &Facts::driver(&case), "tasks").unwrap()["build_constraints"], false);
    assert!(!fs::read_to_string(cwd.join("in/review.md")).unwrap().contains("Build constraints"));
    let (cwd, case) = review_case("constraints", PASSING_LOG);
    fs::write(case.join("work/build-constraints.yaml"), "language: rust\n").unwrap();
    assert_eq!(review_inputs(&cwd, &Facts::driver(&case), "tasks").unwrap()["build_constraints"], true);
    assert!(fs::read_to_string(cwd.join("in/review.md")).unwrap().contains("## Build constraints\nlanguage: rust"));
}

#[test]
fn an_absent_file_is_detectable_from_the_listing() {
    let (cwd, case) = review_case("listing", PASSING_LOG);
    let v = review_inputs(&cwd, &Facts::driver(&case), "tasks").unwrap();
    let doc = fs::read_to_string(cwd.join("in/review.md")).unwrap();
    let (_, listing) = doc.split_once("## Product files not in the diff").unwrap_or_else(|| panic!("no listing: {doc}"));
    let listing = listing.split("\n## ").next().unwrap();
    // The untouched files, with their size and for source files their items; the web page by path and size.
    assert!(listing.contains("- src/server.rs (") && listing.contains(" bytes): Server, serve, GET /api/tasks, private_helper"), "{listing}");
    assert!(listing.contains("- src/untouched.rs (") && listing.contains("untouched_fn"), "{listing}");
    assert!(listing.contains("- public/index.html (28 bytes)\n") && listing.contains("- README.md (17 bytes)\n"), "{listing}");
    // A touched file is shown in full below, not listed; a file the tree lacks is not there: no service worker.
    assert!(!listing.contains("src/lib.rs") && !listing.contains("src/new.rs"), "{listing}");
    assert!(!listing.contains("sw.js") && !listing.contains("service"), "{listing}");
    // Bodies are not in it.
    assert!(!doc.contains("UNTOUCHED_BODY") && !doc.contains("UNTOUCHED_HTML"), "{doc}");
    let l = read_json(&cwd.join("in/listing.json"));
    let paths: Vec<&str> = l["files"].as_array().unwrap().iter().filter_map(|f| f["path"].as_str()).collect();
    assert_eq!(paths, vec!["README.md", "public/index.html", "src/server.rs", "src/untouched.rs"]);
    assert_eq!(v["listed_files"], 4);
    assert_eq!(v["document_bytes"], doc.len());
}

#[test]
fn the_listing_skips_build_output_and_what_the_diff_excludes() {
    let (cwd, case) = review_case("listing-skips", PASSING_LOG);
    for p in ["target/debug/x", ".sdc/gate/y", ".git/HEAD", "node_modules/z/index.js"] {
        let f = case.join("work/impl").join(p);
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(f, "x").unwrap();
    }
    review_inputs(&cwd, &Facts::driver(&case), "tasks").unwrap();
    let l = fs::read_to_string(cwd.join("in/listing.json")).unwrap();
    assert!(!l.contains("target/") && !l.contains(".sdc") && !l.contains(".git/") && !l.contains("node_modules"), "{l}");
}

#[test]
fn a_listed_untouched_file_is_cited_by_path_without_a_line() {
    let (cwd, case) = call("listed");
    let check = |at: &str| check_review(&cwd, &Facts::driver(&case), &json!({"decision": "rejected", "summary": "s", "findings": [finding("violation", at, "r1")]}), "p");
    let (ok, v) = check("impl/src/server.rs").unwrap();
    assert!(!ok);
    assert_eq!(v["reasons"], json!(["impl/src/server.rs: it does not (violates r1)"]));
    assert_eq!(v["citations"][0]["in_diff"], false);
    assert_eq!(v["citations"][0]["listed"], true);
    // A touched file may be cited by path too; a line of an untouched file, or a path nobody listed, is refused.
    let (_, v) = check("impl/README.md").unwrap();
    assert_eq!((&v["citations"][0]["in_diff"], &v["citations"][0]["listed"]), (&json!(true), &json!(false)));
    assert!(check("impl/src/server.rs:3").unwrap_err().join("; ").contains("not a line of the change tree"));
    assert!(check("impl/sw.js").unwrap_err().join("; ").contains("neither a listed product file nor a changed file"));
    assert!(check("impl/").unwrap_err().join("; ").contains("neither a listed product file nor a changed file"));
    assert!(check("impl/../in/scope.json").unwrap_err().join("; ").contains("neither"));
    // The reply schema allows the path alone.
    let p = review_schema(&cwd.join("in"))["properties"]["findings"]["items"]["properties"]["at"]["pattern"].as_str().unwrap().to_string();
    let re = regex::Regex::new(&p).unwrap();
    assert!(re.is_match("impl/src/server.rs") && re.is_match("impl/src/server.rs:3") && re.is_match("impl/src/server.rs:3-5"));
    assert!(!re.is_match("src/server.rs") && !re.is_match("impl/a:b"));
}

// ---- SDC_REVIEW_MODE: session (the default, main's review) or facts --------------------------

#[test]
fn the_review_mode_is_session_unless_set_to_facts() {
    assert_eq!(review_mode(None), Ok(false));
    assert_eq!(review_mode(Some("")), Ok(false));
    assert_eq!(review_mode(Some("session")), Ok(false));
    assert_eq!(review_mode(Some("facts")), Ok(true));
    assert_eq!(review_mode(Some("both")), Err("SDC_REVIEW_MODE=both: neither session nor facts".to_string()));
}

/// The default review inputs are main's (before story:review-minimal-facts): the story, scope,
/// uncovered requirements, the covered commands, the whole change tree, the diff and the test and
/// gate logs as they are; no review document and no listing.
#[test]
fn the_session_review_inputs_are_the_whole_tree_and_the_logs() {
    let (cwd, case) = review_case("session", PASSING_LOG);
    let v = review_inputs_session(&cwd, &Facts::driver(&case), "tasks").unwrap();
    let mut keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["base", "changed_files", "covers", "gate", "requirements", "story", "uncovered"]);
    assert_eq!(v["changed_files"], json!(["src/lib.rs", "src/new.rs"]));
    assert_eq!((&v["requirements"], &v["uncovered"], &v["gate"], &v["base"]), (&json!(["r1"]), &json!(["r3"]), &json!(false), &json!("mainline")));
    let mut files = walk(&cwd.join("in"));
    files.sort();
    assert_eq!(files, ["aep/story/tasks.md", "change.diff", "commands.json", "gate.txt", "impl/README.md", "impl/public/index.html",
        "impl/src/lib.rs", "impl/src/new.rs", "impl/src/server.rs", "impl/src/untouched.rs", "scope.json", "tests.txt", "uncovered.json"]);
    assert_eq!(fs::read_to_string(cwd.join("in/tests.txt")).unwrap(), PASSING_LOG);
    assert_eq!(fs::read_to_string(cwd.join("in/gate.txt")).unwrap(), "no gate run recorded on this specification revision\n");
    assert!(read_json(&cwd.join("in/commands.json"))["todo.task.AddTask"].is_object());
}

/// A session review (no in/listing.json) cites lines only, as on main, and its citations carry no
/// `listed`; its schema's `at` needs a line.
#[test]
fn a_session_review_cites_lines_only() {
    let (cwd, case) = call("session");
    fs::remove_file(cwd.join("in/listing.json")).unwrap();
    let reply = json!({"decision": "rejected", "summary": "s", "findings": [finding("violation", "impl/README.md:2", "r1")]});
    let (ok, v) = check_review(&cwd, &Facts::driver(&case), &reply, "p").unwrap();
    assert!(!ok);
    assert_eq!(v["citations"], json!([{"at": "impl/README.md:2", "violates": "r1", "kind": "violation", "in_diff": true}]));
    let by_path = json!({"decision": "rejected", "summary": "s", "findings": [finding("violation", "impl/README.md", "r1")]});
    assert!(check_review(&cwd, &Facts::driver(&case), &by_path, "p").unwrap_err().join("; ").contains("is not a line of the change tree"));
    assert_eq!(review_schema(&cwd.join("in"))["properties"]["findings"]["items"]["properties"]["at"]["pattern"], "^impl/[^:]+:[0-9]+(-[0-9]+)?$");
}

// --- under Loom: the facts are the workspace's records (Facts::workspace) ---------------------

/// A review case whose workspace is all there is: the driver's case files are removed.
fn workspace_case(name: &str) -> (PathBuf, Facts) {
    let (cwd, case) = review_case(name, PASSING_LOG);
    for f in ["case.json", "events.jsonl"] {
        fs::remove_file(case.join(f)).unwrap();
    }
    for d in ["evidence", "blobs"] {
        fs::remove_dir_all(case.join(d)).unwrap();
    }
    (cwd, Facts::workspace(&case.join("work")))
}

#[test]
fn a_workspace_review_has_no_inputs_before_the_tests_ran_on_the_tree() {
    let (cwd, facts) = workspace_case("ws-none");
    let e = review_inputs(&cwd, &facts, "tasks").unwrap_err();
    assert!(e.starts_with("no conformance run recorded on change revision "), "{e}");
    assert!(!e.contains("revision none"), "the tree has a digest: {e}");
}

#[test]
fn a_workspace_review_reads_the_logs_of_the_records_like_the_drivers() {
    let (cwd, facts) = workspace_case("ws-logs");
    facts.write_record("conformance_run", "impl", PASSING_LOG).unwrap();
    facts.write_record("ess_gate", "spec", "gate log\n").unwrap_err();
    fs::create_dir_all(facts.work().join("spec")).unwrap();
    fs::write(facts.work().join("spec/system.yaml"), "system: tasks\n").unwrap();
    facts.write_record("ess_gate", "spec", "gate log\n").unwrap();
    let v = review_inputs(&cwd, &facts, "tasks").unwrap();
    assert_eq!(v["gate"], true);
    // The document the model reads is the one the driver's case gives (the same tests log).
    let (dcwd, dcase) = review_case("ws-logs-driver", PASSING_LOG);
    review_inputs(&dcwd, &Facts::driver(&dcase), "tasks").unwrap();
    assert_eq!(fs::read(cwd.join("in/review.md")).unwrap(), fs::read(dcwd.join("in/review.md")).unwrap());
}

#[test]
fn a_workspace_review_reads_the_session_inputs_from_the_records_too() {
    let (cwd, facts) = workspace_case("ws-session");
    facts.write_record("conformance_run", "impl", PASSING_LOG).unwrap();
    review_inputs_session(&cwd, &facts, "tasks").unwrap();
    assert_eq!(fs::read_to_string(cwd.join("in/tests.txt")).unwrap(), PASSING_LOG);
    assert_eq!(fs::read_to_string(cwd.join("in/gate.txt")).unwrap(), "no gate run recorded on this specification revision\n");
    fs::create_dir_all(facts.work().join("spec")).unwrap();
    fs::write(facts.work().join("spec/system.yaml"), "system: tasks\n").unwrap();
    facts.write_record("ess_gate", "spec", "gate log\n").unwrap();
    review_inputs_session(&cwd, &facts, "tasks").unwrap();
    assert_eq!(fs::read_to_string(cwd.join("in/gate.txt")).unwrap(), "gate log\n");
}

#[test]
fn a_run_recorded_on_an_earlier_tree_is_no_run_on_the_changed_one() {
    let (cwd, facts) = workspace_case("ws-changed");
    facts.write_record("conformance_run", "impl", PASSING_LOG).unwrap();
    review_inputs(&cwd, &facts, "tasks").unwrap();
    fs::write(facts.work().join("impl/src/new.rs"), "pub fn brand_new_again() {}\n").unwrap();
    assert!(review_inputs(&cwd, &facts, "tasks").unwrap_err().starts_with("no conformance run recorded on change revision "));
    // Build output, `.sdc` and `.derived-from` are no part of the revision.
    fs::write(facts.work().join("impl/src/new.rs"), "pub fn brand_new() {}\n").unwrap();
    fs::create_dir_all(facts.work().join("impl/target/debug")).unwrap();
    fs::write(facts.work().join("impl/target/debug/x"), "x").unwrap();
    fs::create_dir_all(facts.work().join("impl/.sdc")).unwrap();
    fs::write(facts.work().join("impl/.sdc/coverage.json"), "{}").unwrap();
    fs::write(facts.work().join("impl/.derived-from"), "x\n").unwrap();
    review_inputs(&cwd, &facts, "tasks").unwrap();
}

#[test]
fn a_workspace_keeps_its_rejections_and_counts_them() {
    let (cwd, facts) = workspace_case("ws-reject");
    fs::create_dir_all(cwd.join("in/impl")).unwrap();
    fs::write(cwd.join("in/impl/README.md"), "# App\nrun it\nend\n").unwrap();
    fs::write(cwd.join("in/change.diff"), "+++ impl/README.md\t2026\n").unwrap();
    fs::write(cwd.join("in/scope.json"), json!({"covers": [], "requirements": [{"id": "r1"}], "acceptance": []}).to_string()).unwrap();
    fs::write(cwd.join("in/uncovered.json"), json!({"requirements": []}).to_string()).unwrap();
    assert_eq!(facts.rejections("change.review"), json!([]));
    let reply = json!({"decision": "rejected", "summary": "s", "findings": [finding("violation", "impl/README.md:2", "r1")]});
    let (ok, first) = check_review(&cwd, &facts, &reply, "llm-review:m").unwrap();
    assert!(!ok);
    assert_eq!((first["count"].clone(), first["revision"].clone()), (json!(1), facts.revision_value("change")));
    assert_eq!(first["revision"].as_str().map(str::len), Some(16));
    let (_, second) = check_review(&cwd, &facts, &reply, "llm-review:m").unwrap();
    assert_eq!(second["count"], 2);
    let kept = facts.rejections("change.review");
    assert_eq!(kept.as_array().map(Vec::len), Some(2));
    assert_eq!(kept[0]["record"], "001");
    assert_eq!(kept[1]["principal"], "llm-review:m");
    assert_eq!(kept[1]["reasons"], second["reasons"]);
    // An approval keeps nothing, an invalid reply keeps nothing.
    let approved = json!({"decision": "approved", "summary": "s", "findings": []});
    check_review(&cwd, &facts, &approved, "p").unwrap();
    check_review(&cwd, &facts, &json!({"decision": "maybe"}), "p").unwrap_err();
    assert_eq!(facts.rejections("change.review").as_array().map(Vec::len), Some(2));
}

#[test]
fn the_driver_keeps_no_records_of_its_own() {
    let (cwd, case) = call("driver-keeps-none");
    let reply = json!({"decision": "rejected", "summary": "s", "findings": [finding("violation", "impl/README.md:2", "r1")]});
    check_review(&cwd, &Facts::driver(&case), &reply, "p").unwrap();
    assert!(!case.join("work").join(RECORDS).exists());
}

#[test]
fn a_record_names_a_tree_that_is_there_and_a_plain_kind() {
    let (_, facts) = workspace_case("ws-record-refused");
    assert!(facts.write_record("conformance_run", "nowhere", "x").unwrap_err().contains("no such tree"));
    assert!(facts.write_record("../x", "impl", "x").unwrap_err().contains("no record kind"));
    assert!(facts.write_record("a/b", "impl", "x").is_err());
}

#[test]
fn a_workspace_that_is_not_the_work_of_a_case_directory_is_found_from_both_names() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-scratch/review-inputs/layouts");
    let _ = fs::remove_dir_all(&root);
    let (old, split) = (root.join("old/case/work"), root.join("split/work/1"));
    fs::create_dir_all(&old).unwrap();
    fs::create_dir_all(&split).unwrap();
    assert_eq!(case_work(&old.join("..")), old.join("../work"));
    assert_eq!(case_work(&split), split);
    assert_eq!(case_dir_of(&old), old.join(".."));
    assert_eq!(case_dir_of(&split), split);
}
