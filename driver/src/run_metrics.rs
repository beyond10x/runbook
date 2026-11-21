//! Workflow metrics of one recorded run, `sdc.run-metrics/1` (docs/workflow-metrics.md).
//!
//! Shared by `sdc-run-metrics` (one run) and `sdc-experiment` (groups of runs, experiment
//! records). Every figure is read from the run directory's records; nothing is estimated and no
//! clock is read. A record that is absent is reported as absent; a line that does not parse is
//! skipped and counted (`malformed_lines`), because a run that is still going may have a half
//! written last line.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value as J};
use serde_yaml::Value as Y;

pub const FORMAT: &str = "sdc.run-metrics/1";

/// Protocol stages in the order a run reaches them (docs/protocol-notes.md).
pub const STAGES: [&str; 8] = ["idea", "intake", "direction", "epic", "spec", "plan", "story", "release"];

/// Results that send work back: a check or an approval that did not accept its subject.
const NEGATIVE: [&str; 10] =
    ["failed", "finding", "invalid", "rejected", "refused", "uncovered", "unassessed", "draft", "withheld", "not_released"];

/// A run directory that cannot be read: the message names the file.
#[derive(Debug)]
pub struct Fail(pub String);

pub type Res<T> = Result<T, Fail>;

// ---- reading -----------------------------------------------------------------------------------

pub fn read_opt(path: &Path) -> Res<Option<String>> {
    match fs::read_to_string(path) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Fail(format!("cannot read {}: {e}", path.display()))),
    }
}

/// The lines of a JSONL file that parse, and the number that did not.
pub fn jsonl(path: &Path, malformed: &mut usize) -> Res<Option<Vec<J>>> {
    let Some(text) = read_opt(path)? else { return Ok(None) };
    let mut out = vec![];
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        match serde_json::from_str::<J>(line) {
            Ok(v) => out.push(v),
            Err(_) => *malformed += 1,
        }
    }
    Ok(Some(out))
}

/// Every line of a JSONL file in order, `Null` for a line that does not parse (so that an index
/// into the file stays an index into the vector).
pub fn jsonl_indexed(path: &Path, malformed: &mut usize) -> Res<Vec<J>> {
    let Some(text) = read_opt(path)? else { return Ok(vec![]) };
    Ok(text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            serde_json::from_str::<J>(l).unwrap_or_else(|_| {
                *malformed += 1;
                J::Null
            })
        })
        .collect())
}

pub fn json_opt(path: &Path) -> Res<Option<J>> {
    let Some(text) = read_opt(path)? else { return Ok(None) };
    Ok(serde_json::from_str(&text).ok())
}

pub fn js(v: &J) -> String {
    v.as_str().map(String::from).unwrap_or_else(|| if v.is_null() { String::new() } else { v.to_string() })
}

/// Milliseconds since the epoch of an RFC 3339 UTC instant `YYYY-MM-DDTHH:MM:SS[.fff]Z`.
pub fn epoch_ms(t: &str) -> Option<i64> {
    let b = t.as_bytes();
    if b.len() < 20 || !t.ends_with('Z') || b[10] != b'T' || b[4] != b'-' || b[7] != b'-' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let n = |a: usize, z: usize| t.get(a..z).and_then(|s| s.parse::<i64>().ok());
    let (y, m, d, hh, mm, ss) = (n(0, 4)?, n(5, 7)?, n(8, 10)?, n(11, 13)?, n(14, 16)?, n(17, 19)?);
    let frac = match t.get(19..t.len() - 1)? {
        "" => 0,
        f if f.starts_with('.') && f.len() > 1 && f[1..].bytes().all(|c| c.is_ascii_digit()) => {
            format!("{:0<3}", &f[1..]).get(..3)?.parse::<i64>().ok()?
        }
        _ => return None,
    };
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 86_400 + hh * 3600 + mm * 60 + ss) * 1000) + frac)
}

pub fn round6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

fn round3(x: f64) -> f64 {
    (x * 1e3).round() / 1e3
}

/// The stage of a protocol id: `sdc` is the idea, `sdc.<stage>` the rest.
pub fn stage_of(protocol: &str) -> String {
    match protocol.strip_prefix("sdc.") {
        Some(s) => s.to_string(),
        None if protocol == "sdc" => "idea".to_string(),
        None => protocol.to_string(),
    }
}

fn stage_index(stage: &str) -> Option<usize> {
    STAGES.iter().position(|s| *s == stage)
}

/// Every case directory of the tree at `root`: the root, then each `children/<id>` below it.
pub fn case_dirs(root: &Path, out: &mut Vec<PathBuf>) {
    if root.join("events.jsonl").is_file() {
        out.push(root.to_path_buf());
    }
    let Ok(dir) = fs::read_dir(root.join("children")) else { return };
    let mut subs: Vec<PathBuf> = dir.flatten().filter(|e| e.path().is_dir()).map(|e| e.path()).collect();
    subs.sort();
    for d in subs {
        case_dirs(&d, out);
    }
}

// ---- one case ----------------------------------------------------------------------------------

/// One execution of a bound action: an `action_started` event and its `action_finished`.
#[derive(Clone, Debug)]
struct Take {
    action: String,
    start_idx: usize,
    end_idx: Option<usize>,
    llm: bool,
    retry_of: Vec<String>,
    exit: String,
    not_called: bool,
    principal: String,
    evidence: Vec<(String, String)>,
    t_start: Option<i64>,
    t_end: Option<i64>,
    /// Cost and figures of the model call joined to this take (llm-calls.jsonl).
    cost: f64,
    /// Model calls joined to this take.
    calls: usize,
    /// Rework: the cause(s) and edge type, set for every real take after the first of its action.
    causes: Vec<Cause>,
}

#[derive(Clone, Debug)]
struct Cause {
    /// `A:R`, `arrival:outside`, `decision:<action>` or `unexplained`.
    label: String,
    /// The action whose record sent the writer back, when there is one.
    from: Option<String>,
    /// `check`, `approval`, `finding` or `arrival`.
    kind: &'static str,
}

impl Take {
    fn could_not_run(&self) -> bool {
        matches!(self.exit.as_str(), "75" | "126" | "127")
    }
    fn real(&self) -> bool {
        !self.not_called && !self.could_not_run() && self.end_idx.is_some()
    }
    fn driver_s(&self) -> f64 {
        match (self.t_start, self.t_end) {
            (Some(a), Some(b)) if b >= a => (b - a) as f64 / 1000.0,
            _ => 0.0,
        }
    }
}

struct Case {
    id: String,
    stage: String,
    outcome: Option<String>,
    takes: Vec<Take>,
    /// Action -> `write`, `read`, ... from ir.json; action -> manual / copy / argv from bindings.json.
    effect: BTreeMap<String, String>,
    manual: BTreeSet<String>,
    copy: BTreeSet<String>,
    events: Vec<J>,
    times: Vec<Option<i64>>,
}

fn load_case(dir: &Path, malformed: &mut usize) -> Res<Case> {
    let id = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let events = jsonl_indexed(&dir.join("events.jsonl"), malformed)?;
    let observed = jsonl_indexed(&dir.join("observed.jsonl"), malformed)?;
    let mut times: Vec<Option<i64>> = vec![None; events.len()];
    for o in &observed {
        if let (Some(i), Some(t)) = (o["index"].as_u64(), o["observed_at"].as_str().and_then(epoch_ms)) {
            if let Some(slot) = times.get_mut(i as usize) {
                *slot = Some(t);
            }
        }
    }
    let protocol = events.iter().find(|e| e["event"] == "opened").map(|e| js(&e["protocol"]["id"])).unwrap_or_default();
    let ir = json_opt(&dir.join("ir.json"))?.unwrap_or(J::Null);
    let effect: BTreeMap<String, String> = ir["actions"].as_object().into_iter().flatten()
        .map(|(k, v)| (k.clone(), js(&v["effect"]))).collect();
    let bindings = json_opt(&dir.join("bindings.json"))?.unwrap_or(J::Null);
    let mut manual = BTreeSet::new();
    let mut copy = BTreeSet::new();
    for (k, v) in bindings.as_object().into_iter().flatten() {
        if v["manual"] == true {
            manual.insert(k.clone());
        }
        if v.get("copy").is_some_and(|c| !c.is_null()) {
            copy.insert(k.clone());
        }
    }
    let mut evidence_cache: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut takes: Vec<Take> = vec![];
    let mut open: Option<usize> = None;
    let mut outcome = None;
    for (i, e) in events.iter().enumerate() {
        match e["event"].as_str().unwrap_or("") {
            "action_started" => {
                takes.push(Take {
                    action: js(&e["action"]),
                    start_idx: i,
                    end_idx: None,
                    llm: e["llm"] == true,
                    retry_of: e["retry_of"].as_array().into_iter().flatten().map(js).collect(),
                    exit: String::new(),
                    not_called: false,
                    principal: String::new(),
                    evidence: vec![],
                    t_start: times[i],
                    t_end: None,
                    cost: 0.0,
                    calls: 0,
                    causes: vec![],
                });
                open = Some(takes.len() - 1);
            }
            "action_finished" => {
                let Some(k) = open.take().filter(|k| takes[*k].action == js(&e["action"])) else { continue };
                let t = &mut takes[k];
                t.end_idx = Some(i);
                t.exit = js(&e["exit"]);
                t.not_called = e["not_called"].is_string();
                t.principal = js(&e["principal"]);
                t.t_end = times[i];
                for id in e["evidence"].as_array().into_iter().flatten().map(js) {
                    let rec = match evidence_cache.get(&id) {
                        Some(r) => r.clone(),
                        None => {
                            let r = json_opt(&dir.join("evidence").join(format!("{id}.json")))?
                                .map(|v| (js(&v["kind"]), js(&v["result"])))
                                .unwrap_or_default();
                            evidence_cache.insert(id.clone(), r.clone());
                            r
                        }
                    };
                    t.evidence.push(rec);
                }
            }
            "terminated" => outcome = Some(js(&e["outcome"])),
            _ => {}
        }
    }
    Ok(Case { id, stage: stage_of(&protocol), outcome, takes, effect, manual, copy, events, times })
}

impl Case {
    fn kind_of(&self, action: &str, llm: bool) -> &'static str {
        if self.manual.contains(action) {
            "approval"
        } else if self.copy.contains(action) {
            "copy"
        } else if self.effect.get(action).map(String::as_str) == Some("write") {
            if llm { "llm-writer" } else { "writer" }
        } else if self.effect.contains_key(action) {
            "check"
        } else {
            "other"
        }
    }

    /// The first record after event `from` and before event `to` that sends work back: a negative
    /// result, or a non-zero exit of a take that ran, of any action but `skip`.
    fn first_negative(&self, from: usize, to: usize, skip: &str) -> Option<(String, String)> {
        self.takes.iter()
            .filter(|t| t.start_idx > from && t.start_idx < to && t.action != skip && t.real())
            .find_map(|t| {
                if let Some((_, r)) = t.evidence.iter().find(|(_, r)| NEGATIVE.contains(&r.as_str())) {
                    return Some((t.action.clone(), r.clone()));
                }
                (t.exit != "0").then(|| (t.action.clone(), format!("exit {}", t.exit)))
            })
    }

    /// The latest record of `kind` before event `before`: the action that wrote it and its result.
    fn latest_of_kind(&self, kind: &str, before: usize) -> Option<(String, String, bool)> {
        self.takes.iter().rev()
            .filter(|t| t.end_idx.is_some_and(|e| e < before))
            .find_map(|t| t.evidence.iter().find(|(k, _)| k == kind)
                .map(|(_, r)| (t.action.clone(), r.clone(), !t.principal.is_empty() || self.manual.contains(&t.action))))
    }

    /// Causes of every rework take: each real take of an action after its first real take.
    fn assign_causes(&mut self) {
        let mut prev: BTreeMap<String, usize> = BTreeMap::new();
        let n = self.takes.len();
        for k in 0..n {
            if !self.takes[k].real() {
                continue;
            }
            let a = self.takes[k].action.clone();
            let Some(&p) = prev.get(&a) else {
                prev.insert(a, k);
                continue;
            };
            let start = self.takes[k].start_idx;
            let prev_end = self.takes[p].end_idx.unwrap_or(self.takes[p].start_idx);
            let mut causes = vec![];
            for kind in self.takes[k].retry_of.clone() {
                if let Some((from, result, by_decision)) = self.latest_of_kind(&kind, start) {
                    let approval = by_decision || kind.ends_with("_approval");
                    causes.push(Cause { label: format!("{from}:{result}"), from: Some(from), kind: if approval { "approval" } else { "check" } });
                } else {
                    causes.push(Cause { label: format!("{kind}:?"), from: None, kind: "check" });
                }
            }
            if causes.is_empty() {
                let amended = self.takes.iter().any(|t| t.start_idx > prev_end && t.start_idx < start && t.action == "amend.upstream" && t.real());
                let neg = self.first_negative(prev_end, start, &a);
                let kind = if amended { "finding" } else { "arrival" };
                let cause = match neg {
                    Some((from, r)) => Cause { label: format!("arrival:{from}:{r}"), from: Some(from), kind },
                    None => {
                        // What arrived: a decision (an action with a principal), a write from
                        // another case (a `revised` with no step of this case before it).
                        let mut label = if amended { "arrival:amend.upstream".to_string() } else { "unexplained".to_string() };
                        for i in (prev_end + 1)..start {
                            if self.events[i]["event"] != "revised" {
                                continue;
                            }
                            let before = &self.events[i - 1];
                            if before["event"] == "action_finished" && !before["principal"].is_null() {
                                label = format!("decision:{}", js(&before["action"]));
                            } else if before["event"] != "action_finished" && !amended {
                                label = "arrival:outside".to_string();
                            }
                        }
                        Cause { label, from: None, kind }
                    }
                };
                causes.push(cause);
            }
            self.takes[k].causes = causes;
            prev.insert(a, k);
        }
    }

    /// The events of the loop a rework take `k` caused: from its start to the end of the next
    /// take of the action that sent it back (`from`); without one, up to the writer's next take
    /// or the case's last take.
    fn loop_span(&self, k: usize, from: Option<&str>) -> (usize, usize) {
        let t = &self.takes[k];
        let start = t.start_idx;
        let next_w = self.takes[k + 1..].iter().find(|x| x.action == t.action && x.real()).map(|x| x.start_idx);
        let last = self.takes.last().and_then(|x| x.end_idx).unwrap_or(start);
        let cap = next_w.map(|i| i.saturating_sub(1)).unwrap_or(last);
        let end = from
            .and_then(|f| self.takes[k + 1..].iter().find(|x| x.action == f && x.end_idx.is_some()))
            .and_then(|x| x.end_idx)
            .filter(|e| *e <= cap)
            .unwrap_or(cap);
        (start, end.max(t.end_idx.unwrap_or(start)))
    }
}

// ---- the run -----------------------------------------------------------------------------------

#[derive(Default, Clone)]
struct Tokens {
    input: f64,
    output: f64,
    cache_read: f64,
    cache_creation: f64,
    thinking: f64,
}

impl Tokens {
    fn add(&mut self, u: &J) {
        let n = |k: &str| u[k].as_f64().unwrap_or(0.0);
        self.input += n("input_tokens");
        self.output += n("output_tokens");
        self.cache_read += n("cache_read_input_tokens");
        self.cache_creation += n("cache_creation_input_tokens");
        self.thinking += n("thinking_tokens") + u["output_tokens_details"]["thinking_tokens"].as_f64().unwrap_or(0.0);
    }
    fn plus(&mut self, o: &Tokens) {
        self.input += o.input;
        self.output += o.output;
        self.cache_read += o.cache_read;
        self.cache_creation += o.cache_creation;
        self.thinking += o.thinking;
    }
    fn json(&self) -> J {
        json!({"input": self.input as i64, "output": self.output as i64, "cache_read": self.cache_read as i64,
            "cache_creation": self.cache_creation as i64, "thinking": self.thinking as i64})
    }
}

/// Figures of one node, in one case or summed over the cases of a stage.
#[derive(Default, Clone)]
struct Node {
    kind: String,
    instances: usize,
    single: usize,
    takings: usize,
    real_takes: usize,
    not_called: usize,
    could_not_run: usize,
    llm_calls: usize,
    unmatched_llm_calls: usize,
    no_call_takes: usize,
    rework_takes: usize,
    retries_by_cause: BTreeMap<String, usize>,
    cost: f64,
    decider_cost: f64,
    decider_calls: usize,
    rework_cost: f64,
    driver_s: f64,
    rework_driver_s: f64,
    model_s: f64,
    tokens: Tokens,
    models: BTreeMap<String, usize>,
    efforts: BTreeMap<String, usize>,
    sent_back: usize,
}

impl Node {
    fn plus(&mut self, o: &Node) {
        if self.kind.is_empty() {
            self.kind = o.kind.clone();
        }
        self.instances += o.instances;
        self.single += o.single;
        self.takings += o.takings;
        self.real_takes += o.real_takes;
        self.not_called += o.not_called;
        self.could_not_run += o.could_not_run;
        self.llm_calls += o.llm_calls;
        self.unmatched_llm_calls += o.unmatched_llm_calls;
        self.no_call_takes += o.no_call_takes;
        self.rework_takes += o.rework_takes;
        for (k, v) in &o.retries_by_cause {
            *self.retries_by_cause.entry(k.clone()).or_default() += v;
        }
        self.cost += o.cost;
        self.decider_cost += o.decider_cost;
        self.decider_calls += o.decider_calls;
        self.rework_cost += o.rework_cost;
        self.driver_s += o.driver_s;
        self.rework_driver_s += o.rework_driver_s;
        self.model_s += o.model_s;
        self.tokens.plus(&o.tokens);
        for (k, v) in &o.models {
            *self.models.entry(k.clone()).or_default() += v;
        }
        for (k, v) in &o.efforts {
            *self.efforts.entry(k.clone()).or_default() += v;
        }
        self.sent_back += o.sent_back;
    }

    fn looping_factor(&self) -> Option<f64> {
        (self.instances > 0).then(|| round3(self.real_takes as f64 / self.instances as f64))
    }

    fn first_pass_yield(&self) -> Option<f64> {
        (self.instances > 0).then(|| round3(self.single as f64 / self.instances as f64))
    }

    fn json(&self, action: &str, stage: &str) -> J {
        json!({"stage": stage, "action": action, "node": format!("{stage}/{action}"), "kind": self.kind,
            "instances": self.instances, "takings": self.takings, "real_takes": self.real_takes,
            "not_called": self.not_called, "could_not_run": self.could_not_run,
            "llm_calls": self.llm_calls, "unmatched_llm_calls": self.unmatched_llm_calls, "no_call_takes": self.no_call_takes,
            "rework_takes": self.rework_takes, "retries_by_cause": self.retries_by_cause,
            "looping_factor": self.looping_factor(), "first_pass_yield": self.first_pass_yield(),
            "sent_back": self.sent_back,
            "cost_usd": round6(self.cost + self.decider_cost), "llm_cost_usd": round6(self.cost),
            "decider_cost_usd": round6(self.decider_cost), "decider_calls": self.decider_calls,
            "rework_cost_usd": round6(self.rework_cost),
            "driver_s": round3(self.driver_s), "rework_driver_s": round3(self.rework_driver_s), "model_s": round3(self.model_s),
            "tokens": self.tokens.json(), "models": self.models, "efforts": self.efforts})
    }
}

#[derive(Default, Clone)]
struct Edge {
    kind: String,
    stage: String,
    from: String,
    to: String,
    count: usize,
    rework_takes: usize,
    loop_cost: f64,
    loop_driver_s: f64,
    loop_wall_s: f64,
}

#[derive(Default, Clone)]
struct DecisionClass {
    asked: usize,
    approved: usize,
    rejected: usize,
    escalated: usize,
    escalation_suppressed: usize,
    invalid_replies: usize,
    retries: usize,
    refused_by_check: usize,
    calls: usize,
    cost: f64,
    invalid_reasons: BTreeMap<String, usize>,
}

impl DecisionClass {
    fn json(&self) -> J {
        json!({"asked": self.asked, "approved": self.approved, "rejected": self.rejected, "escalated": self.escalated,
            "escalation_suppressed": self.escalation_suppressed, "invalid_replies": self.invalid_replies,
            "retries": self.retries, "refused_by_check": self.refused_by_check, "calls": self.calls,
            "cost_usd": round6(self.cost), "invalid_reasons": self.invalid_reasons})
    }
}

/// The class of an invalid reply's reason: its text up to the first colon, digits dropped.
fn reason_class(r: &str) -> String {
    let head = r.split(':').next().unwrap_or(r).trim();
    let head: String = head.chars().filter(|c| !c.is_ascii_digit()).collect();
    if head.is_empty() { "unknown".into() } else { head.chars().take(80).collect() }
}

/// The decider's figures per class, and per class and action.
fn decisions_by_class(lines: &[J]) -> (BTreeMap<String, DecisionClass>, BTreeMap<String, DecisionClass>) {
    let mut by_class: BTreeMap<String, DecisionClass> = BTreeMap::new();
    let mut by_action: BTreeMap<String, DecisionClass> = BTreeMap::new();
    for l in lines {
        let class = Some(js(&l["class"])).filter(|c| !c.is_empty()).unwrap_or_else(|| js(&l["type"]));
        let key2 = format!("{class}/{}", js(&l["action"]));
        for c in [by_class.entry(class.clone()).or_default(), by_action.entry(key2).or_default()] {
            let d = l["decision"].as_str().unwrap_or("");
            let has_model = l["model"].as_str().is_some_and(|m| !m.is_empty());
            // A call: a line naming a model that is not an effective line repeating an escalation
            // or an invalid reply already counted (decisions.jsonl, scripts/run-trial.sh `line`).
            let repeat = l["suppressed"] == true || l.get("fallback").is_some_and(|f| !f.is_null());
            match d {
                "invalid_reply" => {
                    c.invalid_replies += 1;
                    if l["attempt"].as_u64().unwrap_or(1) <= 2 {
                        c.retries += 1;
                    }
                    *c.invalid_reasons.entry(reason_class(&js(&l["invalid_reason"]))).or_default() += 1;
                    if has_model {
                        c.calls += 1;
                        c.cost += l["cost_usd"].as_f64().unwrap_or(0.0);
                    }
                }
                "escalation_suppressed" => {
                    c.escalation_suppressed += 1;
                    if has_model {
                        c.calls += 1;
                        c.cost += l["cost_usd"].as_f64().unwrap_or(0.0);
                    }
                    if l["valid"] == false {
                        c.invalid_replies += 1;
                        *c.invalid_reasons.entry(reason_class(&js(&l["invalid_reason"]))).or_default() += 1;
                    }
                }
                "approve" | "reject" | "escalate" => {
                    c.asked += 1;
                    match d {
                        "approve" => c.approved += 1,
                        "reject" => c.rejected += 1,
                        _ => c.escalated += 1,
                    }
                    let fallback = l.get("fallback").is_some_and(|f| !f.is_null());
                    let old_invalid = l["valid"] == false;
                    if fallback || old_invalid {
                        c.refused_by_check += 1;
                    }
                    if old_invalid && l["suppressed"] != true {
                        c.invalid_replies += 1;
                        *c.invalid_reasons.entry(reason_class(&js(&l["invalid_reason"]))).or_default() += 1;
                    }
                    if has_model && !repeat {
                        c.calls += 1;
                        c.cost += l["cost_usd"].as_f64().unwrap_or(0.0);
                    }
                }
                _ => {}
            }
        }
    }
    (by_class, by_action)
}

/// The artifact a finding's amended id belongs to.
fn amended_artifact(id: &str) -> String {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if id.strip_prefix('r').is_some_and(digits) {
        return "requirements".into();
    }
    if id.strip_prefix('q').is_some_and(digits) {
        return "questions".into();
    }
    if id.strip_prefix('d').is_some_and(digits) {
        return "dissection".into();
    }
    match id.split_once('.') {
        Some((a, _)) => a.to_string(),
        None => "other".into(),
    }
}

/// The protocol stage that raises findings of a findings.yaml `stage`.
fn finding_protocol_stage(s: &str) -> &str {
    match s {
        "requirements" | "questions" | "dissection" => "intake",
        "context" | "vision" | "roadmap" => "direction",
        other => other,
    }
}

/// Seconds the run spent waiting on a person (as `sdc-run-compare`'s "time waiting on a person"
/// and L7 of `sdc-finding-kpis`): a stop after which the run did not go on by itself.
fn waiting(rounds: &[J], legs: &[i64], stopped: Option<i64>, stopped_by_rule: bool, closed: bool) -> Option<i64> {
    waiting_intervals(rounds, legs, stopped, stopped_by_rule, closed).map(|v| v.iter().map(|(a, b)| (b - a).max(0)).sum())
}

/// The intervals (epoch seconds) `waiting` counts as time waiting on a person.
pub fn waiting_intervals(rounds: &[J], legs: &[i64], stopped: Option<i64>, stopped_by_rule: bool, closed: bool) -> Option<Vec<(i64, i64)>> {
    if rounds.is_empty() {
        return None;
    }
    let first_leg = legs.iter().min().copied();
    let mut out = vec![];
    for (i, r) in rounds.iter().enumerate() {
        let end = r["ended_epoch"].as_i64().unwrap_or(0);
        let verdict = r["verdict"].as_str().unwrap_or("");
        let person_verdict = matches!(verdict, "awaiting_person" | "awaiting_authority");
        let start = r["started_epoch"].as_i64().unwrap_or(0);
        let legacy = first_leg.is_none_or(|l| start < l);
        match rounds.get(i + 1) {
            Some(n) => {
                let ns = n["started_epoch"].as_i64().unwrap_or(end);
                let waits = if legacy { person_verdict } else { legs.iter().any(|&l| l >= end && l <= ns) };
                if waits {
                    out.push((end, ns.max(end)));
                }
            }
            None if closed || stopped_by_rule || !(person_verdict || !legacy) => {}
            None => {
                if let Some(at) = stopped {
                    out.push((end, at.max(end)));
                }
            }
        }
    }
    Some(out)
}

/// Whether a decisions.jsonl line is one decider call, as `decisions_by_class` counts calls: a
/// line naming a model that does not repeat an escalation or an invalid reply already counted.
pub fn decider_line_is_call(l: &J) -> bool {
    let has_model = l["model"].as_str().is_some_and(|m| !m.is_empty());
    let repeat = l["suppressed"] == true || l.get("fallback").is_some_and(|f| !f.is_null());
    match l["decision"].as_str().unwrap_or("") {
        "invalid_reply" | "escalation_suppressed" => has_model,
        "approve" | "reject" | "escalate" => has_model && !repeat,
        _ => false,
    }
}

pub type WaitingInputs = (Vec<J>, Vec<i64>, Option<i64>, bool);

/// The rounds, the starts of the run's legs, the stop time and whether a rule stopped it: what
/// `waiting_intervals` needs.
pub fn waiting_inputs(dir: &Path) -> Res<WaitingInputs> {
    let mut m = 0;
    let rounds = jsonl(&dir.join("rounds.jsonl"), &mut m)?.unwrap_or_default();
    let settings = jsonl(&dir.join("run-settings.jsonl"), &mut m)?.unwrap_or_default();
    let legs: Vec<i64> = settings.iter().filter_map(|s| s["at"].as_str().and_then(epoch_ms)).map(|t| t.div_euclid(1000)).collect();
    let stopped = json_opt(&dir.join("run-stopped.json"))?;
    let stopped_at = stopped.as_ref().and_then(|s| s["at"].as_str()).and_then(epoch_ms).map(|t| t.div_euclid(1000));
    let by_rule = stopped.as_ref().and_then(|s| s["by"].as_str()).is_some_and(|b| b.starts_with("rule:"));
    Ok((rounds, legs, stopped_at, by_rule))
}

/// The value at a dotted path of a JSON document (`decider.model`, `models.writer.model`).
pub fn at_path<'a>(v: &'a J, path: &str) -> &'a J {
    path.split('.').fold(v, |v, k| &v[k])
}

/// The settings of the run's last leg as a flat map of dotted keys to strings.
fn flat_settings(s: &J, prefix: &str, out: &mut BTreeMap<String, String>) {
    match s {
        J::Object(m) => {
            for (k, v) in m {
                if matches!(k.as_str(), "at" | "format" | "nodes" | "llm_calls_before") {
                    continue;
                }
                let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                flat_settings(v, &key, out);
            }
        }
        J::Null => {
            out.insert(prefix.to_string(), "null".into());
        }
        J::Array(_) => {
            out.insert(prefix.to_string(), s.to_string());
        }
        other => {
            out.insert(prefix.to_string(), js(other));
        }
    }
}

/// The root case directory of a run: `store/IDEA-1` (an idea run), else the one directory under
/// `store/` that holds an `events.jsonl` (a bugfix run's `store/BUG-1`), else `store/IDEA-1`.
pub fn root_case_dir(dir: &Path) -> PathBuf {
    let idea = dir.join("store").join("IDEA-1");
    if idea.join("events.jsonl").is_file() {
        return idea;
    }
    let mut found: Vec<PathBuf> = fs::read_dir(dir.join("store")).into_iter().flatten().flatten()
        .map(|e| e.path()).filter(|p| p.join("events.jsonl").is_file()).collect();
    found.sort();
    if found.len() == 1 { found.remove(0) } else { idea }
}

/// `sdc.run-metrics/1` of the run directory `dir`.
pub fn load(dir: &Path) -> Res<J> {
    let root = root_case_dir(dir);
    if !root.join("events.jsonl").is_file() {
        return Err(Fail(format!("cannot read {}: no such file", root.join("events.jsonl").display())));
    }
    let root_id = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let name = fs::canonicalize(dir).ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .or_else(|| dir.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "run".to_string());
    let mut malformed = 0usize;
    let mut dirs = vec![];
    case_dirs(&root, &mut dirs);
    let mut cases = vec![];
    for d in &dirs {
        let mut c = load_case(d, &mut malformed)?;
        c.assign_causes();
        cases.push(c);
    }

    // Model calls (llm-calls.jsonl), joined in order to the real model takes of the same case and
    // action; decider lines (decisions.jsonl).
    let calls = jsonl(&dir.join("llm-calls.jsonl"), &mut malformed)?;
    let decisions = jsonl(&dir.join("decisions.jsonl"), &mut malformed)?;
    let mut calls_by: BTreeMap<(String, String), Vec<&J>> = BTreeMap::new();
    for l in calls.iter().flatten() {
        calls_by.entry((js(&l["case"]), js(&l["action"]))).or_default().push(l);
    }
    let decision_lines: Vec<J> = decisions.clone().unwrap_or_default();
    let decider_call = |l: &J| -> f64 {
        let has_model = l["model"].as_str().is_some_and(|m| !m.is_empty());
        let repeat = l["suppressed"] == true || l.get("fallback").is_some_and(|f| !f.is_null());
        if has_model && !repeat { l["cost_usd"].as_f64().unwrap_or(0.0) } else { 0.0 }
    };

    let mut stage_nodes: BTreeMap<(usize, String, String), Node> = BTreeMap::new();
    let mut case_docs = vec![];
    let mut edges: BTreeMap<String, Edge> = BTreeMap::new();
    // Loop cost of the rework takes whose cause is a finding, per protocol stage, to share out over
    // the finding edges of that stage.
    let mut finding_loops: BTreeMap<String, (usize, f64, f64, f64)> = BTreeMap::new();
    for c in cases.iter_mut() {
        let mut nodes: BTreeMap<String, Node> = BTreeMap::new();
        // Join model calls.
        let actions: BTreeSet<String> = c.takes.iter().map(|t| t.action.clone()).collect();
        for a in &actions {
            let lines = calls_by.get(&(c.id.clone(), a.clone())).cloned().unwrap_or_default();
            // A call belongs to the take whose observed span holds the call's `started` (whole
            // seconds); with no observed times, calls and takes are paired in order.
            let mut unmatched = 0;
            let mut cursor = 0;
            let idx: Vec<usize> = (0..c.takes.len())
                .filter(|&i| { let t = &c.takes[i]; &t.action == a && t.llm && !t.not_called && t.end_idx.is_some() })
                .collect();
            for l in &lines {
                let started = l["started"].as_str().and_then(epoch_ms);
                let hit = idx.iter().copied().skip(cursor).position(|i| {
                    let t = &c.takes[i];
                    match (started, t.t_start, t.t_end) {
                        (Some(s), Some(a0), Some(b0)) => s >= a0.div_euclid(1000) * 1000 && s <= b0,
                        _ => true,
                    }
                });
                match hit {
                    Some(p) => {
                        let i = idx[cursor + p];
                        c.takes[i].cost += l["cost_usd"].as_f64().unwrap_or(0.0);
                        c.takes[i].calls += 1;
                        cursor += if started.is_some() && c.takes[i].t_start.is_some() { p } else { p + 1 };
                    }
                    None => unmatched += 1,
                }
            }
            let n = nodes.entry(a.clone()).or_default();
            n.unmatched_llm_calls = unmatched;
            n.no_call_takes = idx.iter().filter(|&&i| c.takes[i].calls == 0 && c.takes[i].real()).count();
            for l in &lines {
                n.llm_calls += 1;
                n.cost += l["cost_usd"].as_f64().unwrap_or(0.0);
                n.model_s += l["duration_ms"].as_f64().unwrap_or(0.0) / 1000.0;
                n.tokens.add(&l["usage"]);
                *n.models.entry(Some(js(&l["model"])).filter(|m| !m.is_empty()).unwrap_or_else(|| "unknown".into())).or_default() += 1;
                let effort = match l.get("effort") {
                    Some(J::String(e)) => e.clone(),
                    Some(J::Null) => "default".into(),
                    _ => "unrecorded".into(),
                };
                *n.efforts.entry(effort).or_default() += 1;
            }
        }
        // The decider's calls on an approval node of this case.
        for l in decision_lines.iter().filter(|l| js(&l["case"]) == c.id) {
            let a = js(&l["action"]);
            if let Some(n) = nodes.get_mut(&a) {
                let cost = decider_call(l);
                n.decider_cost += cost;
                if l["model"].as_str().is_some_and(|m| !m.is_empty()) && l["suppressed"] != true && l.get("fallback").is_none_or(|f| f.is_null()) {
                    n.decider_calls += 1;
                }
            }
        }
        // Takes.
        for t in &c.takes {
            let kind = c.kind_of(&t.action, t.llm).to_string();
            let n = nodes.entry(t.action.clone()).or_default();
            if n.kind.is_empty() || kind == "llm-writer" {
                n.kind = kind;
            }
            n.takings += 1;
            if t.not_called {
                n.not_called += 1;
            }
            if t.could_not_run() {
                n.could_not_run += 1;
            }
            n.driver_s += t.driver_s();
            if t.real() {
                n.real_takes += 1;
            }
            if !t.causes.is_empty() {
                n.rework_takes += 1;
                n.rework_cost += t.cost;
                n.rework_driver_s += t.driver_s();
                for cause in &t.causes {
                    *n.retries_by_cause.entry(cause.label.clone()).or_default() += 1;
                }
            }
        }
        for n in nodes.values_mut() {
            if n.real_takes > 0 {
                n.instances = 1;
                n.single = usize::from(n.real_takes == 1);
            }
        }
        // Edges: one per cause of each rework take, with the loop it caused.
        for k in 0..c.takes.len() {
            let causes = c.takes[k].causes.clone();
            // An edge sends the run back to a writer: a check asked again, an approval asked
            // again and amend.upstream applying a finding are parts of a loop, not its cause.
            let wk = c.kind_of(&c.takes[k].action, c.takes[k].llm);
            if causes.is_empty() || !matches!(wk, "llm-writer" | "writer") || c.takes[k].action == "amend.upstream" {
                continue;
            }
            let share = 1.0 / causes.len() as f64;
            for cause in &causes {
                if let Some(f) = &cause.from {
                    if let Some(n) = nodes.get_mut(f) {
                        n.sent_back += 1;
                    }
                }
                let (s, e) = c.loop_span(k, cause.from.as_deref());
                let in_loop: Vec<&Take> = c.takes.iter().filter(|t| t.start_idx >= s && t.end_idx.is_some_and(|x| x <= e)).collect();
                let t0 = c.times.get(s).copied().flatten();
                let t1 = c.times.get(e).copied().flatten();
                let decider: f64 = match (t0, t1) {
                    (Some(a), Some(b)) => decision_lines.iter()
                        .filter(|l| js(&l["case"]) == c.id)
                        .filter(|l| l["time"].as_str().and_then(epoch_ms).is_some_and(|t| t >= a - 999 && t <= b))
                        .map(decider_call).sum(),
                    _ => 0.0,
                };
                let cost = (in_loop.iter().map(|t| t.cost).sum::<f64>() + decider) * share;
                let driver: f64 = in_loop.iter().map(|t| t.driver_s()).sum::<f64>() * share;
                let wall = match (t0, t1) {
                    (Some(a), Some(b)) if b >= a => (b - a) as f64 / 1000.0 * share,
                    _ => 0.0,
                };
                let to = c.takes[k].action.clone();
                if cause.kind == "finding" {
                    let f = finding_loops.entry(c.stage.clone()).or_default();
                    f.0 += 1;
                    f.1 += cost;
                    f.2 += driver;
                    f.3 += wall;
                    continue;
                }
                let key = format!("{}: {} -> {}", c.stage, cause.label, to);
                let ed = edges.entry(key).or_default();
                ed.kind = cause.kind.to_string();
                ed.stage = c.stage.clone();
                ed.from = cause.label.clone();
                ed.to = to;
                ed.count += 1;
                ed.rework_takes += 1;
                ed.loop_cost += cost;
                ed.loop_driver_s += driver;
                ed.loop_wall_s += wall;
            }
        }
        let si = stage_index(&c.stage).unwrap_or(STAGES.len());
        for (a, n) in &nodes {
            stage_nodes.entry((si, c.stage.clone(), a.clone())).or_default().plus(n);
        }
        case_docs.push(json!({"case": c.id, "stage": c.stage, "outcome": c.outcome,
            "nodes": nodes.iter().map(|(a, n)| n.json(a, &c.stage)).collect::<Vec<_>>()}));
    }

    // Finding edges: findings raised at stage S that amend an artifact of stage T (work/findings.yaml).
    let findings: Option<Y> = read_opt(&root.join("work").join("findings.yaml"))?.and_then(|t| serde_yaml::from_str(&t).ok());
    let flist: Vec<Y> = findings.as_ref().and_then(|f| f.get("findings")).and_then(Y::as_sequence).cloned().unwrap_or_default();
    let ys = |v: &Y, k: &str| v.get(k).and_then(Y::as_str).unwrap_or("").to_string();
    let mut finding_counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut raised = 0;
    for f in flist.iter().filter(|f| ys(f, "status") != "rejected") {
        raised += 1;
        let s = ys(f, "stage");
        let targets: BTreeSet<String> = f.get("resolution").and_then(|r| r.get("amends")).and_then(Y::as_sequence)
            .into_iter().flatten().filter_map(Y::as_str).map(|id| finding_protocol_stage(&amended_artifact(id)).to_string()).collect();
        for t in targets {
            *finding_counts.entry((s.clone(), t)).or_default() += 1;
        }
    }
    let mut shares: BTreeMap<String, usize> = BTreeMap::new();
    for ((s, _), n) in &finding_counts {
        *shares.entry(finding_protocol_stage(s).to_string()).or_default() += n;
    }
    for ((s, t), n) in &finding_counts {
        let ps = finding_protocol_stage(s).to_string();
        let total = shares[&ps] as f64;
        let (rw, cost, drv, wall) = finding_loops.get(&ps).copied().unwrap_or_default();
        let part = *n as f64 / total;
        edges.insert(format!("{ps}: finding@{s} -> {t}"), Edge {
            kind: "finding".into(), stage: ps.clone(), from: format!("finding@{s}"), to: t.clone(), count: *n,
            rework_takes: ((rw as f64) * part).round() as usize, loop_cost: cost * part, loop_driver_s: drv * part, loop_wall_s: wall * part,
        });
    }
    for (ps, (rw, cost, drv, wall)) in &finding_loops {
        if !shares.contains_key(ps) {
            edges.insert(format!("{ps}: finding@{ps} -> unattributed"), Edge {
                kind: "finding".into(), stage: ps.clone(), from: format!("finding@{ps}"), to: "unattributed".into(), count: 0,
                rework_takes: *rw, loop_cost: *cost, loop_driver_s: *drv, loop_wall_s: *wall,
            });
        }
    }

    // Decisions.
    let (by_class, by_action) = decisions_by_class(&decision_lines);

    // Per stage and per run.
    let mut stages: BTreeMap<(usize, String), Node> = BTreeMap::new();
    let mut total = Node::default();
    let mut writers = Node::default();
    for ((si, st, _), n) in &stage_nodes {
        stages.entry((*si, st.clone())).or_default().plus(n);
        total.plus(n);
        if n.kind == "llm-writer" {
            writers.plus(n);
        }
    }
    let node_docs: Vec<J> = stage_nodes.iter().map(|((_, st, a), n)| n.json(a, st)).collect();
    let stage_docs: Vec<J> = stages.iter().map(|((_, st), n)| {
        let mut d = n.json("*", st);
        d.as_object_mut().unwrap().remove("action");
        d["node"] = json!(st);
        d["kind"] = json!("stage");
        d["cases"] = json!(cases.iter().filter(|c| &c.stage == st).count());
        d
    }).collect();
    let edge_docs: Vec<J> = edges.iter().map(|(k, e)| json!({"edge": k, "kind": e.kind, "stage": e.stage, "from": e.from, "to": e.to,
        "count": e.count, "rework_takes": e.rework_takes, "loop_cost_usd": round6(e.loop_cost),
        "loop_driver_s": round3(e.loop_driver_s), "loop_wall_s": round3(e.loop_wall_s)})).collect();

    // End, clean, time.
    let status = json_opt(&dir.join("status.json"))?;
    let stopped = json_opt(&dir.join("run-stopped.json"))?;
    let mut reopens = 0;
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with("run-stopped.") && n.ends_with(".json") && n != "run-stopped.json" {
                reopens += 1;
            }
        }
    }
    let defects = jsonl(&dir.join("defects.jsonl"), &mut malformed)?;
    let forced_defects = defects.iter().flatten().filter(|d| d["forced"] == true).count();
    let count_of = |v: &J| v.as_u64().map(|n| n as usize).or_else(|| v.as_array().map(Vec::len)).unwrap_or(0);
    let forced = status.as_ref().map(|s| count_of(&s["forced"])).unwrap_or(0).max(forced_defects);
    let unstuck = status.as_ref().map(|s| count_of(&s["unstuck"])).unwrap_or(0);
    let status_reopens = status.as_ref().map(|s| count_of(&s["reopens"])).unwrap_or(0);
    let reopens = reopens.max(status_reopens);
    let root_case = cases.iter().find(|c| c.id == root_id);
    let closed_outcome = root_case.and_then(|c| c.outcome.clone())
        .or_else(|| status.as_ref().filter(|s| s["state"] == "closed").map(|s| js(&s["outcome"])));
    let (end, end_kind) = if let Some(o) = &closed_outcome {
        (format!("closed: {o}"), "closed")
    } else if let Some(s) = &stopped {
        let cause = Some(js(&s["cause"])).filter(|c| !c.is_empty()).unwrap_or_else(|| js(&s["verdict_at_stop"]));
        (format!("stopped: {cause} in {}", js(&s["case"])), "stopped")
    } else {
        ("open: no close and no stop record".to_string(), "open")
    };
    let delivered = match (&closed_outcome, status.as_ref().and_then(|s| s["delivered"].as_bool())) {
        (Some(_), Some(d)) => d,
        (Some(o), None) => o == "operating",
        _ => false,
    };
    let clean = end_kind == "closed" && reopens == 0 && forced == 0 && unstuck == 0;
    let rounds = jsonl(&dir.join("rounds.jsonl"), &mut malformed)?.unwrap_or_default();
    let settings = jsonl(&dir.join("run-settings.jsonl"), &mut malformed)?.unwrap_or_default();
    let legs: Vec<i64> = settings.iter().filter_map(|s| s["at"].as_str().and_then(epoch_ms)).map(|t| t.div_euclid(1000)).collect();
    let stopped_at = stopped.as_ref().and_then(|s| s["at"].as_str()).and_then(epoch_ms).map(|t| t.div_euclid(1000));
    let by_rule = stopped.as_ref().and_then(|s| s["by"].as_str()).is_some_and(|b| b.starts_with("rule:"));
    let wait = waiting(&rounds, &legs, stopped_at, by_rule, closed_outcome.is_some());
    let driver_rounds_s: Option<f64> = (!rounds.is_empty()).then(|| rounds.iter().filter_map(|r| r["seconds"].as_f64()).sum());
    let all_times: Vec<i64> = cases.iter().flat_map(|c| c.times.iter().flatten().copied()).collect();
    let wall_s = match (all_times.iter().min(), all_times.iter().max()) {
        (Some(a), Some(b)) => Some((b - a) / 1000),
        _ => None,
    };
    let furthest_opened = cases.iter().filter_map(|c| stage_index(&c.stage)).max();
    let furthest_passed = cases.iter().filter(|c| c.outcome.as_deref() == Some("passed")).filter_map(|c| stage_index(&c.stage)).max();
    let llm_cost: f64 = calls.iter().flatten().map(|l| l["cost_usd"].as_f64().unwrap_or(0.0)).sum();
    let decider_cost: f64 = by_class.values().map(|c| c.cost).sum();
    let decider_calls: usize = by_class.values().map(|c| c.calls).sum();
    let last_settings = settings.last().cloned().unwrap_or(J::Null);
    let mut flat = BTreeMap::new();
    flat_settings(&last_settings, "", &mut flat);

    let run = json!({
        "end": end, "end_kind": end_kind, "closed": end_kind == "closed", "outcome": closed_outcome, "delivered": delivered,
        "stop_cause": stopped.as_ref().map(|s| Some(js(&s["cause"])).filter(|c| !c.is_empty()).unwrap_or_else(|| js(&s["verdict_at_stop"]))),
        "finished": end_kind != "open",
        "clean": clean, "reopens": reopens, "forced": forced, "unstuck": unstuck,
        "furthest_stage": furthest_opened.map(|i| STAGES[i]), "furthest_stage_index": furthest_opened,
        "furthest_stage_passed": furthest_passed.map(|i| STAGES[i]),
        "cases": cases.len(),
        "takings": total.takings, "real_takes": total.real_takes, "not_called": total.not_called,
        "could_not_run": total.could_not_run, "rework_takes": total.rework_takes,
        "writer_rework_takes": writers.rework_takes,
        "looping_factor": writers.looping_factor(), "first_pass_yield": writers.first_pass_yield(),
        "llm_calls": calls.as_ref().map(Vec::len), "llm_cost_usd": calls.as_ref().map(|_| round6(llm_cost)),
        "decider_calls": decisions.as_ref().map(|_| decider_calls), "decider_cost_usd": decisions.as_ref().map(|_| round6(decider_cost)),
        "cost_usd": round6(llm_cost + decider_cost), "rework_cost_usd": round6(total.rework_cost),
        "tokens": total.tokens.json(),
        "driver_s": driver_rounds_s.map(round3), "take_driver_s": round3(total.driver_s), "model_s": round3(total.model_s),
        "wall_s": wall_s, "waiting_on_person_s": wait,
        "findings_raised": raised,
    });
    Ok(json!({
        "format": FORMAT, "run": name, "settings": flat, "metrics": run,
        "stages": stage_docs, "nodes": node_docs, "edges": edge_docs,
        "decisions": {"by_class": by_class.iter().map(|(k, v)| (k.clone(), v.json())).collect::<Map<_, _>>(),
            "by_action": by_action.iter().map(|(k, v)| (k.clone(), v.json())).collect::<Map<_, _>>()},
        "cases": case_docs,
        "records": {"llm_calls": calls.is_some(), "decisions": decisions.is_some(), "rounds": !rounds.is_empty(),
            "run_settings": !settings.is_empty(), "status": status.is_some(), "run_stopped": stopped.is_some(),
            "defects": defects.is_some(), "findings": findings.is_some(), "malformed_lines": malformed},
    }))
}

// ---- flat metrics (for groups of runs) ---------------------------------------------------------

/// Node fields compared across runs.
pub const NODE_FIELDS: [&str; 8] =
    ["real_takes", "rework_takes", "looping_factor", "first_pass_yield", "not_called", "cost_usd", "rework_cost_usd", "driver_s"];
pub const EDGE_FIELDS: [&str; 3] = ["count", "loop_cost_usd", "loop_driver_s"];
pub const RUN_FIELDS: [&str; 20] = ["closed", "delivered", "clean", "furthest_stage_index", "real_takes", "rework_takes",
    "writer_rework_takes", "looping_factor", "first_pass_yield", "not_called", "llm_calls", "llm_cost_usd", "decider_calls",
    "decider_cost_usd", "cost_usd", "rework_cost_usd", "driver_s", "wall_s", "waiting_on_person_s", "findings_raised"];
pub const DECISION_FIELDS: [&str; 7] =
    ["asked", "approved", "rejected", "escalation_suppressed", "invalid_replies", "retries", "refused_by_check"];

fn num(v: &J) -> Option<f64> {
    match v {
        J::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        J::Number(n) => n.as_f64(),
        _ => None,
    }
}

/// Every comparable figure of one run, by name: `run.<field>`, `node.<stage>/<action>.<field>`,
/// `edge.<stage>: <from> -> <to>.<field>`, `decision.<class>.<field>`. Only what the run recorded; `metric_of` says when a missing
/// figure reads 0.
pub fn flat_metrics(doc: &J) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    for f in RUN_FIELDS {
        if let Some(x) = num(&doc["metrics"][f]) {
            out.insert(format!("run.{f}"), x);
        }
    }
    // All tokens of the run, the four kinds summed (input, output, cache read, cache creation).
    let t = &doc["metrics"]["tokens"];
    if let (Some(a), Some(b), Some(c), Some(d)) = (t["input"].as_f64(), t["output"].as_f64(), t["cache_read"].as_f64(), t["cache_creation"].as_f64()) {
        out.insert("run.tokens_total".into(), a + b + c + d);
    }
    for n in doc["nodes"].as_array().into_iter().flatten() {
        let node = js(&n["node"]);
        for f in NODE_FIELDS {
            if let Some(x) = num(&n[f]) {
                out.insert(format!("node.{node}.{f}"), x);
            }
        }
    }
    for e in doc["edges"].as_array().into_iter().flatten() {
        for f in EDGE_FIELDS {
            if let Some(x) = num(&e[f]) {
                out.insert(format!("edge.{}.{f}", js(&e["edge"])), x);
            }
        }
    }
    for (c, v) in doc["decisions"]["by_class"].as_object().into_iter().flatten() {
        for f in DECISION_FIELDS {
            if let Some(x) = num(&v[f]) {
                out.insert(format!("decision.{c}.{f}"), x);
            }
        }
    }
    out
}

/// The value of metric `name` in run `doc`, with the rule for metrics a run does not have: an
/// edge reads 0 when the run took the edge's target node (a finding edge: when the run opened a
/// case of its stage), a decision field 0 when the run has a decision log; otherwise the run has no value (and is not counted in n).
pub fn metric_of(doc: &J, flat: &BTreeMap<String, f64>, name: &str) -> Option<f64> {
    if let Some(x) = flat.get(name) {
        return Some(*x);
    }
    if let Some(rest) = name.strip_prefix("edge.") {
        let (edge, _field) = rest.rsplit_once('.')?;
        let (stage, tail) = edge.split_once(": ")?;
        let to = tail.rsplit_once(" -> ").map(|(_, t)| t)?;
        if tail.starts_with("finding@") {
            // Defined in a run that opened a case of the stage the loop runs in.
            return doc["stages"].as_array().into_iter().flatten().any(|s| js(&s["stage"]) == stage).then_some(0.0);
        }
        let node = format!("{stage}/{to}");
        return doc["nodes"].as_array().into_iter().flatten().any(|n| js(&n["node"]) == node && n["real_takes"].as_u64().unwrap_or(0) > 0).then_some(0.0);
    }
    if name.starts_with("decision.") {
        return doc["records"]["decisions"].as_bool().filter(|b| *b).map(|_| 0.0);
    }
    None
}

// ---- text form ---------------------------------------------------------------------------------

fn opt(v: &J) -> String {
    if v.is_null() { "-".into() } else { js(v) }
}

/// A short text form of one run's metrics.
pub fn text(doc: &J) -> String {
    let m = &doc["metrics"];
    let mut s = String::new();
    let _ = writeln!(s, "run {}: {}", js(&doc["run"]), js(&m["end"]));
    let _ = writeln!(s, "  clean {} (reopens {}, forced {}, unstuck {}); furthest stage {} (passed {})",
        m["clean"], m["reopens"], m["forced"], m["unstuck"], opt(&m["furthest_stage"]), opt(&m["furthest_stage_passed"]));
    let _ = writeln!(s, "  cost {} USD (llm {} in {} calls, decider {} in {} calls); rework cost {}",
        opt(&m["cost_usd"]), opt(&m["llm_cost_usd"]), opt(&m["llm_calls"]), opt(&m["decider_cost_usd"]), opt(&m["decider_calls"]), opt(&m["rework_cost_usd"]));
    let _ = writeln!(s, "  driver {} s, wall {} s, waiting on a person {} s; takes {} real {} rework {} not called {}",
        opt(&m["driver_s"]), opt(&m["wall_s"]), opt(&m["waiting_on_person_s"]), m["takings"], m["real_takes"], m["rework_takes"], m["not_called"]);
    let _ = writeln!(s, "  writers: looping factor {}, first-pass yield {}", opt(&m["looping_factor"]), opt(&m["first_pass_yield"]));
    let mut nodes: Vec<&J> = doc["nodes"].as_array().into_iter().flatten()
        .filter(|n| n["rework_takes"].as_u64().unwrap_or(0) > 0 && n["kind"] != "check").collect();
    nodes.sort_by(|a, b| b["looping_factor"].as_f64().unwrap_or(0.0).total_cmp(&a["looping_factor"].as_f64().unwrap_or(0.0))
        .then_with(|| js(&a["node"]).cmp(&js(&b["node"]))));
    if !nodes.is_empty() {
        let _ = writeln!(s, "nodes with rework (looping factor, real takes / instances, rework cost USD, causes):");
        for n in nodes {
            let causes: Vec<String> = n["retries_by_cause"].as_object().into_iter().flatten().map(|(k, v)| format!("{k} {v}")).collect();
            let _ = writeln!(s, "  {} {}: LF {} ({}/{}), FPY {}, rework {} USD; {}", n["kind"].as_str().unwrap_or(""), js(&n["node"]),
                opt(&n["looping_factor"]), n["real_takes"], n["instances"], opt(&n["first_pass_yield"]), opt(&n["rework_cost_usd"]), causes.join(", "));
        }
    }
    let edges: Vec<&J> = doc["edges"].as_array().into_iter().flatten().collect();
    if !edges.is_empty() {
        let _ = writeln!(s, "edges (count, loop cost USD, loop driver s):");
        for e in edges {
            let _ = writeln!(s, "  [{}] {}: {}, {}, {}", js(&e["kind"]), js(&e["edge"]), e["count"], e["loop_cost_usd"], e["loop_driver_s"]);
        }
    }
    if let Some(dc) = doc["decisions"]["by_class"].as_object().filter(|m| !m.is_empty()) {
        let _ = writeln!(s, "decisions (asked, approved, rejected, escalation suppressed, invalid replies, retries, refused by check):");
        for (k, v) in dc {
            let _ = writeln!(s, "  {k}: {} {} {} {} {} {} {}", v["asked"], v["approved"], v["rejected"], v["escalation_suppressed"],
                v["invalid_replies"], v["retries"], v["refused_by_check"]);
        }
    }
    s
}

// ---- statistics over groups --------------------------------------------------------------------

/// Median, min and max of values (not empty).
pub fn summary(v: &[f64]) -> (f64, f64, f64) {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let n = s.len();
    let med = if n % 2 == 1 { s[n / 2] } else { (s[n / 2 - 1] + s[n / 2]) / 2.0 };
    (med, s[0], s[n - 1])
}

/// n choose k as a float.
pub fn choose(n: usize, k: usize) -> f64 {
    (0..k).fold(1.0, |acc, i| acc * (n - i) as f64 / (i + 1) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amended_ids_map_to_artifacts() {
        assert_eq!(amended_artifact("r12"), "requirements");
        assert_eq!(amended_artifact("q3"), "questions");
        assert_eq!(amended_artifact("context.audience"), "context");
        assert_eq!(amended_artifact("rx"), "other");
    }

    #[test]
    fn median_and_choose() {
        assert_eq!(summary(&[3.0, 1.0, 2.0]), (2.0, 1.0, 3.0));
        assert_eq!(summary(&[4.0, 1.0, 2.0, 3.0]), (2.5, 1.0, 4.0));
        assert_eq!(choose(6, 3), 20.0);
        assert_eq!(choose(8, 4), 70.0);
    }

    #[test]
    fn stages_from_protocols() {
        assert_eq!(stage_of("sdc"), "idea");
        assert_eq!(stage_of("sdc.story"), "story");
    }
}
