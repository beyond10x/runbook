//! sdc-reply: what an LLM reply may cite, and the JSON schema of the reply, built by program.
//!
//!   sdc-reply citable --cwd <call dir> --case-dir <case>
//!   sdc-reply resolve --cwd <call dir> --case-dir <case> <cite>...
//!   sdc-reply schema  --cwd <call dir> --case-dir <case> --type approval|question|finding|close
//!                     [--final] [--options '<JSON list>'] [--rule]
//!   sdc-reply rejections --case-dir <case> --action <approval action>
//!   sdc-reply later --ir <ir.json> --action <action>
//!   sdc-reply review-inputs --cwd <call dir> --case-dir <case> --story <story>
//!   sdc-reply schema --cwd <call dir> --case-dir <case> --type review
//!   sdc-reply check-review --cwd <call dir> --case-dir <case> --reply <file> --principal <p>
//!   sdc-reply adapters --tools <tools dir>
//!   sdc-reply coverage [--work <work>] [--case-dir <case>] [--story <story>] [--schema]
//!   sdc-reply check-coverage [--work <work>] [--case-dir <case>] [--story <story>] --tree <tree> [--reply <file>] [--shape]
//!   sdc-reply coverage-stub [--work <work>] [--case-dir <case>] [--story <story>] --tree <tree>
//!
//! change.implement (story:implement-cites-requirements): besides its change, the implementer
//! replies with a coverage document, per requirement the story carries (review_scope's story
//! requirements) `built` with the file and line that builds it and a test, or `not_built` with a
//! reason. `coverage --schema` prints its reply schema; `check-coverage --shape` checks a reply
//! against it (llm-node.sh asks again with the reason, at most twice); llm-node.sh puts a valid one
//! at `impl/.sdc/coverage.json`. Without `--shape`, `check-coverage` is the implement binding's
//! `postcondition`, run by sdc-drive right after the writer: exit 1 refuses the revision, naming
//! the requirement ids (missing, named twice, not built, a file not in the tree, a line outside
//! its file, a test the test sources lack); nothing is recorded and the writer is sent back.
//! `coverage-stub` writes the stub implementer's document. review-inputs copies the document to
//! the review's `in/coverage.json`.
//!
//! `adapters`: the adapter of every node class whose reply is a data document made under a
//! response schema (`schema_reply` in tools/node-classes.json: review), and whether it can take a
//! schema. The class's adapter: SDC_<CLASS>_ADAPTER, else claude-cli when SDC_LLM_ADAPTER is
//! metaharness (it has no response-schema option), else SDC_LLM_ADAPTER (stub when unset).
//! claude-cli takes a schema, metaharness none, stub and dry call no model; an adapter program
//! (a path) answers `<path> --capabilities` with `{"json_schema": true|false}`. Prints
//! `{<class>: {adapter, source, json_schema, nodes}}`; exit 64 (stderr names the class and
//! the adapter) when a class's adapter cannot take a schema.
//!
//! change.review (tools/llm-node.sh): `review-inputs` writes the review's inputs into `in/` (the
//! story, its acceptance lines, the requirements it delivers and those no story covers, the
//! covered commands, the change tree, its diff against the mainline, the gate and test logs);
//! `schema --type review` prints the reply schema; `check-review` checks the reply: exit 0
//! approved, 1 rejected (stdout: the sdc.rejection/1 that sends the implementer back), 2 invalid
//! (stderr: why; nothing is recorded).
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
    /// Write change.review's inputs into `<cwd>/in/`; print what was written.
    ReviewInputs { #[arg(long)] cwd: PathBuf, #[arg(long)] case_dir: PathBuf, #[arg(long)] story: String },
    /// Check a change.review reply: exit 0 approved, 1 rejected (the rejection on stdout), 2 invalid.
    CheckReview {
        #[arg(long)] cwd: PathBuf,
        #[arg(long)] case_dir: PathBuf,
        #[arg(long)] reply: PathBuf,
        #[arg(long)] principal: String,
    },
    /// The adapter of every schema-reply node class and whether it takes a schema (exit 64 if not).
    Adapters { #[arg(long)] tools: PathBuf },
    /// change.implement's coverage document: `--shape` checks a reply against the schema (exit 0
    /// fits, 2 not: stderr says why); without it, the document in the tree (or `--reply`) against
    /// the change tree: exit 0 complete, 1 refused (stdout names the requirement ids), 2 when it
    /// cannot check. `--case-dir` and `--story` default to the driver's SDC_CASE_DIR and SDC_CASE.
    CheckCoverage {
        #[arg(long)] work: Option<PathBuf>,
        #[arg(long)] case_dir: Option<PathBuf>,
        #[arg(long)] story: Option<String>,
        #[arg(long)] tree: PathBuf,
        #[arg(long)] reply: Option<PathBuf>,
        #[arg(long)] shape: bool,
    },
    /// The requirements the story carries, `[{id, text}]`, and with `--schema` the coverage schema.
    Coverage {
        #[arg(long)] work: Option<PathBuf>,
        #[arg(long)] case_dir: Option<PathBuf>,
        #[arg(long)] story: Option<String>,
        #[arg(long)] schema: bool,
    },
    /// The stub implementer's coverage document for a tree (no model).
    CoverageStub {
        #[arg(long)] work: Option<PathBuf>,
        #[arg(long)] case_dir: Option<PathBuf>,
        #[arg(long)] story: Option<String>,
        #[arg(long)] tree: PathBuf,
    },
}

/// The adapter of a schema-reply class from the environment, and where it came from.
fn class_adapter(class: &str, env: &dyn Fn(&str) -> Option<String>) -> (String, String) {
    let var = format!("SDC_{}_ADAPTER", class.to_ascii_uppercase());
    if let Some(a) = env(&var).filter(|a| !a.is_empty()) {
        return (a, var);
    }
    match env("SDC_LLM_ADAPTER").filter(|a| !a.is_empty()) {
        Some(a) if a == "metaharness" => ("claude-cli".into(), "default: claude-cli, SDC_LLM_ADAPTER is metaharness (no response schema)".into()),
        Some(a) => (a, "SDC_LLM_ADAPTER".into()),
        None => ("stub".into(), "default: stub".into()),
    }
}

/// Whether an adapter takes a response schema: Some(true|false), or None when it calls no model.
fn takes_schema(adapter: &str) -> Option<bool> {
    match adapter {
        "stub" | "dry" => None,
        "claude-cli" => Some(true),
        "metaharness" => Some(false),
        path => Some(std::process::Command::new(path).arg("--capabilities").output().ok()
            .and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok())
            .is_some_and(|v| v["json_schema"] == true)),
    }
}

/// `{class: {adapter, source, json_schema, nodes}}` for every schema-reply class, and the refusals.
fn adapters(tools: &Path, env: &dyn Fn(&str) -> Option<String>) -> (Value, Vec<String>) {
    let classes = read_json(&tools.join("node-classes.json"));
    let mut out = Map::new();
    let mut refused = Vec::new();
    for c in classes["schema_reply"].as_array().into_iter().flatten().filter_map(|c| c.as_str()) {
        let (a, src) = class_adapter(c, env);
        let schema = takes_schema(&a);
        let nodes = classes["classes"][c].clone();
        if schema == Some(false) {
            refused.push(format!("node class {c} ({}) replies under a response schema, but its adapter {a} ({src}) takes none: \
                set SDC_{}_ADAPTER=claude-cli", nodes.as_array().into_iter().flatten().filter_map(|n| n.as_str()).collect::<Vec<_>>().join(", "),
                c.to_ascii_uppercase()));
        }
        out.insert(c.to_string(), json!({"adapter": a, "source": src, "json_schema": schema, "nodes": nodes}));
    }
    (Value::Object(out), refused)
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
    let decisions: Vec<&str> = if kind == "finding" || kind == "question" { vec!["approve", "escalate"] } else { vec!["approve", "reject", "escalate"] };
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

// --- change.review (story:review-node-is-a-model) ------------------------------------------------

/// The front matter's `summary` of an AEP story file, and the line it starts on (1-based).
fn story_summary(text: &str) -> Option<(usize, String)> {
    let mut lines = text.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let front: Vec<&str> = lines.take_while(|l| l.trim() != "---").collect();
    let v: serde_yaml::Value = serde_yaml::from_str(&front.join("\n")).ok()?;
    let summary = v.get("summary")?.as_str()?.to_string();
    let at = front.iter().position(|l| l.starts_with("summary:"))? + 2;
    Some((at, summary))
}

/// The commands a story covers: the words after `covers` in its summary.
fn covers(summary: &str) -> Vec<String> {
    summary.split_once("covers ").map(|(_, c)| c.split_whitespace()
        .map(|w| w.trim_end_matches(['.', ',']).to_string()).filter(|w| !w.is_empty()).collect()).unwrap_or_default()
}

/// `todo.task.AddTask` -> `todo.task`.
fn domain_of(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(d, _)| d)
}

/// Whether a requirement is delivered by a story covering `cov`: a traced command it covers, or a
/// traced view or actor of a covered command's domain; without a trace entry, a `spec_domains`
/// entry that is a covered command's domain.
fn delivered_by(req: &Value, traces: &Value, commands: &Value, cov: &[String]) -> bool {
    let doms: BTreeSet<&str> = cov.iter().map(|c| domain_of(c)).collect();
    match traces[req["id"].as_str().unwrap_or("")].as_array() {
        Some(t) => t.iter().filter_map(|x| x.as_str()).any(|x| cov.iter().any(|c| c == x)
            || (commands.get(x).is_none() && doms.contains(domain_of(x)))),
        None => req["spec_domains"].as_array().into_iter().flatten().filter_map(|d| d.as_str()).any(|d| doms.contains(d)),
    }
}

/// The requirements of the story (`cov`) and the requirements no story of the plan covers
/// (`all_cov`: what every story covers). A constraint or a dropped requirement is in neither.
fn review_scope(reqs: &[Value], traces: &Value, commands: &Value, cov: &[String], all_cov: &[Vec<String>]) -> (Vec<Value>, Vec<Value>) {
    let mut mine = Vec::new();
    let mut uncovered = Vec::new();
    for r in reqs {
        if matches!(r["behaviour"].as_str(), Some("constraint" | "dropped")) {
            continue;
        }
        let cites = traces.get(r["id"].as_str().unwrap_or("")).cloned().unwrap_or_else(|| r["spec_domains"].clone());
        let e = json!({"id": r["id"], "text": r["text"], "cites": cites});
        if delivered_by(r, traces, commands, cov) {
            mine.push(e);
        } else if !all_cov.iter().any(|c| delivered_by(r, traces, commands, c)) {
            uncovered.push(e);
        }
    }
    (mine, uncovered)
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)?.flatten() {
        let name = e.file_name();
        if name == "target" || name == ".sdc" || name == ".derived-from" {
            continue;
        }
        let (s, d) = (e.path(), to.join(&name));
        if s.is_dir() { copy_tree(&s, &d)? } else { fs::copy(&s, &d).map(|_| ())? }
    }
    Ok(())
}

/// The log of the last record of `kind` about `revision` in the case (its step's log blob).
fn record_log(case_dir: &Path, kind: &str, revision: &str) -> Option<String> {
    let events: Vec<Value> = fs::read_to_string(case_dir.join("events.jsonl")).unwrap_or_default().lines()
        .filter_map(|l| serde_json::from_str(l).ok()).collect();
    events.iter().rev().filter(|e| e["event"] == "action_finished").find_map(|e| {
        let hit = e["evidence"].as_array().into_iter().flatten().filter_map(|i| i.as_str()).any(|id| {
            let r = read_json(&case_dir.join("evidence").join(format!("{id}.json")));
            r["kind"] == kind && r["subject_revision"] == revision
        });
        if !hit { return None }
        e["log"].as_str().and_then(|l| fs::read_to_string(case_dir.join("blobs").join(l)).ok())
    })
}

/// Write the review's inputs into `<cwd>/in/` and return what was written. The story, its
/// acceptance lines (the summary line and the lines of an `## Acceptance` section), the
/// requirements it delivers and those no story covers, the covered commands' IR entries, the
/// change tree, its diff against the mainline (empty for the first story), the gate and test logs.
/// A story's scope in the case's `work/`: its file, its acceptance lines, the commands it covers,
/// the requirements it delivers (`mine`) and those no story of the plan covers.
struct Scope {
    rel: String,
    text: String,
    cov: Vec<String>,
    acceptance: Vec<Value>,
    mine: Vec<Value>,
    uncovered: Vec<Value>,
    traces: Value,
    ir: Value,
}

fn scope_of(work: &Path, story: &str) -> Result<Scope, String> {
    let rel = format!("aep/story/{story}.md");
    let text = fs::read_to_string(work.join(&rel)).map_err(|e| format!("{rel}: {e}"))?;
    let (sl, summary) = story_summary(&text).ok_or(format!("{rel} has no summary"))?;
    let cov = covers(&summary);
    let mut acceptance = vec![json!({"cite": format!("{rel}:{sl}"), "text": summary})];
    let mut in_acc = false;
    for (i, l) in text.lines().enumerate() {
        if l.starts_with("## ") { in_acc = l.trim() == "## Acceptance"; continue }
        if in_acc && !l.trim().is_empty() { acceptance.push(json!({"cite": format!("{rel}:{}", i + 1), "text": l.trim()})) }
    }
    let mut all_cov = Vec::new();
    for f in walk(&work.join("aep/story")) {
        if let Some((_, s)) = fs::read_to_string(work.join("aep/story").join(&f)).ok().as_deref().and_then(story_summary) {
            all_cov.push(covers(&s));
        }
    }
    let reqs = read_data(&work.join("requirements.yaml")).map(|v| v["requirements"].as_array().cloned().unwrap_or_default()).unwrap_or_default();
    let traces = read_json(&work.join("spec/traces.json"))["traces"].clone();
    let ir = read_json(&work.join(".sdc/gate/ir.json"));
    let (mine, uncovered) = review_scope(&reqs, &traces, &ir["commands"], &cov, &all_cov);
    Ok(Scope { rel, text, cov, acceptance, mine, uncovered, traces, ir })
}

fn review_inputs(cwd: &Path, case_dir: &Path, story: &str) -> Result<Value, String> {
    let work = case_dir.join("work");
    let in_dir = cwd.join("in");
    let Scope { rel, text, cov, acceptance, mine, uncovered, traces, ir } = scope_of(&work, story)?;
    fs::create_dir_all(in_dir.join("aep/story")).map_err(|e| e.to_string())?;
    fs::write(in_dir.join(&rel), &text).map_err(|e| e.to_string())?;
    let commands: Map<String, Value> = cov.iter().map(|c| (c.clone(), ir["commands"][c].clone())).collect();
    let w = |name: &str, v: &Value| fs::write(in_dir.join(name), serde_json::to_string_pretty(v).unwrap_or_default()).map_err(|e| e.to_string());
    w("scope.json", &json!({"story": story, "covers": cov, "acceptance": acceptance, "requirements": mine,
        "traced_by": if traces.is_null() { "spec_domains (no spec/traces.json)" } else { "spec/traces.json" }}))?;
    w("uncovered.json", &json!({"about": "requirements no story of the plan covers: outside this story", "requirements": uncovered}))?;
    w("commands.json", &Value::Object(commands))?;
    copy_tree(&work.join("impl"), &in_dir.join("impl")).map_err(|e| format!("impl: {e}"))?;
    // The implementer's coverage document, read-only: where it says each requirement is built.
    let coverage = work.join("impl").join(COVERAGE);
    let has_coverage = coverage.is_file();
    if has_coverage {
        fs::copy(&coverage, in_dir.join("coverage.json")).map_err(|e| format!("coverage: {e}"))?;
    }
    let base = work.join("mainline");
    let empty = cwd.join("base-empty");
    let base = if base.is_dir() { base } else { fs::create_dir_all(&empty).map_err(|e| e.to_string())?; empty };
    let out = std::process::Command::new("diff").args(["-ruN", "-x", "target", "-x", ".sdc", "-x", ".derived-from"]).arg(&base).arg(work.join("impl"))
        .output().map_err(|e| format!("diff: {e}"))?;
    let (b, a) = (format!("{}/", base.display()), format!("{}/", work.join("impl").display()));
    let diff: String = String::from_utf8_lossy(&out.stdout).lines().map(|l| {
        let l = if l.starts_with("diff ") || l.starts_with("--- ") || l.starts_with("+++ ") { l.replace(&b, "mainline/").replace(&a, "impl/") } else { l.to_string() };
        l + "\n"
    }).collect();
    fs::write(in_dir.join("change.diff"), &diff).map_err(|e| e.to_string())?;
    let case = read_json(&case_dir.join("case.json"));
    let rev = |a: &str| case["artifacts"][a]["revision"].as_str().unwrap_or("none").to_string();
    // sdc.story (revision 7) admits change.review only once the conformance run passed at the
    // revision under review, so its log is always there; without it the review has no input.
    let tests = record_log(case_dir, "conformance_run", &rev("change")).ok_or(format!(
        "no conformance run recorded on change revision {} (sdc.story reviews only after tests.pass)", rev("change")))?;
    let gate = record_log(case_dir, "ess_gate", &rev("spec"));
    fs::write(in_dir.join("tests.txt"), &tests).map_err(|e| e.to_string())?;
    fs::write(in_dir.join("gate.txt"), gate.clone().unwrap_or("no gate run recorded on this specification revision\n".into()))
        .map_err(|e| e.to_string())?;
    let changed: Vec<&str> = diff.lines().filter_map(|l| l.strip_prefix("+++ impl/")).map(|l| l.split('\t').next().unwrap_or(l)).collect();
    Ok(json!({"story": story, "covers": cov, "requirements": mine.iter().map(|r| r["id"].clone()).collect::<Vec<_>>(),
        "uncovered": uncovered.iter().map(|r| r["id"].clone()).collect::<Vec<_>>(), "changed_files": changed,
        "gate": gate.is_some(), "coverage": has_coverage, "base": if base.ends_with("mainline") { "mainline" } else { "empty" }}))
}

/// What a review finding may name as violated (`violation`: the story's requirements, acceptance
/// lines and covered commands) and as outside the story (`outside`: requirements no story covers).
fn review_targets(in_dir: &Path) -> (Vec<String>, Vec<String>) {
    let scope = read_json(&in_dir.join("scope.json"));
    let ids = |v: &Value, k: &str| v.as_array().into_iter().flatten().filter_map(|e| e[k].as_str().map(String::from)).collect::<Vec<_>>();
    let mut inside = ids(&scope["requirements"], "id");
    inside.extend(ids(&scope["acceptance"], "cite"));
    inside.extend(scope["covers"].as_array().into_iter().flatten().filter_map(|c| c.as_str().map(String::from)));
    (inside, ids(&read_json(&in_dir.join("uncovered.json"))["requirements"], "id"))
}

/// The reply schema of change.review: a decision, a summary, and findings, each a place in the
/// change tree (`impl/<path>:<line>[-<line>]`) and what it violates.
fn review_schema(in_dir: &Path) -> Value {
    let (inside, outside) = review_targets(in_dir);
    let mut all = inside;
    all.extend(outside);
    reply_schema(vec![
        ("decision", json!({"type": "string", "enum": ["approved", "rejected"]})),
        ("summary", json!({"type": "string"})),
        ("findings", json!({"type": "array", "items": reply_schema(vec![
            ("kind", json!({"type": "string", "enum": ["violation", "outside"]})),
            ("at", json!({"type": "string", "pattern": "^impl/[^:]+:[0-9]+(-[0-9]+)?$"})),
            ("violates", json!({"type": "string", "enum": all})),
            ("says", json!({"type": "string", "minLength": 1})),
        ])})),
    ])
}

/// The review reply checked by program. Ok((approved, rejection or note)), or Err(reasons) when
/// the reply is invalid: an unknown decision, a finding whose place is not a line of the change
/// tree, a violation of something outside the story's set, an `outside` finding about a
/// requirement a story covers, an empty `says`, or a rejection with no violation finding.
fn check_review(cwd: &Path, case_dir: &Path, reply: &Value, principal: &str) -> Result<(bool, Value), Vec<String>> {
    let c = ctx(cwd, case_dir);
    let (inside, outside) = review_targets(&c.in_dir);
    let changed: BTreeSet<String> = fs::read_to_string(c.in_dir.join("change.diff")).unwrap_or_default().lines()
        .filter_map(|l| l.strip_prefix("+++ impl/")).map(|l| l.split('\t').next().unwrap_or(l).to_string()).collect();
    let mut why = Vec::new();
    let decision = reply["decision"].as_str().unwrap_or("");
    if decision != "approved" && decision != "rejected" {
        why.push(format!("decision {:?} is neither approved nor rejected", reply["decision"]));
    }
    let mut reasons = Vec::new();
    let mut notes = Vec::new();
    let mut citations = Vec::new();
    for (i, f) in reply["findings"].as_array().into_iter().flatten().enumerate() {
        let n = i + 1;
        let (kind, at, v, says) = (f["kind"].as_str().unwrap_or(""), f["at"].as_str().unwrap_or(""),
            f["violates"].as_str().unwrap_or(""), f["says"].as_str().unwrap_or("").trim());
        let r = resolve(&c, at);
        if !at.starts_with("impl/") || r["ok"] != true {
            why.push(format!("finding {n}: {at:?} is not a line of the change tree ({})", r["why"].as_str().unwrap_or("not under impl/")));
        }
        match kind {
            "violation" if !inside.iter().any(|x| x == v) =>
                why.push(format!("finding {n}: {v:?} is not a requirement, acceptance line or command of this story ({})", inside.join(", "))),
            "outside" if !outside.iter().any(|x| x == v) =>
                why.push(format!("finding {n}: {v:?} is not a requirement no story covers ({})", if outside.is_empty() { "none".into() } else { outside.join(", ") })),
            "violation" | "outside" => {}
            _ => why.push(format!("finding {n}: kind {kind:?} is neither violation nor outside")),
        }
        if says.is_empty() {
            why.push(format!("finding {n}: says nothing"));
        }
        let file = at.strip_prefix("impl/").and_then(|p| p.rsplit_once(':')).map(|(p, _)| p).unwrap_or("");
        let line = format!("{at}: {says} ({} {v})", if kind == "outside" { "outside this story:" } else { "violates" });
        if kind == "outside" { notes.push(line) } else { reasons.push(line) }
        citations.push(json!({"at": at, "violates": v, "kind": kind, "in_diff": changed.contains(file)}));
    }
    if decision == "rejected" && reasons.is_empty() {
        why.push("a rejection names no violation finding: reject only for a requirement, acceptance line or command of this story".into());
    }
    if !why.is_empty() {
        return Err(why);
    }
    let case = read_json(&case_dir.join("case.json"));
    if decision == "approved" {
        return Ok((true, json!({"approved": true, "principal": principal, "notes": notes, "citations": citations})));
    }
    let count = rejections(case_dir, "change.review").as_array().map_or(0, |a| a.len()) + 1;
    Ok((false, json!({"format": "sdc.rejection/1", "approval": "change.review", "case": case["id"], "subject": "change",
        "revision": case["artifacts"]["change"]["revision"], "principal": principal, "reasons": reasons,
        "notes": notes, "citations": citations, "count": count, "summary": reply["summary"]})))
}

// --- change.implement's requirement coverage (story:implement-cites-requirements) ---------------

/// Where llm-node.sh puts the implementer's coverage document: inside the change tree, so it is
/// part of the revision it describes (`.sdc/` is left out of the review's copy and diff).
const COVERAGE: &str = ".sdc/coverage.json";

/// The requirement ids the story carries: the requirements review_scope gives the story.
fn story_requirements(work: &Path, story: &str) -> Result<Vec<Value>, String> {
    Ok(scope_of(work, story)?.mine)
}

/// The reply schema of change.implement's coverage document: one entry per requirement of the
/// story, `built` with at least one place and a test, or `not_built` with a reason.
fn coverage_schema(ids: &[String]) -> Value {
    let id = if ids.is_empty() { json!({"type": "string"}) } else { json!({"type": "string", "enum": ids}) };
    let built = reply_schema(vec![
        ("id", id.clone()),
        ("status", json!({"type": "string", "enum": ["built"]})),
        ("at", json!({"type": "array", "minItems": 1, "items": reply_schema(vec![
            ("file", json!({"type": "string", "minLength": 1})),
            ("line", json!({"type": "integer", "minimum": 1})),
        ])})),
        ("test", json!({"type": "string", "minLength": 1})),
    ]);
    let not_built = reply_schema(vec![
        ("id", id),
        ("status", json!({"type": "string", "enum": ["not_built"]})),
        ("reason", json!({"type": "string", "minLength": 1})),
    ]);
    let mut list = json!({"type": "array", "items": {"anyOf": [built, not_built]}});
    if ids.is_empty() {
        list["maxItems"] = json!(0);
    }
    reply_schema(vec![("requirements", list)])
}

/// The coverage document checked by program against coverage_schema: why it does not fit, empty
/// when it does.
fn coverage_shape(doc: &Value, ids: &[String]) -> Vec<String> {
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
fn rust_sources(tree: &Path) -> Vec<String> {
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
fn test_names(tree: &Path) -> BTreeSet<String> {
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
fn check_coverage(tree: &Path, doc: &Value, ids: &[String]) -> Vec<(String, String)> {
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
fn refusal_text(story: &str, faults: &[(String, String)]) -> String {
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
fn coverage_stub(tree: &Path, ids: &[String]) -> Value {
    let srcs = rust_sources(tree);
    let file = srcs.iter().find(|f| f.starts_with("src/")).or(srcs.first()).cloned()
        .or_else(|| walk(tree).into_iter().find(|f| !f.starts_with(".sdc/") && !f.starts_with("target/")));
    let test = test_names_in_order(tree).into_iter().next();
    let reqs: Vec<Value> = ids.iter().map(|id| match (&file, &test) {
        (Some(f), Some(t)) => json!({"id": id, "status": "built", "at": [{"file": f, "line": 1}], "test": t}),
        _ => json!({"id": id, "status": "not_built", "reason": "the stub tree has no test"}),
    }).collect();
    json!({"requirements": reqs})
}

/// The test names in source order (files sorted), for the stub document.
fn test_names_in_order(tree: &Path) -> Vec<String> {
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
fn story_arg(story: Option<String>) -> Result<String, String> {
    story.or_else(|| std::env::var("SDC_CASE").ok().and_then(|c| c.split('.').nth(2).map(String::from)))
        .filter(|s| !s.is_empty()).ok_or("no --story and no story in SDC_CASE".into())
}

/// The case's `work/`: `--work` (llm-node.sh passes its own), else `<case dir>/work`.
fn work_arg(work: Option<PathBuf>, case_dir: Option<PathBuf>) -> Result<PathBuf, String> {
    match work {
        Some(w) => Ok(w),
        None => case_dir_arg(case_dir).map(|d| d.join("work")),
    }
}

fn case_dir_arg(case_dir: Option<PathBuf>) -> Result<PathBuf, String> {
    case_dir.or_else(|| std::env::var("SDC_CASE_DIR").ok().map(PathBuf::from)).ok_or("no --case-dir and no SDC_CASE_DIR".into())
}

/// `check-coverage`: exit 0 complete, 1 refused (stdout names the ids), 2 when it cannot check.
fn run_check_coverage(work: Option<PathBuf>, case_dir: Option<PathBuf>, story: Option<String>, tree: &Path, reply: Option<PathBuf>, shape: bool) -> Result<u8, String> {
    let story = story_arg(story)?;
    let reqs = story_requirements(&work_arg(work, case_dir)?, &story)?;
    let ids: Vec<String> = reqs.iter().filter_map(|r| r["id"].as_str().map(String::from)).collect();
    let file = reply.unwrap_or(tree.join(COVERAGE));
    let doc = read_json(&file);
    if shape {
        let why = coverage_shape(&doc, &ids);
        if why.is_empty() {
            return Ok(0);
        }
        let name = file.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let what = if doc.is_null() { format!("no readable coverage document ({name}: the structured reply, else out/coverage.json)") } else { why.join("; ") };
        eprintln!("invalid coverage reply: {what}");
        return Ok(2);
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
        println!("requirement coverage of story {story}: {} built and cited ({}), tests {}", ids.len(),
            if ids.is_empty() { "the story carries no requirement".into() } else { ids.join(" ") },
            if tests.is_empty() { "none".into() } else { tests.into_iter().collect::<Vec<_>>().join(" ") });
        return Ok(0);
    }
    print!("{}", refusal_text(&story, &faults));
    Ok(1)
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
            if kind == "review" {
                println!("{}", review_schema(&cwd.join("in")));
                return ExitCode::SUCCESS;
            }
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
        Cmd::ReviewInputs { cwd, case_dir, story } => match review_inputs(&cwd, &case_dir, &story) {
            Ok(v) => {
                println!("{v}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("sdc-reply review-inputs: {e}");
                ExitCode::from(2)
            }
        },
        Cmd::Adapters { tools } => {
            let (v, refused) = adapters(&tools, &|k| std::env::var(k).ok());
            println!("{v}");
            if refused.is_empty() {
                return ExitCode::SUCCESS;
            }
            for r in refused {
                eprintln!("refused: {r}");
            }
            ExitCode::from(64)
        }
        Cmd::CheckCoverage { work, case_dir, story, tree, reply, shape } => match run_check_coverage(work, case_dir, story, &tree, reply, shape) {
            Ok(c) => ExitCode::from(c),
            Err(e) => {
                eprintln!("sdc-reply check-coverage: {e}");
                ExitCode::from(2)
            }
        },
        Cmd::Coverage { work, case_dir, story, schema } => {
            let reqs = story_arg(story).and_then(|s| work_arg(work, case_dir).and_then(|d| story_requirements(&d, &s)));
            match reqs {
                Ok(r) => {
                    let ids: Vec<String> = r.iter().filter_map(|x| x["id"].as_str().map(String::from)).collect();
                    let list: Vec<Value> = r.iter().map(|x| json!({"id": x["id"], "text": x["text"]})).collect();
                    println!("{}", if schema { coverage_schema(&ids) } else { Value::Array(list) });
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("sdc-reply coverage: {e}");
                    ExitCode::from(2)
                }
            }
        }
        Cmd::CoverageStub { work, case_dir, story, tree } => {
            match story_arg(story).and_then(|s| work_arg(work, case_dir).and_then(|d| story_requirements(&d, &s))) {
                Ok(r) => {
                    let ids: Vec<String> = r.iter().filter_map(|x| x["id"].as_str().map(String::from)).collect();
                    println!("{}", coverage_stub(&tree, &ids));
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("sdc-reply coverage-stub: {e}");
                    ExitCode::from(2)
                }
            }
        }
        Cmd::CheckReview { cwd, case_dir, reply, principal } => {
            let r = read_json(&reply);
            match check_review(&cwd, &case_dir, &r, &principal) {
                Ok((true, v)) => {
                    println!("{v}");
                    ExitCode::SUCCESS
                }
                Ok((false, v)) => {
                    println!("{v}");
                    ExitCode::from(1)
                }
                Err(why) => {
                    let name = reply.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
                    let what = if r.is_null() { format!("no readable reply ({name}: the adapter's structured output, else out/review.json)") } else { why.join("; ") };
                    eprintln!("invalid review reply: {what}");
                    ExitCode::from(2)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |k| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
    }

    #[test]
    fn the_review_runs_on_claude_cli_when_the_writers_run_on_metaharness() {
        let tools = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/sdc/tools");
        let (v, refused) = adapters(&tools, &env_of(&[("SDC_LLM_ADAPTER", "metaharness")]));
        assert_eq!((v["review"]["adapter"].as_str(), v["review"]["json_schema"].as_bool()), (Some("claude-cli"), Some(true)));
        assert!(refused.is_empty());
        // Unset: the stub (no model); an explicit setting wins.
        assert_eq!(adapters(&tools, &env_of(&[])).0["review"]["adapter"], "stub");
        assert_eq!(adapters(&tools, &env_of(&[("SDC_LLM_ADAPTER", "metaharness"), ("SDC_REVIEW_ADAPTER", "claude-cli")])).0["review"]["source"], "SDC_REVIEW_ADAPTER");
    }

    #[test]
    fn a_schema_class_on_an_adapter_without_a_schema_is_refused() {
        let tools = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/sdc/tools");
        let (v, refused) = adapters(&tools, &env_of(&[("SDC_REVIEW_ADAPTER", "metaharness")]));
        assert_eq!(v["review"]["json_schema"], false);
        assert_eq!(refused.len(), 1);
        assert!(refused[0].starts_with("node class review (spec.review, change.review) replies under a response schema, but its adapter metaharness (SDC_REVIEW_ADAPTER) takes none"));
        // An adapter program that does not answer --capabilities takes none.
        assert_eq!(takes_schema("/bin/false"), Some(false));
    }

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
        let (ok, v) = check_review(&cwd, &case, &reply, "llm-review:m").unwrap();
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
        let check = |reply: Value| check_review(&cwd, &case, &reply, "p").unwrap_err().join("; ");
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

    #[test]
    fn an_approval_may_carry_outside_notes() {
        let (cwd, case) = call("approve");
        let reply = json!({"decision": "approved", "summary": "s", "findings": [finding("outside", "impl/README.md:1", "r3")]});
        let (ok, v) = check_review(&cwd, &case, &reply, "p").unwrap();
        assert!(ok);
        assert_eq!(v["notes"].as_array().unwrap().len(), 1);
    }
}
