//! One comparison report over two or more recorded runs (docs/run-compare.md).
//!
//! ```text
//! sdc-run-compare --run <label>=<run dir> [--run <label>=<run dir> ...] --out <report.md> [--json <report.json>]
//! ```
//!
//! A run directory is what `scripts/run-trial.sh` writes: `store/IDEA-1/` (the case tree: every
//! case's `events.jsonl` and `observed.jsonl`, `work/` with `findings.yaml`, `questions.yaml`,
//! `context.yaml`), `rounds.jsonl`, `run-settings.jsonl`, `llm-calls.jsonl`, `decisions.jsonl`,
//! and the records that only some runs have: `run-stopped.json`, `replay.json`, `oracle.json`.
//! Only `store/IDEA-1/events.jsonl` is required. A record that is absent reads "not recorded"
//! (or "none" for the oracle check); a record that is present and malformed is an error.
//!
//! Deterministic: the same run directories give the same bytes. The report names a run directory
//! by its own name (never a path) and carries no clock reading. Exit 0 report written, 2 a run
//! directory cannot be read (the message names the file), 64 usage.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{json, Value as J};
use serde_yaml::Value as Y;

const USAGE: &str = "usage: sdc-run-compare --run <label>=<run dir> [--run <label>=<run dir> ...] --out <report.md> [--json <report.json>]";
const NOT_RECORDED: &str = "not recorded";

/// A run directory that cannot be read: the message names the file.
struct Fail(String);

type Res<T> = Result<T, Fail>;

// ---- reading -----------------------------------------------------------------------------------

/// The text of a file, or None when it does not exist; any other failure names the file.
fn read_opt(path: &Path) -> Res<Option<String>> {
    match fs::read_to_string(path) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Fail(format!("cannot read {}: {e}", path.display()))),
    }
}

fn jsonl_opt(path: &Path) -> Res<Option<Vec<J>>> {
    let Some(text) = read_opt(path)? else { return Ok(None) };
    let mut out = vec![];
    for (i, line) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        match serde_json::from_str::<J>(line) {
            Ok(v) => out.push(v),
            Err(e) => return Err(Fail(format!("cannot read {}:{}: {e}", path.display(), i + 1))),
        }
    }
    Ok(Some(out))
}

fn json_opt(path: &Path) -> Res<Option<J>> {
    let Some(text) = read_opt(path)? else { return Ok(None) };
    serde_json::from_str(&text).map(Some).map_err(|e| Fail(format!("cannot read {}: {e}", path.display())))
}

fn yaml_opt(path: &Path) -> Res<Option<Y>> {
    let Some(text) = read_opt(path)? else { return Ok(None) };
    serde_yaml::from_str(&text).map(Some).map_err(|e| Fail(format!("cannot read {}: {e}", path.display())))
}

fn st<'a>(v: &'a Y, k: &str) -> &'a str {
    v.get(k).and_then(Y::as_str).unwrap_or("")
}

fn seq(v: &Y, k: &str) -> Vec<Y> {
    v.get(k).and_then(Y::as_sequence).cloned().unwrap_or_default()
}

fn js(v: &J) -> String {
    v.as_str().map(String::from).unwrap_or_else(|| if v.is_null() { String::new() } else { v.to_string() })
}

/// Milliseconds since the epoch of an RFC 3339 UTC instant `YYYY-MM-DDTHH:MM:SS[.fff]Z`.
fn epoch_ms(t: &str) -> Option<i64> {
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
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 86_400 + hh * 3600 + mm * 60 + ss) * 1000) + frac)
}

/// A person: a principal that is not an agent, a decider, a rule, a program or a dry run.
/// `person:<id>` is one, and so is a bare id or `human` as older drivers wrote them
/// (docs/kpis.md "Late findings and human interaction").
fn person(p: &str) -> bool {
    !p.is_empty()
        && !["decider:", "rule:", "program", "dry-run", "agent"].iter().any(|x| p.starts_with(x))
        && p != "non-interactive"
        && p != "derived"
}

/// Every case directory of the tree at `root`: the root, then each `children/<id>` below it.
fn cases(root: &Path, out: &mut Vec<PathBuf>) {
    if root.join("events.jsonl").is_file() {
        out.push(root.to_path_buf());
    }
    let Ok(dir) = fs::read_dir(root.join("children")) else { return };
    let mut subs: Vec<PathBuf> = dir.flatten().filter(|e| e.path().is_dir()).map(|e| e.path()).collect();
    subs.sort();
    for d in subs {
        cases(&d, out);
    }
}

// ---- a run's facts -----------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Fmt {
    Int,
    Usd,
}

#[derive(Clone, PartialEq)]
enum Cell {
    Num(f64, Fmt),
    Text(String),
    Missing,
}

fn int(n: usize) -> Cell {
    Cell::Num(n as f64, Fmt::Int)
}

fn usd(x: f64) -> Cell {
    Cell::Num((x * 1e6).round() / 1e6, Fmt::Usd)
}

fn text(s: impl Into<String>) -> Cell {
    Cell::Text(s.into())
}

struct Round {
    start_s: i64,
    end_s: i64,
    seconds: f64,
    verdict: String,
    case_id: String,
}

struct Run {
    label: String,
    name: String,
    /// The fixed rows, in report order.
    metrics: Vec<(String, Cell)>,
    /// Calls and cost per model and per step kind, when `llm-calls.jsonl` is recorded.
    by_model: Option<BTreeMap<String, Tally>>,
    by_step: Option<BTreeMap<String, Tally>>,
    details: J,
}

/// Calls and cost per key.
#[derive(Default, Clone)]
struct Tally {
    calls: usize,
    cost: f64,
}

fn tally(map: &mut BTreeMap<String, Tally>, key: &str, cost: f64) {
    let t = map.entry(key.to_string()).or_default();
    t.calls += 1;
    t.cost += cost;
}

fn load(label: &str, dir: &Path) -> Res<Run> {
    let root = dir.join("store").join("IDEA-1");
    let events_path = root.join("events.jsonl");
    if !events_path.is_file() {
        return Err(Fail(format!("cannot read {}: no such file", events_path.display())));
    }
    let name = fs::canonicalize(dir).ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .or_else(|| dir.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "run".to_string());
    let mut metrics: Vec<(String, Cell)> = vec![];
    let mut details = serde_json::Map::new();

    // The case tree: person actions, revisions, the root's close, the span of event times.
    let mut dirs = vec![];
    cases(&root, &mut dirs);
    let (mut acts, mut grants) = (vec![], vec![]);
    let mut revs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut person_principals: BTreeMap<String, usize> = BTreeMap::new();
    let (mut first_ms, mut last_ms): (Option<i64>, Option<i64>) = (None, None);
    let mut span = |t: Option<i64>| {
        if let Some(t) = t {
            first_ms = Some(first_ms.map_or(t, |f| f.min(t)));
            last_ms = Some(last_ms.map_or(t, |l| l.max(t)));
        }
    };
    let mut closed: Option<String> = None;
    for c in &dirs {
        let cid = c.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        for e in jsonl_opt(&c.join("events.jsonl"))?.unwrap_or_default() {
            let (principal, at) = (js(&e["principal"]), js(&e["at"]));
            span(epoch_ms(&at));
            match e["event"].as_str().unwrap_or("") {
                "action_finished" if person(&principal) => {
                    *person_principals.entry(principal.clone()).or_default() += 1;
                    acts.push((cid.clone(), at, principal));
                }
                "authority_recorded" if person(&principal) => {
                    *person_principals.entry(principal.clone()).or_default() += 1;
                    grants.push((cid.clone(), at, principal));
                }
                "revised" => {
                    for (a, r) in e["artifacts"].as_object().into_iter().flatten() {
                        let r = js(r);
                        if r != "none" {
                            revs.entry(a.clone()).or_default().insert(r);
                        }
                    }
                }
                "terminated" if *c == root => closed = Some(js(&e["outcome"])),
                _ => {}
            }
        }
        for o in jsonl_opt(&c.join("observed.jsonl"))?.unwrap_or_default() {
            span(o["observed_at"].as_str().and_then(epoch_ms));
        }
    }
    let person_actions = acts.len() + grants.iter().filter(|g| !acts.contains(g)).count();
    let elapsed = match (first_ms, last_ms) {
        (Some(f), Some(l)) => Some((l - f).div_euclid(1000)),
        _ => None,
    };

    // The driver's rounds (scripts/run-trial.sh `rounds.jsonl`).
    let rounds: Option<Vec<Round>> = jsonl_opt(&dir.join("rounds.jsonl"))?.map(|v| {
        v.iter().map(|r| Round {
            start_s: r["started_epoch"].as_i64().unwrap_or(0),
            end_s: r["ended_epoch"].as_i64().unwrap_or(0),
            seconds: r["seconds"].as_f64().unwrap_or(0.0),
            verdict: js(&r["verdict"]),
            case_id: js(&r["case_dir"]).rsplit('/').next().unwrap_or("").to_string(),
        }).collect()
    });
    let settings = jsonl_opt(&dir.join("run-settings.jsonl"))?;
    let stopped = json_opt(&dir.join("run-stopped.json"))?;
    let driver_s = rounds.as_ref().map(|r| r.iter().map(|x| x.seconds).sum::<f64>().round());

    // End state.
    let last_round = rounds.as_ref().and_then(|r| r.last());
    let end_state = if let Some(outcome) = &closed {
        format!("closed: {outcome}")
    } else if let Some(s) = &stopped {
        let verdict = Some(js(&s["verdict_at_stop"])).filter(|v| !v.is_empty())
            .or_else(|| last_round.map(|r| r.verdict.clone())).unwrap_or_else(|| "stopped".into());
        let case = Some(js(&s["case"])).filter(|c| !c.is_empty()).or_else(|| last_round.map(|r| r.case_id.clone()));
        let mut t = format!("stopped: {verdict}");
        if let Some(c) = case.filter(|c| !c.is_empty()) {
            let _ = write!(t, " in {c}");
            // a stop record that names another case than the last round's: show both
            if let Some(r) = last_round.filter(|r| !r.case_id.is_empty() && r.case_id != c) {
                let _ = write!(t, " (last round ended in {})", r.case_id);
            }
        }
        let by = js(&s["by"]);
        let reason = js(&s["reason"]);
        match (by.is_empty(), reason.is_empty()) {
            (false, false) => { let _ = write!(t, ", by {by}: {reason}"); }
            (false, true) => { let _ = write!(t, ", by {by}"); }
            (true, false) => { let _ = write!(t, ": {reason}"); }
            _ => {}
        }
        t
    } else if let Some(r) = last_round {
        format!("stopped: {} in {}", r.verdict, r.case_id)
    } else {
        NOT_RECORDED.to_string()
    };
    metrics.push(("end state".into(), text(end_state)));

    // Time: elapsed first event to last event; inside the driver; outside; waiting on a person.
    let settings_s: Vec<i64> = settings.iter().flatten().filter_map(|s| s["at"].as_str().and_then(epoch_ms)).map(|t| t.div_euclid(1000)).collect();
    let stopped_s = stopped.as_ref().and_then(|s| s["at"].as_str()).and_then(epoch_ms).map(|t| t.div_euclid(1000));
    // Stop records a later leg reopened (run-stopped.<at>.json): time stopped for a fix.
    let mut reopened_names: Vec<String> = fs::read_dir(dir).into_iter().flatten().flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("run-stopped.") && n.ends_with(".json") && n != "run-stopped.json").collect();
    reopened_names.sort();
    let mut reopened_s = vec![];
    for n in &reopened_names {
        if let Some(at) = json_opt(&dir.join(n))?.and_then(|s| s["at"].as_str().and_then(epoch_ms)) {
            reopened_s.push(at.div_euclid(1000));
        }
    }
    let (wait, stops, open_end) = match &rounds {
        Some(r) => waiting(r, &settings_s, stopped_s, closed.is_some(), &reopened_s),
        None => (None, vec![], false),
    };
    let fix_s: i64 = rounds.iter().flatten().zip(rounds.iter().flatten().skip(1)).filter_map(|(r, n)| fix_gap(r, n, &reopened_s)).sum();
    let secs = |x: Option<i64>| x.map_or(Cell::Missing, |v| Cell::Num(v as f64, Fmt::Int));
    metrics.push(("elapsed wall time (s)".into(), secs(elapsed)));
    metrics.push(("time inside the driver (s)".into(), driver_s.map_or(Cell::Missing, |v| Cell::Num(v, Fmt::Int))));
    metrics.push(("time outside the driver (s)".into(), match (elapsed, driver_s) {
        (Some(e), Some(d)) => Cell::Num((e as f64 - d).max(0.0), Fmt::Int),
        _ => Cell::Missing,
    }));
    metrics.push(("time waiting on a person (s)".into(), secs(wait)));
    metrics.push(("time stopped for a fix (s)".into(), rounds.as_ref().map_or(Cell::Missing, |_| int(fix_s as usize))));
    details.insert("waiting_on_a_person".into(), json!({"stops": stops, "open_end_not_counted": open_end}));

    // Decisions by a person: person actions (an authority record taken with an action counts
    // once), answered questions, finding decisions, context fields a person changed.
    let work = root.join("work");
    let questions = yaml_opt(&work.join("questions.yaml"))?;
    let findings = yaml_opt(&work.join("findings.yaml"))?;
    let context = yaml_opt(&work.join("context.yaml"))?;
    let q_by_person = questions.as_ref().map_or(0, |q| {
        seq(q, "questions").iter().filter(|q| person(st(q, "answered_by")) && matches!(st(q, "status"), "accepted" | "changed" | "superseded")).count()
    });
    let f_by_person = findings.as_ref().map_or(0, |f| {
        seq(f, "findings").iter().filter(|f| st(f, "status") == "resolved" && person(st(f, "decided_by"))).count()
    });
    let ctx_changed = context.as_ref().and_then(Y::as_mapping)
        .map_or(0, |m| m.values().filter(|v| v.is_mapping() && st(v, "status") == "changed").count());
    metrics.push(("decisions by a person".into(), int(person_actions + q_by_person + f_by_person + ctx_changed)));
    details.insert("decisions_by_a_person".into(), json!({"actions": person_actions, "questions": q_by_person,
        "findings": f_by_person, "context_fields": ctx_changed, "principals": person_principals}));

    // LLM calls and cost (llm-calls.jsonl), per model and per step kind (`action`).
    let calls = jsonl_opt(&dir.join("llm-calls.jsonl"))?;
    let (mut by_model, mut by_step): (BTreeMap<String, Tally>, BTreeMap<String, Tally>) = Default::default();
    if let Some(c) = &calls {
        for l in c {
            let cost = l["cost_usd"].as_f64().unwrap_or(0.0);
            tally(&mut by_model, Some(js(&l["model"])).filter(|m| !m.is_empty()).as_deref().unwrap_or("unknown"), cost);
            tally(&mut by_step, Some(js(&l["action"])).filter(|m| !m.is_empty()).as_deref().unwrap_or("unknown"), cost);
        }
    }
    let llm_cost: f64 = by_model.values().map(|t| t.cost).sum();
    metrics.push(("LLM calls".into(), calls.as_ref().map_or(Cell::Missing, |c| int(c.len()))));
    metrics.push(("LLM cost (USD)".into(), calls.as_ref().map_or(Cell::Missing, |_| usd(llm_cost))));

    // Decider calls and cost (decisions.jsonl): every decider model call that was made, whatever
    // became of its reply, once. A call is its record (`record`, decider/NNN-*); several lines
    // can share one: an escalation suppressed afterwards carries the call's cost on its
    // `escalation_suppressed` line and none on the decision line after it, and a fallback line
    // repeats the last invalid reply's call. A line without a record is a call of its own. Rule
    // deciders (`decider:approve`, `decider:reject`) name no model.
    let decisions = jsonl_opt(&dir.join("decisions.jsonl"))?;
    let mut decider_by_model: BTreeMap<String, Tally> = BTreeMap::new();
    let mut decider_by_principal: BTreeMap<String, usize> = BTreeMap::new();
    if let Some(d) = &decisions {
        let mut by_call: BTreeMap<String, (String, f64)> = BTreeMap::new();
        for (i, l) in d.iter().enumerate() {
            *decider_by_principal.entry(js(&l["principal"])).or_default() += 1;
            let model = js(&l["model"]);
            if model.is_empty() {
                continue;
            }
            let key = l["record"].as_str().map_or_else(|| format!("line {i}"), str::to_string);
            let cost = l["cost_usd"].as_f64().unwrap_or(0.0);
            let e = by_call.entry(key).or_insert((model, 0.0));
            e.1 = e.1.max(cost);
        }
        for (model, cost) in by_call.into_values() {
            tally(&mut decider_by_model, &model, cost);
        }
    }
    let decider_calls: usize = decider_by_model.values().map(|t| t.calls).sum();
    let decider_cost: f64 = decider_by_model.values().map(|t| t.cost).sum();
    metrics.push(("decider calls".into(), decisions.as_ref().map_or(Cell::Missing, |_| int(decider_calls))));
    metrics.push(("decider cost (USD)".into(), decisions.as_ref().map_or(Cell::Missing, |_| usd(decider_cost))));
    metrics.push(("total cost (USD)".into(), match (&calls, &decisions) {
        (None, None) => Cell::Missing,
        _ => usd(llm_cost + decider_cost),
    }));
    details.insert("decider".into(), json!({"decisions_by_principal": decider_by_principal,
        "calls_by_model": decider_by_model.iter().map(|(m, t)| (m.clone(), json!({"calls": t.calls, "cost_usd": (t.cost * 1e6).round() / 1e6}))).collect::<BTreeMap<_, _>>()}));

    // Findings (work/findings.yaml, as docs/kpis.md L3-L5): raised = not rejected; late = raised
    // after intake (stage not `requirements`); resolution by `decided_by`.
    let findings_known = findings.is_some() || work.is_dir();
    let fl = findings.as_ref().map(|f| seq(f, "findings")).unwrap_or_default();
    let raised: Vec<&Y> = fl.iter().filter(|f| st(f, "status") != "rejected").collect();
    let late = raised.iter().filter(|f| st(f, "stage") != "requirements").count();
    let resolved = |f: &&&Y| st(f, "status") == "resolved";
    let by_decider = raised.iter().filter(|f| resolved(f) && st(f, "decided_by").starts_with("decider:")).count();
    let by_person = raised.iter().filter(|f| resolved(f) && person(st(f, "decided_by"))).count();
    let by_agent = raised.iter().filter(|f| resolved(f)).count() - by_decider - by_person;
    let dismissed = raised.iter().filter(|f| st(f, "status") == "dismissed").count();
    let open = raised.len() - by_decider - by_person - by_agent - dismissed;
    let f = |n: usize| if findings_known { int(n) } else { Cell::Missing };
    metrics.push(("findings raised".into(), f(raised.len())));
    metrics.push(("findings late".into(), f(late)));
    metrics.push(("findings early".into(), f(raised.len() - late)));
    metrics.push(("findings resolved by agent default".into(), f(by_agent)));
    metrics.push(("findings resolved by decider".into(), f(by_decider)));
    metrics.push(("findings resolved by person".into(), f(by_person)));
    metrics.push(("findings dismissed".into(), f(dismissed)));
    metrics.push(("findings unresolved".into(), f(open)));
    metrics.push(("findings rejected by the program check".into(), f(fl.len() - raised.len())));

    // Rework: revisions after the first, per artifact, over the whole tree (docs/kpis.md L6).
    let rework: usize = revs.values().map(|v| v.len() - 1).sum();
    metrics.push(("rework revisions".into(), int(rework)));
    details.insert("rework_by_artifact".into(), json!(revs.iter().map(|(a, v)| (a.clone(), v.len() - 1)).collect::<BTreeMap<_, _>>()));

    // The oracle check (docs/eval-design.md, `oracle.json`) and the replay record.
    let oracle = match json_opt(&dir.join("oracle.json"))? {
        Some(o) => Some(o),
        None => json_opt(&dir.join("store").join("oracle.json"))?,
    };
    metrics.push(("oracle check".into(), text(oracle.as_ref().map_or("none".to_string(), oracle_summary))));
    // The conformance observation after close (docs/conformance.md):
    // `k passed, f failed, d disputed, u not measurable of n`.
    let conformance = oracle.as_ref().map(|o| &o["conformance"]).filter(|c| c["result"].is_string());
    metrics.push(("conformance".into(), text(conformance.map_or("not observed".to_string(), conformance_cell))));
    let replay = json_opt(&dir.join("replay.json"))?;
    metrics.push(("replay".into(), text(replay.as_ref().map_or(NOT_RECORDED.to_string(), replay_summary))));

    metrics.push(("setting".into(), text(settings.as_ref().and_then(|s| s.last()).map_or(NOT_RECORDED.to_string(), setting_summary))));

    // Keep-going (D142, defects.jsonl): continuations forced by rule:keep-going, and the defects.
    let defects = jsonl_opt(&dir.join("defects.jsonl"))?;
    let forced = defects.iter().flatten().filter(|d| d["forced"] == json!(true)).count();
    metrics.push(("forced continuations".into(), defects.as_ref().map_or(Cell::Missing, |_| int(forced))));
    // Unstick (D147): the moves run.unstick took (stop is none), each a diagnosed defect.
    let unstuck = defects.iter().flatten().filter(|d| d["unstuck"] == json!(true)).count();
    metrics.push(("unstuck".into(), defects.as_ref().map_or(Cell::Missing, |_| int(unstuck))));
    metrics.push(("reopens".into(), int(reopened_names.len())));
    metrics.push(("clean run".into(), text(if forced == 0 && unstuck == 0 && reopened_names.is_empty() { "yes" } else { "no" }.to_string())));
    metrics.push(("defects".into(), text(defects.as_ref().map_or(NOT_RECORDED.to_string(), |d| {
        if d.is_empty() {
            return "none".to_string();
        }
        d.iter().map(|x| {
            let how = if x["forced"] == json!(true) {
                "forced".to_string()
            } else if x["unstuck"] == json!(true) {
                format!("unstuck: {}, {}: {}", js(&x["move"]), js(&x["defect_class"]), js(&x["diagnosis"]))
            } else {
                "stops".to_string()
            };
            format!("{} at {} in {} ({how})", js(&x["cause"]), js(&x["action"]), js(&x["case"]))
        }).collect::<Vec<_>>().join("; ")
    }))));

    let recorded = calls.is_some();
    Ok(Run {
        label: label.to_string(),
        name,
        metrics,
        by_model: recorded.then_some(by_model),
        by_step: recorded.then_some(by_step),
        details: J::Object(details),
    })
}

/// Seconds the run spent waiting on a person, with the stops that caused it. A stop waits on a
/// person when the run did not go on by itself: for a round that starts after the first
/// `run-settings.jsonl` line, a leg line lies between its end and the next round's start (the
/// script had ended and was started again); for an older round, its verdict is `awaiting_person`
/// or `awaiting_authority`. The last round counts when the run was not closed and a stop record
/// (`run-stopped.json`) gives its end; with no stop record its end is open and it is not counted
/// (no clock reading enters the report). Same reading as L7 of `sdc-finding-kpis`.
fn waiting(rounds: &[Round], legs: &[i64], stopped: Option<i64>, closed: bool, reopened: &[i64]) -> (Option<i64>, Vec<J>, bool) {
    let first_leg = legs.iter().min().copied();
    let mut stops = vec![];
    let mut open_end = false;
    for (i, r) in rounds.iter().enumerate() {
        let person_verdict = matches!(r.verdict.as_str(), "awaiting_person" | "awaiting_authority");
        let legacy = first_leg.is_none_or(|l| r.start_s < l);
        let (until, how) = match rounds.get(i + 1) {
            Some(n) => {
                let waits = if legacy { person_verdict } else { legs.iter().any(|&l| l >= r.end_s && l <= n.start_s) };
                // A gap a reopened stop record lies in is stopped for a fix (`fix_gaps`), no wait.
                if !waits || fix_gap(r, n, reopened).is_some() {
                    continue;
                }
                (n.start_s, "resumed")
            }
            None if closed || !(person_verdict || !legacy) => continue,
            None => match stopped {
                Some(at) => (at, "stopped"),
                None => {
                    open_end = true;
                    continue;
                }
            },
        };
        stops.push(json!({"verdict": r.verdict, "case": r.case_id, "until": how, "seconds": (until - r.end_s).max(0)}));
    }
    let total: i64 = stops.iter().filter_map(|s| s["seconds"].as_i64()).sum();
    (Some(total), stops, open_end)
}

/// The seconds from a reopened stop record (`run-stopped.<at>.json`) that lies between round `r`
/// and the next round `n` to `n`'s start: the time the run was stopped for a fix.
fn fix_gap(r: &Round, n: &Round, reopened: &[i64]) -> Option<i64> {
    reopened.iter().find(|&&at| at >= r.end_s - 5 && at <= n.start_s).map(|&at| (n.start_s - at).max(0))
}

/// `held`, `contradicted` or `unknown` per check (docs/eval-design.md, oracle file): the checks
/// are an array of `{id, result}`, a map id -> result, or the file holds one `result`.
/// The figure of `sdc-conform` (`docs/conformance.md`): `k passed, f failed, u not measurable of
/// n; t tables agree on a of n (held|contradicted)` (`; 1 table` for one), with
/// `, unknown: <cause>` for a measured unknown; `not measured (<cause>), n scenarios` when no
/// table was counted.
fn conformance_cell(c: &J) -> String {
    let cause = c["cause"].as_str().unwrap_or("no cause");
    let n = c["n"].as_u64();
    let (Some(k), Some(f), Some(u), Some(n)) = (c["k"].as_u64(), c["f"].as_u64(), c["u"].as_u64(), n) else {
        return match n {
            Some(n) => format!("not measured ({cause}), {n} scenarios"),
            None => format!("not measured ({cause})"),
        };
    };
    let d = c["d"].as_u64().unwrap_or(0);
    let mut text = format!("{k} passed, {f} failed, {d} disputed, {u} not measurable of {n}");
    match (c["tables_counted"].as_u64(), c["agreement"].as_u64()) {
        (Some(1), _) => text.push_str("; 1 table"),
        (Some(t), Some(a)) => text.push_str(&format!("; {t} tables agree on {a} of {n}")),
        _ => {}
    }
    match c["result"].as_str() {
        Some(r @ ("held" | "contradicted")) => text.push_str(&format!(" ({r})")),
        _ => text.push_str(&format!(", unknown: {cause}")),
    }
    text
}

fn oracle_summary(o: &J) -> String {
    let one = |v: &J| -> String {
        let r = v.as_str().map(String::from).unwrap_or_else(|| ["result", "status", "verdict"].iter().find_map(|k| v[*k].as_str()).unwrap_or("").to_string());
        match r.as_str() { "held" | "contradicted" => r, _ => "unknown".to_string() }
    };
    let results: Vec<String> = match &o["checks"] {
        J::Array(a) => a.iter().map(one).collect(),
        J::Object(m) => m.values().map(one).collect(),
        _ => vec![one(o)],
    };
    let n = |k: &str| results.iter().filter(|r| *r == k).count();
    let (held, contradicted, unknown) = (n("held"), n("contradicted"), n("unknown"));
    if results.is_empty() {
        "none".to_string()
    } else if held == results.len() {
        format!("held ({held} of {} checks)", results.len())
    } else {
        let overall = if contradicted > 0 { "contradicted" } else { "unknown" };
        format!("{overall} (held {held}, contradicted {contradicted}, unknown {unknown})")
    }
}

/// `replay.json`: one entry per case with `state_matches_log` and `decision_matches_log`.
fn replay_summary(r: &J) -> String {
    let Some(a) = r.as_array() else { return "not recognised".to_string() };
    let ok = a.iter().filter(|c| c["state_matches_log"] == true && c["decision_matches_log"] == true).count();
    format!("{ok} of {} cases match", a.len())
}

/// Who held the decisions, and the models and reasoning efforts the settings record names.
fn setting_summary(s: &J) -> String {
    let mut p: Vec<String> = vec![];
    if let Some(a) = s["autonomy"].as_str() {
        p.push(format!("autonomy {a}"));
    }
    if s["stand_in"] == true {
        p.push("stand-in".into());
    }
    if let Some(k) = s["decider"]["kind"].as_str() {
        p.push(match s["decider"]["model"].as_str() {
            Some(m) => format!("decider {k} ({m})"),
            None => format!("decider {k}"),
        });
    }
    for key in ["models", "efforts", "model"] {
        if !s[key].is_null() {
            p.push(format!("{key} {}", s[key].as_str().map(String::from).unwrap_or_else(|| s[key].to_string())));
        }
    }
    if p.is_empty() { NOT_RECORDED.to_string() } else { p.join("; ") }
}

// ---- the report --------------------------------------------------------------------------------

fn render(c: &Cell) -> String {
    match c {
        Cell::Num(x, Fmt::Int) => if x.fract() == 0.0 { format!("{}", *x as i64) } else { format!("{x:.1}") },
        Cell::Num(x, Fmt::Usd) => format!("{x:.3}"),
        Cell::Text(t) => t.clone(),
        Cell::Missing => NOT_RECORDED.to_string(),
    }
}

/// `b - a` for two cells: a signed number, `same` or `differs` for text, `n/a` when one side is
/// not recorded.
fn difference(a: &Cell, b: &Cell) -> Option<String> {
    match (a, b) {
        (Cell::Num(x, f), Cell::Num(y, _)) => {
            let d = y - x;
            Some(match f {
                Fmt::Int if d.fract() == 0.0 => format!("{}{}", if d > 0.0 { "+" } else { "" }, d as i64),
                Fmt::Int => format!("{}{d:.1}", if d > 0.0 { "+" } else { "" }),
                Fmt::Usd => format!("{}{d:.3}", if d > 0.0 { "+" } else { "" }),
            })
        }
        (Cell::Text(x), Cell::Text(y)) => Some(if x == y { "same" } else { "differs" }.to_string()),
        (Cell::Missing, Cell::Missing) => Some("same".to_string()),
        _ => None,
    }
}

fn cell_md(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn cell_json(c: &Cell) -> J {
    match c {
        Cell::Num(x, Fmt::Int) if x.fract() == 0.0 => json!(*x as i64),
        Cell::Num(x, _) => json!(x),
        Cell::Text(t) => json!(t),
        Cell::Missing => J::Null,
    }
}

/// The rows of one run: the fixed rows, then calls and cost per model, then per step kind.
fn rows(run: &Run) -> Vec<(String, Cell)> {
    let mut out = run.metrics.clone();
    for map in [&run.by_model, &run.by_step].into_iter().flatten() {
        for (k, t) in map {
            out.push((format!("LLM calls [{k}]"), int(t.calls)));
            out.push((format!("LLM cost (USD) [{k}]"), usd(t.cost)));
        }
    }
    out
}

/// The metric names of the report: the fixed rows in order, then every model and every step kind
/// any run used, sorted.
fn metric_names(runs: &[Run]) -> Vec<String> {
    let mut names: Vec<String> = runs[0].metrics.iter().map(|(k, _)| k.clone()).collect();
    for pick in [|r: &Run| r.by_model.clone(), |r: &Run| r.by_step.clone()] {
        let keys: BTreeSet<String> = runs.iter().filter_map(pick).flat_map(|m| m.into_keys()).collect();
        for k in keys {
            names.push(format!("LLM calls [{k}]"));
            names.push(format!("LLM cost (USD) [{k}]"));
        }
    }
    names
}

/// A run's value of a metric; a model or step kind the run never used reads zero when its calls
/// are recorded, and "not recorded" when they are not.
fn cell_of(rows: &[(String, Cell)], run: &Run, key: &str) -> Cell {
    if let Some((_, c)) = rows.iter().find(|(k, _)| k == key) {
        return c.clone();
    }
    if key.contains('[') && run.by_model.is_some() {
        return if key.starts_with("LLM cost") { usd(0.0) } else { int(0) };
    }
    Cell::Missing
}

fn report(runs: &[Run]) -> (String, J) {
    let names = metric_names(runs);
    let all: Vec<Vec<(String, Cell)>> = runs.iter().map(rows).collect();
    let mut md = String::new();
    let _ = writeln!(md, "# Run comparison\n");
    for r in runs {
        let _ = writeln!(md, "- {}: {}", r.label, r.name);
    }
    let _ = writeln!(md, "\nFields and sources: `docs/run-compare.md`. Seconds are whole seconds; costs are USD.");
    for (r, rs) in runs.iter().zip(&all) {
        let _ = writeln!(md, "\n## {}: {}\n\n| metric | value |\n|---|---|", r.label, r.name);
        for (k, v) in rs {
            let _ = writeln!(md, "| {} | {} |", cell_md(k), cell_md(&render(v)));
        }
    }
    let _ = write!(md, "\n## Comparison\n\n| metric | {}", runs.iter().map(|r| r.label.as_str()).collect::<Vec<_>>().join(" | "));
    for r in &runs[1..] {
        let _ = write!(md, " | {} - {}", r.label, runs[0].label);
    }
    let _ = writeln!(md, " |\n|{}", "---|".repeat(1 + runs.len() + runs.len() - 1));
    let mut comparison = vec![];
    for k in &names {
        let cells: Vec<Cell> = runs.iter().zip(&all).map(|(r, rs)| cell_of(rs, r, k)).collect();
        let diffs: Vec<Option<String>> = cells[1..].iter().map(|c| difference(&cells[0], c)).collect();
        let _ = write!(md, "| {} |", cell_md(k));
        for c in &cells {
            let _ = write!(md, " {} |", cell_md(&render(c)));
        }
        for d in &diffs {
            let _ = write!(md, " {} |", d.as_deref().unwrap_or("n/a"));
        }
        let _ = writeln!(md);
        comparison.push(json!({"metric": k,
            "values": runs.iter().zip(&cells).map(|(r, c)| json!({"label": r.label, "value": cell_json(c)})).collect::<Vec<_>>(),
            "difference": runs[1..].iter().zip(&diffs).map(|(r, d)| json!({"label": r.label, "against": runs[0].label, "value": d})).collect::<Vec<_>>()}));
    }
    let doc = json!({"format": "sdc.run-compare/1",
        "runs": runs.iter().zip(&all).map(|(r, rs)| json!({"label": r.label, "run": r.name, "details": r.details,
            "metrics": rs.iter().map(|(k, v)| json!({"metric": k, "value": cell_json(v)})).collect::<Vec<_>>()})).collect::<Vec<_>>(),
        "comparison": comparison});
    (md, doc)
}

// ---- main --------------------------------------------------------------------------------------

struct Cli {
    runs: Vec<(String, PathBuf)>,
    out: PathBuf,
    json: Option<PathBuf>,
}

fn parse_args(args: &[String]) -> Result<Cli, String> {
    let (mut runs, mut out, mut json_out): (Vec<(String, PathBuf)>, Option<PathBuf>, Option<PathBuf>) = (vec![], None, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = |what: &str| it.next().cloned().ok_or(format!("{a} needs {what}"));
        match a.as_str() {
            "--run" => {
                let v = value("<label>=<run dir>")?;
                let (l, d) = v.split_once('=').filter(|(l, d)| !l.is_empty() && !d.is_empty()).ok_or(format!("--run {v}: expected <label>=<run dir>"))?;
                if runs.iter().any(|(x, _)| x == l) {
                    return Err(format!("--run: label {l} given twice"));
                }
                runs.push((l.to_string(), PathBuf::from(d)));
            }
            "--out" => out = Some(PathBuf::from(value("a file")?)),
            "--json" => json_out = Some(PathBuf::from(value("a file")?)),
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    if runs.len() < 2 {
        return Err("a comparison needs at least two --run".into());
    }
    Ok(Cli { runs, out: out.ok_or("--out is required")?, json: json_out })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = match parse_args(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("sdc-run-compare: {e}\n{USAGE}");
            return ExitCode::from(64);
        }
    };
    let mut runs = vec![];
    for (label, dir) in &cli.runs {
        match load(label, dir) {
            Ok(r) => runs.push(r),
            Err(Fail(m)) => {
                eprintln!("sdc-run-compare: {m}");
                return ExitCode::from(2);
            }
        }
    }
    let (md, doc) = report(&runs);
    let write = |p: &Path, s: String| {
        if let Err(e) = fs::write(p, s) {
            eprintln!("sdc-run-compare: cannot write {}: {e}", p.display());
            return false;
        }
        true
    };
    if !write(&cli.out, md) {
        return ExitCode::from(1);
    }
    if let Some(j) = &cli.json {
        if !write(j, serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n") {
            return ExitCode::from(1);
        }
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_reads_rfc_3339_utc() {
        assert_eq!(epoch_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(epoch_ms("2026-11-22T09:00:00Z"), Some(1_795_338_000_000));
        assert_eq!(epoch_ms("2026-11-22T09:41:00.9Z"), Some(1_795_340_460_900));
        assert_eq!(epoch_ms("2026-11-21T00:53:37.755Z"), Some(1_795_222_417_755));
        assert_eq!(epoch_ms("-"), None);
        assert_eq!(epoch_ms("2026-11-22 09:00:00Z"), None);
    }

    #[test]
    fn a_person_is_not_an_agent_a_decider_or_a_rule() {
        assert!(person("person:ada") && person("operator") && person("human"));
        assert!(!person("") && !person("decider:haiku") && !person("decider:model") && !person("rule:x") && !person("non-interactive") && !person("derived"));
    }

    #[test]
    fn the_last_round_waits_only_when_a_stop_record_ends_it() {
        let r = |s, e, v: &str| Round { start_s: s, end_s: e, seconds: (e - s) as f64, verdict: v.into(), case_id: "c".into() };
        let rounds = [r(0, 10, "awaiting_person")];
        assert_eq!(waiting(&rounds, &[], None, false, &[]), (Some(0), vec![], true));
        assert_eq!(waiting(&rounds, &[], Some(70), false, &[]).0, Some(60));
        assert_eq!(waiting(&rounds, &[], Some(70), true, &[]).0, Some(0));
    }
}
