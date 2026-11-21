//! sdc-reply: what an LLM reply may cite, and the JSON schema of the reply, built by program.
//!
//!   sdc-reply citable --cwd <call dir> --case-dir <case>
//!   sdc-reply resolve --cwd <call dir> --case-dir <case> <cite>...
//!   sdc-reply schema  --cwd <call dir> --case-dir <case> --type approval|question|finding|close
//!                     [--final] [--options '<JSON list>'] [--rule]
//!   sdc-reply rejections --case-dir <case> --action <approval action>
//!   sdc-reply later --ir <ir.json> --action <action>
//!
//! The call dir holds the inputs the model was shown (`in/`), as tools/decider.sh writes them.
//! Citable (the forms of sdc.decision.Citation, ess/domains/decision.yaml, docs/decisions.md):
//! - an id: a requirement, dissection item, question or finding id of the case's `work/`, a
//!   context field (`context.<field>`), or a claim id of `in/checks.json`;
//! - `<artifact>@<revision>[#<entry>]` (artifact_revision): an artifact of the case's bindings
//!   whose file is in `in/`, at its current revision in `case.json`; the entry is a dotted path
//!   into a YAML or JSON file, a list element named by its `id` (`requirements.r1`);
//! - `<path>:<line>[-<line_end>]` (path_line): a file under `in/` (written without `in/`), the
//!   lines inside it; it is recorded as `work/<path>`, which resolves against the case directory.
//!
//! `citable` prints the enumerable set (ids, every artifact entry two levels deep, every non-empty
//! line of a Markdown input), the artifacts, and one example of each form from the actual input.
//! `resolve` prints one line per cite with its form and value (exit 0 when all resolve, else 1).
//! `schema` prints the reply schema of tools/decider.sh: `decision` an enum of the admissible
//! decisions, `cites` an enum of the citable set, `answer` the options of a rule finding.
//! `reply_schema` and `citable_set` are the parts another LLM node's schema can reuse.
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use serde_json::{json, Map, Value};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Citable { #[arg(long)] cwd: PathBuf, #[arg(long)] case_dir: PathBuf },
    Resolve { #[arg(long)] cwd: PathBuf, #[arg(long)] case_dir: PathBuf, cites: Vec<String> },
    Schema {
        #[arg(long)] cwd: PathBuf,
        #[arg(long)] case_dir: PathBuf,
        #[arg(long = "type")] kind: String,
        #[arg(long)] r#final: bool,
        #[arg(long, default_value = "[]")] options: String,
        #[arg(long)] rule: bool,
    },
    /// The rejected records of an approval action in a case, oldest first, each with the reasons
    /// its step kept (the sdc.rejection/1 log blob of `approve --result rejected`).
    Rejections { #[arg(long)] case_dir: PathBuf, #[arg(long)] action: String },
    /// What can only come after `action` (from the compiled protocol): the actions whose
    /// precondition needs a claim only it or they make true, the claims only those actions make
    /// true, and the obligations only those claims discharge. A decision about `action` must not
    /// show them as open conditions (runs/trial-3-mixed-3: health.required before release.publish).
    Later { #[arg(long)] ir: PathBuf, #[arg(long)] action: String },
}

/// The actions that may produce evidence `kind`.
fn producers<'a>(ir: &'a Value, kind: &Value) -> Vec<&'a str> {
    ir["actions"].as_object().into_iter().flatten()
        .filter(|(_, a)| a["may_produce"].as_array().is_some_and(|m| m.iter().any(|p| &p["evidence"] == kind)))
        .map(|(k, _)| k.as_str()).collect()
}

/// Claim `id` can be made true only through actions in `set`: its evidence has producers, all in
/// `set`; a claim defined over claims is read as a formula; a children claim never.
fn claim_needs(ir: &Value, id: &str, set: &BTreeSet<String>, depth: u8) -> bool {
    let w = &ir["claims"][id]["true_when"];
    if depth > 32 {
        return false;
    }
    if w["evidence"].is_object() {
        let p = producers(ir, &w["evidence"]["kind"]);
        !p.is_empty() && p.iter().all(|a| set.contains(*a))
    } else {
        formula_needs(ir, w, set, depth + 1)
    }
}

/// Formula `f` (a precondition, a discharge condition) holds only once a claim it requires true
/// is made true, which only actions in `set` can do: `all` when one part needs that, `any` when
/// every part does; a claim required to be anything but true (unknown, false) never needs it.
fn formula_needs(ir: &Value, f: &Value, set: &BTreeSet<String>, depth: u8) -> bool {
    if let Some(c) = f["claim"].as_object() {
        c["is"] == "true" && c["id"].as_str().is_some_and(|id| claim_needs(ir, id, set, depth))
    } else if let Some(all) = f["all"].as_array() {
        all.iter().any(|g| formula_needs(ir, g, set, depth))
    } else if let Some(any) = f["any"].as_array() {
        !any.is_empty() && any.iter().all(|g| formula_needs(ir, g, set, depth))
    } else {
        false
    }
}

/// `sdc-reply later`: see `Cmd::Later`.
fn later(ir: &Value, action: &str) -> Value {
    let mut reach: BTreeSet<String> = BTreeSet::from([action.to_string()]);
    loop {
        let add: Vec<String> = ir["actions"].as_object().into_iter().flatten()
            .filter(|(k, a)| !reach.contains(*k) && formula_needs(ir, &a["precondition"], &reach, 0))
            .map(|(k, _)| k.clone()).collect();
        if add.is_empty() {
            break;
        }
        reach.extend(add);
    }
    reach.remove(action);
    let claims: Vec<&String> = ir["claims"].as_object().into_iter().flatten()
        .filter(|(k, _)| claim_needs(ir, k, &reach, 0)).map(|(k, _)| k).collect();
    let obligations: Vec<&String> = ir["obligations"].as_object().into_iter().flatten()
        .filter(|(_, o)| formula_needs(ir, &o["discharged_when"], &reach, 0)).map(|(k, _)| k).collect();
    json!({"action": action, "after_actions": reach, "claims": claims, "obligations": obligations})
}

/// The rejected records of `action` (the evidence kind its binding produces, result `rejected`)
/// in the case, oldest first: `{record, revision, at, principal, reasons, citations}`. Derived from
/// the evidence and the event log, so no counter is kept anywhere.
fn rejections(case_dir: &Path, action: &str) -> Value {
    let bindings = read_json(&case_dir.join("bindings.json"));
    let Some(kind) = bindings[action]["produces"].as_str() else { return json!([]) };
    let events: Vec<Value> = fs::read_to_string(case_dir.join("events.jsonl")).unwrap_or_default().lines()
        .filter_map(|l| serde_json::from_str(l).ok()).collect();
    let mut out = Vec::new();
    for e in events.iter().filter(|e| e["event"] == "action_finished" && e["action"] == action) {
        for id in e["evidence"].as_array().into_iter().flatten().filter_map(|i| i.as_str()) {
            let r = read_json(&case_dir.join("evidence").join(format!("{id}.json")));
            if r["kind"] != kind || r["result"] != "rejected" {
                continue;
            }
            let why = e["log"].as_str().map(|l| read_json(&case_dir.join("blobs").join(l))).unwrap_or(Value::Null);
            out.push(json!({"record": id, "revision": r["subject_revision"], "at": r["observed_at"],
                "principal": r["producer"], "reasons": why["reasons"], "citations": why["citations"]}));
        }
    }
    Value::Array(out)
}

fn read_json(p: &Path) -> Value {
    fs::read(p).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null)
}

/// A YAML or JSON file as JSON; None when it is neither.
fn read_data(p: &Path) -> Option<Value> {
    let text = fs::read_to_string(p).ok()?;
    match p.extension().and_then(|e| e.to_str()) {
        Some("json") => serde_json::from_str(&text).ok(),
        Some("yaml" | "yml") => serde_yaml::from_str::<serde_yaml::Value>(&text).ok()
            .and_then(|v| serde_json::to_value(v).ok()),
        _ => None,
    }
}

struct Ctx {
    in_dir: PathBuf,
    work: PathBuf,
    /// artifact -> (file relative to work, current revision)
    artifacts: Vec<(String, String, String)>,
    ids: BTreeSet<String>,
}

fn ctx(cwd: &Path, case_dir: &Path) -> Ctx {
    let in_dir = cwd.join("in");
    let work = case_dir.join("work");
    let bindings = read_json(&case_dir.join("bindings.json"));
    let case = read_json(&case_dir.join("case.json"));
    let mut artifacts = Vec::new();
    for (a, f) in bindings["$artifacts"].as_object().into_iter().flatten() {
        let (Some(f), Some(rev)) = (f.as_str(), case["artifacts"][a]["revision"].as_str()) else { continue };
        if rev != "none" && in_dir.join(f).exists() {
            artifacts.push((a.clone(), f.to_string(), rev.to_string()));
        }
    }
    let mut ids = BTreeSet::new();
    for (file, list) in [("requirements.yaml", "requirements"), ("dissection.yaml", "items"),
                         ("questions.yaml", "questions"), ("findings.yaml", "findings")] {
        if let Some(v) = read_data(&work.join(file)) {
            for e in v[list].as_array().into_iter().flatten() {
                if let Some(id) = e["id"].as_str() { ids.insert(id.to_string()); }
            }
        }
    }
    if let Some(Value::Object(m)) = read_data(&work.join("context.yaml")) {
        ids.extend(m.keys().map(|k| format!("context.{k}")));
    }
    if let Some(m) = read_json(&in_dir.join("checks.json"))["claims"].as_object() {
        ids.extend(m.keys().cloned());
    }
    Ctx { in_dir, work, artifacts, ids }
}

/// One step into a value: an object key, a list element by its `id`, or a list index.
fn step<'a>(v: &'a Value, seg: &str) -> Option<&'a Value> {
    match v {
        Value::Object(m) => m.get(seg),
        Value::Array(a) => a.iter().find(|e| e["id"] == seg).or_else(|| seg.parse::<usize>().ok().and_then(|i| a.get(i))),
        _ => None,
    }
}

/// Entries two levels deep: `key`, `key.sub` (object) or `key.<id>` (list of objects with ids).
fn entries(v: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let Value::Object(m) = v else { return out };
    for (k, x) in m {
        out.push(k.clone());
        match x {
            Value::Object(n) => out.extend(n.keys().map(|s| format!("{k}.{s}"))),
            Value::Array(a) => out.extend(a.iter().filter_map(|e| e["id"].as_str()).map(|id| format!("{k}.{id}"))),
            _ => {}
        }
    }
    out
}

fn md_lines(p: &Path) -> Vec<usize> {
    fs::read_to_string(p).map(|t| t.lines().enumerate().filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, _)| i + 1).collect()).unwrap_or_default()
}

/// The enumerable citable set, the artifacts, and one example of each form.
fn citable_set(c: &Ctx) -> Value {
    let mut addresses = Vec::new();
    let mut ex_entry = Value::Null;
    let mut ex_line = Value::Null;
    for (a, f, rev) in &c.artifacts {
        let p = c.in_dir.join(f);
        addresses.push(format!("{a}@{rev}"));
        if let Some(v) = read_data(&p) {
            let es = entries(&v);
            if ex_entry.is_null() {
                if let Some(e) = es.iter().find(|e| e.contains('.')).or(es.first()) { ex_entry = json!(format!("{a}@{rev}#{e}")); }
            }
            addresses.extend(es.into_iter().map(|e| format!("{a}@{rev}#{e}")));
        } else {
            for l in md_lines(&p) {
                if ex_line.is_null() { ex_line = json!(format!("{f}:{l}")); }
                addresses.push(format!("{f}:{l}"));
            }
        }
    }
    // Markdown inputs that are no artifact of this case (an epic, a story): lines only.
    let known: BTreeSet<&str> = c.artifacts.iter().map(|(_, f, _)| f.as_str()).collect();
    for f in walk(&c.in_dir).into_iter().filter(|f| f.ends_with(".md") && !known.contains(f.as_str())) {
        for l in md_lines(&c.in_dir.join(&f)) {
            if ex_line.is_null() { ex_line = json!(format!("{f}:{l}")); }
            addresses.push(format!("{f}:{l}"));
        }
    }
    if ex_line.is_null() {
        // A YAML input's line: the subject's first, else any.
        if let Some((_, f, _)) = c.artifacts.iter().find(|(_, f, _)| f.ends_with(".yaml")) {
            if let Some(l) = md_lines(&c.in_dir.join(f)).first() { ex_line = json!(format!("{f}:{l}")); }
        }
    }
    let ex_id = c.ids.iter().find(|i| i.starts_with('r')).or(c.ids.iter().next()).cloned();
    json!({"ids": c.ids, "addresses": addresses,
        "artifacts": c.artifacts.iter().map(|(a, f, r)| json!({"artifact": a, "file": f, "revision": r})).collect::<Vec<_>>(),
        "examples": {"id": ex_id, "entry": ex_entry, "line": ex_line}})
}

fn walk(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let Ok(rd) = fs::read_dir(dir.join(&rel)) else { continue };
        for e in rd.flatten() {
            let r = rel.join(e.file_name());
            if e.path().is_dir() { stack.push(r) } else { out.push(r.to_string_lossy().into_owned()) }
        }
    }
    out.sort();
    out
}

/// One cite: its form and value (sdc.decision.Citation), or why it does not resolve.
fn resolve(c: &Ctx, cite: &str) -> Value {
    if c.ids.contains(cite) {
        return json!({"cite": cite, "ok": true, "form": "id", "value": {"id": cite}});
    }
    let fail = |why: String| json!({"cite": cite, "ok": false, "why": why});
    if let Some((a, rest)) = cite.split_once('@') {
        let (rev, entry) = match rest.split_once('#') { Some((r, e)) => (r, Some(e)), None => (rest, None) };
        let Some((_, f, cur)) = c.artifacts.iter().find(|(x, _, _)| x == a) else {
            return fail(format!("{a} is not an artifact in in/ (artifacts: {})",
                c.artifacts.iter().map(|(x, _, _)| x.as_str()).collect::<Vec<_>>().join(", ")));
        };
        if rev != cur {
            return fail(format!("{a}@{rev} is not the current revision ({cur})"));
        }
        let mut value = json!({"artifact": a, "revision": rev});
        if let Some(e) = entry {
            let Some(data) = read_data(&c.in_dir.join(f)) else { return fail(format!("{f} has no entries (not YAML or JSON): cite its lines, {f}:<line>")) };
            let mut v = &data;
            for seg in e.split('.') {
                match step(v, seg) { Some(x) => v = x, None => return fail(format!("{f} has no entry {e} (no `{seg}`)")) }
            }
            value["entry"] = json!(e);
        }
        return json!({"cite": cite, "ok": true, "form": "artifact_revision", "value": value});
    }
    if let Some((p, lines)) = cite.rsplit_once(':') {
        let p = p.strip_prefix("in/").or(p.strip_prefix("work/")).unwrap_or(p);
        let (l, e) = match lines.split_once('-') { Some((a, b)) => (a.parse::<usize>(), b.parse::<usize>()), None => (lines.parse(), lines.parse()) };
        if let (Ok(l), Ok(e)) = (l, e) {
            let f = c.in_dir.join(p);
            if p.contains("..") || !f.is_file() { return fail(format!("no file {p} in in/")) }
            let n = fs::read_to_string(&f).map(|t| t.lines().count()).unwrap_or(0);
            if !(l >= 1 && e >= l && e <= n) { return fail(format!("lines {l}-{e} outside {p} ({n} lines)")) }
            let mut v = json!({"path": format!("work/{p}"), "line": l});
            if e != l { v["line_end"] = json!(e); }
            return json!({"cite": cite, "ok": true, "form": "path_line", "value": v});
        }
    }
    let _ = &c.work;
    fail("not an id of this case, not <artifact>@<revision>[#<entry>], not <file>:<line>".into())
}

/// A JSON schema of one object reply: every property required, nothing else allowed.
pub fn reply_schema(props: Vec<(&str, Value)>) -> Value {
    let required: Vec<&str> = props.iter().map(|(k, _)| *k).collect();
    let properties: Map<String, Value> = props.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    json!({"type": "object", "additionalProperties": false, "required": required, "properties": properties})
}

fn decider_schema(c: &Ctx, kind: &str, fin: bool, options: &[String], rule: bool) -> Value {
    let set = citable_set(c);
    let mut cites: Vec<Value> = set["ids"].as_array().cloned().unwrap_or_default();
    cites.extend(set["addresses"].as_array().cloned().unwrap_or_default());
    let decisions: Vec<&str> = if kind == "finding" { vec!["approve", "escalate"] } else { vec!["approve", "reject", "escalate"] };
    let answer = match kind {
        "finding" if rule => json!({"anyOf": [{"type": "string", "enum": options}, {"type": "null"}]}),
        "question" | "finding" => json!({"type": ["string", "null"]}),
        _ => json!({"type": "null"}),
    };
    let best = match (kind, fin) {
        ("finding", true) if rule => json!({"anyOf": [{"type": "string", "enum": options}, {"type": "null"}]}),
        ("finding", true) => json!({"type": ["string", "null"]}),
        ("question", _) | (_, false) => json!({"type": "null"}),
        _ => json!({"anyOf": [{"type": "string", "enum": ["approve", "reject"]}, {"type": "null"}]}),
    };
    reply_schema(vec![
        ("decision", json!({"type": "string", "enum": decisions})),
        ("answer", answer),
        ("reasons", json!({"type": "array", "minItems": 1, "items": {"type": "string"}})),
        ("cites", json!({"type": "array", "items": {"type": "string", "enum": cites}})),
        ("best", best),
    ])
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Citable { cwd, case_dir } => {
            println!("{}", citable_set(&ctx(&cwd, &case_dir)));
            ExitCode::SUCCESS
        }
        Cmd::Resolve { cwd, case_dir, cites } => {
            let c = ctx(&cwd, &case_dir);
            let rs: Vec<Value> = cites.iter().map(|x| resolve(&c, x)).collect();
            let ok = rs.iter().all(|r| r["ok"] == true);
            println!("{}", Value::Array(rs));
            if ok { ExitCode::SUCCESS } else { ExitCode::from(1) }
        }
        Cmd::Schema { cwd, case_dir, kind, r#final, options, rule } => {
            let opts: Vec<String> = serde_json::from_str(&options).unwrap_or_default();
            println!("{}", decider_schema(&ctx(&cwd, &case_dir), &kind, r#final, &opts, rule));
            ExitCode::SUCCESS
        }
        Cmd::Rejections { case_dir, action } => {
            println!("{}", rejections(&case_dir, &action));
            ExitCode::SUCCESS
        }
        Cmd::Later { ir, action } => {
            let ir = read_json(&ir);
            if ir["actions"].get(&action).is_none() {
                eprintln!("sdc-reply later: no action `{action}` in the protocol");
                return ExitCode::from(64);
            }
            println!("{}", later(&ir, &action));
            ExitCode::SUCCESS
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ir(name: &str) -> Value {
        read_json(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../protocol/compiled").join(format!("{name}.ir.json")))
    }

    #[test]
    fn health_is_met_after_release_publish() {
        let l = later(&ir("sdc-release"), "release.publish");
        assert_eq!(l["obligations"], json!(["health.required"]));
        assert_eq!(l["claims"], json!(["release.healthy", "release.observed", "rollback.complete"]));
        assert_eq!(l["after_actions"], json!(["release.observe", "release.rollback"]));
    }

    #[test]
    fn what_the_action_itself_settles_is_not_later() {
        // merge.approve makes merge.approved true itself (settled by the decision, not later);
        // change.merge needs it, so change.merged comes after.
        let l = later(&ir("sdc-story"), "merge.approve");
        assert!(!l["claims"].as_array().unwrap().iter().any(|c| c == "merge.approved"), "{l}");
        assert!(l["claims"].as_array().unwrap().iter().any(|c| c == "change.merged"), "{l}");
        assert_eq!(l["obligations"], json!([]));
    }
}
