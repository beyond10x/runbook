//! Timeline of one recorded run, `sdc.run-timeline/1` (docs/llm-is-not-the-runtime.md).
//!
//! Two kinds of run directory are read into the same document (an `sdc` run is the driver's or a
//! `runbook loom-run` directory, `run_kind.rs`):
//!
//! - `sdc`: a run of the removed driver or `sdc-experiment run` wrote. Every take of a bound
//!   action is a span: `program` (a tool the driver ran because the action was admissible), `model`
//!   (a model call as the action's executor, joined from `llm-calls.jsonl` and the call's record
//!   directory), `rule` (a step whose principal is a rule, `rule:autopilot`); every decider call of
//!   `decisions.jsonl` is a `decider` span; the time between spans in which nothing ran is `idle`.
//! - `free-agent`: a run `sdc-free-agent` wrote (`free-agent.json`, the session's stream
//!   `events.jsonl` and the receipt times `received.jsonl`): one `model` span per API request, one
//!   `tool` span per tool call the agent made, `idle` between them.
//!
//! Every figure is read from the run's records; nothing is estimated. A figure a record does not
//! hold is null, and `notes` says which. Labels are case and action names, tool names and model
//! ids: no prompt, reply, file content or absolute path is copied.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use serde_json::{json, Map, Value as J};

use crate::run_kind::{self, RunKind};
use crate::run_metrics::{self as rm, epoch_ms, js, Fail, Res};

pub const FORMAT: &str = "sdc.run-timeline/1";

/// Lanes in the order a document lists spans that start at the same instant.
const LANES: [&str; 7] = ["program", "model", "decider", "rule", "tool", "idle", "other"];

#[derive(Clone, Debug)]
struct Span {
    t0: i64,
    t1: i64,
    lane: &'static str,
    case: Option<String>,
    action: Option<String>,
    cause: J,
    extra: Map<String, J>,
}

impl Span {
    fn new(t0: i64, t1: i64, lane: &'static str) -> Self {
        Span { t0, t1: t1.max(t0), lane, case: None, action: None, cause: J::Null, extra: Map::new() }
    }
    fn dur(&self) -> i64 {
        self.t1 - self.t0
    }
}

fn round3(x: f64) -> f64 {
    (x * 1e3).round() / 1e3 + 0.0
}

fn secs(ms: i64) -> f64 {
    round3(ms as f64 / 1000.0)
}

fn base_name(p: &str) -> Option<String> {
    Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned())
}

/// Tokens of a usage object in the four kinds a document reports; null when there is no usage.
fn tokens_of(u: &J) -> J {
    if !u.is_object() {
        return J::Null;
    }
    let n = |k: &str| u[k].as_i64();
    json!({"input": n("input_tokens"), "output": n("output_tokens"),
        "cache_read": n("cache_read_input_tokens"), "cache_write": n("cache_creation_input_tokens")})
}

const TOKEN_KINDS: [&str; 4] = ["input", "output", "cache_read", "cache_write"];

fn add_tokens(acc: &mut J, t: &J) {
    if !t.is_object() {
        return;
    }
    if !acc.is_object() {
        *acc = json!({"input": 0, "output": 0, "cache_read": 0, "cache_write": 0});
    }
    for k in TOKEN_KINDS {
        let a = acc[k].as_i64();
        acc[k] = match (a, t[k].as_i64()) {
            (Some(a), Some(b)) => json!(a + b),
            _ => J::Null,
        };
    }
}

fn add_counts(acc: &mut BTreeMap<String, i64>, m: &J) {
    for (k, v) in m.as_object().into_iter().flatten() {
        *acc.entry(k.clone()).or_default() += v.as_i64().unwrap_or(0);
    }
}

/// API requests and tool calls by name of one model session, from its record's event stream: a
/// metaharness stream (`usage` events by request id, `tool.requested`) or a `claude -p`
/// stream-json (assistant messages by request id, their `tool_use` blocks). None without a stream.
type Figures = (Option<i64>, Option<BTreeMap<String, i64>>, Option<i64>);

/// The third figure: tool calls the harness denied (metaharness `tool.decided` deny; a `claude -p`
/// result's `permission_denials`).
fn session_figures(events: &Path) -> Res<Figures> {
    let mut m = 0;
    let Some(lines) = rm::jsonl(events, &mut m)? else { return Ok((None, None, None)) };
    let mut denied = BTreeSet::new();
    let mut requests = BTreeSet::new();
    let mut tools: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let metaharness = lines.iter().any(|l| l["format"] == "metaharness.event/1");
    for l in &lines {
        if metaharness {
            match l["event"].as_str().unwrap_or("") {
                "usage" => {
                    requests.insert(js(&l["request_id"]));
                }
                "tool.requested" => {
                    tools.entry(js(&l["name"])).or_default().insert(js(&l["call_id"]));
                }
                "tool.decided" if l["decision"]["decision"] == "deny" => {
                    denied.insert(js(&l["call_id"]));
                }
                _ => {}
            }
        } else if l["type"] == "result" {
            for (i, d) in l["permission_denials"].as_array().into_iter().flatten().enumerate() {
                denied.insert(Some(js(&d["tool_use_id"])).filter(|s| !s.is_empty()).unwrap_or_else(|| format!("#{i}")));
            }
        } else if l["type"] == "assistant" && l["parent_tool_use_id"].is_null() {
            let rid = Some(js(&l["request_id"])).filter(|s| !s.is_empty()).unwrap_or_else(|| js(&l["message"]["id"]));
            requests.insert(rid);
            for c in l["message"]["content"].as_array().into_iter().flatten().filter(|c| c["type"] == "tool_use") {
                tools.entry(js(&c["name"])).or_default().insert(js(&c["id"]));
            }
        }
    }
    let counts = tools.into_iter().map(|(k, v)| (k, v.len() as i64)).collect();
    Ok((Some(requests.len() as i64), Some(counts), Some(denied.len() as i64)))
}

/// Start and end (ms) of a call from its record's `status.json`.
fn call_times(rec: &Path) -> Res<Option<(i64, i64)>> {
    let Some(s) = rm::json_opt(&rec.join("status.json"))? else { return Ok(None) };
    Ok(match (s["started_at"].as_str().and_then(epoch_ms), s["finished_at"].as_str().and_then(epoch_ms)) {
        (Some(a), Some(b)) => Some((a, b.max(a))),
        _ => None,
    })
}

// ---- cause ---------------------------------------------------------------------------------

/// The leaves of a precondition that hold in `claims`: for `all` those of every child, for `any`
/// those of the children that hold; a negation contributes none.
fn held(pre: &J, claims: &J, out: &mut Vec<(String, String)>) -> bool {
    if let Some(c) = pre.get("claim") {
        let id = js(&c["id"]);
        let want = js(&c["is"]);
        let value = claims[&id]["value"].as_str().unwrap_or("unknown").to_string();
        if value == want {
            out.push((id, value));
            return true;
        }
        return false;
    }
    if let Some(all) = pre["all"].as_array() {
        let mut mine = vec![];
        let ok = all.iter().all(|p| held(p, claims, &mut mine));
        if ok {
            out.extend(mine);
        }
        return ok;
    }
    if let Some(any) = pre["any"].as_array() {
        let mut ok = false;
        for p in any {
            let mut mine = vec![];
            if held(p, claims, &mut mine) {
                out.extend(mine);
                ok = true;
            }
        }
        return ok;
    }
    if pre.get("not").is_some() {
        let mut ignore = vec![];
        return !held(&pre["not"], claims, &mut ignore);
    }
    false
}

/// What made `action` admissible: the claims of its precondition that held in the decision the
/// driver read before starting it, each with the evidence the decision names for it. Null when
/// the case log has no decision before the take or the protocol IR no precondition.
fn cause_of(action: &str, ir: &J, decision: Option<(&J, i64)>, retry_of: &J) -> J {
    let Some((d, seq)) = decision else { return J::Null };
    let pre = &ir["actions"][action]["precondition"];
    if pre.is_null() {
        return J::Null;
    }
    let mut leaves = vec![];
    held(pre, &d["claims"], &mut leaves);
    let mut seen = BTreeSet::new();
    let claims: Vec<J> = leaves.into_iter().filter(|(c, _)| seen.insert(c.clone())).map(|(c, v)| {
        let ev: Vec<J> = d["explanation"]["claims"][&c]["because"].as_array().into_iter().flatten()
            .filter_map(|b| b["evidence"].as_str()).map(|s| json!(s)).collect();
        json!({"claim": c, "value": v, "evidence": ev})
    }).collect();
    let mut out = json!({"decision_seq": seq, "claims": claims});
    if retry_of.as_array().is_some_and(|a| !a.is_empty()) {
        out["retry_of"] = retry_of.clone();
    }
    let requires: Vec<J> = ir["actions"][action]["requires"].as_array().into_iter().flatten()
        .filter_map(|r| r["capability"].as_str()).map(|s| json!(s)).collect();
    if !requires.is_empty() {
        out["capabilities"] = J::Array(requires);
    }
    out
}

// ---- sdc runs ------------------------------------------------------------------------------

struct Rounds {
    rounds: Vec<(i64, i64)>,
    waiting: Option<Vec<(i64, i64)>>,
}

fn settings_of(last: &J) -> J {
    let mut models = Map::new();
    for (k, v) in last["models"].as_object().into_iter().flatten() {
        models.insert(k.clone(), v["model"].clone());
    }
    let mut efforts = Map::new();
    for (k, v) in last["efforts"].as_object().into_iter().flatten() {
        efforts.insert(k.clone(), v["effort"].clone());
    }
    let mut adapters = Map::new();
    for (k, v) in last["adapters"].as_object().into_iter().flatten() {
        adapters.insert(k.clone(), v["adapter"].clone());
    }
    json!({"models": models, "efforts": efforts, "adapter": last["adapter"], "adapters": adapters,
        "decider": {"kind": last["decider"]["kind"], "model": last["decider"]["model"], "escalate": last["decider"]["escalate"]},
        "autonomy": last["autonomy"], "max_llm_calls": last["max_llm_calls"], "max_wall_seconds": last["max_wall_seconds"]})
}

fn load_sdc(dir: &Path, name: &str, kind: RunKind) -> Res<J> {
    let root = rm::root_case_dir(dir);
    if !root.join("events.jsonl").is_file() {
        return Err(Fail(format!("cannot read {}: no such file", root.join("events.jsonl").display())));
    }
    let mut malformed = 0usize;
    let mut dirs = vec![];
    rm::case_dirs(&root, &mut dirs);
    let mut spans: Vec<Span> = vec![];
    let mut tree = vec![];
    let mut event_times: Vec<i64> = vec![];
    let mut notes: BTreeSet<String> = BTreeSet::new();
    for d in &dirs {
        let id = d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let parent = d.parent().filter(|p| p.file_name().is_some_and(|n| n == "children"))
            .and_then(Path::parent).and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
        let events = rm::jsonl_indexed(&d.join("events.jsonl"), &mut malformed)?;
        let observed = rm::jsonl_indexed(&d.join("observed.jsonl"), &mut malformed)?;
        let mut times: Vec<Option<i64>> = events.iter().map(|e| e["at"].as_str().and_then(epoch_ms)).collect();
        for o in &observed {
            if let (Some(i), Some(t)) = (o["index"].as_u64(), o["observed_at"].as_str().and_then(epoch_ms)) {
                if let Some(s) = times.get_mut(i as usize) {
                    *s = Some(t);
                }
            }
        }
        let protocol = events.iter().find(|e| e["event"] == "opened").map(|e| js(&e["protocol"]["id"])).unwrap_or_default();
        let outcome = events.iter().rev().find(|e| e["event"] == "terminated").map(|e| e["outcome"].clone()).unwrap_or(J::Null);
        tree.push(json!({"case": id, "parent": parent, "stage": rm::stage_of(&protocol), "outcome": outcome}));
        let ir = rm::json_opt(&d.join("ir.json"))?.unwrap_or(J::Null);
        let mut decisions: BTreeMap<String, J> = BTreeMap::new();
        let mut last_eval: Option<(String, i64)> = None;
        let mut open: Option<(usize, String, bool, J)> = None;
        for (i, e) in events.iter().enumerate() {
            let t = times[i];
            if let Some(t) = t {
                event_times.push(t);
            }
            match e["event"].as_str().unwrap_or("") {
                "evaluated" => {
                    let f = js(&e["file"]);
                    if !decisions.contains_key(&f) && !f.is_empty() && !f.contains("..") {
                        let doc = rm::json_opt(&d.join(&f))?.unwrap_or(J::Null);
                        decisions.insert(f.clone(), doc);
                    }
                    last_eval = Some((f, e["seq"].as_i64().unwrap_or(i as i64)));
                }
                "action_started" => {
                    let a = js(&e["action"]);
                    let dec = last_eval.as_ref().and_then(|(f, s)| decisions.get(f).filter(|v| !v.is_null()).map(|v| (v, *s)));
                    let cause = cause_of(&a, &ir, dec, &e["retry_of"]);
                    if cause.is_null() {
                        notes.insert("a span with cause null: no decision before the take in its case log, or no precondition in the case's ir.json".into());
                    }
                    open = Some((i, a, e["llm"] == true, cause));
                }
                "action_finished" => {
                    let Some((si, a, llm, cause)) = open.take() else { continue };
                    if a != js(&e["action"]) {
                        continue;
                    }
                    let (Some(t0), Some(t1)) = (times[si], t) else {
                        notes.insert("a take without a time in its case log is left out".into());
                        continue;
                    };
                    let principal = e["principal"].as_str().map(String::from);
                    let lane = if principal.as_deref().is_some_and(|p| p.starts_with("rule:")) {
                        "rule"
                    } else if llm {
                        "model"
                    } else {
                        "program"
                    };
                    let mut s = Span::new(t0, t1, lane);
                    s.case = Some(id.clone());
                    s.action = Some(a);
                    s.cause = cause;
                    s.extra.insert("exit".into(), json!(js(&e["exit"])));
                    if e["not_called"].is_string() {
                        s.extra.insert("not_called".into(), json!(true));
                    }
                    if let Some(p) = principal {
                        s.extra.insert("principal".into(), json!(p));
                    }
                    let ev: Vec<J> = e["evidence"].as_array().into_iter().flatten().map(|x| json!(js(x))).collect();
                    s.extra.insert("evidence".into(), J::Array(ev));
                    if lane == "model" {
                        for k in ["model", "effort", "adapter", "class", "api_requests", "tool_calls", "tool_calls_denied", "tokens", "cost_usd"] {
                            s.extra.insert(k.into(), J::Null);
                        }
                        s.extra.insert("sessions".into(), json!(0));
                    }
                    spans.push(s);
                }
                "terminated" => {
                    if let (Some(p), Some(t)) = (e["principal"].as_str().filter(|p| p.starts_with("rule:")), t) {
                        let mut s = Span::new(t, t, "rule");
                        s.case = Some(id.clone());
                        s.action = Some(format!("terminated:{}", js(&e["outcome"])));
                        s.extra.insert("principal".into(), json!(p));
                        spans.push(s);
                    }
                }
                _ => {}
            }
        }
    }

    // Model calls: each llm-calls.jsonl line joined to the model take of its case and action whose
    // span holds the call's start; a call no take holds is a span of its own.
    // The call record directories: the driver's `llm/`; Loom's tool writes them in the product tree.
    let llm_dir = match kind {
        RunKind::Driver => dir.join("llm"),
        RunKind::Loom => dir.join("work").join(".sdc").join("llm"),
    };
    let calls = rm::jsonl(&dir.join("llm-calls.jsonl"), &mut malformed)?;
    if calls.is_none() {
        notes.insert("no llm-calls.jsonl: model spans carry no figures".into());
    }
    for l in calls.iter().flatten() {
        let rec = l["record"].as_str().and_then(base_name).map(|b| llm_dir.join(b));
        let times = match &rec {
            Some(r) => call_times(r)?,
            None => None,
        };
        let times = times.or_else(|| {
            let s = l["started"].as_str().and_then(epoch_ms)?;
            Some((s, s + l["duration_ms"].as_i64().unwrap_or(0)))
        });
        let Some((c0, c1)) = times else {
            notes.insert("a model call without a time is left out of the spans".into());
            continue;
        };
        let (requests, tools, denied) = match &rec {
            Some(r) => session_figures(&r.join("events.jsonl"))?,
            None => (None, None, None),
        };
        let (case, action) = (js(&l["case"]), js(&l["action"]));
        let holds = |slack: i64| spans.iter().position(|s| s.lane == "model" && s.case.as_deref() == Some(case.as_str())
            && s.action.as_deref() == Some(action.as_str()) && s.extra.get("not_called").is_none()
            && c0 >= s.t0.div_euclid(1000) * 1000 && c0 <= s.t1 + slack);
        let hit = holds(0).or_else(|| holds(1000));
        let idx = match hit {
            Some(i) => i,
            None => {
                let mut s = Span::new(c0, c1, "model");
                s.case = Some(case);
                s.action = Some(action);
                s.extra.insert("take".into(), json!(false));
                s.extra.insert("sessions".into(), json!(0));
                notes.insert("a model span with take false: a model call no take of its case and action holds".into());
                spans.push(s);
                spans.len() - 1
            }
        };
        let s = &mut spans[idx];
        let first = s.extra["sessions"].as_i64().unwrap_or(0) == 0;
        s.extra["sessions"] = json!(s.extra["sessions"].as_i64().unwrap_or(0) + 1);
        let tok = tokens_of(&l["usage"]);
        if first {
            for k in ["model", "effort", "adapter", "class"] {
                s.extra.insert(k.into(), l[k].clone());
            }
            s.extra.insert("api_requests".into(), json!(requests));
            s.extra.insert("tool_calls".into(), json!(tools));
            s.extra.insert("tool_calls_denied".into(), json!(denied));
            s.extra.insert("tokens".into(), tok);
            s.extra.insert("cost_usd".into(), l["cost_usd"].clone());
        } else {
            let r = match (s.extra["api_requests"].as_i64(), requests) {
                (Some(a), Some(b)) => json!(a + b),
                _ => J::Null,
            };
            s.extra.insert("api_requests".into(), r);
            let mut m = BTreeMap::new();
            add_counts(&mut m, &s.extra["tool_calls"]);
            add_counts(&mut m, &json!(tools));
            s.extra.insert("tool_calls".into(), json!(m));
            let dn = match (s.extra["tool_calls_denied"].as_i64(), denied) {
                (Some(a), Some(b)) => json!(a + b),
                _ => J::Null,
            };
            s.extra.insert("tool_calls_denied".into(), dn);
            let mut t = s.extra["tokens"].clone();
            add_tokens(&mut t, &tok);
            s.extra.insert("tokens".into(), t);
            let c = s.extra["cost_usd"].as_f64().unwrap_or(0.0) + l["cost_usd"].as_f64().unwrap_or(0.0);
            s.extra.insert("cost_usd".into(), json!(rm::round6(c)));
        }
        if requests.is_none() {
            notes.insert("a model span with api_requests null: the call's record has no event stream".into());
        }
    }
    if spans.iter().any(|s| s.lane == "model" && s.extra["sessions"] == 0 && s.extra.get("not_called").is_none()) {
        notes.insert("a model span with sessions 0: a model take with no llm-calls.jsonl line".into());
    }

    // Decider calls.
    let decisions = rm::jsonl(&dir.join("decisions.jsonl"), &mut malformed)?;
    for l in decisions.iter().flatten().filter(|l| rm::decider_line_is_call(l)) {
        let rec = l["record"].as_str().and_then(base_name).map(|b| dir.join("decider").join(b));
        let times = match &rec {
            Some(r) => call_times(r)?,
            None => None,
        };
        let times = times.or_else(|| {
            let end = l["time"].as_str().and_then(epoch_ms)?;
            Some((end - l["duration_ms"].as_i64().unwrap_or(0), end))
        });
        let Some((c0, c1)) = times else {
            notes.insert("a decider call without a time is left out of the spans".into());
            continue;
        };
        let (requests, tools, denied) = match &rec {
            Some(r) => session_figures(&r.join("events.jsonl"))?,
            None => (None, None, None),
        };
        let mut s = Span::new(c0, c1, "decider");
        s.case = Some(js(&l["case"]));
        s.action = Some(js(&l["action"]));
        let class = Some(js(&l["class"])).filter(|c| !c.is_empty()).unwrap_or_else(|| "-".into());
        s.extra.insert("asked".into(), json!(format!("{}/{class}", js(&l["type"]))));
        s.extra.insert("decision".into(), l["decision"].clone());
        for k in ["model", "effort", "adapter"] {
            s.extra.insert(k.into(), l[k].clone());
        }
        s.extra.insert("api_requests".into(), json!(requests));
        s.extra.insert("tool_calls".into(), json!(tools));
        s.extra.insert("tool_calls_denied".into(), json!(denied));
        s.extra.insert("tokens".into(), tokens_of(&l["tokens"]));
        s.extra.insert("cost_usd".into(), l["cost_usd"].clone());
        if requests.is_none() {
            notes.insert("a decider span with api_requests null: the call's record has no event stream".into());
        }
        spans.push(s);
    }

    // Rounds and the time waiting on a person, for the idle spans.
    let (rounds, legs, stopped_at, by_rule) = rm::waiting_inputs(dir)?;
    let status = rm::json_opt(&dir.join("status.json"))?;
    let closed = tree.first().is_some_and(|c| !c["outcome"].is_null()) || status.as_ref().is_some_and(|s| s["state"] == "closed");
    let waiting = rm::waiting_intervals(&rounds, &legs, stopped_at, by_rule, closed)
        .map(|v| v.into_iter().map(|(a, b)| (a * 1000, b * 1000)).collect());
    let rr = Rounds {
        rounds: rounds.iter().filter_map(|r| Some((r["started_epoch"].as_i64()? * 1000, (r["ended_epoch"].as_i64()? + 1) * 1000))).collect(),
        waiting,
    };
    if rr.rounds.is_empty() {
        notes.insert("no rounds.jsonl: idle time is unattributed".into());
    }
    let settings = rm::jsonl(&dir.join("run-settings.jsonl"), &mut malformed)?.unwrap_or_default();
    let settings = settings.last().map(settings_of).unwrap_or(J::Null);
    let end = status.as_ref().map(|s| json!({"state": s["state"], "outcome": s["outcome"], "delivered": s["delivered"]})).unwrap_or(J::Null);
    notes.insert("model and decider spans carry the session totals of llm-calls.jsonl and decisions.jsonl; per-request output tokens are not recorded correctly by the stream (it holds the count at the start of each response), so they are not split per request".into());
    if kind == RunKind::Loom {
        notes.insert("a runbook loom-run directory (docs/loom-run-directory.md, \"The two evaluation tools\"): model call records are read from work/.sdc/llm, a take's start is the start of its step (Loom's run log has one line, at the finish), and no take has a cause (Loom's run log has no evaluation record)".into());
    }
    finish("sdc", name, settings, J::Array(tree), spans, &event_times, Some(&rr), end, notes, malformed, None)
}

/// Idle spans in the gaps of the busy time between the run's first and last instant, cut at round
/// boundaries; the document; its totals.
#[allow(clippy::too_many_arguments)]
fn finish(kind: &str, name: &str, settings: J, tree: J, mut spans: Vec<Span>, event_times: &[i64], rounds: Option<&Rounds>,
          end: J, mut notes: BTreeSet<String>, malformed: usize, session: Option<J>) -> Res<J> {
    let lo = spans.iter().map(|s| s.t0).chain(event_times.iter().copied()).min();
    let hi = spans.iter().map(|s| s.t1).chain(event_times.iter().copied()).max();
    let (Some(lo), Some(hi)) = (lo, hi) else {
        return Err(Fail("the run has no timed event".into()));
    };
    let mut busy: Vec<(i64, i64)> = spans.iter().filter(|s| s.dur() > 0).map(|s| (s.t0, s.t1)).collect();
    busy.sort();
    let mut gaps = vec![];
    let mut cur = lo;
    for (a, b) in busy {
        if a > cur {
            gaps.push((cur, a));
        }
        cur = cur.max(b);
    }
    if hi > cur {
        gaps.push((cur, hi));
    }
    for (a, b) in gaps {
        let mut cuts: Vec<i64> = vec![a, b];
        if let Some(r) = rounds {
            for &(s, e) in &r.rounds {
                cuts.extend([s, e]);
            }
            for &(s, e) in r.waiting.iter().flatten() {
                cuts.extend([s, e]);
            }
        }
        cuts.retain(|&c| c >= a && c <= b);
        cuts.sort();
        cuts.dedup();
        for w in cuts.windows(2) {
            let (x, y) = (w[0], w[1]);
            if y <= x {
                continue;
            }
            let mid = (x + y) / 2;
            let cause = match rounds {
                Some(r) if r.waiting.iter().flatten().any(|&(s, e)| mid >= s && mid < e) => "waiting on a person",
                Some(r) if r.rounds.iter().any(|&(s, e)| mid >= s && mid < e) => "driver",
                Some(r) if !r.rounds.is_empty() => "between rounds",
                Some(_) => "unattributed",
                None => "harness",
            };
            match spans.last_mut() {
                Some(l) if l.lane == "idle" && l.t1 == x && l.cause == json!(cause) => l.t1 = y,
                _ => {
                    let mut s = Span::new(x, y, "idle");
                    s.cause = json!(cause);
                    spans.push(s);
                }
            }
        }
    }
    let lane_ix = |l: &str| LANES.iter().position(|x| *x == l).unwrap_or(LANES.len());
    spans.sort_by(|a, b| (a.t0, lane_ix(a.lane), a.t1, &a.case, &a.action).cmp(&(b.t0, lane_ix(b.lane), b.t1, &b.case, &b.action)));
    let docs: Vec<J> = spans.iter().map(|s| {
        let mut m = Map::new();
        m.insert("t0_s".into(), json!(secs(s.t0 - lo)));
        m.insert("t1_s".into(), json!(secs(s.t1 - lo)));
        m.insert("lane".into(), json!(s.lane));
        m.insert("case".into(), json!(s.case));
        m.insert("action".into(), json!(s.action));
        m.insert("cause".into(), s.cause.clone());
        for (k, v) in &s.extra {
            m.insert(k.clone(), v.clone());
        }
        J::Object(m)
    }).collect();
    if malformed > 0 {
        notes.insert(format!("{malformed} record lines did not parse and were skipped"));
    }
    let mut doc = json!({"format": FORMAT, "run": name, "kind": kind, "settings": settings, "end": end, "tree": tree,
        "totals": J::Null, "spans": docs, "notes": notes.into_iter().collect::<Vec<_>>()});
    if let Some(s) = session {
        doc["session"] = s;
    }
    doc["totals"] = totals_of(&doc);
    Ok(doc)
}

/// The totals of a document, from its spans only (and, for a free-agent run, the session's own
/// cost, which no span holds).
pub fn totals_of(doc: &J) -> J {
    let spans: Vec<&J> = doc["spans"].as_array().into_iter().flatten().collect();
    let d = |s: &J| round3(s["t1_s"].as_f64().unwrap_or(0.0) - s["t0_s"].as_f64().unwrap_or(0.0));
    let lane = |l: &'static str| spans.iter().copied().filter(move |s| s["lane"] == l);
    let lane_s = |l: &'static str| round3(lane(l).map(d).sum::<f64>());
    let wall = match (spans.iter().filter_map(|s| s["t0_s"].as_f64()).reduce(f64::min), spans.iter().filter_map(|s| s["t1_s"].as_f64()).reduce(f64::max)) {
        (Some(a), Some(b)) => round3(b - a),
        _ => 0.0,
    };
    let mut tokens = J::Null;
    let mut cost = 0.0;
    let mut cost_null = 0;
    let mut requests = 0;
    let mut requests_null = 0;
    for s in lane("model").chain(lane("decider")) {
        add_tokens(&mut tokens, &s["tokens"]);
        match s["cost_usd"].as_f64() {
            Some(c) => cost += c,
            None if s.get("cost_usd").is_some() => cost_null += 1,
            None => {}
        }
        match s["api_requests"].as_i64() {
            Some(r) => requests += r,
            None if s["sessions"].as_i64().unwrap_or(1) > 0 => requests_null += 1,
            None => {}
        }
    }
    let mut model_tools = BTreeMap::new();
    let mut denied = 0;
    for s in lane("model") {
        add_counts(&mut model_tools, &s["tool_calls"]);
        denied += s["tool_calls_denied"].as_i64().unwrap_or(0);
    }
    let mut agent_tools: BTreeMap<String, i64> = BTreeMap::new();
    for s in lane("tool") {
        *agent_tools.entry(js(&s["action"])).or_default() += 1;
    }
    let mut idle_by: BTreeMap<String, f64> = BTreeMap::new();
    for s in lane("idle") {
        *idle_by.entry(js(&s["cause"])).or_default() += d(s);
    }
    let idle_by: BTreeMap<String, f64> = idle_by.into_iter().map(|(k, v)| (k, round3(v))).collect();
    let busy: f64 = spans.iter().map(|s| d(s)).sum();
    let free = doc["kind"] == "free-agent";
    let (cost_usd, cost_source) = if free {
        (doc["session"]["cost_usd"].clone(), json!("session result (no API request records its own cost)"))
    } else if cost_null > 0 && cost == 0.0 {
        (J::Null, json!("not recorded"))
    } else {
        (json!(rm::round6(cost)), json!("sum of model and decider spans"))
    };
    let waiting = if free || spans.iter().all(|s| s["lane"] != "idle") {
        json!(0.0)
    } else if doc["notes"].as_array().into_iter().flatten().any(|n| n.as_str().is_some_and(|n| n.starts_with("no rounds.jsonl"))) {
        J::Null
    } else {
        json!(idle_by.get("waiting on a person").copied().unwrap_or(0.0))
    };
    let mut t = json!({
        "wall_s": wall,
        "cost_usd": cost_usd, "cost_source": cost_source,
        "tokens": tokens,
        "program_steps": lane("program").count(),
        "model_sessions": if free { json!(1) } else { json!(lane("model").map(|s| s["sessions"].as_i64().unwrap_or(0)).sum::<i64>()) },
        "api_requests": requests, "api_requests_unrecorded_spans": requests_null,
        "model_tool_calls": if free { json!(agent_tools.values().sum::<i64>()) } else { json!(model_tools.values().sum::<i64>()) },
        "model_tool_calls_by_name": if free { json!(agent_tools) } else { json!(model_tools) },
        "model_tool_calls_denied": denied,
        "decider_calls": lane("decider").count(),
        "rule_steps": lane("rule").count(),
        "model_s": lane_s("model"), "program_s": lane_s("program"), "decider_s": lane_s("decider"),
        "rule_s": lane_s("rule") + 0.0, "tool_s": lane_s("tool") + 0.0, "idle_s": lane_s("idle"),
        "idle_by_cause": idle_by,
        "waiting_on_person_s": waiting,
        "overlap_s": round3(busy - wall),
    });
    if !free {
        t["api_requests"] = json!(requests);
    }
    t
}

/// Every difference between a document's totals and the totals its spans give.
pub fn check(doc: &J) -> Vec<String> {
    let want = totals_of(doc);
    let have = &doc["totals"];
    let mut out = vec![];
    for (k, v) in want.as_object().into_iter().flatten() {
        let h = &have[k];
        let same = match (v.as_f64(), h.as_f64()) {
            (Some(a), Some(b)) => (a - b).abs() < 1e-6,
            _ => v == h,
        };
        if !same {
            out.push(format!("totals.{k} is {h} but its spans give {v}"));
        }
    }
    out
}

// ---- free-agent runs -------------------------------------------------------------------------

/// A free-running agent's run directory (`sdc-free-agent`): `free-agent.json`, `events.jsonl`
/// (the `claude -p` stream-json with partial messages) and `received.jsonl` (when each line of it
/// arrived).
fn load_free(dir: &Path, name: &str, meta: J) -> Res<J> {
    let mut malformed = 0;
    let events = rm::jsonl_indexed(&dir.join("events.jsonl"), &mut malformed)?;
    let received = rm::jsonl_indexed(&dir.join("received.jsonl"), &mut malformed)?;
    let mut times: Vec<Option<i64>> = vec![None; events.len()];
    for r in &received {
        if let (Some(i), Some(t)) = (r["index"].as_u64(), r["received_at"].as_str().and_then(epoch_ms)) {
            if let Some(s) = times.get_mut(i as usize) {
                *s = Some(t);
            }
        }
    }
    let mut notes: BTreeSet<String> = BTreeSet::new();
    notes.insert("times are when the launcher received each stream line; a request starts at the harness's status line `requesting` before it, else when the agent's previous step ended (the session start, the last tool result, the previous response); per-request tokens are the final usage of each response (message_delta)".into());
    notes.insert("per-request cost is not recorded by the stream: model spans carry cost_usd null and totals.cost_usd is the session result's total_cost_usd".into());
    let mut spans: Vec<Span> = vec![];
    let mut event_times = vec![];
    let start = meta["started_at"].as_str().and_then(epoch_ms);
    let stop = meta["ended_at"].as_str().and_then(epoch_ms);
    event_times.extend(start);
    event_times.extend(stop);
    // Requests by message id: start (ms), first token (ms), end (ms), model, usage at start,
    // output tokens at the end, and the tool calls the response asked for.
    struct Req {
        sent: i64,
        first: Option<i64>,
        end: Option<i64>,
        model: J,
        usage: J,
        output: Option<i64>,
        final_usage: J,
        tools: Vec<String>,
    }
    let mut reqs: Vec<(String, Req)> = vec![];
    let mut cur: Option<usize> = None;
    let mut last_step_end = start;
    let mut tool_open: BTreeMap<String, (i64, String)> = BTreeMap::new();
    let mut tools_seen: BTreeSet<String> = BTreeSet::new();
    let mut requesting: Option<i64> = None;
    let mut result = J::Null;
    let mut retries = 0;
    let mut sub_ignored = 0;
    for (i, e) in events.iter().enumerate() {
        let Some(t) = times[i] else { continue };
        event_times.push(t);
        if !e["parent_tool_use_id"].is_null() {
            sub_ignored += 1;
            continue;
        }
        match e["type"].as_str().unwrap_or("") {
            "system" if e["subtype"] == "init" => {
                if last_step_end.is_none() {
                    last_step_end = Some(t);
                }
            }
            "system" if e["subtype"] == "api_retry" => retries += 1,
            "system" if e["subtype"] == "status" && e["status"] == "requesting" => requesting = Some(t),
            "stream_event" => {
                let se = &e["event"];
                match se["type"].as_str().unwrap_or("") {
                    "message_start" => {
                        let m = &se["message"];
                        let sent = requesting.take().or(last_step_end).unwrap_or(t);
                        reqs.push((js(&m["id"]), Req { sent, first: Some(t), end: None, model: m["model"].clone(),
                            usage: m["usage"].clone(), output: None, final_usage: J::Null, tools: vec![] }));
                        cur = Some(reqs.len() - 1);
                    }
                    "message_delta" => {
                        if let Some(k) = cur {
                            if let Some(o) = se["usage"]["output_tokens"].as_i64() {
                                reqs[k].1.output = Some(o);
                            }
                            if se["usage"].is_object() {
                                reqs[k].1.final_usage = se["usage"].clone();
                            }
                        }
                    }
                    "message_stop" => {
                        if let Some(k) = cur.take() {
                            reqs[k].1.end = Some(t);
                            last_step_end = Some(t);
                        }
                    }
                    _ => {}
                }
            }
            "assistant" => {
                let mid = js(&e["message"]["id"]);
                for c in e["message"]["content"].as_array().into_iter().flatten().filter(|c| c["type"] == "tool_use") {
                    let tid = js(&c["id"]);
                    if tools_seen.insert(tid.clone()) {
                        tool_open.insert(tid, (t, js(&c["name"])));
                        if let Some((_, r)) = reqs.iter_mut().rev().find(|(id, _)| *id == mid) {
                            r.tools.push(js(&c["name"]));
                        }
                    }
                }
            }
            "user" => {
                for c in e["message"]["content"].as_array().into_iter().flatten().filter(|c| c["type"] == "tool_result") {
                    let tid = js(&c["tool_use_id"]);
                    if let Some((t0, name)) = tool_open.remove(&tid) {
                        let mut s = Span::new(t0, t, "tool");
                        s.action = Some(name);
                        s.extra.insert("is_error".into(), json!(c["is_error"] == true));
                        spans.push(s);
                        last_step_end = Some(last_step_end.map_or(t, |l| l.max(t)));
                    }
                }
            }
            "result" => result = e.clone(),
            _ => {}
        }
    }
    for (_, (t0, name)) in tool_open {
        let mut s = Span::new(t0, stop.unwrap_or(t0), "tool");
        s.action = Some(name);
        s.extra.insert("finished".into(), json!(false));
        notes.insert("a tool span with finished false: the tool call had no result when the session ended".into());
        spans.push(s);
    }
    if sub_ignored > 0 {
        notes.insert(format!("{sub_ignored} stream lines of subagents are not drawn"));
    }
    let mut no_end = 0;
    for (_, r) in reqs {
        let end = r.end.or(stop).unwrap_or(r.sent);
        if r.end.is_none() {
            no_end += 1;
        }
        let mut s = Span::new(r.sent, end, "model");
        s.action = Some("request".into());
        s.extra.insert("model".into(), r.model);
        s.extra.insert("effort".into(), meta["effort"].clone());
        s.extra.insert("api_requests".into(), json!(1));
        s.extra.insert("ttft_s".into(), json!(r.first.map(|f| secs(f - r.sent))));
        let mut tc: BTreeMap<String, i64> = BTreeMap::new();
        for n in r.tools {
            *tc.entry(n).or_default() += 1;
        }
        s.extra.insert("tool_calls".into(), json!(tc));
        // The final usage of the response (message_delta), else the start's input figures with output null.
        let tok = if r.final_usage["input_tokens"].is_i64() {
            tokens_of(&r.final_usage)
        } else {
            let mut t = tokens_of(&r.usage);
            if t.is_object() {
                t["output"] = json!(r.output);
            }
            t
        };
        s.extra.insert("tokens".into(), tok);
        s.extra.insert("cost_usd".into(), J::Null);
        spans.push(s);
    }
    if no_end > 0 {
        notes.insert(format!("{no_end} API requests had no message_stop: drawn up to the session's end"));
    }
    let mut model_usage = Map::new();
    for (m, u) in result["modelUsage"].as_object().into_iter().flatten() {
        model_usage.insert(m.clone(), json!({"input": u["inputTokens"], "output": u["outputTokens"], "cache_read": u["cacheReadInputTokens"],
            "cache_write": u["cacheCreationInputTokens"], "cost_usd": u["costUSD"]}));
    }
    let session = json!({
        "cost_usd": result["total_cost_usd"].clone(),
        "num_turns": result["num_turns"].clone(),
        "subtype": result["subtype"].clone(),
        "is_error": result["is_error"].clone(),
        "duration_ms": result["duration_ms"].clone(),
        "duration_api_ms": result["duration_api_ms"].clone(),
        "usage": tokens_of(&result["usage"]),
        "model_usage": model_usage,
        "api_retries": retries,
    });
    if result.is_null() {
        notes.insert("the stream has no result line: the session's cost and usage are not recorded".into());
    }
    let settings = json!({"model": meta["model"], "effort": meta["effort"], "adapter": meta["adapter"],
        "tools": meta["tools"], "max_budget_usd": meta["caps"]["max_usd"], "max_wall_s": meta["caps"]["max_wall_s"],
        "confinement": meta["confinement"]["summary"]});
    let end = json!({"state": meta["end"], "exit": meta["exit"], "capped": meta["capped"]});
    let mut doc = finish("free-agent", name, settings, J::Null, spans, &event_times, None, end, notes, malformed, Some(session))?;
    // Tokens the session result counts beyond what its streamed requests hold (requests the
    // harness makes outside the stream).
    let spans_tok = &doc["totals"]["tokens"];
    let sess = &doc["session"]["usage"];
    if sess.is_object() && spans_tok.is_object() {
        let diff: Map<String, J> = TOKEN_KINDS.iter().map(|k| {
            let v = match (sess[*k].as_i64(), spans_tok[*k].as_i64()) {
                (Some(a), Some(b)) => json!(a - b),
                _ => J::Null,
            };
            (k.to_string(), v)
        }).collect();
        doc["session"]["usage_beyond_streamed_requests"] = J::Object(diff);
    }
    Ok(doc)
}

// ---- entry ------------------------------------------------------------------------------------

/// `sdc.run-timeline/1` of the run directory `dir`.
pub fn load(dir: &Path) -> Res<J> {
    let name = std::fs::canonicalize(dir).ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .or_else(|| dir.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "run".to_string());
    if let Some(meta) = rm::json_opt(&dir.join("free-agent.json"))? {
        return load_free(dir, &name, meta);
    }
    let kind = run_kind::detect(dir).map_err(Fail)?;
    load_sdc(dir, &name, kind)
}

/// A number as a record writes it: `25`, not `25.0`.
fn num_str(v: &J) -> String {
    match v.as_f64() {
        Some(x) if x.fract() == 0.0 && x.abs() < 1e15 => format!("{}", x as i64),
        Some(x) => format!("{x}"),
        None => js(v),
    }
}

/// The metrics of a free-running agent's run in the shape of `sdc.run-metrics/1` (the run fields
/// `sdc-experiment` compares: `metrics.cost_usd`, `wall_s`, `tokens`, `closed`, `delivered`,
/// `clean`, `finished`), from its timeline, `free-agent.json` and the measured `outcome.json`.
/// None for a directory that is no free-agent run. A run is finished once it has ended and its
/// outcome is measured.
pub fn free_metrics(dir: &Path) -> Res<Option<J>> {
    let Some(meta) = rm::json_opt(&dir.join("free-agent.json"))? else { return Ok(None) };
    let t = load(dir)?;
    let outcome = rm::json_opt(&dir.join("outcome.json"))?;
    let tot = &t["totals"];
    let ended = meta["end"] != "running";
    let o = outcome.clone().unwrap_or(J::Null);
    let closed = o["ended_by_itself"] == true;
    let delivered = o["delivered"] == true;
    let capped = meta["capped"].clone();
    let end = match (meta["end"].as_str().unwrap_or(""), capped.as_str()) {
        ("capped", Some(c)) => format!("capped: {c}"),
        (e, _) => e.to_string(),
    };
    let tk = &tot["tokens"];
    let mut settings = json!({"free.model": js(&meta["model"]), "free.effort": js(&meta["effort"]), "free.adapter": js(&meta["adapter"]),
        "free.max_usd": num_str(&meta["caps"]["max_usd"]), "free.max_wall_s": num_str(&meta["caps"]["max_wall_s"]),
        "free.max_turns": num_str(&meta["caps"]["max_turns"])});
    // A run that named no money cap records none: the setting is left out, not reported as null.
    if meta["caps"]["max_usd"].is_null() {
        settings.as_object_mut().map(|o| o.remove("free.max_usd"));
    }
    let metrics = json!({
        "end": end, "end_kind": if !ended { "open" } else if closed { "closed" } else { "stopped" },
        "closed": closed, "outcome": if delivered { json!("delivered") } else { json!("not delivered") },
        "delivered": delivered, "clean": delivered && capped.is_null(), "finished": ended && outcome.is_some(),
        "capped": capped, "reopens": 0, "forced": 0, "unstuck": 0,
        "cost_usd": tot["cost_usd"], "llm_calls": null, "llm_cost_usd": tot["cost_usd"], "decider_calls": 0, "decider_cost_usd": 0.0,
        "api_requests": tot["api_requests"], "tool_calls": tot["model_tool_calls"], "tool_calls_by_name": tot["model_tool_calls_by_name"],
        "tokens": {"input": tk["input"], "output": tk["output"], "cache_read": tk["cache_read"], "cache_creation": tk["cache_write"]},
        "session_usage": t["session"]["usage"], "usage_beyond_streamed_requests": t["session"]["usage_beyond_streamed_requests"],
        "wall_s": tot["wall_s"], "model_s": tot["model_s"], "tool_s": tot["tool_s"], "idle_s": tot["idle_s"], "waiting_on_person_s": 0,
    });
    Ok(Some(json!({
        "format": "sdc.free-agent-metrics/1", "run": t["run"], "settings": settings, "metrics": metrics,
        "outcome": o, "stages": [], "nodes": [], "edges": [], "decisions": {"by_class": {}, "by_action": {}},
        "records": {"free_agent": true, "outcome": outcome.is_some()},
    })))
}

fn opt(v: &J) -> String {
    if v.is_null() { "-".into() } else { js(v) }
}

/// The totals line of a document.
pub fn text(doc: &J) -> String {
    let t = &doc["totals"];
    let k = &t["tokens"];
    let mut s = String::new();
    let _ = write!(s, "{} ({}): wall {} s, cost {} USD, tokens in {} out {} cache-read {} cache-write {}; ",
        js(&doc["run"]), js(&doc["kind"]), opt(&t["wall_s"]), opt(&t["cost_usd"]), opt(&k["input"]), opt(&k["output"]), opt(&k["cache_read"]), opt(&k["cache_write"]));
    if doc["kind"] == "free-agent" {
        let _ = write!(s, "{} API requests ({} s), {} tool calls ({} s), idle {} s",
            opt(&t["api_requests"]), opt(&t["model_s"]), opt(&t["model_tool_calls"]), opt(&t["tool_s"]), opt(&t["idle_s"]));
    } else {
        let idle: Vec<String> = t["idle_by_cause"].as_object().into_iter().flatten().map(|(k, v)| format!("{k} {v}")).collect();
        let _ = write!(s, "{} program steps ({} s), {} model sessions ({} s, {} API requests incl. decider, {} tool calls), {} decider calls ({} s), {} rule steps, idle {} s ({}), waiting on a person {} s",
            opt(&t["program_steps"]), opt(&t["program_s"]), opt(&t["model_sessions"]), opt(&t["model_s"]), opt(&t["api_requests"]),
            opt(&t["model_tool_calls"]), opt(&t["decider_calls"]), opt(&t["decider_s"]), opt(&t["rule_steps"]), opt(&t["idle_s"]), idle.join(", "),
            opt(&t["waiting_on_person_s"]));
    }
    s.push('\n');
    s
}
