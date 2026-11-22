//! `runbook parity`: compare a run under the old driver with a run under Loom. This module holds the
//! runtime-neutral summary (`runbook.parity-summary/1`), the reader of the driver's side, the
//! comparison with its rules table, and the list of person steps a second runtime must repeat.
//! The reader of Loom's side reads the JSON `tree status` printed and the product tree, nothing else.

use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

pub const FORMAT: &str = "runbook.parity-summary/1";

/// The format of a `tree status` document the Loom reader accepts.
pub const LOOM_STATUS_FORMAT: &str = "loom.tree-status/1";

/// Directory names left out of the product tree, at any depth.
const EXCLUDED_DIRS: [&str; 3] = ["target", "node_modules", ".git"];

/// Workspace directories that only carry facts to a program under Loom, relative to the product
/// tree: the retry feedback `sdc-node` copies (`.sdc/loom-feedback`) and the record files
/// `sdc-reply record` writes, since Loom gives a program no case store (`.sdc/records`; the log
/// of a test run in them has the test order and the elapsed time of that run). Left out of the
/// listing of both runtimes (rules `product_tree[.sdc/loom-feedback/**]` and
/// `product_tree[.sdc/records/**]`).
const TRANSPORT_DIRS: [&str; 2] = [".sdc/loom-feedback", ".sdc/records"];

/// The kind of the record a child's outcome leaves in its parent (both readers name it so; the
/// summary holds no other mark of the action that opened the child).
const OUTCOME_KIND: &str = "case_outcome";

/// The registration stem of a protocol id: `.` and `_` written as `-`.
fn stem(id: &str) -> String {
    id.replace(['.', '_'], "-")
}

/// What a summary is read from (story `parity-run-on-loom`).
pub enum Source {
    Driver { run: PathBuf, store: PathBuf },
    /// A file holding the JSON `tree status` printed; the product tree is `product_tree`, or the
    /// status's `workspace` when none is given.
    Loom { status: PathBuf, product_tree: Option<PathBuf> },
    /// The run directory `runbook run` wrote: its `tree-status.json` and `work/`, and what only
    /// that directory holds (`run-settings.jsonl`, `decisions.jsonl`, `protocols/`).
    LoomRun { dir: PathBuf },
}

/// Read a finished run into a summary.
pub fn read(source: &Source) -> Result<Value, String> {
    match source {
        Source::Driver { run, store } => read_driver(run, store),
        Source::Loom { status, product_tree } => read_loom(status, product_tree.as_deref(), None),
        Source::LoomRun { dir } => {
            let run = read_run_info(dir)?;
            read_loom(&dir.join("tree-status.json"), Some(&dir.join("work")), Some(&run))
        }
    }
}

// ---------------------------------------------------------------- the rules table

#[derive(Clone, Copy, PartialEq)]
enum How {
    Value,
    Pattern(&'static str),
    Digest,
    /// Equal when the registration stems are equal: `.` and `_` read as `-` on both sides.
    Stem,
    Never,
}

struct Rule {
    member: &'static str,
    how: How,
    why: &'static str,
    /// Where the Loom reader finds the member in `tree status`, or `NOT_RECORDED` or `UNDER`.
    loom: &'static str,
    /// The rule that turns Loom's member into this one.
    map: &'static str,
    /// Loom's word and the summary's word for it, where the two runtimes name one thing apart.
    words: &'static [(&'static str, &'static str)],
}

const fn rule(member: &'static str, how: How, why: &'static str, loom: &'static str, map: &'static str) -> Rule {
    Rule { member, how, why, loom, map, words: &[] }
}

const fn rule_with(r: Rule, words: &'static [(&'static str, &'static str)]) -> Rule {
    Rule { words, ..r }
}

/// The summary's word for Loom's `loom_word` under `member`: the one place the vocabularies meet.
fn word(member: &str, loom_word: &str) -> Option<&'static str> {
    rule_for(member)?.words.iter().find(|(l, _)| *l == loom_word).map(|(_, s)| *s)
}

/// What the Loom column says of a member `tree status` holds nothing for: the Loom reader lists it
/// under `not_recorded` and `compare` reports it as "not recorded on one side".
const NOT_RECORDED: &str = "(not recorded)";
/// What the Loom column says of a member under a member that is itself not recorded.
const UNDER: &str = "(under a member not recorded)";
/// What the Loom column says of a member that only the run directory of `runbook run` holds
/// (`summarize --loom-run`); a summary read from the status alone lists it under `not_recorded`.
const FROM_SETTINGS: &str = "run directory: run-settings.jsonl";
const FROM_DECISIONS: &str = "run directory: decisions.jsonl";
const FROM_PROTOCOLS: &str = "run directory: protocols/*.protocol.yaml";

/// The one table of what is compared and how, and where the Loom reader finds each member and by
/// which rule. `[]` stands for any list index, `*` for any key of `upstream_revisions` (an artifact
/// name). `runbook parity compare --rules` prints it; `docs/parity.md` repeats the member and the
/// last two columns, and a test holds the two equal.
const RULES: &[Rule] = &[
    rule("format", How::Value, "the format id", "format", "must be loom.tree-status/1, else refused by name; the summary's format is its own"),
    rule("source", How::Never, "names the runtime and its settings; it differs by design", "format, version", "runtime loom, status_version the version; no path of the store is copied"),
    rule("source.run_settings", How::Never, "the settings of the run (decider, adapter, stand-in)", FROM_SETTINGS, "the first line, read as decider kind, adapter, stand_in (a decider that is not a model) and its principal; tree status alone holds no run settings (not recorded)"),
    rule("not_recorded", How::Never, "what one runtime does not record; read it before trusting an empty list", "the rows of this table marked not recorded", "listed in the order of this table"),
    rule("cases[].id", How::Pattern("c"), "a case id is generated by the run", "cases[].case", "as is; Loom ids (root, root.first) and the driver's compare by pattern"),
    rule("cases[].link", How::Value, "the name of the case link: what the protocol's binding calls the child", "cases[].name", "as is; none on the root (null); another case without a name is an error"),
    rule("cases[].parent", How::Pattern("c"), "a case id", "cases[].parent", "as is; none on the root only; a parent that is not a case of the tree is an error"),
    rule("cases[].protocol", How::Stem, "the protocol id, compared as its registration stem", "cases[].protocol_id", "as is (the id without @revision); compared by registration stem"),
    rule("cases[].protocol_revision", How::Value, "the protocol's own revision", "cases[].protocol_revision", "text to integer; anything else is an error"),
    rule("cases[].outcome", How::Value, "the way the case ended", "cases[].outcome", "as is; none is null (the case runs on)"),
    rule("cases[].stop", How::Value, "null while the case ended or runs on", "cases[].stop", "null when the case has an outcome (its last stop is then old); none without an outcome is null"),
    rule_with(
        rule("cases[].stop.cause", How::Value, "why the case waits", "cases[].stop", "the stop's name in the words of the summary; another name is an error"),
        &[("AwaitingPerson", "awaiting_person"), ("AwaitingAuthority", "awaiting_authority"), ("Stuck", "stopped"), ("BudgetSpent", "budget_spent")],
    ),
    rule("cases[].open_obligations[]", How::Value, "obligation ids of the last evaluation", "cases[].open_obligations[]", "as is"),
    rule("cases[].evidence[].index", How::Value, "the order received", "cases[].evidence[].index", "as is"),
    rule("cases[].evidence[].kind", How::Value, "evidence kind", "cases[].evidence[].kind", "as is"),
    rule("cases[].evidence[].result", How::Value, "evidence result", "cases[].evidence[].result", "as is; none is null"),
    rule("cases[].evidence[].subject", How::Value, "artifact name", "cases[].evidence[].subject", "as is"),
    rule("cases[].evidence[].subject_revision", How::Pattern("r"), "a revision is generated by the run", "cases[].evidence[].subject_revision", "as is"),
    rule("cases[].evidence[].upstream_revisions.*", How::Pattern("r"), "a revision is generated by the run", "cases[].evidence[].upstream_revisions.*", "as is"),
    rule("cases[].evidence[case_outcome].subject_revision", How::Never, "the record a child's outcome leaves in its parent: the driver keeps the parent's revision of the subject at the child's opening, Loom replaces it at the child's termination; neither compared nor counted when the labels r1, r2, ... are given", "cases[].evidence[].subject_revision", "taken out of both summaries before the labels are named; a known difference of the two runtimes (docs/parity.md)"),
    rule("cases[].evidence[case_outcome].upstream_revisions.*", How::Never, "as the subject_revision of the same record: the driver keeps the revision of the opening, Loom replaces it at the termination", "cases[].evidence[].upstream_revisions.*", "taken out of both summaries before the labels are named, as the subject_revision"),
    rule("cases[].evidence[].action", How::Value, "the action that produced the record", "cases[].evidence[].action", "as is, none is null; a case_outcome record with none takes the action of the next record (the opening step's own)"),
    rule("cases[].program_runs[].action", How::Value, "action name", "cases[].runs[].action", "as is"),
    rule("cases[].program_runs[].attempt", How::Value, "n-th run of this action in this case", "(counted)", "n-th run of the action in the case's runs[]; Loom's attempt counts within a retry chain and is not read"),
    rule("cases[].program_runs[].exit", How::Value, "exit status of the program", "cases[].runs[].exit", "as is; none is an error (no status to compare)"),
    rule("cases[].program_runs[].retry_followed", How::Value, "whether the next run of the action was a retry", "cases[].runs[].retried", "as is"),
    rule("cases[].person_steps[].order", How::Value, "position among all person steps of the run", "cases[].approvals[].at", "position by (at, case position, approval position) over every approval of the tree"),
    rule("cases[].person_steps[].action", How::Value, "action name", "cases[].approvals[].action", "as is; none is null (a grant)"),
    rule("cases[].person_steps[].capability", How::Value, "the capability the action requires", "cases[].approvals[].capability", "as is"),
    rule_with(
        rule("cases[].person_steps[].answer", How::Value, "the answer or approval that ended the step", "cases[].approvals[].answer", "Allow with an action: the result of the n-th record of that action in the case; Allow without: granted; Deny: denied"),
        &[("Allow", "granted"), ("Deny", "denied")],
    ),
    rule("cases[].person_steps[].actor", How::Value, "who took the step", "cases[].approvals[].actor", "as is; equal only when the Loom run answered with the principal the driver run used: under `runbook run` that is `decider.principal` of the settings for a scripted decider (default of its answers), `decider:model` or `decider:<verdict>` for a model or rule decider, and the principal Loom printed (`operator` for a capability approval) when the settings name none"),
    rule("copies[].action", How::Value, "the copy action", "cases[].evidence[].action", "the n-th record of copies[].kind in the target case whose action the source case's records of that kind do not name; the entry itself names source_case, target_case and record_kind only"),
    rule("copies[].kind", How::Value, "evidence kind copied", "cases[].copies[].record_kind", "as is"),
    rule("copies[].from_protocol", How::Stem, "protocol of the source case, compared as its registration stem", "cases[].copies[].source_case", "protocol_id of that case; compared by registration stem"),
    rule("copies[].from", How::Pattern("c"), "a case id", "cases[].copies[].source_case", "as is"),
    rule("copies[].to", How::Pattern("c"), "a case id", "cases[].copies[].target_case", "as is"),
    rule("copies[].subject", How::Value, "artifact name", "cases[].evidence[].subject", "that record of the target case"),
    rule("copies[].subject_revision", How::Pattern("r"), "a revision is generated by the run", "cases[].evidence[].subject_revision", "that record of the target case"),
    rule("decision_requests", How::Value, "the decisions recorded in the run", FROM_DECISIONS, "one entry per answered decision that is a question; the runner answers approvals only, so the list is empty; a row of type question or finding (the decider answering a request of the answers file) is read and compared to nothing, a row of another type is an error; tree status alone holds no decision record (not recorded)"),
    rule("decision_requests[].id", How::Pattern("q"), "a request id is generated by the run", UNDER, "an empty list"),
    rule("decision_requests[].case", How::Pattern("c"), "a case id", UNDER, "an empty list"),
    rule("decision_requests[].question", How::Value, "the question", UNDER, "an empty list"),
    rule("decision_requests[].options[]", How::Value, "the options", UNDER, "an empty list"),
    rule("decision_requests[].answer", How::Value, "the answer", UNDER, "an empty list"),
    rule("decision_requests[].who", How::Value, "who answered", UNDER, "an empty list"),
    rule("protocols[].id", How::Stem, "the protocol id, compared as its registration stem", "cases[].protocol_id", "the distinct protocols of the cases, by id then revision; compared by registration stem"),
    rule("protocols[].revision", How::Value, "the protocol's own revision", "cases[].protocol_revision", "as for the case"),
    rule("protocols[].digest", How::Pattern("d"), "each runtime digests its own form of the protocol", FROM_PROTOCOLS, "sha256 of the file the runner copied for that id and revision; the status protocols[] is empty and is not read (not recorded)"),
    rule("product_tree[].path", How::Value, "the set of files: a path missing on one side is a difference", "--product-tree, else workspace", "the directory walked as the driver's work/ is"),
    rule("product_tree[.sdc/loom-feedback/**]", How::Never, "the retry feedback that Loom's sdc-node copies into the workspace; the driver hands the same feedback over in a variable and writes no file: transport, not product", "--product-tree, else workspace", "left out of the listing of both sides when the summary is written"),
    rule("product_tree[.sdc/records/**]", How::Never, "the record files `sdc-reply record` writes into the workspace for the inputs of change.review, since Loom gives a program no case store; the driver keeps the same facts in its case store and writes no file; the log of a test run in them carries the test order and the elapsed time of that run: transport, not product", "--product-tree, else workspace", "left out of the listing of both sides when the summary is written"),
    rule("product_tree[].sha256", How::Digest, "compared only with --product-digests (a stub run is deterministic, a model run is not); stamps like 2026-11-20T12:00:00Z in text files are read as <time> (aep writes the wall clock into planning files)", "--product-tree, else workspace", "the same digest of the same file"),
];

fn rule_for(tpl: &str) -> Option<&'static Rule> {
    RULES.iter().find(|r| r.member == tpl)
}

/// The rule of a row for the Loom reader, with the pairs of words it holds.
fn loom_rule(r: &Rule) -> String {
    if r.words.is_empty() {
        return r.map.to_string();
    }
    let pairs: Vec<String> = r.words.iter().map(|(l, s)| format!("{l} = {s}")).collect();
    format!("{} ({})", r.map, pairs.join(", "))
}

pub fn rules_text() -> String {
    let mut s = String::from("member | how | why | loom member | loom rule\n");
    for r in RULES {
        let how = match r.how {
            How::Value => "equal by value".to_string(),
            How::Pattern(p) => format!("equal by pattern ({p}1, {p}2, ... in order of first appearance)"),
            How::Digest => "equal by value only with --product-digests".to_string(),
            How::Stem => "equal by registration stem (. and _ read as -)".to_string(),
            How::Never => "never compared".to_string(),
        };
        s.push_str(&format!("{} | {} | {} | {} | {}\n", r.member, how, r.why, r.loom, loom_rule(r)));
    }
    s.push_str("list members: by position, except product_tree, which is by path; a record present on one side only is a difference\n");
    s
}

// ---------------------------------------------------------------- sha-256

pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------- the driver's reader
//
// Every input the reader needs is read or it is an error that names the file: nothing unreadable,
// malformed or missing is dropped or replaced by a default, so a damaged store never compares equal
// to an empty one.

fn read_json(p: &Path) -> Result<Value, String> {
    let t = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    serde_json::from_str(&t).map_err(|e| format!("{}: {e}", p.display()))
}

/// The newest evaluation file of a case: the one whose name sorts last.
fn latest_evaluation(dir: &Path) -> Result<Value, String> {
    let evals = dir.join("evaluations");
    let rd = std::fs::read_dir(&evals).map_err(|e| format!("{}: {e}", evals.display()))?;
    let mut files = Vec::new();
    for e in rd {
        files.push(e.map_err(|e| format!("{}: {e}", evals.display()))?.path());
    }
    match files.into_iter().max() {
        Some(p) => read_json(&p),
        None => Err(format!("{}: no evaluation file", evals.display())),
    }
}

fn read_events(dir: &Path) -> Result<Vec<Value>, String> {
    let p = dir.join("events.jsonl");
    let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut out = Vec::new();
    for (i, l) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        out.push(serde_json::from_str(l).map_err(|e| format!("{}:{}: {e}", p.display(), i + 1))?);
    }
    Ok(out)
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).map(String::from)
}

fn jstr(v: Option<String>) -> Value {
    v.map_or(Value::Null, Value::String)
}

/// A case directory of the store: the case file, its events, and the cases opened below it, in the
/// order the log opened them.
struct Case {
    dir: PathBuf,
    id: String,
    parent: Option<String>,
    events: Vec<Value>,
    case: Value,
}

fn collect_cases(dir: &Path, parent: Option<&str>, out: &mut Vec<Case>) -> Result<(), String> {
    let case = read_json(&dir.join("case.json"))?;
    let id = s(&case, "id").ok_or_else(|| format!("{}: case.json has no id", dir.display()))?;
    let events = read_events(dir)?;
    let mut kids: Vec<String> =
        events.iter().filter(|e| e["event"] == "child_opened").filter_map(|e| s(e, "child_case")).collect();
    if let Ok(rd) = std::fs::read_dir(dir.join("children")) {
        let mut rest: Vec<String> = rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().join("case.json").is_file())
            .filter_map(|e| e.file_name().to_str().map(String::from))
            .filter(|n| !kids.contains(n))
            .collect();
        rest.sort();
        kids.extend(rest);
    }
    out.push(Case { dir: dir.to_path_buf(), id: id.clone(), parent: parent.map(String::from), events, case });
    for k in kids {
        let d = dir.join("children").join(&k);
        if d.join("case.json").is_file() {
            collect_cases(&d, Some(&id), out)?;
        }
    }
    Ok(())
}

fn root_of(store: &Path) -> Result<PathBuf, String> {
    if store.join("case.json").is_file() {
        return Ok(store.to_path_buf());
    }
    let mut roots: Vec<PathBuf> = std::fs::read_dir(store)
        .map_err(|e| format!("{}: {e}", store.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.join("case.json").is_file())
        .collect();
    roots.sort();
    match roots.len() {
        1 => Ok(roots.remove(0)),
        0 => Err(format!("{}: no case directory (a directory with case.json)", store.display())),
        n => Err(format!("{}: {n} root cases; expected one", store.display())),
    }
}

fn exit_value(v: &Value) -> Value {
    match v.as_str().and_then(|t| t.parse::<i64>().ok()) {
        Some(n) => json!(n),
        None => v.clone(),
    }
}

/// The records of a case in the order received: by the case log (a child opened, then an action
/// finished), then records the log never names, by file name.
fn evidence_of(c: &Case) -> Result<Vec<Value>, String> {
    let mut ids: Vec<(String, Option<String>)> = Vec::new();
    let mut current: Option<String> = None;
    for e in &c.events {
        match e["event"].as_str() {
            Some("action_started") => current = s(e, "action"),
            Some("child_opened") => {
                if let Some(id) = s(e, "evidence") {
                    ids.push((id, current.clone()));
                }
            }
            Some("action_finished") => {
                for id in e["evidence"].as_array().into_iter().flatten().filter_map(|x| x.as_str()) {
                    ids.push((id.to_string(), s(e, "action")));
                }
            }
            _ => {}
        }
    }
    let mut seen = BTreeSet::new();
    ids.retain(|(id, _)| seen.insert(id.clone()));
    let evidence_dir = c.dir.join("evidence");
    match std::fs::read_dir(&evidence_dir) {
        Ok(rd) => {
            let mut files: Vec<String> = Vec::new();
            for e in rd {
                let e = e.map_err(|e| format!("{}: {e}", evidence_dir.display()))?;
                if let Some(id) = e.file_name().to_str().and_then(|n| n.strip_suffix(".json")) {
                    if !seen.contains(id) {
                        files.push(id.to_string());
                    }
                }
            }
            files.sort();
            ids.extend(files.into_iter().map(|f| (f, None)));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("{}: {e}", evidence_dir.display())),
    }
    let mut out = Vec::new();
    for (id, action) in ids {
        let r = read_json(&evidence_dir.join(format!("{id}.json")))?;
        let up: BTreeMap<String, Value> = r["upstream_revisions"]
            .as_object()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        out.push(json!({
            "index": out.len(),
            "kind": r["kind"],
            "result": r.get("result").cloned().unwrap_or(Value::Null),
            "subject": r["subject"],
            "subject_revision": r["subject_revision"],
            "upstream_revisions": Value::Object(up.into_iter().collect()),
            "action": jstr(action),
        }));
    }
    Ok(out)
}

/// The result a record of the case holds, by id; the record must be readable.
fn result_of(c: &Case, id: &str) -> Result<Option<String>, String> {
    Ok(s(&read_json(&c.dir.join("evidence").join(format!("{id}.json")))?, "result"))
}

struct Run {
    action: String,
    exit: Value,
    retry_of: bool,
}

/// Program runs and person steps of one case, from its log. Person step: an action finished with a
/// principal. Program run: an action started with an argv, not a person step.
type Steps = (Vec<Run>, Vec<(String, Value)>);
fn steps_of(c: &Case) -> Result<Steps, String> {
    let mut runs = Vec::new();
    let mut persons = Vec::new();
    let mut started: Option<(bool, bool)> = None;
    // A capability granted by a person with no action of theirs after it (`approve` without
    // `--action`): the driver then runs the action itself. A grant followed by the person's own
    // action is one step, recorded by that action.
    let mut grant: Option<Value> = None;
    let standalone = |grant: &mut Option<Value>, persons: &mut Vec<(String, Value)>| {
        if let Some(g) = grant.take() {
            persons.push((s(&g, "at").unwrap_or_default(), json!({
                "action": null, "answer": g["decision"], "actor": g["principal"], "capability": g["capability"]})));
        }
    };
    for e in &c.events {
        match e["event"].as_str() {
            Some("authority_recorded") => {
                standalone(&mut grant, &mut persons);
                grant = Some(e.clone());
            }
            Some("action_started") => {
                started = Some((
                    e["argv"].as_array().is_some_and(|a| !a.is_empty()),
                    e["retry_of"].as_array().is_some_and(|a| !a.is_empty()),
                ));
            }
            Some("action_finished") => {
                let (argv, retry) = started.take().unwrap_or((false, false));
                let action = s(e, "action").unwrap_or_default();
                if s(e, "principal").is_none() {
                    standalone(&mut grant, &mut persons);
                } else {
                    grant = None;
                }
                if let Some(who) = s(e, "principal") {
                    let mut answer = s(e, "result");
                    if answer.is_none() {
                        if let Some(i) = e["evidence"].as_array().and_then(|a| a.first()).and_then(|i| i.as_str()) {
                            answer = result_of(c, i)?;
                        }
                    }
                    let answer = answer.unwrap_or_else(|| format!("exit:{}", e["exit"].as_str().unwrap_or("?")));
                    let at = s(e, "at").unwrap_or_default();
                    persons.push((at, json!({"action": action, "answer": answer, "actor": who})));
                } else if argv {
                    runs.push(Run { action, exit: exit_value(&e["exit"]), retry_of: retry });
                }
            }
            _ => {}
        }
    }
    standalone(&mut grant, &mut persons);
    Ok((runs, persons))
}

/// The capability an action of the case's IR requires: null when the IR names the action and it
/// requires none; the IR file itself must be readable.
fn capability_of(c: &Case, action: &str) -> Result<Value, String> {
    let ir = read_json(&c.dir.join("ir.json"))?;
    Ok(ir["actions"][action]["requires"][0]["capability"].as_str().map_or(Value::Null, |s| Value::String(s.to_string())))
}

fn stop_of(c: &Case) -> Result<Value, String> {
    if c.case["termination"].is_string() {
        return Ok(Value::Null);
    }
    let last = latest_evaluation(&c.dir)?;
    let bindings = read_json(&c.dir.join("bindings.json"))?;
    let status = |want: &str, manual: Option<bool>| {
        last["actions"].as_object().is_some_and(|m| {
            m.iter().any(|(a, v)| {
                v["status"] == want && manual.is_none_or(|mm| (bindings[a]["manual"] == true) == mm)
            })
        })
    };
    let cause = if status("admissible", Some(true)) {
        "awaiting_person".to_string()
    } else if status("approval-required", None) {
        "awaiting_authority".to_string()
    } else {
        c.events
            .iter()
            .rev()
            .filter_map(|e| e["event"].as_str())
            .find(|k| {
                ["writer_spent", "writer_not_called", "llm_budget_spent", "step_budget_spent", "wall_time_spent"]
                    .contains(k)
            })
            .unwrap_or("stopped")
            .to_string()
    };
    Ok(json!({"cause": cause}))
}

/// The ids of the obligations still open in the newest evaluation. An evaluation without an
/// `obligations` member has none; one whose member is not a list is an error.
fn open_obligations(c: &Case) -> Result<Vec<Value>, String> {
    let d = latest_evaluation(&c.dir)?;
    let list = match &d["obligations"] {
        Value::Null => Vec::new(),
        Value::Array(a) => a.clone(),
        _ => return Err(format!("{}: obligations is not a list", c.dir.join("evaluations").display())),
    };
    Ok(list.into_iter().filter(|o| o["status"] == "open").map(|o| o["id"].clone()).collect())
}

/// A text file with every wall-clock stamp `YYYY-MM-DDTHH:MM:SSZ` replaced by `<time>`: `aep` writes
/// the time of each move into the planning files of the product, and that time is not the run's.
fn without_times(bytes: Vec<u8>) -> Vec<u8> {
    let Ok(text) = String::from_utf8(bytes.clone()) else { return bytes };
    let b = text.as_bytes();
    let shape = b"dddd-dd-ddTdd:dd:ddZ";
    let at = |i: usize| i + shape.len() <= b.len() && shape.iter().zip(&b[i..]).all(|(s, c)| if *s == b'd' { c.is_ascii_digit() } else { s == c });
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < b.len() {
        if at(i) {
            out.push_str("<time>");
            i += shape.len();
        } else {
            let n = text[i..].chars().next().map_or(1, |c| c.len_utf8());
            out.push_str(&text[i..i + n]);
            i += n;
        }
    }
    out.into_bytes()
}

fn walk_product(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) -> Result<(), String> {
    for e in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let e = e.map_err(|e| e.to_string())?;
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        let ty = e.file_type().map_err(|e| e.to_string())?;
        if ty.is_symlink() {
            continue;
        } else if ty.is_dir() {
            let rel = p.strip_prefix(root).unwrap().to_string_lossy();
            if !EXCLUDED_DIRS.contains(&name.as_str()) && !TRANSPORT_DIRS.contains(&&*rel) {
                walk_product(root, &p, out)?;
            }
        } else if ty.is_file() {
            // aep's lock file is named by the digest of the absolute path it works in.
            if name.starts_with(".aep-planning-writer-") && name.ends_with(".lock") {
                continue;
            }
            let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
            let bytes = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            out.insert(rel, sha256_hex(&without_times(bytes)));
        }
    }
    Ok(())
}

fn read_driver(run: &Path, store: &Path) -> Result<Value, String> {
    if !run.is_dir() {
        return Err(format!("{}: not a directory", run.display()));
    }
    let root = root_of(store)?;
    let mut cases = Vec::new();
    collect_cases(&root, None, &mut cases)?;
    let id_link = |c: &Case| -> Value {
        match &c.parent {
            None => Value::Null,
            Some(p) => json!(c.id.strip_prefix(&format!("{p}.")).unwrap_or(&c.id)),
        }
    };

    let mut persons_all: Vec<(String, usize, usize, usize)> = Vec::new();
    let mut per_case_persons: Vec<Vec<Value>> = Vec::new();
    let mut per_case_runs: Vec<Vec<Value>> = Vec::new();
    for (ci, c) in cases.iter().enumerate() {
        let (runs, persons) = steps_of(c)?;
        let mut attempts: HashMap<String, i64> = HashMap::new();
        let mut rs = Vec::new();
        for (i, r) in runs.iter().enumerate() {
            let n = attempts.entry(r.action.clone()).or_insert(0);
            *n += 1;
            let next = runs[i + 1..].iter().find(|x| x.action == r.action);
            rs.push(json!({
                "action": r.action, "attempt": *n, "exit": r.exit,
                "retry_followed": next.is_some_and(|x| x.retry_of),
            }));
        }
        per_case_runs.push(rs);
        let mut ps = Vec::new();
        for (pi, (at, mut v)) in persons.into_iter().enumerate() {
            if let Some(a) = v["action"].as_str().map(String::from) {
                v["capability"] = capability_of(c, &a)?;
            }
            persons_all.push((at, ci, pi, 0));
            ps.push(v);
        }
        per_case_persons.push(ps);
    }
    persons_all.sort();
    let mut order: HashMap<(usize, usize), usize> = HashMap::new();
    for (n, (_, ci, pi, _)) in persons_all.iter().enumerate() {
        order.insert((*ci, *pi), n);
    }

    let mut case_values = Vec::new();
    let mut protocols: BTreeSet<(String, String, String)> = BTreeSet::new();
    let mut decisions = Vec::new();
    let mut copies = Vec::new();
    for (ci, c) in cases.iter().enumerate() {
        if let Some(o) = c.events.iter().find(|e| e["event"] == "opened") {
            protocols.insert((
                o["protocol"]["id"].as_str().unwrap_or("").to_string(),
                o["protocol"]["revision"].to_string(),
                s(o, "ir").unwrap_or_default(),
            ));
        }
        let revision = c.events.iter().find(|e| e["event"] == "opened").map_or(Value::Null, |o| o["protocol"]["revision"].clone());
        let mut persons = Vec::new();
        for (pi, p) in per_case_persons[ci].iter().enumerate() {
            let mut m = Map::new();
            m.insert("order".into(), json!(order[&(ci, pi)]));
            for (k, v) in p.as_object().unwrap() {
                m.insert(k.clone(), v.clone());
            }
            persons.push(Value::Object(m));
        }
        let evidence = evidence_of(c)?;
        case_values.push(json!({
            "id": c.id,
            "link": id_link(c),
            "parent": jstr(c.parent.clone()),
            "protocol": c.case["protocol"],
            "protocol_revision": revision,
            "outcome": c.case.get("termination").cloned().unwrap_or(Value::Null),
            "stop": stop_of(c)?,
            "open_obligations": open_obligations(c)?,
            "evidence": evidence,
            "program_runs": per_case_runs[ci],
            "person_steps": persons,
        }));

        let mut copy: Option<Value> = None;
        for e in &c.events {
            match e["event"].as_str() {
                Some("action_started") if e.get("copy").is_some() => copy = Some(e.clone()),
                Some("action_finished") => {
                    if let Some(st) = copy.take() {
                        for id in e["evidence"].as_array().into_iter().flatten().filter_map(|x| x.as_str()) {
                            let rec = read_json(&c.dir.join("evidence").join(format!("{id}.json")))?;
                            let from = cases.iter().find(|o| {
                                o.id != c.id
                                    && o.case["protocol"] == st["copy"]["from"]
                                    && o.dir.join("evidence").join(format!("{id}.json")).is_file()
                            });
                            copies.push(json!({
                                "action": e["action"], "kind": rec["kind"], "from_protocol": st["copy"]["from"],
                                "from": from.map_or(Value::Null, |f| json!(f.id)), "to": c.id,
                                "subject": rec["subject"], "subject_revision": rec["subject_revision"],
                            }));
                        }
                    }
                }
                Some("decision_recorded") => decisions.push(json!({
                    "id": e["decision"], "case": c.id, "question": null, "options": [],
                    "answer": e["outcome"], "who": e["principal"],
                })),
                _ => {}
            }
        }
    }

    let mut product = BTreeMap::new();
    let work = root.join("work");
    if work.is_dir() {
        walk_product(&work, &work, &mut product)?;
    }
    // A run without a settings file records none (null members); a file that is there must parse.
    let settings_file = run.join("run-settings.jsonl");
    let settings = match std::fs::read_to_string(&settings_file) {
        Ok(t) => match t.lines().next() {
            Some(l) => serde_json::from_str::<Value>(l).map_err(|e| format!("{}:1: {e}", settings_file.display()))?,
            None => Value::Null,
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Value::Null,
        Err(e) => return Err(format!("{}: {e}", settings_file.display())),
    };
    Ok(json!({
        "format": FORMAT,
        "source": {"runtime": "driver", "run_settings": {
            "decider": settings["decider"]["kind"], "adapter": settings["adapter"],
            "stand_in": settings["stand_in"], "stand_in_principal": settings["stand_in_principal"]}},
        "not_recorded": ["decision_requests[].question", "decision_requests[].options"],
        "cases": case_values,
        "copies": copies,
        "decision_requests": decisions,
        "protocols": protocols.into_iter().map(|(id, revision, digest)| json!({
            "id": id, "revision": serde_json::from_str::<Value>(&revision).unwrap_or(Value::Null), "digest": digest})).collect::<Vec<_>>(),
        "product_tree": product.into_iter().map(|(path, sha)| json!({"path": path, "sha256": sha})).collect::<Vec<_>>(),
    }))
}

// ---------------------------------------------------------------- Loom's reader
//
// Reads the JSON `tree status` printed (format `loom.tree-status/1`) and the product tree, nothing
// else: no store file, no run log. A member the reader needs and does not find is an error that
// names the member and the case; nothing is replaced by a default. Where the status holds nothing
// for a member of the summary, the summary lists it under `not_recorded` (the rows of `RULES`
// marked `NOT_RECORDED`) and holds null or an empty list there.

const STOP_CAUSE: &str = "cases[].stop.cause";
const ANSWER: &str = "cases[].person_steps[].answer";

fn l_member<'a>(v: &'a Value, at: &str, name: &str) -> Result<&'a Value, String> {
    v.get(name).ok_or_else(|| format!("{at}: the member {name} is missing"))
}

fn l_text(v: &Value, at: &str, name: &str) -> Result<String, String> {
    l_member(v, at, name)?.as_str().map(String::from).ok_or_else(|| format!("{at}: the member {name} is not a string"))
}

fn l_opt(v: &Value, at: &str, name: &str) -> Result<Option<String>, String> {
    match v.get(name) {
        None => Ok(None),
        Some(x) => x.as_str().map(|t| Some(t.to_string())).ok_or_else(|| format!("{at}: the member {name} is not a string")),
    }
}

fn l_int(v: &Value, at: &str, name: &str) -> Result<i64, String> {
    l_member(v, at, name)?.as_i64().ok_or_else(|| format!("{at}: the member {name} is not an integer"))
}

fn l_list<'a>(v: &'a Value, at: &str, name: &str) -> Result<&'a Vec<Value>, String> {
    l_member(v, at, name)?.as_array().ok_or_else(|| format!("{at}: the member {name} is not a list"))
}

struct LEvidence {
    index: i64,
    kind: String,
    result: Option<String>,
    subject: String,
    subject_revision: String,
    upstream: BTreeMap<String, Value>,
    action: Option<String>,
}

struct LApproval {
    capability: String,
    action: Option<String>,
    answer: String,
    actor: String,
    at: String,
}

struct LCase {
    id: String,
    parent: Option<String>,
    name: Option<String>,
    protocol: String,
    revision: i64,
    outcome: Option<String>,
    stop: Option<String>,
    open_obligations: Vec<Value>,
    evidence: Vec<LEvidence>,
    runs: Vec<(String, i64, bool)>,
    approvals: Vec<LApproval>,
    copies: Vec<(String, String, String)>,
}

fn l_case(v: &Value, i: usize) -> Result<LCase, String> {
    let id = l_text(v, &format!("cases[{i}]"), "case")?;
    let at = format!("case {id}");
    let revision_text = l_text(v, &at, "protocol_revision")?;
    let revision = revision_text
        .parse::<i64>()
        .map_err(|_| format!("{at}: the member protocol_revision is {revision_text:?}, not an integer"))?;
    let mut open_obligations = Vec::new();
    for o in l_list(v, &at, "open_obligations")? {
        if !o.is_string() {
            return Err(format!("{at}: the member open_obligations holds an entry that is not a string"));
        }
        open_obligations.push(o.clone());
    }
    let mut evidence = Vec::new();
    for (n, e) in l_list(v, &at, "evidence")?.iter().enumerate() {
        let at = format!("{at}: evidence[{n}]");
        let upstream = l_member(e, &at, "upstream_revisions")?
            .as_object()
            .ok_or_else(|| format!("{at}: the member upstream_revisions is not an object"))?
            .iter()
            .map(|(k, x)| (k.clone(), x.clone()))
            .collect();
        evidence.push(LEvidence {
            index: l_int(e, &at, "index")?,
            kind: l_text(e, &at, "kind")?,
            result: l_opt(e, &at, "result")?,
            subject: l_text(e, &at, "subject")?,
            subject_revision: l_text(e, &at, "subject_revision")?,
            upstream,
            action: l_opt(e, &at, "action")?,
        });
    }
    let mut runs = Vec::new();
    for (n, r) in l_list(v, &at, "runs")?.iter().enumerate() {
        let at = format!("{at}: runs[{n}]");
        let retried = l_member(r, &at, "retried")?.as_bool().ok_or_else(|| format!("{at}: the member retried is not a boolean"))?;
        runs.push((l_text(r, &at, "action")?, l_int(r, &at, "exit")?, retried));
    }
    let mut approvals = Vec::new();
    for (n, a) in l_list(v, &at, "approvals")?.iter().enumerate() {
        let at = format!("{at}: approvals[{n}]");
        let answer = l_text(a, &at, "answer")?;
        if word(ANSWER, &answer).is_none() {
            return Err(format!("{at}: the member answer is {answer:?}, which the table of rules has no word for"));
        }
        approvals.push(LApproval {
            capability: l_text(a, &at, "capability")?,
            action: l_opt(a, &at, "action")?,
            answer,
            actor: l_text(a, &at, "actor")?,
            at: l_text(a, &at, "at")?,
        });
    }
    let mut copies = Vec::new();
    for (n, c) in l_list(v, &at, "copies")?.iter().enumerate() {
        let at = format!("{at}: copies[{n}]");
        copies.push((l_text(c, &at, "source_case")?, l_text(c, &at, "target_case")?, l_text(c, &at, "record_kind")?));
    }
    Ok(LCase {
        parent: l_opt(v, &at, "parent")?,
        name: l_opt(v, &at, "name")?,
        protocol: l_text(v, &at, "protocol_id")?,
        revision,
        outcome: l_opt(v, &at, "outcome")?,
        stop: l_opt(v, &at, "stop")?,
        open_obligations,
        evidence,
        runs,
        approvals,
        copies,
        id,
    })
}

/// The cases in the order of the summary: the root, then each case before its children, siblings
/// in the order the status lists them.
fn l_order(cases: &[LCase], root: &str) -> Result<Vec<usize>, String> {
    let Some(r) = cases.iter().position(|c| c.id == root) else {
        return Err(format!("the root case {root} is not in the member cases"));
    };
    if cases[r].parent.is_some() {
        return Err(format!("case {root}: the root has a parent"));
    }
    let mut seen = BTreeSet::new();
    for c in cases {
        if !seen.insert(c.id.as_str()) {
            return Err(format!("case {}: listed twice", c.id));
        }
        if c.id != root {
            match &c.parent {
                None => return Err(format!("case {}: the member parent is missing", c.id)),
                Some(p) if !cases.iter().any(|x| &x.id == p) => {
                    return Err(format!("case {}: the member parent names {p}, which is not a case of the tree", c.id))
                }
                Some(_) => {}
            }
        }
    }
    fn visit(i: usize, cases: &[LCase], out: &mut Vec<usize>) {
        out.push(i);
        for (j, c) in cases.iter().enumerate() {
            if c.parent.as_deref() == Some(cases[i].id.as_str()) {
                visit(j, cases, out);
            }
        }
    }
    let mut out = Vec::new();
    visit(r, cases, &mut out);
    if let Some(c) = cases.iter().enumerate().find(|(i, _)| !out.contains(i)).map(|(_, c)| c) {
        return Err(format!("case {}: is not below the root {root}", c.id));
    }
    Ok(out)
}

fn read_loom(status: &Path, product_tree: Option<&Path>, run: Option<&RunInfo>) -> Result<Value, String> {
    let doc = read_json(status)?;
    loom_summary(&doc, product_tree, run).map_err(|e| format!("{}: {e}", status.display()))
}

/// What the run directory of `runbook run` holds that `tree status` does not.
struct RunInfo {
    /// `source.run_settings`.
    settings: Value,
    /// The sha256 of each protocol file the runner copied, by (registration stem, revision).
    digests: BTreeMap<(String, i64), String>,
}

/// Reads `run-settings.jsonl` (first line), `decisions.jsonl` and `protocols/*.protocol.yaml`.
/// A file that is missing, a settings line that does not parse, a protocol file with no id or
/// revision, and a decision that is neither an approval nor the decider's answer to a question or
/// a finding of the answers file (which the table of rules compares to nothing) are errors that
/// name the file.
fn read_run_info(dir: &Path) -> Result<RunInfo, String> {
    let file = dir.join("run-settings.jsonl");
    let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let line = text.lines().next().ok_or_else(|| format!("{}: empty", file.display()))?;
    let s: Value = serde_json::from_str(line).map_err(|e| format!("{}:1: {e}", file.display()))?;
    let kind = s["decider"]["kind"].as_str().map(String::from);
    let settings = json!({
        "decider": kind,
        "adapter": s.get("adapter").cloned().unwrap_or(Value::Null),
        "stand_in": kind.as_deref().is_some_and(|k| k != "model"),
        "stand_in_principal": s["decider"].get("principal").cloned().unwrap_or(Value::Null),
    });
    let file = dir.join("decisions.jsonl");
    let rows = match std::fs::read_to_string(&file) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", file.display())),
    };
    for (n, row) in rows.lines().enumerate() {
        let v: Value = serde_json::from_str(row).map_err(|e| format!("{}:{}: {e}", file.display(), n + 1))?;
        let known = (v["type"] == "approval" && matches!(v["kind"].as_str(), Some("Approve" | "ApproveAction"))) || matches!(v["type"].as_str(), Some("question" | "finding"));
        if !known {
            return Err(format!(
                "{}:{}: a decision of type {} and kind {} has no rule: the runner answers approvals (Approve, ApproveAction) and the table of rules has no row for another",
                file.display(),
                n + 1,
                v["type"],
                v["kind"]
            ));
        }
    }
    let protocols = dir.join("protocols");
    let mut digests = BTreeMap::new();
    for e in std::fs::read_dir(&protocols).map_err(|e| format!("{}: {e}", protocols.display()))? {
        let path = e.map_err(|e| format!("{}: {e}", protocols.display()))?.path();
        if !path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(".protocol.yaml")) {
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let doc: serde_yaml::Value = serde_yaml::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        let id = doc["protocol"]["id"].as_str().ok_or_else(|| format!("{}: protocol.id is missing", path.display()))?;
        let revision = doc["protocol"]["revision"].as_i64().ok_or_else(|| format!("{}: protocol.revision is missing", path.display()))?;
        digests.insert((stem(id), revision), sha256_hex(&bytes));
    }
    Ok(RunInfo { settings, digests })
}

fn loom_summary(doc: &Value, product_tree: Option<&Path>, run: Option<&RunInfo>) -> Result<Value, String> {
    if !doc.is_object() {
        return Err("the status is not a JSON object".into());
    }
    match doc.get("format") {
        Some(Value::String(f)) if f == LOOM_STATUS_FORMAT => {}
        Some(f) => return Err(format!("the status names the format {f}, and {LOOM_STATUS_FORMAT} is read")),
        None => return Err(format!("the status names no format, and {LOOM_STATUS_FORMAT} is read")),
    }
    let version = l_text(doc, "status", "version")?;
    let root = l_text(doc, "status", "root")?;
    let mut cases = Vec::new();
    for (i, c) in l_list(doc, "status", "cases")?.iter().enumerate() {
        cases.push(l_case(c, i)?);
    }
    let order = l_order(&cases, &root)?;

    // Person steps: `order` is the position over every approval of the tree, by the instant each
    // was recorded, then by the case's place and the approval's.
    let mut persons_all: Vec<(String, usize, usize)> = Vec::new();
    for (ci, &i) in order.iter().enumerate() {
        for (pi, a) in cases[i].approvals.iter().enumerate() {
            persons_all.push((a.at.clone(), ci, pi));
        }
    }
    persons_all.sort();
    let place: HashMap<(usize, usize), usize> =
        persons_all.iter().enumerate().map(|(n, (_, ci, pi))| ((*ci, *pi), n)).collect();

    let mut case_values = Vec::new();
    let mut copies = Vec::new();
    let mut copied: HashMap<(String, String), usize> = HashMap::new();
    let mut protocols: BTreeSet<(String, String)> = BTreeSet::new();
    for (ci, &i) in order.iter().enumerate() {
        let c = &cases[i];
        let at = format!("case {}", c.id);
        protocols.insert((c.protocol.clone(), c.revision.to_string()));

        // The governor writes a child's `case_outcome` record with the step that opened the child;
        // the log names that step on the record after it, and the driver's summary on both.
        let mut evidence = Vec::new();
        for (n, e) in c.evidence.iter().enumerate() {
            let action = match (&e.action, e.kind.as_str()) {
                (Some(a), _) => Value::String(a.clone()),
                (None, "case_outcome") => match c.evidence[n + 1..].iter().find_map(|x| x.action.clone()) {
                    Some(a) => Value::String(a),
                    None => {
                        return Err(format!(
                            "{at}: evidence[{n}]: the member action is missing on a case_outcome record and no record after it names the action of its step"
                        ))
                    }
                },
                (None, _) => Value::Null,
            };
            evidence.push(json!({
                "index": e.index,
                "kind": e.kind,
                "result": jstr(e.result.clone()),
                "subject": e.subject,
                "subject_revision": e.subject_revision,
                "upstream_revisions": Value::Object(e.upstream.iter().map(|(k, v)| (k.clone(), v.clone())).collect()),
                "action": action,
            }));
        }

        let mut attempts: HashMap<&str, i64> = HashMap::new();
        let runs: Vec<Value> = c
            .runs
            .iter()
            .map(|(action, exit, retried)| {
                let n = attempts.entry(action.as_str()).or_insert(0);
                *n += 1;
                json!({"action": action, "attempt": *n, "exit": exit, "retry_followed": retried})
            })
            .collect();

        let mut persons = Vec::new();
        for (pi, a) in c.approvals.iter().enumerate() {
            let answer = match (&a.action, a.answer.as_str()) {
                (Some(action), "Allow") => {
                    let nth = c.approvals[..pi].iter().filter(|x| x.answer == "Allow" && x.action.as_ref() == Some(action)).count();
                    c.evidence
                        .iter()
                        .filter(|e| e.action.as_ref() == Some(action))
                        .nth(nth)
                        .and_then(|e| e.result.clone())
                        .ok_or_else(|| format!("{at}: approvals[{pi}]: no record of the action {action} holds the result it recorded"))?
                }
                (_, loom) => word(ANSWER, loom).expect("checked when the case was read").to_string(),
            };
            persons.push(json!({
                "order": place[&(ci, pi)],
                "action": jstr(a.action.clone()),
                "answer": answer,
                "actor": a.actor,
                "capability": a.capability,
            }));
        }

        for (n, (source, target, kind)) in c.copies.iter().enumerate() {
            let at = format!("{at}: copies[{n}]");
            let from = cases.iter().find(|x| &x.id == source).ok_or_else(|| format!("{at}: source_case {source} is not a case of the tree"))?;
            let to = cases.iter().find(|x| &x.id == target).ok_or_else(|| format!("{at}: target_case {target} is not a case of the tree"))?;
            // The entry names no record. The copied one is the record of that kind in the target case
            // that the source's own action did not produce (the copy's action wrote it); the k-th copy
            // of a kind into a case is the k-th such record.
            let own: Vec<&str> = from.evidence.iter().filter(|e| &e.kind == kind).filter_map(|e| e.action.as_deref()).collect();
            let k = copied.entry((target.clone(), kind.clone())).or_insert(0);
            let record = to.evidence.iter().filter(|e| &e.kind == kind && !e.action.as_deref().is_some_and(|a| own.contains(&a))).nth(*k);
            *k += 1;
            let record = record.ok_or_else(|| {
                format!("{at}: the record of kind {kind} that the copy from {source} left in case {target} is not in that case's evidence (the {} copy of that kind into it)", *k)
            })?;
            copies.push(json!({
                "action": jstr(record.action.clone()), "kind": kind, "from_protocol": from.protocol,
                "from": source, "to": target, "subject": record.subject, "subject_revision": record.subject_revision,
            }));
        }

        let stop = match (&c.outcome, &c.stop) {
            (None, Some(s)) => {
                let cause = word(STOP_CAUSE, s).ok_or_else(|| format!("{at}: the member stop is {s:?}, which the table of rules has no word for"))?;
                json!({"cause": cause})
            }
            _ => Value::Null,
        };
        case_values.push(json!({
            "id": c.id,
            "link": jstr(c.name.clone()),
            "parent": jstr(c.parent.clone()),
            "protocol": c.protocol,
            "protocol_revision": c.revision,
            "outcome": jstr(c.outcome.clone()),
            "stop": stop,
            "open_obligations": c.open_obligations,
            "evidence": evidence,
            "program_runs": runs,
            "person_steps": persons,
        }));
        if c.id != root && c.name.is_none() {
            return Err(format!("{at}: the member name is missing"));
        }
    }

    let tree = match product_tree {
        Some(p) => p.to_path_buf(),
        None => PathBuf::from(l_text(doc, "status", "workspace")?),
    };
    if !tree.is_dir() {
        return Err(format!("{}: the product tree is not a directory", tree.display()));
    }
    let mut product = BTreeMap::new();
    walk_product(&tree, &tree, &mut product)?;

    let mut protocol_values = Vec::new();
    for (id, revision) in protocols {
        let revision = revision.parse::<i64>().expect("written from an integer");
        let digest = match run {
            None => Value::Null,
            Some(r) => json!(r.digests.get(&(stem(&id), revision)).ok_or_else(|| format!(
                "the run directory's protocols/ holds no file for the protocol {id}@{revision}, which a case of the tree runs"
            ))?),
        };
        protocol_values.push(json!({"id": id, "revision": revision, "digest": digest}));
    }
    // A member only the run directory holds is listed as not recorded when the summary was read
    // from the status alone.
    let not_recorded: Vec<&str> =
        RULES.iter().filter(|r| r.loom == NOT_RECORDED || (run.is_none() && r.loom.starts_with("run directory: "))).map(|r| r.member).collect();
    Ok(json!({
        "format": FORMAT,
        "source": {"runtime": "loom", "status_version": version, "run_settings": run.map_or(Value::Null, |r| r.settings.clone())},
        "not_recorded": not_recorded,
        "cases": case_values,
        "copies": copies,
        "decision_requests": [],
        "protocols": protocol_values,
        "product_tree": product.into_iter().map(|(path, sha)| json!({"path": path, "sha256": sha})).collect::<Vec<_>>(),
    }))
}

// ---------------------------------------------------------------- compare

/// Take the `subject_revision` and the `upstream_revisions` out of every child outcome record, so
/// that they are neither compared nor counted when `rename` names the revisions (rules
/// `cases[].evidence[case_outcome].*`).
fn without_outcome_revisions(v: &mut Value) {
    for c in v["cases"].as_array_mut().into_iter().flatten() {
        for e in c["evidence"].as_array_mut().into_iter().flatten() {
            if e["kind"] == OUTCOME_KIND {
                e["subject_revision"] = Value::Null;
                e["upstream_revisions"] = json!({});
            }
        }
    }
}

/// Rename the values a run generates to `c1`, `r1`, ... in order of first appearance, one counter
/// per class, over the whole summary in document order.
fn rename(v: &mut Value) {
    struct Namer(HashMap<&'static str, Vec<String>>);
    impl Namer {
        fn name(&mut self, class: &'static str, slot: &mut Value) {
            if let Some(t) = slot.as_str() {
                let seen = self.0.entry(class).or_default();
                let n = seen.iter().position(|x| x == t).unwrap_or_else(|| {
                    seen.push(t.to_string());
                    seen.len() - 1
                });
                *slot = json!(format!("{class}{}", n + 1));
            }
        }
    }
    let mut n = Namer(HashMap::new());
    let pairs: [(&str, &[(&str, &'static str)]); 3] = [
        ("copies", &[("from", "c"), ("to", "c"), ("subject_revision", "r")]),
        ("decision_requests", &[("id", "q"), ("case", "c")]),
        ("protocols", &[("digest", "d")]),
    ];
    if let Some(cases) = v["cases"].as_array_mut() {
        for c in cases {
            n.name("c", &mut c["id"]);
            n.name("c", &mut c["parent"]);
            for e in c["evidence"].as_array_mut().into_iter().flatten() {
                n.name("r", &mut e["subject_revision"]);
                for (_, u) in e["upstream_revisions"].as_object_mut().into_iter().flatten() {
                    n.name("r", u);
                }
            }
        }
    }
    for (member, fields) in pairs {
        for item in v[member].as_array_mut().into_iter().flatten() {
            for (f, class) in fields {
                n.name(class, &mut item[*f]);
            }
        }
    }
}

pub struct Options {
    pub product_digests: bool,
}

fn show(v: Option<&Value>) -> String {
    v.map_or("(absent)".to_string(), |x| x.to_string())
}

/// Whether the member `member` is `listed` or lies below it (`decision_requests[].id` is below
/// `decision_requests`).
fn covers(listed: &str, member: &str) -> bool {
    member == listed
        || member.strip_prefix(listed).is_some_and(|rest| rest.starts_with('.') || rest.starts_with('['))
}

/// The members a summary lists under `not_recorded`.
fn not_recorded_of(v: &Value) -> Vec<String> {
    v["not_recorded"].as_array().into_iter().flatten().filter_map(|m| m.as_str().map(String::from)).collect()
}

/// A member one runtime does not record is not compared: the other side's value has nothing to be
/// equal to, and a null or an empty list there is no fact.
fn diff(tpl: &str, shown: &str, a: Option<&Value>, b: Option<&Value>, o: &Options, skip: &[String], out: &mut Vec<String>) {
    if skip.iter().any(|s| covers(s, tpl)) {
        return;
    }
    if let Some(r) = rule_for(tpl) {
        match r.how {
            How::Never => return,
            How::Digest if !o.product_digests => return,
            _ => {}
        }
    }
    match (a, b) {
        (Some(Value::Object(x)), Some(Value::Object(y))) => {
            let mut keys: Vec<&String> = x.keys().collect();
            keys.extend(y.keys().filter(|k| !x.contains_key(*k)));
            for k in keys {
                let t = if tpl.is_empty() {
                    k.clone()
                } else if tpl.ends_with("upstream_revisions") {
                    format!("{tpl}.*")
                } else {
                    format!("{tpl}.{k}")
                };
                let sh = if shown.is_empty() { k.clone() } else { format!("{shown}.{k}") };
                diff(&t, &sh, x.get(k), y.get(k), o, skip, out);
            }
        }
        (Some(Value::Array(x)), Some(Value::Array(y))) if tpl == "product_tree" => {
            let by_path = |l: &[Value]| -> BTreeMap<String, Value> {
                l.iter().map(|e| (e["path"].as_str().unwrap_or("").to_string(), e["sha256"].clone())).collect()
            };
            let (mx, my) = (by_path(x), by_path(y));
            let paths: BTreeSet<&String> = mx.keys().chain(my.keys()).collect();
            for p in paths {
                match (mx.get(p), my.get(p)) {
                    (Some(_), None) => out.push(format!("product_tree[{p}]: a=present b=(absent)")),
                    (None, Some(_)) => out.push(format!("product_tree[{p}]: a=(absent) b=present")),
                    (Some(da), Some(db)) if da != db && o.product_digests => {
                        out.push(format!("product_tree[{p}].sha256: a={da} b={db}"))
                    }
                    _ => {}
                }
            }
        }
        (Some(Value::Array(x)), Some(Value::Array(y))) => {
            for i in 0..x.len().max(y.len()) {
                diff(&format!("{tpl}[]"), &format!("{shown}[{i}]"), x.get(i), y.get(i), o, skip, out);
            }
        }
        _ => {
            let by_stem = rule_for(tpl).is_some_and(|r| r.how == How::Stem);
            let same = match (a, b) {
                (Some(Value::String(x)), Some(Value::String(y))) if by_stem => stem(x) == stem(y),
                _ => a == b,
            };
            if !same {
                out.push(format!("{shown}: a={} b={}", show(a), show(b)));
            }
        }
    }
}

/// The differences of two summaries, one line each; empty when equal under the rules.
pub fn compare(a: &Value, b: &Value, o: &Options) -> Vec<String> {
    let (mut a, mut b) = (a.clone(), b.clone());
    without_outcome_revisions(&mut a);
    without_outcome_revisions(&mut b);
    rename(&mut a);
    rename(&mut b);
    let skip: Vec<String> = not_recorded_of(&a).into_iter().chain(not_recorded_of(&b)).collect();
    let mut out = Vec::new();
    diff("", "", Some(&a), Some(&b), o, &skip, &mut out);
    out
}

/// Whether `member` is a member of the entries of a list (`decision_requests[].question`) that
/// neither summary has an entry of: nothing is lost by not recording it.
fn of_an_empty_list(member: &str, a: &Value, b: &Value) -> bool {
    let Some((list, _)) = member.split_once("[]") else {
        return false;
    };
    let empty = |v: &Value| v.get(list).and_then(Value::as_array).is_some_and(|l| l.is_empty());
    empty(a) && empty(b)
}

/// The members exactly one side does not record, one line each. A member both sides list is not
/// reported: nothing is compared there and nothing is lost. Nor is a member of the entries of a
/// list both summaries hold none of.
pub fn not_recorded_notes(a: &Value, b: &Value) -> Vec<String> {
    let (la, lb) = (not_recorded_of(a), not_recorded_of(b));
    let mut out = Vec::new();
    for (side, mine, other) in [("a", &la, &lb), ("b", &lb, &la)] {
        for m in mine {
            if !other.iter().any(|t| covers(t, m)) && !of_an_empty_list(m, a, b) {
                out.push(format!("not recorded on one side: {m} (not recorded by {side})"));
            }
        }
    }
    out
}

pub fn load(path: &Path) -> Result<Value, String> {
    let v = read_json(path)?;
    if v["format"] != FORMAT {
        return Err(format!("{}: format is not {FORMAT}", path.display()));
    }
    Ok(v)
}

pub fn counts(v: &Value) -> String {
    let cases = v["cases"].as_array().map_or(&[][..], |a| a.as_slice());
    let sum = |k: &str| -> usize { cases.iter().map(|c| c[k].as_array().map_or(0, |a| a.len())).sum() };
    format!(
        "{} cases, {} evidence records, {} program runs, {} person steps, {} product files",
        cases.len(),
        sum("evidence"),
        sum("program_runs"),
        sum("person_steps"),
        v["product_tree"].as_array().map_or(0, |a| a.len())
    )
}

// ---------------------------------------------------------------- approvals

/// The person steps of a run in order: the `tree approve` calls a second runtime must make.
pub fn approvals(v: &Value) -> Value {
    let cases = v["cases"].as_array().cloned().unwrap_or_default();
    let by_id: HashMap<&str, &Value> = cases.iter().filter_map(|c| c["id"].as_str().map(|i| (i, c))).collect();
    let path_of = |c: &Value| -> String {
        let mut names = Vec::new();
        let mut cur = Some(c);
        while let Some(x) = cur {
            if let Some(l) = x["link"].as_str() {
                names.push(l.to_string());
            }
            cur = x["parent"].as_str().and_then(|p| by_id.get(p).copied());
        }
        names.reverse();
        if names.is_empty() { ".".to_string() } else { names.join("/") }
    };
    let mut steps: Vec<(i64, Value)> = Vec::new();
    for c in &cases {
        for p in c["person_steps"].as_array().into_iter().flatten() {
            steps.push((
                p["order"].as_i64().unwrap_or(0),
                json!({"path": path_of(c), "case": c["link"], "action": p["action"], "capability": p["capability"],
                    "answer": p["answer"], "actor": p["actor"]}),
            ));
        }
    }
    steps.sort_by_key(|(o, _)| *o);
    Value::Array(steps.into_iter().map(|(_, v)| v).collect())
}

