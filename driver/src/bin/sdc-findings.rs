//! The late-finding loop's programs (docs/trial-2.md, "The late-finding loop"). Run with cwd = the
//! case tree's `work/`. Deterministic: no clock, no network, no LLM.
//!
//! ```text
//! sdc-findings --rules <consistency-rules.json> consistency
//!     print every consistency-rule hit; exit 0 none, 5 some
//! sdc-findings --rules <…> trace
//!     the requirement trace of spec/ (see `TRACES`): one line per requirement and the counts;
//!     exit 0 every requirement traced or exempt, 5 a gap, 127 no `ess`
//! sdc-findings --rules <…> gate <stage>
//!     exit 5 while a finding is open or a rule hit (at the spec stage also a trace gap) is
//!     neither resolved nor dismissed, else 0 (folded into each stage check)
//! sdc-findings --rules <…> promote <node> <file>
//!     check the findings a stage agent wrote (shape, citations) and add the valid ones to
//!     findings.yaml as open; the invalid ones as rejected, with the reason; one that repeats a
//!     finding not rejected (same type, cited items and text) is not added
//! sdc-findings --rules <…> amend <stage>
//!     resolve the open findings: classify each (agent or person), take the default or the
//!     person's decision, amend questions.yaml and requirements.yaml, verify the amendment, and
//!     mark the stage's derived artifact as derived from earlier requirements
//! sdc-findings --rules <…> check-requirements
//!     the requirements' citation rule (`citation_fault`) and the run's build constraints
//!     (`constraint_fault`, work/build-constraints.yaml, D154), one line per fault; exit 0 none, 1 some
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
//! A contradiction's decision supersedes each cited answer it differs from (D141): the question gets
//! `status: superseded` and `superseded_by`, and a requirement citing it is a citation fault.
//! A finding whose applied decision did not take effect waits with `reraised` {decision,
//! decided_by, says, of}: a resolved program finding that fires again, or an agent's contradiction
//! citing the answer of a finding a person or a decider resolved (`reraised_of`).

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
    /// The run's build constraints (`build-constraints.yaml`, D154); empty when the run has none.
    bc: Value,
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
            bc: load("build-constraints.yaml"),
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

    fn question_mut(&mut self, id: &str) -> Option<&mut Value> {
        self.qs
            .get_mut("questions")?
            .as_sequence_mut()?
            .iter_mut()
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

/// The requirement trace of the spec stage (runs/trial-3-haiku-3: r3 cited the domain
/// `todo.access`, which declared one actor and no command, and passed). The spec writer names, per
/// requirement, the declarations that deliver it in `spec/traces.json`
/// (`{"traces": {"r1": ["todo.task.AddTask", ...]}}`; JSON because `ess` reads every `.yaml` in the
/// tree). Behaviour is what the conformance suite exercises: a command (its outcomes), a view (what
/// is read), or an actor whose `may` names commands (who may act). A domain, an entity, an event, an
/// error, a type or an actor that may take nothing carries no behaviour of its own. A requirement
/// that is not behaviour says so in requirements.yaml: `behaviour: constraint` or `behaviour:
/// dropped` with `behaviour_reason` (set upstream or by a scope decision on a trace finding, never
/// by the spec writer).
const TRACES: &str = "spec/traces.json";

/// What the trace found: the gaps as program findings (`p-trace-<rid>`), one line per requirement,
/// and the counts.
struct Trace {
    hits: Vec<Value>,
    lines: Vec<String>,
    summary: String,
}

fn trace_options(rid: &str) -> Vec<String> {
    vec![
        format!("specify {rid} in the spec: name in spec/traces.json the commands or views that deliver it"),
        format!("{rid} is a constraint, not behaviour"),
        format!("drop {rid} from this epic's scope"),
    ]
}

/// The scope a decision on the trace finding of `rid` takes: `constraint` or `dropped` when it is
/// that option, or starts with it (runs/trial-3-small-sonnet-1: the decider answered "r5 is a
/// constraint, not behaviour: mark r5 as a constraint outside the specification", which matched no
/// option exactly, so nothing was recorded and the gap came back eight times); else empty.
fn trace_kind(rid: &str, decision: &str) -> &'static str {
    if rid.is_empty() {
        return "";
    }
    let opts = trace_options(rid);
    let d = decision.trim();
    let is = |o: &str| d == o || d.strip_prefix(o).is_some_and(|rest| rest.starts_with([':', '.', ';', ',', ' ']));
    if is(&opts[1]) {
        "constraint"
    } else if is(&opts[2]) {
        "dropped"
    } else {
        ""
    }
}

/// A resolved program finding raised again (`h`, the hit as computed now): the decision on it was
/// applied and should have removed it, so this is a defect of that apply, never a new question.
/// The finding waits (needs_person, decision taken off) with `reraised` {decision, decided_by,
/// says}; scripts/run-trial.sh stops the run `finding_reraised` on it.
fn reraise(f: &mut Value, h: &Value) {
    let mut r = Mapping::new();
    r.insert(s("decision"), f.get("decision").cloned().unwrap_or(Value::Null));
    r.insert(s("decided_by"), f.get("decided_by").cloned().unwrap_or(Value::Null));
    r.insert(s("says"), h.get("says").cloned().unwrap_or(Value::Null));
    f["reraised"] = Value::Mapping(r);
    f["status"] = s("needs_person");
    f["basis"] = s(&format!(
        "raised again after the decision \"{}\" ({}) was applied: a defect of that apply",
        scalar(f.get("decision").unwrap_or(&Value::Null)),
        st(f, "decided_by")
    ));
    if let Some(m) = f.as_mapping_mut() {
        for k in ["decision", "decided_by", "resolution"] {
            m.remove(k);
        }
    }
}

/// The earlier finding an agent's contradiction `f` raises again, or None: `f` cites the answer
/// (a question whose `item` is that finding, not superseded) of a finding a person or a decider
/// resolved. runs/trial-3-small-haiku-1: f9 (r5's Python 3 against the builder's Rust) was decided
/// "Python 3"; the apply only added that answer (q17) to r5, which said Python already, and the
/// builder's Rust came from its prompt, which no amendment reaches; f11 then contradicted q17. A
/// contradiction against an applied decision is that decision not taking effect, as `reraise`.
fn reraised_of(w: &Work, f: &Value) -> Option<Value> {
    if st(f, "type") != "contradiction" || !st(f, "raised_by").starts_with("llm:") {
        return None;
    }
    strs(f, "cites").iter().filter_map(|c| w.question(c)).filter(|q| st(q, "status") != "superseded").find_map(|q| {
        w.findings()
            .iter()
            .find(|g| {
                st(g, "id") == st(q, "item")
                    && st(g, "id") != st(f, "id")
                    && st(g, "status") == "resolved"
                    && (is_person(st(g, "decided_by")) || is_decider(st(g, "decided_by")))
            })
            .cloned()
    })
}

/// The trace of every requirement against the compiled spec `ir` and the writer's `traces`
/// document (None: no spec/traces.json; Err: it does not parse). Pure: no file is read.
fn trace_of(req: &Value, ir: &serde_json::Value, traces: Option<Result<serde_json::Value, String>>) -> Trace {
    let names = |k: &str| -> Vec<String> { ir[k].as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default() };
    let (commands, views) = (names("commands"), names("views"));
    let kind_of = |c: &str| -> String {
        if commands.iter().any(|x| x == c) || views.iter().any(|x| x == c) {
            return String::new();
        }
        if let Some(a) = ir["actors"].get(c) {
            return if a["may"].as_array().is_some_and(|m| !m.is_empty()) { String::new() } else { format!("{c}, an actor that may take no command") };
        }
        if let Some(d) = ir["domains"].get(c) {
            let own: Vec<String> = ["commands", "views"].iter()
                .flat_map(|k| d[*k].as_array().cloned().unwrap_or_default())
                .filter_map(|x| x.as_str().map(String::from)).collect();
            return if own.is_empty() { format!("{c}, which declares no command") }
                else { format!("{c}, a domain: name the ones that deliver it ({})", own.join(", ")) };
        }
        for (k, what) in [("entities", "an entity"), ("events", "an event"), ("errors", "an error"), ("types", "a type")] {
            if ir[k].get(c).is_some() {
                return format!("{c}, {what}");
            }
        }
        format!("{c}, which the compiled spec does not declare")
    };
    let (mut hits, mut lines) = (Vec::new(), Vec::new());
    let (mut traced, mut constraint, mut dropped) = (0, Vec::new(), Vec::new());
    let doc = traces.as_ref().map(|t| t.as_ref().map(|d| &d["traces"]));
    for r in seq(req, "requirements") {
        let rid = st(r, "id");
        if rid.is_empty() {
            continue;
        }
        let reason = st(r, "behaviour_reason").trim().to_string();
        let gap = match st(r, "behaviour") {
            b @ ("constraint" | "dropped") if !reason.is_empty() => {
                lines.push(format!("{rid}: {b}: {reason}"));
                if b == "constraint" { constraint.push(rid.to_string()) } else { dropped.push(rid.to_string()) }
                continue;
            }
            b @ ("constraint" | "dropped") => format!("{rid}: behaviour: {b} gives no behaviour_reason"),
            _ => {
                let (cites, source) = match doc {
                    Some(Err(e)) => (vec![], format!("{TRACES} does not parse: {e}")),
                    Some(Ok(t)) if t[rid].is_array() => (
                        t[rid].as_array().unwrap().iter().filter_map(|x| x.as_str().map(String::from)).collect(), String::new()),
                    _ => (strs(r, "spec_domains"), format!("no entry in {TRACES}")),
                };
                let kinds: Vec<String> = cites.iter().map(|c| kind_of(c)).collect();
                let behaviour: Vec<&String> = cites.iter().zip(&kinds).filter(|(_, k)| k.is_empty()).map(|(c, _)| c).collect();
                let unknown: Vec<&String> = kinds.iter().filter(|k| k.ends_with("does not declare")).collect();
                let note = if source.is_empty() { String::new() } else { format!("; {source}") };
                if !unknown.is_empty() {
                    format!("{rid}: cites what the compiled spec does not declare ({}){note}",
                        unknown.iter().map(|k| k.trim_end_matches(", which the compiled spec does not declare")).collect::<Vec<_>>().join(", "))
                } else if behaviour.is_empty() {
                    let what = if cites.is_empty() { "nothing".to_string() } else { kinds.join("; ") };
                    format!("{rid}: traces to no behaviour (cites {what}){note}")
                } else {
                    lines.push(format!("{rid}: traced to {}", behaviour.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(", ")));
                    traced += 1;
                    continue;
                }
            }
        };
        lines.push(gap.clone());
        let opts = trace_options(rid);
        let mut h = Mapping::new();
        h.insert(s("id"), s(&format!("p-trace-{rid}")));
        h.insert(s("type"), s("missing_decision"));
        h.insert(s("raised_by"), s("program:trace"));
        h.insert(s("cites"), sv(&[rid.to_string()]));
        h.insert(s("says"), s(&gap));
        h.insert(s("options"), sv(&opts));
        h.insert(s("default"), s(&opts[0]));
        h.insert(s("scope_changing"), Value::Bool(true));
        h.insert(s("trace"), s(rid));
        hits.push(Value::Mapping(h));
    }
    let list = |v: &[String]| if v.is_empty() { String::new() } else { format!(" ({})", v.join(", ")) };
    let summary = format!("requirement trace: {} requirement(s): {traced} traced to behaviour, {} constraint{}, {} dropped{}, {} untraced",
        traced + constraint.len() + dropped.len() + hits.len(), constraint.len(), list(&constraint), dropped.len(), list(&dropped), hits.len());
    Trace { hits, lines, summary }
}

/// The trace of the spec in `work/`: compiles it with `ess` into `.sdc/trace-ir.json`. None when
/// there is no spec or it does not compile (the stage's own checks report that).
fn trace(w: &Work) -> R<Option<Trace>> {
    if !Path::new("spec/system.yaml").exists() {
        return Ok(None);
    }
    fs::create_dir_all(".sdc").map_err(|e| e.to_string())?;
    let out = std::process::Command::new("ess").args(["specify", "compile", "--path", "spec", "--out", ".sdc/trace-ir.json"]).output();
    let out = match out {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err("program error: ess is not on PATH: the requirement trace could not run".into()),
        Err(e) => return Err(e.to_string()),
    };
    if !out.status.success() {
        return Ok(None);
    }
    let ir: serde_json::Value = serde_json::from_str(&fs::read_to_string(".sdc/trace-ir.json").map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let traces = fs::read_to_string(TRACES).ok().map(|t| serde_json::from_str::<serde_json::Value>(&t).map_err(|e| e.to_string())
        .and_then(|d| if d["traces"].is_object() { Ok(d) } else { Err("no object `traces`".into()) }));
    Ok(Some(trace_of(&w.req, &ir, traces)))
}

fn blocking(w: &Work, stage: &str) -> R<Vec<String>> {
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
    if stage == "spec" {
        if let Some(t) = trace(w)? {
            println!("finding gate ({stage}): {}", t.summary);
            for h in &t.hits {
                if matches!(status(st(h, "id")).as_deref(), None | Some("resolved")) {
                    out.push(format!("{} (trace): {}", st(h, "id"), st(h, "says")));
                }
            }
        }
    }
    // D141: a requirement citing a superseded answer blocks every stage, whoever wrote the cite.
    for r in seq(&w.req, "requirements") {
        if let Some(why) = superseded_cite(w, r) {
            out.push(format!("{} (citation): {why}", st(r, "id")));
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

/// Two findings are the same when type, the set of cited items and the text (case and runs of
/// whitespace ignored) are equal.
fn same_finding(a: &Value, b: &Value) -> bool {
    let cites = |v: &Value| -> BTreeSet<String> { seq(v, "cites").iter().map(scalar).collect() };
    let text = |v: &Value| st(v, "says").split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    st(a, "type") == st(b, "type") && cites(a) == cites(b) && text(a) == text(b)
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
        // The same finding raised again (runs/trial-3-haiku-2: a writer's retry re-raised its own
        // open findings, f10-f13 = f6-f9): not added; the earlier one stands with its decision.
        if let Some(e) = w.findings().iter().find(|e| st(e, "status") != "rejected" && same_finding(e, &f)) {
            println!("promote: not added, the same as {} ({}): {}", st(e, "id"), st(e, "status"), st(&f, "says"));
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
    // A requirement the spec traces to no behaviour: the writer is sent back to specify it once
    // (the agent's decision, which names the gap); if the gap stays, specifying it, calling it a
    // constraint or dropping it is a scope decision. Never a default that lets it pass.
    if f.get("trace").is_some() {
        let rid = st(f, "trace");
        return if f.get("attempts").and_then(Value::as_u64).unwrap_or(0) == 0 {
            ("agent".into(), opt(0), format!("the spec writer is sent back to specify {rid} first"))
        } else {
            ("person".into(), opt(0), format!(
                "the spec still traces {rid} to no behaviour after the writer was sent back: specify it, call it a constraint, or drop it from scope"))
        };
    }
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
    let scope_changing = is_person(decided_by) || is_decider(decided_by);
    q.insert(s("scope_changing"), Value::Bool(scope_changing));
    if scope_changing {
        // A scope-changing question carries the keys questions.check requires of one
        // (story:interview-question-context): the raiser's own, else what the finding itself says.
        let own = |k: &str| f.get(k).filter(|v| v.as_str().is_some_and(|x| !x.trim().is_empty())).cloned();
        q.insert(s("excerpt"), own("excerpt").unwrap_or_else(|| s(st(f, "says"))));
        q.insert(s("default_reason"), own("default_reason").unwrap_or_else(|| s(&format!("the default of finding {fid}: {basis}"))));
        let own_c = f.get("consequences").filter(|c| c.as_sequence().is_some_and(|c| c.len() == strs(f, "options").len())).cloned();
        let c = own_c.unwrap_or_else(|| Value::Sequence(strs(f, "options").iter().map(|o| {
            let mut e = Mapping::new();
            e.insert(s("does"), s(&format!("Takes the option \"{o}\" as the decision.")));
            e.insert(s("cost"), s("The raiser of the finding did not state a cost."));
            Value::Mapping(e)
        }).collect()));
        q.insert(s("consequences"), c);
    }
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
    // D141: a contradiction's decision replaces each cited answer it differs from. That answer is
    // superseded; each requirement citing it drops the cite and the sentence the answer wrote
    // ("Decision qX: <answer>.") and cites the new decision instead (below, `carry`).
    // A finding decided again (a trace gap reopened after its first decision) replaces the earlier
    // answers to it the same way (runs/trial-3-small-sonnet-1: q5 "specify r5 in the spec" stayed
    // beside the decision that r5 is a constraint).
    let mut carry: Vec<String> = vec![];
    let mut replaced: Vec<String> = seq(&w.qs, "questions")
        .iter()
        .filter(|q| st(q, "item") == fid && st(q, "id") != qid)
        .map(|q| st(q, "id").to_string())
        .collect();
    if st(f, "type") == "contradiction" {
        replaced.extend(strs(f, "cites").into_iter().filter(|c| {
            w.question(c).is_some_and(|q| scalar(q.get("answer").unwrap_or(&Value::Null)) != decision)
        }));
    }
    for c in replaced {
        let Some(q) = w.question_mut(&c) else { continue };
        let answer = scalar(q.get("answer").unwrap_or(&Value::Null));
        if !matches!(st(q, "status"), "accepted" | "changed") {
            continue;
        }
        q["status"] = s("superseded");
        q["superseded_by"] = s(&qid);
        amended.push(c.clone());
        let sentence = re(&format!(r"\s*Decision {}: {}\.", regex::escape(&c), regex::escape(&answer)))?;
        for r in w.req.get_mut("requirements").and_then(Value::as_sequence_mut).into_iter().flatten() {
            if !strs(r, "cites").contains(&c) {
                continue;
            }
            if let Some(cs) = r["cites"].as_sequence_mut() {
                cs.retain(|x| scalar(x) != c);
            }
            r["text"] = s(sentence.replace_all(st(r, "text"), "").trim());
            let rid = st(r, "id").to_string();
            if !carry.contains(&rid) {
                carry.push(rid);
            }
        }
    }
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
    let mut cites = strs(f, "cites");
    cites.extend(carry.iter().filter(|r| !cites.contains(r)).cloned().collect::<Vec<_>>());
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

/// The citation rule of a requirement (D136; runs/trial-3-mixed-4): why `r` fails it, or None.
/// Every cite is a fact or wish of the dissection, a question id, or an open item (belief,
/// unknown, ambiguity, dark_spot: the items questions.check gives exactly one question each) that a
/// cited question asks about (its `item`); at least one cite is a fact, a wish or a question, so an
/// open item is never the only ground. A question a later decision replaced (`status:
/// superseded`, D141) is no ground: citing it is a fault.
fn citation_fault(w: &Work, r: &Value) -> Option<String> {
    if let Some(why) = superseded_cite(w, r) {
        return Some(why);
    }
    let cites = strs(r, "cites");
    let kind = |c: &str| seq(&w.diss, "items").iter().find(|i| st(i, "id") == c).map(|i| st(i, "type").to_string());
    let asked: BTreeSet<String> = cites.iter().filter_map(|c| w.question(c)).map(|q| st(q, "item").to_string()).collect();
    let ground = cites.iter().any(|c| w.question(c).is_some() || matches!(kind(c).as_deref(), Some("fact" | "wish")));
    if !ground {
        return Some(format!("{} cites no fact, wish or question (cites: {})", st(r, "id"), cites.join(" ")));
    }
    let open = |t: &str| matches!(t, "belief" | "unknown" | "ambiguity" | "dark_spot");
    for c in cites.iter().filter(|c| w.question(c).is_none()) {
        match kind(c).as_deref() {
            Some("fact" | "wish") => {}
            Some(t) if open(t) && asked.contains(c) => {}
            Some(t) if open(t) => {
                let a = if t.starts_with(['a', 'u']) { "an" } else { "a" };
                return Some(format!("{} cites {c}, {a} {t} that no question it cites asks about: cite the question that answers {c}, or leave {c} out", st(r, "id")));
            }
            Some(t) => return Some(format!("{} cites {c}, a {t}: a requirement cites facts, wishes and questions", st(r, "id"))),
            None => return Some(format!("{} cites {c}, which is no dissection item and no question", st(r, "id"))),
        }
    }
    None
}

/// D141: why `r` cites an answer a later decision superseded, or None.
fn superseded_cite(w: &Work, r: &Value) -> Option<String> {
    let q = strs(r, "cites").iter().filter_map(|c| w.question(c)).find(|q| st(q, "status") == "superseded")?;
    let by = st(q, "superseded_by");
    Some(format!("{} cites {}, an answer superseded by {by}: cite {by} instead", st(r, "id"), st(q, "id")))
}

/// D154: why `r` breaks the run's build constraints (`w.bc`, build-constraints.yaml), or None.
/// Exact, on declared fields only: `language` other than the constraints' `language` (equal, or it
/// followed by a space and more, ignoring case), a `dependencies` entry not in the constraints'
/// `dependencies`, and a requirement declaring either without `behaviour: constraint` (it restates
/// a constraint). No constraints file: no fault (a run started before D154).
fn constraint_fault(w: &Work, r: &Value) -> Option<String> {
    let lang = st(&w.bc, "language");
    if lang.is_empty() {
        return None;
    }
    let id = st(r, "id");
    let named = scalar(r.get("language").unwrap_or(&Value::Null));
    let n = named.trim().to_lowercase();
    let l = lang.to_lowercase();
    if !n.is_empty() && n != l && !n.starts_with(&format!("{l} ")) {
        return Some(format!("{id} names the language {named}; the build constraints of this run (build-constraints.yaml) fix it to {lang}: write the requirement in {lang} with `language: {lang}` and `behaviour: constraint`, or leave the language out"));
    }
    let allowed = strs(&w.bc, "dependencies");
    let deps: Vec<String> = match r.get("dependencies") {
        Some(Value::Sequence(xs)) => xs.iter().map(scalar).collect(),
        Some(v) if !scalar(v).is_empty() => vec![scalar(v)],
        _ => vec![],
    };
    let extra: Vec<String> = deps
        .iter()
        .filter(|d| !allowed.iter().any(|a| a.eq_ignore_ascii_case(d.trim())))
        .cloned()
        .collect();
    if !extra.is_empty() {
        let ok = if allowed.is_empty() { "none".to_string() } else { allowed.join(", ") };
        return Some(format!("{id} names the dependencies {}; the build constraints of this run (build-constraints.yaml) allow {ok}: build it with what they allow, or leave the requirement out", extra.join(", ")));
    }
    let declares = !n.is_empty() || !deps.is_empty();
    if declares && st(r, "behaviour") != "constraint" {
        return Some(format!("{id} restates a build constraint (`language` or `dependencies`): mark it `behaviour: constraint` with a `behaviour_reason`"));
    }
    None
}

/// `sdc-findings check-requirements`: the requirements' shape at write time (requirements.check):
/// at least one requirement, each with an id, the citation rule (`citation_fault`) and the build
/// constraints (`constraint_fault`, D154). One line per fault; exit 0 none, 1 some.
fn check_requirements(w: &Work) -> u8 {
    let reqs = seq(&w.req, "requirements");
    let mut faults: Vec<String> = reqs.iter().filter_map(|r| citation_fault(w, r)).collect();
    faults.extend(reqs.iter().filter_map(|r| constraint_fault(w, r)));
    if reqs.is_empty() {
        faults.push("requirements.yaml has no requirement".into());
    }
    faults.extend(reqs.iter().filter(|r| st(r, "id").is_empty()).map(|_| "a requirement has no id".to_string()));
    for f in &faults {
        println!("requirements: {f}");
    }
    u8::from(!faults.is_empty())
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
    // Only what the decision wrote can make it untakeable (D137, runs/trial-3-mixed-4): a citation
    // fault the requirement had before the amendment is the writer's, sent back by
    // requirements.check; and the only question the amendment writes is its own. A cite of an
    // answer this decision superseded (D141) is the decision's own fault, whatever came before.
    for r in seq(&w.req, "requirements") {
        if let Some(c) = strs(r, "cites").into_iter().find(|c| {
            w.question(c).is_some_and(|q| st(q, "status") == "superseded" && st(q, "superseded_by") == new_q)
        }) {
            return Ok(Some(format!("{} cites {c}, an answer superseded by {new_q}", st(r, "id"))));
        }
    }
    for r in seq(&w.req, "requirements") {
        if let Some(why) = citation_fault(w, r).filter(|_| seq(before, "requirements").iter()
            .find(|b| st(b, "id") == st(r, "id")).is_none_or(|b| citation_fault(w, b).is_none())) {
            return Ok(Some(why));
        }
    }
    for q in seq(&w.qs, "questions").iter().filter(|q| st(q, "id") == new_q) {
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
                bc: Value::Null,
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
            // Appended: the writer's own first line (`requirements <digest> resolved <ids>`, the
            // findings resolved when it wrote the tree) stays, so it can tell which decisions are new.
            let line = format!("requirements {} amended by {}: re-derive\n", sha16("requirements.yaml"), amended.join(" "));
            let file = format!("{path}/.derived-from");
            let before = fs::read_to_string(&file).unwrap_or_default();
            fs::write(&file, before + &line).map_err(|e| e.to_string())?;
            return Ok(Some(format!("{path}/ marked as derived from an earlier requirements revision")));
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
                // The amendment that resolved it made the rule stop firing (verify); firing again
                // is a defect of an apply, never a new question.
                reraise(f, &h);
                changed = true;
            }
            Some(_) => {}
        }
    }
    // The requirement trace's gaps, as the rule hits: a new gap is opened, a resolved one that
    // still holds is reopened (attempts + 1), with its current text, when the agent resolved it
    // (the writer sent back); one a person or a decider resolved is re-raised (`reraise`).
    let trace_hits = if stage == "spec" { trace(&w)?.map(|t| t.hits).unwrap_or_default() } else { vec![] };
    for h in trace_hits {
        let id = st(&h, "id").to_string();
        match w.findings_mut().iter_mut().find(|f| st(f, "id") == id) {
            None => {
                let mut e = h.as_mapping().cloned().unwrap();
                e.insert(s("stage"), s(stage));
                e.insert(s("status"), s("open"));
                w.findings_mut().push(Value::Mapping(e));
                changed = true;
            }
            // The agent's decision sends the writer back once (`classify`); any later decision was
            // a person's or a decider's, and the gap coming back means it did not take effect.
            Some(f) if st(f, "status") == "resolved" && st(f, "decided_by") != "agent" => {
                reraise(f, &h);
                changed = true;
            }
            Some(f) if st(f, "status") == "resolved" => {
                let attempts = f.get("attempts").and_then(Value::as_u64).unwrap_or(0) + 1;
                f["status"] = s("open");
                f["attempts"] = Value::from(attempts);
                f["says"] = h["says"].clone();
                for k in ["decision", "decided_by", "resolution"] {
                    if let Some(m) = f.as_mapping_mut() {
                        m.remove(k);
                    }
                }
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
        } else if let Some(g) = reraised_of(&w, &f) {
            let mut e = f.clone();
            e["decision"] = g.get("decision").cloned().unwrap_or(Value::Null);
            e["decided_by"] = g.get("decided_by").cloned().unwrap_or(Value::Null);
            let h = e.clone();
            reraise(&mut e, &h);
            e["reraised"]["of"] = s(st(&g, "id"));
            println!("amend: {fid} needs a person: it raises {} again: {}", st(&g, "id"), st(&e, "basis"));
            w.findings_mut()[i] = e;
            changed = true;
            continue;
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
        // A scope decision on a trace finding that the requirement is a constraint, or dropped:
        // recorded on the requirement, where the trace reads it.
        if f.get("trace").is_some() {
            let rid = st(&f, "trace").to_string();
            let kind = trace_kind(&rid, &decision);
            if !kind.is_empty() {
                if let Some(r) = w.req_mut(&rid) {
                    r["behaviour"] = s(kind);
                    r["behaviour_reason"] = s(&format!("{decision} (finding {fid}, decided by {decided_by})"));
                }
            }
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
        [c] if c == "trace" => {
            let w = Work::open(&rules)?;
            let Some(t) = trace(&w)? else { return Err("no spec/system.yaml, or the spec does not compile".into()) };
            for l in &t.lines {
                println!("{l}");
            }
            println!("{}", t.summary);
            Ok(if t.hits.is_empty() { 0 } else { 5 })
        }
        [c, stage] if c == "gate" => {
            let out = blocking(&Work::open(&rules)?, stage)?;
            for line in &out {
                println!("finding gate ({stage}): {line}");
            }
            Ok(if out.is_empty() { 0 } else { 5 })
        }
        [c, node, file] if c == "promote" => promote(&rules, node, file),
        [c, stage] if c == "amend" => amend(&rules, stage),
        [c] if c == "check-requirements" => Ok(check_requirements(&Work::open(&rules)?)),
        _ => Err("usage: sdc-findings --rules <file> consistency | trace | gate <stage> | promote <node> <file> | amend <stage> | check-requirements".into()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("sdc-findings: {e}");
            ExitCode::from(if e.starts_with("usage") { 64 } else if e.starts_with("program error") { 127 } else { 1 })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work() -> Work {
        let qs = "questions:\n- id: q1\n  item: d3\n  question: Which states?\n  default: a\n  scope_changing: false\n  status: accepted\n  answer: a\n";
        Work {
            req: serde_yaml::from_str("requirements:\n- id: r1\n  text: Bill customers.\n  cites: [d1]\n").unwrap(),
            qs: serde_yaml::from_str(qs).unwrap(),
            diss: Value::Mapping(Mapping::new()),
            ctx: Value::Mapping(Mapping::new()),
            fd: Value::Mapping(Mapping::new()),
            bc: Value::Mapping(Mapping::new()),
            rules: vec![],
        }
    }

    fn last_question(w: &Work) -> Value {
        w.qs["questions"].as_sequence().unwrap().last().unwrap().clone()
    }

    /// A question a finding's decision adds is scope-changing when a person or a decider decided it,
    /// so it carries what `questions.check` requires of such a question
    /// (`story:interview-question-context`).
    fn assert_context_keys(decided_by: &str) {
        let f: Value = serde_yaml::from_str(
            "id: f1\ntype: ambiguity\nraised_by: llm\ncites: [r1]\nsays: r1 names the clerk only\noptions: [only the clerk, the customer too]\ndefault: only the clerk\nscope_changing: true\n",
        )
        .unwrap();
        let mut w = work();
        amend_one(&mut w, &f, "the customer too", decided_by, "decided").unwrap();
        let q = last_question(&w);
        assert_eq!(q["scope_changing"], Value::Bool(true));
        assert!(q["excerpt"].as_str().is_some_and(|x| !x.trim().is_empty()), "excerpt: {q:?}");
        assert!(q["default_reason"].as_str().is_some_and(|x| !x.trim().is_empty()), "default_reason: {q:?}");
        let c = q["consequences"].as_sequence().expect("consequences");
        assert_eq!(c.len(), 2);
        for e in c {
            assert!(e["does"].as_str().is_some_and(|x| !x.trim().is_empty()), "does: {e:?}");
            assert!(e["cost"].as_str().is_some_and(|x| !x.trim().is_empty()), "cost: {e:?}");
        }
    }

    #[test]
    fn a_question_added_from_a_person_decided_finding_carries_the_context_keys() {
        assert_context_keys("person:operator");
    }

    #[test]
    fn a_question_added_from_a_decider_decided_finding_carries_the_context_keys() {
        assert_context_keys("decider:claude");
    }

    /// runs/trial-3-haiku-3's spec shape: an access domain with one actor and no command, a task
    /// domain with commands and a view.
    fn trace_ir() -> serde_json::Value {
        json!({
            "commands": {"todo.task.AddTask": {}, "todo.sync.SyncChanges": {}},
            "views": {"todo.task.SharedList": {}},
            "actors": {"todo.access.Member": {"may": []}, "todo.task.Owner": {"may": ["todo.task.AddTask"]}},
            "entities": {"todo.task.Task": {}},
            "domains": {
                "todo.access": {"commands": [], "views": [], "actors": ["todo.access.Member"]},
                "todo.task": {"commands": ["todo.task.AddTask"], "views": ["todo.task.SharedList"]}}})
    }

    fn trace_req(extra: &str) -> Value {
        serde_yaml::from_str(&format!("requirements:\n- id: r1\n  spec_domains: [todo.task]\n- id: r3\n  spec_domains: [todo.access]\n{extra}")).unwrap()
    }

    #[test]
    fn a_requirement_citing_an_empty_domain_traces_to_no_behaviour() {
        let t = trace_of(&trace_req(""), &trace_ir(), Some(Ok(json!({"traces": {"r1": ["todo.task.AddTask"], "r3": ["todo.access"]}}))));
        assert_eq!(t.lines[1], "r3: traces to no behaviour (cites todo.access, which declares no command)");
        assert_eq!(t.hits.len(), 1);
        assert_eq!(t.hits[0]["id"].as_str(), Some("p-trace-r3"));
        assert_eq!(t.lines[0], "r1: traced to todo.task.AddTask");
    }

    #[test]
    fn domains_entities_and_actors_without_may_carry_no_behaviour() {
        let traces = json!({"traces": {"r1": ["todo.task", "todo.task.Task"], "r3": ["todo.access.Member"]}});
        let t = trace_of(&trace_req(""), &trace_ir(), Some(Ok(traces)));
        assert!(t.lines[0].starts_with("r1: traces to no behaviour (cites todo.task, a domain: name the ones that deliver it (todo.task.AddTask, todo.task.SharedList); todo.task.Task, an entity)"), "{}", t.lines[0]);
        assert_eq!(t.lines[1], "r3: traces to no behaviour (cites todo.access.Member, an actor that may take no command)");
        let t = trace_of(&trace_req(""), &trace_ir(), Some(Ok(json!({"traces": {"r1": ["todo.task.Owner"], "r3": ["todo.task.SharedList"]}}))));
        assert!(t.hits.is_empty(), "{:?}", t.lines);
    }

    #[test]
    fn an_undeclared_citation_and_a_missing_traces_file_are_gaps() {
        let t = trace_of(&trace_req(""), &trace_ir(), Some(Ok(json!({"traces": {"r1": ["todo.task.AddTask", "todo.access.SignIn"]}}))));
        assert_eq!(t.lines[0], "r1: cites what the compiled spec does not declare (todo.access.SignIn)");
        assert!(t.lines[1].ends_with("; no entry in spec/traces.json"), "{}", t.lines[1]);
        let t = trace_of(&trace_req(""), &trace_ir(), None);
        assert_eq!(t.hits.len(), 2);
    }

    #[test]
    fn a_constraint_with_its_reason_passes_and_is_counted() {
        let t = trace_of(&trace_req("- id: r6\n  behaviour: constraint\n  behaviour_reason: the licence, no behaviour\n"), &trace_ir(),
            Some(Ok(json!({"traces": {"r1": ["todo.task.AddTask"], "r3": ["todo.sync.SyncChanges"]}}))));
        assert!(t.hits.is_empty(), "{:?}", t.lines);
        assert_eq!(t.summary, "requirement trace: 3 requirement(s): 2 traced to behaviour, 1 constraint (r6), 0 dropped, 0 untraced");
        let t = trace_of(&trace_req("- id: r6\n  behaviour: constraint\n"), &trace_ir(),
            Some(Ok(json!({"traces": {"r1": ["todo.task.AddTask"], "r3": ["todo.sync.SyncChanges"]}}))));
        assert_eq!(t.lines[2], "r6: behaviour: constraint gives no behaviour_reason");
    }

    #[test]
    fn a_trace_gap_is_the_agents_once_then_a_scope_decision() {
        let w = work();
        let mut h = trace_of(&trace_req(""), &trace_ir(), None).hits.remove(1);
        assert_eq!(classify(&w, &h).0, "agent");
        h["attempts"] = Value::from(1u64);
        assert_eq!(classify(&w, &h).0, "person");
    }

    /// The raiser's own `excerpt`, `default_reason` and `consequences` are kept when it wrote them.
    #[test]
    fn the_raisers_own_context_keys_are_kept() {
        let f: Value = serde_yaml::from_str(
            "id: f1\ntype: ambiguity\nraised_by: llm\ncites: [r1]\nsays: r1 names the clerk only\noptions: [a, b]\ndefault: a\nscope_changing: true\nexcerpt: Bill customers\ndefault_reason: simplest\nconsequences:\n- {does: builds a, cost: small}\n- {does: builds b, cost: large}\n",
        )
        .unwrap();
        let mut w = work();
        amend_one(&mut w, &f, "b", "person:operator", "a person decided").unwrap();
        let q = last_question(&w);
        assert_eq!(q["excerpt"].as_str(), Some("Bill customers"));
        assert_eq!(q["default_reason"].as_str(), Some("simplest"));
        assert_eq!(q["consequences"][1]["cost"].as_str(), Some("large"));
    }

    /// runs/trial-3-mixed-4: q3 asks about the dark spot d6; r3 cites both.
    fn mixed4(cites: &str) -> Work {
        let mut w = work();
        w.diss = serde_yaml::from_str("items:\n- {id: d1, type: fact}\n- {id: d5, type: unknown}\n- {id: d6, type: dark_spot}\n").unwrap();
        w.qs = serde_yaml::from_str("questions:\n- {id: q3, item: d6, status: accepted, answer: a, default: a, scope_changing: false}\n").unwrap();
        w.req = serde_yaml::from_str(&format!("requirements:\n- id: r3\n  text: Anyone with the link opens it.\n  cites: {cites}\n")).unwrap();
        w
    }

    fn r3(w: &Work) -> Value {
        w.req["requirements"][0].clone()
    }

    #[test]
    fn an_open_item_beside_the_question_that_asks_about_it_is_cited() {
        let w = mixed4("[q3, d6]");
        assert_eq!(citation_fault(&w, &r3(&w)), None);
    }

    #[test]
    fn an_open_item_is_never_the_only_ground_nor_cited_without_its_question() {
        let w = mixed4("[d6]");
        assert!(citation_fault(&w, &r3(&w)).unwrap().contains("cites no fact, wish or question"));
        let w = mixed4("[d1, d5]");
        assert!(citation_fault(&w, &r3(&w)).unwrap().contains("r3 cites d5, an unknown that no question it cites asks about"));
        let w = mixed4("[d1, d9]");
        assert!(citation_fault(&w, &r3(&w)).unwrap().contains("no dissection item and no question"));
    }

    /// D137: a decision on r3 is not refused for a citation fault r3 had before it.
    #[test]
    fn a_fault_the_decision_did_not_write_does_not_refuse_it() {
        let mut w = mixed4("[d1, d5]");
        let f: Value = serde_yaml::from_str("id: f1\ntype: missing_decision\nraised_by: llm\ncites: [r3]\nsays: how is the link made\noptions: [a, b]\ndefault: a\nscope_changing: true\n").unwrap();
        let before = w.req.clone();
        let ctx = w.ctx.clone();
        let q = amend_one(&mut w, &f, "a", "decider:haiku", "decided").unwrap()[0].clone();
        assert_eq!(verify(&before, &ctx, &BTreeSet::new(), &w, &f, "a", &q, true).unwrap(), None);
    }

    /// runs/trial-3-haiku-5: the spec stage's agent answered q10 "Postgres database for the list";
    /// change.implement's contradiction f8 (cites q10 and r1) was answered "store the list in the
    /// data file", and r1 kept citing both.
    fn haiku5() -> (Work, Value) {
        let mut w = work();
        w.diss = serde_yaml::from_str("items:\n- {id: d1, type: wish}\n").unwrap();
        w.qs = serde_yaml::from_str("questions:\n- {id: q10, item: f7, question: where is the list kept, options: [Postgres database for the list, hosted key-value store for the list], default: Postgres database for the list, scope_changing: false, status: accepted, answer: Postgres database for the list, answered_by: non-interactive}\n").unwrap();
        w.req = serde_yaml::from_str("requirements:\n- id: r1\n  text: 'The app is a to-do list web app. Decision q10: Postgres database for the list.'\n  cites: [d1, q10]\n- id: r2\n  text: 'The list survives a restart. Decision q10: Postgres database for the list.'\n  cites: [d1, q10]\n").unwrap();
        let f: Value = serde_yaml::from_str("id: f8\ntype: contradiction\nraised_by: llm:change.implement\ncites: [q10, r1]\nsays: q10 and r1 keep the list in Postgres, but the build has no Postgres client\noptions: [store the list in the data file (standard library only), implement the Postgres wire protocol by hand]\ndefault: store the list in the data file (standard library only)\nscope_changing: false\n").unwrap();
        (w, f)
    }

    const DATA_FILE: &str = "store the list in the data file (standard library only)";

    /// D141: a decision that replaces an earlier answer marks it superseded; every requirement that
    /// cited it cites the new decision instead, and no sentence of the old answer is left.
    #[test]
    fn a_contradiction_decision_supersedes_the_answer_it_replaces() {
        let (mut w, f) = haiku5();
        let before = w.req.clone();
        let ctx = w.ctx.clone();
        let amended = amend_one(&mut w, &f, DATA_FILE, "agent", "changes no scope").unwrap();
        let q10 = w.question("q10").unwrap();
        assert_eq!(st(q10, "status"), "superseded");
        assert_eq!(st(q10, "superseded_by"), "q11");
        for (i, rid) in ["r1", "r2"].iter().enumerate() {
            let r = &w.req["requirements"][i];
            assert_eq!(strs(r, "cites"), vec!["d1", "q11"], "{rid}: {r:?}");
            let t = st(r, "text");
            assert!(!t.contains("q10") && !t.contains("Postgres"), "{rid}: {t}");
            assert_eq!(t.matches(&format!("Decision q11: {DATA_FILE}.")).count(), 1, "{rid}: {t}");
            assert!(amended.contains(&rid.to_string()), "{amended:?}");
        }
        assert!(amended.contains(&"q10".to_string()), "{amended:?}");
        assert_eq!(verify(&before, &ctx, &BTreeSet::new(), &w, &f, DATA_FILE, "q11", true).unwrap(), None);
    }

    /// A contradiction decided for the cited answer replaces nothing.
    #[test]
    fn a_decision_that_keeps_the_cited_answer_supersedes_nothing() {
        let (mut w, mut f) = haiku5();
        f["options"] = serde_yaml::from_str("[Postgres database for the list, store the list in the data file]").unwrap();
        amend_one(&mut w, &f, "Postgres database for the list", "agent", "kept").unwrap();
        assert_eq!(st(w.question("q10").unwrap(), "status"), "accepted");
        assert_eq!(strs(&w.req["requirements"][0], "cites"), vec!["d1", "q10", "q11"]);
    }

    /// D141: a requirement citing a superseded answer is a citation fault, for requirements.check,
    /// for every stage's finding gate, and for the amendment that left it.
    #[test]
    fn a_requirement_citing_a_superseded_answer_is_a_citation_fault() {
        let (mut w, f) = haiku5();
        let before = w.req.clone();
        let ctx = w.ctx.clone();
        amend_one(&mut w, &f, DATA_FILE, "agent", "changes no scope").unwrap();
        w.req["requirements"][0]["cites"].as_sequence_mut().unwrap().push(s("q10"));
        let why = citation_fault(&w, &w.req["requirements"][0]).expect("a fault");
        assert!(why.contains("r1 cites q10, an answer superseded by q11"), "{why}");
        assert_eq!(check_requirements(&w), 1);
        assert!(blocking(&w, "story").unwrap().iter().any(|b| b.contains("superseded by q11")));
        let v = verify(&before, &ctx, &BTreeSet::new(), &w, &f, DATA_FILE, "q11", true).unwrap();
        assert!(v.is_some_and(|v| v.contains("superseded by q11")));
    }

    const SONNET_R5: &str = "r5 is a constraint, not behaviour: mark r5 as a constraint outside the specification.";

    /// runs/trial-3-small-sonnet-1: the decider's answer to p-trace-r5 extends the option's text.
    #[test]
    fn a_trace_decision_that_starts_with_the_constraint_option_is_a_constraint() {
        assert_eq!(trace_kind("r5", SONNET_R5), "constraint");
        assert_eq!(trace_kind("r5", "r5 is a constraint, not behaviour"), "constraint");
        assert_eq!(trace_kind("r5", "drop r5 from this epic's scope: it is not wanted"), "dropped");
        assert_eq!(trace_kind("r5", &trace_options("r5")[0]), "");
        assert_eq!(trace_kind("r5", "r5 is a constraint, not behaviourally tested"), "");
        assert_eq!(trace_kind("", SONNET_R5), "");
    }

    /// runs/trial-3-small-sonnet-1: q5 "specify r5 in the spec" (the agent's first decision on
    /// p-trace-r5) is superseded by the decision taken when the gap came back.
    #[test]
    fn a_finding_decided_again_supersedes_its_earlier_answer() {
        let mut w = work();
        w.diss = serde_yaml::from_str("items:\n- {id: d4, type: fact}\n").unwrap();
        let specify = trace_options("r5")[0].clone();
        w.qs = serde_yaml::from_str(&format!("questions:\n- {{id: q5, item: p-trace-r5, status: accepted, answer: '{specify}', answered_by: non-interactive, default: '{specify}', scope_changing: false}}\n")).unwrap();
        w.req = serde_yaml::from_str(&format!("requirements:\n- id: r5\n  text: 'The tool uses only the standard library. Decision q5: {specify}.'\n  cites: [d4, q5]\n  spec_domains: [wc.cli]\n")).unwrap();
        let mut f = trace_of(&w.req, &json!({"commands": {}, "domains": {"wc.cli": {"commands": [], "views": []}}}), None).hits.remove(0);
        f["attempts"] = Value::from(1u64);
        amend_one(&mut w, &f, SONNET_R5, "decider:haiku", "decider:haiku decided").unwrap();
        let q5 = w.question("q5").unwrap();
        assert_eq!((st(q5, "status"), st(q5, "superseded_by")), ("superseded", "q6"));
        let r5 = &w.req["requirements"][0];
        assert_eq!(strs(r5, "cites"), vec!["d4", "q6"]);
        assert!(!st(r5, "text").contains("specify r5"), "{}", st(r5, "text"));
        assert_eq!(citation_fault(&w, r5), None);
    }

    /// A resolved program finding raised again waits with `reraised` and its decision taken off.
    #[test]
    fn a_resolved_program_finding_raised_again_is_reraised() {
        let mut f: Value = serde_yaml::from_str(&format!("id: p-trace-r5\ntrace: r5\nstatus: resolved\ndecision: '{SONNET_R5}'\ndecided_by: decider:haiku\nresolution: {{amends: [q6]}}\n")).unwrap();
        let h: Value = serde_yaml::from_str("says: 'r5: traces to no behaviour'\n").unwrap();
        reraise(&mut f, &h);
        assert_eq!(st(&f, "status"), "needs_person");
        assert_eq!(st(&f["reraised"], "decision"), SONNET_R5);
        assert_eq!(st(&f["reraised"], "decided_by"), "decider:haiku");
        assert!(f.get("decision").is_none() && f.get("resolution").is_none());
    }

    /// D154: the run's build constraints, as driver/examples/sdc/build-constraints.yaml writes them.
    fn constrained(req: &str) -> Work {
        let mut w = work();
        w.diss = serde_yaml::from_str("items:\n- {id: d1, type: wish}\n").unwrap();
        w.bc = serde_yaml::from_str("format: sdc.build-constraints/1\nlanguage: Rust\ndependencies: []\n").unwrap();
        w.req = serde_yaml::from_str(req).unwrap();
        w
    }

    /// runs/trial-3-small-haiku-1: r5 named Python 3 and the builder was told Rust. A requirement
    /// naming another language or a dependency is a fault of requirements.check.
    #[test]
    fn a_requirement_naming_another_language_or_a_dependency_breaks_the_constraints() {
        let w = constrained("requirements:\n- id: r5\n  text: The tool is written in Python 3.\n  cites: [d1]\n  language: Python 3\n");
        let why = constraint_fault(&w, &w.req["requirements"][0]).unwrap();
        assert!(why.contains("r5 names the language Python 3") && why.contains("fix it to Rust"), "{why}");
        assert_eq!(check_requirements(&w), 1);
        let w = constrained("requirements:\n- id: r1\n  text: The list is kept in Postgres.\n  cites: [d1]\n  dependencies: [Postgres]\n");
        let why = constraint_fault(&w, &w.req["requirements"][0]).unwrap();
        assert!(why.contains("r1 names the dependencies Postgres") && why.contains("allow none"), "{why}");
    }

    /// A requirement that restates the constraint is marked `behaviour: constraint`; one that names
    /// nothing is not checked, nor is any requirement in a run without a constraints file.
    #[test]
    fn a_requirement_restating_the_constraint_is_a_constraint() {
        let w = constrained("requirements:\n- id: r5\n  text: The tool is written in Rust.\n  cites: [d1]\n  language: rust\n");
        assert!(constraint_fault(&w, &w.req["requirements"][0]).unwrap().contains("behaviour: constraint"));
        let w = constrained("requirements:\n- id: r5\n  text: The tool is written in Rust.\n  cites: [d1]\n  language: Rust 2021\n  behaviour: constraint\n  behaviour_reason: the build constraints fix the language\n- id: r1\n  text: Bill customers.\n  cites: [d1]\n");
        assert_eq!(check_requirements(&w), 0);
        let mut w = constrained("requirements:\n- id: r5\n  text: Python.\n  cites: [d1]\n  language: Python 3\n");
        w.bc = Value::Mapping(Mapping::new());
        assert_eq!(constraint_fault(&w, &w.req["requirements"][0]), None);
    }

    /// runs/trial-3-small-haiku-1: f11 contradicted q17, the answer of f9, which decider:haiku had
    /// decided; f11 is f9 raised again, so it waits with `reraised` naming f9 and its decision.
    #[test]
    fn an_agent_contradiction_against_an_applied_decision_raises_it_again() {
        let mut w = work();
        w.qs = serde_yaml::from_str("questions:\n- {id: q17, item: f9, status: accepted, answer: Python 3, default: Rust, scope_changing: true}\n- {id: q7, item: d3, status: accepted, answer: a, default: a, scope_changing: false}\n").unwrap();
        w.fd = serde_yaml::from_str("findings:\n- {id: f9, type: contradiction, status: resolved, decision: Implement in Python 3, decided_by: 'decider:haiku'}\n- {id: f10, type: contradiction, status: resolved, decision: Rust, decided_by: agent}\n").unwrap();
        let f11: Value = serde_yaml::from_str("id: f11\ntype: contradiction\nraised_by: llm:change.implement\ncites: [r5, q17]\nsays: q17 says Python 3, the step says Rust\n").unwrap();
        assert_eq!(st(&reraised_of(&w, &f11).unwrap(), "id"), "f9");
        let mut amb = f11.clone();
        amb["type"] = s("ambiguity");
        assert!(reraised_of(&w, &amb).is_none());
        let mut interview = f11.clone();
        interview["cites"] = serde_yaml::from_str("[r5, q7]").unwrap();
        assert!(reraised_of(&w, &interview).is_none());
        w.question_mut("q17").unwrap()["status"] = s("superseded");
        assert!(reraised_of(&w, &f11).is_none());
    }
}
