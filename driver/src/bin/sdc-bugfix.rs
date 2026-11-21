//! sdc-bugfix: the programs of `sdc.bugfix` (protocol/sdc-bugfix.protocol.yaml, docs/bugfix.md).
//! Run by sdc-drive with cwd = the case's work directory; every path is relative to it.
//!
//!   sdc-bugfix schema dissection|repro   the JSON schema of an LLM data document
//!   sdc-bugfix schema ticket             the JSON schema of the ticket document sdc.ticket/1
//!   sdc-bugfix dissection-check          bug-dissection.json against its schema and impl/: exit 0 valid, 1 invalid
//!   sdc-bugfix repro-install             impl/ := mainline/ plus the reproducing test of repro/
//!   sdc-bugfix repro-run                 exit 0 red, 1 not red (the reason on stdout)
//!   sdc-bugfix tests-run                 exit 0 passed, 1 failed
//!   sdc-bugfix gate-run                  build, test, clippy -D warnings, fmt --check: exit 0 passed, 1 failed
//!   sdc-bugfix story-update              the bug's story (bug.yaml `story`) to implemented through aep
//!   sdc-bugfix seed --from <dir> [--text <report>] [--story <id>]
//!                                        a new case's work/: mainline/ and impl/ from the crate, bug.yaml,
//!                                        the story created and moved to active (what a parent hands in)
//!   sdc-bugfix seed --ticket <file> [--repo <worktree>]
//!                                        the same from a ticket (sdc.ticket/1) on a clean git worktree
//!                                        at the ticket's base commit: mainline/ and impl/ are exactly
//!                                        what `git ls-files` lists, bug.yaml is written from the
//!                                        ticket, .sdc/seed.json records the base (docs/bugfix.md)
//!
//! Artifacts (driver/examples/sdc/sdc-bugfix.bindings.json): bug.yaml (report), bug-dissection.json
//! (dissection), repro/ (repro: repro.json `{file, test}` and the test file), impl/ (change), aep/
//! (story). mainline/ (or bug.yaml `base`) is the unchanged code the red is observed against. Every cargo run is made in a
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
        "ticket" => ticket_schema(),
        _ => return Err("schema dissection|repro|ticket".into()),
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
    if base().join(&r.file).exists() {
        return Err(format!("{} exists in the unchanged code: the reproducing test is a new file", r.file));
    }
    Ok(r)
}

fn repro_install() -> R<u8> {
    reset_dir(Path::new("impl"))?;
    let b = base();
    copy_tree(&b, Path::new("impl"))?;
    match read_repro() {
        Ok(r) => {
            let to = Path::new("impl").join(&r.file);
            fs::create_dir_all(to.parent().unwrap()).map_err(|e| e.to_string())?;
            fs::copy(Path::new("repro").join(r.base()), &to).map_err(|e| e.to_string())?;
            println!("repro-install: impl/ is {}/ plus {}", b.display(), r.file);
        }
        // repro.run reports it, so the writer gets the reason back.
        Err(e) => println!("repro-install: impl/ is {}/ ({e})", b.display()),
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
    let diff = tree_diff(&base(), Path::new("impl"), &r.file)?;
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
    let own_run = own_run(&r)?;
    let own: Vec<&Entry> = own_run.entries.iter().filter(|e| e.is(&r)).collect();
    // In the full run the reproducing test, failing, is the one entry that may fail.
    let mut skipped = false;
    let others: Vec<&Entry> = run
        .entries
        .iter()
        .filter(|e| e.status != "ok")
        .filter(|e| !(e.is(&r) && e.status == "FAILED" && !std::mem::replace(&mut skipped, true)))
        .collect();
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
    let own_run = own_run(&r)?;
    let own = own_run.entries.iter().filter(|e| e.is(&r)).collect::<Vec<_>>();
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
    /// By name only: which target an entry of the full run belongs to cannot be told (see
    /// `own_run`); the reproducing test's own run holds only its target.
    fn is(&self, r: &Repro) -> bool {
        self.name == r.test
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

/// Parse `cargo test` output: `Running <target>` and `Doc-tests <crate>` open a section (for the
/// log only: the order of the two streams is not kept through `slot`), `test <name> ... <status>` is one entry.
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

/// The reproducing test alone, in the copy `suite` made: `cargo test --test <name> -- --exact
/// <test>`, so its entry belongs to its file by construction. The full run cannot tell: through
/// `slot`, stdout and stderr reach the log as two streams, and a `Running <target>` line may land
/// after the tests it opened (`runs/bugfix-tally-sonnet`: the green test read as "did not run").
fn own_run(r: &Repro) -> R<Run> {
    let tree = fs::canonicalize(".sdc/bugfix/tree").map_err(|e| e.to_string())?;
    let stem = r.base().trim_end_matches(".rs").to_string();
    let (code, out) = cargo(&tree, &["test", "--offline", "--test", &stem, "--", "--exact", &r.test])?;
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

/// The unchanged code the red is observed against: `bug.yaml` `base` (a parent that opens this case
/// as a child names a copy of its code at the opening), else `mainline`.
fn base() -> PathBuf {
    report()
        .ok()
        .and_then(|r| r.get("base").and_then(|b| b.as_str()).map(PathBuf::from))
        .filter(|p| p.is_relative() && !p.to_string_lossy().contains(".."))
        .unwrap_or_else(|| PathBuf::from("mainline"))
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
    let mut ticket = None;
    let mut repo = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--from" => from = it.next().cloned(),
            "--text" => text = it.next().cloned(),
            "--story" => story = it.next().cloned(),
            "--ticket" => ticket = it.next().cloned(),
            "--repo" => repo = it.next().cloned(),
            _ => return Err(format!("seed: unknown argument {a}")),
        }
    }
    if let Some(t) = ticket {
        if from.is_some() || text.is_some() || story.is_some() {
            return Err("seed --ticket <file> [--repo <worktree>]: --from, --text and --story go with the other form".into());
        }
        return seed_ticket(Path::new(&t), repo.as_deref().map(Path::new));
    }
    if repo.is_some() {
        return Err("seed: --repo goes with --ticket <file>".into());
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

// --- the ticket (sdc.ticket/1) -------------------------------------------------------------------
//
// What a controller outside writes for one ticket of any system (docs/bugfix.md, "From a ticket").
// Its text reaches the prompts (bug.yaml) and the run's own records only: no error or log line
// here quotes a value of the ticket but its id.

const TICKET_FORMAT: &str = "sdc.ticket/1";
const TICKET_KEYS: [&str; 11] =
    ["format", "id", "kind", "title", "text", "context", "source", "repository", "check", "constraints", "branch"];
const TICKET_REQUIRED: [&str; 6] = ["format", "id", "kind", "title", "text", "repository"];
const CONTEXT_KINDS: [&str; 3] = ["comment", "linked", "doc"];
const COMMIT_RE: &str = r"^([0-9a-f]{40}|[0-9a-f]{64})$";
const SYSTEM_RE: &str = r"^[A-Za-z0-9][A-Za-z0-9._-]*$";
const INSTANT_RE: &str = r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$";
const BRANCH_RE: &str = r"^[A-Za-z0-9][A-Za-z0-9._/-]*$";

/// The size cap of a repository seeded from a ticket. Every model call copies the code tree into
/// its scratch directory and back (`tools/llm-node.sh`, `input impl`, `seed impl`), so a large tree
/// costs every call. Raised for one run with SDC_SEED_MAX_FILES and SDC_SEED_MAX_BYTES.
const SEED_MAX_FILES: u64 = 2000;
const SEED_MAX_BYTES: u64 = 16 * 1024 * 1024;

struct Context {
    kind: String,
    reference: String,
    text: String,
}

struct Source {
    system: String,
    url: Option<String>,
    fetched_at: String,
}

struct Ticket {
    id: String,
    kind: String,
    title: String,
    text: String,
    context: Vec<Context>,
    source: Option<Source>,
    path: String,
    remote: Option<String>,
    base_ref: String,
    base_commit: String,
    check: Option<Vec<String>>,
    constraints: Vec<String>,
    branch: Option<String>,
}

struct Caps {
    files: u64,
    bytes: u64,
}

fn ticket_schema() -> Value {
    let s = json!({"type": "string", "minLength": 1});
    let strings = json!({"type": "array", "items": s});
    json!({
        "$id": TICKET_FORMAT, "type": "object", "additionalProperties": false, "required": TICKET_REQUIRED,
        "properties": {
            "format": {"const": TICKET_FORMAT},
            "id": {"type": "string", "minLength": 1, "description": "the ticket's key in its system"},
            "kind": {"enum": ["bug", "feature"], "description": "sdc-bugfix takes bug only"},
            "title": s, "text": s,
            "context": {"type": "array", "items": {"type": "object", "additionalProperties": false,
                "required": ["kind", "ref", "text"], "properties": {
                    "kind": {"enum": CONTEXT_KINDS}, "ref": s, "text": s}}},
            "source": {"type": "object", "additionalProperties": false, "required": ["system", "fetched_at"],
                "properties": {"system": {"type": "string", "pattern": SYSTEM_RE, "description": "a free word: jira, github, ..."},
                    "url": s, "fetched_at": {"type": "string", "pattern": INSTANT_RE}}},
            "repository": {"type": "object", "additionalProperties": false, "required": ["path", "base"],
                "properties": {"path": {"type": "string", "minLength": 1, "description": "a local git worktree; relative to the ticket file"},
                    "remote": s,
                    "base": {"type": "object", "additionalProperties": false, "required": ["ref", "commit"],
                        "properties": {"ref": s, "commit": {"type": "string", "pattern": COMMIT_RE,
                            "description": "checked against the worktree's HEAD"}}}}},
            "check": {"type": "object", "additionalProperties": false, "required": ["argv"],
                "properties": {"argv": {"type": "array", "minItems": 1, "items": s}}},
            "constraints": strings,
            "branch": {"type": "string", "pattern": BRANCH_RE}
        }
    })
}

type Ym = serde_yaml::Mapping;

/// A mapping with only `allowed` keys and every `required` one; each miss one line in `errs`.
fn mapping<'a>(v: &'a serde_yaml::Value, at: &str, allowed: &[&str], required: &[&str], errs: &mut Vec<String>) -> Option<&'a Ym> {
    let Some(m) = v.as_mapping() else {
        errs.push(format!("{at} is not a mapping"));
        return None;
    };
    for k in m.keys() {
        match k.as_str() {
            Some(k) if allowed.contains(&k) => {}
            Some(k) => errs.push(format!("{at}: key `{k}` is not in {TICKET_FORMAT} (keys: {})", allowed.join(", "))),
            None => errs.push(format!("{at}: a key is not a string")),
        }
    }
    let missing: Vec<&str> = required.iter().copied().filter(|k| m.get(*k).is_none()).collect();
    if !missing.is_empty() {
        errs.push(format!("{at}: required key{} missing: {}", if missing.len() == 1 { "" } else { "s" }, missing.join(", ")));
    }
    Some(m)
}

/// A present key as a non-empty string; never quotes the value.
fn string(m: &Ym, k: &str, at: &str, errs: &mut Vec<String>) -> Option<String> {
    match m.get(k)? {
        serde_yaml::Value::String(s) if !s.trim().is_empty() => Some(s.clone()),
        serde_yaml::Value::String(_) => {
            errs.push(format!("{at}.{k} is empty"));
            None
        }
        _ => {
            let hint = if k == "id" { " (quote a number: id: \"123\")" } else { "" };
            errs.push(format!("{at}.{k} is not a string{hint}"));
            None
        }
    }
}

fn matching(m: &Ym, k: &str, at: &str, re: &str, what: &str, errs: &mut Vec<String>) -> Option<String> {
    let s = string(m, k, at, errs)?;
    if Regex::new(re).unwrap().is_match(&s) {
        Some(s)
    } else {
        errs.push(format!("{at}.{k} is not {what}"));
        None
    }
}

fn strings(m: &Ym, k: &str, at: &str, non_empty: bool, errs: &mut Vec<String>) -> Option<Vec<String>> {
    let Some(seq) = m.get(k)?.as_sequence() else {
        errs.push(format!("{at}.{k} is not a list"));
        return None;
    };
    if non_empty && seq.is_empty() {
        errs.push(format!("{at}.{k} is empty"));
        return None;
    }
    let out: Vec<String> = seq.iter().filter_map(|v| v.as_str().filter(|s| !s.trim().is_empty()).map(str::to_string)).collect();
    if out.len() != seq.len() {
        errs.push(format!("{at}.{k}: every entry is a non-empty string"));
        return None;
    }
    Some(out)
}

/// The ticket, or every way it misses sdc.ticket/1, one line each.
fn ticket_parse(doc: &str) -> Result<Ticket, Vec<String>> {
    let v: serde_yaml::Value = serde_yaml::from_str(doc).map_err(|e| vec![format!("not YAML: {e}")])?;
    let mut errs = Vec::new();
    let Some(m) = mapping(&v, "ticket", &TICKET_KEYS, &TICKET_REQUIRED, &mut errs) else { return Err(errs) };
    let at = "ticket";
    if let Some(f) = string(m, "format", at, &mut errs) {
        if f != TICKET_FORMAT {
            errs.push(format!("ticket.format is not {TICKET_FORMAT}"));
        }
    }
    let id = string(m, "id", at, &mut errs);
    let kind = string(m, "kind", at, &mut errs).filter(|k| {
        let ok = k == "bug" || k == "feature";
        if !ok {
            errs.push("ticket.kind is not bug or feature".into());
        }
        ok
    });
    let title = string(m, "title", at, &mut errs);
    let text = string(m, "text", at, &mut errs);
    let mut context = Vec::new();
    if let Some(c) = m.get("context") {
        match c.as_sequence() {
            None => errs.push("ticket.context is not a list".into()),
            Some(seq) => {
                for (i, e) in seq.iter().enumerate() {
                    let at = format!("ticket.context[{i}]");
                    let Some(e) = mapping(e, &at, &["kind", "ref", "text"], &["kind", "ref", "text"], &mut errs) else { continue };
                    let kind = string(e, "kind", &at, &mut errs);
                    if kind.as_deref().is_some_and(|k| !CONTEXT_KINDS.contains(&k)) {
                        errs.push(format!("{at}.kind is not comment, linked or doc"));
                    }
                    if let (Some(kind), Some(reference), Some(text)) =
                        (kind, string(e, "ref", &at, &mut errs), string(e, "text", &at, &mut errs))
                    {
                        context.push(Context { kind, reference, text });
                    }
                }
            }
        }
    }
    let source = m.get("source").and_then(|s| {
        let at = "ticket.source";
        let s = mapping(s, at, &["system", "url", "fetched_at"], &["system", "fetched_at"], &mut errs)?;
        let system = matching(s, "system", at, SYSTEM_RE, "one word (jira, github, ...)", &mut errs);
        let url = string(s, "url", at, &mut errs);
        let fetched_at = matching(s, "fetched_at", at, INSTANT_RE, "an RFC 3339 instant (2026-11-21T09:00:00Z)", &mut errs);
        Some(Source { system: system?, url, fetched_at: fetched_at? })
    });
    let (mut path, mut remote, mut base_ref, mut base_commit) = (None, None, None, None);
    if let Some(r) = m.get("repository") {
        let at = "ticket.repository";
        if let Some(r) = mapping(r, at, &["path", "remote", "base"], &["path", "base"], &mut errs) {
            path = string(r, "path", at, &mut errs);
            remote = string(r, "remote", at, &mut errs);
            if let Some(b) = r.get("base") {
                let at = "ticket.repository.base";
                if let Some(b) = mapping(b, at, &["ref", "commit"], &["ref", "commit"], &mut errs) {
                    base_ref = string(b, "ref", at, &mut errs);
                    base_commit = matching(b, "commit", at, COMMIT_RE, "a full commit id (40 or 64 lower-case hex digits)", &mut errs);
                }
            }
        }
    }
    let check = m.get("check").and_then(|c| {
        let c = mapping(c, "ticket.check", &["argv"], &["argv"], &mut errs)?;
        strings(c, "argv", "ticket.check", true, &mut errs)
    });
    let constraints = strings(m, "constraints", at, false, &mut errs).unwrap_or_default();
    let branch = m.get("branch").and_then(|_| {
        let b = matching(m, "branch", at, BRANCH_RE, "a branch name (letters, digits, . _ / -)", &mut errs)?;
        if b.contains("..") || b.ends_with('/') || b.ends_with(".lock") {
            errs.push("ticket.branch is not a branch name git takes".into());
            return None;
        }
        Some(b)
    });
    if !errs.is_empty() {
        return Err(errs);
    }
    match (id, kind, title, text, path, base_ref, base_commit) {
        (Some(id), Some(kind), Some(title), Some(text), Some(path), Some(base_ref), Some(base_commit)) => Ok(Ticket {
            id, kind, title, text, context, source, path, remote, base_ref, base_commit, check, constraints, branch,
        }),
        _ => Err(vec!["ticket: incomplete".into()]),
    }
}

/// sdc-bugfix runs bug tickets only.
fn ticket_runnable(t: &Ticket) -> R<()> {
    if t.kind == "bug" {
        Ok(())
    } else {
        Err(format!("ticket {} is kind `{}`: sdc-bugfix takes bug tickets only, and the protocol for a feature \
            (sdc-feature, a feature runbook) does not exist yet", t.id, t.kind))
    }
}

/// `story:ticket-<id>`, the id lower case with every run of other characters one `-`.
fn ticket_story(id: &str) -> R<String> {
    let mut s = String::new();
    for c in id.chars().map(|c| c.to_ascii_lowercase()) {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.is_empty() && !s.ends_with('-') {
            s.push('-');
        }
    }
    let s: String = s.trim_end_matches('-').chars().take(60).collect();
    let s = s.trim_end_matches('-');
    if s.is_empty() {
        return Err("ticket.id has no letter or digit to name the story with".into());
    }
    Ok(format!("story:ticket-{s}"))
}

/// bug.yaml from the ticket: `text` is the title, the text and the context entries in that order;
/// `ticket` names it (id, kind, source) without its text.
fn ticket_report(t: &Ticket) -> R<serde_yaml::Value> {
    use serde_yaml::Value as Y;
    let mut text = format!("{}\n\n{}", t.title.trim(), t.text.trim());
    for c in &t.context {
        text.push_str(&format!("\n\n--- {} {}\n{}", c.kind, c.reference, c.text.trim()));
    }
    let mut ticket = Ym::new();
    ticket.insert("id".into(), Y::String(t.id.clone()));
    ticket.insert("kind".into(), Y::String(t.kind.clone()));
    if let Some(s) = &t.source {
        let mut src = Ym::new();
        src.insert("system".into(), Y::String(s.system.clone()));
        if let Some(u) = &s.url {
            src.insert("url".into(), Y::String(u.clone()));
        }
        src.insert("fetched_at".into(), Y::String(s.fetched_at.clone()));
        ticket.insert("source".into(), Y::Mapping(src));
    }
    let mut m = Ym::new();
    m.insert("title".into(), Y::String(t.title.trim().to_string()));
    m.insert("text".into(), Y::String(text));
    m.insert("story".into(), Y::String(ticket_story(&t.id)?));
    m.insert("ticket".into(), Y::Mapping(ticket));
    m.insert("constraints".into(), Y::Sequence(t.constraints.iter().cloned().map(Y::String).collect()));
    if let Some(argv) = &t.check {
        let mut c = Ym::new();
        c.insert("argv".into(), Y::Sequence(argv.iter().cloned().map(Y::String).collect()));
        m.insert("check".into(), Y::Mapping(c));
    }
    Ok(Y::Mapping(m))
}

fn seed_caps() -> R<Caps> {
    let get = |name: &str, default: u64| -> R<u64> {
        match std::env::var(name) {
            Ok(v) => v.trim().parse().map_err(|_| format!("{name}={v} is not a whole number")),
            Err(_) => Ok(default),
        }
    };
    Ok(Caps { files: get("SDC_SEED_MAX_FILES", SEED_MAX_FILES)?, bytes: get("SDC_SEED_MAX_BYTES", SEED_MAX_BYTES)? })
}

fn git(repo: &Path, args: &[&str]) -> R<String> {
    let o = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("git is not on PATH: {e}"))?;
    if !o.status.success() {
        return Err(format!("git {} in {}: {}", args.join(" "), repo.display(), String::from_utf8_lossy(&o.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

/// The files `git ls-files` lists in the ticket's repository and their bytes, after its checks: the
/// top of a git worktree, no change to a tracked file, HEAD at `repository.base.commit`, a
/// `Cargo.toml` at the root, no submodule or symlink, within the size cap.
fn repo_files(repo: &Path, t: &Ticket, caps: &Caps) -> R<(Vec<String>, u64)> {
    let canon = fs::canonicalize(repo).map_err(|e| format!("repository {}: {e}", repo.display()))?;
    let top = git(&canon, &["rev-parse", "--show-toplevel"])?;
    if fs::canonicalize(top.trim()).ok().as_deref() != Some(canon.as_path()) {
        return Err(format!("repository {} is not the top directory of a git worktree", repo.display()));
    }
    let dirty = git(&canon, &["status", "--porcelain", "--untracked-files=no"])?;
    if !dirty.trim().is_empty() {
        return Err(format!("repository {} is dirty: {} tracked path(s) changed; commit or stash them, then seed again",
            repo.display(), dirty.lines().count()));
    }
    let head = git(&canon, &["rev-parse", "HEAD"])?.trim().to_string();
    if head != t.base_commit {
        return Err(format!("repository {}: HEAD is {head}, the ticket's repository.base.commit is {}", repo.display(), t.base_commit));
    }
    let listed = git(&canon, &["ls-files", "-z", "--stage"])?;
    let mut files = Vec::new();
    for entry in listed.split('\0').filter(|e| !e.is_empty()) {
        let (meta, path) = entry.split_once('\t').ok_or(format!("git ls-files: unreadable entry {entry}"))?;
        match meta.split(' ').next() {
            Some("160000") => return Err(format!("repository {}: {path} is a submodule; the seed takes none", repo.display())),
            Some("120000") => return Err(format!("repository {}: {path} is a symlink; the seed takes none", repo.display())),
            _ => files.push(path.to_string()),
        }
    }
    files.dedup();
    if !files.iter().any(|f| f == "Cargo.toml") {
        return Err(format!("repository {} has no Cargo.toml at its root: sdc-bugfix takes a Rust repository built with \
            cargo only today, with Cargo.toml at the root (other languages are a later story)", repo.display()));
    }
    if files.len() as u64 > caps.files {
        return Err(format!("repository {}: {} files, above the cap of {} (SDC_SEED_MAX_FILES): every model call copies \
            the tree in and out", repo.display(), files.len(), caps.files));
    }
    let mut bytes = 0u64;
    for f in &files {
        bytes += fs::symlink_metadata(canon.join(f)).map_err(|e| format!("{f}: {e}"))?.len();
    }
    if bytes > caps.bytes {
        return Err(format!("repository {}: {bytes} bytes in {} files, above the cap of {} (SDC_SEED_MAX_BYTES): every \
            model call copies the tree in and out", repo.display(), files.len(), caps.bytes));
    }
    Ok((files, bytes))
}

/// `to` := exactly `files` of `repo`.
fn copy_listed(repo: &Path, files: &[String], to: &Path) -> R<()> {
    reset_dir(to)?;
    for f in files {
        let dst = to.join(f);
        if let Some(p) = dst.parent() {
            fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        fs::copy(repo.join(f), &dst).map_err(|e| format!("{f}: {e}"))?;
    }
    Ok(())
}

fn seed_ticket(file: &Path, repo_arg: Option<&Path>) -> R<u8> {
    let doc = fs::read_to_string(file).map_err(|e| format!("ticket {}: {e}", file.display()))?;
    let t = ticket_parse(&doc).map_err(|errs| format!("ticket {} is not {TICKET_FORMAT}:\n  {}", file.display(), errs.join("\n  ")))?;
    ticket_runnable(&t)?;
    let mut repo = PathBuf::from(&t.path);
    if repo.is_relative() {
        repo = file.parent().unwrap_or(Path::new(".")).join(repo);
    }
    if let Some(r) = repo_arg {
        if !matches!((fs::canonicalize(r), fs::canonicalize(&repo)), (Ok(a), Ok(b)) if a == b) {
            return Err(format!("--repo {} is not the ticket's repository.path {}", r.display(), repo.display()));
        }
    }
    let (files, bytes) = repo_files(&repo, &t, &seed_caps()?)?;
    for d in ["mainline", "impl"] {
        copy_listed(&repo, &files, Path::new(d))?;
    }
    let rep = ticket_report(&t)?;
    fs::write("bug.yaml", serde_yaml::to_string(&rep).unwrap()).map_err(|e| e.to_string())?;
    let record = json!({"format": "sdc.bugfix-seed/1", "ticket": {"id": t.id, "kind": t.kind, "branch": t.branch},
        "repository": {"path": fs::canonicalize(&repo).map_err(|e| e.to_string())?, "remote": t.remote,
            "base": {"ref": t.base_ref, "commit": t.base_commit}},
        "files": files.len(), "bytes": bytes});
    fs::create_dir_all(".sdc").map_err(|e| e.to_string())?;
    fs::write(".sdc/seed.json", serde_json::to_string_pretty(&record).unwrap() + "\n").map_err(|e| e.to_string())?;
    println!("seed: mainline/ and impl/ from {} at {} ({} files, {bytes} bytes, as git ls-files lists them), \
        bug.yaml from the ticket, .sdc/seed.json", repo.display(), t.base_commit, files.len());
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
/// copy; stdout and stderr into one log file.
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
        assert_eq!(ticket_schema()["required"], json!(TICKET_REQUIRED));
        assert_eq!(ticket_schema()["properties"].as_object().unwrap().len(), TICKET_KEYS.len());
    }

    // --- the ticket: invented tickets only ---

    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    fn ticket_doc(commit: &str) -> String {
        format!("format: sdc.ticket/1
id: PAGER-7
kind: bug
title: The last partial page is never shown
text: With 11 items and 5 per page, page_count returns 2.
context:
- kind: comment
  ref: comment-1
  text: Any total that is not a multiple of the page size.
- kind: linked
  ref: PAGER-3
  text: The page view.
source:
  system: example
  url: https://tickets.example.invalid/PAGER-7
  fetched_at: 2026-11-21T09:00:00Z
repository:
  path: repo
  base:
    ref: main
    commit: {commit}
check:
  argv: [cargo, test]
constraints:
- Keep the public API of the crate unchanged.
branch: fix/pager-7
")
    }

    fn errs_of(doc: &str) -> Vec<String> {
        match ticket_parse(doc) {
            Ok(_) => vec![],
            Err(e) => e,
        }
    }

    #[test]
    fn a_full_and_a_minimal_ticket_parse() {
        let t = ticket_parse(&ticket_doc(COMMIT)).unwrap();
        assert_eq!((t.id.as_str(), t.kind.as_str(), t.context.len()), ("PAGER-7", "bug", 2));
        assert_eq!(t.check.as_deref(), Some(&["cargo".to_string(), "test".to_string()][..]));
        assert_eq!(t.branch.as_deref(), Some("fix/pager-7"));
        let min = format!("format: sdc.ticket/1\nid: \"7\"\nkind: bug\ntitle: t\ntext: x\nrepository: {{path: r, base: {{ref: main, commit: {COMMIT}}}}}\n");
        let t = ticket_parse(&min).unwrap();
        assert!(t.context.is_empty() && t.source.is_none() && t.check.is_none() && t.constraints.is_empty());
    }

    #[test]
    fn every_refusal_of_the_format_is_named() {
        let ok = ticket_doc(COMMIT);
        let cases: Vec<(String, &str)> = vec![
            (ok.replace("branch:", "owner: someone\nbranch:"), "key `owner` is not in sdc.ticket/1"),
            (ok.replace("    ref: main\n", "    ref: main\n    tag: v1\n"), "ticket.repository.base: key `tag`"),
            (ok.replace("title: The last partial page is never shown\n", "").replace("text: With", "txt: With"),
                "ticket: required keys missing: title, text"),
            (ok.replace("sdc.ticket/1", "sdc.ticket/2"), "ticket.format is not sdc.ticket/1"),
            (ok.replace("kind: bug", "kind: epic"), "ticket.kind is not bug or feature"),
            (ok.replace("kind: comment", "kind: chat"), "ticket.context[0].kind is not comment, linked or doc"),
            (ok.replace("  ref: comment-1\n", ""), "ticket.context[0]: required key missing: ref"),
            (ok.replace(COMMIT, "abc123"), "ticket.repository.base.commit is not a full commit id"),
            (ok.replace("id: PAGER-7", "id: 7"), "ticket.id is not a string (quote a number"),
            (ok.replace("2026-11-21T09:00:00Z", "yesterday"), "ticket.source.fetched_at is not an RFC 3339 instant"),
            (ok.replace("  system: example\n", ""), "ticket.source: required key missing: system"),
            (ok.replace("argv: [cargo, test]", "argv: []"), "ticket.check.argv is empty"),
            (ok.replace("- Keep the public API of the crate unchanged.", "- 3"), "ticket.constraints: every entry is a non-empty string"),
            (ok.replace("branch: fix/pager-7", "branch: fix..x"), "ticket.branch is not a branch name"),
            (ok.replace("repository:\n  path: repo\n", "repository:\n"), "ticket.repository: required key missing: path"),
            ("- a list".to_string(), "ticket is not a mapping"),
        ];
        for (doc, want) in cases {
            let e = errs_of(&doc);
            assert!(e.iter().any(|e| e.contains(want)), "want `{want}`, got {e:?}");
        }
    }

    #[test]
    fn no_refusal_quotes_the_ticket_text() {
        let doc = ticket_doc(COMMIT).replace("kind: bug", "kind: secret-word").replace("2026-11-21T09:00:00Z", "secret-instant");
        let e = errs_of(&doc).join("\n");
        assert!(!e.is_empty() && !e.contains("secret"), "{e}");
    }

    #[test]
    fn a_feature_ticket_is_refused_naming_the_missing_protocol() {
        let t = ticket_parse(&ticket_doc(COMMIT).replace("kind: bug", "kind: feature")).unwrap();
        let e = ticket_runnable(&t).unwrap_err();
        assert!(e.contains("sdc-feature") && e.contains("bug tickets only"), "{e}");
        assert!(ticket_runnable(&ticket_parse(&ticket_doc(COMMIT)).unwrap()).is_ok());
    }

    #[test]
    fn bug_yaml_carries_the_ticket_text_in_order_and_names_it_without_text() {
        let t = ticket_parse(&ticket_doc(COMMIT)).unwrap();
        let y = ticket_report(&t).unwrap();
        assert_eq!(y["text"].as_str().unwrap(), "The last partial page is never shown\n\n\
            With 11 items and 5 per page, page_count returns 2.\n\n\
            --- comment comment-1\nAny total that is not a multiple of the page size.\n\n\
            --- linked PAGER-3\nThe page view.");
        assert_eq!(y["title"].as_str(), Some("The last partial page is never shown"));
        assert_eq!(y["story"].as_str(), Some("story:ticket-pager-7"));
        assert_eq!(y["ticket"]["id"].as_str(), Some("PAGER-7"));
        assert_eq!(y["ticket"]["kind"].as_str(), Some("bug"));
        assert_eq!(y["ticket"]["source"]["system"].as_str(), Some("example"));
        assert_eq!(y["ticket"].as_mapping().unwrap().len(), 3);
        assert_eq!(y["constraints"][0].as_str(), Some("Keep the public API of the crate unchanged."));
        assert_eq!(y["check"]["argv"][1].as_str(), Some("test"));
        assert!(y.get("base").is_none(), "bug.yaml `base` is the base directory, not the commit");
        assert_eq!(ticket_story("Proj/ABC  12--").unwrap(), "story:ticket-proj-abc-12");
        assert!(ticket_story("---").is_err());
    }

    /// git in a fixture repository, with no hook, signing or identity of the machine's.
    fn g(dir: &Path, args: &[&str]) -> String {
        let o = Command::new("git")
            .args(["-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false",
                "-c", "core.hooksPath=/dev/null", "-c", "init.defaultBranch=main"])
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    }

    /// A repository in target/: Cargo.toml (unless `cargo` is false), src/lib.rs, .gitignore,
    /// committed; then an ignored and an untracked file.
    fn fixture_repo(name: &str, cargo: bool) -> (PathBuf, String) {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("target/sdc-bugfix-repo-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        g(&dir, &["init", "-q"]);
        if cargo {
            fs::write(dir.join("Cargo.toml"), "[package]\nname = \"pager\"\nversion = \"0.1.0\"\nedition = \"2021\"\n").unwrap();
        }
        fs::write(dir.join("src/lib.rs"), "pub fn f() {}\n").unwrap();
        fs::write(dir.join(".gitignore"), "ignored.txt\n").unwrap();
        g(&dir, &["add", "."]);
        g(&dir, &["commit", "-q", "-m", "fixture"]);
        fs::write(dir.join("ignored.txt"), "ignored\n").unwrap();
        fs::write(dir.join("untracked.txt"), "untracked\n").unwrap();
        let head = g(&dir, &["rev-parse", "HEAD"]);
        (dir, head)
    }

    const CAPS: Caps = Caps { files: SEED_MAX_FILES, bytes: SEED_MAX_BYTES };

    #[test]
    fn the_seed_copies_exactly_what_git_ls_files_lists() {
        let (dir, head) = fixture_repo("copy", true);
        let t = ticket_parse(&ticket_doc(&head)).unwrap();
        let (files, bytes) = repo_files(&dir, &t, &CAPS).unwrap();
        assert_eq!(files, vec![".gitignore", "Cargo.toml", "src/lib.rs"]);
        assert!(bytes > 0);
        let to = dir.with_extension("copy");
        copy_listed(&dir, &files, &to).unwrap();
        assert!(to.join("src/lib.rs").is_file() && to.join("Cargo.toml").is_file());
        assert!(!to.join("ignored.txt").exists() && !to.join("untracked.txt").exists() && !to.join(".git").exists());
        fs::remove_dir_all(&to).unwrap();
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_dirty_worktree_and_a_wrong_head_are_refused() {
        let (dir, head) = fixture_repo("dirty", true);
        let t = ticket_parse(&ticket_doc(&head)).unwrap();
        fs::write(dir.join("src/lib.rs"), "pub fn g() {}\n").unwrap();
        let e = repo_files(&dir, &t, &CAPS).unwrap_err();
        assert!(e.contains("is dirty"), "{e}");
        fs::write(dir.join("src/lib.rs"), "pub fn f() {}\n").unwrap();
        assert!(repo_files(&dir, &t, &CAPS).is_ok());
        let wrong = ticket_parse(&ticket_doc(COMMIT)).unwrap();
        let e = repo_files(&dir, &wrong, &CAPS).unwrap_err();
        assert!(e.contains(&format!("HEAD is {head}")) && e.contains(COMMIT), "{e}");
        let e = repo_files(&dir.join("src"), &t, &CAPS).unwrap_err();
        assert!(e.contains("not the top directory"), "{e}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_repository_without_cargo_toml_is_refused_naming_the_limit() {
        let (dir, head) = fixture_repo("nocargo", false);
        let t = ticket_parse(&ticket_doc(&head)).unwrap();
        let e = repo_files(&dir, &t, &CAPS).unwrap_err();
        assert!(e.contains("no Cargo.toml at its root") && e.contains("other languages are a later story"), "{e}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_size_cap_refuses_files_and_bytes() {
        let (dir, head) = fixture_repo("cap", true);
        let t = ticket_parse(&ticket_doc(&head)).unwrap();
        let e = repo_files(&dir, &t, &Caps { files: 2, bytes: SEED_MAX_BYTES }).unwrap_err();
        assert!(e.contains("3 files, above the cap of 2 (SDC_SEED_MAX_FILES)"), "{e}");
        let e = repo_files(&dir, &t, &Caps { files: SEED_MAX_FILES, bytes: 10 }).unwrap_err();
        assert!(e.contains("above the cap of 10 (SDC_SEED_MAX_BYTES)"), "{e}");
        assert!(repo_files(&dir, &t, &Caps { files: 3, bytes: 1 << 20 }).is_ok());
        fs::remove_dir_all(&dir).unwrap();
    }
}
