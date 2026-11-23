//! The coverage document of `change.implement`: per requirement of the story `built` with a file,
//! a line and a test, or `not_built` with a reason; checked against the schema and the change tree.
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::review::scope_of;
use super::{read_json, sorted, walk};

// --- change.implement's requirement coverage (story:implement-cites-requirements) ---------------

/// Where `sdc-node` puts the implementer's coverage document: inside the change tree, so it is
/// part of the revision it describes (`.sdc/` is left out of the review's copy and diff).
pub const COVERAGE: &str = ".sdc/coverage.json";

/// The requirement ids the story carries: the requirements review_scope gives the story.
pub fn story_requirements(work: &Path, story: &str) -> Result<Vec<Value>, String> {
    Ok(scope_of(work, story)?.mine)
}

/// The coverage document checked by program against coverage_schema: why it does not fit, empty
/// when it does.
pub fn coverage_shape(doc: &Value, ids: &[String]) -> Vec<String> {
    let mut why = Vec::new();
    let Some(top) = doc.as_object() else { return vec!["the coverage document is not a JSON object".into()] };
    for k in top.keys().filter(|k| *k != "requirements") {
        why.push(format!("unknown key {k} (only `requirements`)"));
    }
    let Some(list) = top.get("requirements").and_then(|v| v.as_array()) else {
        why.push("`requirements` is missing or not a list".into());
        return why;
    };
    for (i, e) in list.iter().enumerate() {
        let n = i + 1;
        let Some(o) = e.as_object() else { why.push(format!("entry {n} is not an object")); continue };
        let id = o.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let what = format!("entry {n} ({})", if id.is_empty() { "no id" } else { id });
        if !ids.iter().any(|x| x == id) {
            why.push(format!("{what}: {id} is not a requirement of this story ({})", if ids.is_empty() { "none".into() } else { ids.join(", ") }));
        }
        let text = |k: &str| o.get(k).and_then(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty());
        let keys: &[&str] = match o.get("status").and_then(|v| v.as_str()) {
            Some("built") => {
                match o.get("at").and_then(|v| v.as_array()) {
                    Some(at) if !at.is_empty() => {
                        for (j, a) in at.iter().enumerate() {
                            let ok = a.as_object().is_some_and(|m| m.len() == 2
                                && m.get("file").and_then(|f| f.as_str()).is_some_and(|f| !f.is_empty())
                                && m.get("line").and_then(|l| l.as_u64()).is_some_and(|l| l >= 1));
                            if !ok {
                                why.push(format!("{what}: at[{j}] must be {{file, line}} with a file name and a line of at least 1"));
                            }
                        }
                    }
                    _ => why.push(format!("{what}: built needs `at`, a list of at least one {{file, line}}")),
                }
                if text("test").is_none() {
                    why.push(format!("{what}: built needs `test`, the name of a test that exercises it"));
                }
                &["id", "status", "at", "test"]
            }
            Some("not_built") => {
                if text("reason").is_none() {
                    why.push(format!("{what}: not_built needs `reason`, one sentence"));
                }
                &["id", "status", "reason"]
            }
            _ => {
                why.push(format!("{what}: status must be built or not_built"));
                continue;
            }
        };
        for k in o.keys().filter(|k| !keys.contains(&k.as_str())) {
            why.push(format!("{what}: unknown key {k} for status {}", o["status"].as_str().unwrap_or("")));
        }
    }
    why
}

/// The `.rs` files of a tree, relative to it, outside `target/` and `.sdc/`.
pub fn rust_sources(tree: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let Ok(rd) = fs::read_dir(tree.join(&rel)) else { continue };
        for e in rd.flatten() {
            let name = e.file_name();
            if name == "target" || name == ".sdc" {
                continue;
            }
            let r = rel.join(&name);
            if e.path().is_dir() {
                stack.push(r)
            } else if r.extension().is_some_and(|x| x == "rs") {
                out.push(r.to_string_lossy().into_owned())
            }
        }
    }
    out.sort();
    out
}

/// The names of the functions marked `#[test]` in the tree's Rust sources: a `fn <name>` after a
/// line holding `#[test]` with only attributes, comments or blank lines between.
pub fn test_names(tree: &Path) -> BTreeSet<String> {
    let fn_re = regex::Regex::new(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    let mut names = BTreeSet::new();
    for f in rust_sources(tree) {
        let text = fs::read_to_string(tree.join(&f)).unwrap_or_default();
        let mut marked = false;
        for line in text.lines() {
            let t = line.trim();
            if let Some(c) = fn_re.captures(t) {
                if marked || t.contains("#[test]") {
                    names.insert(c[1].to_string());
                }
                marked = false;
            } else if t.contains("#[test]") {
                marked = true;
            } else if !(t.is_empty() || t.starts_with("#[") || t.starts_with("//")) {
                marked = false;
            }
        }
    }
    names
}

/// The coverage document checked against the change tree: (requirement id, fault) for every id of
/// the story that is missing, named more than once or `not_built`, every cited file that is not in
/// the tree, every cited line outside its file and every named test the test sources lack; and
/// every id the story does not carry. Empty when the coverage is complete.
pub fn check_coverage(tree: &Path, doc: &Value, ids: &[String]) -> Vec<(String, String)> {
    let entries: Vec<&Value> = doc["requirements"].as_array().into_iter().flatten().collect();
    let tests = test_names(tree);
    let mut faults = Vec::new();
    for id in ids {
        let mine: Vec<&&Value> = entries.iter().filter(|e| e["id"] == id.as_str()).collect();
        let e = match mine.len() {
            0 => { faults.push((id.clone(), "missing: the coverage document does not name it".into())); continue }
            1 => mine[0],
            n => { faults.push((id.clone(), format!("named {n} times: name it exactly once"))); continue }
        };
        if e["status"] != "built" {
            let reason = e["reason"].as_str().map(str::trim).filter(|r| !r.is_empty()).unwrap_or("no reason given");
            faults.push((id.clone(), format!("not built: {reason}")));
            continue;
        }
        let at = e["at"].as_array().cloned().unwrap_or_default();
        if at.is_empty() {
            faults.push((id.clone(), "built, but cites no file and line".into()));
        }
        for a in &at {
            let raw = a["file"].as_str().unwrap_or("");
            let file = raw.strip_prefix("impl/").unwrap_or(raw);
            let line = a["line"].as_u64().unwrap_or(0);
            let p = Path::new(file);
            if file.is_empty() || p.is_absolute() || p.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
                faults.push((id.clone(), format!("{raw}:{line}: not a path inside the change tree")));
                continue;
            }
            if !tree.join(p).is_file() {
                faults.push((id.clone(), format!("{file}:{line}: no file {file} in the change tree")));
                continue;
            }
            let n = fs::read_to_string(tree.join(p)).map(|t| t.lines().count()).unwrap_or(0);
            if line < 1 || line as usize > n {
                faults.push((id.clone(), format!("{file}:{line} is outside {file} ({n} lines)")));
            }
        }
        let test = e["test"].as_str().unwrap_or("").trim();
        let name = test.rsplit("::").next().unwrap_or(test).trim_end_matches("()");
        if !tests.contains(name) {
            faults.push((id.clone(), format!("test {test}: no #[test] function of that name in the test sources")));
        }
    }
    for e in &entries {
        let id = e["id"].as_str().unwrap_or("");
        if !ids.iter().any(|x| x == id) {
            faults.push((id.to_string(), format!("{id} is not a requirement of this story ({})", if ids.is_empty() { "none".into() } else { ids.join(", ") })));
        }
    }
    faults
}

/// The refusal as the step's log: one line per fault, the refused ids last (the line
/// `writer_spent` and the next run's in/feedback.md show).
pub fn refusal_text(story: &str, faults: &[(String, String)]) -> String {
    let mut ids: Vec<&str> = Vec::new();
    for (id, _) in faults {
        if id != "document" && !ids.contains(&id.as_str()) {
            ids.push(id);
        }
    }
    let mut s = format!("requirement coverage of story {story} refused: the change was not handed to tests.run\n");
    for (id, f) in faults {
        s.push_str(&format!("- {id}: {f}\n"));
    }
    s.push_str("Build every requirement the story carries, cite the file and line that builds it and a #[test] that exercises it, in out/coverage.json.\n");
    s.push_str(&format!("refused requirements: {}\n", ids.join(" ")));
    s
}

/// The stub implementer's coverage document (no model): every requirement built at the first line
/// of the first Rust source, exercised by the first test; with no test in the tree, not built.
pub fn coverage_stub(tree: &Path, ids: &[String]) -> Value {
    let srcs = rust_sources(tree);
    let file = srcs.iter().find(|f| f.starts_with("src/")).or(srcs.first()).cloned()
        .or_else(|| walk(tree).into_iter().find(|f| !f.starts_with(".sdc/") && !f.starts_with("target/")));
    let test = test_names_in_order(tree).into_iter().next();
    let reqs: Vec<Value> = ids.iter().map(|id| match (&file, &test) {
        (Some(f), Some(t)) => json!({"id": id, "status": "built", "at": [{"file": f, "line": 1}], "test": t}),
        _ => json!({"id": id, "status": "not_built", "reason": "the stub tree has no test"}),
    }).collect();
    sorted(json!({"requirements": reqs}))
}

/// The test names in source order (files sorted), for the stub document.
pub fn test_names_in_order(tree: &Path) -> Vec<String> {
    let all = test_names(tree);
    let fn_re = regex::Regex::new(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    let mut out = Vec::new();
    for f in rust_sources(tree) {
        for c in fn_re.captures_iter(&fs::read_to_string(tree.join(&f)).unwrap_or_default()) {
            if all.contains(&c[1]) && !out.contains(&c[1].to_string()) {
                out.push(c[1].to_string());
            }
        }
    }
    out
}

/// The story of the driver's case (`SDC_CASE`, `<root>.<epic>.<story>`) when none is given.
pub fn story_arg(story: Option<String>) -> Result<String, String> {
    story.or_else(|| std::env::var("SDC_CASE").ok().and_then(|c| c.split('.').nth(2).map(String::from)))
        .filter(|s| !s.is_empty()).ok_or("no --story and no story in SDC_CASE".into())
}

/// The case's `work/`: `--work` (`sdc-node` passes its own), else `<case dir>/work`.
pub fn work_arg(work: Option<PathBuf>, case_dir: Option<PathBuf>) -> Result<PathBuf, String> {
    match work {
        Some(w) => Ok(w),
        None => case_dir_arg(case_dir).map(|d| d.join("work")),
    }
}

/// The case directory: `--case-dir`, else the driver's SDC_CASE_DIR.
pub fn case_dir_arg(case_dir: Option<PathBuf>) -> Result<PathBuf, String> {
    case_dir.or_else(|| std::env::var("SDC_CASE_DIR").ok().map(PathBuf::from)).ok_or("no --case-dir and no SDC_CASE_DIR".into())
}

/// `check-coverage`: exit 0 complete, 1 refused (stdout names the ids), 2 when it cannot check.
pub fn run_check_coverage(work: Option<PathBuf>, case_dir: Option<PathBuf>, story: Option<String>, tree: &Path, reply: Option<PathBuf>, shape: bool) -> Result<u8, String> {
    let (code, out, err) = check_coverage_text(work, case_dir, story, tree, reply, shape)?;
    print!("{out}");
    eprint!("{err}");
    Ok(code)
}

/// `check-coverage` as text: the exit, what it prints on standard output and what it says on
/// standard error. `Err` when it cannot check (no story, no readable requirements).
pub fn check_coverage_text(work: Option<PathBuf>, case_dir: Option<PathBuf>, story: Option<String>, tree: &Path, reply: Option<PathBuf>, shape: bool) -> Result<(u8, String, String), String> {
    let story = story_arg(story)?;
    let reqs = story_requirements(&work_arg(work, case_dir)?, &story)?;
    let ids: Vec<String> = reqs.iter().filter_map(|r| r["id"].as_str().map(String::from)).collect();
    let file = reply.unwrap_or(tree.join(COVERAGE));
    let doc = read_json(&file);
    if shape {
        let why = coverage_shape(&doc, &ids);
        if why.is_empty() {
            return Ok((0, String::new(), String::new()));
        }
        let name = file.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let what = if doc.is_null() { format!("no readable coverage document ({name}: the structured reply, else out/coverage.json)") } else { why.join("; ") };
        return Ok((2, String::new(), format!("invalid coverage reply: {what}\n")));
    }
    let faults = if doc.is_null() {
        ids.iter().map(|id| (id.clone(), format!("no coverage document at impl/{COVERAGE}"))).collect()
    } else {
        let mut f: Vec<(String, String)> = coverage_shape(&doc, &ids).into_iter().map(|w| ("document".to_string(), w)).collect();
        f.extend(check_coverage(tree, &doc, &ids));
        f
    };
    if faults.is_empty() {
        let tests: BTreeSet<&str> = doc["requirements"].as_array().into_iter().flatten().filter_map(|e| e["test"].as_str()).collect();
        let line = format!("requirement coverage of story {story}: {} built and cited ({}), tests {}\n", ids.len(),
            if ids.is_empty() { "the story carries no requirement".into() } else { ids.join(" ") },
            if tests.is_empty() { "none".into() } else { tests.into_iter().collect::<Vec<_>>().join(" ") });
        return Ok((0, line, String::new()));
    }
    Ok((1, refusal_text(&story, &faults), String::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reply::schemas::coverage_schema;

    /// A change tree: src/lib.rs (4 lines), tests/billing.rs with two #[test] functions and one
    /// helper, a test module in src/lib.rs, and target/ (never read).
    fn tree(name: &str) -> PathBuf {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-scratch/coverage").join(name);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::create_dir_all(root.join("target/debug")).unwrap();
        fs::write(root.join("src/lib.rs"), "pub fn add() {}\npub fn send() {}\n#[cfg(test)]\nmod t { #[test]\n fn unit_add() {} }\n").unwrap();
        fs::write(root.join("tests/billing.rs"), "fn helper() {}\n\n#[test]\nfn create_invoice() {}\n\n// covers SendEmail\n#[test]\n#[allow(unused)]\npub fn send_email() { helper() }\n").unwrap();
        fs::write(root.join("target/debug/x.rs"), "#[test]\nfn in_target() {}\n").unwrap();
        root
    }

    fn built(id: &str, file: &str, line: u64, test: &str) -> Value {
        json!({"id": id, "status": "built", "at": [{"file": file, "line": line}], "test": test})
    }

    fn reqs(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_names_are_the_functions_marked_test() {
        let t = tree("names");
        let names = test_names(&t);
        assert_eq!(names.into_iter().collect::<Vec<_>>(), vec!["create_invoice", "send_email", "unit_add"]);
    }

    #[test]
    fn complete_coverage_passes() {
        let t = tree("complete");
        let doc = json!({"requirements": [built("r1", "src/lib.rs", 1, "create_invoice"), built("r2", "impl/src/lib.rs", 2, "tests::send_email")]});
        assert_eq!(coverage_shape(&doc, &reqs(&["r1", "r2"])), Vec::<String>::new());
        assert_eq!(check_coverage(&t, &doc, &reqs(&["r1", "r2"])), Vec::<(String, String)>::new());
    }

    #[test]
    fn not_built_and_missing_ids_are_refused_by_id() {
        let t = tree("refused");
        let doc = json!({"requirements": [built("r1", "src/lib.rs", 1, "create_invoice"),
            {"id": "r2", "status": "not_built", "reason": "no offline page yet"}]});
        let faults = check_coverage(&t, &doc, &reqs(&["r1", "r2", "r3"]));
        assert_eq!(faults, vec![("r2".to_string(), "not built: no offline page yet".to_string()),
            ("r3".to_string(), "missing: the coverage document does not name it".to_string())]);
        let msg = refusal_text("invoices", &faults);
        assert!(msg.ends_with("refused requirements: r2 r3\n"), "{msg}");
    }

    #[test]
    fn a_cited_line_outside_its_file_or_a_missing_file_is_refused() {
        let t = tree("lines");
        let doc = json!({"requirements": [built("r1", "src/lib.rs", 99, "create_invoice"), built("r2", "src/sync.rs", 1, "send_email"),
            built("r3", "../outside.rs", 1, "send_email")]});
        let faults = check_coverage(&t, &doc, &reqs(&["r1", "r2", "r3"]));
        assert_eq!(faults[0], ("r1".to_string(), "src/lib.rs:99 is outside src/lib.rs (5 lines)".to_string()));
        assert_eq!(faults[1], ("r2".to_string(), "src/sync.rs:1: no file src/sync.rs in the change tree".to_string()));
        assert_eq!(faults[2].0, "r3");
        assert_eq!(faults.len(), 3);
    }

    #[test]
    fn a_named_test_that_does_not_exist_is_refused() {
        let t = tree("tests");
        // helper is a function but no test; in_target lies under target/.
        let doc = json!({"requirements": [built("r1", "src/lib.rs", 1, "helper"), built("r2", "src/lib.rs", 1, "in_target")]});
        let faults = check_coverage(&t, &doc, &reqs(&["r1", "r2"]));
        assert_eq!(faults, vec![("r1".to_string(), "test helper: no #[test] function of that name in the test sources".to_string()),
            ("r2".to_string(), "test in_target: no #[test] function of that name in the test sources".to_string())]);
    }

    #[test]
    fn an_id_named_twice_is_refused() {
        let t = tree("twice");
        let doc = json!({"requirements": [built("r1", "src/lib.rs", 1, "create_invoice"), built("r1", "src/lib.rs", 2, "send_email")]});
        assert_eq!(check_coverage(&t, &doc, &reqs(&["r1"])), vec![("r1".to_string(), "named 2 times: name it exactly once".to_string())]);
    }

    #[test]
    fn the_shape_follows_the_schema() {
        let ids = reqs(&["r1", "r2"]);
        let schema = coverage_schema(&ids);
        assert_eq!(schema["required"], json!(["requirements"]));
        assert_eq!(schema["properties"]["requirements"]["items"]["anyOf"].as_array().unwrap().len(), 2);
        let shape = |v: Value| coverage_shape(&v, &ids).join("; ");
        assert!(shape(Value::Null).contains("not a JSON object"));
        assert!(shape(json!({"requirements": [], "extra": 1})).contains("unknown key extra"));
        assert!(shape(json!({"requirements": [{"id": "r9", "status": "built", "at": [{"file": "a", "line": 1}], "test": "t"}]})).contains("r9 is not a requirement of this story"));
        assert!(shape(json!({"requirements": [{"id": "r1", "status": "built", "at": [], "test": "t"}]})).contains("at least one"));
        assert!(shape(json!({"requirements": [{"id": "r1", "status": "built", "at": [{"file": "a", "line": 0}], "test": "t"}]})).contains("line"));
        assert!(shape(json!({"requirements": [{"id": "r1", "status": "built", "at": [{"file": "a", "line": 1}], "test": ""}]})).contains("test"));
        assert!(shape(json!({"requirements": [{"id": "r1", "status": "not_built", "reason": " "}]})).contains("reason"));
        assert!(shape(json!({"requirements": [{"id": "r1", "status": "maybe"}]})).contains("status"));
        assert_eq!(shape(json!({"requirements": [{"id": "r1", "status": "not_built", "reason": "later"}]})), "");
    }

    #[test]
    fn the_stub_document_cites_the_first_source_line_and_test() {
        let t = tree("stub");
        let doc = coverage_stub(&t, &reqs(&["r1", "r2"]));
        assert_eq!(doc["requirements"][0], built("r1", "src/lib.rs", 1, "unit_add"));
        assert_eq!(check_coverage(&t, &doc, &reqs(&["r1", "r2"])), Vec::<(String, String)>::new());
        // A tree without a test: every requirement not built.
        fs::remove_dir_all(t.join("tests")).unwrap();
        fs::write(t.join("src/lib.rs"), "pub fn add() {}\n").unwrap();
        assert_eq!(coverage_stub(&t, &reqs(&["r1"]))["requirements"][0]["status"], "not_built");
    }
}
