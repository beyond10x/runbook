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
//!   sdc-reply coverage [--work <work>] [--case-dir <case>] [--story <story>] [--schema]
//!   sdc-reply check-coverage [--work <work>] [--case-dir <case>] [--story <story>] --tree <tree> [--reply <file>] [--shape]
//!   sdc-reply coverage-stub [--work <work>] [--case-dir <case>] [--story <story>] --tree <tree>
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
//! change.review (tools/llm-node.sh): `review-inputs` writes the review's inputs into `in/`. With
//! SDC_REVIEW_MODE unset or `session` (the default) they are the story, its acceptance lines, the
//! requirements it delivers and those no story covers, the covered commands, the whole change tree,
//! its diff against the mainline and the gate and test logs. With `facts` (story:review-minimal-facts)
//! they are the one document `review.md` the model reads (the requirements the story delivers and
//! those no story covers, its acceptance lines, the covered commands, the test summary of the
//! conformance run, the diff against the mainline, the full text of only the files the diff touches,
//! the build constraints when the tree has them, a listing of the other product files) and, for the
//! checks, `scope.json`, `uncovered.json`, `change.diff`, `listing.json` and the touched files under
//! `impl/`; it prints `document_bytes`;
//! `schema --type review` prints the reply schema; `check-review` checks the reply: exit 0
//! approved, 1 rejected (stdout: the sdc.rejection/1 that sends the implementer back), 2 invalid
//! (stderr: why; nothing is recorded).
//!
//! Where the facts come from (`Facts`): the old driver's case directory (`case.json`, the event
//! log, the evidence and the blobs) or, with `LOOM_CASE` set, record files of the workspace
//! (`<case-dir>/work/.sdc/records/`; Loom gives a bound program its working directory and nothing
//! else). The revision of an artifact is then the digest of its tree (`impl`, `spec`); the log of
//! the conformance run and of the gate is `<kind>/<digest>.log`, written by `sdc-reply record
//! --kind <kind> --tree <tree> --log <file>` from the tools of tests.run and spec.gate; the
//! review's own rejections are `rejections/change.review/<n>.json`, written by `check-review`.
//! `citable`, `resolve`, `schema`, `rejections`, `review-inputs` and `check-review` read them
//! through the same functions in both modes; no `<artifact>@<revision>` cite resolves under Loom.
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
    /// Under Loom: keep the log of a step (a file) as the record of `kind` about the tree `tree`
    /// of the workspace, where `review-inputs` reads it (the tools of tests.run and spec.gate).
    Record {
        #[arg(long, default_value = ".")] work: PathBuf,
        #[arg(long)] kind: String,
        #[arg(long)] tree: String,
        #[arg(long)] log: PathBuf,
    },
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

/// Where the facts about a case come from. Under the old driver (`Driver`) they are the driver's
/// case directory: `case.json` (the revisions), the event log, the evidence and the blobs. Under
/// Loom (`LOOM_CASE` set; `Workspace`) a bound program gets its working directory, the workspace,
/// and nothing else, so the same facts are small record files below `<work>/.sdc/records/`, each
/// written by the tool that owns the fact: `<kind>/<digest>.log` (`sdc-reply record`, called by the
/// tools of tests.run and spec.gate: the log of the step, about the tree whose content has that
/// digest) and `rejections/<action>/<n>.json` (the review's own rejections, written by
/// `check-review`). The revision of an artifact is then the digest of its tree.
struct Facts {
    dir: PathBuf,
    work: PathBuf,
    workspace: bool,
}

/// The records directory below a workspace.
const RECORDS: &str = ".sdc/records";

impl Facts {
    /// The source the environment names: the workspace's records when `LOOM_CASE` is set.
    fn open(case_dir: &Path) -> Facts {
        let loom = std::env::var_os("LOOM_CASE").is_some_and(|v| !v.is_empty());
        Facts { dir: case_dir.to_path_buf(), work: case_dir.join("work"), workspace: loom }
    }

    #[cfg(test)]
    fn driver(case_dir: &Path) -> Facts {
        Facts { dir: case_dir.to_path_buf(), work: case_dir.join("work"), workspace: false }
    }

    /// The records of the workspace `work` (no case directory).
    fn workspace(work: &Path) -> Facts {
        Facts { dir: work.join(".."), work: work.to_path_buf(), workspace: true }
    }

    fn work(&self) -> PathBuf {
        self.work.clone()
    }

    fn records(&self) -> PathBuf {
        self.work.join(RECORDS)
    }

    /// The case's id: `case.json`, else the id Loom names in `LOOM_CASE`.
    fn case_id(&self) -> Value {
        if self.workspace {
            return std::env::var("LOOM_CASE").map(Value::String).unwrap_or(Value::Null);
        }
        read_json(&self.dir.join("case.json"))["id"].clone()
    }

    /// The tree of a case artifact, relative to the work directory (the artifacts of sdc.story).
    fn tree_of(artifact: &str) -> Option<&'static str> {
        match artifact {
            "change" => Some("impl"),
            "spec" => Some("spec"),
            _ => None,
        }
    }

    /// The current revision of an artifact: `case.json`'s, else the digest of its tree.
    fn revision(&self, artifact: &str) -> String {
        self.revision_value(artifact).as_str().unwrap_or("none").to_string()
    }

    /// The same as JSON: null where the driver's `case.json` has none.
    fn revision_value(&self, artifact: &str) -> Value {
        if !self.workspace {
            return read_json(&self.dir.join("case.json"))["artifacts"][artifact]["revision"].clone();
        }
        Self::tree_of(artifact).and_then(|t| tree_digest(&self.work().join(t))).map_or(Value::Null, Value::String)
    }

    /// The log of the last record of `kind` about the artifact's current revision.
    fn record_log(&self, kind: &str, artifact: &str) -> Option<String> {
        let revision = self.revision(artifact);
        if !self.workspace {
            return record_log(&self.dir, kind, &revision);
        }
        fs::read_to_string(self.records().join(kind).join(format!("{revision}.log"))).ok()
    }

    /// The rejected records of `action`, oldest first (`rejections`'s shape).
    fn rejections(&self, action: &str) -> Value {
        if !self.workspace {
            return rejections(&self.dir, action);
        }
        let dir = self.records().join("rejections").join(action);
        let mut files: Vec<PathBuf> = fs::read_dir(dir).into_iter().flatten().filter_map(|e| e.ok().map(|e| e.path())).collect();
        files.sort();
        Value::Array(files.iter().map(|p| read_json(p)).filter(|v| !v.is_null()).collect())
    }

    /// Keep a rejection `doc` (sdc.rejection/1) of `action` where `rejections` finds it. The driver
    /// keeps its own record of an action's result, so under it nothing is written.
    fn note_rejection(&self, action: &str, doc: &Value) -> Result<(), String> {
        if !self.workspace {
            return Ok(());
        }
        let dir = self.records().join("rejections").join(action);
        fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let n = fs::read_dir(&dir).map_or(0, |d| d.flatten().count()) + 1;
        let entry = json!({"record": format!("{n:03}"), "revision": doc["revision"], "at": now_utc(),
            "principal": doc["principal"], "reasons": doc["reasons"], "citations": doc["citations"]});
        let p = dir.join(format!("{n:03}.json"));
        fs::write(&p, entry.to_string()).map_err(|e| format!("{}: {e}", p.display()))
    }

    /// Store the log of a step as the record of `kind` about the tree `tree` (relative to the
    /// work directory) in its present content. Returns where it went.
    fn write_record(&self, kind: &str, tree: &str, log: &str) -> Result<PathBuf, String> {
        if kind.contains('/') || kind.contains("..") || kind.is_empty() {
            return Err(format!("{kind:?} is no record kind"));
        }
        let digest = tree_digest(&self.work().join(tree)).ok_or(format!("{tree}: no such tree in the workspace"))?;
        let dir = self.records().join(kind);
        fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let p = dir.join(format!("{digest}.log"));
        fs::write(&p, log).map_err(|e| format!("{}: {e}", p.display()))?;
        Ok(p)
    }

    /// The artifacts of the case as (name, file in the work directory, revision): the driver's
    /// bindings file names them; Loom's workspace has no such file, so none resolve there.
    fn artifacts(&self) -> Vec<(String, String, String)> {
        if self.workspace {
            return Vec::new();
        }
        let bindings = read_json(&self.dir.join("bindings.json"));
        let case = read_json(&self.dir.join("case.json"));
        bindings["$artifacts"].as_object().into_iter().flatten()
            .filter_map(|(a, f)| Some((a.clone(), f.as_str()?.to_string(), case["artifacts"][a]["revision"].as_str()?.to_string())))
            .collect()
    }
}

/// The digest of a tree's content: its files by relative path, without build output (`target`),
/// `.sdc` and `.derived-from`, as the review's diff leaves them out. None when `dir` is no directory.
fn tree_digest(dir: &Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    if !dir.is_dir() {
        return None;
    }
    let mut files = Vec::new();
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let Ok(rd) = fs::read_dir(dir.join(&rel)) else { continue };
        for e in rd.flatten() {
            let name = e.file_name();
            if matches!(name.to_str(), Some("target" | ".sdc" | ".derived-from")) {
                continue;
            }
            let r = rel.join(&name);
            if e.path().is_dir() { stack.push(r) } else { files.push(r) }
        }
    }
    files.sort();
    let mut h = Sha256::new();
    for f in &files {
        let body = fs::read(dir.join(f)).unwrap_or_default();
        h.update(f.to_string_lossy().as_bytes());
        h.update([0]);
        h.update(Sha256::digest(&body));
    }
    Some(h.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect())
}

/// The present time as `YYYY-MM-DDTHH:MM:SSZ`.
fn now_utc() -> String {
    let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
    let (days, rest) = (s.div_euclid(86400), s.rem_euclid(86400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rest / 3600, rest % 3600 / 60, rest % 60)
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

fn ctx(cwd: &Path, facts: &Facts) -> Ctx {
    let in_dir = cwd.join("in");
    let work = facts.work();
    let mut artifacts = Vec::new();
    for (a, f, rev) in facts.artifacts() {
        if rev != "none" && in_dir.join(&f).exists() {
            artifacts.push((a, f, rev));
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

/// One product file the diff does not touch: its path under `impl/`, its size, and the names of the
/// items it declares at top level when a rule of its language finds them.
struct Listed {
    path: String,
    bytes: usize,
    items: Option<Vec<String>>,
}

/// What the listing is allowed to cost: item-bearing lines up to this many bytes, then paths only.
const LISTING_CAP: usize = 4096;
/// Items named per file; the rest are counted.
const LISTING_ITEMS: usize = 24;
/// A source file larger than this is listed by path and size only.
const LISTING_READ_MAX: usize = 512 * 1024;

/// The names a source file declares at top level (column 0), by a simple rule per language, plus the
/// routes and event listeners it registers (`GET /tasks`, `/ping`, `on fetch`), in file order, once
/// each. None when no rule applies to the file's extension (a page, a manifest, a document).
fn top_level_items(path: &str, text: &str) -> Option<Vec<String>> {
    let ext = path.rsplit_once('.')?.1.to_ascii_lowercase();
    let decl = match ext.as_str() {
        "rs" => r"^(?:pub(?:\([a-z: ]+\))?\s+)?(?:(?:async|unsafe|const)\s+)*(?:fn|struct|enum|trait|mod|type|static|const)\s+([A-Za-z_]\w*)",
        "js" | "mjs" | "cjs" | "jsx" | "ts" | "tsx" | "mts" | "cts" =>
            r"^(?:export\s+)?(?:default\s+)?(?:async\s+)?(?:function\*?|class|const|let|var|interface|type|enum)\s+([A-Za-z_$][\w$]*)",
        "py" => r"^(?:async\s+)?(?:def|class)\s+([A-Za-z_]\w*)",
        "go" => r"^(?:func\s+(?:\([^)]*\)\s*)?|type\s+|var\s+|const\s+)([A-Za-z_]\w*)",
        "sh" | "bash" => r"^(?:function\s+([A-Za-z_][\w-]*)|([A-Za-z_][\w-]*)\s*\(\))",
        _ => return None,
    };
    let decl = regex::Regex::new(decl).ok()?;
    let method = regex::Regex::new(r#"\.(get|post|put|patch|delete|head|options)\(\s*["'`](/[^"'`]*)["'`]"#).ok()?;
    let route = regex::Regex::new(r#"(?:\.route|HandleFunc|Handle)\(\s*["'`](/[^"'`]*)["'`]"#).ok()?;
    let listener = regex::Regex::new(r#"addEventListener\(\s*["'](\w+)["']"#).ok()?;
    let mut items: Vec<String> = Vec::new();
    let mut add = |s: String| {
        if !items.contains(&s) {
            items.push(s)
        }
    };
    for l in text.lines() {
        if let Some(c) = decl.captures(l) {
            if let Some(n) = c.iter().skip(1).flatten().next() {
                add(n.as_str().to_string());
            }
        }
        for c in method.captures_iter(l) {
            add(format!("{} {}", c[1].to_ascii_uppercase(), &c[2]));
        }
        for c in route.captures_iter(l) {
            add(c[1].to_string());
        }
        for c in listener.captures_iter(l) {
            add(format!("on {}", &c[1]));
        }
    }
    Some(items)
}

/// The product files the diff does not touch under `dir`, sorted by path: not build output, not the
/// gate's state, not version control or installed packages.
fn product_files(dir: &Path, changed: &[String]) -> Vec<Listed> {
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let Ok(rd) = fs::read_dir(dir.join(&rel)) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if ["target", ".sdc", ".git", ".derived-from", "node_modules"].contains(&name.as_str()) {
                continue;
            }
            let Ok(t) = e.file_type() else { continue };
            let r = rel.join(&name);
            if t.is_dir() {
                stack.push(r);
            } else if t.is_file() {
                let p = r.to_string_lossy().into_owned();
                if changed.contains(&p) {
                    continue;
                }
                let bytes = e.metadata().map(|m| m.len() as usize).unwrap_or(0);
                let items = if bytes <= LISTING_READ_MAX { fs::read(e.path()).ok().and_then(|b| top_level_items(&p, &String::from_utf8_lossy(&b))) } else { None };
                out.push(Listed { path: p, bytes, items });
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// The listing as the document's section: one line per file, path and size and, for a source file,
/// its items. The lines with sizes and items stop at `cap` bytes; the files after that are paths only.
fn listing_section(files: &[Listed], cap: usize) -> String {
    let mut d = String::from("\n## Product files not in the diff (path, size, and for source files the top-level items it declares; a file neither listed here nor in the diff does not exist in the tree; `at` may cite a listed file by path alone)\n");
    if files.is_empty() {
        d += "none\n";
    }
    let (mut used, mut paths_only) = (0, false);
    for f in files {
        let mut line = format!("- {} ({} bytes)", f.path, f.bytes);
        if let Some(items) = f.items.as_ref().filter(|i| !i.is_empty()) {
            let more = items.len().saturating_sub(LISTING_ITEMS);
            line += &format!(": {}{}", items.iter().take(LISTING_ITEMS).cloned().collect::<Vec<_>>().join(", "), if more > 0 { format!(", +{more} more") } else { String::new() });
        }
        line += "\n";
        if !paths_only && used + line.len() > cap {
            paths_only = true;
            d += &format!("(the listing is capped at {cap} bytes: the files below are paths only)\n");
        }
        if paths_only {
            d += &format!("- {}\n", f.path);
        } else {
            used += line.len();
            d += &line;
        }
    }
    d
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
    /// Every other product file of the tree, by path, size and top-level items.
    listing: &'a [Listed],
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
    d += "\n## Changed files, full text (the line numbers are the ones `at` cites; a file the diff does not touch is only listed at the end)\n";
    for (path, text) in f.files {
        match text {
            Some(t) => d += &format!("\n### impl/{path} ({} lines)\n~~~\n{}~~~\n", t.lines().count(), number_lines(t)),
            None => d += &format!("\n### impl/{path}\nDeleted by this change.\n"),
        }
    }
    d + &listing_section(f.listing, LISTING_CAP)
}

/// SDC_REVIEW_MODE: `session` (unset or empty, the default: main's review, a session with read
/// tools over the whole change tree) or `facts` (story:review-minimal-facts: one request on one
/// document of facts). True for facts.
fn review_mode(v: Option<&str>) -> Result<bool, String> {
    match v.unwrap_or("") {
        "" | "session" => Ok(false),
        "facts" => Ok(true),
        m => Err(format!("SDC_REVIEW_MODE={m}: neither session nor facts")),
    }
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

/// The log of the conformance run on the change revision under review, and the gate's log on the
/// specification revision (None without one). sdc.story (revision 7) admits change.review only once
/// the conformance run passed at the revision under review, so its log is always there; without it
/// the review has no input.
fn run_logs(facts: &Facts) -> Result<(String, Option<String>), String> {
    let tests = facts.record_log("conformance_run", "change").ok_or(format!(
        "no conformance run recorded on change revision {} (sdc.story reviews only after tests.pass)", facts.revision("change")))?;
    Ok((tests, facts.record_log("ess_gate", "spec")))
}

/// The session review's inputs (SDC_REVIEW_MODE=session, the default), as before
/// story:review-minimal-facts: written into `<cwd>/in/` and returned. The story, its
/// acceptance lines (the summary line and the lines of an `## Acceptance` section), the
/// requirements it delivers and those no story covers, the covered commands' IR entries, the
/// change tree, its diff against the mainline (empty for the first story), the gate and test logs.
fn review_inputs_session(cwd: &Path, facts: &Facts, story: &str) -> Result<Value, String> {
    let work = facts.work();
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
    let (tests, gate) = run_logs(facts)?;
    fs::write(in_dir.join("tests.txt"), &tests).map_err(|e| e.to_string())?;
    fs::write(in_dir.join("gate.txt"), gate.clone().unwrap_or("no gate run recorded on this specification revision\n".into()))
        .map_err(|e| e.to_string())?;
    let changed: Vec<&str> = diff.lines().filter_map(|l| l.strip_prefix("+++ impl/")).map(|l| l.split('\t').next().unwrap_or(l)).collect();
    let mut out = json!({"story": story, "covers": cov, "requirements": mine.iter().map(|r| r["id"].clone()).collect::<Vec<_>>(),
        "uncovered": uncovered.iter().map(|r| r["id"].clone()).collect::<Vec<_>>(), "changed_files": changed,
        "gate": gate.is_some(), "base": if base.ends_with("mainline") { "mainline" } else { "empty" }});
    // `coverage` only when the tree carries the document (SDC_IMPLEMENT_COVERAGE=on): without it
    // the output is main's.
    if has_coverage {
        out["coverage"] = json!(true);
    }
    Ok(out)
}

/// Write the review's inputs into `<cwd>/in/` and return what was written. `review.md` is the
/// document the model reads (`review_document`): the requirements the story delivers and the
/// commands it covers, its acceptance lines, the test summary, the diff against the mainline (empty
/// tree for the first story) and the full text of only the files the diff touches, plus the
/// build constraints when the tree has them, and a listing of every other product file (path, size,
/// top-level items; `listing_section`) so that what the tree lacks can be seen. Beside it, for the
/// programs only: `scope.json` and `uncovered.json` (what a finding may name), `change.diff` and the
/// touched files under `impl/` (the lines `at` may cite), `listing.json` (the files `at` may cite by
/// path alone). No other file's text, no other story, no log.
fn review_inputs(cwd: &Path, facts: &Facts, story: &str) -> Result<Value, String> {
    let work = facts.work();
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
    let listing = product_files(&work.join("impl"), &changed);
    w("listing.json", &json!({"about": "product files the diff does not touch: what `at` may cite by path alone",
        "files": listing.iter().map(|f| json!({"path": f.path, "bytes": f.bytes, "items": f.items})).collect::<Vec<_>>()}))?;
    let (tests, gate) = run_logs(facts)?;
    let tests = test_summary(&tests);
    let constraints = fs::read_to_string(work.join("build-constraints.yaml")).ok();
    let doc = review_document(&ReviewFacts { story, base: if base.ends_with("mainline") { "mainline" } else { "empty" },
        acceptance: &acceptance, requirements: &mine, uncovered: &uncovered, commands: &commands,
        constraints: constraints.as_deref(), tests: &tests, diff: &diff, files: &files, listing: &listing });
    fs::write(in_dir.join("review.md"), &doc).map_err(|e| e.to_string())?;
    Ok(json!({"story": story, "covers": cov, "requirements": mine.iter().map(|r| r["id"].clone()).collect::<Vec<_>>(),
        "uncovered": uncovered.iter().map(|r| r["id"].clone()).collect::<Vec<_>>(), "changed_files": changed,
        "listed_files": listing.len(), "gate": gate.is_some(), "base": if base.ends_with("mainline") { "mainline" } else { "empty" },
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
/// change tree (`impl/<path>:<line>[-<line>]`) and what it violates. A facts review (its inputs
/// have `listing.json`) may also cite a listed or changed file by path alone.
fn review_schema(in_dir: &Path) -> Value {
    let (inside, outside) = review_targets(in_dir);
    let mut all = inside;
    all.extend(outside);
    let at = if in_dir.join("listing.json").is_file() { "^impl/[^:]+(:[0-9]+(-[0-9]+)?)?$" } else { "^impl/[^:]+:[0-9]+(-[0-9]+)?$" };
    reply_schema(vec![
        ("decision", json!({"type": "string", "enum": ["approved", "rejected"]})),
        ("summary", json!({"type": "string"})),
        ("findings", json!({"type": "array", "items": reply_schema(vec![
            ("kind", json!({"type": "string", "enum": ["violation", "outside"]})),
            ("at", json!({"type": "string", "pattern": at})),
            ("violates", json!({"type": "string", "enum": all})),
            ("says", json!({"type": "string", "minLength": 1})),
        ])})),
    ])
}

/// The review reply checked by program. Ok((approved, rejection or note)), or Err(reasons) when
/// the reply is invalid: an unknown decision, a finding whose place is not a line of the change
/// tree, a violation of something outside the story's set, an `outside` finding about a
/// requirement a story covers, an empty `says`, or a rejection with no violation finding.
fn check_review(cwd: &Path, source: &Facts, reply: &Value, principal: &str) -> Result<(bool, Value), Vec<String>> {
    let c = ctx(cwd, source);
    let (inside, outside) = review_targets(&c.in_dir);
    let changed: BTreeSet<String> = changed_files(&fs::read_to_string(c.in_dir.join("change.diff")).unwrap_or_default()).into_iter().collect();
    // A facts review (SDC_REVIEW_MODE=facts) has listing.json: a listed file may be cited by path.
    let facts = c.in_dir.join("listing.json").is_file();
    let listed: BTreeSet<String> = read_json(&c.in_dir.join("listing.json"))["files"].as_array().into_iter().flatten()
        .filter_map(|f| f["path"].as_str().map(String::from)).collect();
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
        // A file by path alone: a product file the listing names, or a changed file.
        let by_path = at.strip_prefix("impl/").filter(|p| facts && !p.contains(':'));
        let r = if by_path.is_some() { json!({"ok": true}) } else { resolve(&c, at) };
        if let Some(p) = by_path {
            if p.contains("..") || !(listed.contains(p) || changed.contains(p)) {
                why.push(format!("finding {n}: {at:?} is neither a listed product file nor a changed file (cite a listed file by path, or <path>:<line> of a changed file)"));
            }
        } else if !at.starts_with("impl/") || r["ok"] != true {
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
        let rest = at.strip_prefix("impl/").unwrap_or("");
        let file = rest.rsplit_once(':').map_or(rest, |(p, _)| p);
        let line = format!("{at}: {says} ({} {v})", if kind == "outside" { "outside this story:" } else { "violates" });
        if kind == "outside" { notes.push(line) } else { reasons.push(line) }
        let mut citation = json!({"at": at, "violates": v, "kind": kind, "in_diff": changed.contains(file)});
        if facts {
            citation["listed"] = json!(listed.contains(file));
        }
        citations.push(citation);
    }
    if decision == "rejected" && reasons.is_empty() {
        why.push("a rejection names no violation finding: reject only for a requirement, acceptance line or command of this story".into());
    }
    if !why.is_empty() {
        return Err(why);
    }
    if decision == "approved" {
        return Ok((true, json!({"approved": true, "principal": principal, "notes": notes, "citations": citations})));
    }
    let count = source.rejections("change.review").as_array().map_or(0, |a| a.len()) + 1;
    let rejection = json!({"format": "sdc.rejection/1", "approval": "change.review", "case": source.case_id(), "subject": "change",
        "revision": source.revision_value("change"), "principal": principal, "reasons": reasons,
        "notes": notes, "citations": citations, "count": count, "summary": reply["summary"]});
    source.note_rejection("change.review", &rejection).map_err(|e| vec![format!("the rejection could not be kept: {e}")])?;
    Ok((false, rejection))
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
            println!("{}", citable_set(&ctx(&cwd, &Facts::open(&case_dir))));
            ExitCode::SUCCESS
        }
        Cmd::Resolve { cwd, case_dir, cites } => {
            let c = ctx(&cwd, &Facts::open(&case_dir));
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
            println!("{}", decider_schema(&ctx(&cwd, &Facts::open(&case_dir)), &kind, r#final, &opts, rule));
            ExitCode::SUCCESS
        }
        Cmd::Rejections { case_dir, action } => {
            println!("{}", Facts::open(&case_dir).rejections(&action));
            ExitCode::SUCCESS
        }
        Cmd::Record { work, kind, tree, log } => match fs::read_to_string(&log).map_err(|e| format!("{}: {e}", log.display()))
            .and_then(|text| Facts::workspace(&work).write_record(&kind, &tree, &text)) {
            Ok(p) => {
                println!("{}", p.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("sdc-reply record: {e}");
                ExitCode::from(2)
            }
        },
        Cmd::Later { ir, action } => {
            let ir = read_json(&ir);
            if ir["actions"].get(&action).is_none() {
                eprintln!("sdc-reply later: no action `{action}` in the protocol");
                return ExitCode::from(64);
            }
            println!("{}", later(&ir, &action));
            ExitCode::SUCCESS
        }
        Cmd::ReviewInputs { cwd, case_dir, story } => match review_mode(std::env::var("SDC_REVIEW_MODE").ok().as_deref())
            .and_then(|facts| {
                let source = Facts::open(&case_dir);
                if facts { review_inputs(&cwd, &source, &story) } else { review_inputs_session(&cwd, &source, &story) }
            }) {
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
            match check_review(&cwd, &Facts::open(&case_dir), &r, &principal) {
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
        let tools = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bindings/tools");
        let (v, refused) = adapters(&tools, &env_of(&[("SDC_LLM_ADAPTER", "metaharness")]));
        assert_eq!((v["review"]["adapter"].as_str(), v["review"]["json_schema"].as_bool()), (Some("claude-cli"), Some(true)));
        assert!(refused.is_empty());
        // Unset: the stub (no model); an explicit setting wins.
        assert_eq!(adapters(&tools, &env_of(&[])).0["review"]["adapter"], "stub");
        assert_eq!(adapters(&tools, &env_of(&[("SDC_LLM_ADAPTER", "metaharness"), ("SDC_REVIEW_ADAPTER", "claude-cli")])).0["review"]["source"], "SDC_REVIEW_ADAPTER");
    }

    #[test]
    fn a_schema_class_on_an_adapter_without_a_schema_is_refused() {
        let tools = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bindings/tools");
        let (v, refused) = adapters(&tools, &env_of(&[("SDC_REVIEW_ADAPTER", "metaharness")]));
        assert_eq!(v["review"]["json_schema"], false);
        assert_eq!(refused.len(), 1);
        assert!(refused[0].starts_with("node class review (spec.review, change.review) replies under a response schema, but its adapter metaharness (SDC_REVIEW_ADAPTER) takes none"));
        // An adapter program that does not answer --capabilities takes none.
        assert_eq!(takes_schema("/bin/false"), Some(false));
    }

    #[test]
    fn request_mode_puts_change_implement_on_a_schema_adapter() {
        let tools = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bindings/tools");
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
        fs::write(cwd.join("in/listing.json"), json!({"files": [{"path": "src/server.rs", "bytes": 60}, {"path": "public/index.html", "bytes": 28}]}).to_string()).unwrap();
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
        let (ok, v) = check_review(&cwd, &Facts::driver(&case), &reply, "llm-review:m").unwrap();
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
        let check = |reply: Value| check_review(&cwd, &Facts::driver(&case), &reply, "p").unwrap_err().join("; ");
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
        let (ok, v) = check_review(&cwd, &Facts::driver(&case), &reply, "p").unwrap();
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

    // ---- review-inputs ---------------------------------------------------------------------------

    /// A case of two stories (tasks covers todo.task.AddTask, sync covers todo.sync.SyncChanges) and
    /// three requirements (r1 tasks, r2 sync, r3 nobody). The change under review edits
    /// `src/lib.rs` and adds `src/new.rs`; `src/untouched.rs` and `README.md` are not in the diff.
    fn review_case(name: &str, tests_log: &str) -> (PathBuf, PathBuf) {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-scratch/review-inputs").join(name);
        let _ = fs::remove_dir_all(&root);
        let (cwd, case) = (root.join("call"), root.join("case"));
        let w = case.join("work");
        for d in ["aep/story", "impl/src", "impl/public", "mainline/src", "mainline/public", ".sdc/gate", "../evidence", "../blobs"] {
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
            put(w.join(d).join("src/untouched.rs"), "pub fn untouched_fn() {\n    let UNTOUCHED_BODY = 1;\n}\n");
            put(w.join(d).join("README.md"), "UNTOUCHED_README\n");
            put(w.join(d).join("src/server.rs"), "use std::io;\npub struct Server;\npub fn serve() {\n    router.get(\"/api/tasks\");\n}\nfn private_helper() {}\n");
            put(w.join(d).join("public/index.html"), "<html>UNTOUCHED_HTML</html>\n");
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
        let v = review_inputs(&cwd, &Facts::driver(&case), "tasks").unwrap();
        let doc = fs::read_to_string(cwd.join("in/review.md")).unwrap();
        assert_eq!(v["changed_files"], json!(["src/lib.rs", "src/new.rs"]));
        assert!(doc.contains("### impl/src/lib.rs") && doc.contains("pub fn remove()"), "{doc}");
        assert!(doc.contains("### impl/src/new.rs") && doc.contains("brand_new"), "{doc}");
        // A file outside the diff is in neither the document nor in/.
        assert!(!doc.contains("UNTOUCHED_BODY") && !doc.contains("UNTOUCHED_README") && !doc.contains("UNTOUCHED_HTML"), "{doc}");
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
        let v = review_inputs(&cwd, &Facts::driver(&case), "tasks").unwrap();
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
        review_inputs(&cwd, &Facts::driver(&case), "tasks").unwrap();
        let doc = fs::read_to_string(cwd.join("in/review.md")).unwrap();
        assert!(doc.contains("2 scenarios: 2 passed, 0 failed, 0 error, 0 unsupported"), "{doc}");
        assert!(doc.contains("unit tests: 2 passed, 0 failed, 0 ignored") && doc.contains("failing: none"), "{doc}");
        assert!(!doc.contains("passed todo.task.AddTask/outcome/added") && !doc.contains("test tests::add"), "{doc}");
        assert!(!cwd.join("in/tests.txt").exists() && !cwd.join("in/gate.txt").exists());
        let failing = PASSING_LOG.replace("  passed todo.task.AddTask/outcome/refused", "  failed todo.task.AddTask/outcome/refused")
            .replace("test tests::remove ... ok", "test tests::remove ... FAILED");
        let (cwd, case) = review_case("tests-failing", &failing);
        review_inputs(&cwd, &Facts::driver(&case), "tasks").unwrap();
        let doc = fs::read_to_string(cwd.join("in/review.md")).unwrap();
        assert!(doc.contains("failing: todo.task.AddTask/outcome/refused, tests::remove"), "{doc}");
    }

    #[test]
    fn the_build_constraints_are_in_the_document_when_the_tree_has_them() {
        let (cwd, case) = review_case("no-constraints", PASSING_LOG);
        assert_eq!(review_inputs(&cwd, &Facts::driver(&case), "tasks").unwrap()["build_constraints"], false);
        assert!(!fs::read_to_string(cwd.join("in/review.md")).unwrap().contains("Build constraints"));
        let (cwd, case) = review_case("constraints", PASSING_LOG);
        fs::write(case.join("work/build-constraints.yaml"), "language: rust\n").unwrap();
        assert_eq!(review_inputs(&cwd, &Facts::driver(&case), "tasks").unwrap()["build_constraints"], true);
        assert!(fs::read_to_string(cwd.join("in/review.md")).unwrap().contains("## Build constraints\nlanguage: rust"));
    }

    #[test]
    fn an_absent_file_is_detectable_from_the_listing() {
        let (cwd, case) = review_case("listing", PASSING_LOG);
        let v = review_inputs(&cwd, &Facts::driver(&case), "tasks").unwrap();
        let doc = fs::read_to_string(cwd.join("in/review.md")).unwrap();
        let (_, listing) = doc.split_once("## Product files not in the diff").unwrap_or_else(|| panic!("no listing: {doc}"));
        let listing = listing.split("\n## ").next().unwrap();
        // The untouched files, with their size and for source files their items; the web page by path and size.
        assert!(listing.contains("- src/server.rs (") && listing.contains(" bytes): Server, serve, GET /api/tasks, private_helper"), "{listing}");
        assert!(listing.contains("- src/untouched.rs (") && listing.contains("untouched_fn"), "{listing}");
        assert!(listing.contains("- public/index.html (28 bytes)\n") && listing.contains("- README.md (17 bytes)\n"), "{listing}");
        // A touched file is shown in full below, not listed; a file the tree lacks is not there: no service worker.
        assert!(!listing.contains("src/lib.rs") && !listing.contains("src/new.rs"), "{listing}");
        assert!(!listing.contains("sw.js") && !listing.contains("service"), "{listing}");
        // Bodies are not in it.
        assert!(!doc.contains("UNTOUCHED_BODY") && !doc.contains("UNTOUCHED_HTML"), "{doc}");
        let l = read_json(&cwd.join("in/listing.json"));
        let paths: Vec<&str> = l["files"].as_array().unwrap().iter().filter_map(|f| f["path"].as_str()).collect();
        assert_eq!(paths, vec!["README.md", "public/index.html", "src/server.rs", "src/untouched.rs"]);
        assert_eq!(v["listed_files"], 4);
        assert_eq!(v["document_bytes"], doc.len());
    }

    #[test]
    fn the_listing_skips_build_output_and_what_the_diff_excludes() {
        let (cwd, case) = review_case("listing-skips", PASSING_LOG);
        for p in ["target/debug/x", ".sdc/gate/y", ".git/HEAD", "node_modules/z/index.js"] {
            let f = case.join("work/impl").join(p);
            fs::create_dir_all(f.parent().unwrap()).unwrap();
            fs::write(f, "x").unwrap();
        }
        review_inputs(&cwd, &Facts::driver(&case), "tasks").unwrap();
        let l = fs::read_to_string(cwd.join("in/listing.json")).unwrap();
        assert!(!l.contains("target/") && !l.contains(".sdc") && !l.contains(".git/") && !l.contains("node_modules"), "{l}");
    }

    #[test]
    fn top_level_items_are_found_by_a_rule_per_language() {
        let items = |p: &str, t: &str| top_level_items(p, t).map(|v| v.join(", "));
        assert_eq!(items("a.rs", "use x;\npub struct S;\npub(crate) async fn go() {}\n    fn nested() {}\nenum E {}\nimpl S {}\nmod m;\n"),
            Some("S, go, E, m".into()));
        assert_eq!(items("a.js", "export function f() {}\nexport default class K {}\nconst x = 1;\n  function inner() {}\napp.post('/tasks', h);\nself.addEventListener('fetch', e => {});\n"),
            Some("f, K, x, POST /tasks, on fetch".into()));
        assert_eq!(items("a.ts", "export const t: number = 1;\nexport interface I {}\nexport type T = 1;\n"), Some("t, I, T".into()));
        assert_eq!(items("a.py", "import os\n@app.route('/x')\ndef view():\n    def inner(): pass\nclass C:\n    pass\nasync def go(): pass\n"),
            Some("/x, view, C, go".into()));
        assert_eq!(items("a.go", "package main\nfunc Main() {}\nfunc (s *S) Do() {}\ntype S struct{}\nmux.HandleFunc(\"/ping\", h)\n"),
            Some("Main, Do, S, /ping".into()));
        assert_eq!(items("run.sh", "#!/bin/sh\nrun() {\n  :\n}\nfunction stop { :; }\n"), Some("run, stop".into()));
        // No rule for a page, a manifest or a document: path and size only. A source file without items has an empty list.
        for p in ["index.html", "package.json", "README.md", "style.css", "logo.png"] {
            assert_eq!(items(p, "anything"), None, "{p}");
        }
        assert_eq!(items("empty.rs", "// nothing\n"), Some(String::new()));
    }

    #[test]
    fn the_listing_is_capped_and_beyond_the_cap_it_is_paths_only() {
        let f = |p: &str, items: Option<&[&str]>| Listed { path: p.into(), bytes: 100, items: items.map(|i| i.iter().map(|s| s.to_string()).collect()) };
        let files = vec![f("a.rs", Some(&["alpha_item_name", "beta_item_name"])), f("b.rs", Some(&["gamma_item_name"])), f("c.html", None), f("d.rs", Some(&["delta_item_name"]))];
        let all = listing_section(&files, 4096);
        assert!(all.contains("- a.rs (100 bytes): alpha_item_name, beta_item_name\n") && all.contains("- d.rs (100 bytes): delta_item_name\n"), "{all}");
        assert!(all.contains("- c.html (100 bytes)\n"), "{all}");
        let capped = listing_section(&files, 80);
        assert!(capped.contains("- a.rs (100 bytes): alpha_item_name, beta_item_name\n"), "{capped}");
        assert!(!capped.contains("delta_item_name") && !capped.contains("gamma_item_name"), "{capped}");
        assert!(capped.contains("- b.rs\n") && capped.contains("- c.html\n") && capped.contains("- d.rs\n"), "{capped}");
        assert!(capped.contains("paths only"), "{capped}");
        assert!(listing_section(&[], 4096).contains("none"));
    }

    #[test]
    fn a_listed_untouched_file_is_cited_by_path_without_a_line() {
        let (cwd, case) = call("listed");
        let check = |at: &str| check_review(&cwd, &Facts::driver(&case), &json!({"decision": "rejected", "summary": "s", "findings": [finding("violation", at, "r1")]}), "p");
        let (ok, v) = check("impl/src/server.rs").unwrap();
        assert!(!ok);
        assert_eq!(v["reasons"], json!(["impl/src/server.rs: it does not (violates r1)"]));
        assert_eq!(v["citations"][0]["in_diff"], false);
        assert_eq!(v["citations"][0]["listed"], true);
        // A touched file may be cited by path too; a line of an untouched file, or a path nobody listed, is refused.
        let (_, v) = check("impl/README.md").unwrap();
        assert_eq!((&v["citations"][0]["in_diff"], &v["citations"][0]["listed"]), (&json!(true), &json!(false)));
        assert!(check("impl/src/server.rs:3").unwrap_err().join("; ").contains("not a line of the change tree"));
        assert!(check("impl/sw.js").unwrap_err().join("; ").contains("neither a listed product file nor a changed file"));
        assert!(check("impl/").unwrap_err().join("; ").contains("neither a listed product file nor a changed file"));
        assert!(check("impl/../in/scope.json").unwrap_err().join("; ").contains("neither"));
        // The reply schema allows the path alone.
        let p = review_schema(&cwd.join("in"))["properties"]["findings"]["items"]["properties"]["at"]["pattern"].as_str().unwrap().to_string();
        let re = regex::Regex::new(&p).unwrap();
        assert!(re.is_match("impl/src/server.rs") && re.is_match("impl/src/server.rs:3") && re.is_match("impl/src/server.rs:3-5"));
        assert!(!re.is_match("src/server.rs") && !re.is_match("impl/a:b"));
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

    // ---- SDC_REVIEW_MODE: session (the default, main's review) or facts --------------------------

    #[test]
    fn the_review_mode_is_session_unless_set_to_facts() {
        assert_eq!(review_mode(None), Ok(false));
        assert_eq!(review_mode(Some("")), Ok(false));
        assert_eq!(review_mode(Some("session")), Ok(false));
        assert_eq!(review_mode(Some("facts")), Ok(true));
        assert_eq!(review_mode(Some("both")), Err("SDC_REVIEW_MODE=both: neither session nor facts".to_string()));
    }

    /// The default review inputs are main's (before story:review-minimal-facts): the story, scope,
    /// uncovered requirements, the covered commands, the whole change tree, the diff and the test and
    /// gate logs as they are; no review document and no listing.
    #[test]
    fn the_session_review_inputs_are_the_whole_tree_and_the_logs() {
        let (cwd, case) = review_case("session", PASSING_LOG);
        let v = review_inputs_session(&cwd, &Facts::driver(&case), "tasks").unwrap();
        let mut keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(keys, ["base", "changed_files", "covers", "gate", "requirements", "story", "uncovered"]);
        assert_eq!(v["changed_files"], json!(["src/lib.rs", "src/new.rs"]));
        assert_eq!((&v["requirements"], &v["uncovered"], &v["gate"], &v["base"]), (&json!(["r1"]), &json!(["r3"]), &json!(false), &json!("mainline")));
        let mut files = walk(&cwd.join("in"));
        files.sort();
        assert_eq!(files, ["aep/story/tasks.md", "change.diff", "commands.json", "gate.txt", "impl/README.md", "impl/public/index.html",
            "impl/src/lib.rs", "impl/src/new.rs", "impl/src/server.rs", "impl/src/untouched.rs", "scope.json", "tests.txt", "uncovered.json"]);
        assert_eq!(fs::read_to_string(cwd.join("in/tests.txt")).unwrap(), PASSING_LOG);
        assert_eq!(fs::read_to_string(cwd.join("in/gate.txt")).unwrap(), "no gate run recorded on this specification revision\n");
        assert!(read_json(&cwd.join("in/commands.json"))["todo.task.AddTask"].is_object());
    }

    /// A session review (no in/listing.json) cites lines only, as on main, and its citations carry no
    /// `listed`; its schema's `at` needs a line.
    #[test]
    fn a_session_review_cites_lines_only() {
        let (cwd, case) = call("session");
        fs::remove_file(cwd.join("in/listing.json")).unwrap();
        let reply = json!({"decision": "rejected", "summary": "s", "findings": [finding("violation", "impl/README.md:2", "r1")]});
        let (ok, v) = check_review(&cwd, &Facts::driver(&case), &reply, "p").unwrap();
        assert!(!ok);
        assert_eq!(v["citations"], json!([{"at": "impl/README.md:2", "violates": "r1", "kind": "violation", "in_diff": true}]));
        let by_path = json!({"decision": "rejected", "summary": "s", "findings": [finding("violation", "impl/README.md", "r1")]});
        assert!(check_review(&cwd, &Facts::driver(&case), &by_path, "p").unwrap_err().join("; ").contains("is not a line of the change tree"));
        assert_eq!(review_schema(&cwd.join("in"))["properties"]["findings"]["items"]["properties"]["at"]["pattern"], "^impl/[^:]+:[0-9]+(-[0-9]+)?$");
    }

    // --- under Loom: the facts are the workspace's records (Facts::workspace) ---------------------

    /// A review case whose workspace is all there is: the driver's case files are removed.
    fn workspace_case(name: &str) -> (PathBuf, Facts) {
        let (cwd, case) = review_case(name, PASSING_LOG);
        for f in ["case.json", "events.jsonl"] {
            fs::remove_file(case.join(f)).unwrap();
        }
        for d in ["evidence", "blobs"] {
            fs::remove_dir_all(case.join(d)).unwrap();
        }
        (cwd, Facts::workspace(&case.join("work")))
    }

    #[test]
    fn a_workspace_review_has_no_inputs_before_the_tests_ran_on_the_tree() {
        let (cwd, facts) = workspace_case("ws-none");
        let e = review_inputs(&cwd, &facts, "tasks").unwrap_err();
        assert!(e.starts_with("no conformance run recorded on change revision "), "{e}");
        assert!(!e.contains("revision none"), "the tree has a digest: {e}");
    }

    #[test]
    fn a_workspace_review_reads_the_logs_of_the_records_like_the_drivers() {
        let (cwd, facts) = workspace_case("ws-logs");
        facts.write_record("conformance_run", "impl", PASSING_LOG).unwrap();
        facts.write_record("ess_gate", "spec", "gate log\n").unwrap_err();
        fs::create_dir_all(facts.work().join("spec")).unwrap();
        fs::write(facts.work().join("spec/system.yaml"), "system: tasks\n").unwrap();
        facts.write_record("ess_gate", "spec", "gate log\n").unwrap();
        let v = review_inputs(&cwd, &facts, "tasks").unwrap();
        assert_eq!(v["gate"], true);
        // The document the model reads is the one the driver's case gives (the same tests log).
        let (dcwd, dcase) = review_case("ws-logs-driver", PASSING_LOG);
        review_inputs(&dcwd, &Facts::driver(&dcase), "tasks").unwrap();
        assert_eq!(fs::read(cwd.join("in/review.md")).unwrap(), fs::read(dcwd.join("in/review.md")).unwrap());
    }

    #[test]
    fn a_workspace_review_reads_the_session_inputs_from_the_records_too() {
        let (cwd, facts) = workspace_case("ws-session");
        facts.write_record("conformance_run", "impl", PASSING_LOG).unwrap();
        review_inputs_session(&cwd, &facts, "tasks").unwrap();
        assert_eq!(fs::read_to_string(cwd.join("in/tests.txt")).unwrap(), PASSING_LOG);
        assert_eq!(fs::read_to_string(cwd.join("in/gate.txt")).unwrap(), "no gate run recorded on this specification revision\n");
        fs::create_dir_all(facts.work().join("spec")).unwrap();
        fs::write(facts.work().join("spec/system.yaml"), "system: tasks\n").unwrap();
        facts.write_record("ess_gate", "spec", "gate log\n").unwrap();
        review_inputs_session(&cwd, &facts, "tasks").unwrap();
        assert_eq!(fs::read_to_string(cwd.join("in/gate.txt")).unwrap(), "gate log\n");
    }

    #[test]
    fn a_run_recorded_on_an_earlier_tree_is_no_run_on_the_changed_one() {
        let (cwd, facts) = workspace_case("ws-changed");
        facts.write_record("conformance_run", "impl", PASSING_LOG).unwrap();
        review_inputs(&cwd, &facts, "tasks").unwrap();
        fs::write(facts.work().join("impl/src/new.rs"), "pub fn brand_new_again() {}\n").unwrap();
        assert!(review_inputs(&cwd, &facts, "tasks").unwrap_err().starts_with("no conformance run recorded on change revision "));
        // Build output, `.sdc` and `.derived-from` are no part of the revision.
        fs::write(facts.work().join("impl/src/new.rs"), "pub fn brand_new() {}\n").unwrap();
        fs::create_dir_all(facts.work().join("impl/target/debug")).unwrap();
        fs::write(facts.work().join("impl/target/debug/x"), "x").unwrap();
        fs::create_dir_all(facts.work().join("impl/.sdc")).unwrap();
        fs::write(facts.work().join("impl/.sdc/coverage.json"), "{}").unwrap();
        fs::write(facts.work().join("impl/.derived-from"), "x\n").unwrap();
        review_inputs(&cwd, &facts, "tasks").unwrap();
    }

    #[test]
    fn a_workspace_keeps_its_rejections_and_counts_them() {
        let (cwd, facts) = workspace_case("ws-reject");
        fs::create_dir_all(cwd.join("in/impl")).unwrap();
        fs::write(cwd.join("in/impl/README.md"), "# App\nrun it\nend\n").unwrap();
        fs::write(cwd.join("in/change.diff"), "+++ impl/README.md\t2026\n").unwrap();
        fs::write(cwd.join("in/scope.json"), json!({"covers": [], "requirements": [{"id": "r1"}], "acceptance": []}).to_string()).unwrap();
        fs::write(cwd.join("in/uncovered.json"), json!({"requirements": []}).to_string()).unwrap();
        assert_eq!(facts.rejections("change.review"), json!([]));
        let reply = json!({"decision": "rejected", "summary": "s", "findings": [finding("violation", "impl/README.md:2", "r1")]});
        let (ok, first) = check_review(&cwd, &facts, &reply, "llm-review:m").unwrap();
        assert!(!ok);
        assert_eq!((first["count"].clone(), first["revision"].clone()), (json!(1), facts.revision_value("change")));
        assert_eq!(first["revision"].as_str().map(str::len), Some(16));
        let (_, second) = check_review(&cwd, &facts, &reply, "llm-review:m").unwrap();
        assert_eq!(second["count"], 2);
        let kept = facts.rejections("change.review");
        assert_eq!(kept.as_array().map(Vec::len), Some(2));
        assert_eq!(kept[0]["record"], "001");
        assert_eq!(kept[1]["principal"], "llm-review:m");
        assert_eq!(kept[1]["reasons"], second["reasons"]);
        // An approval keeps nothing, an invalid reply keeps nothing.
        let approved = json!({"decision": "approved", "summary": "s", "findings": []});
        check_review(&cwd, &facts, &approved, "p").unwrap();
        check_review(&cwd, &facts, &json!({"decision": "maybe"}), "p").unwrap_err();
        assert_eq!(facts.rejections("change.review").as_array().map(Vec::len), Some(2));
    }

    #[test]
    fn the_driver_keeps_no_records_of_its_own() {
        let (cwd, case) = call("driver-keeps-none");
        let reply = json!({"decision": "rejected", "summary": "s", "findings": [finding("violation", "impl/README.md:2", "r1")]});
        check_review(&cwd, &Facts::driver(&case), &reply, "p").unwrap();
        assert!(!case.join("work").join(RECORDS).exists());
    }

    #[test]
    fn a_record_names_a_tree_that_is_there_and_a_plain_kind() {
        let (_, facts) = workspace_case("ws-record-refused");
        assert!(facts.write_record("conformance_run", "nowhere", "x").unwrap_err().contains("no such tree"));
        assert!(facts.write_record("../x", "impl", "x").unwrap_err().contains("no record kind"));
        assert!(facts.write_record("a/b", "impl", "x").is_err());
    }

    #[test]
    fn the_time_of_a_record_is_a_utc_stamp() {
        let t = now_utc();
        assert!(regex::Regex::new(r"^20\d\d-\d\d-\d\dT\d\d:\d\d:\d\dZ$").unwrap().is_match(&t), "{t}");
    }
}
