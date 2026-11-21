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
//! change.review (tools/llm-node.sh): `review-inputs` writes the review's inputs into `in/`: the one
//! document `review.md` the model reads (the requirements the story delivers and those no story
//! covers, its acceptance lines, the covered commands, the test summary of the conformance run, the
//! diff against the mainline, the full text of only the files the diff touches, the build
//! constraints when the tree has them) and, for the checks, `scope.json`, `uncovered.json`,
//! `change.diff` and the touched files under `impl/`; it prints `document_bytes`;
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

/// The files a unified diff touches (its `+++ impl/<path>` lines), in order, once each.
fn changed_files(diff: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in diff.lines().filter_map(|l| l.strip_prefix("+++ impl/")).map(|l| l.split('\t').next().unwrap_or(l)) {
        if !out.iter().any(|o| o == p) {
            out.push(p.to_string());
        }
    }
    out
}

/// The diff, but a file the change creates (`@@ -0,0 +1,N @@`) keeps only its header and one line:
/// its full text follows in the document, so its lines would be there twice.
fn compact_diff(diff: &str) -> String {
    fn flush(section: &mut Vec<&str>, out: &mut String) {
        let created = section.iter().any(|l| l.starts_with("@@ -0,0 +1"));
        // The `diff -ruN ...` command line and the files' timestamps are not facts about the change.
        for l in section.iter().take_while(|l| !(created && l.starts_with("@@"))).filter(|l| !l.starts_with("diff ")) {
            out.push_str(if l.starts_with("--- ") || l.starts_with("+++ ") { l.split('\t').next().unwrap_or(l) } else { l });
            out.push('\n');
        }
        if created {
            out.push_str("(new file: every line is added; its full text is below)\n");
        }
        section.clear();
    }
    let mut out = String::new();
    let mut section: Vec<&str> = Vec::new();
    for l in diff.lines() {
        if l.starts_with("diff ") {
            flush(&mut section, &mut out);
        }
        section.push(l);
    }
    flush(&mut section, &mut out);
    out
}

/// A file's text with its line numbers, the numbers a finding's `at` cites.
fn number_lines(text: &str) -> String {
    let w = text.lines().count().to_string().len().max(3);
    text.lines().enumerate().map(|(i, l)| format!("{:>w$}  {l}\n", i + 1)).collect()
}

/// The counts and the failing names of a conformance and test log, not the log: the run's header
/// line, the scenario counts, the unit tests' totals, and every scenario or test that failed.
fn test_summary(log: &str) -> String {
    let scenarios = regex::Regex::new(r"^\d+ scenarios?: ").unwrap();
    let unit = regex::Regex::new(r"^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored").unwrap();
    let scenario_bad = regex::Regex::new(r"^(?:failed|error)\s+(\S+)").unwrap();
    let test_bad = regex::Regex::new(r"^test (\S+) \.\.\. (?:FAILED|failed)").unwrap();
    let (mut head, mut counts) = (None, None);
    let mut totals: Option<[u64; 3]> = None;
    let mut failing: Vec<String> = Vec::new();
    for l in log.lines() {
        let t = l.trim();
        if head.is_none() && t.contains(" against ") && (t.ends_with("— passed") || t.ends_with("— failed")) {
            head = Some(t.to_string());
        } else if counts.is_none() && scenarios.is_match(t) {
            counts = Some(t.to_string());
        } else if let Some(c) = unit.captures(t) {
            let n = |i: usize| c[i].parse::<u64>().unwrap_or(0);
            let s = totals.get_or_insert([0; 3]);
            (s[0], s[1], s[2]) = (s[0] + n(1), s[1] + n(2), s[2] + n(3));
        } else if let Some(c) = scenario_bad.captures(t).or_else(|| test_bad.captures(t)) {
            if !failing.contains(&c[1].to_string()) {
                failing.push(c[1].to_string());
            }
        }
    }
    let mut out = String::new();
    if let Some(h) = head {
        out += &format!("conformance: {h}\n");
    }
    if let Some(c) = counts {
        out += &format!("scenarios: {c}\n");
    }
    if let Some([p, f, i]) = totals {
        out += &format!("unit tests: {p} passed, {f} failed, {i} ignored\n");
    }
    if out.is_empty() {
        out += "no counts found in the run's log\n";
    }
    out += &format!("failing: {}\n", if failing.is_empty() { "none".to_string() } else { failing.join(", ") });
    out
}

/// What the review is given, as facts: nothing of the tree beyond `files`, the files the diff touches.
struct ReviewFacts<'a> {
    story: &'a str,
    /// `mainline` when the diff is against the mainline tree, else `empty` (the first story).
    base: &'a str,
    /// `{cite, text}`: the summary line and the lines of the `## Acceptance` section.
    acceptance: &'a [Value],
    /// `{id, text, cites}`: the requirements the story delivers.
    requirements: &'a [Value],
    /// The requirements no story of the plan covers.
    uncovered: &'a [Value],
    /// The compiled specification's entries of the commands the story covers.
    commands: &'a Map<String, Value>,
    /// The text of the tree's build constraints, when it has them.
    constraints: Option<&'a str>,
    /// `test_summary` of the conformance run on the revision under review.
    tests: &'a str,
    diff: &'a str,
    /// The touched files: path under `impl/` and the text, None for a file the change deletes.
    files: &'a [(String, Option<String>)],
}

/// The one document the review reads: the story's requirements and commands, the test summary, the
/// diff and the full text of every file the diff touches.
fn review_document(f: &ReviewFacts) -> String {
    let line = |v: &Value, k: &str| format!("- {}: {}\n", v[k].as_str().unwrap_or(""), v["text"].as_str().unwrap_or("").trim());
    let mut d = format!("# Review input: story `{}`\n\nThis is everything you are given. Judge the change from it alone.\n", f.story);
    d += "\n## Requirements this story delivers (a violation names the id)\n";
    d += &f.requirements.iter().map(|r| line(r, "id")).collect::<String>();
    d += "\n## Acceptance lines (a violation names the cite)\n";
    d += &f.acceptance.iter().map(|a| line(a, "cite")).collect::<String>();
    d += "\n## Commands it must serve (a violation names the command; the compiled specification)\n";
    for (name, v) in f.commands {
        d += &format!("- {name}: {v}\n");
    }
    d += "\n## Requirements no story of the plan covers (kind `outside`; never a reason to reject)\n";
    d += &if f.uncovered.is_empty() { "none\n".to_string() } else { f.uncovered.iter().map(|r| line(r, "id")).collect() };
    if let Some(c) = f.constraints {
        d += &format!("\n## Build constraints\n{}\n", c.trim_end());
    }
    d += &format!("\n## Test summary (the conformance run that passed on this revision)\n{}", f.tests);
    d += &format!("\n## Change against the {} tree (unified diff)\n~~~diff\n{}~~~\n", f.base, compact_diff(f.diff));
    d += "\n## Changed files, full text (the line numbers are the ones `at` cites; a file the diff does not touch is not shown)\n";
    for (path, text) in f.files {
        match text {
            Some(t) => d += &format!("\n### impl/{path} ({} lines)\n~~~\n{}~~~\n", t.lines().count(), number_lines(t)),
            None => d += &format!("\n### impl/{path}\nDeleted by this change.\n"),
        }
    }
    d
}

/// Write the review's inputs into `<cwd>/in/` and return what was written. `review.md` is the
/// document the model reads (`review_document`): the requirements the story delivers and the
/// commands it covers, its acceptance lines, the test summary, the diff against the mainline (empty
/// tree for the first story) and the full text of only the files the diff touches, plus the
/// build constraints when the tree has them. Beside it, for the programs only: `scope.json` and
/// `uncovered.json` (what a finding may name), `change.diff` and the touched files under `impl/`
/// (what `at` may cite). No other file of the tree, no other story, no log.
fn review_inputs(cwd: &Path, case_dir: &Path, story: &str) -> Result<Value, String> {
    let work = case_dir.join("work");
    let in_dir = cwd.join("in");
    let rel = format!("aep/story/{story}.md");
    let text = fs::read_to_string(work.join(&rel)).map_err(|e| format!("{rel}: {e}"))?;
    fs::create_dir_all(&in_dir).map_err(|e| e.to_string())?;
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
    let commands: Map<String, Value> = cov.iter().map(|c| (c.clone(), ir["commands"][c].clone())).collect();
    let w = |name: &str, v: &Value| fs::write(in_dir.join(name), serde_json::to_string_pretty(v).unwrap_or_default()).map_err(|e| e.to_string());
    w("scope.json", &json!({"story": story, "covers": cov, "acceptance": acceptance, "requirements": mine,
        "traced_by": if traces.is_null() { "spec_domains (no spec/traces.json)" } else { "spec/traces.json" }}))?;
    w("uncovered.json", &json!({"about": "requirements no story of the plan covers: outside this story", "requirements": uncovered}))?;
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
    let changed = changed_files(&diff);
    let mut files: Vec<(String, Option<String>)> = Vec::new();
    for p in &changed {
        let src = work.join("impl").join(p);
        if p.contains("..") || !src.is_file() {
            files.push((p.clone(), None));
            continue;
        }
        let dst = in_dir.join("impl").join(p);
        fs::create_dir_all(dst.parent().unwrap_or(&in_dir)).map_err(|e| e.to_string())?;
        fs::copy(&src, &dst).map_err(|e| format!("impl/{p}: {e}"))?;
        files.push((p.clone(), Some(String::from_utf8_lossy(&fs::read(&src).map_err(|e| e.to_string())?).into_owned())));
    }
    let case = read_json(&case_dir.join("case.json"));
    let rev = |a: &str| case["artifacts"][a]["revision"].as_str().unwrap_or("none").to_string();
    // sdc.story (revision 7) admits change.review only once the conformance run passed at the
    // revision under review, so its log is always there; without it the review has no input.
    let tests = record_log(case_dir, "conformance_run", &rev("change")).ok_or(format!(
        "no conformance run recorded on change revision {} (sdc.story reviews only after tests.pass)", rev("change")))?;
    let gate = record_log(case_dir, "ess_gate", &rev("spec"));
    let tests = test_summary(&tests);
    let constraints = fs::read_to_string(work.join("build-constraints.yaml")).ok();
    let doc = review_document(&ReviewFacts { story, base: if base.ends_with("mainline") { "mainline" } else { "empty" },
        acceptance: &acceptance, requirements: &mine, uncovered: &uncovered, commands: &commands,
        constraints: constraints.as_deref(), tests: &tests, diff: &diff, files: &files });
    fs::write(in_dir.join("review.md"), &doc).map_err(|e| e.to_string())?;
    Ok(json!({"story": story, "covers": cov, "requirements": mine.iter().map(|r| r["id"].clone()).collect::<Vec<_>>(),
        "uncovered": uncovered.iter().map(|r| r["id"].clone()).collect::<Vec<_>>(), "changed_files": changed,
        "gate": gate.is_some(), "base": if base.ends_with("mainline") { "mainline" } else { "empty" },
        "build_constraints": constraints.is_some(), "document": "in/review.md", "document_bytes": doc.len()}))
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
    let changed: BTreeSet<String> = changed_files(&fs::read_to_string(c.in_dir.join("change.diff")).unwrap_or_default()).into_iter().collect();
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

    #[test]
    fn an_approval_may_carry_outside_notes() {
        let (cwd, case) = call("approve");
        let reply = json!({"decision": "approved", "summary": "s", "findings": [finding("outside", "impl/README.md:1", "r3")]});
        let (ok, v) = check_review(&cwd, &case, &reply, "p").unwrap();
        assert!(ok);
        assert_eq!(v["notes"].as_array().unwrap().len(), 1);
    }

    /// A case of two stories (tasks covers todo.task.AddTask, sync covers todo.sync.SyncChanges) and
    /// three requirements (r1 tasks, r2 sync, r3 nobody). The change under review edits
    /// `src/lib.rs` and adds `src/new.rs`; `src/untouched.rs` and `README.md` are not in the diff.
    fn review_case(name: &str, tests_log: &str) -> (PathBuf, PathBuf) {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-scratch/review-inputs").join(name);
        let _ = fs::remove_dir_all(&root);
        let (cwd, case) = (root.join("call"), root.join("case"));
        let w = case.join("work");
        for d in ["aep/story", "impl/src", "mainline/src", ".sdc/gate", "../evidence", "../blobs"] {
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
            put(w.join(d).join("src/untouched.rs"), "pub fn UNTOUCHED_MARKER() {}\n");
            put(w.join(d).join("README.md"), "UNTOUCHED_README\n");
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
        let v = review_inputs(&cwd, &case, "tasks").unwrap();
        let doc = fs::read_to_string(cwd.join("in/review.md")).unwrap();
        assert_eq!(v["changed_files"], json!(["src/lib.rs", "src/new.rs"]));
        assert!(doc.contains("### impl/src/lib.rs") && doc.contains("pub fn remove()"), "{doc}");
        assert!(doc.contains("### impl/src/new.rs") && doc.contains("brand_new"), "{doc}");
        // A file outside the diff is in neither the document nor in/.
        assert!(!doc.contains("UNTOUCHED_MARKER") && !doc.contains("UNTOUCHED_README"), "{doc}");
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
        let v = review_inputs(&cwd, &case, "tasks").unwrap();
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
        review_inputs(&cwd, &case, "tasks").unwrap();
        let doc = fs::read_to_string(cwd.join("in/review.md")).unwrap();
        assert!(doc.contains("2 scenarios: 2 passed, 0 failed, 0 error, 0 unsupported"), "{doc}");
        assert!(doc.contains("unit tests: 2 passed, 0 failed, 0 ignored") && doc.contains("failing: none"), "{doc}");
        assert!(!doc.contains("passed todo.task.AddTask/outcome/added") && !doc.contains("test tests::add"), "{doc}");
        assert!(!cwd.join("in/tests.txt").exists() && !cwd.join("in/gate.txt").exists());
        let failing = PASSING_LOG.replace("  passed todo.task.AddTask/outcome/refused", "  failed todo.task.AddTask/outcome/refused")
            .replace("test tests::remove ... ok", "test tests::remove ... FAILED");
        let (cwd, case) = review_case("tests-failing", &failing);
        review_inputs(&cwd, &case, "tasks").unwrap();
        let doc = fs::read_to_string(cwd.join("in/review.md")).unwrap();
        assert!(doc.contains("failing: todo.task.AddTask/outcome/refused, tests::remove"), "{doc}");
    }

    #[test]
    fn the_build_constraints_are_in_the_document_when_the_tree_has_them() {
        let (cwd, case) = review_case("no-constraints", PASSING_LOG);
        assert_eq!(review_inputs(&cwd, &case, "tasks").unwrap()["build_constraints"], false);
        assert!(!fs::read_to_string(cwd.join("in/review.md")).unwrap().contains("Build constraints"));
        let (cwd, case) = review_case("constraints", PASSING_LOG);
        fs::write(case.join("work/build-constraints.yaml"), "language: rust\n").unwrap();
        assert_eq!(review_inputs(&cwd, &case, "tasks").unwrap()["build_constraints"], true);
        assert!(fs::read_to_string(cwd.join("in/review.md")).unwrap().contains("## Build constraints\nlanguage: rust"));
    }

    #[test]
    fn changed_files_are_the_diffs_plus_files_once_each() {
        let diff = "diff -ruN a/x b/x\n--- mainline/src/a.rs\t1\n+++ impl/src/a.rs\t2\n@@ -1 +1 @@\n-x\n+y\ndiff -ruN a/y b/y\n--- mainline/b.md\t1\n+++ impl/b.md\t2\n";
        assert_eq!(changed_files(diff), vec!["src/a.rs", "b.md"]);
        assert!(changed_files("").is_empty());
    }

    #[test]
    fn a_created_file_keeps_its_header_and_not_its_lines_in_the_diff() {
        let diff = "diff -ruN m/a i/a\n--- mainline/a.rs\t1\n+++ impl/a.rs\t2\n@@ -1 +1,2 @@\n x\n+kept\ndiff -ruN m/n i/n\n--- mainline/n.rs\t1\n+++ impl/n.rs\t2\n@@ -0,0 +1,2 @@\n+gone1\n+gone2\n";
        let c = compact_diff(diff);
        assert!(c.contains("+kept") && c.contains("+++ impl/n.rs") && c.contains("(new file"), "{c}");
        assert!(!c.contains("gone1") && !c.contains("gone2"), "{c}");
    }

    #[test]
    fn the_test_summary_has_counts_and_failing_names_only() {
        let log = "Requires an empty namespace.\ntodo v1 against interpreted 0.96.0 — failed\n  passed a/outcome/x\n  failed todo.task.AddTask/outcome/added\n  error todo.sync.Send/outcome/e\n\
            3 scenarios: 1 passed, 1 failed, 1 error, 0 unsupported\nrunning 2 tests\ntest tests::ok ... ok\ntest tests::bad ... FAILED\n\
            test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured\nrunning 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured\n";
        let s = test_summary(log);
        assert_eq!(s, "conformance: todo v1 against interpreted 0.96.0 — failed\nscenarios: 3 scenarios: 1 passed, 1 failed, 1 error, 0 unsupported\n\
            unit tests: 1 passed, 1 failed, 0 ignored\nfailing: todo.task.AddTask/outcome/added, todo.sync.Send/outcome/e, tests::bad\n");
        assert!(test_summary("nothing\n").starts_with("no counts found"));
    }
}
