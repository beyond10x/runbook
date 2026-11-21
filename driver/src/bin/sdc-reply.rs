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
//!   sdc-reply implement-run --rec <call record> --cwd <call dir> --case-dir <case> --story <story>
//!                     --adapter <adapter program> [--rework-limit N] [--assemble-only]
//!   sdc-reply implement-pack --tree <dir> [--note <text>]
//!   sdc-reply schema --cwd <call dir> --case-dir <case> --type implement
//!   sdc-reply rule-approval --request <approval request file> --ends <rule-ends.json>
//!
//! change.implement as one request (`SDC_IMPLEMENT_MODE=request`, sdc-reply/implement.rs,
//! docs/driver.md "Implement as one request"): `implement-run` assembles the node's whole input
//! into one prompt, calls the adapter, checks the reply against the schema and the path rule,
//! retries an invalid reply at most twice with the check's error, continues by rule for paths a
//! valid reply left `remaining`, and writes the files into `<cwd>/out/impl/`. `adapters` adds the
//! class `implement` (node change.implement) in request mode: SDC_IMPLEMENT_ADAPTER, else as the
//! review's; an SDC_IMPLEMENT_MODE other than session or request is refused.
//!
//! `rule-approval` (docs/driver.md, "Approvals by program"): whether `rule:autopilot` takes an
//! approval because the program checks guarding it (`approvals.<action>.by_rule` in rule-ends.json)
//! recorded a passing result for the exact revision being approved. It reads the claims from the
//! request and re-reads the evidence on disk. Prints `{take: false, covered, why}` (exit 0) or
//! `{take: true, rule, decision}`: the decision line for decisions.jsonl, citing the evidence ids.
//! An action with no rule prints `covered: false`; exit 64 only when an input is unreadable.
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

#[path = "sdc-reply/implement.rs"]
mod implement;

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
    /// change.implement as one request (SDC_IMPLEMENT_MODE=request): assemble the input, ask,
    /// check the reply, retry or continue by rule, write the files into `<cwd>/out/impl/`.
    ImplementRun {
        #[arg(long)] rec: PathBuf,
        #[arg(long)] cwd: PathBuf,
        #[arg(long)] case_dir: PathBuf,
        #[arg(long)] story: String,
        #[arg(long)] adapter: Option<PathBuf>,
        #[arg(long)] rework_limit: Option<u64>,
        #[arg(long)] assemble_only: bool,
        #[arg(long, default_value_t = implement::INLINE_CAP)] max_bytes: usize,
    },
    /// A change.implement reply holding every product file of a tree (the fixture adapter).
    ImplementPack { #[arg(long)] tree: PathBuf, #[arg(long, default_value = "")] note: String },
    /// Whether an approval request is taken by `rule:autopilot` (its program checks passed on the
    /// exact revision) instead of the decider model; prints `{take, why | decision, rule}`.
    RuleApproval { #[arg(long)] request: PathBuf, #[arg(long)] ends: PathBuf },
}

/// The principal of an approval taken by the rule.
const RULE_PRINCIPAL: &str = "rule:autopilot";

/// Every `(claim id, is)` a condition names.
fn claim_conditions(f: &Value, out: &mut Vec<(String, String)>) {
    match f {
        Value::Object(m) => {
            if let Some(c) = m.get("claim").filter(|c| c.is_object()) {
                out.push((c["id"].as_str().unwrap_or("").into(), c["is"].as_str().unwrap_or("true").into()));
            }
            m.values().for_each(|x| claim_conditions(x, out));
        }
        Value::Array(a) => a.iter().for_each(|x| claim_conditions(x, out)),
        _ => {}
    }
}

/// Whether binding `b` is a program check of the evidence `w` (a claim's `true_when.evidence`):
/// bound, not a person's, not a model's, producing that kind about that subject.
fn program_check(b: &Value, w: &Value) -> Result<(), String> {
    if !b.is_object() {
        return Err("it has no binding".into());
    }
    if b["manual"] == true {
        return Err("it is a person's action, not a program check".into());
    }
    let llm_argv = b["argv"].as_array().is_some_and(|a| a.iter().any(|x| x.as_str().is_some_and(|s| s.ends_with("/llm-node.sh"))));
    if !b["llm"].is_null() || llm_argv {
        return Err("it is run by a model, not a program check".into());
    }
    let kind = w["kind"].as_str().unwrap_or("");
    let produces = match &b["produces"] {
        Value::String(k) => vec![k.as_str()],
        Value::Array(a) => a.iter().filter_map(|k| k.as_str()).collect(),
        _ => vec![],
    };
    if !produces.contains(&kind) {
        return Err(format!("it does not produce {kind} (it produces {})", if produces.is_empty() { "nothing".into() } else { produces.join(", ") }));
    }
    if b["subject"] != w["subject"] {
        return Err(format!("it checks {}, the claim is about {}", b["subject"], w["subject"]));
    }
    Ok(())
}

/// The decision of `rule:autopilot` on an approval, or why the decider is asked: the data file
/// (rule-ends.json `approvals.<protocol>/<action>.by_rule`) names, for each claim the approval
/// rests on, the program check that records it. Each guard needs the claim true in the request's
/// position and a passing record of the check about the exact revision now (the evidence the
/// claim's `true_when` names, produced by that check, not set aside, with the upstream revisions it
/// was observed against still current), and no earlier rejection of this approval may exist.
fn rule_checks(req: &Value, ends: &Value) -> Result<(String, String, Vec<Value>), String> {
    let case_dir = PathBuf::from(req["case_dir"].as_str().ok_or("the request names no case_dir")?);
    let action = req["action"].as_str().ok_or("the request names no action")?;
    let (ir, bindings, case) = (read_json(&case_dir.join("ir.json")), read_json(&case_dir.join("bindings.json")), read_json(&case_dir.join("case.json")));
    let proto = ir["protocol"]["id"].as_str().or(ir["protocol"].as_str()).or(case["protocol"].as_str()).unwrap_or("");
    let key = format!("{proto}/{action}");
    let rule = &ends["approvals"][&key]["by_rule"];
    let guards = rule["guards"].as_array().filter(|g| !g.is_empty()).ok_or_else(|| format!("no rule takes {key}: it has no program check to rest on"))?;
    let revision = req["revision"].as_str().unwrap_or("");
    let position = &req["position"];
    // Every claim the action requires true is true now.
    let mut cond = Vec::new();
    claim_conditions(&ir["actions"][action]["precondition"], &mut cond);
    for (c, _) in cond.iter().filter(|(_, is)| is == "true") {
        let v = position["claims"][c]["value"].as_str().unwrap_or("unknown");
        if v != "true" {
            return Err(format!("{c} is {v}, not true"));
        }
    }
    let mut checks = Vec::new();
    for g in guards {
        let (claim, check) = (g["claim"].as_str().unwrap_or(""), g["check"].as_str().unwrap_or(""));
        if !cond.iter().any(|(c, is)| c == claim && is == "true") {
            return Err(format!("the rule's guard {claim} is not a condition of {action}"));
        }
        let w = &ir["claims"][claim]["true_when"]["evidence"];
        program_check(&bindings[check], w).map_err(|e| format!("{check}, which should record {claim}: {e}"))?;
        let (kind, subject) = (w["kind"].as_str().unwrap_or(""), w["subject"].as_str().unwrap_or(""));
        let current = case["artifacts"][subject]["revision"].as_str().unwrap_or("none");
        if req["subject"] == subject && current != revision {
            return Err(format!("the request is about {subject}@{revision}, the case is at {current}"));
        }
        let aside: Vec<&str> = position["claims"][claim]["excluded_evidence"].as_array().into_iter().flatten()
            .filter_map(|x| x["evidence"].as_str()).collect();
        let mut files: Vec<PathBuf> = fs::read_dir(case_dir.join("evidence")).into_iter().flatten().filter_map(|e| e.ok().map(|e| e.path())).collect();
        files.sort();
        let found = files.iter().map(|p| read_json(p)).rfind(|r| {
            let id = r["id"].as_str().unwrap_or("");
            r["kind"] == kind && r["subject"] == subject && r["result"] == w["result"] && r["subject_revision"] == current
                && id.ends_with(&format!("-{check}")) && !aside.contains(&id)
                && r["upstream_revisions"].as_object().into_iter().flatten()
                    .all(|(a, v)| case["artifacts"][a]["revision"].is_null() || &case["artifacts"][a]["revision"] == v)
        });
        let Some(r) = found else {
            return Err(format!("{check} has recorded no passing {kind} ({}) about {subject}@{current}", w["result"].as_str().unwrap_or("any result")));
        };
        checks.push(json!({"claim": claim, "check": check, "evidence": r["id"], "kind": kind, "result": r["result"],
            "subject": subject, "revision": current}));
    }
    let prior = rejections(&case_dir, action);
    if prior.as_array().is_some_and(|p| !p.is_empty()) {
        return Err(format!("an earlier rejection of {action} exists ({}): the decider checks the rework against it", prior[0]["record"].as_str().unwrap_or("?")));
    }
    Ok((key, rule["rule"].as_str().unwrap_or("program-checks").to_string(), checks))
}

/// `sdc-reply rule-approval`: see `Cmd::RuleApproval`. `{take: true, decision, rule}` where
/// `decision` is a decider reply (decisions.jsonl's shape) and `rule` names the rule, the checks and
/// the evidence ids; `{take: false, why}` leaves the approval to the decider.
fn rule_approval(req: &Value, ends: &Value) -> Value {
    match rule_checks(req, ends) {
        Err(why) => json!({"take": false, "covered": !why.starts_with("no rule takes"), "why": why}),
        Ok((key, name, checks)) => {
            let ids: Vec<&Value> = checks.iter().map(|c| &c["evidence"]).collect();
            let said: Vec<String> = checks.iter().map(|c| format!("{} by {} on {} ({}={} about {}@{})", c["claim"].as_str().unwrap_or(""),
                c["check"].as_str().unwrap_or(""), c["evidence"].as_str().unwrap_or(""), c["kind"].as_str().unwrap_or(""),
                c["result"].as_str().unwrap_or("recorded"), c["subject"].as_str().unwrap_or(""), c["revision"].as_str().unwrap_or(""))).collect();
            let rule = json!({"principal": RULE_PRINCIPAL, "rule": name, "approval": key, "checks": checks});
            json!({"take": true, "rule": rule, "decision": {"decision": "approve", "answer": null, "best": null,
                "reasons": [format!("taken by {RULE_PRINCIPAL} ({name}): the program checks passed on the exact revision: {}; no earlier rejection; no model was asked", said.join("; "))],
                "cites": ids, "kind": "rule", "model": null, "adapter": "rule", "usage": null, "cost_usd": 0, "valid": true, "dry": false,
                "rule": rule}})
        }
    }
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
    // change.implement as one request (SDC_IMPLEMENT_MODE=request): its reply is the files under a
    // response schema, so it needs a schema adapter like the review; session mode adds nothing.
    match env("SDC_IMPLEMENT_MODE").filter(|m| !m.is_empty()).as_deref() {
        None | Some("session") => {}
        Some("request") => {
            let (a, src) = class_adapter("implement", env);
            let schema = takes_schema(&a);
            if schema == Some(false) {
                refused.push(format!("node class implement (change.implement) replies under a response schema in SDC_IMPLEMENT_MODE=request, \
                    but its adapter {a} ({src}) takes none: set SDC_IMPLEMENT_ADAPTER=claude-cli"));
            }
            out.insert("implement".into(), json!({"adapter": a, "source": src, "json_schema": schema, "nodes": ["change.implement"], "mode": "request"}));
        }
        Some(m) => refused.push(format!("SDC_IMPLEMENT_MODE={m}: neither session nor request")),
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
fn review_inputs(cwd: &Path, case_dir: &Path, story: &str) -> Result<Value, String> {
    let work = case_dir.join("work");
    let in_dir = cwd.join("in");
    let rel = format!("aep/story/{story}.md");
    let text = fs::read_to_string(work.join(&rel)).map_err(|e| format!("{rel}: {e}"))?;
    fs::create_dir_all(in_dir.join("aep/story")).map_err(|e| e.to_string())?;
    fs::write(in_dir.join(&rel), &text).map_err(|e| e.to_string())?;
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
    w("commands.json", &Value::Object(commands))?;
    copy_tree(&work.join("impl"), &in_dir.join("impl")).map_err(|e| format!("impl: {e}"))?;
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
        "gate": gate.is_some(), "base": if base.ends_with("mainline") { "mainline" } else { "empty" }}))
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
            if kind == "implement" {
                println!("{}", implement::schema());
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
        Cmd::ImplementRun { rec, cwd, case_dir, story, adapter, rework_limit, assemble_only, max_bytes } => {
            let (code, line) = implement::run(&implement::RunArgs { rec, cwd, case_dir, story, adapter, rework_limit, assemble_only, cap: max_bytes });
            println!("{line}");
            ExitCode::from(code)
        }
        Cmd::ImplementPack { tree, note } => {
            println!("{}", implement::pack(&tree, &note));
            ExitCode::SUCCESS
        }
        Cmd::RuleApproval { request, ends } => {
            let (req, ends) = (read_json(&request), read_json(&ends));
            if req.is_null() || ends.is_null() {
                eprintln!("sdc-reply rule-approval: the request or the rule ends are not readable JSON");
                return ExitCode::from(64);
            }
            println!("{}", rule_approval(&req, &ends));
            ExitCode::SUCCESS
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

    #[test]
    fn request_mode_puts_change_implement_on_a_schema_adapter() {
        let tools = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/sdc/tools");
        // Session mode (unset or `session`): no implement entry, the adapters line as before.
        assert!(adapters(&tools, &env_of(&[("SDC_LLM_ADAPTER", "metaharness")])).0.get("implement").is_none());
        assert!(adapters(&tools, &env_of(&[("SDC_IMPLEMENT_MODE", "session")])).0.get("implement").is_none());
        let (v, refused) = adapters(&tools, &env_of(&[("SDC_LLM_ADAPTER", "metaharness"), ("SDC_IMPLEMENT_MODE", "request")]));
        assert_eq!(v["implement"], json!({"adapter": "claude-cli", "source": "default: claude-cli, SDC_LLM_ADAPTER is metaharness (no response schema)",
            "json_schema": true, "nodes": ["change.implement"], "mode": "request"}));
        assert!(refused.is_empty());
        let (_, refused) = adapters(&tools, &env_of(&[("SDC_IMPLEMENT_MODE", "request"), ("SDC_IMPLEMENT_ADAPTER", "metaharness")]));
        assert_eq!(refused.len(), 1);
        assert!(refused[0].starts_with("node class implement (change.implement) replies under a response schema"), "{}", refused[0]);
        let (_, refused) = adapters(&tools, &env_of(&[("SDC_IMPLEMENT_MODE", "both")]));
        assert_eq!(refused, vec!["SDC_IMPLEMENT_MODE=both: neither session nor request"]);
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

    // ---- rule-approval ---------------------------------------------------------------------------

    fn repo(p: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(p)
    }

    /// A case dir of protocol `proto` (its compiled IR and bindings from the repository) with the
    /// given artifact revisions and evidence records `(id, kind, result, subject, subject_revision)`.
    fn rule_case(name: &str, proto: &str, revisions: Value, evidence: &[(&str, &str, Value, &str, &str)]) -> PathBuf {
        let case = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-scratch/rule-approval").join(name);
        let _ = fs::remove_dir_all(&case);
        fs::create_dir_all(case.join("evidence")).unwrap();
        fs::copy(repo(&format!("protocol/compiled/{proto}.ir.json")), case.join("ir.json")).unwrap();
        fs::copy(repo(&format!("driver/examples/sdc/{proto}.bindings.json")), case.join("bindings.json")).unwrap();
        let arts: Map<String, Value> = revisions.as_object().unwrap().iter().map(|(k, v)| (k.clone(), json!({"revision": v}))).collect();
        fs::write(case.join("case.json"), json!({"id": "IDEA-1.x", "protocol": proto.replacen('-', ".", 1), "artifacts": arts}).to_string()).unwrap();
        for (id, kind, result, subject, rev) in evidence {
            fs::write(case.join("evidence").join(format!("{id}.json")), json!({"format": "canon-evidence/1", "id": id, "kind": kind,
                "result": result, "subject": subject, "subject_revision": rev, "upstream_revisions": null}).to_string()).unwrap();
        }
        case
    }

    fn request(case: &Path, action: &str, subject: &str, revision: &str, claims: &[(&str, &str)]) -> Value {
        let claims: Map<String, Value> = claims.iter().map(|(k, v)| (k.to_string(), json!({"value": v}))).collect();
        json!({"type": "approval", "case_dir": case, "action": action, "subject": subject, "revision": revision,
            "position": {"claims": claims, "open_obligations": []}})
    }

    fn ends() -> Value {
        read_json(&repo("driver/examples/sdc/rule-ends.json"))
    }

    fn context_case(name: &str, evidence: &[(&str, &str, Value, &str, &str)]) -> (PathBuf, Value) {
        let case = rule_case(name, "sdc-direction", json!({"context": "t1", "requirements": "r1"}), evidence);
        let req = request(&case, "context.approve", "context", "t1", &[("context.valid", "true"), ("context.decided", "unknown")]);
        (case, req)
    }

    #[test]
    fn a_passed_check_at_the_exact_revision_is_taken_by_the_rule() {
        let (_, req) = context_case("taken", &[("e0004-context.check", "context_defaults", json!("valid"), "context", "t1")]);
        let v = rule_approval(&req, &ends());
        assert_eq!(v["take"], true, "{v}");
        assert_eq!(v["decision"]["decision"], "approve");
        assert_eq!(v["decision"]["model"], Value::Null);
        assert_eq!(v["decision"]["cost_usd"], 0);
        assert_eq!(v["decision"]["cites"], json!(["e0004-context.check"]));
        let r = &v["decision"]["rule"];
        assert_eq!(r["principal"], "rule:autopilot");
        assert_eq!(r["checks"][0]["check"], "context.check");
        assert_eq!(r["checks"][0]["claim"], "context.valid");
        assert_eq!(r["checks"][0]["evidence"], "e0004-context.check");
        assert_eq!(r["checks"][0]["revision"], "t1");
    }

    #[test]
    fn a_failed_missing_or_stale_check_leaves_it_to_the_decider() {
        // The check ran and found something: no passing record.
        let (_, req) = context_case("failed", &[("e0004-context.check", "context_defaults", json!("finding"), "context", "t1")]);
        let v = rule_approval(&req, &ends());
        assert_eq!(v["take"], false, "{v}");
        assert!(v["why"].as_str().unwrap().contains("context.check"), "{v}");
        assert_eq!(v["covered"], true, "a rule exists, it declined: the line says why");
        // No record at all.
        let (_, req) = context_case("missing", &[]);
        assert_eq!(rule_approval(&req, &ends())["take"], false);
        // The only passing record is about an earlier revision: the revision changed after the check.
        let (_, req) = context_case("stale", &[("e0004-context.check", "context_defaults", json!("valid"), "context", "t0")]);
        let v = rule_approval(&req, &ends());
        assert_eq!(v["take"], false, "{v}");
        assert!(v["why"].as_str().unwrap().contains("t1"), "{v}");
        // The request's revision is not the case's current one.
        let (case, _) = context_case("moved", &[("e0004-context.check", "context_defaults", json!("valid"), "context", "t1")]);
        let req = request(&case, "context.approve", "context", "t0", &[("context.valid", "true")]);
        assert_eq!(rule_approval(&req, &ends())["take"], false);
        // A passing record Canon set aside is no record either.
        let (case, mut req) = context_case("excluded", &[("e0004-context.check", "context_defaults", json!("valid"), "context", "t1")]);
        req["position"]["claims"]["context.valid"]["excluded_evidence"] = json!([{"evidence": "e0004-context.check", "reason": "invalidated"}]);
        assert_eq!(rule_approval(&req, &ends())["take"], false, "{case:?}");
    }

    #[test]
    fn the_claims_the_action_requires_must_be_true() {
        let (case, _) = context_case("claim", &[("e0004-context.check", "context_defaults", json!("valid"), "context", "t1")]);
        let req = request(&case, "context.approve", "context", "t1", &[("context.valid", "unknown")]);
        let v = rule_approval(&req, &ends());
        assert_eq!(v["take"], false, "{v}");
        assert!(v["why"].as_str().unwrap().contains("context.valid"), "{v}");
    }

    #[test]
    fn an_earlier_rejection_is_asked_of_the_decider() {
        let (case, req) = context_case("rejected", &[("e0004-context.check", "context_defaults", json!("valid"), "context", "t1"),
            ("e0009-context.approve", "context_approval", json!("rejected"), "context", "t0")]);
        fs::write(case.join("events.jsonl"), json!({"event": "action_finished", "action": "context.approve", "evidence": ["e0009-context.approve"]}).to_string() + "\n").unwrap();
        let v = rule_approval(&req, &ends());
        assert_eq!(v["take"], false, "{v}");
        assert!(v["why"].as_str().unwrap().contains("rejection"), "{v}");
    }

    #[test]
    fn a_check_that_is_a_model_or_a_person_is_no_program_check() {
        let (case, req) = context_case("model", &[("e0004-context.check", "context_defaults", json!("valid"), "context", "t1")]);
        for (patch, word) in [(json!({"manual": true}), "person"), (json!({"llm": {"model": "m"}}), "model"),
            (json!({"produces": "other_kind"}), "other_kind")] {
            let mut b = read_json(&case.join("bindings.json"));
            for (k, x) in patch.as_object().unwrap() {
                b["context.check"][k] = x.clone();
            }
            fs::write(case.join("bindings.json"), b.to_string()).unwrap();
            let v = rule_approval(&req, &ends());
            assert_eq!(v["take"], false, "{v}");
            assert!(v["why"].as_str().unwrap().contains(word), "{word}: {v}");
            fs::copy(repo("driver/examples/sdc/sdc-direction.bindings.json"), case.join("bindings.json")).unwrap();
        }
    }

    #[test]
    fn the_vision_has_no_program_check_and_is_never_taken() {
        let (case, _) = context_case("vision", &[("e0004-context.check", "context_defaults", json!("valid"), "context", "t1")]);
        let req = request(&case, "vision.approve", "vision", "v1", &[("context.valid", "true"), ("context.approved", "true")]);
        let v = rule_approval(&req, &ends());
        assert_eq!(v["take"], false, "{v}");
        assert!(v["why"].as_str().unwrap().contains("no rule"), "{v}");
        assert_eq!(v["covered"], false);
        assert!(ends()["approvals"]["sdc.direction/vision.approve"]["by_rule"].is_null());
    }

    #[test]
    fn the_roadmap_is_taken_on_its_validation() {
        let case = rule_case("roadmap", "sdc-direction", json!({"context": "t1", "vision": "v1", "roadmap": "m1"}),
            &[("e0008-roadmap.validate", "roadmap_validation", json!("valid"), "roadmap", "m1")]);
        let claims = [("context.valid", "true"), ("vision.approved", "true"), ("roadmap.valid", "true"), ("roadmap.decided", "unknown")];
        let v = rule_approval(&request(&case, "roadmap.approve", "roadmap", "m1", &claims), &ends());
        assert_eq!(v["take"], true, "{v}");
        assert_eq!(v["decision"]["rule"]["checks"][0]["check"], "roadmap.validate");
    }

    #[test]
    fn the_release_needs_its_gate_its_build_and_its_acceptance() {
        let all = [("e0004-spec.gate", "ess_gate", json!("passed"), "spec", "s1"),
            ("e0008-release.build", "build_provenance", Value::Null, "release", "d1"),
            ("e0013-acceptance.run", "release_conformance", json!("passed"), "release", "d1")];
        let claims = [("gate.passed", "true"), ("release.built", "true"), ("release.accepted", "true")];
        let case = rule_case("release", "sdc-release", json!({"spec": "s1", "release": "d1"}), &all);
        let v = rule_approval(&request(&case, "release.publish", "release", "d1", &claims), &ends());
        assert_eq!(v["take"], true, "{v}");
        let checks: Vec<&str> = v["decision"]["rule"]["checks"].as_array().unwrap().iter().filter_map(|c| c["check"].as_str()).collect();
        assert_eq!(checks, vec!["spec.gate", "release.build", "acceptance.run"]);
        assert_eq!(v["decision"]["cites"], json!(["e0004-spec.gate", "e0008-release.build", "e0013-acceptance.run"]));
        // The conformance run failed: the decider is asked.
        let bad = [all[0].clone(), all[1].clone(), ("e0013-acceptance.run", "release_conformance", json!("failed"), "release", "d1")];
        let case = rule_case("release-failed", "sdc-release", json!({"spec": "s1", "release": "d1"}), &bad);
        let v = rule_approval(&request(&case, "release.publish", "release", "d1", &claims), &ends());
        assert_eq!(v["take"], false, "{v}");
        assert!(v["why"].as_str().unwrap().contains("acceptance.run"), "{v}");
        // The release was rebuilt after its acceptance run: that run is about the old revision.
        let case = rule_case("release-stale", "sdc-release", json!({"spec": "s1", "release": "d2"}), &all);
        let v = rule_approval(&request(&case, "release.publish", "release", "d2", &claims), &ends());
        assert_eq!(v["take"], false, "{v}");
    }

    #[test]
    fn every_rule_in_the_data_names_a_program_check_of_its_protocol() {
        // The verification the data file stands on: for each approval with `by_rule`, every guard's
        // claim is a condition of the action, and the check is a bound program action (no model,
        // no person) producing that claim's evidence kind about that claim's subject.
        let e = ends();
        let mut seen = 0;
        for (qa, end) in e["approvals"].as_object().unwrap() {
            let Some(rule) = end.get("by_rule").filter(|r| !r.is_null()) else { continue };
            let (proto, action) = qa.split_once('/').unwrap();
            let name = proto.replacen("sdc.", "sdc-", 1);
            let ir = read_json(&repo(&format!("protocol/compiled/{name}.ir.json")));
            let b = read_json(&repo(&format!("driver/examples/sdc/{name}.bindings.json")));
            let mut cond = Vec::new();
            claim_conditions(&ir["actions"][action]["precondition"], &mut cond);
            assert!(rule["guards"].as_array().is_some_and(|g| !g.is_empty()), "{qa}: no guards");
            for g in rule["guards"].as_array().unwrap() {
                let (claim, check) = (g["claim"].as_str().unwrap(), g["check"].as_str().unwrap());
                assert!(cond.iter().any(|(c, is)| c == claim && is == "true"), "{qa}: {claim} is no condition of {action}");
                let w = &ir["claims"][claim]["true_when"]["evidence"];
                assert!(program_check(&b[check], w).is_ok(), "{qa}: {check}: {:?}", program_check(&b[check], w));
                seen += 1;
            }
        }
        assert!(seen >= 5, "the data covers context, roadmap and release: {seen} guards");
    }
}
