//! sdc-bugfix: the programs of `sdc.bugfix` (protocol/sdc-bugfix.protocol.yaml, docs/bugfix.md).
//! Run by Loom with cwd = the case's work directory; every path is relative to it.
//!
//!   sdc-bugfix schema dissection|repro   the JSON schema of an LLM data document
//!   sdc-bugfix schema ticket             the JSON schema of the ticket document sdc.ticket/1
//!   sdc-bugfix checks                    the case's commands (checks.yaml, sdc.checks/1) as they will run
//!   sdc-bugfix dissection-check          bug-dissection.json against its schema and impl/: exit 0 valid, 1 invalid
//!   sdc-bugfix repro-install             impl/ := mainline/ plus the reproducing test of repro/
//!   sdc-bugfix repro-run                 exit 0 red, 1 not red (the reason on stdout)
//!   sdc-bugfix tests-run                 exit 0 passed, 1 failed
//!   sdc-bugfix gate-run                  every gate command of checks.yaml: exit 0 passed, 1 failed
//!   sdc-bugfix story-update              the bug's story (bug.yaml `story`) to implemented through aep
//!   sdc-bugfix handback-describe         handback/mr-description.md: the fix summary and the ticket's text
//!   sdc-bugfix handback-check -- <check argv>
//!                                        runs `<check argv> handback/mr-description.md <kind>` (bug.yaml
//!                                        `change_kind`, default defect): exit 0 allowed, 77 refused, the
//!                                        check's one line passed through; 75, 126, 127 and any other end
//!                                        are program errors
//!   sdc-bugfix spec-check [--spec-dir <dir>]... [--spec-repo <url>]... <description file> <kind>
//!                                        the default check: one line `spec-first: allowed|refused: ...`,
//!                                        exit 0 or 77
//!   sdc-bugfix seed --from <dir> [--text <report>] [--story <id>] [--checks <file>]
//!                                        a new case's work/: mainline/ and impl/ from the code tree
//!                                        (<dir>, <dir>/crate or <dir>/repo), checks.yaml, bug.yaml, the
//!                                        story created and moved to active (what a parent hands in)
//!   sdc-bugfix seed --ticket <file> [--repo <worktree>] [--checks <file>]
//!                                        the same from a ticket (sdc.ticket/1) on a clean git worktree
//!                                        at the ticket's base commit: mainline/ and impl/ are exactly
//!                                        what `git ls-files` lists, bug.yaml is written from the
//!                                        ticket, .sdc/seed.json records the base (docs/bugfix.md)
//!
//! Artifacts (driver/examples/sdc/sdc-bugfix.bindings.json): bug.yaml (report), bug-dissection.json
//! (dissection), repro/ (repro: repro.json `{file, test}` and the test file), impl/ (change), aep/
//! (story). mainline/ (or bug.yaml `base`) is the unchanged code the red is observed against. The
//! commands are the case's checks.yaml (seed: `--checks`, else the repository's
//! .runbook/checks.yaml, else the detection table examples/sdc/checks-detect.yaml). Every command
//! runs as an argv, never through a shell, in a copy of impl/ (or of the base) under .sdc/bugfix/
//! (nothing is written into an artifact), through `slot` when it is on PATH. A program that is
//! missing exits 127 (nothing is recorded), never a check result; so does an unusable
//! checks.yaml (126).
//!
//! The build pool (docs/driver.md, "The build pool wait"): `repro-run`, `tests-run` and `gate-run`
//! bound their wait for a slot (`--pool-wait <seconds>`, or SDC_POOL_WAIT, which wins; one bound per
//! step). A job still queued at the bound is cancelled and the step exits 69, nothing ran; a job the
//! pool ended at its run limit (`slot` exit 124) is exit 70. Neither is a verdict about the code.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use regex::Regex;
use serde_json::{json, Value};

const DISSECTION: &str = "bug-dissection.json";
const FIELDS: [(&str, &str); 4] = [
    ("observed", "what happens, as the report states it: inputs and the wrong result"),
    ("expected", "what should happen instead, for the same inputs"),
    ("where", "the file of the code tree the bug is in, relative to the tree, optionally with :<line> (path/to/file:9)"),
    ("reproduce", "how to reproduce it: the call or command and its inputs"),
];

type R<T> = Result<T, String>;

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // `--pool-wait <seconds>`: the step's bound for waiting in the build pool's queue, as for
    // sdc-tool.sh (SDC_POOL_WAIT in the environment wins over it). Read before a `--` only: what
    // follows it is the argv of another program (`handback-check -- <check argv>`).
    let own = args.iter().position(|a| a == "--").unwrap_or(args.len());
    if let Some(i) = args[..own].iter().position(|a| a == "--pool-wait") {
        match args.get(i + 1).filter(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit())).and_then(|v| v.parse().ok()) {
            Some(secs) => {
                let _ = POOL_WAIT_ARG.set(secs);
                args.drain(i..i + 2);
            }
            None => {
                eprintln!("sdc-bugfix: {BAD_BOUND}");
                return ExitCode::from(64);
            }
        }
    }
    if let Err(e) = pool_bound() {
        eprintln!("sdc-bugfix: {e}");
        return ExitCode::from(64);
    }
    let r = match args.first().map(String::as_str) {
        Some("schema") => schema(args.get(1).map(String::as_str).unwrap_or("")),
        Some("checks") => show_checks(),
        Some("dissection-check") => dissection_check(),
        Some("repro-install") => repro_install(),
        Some("repro-run") => repro_run(),
        Some("tests-run") => tests_run(),
        Some("gate-run") => gate_run(),
        Some("story-update") => story_update(),
        Some("seed") => seed(&args[1..]),
        // The hand-back steps answer with their own exit codes (docs/bugfix.md, "Hand-back: the
        // spec reference"): no error of theirs goes through the mapping below.
        Some("handback-describe") => return ExitCode::from(handback_describe()),
        Some("handback-check") => return ExitCode::from(handback_check(&args[1..])),
        Some("spec-check") => return ExitCode::from(spec_check(&args[1..])),
        _ => Err("usage: sdc-bugfix schema|checks|dissection-check|repro-install|repro-run|tests-run|gate-run|story-update|seed|\
            handback-describe|handback-check|spec-check".into()),
    };
    match r {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("sdc-bugfix: {e}");
            // A missing program (127) or commands that cannot run (126) are program errors. The seed's refusals are 2.
            let seed = args.first().is_some_and(|a| a == "seed");
            // The build pool's: no slot within the wait bound (nothing ran), or a job the pool ended
            // at its run limit (docs/driver.md, "The build pool wait").
            ExitCode::from(if e.starts_with(POOL_WAIT) {
                POOL_WAIT_EXIT
            } else if let Some(rest) = e.strip_prefix(NOT_RUN) {
                if rest.starts_with("exit 126") {
                    126
                } else {
                    127
                }
            } else if e.starts_with(POOL_OVER) {
                OVER_TIME_EXIT
            } else if e.contains("not on PATH") {
                127
            } else if e.starts_with(CHECKS) && !seed {
                126
            } else {
                2
            })
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

/// The repro record's schema under the case's commands: the test file and name patterns of
/// checks.yaml.
fn repro_schema(ck: &Checks) -> Value {
    json!({"type": "object", "additionalProperties": false, "required": ["file", "test"], "properties": {
        "file": {"type": "string", "pattern": ck.test_file,
            "description": "the new test file, relative to the code tree (checks.yaml test_file)"},
        "test": {"type": "string", "pattern": ck.test_name,
            "description": "the name of the one test in that file that reproduces the bug"}}})
}

fn schema(which: &str) -> R<u8> {
    let s = match which {
        "dissection" => dissection_schema(),
        // Outside a case (no checks.yaml, no base), the Rust row of the detection table.
        "repro" => repro_schema(&match load_checks() {
            Ok((ck, _)) => ck,
            Err(_) => detect_rows()?.into_iter().find(|r| r.name == "rust").ok_or("the detection table has no rust row")?.checks,
        }),
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
    /// The test file's name in repro/ (the file without its directories).
    fn base(&self) -> &str {
        self.file.rsplit('/').next().unwrap()
    }

    /// The placeholders of checks.yaml: `{file}`, `{dir}` (`.` at the root), `{stem}` (the file
    /// name without its last extension), `{test}`.
    fn vars(&self) -> [(&'static str, String); 4] {
        let p = Path::new(&self.file);
        let dir = p.parent().map(|d| d.to_string_lossy().into_owned()).filter(|d| !d.is_empty()).unwrap_or_else(|| ".".into());
        let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        [("{file}", self.file.clone()), ("{dir}", dir), ("{stem}", stem), ("{test}", self.test.clone())]
    }
}

/// repro/repro.json against its schema, its test file present and holding the named test, and the
/// file new to the code tree; the file and name patterns are the case's (checks.yaml).
fn read_repro(ck: &Checks) -> Result<Repro, String> {
    let t = fs::read_to_string("repro/repro.json").map_err(|_| "no repro/repro.json".to_string())?;
    let v: Value = serde_json::from_str(&t).map_err(|e| format!("repro/repro.json is not JSON: {e}"))?;
    let o = v.as_object().ok_or("repro/repro.json is not an object")?;
    if let Some(k) = o.keys().find(|k| *k != "file" && *k != "test") {
        return Err(format!("repro/repro.json: key `{k}` is not in the schema (file, test)"));
    }
    let file = o.get("file").and_then(Value::as_str).unwrap_or("").to_string();
    let test = o.get("test").and_then(Value::as_str).unwrap_or("").to_string();
    // Inside the tree whatever the pattern says.
    let outside = file.starts_with('/') || file.split('/').any(|c| c.is_empty() || c == "." || c == "..");
    if outside || !Regex::new(&ck.test_file).unwrap().is_match(&file) {
        return Err(format!("repro/repro.json: file `{file}` does not match checks.yaml test_file {}", ck.test_file));
    }
    if !Regex::new(&ck.test_name).unwrap().is_match(&test) {
        return Err(format!("repro/repro.json: test `{test}` does not match checks.yaml test_name {}", ck.test_name));
    }
    let r = Repro { file, test };
    let src = fs::read_to_string(Path::new("repro").join(r.base()))
        .map_err(|_| format!("repro/{} (the test file repro.json names) is missing", r.base()))?;
    match &ck.test_def {
        Some(d) if !Regex::new(&subst_re(d, &r)).unwrap().is_match(&src) => {
            return Err(format!("repro/{} does not define test {} (checks.yaml test_def {d})", r.base(), r.test));
        }
        None if !src.contains(&r.test) => return Err(format!("repro/{} does not name test {}", r.base(), r.test)),
        _ => {}
    }
    if let Some(f) = ck.test_forbid.as_deref().filter(|f| Regex::new(f).unwrap().is_match(&src)) {
        return Err(format!("repro/{} skips, ignores or expects the failure of a test (checks.yaml test_forbid {f})", r.base()));
    }
    if base().join(&r.file).exists() {
        return Err(format!("{} exists in the unchanged code: the reproducing test is a new file", r.file));
    }
    Ok(r)
}

fn repro_install() -> R<u8> {
    let ck = load_checks()?.0;
    reset_dir(Path::new("impl"))?;
    let b = base();
    copy_tree(&b, Path::new("impl"))?;
    match read_repro(&ck) {
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

/// The output of a failed run, its last 40 lines, for the writer that is sent back.
fn print_tail(out: &str) {
    for l in tail(out, 40) {
        println!("  | {l}");
    }
}

/// Red: the reproducing test alone (checks.yaml `repro`) fails as a test on the unchanged code
/// with the test (the `red` rule; without one, any exit but 0), and the whole suite (`test`) on
/// the unchanged code without the test passes (the `suite` rule; without one, exit 0). The second
/// run is what tells "the test fails" from "the code fails" for every language: a failure that the
/// suite shows without the test was not caused by the test. The test's status is taken from its own
/// run because, through `slot`, stdout and stderr reach the log as two streams, so a full run cannot
/// say which target a line belongs to (`runs/bugfix-tally-sonnet`).
fn repro_run() -> R<u8> {
    let ck = load_checks()?.0;
    let r = match read_repro(&ck) {
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
    let tree = copy_of_impl()?;
    let argv = ck.repro_argv(&r);
    let (code, out) = run(&tree, &argv, &ck)?;
    let verdict = if code == 0 {
        Some(format!("test {} passes on the unchanged code: it does not reproduce the bug (`{}` exit 0)", r.test, argv.join(" ")))
    } else if !rule_holds(ck.red.as_ref(), false, code, &out, Some(&r)) {
        Some(format!("test {} did not fail as a test (`{}` exit {code}, red rule {}): it does not build or load, it did not \
            run, or the command failed", r.test, argv.join(" "), describe(ck.red.as_ref())))
    } else {
        None
    };
    if let Some(v) = verdict {
        println!("repro: not red: {v}");
        print_tail(&out);
        return Ok(1);
    }
    let base = copy_of_base()?;
    let (scode, sout) = run(&base, &ck.test, &ck)?;
    if !rule_holds(ck.suite.as_ref(), true, scode, &sout, None) {
        println!("repro: not red: other tests do not pass on the unchanged code: `{}` without the test exits {scode} \
            (suite rule {})", ck.test.join(" "), describe(ck.suite.as_ref()));
        print_tail(&sout);
        return Ok(1);
    }
    println!("repro: red: test {} ({}) fails on the unchanged code (`{}` exit {code}), the suite without it passes (`{}`)",
        r.test, r.file, argv.join(" "), ck.test.join(" "));
    Ok(0)
}

/// Passed: the reproducing test is byte for byte as written, its own run passes (the `green` rule;
/// without one, exit 0) and the whole suite passes (the `suite` rule; without one, exit 0).
fn tests_run() -> R<u8> {
    let ck = load_checks()?.0;
    let r = match read_repro(&ck) {
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
    let tree = copy_of_impl()?;
    let (scode, sout) = run(&tree, &ck.test, &ck)?;
    let argv = ck.repro_argv(&r);
    let (code, out) = run(&tree, &argv, &ck)?;
    if !rule_holds(ck.green.as_ref(), true, code, &out, Some(&r)) {
        println!("tests: FAILED test {} ({}) does not pass: `{}` exit {code} (green rule {})", r.test, r.file,
            argv.join(" "), describe(ck.green.as_ref()));
        print_tail(&out);
        return Ok(1);
    }
    if !rule_holds(ck.suite.as_ref(), true, scode, &sout, None) {
        println!("tests: FAILED not every test passes: `{}` exit {scode} (suite rule {})", ck.test.join(" "),
            describe(ck.suite.as_ref()));
        print_tail(&sout);
        return Ok(1);
    }
    println!("tests: passed: test {} ({}) ok, the whole suite passes (`{}`)", r.test, r.file, ck.test.join(" "));
    Ok(0)
}

/// Every gate command of checks.yaml, in order, each must exit 0; every one runs, so the log names
/// all that fail.
fn gate_run() -> R<u8> {
    let ck = load_checks()?.0;
    let tree = copy_of_impl()?;
    let mut ok = true;
    for (name, argv) in &ck.gate {
        let (code, out) = run(&tree, argv, &ck)?;
        if code == 0 {
            println!("gate: ok     {name}: {}", argv.join(" "));
        } else {
            ok = false;
            println!("gate: FAILED {name}: {} exit {code}", argv.join(" "));
            print_tail(&out);
        }
    }
    if ck.gate.is_empty() {
        println!("gate: ok     checks.yaml has no gate command");
    }
    Ok(if ok { 0 } else { 1 })
}

// --- hand-back: the description and its spec reference ------------------------------------------
//
// docs/bugfix.md, "Hand-back: the spec reference". After the gate, `handback-describe` writes the
// description of the code merge request; `handback-check -- <check argv>` runs a check program as
// `<check argv> <description file> <kind>` and keeps its interface: one line `spec-first: allowed:
// <why>` and exit 0, or `spec-first: refused: <why>` and exit 77 (a tool error of the check is 77
// too); 75, 126 and 127 are program errors. `spec-check` is this repository's default check.

const DESCRIPTION: &str = "handback/mr-description.md";
const ALLOWED: &str = "spec-first: allowed: ";
const REFUSED: &str = "spec-first: refused: ";
const SPEC_REFUSED_EXIT: u8 = 77;
/// The exits of a check that are a program error: no result is recorded.
const CHECK_ERRORS: [i32; 3] = [75, 126, 127];

/// The case's change kind: bug.yaml `change_kind` (from the ticket's), `defect` when it has none.
fn change_kind() -> R<String> {
    match report()?.get("change_kind") {
        None => Ok("defect".into()),
        Some(v) => match v.as_str() {
            Some(k) if CHANGE_KINDS.contains(&k) => Ok(k.to_string()),
            _ => Err("bug.yaml change_kind is not defect or change".into()),
        },
    }
}

/// The description from what the run holds: bug.yaml (title, text), the dissection, the repro
/// record and the files impl/ changed against the base. No model call: these suffice.
fn description_text() -> R<String> {
    let rep = report()?;
    let text = rep.get("text").and_then(|t| t.as_str()).map(str::trim).filter(|t| !t.is_empty())
        .ok_or("bug.yaml has no text")?.to_string();
    let title = rep.get("title").and_then(|t| t.as_str()).map(str::trim).filter(|t| !t.is_empty()).map(str::to_string)
        .unwrap_or_else(|| text.lines().next().unwrap_or("").chars().take(80).collect());
    let d: Value = serde_json::from_str(&fs::read_to_string(DISSECTION).map_err(|_| format!("no {DISSECTION}"))?)
        .map_err(|_| format!("{DISSECTION} is not JSON"))?;
    let field = |k: &str| d.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
        .ok_or(format!("{DISSECTION} has no `{k}`"));
    let (observed, expected, place) = (field("observed")?, field("expected")?, field("where")?);
    let r: Value = serde_json::from_str(&fs::read_to_string("repro/repro.json").map_err(|_| "no repro/repro.json".to_string())?)
        .map_err(|_| "repro/repro.json is not JSON".to_string())?;
    let (file, test) = match (r.get("file").and_then(Value::as_str), r.get("test").and_then(Value::as_str)) {
        (Some(f), Some(t)) => (f.to_string(), t.to_string()),
        _ => return Err("repro/repro.json names no file and test".into()),
    };
    let changed = tree_diff(&base(), Path::new("impl"), &file)?;
    let changed = if changed.is_empty() {
        "none besides the test".to_string()
    } else {
        changed.iter().map(|f| format!("`{f}`")).collect::<Vec<_>>().join(", ")
    };
    Ok(format!("# {title}\n\n## The fix\n\n{observed}\n\nExpected: {expected}\n\n- Where: `{place}`\n\
        - Reproducing test: `{test}` in `{file}`, red on the unchanged code and green on the fix\n\
        - Changed files: {changed}\n\n## The ticket\n\n{text}\n"))
}

/// `handback-describe`: writes handback/mr-description.md; exit 0. An input that is missing or
/// unreadable is a program error (126). Nothing of the ticket's text is printed.
fn handback_describe() -> u8 {
    let text = match description_text() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("sdc-bugfix: handback-describe: {e}");
            return 126;
        }
    };
    if let Err(e) = fs::create_dir_all("handback").and_then(|_| fs::write(DESCRIPTION, &text)) {
        eprintln!("sdc-bugfix: handback-describe: {DESCRIPTION}: {e}");
        return 126;
    }
    println!("handback: wrote {DESCRIPTION} ({} bytes)", text.len());
    0
}

/// `handback-check -- <check argv>`: runs the check with the description file and the case's kind
/// appended, passes its standard output through and keeps its exit when the interface holds; every
/// other end is a program error (126), never a result.
fn handback_check(args: &[String]) -> u8 {
    let argv = match args.split_first() {
        Some((sep, rest)) if sep == "--" && !rest.is_empty() => rest,
        _ => {
            eprintln!("sdc-bugfix: handback-check -- <check argv>: the check's argv follows `--`");
            return 126;
        }
    };
    let kind = match change_kind() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("sdc-bugfix: handback-check: {e}");
            return 126;
        }
    };
    // The default check is this program: found beside itself, whatever PATH holds.
    let prog = match argv[0].as_str() {
        "sdc-bugfix" => std::env::current_exe().unwrap_or_else(|_| PathBuf::from("sdc-bugfix")),
        p => PathBuf::from(p),
    };
    let out = Command::new(&prog).args(&argv[1..]).arg(DESCRIPTION).arg(&kind).stdin(Stdio::null()).stderr(Stdio::inherit()).output();
    let o = match out {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("sdc-bugfix: handback-check: the check {} is not found: a program error, no result", argv[0]);
            return 127;
        }
        Err(e) => {
            eprintln!("sdc-bugfix: handback-check: the check {} cannot be run ({e}): a program error, no result", argv[0]);
            return 126;
        }
    };
    let stdout = String::from_utf8_lossy(&o.stdout);
    print!("{stdout}");
    let Some(code) = o.status.code() else {
        eprintln!("sdc-bugfix: handback-check: the check was ended by a signal; the interface knows exits only: a program error, no result");
        return 126;
    };
    if CHECK_ERRORS.contains(&code) {
        eprintln!("sdc-bugfix: handback-check: the check exited {code}: a program error, no result");
        return code as u8;
    }
    let want = match code {
        0 => ALLOWED,
        77 => REFUSED,
        _ => {
            eprintln!("sdc-bugfix: handback-check: the check exited {code}; the interface knows 0 (allowed), 77 (refused) and \
                75, 126 and 127 (program errors): a program error, no result");
            return 126;
        }
    };
    let lines: Vec<&str> = stdout.lines().collect();
    if lines.len() != 1 || !lines[0].starts_with(want) || lines[0].len() == want.len() {
        eprintln!("sdc-bugfix: handback-check: the check exited {code} with {} line(s) on standard output; the interface asks \
            for one line `{want}<why>`: a program error, no result", lines.len());
        return 126;
    }
    code as u8
}

/// `spec-check [--spec-dir <dir>]... [--spec-repo <url>]... <description file> <kind>`, the default
/// check: one line on standard output, exit 0 allowed, 77 refused (a usage or read error too).
fn spec_check(args: &[String]) -> u8 {
    let (ok, why) = match spec_verdict(args) {
        Ok(v) => v,
        Err(e) => (false, e),
    };
    // One line, whatever a quoted reference held.
    let why: String = why.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    if ok {
        println!("{ALLOWED}{why}");
        0
    } else {
        println!("{REFUSED}{why}");
        SPEC_REFUSED_EXIT
    }
}

/// A token of the description, its quoting and punctuation taken off.
fn spec_token(raw: &str) -> &str {
    raw.trim_start_matches(['`', '\'', '"', '(', '[', '<', '{', '*'])
        .trim_end_matches(['`', '\'', '"', ')', ']', '>', '}', ',', ';', '.', '!', '?', ':', '*'])
}

/// A URL's path segments (no scheme, host, query or fragment) and its fragment.
fn url_parts(url: &str) -> (Vec<&str>, Option<&str>) {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let (rest, fragment) = match rest.split_once('#') {
        Some((r, f)) => (r, Some(f)),
        None => (rest, None),
    };
    let rest = rest.split('?').next().unwrap_or(rest);
    let path = rest.split_once('/').map(|(_, p)| p).unwrap_or("");
    (path.split('/').filter(|s| !s.is_empty()).collect(), fragment)
}

/// The verdict and its reason: `change` passes with a merge request link of a spec repository or a
/// path under a named spec directory; `defect` with a spec line (`<file>:<line>` under a spec
/// directory, or a link into a spec repository with a line anchor) or the words "spec says nothing".
/// A spec repository is a URL prefix `--spec-repo` names; with none named, a URL with a path segment
/// that holds `spec`.
fn spec_verdict(args: &[String]) -> R<(bool, String)> {
    const USAGE: &str = "usage: sdc-bugfix spec-check [--spec-dir <dir>]... [--spec-repo <url>]... <description file> <kind>";
    let (mut dirs, mut repos, mut pos) = (Vec::new(), Vec::new(), Vec::new());
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--spec-dir" => {
                let d = it.next().ok_or(USAGE)?.trim_end_matches('/').to_string();
                if d.is_empty() || d.starts_with('/') || d.split('/').any(|c| c.is_empty() || c == "." || c == "..") {
                    return Err(format!("--spec-dir {d} is not a relative directory without `.` or `..`"));
                }
                dirs.push(d);
            }
            "--spec-repo" => {
                let r = it.next().ok_or(USAGE)?.to_string();
                if !(r.starts_with("https://") || r.starts_with("http://")) {
                    return Err(format!("--spec-repo {r} is not an http(s) URL"));
                }
                repos.push(r);
            }
            _ => pos.push(a.clone()),
        }
    }
    let [file, kind] = pos.as_slice() else { return Err(USAGE.into()) };
    if !CHANGE_KINDS.contains(&kind.as_str()) {
        return Err(format!("kind `{kind}` is not defect or change"));
    }
    let text = fs::read_to_string(file).map_err(|e| format!("the description {file} cannot be read: {e}"))?;
    let spec_repo = |url: &str| {
        if repos.is_empty() {
            url_parts(url).0.iter().any(|s| s.to_ascii_lowercase().contains("spec"))
        } else {
            repos.iter().any(|r| url.starts_with(r.as_str()))
        }
    };
    let under_spec = |path: &str| dirs.iter().any(|d| path.strip_prefix(d.as_str()).is_some_and(|r| r.len() > 1 && r.starts_with('/')));
    let path_re = Regex::new(r"^(?:\./)?([A-Za-z0-9_][A-Za-z0-9_./-]*?)(?::([1-9][0-9]*)(?:-[1-9][0-9]*)?)?$").unwrap();
    let anchor_re = Regex::new(r"^L[1-9][0-9]*(?:-L?[1-9][0-9]*)?$").unwrap();
    let quote = |t: &str| t.chars().take(200).collect::<String>();
    for raw in text.split_whitespace() {
        let tok = spec_token(raw);
        if tok.starts_with("https://") || tok.starts_with("http://") {
            let (segs, fragment) = url_parts(tok);
            let mr = segs.windows(2).any(|w| matches!(w[0], "merge_requests" | "pull" | "pulls") && w[1].bytes().all(|b| b.is_ascii_digit()));
            let anchored = fragment.is_some_and(|f| anchor_re.is_match(f));
            if kind == "change" && mr && spec_repo(tok) {
                return Ok((true, format!("change: a merge request of a spec repository, {}", quote(tok))));
            }
            if kind == "defect" && anchored && spec_repo(tok) {
                return Ok((true, format!("defect: a spec line, {}", quote(tok))));
            }
            continue;
        }
        let Some(c) = path_re.captures(tok) else { continue };
        let path = &c[1];
        if path.split('/').any(|s| s == "..") || !under_spec(path) {
            continue;
        }
        if kind == "change" {
            return Ok((true, format!("change: a path under the spec directory, {}", quote(tok))));
        }
        if c.get(2).is_some() {
            return Ok((true, format!("defect: a spec line, {}", quote(tok))));
        }
    }
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    if kind == "defect" && flat.contains("spec says nothing") {
        return Ok((true, "defect: the description says the spec says nothing".into()));
    }
    let dirs_named = if dirs.is_empty() {
        "under a spec directory (none is named)".to_string()
    } else {
        format!("under {}", dirs.iter().map(|d| format!("{d}/")).collect::<Vec<_>>().join(", "))
    };
    Ok((false, if kind == "change" {
        format!("change: {file} names no merge request of a spec repository and no path {dirs_named}")
    } else {
        format!("defect: {file} names no spec line (a <file>:<line> {dirs_named}, or a link into a spec repository with a line \
            anchor) and does not say the spec says nothing")
    }))
}

// --- the commands of a case (sdc.checks/1) -----------------------------------------------------
//
// docs/bugfix.md, "The commands of a case". The case's checks.yaml says how its code tree is tested:
// nothing of a language is code here. It comes, at seed time, from `--checks`, else from the
// repository's own .runbook/checks.yaml, else from the detection table, a data file shipped beside
// the bindings (examples/sdc/checks-detect.yaml; compiled in, SDC_CHECKS_DETECT names another).

const CHECKS: &str = "checks.yaml";
const CHECKS_FORMAT: &str = "sdc.checks/1";
const REPO_CHECKS: &str = ".runbook/checks.yaml";
const DETECT_FORMAT: &str = "sdc.checks-detect/1";
const DETECT_TABLE: &str = include_str!("../../examples/sdc/checks-detect.yaml");
const CHECKS_KEYS: [&str; 12] =
    ["format", "test", "repro", "gate", "test_file", "test_name", "test_def", "test_forbid", "env", "red", "green", "suite"];
const CHECKS_REQUIRED: [&str; 4] = ["test", "repro", "test_file", "format"];
const DEFAULT_TEST_NAME: &str = r"^[A-Za-z_][A-Za-z0-9_]*$";
const ENV_RE: &str = r"^[A-Za-z_][A-Za-z0-9_]*$";

/// When a run counts: its exit code is one of `exit` (absent: 0 for a pass, any but 0 for a
/// failure), its output matches `output` and does not match `refuse`. The patterns are multi-line
/// (`^` and `$` match at each line); `{test}`, `{file}`, `{dir}`, `{stem}` stand for the test.
#[derive(Clone, Debug, PartialEq)]
struct Rule {
    exit: Option<Vec<i32>>,
    output: Option<String>,
    refuse: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct Checks {
    test: Vec<String>,
    repro: Vec<String>,
    gate: Vec<(String, Vec<String>)>,
    test_file: String,
    test_name: String,
    test_def: Option<String>,
    test_forbid: Option<String>,
    env: Vec<(String, String)>,
    red: Option<Rule>,
    green: Option<Rule>,
    suite: Option<Rule>,
}

impl Checks {
    fn repro_argv(&self, r: &Repro) -> Vec<String> {
        self.repro.iter().map(|a| subst(a, r)).collect()
    }
}

fn subst(s: &str, r: &Repro) -> String {
    r.vars().iter().fold(s.to_string(), |s, (k, v)| s.replace(k, v))
}

fn subst_re(s: &str, r: &Repro) -> String {
    r.vars().iter().fold(s.to_string(), |s, (k, v)| s.replace(k, &regex::escape(v)))
}

fn multiline(p: &str) -> String {
    format!("(?m){p}")
}

/// A run under a rule (`pass`: a pass, else a failure); no rule is the exit code alone.
fn rule_holds(rule: Option<&Rule>, pass: bool, code: i32, out: &str, r: Option<&Repro>) -> bool {
    let fill = |p: &str| r.map_or(p.to_string(), |r| subst_re(p, r));
    let matches = |p: &str| Regex::new(&multiline(&fill(p))).is_ok_and(|re| re.is_match(out));
    let exit_ok = match rule.and_then(|r| r.exit.as_ref()) {
        Some(codes) => codes.contains(&code),
        None => (code == 0) == pass,
    };
    exit_ok
        && rule.and_then(|r| r.output.as_deref()).is_none_or(matches)
        && !rule.and_then(|r| r.refuse.as_deref()).is_some_and(matches)
}

fn describe(rule: Option<&Rule>) -> String {
    let Some(r) = rule else { return "none".into() };
    let mut parts = Vec::new();
    if let Some(e) = &r.exit {
        parts.push(format!("exit {}", e.iter().map(i32::to_string).collect::<Vec<_>>().join("|")));
    }
    if let Some(o) = &r.output {
        parts.push(format!("output {o}"));
    }
    if let Some(o) = &r.refuse {
        parts.push(format!("refuse {o}"));
    }
    parts.join(", ")
}

/// An argv: a non-empty list of non-empty strings.
fn argv_of(m: &Ym, k: &str, at: &str, errs: &mut Vec<String>) -> Option<Vec<String>> {
    strings(m, k, at, true, errs)
}

fn regex_in(m: &Ym, k: &str, at: &str, anchored: bool, errs: &mut Vec<String>) -> Option<String> {
    let s = string(m, k, at, errs)?;
    // `{test}` and the others stand for literal text; check the pattern with a stand-in.
    let probe = ["{file}", "{dir}", "{stem}", "{test}"].iter().fold(s.clone(), |p, v| p.replace(v, "x"));
    if let Err(e) = Regex::new(&probe) {
        errs.push(format!("{at}.{k} is not a regular expression: {}", e.to_string().lines().last().unwrap_or("")));
        return None;
    }
    if anchored && !(s.starts_with('^') && s.ends_with('$')) {
        errs.push(format!("{at}.{k} must match the whole value: begin with ^ and end with $"));
        return None;
    }
    Some(s)
}

fn rule_in(m: &Ym, k: &str, at: &str, errs: &mut Vec<String>) -> Option<Rule> {
    let at = format!("{at}.{k}");
    let r = mapping_of(m.get(k)?, &at, &["exit", "output", "refuse"], &[], CHECKS_FORMAT, errs)?;
    let exit = r.get("exit").and_then(|e| {
        let codes: Option<Vec<i32>> =
            e.as_sequence().and_then(|s| s.iter().map(|c| c.as_i64().and_then(|c| i32::try_from(c).ok())).collect());
        if codes.as_ref().is_none_or(Vec::is_empty) {
            errs.push(format!("{at}.exit is not a non-empty list of exit codes"));
        }
        codes
    });
    let output = r.get("output").and_then(|_| regex_in(r, "output", &at, false, errs));
    let refuse = r.get("refuse").and_then(|_| regex_in(r, "refuse", &at, false, errs));
    Some(Rule { exit, output, refuse })
}

/// sdc.checks/1, or every way it misses the format, one line each. `with_format`: a document
/// (`format` required); a row of the detection table carries none.
fn checks_from(v: &serde_yaml::Value, at: &str, with_format: bool) -> Result<Checks, Vec<String>> {
    let mut errs = Vec::new();
    let required: Vec<&str> = CHECKS_REQUIRED.iter().copied().filter(|k| with_format || *k != "format").collect();
    let Some(m) = mapping_of(v, at, &CHECKS_KEYS, &required, CHECKS_FORMAT, &mut errs) else { return Err(errs) };
    if let Some(f) = m.get("format") {
        if f.as_str() != Some(CHECKS_FORMAT) {
            errs.push(format!("{at}.format is not {CHECKS_FORMAT}"));
        }
    }
    let test = argv_of(m, "test", at, &mut errs);
    let repro = argv_of(m, "repro", at, &mut errs);
    let mut gate = Vec::new();
    if let Some(g) = m.get("gate") {
        match g.as_sequence() {
            None => errs.push(format!("{at}.gate is not a list")),
            Some(seq) => {
                for (i, s) in seq.iter().enumerate() {
                    let at = format!("{at}.gate[{i}]");
                    let Some(s) = mapping_of(s, &at, &["name", "argv"], &["name", "argv"], CHECKS_FORMAT, &mut errs) else { continue };
                    if let (Some(name), Some(argv)) = (string(s, "name", &at, &mut errs), argv_of(s, "argv", &at, &mut errs)) {
                        if gate.iter().any(|(n, _): &(String, Vec<String>)| *n == name) {
                            errs.push(format!("{at}.name {name} is not unique"));
                        }
                        gate.push((name, argv));
                    }
                }
            }
        }
    }
    let test_file = regex_in(m, "test_file", at, true, &mut errs);
    let test_name = match m.get("test_name") {
        Some(_) => regex_in(m, "test_name", at, true, &mut errs),
        None => Some(DEFAULT_TEST_NAME.to_string()),
    };
    let test_def = m.get("test_def").and_then(|_| regex_in(m, "test_def", at, false, &mut errs));
    let test_forbid = m.get("test_forbid").and_then(|_| regex_in(m, "test_forbid", at, false, &mut errs));
    let mut env = Vec::new();
    if let Some(e) = m.get("env") {
        match e.as_mapping() {
            None => errs.push(format!("{at}.env is not a mapping")),
            Some(e) => {
                let name = Regex::new(ENV_RE).unwrap();
                for (k, v) in e {
                    match (k.as_str(), v.as_str()) {
                        (Some(k), Some(v)) if name.is_match(k) => env.push((k.to_string(), v.to_string())),
                        _ => errs.push(format!("{at}.env: every entry is NAME: \"string\"")),
                    }
                }
            }
        }
    }
    let red = rule_in(m, "red", at, &mut errs);
    let green = rule_in(m, "green", at, &mut errs);
    let suite = rule_in(m, "suite", at, &mut errs);
    if !errs.is_empty() {
        return Err(errs);
    }
    match (test, repro, test_file, test_name) {
        (Some(test), Some(repro), Some(test_file), Some(test_name)) => {
            Ok(Checks { test, repro, gate, test_file, test_name, test_def, test_forbid, env, red, green, suite })
        }
        _ => Err(vec![format!("{at}: incomplete")]),
    }
}

fn checks_parse(doc: &str, at: &str) -> Result<Checks, Vec<String>> {
    let v: serde_yaml::Value = serde_yaml::from_str(doc).map_err(|e| vec![format!("{at} is not YAML: {e}")])?;
    checks_from(&v, at, true)
}

fn checks_yaml(ck: &Checks) -> serde_yaml::Value {
    use serde_yaml::Value as Y;
    let seq = |v: &[String]| Y::Sequence(v.iter().cloned().map(Y::String).collect());
    let rule = |r: &Rule| {
        let mut m = Ym::new();
        if let Some(e) = &r.exit {
            m.insert("exit".into(), Y::Sequence(e.iter().map(|c| Y::Number((*c).into())).collect()));
        }
        if let Some(o) = &r.output {
            m.insert("output".into(), Y::String(o.clone()));
        }
        if let Some(o) = &r.refuse {
            m.insert("refuse".into(), Y::String(o.clone()));
        }
        Y::Mapping(m)
    };
    let mut m = Ym::new();
    m.insert("format".into(), Y::String(CHECKS_FORMAT.into()));
    m.insert("test".into(), seq(&ck.test));
    m.insert("repro".into(), seq(&ck.repro));
    let gate = ck.gate.iter().map(|(n, a)| {
        let mut s = Ym::new();
        s.insert("name".into(), Y::String(n.clone()));
        s.insert("argv".into(), seq(a));
        Y::Mapping(s)
    });
    m.insert("gate".into(), Y::Sequence(gate.collect()));
    m.insert("test_file".into(), Y::String(ck.test_file.clone()));
    m.insert("test_name".into(), Y::String(ck.test_name.clone()));
    for (k, v) in [("test_def", &ck.test_def), ("test_forbid", &ck.test_forbid)] {
        if let Some(v) = v {
            m.insert(k.into(), Y::String(v.clone()));
        }
    }
    if !ck.env.is_empty() {
        m.insert("env".into(), Y::Mapping(ck.env.iter().map(|(k, v)| (Y::String(k.clone()), Y::String(v.clone()))).collect()));
    }
    for (k, r) in [("red", &ck.red), ("green", &ck.green), ("suite", &ck.suite)] {
        if let Some(r) = r {
            m.insert(k.into(), rule(r));
        }
    }
    Y::Mapping(m)
}

struct Marker {
    file: String,
    contains: Option<String>,
}

struct DetectRow {
    name: String,
    markers: Vec<Marker>,
    checks: Checks,
}

/// The detection table: SDC_CHECKS_DETECT, else the one compiled in from
/// examples/sdc/checks-detect.yaml.
fn detect_rows() -> R<Vec<DetectRow>> {
    let (doc, at) = match std::env::var("SDC_CHECKS_DETECT") {
        Ok(f) => (fs::read_to_string(&f).map_err(|e| format!("SDC_CHECKS_DETECT {f}: {e}"))?, f),
        Err(_) => (DETECT_TABLE.to_string(), "examples/sdc/checks-detect.yaml".to_string()),
    };
    detect_parse(&doc).map_err(|errs| format!("the detection table {at} is not {DETECT_FORMAT}:\n  {}", errs.join("\n  ")))
}

fn detect_parse(doc: &str) -> Result<Vec<DetectRow>, Vec<String>> {
    let v: serde_yaml::Value = serde_yaml::from_str(doc).map_err(|e| vec![format!("not YAML: {e}")])?;
    let mut errs = Vec::new();
    let Some(m) = mapping_of(&v, "table", &["format", "languages"], &["format", "languages"], DETECT_FORMAT, &mut errs) else {
        return Err(errs);
    };
    if m.get("format").and_then(|f| f.as_str()) != Some(DETECT_FORMAT) {
        errs.push(format!("table.format is not {DETECT_FORMAT}"));
    }
    let mut rows = Vec::new();
    for (i, row) in m.get("languages").and_then(|l| l.as_sequence()).into_iter().flatten().enumerate() {
        let at = format!("table.languages[{i}]");
        let Some(r) = mapping_of(row, &at, &["name", "markers", "checks"], &["name", "markers", "checks"], DETECT_FORMAT, &mut errs)
        else {
            continue;
        };
        let name = string(r, "name", &at, &mut errs);
        let mut markers = Vec::new();
        for (j, mk) in r.get("markers").and_then(|l| l.as_sequence()).into_iter().flatten().enumerate() {
            let at = format!("{at}.markers[{j}]");
            let Some(mk) = mapping_of(mk, &at, &["file", "contains"], &["file"], DETECT_FORMAT, &mut errs) else { continue };
            if let Some(file) = string(mk, "file", &at, &mut errs) {
                let contains = mk.get("contains").and_then(|_| regex_in(mk, "contains", &at, false, &mut errs));
                markers.push(Marker { file, contains });
            }
        }
        if markers.is_empty() {
            errs.push(format!("{at}.markers is not a non-empty list"));
        }
        match (name, checks_from(&r["checks"], &format!("{at}.checks"), false)) {
            (Some(name), Ok(checks)) => rows.push(DetectRow { name, markers, checks }),
            (_, Err(e)) => errs.extend(e),
            _ => {}
        }
    }
    if rows.is_empty() && errs.is_empty() {
        errs.push("table.languages is empty".into());
    }
    if errs.is_empty() {
        Ok(rows)
    } else {
        Err(errs)
    }
}

/// The case's commands for the code tree `tree`, and where they came from: `arg` (`--checks`), else
/// the tree's .runbook/checks.yaml, else the first row of the detection table one of whose markers
/// is at the tree's root. `tracked`, when given, is the list of files a marker must be in (a seed
/// from a git worktree takes only what git tracks). Nothing found: an error that names `--checks`.
fn resolve_checks(tree: &Path, arg: Option<&Path>, tracked: Option<&[String]>) -> R<(Checks, String)> {
    let has = |f: &str| tree.join(f).is_file() && tracked.is_none_or(|t| t.iter().any(|x| x == f));
    let parse = |doc: &str, at: &str| checks_parse(doc, at).map_err(|e| format!("{at} is not {CHECKS_FORMAT}:\n  {}", e.join("\n  ")));
    if let Some(f) = arg {
        let doc = fs::read_to_string(f).map_err(|e| format!("--checks {}: {e}", f.display()))?;
        return Ok((parse(&doc, &f.display().to_string())?, "--checks".into()));
    }
    if has(REPO_CHECKS) {
        let doc = fs::read_to_string(tree.join(REPO_CHECKS)).map_err(|e| format!("{REPO_CHECKS}: {e}"))?;
        return Ok((parse(&doc, REPO_CHECKS)?, REPO_CHECKS.into()));
    }
    let rows = detect_rows()?;
    for row in &rows {
        for mk in &row.markers {
            let hit = has(&mk.file)
                && mk.contains.as_ref().is_none_or(|c| {
                    let text = fs::read_to_string(tree.join(&mk.file)).unwrap_or_default();
                    Regex::new(&multiline(c)).is_ok_and(|re| re.is_match(&text))
                });
            if hit {
                return Ok((row.checks.clone(), format!("detected {} ({})", row.name, mk.file)));
            }
        }
    }
    let mut markers: Vec<&str> = rows.iter().flat_map(|r| r.markers.iter().map(|m| m.file.as_str())).collect();
    markers.dedup();
    Err(format!("{}: no commands to test it with: no --checks <file>, no {REPO_CHECKS} in it, and none of the detection \
        table's markers at its root ({}). Pass the commands with --checks <file> ({CHECKS_FORMAT}, docs/bugfix.md); the \
        seed does not guess", tree.display(), markers.join(", ")))
}

/// The case's commands: work/checks.yaml, written by the seed. A case without one (a parent that
/// handed in its work/ before checks.yaml existed) takes what the seed would have found for the
/// base: the base's .runbook/checks.yaml, else the detection table.
fn load_checks() -> R<(Checks, String)> {
    match fs::read_to_string(CHECKS) {
        Ok(doc) => checks_parse(&doc, CHECKS)
            .map(|c| (c, CHECKS.to_string()))
            .map_err(|e| format!("{CHECKS} is not {CHECKS_FORMAT}:\n  {}", e.join("\n  "))),
        Err(_) => resolve_checks(&base(), None, None).map_err(|e| format!("{CHECKS}: none in the case, and {e}")),
    }
}

fn show_checks() -> R<u8> {
    let (ck, _) = load_checks()?;
    print!("{}", serde_yaml::to_string(&checks_yaml(&ck)).unwrap());
    Ok(0)
}

fn write_checks(ck: &Checks) -> R<()> {
    fs::write(CHECKS, serde_yaml::to_string(&checks_yaml(ck)).unwrap()).map_err(|e| format!("{CHECKS}: {e}"))
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
    let mut checks = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--from" => from = it.next().cloned(),
            "--text" => text = it.next().cloned(),
            "--story" => story = it.next().cloned(),
            "--ticket" => ticket = it.next().cloned(),
            "--repo" => repo = it.next().cloned(),
            "--checks" => checks = Some(PathBuf::from(it.next().ok_or("seed: --checks <file>")?)),
            _ => return Err(format!("seed: unknown argument {a}")),
        }
    }
    if let Some(t) = ticket {
        if from.is_some() || text.is_some() || story.is_some() {
            return Err("seed --ticket <file> [--repo <worktree>]: --from, --text and --story go with the other form".into());
        }
        return seed_ticket(Path::new(&t), repo.as_deref().map(Path::new), checks.as_deref());
    }
    if repo.is_some() {
        return Err("seed: --repo goes with --ticket <file>".into());
    }
    let from = PathBuf::from(from.ok_or("seed --from <code tree or fixture dir>")?);
    if !from.is_dir() {
        return Err(format!("{} is not a directory", from.display()));
    }
    // A fixture directory holds its code tree in crate/ or repo/, beside bug.yaml and its stubs.
    let tree = if from.join("Cargo.toml").is_file() {
        from.clone()
    } else {
        ["crate", "repo"].iter().map(|d| from.join(d)).find(|d| d.is_dir()).unwrap_or_else(|| from.clone())
    };
    let (ck, source) = resolve_checks(&tree, checks.as_deref(), None)?;
    let mut rep: serde_yaml::Value = match &text {
        Some(t) => serde_yaml::to_value(serde_json::json!({"text": t})).unwrap(),
        None => {
            let f = from.join("bug.yaml");
            let t = fs::read_to_string(&f).map_err(|_| format!("no report text and no {}", f.display()))?;
            serde_yaml::from_str(&t).map_err(|e| format!("{}: {e}", f.display()))?
        }
    };
    for d in ["mainline", "impl"] {
        reset_dir(Path::new(d))?;
        copy_tree(&tree, Path::new(d))?;
    }
    write_checks(&ck)?;
    if let Some(s) = story {
        rep["story"] = serde_yaml::Value::String(s);
    } else if rep.get("story").is_none() {
        rep["story"] = serde_yaml::Value::String("story:bug".into());
    }
    fs::write("bug.yaml", serde_yaml::to_string(&rep).unwrap()).map_err(|e| e.to_string())?;
    println!("seed: mainline/ and impl/ from {}, checks.yaml from {source}, bug.yaml", tree.display());
    std::env::set_var("SDC_ACTION", "bugfix.seed");
    story_to(&rep, "active")
}

// --- the ticket (sdc.ticket/1) -------------------------------------------------------------------
//
// What a controller outside writes for one ticket of any system (docs/bugfix.md, "From a ticket").
// Its text reaches the prompts (bug.yaml) and the run's own records only: no error or log line
// here quotes a value of the ticket but its id.

const TICKET_FORMAT: &str = "sdc.ticket/1";
const TICKET_KEYS: [&str; 12] =
    ["format", "id", "kind", "change_kind", "title", "text", "context", "source", "repository", "check", "constraints", "branch"];
const TICKET_REQUIRED: [&str; 6] = ["format", "id", "kind", "title", "text", "repository"];
const CONTEXT_KINDS: [&str; 3] = ["comment", "linked", "doc"];
const CHANGE_KINDS: [&str; 2] = ["defect", "change"];
const COMMIT_RE: &str = r"^([0-9a-f]{40}|[0-9a-f]{64})$";
const SYSTEM_RE: &str = r"^[A-Za-z0-9][A-Za-z0-9._-]*$";
const INSTANT_RE: &str = r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$";
const BRANCH_RE: &str = r"^[A-Za-z0-9][A-Za-z0-9._/-]*$";

/// The size cap of a repository seeded from a ticket. Every model call copies the code tree into
/// its scratch directory and back (`sdc-node`, `input impl`, `seed impl`), so a large tree
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
    /// `defect` or `change`, default `defect`: the kind of the spec reference the hand-back check asks for.
    change_kind: String,
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
            "change_kind": {"enum": CHANGE_KINDS, "default": "defect",
                "description": "what the fix is against the spec: a defect (the spec line it restores, or the spec says nothing) or a change (a spec merge request first); the hand-back check reads it"},
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

/// A mapping of sdc.ticket/1 with only `allowed` keys and every `required` one; each miss one line
/// in `errs`.
fn mapping<'a>(v: &'a serde_yaml::Value, at: &str, allowed: &[&str], required: &[&str], errs: &mut Vec<String>) -> Option<&'a Ym> {
    mapping_of(v, at, allowed, required, TICKET_FORMAT, errs)
}

/// A mapping of the format `fmt` with only `allowed` keys and every `required` one.
fn mapping_of<'a>(v: &'a serde_yaml::Value, at: &str, allowed: &[&str], required: &[&str], fmt: &str, errs: &mut Vec<String>) -> Option<&'a Ym> {
    let Some(m) = v.as_mapping() else {
        errs.push(format!("{at} is not a mapping"));
        return None;
    };
    for k in m.keys() {
        match k.as_str() {
            Some(k) if allowed.contains(&k) => {}
            Some(k) => errs.push(format!("{at}: key `{k}` is not in {fmt} (keys: {})", allowed.join(", "))),
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
    let change_kind = match m.get("change_kind") {
        None => Some("defect".to_string()),
        Some(_) => string(m, "change_kind", at, &mut errs).filter(|k| {
            let ok = CHANGE_KINDS.contains(&k.as_str());
            if !ok {
                errs.push("ticket.change_kind is not defect or change".into());
            }
            ok
        }),
    };
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
    match (id, kind, change_kind, title, text, path, base_ref, base_commit) {
        (Some(id), Some(kind), Some(change_kind), Some(title), Some(text), Some(path), Some(base_ref), Some(base_commit)) => Ok(Ticket {
            id, kind, change_kind, title, text, context, source, path, remote, base_ref, base_commit, check, constraints, branch,
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
    m.insert("change_kind".into(), Y::String(t.change_kind.clone()));
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
/// top of a git worktree, no change to a tracked file, HEAD at `repository.base.commit`, no
/// submodule or symlink, within the size cap.
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

fn seed_ticket(file: &Path, repo_arg: Option<&Path>, checks_arg: Option<&Path>) -> R<u8> {
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
    let (ck, source) = resolve_checks(&repo, checks_arg, Some(&files))?;
    let rep = ticket_report(&t)?;
    for d in ["mainline", "impl"] {
        copy_listed(&repo, &files, Path::new(d))?;
    }
    write_checks(&ck)?;
    fs::write("bug.yaml", serde_yaml::to_string(&rep).unwrap()).map_err(|e| e.to_string())?;
    let record = json!({"format": "sdc.bugfix-seed/1", "ticket": {"id": t.id, "kind": t.kind, "branch": t.branch},
        "repository": {"path": fs::canonicalize(&repo).map_err(|e| e.to_string())?, "remote": t.remote,
            "base": {"ref": t.base_ref, "commit": t.base_commit}},
        "files": files.len(), "bytes": bytes, "checks": source});
    fs::create_dir_all(".sdc").map_err(|e| e.to_string())?;
    fs::write(".sdc/seed.json", serde_json::to_string_pretty(&record).unwrap() + "\n").map_err(|e| e.to_string())?;
    println!("seed: mainline/ and impl/ from {} at {} ({} files, {bytes} bytes, as git ls-files lists them), \
        checks.yaml from {source}, bug.yaml from the ticket, .sdc/seed.json", repo.display(), t.base_commit, files.len());
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

/// `from` copied to .sdc/bugfix/<name>, the directory a command runs in, made a git tree.
fn copy_to(from: &Path, name: &str) -> R<PathBuf> {
    let tree = Path::new(".sdc/bugfix").join(name);
    reset_dir(&tree)?;
    copy_tree(from, &tree)?;
    git_tree(&tree)?;
    fs::canonicalize(&tree).map_err(|e| e.to_string())
}

/// The identity and instant of the one commit of a check tree: fixed, so its id depends on the
/// copied files only.
const TREE_GIT_IDENTITY: [&str; 4] = ["-c", "user.name=runbook", "-c", "user.email=runbook@localhost"];
const TREE_GIT_DATE: &str = "2000-01-01T00:00:00+0000";

/// `tree` made a git repository with one commit of its files (docs/bugfix.md, "The trees a check
/// runs in"): the build pool's remote mode needs one. No remote, no history; maintenance off and no
/// hook in its own configuration. Nothing of the host's git configuration is read: no global or
/// system file, and no `GIT_*` variable of the environment reaches the git calls.
fn git_tree(tree: &Path) -> R<()> {
    let git = |args: &[&str]| -> R<()> {
        let mut c = Command::new("git");
        for (k, _) in std::env::vars_os().filter(|(k, _)| k.to_string_lossy().starts_with("GIT_")) {
            c.env_remove(k);
        }
        let o = c
            .args(["-c", "init.defaultBranch=main", "-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null"])
            .args(args)
            .current_dir(tree)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_DATE", TREE_GIT_DATE)
            .env("GIT_COMMITTER_DATE", TREE_GIT_DATE)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("git is not on PATH: a check runs in a git tree (docs/bugfix.md): {e}"))?;
        if o.status.success() {
            Ok(())
        } else {
            Err(format!("git {} in {}: {}", args.join(" "), tree.display(), String::from_utf8_lossy(&o.stderr).trim()))
        }
    };
    git(&["init", "-q"])?;
    for (k, v) in [("maintenance.auto", "false"), ("gc.auto", "0"), ("core.hooksPath", "/dev/null")] {
        git(&["config", "--local", k, v])?;
    }
    // Every copied file, also one a .gitignore of the tree names.
    git(&["add", "--all", "--force", "."])?;
    let mut commit = TREE_GIT_IDENTITY.to_vec();
    commit.extend(["commit", "-q", "--no-verify", "--allow-empty", "-m", "the code tree as copied for a check"]);
    git(&commit)
}

fn copy_of_impl() -> R<PathBuf> {
    copy_to(Path::new("impl"), "tree")
}

/// The unchanged code without the reproducing test (bug.yaml `base`, else mainline/).
fn copy_of_base() -> R<PathBuf> {
    copy_to(&base(), "base")
}

fn on_path(p: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|ps| std::env::split_paths(&ps).any(|d| d.join(p).is_file()))
}

/// `argv` in `dir`, as an argv (never through a shell), through `slot` when it is on PATH, with
/// checks.yaml's `env` and cargo's target directory beside the copy; stdout and stderr into one log
/// file. A program with a `/` is a path in `dir` (or absolute); one without is looked up on PATH.
///
/// The build pool (docs/driver.md, "The build pool wait"): the wait for a slot is bounded
/// (`pool_bound`), a job still queued at the bound is cancelled and the call is an `Err` that
/// starts with POOL_WAIT (exit 69, nothing ran), and a job the pool ended at its run limit (exit
/// 124) is an `Err` that starts with POOL_OVER (exit 70). Neither is a result about the code.
fn run(dir: &Path, argv: &[String], ck: &Checks) -> R<(i32, String)> {
    let prog = argv.first().ok_or("an empty argv")?;
    let path = if prog.contains('/') {
        let p = dir.join(prog);
        if !p.is_file() {
            return Err(format!("{prog} is not on PATH, nor a file of the code tree"));
        }
        p.into_os_string().into_string().map_err(|_| format!("{prog}: not a UTF-8 path"))?
    } else {
        if !on_path(prog) {
            return Err(format!("{prog} is not on PATH"));
        }
        prog.clone()
    };
    let pooled = on_path("slot");
    let mut c = if pooled {
        let mut c = Command::new("slot");
        c.arg(&path);
        c
    } else {
        Command::new(&path)
    };
    let log = dir.parent().unwrap().join("run.log");
    let f = fs::File::create(&log).map_err(|e| e.to_string())?;
    let target = dir.parent().unwrap().join("target");
    c.args(&argv[1..])
        .current_dir(dir)
        .envs(ck.env.iter().map(|(k, v)| (k, v)))
        .env("CARGO_TARGET_DIR", &target)
        .stdin(Stdio::null())
        .stdout(f.try_clone().map_err(|e| e.to_string())?);
    let bound = if pooled { pool_bound()? } else { None };
    let code = match bound {
        None => {
            let st = c.stderr(f).status().map_err(|e| format!("{prog} could not start: {e}"))?;
            let code = st.code().unwrap_or(1);
            if pooled && code == POOL_KILL {
                return Err(format!("{POOL_OVER}the job ran over its time: the pool ended it at its run limit; the command did not finish"));
            }
            code
        }
        Some(secs) => {
            let errf = dir.parent().unwrap().join("run.err");
            let job = pool_job(&mut c, &errf, secs)?;
            let mut note = String::new();
            let code = job.code;
            if code == 130 && job.cancelled {
                if job.started {
                    note = format!("sdc-bugfix: pool: job {} was cancelled by the pool while it ran (over its run limit, 10 minutes unless the command names a longer one, or by `slot cancel`); the command did not finish\n", job.id.as_deref().unwrap_or("?"));
                } else {
                    return Err(format!("{POOL_WAIT}job {} was cancelled while it was still queued; nothing ran", job.id.as_deref().unwrap_or("?")));
                }
            }
            if code == POOL_KILL {
                return Err(format!("{POOL_OVER}job {} ran over its time: the pool ended it at its run limit; the command did not finish", job.id.as_deref().unwrap_or("?")));
            }
            // slot's own lines come after the command's output, as the shell tool prints them.
            let mut l = fs::OpenOptions::new().append(true).open(&log).map_err(|e| e.to_string())?;
            std::io::Write::write_all(&mut l, job.err.as_bytes()).map_err(|e| e.to_string())?;
            std::io::Write::write_all(&mut l, note.as_bytes()).map_err(|e| e.to_string())?;
            code
        }
    };
    let out = fs::read_to_string(&log).unwrap_or_default();
    // Through the pool, 127 and 126 are the shell's "not found" and "cannot run": the pool, or what
    // it ran, could not run the command, so the exit says nothing about the code.
    if pooled && (code == 126 || code == 127) {
        let last = out.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("");
        return Err(format!("{NOT_RUN}exit {code}: `{}` did not run through slot: {last}", argv.join(" ")));
    }
    Ok((code, out))
}

/// The start of the error of a pool job that exited 126 or 127: `not run: exit <code>: ...`; the
/// step exits with that code, a program error.
const NOT_RUN: &str = "not run: ";

/// `slot` exits so when the pool ended a started job at its run limit (slot 0.22).
const POOL_KILL: i32 = 124;
/// The exits of a step whose pool job did not start within the wait bound (nothing ran) and of one
/// the pool ended at its run limit, as `sdc-tool.sh` (`pool_run`) and the driver (`program_error`)
/// know them; the messages that carry them out of `run`.
const POOL_WAIT_EXIT: u8 = 69;
const OVER_TIME_EXIT: u8 = 70;
const POOL_WAIT: &str = "pool-wait: ";
const POOL_OVER: &str = "pool: ";

/// The step's wait bound in seconds: `SDC_POOL_WAIT`, else `--pool-wait <s>` (main), else none.
fn pool_bound() -> R<Option<u64>> {
    match std::env::var("SDC_POOL_WAIT") {
        Ok(v) if !v.is_empty() => v.parse().map(Some).map_err(|_| format!("{BAD_BOUND}: SDC_POOL_WAIT={v}")),
        _ => Ok(POOL_WAIT_ARG.get().copied()),
    }
}

const BAD_BOUND: &str = "--pool-wait takes whole seconds";
static POOL_WAIT_ARG: OnceLock<u64> = OnceLock::new();
/// Milliseconds the pool commands of this step waited so far: one bound per step, and this
/// process is the step.
static POOL_SPENT_MS: AtomicU64 = AtomicU64::new(0);

struct Job {
    code: i32,
    id: Option<String>,
    started: bool,
    cancelled: bool,
    err: String,
}

/// One pool job (`c` is `slot <command>`) with the wait for its slot bounded, as `pool_run` of
/// bindings/tools/sdc-tool.sh: `slot` runs with its stderr in `errf`; the job's id is on slot's own
/// `slot: job <id> queued ...` line, printed only for a job that has to wait, and `slot status`
/// shows the job as `queued` until it starts. A job still queued when the step's bound (`secs`, less
/// what its earlier jobs waited) is spent is cancelled and the result is an `Err` that starts with
/// POOL_WAIT. A start is never inferred from a missing line (on a loaded machine the line comes
/// seconds late): the line is read for as long as the process lives, and a job that ends without
/// one was admitted at once and waited nothing. The pool is looked at after 0.2 s, then at doubling
/// pauses up to SDC_POOL_POLL seconds (default 2).
fn pool_job(c: &mut Command, errf: &Path, secs: u64) -> R<Job> {
    let poll: f64 = std::env::var("SDC_POOL_POLL").ok().and_then(|v| v.parse().ok()).unwrap_or(2.0);
    let spent = POOL_SPENT_MS.load(Ordering::Relaxed);
    let left = (secs * 1000).saturating_sub(spent);
    let queued = Regex::new(r"(?m)^slot: job ([0-9]+) queued").unwrap();
    let cancelled = Regex::new(r"(?m)^slot: job [0-9]+ cancelled").unwrap();
    let ef = fs::File::create(errf).map_err(|e| e.to_string())?;
    let t0 = Instant::now();
    let mut child = c.stderr(ef).spawn().map_err(|e| format!("slot could not start: {e}"))?;
    let (mut id, mut started, mut pause) = (None::<String>, false, 0.2f64);
    loop {
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            break;
        }
        let elapsed = t0.elapsed().as_millis() as u64;
        if id.is_none() {
            let err = fs::read_to_string(errf).unwrap_or_default();
            id = queued.captures(&err).map(|m| m[1].to_string());
        }
        if let Some(i) = &id {
            if pool_state(i).as_deref() != Some("queued") {
                started = true;
            } else if elapsed >= left {
                let _ = Command::new("slot").args(["cancel", i]).stdout(Stdio::null()).stderr(Stdio::null()).status();
                let _ = child.kill();
                let _ = child.wait();
                POOL_SPENT_MS.store(spent + elapsed, Ordering::Relaxed);
                return Err(format!("{POOL_WAIT}job {i} was still queued and this step has waited {} s of its wait bound of {secs} s; cancelled, nothing ran",
                    (spent + elapsed) / 1000));
            }
        }
        if started {
            POOL_SPENT_MS.store(spent + elapsed, Ordering::Relaxed);
            break;
        }
        std::thread::sleep(Duration::from_secs_f64(pause));
        pause = (pause * 2.0).min(poll);
    }
    let st = child.wait().map_err(|e| e.to_string())?;
    let err = fs::read_to_string(errf).unwrap_or_default();
    if !started {
        if id.is_none() {
            id = queued.captures(&err).map(|m| m[1].to_string());
        }
        if id.is_some() {
            // It queued and ended between two looks, so it ran less than one pause: what it took was wait.
            POOL_SPENT_MS.store(spent + t0.elapsed().as_millis() as u64, Ordering::Relaxed);
        } else {
            // The process ended and never printed a queued line: the pool admitted the job at
            // once and it waited nothing.
            started = true;
            POOL_SPENT_MS.store(spent, Ordering::Relaxed);
        }
    }
    Ok(Job { code: st.code().unwrap_or(1), id, started, cancelled: cancelled.is_match(&err), err })
}

/// The state column of job `id` in `slot status` (`queued`, `running`, ...), none when it is not listed.
fn pool_state(id: &str) -> Option<String> {
    let o = Command::new("slot").arg("status").stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    let want = format!("#{id}");
    String::from_utf8_lossy(&o.stdout).lines().find_map(|l| {
        let mut w = l.split_whitespace();
        (w.next() == Some(want.as_str())).then(|| w.next().unwrap_or("").to_string())
    })
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

    fn rust() -> Checks {
        detect_rows().unwrap().into_iter().find(|r| r.name == "rust").unwrap().checks
    }

    fn pager_repro() -> Repro {
        Repro { file: "tests/last_page.rs".into(), test: "last_partial_page_is_shown".into() }
    }

    #[test]
    fn parse_attributes_tests_to_their_target() {
        let ck = rust();
        let r = pager_repro();
        assert!(rule_holds(ck.red.as_ref(), false, 101, OUT, Some(&r)), "the reproducing test's own FAILED line");
        let other = Repro { test: "full_pages".into(), ..pager_repro() };
        assert!(!rule_holds(ck.red.as_ref(), false, 101, OUT, Some(&other)), "full_pages is ok");
        assert!(!rule_holds(ck.green.as_ref(), true, 101, OUT, Some(&r)));
        assert!(!rule_holds(ck.suite.as_ref(), true, 101, OUT, None));
        let green = OUT.replace("shown ... FAILED", "shown ... ok");
        assert!(rule_holds(ck.green.as_ref(), true, 0, &green, Some(&r)));
        assert!(!rule_holds(ck.suite.as_ref(), true, 0, &green.replace("no_items ... ok", "no_items ... ignored"), None));
        assert_eq!(ck.repro_argv(&r).join(" "), "cargo test --offline --test last_page -- --exact last_partial_page_is_shown");
    }

    #[test]
    fn parse_finds_a_compile_error() {
        let out = "   Compiling pager v0.1.0\nerror[E0425]: cannot find function `nope`\nerror: could not compile `pager`\n";
        assert!(!rule_holds(rust().red.as_ref(), false, 101, out, Some(&pager_repro())), "a compile error is not red");
        let none = "running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out\n";
        assert!(!rule_holds(rust().green.as_ref(), true, 0, none, Some(&pager_repro())), "no such test is not green");
    }

    #[test]
    fn the_detection_table_is_valid_and_rust_keeps_todays_commands() {
        let rows = detect_rows().unwrap();
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names[0], "rust");
        for want in ["go", "javascript-node-test", "javascript-npm", "python-pytest", "java-maven", "java-gradle", "make"] {
            assert!(names.contains(&want), "{want} in {names:?}");
        }
        let ck = rust();
        assert_eq!(ck.test, ["cargo", "test", "--offline", "--no-fail-fast"]);
        let gate: Vec<String> = ck.gate.iter().map(|(n, a)| format!("{n}: {}", a.join(" "))).collect();
        assert_eq!(gate, ["build: cargo build --offline", "test: cargo test --offline",
            "clippy: cargo clippy --offline --all-targets -- -D warnings", "fmt: cargo fmt --check"]);
        assert_eq!((ck.test_file.as_str(), ck.test_name.as_str()), (r"^tests/[a-z][a-z0-9_]*\.rs$", "^[a-z_][a-z0-9_]*$"));
        // checks.yaml as written reads back the same.
        let doc = serde_yaml::to_string(&checks_yaml(&ck)).unwrap();
        assert_eq!(checks_parse(&doc, "checks.yaml").unwrap(), ck);
    }

    #[test]
    fn every_refusal_of_the_checks_format_is_named() {
        let ok = "format: sdc.checks/1\ntest: [./t]\nrepro: [./t, \"{file}\"]\ntest_file: '^tests/[a-z]+$'\n";
        assert!(checks_parse(ok, "c").is_ok());
        let cases: Vec<(String, &str)> = vec![
            (format!("{ok}shell: sh\n"), "c: key `shell` is not in sdc.checks/1"),
            (ok.replace("format: sdc.checks/1\n", ""), "c: required key missing: format"),
            (ok.replace("sdc.checks/1", "sdc.checks/2"), "c.format is not sdc.checks/1"),
            (ok.replace("repro: [./t, \"{file}\"]", "repro: []"), "c.repro is empty"),
            (ok.replace("'^tests/[a-z]+$'", "'tests/'"), "c.test_file must match the whole value"),
            (ok.replace("'^tests/[a-z]+$'", "'^tests/[a-z+$'"), "c.test_file is not a regular expression"),
            (format!("{ok}gate:\n- name: a\n  argv: [x]\n- name: a\n  argv: [y]\n"), "c.gate[1].name a is not unique"),
            (format!("{ok}gate:\n- name: a\n"), "c.gate[0]: required key missing: argv"),
            (format!("{ok}red: {{exit: []}}\n"), "c.red.exit is not a non-empty list"),
            (format!("{ok}env: {{A: 1}}\n"), "c.env: every entry is NAME"),
        ];
        for (doc, want) in cases {
            let e = checks_parse(&doc, "c").err().unwrap_or_default();
            assert!(e.iter().any(|e| e.contains(want)), "want `{want}`, got {e:?}");
        }
    }

    #[test]
    fn a_tree_with_no_marker_and_no_checks_is_refused_naming_the_flag() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("target/sdc-bugfix-nomarker-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("main.c"), "int main(void) { return 0; }\n").unwrap();
        let e = resolve_checks(&dir, None, None).unwrap_err();
        assert!(e.contains("--checks <file>") && e.contains("Cargo.toml, go.mod, package.json") && e.contains("does not guess"), "{e}");
        // A marker git does not track does not count.
        fs::write(dir.join("Cargo.toml"), "").unwrap();
        assert!(resolve_checks(&dir, None, Some(&["main.c".to_string()])).is_err());
        assert_eq!(resolve_checks(&dir, None, None).unwrap().1, "detected rust (Cargo.toml)");
        fs::remove_dir_all(&dir).unwrap();
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
        assert_eq!(repro_schema(&rust())["required"], json!(["file", "test"]));
        assert_eq!(repro_schema(&rust())["properties"]["file"]["pattern"], r"^tests/[a-z][a-z0-9_]*\.rs$");
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
    fn change_kind_is_defect_or_change_and_defect_by_default_and_reaches_bug_yaml() {
        let t = ticket_parse(&ticket_doc(COMMIT)).unwrap();
        assert_eq!(t.change_kind, "defect");
        assert_eq!(ticket_report(&t).unwrap()["change_kind"].as_str(), Some("defect"));
        let t = ticket_parse(&ticket_doc(COMMIT).replace("kind: bug\n", "kind: bug\nchange_kind: change\n")).unwrap();
        assert_eq!(t.change_kind, "change");
        assert_eq!(ticket_report(&t).unwrap()["change_kind"].as_str(), Some("change"));
        let e = errs_of(&ticket_doc(COMMIT).replace("kind: bug\n", "kind: bug\nchange_kind: secret-kind\n"));
        assert!(e.iter().any(|e| e == "ticket.change_kind is not defect or change"), "{e:?}");
        assert!(!e.join("\n").contains("secret"), "{e:?}");
        let s = ticket_schema();
        assert_eq!(s["properties"]["change_kind"]["enum"], json!(["defect", "change"]));
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
    fn a_repository_without_cargo_toml_is_taken_and_its_commands_are_looked_up() {
        let (dir, head) = fixture_repo("nocargo", false);
        let t = ticket_parse(&ticket_doc(&head)).unwrap();
        let (files, _) = repo_files(&dir, &t, &CAPS).unwrap();
        let e = resolve_checks(&dir, None, Some(&files)).unwrap_err();
        assert!(e.contains("--checks <file>"), "{e}");
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
