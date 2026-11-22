//! Slow lane `task test:decider-parity`: the Rust decider writes the bytes `bindings/tools/decider.sh`
//! writes. Each case of `parity_cases.rs` (a kind, a request type, an adapter: `dry`, `stub`, or a
//! scripted adapter that writes a fixed `out/decision.json`) runs once through the script and once
//! through [`NativeDecider`]; the reply, the standard error and every file of the call directory are
//! compared byte for byte after the times (`status.json`) and the two directories' names are masked.
//! The decisions.jsonl line and the cost the runner derives from a reply are compared too.
//!
//! This lane and its fixtures are deleted with `decider.sh` (then the decider has one implementation).
//! It needs jq, yq, rsync and the built driver (`driver/target/debug/sdc-reply`), makes no model
//! call, and is `#[ignore]`d in the gate.

use super::parity_cases::{cases, write_fixture, Case, Kind, CASE};
use super::*;
use crate::assets::testdir::Dir;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

const BUDGET_MICRO: u64 = 250_000;
const MODEL: &str = "claude-opus-5-5";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn driver_bin() -> PathBuf {
    repo().join("driver/target/debug")
}

fn host_path() -> String {
    format!("{}:{}", driver_bin().display(), std::env::var("PATH").unwrap_or_default())
}

/// The scripted adapter of the case at `file`: writes the model's files and prints the response.
fn write_adapter(file: &Path, dir: &Path, c: &Case) {
    use std::os::unix::fs::PermissionsExt;
    let mut text = String::from("#!/bin/sh\nd=$(dirname \"$1\")\n");
    for (name, content, target) in [("decision.json", &c.decision, "$d/cwd/out/decision.json"), ("structured.json", &c.structured, "$d/structured-output.json")] {
        if let Some(content) = content {
            std::fs::write(dir.join(name), content).unwrap();
            text.push_str(&format!("cp '{}' \"{target}\"\n", dir.join(name).display()));
        }
    }
    text.push_str(&format!("printf '%s\\n' '{}'\n{}", c.response, c.tail));
    std::fs::write(file, text).unwrap();
    std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// What each side runs in: `<dir>/script` and `<dir>/rust`, each with a call directory `rec`.
fn side(dir: &Path, name: &str) -> PathBuf {
    let side = dir.join(name);
    std::fs::create_dir_all(side.join("rec")).unwrap();
    side
}

fn text_of(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// A text with the two sides' directories masked.
fn mask(text: &str, dir: &Path) -> String {
    text.replace(&dir.join("script").display().to_string(), "@S").replace(&dir.join("rust").display().to_string(), "@S")
}

/// `status.json` without the times and the process id, which differ by nature.
fn mask_times(text: &str) -> String {
    text.lines()
        .map(|l| match l.trim_start().split_once(':') {
            Some((key, _)) if ["\"pid\"", "\"started_at\"", "\"finished_at\""].contains(&key) => format!("{key}: @"),
            _ => l.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn walk(base: &Path, at: &Path, out: &mut BTreeMap<String, String>) {
    let mut entries: Vec<_> = std::fs::read_dir(at).unwrap().filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let path = e.path();
        if path.is_dir() {
            walk(base, &path, out);
        } else {
            out.insert(path.strip_prefix(base).unwrap().display().to_string(), text_of(&std::fs::read(&path).unwrap()));
        }
    }
}

struct Outcome {
    reply: String,
    stderr: String,
    files: BTreeMap<String, String>,
}

fn request_body(c: &Case, dir: &Path, rec: &Path) -> Value {
    let mut body = c.body.clone();
    body["case_dir"] = json!(dir.join(CASE).display().to_string());
    body["record_dir"] = json!(rec.display().to_string());
    body
}

fn adapter_arg(c: &Case, dir: &Path) -> Option<String> {
    match &c.kind {
        Kind::Model { adapter: "scripted", .. } => Some(dir.join(c.adapter_file).display().to_string()),
        Kind::Model { adapter, .. } => Some((*adapter).to_owned()),
        Kind::Rule(_) => None,
    }
}

fn run_script(c: &Case, dir: &Path) -> Result<Outcome, String> {
    let side = side(dir, "script");
    let rec = side.join("rec");
    let request = side.join("request-in.json");
    std::fs::write(&request, request_body(c, dir, &rec).to_string()).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(side.join("home")).unwrap();
    let (kind, final_escalate) = match &c.kind {
        Kind::Rule(v) => (*v, false),
        Kind::Model { final_escalate, .. } => ("model", *final_escalate),
    };
    let mut command = Command::new(repo().join("bindings/tools/decider.sh"));
    command.arg(kind).arg(&request).env_clear().current_dir(&side);
    command.env("PATH", host_path()).env("HOME", side.join("home")).env("SDC_REPLY", driver_bin().join("sdc-reply"));
    if let Some(adapter) = adapter_arg(c, dir) {
        command.env("SDC_DECIDER_ADAPTER", adapter).env("SDC_LLM_MODEL_DECIDER", MODEL);
        command.env("SDC_DECIDER_ESCALATE", if final_escalate { "final" } else { "stop" });
        command.env("SDC_DECIDER_MAX_BUDGET_USD", super::approval::usd_text(BUDGET_MICRO));
    }
    let out = command.output().map_err(|e| format!("decider.sh: {e}"))?;
    if !out.status.success() {
        return Err(format!("decider.sh exited {:?}: {}", out.status.code(), text_of(&out.stderr)));
    }
    let mut files = BTreeMap::new();
    walk(&rec, &rec, &mut files);
    Ok(Outcome { reply: text_of(&out.stdout), stderr: text_of(&out.stderr), files })
}

fn run_rust(c: &Case, dir: &Path) -> Result<Outcome, String> {
    let side = side(dir, "rust");
    let rec = side.join("rec");
    let run = side.join("run");
    std::fs::create_dir_all(&run).unwrap();
    for name in ["bindings", "instructions"] {
        let link = run.join(name);
        if !link.exists() {
            std::os::unix::fs::symlink(repo().join(name), link).unwrap();
        }
    }
    let decider = match &c.kind {
        Kind::Rule(verdict) => json!({"kind": "rule", "verdict": verdict}),
        Kind::Model { final_escalate, .. } => json!({"kind": "model", "model": MODEL, "adapter": adapter_arg(c, dir), "without_claims": true, "escalate": if *final_escalate { "final" } else { "stop" }}),
    };
    let settings = json!({"autonomy": "A5", "decider": decider});
    let native = NativeDecider::parse(&settings, &settings["decider"])?;
    let body = request_body(c, dir, &rec);
    let env = vec![("PATH".to_owned(), host_path())];
    let decision = json!({});
    let request = DecisionRequest { decision: &decision, run_dir: &run, workspace: &side, budget_micro_usd: BUDGET_MICRO, child_env: &env, login_link: None };
    let budget = if matches!(c.kind, Kind::Rule(_)) { 0 } else { BUDGET_MICRO };
    native.reply(&body, &rec, &request, budget)?;
    let mut files = BTreeMap::new();
    walk(&rec, &rec, &mut files);
    let reply = files.remove("reply.json").unwrap_or_default();
    let stderr = files.remove("decider-stderr.txt").unwrap_or_default();
    Ok(Outcome { reply, stderr, files })
}

/// The first line where the two texts differ, with both.
fn first_difference(a: &str, b: &str) -> String {
    for (n, (x, y)) in a.lines().zip(b.lines()).enumerate() {
        if x != y {
            return format!("line {}: script {x:?}, rust {y:?}", n + 1);
        }
    }
    format!("script has {} lines, rust {}", a.lines().count(), b.lines().count())
}

fn request_of(c: &Case) -> Option<crate::loom_run::answers::Request> {
    let kind = c.body["type"].as_str().filter(|k| *k == "question" || *k == "finding")?;
    let line = json!({"type": kind, "id": "x1", kind: c.body[kind].clone()});
    crate::loom_run::answers::parse_requests(&line.to_string()).ok()?.into_iter().next()
}

/// Every way the two sides differ for the case, as one text each.
fn compare(c: &Case, dir: &Path) -> (Vec<String>, &'static str) {
    let (script, rust) = match (run_script(c, dir), run_rust(c, dir)) {
        (Ok(s), Ok(r)) => (s, r),
        (s, r) => return (vec![format!("{}: a side failed: script {:?}, rust {:?}", c.name, s.err(), r.err())], "failed"),
    };
    let mut found = Vec::new();
    let (sr, rr) = (mask(&script.reply, dir), mask(&rust.reply, dir));
    if sr != rr {
        found.push(format!("{}: reply differs: {}", c.name, first_difference(&sr, &rr)));
    }
    let (se, re) = (mask(&script.stderr, dir), mask(&rust.stderr, dir));
    if se != re {
        found.push(format!("{}: standard error differs: {}", c.name, first_difference(&se, &re)));
    }
    let names: Vec<&String> = script.files.keys().chain(rust.files.keys()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    for name in names {
        match (script.files.get(name), rust.files.get(name)) {
            (Some(a), Some(b)) => {
                let (a, b) = (mask_times(&mask(a, dir)), mask_times(&mask(b, dir)));
                if a != b {
                    found.push(format!("{}: {name} differs: {}", c.name, first_difference(&a, &b)));
                }
            }
            (a, _) => found.push(format!("{}: {name} only in {}", c.name, if a.is_some() { "the script's directory" } else { "the Rust directory" })),
        }
    }
    let (sv, rv): (Value, Value) = (serde_json::from_str(&sr).unwrap_or(Value::Null), serde_json::from_str(&rr).unwrap_or(Value::Null));
    if super::approval::cost_micro(&sv) != super::approval::cost_micro(&rv) {
        found.push(format!("{}: the cost the runner books differs", c.name));
    }
    if let Some(request) = request_of(c) {
        let line = |reply: &Value| crate::loom_run::answers::decision_line(&json!({"case": CASE, "action": c.body["action"]}), &request, "rev", "approve", reply, "decider:model", json!({})).to_string();
        if line(&sv) != line(&rv) {
            found.push(format!("{}: the decisions.jsonl line differs", c.name));
        }
    }
    let class = if sv["dry"] == true { "dry" } else if sv["adapter"] == "rule" { "rule" } else if sv["valid"] == true { "valid" } else if sv["valid"] == false { "invalid" } else { "other" };
    (found, class)
}

#[test]
fn the_case_table_covers_every_kind_and_request_type_and_names_each_case_once() {
    let all = cases();
    let mut names: Vec<&str> = all.iter().map(|c| c.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), all.len(), "a case name is used twice");
    for kind in ["approval", "question", "finding", "close"] {
        for (label, want) in [("rule-approve", "rule-approve"), ("rule-reject", "rule-reject"), ("dry", "dry"), ("stub", "stub"), ("scripted", "scripted")] {
            let found = all.iter().any(|c| c.name.starts_with(want) && c.body["type"] == kind);
            assert!(found, "no case of {label} on a {kind} request");
        }
    }
    let decisions = ["approve", "reject", "escalate"];
    for d in decisions {
        assert!(all.iter().any(|c| c.decision.as_deref().is_some_and(|t| t.contains(&format!("\"decision\":\"{d}\"")))), "no scripted {d}");
    }
    assert!(all.iter().any(|c| c.decision.as_deref().is_some_and(|t| serde_json::from_str::<Value>(t).is_err())), "no invalid JSON");
    assert!(all.iter().any(|c| c.decision.is_none() && c.structured.is_none() && c.name.starts_with("scripted")), "no missing reply");
    assert!(all.iter().any(|c| c.decision.as_deref().is_some_and(|t| t.contains("\"best\""))), "no escalate with best");
}

#[test]
#[ignore = "slow lane: `task test:decider-parity` (decider.sh against the Rust decider; needs jq, yq, rsync and the built driver)"]
fn the_rust_decider_writes_the_bytes_of_decider_sh_for_every_case() {
    assert!(driver_bin().join("sdc-reply").is_file(), "{} holds no sdc-reply: run `cargo build --locked --bins` in driver/", driver_bin().display());
    let only = std::env::var("DECIDER_PARITY_ONLY").unwrap_or_default();
    let all: Vec<Case> = cases().into_iter().filter(|c| c.name.contains(&only)).collect();
    let next = AtomicUsize::new(0);
    let differences: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let classes: Mutex<BTreeMap<&str, usize>> = Mutex::new(BTreeMap::new());
    let started = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| loop {
                let n = next.fetch_add(1, Ordering::SeqCst);
                let Some(c) = all.get(n) else { break };
                let dir = Dir::new(&format!("parity-{n}"));
                write_fixture(dir.path());
                if let Kind::Model { adapter: "scripted", .. } = c.kind {
                    write_adapter(&dir.path().join(c.adapter_file), dir.path(), c);
                }
                let (found, class) = compare(c, dir.path());
                *classes.lock().unwrap().entry(class).or_insert(0) += 1;
                differences.lock().unwrap().extend(found);
            });
        }
    });
    let differences = differences.into_inner().unwrap();
    for d in &differences {
        eprintln!("DIFFERENCE {d}");
    }
    eprintln!("decider-parity: cases compared {}, differences {}, {:?}; replies by class {:?}", all.len(), differences.len(), started.elapsed(), classes.into_inner().unwrap());
    assert!(differences.is_empty(), "{} differences", differences.len());
}
