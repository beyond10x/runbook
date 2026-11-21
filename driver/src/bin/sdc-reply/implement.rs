//! change.implement as one request (story:implement-one-request; `SDC_IMPLEMENT_MODE=request`,
//! docs/driver.md "Implement as one request").
//!
//! - `assemble`: the node's whole input in one prompt; the node reads nothing.
//! - `select_tree`: which files of the product tree are inlined (the input rule, below).
//! - `check_reply`: the reply's envelope checked against `schema`; `confine` and `write_files`:
//!   the files written into the call's `out/impl/`, never outside it.
//! - `run`: the requests of one implement call: the first, at most `MAX_RETRIES` retries with the
//!   check's error, at most `MAX_CONTINUATIONS` continuations by rule; never a tool loop.
//!
//! The input rule (`select_tree`): candidates are the files of the product tree (`impl/`), less
//! build output (`target/`) and hidden entries (`.sdc`, `.derived-from`, `.rework`). A file whose
//! text is not UTF-8 is listed, never inlined. Each candidate gets a rank: 0 its path is named in
//! the story's text; 1 this story's change: absent from or different in `mainline/` (the tree the
//! epic's earlier stories merged), which for a first story (no `mainline/`) is the whole tree;
//! 2 a crate root file (`ROOT_FILES`); 3 a file earlier stories merged and this story has not
//! touched. In rank and path order a file is inlined while the inlined bytes stay within the cap
//! (`INLINE_CAP`); every other file is listed by path and size only.
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Map, Value};

use super::{covers, read_data, read_json, reply_schema, review_scope, story_summary, walk};

/// Bytes of file text inlined per request; the rest of the tree is listed by path and size.
pub const INLINE_CAP: usize = 64_000;
/// Requests after an invalid reply, per implement call (AGENTS.md: at most two retries).
pub const MAX_RETRIES: usize = 2;
/// Continuation requests per implement call, each for the paths a valid reply left `remaining`.
pub const MAX_CONTINUATIONS: usize = 2;
/// The crate's root files: inlined after the story's own files.
pub const ROOT_FILES: [&str; 5] = ["Cargo.toml", "src/lib.rs", "src/main.rs", "README.md", "Taskfile.yml"];

#[derive(Debug, Clone, PartialEq)]
pub struct TreeFile {
    pub path: String,
    pub bytes: usize,
    pub why: &'static str,
}

#[derive(Debug, Default)]
pub struct Selection {
    pub first_story: bool,
    pub inlined: Vec<TreeFile>,
    pub listed: Vec<TreeFile>,
    pub inlined_bytes: usize,
}

/// Build output or a hidden entry: never a file of the product.
fn not_product(rel: &str) -> bool {
    rel.split('/').next() == Some("target") || rel.split('/').any(|c| c.starts_with('.'))
}

/// The product files of a tree, sorted: no build output, no hidden entry.
fn product_files(dir: &Path) -> Vec<String> {
    if !dir.is_dir() {
        return vec![];
    }
    walk(dir).into_iter().filter(|p| !not_product(p)).collect()
}

/// The input rule (module docs).
pub fn select_tree(tree: &Path, mainline: &Path, story_text: &str, cap: usize) -> Selection {
    let base: BTreeSet<String> = product_files(mainline).into_iter().collect();
    let first_story = base.is_empty();
    let mut ranked = Vec::new();
    let mut listed = Vec::new();
    for p in product_files(tree) {
        let bytes = fs::read(tree.join(&p)).unwrap_or_default();
        let n = bytes.len();
        if String::from_utf8(bytes.clone()).is_err() {
            listed.push(TreeFile { path: p, bytes: n, why: "not UTF-8 text" });
            continue;
        }
        let (rank, why) = if story_text.contains(p.as_str()) {
            (0, "named by the story")
        } else if first_story {
            (1, "first story: the whole tree")
        } else if !base.contains(&p) || fs::read(mainline.join(&p)).ok().as_deref() != Some(bytes.as_slice()) {
            (1, "this story's change")
        } else if ROOT_FILES.contains(&p.as_str()) {
            (2, "crate root")
        } else {
            (3, "merged by an earlier story")
        };
        ranked.push((rank, TreeFile { path: p, bytes: n, why }));
    }
    ranked.sort_by(|a, b| (a.0, &a.1.path).cmp(&(b.0, &b.1.path)));
    let mut s = Selection { first_story, ..Selection::default() };
    for (_, f) in ranked {
        if s.inlined_bytes + f.bytes <= cap {
            s.inlined_bytes += f.bytes;
            s.inlined.push(f);
        } else {
            listed.push(TreeFile { why: "over the cap", ..f });
        }
    }
    listed.sort_by(|a, b| a.path.cmp(&b.path));
    s.listed = listed;
    s
}

/// A reply path, checked: relative, normal, inside the product tree, not build output, not
/// hidden. Ok(the path) or Err(why).
pub fn confine(p: &str) -> Result<String, String> {
    let why = |w: &str| Err(format!("path {p:?}: {w}"));
    if p.is_empty() {
        return why("an empty path");
    }
    if p.contains('\\') {
        return why("a backslash: write paths with /");
    }
    let b = p.as_bytes();
    if p.starts_with('/') || (b.len() > 1 && b[1] == b':' && b[0].is_ascii_alphabetic()) {
        return why("an absolute path: write it relative to impl/ (for example src/lib.rs)");
    }
    if p.contains('\0') {
        return why("a NUL byte");
    }
    for c in p.split('/') {
        match c {
            ".." => return why("leaves the product tree (`..`)"),
            "" | "." => return why("not a normal relative path (an empty or `.` segment)"),
            _ if c.starts_with('.') => return why("a hidden entry: .sdc, .git, .rework and the like are not the product's"),
            _ => {}
        }
    }
    if p.split('/').next() == Some("target") {
        return why("build output (target/) is not the product's");
    }
    Ok(p.to_string())
}

/// Write the files into `root` (the call's `out/impl/`). Every path is checked first (`confine`,
/// and no existing symlink or file on the way): one refused path writes nothing.
pub fn write_files(root: &Path, files: &[(String, String)]) -> Result<Vec<String>, String> {
    for (p, _) in files {
        confine(p)?;
        let mut at = root.to_path_buf();
        let parts: Vec<&str> = p.split('/').collect();
        for (i, c) in parts.iter().enumerate() {
            at.push(c);
            let Ok(m) = fs::symlink_metadata(&at) else { break };
            if m.file_type().is_symlink() {
                return Err(format!("path {p:?}: goes through a symlink ({})", parts[..=i].join("/")));
            }
            if i + 1 < parts.len() && !m.is_dir() {
                return Err(format!("path {p:?}: {} is a file, not a directory", parts[..=i].join("/")));
            }
            if i + 1 == parts.len() && m.is_dir() {
                return Err(format!("path {p:?}: is a directory"));
            }
        }
    }
    let mut written = Vec::new();
    for (p, content) in files {
        let at = root.join(p);
        if let Some(d) = at.parent() {
            fs::create_dir_all(d).map_err(|e| format!("path {p:?}: {e}"))?;
        }
        fs::write(&at, content).map_err(|e| format!("path {p:?}: {e}"))?;
        written.push(p.clone());
    }
    Ok(written)
}

#[derive(Debug, Default, PartialEq)]
pub struct Reply {
    pub files: Vec<(String, String)>,
    pub note: String,
    pub findings: Vec<Value>,
    pub remaining: Vec<String>,
}

const FINDING_TYPES: [&str; 3] = ["ambiguity", "contradiction", "missing_decision"];
const PATH_PATTERN: &str = "^[^/.\\\\][^\\\\]*$";

/// The reply schema: `files` (path and full content each), `note`, `findings`, `remaining`.
/// The envelope is schema-checked; a file's `content` is free text (docs/driver.md).
pub fn schema() -> Value {
    let path = json!({"type": "string", "minLength": 1, "pattern": PATH_PATTERN});
    let mut files = json!({"type": "array", "items": reply_schema(vec![("path", path.clone()), ("content", json!({"type": "string"}))])});
    files["minItems"] = json!(1);
    let mut s = reply_schema(vec![
        ("files", files),
        ("note", json!({"type": "string"})),
        ("findings", json!({"type": "array", "items": reply_schema(vec![
            ("type", json!({"type": "string", "enum": FINDING_TYPES})),
            ("cites", json!({"type": "array", "minItems": 1, "items": {"type": "string"}})),
            ("says", json!({"type": "string", "minLength": 1})),
            // 2 to 4: checked by program (check_reply); the schema keeps to the keywords the
            // review and decider schemas already use with claude -p --json-schema.
            ("options", json!({"type": "array", "minItems": 1, "items": {"type": "string"}})),
            ("default", json!({"type": "string"})),
            ("scope_changing", json!({"type": "boolean"})),
        ])})),
        ("remaining", json!({"type": "array", "items": path})),
    ]);
    s["description"] = json!("change.implement's reply: the complete text of every file written or replaced");
    s
}

/// The keys of an object against the expected set: missing and unknown keys, named.
fn keys(v: &Map<String, Value>, want: &[&str], at: &str, why: &mut Vec<String>) {
    for k in want {
        if !v.contains_key(*k) {
            why.push(format!("{at}`{k}` is missing"));
        }
    }
    for k in v.keys() {
        if !want.contains(&k.as_str()) {
            why.push(format!("{at}unknown key `{k}`"));
        }
    }
}

fn string_list(v: &Value) -> Option<Vec<String>> {
    v.as_array()?.iter().map(|x| x.as_str().map(String::from)).collect()
}

/// The reply checked by program against `schema` and the path rule. `asked`: on a continuation,
/// the paths asked for; each must be in `files` or again in `remaining`.
pub fn check_reply(v: &Value, asked: Option<&[String]>) -> Result<Reply, Vec<String>> {
    let Some(o) = v.as_object() else { return Err(vec!["the reply is not a JSON object".into()]) };
    let mut why = Vec::new();
    keys(o, &["files", "note", "findings", "remaining"], "", &mut why);
    let mut r = Reply::default();
    match o.get("files").map(|f| f.as_array()) {
        Some(Some(fs_)) if fs_.is_empty() => why.push("`files` is empty: write at least one file".into()),
        Some(Some(fs_)) => {
            let mut seen = BTreeSet::new();
            for (i, f) in fs_.iter().enumerate() {
                let at = format!("files[{i}]: ");
                let Some(fo) = f.as_object() else { why.push(format!("{at}not an object")); continue };
                keys(fo, &["path", "content"], &at, &mut why);
                let content = fo.get("content").and_then(|c| c.as_str());
                if content.is_none() {
                    why.push(format!("{at}`content` is not a string"));
                }
                match fo.get("path").and_then(|p| p.as_str()) {
                    None => why.push(format!("{at}`path` is not a string")),
                    Some(p) => match confine(p) {
                        Err(e) => why.push(format!("{at}{e}")),
                        Ok(p) if !seen.insert(p.clone()) => why.push(format!("{at}{p:?} twice")),
                        Ok(p) => r.files.push((p, content.unwrap_or_default().to_string())),
                    },
                }
            }
        }
        Some(None) => why.push("`files` is not a list".into()),
        None => {}
    }
    match o.get("note") {
        Some(Value::String(s)) => r.note = s.clone(),
        Some(_) => why.push("`note` is not a string".into()),
        None => {}
    }
    match o.get("findings").map(|f| f.as_array()) {
        Some(Some(list)) => {
            for (i, f) in list.iter().enumerate() {
                let at = format!("findings[{i}]: ");
                let Some(fo) = f.as_object() else { why.push(format!("{at}not an object")); continue };
                keys(fo, &["type", "cites", "says", "options", "default", "scope_changing"], &at, &mut why);
                if !fo.get("type").and_then(|t| t.as_str()).is_some_and(|t| FINDING_TYPES.contains(&t)) {
                    why.push(format!("{at}`type` is not one of {}", FINDING_TYPES.join(", ")));
                }
                if !fo.get("cites").and_then(string_list).is_some_and(|c| !c.is_empty()) {
                    why.push(format!("{at}`cites` is not a non-empty list of ids"));
                }
                if !fo.get("says").and_then(|s| s.as_str()).is_some_and(|s| !s.trim().is_empty()) {
                    why.push(format!("{at}`says` is empty"));
                }
                let opts = fo.get("options").and_then(string_list).filter(|o| (2..=4).contains(&o.len()));
                if opts.is_none() {
                    why.push(format!("{at}`options` is not a list of 2 to 4 answers"));
                }
                if !fo.get("default").and_then(|d| d.as_str()).is_some_and(|d| opts.as_ref().is_some_and(|o| o.iter().any(|x| x == d))) {
                    why.push(format!("{at}`default` is not one of its options"));
                }
                if !fo.get("scope_changing").is_some_and(|s| s.is_boolean()) {
                    why.push(format!("{at}`scope_changing` is not true or false"));
                }
                r.findings.push(f.clone());
            }
        }
        Some(None) => why.push("`findings` is not a list".into()),
        None => {}
    }
    match o.get("remaining").map(string_list) {
        Some(Some(list)) => {
            for p in list {
                match confine(&p) {
                    Err(e) => why.push(format!("remaining: {e}")),
                    Ok(p) if r.files.iter().any(|(f, _)| *f == p) => why.push(format!("remaining: {p} is both written and remaining")),
                    Ok(p) => r.remaining.push(p),
                }
            }
        }
        Some(None) => why.push("`remaining` is not a list of paths".into()),
        None => {}
    }
    for p in asked.unwrap_or(&[]) {
        if !r.files.iter().any(|(f, _)| f == p) && !r.remaining.contains(p) {
            why.push(format!("{p} was asked for: write it, or list it in `remaining` again"));
        }
    }
    if why.is_empty() { Ok(r) } else { Err(why) }
}

/// What `assemble` needs.
pub struct Inputs<'a> {
    pub case_dir: &'a Path,
    /// The tree whose current text is shown (the call's `out/impl/`).
    pub tree: &'a Path,
    pub story: &'a str,
    pub feedback: Option<String>,
    pub rejection: Option<Value>,
    pub rework_limit: Option<u64>,
    pub cap: usize,
    /// A continuation: the paths written so far and the paths asked for now.
    pub continuation: Option<(Vec<String>, Vec<String>)>,
}

pub struct Assembly {
    pub prompt: String,
    pub manifest: Value,
}

/// A Markdown fence longer than any run of backticks in `text`.
fn fence(text: &str) -> String {
    let (mut longest, mut cur) = (0, 0);
    for ch in text.chars() {
        if ch == '`' {
            cur += 1;
            longest = longest.max(cur);
        } else {
            cur = 0;
        }
    }
    "`".repeat(longest.max(2) + 1)
}

/// An IR entry without its empty `naming` objects (they say nothing and cost tokens), and a type
/// reference `{kind: primitive|declared, name}` written as its name.
fn strip_naming(v: &Value) -> Value {
    match v {
        Value::Object(m) if m.len() == 2 && matches!(m.get("kind").and_then(|k| k.as_str()), Some("primitive" | "declared"))
            && m.get("name").is_some_and(|n| n.is_string()) => m["name"].clone(),
        Value::Object(m) => Value::Object(m.iter()
            .filter(|(k, x)| !(k.as_str() == "naming" && x.as_object().is_some_and(|o| o.is_empty())))
            .map(|(k, x)| (k.clone(), strip_naming(x))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(strip_naming).collect()),
        _ => v.clone(),
    }
}

fn strings_in(v: &Value, out: &mut BTreeSet<String>) {
    match v {
        Value::String(s) => {
            out.insert(s.clone());
        }
        Value::Array(a) => a.iter().for_each(|x| strings_in(x, out)),
        Value::Object(m) => m.values().for_each(|x| strings_in(x, out)),
        _ => {}
    }
}

/// The specification a story must serve: its covered commands (with their outcomes), the views
/// its requirements trace to and the views of its commands' domains, and the entities, events,
/// errors and types those name, followed name by name. Also the covered names the IR lacks.
fn spec_facts(ir: &Value, traces: &Value, cov: &[String], mine: &[Value]) -> (Value, Vec<String>) {
    let mut out = Map::new();
    let commands: Map<String, Value> = cov.iter().filter_map(|c| ir["commands"].get(c).map(|x| (c.clone(), strip_naming(x)))).collect();
    let missing: Vec<String> = cov.iter().filter(|c| ir["commands"].get(c.as_str()).is_none()).cloned().collect();
    let doms: BTreeSet<&str> = cov.iter().map(|c| super::domain_of(c)).collect();
    let traced: BTreeSet<String> = mine.iter()
        .flat_map(|r| traces[r["id"].as_str().unwrap_or("")].as_array().cloned().unwrap_or_default())
        .filter_map(|x| x.as_str().map(String::from)).collect();
    let views: Map<String, Value> = ir["views"].as_object().into_iter().flatten()
        .filter(|(k, _)| traced.contains(k.as_str()) || doms.contains(super::domain_of(k)))
        .map(|(k, v)| (k.clone(), strip_naming(v))).collect();
    let mut names = BTreeSet::new();
    strings_in(&Value::Object(commands.clone()), &mut names);
    strings_in(&Value::Object(views.clone()), &mut names);
    out.insert("commands".into(), Value::Object(commands));
    out.insert("views".into(), Value::Object(views));
    let kinds = ["entities", "events", "errors", "types"];
    let mut picked: Vec<Map<String, Value>> = vec![Map::new(); kinds.len()];
    loop {
        let before: usize = picked.iter().map(|m| m.len()).sum();
        for (i, k) in kinds.iter().enumerate() {
            for (name, v) in ir[*k].as_object().into_iter().flatten() {
                if names.contains(name) && !picked[i].contains_key(name) {
                    let v = strip_naming(v);
                    strings_in(&v, &mut names);
                    picked[i].insert(name.clone(), v);
                }
            }
        }
        if picked.iter().map(|m| m.len()).sum::<usize>() == before {
            break;
        }
    }
    for (k, m) in kinds.iter().zip(picked) {
        if !m.is_empty() {
            out.insert((*k).into(), Value::Object(m));
        }
    }
    (Value::Object(out), missing)
}

/// The build rules without a build-constraints file (the session task's own wording).
const DEFAULT_BUILD: &str = "Rust, edition 2021. Standard library only: no dependencies in Cargo.toml (there is no network). \
impl/Cargo.toml and code under impl/src/. A service is a daemon binary that serves its web UI and a JSON API with \
std::net::TcpListener, takes its port and data file as arguments, and can run as several instances side by side; a CLI \
parses std::env::args.";

const TASK_RULES: &str = "- Keep the domain logic in a library (src/lib.rs) so tests call it directly.
- Tests: #[test] functions in src (#[cfg(test)] modules) and/or tests/*.rs, covering every command the story covers,
  including its refusals; they must pass without network access or a running server. At least one test per command;
  each command's name after its last dot (CreateTask for todo.task.CreateTask) appears in its test function's name
  (snake_case is fine, for example create_task) or in a comment directly above it. No #[ignore] and no #[should_panic]
  used to hide a failure.
- Taskfile.yml: version: '3', with a task `check` that runs `cargo test --locked` (commit a Cargo.lock: with no
  dependencies it holds only the package itself).
- README.md: how to run it and how it maps to the specification's commands.";

const FINDINGS_RULE: &str = "If the inputs above leave you an ambiguity (two or more readings), a contradiction (two items
that cannot both hold, for example the context against a requirement) or a missing decision (something you must decide
that no input decides), do not decide it silently: put it in `findings`, each with type (ambiguity, contradiction or
missing_decision), cites (the ids involved: r1, q2, d3, or context.<field>; a contradiction cites both sides), says (one
sentence), options (2 to 4 short answers), default (the option you would pick, the simplest that fits the idea) and
scope_changing (true when an option adds or removes a user-visible capability, a kind of user or a component). Then
still do the task, following each default. A finding listed above with status resolved is a decision already taken:
follow its `decision`, never raise it again; one with status open waits for its decision: follow its `default`, never
raise it again. `findings` is [] when there is nothing to report.";

/// The node's whole input as one prompt, and a manifest of what went in (sections and bytes,
/// the files inlined and listed with the rank that put them there).
pub fn assemble(i: &Inputs) -> Result<Assembly, String> {
    let work = i.case_dir.join("work");
    let rel = format!("aep/story/{}.md", i.story);
    let story_text = fs::read_to_string(work.join(&rel)).map_err(|e| format!("{rel}: {e}"))?;
    let (sl, summary) = story_summary(&story_text).ok_or(format!("{rel} has no summary"))?;
    let cov = covers(&summary);
    let mut acceptance = vec![(format!("{rel}:{sl}"), summary.clone())];
    let mut in_acc = false;
    for (n, l) in story_text.lines().enumerate() {
        if l.starts_with("## ") {
            in_acc = l.trim() == "## Acceptance";
            continue;
        }
        if in_acc && !l.trim().is_empty() {
            acceptance.push((format!("{rel}:{}", n + 1), l.trim().to_string()));
        }
    }
    let reqs = read_data(&work.join("requirements.yaml")).map(|v| v["requirements"].as_array().cloned().unwrap_or_default()).unwrap_or_default();
    let traces = read_json(&work.join("spec/traces.json"))["traces"].clone();
    let ir = read_json(&work.join(".sdc/gate/ir.json"));
    let (mine, _) = review_scope(&reqs, &traces, &ir["commands"], &cov, std::slice::from_ref(&cov));
    let (spec, missing) = spec_facts(&ir, &traces, &cov, &mine);
    let sel = select_tree(i.tree, &work.join("mainline"), &story_text, i.cap);

    let mut sections: Vec<(&str, String)> = vec![("head", "You are one step (`change.implement`) of a governed software delivery cycle, asked in one request.
Everything you need is in this message: you have no tools, nothing to read and no command to run. Reply with the JSON
object of the response schema only. Programs write your files into the product tree and check them afterwards; your
words decide nothing.\n".to_string())];
    let f = fence(&story_text);
    sections.push(("story", format!("## The story `{}` (impl/ serves it)\n\n{f}markdown\n{}\n{f}\n", i.story, story_text.trim_end())));
    let mut r = String::from("## The requirements this story delivers\n\n");
    if mine.is_empty() {
        r.push_str("None traced to this story's commands: build what the story and its commands say.\n");
    }
    for q in &mine {
        r.push_str(&format!("- {}: {}\n", q["id"].as_str().unwrap_or("?"), q["text"].as_str().unwrap_or("")));
    }
    r.push_str("\nAcceptance lines:\n\n");
    for (c, t) in &acceptance {
        r.push_str(&format!("- {t} ({c})\n"));
    }
    sections.push(("requirements", r));
    let mut s = String::from("## The specification it serves (compiled; only this story's commands, the views it serves, and what they name)\n\n");
    if !missing.is_empty() {
        s.push_str(&format!("Not in the compiled specification: {}.\n\n", missing.join(", ")));
    }
    // One compact JSON line per declaration (a type reference is written as its type's name).
    for (kind, decls) in spec.as_object().into_iter().flatten() {
        s.push_str(&format!("{kind}:\n"));
        for (name, d) in decls.as_object().into_iter().flatten() {
            s.push_str(&format!("- {name}: {d}\n"));
        }
        s.push('\n');
    }
    sections.push(("spec", s));
    let bc = read_data(&work.join("build-constraints.yaml"));
    let constraints = match &bc {
        Some(v) => {
            let mut t = v["text"].as_str().unwrap_or("").trim().to_string();
            for k in ["language", "edition", "dependencies"] {
                if !v[k].is_null() {
                    t.push_str(&format!("\n{k}: {}", if v[k].is_string() { v[k].as_str().unwrap_or("").to_string() } else { v[k].to_string() }));
                }
            }
            t
        }
        None => DEFAULT_BUILD.to_string(),
    };
    sections.push(("constraints", format!("## Build constraints{}\n\n{constraints}\n",
        if bc.is_some() { " (the run's build-constraints.yaml)" } else { "" })));
    let mut t = String::from("## The product tree impl/ (current text)\n\n");
    if sel.inlined.is_empty() && sel.listed.is_empty() {
        t.push_str("impl/ is empty: this is the first story; create the crate.\n");
    } else {
        t.push_str(&format!("{} file(s), {} bytes shown{}. A file you do not write stays exactly as it is.\n\n",
            sel.inlined.len(), sel.inlined_bytes, if sel.listed.is_empty() { String::new() } else { format!("; {} listed without text", sel.listed.len()) }));
        for f in &sel.inlined {
            let text = fs::read_to_string(i.tree.join(&f.path)).unwrap_or_default();
            let fc = fence(&text);
            t.push_str(&format!("### impl/{} ({} bytes)\n\n{fc}\n{}{}{fc}\n\n", f.path, f.bytes, text, if text.ends_with('\n') || text.is_empty() { "" } else { "\n" }));
        }
        if !sel.listed.is_empty() {
            t.push_str("### Listed without text (they exist; keep them working; replace one only with its complete new text)\n\n");
            for f in &sel.listed {
                t.push_str(&format!("- impl/{} ({} bytes, {})\n", f.path, f.bytes, f.why));
            }
            t.push('\n');
        }
    }
    sections.push(("tree", t));
    if let Ok(ft) = fs::read_to_string(work.join("findings.yaml")) {
        let fc = fence(&ft);
        sections.push(("findings", format!("## Findings of this run so far\n\n{fc}yaml\n{}\n{fc}\n", ft.trim_end())));
    }
    if let Some(fb) = &i.feedback {
        sections.push(("feedback", format!("## Why the previous attempt was sent back: fix exactly that\n\n{}\n", fb.trim_end())));
    }
    if let Some(rj) = &i.rejection {
        let mut x = format!("## Rework\n\nYour previous output was rejected at `{}` (revision {}; rejection {} of at most {}) by {}. Address every \
reason; change only what a reason asks for. In `note`, write one line per reason: its number, then where you addressed it as \
<file>:<line> (or <file>:<first>-<last>), then what you changed.\n\n",
            rj["approval"].as_str().unwrap_or("?"), rj["revision"].as_str().unwrap_or("?"), rj["count"],
            i.rework_limit.map_or("?".to_string(), |n| n.to_string()), rj["principal"].as_str().unwrap_or("its approver"));
        for (n, reason) in rj["reasons"].as_array().into_iter().flatten().enumerate() {
            x.push_str(&format!("{}. {}\n", n + 1, reason.as_str().unwrap_or("")));
        }
        sections.push(("rejection", x));
    }
    sections.push(("task", format!("## Task

Implement story `{}` of the specification above in the product tree impl/ (shown above with its current text; extend it,
keep what earlier stories built).
- The build constraints above.
{TASK_RULES}
You cannot run commands and cannot read anything else: write code that compiles and passes its own tests against exactly
the files shown.

## Reply

The JSON object of the response schema, nothing else:
- files: every file you create or change, each {{\"path\", \"content\"}}: path relative to impl/ (for example src/lib.rs),
  content the file's complete new text. Never a path with `..`, an absolute path, a hidden entry or target/: a program
  refuses the whole reply.
- note: one or two sentences on what you built (on a rework: the lines asked for above).
- findings: {FINDINGS_RULE}
- remaining: [] as a rule. Only when the files would not fit in one reply: write the files you can, each complete, and
  list here the paths still to write; you are then asked for exactly those (at most {MAX_CONTINUATIONS} more times), with
  the files you wrote shown.\n", i.story)));
    if let Some((written, asked)) = &i.continuation {
        sections.push(("continuation", format!("## Continuation

Your previous reply wrote: {} (shown above with their new text). Write exactly these files now, each complete: {}.
List in `remaining` any of them that still does not fit.\n", written.join(", "), asked.join(", "))));
    }
    let prompt = sections.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>().join("\n");
    let tf = |v: &Vec<TreeFile>| v.iter().map(|f| json!({"path": f.path, "bytes": f.bytes, "why": f.why})).collect::<Vec<_>>();
    let manifest = json!({"format": "sdc.implement-inputs/1", "story": i.story,
        "requirements": mine.iter().map(|r| r["id"].clone()).collect::<Vec<_>>(),
        "commands": spec["commands"].as_object().map(|m| m.keys().cloned().collect::<Vec<_>>()).unwrap_or_default(),
        "views": spec["views"].as_object().map(|m| m.keys().cloned().collect::<Vec<_>>()).unwrap_or_default(),
        "missing_commands": missing, "constraints": if bc.is_some() { "build-constraints.yaml" } else { "default" },
        "tree": {"rule": "named by the story, this story's change (all of it for a first story), crate root, merged by an earlier story; inlined in that order up to the cap, the rest listed",
            "cap": i.cap, "first_story": sel.first_story, "inlined_bytes": sel.inlined_bytes, "inlined": tf(&sel.inlined), "listed": tf(&sel.listed)},
        "sections": sections.iter().map(|(k, s)| ((*k).to_string(), json!(s.len()))).collect::<Map<String, Value>>(),
        "prompt_bytes": prompt.len()});
    Ok(Assembly { prompt, manifest })
}

/// A reply of every product file of a tree (fixture adapter, tests).
pub fn pack(tree: &Path, note: &str) -> Value {
    let files: Vec<Value> = product_files(tree).into_iter()
        .filter_map(|p| fs::read_to_string(tree.join(&p)).ok().map(|c| json!({"path": p, "content": c}))).collect();
    json!({"files": files, "note": note, "findings": [], "remaining": []})
}

/// The arguments of `sdc-reply implement-run`.
pub struct RunArgs {
    pub rec: PathBuf,
    pub cwd: PathBuf,
    pub case_dir: PathBuf,
    pub story: String,
    pub adapter: Option<PathBuf>,
    pub rework_limit: Option<u64>,
    pub assemble_only: bool,
    pub cap: usize,
}

/// Numbers summed key by key (a usage object, nested); anything else keeps the last value.
fn add_numbers(acc: &mut Value, v: &Value) {
    match (acc.as_object_mut(), v) {
        (Some(a), Value::Object(m)) => {
            for (k, x) in m {
                let cur = a.entry(k.clone()).or_insert(Value::Null);
                match (cur.as_i64(), x.as_i64(), cur.as_f64(), x.as_f64()) {
                    (Some(p), Some(q), _, _) => *cur = json!(p + q),
                    (_, _, Some(p), Some(q)) => *cur = json!(p + q),
                    _ if x.is_object() && (cur.is_object() || cur.is_null()) => {
                        if cur.is_null() {
                            *cur = json!({});
                        }
                        add_numbers(cur, x);
                    }
                    _ => *cur = x.clone(),
                }
            }
        }
        _ => *acc = v.clone(),
    }
}

/// The call's response.json: the requests' responses summed (cost, duration, turns, usage), the
/// last one's adapter, model and reasons, and the request counts.
fn aggregate(responses: &[Value], exit: u8, requests: usize, continuations: usize, retries: usize, cause: Option<&str>) -> Value {
    let last = responses.last().cloned().unwrap_or(json!({}));
    let sum = |k: &str| {
        let xs: Vec<f64> = responses.iter().filter_map(|r| r[k].as_f64()).collect();
        if xs.is_empty() { Value::Null } else { json!(xs.iter().sum::<f64>()) }
    };
    let mut usage = Value::Null;
    for r in responses.iter().filter(|r| r["usage"].is_object()) {
        if usage.is_null() {
            usage = json!({});
        }
        add_numbers(&mut usage, &r["usage"]);
    }
    let tool_calls: Vec<Value> = responses.iter().flat_map(|r| r["tool_calls"].as_array().cloned().unwrap_or_default()).collect();
    json!({"adapter": last["adapter"], "model": last["model"], "effort": last["effort"], "exit": exit,
        "duration_ms": sum("duration_ms"), "wall_ms": sum("wall_ms"), "cost_usd": sum("cost_usd"), "num_turns": sum("num_turns"),
        "usage": usage, "tool_calls": tool_calls, "offered_tools": last["offered_tools"], "observed_models": last["observed_models"],
        "is_error": last["is_error"], "subtype": last["subtype"], "terminal_reason": last["terminal_reason"],
        "api_error_status": last["api_error_status"], "schema_constrained": last["schema_constrained"],
        "implement_mode": "request", "requests": requests, "continuations": continuations, "reply_retries": retries, "cause": cause})
}

/// One implement call's requests. Returns (exit, the line for the step's log): 0 the files are
/// written into `<cwd>/out/impl/`; 2 the reply stayed invalid after `MAX_RETRIES` retries
/// (`implement_reply_invalid`) or files stayed `remaining` after `MAX_CONTINUATIONS`
/// continuations (`implement_reply_incomplete`), or the input could not be assembled
/// (`implement_inputs_missing`); 75 the model's API ended a request or a re-driven recording
/// diverged; 126/127 the adapter could not run; 64 a usage error.
pub fn run(a: &RunArgs) -> (u8, String) {
    let tree = a.cwd.join("out/impl");
    let feedback = fs::read_to_string(a.cwd.join("in/feedback.md")).ok().filter(|s| !s.trim().is_empty());
    let rejection: Option<Value> = fs::read(a.cwd.join("in/rejection.json")).ok().and_then(|b| serde_json::from_slice(&b).ok());
    let inputs = |continuation| Inputs { case_dir: &a.case_dir, tree: &tree, story: &a.story, feedback: feedback.clone(),
        rejection: rejection.clone(), rework_limit: a.rework_limit, cap: a.cap, continuation };
    let first = match assemble(&inputs(None)) {
        Ok(x) => x,
        Err(e) => return (2, format!("implement_inputs_missing: {e}")),
    };
    let w = |name: &str, text: &str| fs::write(a.rec.join(name), text).map_err(|e| format!("{name}: {e}"));
    if let Err(e) = fs::create_dir_all(&a.rec).map_err(|e| e.to_string())
        .and_then(|_| w("prompt.md", &first.prompt))
        .and_then(|_| w("reply-schema.json", &schema().to_string()))
        .and_then(|_| w("implement-inputs.json", &serde_json::to_string_pretty(&first.manifest).unwrap_or_default())) {
        return (127, format!("the call record could not be written: {e}"));
    }
    let summary = format!("{} bytes of prompt: {} file(s) of impl/ inlined ({} bytes), {} listed", first.prompt.len(),
        first.manifest["tree"]["inlined"].as_array().map_or(0, |v| v.len()), first.manifest["tree"]["inlined_bytes"],
        first.manifest["tree"]["listed"].as_array().map_or(0, |v| v.len()));
    if a.assemble_only {
        return (0, format!("assembled: {summary}"));
    }
    let Some(adapter) = &a.adapter else { return (64, "implement-run: no --adapter".into()) };
    let base = read_json(&a.rec.join("request.json"));
    if !base.is_object() {
        return (127, format!("no request.json in {}", a.rec.display()));
    }
    let stderr = || fs::OpenOptions::new().create(true).append(true).open(a.rec.join("adapter-stderr.txt"));
    let (mut slot, mut prompt) = (first.prompt.clone(), first.prompt);
    let mut asked: Option<Vec<String>> = None;
    let (mut written, mut findings, mut notes): (Vec<String>, Vec<Value>, Vec<String>) = (vec![], vec![], vec![]);
    let (mut retries, mut conts, mut k, mut kind) = (0usize, 0usize, 0usize, "first");
    let mut responses = Vec::new();
    let mut log = String::new();
    let (code, line, cause): (u8, String, Option<&str>) = loop {
        k += 1;
        let dir = a.rec.join(format!("req-{k}"));
        let mut req = base.clone();
        req["record_dir"] = json!(dir);
        req["prompt_file"] = json!(dir.join("prompt.md"));
        req["json_schema"] = json!(a.rec.join("reply-schema.json"));
        req["reply_attempt"] = json!(k);
        req["request_kind"] = json!(kind);
        if let Err(e) = fs::create_dir_all(&dir).and_then(|_| fs::write(dir.join("prompt.md"), &prompt))
            .and_then(|_| fs::write(dir.join("request.json"), serde_json::to_string_pretty(&req).unwrap_or_default())) {
            break (127, format!("request {k} could not be written: {e}"), None);
        }
        let err = match stderr() { Ok(f) => Stdio::from(f), Err(_) => Stdio::null() };
        let out = match Command::new(adapter).arg(dir.join("request.json")).stdin(Stdio::null()).stderr(err).output() {
            Ok(o) => o,
            Err(e) => break (127, format!("the adapter {} did not start: {e}", adapter.display()), None),
        };
        let exit = out.status.code().unwrap_or(1);
        let resp: Value = serde_json::from_slice(&out.stdout).unwrap_or(json!({}));
        let _ = fs::write(dir.join("response.json"), resp.to_string());
        responses.push(resp.clone());
        let term = resp["terminal_reason"].as_str().unwrap_or("");
        if term == "api_error" || term == "recording_diverged" {
            break (75, format!("request {k}: {term}{}", resp["api_error_status"].as_i64().map_or(String::new(), |s| format!(" {s}"))), None);
        }
        if exit == 126 || exit == 127 {
            break (exit as u8, format!("request {k}: the adapter could not run (exit {exit})"), None);
        }
        let checked = if exit != 0 {
            Err(vec![format!("the request failed (adapter exit {exit}{}): no reply was taken; if the files did not fit in one reply, \
                write fewer, each complete, and list the rest in `remaining`", resp["subtype"].as_str().map_or(String::new(), |s| format!(", {s}")))])
        } else {
            match fs::read(dir.join("structured-output.json")).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()) {
                None => Err(vec!["no structured reply: reply with the JSON object of the response schema; if the files did not fit in \
                    one reply, write fewer, each complete, and list the rest in `remaining`".to_string()]),
                Some(v) => check_reply(&v, asked.as_deref()).and_then(|r| write_files(&tree, &r.files).map(|w| (r, w)).map_err(|e| vec![e])),
            }
        };
        match checked {
            Err(why) => {
                let why = why.join("; ");
                log.push_str(&format!("{}\n", json!({"request": k, "kind": kind, "valid": false, "error": why})));
                if retries >= MAX_RETRIES {
                    break (2, format!("implement_reply_invalid after {k} request(s), {retries} retries: {why}"), Some("implement_reply_invalid"));
                }
                retries += 1;
                kind = "retry";
                prompt = format!("{slot}\n## Your previous reply was refused\n\nThe program check refused it: {why}\nReply again with the whole JSON object, fixing exactly that.\n");
            }
            Ok((r, w)) => {
                log.push_str(&format!("{}\n", json!({"request": k, "kind": kind, "valid": true, "files": w, "remaining": r.remaining})));
                written.extend(w);
                findings.extend(r.findings);
                notes.push(r.note);
                if r.remaining.is_empty() {
                    break (0, format!("change.implement wrote {} file(s) in {k} request(s) ({retries} retries, {conts} continuations; {summary}): {}",
                        written.len(), notes.join(" / ").replace('\n', " ")), None);
                }
                if conts >= MAX_CONTINUATIONS {
                    break (2, format!("implement_reply_incomplete after {k} request(s): {} still remaining after {MAX_CONTINUATIONS} continuations",
                        r.remaining.join(", ")), Some("implement_reply_incomplete"));
                }
                conts += 1;
                kind = "continuation";
                asked = Some(r.remaining.clone());
                match assemble(&inputs(Some((written.clone(), r.remaining)))) {
                    Ok(x) => {
                        slot = x.prompt;
                        let _ = fs::write(a.rec.join(format!("implement-inputs.continuation-{conts}.json")),
                            serde_json::to_string_pretty(&x.manifest).unwrap_or_default());
                    }
                    Err(e) => break (2, format!("implement_inputs_missing: {e}"), Some("implement_inputs_missing")),
                }
                prompt = slot.clone();
            }
        }
    };
    if code == 0 && !findings.is_empty() {
        let y = serde_yaml::to_string(&json!({"findings": findings})).unwrap_or_default();
        if let Err(e) = fs::write(a.cwd.join("out/findings.yaml"), y) {
            return (127, format!("out/findings.yaml: {e}"));
        }
    }
    let _ = fs::write(a.rec.join("requests.jsonl"), &log);
    let mut events = Vec::new();
    for n in 1..=k {
        if let Ok(b) = fs::read(a.rec.join(format!("req-{n}/events.jsonl"))) {
            events.extend(b);
            if !events.ends_with(b"\n") && !events.is_empty() {
                events.push(b'\n');
            }
        }
    }
    if !events.is_empty() {
        let _ = fs::File::create(a.rec.join("events.jsonl")).and_then(|mut f| f.write_all(&events));
    }
    let agg = aggregate(&responses, code, k, conts, retries, cause);
    let _ = fs::write(a.rec.join("response.json"), agg.to_string());
    (code, line)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
