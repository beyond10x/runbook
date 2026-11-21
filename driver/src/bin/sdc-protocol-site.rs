//! The data of the public documentation site (`website/`), generated from the protocols.
//!
//! ```text
//! sdc-protocol-site write [--root <repository>]
//! sdc-protocol-site check [--root <repository>]
//! sdc-protocol-site site-manifest --out <built site> --commit <full revision>
//! ```
//!
//! `write` reads `protocol/*.protocol.yaml`, the bindings beside the driver
//! (`driver/examples/sdc/<name>.bindings.json`, `tools/node-classes.json`, `decision-classes.json`),
//! the committed `canon check` output (`docs/check-output.txt`), the pinned `canon simulate` reports
//! (`protocol/simulate/<name>.txt`), `CHANGELOG.md` and `docs/releases/<version>.md`, and writes one
//! JSON document per protocol and `index.json` into `website/data/protocols/`. Nothing in those
//! files is written by hand. `check` fails (exit 1) when one of them is missing, stale or no longer
//! generated; `scripts/check-bindings.sh` runs it. `site-manifest` writes
//! `.well-known/b10x-site.json` into a built site, the declaration the beyond10x project-site
//! publisher refuses a mismatch on. Exit 0, 1 stale or a problem, 2 an input that cannot be read,
//! 64 usage.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use regex::Regex;
use serde_json::{json, Value as J};
use serde_yaml::Value as Y;
use sha2::{Digest, Sha256};

const FORMAT: &str = "sdc.protocol-site/1";
const OUT_DIR: &str = "website/data/protocols";
const REPOSITORY: &str = "beyond10x/runbook";
const BASE_URL: &str = "/runbook/";
const BINDINGS_DIR: &str = "driver/examples/sdc";
/// The tools the bindings run, and the node classes beside them.
const TOOLS_DIR: &str = "bindings/tools";
const CHECK_OUTPUT: &str = "docs/check-output.txt";

// ---- predicates --------------------------------------------------------------------------------

/// A `protocol/1` predicate as the YAML writes it, normalised.
#[derive(Debug, Clone, PartialEq)]
enum Pred {
    All(Vec<Pred>),
    Any(Vec<Pred>),
    Not(Box<Pred>),
    Claim { id: String, is: String },
    Evidence { kind: String, result: Option<String>, subject: Option<String>, independent_on: Vec<String> },
    Children { within: Vec<String> },
    Decision(String),
    /// A form this generator does not know, kept as written.
    Other(J),
}

fn scalar(v: &Y) -> String {
    match v {
        Y::String(s) => s.clone(),
        Y::Bool(b) => b.to_string(),
        Y::Number(n) => n.to_string(),
        Y::Null => String::new(),
        other => serde_yaml::to_string(other).unwrap_or_default().trim().to_string(),
    }
}

fn strings(v: Option<&Y>) -> Vec<String> {
    match v {
        Some(Y::Sequence(s)) => s.iter().map(scalar).collect(),
        Some(Y::Null) | None => vec![],
        Some(one) => vec![scalar(one)],
    }
}

fn to_json(v: &Y) -> J {
    serde_json::to_value(v).unwrap_or(J::Null)
}

fn text(v: &Y, key: &str) -> String {
    v.get(key).map(scalar).unwrap_or_default().trim().to_string()
}

fn pred(v: &Y) -> Pred {
    let Some(map) = v.as_mapping() else { return Pred::Other(to_json(v)) };
    let get = |k: &str| map.get(Y::String(k.into()));
    if let Some(Y::Sequence(items)) = get("all") {
        return Pred::All(items.iter().map(pred).collect());
    }
    if let Some(Y::Sequence(items)) = get("any") {
        return Pred::Any(items.iter().map(pred).collect());
    }
    if let Some(inner) = get("not") {
        return Pred::Not(Box::new(pred(inner)));
    }
    if let Some(c) = get("claim") {
        return match c {
            Y::Mapping(_) => Pred::Claim { id: text(c, "id"), is: c.get("is").map(scalar).unwrap_or_else(|| "true".into()) },
            other => Pred::Claim { id: scalar(other), is: get("is").map(scalar).unwrap_or_else(|| "true".into()) },
        };
    }
    if let Some(e) = get("evidence") {
        let opt = |k: &str| e.get(k).filter(|x| !x.is_null()).map(scalar);
        return Pred::Evidence { kind: text(e, "kind"), result: opt("result"), subject: opt("subject"), independent_on: strings(e.get("independent_on")) };
    }
    if let Some(c) = get("children") {
        return Pred::Children { within: strings(c.get("in")) };
    }
    if let Some(d) = get("decision") {
        return Pred::Decision(scalar(d));
    }
    Pred::Other(to_json(v))
}

/// A predicate as indented lines, the way the site lists it. `reference` names a node of the
/// protocol (`claim:x`, `evidence:y`, `decision:z`) when the line is about one.
fn lines(p: &Pred, depth: usize, out: &mut Vec<J>) {
    let mut line = |text: String, reference: Option<String>, suffix: Option<String>| {
        let mut l = json!({"depth": depth, "text": text});
        if let Some(r) = reference {
            l["reference"] = json!(r);
        }
        if let Some(s) = suffix {
            l["suffix"] = json!(s);
        }
        out.push(l);
    };
    match p {
        Pred::All(items) if items.len() == 1 => lines(&items[0], depth, out),
        Pred::All(items) => {
            line("all of".into(), None, None);
            items.iter().for_each(|i| lines(i, depth + 1, out));
        }
        Pred::Any(items) => {
            line("any of".into(), None, None);
            items.iter().for_each(|i| lines(i, depth + 1, out));
        }
        Pred::Not(inner) => {
            line("not".into(), None, None);
            lines(inner, depth + 1, out);
        }
        Pred::Claim { id, is } => {
            let t = if is == "true" { "claim".to_string() } else { format!("claim is {is}:") };
            line(t, Some(format!("claim:{id}")), None);
        }
        Pred::Evidence { kind, result, subject, independent_on } => {
            let t = match result {
                Some(r) => format!("evidence = {r}:"),
                None => "any evidence:".to_string(),
            };
            let mut suffix = vec![];
            if let Some(s) = subject {
                suffix.push(format!("about {s}"));
            }
            if !independent_on.is_empty() {
                suffix.push(format!("by a producer independent on {}", independent_on.join(", ")));
            }
            line(t, Some(format!("evidence:{kind}")), (!suffix.is_empty()).then(|| suffix.join(", ")));
        }
        Pred::Children { within } => line("every child case ended".into(), None, Some(format!("in {}", within.join(", ")))),
        Pred::Decision(d) => line("decision taken:".into(), Some(format!("decision:{d}")), None),
        Pred::Other(j) => line(serde_json::to_string(j).unwrap_or_default(), None, None),
    }
}

fn pred_lines(p: &Pred) -> Vec<J> {
    let mut out = vec![];
    lines(p, 0, &mut out);
    out
}

/// Every leaf, with whether it is reached through an odd number of `not`.
fn leaves<'a>(p: &'a Pred, negated: bool, out: &mut Vec<(&'a Pred, bool)>) {
    match p {
        Pred::All(items) | Pred::Any(items) => items.iter().for_each(|i| leaves(i, negated, out)),
        Pred::Not(inner) => leaves(inner, !negated, out),
        leaf => out.push((leaf, negated)),
    }
}

fn leaf_list(p: &Pred) -> Vec<(&Pred, bool)> {
    let mut out = vec![];
    leaves(p, false, &mut out);
    out
}

// ---- the protocol model ------------------------------------------------------------------------

struct Named {
    name: String,
    description: String,
}

struct Action {
    name: String,
    description: String,
    effect: String,
    precondition: Option<Pred>,
    capabilities: Vec<String>,
    produces: Vec<String>,
    opens: Option<(String, String)>,
    arguments: Vec<String>,
}

struct Claim {
    name: String,
    description: String,
    true_when: Option<Pred>,
}

struct Outcome {
    name: String,
    description: String,
    requires: Option<Pred>,
}

struct Obligation {
    name: String,
    description: String,
    discharged_when: Option<Pred>,
}

struct Protocol {
    stem: String,
    path: String,
    id: String,
    revision: u64,
    description: String,
    artifacts: Vec<Named>,
    evidence: Vec<(Named, Option<String>)>,
    claims: Vec<Claim>,
    obligations: Vec<Obligation>,
    actions: Vec<Action>,
    outcomes: Vec<Outcome>,
    invalidation: Vec<Named>,
}

fn entries(doc: &Y, key: &str) -> Vec<(String, Y)> {
    let mut v: Vec<(String, Y)> = doc
        .get(key)
        .and_then(Y::as_mapping)
        .map(|m| m.iter().map(|(k, v)| (scalar(k), v.clone())).collect())
        .unwrap_or_default();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

fn parse_protocol(stem: &str, path: &str, raw: &str) -> Result<Protocol, String> {
    let doc: Y = serde_yaml::from_str(raw).map_err(|e| format!("{path}: {e}"))?;
    if text(&doc, "format") != "protocol/1" {
        return Err(format!("{path}: format is not protocol/1"));
    }
    let head = doc.get("protocol").ok_or_else(|| format!("{path}: no protocol section"))?;
    let named = |key: &str| entries(&doc, key).into_iter().map(|(name, v)| Named { name, description: text(&v, "description") }).collect::<Vec<_>>();
    let opt_pred = |v: &Y, k: &str| v.get(k).filter(|x| !x.is_null()).map(pred);
    Ok(Protocol {
        stem: stem.to_string(),
        path: path.to_string(),
        id: text(head, "id"),
        revision: head.get("revision").and_then(Y::as_u64).ok_or_else(|| format!("{path}: no revision"))?,
        description: text(head, "description"),
        artifacts: named("artifacts"),
        evidence: entries(&doc, "evidence_kinds")
            .into_iter()
            .map(|(name, v)| {
                let max_age = v.get("max_age").map(scalar);
                (Named { name, description: text(&v, "description") }, max_age)
            })
            .collect(),
        claims: entries(&doc, "claims").into_iter().map(|(name, v)| Claim { name, description: text(&v, "description"), true_when: opt_pred(&v, "true_when") }).collect(),
        obligations: entries(&doc, "obligations")
            .into_iter()
            .map(|(name, v)| Obligation { name, description: text(&v, "description"), discharged_when: opt_pred(&v, "discharged_when") })
            .collect(),
        actions: entries(&doc, "actions")
            .into_iter()
            .map(|(name, v)| Action {
                description: text(&v, "description"),
                effect: text(&v, "effect"),
                precondition: opt_pred(&v, "precondition"),
                capabilities: v.get("requires").and_then(Y::as_sequence).into_iter().flatten().filter_map(|r| r.get("capability").map(scalar)).collect(),
                produces: v.get("may_produce").and_then(Y::as_sequence).into_iter().flatten().filter_map(|r| r.get("evidence").map(scalar)).collect(),
                opens: v.get("case_open").map(|c| (text(c, "protocol"), text(c, "subject"))),
                arguments: v
                    .get("arguments")
                    .and_then(|a| a.get("properties"))
                    .and_then(Y::as_mapping)
                    .map(|m| {
                        let mut k: Vec<String> = m.keys().map(scalar).collect();
                        k.sort();
                        k
                    })
                    .unwrap_or_default(),
                name,
            })
            .collect(),
        outcomes: entries(&doc, "outcomes").into_iter().map(|(name, v)| Outcome { name, description: text(&v, "description"), requires: opt_pred(&v, "requires") }).collect(),
        invalidation: named("invalidation"),
    })
}

// ---- derived facts -----------------------------------------------------------------------------

/// Evidence kinds a claim is TRUE on, through claims it reads positively.
fn claim_evidence(p: &Protocol, claim: &str, seen: &mut BTreeSet<String>, out: &mut BTreeSet<String>) {
    if !seen.insert(claim.to_string()) {
        return;
    }
    let Some(c) = p.claims.iter().find(|c| c.name == claim) else { return };
    let Some(tw) = &c.true_when else { return };
    for (leaf, negated) in leaf_list(tw) {
        if negated {
            continue;
        }
        match leaf {
            Pred::Evidence { kind, .. } => {
                out.insert(kind.clone());
            }
            Pred::Claim { id, is } if is == "true" => claim_evidence(p, id, seen, out),
            _ => {}
        }
    }
}

/// The actions an action waits for: those producing evidence its precondition needs TRUE.
/// Guards (`is: unknown`, `is: false`, under `not`) and the children form are not waits.
fn waits_for(p: &Protocol, a: &Action) -> BTreeSet<String> {
    let mut kinds = BTreeSet::new();
    if let Some(pre) = &a.precondition {
        for (leaf, negated) in leaf_list(pre) {
            if negated {
                continue;
            }
            match leaf {
                Pred::Claim { id, is } if is == "true" => claim_evidence(p, id, &mut BTreeSet::new(), &mut kinds),
                Pred::Evidence { kind, .. } => {
                    kinds.insert(kind.clone());
                }
                _ => {}
            }
        }
    }
    p.actions.iter().filter(|b| b.name != a.name && b.produces.iter().any(|k| kinds.contains(k))).map(|b| b.name.clone()).collect()
}

/// Each action's step: 0 for an action that waits for none, else one more than the latest action
/// it waits for. A wait that closes a loop is ignored.
fn steps(p: &Protocol) -> BTreeMap<String, usize> {
    let deps: BTreeMap<String, BTreeSet<String>> = p.actions.iter().map(|a| (a.name.clone(), waits_for(p, a))).collect();
    fn visit(n: &str, deps: &BTreeMap<String, BTreeSet<String>>, stack: &mut Vec<String>, memo: &mut BTreeMap<String, usize>) -> usize {
        if let Some(&s) = memo.get(n) {
            return s;
        }
        stack.push(n.to_string());
        let mut s = 0;
        for d in deps.get(n).into_iter().flatten() {
            if stack.iter().any(|x| x == d) {
                continue;
            }
            s = s.max(visit(d, deps, stack, memo) + 1);
        }
        stack.pop();
        memo.insert(n.to_string(), s);
        s
    }
    let mut memo = BTreeMap::new();
    for a in &p.actions {
        visit(&a.name, &deps, &mut vec![], &mut memo);
    }
    memo
}

fn graph(p: &Protocol) -> J {
    let mut nodes: BTreeMap<String, J> = BTreeMap::new();
    let mut edges: BTreeSet<(String, String, String, String)> = BTreeSet::new();
    let mut node = |id: String, kind: &str, name: &str| {
        nodes.entry(id.clone()).or_insert_with(|| json!({"id": id, "kind": kind, "name": name}));
    };
    for a in &p.actions {
        node(format!("action:{}", a.name), "action", &a.name);
    }
    for (e, _) in &p.evidence {
        node(format!("evidence:{}", e.name), "evidence", &e.name);
    }
    for c in &p.claims {
        node(format!("claim:{}", c.name), "claim", &c.name);
    }
    for o in &p.outcomes {
        node(format!("outcome:{}", o.name), "outcome", &o.name);
    }
    let children: Vec<&String> = p.actions.iter().filter_map(|a| a.opens.as_ref().map(|o| &o.0)).collect();
    for c in &children {
        node(format!("child:{c}"), "child", c);
    }
    let mut decisions = BTreeSet::new();
    // A leaf of a predicate that targets `to`: which edge it gives.
    let mut leaf_edges = |pred: &Pred, to: &str, claim_kind: &str, edges: &mut BTreeSet<(String, String, String, String)>| {
        for (leaf, negated) in leaf_list(pred) {
            let not = if negated { "not " } else { "" };
            match leaf {
                Pred::Claim { id, is } => {
                    let q = if negated || is != "true" { format!("{not}{is}") } else { String::new() };
                    edges.insert((format!("claim:{id}"), to.to_string(), claim_kind.to_string(), q));
                }
                Pred::Evidence { kind, result, .. } => {
                    let q = format!("{not}{}", result.clone().unwrap_or_default());
                    let ek = if to.starts_with("claim:") { "establishes" } else { claim_kind };
                    edges.insert((format!("evidence:{kind}"), to.to_string(), ek.to_string(), q.trim().to_string()));
                }
                Pred::Children { within } => {
                    for c in &children {
                        edges.insert((format!("child:{c}"), to.to_string(), "reports".into(), format!("{not}in {}", within.join(", "))));
                    }
                }
                Pred::Decision(d) => {
                    decisions.insert(d.clone());
                    edges.insert((format!("decision:{d}"), to.to_string(), "decides".into(), String::new()));
                }
                _ => {}
            }
        }
    };
    for a in &p.actions {
        let id = format!("action:{}", a.name);
        for k in &a.produces {
            edges.insert((id.clone(), format!("evidence:{k}"), "produces".into(), String::new()));
        }
        if let Some((c, _)) = &a.opens {
            edges.insert((id.clone(), format!("child:{c}"), "opens".into(), String::new()));
        }
        if let Some(pre) = &a.precondition {
            leaf_edges(pre, &id, "gates", &mut edges);
        }
    }
    for c in &p.claims {
        if let Some(tw) = &c.true_when {
            leaf_edges(tw, &format!("claim:{}", c.name), "supports", &mut edges);
        }
    }
    for o in &p.outcomes {
        if let Some(r) = &o.requires {
            leaf_edges(r, &format!("outcome:{}", o.name), "requires", &mut edges);
        }
    }
    for d in &decisions {
        node(format!("decision:{d}"), "decision", d);
    }
    // Edges whose ends are both declared; a dangling reference is the validator's to report.
    let edges: Vec<J> = edges
        .into_iter()
        .filter(|(f, t, _, _)| nodes.contains_key(f) && nodes.contains_key(t))
        .map(|(from, to, kind, q)| {
            let mut e = json!({"from": from, "to": to, "kind": kind});
            if !q.is_empty() {
                e["qualifier"] = json!(q);
            }
            e
        })
        .collect();
    let order = ["action", "evidence", "child", "claim", "decision", "outcome"];
    let mut nodes: Vec<J> = nodes.into_values().collect();
    nodes.sort_by_key(|n| (order.iter().position(|k| n["kind"] == *k).unwrap_or(9), n["name"].as_str().unwrap_or("").to_string()));
    json!({"nodes": nodes, "edges": edges})
}

// ---- bindings ----------------------------------------------------------------------------------

struct Context {
    node_classes: BTreeMap<String, String>,
    decision_classes: BTreeMap<String, J>,
}

fn load_context(root: &Path) -> Result<Context, String> {
    let nc = read_json(&root.join(TOOLS_DIR).join("node-classes.json"))?;
    let mut node_classes = BTreeMap::new();
    for (class, actions) in nc["classes"].as_object().into_iter().flatten() {
        for a in actions.as_array().into_iter().flatten().filter_map(J::as_str) {
            node_classes.insert(a.to_string(), class.clone());
        }
    }
    let dc = read_json(&root.join(BINDINGS_DIR).join("decision-classes.json"))?;
    let mut decision_classes = BTreeMap::new();
    for (class, v) in dc["classes"].as_object().into_iter().flatten() {
        for a in v["actions"].as_array().into_iter().flatten().filter_map(J::as_str) {
            let mut d = json!({"class": class, "type": v["type"]});
            if let Some(side) = v["side"].as_str() {
                d["side"] = json!(side);
            }
            decision_classes.insert(a.to_string(), d);
        }
    }
    Ok(Context { node_classes, decision_classes })
}

/// A bound argv as the site shows it: the tool directory placeholder dropped.
fn program(argv: &J) -> String {
    argv.as_array()
        .into_iter()
        .flatten()
        .filter_map(J::as_str)
        .map(|a| a.strip_prefix("{dir}/../../../bindings/tools/").or_else(|| a.strip_prefix("{dir}/")).unwrap_or(a).to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn executor(p: &Protocol, a: &Action, bindings: Option<&J>, ctx: &Context) -> J {
    let independent: BTreeSet<String> = bindings
        .and_then(|b| b["$independence"].as_object())
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    let Some(b) = bindings.and_then(|b| b.get(&a.name)) else {
        let mut e = json!({"kind": "person", "bound": false});
        if bindings.is_none() {
            e["note"] = json!("no bindings file for this protocol");
        }
        return e;
    };
    let mut e = if let Some(child) = b.get("child") {
        let mut e = json!({"kind": "child-case", "protocol": a.opens.as_ref().map(|o| o.0.clone()).unwrap_or_default()});
        if let Some(each) = child["each"].as_str() {
            e["each"] = json!(each);
        }
        e
    } else if let Some(copy) = b.get("copy") {
        json!({"kind": "copy", "from": copy["from"], "record": copy["kind"]})
    } else if b["manual"].as_bool() == Some(true) {
        let mut e = json!({"kind": "person", "bound": true});
        let prog = program(&b["argv"]);
        if prog != "true" && !prog.is_empty() {
            e["program"] = json!(prog);
        }
        e
    } else if let Some(llm) = b.get("llm") {
        let mut e = json!({"kind": "model", "program": program(&b["argv"])});
        e["node_class"] = json!(ctx.node_classes.get(&a.name).cloned().unwrap_or_else(|| "unlisted".into()));
        if let Some(m) = llm["model"].as_str() {
            e["default_model"] = json!(m);
        }
        if let Some(ops) = llm["operations"].as_array() {
            e["operations"] = json!(ops);
        }
        e
    } else {
        json!({"kind": "program", "program": program(&b["argv"])})
    };
    if let Some(prod) = b["producer"].as_str() {
        e["producer"] = json!(prod);
        if independent.contains(prod) {
            e["independent"] = json!(true);
        }
    }
    if let Some(results) = b["results"].as_object() {
        e["results"] = json!(results);
    }
    if let Some(r) = b["retry_when"].as_object() {
        e["retry_when"] = json!(r.keys().collect::<Vec<_>>());
    }
    for k in ["max_runs", "timeout_s"] {
        if let Some(n) = b[k].as_u64() {
            e[k] = json!(n);
        }
    }
    if b.get("handoff").is_some() {
        e["handoff"] = json!(true);
    }
    let _ = p;
    e
}

// ---- committed check and simulate output -------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
struct Checked {
    revision: u64,
    states: u64,
}

/// The first `checked:` line per protocol id; later lines of the same id are the gate probes.
fn parse_check_output(raw: &str) -> BTreeMap<String, Checked> {
    let re = Regex::new(r"^checked: protocol `([^`]+)` revision (\d+): (\d+) states").expect("regex");
    let mut out = BTreeMap::new();
    for l in raw.lines() {
        if let Some(c) = re.captures(l) {
            out.entry(c[1].to_string()).or_insert(Checked { revision: c[2].parse().unwrap_or(0), states: c[3].parse().unwrap_or(0) });
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
struct Simulated {
    id: String,
    revision: u64,
    reachable: u64,
    terminable: u64,
    findings: u64,
    stuck_or_cycle: u64,
    by_outcome: BTreeMap<String, u64>,
}

fn parse_simulate(raw: &str) -> Option<Simulated> {
    let line = raw.lines().rev().find(|l| l.starts_with("simulated: "))?;
    let head = Regex::new(r"^simulated: protocol `([^`]+)` revision (\d+): (\d+) reachable states?, (\d+) terminable").expect("regex");
    let c = head.captures(line)?;
    let num = |re: &str| Regex::new(re).expect("regex").captures(line).and_then(|c| c[1].parse().ok());
    let by_outcome = Regex::new(r"outcome `([^`]+)` (\d+)")
        .expect("regex")
        .captures_iter(line)
        .map(|c| (c[1].to_string(), c[2].parse().unwrap_or(0)))
        .collect();
    Some(Simulated {
        id: c[1].to_string(),
        revision: c[2].parse().ok()?,
        reachable: c[3].parse().ok()?,
        terminable: c[4].parse().ok()?,
        findings: num(r"(\d+) findings?")?,
        stuck_or_cycle: num(r"\((\d+) stuck or cycle states?\)").unwrap_or(0),
        by_outcome,
    })
}

// ---- releases ----------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
struct Release {
    version: String,
    date: String,
    summary: String,
    limits: Vec<String>,
    notes: Option<String>,
}

fn parse_changelog(raw: &str) -> Vec<(String, String)> {
    let re = Regex::new(r"^## \[(\d+\.\d+\.\d+)\] - (\d{4}-\d{2}-\d{2})\s*$").expect("regex");
    raw.lines().filter_map(|l| re.captures(l).map(|c| (c[1].to_string(), c[2].to_string()))).collect()
}

/// The first paragraph and the bullets under `### Limits` of a release note.
fn parse_notes(raw: &str) -> (String, Vec<String>) {
    let mut summary = vec![];
    let mut started = false;
    for l in raw.lines() {
        if l.starts_with('#') {
            if started {
                break;
            }
            continue;
        }
        if l.trim().is_empty() {
            if started {
                break;
            }
            continue;
        }
        started = true;
        summary.push(l.trim());
    }
    let mut limits: Vec<String> = vec![];
    let mut inside = false;
    for l in raw.lines() {
        if l.starts_with('#') {
            inside = l.trim_start_matches('#').trim().eq_ignore_ascii_case("limits");
            continue;
        }
        if !inside {
            continue;
        }
        if let Some(b) = l.strip_prefix("- ") {
            limits.push(b.trim().to_string());
        } else if !l.trim().is_empty() && l.starts_with(' ') {
            if let Some(last) = limits.last_mut() {
                last.push(' ');
                last.push_str(l.trim());
            }
        } else if !l.trim().is_empty() {
            inside = false;
        }
    }
    (summary.join(" "), limits)
}

fn version_key(v: &str) -> Vec<u64> {
    v.split('.').map(|x| x.parse().unwrap_or(0)).collect()
}

// ---- rendering ---------------------------------------------------------------------------------

struct Inputs {
    protocols: Vec<Protocol>,
    bindings: BTreeMap<String, J>,
    ctx: Context,
    checked: BTreeMap<String, Checked>,
    simulated: BTreeMap<String, Simulated>,
    canon_rev: Option<String>,
    releases: Vec<Release>,
    sources: Vec<(String, String)>,
}

fn sha256_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes()).iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn read(root: &Path, rel: &str, sources: &mut Vec<(String, String)>) -> Result<String, String> {
    let raw = fs::read_to_string(root.join(rel)).map_err(|e| format!("cannot read {rel}: {e}"))?;
    sources.push((rel.to_string(), sha256_hex(&raw)));
    Ok(raw)
}

/// The changelog from its first released version on: what the site renders. An `[Unreleased]`
/// line changes nothing the site shows, so it must not make the committed data stale.
fn released_part(changelog: &str) -> &str {
    let mut at = 0;
    for line in changelog.split_inclusive('\n') {
        if line.starts_with("## [") && !line.starts_with("## [Unreleased]") {
            return &changelog[at..];
        }
        at += line.len();
    }
    ""
}

fn read_json(path: &Path) -> Result<J, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("{}: {e}", path.display()))
}

fn load(root: &Path) -> Result<Inputs, String> {
    let mut sources = vec![];
    let mut stems: Vec<String> = fs::read_dir(root.join("protocol"))
        .map_err(|e| format!("cannot read protocol/: {e}"))?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".protocol.yaml")).map(String::from))
        .collect();
    stems.sort();
    let mut protocols = vec![];
    let mut bindings = BTreeMap::new();
    let mut simulated = BTreeMap::new();
    for stem in &stems {
        let rel = format!("protocol/{stem}.protocol.yaml");
        protocols.push(parse_protocol(stem, &rel, &read(root, &rel, &mut sources)?)?);
        let b = format!("{BINDINGS_DIR}/{stem}.bindings.json");
        if root.join(&b).is_file() {
            let raw = read(root, &b, &mut sources)?;
            bindings.insert(stem.clone(), serde_json::from_str(&raw).map_err(|e| format!("{b}: {e}"))?);
        }
        let s = format!("protocol/simulate/{stem}.txt");
        if root.join(&s).is_file() {
            if let Some(sim) = parse_simulate(&read(root, &s, &mut sources)?) {
                simulated.insert(stem.clone(), sim);
            }
        }
    }
    for rel in [format!("{TOOLS_DIR}/node-classes.json"), format!("{BINDINGS_DIR}/decision-classes.json")] {
        read(root, &rel, &mut sources)?;
    }
    let ctx = load_context(root)?;
    let checked = if root.join(CHECK_OUTPUT).is_file() { parse_check_output(&read(root, CHECK_OUTPUT, &mut sources)?) } else { BTreeMap::new() };
    let canon_rev = if root.join("protocol/simulate/CANON_REV").is_file() {
        Some(read(root, "protocol/simulate/CANON_REV", &mut sources)?.trim().to_string())
    } else {
        None
    };
    let mut releases = vec![];
    if root.join("CHANGELOG.md").is_file() {
        let raw = fs::read_to_string(root.join("CHANGELOG.md")).map_err(|e| format!("cannot read CHANGELOG.md: {e}"))?;
        let released = released_part(&raw);
        sources.push(("CHANGELOG.md".to_string(), sha256_hex(released)));
        for (version, date) in parse_changelog(released) {
            let rel = format!("docs/releases/{version}.md");
            let (summary, limits, notes) = if root.join(&rel).is_file() {
                let (s, l) = parse_notes(&read(root, &rel, &mut sources)?);
                (s, l, Some(rel))
            } else {
                (String::new(), vec![], None)
            };
            releases.push(Release { version, date, summary, limits, notes });
        }
    }
    releases.sort_by_key(|a| std::cmp::Reverse(version_key(&a.version)));
    sources.sort();
    Ok(Inputs { protocols, bindings, ctx, checked, simulated, canon_rev, releases, sources })
}

fn source_url(tag: Option<&str>, path: &str) -> String {
    format!("https://github.com/{REPOSITORY}/blob/{}/{path}", tag.unwrap_or("main"))
}

fn first_sentence(s: &str) -> String {
    match s.find(". ") {
        Some(i) => s[..=i].to_string(),
        None => s.to_string(),
    }
}

fn render_protocol(inp: &Inputs, p: &Protocol, tag: Option<&str>) -> J {
    let steps = steps(p);
    let bindings = inp.bindings.get(&p.stem);
    let mut actions: Vec<&Action> = p.actions.iter().collect();
    actions.sort_by_key(|a| (steps.get(&a.name).copied().unwrap_or(0), a.name.clone()));
    let actions: Vec<J> = actions
        .iter()
        .map(|a| {
            let mut j = json!({
                "name": a.name,
                "description": a.description,
                "effect": a.effect,
                "step": steps.get(&a.name).copied().unwrap_or(0),
                "capabilities": a.capabilities,
                "produces": a.produces,
                "waits_for": waits_for(p, a),
                "arguments": a.arguments,
                "executor": executor(p, a, bindings, &inp.ctx),
            });
            j["precondition"] = a.precondition.as_ref().map(pred_lines).map(J::from).unwrap_or(json!([]));
            if let Some((protocol, subject)) = &a.opens {
                j["opens"] = json!({"protocol": protocol, "subject": subject});
            }
            if let Some(d) = inp.ctx.decision_classes.get(&format!("{}/{}", p.id, a.name)) {
                j["decision_class"] = d.clone();
            }
            j
        })
        .collect();
    let parents: Vec<J> = inp
        .protocols
        .iter()
        .flat_map(|q| q.actions.iter().filter(|a| a.opens.as_ref().is_some_and(|o| o.0 == p.id)).map(move |a| json!({"protocol": q.id, "stem": q.stem, "action": a.name})))
        .collect();
    let stem_of = |id: &str| inp.protocols.iter().find(|q| q.id == id).map(|q| q.stem.clone());
    let children: Vec<J> = p
        .actions
        .iter()
        .filter_map(|a| a.opens.as_ref().map(|o| (a, o)))
        .map(|(a, o)| {
            let each = bindings.and_then(|b| b[&a.name]["child"]["each"].as_str()).map(String::from);
            json!({"protocol": o.0, "stem": stem_of(&o.0), "action": a.name, "subject": o.1, "each": each, "step": steps.get(&a.name)})
        })
        .collect::<Vec<_>>();
    let mut children = children;
    children.sort_by_key(|c| (c["step"].as_u64().unwrap_or(0), c["action"].as_str().unwrap_or("").to_string()));
    let check = match inp.checked.get(&p.id) {
        Some(c) if c.revision == p.revision => json!({"states": c.states, "revision": c.revision, "source": CHECK_OUTPUT}),
        Some(c) => json!({"states": null, "revision": c.revision, "source": CHECK_OUTPUT, "note": format!("the committed check output is of revision {}, not {}", c.revision, p.revision)}),
        None => json!({"states": null, "source": CHECK_OUTPUT, "note": "no line for this protocol in the committed check output"}),
    };
    let simulate = match inp.simulated.get(&p.stem) {
        Some(s) if s.revision == p.revision && s.id == p.id => json!({
            "reachable": s.reachable, "terminable": s.terminable, "findings": s.findings, "stuck_or_cycle": s.stuck_or_cycle,
            "by_outcome": s.by_outcome, "revision": s.revision, "canon": inp.canon_rev, "source": format!("protocol/simulate/{}.txt", p.stem),
        }),
        Some(s) => json!({"findings": null, "revision": s.revision, "source": format!("protocol/simulate/{}.txt", p.stem), "note": format!("the pinned report is of revision {}, not {}", s.revision, p.revision)}),
        None => json!({"findings": null, "note": "no pinned simulate report"}),
    };
    let close = inp.ctx.decision_classes.iter().filter_map(|(k, v)| k.strip_prefix(&format!("{}/close:", p.id)).map(|o| (o.to_string(), v.clone()))).collect::<BTreeMap<_, _>>();
    json!({
        "format": FORMAT,
        "written_by": "sdc-protocol-site write",
        "id": p.id,
        "stem": p.stem,
        "revision": p.revision,
        "description": p.description,
        "summary": first_sentence(&p.description),
        "source": {"path": p.path, "url": source_url(tag, &p.path), "tag": tag},
        "bindings": bindings.map(|_| format!("{BINDINGS_DIR}/{}.bindings.json", p.stem)),
        "artifacts": p.artifacts.iter().map(|a| json!({"name": a.name, "description": a.description})).collect::<Vec<_>>(),
        "evidence": p.evidence.iter().map(|(e, max_age)| {
            let mut j = json!({"name": e.name, "description": e.description,
                "produced_by": p.actions.iter().filter(|a| a.produces.contains(&e.name)).map(|a| a.name.clone()).collect::<Vec<_>>()});
            if let Some(m) = max_age { j["max_age"] = json!(m); }
            j
        }).collect::<Vec<_>>(),
        "claims": p.claims.iter().map(|c| json!({"name": c.name, "description": c.description,
            "true_when": c.true_when.as_ref().map(pred_lines).unwrap_or_default()})).collect::<Vec<_>>(),
        "obligations": p.obligations.iter().map(|o| json!({"name": o.name, "description": o.description,
            "discharged_when": o.discharged_when.as_ref().map(pred_lines).unwrap_or_default()})).collect::<Vec<_>>(),
        "actions": actions,
        "outcomes": p.outcomes.iter().map(|o| {
            let mut j = json!({"name": o.name, "description": o.description, "requires": o.requires.as_ref().map(pred_lines).unwrap_or_default()});
            if let Some(d) = close.get(&o.name) { j["decision_class"] = d.clone(); }
            j
        }).collect::<Vec<_>>(),
        "invalidation": p.invalidation.iter().map(|i| json!({"name": i.name, "description": i.description})).collect::<Vec<_>>(),
        "children": children,
        "parents": parents,
        "check": check,
        "simulate": simulate,
        "graph": graph(p),
    })
}

fn render(inp: &Inputs) -> BTreeMap<String, String> {
    let tag = inp.releases.first().map(|r| r.version.clone());
    let mut out = BTreeMap::new();
    let mut summaries = vec![];
    let mut flow = vec![];
    for p in &inp.protocols {
        let doc = render_protocol(inp, p, tag.as_deref());
        for c in doc["children"].as_array().into_iter().flatten() {
            flow.push(json!({"parent": p.id, "child": c["protocol"], "action": c["action"], "each": c["each"], "step": c["step"]}));
        }
        summaries.push(json!({
            "id": p.id, "stem": p.stem, "revision": p.revision, "summary": doc["summary"],
            "actions": p.actions.len(), "evidence": p.evidence.len(), "claims": p.claims.len(),
            "outcomes": p.outcomes.iter().map(|o| o.name.clone()).collect::<Vec<_>>(),
            "children": doc["children"].as_array().into_iter().flatten().map(|c| c["protocol"].clone()).collect::<Vec<_>>(),
            "parents": doc["parents"].as_array().into_iter().flatten().map(|c| c["protocol"].clone()).collect::<Vec<_>>(),
            "states": doc["check"]["states"], "simulate_findings": doc["simulate"]["findings"],
            "action_order": doc["actions"].as_array().into_iter().flatten().map(|a| json!({"name": a["name"], "step": a["step"], "executor": a["executor"]["kind"], "opens": a["opens"]["protocol"]})).collect::<Vec<_>>(),
        }));
        out.insert(format!("{}.json", p.stem), pretty(&doc));
    }
    let opened: BTreeSet<&str> = inp.protocols.iter().flat_map(|p| p.actions.iter().filter_map(|a| a.opens.as_ref().map(|o| o.0.as_str()))).collect();
    let index = json!({
        "format": FORMAT,
        "written_by": "sdc-protocol-site write",
        "repository": format!("https://github.com/{REPOSITORY}"),
        "release": inp.releases.first().map(|r| json!({"version": r.version, "date": r.date, "tag": r.version, "notes": r.notes})),
        "releases": inp.releases.iter().map(|r| json!({"version": r.version, "date": r.date, "summary": r.summary, "limits": r.limits,
            "notes": r.notes, "notes_url": r.notes.as_ref().map(|n| source_url(Some(&r.version), n))})).collect::<Vec<_>>(),
        "protocols": summaries,
        "roots": inp.protocols.iter().filter(|p| !opened.contains(p.id.as_str())).map(|p| p.id.clone()).collect::<Vec<_>>(),
        "flow": flow,
        "sources": inp.sources.iter().map(|(p, h)| json!({"path": p, "sha256": h})).collect::<Vec<_>>(),
    });
    out.insert("index.json".to_string(), pretty(&index));
    out
}

fn pretty(v: &J) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default() + "\n"
}

// ---- write and check ---------------------------------------------------------------------------

fn on_disk(dir: &Path) -> BTreeMap<String, String> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            name.ends_with(".json").then(|| (name, fs::read_to_string(e.path()).unwrap_or_default()))
        })
        .collect()
}

fn drift(want: &BTreeMap<String, String>, have: &BTreeMap<String, String>) -> Vec<String> {
    let mut p = vec![];
    for (name, text) in want {
        match have.get(name) {
            None => p.push(format!("{OUT_DIR}/{name}: missing; run sdc-protocol-site write")),
            Some(t) if t != text => p.push(format!("{OUT_DIR}/{name}: stale; run sdc-protocol-site write")),
            _ => {}
        }
    }
    for name in have.keys().filter(|n| !want.contains_key(*n)) {
        p.push(format!("{OUT_DIR}/{name}: no longer generated; delete it or run sdc-protocol-site write"));
    }
    p
}

fn write(root: &Path) -> Result<Vec<String>, String> {
    let want = render(&load(root)?);
    let dir = root.join(OUT_DIR);
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let have = on_disk(&dir);
    let mut log = vec![];
    for (name, text) in &want {
        if have.get(name) != Some(text) {
            fs::write(dir.join(name), text).map_err(|e| format!("cannot write {name}: {e}"))?;
            log.push(format!("wrote {OUT_DIR}/{name}"));
        }
    }
    for name in have.keys().filter(|n| !want.contains_key(*n)) {
        fs::remove_file(dir.join(name)).map_err(|e| format!("cannot remove {name}: {e}"))?;
        log.push(format!("removed {OUT_DIR}/{name}"));
    }
    Ok(log)
}

fn check(root: &Path) -> Result<Vec<String>, String> {
    Ok(drift(&render(&load(root)?), &on_disk(&root.join(OUT_DIR))))
}

fn valid_commit(commit: &str) -> bool {
    commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) && commit.bytes().any(|b| b != b'0')
}

fn site_manifest(commit: &str) -> String {
    pretty(&json!({"schema": "b10x-project-site/v1", "repository": "runbook", "commit": commit, "baseUrl": BASE_URL}))
}

fn write_site_manifest(out: &Path, commit: &str) -> Result<PathBuf, String> {
    if !valid_commit(commit) {
        return Err("--commit must be a nonzero full Git revision".into());
    }
    if !out.join("index.html").is_file() {
        return Err(format!("{} holds no built site", out.display()));
    }
    let dir = out.join(".well-known");
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let path = dir.join("b10x-site.json");
    fs::write(&path, site_manifest(commit)).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(path)
}

// ---- command line ------------------------------------------------------------------------------

const USAGE: &str = "usage: sdc-protocol-site write|check [--root <repository>]\n       sdc-protocol-site site-manifest --out <built site> --commit <full revision>";

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = PathBuf::from(flag(&args, "--root").unwrap_or_else(|| ".".into()));
    match args.first().map(String::as_str) {
        Some("write") => match write(&root) {
            Ok(log) => {
                log.iter().for_each(|l| println!("{l}"));
                println!("protocol site data: ok");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("sdc-protocol-site: {e}");
                ExitCode::from(2)
            }
        },
        Some("check") => match check(&root) {
            Ok(p) if p.is_empty() => {
                println!("protocol site data: current");
                ExitCode::SUCCESS
            }
            Ok(p) => {
                p.iter().for_each(|l| println!("{l}"));
                ExitCode::from(1)
            }
            Err(e) => {
                eprintln!("sdc-protocol-site: {e}");
                ExitCode::from(2)
            }
        },
        Some("site-manifest") => match (flag(&args, "--out"), flag(&args, "--commit")) {
            (Some(out), Some(commit)) => match write_site_manifest(Path::new(&out), &commit) {
                Ok(p) => {
                    println!("wrote {}", p.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("sdc-protocol-site: {e}");
                    ExitCode::from(1)
                }
            },
            _ => {
                eprintln!("{USAGE}");
                ExitCode::from(64)
            }
        },
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"format: protocol/1
protocol:
  id: demo
  revision: 3
  description: >-
    A demo parent. It opens one child.
artifacts:
  thing:
    description: the thing
evidence_kinds:
  thing_check:
    description: a check of the thing
  child_opened:
    description: the child was opened
  approval:
    description: a person's approval
claims:
  thing.valid:
    description: the thing is valid
    true_when:
      evidence:
        kind: thing_check
        result: valid
        subject: thing
  child.opened:
    description: the child was opened
    true_when:
      evidence:
        kind: child_opened
        subject: thing
  approved:
    description: approved by someone else
    true_when:
      evidence:
        kind: approval
        result: approved
        subject: thing
        independent_on: [different_principal]
  children.passed:
    description: every child passed
    true_when:
      children:
        in: [passed]
obligations:
  valid.required:
    description: the thing is valid
    discharged_when:
      claim: thing.valid
actions:
  thing.check:
    description: check the thing
    effect: read
    precondition:
      claim: thing.valid
      is: unknown
    may_produce:
      - evidence: thing_check
  child.open:
    description: open the child once the thing is valid
    effect: write
    precondition:
      all:
        - claim: thing.valid
        - claim: child.opened
          is: unknown
    case_open:
      protocol: demo.child
      subject: thing
    may_produce:
      - evidence: child_opened
  thing.approve:
    description: approve the thing
    effect: write
    precondition:
      claim: children.passed
    requires:
      - capability: owner.approve
    may_produce:
      - evidence: approval
outcomes:
  passed:
    description: valid, approved, and the child passed
    requires:
      all:
        - claim: approved
        - claim: children.passed
  abandoned:
    description: given up
    requires:
      decision: explicitly_abandoned
  failed:
    description: the child failed
    requires:
      not:
        claim: children.passed
        is: unknown
"#;

    const CHILD: &str = "format: protocol/1\nprotocol:\n  id: demo.child\n  revision: 1\n  description: The child.\nactions:\n  work.do:\n    description: do it\n    effect: write\n    may_produce:\n      - evidence: work\nevidence_kinds:\n  work:\n    description: work\nclaims:\n  done:\n    description: done\n    true_when:\n      evidence:\n        kind: work\noutcomes:\n  passed:\n    description: done\n    requires:\n      claim: done\n";

    const BINDINGS: &str = r#"{"$independence": {"reviewer": ["different_principal"]},
      "thing.check": {"argv": ["{dir}/../../../bindings/tools/sdc-tool.sh", "thing-check"], "produces": "thing_check", "producer": "tool", "results": {"0": "valid", "*": "invalid"}},
      "child.open": {"child": {"name": "child", "protocol": "x", "bindings": "y"}}}"#;

    const NODE_CLASSES: &str = r#"{"classes": {"writer": ["thing.write"]}}"#;
    const DECISION_CLASSES: &str = r#"{"classes": {"owner": {"type": "approval", "side": "requester", "actions": ["demo/thing.approve"]}, "close": {"type": "close", "actions": ["demo/close:passed"]}}}"#;

    fn scratch(name: &str) -> PathBuf {
        let d = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-scratch/sdc-protocol-site").join(name);
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn fixture_repo(name: &str) -> PathBuf {
        let root = scratch(name);
        for (rel, text) in [
            ("protocol/demo.protocol.yaml", FIXTURE),
            ("protocol/demo-child.protocol.yaml", CHILD),
            ("driver/examples/sdc/demo.bindings.json", BINDINGS),
            ("bindings/tools/node-classes.json", NODE_CLASSES),
            ("driver/examples/sdc/decision-classes.json", DECISION_CLASSES),
            ("docs/check-output.txt", "checked: protocol `demo` revision 3: 96 states, 0 properties, 0 findings\nchecked: protocol `demo` revision 3: 96 states, 0 properties, 1 finding\nchecked: protocol `demo.child` revision 0: 4 states, 0 properties, 0 findings\n"),
            ("protocol/simulate/demo.txt", "stuck-state: x\nsimulated: protocol `demo` revision 3: 12 reachable states, 4 terminable (outcome `abandoned` 3, outcome `passed` 1), 2 findings (3 stuck or cycle states)\n"),
            ("protocol/simulate/CANON_REV", "abc1234\n"),
            ("CHANGELOG.md", "# Changelog\n\n## [Unreleased]\n\n## [0.2.0] - 2026-11-22\n\nx\n\n## [0.1.0] - 2026-11-21\n"),
            ("docs/releases/0.2.0.md", "# demo 0.2.0\n\nThe second release.\nTwo lines.\n\n### Limits\n\n- One limit,\n  continued.\n- `Two`.\n\n### After\n\n- not a limit\n"),
        ] {
            let p = root.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text).unwrap();
        }
        root
    }

    fn doc(out: &BTreeMap<String, String>, name: &str) -> J {
        serde_json::from_str(&out[name]).unwrap()
    }

    #[test]
    fn an_unreleased_changelog_line_leaves_the_data_current_and_a_release_does_not() {
        let root = fixture_repo("unreleased");
        let before = render(&load(&root).unwrap());
        let changelog = root.join("CHANGELOG.md");
        let text = fs::read_to_string(&changelog).unwrap();
        fs::write(&changelog, text.replace("## [Unreleased]\n", "## [Unreleased]\n\n### Added\n\n- a line nobody released\n")).unwrap();
        assert_eq!(render(&load(&root).unwrap()), before);
        fs::write(&changelog, text.replace("## [Unreleased]\n", "## [Unreleased]\n\n## [0.3.0] - 2026-11-23\n\ny\n")).unwrap();
        let after = render(&load(&root).unwrap());
        assert_ne!(after, before);
        assert_eq!(doc(&after, "index.json")["release"]["version"], "0.3.0");
    }

    #[test]
    fn predicate_forms_read_as_lines() {
        let y: Y = serde_yaml::from_str("all:\n  - claim: a\n  - claim: b\n    is: unknown\n  - evidence: {kind: k, result: passed, subject: spec, independent_on: [different_principal]}\n  - any:\n      - children: {in: [passed]}\n      - decision: explicitly_abandoned\n  - not: {claim: c}\n").unwrap();
        let text: Vec<String> = pred_lines(&pred(&y))
            .iter()
            .map(|l| format!("{}{} {} {}", "  ".repeat(l["depth"].as_u64().unwrap() as usize), l["text"].as_str().unwrap(), l["reference"].as_str().unwrap_or(""), l["suffix"].as_str().unwrap_or("")).trim_end().to_string())
            .collect();
        assert_eq!(
            text,
            [
                "all of",
                "  claim claim:a",
                "  claim is unknown: claim:b",
                "  evidence = passed: evidence:k about spec, by a producer independent on different_principal",
                "  any of",
                "    every child case ended  in passed",
                "    decision taken: decision:explicitly_abandoned",
                "  not",
                "    claim claim:c",
            ]
        );
    }

    #[test]
    fn the_fixture_protocol_gives_its_actions_records_ends_and_child() {
        let root = fixture_repo("model");
        let out = render(&load(&root).unwrap());
        assert_eq!(out.keys().collect::<Vec<_>>(), ["demo-child.json", "demo.json", "index.json"]);
        let d = doc(&out, "demo.json");
        assert_eq!(d["id"], "demo");
        assert_eq!(d["revision"], 3);
        assert_eq!(d["summary"], "A demo parent.");
        let names: Vec<&str> = d["actions"].as_array().unwrap().iter().map(|a| a["name"].as_str().unwrap()).collect();
        // thing.check waits for nothing; child.open waits for thing.check; thing.approve only on
        // the children form, which is no wait.
        assert_eq!(names, ["thing.approve", "thing.check", "child.open"]);
        let open = &d["actions"][2];
        assert_eq!(open["step"], 1);
        assert_eq!(open["waits_for"], json!(["thing.check"]));
        assert_eq!(open["opens"], json!({"protocol": "demo.child", "subject": "thing"}));
        assert_eq!(open["executor"]["kind"], "child-case");
        assert_eq!(d["children"][0]["stem"], "demo-child");
        let edges: BTreeSet<String> = d["graph"]["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| format!("{} -{}-> {} {}", e["from"].as_str().unwrap(), e["kind"].as_str().unwrap(), e["to"].as_str().unwrap(), e["qualifier"].as_str().unwrap_or("")).trim_end().to_string())
            .collect();
        for want in [
            "action:thing.check -produces-> evidence:thing_check",
            "action:child.open -opens-> child:demo.child",
            "evidence:thing_check -establishes-> claim:thing.valid valid",
            "claim:thing.valid -gates-> action:thing.check unknown",
            "claim:thing.valid -gates-> action:child.open",
            "child:demo.child -reports-> claim:children.passed in passed",
            "claim:approved -requires-> outcome:passed",
            "decision:explicitly_abandoned -decides-> outcome:abandoned",
            "claim:children.passed -requires-> outcome:failed not unknown",
        ] {
            assert!(edges.contains(want), "missing {want} in {edges:#?}");
        }
        assert_eq!(d["check"]["states"], 96);
        assert_eq!(d["simulate"]["findings"], 2);
        assert_eq!(d["simulate"]["by_outcome"]["abandoned"], 3);
        assert_eq!(d["simulate"]["canon"], "abc1234");
        let child = doc(&out, "demo-child.json");
        assert_eq!(child["parents"], json!([{"protocol": "demo", "stem": "demo", "action": "child.open"}]));
        assert_eq!(child["check"]["states"], J::Null, "revision 0 in the check output is not revision 1");
        assert_eq!(child["simulate"]["findings"], J::Null);
        assert_eq!(child["actions"][0]["executor"], json!({"kind": "person", "bound": false, "note": "no bindings file for this protocol"}));
        let index = doc(&out, "index.json");
        assert_eq!(index["roots"], json!(["demo"]));
        assert_eq!(index["flow"][0]["child"], "demo.child");
        assert_eq!(index["release"]["version"], "0.2.0");
        assert_eq!(d["source"]["url"], "https://github.com/beyond10x/runbook/blob/0.2.0/protocol/demo.protocol.yaml");
    }

    #[test]
    fn executors_come_from_the_bindings_and_the_class_tables() {
        let root = fixture_repo("executors");
        let out = render(&load(&root).unwrap());
        let d = doc(&out, "demo.json");
        let by = |n: &str| d["actions"].as_array().unwrap().iter().find(|a| a["name"] == n).unwrap().clone();
        assert_eq!(by("thing.check")["executor"], json!({"kind": "program", "program": "sdc-tool.sh thing-check", "producer": "tool", "results": {"0": "valid", "*": "invalid"}}));
        let approve = by("thing.approve");
        assert_eq!(approve["executor"], json!({"kind": "person", "bound": false}));
        assert_eq!(approve["capabilities"], json!(["owner.approve"]));
        assert_eq!(approve["decision_class"], json!({"class": "owner", "type": "approval", "side": "requester"}));
        let passed = d["outcomes"].as_array().unwrap().iter().find(|o| o["name"] == "passed").unwrap();
        assert_eq!(passed["decision_class"]["class"], "close");

        let ctx = Context { node_classes: [("w".to_string(), "writer".to_string())].into(), decision_classes: BTreeMap::new() };
        let p = parse_protocol("demo", "x", FIXTURE).unwrap();
        let a = Action { name: "w".into(), description: String::new(), effect: "write".into(), precondition: None, capabilities: vec![], produces: vec![], opens: None, arguments: vec![] };
        let b = json!({"$independence": {"llm-review": []}, "w": {"argv": ["{dir}/../../../bindings/tools/llm-node.sh", "w"], "llm": {"model": "m", "operations": ["file.read"]}, "producer": "llm-review", "max_runs": 3}});
        assert_eq!(
            executor(&p, &a, Some(&b), &ctx),
            json!({"kind": "model", "program": "llm-node.sh w", "node_class": "writer", "default_model": "m", "operations": ["file.read"], "producer": "llm-review", "independent": true, "max_runs": 3})
        );
        let b = json!({"w": {"manual": true, "argv": ["true"]}});
        assert_eq!(executor(&p, &a, Some(&b), &ctx), json!({"kind": "person", "bound": true}));
        let b = json!({"w": {"copy": {"kind": "approval", "from": "sdc.direction"}}});
        assert_eq!(executor(&p, &a, Some(&b), &ctx), json!({"kind": "copy", "from": "sdc.direction", "record": "approval"}));
    }

    #[test]
    fn a_change_of_the_protocol_changes_the_output() {
        let root = fixture_repo("change");
        let before = render(&load(&root).unwrap());
        let changed = FIXTURE.replace("      - evidence: thing_check\n", "      - evidence: thing_check\n      - evidence: approval\n");
        fs::write(root.join("protocol/demo.protocol.yaml"), changed).unwrap();
        let after = render(&load(&root).unwrap());
        assert_ne!(before["demo.json"], after["demo.json"]);
        assert_ne!(before["index.json"], after["index.json"], "the sources' digests change too");
        let d = doc(&after, "demo.json");
        let check = d["actions"].as_array().unwrap().iter().find(|a| a["name"] == "thing.check").unwrap();
        assert_eq!(check["produces"], json!(["thing_check", "approval"]));
    }

    #[test]
    fn check_fails_on_a_stale_a_missing_and_an_extra_file() {
        let root = fixture_repo("check");
        assert_eq!(check(&root).unwrap().len(), 3, "nothing written yet: three files missing");
        write(&root).unwrap();
        assert!(check(&root).unwrap().is_empty());
        assert!(write(&root).unwrap().is_empty(), "a second write changes nothing");
        fs::write(root.join("protocol/demo.protocol.yaml"), FIXTURE.replace("revision: 3", "revision: 4")).unwrap();
        fs::write(root.join(OUT_DIR).join("old.json"), "{}").unwrap();
        let p = check(&root).unwrap();
        assert!(p.iter().any(|l| l.contains("demo.json: stale")), "{p:?}");
        assert!(p.iter().any(|l| l.contains("old.json: no longer generated")), "{p:?}");
        write(&root).unwrap();
        assert!(check(&root).unwrap().is_empty());
        assert!(!root.join(OUT_DIR).join("old.json").exists());
    }

    #[test]
    fn check_and_simulate_lines_are_read_by_protocol_and_revision() {
        let c = parse_check_output("valid: x\nchecked: protocol `a.b` revision 7: 49152 states, 0 properties, 0 findings\nchecked: protocol `a.b` revision 7: 1 states, 0 properties, 1 finding\n");
        assert_eq!(c["a.b"], Checked { revision: 7, states: 49152 });
        let s = parse_simulate("simulated: protocol `sdc` revision 8: 78 reachable states, 54 terminable (outcome `abandoned` 39, outcome `operating` 8), 10 waiting on supplied evidence, a child outcome or expiry, 1 finding (1 stuck or cycle state)\n").unwrap();
        assert_eq!((s.reachable, s.terminable, s.findings, s.stuck_or_cycle), (78, 54, 1, 1));
        assert_eq!(s.by_outcome["operating"], 8);
    }

    #[test]
    fn releases_come_from_the_changelog_and_the_notes() {
        assert_eq!(parse_changelog("## [Unreleased]\n## [0.1.1] - 2026-11-21\n## [0.1.0] - 2026-11-20\n"), [("0.1.1".to_string(), "2026-11-21".to_string()), ("0.1.0".to_string(), "2026-11-20".to_string())]);
        let (s, l) = parse_notes("# x 1\n\nFirst para\nwraps.\n\n### Limits\n\n- a,\n  b.\n- c.\n\n### Other\n- d\n");
        assert_eq!(s, "First para wraps.");
        assert_eq!(l, ["a, b.", "c."]);
        let (s, _) = parse_notes("No title here.\n\n### What\n");
        assert_eq!(s, "No title here.");
        assert!(version_key("0.10.0") > version_key("0.9.1"));
    }

    #[test]
    fn the_site_manifest_names_the_project_site_and_its_commit() {
        let commit = "0123456789abcdef0123456789abcdef01234567";
        assert_eq!(site_manifest(commit), format!("{{\n  \"baseUrl\": \"/runbook/\",\n  \"commit\": \"{commit}\",\n  \"repository\": \"runbook\",\n  \"schema\": \"b10x-project-site/v1\"\n}}\n"));
        assert!(valid_commit(commit));
        assert!(!valid_commit(&"0".repeat(40)));
        assert!(!valid_commit("abc123"));
        let site = scratch("manifest");
        assert!(write_site_manifest(&site, commit).is_err(), "no index.html");
        fs::write(site.join("index.html"), "x").unwrap();
        let p = write_site_manifest(&site, commit).unwrap();
        assert_eq!(fs::read_to_string(p).unwrap(), site_manifest(commit));
    }

    #[test]
    fn the_committed_site_data_is_current() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
        let p = check(&root).unwrap();
        assert!(p.is_empty(), "run sdc-protocol-site write: {p:?}");
    }
}
