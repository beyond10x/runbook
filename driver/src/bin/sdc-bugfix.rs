//! sdc-bugfix: the programs of `sdc.bugfix` (protocol/sdc-bugfix.protocol.yaml, docs/bugfix.md).
//! Run by sdc-drive with cwd = the case's work directory; every path is relative to it.
//!
//!   sdc-bugfix schema dissection|repro   the JSON schema of an LLM data document
//!   sdc-bugfix dissection-check          bug-dissection.json against its schema and impl/: exit 0 valid, 1 invalid
//!   sdc-bugfix repro-install             impl/ := mainline/ plus the reproducing test of repro/
//!   sdc-bugfix repro-run                 exit 0 red, 1 not red (the reason on stdout)
//!   sdc-bugfix tests-run                 exit 0 passed, 1 failed
//!   sdc-bugfix gate-run                  build, test, clippy -D warnings, fmt --check: exit 0 passed, 1 failed
//!   sdc-bugfix story-update              the bug's story (bug.yaml `story`) to implemented through aep
//!   sdc-bugfix seed --from <dir> [--text <report>] [--story <id>]
//!                                        a new case's work/: mainline/ and impl/ from the crate, bug.yaml,
//!                                        the story created and moved to active (what a parent hands in)
//!
//! Artifacts (driver/examples/sdc/sdc-bugfix.bindings.json): bug.yaml (report), bug-dissection.json
//! (dissection), repro/ (repro: repro.json `{file, test}` and the test file), impl/ (change), aep/
//! (story). mainline/ is the unchanged code the red is observed against. Every cargo run is made in a
//! copy of impl/ under .sdc/bugfix/ (nothing is written into an artifact) through `slot` when it is
//! on PATH. A program that is missing exits 127 (sdc-drive records nothing), never a check result.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use regex::Regex;
use serde_json::{json, Value};

const DISSECTION: &str = "bug-dissection.json";
const FIELDS: [(&str, &str); 4] = [
    ("observed", "what happens, as the report states it: inputs and the wrong result"),
    ("expected", "what should happen instead, for the same inputs"),
    ("where", "the file of the code tree the bug is in, relative to the tree, optionally with :<line> (src/lib.rs:9)"),
    ("reproduce", "how to reproduce it: the call or command and its inputs"),
];
const FILE_RE: &str = r"^tests/[a-z][a-z0-9_]*\.rs$";
const TEST_RE: &str = r"^[a-z_][a-z0-9_]*$";

type R<T> = Result<T, String>;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(String::as_str) {
        Some("schema") => schema(args.get(1).map(String::as_str).unwrap_or("")),
        Some("dissection-check") => dissection_check(),
        Some("repro-install") => repro_install(),
        Some("repro-run") => repro_run(),
        Some("tests-run") => tests_run(),
        Some("gate-run") => gate_run(),
        Some("story-update") => story_update(),
        Some("seed") => seed(&args[1..]),
        _ => Err("usage: sdc-bugfix schema|dissection-check|repro-install|repro-run|tests-run|gate-run|story-update|seed".into()),
    };
    match r {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("sdc-bugfix: {e}");
            ExitCode::from(if e.contains("not on PATH") { 127 } else { 2 })
        }
    }
}

// --- schemas -----------------------------------------------------------------------------------

fn dissection_schema() -> Value {
    let props: serde_json::Map<String, Value> = FIELDS
        .iter()
        .map(|(k, d)| (k.to_string(), json!({"type": "string", "minLength": 1, "maxLength": 600, "description": d})))
        .collect();
    json!({"type": "object", "additionalProperties": false,
        "required": FIELDS.iter().map(|(k, _)| k).collect::<Vec<_>>(), "properties": props})
}

fn repro_schema() -> Value {
    json!({"type": "object", "additionalProperties": false, "required": ["file", "test"], "properties": {
        "file": {"type": "string", "pattern": FILE_RE,
            "description": "the new integration test file, relative to the code tree (tests/<name>.rs)"},
        "test": {"type": "string", "pattern": TEST_RE,
            "description": "the name of the one #[test] function in that file that reproduces the bug"}}})
}

fn schema(which: &str) -> R<u8> {
    let s = match which {
        "dissection" => dissection_schema(),
        "repro" => repro_schema(),
        _ => return Err("schema dissection|repro".into()),
    };
    println!("{}", serde_json::to_string_pretty(&s).unwrap());
    Ok(0)
}

// --- the dissection ----------------------------------------------------------------------------

/// Every way the document misses its schema or the code tree, one line each.
fn dissection_errors(doc: &str, tree: &Path) -> Vec<String> {
    let v: Value = match serde_json::from_str(doc) {
        Ok(v) => v,
        Err(e) => return vec![format!("not JSON: {e}")],
    };
    let Some(o) = v.as_object() else { return vec!["not a JSON object".into()] };
    let mut errs = Vec::new();
    for k in o.keys().filter(|k| !FIELDS.iter().any(|(f, _)| f == k)) {
        errs.push(format!("key `{k}` is not in the schema (keys: observed, expected, where, reproduce)"));
    }
    for (k, _) in FIELDS {
        match o.get(k).and_then(Value::as_str) {
            None => errs.push(format!("`{k}` is missing or not a string")),
            Some(s) if s.trim().is_empty() => errs.push(format!("`{k}` is empty")),
            Some(s) if s.chars().count() > 600 => errs.push(format!("`{k}` is longer than 600 characters")),
            _ => {}
        }
    }
    if let (Some(a), Some(b)) = (o.get("observed").and_then(Value::as_str), o.get("expected").and_then(Value::as_str)) {
        if a.trim() == b.trim() && !a.trim().is_empty() {
            errs.push("`observed` and `expected` are the same".into());
        }
    }
    if let Some(w) = o.get("where").and_then(Value::as_str).map(str::trim).filter(|w| !w.is_empty()) {
        let (path, line) = match w.rsplit_once(':') {
            Some((p, l)) if l.chars().all(|c| c.is_ascii_digit()) && !l.is_empty() => (p, l.parse::<usize>().ok()),
            _ => (w, None),
        };
        let f = tree.join(path);
        if path.starts_with('/') || path.contains("..") || !f.is_file() {
            errs.push(format!("`where`: {path} is not a file of the code tree"));
        } else if let Some(n) = line {
            let lines = fs::read_to_string(&f).map(|t| t.lines().count()).unwrap_or(0);
            if n == 0 || n > lines {
                errs.push(format!("`where`: line {n} is not in {path} ({lines} lines)"));
            }
        }
    }
    errs
}

fn dissection_check() -> R<u8> {
    let Ok(doc) = fs::read_to_string(DISSECTION) else {
        println!("dissection: FAILED no {DISSECTION}");
        return Ok(1);
    };
    let errs = dissection_errors(&doc, Path::new("impl"));
    for e in &errs {
        println!("dissection: FAILED {e}");
    }
    if errs.is_empty() {
        println!("dissection: ok {DISSECTION} fits its schema and names a file of impl/");
    }
    Ok(if errs.is_empty() { 0 } else { 1 })
}

// --- the reproducing test ----------------------------------------------------------------------

struct Repro {
    file: String,
    test: String,
}

impl Repro {
    fn base(&self) -> &str {
        self.file.rsplit('/').next().unwrap()
    }
    /// The libtest name of the test in its binary, and the cargo target section it runs under.
    fn section(&self) -> String {
        self.file.clone()
    }
}

/// repro/repro.json against its schema, its test file present and holding the named #[test], and
/// the file new to the code tree.
fn read_repro() -> Result<Repro, String> {
    let t = fs::read_to_string("repro/repro.json").map_err(|_| "no repro/repro.json".to_string())?;
    let v: Value = serde_json::from_str(&t).map_err(|e| format!("repro/repro.json is not JSON: {e}"))?;
    let o = v.as_object().ok_or("repro/repro.json is not an object")?;
    if let Some(k) = o.keys().find(|k| *k != "file" && *k != "test") {
        return Err(format!("repro/repro.json: key `{k}` is not in the schema (file, test)"));
    }
    let file = o.get("file").and_then(Value::as_str).unwrap_or("").to_string();
    let test = o.get("test").and_then(Value::as_str).unwrap_or("").to_string();
    if !Regex::new(FILE_RE).unwrap().is_match(&file) {
        return Err(format!("repro/repro.json: file `{file}` is not tests/<name>.rs"));
    }
    if !Regex::new(TEST_RE).unwrap().is_match(&test) {
        return Err(format!("repro/repro.json: test `{test}` is not a function name"));
    }
    let r = Repro { file, test };
    let src = fs::read_to_string(Path::new("repro").join(r.base()))
        .map_err(|_| format!("repro/{} (the test file repro.json names) is missing", r.base()))?;
    let def = Regex::new(&format!(r"#\[test\]\s*(#\[[^\]]*\]\s*)*fn\s+{}\s*\(", regex::escape(&r.test))).unwrap();
    if !def.is_match(&src) {
        return Err(format!("repro/{} has no #[test] fn {}()", r.base(), r.test));
    }
    if Regex::new(r"#\[(ignore|should_panic)").unwrap().is_match(&src) {
        return Err(format!("repro/{} uses #[ignore] or #[should_panic]", r.base()));
    }
    if Path::new("mainline").join(&r.file).exists() {
        return Err(format!("{} exists in the unchanged code: the reproducing test is a new file", r.file));
    }
    Ok(r)
}

fn repro_install() -> R<u8> {
    reset_dir(Path::new("impl"))?;
    copy_tree(Path::new("mainline"), Path::new("impl"))?;
    match read_repro() {
        Ok(r) => {
            let to = Path::new("impl").join(&r.file);
            fs::create_dir_all(to.parent().unwrap()).map_err(|e| e.to_string())?;
            fs::copy(Path::new("repro").join(r.base()), &to).map_err(|e| e.to_string())?;
            println!("repro-install: impl/ is mainline/ plus {}", r.file);
        }
        // repro.run reports it, so the writer gets the reason back.
        Err(e) => println!("repro-install: impl/ is mainline/ ({e})"),
    }
    Ok(0)
}

/// impl/<file> is byte for byte repro/<file>.
fn test_unchanged(r: &Repro) -> Option<String> {
    let a = fs::read(Path::new("impl").join(&r.file)).ok();
    let b = fs::read(Path::new("repro").join(r.base())).ok();
    match (a, b) {
        (None, _) => Some(format!("impl/{} is missing: the reproducing test was removed", r.file)),
        (Some(a), Some(b)) if a == b => None,
        _ => Some(format!("impl/{} differs from repro/{}: the reproducing test was changed", r.file, r.base())),
    }
}

fn repro_run() -> R<u8> {
    let r = match read_repro() {
        Ok(r) => r,
        Err(e) => {
            println!("repro: not red: {e}");
            return Ok(1);
        }
    };
    if let Some(e) = test_unchanged(&r) {
        println!("repro: not red: {e}");
        return Ok(1);
    }
    let diff = tree_diff(Path::new("mainline"), Path::new("impl"), &r.file)?;
    if !diff.is_empty() {
        println!("repro: not red: the code is changed, not only the test added: {}", diff.join(", "));
        return Ok(1);
    }
    let run = suite()?;
    print!("{}", run.summary());
    if let Some(e) = &run.compile_error {
        println!("repro: not red: the code tree with the test does not compile: {e}");
        return Ok(1);
    }
    let own: Vec<&Entry> = run.entries.iter().filter(|e| e.is(&r)).collect();
    let others: Vec<&Entry> = run.entries.iter().filter(|e| !e.is(&r) && e.status != "ok").collect();
    let verdict = match own.as_slice() {
        [] => Some(format!("test {} in {} did not run", r.test, r.file)),
        [e] if e.status == "ok" => Some(format!("test {} passes on the unchanged code: it does not reproduce the bug", r.test)),
        [e] if e.status != "FAILED" => Some(format!("test {} is {}", r.test, e.status)),
        [_] => None,
        _ => Some(format!("test {} ran more than once", r.test)),
    };
    let verdict = verdict.or_else(|| {
        (!others.is_empty()).then(|| {
            format!("other tests do not pass on the unchanged code: {}",
                others.iter().map(|e| format!("{} {}", e.name, e.status)).collect::<Vec<_>>().join(", "))
        })
    });
    match verdict {
        Some(v) => {
            println!("repro: not red: {v}");
            Ok(1)
        }
        None => {
            println!("repro: red: test {} ({}) fails on the unchanged code, the other {} tests pass",
                r.test, r.file, run.entries.len() - 1);
            Ok(0)
        }
    }
}

fn tests_run() -> R<u8> {
    let r = match read_repro() {
        Ok(r) => r,
        Err(e) => {
            println!("tests: FAILED {e}");
            return Ok(1);
        }
    };
    if let Some(e) = test_unchanged(&r) {
        println!("tests: FAILED refused: {e}; the fix may not change the reproducing test");
        return Ok(1);
    }
    let run = suite()?;
    print!("{}", run.summary());
    if let Some(e) = &run.compile_error {
        println!("tests: FAILED the code tree does not compile: {e}");
        return Ok(1);
    }
    let own = run.entries.iter().filter(|e| e.is(&r)).collect::<Vec<_>>();
    let bad: Vec<String> = run.entries.iter().filter(|e| e.status != "ok").map(|e| format!("{} {}", e.name, e.status)).collect();
    if own.len() != 1 || own[0].status != "ok" {
        println!("tests: FAILED test {} ({}) does not pass: {}", r.test, r.file,
            own.first().map(|e| e.status.as_str()).unwrap_or("did not run"));
        return Ok(1);
    }
    if !bad.is_empty() || run.exit != 0 {
        println!("tests: FAILED not every test passes (cargo exit {}): {}", run.exit, bad.join(", "));
        return Ok(1);
    }
    println!("tests: passed: test {} ({}) ok, {} tests ok, 0 failed, 0 ignored", r.test, r.file, run.entries.len());
    Ok(0)
}

fn gate_run() -> R<u8> {
    let tree = copy_of_impl()?;
    let steps: [(&str, &[&str]); 4] = [
        ("build", &["build", "--offline"]),
        ("test", &["test", "--offline"]),
        ("clippy", &["clippy", "--offline", "--all-targets", "--", "-D", "warnings"]),
        ("fmt", &["fmt", "--check"]),
    ];
    let mut ok = true;
    for (name, args) in steps {
        let (code, out) = cargo(&tree, args)?;
        if code == 0 {
            println!("gate: ok     {name}: cargo {}", args.join(" "));
        } else {
            ok = false;
            println!("gate: FAILED {name}: cargo {} exit {code}", args.join(" "));
            for l in tail(&out, 40) {
                println!("  | {l}");
            }
        }
    }
    Ok(if ok { 0 } else { 1 })
}

// --- cargo test output ---------------------------------------------------------------------------

struct Entry {
    section: String,
    name: String,
    status: String,
}

impl Entry {
    fn is(&self, r: &Repro) -> bool {
        self.name == r.test && self.section.contains(&r.section())
    }
}

struct Run {
    exit: i32,
    entries: Vec<Entry>,
    compile_error: Option<String>,
}

impl Run {
    fn summary(&self) -> String {
        let mut s = String::new();
        for e in &self.entries {
            s.push_str(&format!("  | {} :: {} ... {}\n", e.section, e.name, e.status));
        }
        s
    }
}

/// Parse `cargo test` output with stdout and stderr in one stream: `Running <target>` and
/// `Doc-tests <crate>` open a section, `test <name> ... <status>` is one entry.
fn parse(out: &str, exit: i32) -> Run {
    let test = Regex::new(r"^test (.+?) \.\.\. (ok|FAILED|ignored)\b").unwrap();
    let mut section = String::new();
    let mut entries = Vec::new();
    for l in out.lines() {
        let t = l.trim_start();
        if let Some(s) = t.strip_prefix("Running ") {
            section = s.split(" (").next().unwrap_or(s).to_string();
        } else if t.starts_with("Doc-tests ") {
            section = t.to_string();
        } else if let Some(c) = test.captures(l) {
            entries.push(Entry { section: section.clone(), name: c[1].to_string(), status: c[2].to_string() });
        }
    }
    let compile_error = out
        .lines()
        .find(|l| l.starts_with("error[") || l.starts_with("error: could not compile"))
        .filter(|_| exit != 0)
        .map(|l| l.to_string());
    Run { exit, entries, compile_error }
}

fn suite() -> R<Run> {
    let tree = copy_of_impl()?;
    let (code, out) = cargo(&tree, &["test", "--offline", "--no-fail-fast"])?;
    Ok(parse(&out, code))
}

// --- the planning store ----------------------------------------------------------------------------

fn aep(args: &[&str]) -> R<(i32, String)> {
    let actor = format!("agent:{}", std::env::var("SDC_ACTION").unwrap_or_else(|_| "story.update".into()));
    let o = Command::new("aep")
        .args(args)
        .args(["--store", "aep", "--root", "."])
        .env("AEP_ACTOR", &actor)
        .output()
        .map_err(|e| format!("aep is not on PATH: {e}"))?;
    let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    Ok((o.status.code().unwrap_or(1), text))
}

fn report() -> R<serde_yaml::Value> {
    let t = fs::read_to_string("bug.yaml").map_err(|_| "no bug.yaml".to_string())?;
    serde_yaml::from_str(&t).map_err(|e| format!("bug.yaml: {e}"))
}

fn story_id(rep: &serde_yaml::Value) -> String {
    rep.get("story").and_then(|s| s.as_str()).unwrap_or("story:bug").to_string()
}

fn story_status(name: &str) -> Option<String> {
    let t = fs::read_to_string(Path::new("aep/story").join(format!("{name}.md"))).ok()?;
    t.lines().find_map(|l| l.strip_prefix("status: ")).map(|s| s.trim().to_string())
}

/// Create the story when no parent handed it in, then move it along draft, proposed, active up to
/// `to`; a story already at or past `to` is left as it is.
fn story_to(rep: &serde_yaml::Value, to: &str) -> R<u8> {
    let id = story_id(rep);
    let name = id.strip_prefix("story:").ok_or(format!("bug.yaml story `{id}` is not story:<name>"))?;
    let executor = format!("agent:{}", std::env::var("SDC_ACTION").unwrap_or_else(|_| "story.update".into()));
    if story_status(name).is_none() {
        let text = rep.get("text").and_then(|t| t.as_str()).unwrap_or("").trim().to_string();
        let title = rep.get("title").and_then(|t| t.as_str()).map(str::to_string)
            .unwrap_or_else(|| text.chars().take(60).collect());
        let summary: String = text.split(". ").next().unwrap_or(&text).chars().take(200).collect();
        let (c, out) = aep(&["plan", "artifact", "new", "story", name, "--title", &title, "--summary", &summary])?;
        print!("{out}");
        if c != 0 {
            return Ok(1);
        }
    }
    let order = ["draft", "proposed", "active", "implemented"];
    let target = order.iter().position(|s| *s == to).unwrap();
    loop {
        let now = story_status(name).unwrap_or_default();
        let Some(i) = order.iter().position(|s| *s == now) else {
            println!("story: {id} is {now}: left as it is");
            return Ok(0);
        };
        if i >= target {
            println!("story: {id} is {now}");
            return Ok(0);
        }
        let (c, out) = aep(&["plan", "artifact", "move", &id, "--to", order[i + 1], "--executor", &executor])?;
        print!("{out}");
        if c != 0 {
            return Ok(1);
        }
    }
}

fn story_update() -> R<u8> {
    story_to(&report()?, "implemented")
}

fn seed(args: &[String]) -> R<u8> {
    let mut from = None;
    let mut text = None;
    let mut story = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--from" => from = it.next().cloned(),
            "--text" => text = it.next().cloned(),
            "--story" => story = it.next().cloned(),
            _ => return Err(format!("seed: unknown argument {a}")),
        }
    }
    let from = PathBuf::from(from.ok_or("seed --from <crate or fixture dir>")?);
    let krate = if from.join("Cargo.toml").is_file() { from.clone() } else { from.join("crate") };
    if !krate.join("Cargo.toml").is_file() {
        return Err(format!("{}: no Cargo.toml, nor crate/Cargo.toml", from.display()));
    }
    for d in ["mainline", "impl"] {
        reset_dir(Path::new(d))?;
        copy_tree(&krate, Path::new(d))?;
    }
    let mut rep: serde_yaml::Value = match &text {
        Some(t) => serde_yaml::to_value(serde_json::json!({"text": t})).unwrap(),
        None => {
            let f = from.join("bug.yaml");
            let t = fs::read_to_string(&f).map_err(|_| format!("no report text and no {}", f.display()))?;
            serde_yaml::from_str(&t).map_err(|e| format!("{}: {e}", f.display()))?
        }
    };
    if let Some(s) = story {
        rep["story"] = serde_yaml::Value::String(s);
    } else if rep.get("story").is_none() {
        rep["story"] = serde_yaml::Value::String("story:bug".into());
    }
    fs::write("bug.yaml", serde_yaml::to_string(&rep).unwrap()).map_err(|e| e.to_string())?;
    println!("seed: mainline/ and impl/ from {}, bug.yaml", krate.display());
    std::env::set_var("SDC_ACTION", "bugfix.seed");
    story_to(&rep, "active")
}

// --- files and processes -----------------------------------------------------------------------------

fn skip(name: &str) -> bool {
    name == "target" || name == ".git"
}

fn reset_dir(d: &Path) -> R<()> {
    if d.exists() {
        fs::remove_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))
}

fn copy_tree(from: &Path, to: &Path) -> R<()> {
    fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for e in fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let e = e.map_err(|e| e.to_string())?;
        let name = e.file_name().to_string_lossy().into_owned();
        if skip(&name) {
            continue;
        }
        let (src, dst) = (e.path(), to.join(&name));
        if e.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_tree(&src, &dst)?;
        } else {
            fs::copy(&src, &dst).map_err(|e| format!("{}: {e}", src.display()))?;
        }
    }
    Ok(())
}

fn files(root: &Path, rel: &Path, out: &mut Vec<String>) {
    let Ok(rd) = fs::read_dir(root.join(rel)) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if skip(&name) {
            continue;
        }
        let r = rel.join(&name);
        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            files(root, &r, out);
        } else {
            out.push(r.to_string_lossy().into_owned());
        }
    }
}

/// The files that differ between two trees, but `except`.
fn tree_diff(a: &Path, b: &Path, except: &str) -> R<Vec<String>> {
    let (mut fa, mut fb) = (Vec::new(), Vec::new());
    files(a, Path::new(""), &mut fa);
    files(b, Path::new(""), &mut fb);
    let mut all: Vec<String> = fa.iter().chain(fb.iter()).filter(|f| *f != except).cloned().collect();
    all.sort();
    all.dedup();
    Ok(all.into_iter().filter(|f| fs::read(a.join(f)).ok() != fs::read(b.join(f)).ok()).collect())
}

fn copy_of_impl() -> R<PathBuf> {
    let tree = PathBuf::from(".sdc/bugfix/tree");
    reset_dir(&tree)?;
    copy_tree(Path::new("impl"), &tree)?;
    fs::canonicalize(&tree).map_err(|e| e.to_string())
}

fn on_path(p: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|ps| std::env::split_paths(&ps).any(|d| d.join(p).is_file()))
}

/// `cargo <args>` in `dir`, through `slot` when it is on PATH, the target directory beside the
/// copy; stdout and stderr in one stream, in the order they were written.
fn cargo(dir: &Path, args: &[&str]) -> R<(i32, String)> {
    if !on_path("cargo") {
        return Err("cargo is not on PATH".into());
    }
    let mut c = if on_path("slot") {
        let mut c = Command::new("slot");
        c.arg("cargo");
        c
    } else {
        Command::new("cargo")
    };
    let log = dir.parent().unwrap().join("cargo.log");
    let f = fs::File::create(&log).map_err(|e| e.to_string())?;
    let target = dir.parent().unwrap().join("target");
    let st = c
        .args(args)
        .current_dir(dir)
        .env("CARGO_TARGET_DIR", &target)
        .stdin(Stdio::null())
        .stdout(f.try_clone().map_err(|e| e.to_string())?)
        .stderr(f)
        .status()
        .map_err(|e| format!("cargo could not start: {e}"))?;
    let out = fs::read_to_string(&log).unwrap_or_default();
    Ok((st.code().unwrap_or(1), out))
}

fn tail(s: &str, n: usize) -> Vec<&str> {
    let l: Vec<&str> = s.lines().collect();
    l[l.len().saturating_sub(n)..].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUT: &str = "   Compiling pager v0.1.0
     Running unittests src/lib.rs (target/debug/deps/pager-1)

running 2 tests
test tests::full_pages ... ok
test tests::no_items ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/last_page.rs (target/debug/deps/last_page-2)

running 1 test
test last_partial_page_is_shown ... FAILED

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
error: test failed, to rerun pass `--test last_page`
   Doc-tests pager
";

    #[test]
    fn parse_attributes_tests_to_their_target() {
        let r = parse(OUT, 101);
        assert!(r.compile_error.is_none());
        assert_eq!(r.entries.len(), 3);
        let repro = Repro { file: "tests/last_page.rs".into(), test: "last_partial_page_is_shown".into() };
        let own: Vec<&Entry> = r.entries.iter().filter(|e| e.is(&repro)).collect();
        assert_eq!(own.len(), 1);
        assert_eq!(own[0].status, "FAILED");
        assert!(r.entries.iter().filter(|e| !e.is(&repro)).all(|e| e.status == "ok"));
    }

    #[test]
    fn parse_finds_a_compile_error() {
        let out = "   Compiling pager v0.1.0\nerror[E0425]: cannot find function `nope`\nerror: could not compile `pager`\n";
        let r = parse(out, 101);
        assert!(r.compile_error.is_some());
        assert!(r.entries.is_empty());
    }

    #[test]
    fn dissection_needs_every_field_and_a_real_file() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("target/sdc-bugfix-test-{}", std::process::id()));
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src/lib.rs"), "a\nb\n").unwrap();
        let ok = r#"{"observed": "2", "expected": "3", "where": "src/lib.rs:2", "reproduce": "call f"}"#;
        assert!(dissection_errors(ok, &dir).is_empty());
        let bad = r#"{"observed": "2", "expected": "2", "where": "src/nope.rs", "extra": 1}"#;
        let e = dissection_errors(bad, &dir);
        assert!(e.iter().any(|e| e.contains("extra")));
        assert!(e.iter().any(|e| e.contains("reproduce")));
        assert!(e.iter().any(|e| e.contains("the same")));
        assert!(e.iter().any(|e| e.contains("not a file")));
        assert!(dissection_errors(r#"{"observed": "a", "expected": "b", "where": "src/lib.rs:9", "reproduce": "c"}"#, &dir)
            .iter()
            .any(|e| e.contains("line 9")));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn schemas_name_every_field() {
        assert_eq!(dissection_schema()["required"].as_array().unwrap().len(), 4);
        assert_eq!(repro_schema()["required"], json!(["file", "test"]));
    }
}
