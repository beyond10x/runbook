//! The late-finding loop's programs (docs/trial-2.md, "The late-finding loop"). Run with cwd = the
//! case tree's `work/`. Deterministic: no clock, no network, no LLM.
//!
//! ```text
//! sdc-findings --rules <consistency-rules.json> consistency
//!     print every consistency-rule hit; exit 0 none, 5 some
//! sdc-findings --rules <…> gate <stage>
//!     exit 5 while a finding is open or a rule hit is neither resolved nor dismissed, else 0
//!     (folded into each stage check)
//! sdc-findings --rules <…> promote <node> <file>
//!     check the findings a stage agent wrote (shape, citations) and add the valid ones to
//!     findings.yaml as open; the invalid ones as rejected, with the reason
//! sdc-findings --rules <…> amend <stage>
//!     resolve the open findings: classify each (agent or person), take the default or the
//!     person's decision, amend questions.yaml and requirements.yaml, verify the amendment, and
//!     mark the stage's derived artifact as derived from earlier requirements
//! ```
//!
//! `findings.yaml`: `{findings: [{id, type, stage, raised_by, cites, says, options, default,
//! scope_changing, status, scope, basis, decision, decided_by, resolution, attempts,
//! refused_decision}]}`. status: open | needs_person | resolved | dismissed | rejected. A person
//! answers a needs_person finding by setting `decision` (one of the options or their own text)
//! with `decided_by: person:<id>`, or `status: dismissed`; a decision with no principal is not
//! applied (nothing records a person who did not act). A decider sets `decision` and `decided_by: decider:<kind>`; its
//! amendment is checked as the agent's are, and a refused one is reverted, taken off the finding
//! and kept as `refused_decision` {decision, decided_by, why}, so the finding waits for a person.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use regex::{NoExpand, Regex, RegexBuilder};
use serde_json::json;
use serde_yaml::{Mapping, Value};
use sha2::{Digest, Sha256};

const TYPES: [&str; 3] = ["ambiguity", "contradiction", "missing_decision"];
/// The artifact of each stage that is derived from the requirements and is re-derived after an
/// amendment: a tree gets a `.derived-from` file (a new revision); the vision is set aside.
const DERIVED: [(&str, &str); 3] = [("spec", "spec"), ("plan", "aep"), ("story", "impl")];
const VISION: &str = "aep/vision/product.md";

type R<T> = Result<T, String>;

fn is_open(s: &str) -> bool {
    s == "open" || s == "needs_person"
}

/// A decider's principal (`decider:<kind>`), as scripts/run-trial.sh writes it.
fn is_decider(principal: &str) -> bool {
    principal
        .strip_prefix("decider:")
        .is_some_and(|k| !k.is_empty())
}

/// A person's principal (`person:<id>`): written only where a person acted.
fn is_person(principal: &str) -> bool {
    principal
        .strip_prefix("person:")
        .is_some_and(|k| !k.is_empty())
}

fn load(path: &str) -> Value {
    fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_yaml::from_str::<Value>(&t).ok())
        .filter(|v| v.is_mapping())
        .unwrap_or_else(|| Value::Mapping(Mapping::new()))
}

fn save(path: &str, v: &Value) -> R<()> {
    let text = serde_yaml::to_string(v).map_err(|e| e.to_string())?;
    let tmp = format!("{path}.tmp");
    fs::write(&tmp, text).map_err(|e| format!("{tmp}: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("{path}: {e}"))
}

fn sha16(path: &str) -> String {
    match fs::read(path) {
        Ok(b) => Sha256::digest(&b)
            .iter()
            .take(8)
            .map(|x| format!("{x:02x}"))
            .collect(),
        Err(_) => "none".into(),
    }
}

fn st<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

fn seq<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k)
        .and_then(Value::as_sequence)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn strs(v: &Value, k: &str) -> Vec<String> {
    seq(v, k)
        .iter()
        .filter_map(|x| x.as_str().map(String::from))
        .collect()
}

fn s(x: &str) -> Value {
    Value::String(x.to_string())
}

fn sv(xs: &[String]) -> Value {
    Value::Sequence(xs.iter().map(|x| s(x)).collect())
}

fn re(pattern: &str) -> R<Regex> {
    RegexBuilder::new(pattern)
        .case_insensitive(true)
        .build()
        .map_err(|e| e.to_string())
}

fn by_len_then_id(ids: &mut [String]) {
    ids.sort_by(|a, b| (a.len(), a).cmp(&(b.len(), b)));
}

fn qnum(id: &str) -> Option<u64> {
    id.strip_prefix('q')
        .filter(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))?
        .parse()
        .ok()
}

fn is_rid(id: &str) -> bool {
    id.strip_prefix('r')
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
}

/// The work files the loop reads and amends.
struct Work {
    req: Value,
    qs: Value,
    diss: Value,
    ctx: Value,
    fd: Value,
    rules: Vec<serde_json::Value>,
}

impl Work {
    fn open(rules: &str) -> R<Work> {
        let text = fs::read_to_string(rules).map_err(|e| format!("{rules}: {e}"))?;
        let doc: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("{rules}: {e}"))?;
        let mut fd = load("findings.yaml");
        if !fd.get("findings").is_some_and(Value::is_sequence) {
            fd["findings"] = Value::Sequence(vec![]);
        }
        Ok(Work {
            req: load("requirements.yaml"),
            qs: load("questions.yaml"),
            diss: load("dissection.yaml"),
            ctx: load("context.yaml"),
            fd,
            rules: doc["rules"].as_array().cloned().unwrap_or_default(),
        })
    }

    fn findings(&self) -> &[Value] {
        seq(&self.fd, "findings")
    }

    fn findings_mut(&mut self) -> &mut Vec<Value> {
        self.fd["findings"]
            .as_sequence_mut()
            .expect("findings is a sequence")
    }

    fn req(&self, id: &str) -> Option<&Value> {
        seq(&self.req, "requirements")
            .iter()
            .find(|r| st(r, "id") == id)
    }

    fn req_mut(&mut self, id: &str) -> Option<&mut Value> {
        self.req
            .get_mut("requirements")?
            .as_sequence_mut()?
            .iter_mut()
            .find(|r| st(r, "id") == id)
    }

    fn req_ids(&self) -> Vec<String> {
        seq(&self.req, "requirements")
            .iter()
            .map(|r| st(r, "id").to_string())
            .filter(|i| !i.is_empty())
            .collect()
    }

    fn question(&self, id: &str) -> Option<&Value> {
        seq(&self.qs, "questions")
            .iter()
            .find(|q| st(q, "id") == id)
    }

    fn question_ids(&self) -> Vec<String> {
        seq(&self.qs, "questions")
            .iter()
            .map(|q| st(q, "id").to_string())
            .filter(|i| !i.is_empty())
            .collect()
    }

    fn context_field(&self, item: &str) -> Option<&Value> {
        item.strip_prefix("context.")
            .and_then(|k| self.ctx.get(k))
            .filter(|v| v.is_mapping())
    }

    fn ids(&self) -> BTreeSet<String> {
        let mut ids: BTreeSet<String> = self
            .req_ids()
            .into_iter()
            .chain(self.question_ids())
            .collect();
        ids.extend(
            seq(&self.diss, "items")
                .iter()
                .map(|i| st(i, "id").to_string())
                .filter(|i| !i.is_empty()),
        );
        if let Some(m) = self.ctx.as_mapping() {
            ids.extend(
                m.iter()
                    .filter(|(_, v)| v.is_mapping())
                    .filter_map(|(k, _)| k.as_str().map(|k| format!("context.{k}"))),
            );
        }
        ids.extend(self.findings().iter().map(|f| st(f, "id").to_string()));
        ids
    }

    fn text(&self, item: &str) -> String {
        if item.starts_with("context.") {
            return self
                .context_field(item)
                .and_then(|v| v.get("value"))
                .map(scalar)
                .unwrap_or_default();
        }
        self.req(item)
            .map(|r| st(r, "text").to_string())
            .unwrap_or_default()
    }

    /// 2: a principal's own decision (or derived from one); 1: a principal accepted a proposed
    /// default; 0: an agent's. A requirement takes the highest rank of the questions it cites.
    fn authority(&self, item: &str) -> u8 {
        let rank = |status: &str| match status {
            "changed" => 2,
            "accepted" => 1,
            _ => 0,
        };
        if item.starts_with("context.") {
            return self
                .context_field(item)
                .map(|v| rank(st(v, "status")))
                .unwrap_or(0);
        }
        if let Some(q) = self.question(item) {
            return match st(q, "answered_by") {
                "derived" => 2,
                "" | "non-interactive" => 0,
                _ => rank(st(q, "status")),
            };
        }
        if let Some(r) = self.req(item) {
            return strs(r, "cites")
                .iter()
                .filter(|c| self.question(c).is_some())
                .map(|c| self.authority(c))
                .max()
                .unwrap_or(0);
        }
        0
    }

    /// The scope-changing questions behind the cited items.
    fn scope_questions(&self, cites: &[String]) -> Vec<String> {
        let mut out = BTreeSet::new();
        for c in cites {
            let qids = if self.question(c).is_some() {
                vec![c.clone()]
            } else {
                self.req(c).map(|r| strs(r, "cites")).unwrap_or_default()
            };
            out.extend(qids.into_iter().filter(|q| {
                self.question(q).and_then(|q| q.get("scope_changing")) == Some(&Value::Bool(true))
            }));
        }
        out.into_iter().collect()
    }

    fn expand(&self, items: &[serde_json::Value]) -> Vec<String> {
        let mut out = Vec::new();
        for i in items.iter().filter_map(|i| i.as_str()) {
            if i == "requirements" {
                let mut ids = self.req_ids();
                by_len_then_id(&mut ids);
                out.extend(ids);
            } else {
                out.push(i.to_string());
            }
        }
        out
    }

    fn rule(&self, id: &str) -> Option<&serde_json::Value> {
        self.rules.iter().find(|r| r["id"] == id)
    }

    fn rule_hits(&self) -> R<Vec<Value>> {
        let mut hits = Vec::new();
        for rule in &self.rules {
            let (pa, pb) = (
                re(rule["a"]["pattern"].as_str().unwrap_or(""))?,
                re(rule["b"]["pattern"].as_str().unwrap_or(""))?,
            );
            let items = |side: &str| rule[side]["items"].as_array().cloned().unwrap_or_default();
            // `unless`: the rule does not fire while one of these items states what makes both
            // sides hold (offline-hosted: the client keeps a local copy and works offline).
            if let Some(p) = rule["unless"]["pattern"].as_str() {
                let pu = re(p)?;
                if self.expand(&items("unless")).iter().any(|i| {
                    let t = self.text(i);
                    !t.is_empty() && pu.is_match(&t)
                }) {
                    continue;
                }
            }
            let side_a: Vec<String> = self
                .expand(&items("a"))
                .into_iter()
                .filter(|a| {
                    let t = self.text(a);
                    !t.is_empty() && pa.is_match(&t)
                })
                .collect();
            for b in self.expand(&items("b")) {
                let tb = self.text(&b);
                if tb.is_empty() || !pb.is_match(&tb) {
                    continue;
                }
                let a: Vec<String> = side_a.iter().filter(|x| **x != b).cloned().collect();
                if a.is_empty() {
                    continue;
                }
                let id = rule["id"].as_str().unwrap_or("");
                let mut cites = a.clone();
                cites.push(b.clone());
                let mut h = Mapping::new();
                h.insert(s("id"), s(&format!("p-{id}-{b}")));
                h.insert(
                    s("type"),
                    s(rule["type"].as_str().unwrap_or("contradiction")),
                );
                h.insert(s("raised_by"), s(&format!("program:{id}")));
                h.insert(s("cites"), sv(&cites));
                h.insert(
                    s("says"),
                    s(&format!(
                        "{} against {b}: {}",
                        a.join(", "),
                        rule["says"].as_str().unwrap_or("")
                    )),
                );
                h.insert(
                    s("options"),
                    sv(&[
                        rule["options"]["a"].as_str().unwrap_or("").into(),
                        rule["options"]["b"].as_str().unwrap_or("").into(),
                    ]),
                );
                h.insert(s("rule"), s(id));
                h.insert(
                    s("sides"),
                    Value::Sequence(vec![sv(&a), sv(std::slice::from_ref(&b))]),
                );
                hits.push(Value::Mapping(h));
            }
        }
        Ok(hits)
    }
}

fn scalar(v: &Value) -> String {
    match v {
        Value::String(x) => x.clone(),
        Value::Null => String::new(),
        other => serde_yaml::to_string(other)
            .unwrap_or_default()
            .trim()
            .to_string(),
    }
}

fn blocking(w: &Work) -> R<Vec<String>> {
    let status = |id: &str| {
        w.findings()
            .iter()
            .find(|f| st(f, "id") == id)
            .map(|f| st(f, "status").to_string())
    };
    let mut out: Vec<String> = w
        .findings()
        .iter()
        .filter(|f| is_open(st(f, "status")))
        .map(|f| format!("{} ({}): {}", st(f, "id"), st(f, "status"), st(f, "says")))
        .collect();
    for h in w.rule_hits()? {
        if matches!(status(st(&h, "id")).as_deref(), None | Some("resolved")) {
            out.push(format!("{} (rule hit): {}", st(&h, "id"), st(&h, "says")));
        }
    }
    for fid in strs(&w.req, "open_findings") {
        let closed = matches!(
            status(&fid).as_deref(),
            Some("resolved" | "dismissed" | "rejected")
        );
        if !closed && !out.iter().any(|o| o.starts_with(&format!("{fid} "))) {
            out.push(format!("{fid} (held in requirements.yaml open_findings)"));
        }
    }
    Ok(out)
}

/// Why a raised finding is not well-formed, or None.
fn check_finding(f: &Value, ids: &BTreeSet<String>) -> Option<String> {
    let t = st(f, "type");
    if !TYPES.contains(&t) {
        return Some(format!("type {t:?} is not one of {}", TYPES.join(", ")));
    }
    let Some(cites) = f
        .get("cites")
        .and_then(Value::as_sequence)
        .filter(|c| !c.is_empty())
    else {
        return Some("cites no item".into());
    };
    let cites: Vec<String> = cites.iter().map(scalar).collect();
    let bad: Vec<&String> = cites.iter().filter(|c| !ids.contains(*c)).collect();
    if !bad.is_empty() {
        return Some(format!(
            "cites items that do not exist: {}",
            bad.iter()
                .map(|b| b.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if t == "contradiction" && cites.iter().collect::<BTreeSet<_>>().len() < 2 {
        return Some("a contradiction cites at least two items".into());
    }
    if !cites
        .iter()
        .any(|c| c.starts_with('r') || c.starts_with('q') || c.starts_with("context."))
    {
        return Some(
            "cites no requirement, question or context field (nothing upstream to amend)".into(),
        );
    }
    if st(f, "says").trim().is_empty() {
        return Some("says nothing".into());
    }
    let opts = seq(f, "options");
    if !(2..=4).contains(&opts.len()) {
        return Some("needs 2 to 4 options".into());
    }
    if !f.get("default").is_some_and(|d| opts.contains(d)) {
        return Some("its default is not one of its options".into());
    }
    if !f.get("scope_changing").is_some_and(Value::is_bool) {
        return Some("scope_changing is not true or false".into());
    }
    None
}

fn promote(rules: &str, node: &str, path: &str) -> R<u8> {
    let mut w = Work::open(rules)?;
    let raised: Vec<Value> = seq(&load(path), "findings").to_vec();
    let ids = w.ids();
    let mut n = w
        .findings()
        .iter()
        .filter(|f| {
            st(f, "id")
                .strip_prefix('f')
                .is_some_and(|x| !x.is_empty() && x.bytes().all(|c| c.is_ascii_digit()))
        })
        .count();
    let stage = match node {
        "requirements.normalise" => "requirements",
        "context.default" => "context",
        "vision.write" => "vision",
        "roadmap.write" => "roadmap",
        "spec.author" => "spec",
        "plan.decompose" => "plan",
        "change.implement" => "story",
        other => other,
    };
    let keys: BTreeSet<String> = w
        .findings()
        .iter()
        .map(|f| st(f, "key").to_string())
        .filter(|k| !k.is_empty())
        .collect();
    let mut added = 0;
    for f in raised {
        if !f.is_mapping() || (!st(&f, "key").is_empty() && keys.contains(st(&f, "key"))) {
            continue;
        }
        n += 1;
        let id = format!("f{n}");
        let mut e = Mapping::new();
        e.insert(s("id"), s(&id));
        for k in [
            "type",
            "cites",
            "says",
            "options",
            "default",
            "scope_changing",
        ] {
            if let Some(v) = f.get(k) {
                e.insert(s(k), v.clone());
            }
        }
        e.insert(s("stage"), s(stage));
        e.insert(s("raised_by"), s(&format!("llm:{node}")));
        if !st(&f, "key").is_empty() {
            e.insert(s("key"), s(st(&f, "key")));
        }
        match check_finding(&f, &ids) {
            Some(why) => {
                e.insert(s("status"), s("rejected"));
                e.insert(s("basis"), s(&format!("program check: {why}")));
                println!("promote: {id} rejected: {why}");
            }
            None => {
                e.insert(s("status"), s("open"));
                if !w.req.get("open_findings").is_some_and(Value::is_sequence) {
                    w.req["open_findings"] = Value::Sequence(vec![]);
                }
                w.req["open_findings"]
                    .as_sequence_mut()
                    .unwrap()
                    .push(s(&id));
                println!("promote: {id} open: {}", st(&f, "says"));
            }
        }
        w.findings_mut().push(Value::Mapping(e));
        added += 1;
    }
    if added > 0 {
        save("findings.yaml", &w.fd)?;
        if !seq(&w.req, "open_findings").is_empty() {
            save("requirements.yaml", &w.req)?;
        }
    }
    Ok(0)
}

/// (scope, default, basis): who decides, by program rules, never by the raiser's word alone.
fn classify(w: &Work, f: &Value) -> (String, String, String) {
    let opts = strs(f, "options");
    let opt = |i: usize| opts.get(i).cloned().unwrap_or_default();
    let default = st(f, "default").to_string();
    let cites = strs(f, "cites");
    let sides = seq(f, "sides");
    if st(f, "type") == "contradiction" && sides.len() == 2 {
        let side = |i: usize| -> Vec<String> {
            sides[i]
                .as_sequence()
                .map(|x| x.iter().map(scalar).collect())
                .unwrap_or_default()
        };
        let (a, b) = (side(0), side(1));
        let ra = a.iter().map(|x| w.authority(x)).max().unwrap_or(0);
        let rb = b.iter().map(|x| w.authority(x)).max().unwrap_or(0);
        if ra != rb {
            let (win, who) = if ra > rb {
                (0, a.join(", "))
            } else {
                (1, b.join(", "))
            };
            let (hi, lo) = (ra.max(rb), ra.min(rb));
            if hi > 0 {
                return ("agent".into(), opt(win), format!(
                    "derived: {who} is a person's decision of higher rank ({hi} over {lo}), so it stands and the other side follows it"));
            }
            return (
                "agent".into(),
                opt(win),
                "both sides are an agent's defaults".into(),
            );
        }
        if ra > 0 {
            return (
                "person".into(),
                opt(0),
                format!("both sides are a person's decisions of the same rank ({ra})"),
            );
        }
        let sq = w.scope_questions(&cites);
        if !sq.is_empty() {
            return (
                "person".into(),
                opt(0),
                format!(
                    "the cited items rest on scope-changing questions {}",
                    sq.join(", ")
                ),
            );
        }
        return (
            "agent".into(),
            opt(0),
            "both sides are an agent's defaults and no scope-changing question is behind them"
                .into(),
        );
    }
    if f.get("scope_changing") == Some(&Value::Bool(true)) {
        return (
            "person".into(),
            default,
            "the raiser marks it scope-changing".into(),
        );
    }
    if st(f, "type") == "contradiction" {
        let people: Vec<String> = cites
            .iter()
            .filter(|c| w.authority(c) > 0)
            .cloned()
            .collect();
        if !people.is_empty() {
            return (
                "person".into(),
                default,
                format!(
                    "it would overturn a person's decision: {}",
                    people.join(", ")
                ),
            );
        }
    }
    (
        "agent".into(),
        default,
        "changes no scope: it adds a decision and overturns no person's decision".into(),
    )
}

fn next_qid(w: &Work) -> String {
    format!(
        "q{}",
        w.question_ids()
            .iter()
            .filter_map(|q| qnum(q))
            .max()
            .unwrap_or(0)
            + 1
    )
}

/// Amend questions and requirements for one decided finding; returns the ids it amended.
fn amend_one(
    w: &mut Work,
    f: &Value,
    decision: &str,
    decided_by: &str,
    basis: &str,
) -> R<Vec<String>> {
    let qid = next_qid(w);
    let fid = st(f, "id").to_string();
    let reason = format!(
        "finding {fid} ({}, raised by {}): {basis}",
        st(f, "type"),
        st(f, "raised_by")
    );
    let answered_by = if is_person(decided_by) || is_decider(decided_by) {
        decided_by
    } else if basis.starts_with("derived") {
        "derived"
    } else {
        "non-interactive"
    };
    let mut q = Mapping::new();
    q.insert(s("id"), s(&qid));
    q.insert(s("item"), s(&fid));
    q.insert(s("question"), f.get("says").cloned().unwrap_or(Value::Null));
    q.insert(
        s("options"),
        f.get("options").cloned().unwrap_or(Value::Null),
    );
    q.insert(
        s("default"),
        f.get("default").cloned().unwrap_or(Value::Null),
    );
    q.insert(
        s("scope_changing"),
        Value::Bool(is_person(decided_by) || is_decider(decided_by)),
    );
    q.insert(
        s("status"),
        s(if decision == st(f, "default") {
            "accepted"
        } else {
            "changed"
        }),
    );
    q.insert(s("answer"), s(decision));
    q.insert(s("answered_by"), s(answered_by));
    q.insert(s("reason"), s(&reason));
    if !w.qs.get("questions").is_some_and(Value::is_sequence) {
        w.qs["questions"] = Value::Sequence(vec![]);
    }
    w.qs["questions"]
        .as_sequence_mut()
        .unwrap()
        .push(Value::Mapping(q));
    let mut amended = vec![qid.clone()];
    let opts = strs(f, "options");
    let mut lose: Vec<String> = vec![];
    if let Some(rule) = w.rule(st(f, "rule")).cloned() {
        if opts.iter().any(|o| o == decision) {
            let lose_side = if opts.first().map(String::as_str) == Some(decision) {
                "b"
            } else {
                "a"
            };
            let idx = if lose_side == "a" { 0 } else { 1 };
            lose = seq(f, "sides")
                .get(idx)
                .and_then(Value::as_sequence)
                .map(|x| x.iter().map(scalar).collect())
                .unwrap_or_default();
            let pat = re(rule[lose_side]["pattern"].as_str().unwrap_or(""))?;
            let rep = rule["replace"][lose_side]
                .as_str()
                .unwrap_or("")
                .to_string();
            for x in &lose {
                if let Some(k) = x.strip_prefix("context.") {
                    if let Some(field) = w.ctx.get_mut(k).filter(|v| v.is_mapping()) {
                        field["value"] = s(&rep);
                        field["status"] = s("default");
                        field["reason"] = s(&format!("amended by {fid}: {decision}"));
                        amended.push(x.clone());
                    }
                } else if let Some(r) = w.req_mut(x) {
                    let t = pat.replace_all(st(r, "text"), NoExpand(&rep)).into_owned();
                    r["text"] = s(&t);
                }
            }
        }
    }
    let cites = strs(f, "cites");
    for c in &cites {
        let Some(r) = w.req_mut(c) else { continue };
        if !r.get("cites").is_some_and(Value::is_sequence) {
            r["cites"] = Value::Sequence(vec![]);
        }
        if !strs(r, "cites").contains(&qid) {
            r["cites"].as_sequence_mut().unwrap().push(s(&qid));
        }
        if !lose.contains(c) {
            let t = format!("{} Decision {qid}: {decision}.", st(r, "text").trim_end());
            r["text"] = s(&t);
        }
        amended.push(c.clone());
    }
    if !cites.iter().any(|c| w.req(c).is_some()) {
        // a finding about context fields or questions only: the decision still enters the requirements
        let mut ids = w.req_ids();
        by_len_then_id(&mut ids);
        if let Some(target) = ids.first().cloned() {
            let r = w.req_mut(&target).unwrap();
            if !r.get("cites").is_some_and(Value::is_sequence) {
                r["cites"] = Value::Sequence(vec![]);
            }
            r["cites"].as_sequence_mut().unwrap().push(s(&qid));
            let t = format!("{} Decision {qid}: {decision}.", st(r, "text").trim_end());
            r["text"] = s(&t);
            amended.push(target);
        }
    }
    Ok(amended)
}

/// Program check of an amendment: why it fails, or None.
#[allow(clippy::too_many_arguments)]
fn verify(
    before: &Value,
    ctx_before: &Value,
    hits_before: &BTreeSet<String>,
    w: &Work,
    f: &Value,
    decision: &str,
    new_q: &str,
    agent: bool,
) -> R<Option<String>> {
    let mut allowed: BTreeSet<String> = seq(&w.diss, "items")
        .iter()
        .filter(|i| matches!(st(i, "type"), "fact" | "wish"))
        .map(|i| st(i, "id").to_string())
        .collect();
    allowed.extend(w.question_ids());
    for r in seq(&w.req, "requirements") {
        let cites = strs(r, "cites");
        if cites.is_empty() || cites.iter().any(|c| !allowed.contains(c)) {
            return Ok(Some(format!(
                "{} cites an item that is neither a fact, a wish nor a question",
                st(r, "id")
            )));
        }
    }
    for q in seq(&w.qs, "questions") {
        let status = st(q, "status");
        if !matches!(status, "accepted" | "changed" | "open") {
            return Ok(Some(format!("{} is not answered", st(q, "id"))));
        }
        if q.get("scope_changing") == Some(&Value::Bool(true))
            && status != "open"
            && matches!(st(q, "answered_by"), "" | "non-interactive")
        {
            return Ok(Some(format!(
                "{} is scope-changing and names no principal that answered it",
                st(q, "id")
            )));
        }
    }
    let domains = |v: &Value| -> BTreeSet<String> {
        seq(v, "requirements")
            .iter()
            .flat_map(|r| strs(r, "spec_domains"))
            .collect()
    };
    if seq(before, "requirements").len() != seq(&w.req, "requirements").len()
        || domains(before) != domains(&w.req)
    {
        return Ok(Some("the amendment adds or removes a requirement or a specification domain: that changes scope".into()));
    }
    let hits = w.rule_hits()?;
    if !st(f, "rule").is_empty() && hits.iter().any(|h| st(h, "id") == st(f, "id")) {
        return Ok(Some(
            "the consistency rule still fires after the amendment".into(),
        ));
    }
    if !agent {
        return Ok(None); // a person's decision stands; the checks below are for the agent's amendments
    }
    if let Some(h) = hits.iter().find(|h| !hits_before.contains(st(h, "id"))) {
        return Ok(Some(format!(
            "the amendment makes another rule fire: {}",
            st(h, "says")
        )));
    }
    // A rule decision removes the losing side's content. A decision that a requirement outside
    // the losing side cites and that states that content is contradicted by the amendment (trial
    // 2: r4 cited q8 "a hosted web server" while context.deployment became "on the user's own
    // machine"): the two sides may both hold, and that is a person's call.
    if let Some(rule) = w.rule(st(f, "rule")) {
        let opts = strs(f, "options");
        if opts.iter().any(|o| o == decision) {
            let (lose_side, idx) = if opts.first().map(String::as_str) == Some(decision) {
                ("b", 1)
            } else {
                ("a", 0)
            };
            let lose: Vec<String> = seq(f, "sides")
                .get(idx)
                .and_then(Value::as_sequence)
                .map(|x| x.iter().map(scalar).collect())
                .unwrap_or_default();
            let win: Vec<String> = seq(f, "sides")
                .get(1 - idx)
                .and_then(Value::as_sequence)
                .map(|x| x.iter().map(scalar).collect())
                .unwrap_or_default();
            let pat = re(rule[lose_side]["pattern"].as_str().unwrap_or(""))?;
            let bw = Work {
                req: before.clone(),
                qs: Value::Null,
                diss: Value::Null,
                ctx: ctx_before.clone(),
                fd: Value::Null,
                rules: vec![],
            };
            let side = |items: &[String]| {
                items
                    .iter()
                    .map(|i| format!("{i} \"{}\"", bw.text(i)))
                    .collect::<Vec<_>>()
                    .join("; ")
            };
            for r in seq(&w.req, "requirements")
                .iter()
                .filter(|r| !lose.iter().any(|l| l == st(r, "id")))
            {
                for c in strs(r, "cites").iter().filter(|c| c.as_str() != new_q) {
                    let Some(q) = w.question(c) else { continue };
                    let answer = scalar(q.get("answer").unwrap_or(&Value::Null));
                    if matches!(st(q, "status"), "accepted" | "changed")
                        && !answer.is_empty()
                        && pat.is_match(&answer)
                    {
                        return Ok(Some(format!(
                            "decision {c} (\"{answer}\", answered by {}, cited by {}) states what the amendment removes from {}; both sides: {} | {}",
                            st(q, "answered_by"), st(r, "id"), lose.join(", "), side(&win), side(&lose))));
                    }
                }
            }
        }
    }
    Ok(None)
}

fn mark_derived(stage: &str, amended: &[String]) -> R<Option<String>> {
    if let Some((_, path)) = DERIVED.iter().find(|(st, _)| *st == stage) {
        if Path::new(path).is_dir() {
            let line = format!(
                "requirements {} amended by {}: re-derive\n",
                sha16("requirements.yaml"),
                amended.join(" ")
            );
            fs::write(format!("{path}/.derived-from"), line).map_err(|e| e.to_string())?;
            return Ok(Some(format!(
                "{path}/ marked as derived from an earlier requirements revision"
            )));
        }
    }
    if stage == "context" && Path::new(VISION).exists() {
        let text = fs::read_to_string(VISION).map_err(|e| e.to_string())?;
        let cites_amended = amended.iter().filter(|a| is_rid(a)).any(|a| {
            Regex::new(&format!(r"\b{}\b", regex::escape(a))).is_ok_and(|r| r.is_match(&text))
        });
        let cites_any = Regex::new(r"\br\d+\b").unwrap().is_match(&text);
        if cites_amended || !cites_any {
            fs::create_dir_all(".sdc/superseded").map_err(|e| e.to_string())?;
            let n = fs::read_dir(".sdc/superseded")
                .map(|d| d.count())
                .unwrap_or(0)
                + 1;
            let to = format!(".sdc/superseded/vision-{n}.md");
            fs::rename(VISION, &to).map_err(|e| e.to_string())?;
            return Ok(Some(format!(
                "{VISION} set aside to {to} (it cites an amended requirement)"
            )));
        }
    }
    Ok(None)
}

fn amend(rules: &str, stage: &str) -> R<u8> {
    let mut w = Work::open(rules)?;
    let mut changed = false;
    for h in w.rule_hits()? {
        let id = st(&h, "id").to_string();
        match w.findings_mut().iter_mut().find(|f| st(f, "id") == id) {
            None => {
                let mut e = h.as_mapping().cloned().unwrap();
                e.insert(s("stage"), s(stage));
                e.insert(s("status"), s("open"));
                w.findings_mut().push(Value::Mapping(e));
                changed = true;
            }
            Some(f) if st(f, "status") == "resolved" => {
                let attempts = f.get("attempts").and_then(Value::as_u64).unwrap_or(0) + 1;
                f["status"] = s("open");
                f["attempts"] = Value::from(attempts);
                changed = true;
            }
            Some(_) => {}
        }
    }
    let mut amended_all: Vec<String> = vec![];
    for i in 0..w.findings().len() {
        let f = w.findings()[i].clone();
        let status = st(&f, "status").to_string();
        if !is_open(&status) {
            continue;
        }
        let fid = st(&f, "id").to_string();
        let has_decision = !scalar(f.get("decision").unwrap_or(&Value::Null)).is_empty();
        if status == "needs_person" && !has_decision {
            println!("amend: {fid} waits for a person's decision");
            continue;
        }
        let (decision, decided_by, basis);
        if status == "needs_person" {
            decision = scalar(&f["decision"]);
            // The decision carries the principal that took it: `decider:<kind>` (scripts/run-trial.sh)
            // or `person:<id>` (a person's edit names them). Without one nobody is recorded as
            // having decided: the decision is not applied and the finding keeps waiting.
            let by = st(&f, "decided_by");
            if !is_decider(by) && !is_person(by) {
                println!("amend: {fid} has a decision but no principal (decided_by: {by:?}): set `decided_by: person:<your id>` beside `decision:`, then run again");
                continue;
            }
            decided_by = by.to_string();
            basis = format!("{by} decided");
        } else {
            let (scope, default, why) = classify(&w, &f);
            {
                let e = &mut w.findings_mut()[i];
                e["scope"] = s(&scope);
                e["basis"] = s(&why);
                if f.get("sides").is_some() {
                    e["default"] = s(&default);
                }
            }
            let attempts = f.get("attempts").and_then(Value::as_u64).unwrap_or(0);
            if scope == "person" || attempts >= 2 {
                w.findings_mut()[i]["status"] = s("needs_person");
                changed = true;
                println!("amend: {fid} needs a person: {why}");
                continue;
            }
            decision = default;
            decided_by = "agent".to_string();
            basis = why;
        }
        let f = w.findings()[i].clone();
        let before = w.req.clone();
        let snap = (w.req.clone(), w.qs.clone(), w.ctx.clone());
        let hits_before: BTreeSet<String> = w
            .rule_hits()?
            .iter()
            .map(|h| st(h, "id").to_string())
            .collect();
        let amended = amend_one(&mut w, &f, &decision, &decided_by, &basis)?;
        let new_q = amended.first().cloned().unwrap_or_default();
        // A decider's decision goes through the same checks as the agent's amendments: only a
        // person's decision stands unchecked.
        let checked = decided_by == "agent" || is_decider(&decided_by);
        if let Some(why) = verify(
            &before,
            &snap.2,
            &hits_before,
            &w,
            &f,
            &decision,
            &new_q,
            checked,
        )? {
            (w.req, w.qs, w.ctx) = snap;
            let attempts = f.get("attempts").and_then(Value::as_u64).unwrap_or(0) + 1;
            let e = &mut w.findings_mut()[i];
            e["attempts"] = Value::from(attempts);
            e["status"] = s("needs_person");
            if is_decider(&decided_by) {
                // Reverted: the decision is taken off the finding, so it waits for a person, and
                // kept as refused_decision, so the decider is not asked again.
                if let Some(m) = e.as_mapping_mut() {
                    m.remove("decision");
                    m.remove("decided_by");
                }
                let mut rd = Mapping::new();
                rd.insert(s("decision"), s(&decision));
                rd.insert(s("decided_by"), s(&decided_by));
                rd.insert(s("why"), s(&why));
                e["refused_decision"] = Value::Mapping(rd);
                e["basis"] = s(&format!(
                    "program check of {decided_by}'s amendment failed: {why}"
                ));
            } else {
                e["basis"] = s(&format!(
                    "program check of the agent's amendment failed: {why}"
                ));
            }
            println!("amend: {fid} needs a person: {why}");
            changed = true;
            continue;
        }
        let default_taken = decision == st(&f, "default");
        let mut res = Mapping::new();
        res.insert(s("amends"), sv(&amended));
        res.insert(s("reason"), s(&basis));
        res.insert(s("default_taken"), Value::Bool(default_taken));
        let e = &mut w.findings_mut()[i];
        e["status"] = s("resolved");
        e["decision"] = s(&decision);
        e["decided_by"] = s(&decided_by);
        e["resolution"] = Value::Mapping(res);
        println!(
            "amend: {fid} resolved by {decided_by}: {decision} (amends {})",
            amended.join(", ")
        );
        amended_all.extend(amended);
        changed = true;
    }
    let held: Vec<String> = w
        .findings()
        .iter()
        .filter(|f| is_open(st(f, "status")))
        .map(|f| st(f, "id").to_string())
        .collect();
    if strs(&w.req, "open_findings") != held {
        if held.is_empty() {
            if let Some(m) = w.req.as_mapping_mut() {
                m.remove("open_findings");
            }
        } else {
            w.req["open_findings"] = sv(&held);
        }
        changed = true;
    }
    if changed {
        save("findings.yaml", &w.fd)?;
        save("requirements.yaml", &w.req)?;
        if w.qs.as_mapping().is_some_and(|m| !m.is_empty()) {
            save("questions.yaml", &w.qs)?;
        }
        if w.ctx.as_mapping().is_some_and(|m| !m.is_empty())
            && amended_all.iter().any(|a| a.starts_with("context."))
        {
            save("context.yaml", &w.ctx)?;
        }
    }
    if !amended_all.is_empty() {
        if let Some(note) = mark_derived(stage, &amended_all)? {
            println!("amend: {note}");
        }
    }
    if !changed {
        println!("amend: nothing open");
    }
    Ok(0)
}

fn run(args: &[String]) -> R<u8> {
    let (rules, rest) = match args {
        [flag, path, rest @ ..] if flag == "--rules" => (path.clone(), rest),
        rest => (
            std::env::var("SDC_CONSISTENCY_RULES")
                .map_err(|_| "--rules <consistency-rules.json> is required")?,
            rest,
        ),
    };
    match rest {
        [c] if c == "consistency" => {
            let hits = Work::open(&rules)?.rule_hits()?;
            for h in &hits {
                println!("{{\"id\": {}, \"type\": {}, \"cites\": {}, \"says\": {}}}", json!(st(h, "id")), json!(st(h, "type")),
                    serde_json::to_string(&strs(h, "cites")).unwrap_or_default().replace("\",\"", "\", \""), json!(st(h, "says")));
            }
            Ok(if hits.is_empty() { 0 } else { 5 })
        }
        [c, stage] if c == "gate" => {
            let out = blocking(&Work::open(&rules)?)?;
            for line in &out {
                println!("finding gate ({stage}): {line}");
            }
            Ok(if out.is_empty() { 0 } else { 5 })
        }
        [c, node, file] if c == "promote" => promote(&rules, node, file),
        [c, stage] if c == "amend" => amend(&rules, stage),
        _ => Err("usage: sdc-findings --rules <file> consistency | gate <stage> | promote <node> <file> | amend <stage>".into()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("sdc-findings: {e}");
            ExitCode::from(if e.starts_with("usage") { 64 } else { 1 })
        }
    }
}
