//! The late-finding KPIs L1-L6 of one run (docs/kpis.md, "Late findings and human interaction").
//!
//! ```text
//! sdc-finding-kpis <run dir> [--json]
//! ```
//!
//! `<run dir>` is what run-trial.sh or run-full-cycle.sh writes: store/IDEA-1/ (the case tree;
//! every case's events.jsonl), optionally rounds.jsonl (run-trial.sh: one line per round with
//! started, ended, seconds, verdict), run-settings.jsonl (one line per leg), run-stopped.json (the
//! stop record), decision.json (present once the root is closed) and decisions.jsonl (a decider's
//! decisions). Reads nothing else. A KPI reported as a failure carries `failure: true` and prints
//! as `FAILED`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{json, Value as J};
use serde_yaml::Value;

fn yaml(path: &Path) -> Value {
    fs::read_to_string(path).ok().and_then(|t| serde_yaml::from_str::<Value>(&t).ok())
        .filter(Value::is_mapping).unwrap_or_else(|| Value::Mapping(Default::default()))
}

fn st<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

fn seq(v: &Value, k: &str) -> Vec<Value> {
    v.get(k).and_then(Value::as_sequence).cloned().unwrap_or_default()
}

/// A person: `person:<id>`. Runs recorded before principals were typed name a person by a bare id
/// (`operator`, a login) or `human`; those still count. Nothing with another prefix (`decider:`,
/// `rule:`, `agent:`), and not `non-interactive`, `derived`, `dry-run` or `operator-step`.
fn person(p: &str) -> bool {
    if let Some(id) = p.strip_prefix("person:") {
        return !id.is_empty();
    }
    !p.is_empty() && !p.contains(':') && !matches!(p, "non-interactive" | "derived" | "dry-run" | "operator-step" | "agent")
}

/// Every case directory under `root` (events.jsonl and case.json), not following symlinks.
fn cases(root: &Path, out: &mut Vec<PathBuf>) {
    if root.join("events.jsonl").is_file() && root.join("case.json").is_file() {
        out.push(root.to_path_buf());
    }
    let Ok(dir) = fs::read_dir(root) else { return };
    let mut subs: Vec<PathBuf> = dir.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.path()).collect();
    subs.sort();
    for d in subs {
        cases(&d, out);
    }
}

fn events(case: &Path) -> Vec<J> {
    fs::read_to_string(case.join("events.jsonl")).unwrap_or_default().lines()
        .filter(|l| !l.trim().is_empty()).filter_map(|l| serde_json::from_str(l).ok()).collect()
}

fn js(v: &J) -> String {
    v.as_str().map(String::from).unwrap_or_else(|| if v.is_null() { String::new() } else { v.to_string() })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(run) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("usage: sdc-finding-kpis <run dir> [--json]");
        return ExitCode::from(64);
    };
    let as_json = args.iter().any(|a| a == "--json");
    let run = Path::new(run);
    let root = run.join("store").join("IDEA-1");
    let work = root.join("work");
    let mut k: Vec<(&str, J)> = vec![];

    // L1 human interventions: person actions (approvals and scope.answer through `approve`) plus
    // person answers (scope-changing questions, finding decisions, findings raised by a person,
    // context fields a person set).
    let mut dirs = vec![];
    cases(&root, &mut dirs);
    let (mut acts, mut grants) = (vec![], vec![]);
    let mut revs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for c in &dirs {
        let cid = c.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        for e in events(c) {
            let principal = js(&e["principal"]);
            match e["event"].as_str().unwrap_or("") {
                "action_finished" if person(&principal) => acts.push((cid.clone(), js(&e["at"]), principal)),
                "authority_recorded" if person(&principal) => grants.push((cid.clone(), js(&e["at"]), principal)),
                "revised" => {
                    for (a, r) in e["artifacts"].as_object().into_iter().flatten() {
                        let r = js(r);
                        if r != "none" {
                            revs.entry(a.clone()).or_default().insert(r);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let person_actions = acts.len() + grants.iter().filter(|g| !acts.contains(g)).count();
    let qs = seq(&yaml(&work.join("questions.yaml")), "questions");
    let fd = seq(&yaml(&work.join("findings.yaml")), "findings");
    let ctx = yaml(&work.join("context.yaml"));
    let fields: Vec<&Value> = ctx.as_mapping().map(|m| m.values().filter(|v| v.is_mapping()).collect()).unwrap_or_default();
    let answered = |q: &Value| matches!(st(q, "status"), "accepted" | "changed");
    let answers = [
        ("questions", qs.iter().filter(|q| person(st(q, "answered_by")) && answered(q)).count()),
        ("finding_decisions", fd.iter().filter(|f| person(st(f, "decided_by"))).count()),
        ("findings_raised", fd.iter().filter(|f| person(st(f, "raised_by"))).count()),
        ("context_fields", fields.iter().filter(|v| answered(v)).count()),
    ];
    let answers_sum: usize = answers.iter().map(|(_, n)| n).sum();
    let answers_map: serde_json::Map<String, J> = answers.iter().map(|(a, n)| (a.to_string(), json!(n))).collect();
    // A run in which a script or a session stands in for the person (run-settings `stand_in`) is an
    // end-to-end test of the person path: its person counts are not eval numbers.
    let stand_in = jsonl(&run.join("run-settings.jsonl")).iter().find(|s| s["stand_in"] == json!(true))
        .map(|s| s["stand_in_by"].clone());
    k.push(("L1_human_interventions", json!({"value": person_actions + answers_sum, "person_actions": person_actions,
        "person_answers": answers_map, "stand_in": stand_in.is_some(), "stand_in_by": stand_in})));

    // L2 autonomous run time between human interventions: the wall seconds of each round that
    // ended waiting on a person (or at the end).
    let rounds_path = run.join("rounds.jsonl");
    if rounds_path.is_file() {
        let rounds: Vec<J> = fs::read_to_string(&rounds_path).unwrap_or_default().lines()
            .filter(|l| !l.trim().is_empty()).filter_map(|l| serde_json::from_str(l).ok()).collect();
        let secs: Vec<f64> = rounds.iter().filter_map(|r| r["seconds"].as_f64()).collect();
        let span = match (rounds.first(), rounds.last()) {
            (Some(a), Some(b)) => b["ended_epoch"].as_f64().unwrap_or(0.0) - a["started_epoch"].as_f64().unwrap_or(0.0),
            _ => 0.0,
        };
        let mut sorted = secs.clone();
        sorted.sort_by(f64::total_cmp);
        let median = match sorted.len() {
            0 => J::Null,
            n if n % 2 == 1 => num(sorted[n / 2]),
            n => num((sorted[n / 2 - 1] + sorted[n / 2]) / 2.0),
        };
        let sum: f64 = secs.iter().sum();
        let share = if span != 0.0 { num((sum / span * 1000.0).round() / 1000.0) } else { J::Null };
        k.push(("L2_autonomous_seconds", json!({"value": median, "max": num(sorted.last().copied().unwrap_or(0.0)),
            "sum": num(sum), "rounds": secs.len(), "share_of_wall": share})));
    } else {
        k.push(("L2_autonomous_seconds", json!({"value": null, "why": "no rounds.jsonl (scripted runs use a fixed --at)"})));
    }

    // L7 time waiting on a person: every stop after which the run did not go on by itself.
    k.push(("L7_waiting_on_person_seconds", waiting(run)));

    // L3 defaults accepted without change.
    let res: Vec<&Value> = fd.iter().filter(|f| st(f, "status") == "resolved").collect();
    let taken = res.iter().filter(|f| f.get("resolution").and_then(|r| r.get("default_taken")).and_then(Value::as_bool) == Some(true)).count();
    k.push(("L3_defaults_accepted", json!({
        "findings": {"value": taken, "of": res.len()},
        "questions": {"value": qs.iter().filter(|q| st(q, "status") == "accepted").count(), "of": qs.iter().filter(|q| answered(q)).count()},
        "context": {"value": fields.iter().filter(|v| matches!(st(v, "status"), "default" | "accepted")).count(), "of": fields.len()}})));

    // L4 late findings per stage: findings raised after intake, by the stage that raised them.
    let mut stages: BTreeMap<String, usize> = BTreeMap::new();
    for f in fd.iter().filter(|f| st(f, "status") != "rejected" && st(f, "stage") != "requirements") {
        *stages.entry(st(f, "stage").to_string()).or_default() += 1;
    }
    k.push(("L4_late_findings", json!({"value": stages.values().sum::<usize>(), "by_stage": stages})));

    // L5 who caught them.
    let by = |src: &str| fd.iter().filter(|f| st(f, "raised_by").split(':').next() == Some(src) && st(f, "status") != "rejected").count();
    k.push(("L5_caught_by", json!({"program": by("program"), "llm": by("llm"), "human": by("human") + by("person"),
        "llm_rejected_by_program": fd.iter().filter(|f| st(f, "status") == "rejected").count(),
        "resolved_by_agent": res.iter().filter(|f| st(f, "decided_by") == "agent").count(),
        "resolved_by_person": res.iter().filter(|f| person(st(f, "decided_by"))).count(),
        "resolved_by_decider": res.iter().filter(|f| st(f, "decided_by").starts_with("decider:")).count()})));

    // L6 rework: revisions after the first, per artifact, over the whole tree.
    let by_artifact: BTreeMap<&String, usize> = revs.iter().map(|(a, v)| (a, v.len() - 1)).collect();
    k.push(("L6_rework", json!({"value": by_artifact.values().sum::<usize>(), "by_artifact": by_artifact})));

    if as_json {
        let obj: serde_json::Map<String, J> = k.into_iter().map(|(a, v)| (a.to_string(), v)).collect();
        println!("{}", serde_json::to_string_pretty(&J::Object(obj)).unwrap_or_default());
    } else {
        for (key, v) in k {
            let failed = if v["failure"] == json!(true) { "FAILED " } else { "" };
            println!("{key}: {failed}{v}");
        }
    }
    ExitCode::SUCCESS
}

fn jsonl(path: &Path) -> Vec<J> {
    fs::read_to_string(path).unwrap_or_default().lines()
        .filter(|l| !l.trim().is_empty()).filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// Seconds since the epoch of an RFC 3339 UTC instant `YYYY-MM-DDTHH:MM:SS[.fff]Z`.
fn epoch(t: &str) -> Option<i64> {
    let n = |a: usize, b: usize| t.get(a..b).and_then(|s| s.parse::<i64>().ok());
    let (y, m, d, hh, mm, ss) = (n(0, 4)?, n(5, 7)?, n(8, 10)?, n(11, 13)?, n(14, 16)?, n(17, 19)?);
    if !t.ends_with('Z') || t.as_bytes().get(10) != Some(&b'T') {
        return None;
    }
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146_097 + doe - 719_468) * 86_400 + hh * 3600 + mm * 60 + ss)
}

/// L7: the time the run spent waiting on a person, with the stops that caused it. A stop waits on
/// a person when the next round starts in a new leg (a `run-settings.jsonl` line between the two:
/// the script had ended and was started again), or when it is the last round and the run was not
/// closed (`decision.json`): then it waits until the stop record's instant (`run-stopped.json`)
/// or, while the run is neither closed nor stopped, until now (`ongoing`). Without
/// run-settings.jsonl every round ending awaiting_person or awaiting_authority counts. At A3 and
/// up (a decider holds the decisions: the last leg's `decider.kind` is not `person`) a value above
/// 0 is a failure.
fn waiting(run: &Path) -> J {
    let rounds = jsonl(&run.join("rounds.jsonl"));
    if rounds.is_empty() {
        return json!({"value": null, "why": "no rounds.jsonl (scripted runs use a fixed --at)"});
    }
    let settings = jsonl(&run.join("run-settings.jsonl"));
    let legs: Vec<i64> = settings.iter().filter_map(|s| s["at"].as_str().and_then(epoch)).collect();
    let stopped = fs::read_to_string(run.join("run-stopped.json")).ok()
        .and_then(|t| serde_json::from_str::<J>(&t).ok());
    let closed = run.join("decision.json").is_file();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let person_verdict = |v: &str| matches!(v, "awaiting_person" | "awaiting_authority");
    let mut stops = vec![];
    for (i, r) in rounds.iter().enumerate() {
        let ended = r["ended_epoch"].as_i64().unwrap_or(0);
        let verdict = r["verdict"].as_str().unwrap_or("");
        let (until, how) = match rounds.get(i + 1) {
            Some(next) => {
                let start = next["started_epoch"].as_i64().unwrap_or(ended);
                let new_leg = if legs.is_empty() { person_verdict(verdict) } else { legs.iter().any(|&l| l >= ended && l <= start) };
                if !new_leg {
                    continue;
                }
                (start, "resumed")
            }
            None if closed => continue,
            None => match stopped.as_ref().and_then(|s| s["at"].as_str()).and_then(epoch) {
                Some(at) => (at, "stopped"),
                None => (now, "ongoing"),
            },
        };
        stops.push(json!({"verdict": verdict, "case_dir": r["case_dir"], "ended": r["ended"],
            "until": how, "seconds": (until - ended).max(0)}));
    }
    let value: i64 = stops.iter().filter_map(|s| s["seconds"].as_i64()).sum();
    let decider = settings.last().and_then(|s| s["decider"]["kind"].as_str()).unwrap_or("person").to_string();
    let level = settings.last().and_then(|s| s["autonomy"].as_str()).map(String::from);
    let attended = decider == "person";
    let mut out = json!({"value": value, "stops": stops, "decider": decider, "autonomy": level,
        "ongoing": stops.iter().any(|s| s["until"] == "ongoing")});
    if !attended {
        out["failure"] = json!(value > 0);
        if value > 0 {
            out["why"] = json!("a decider holds the decisions (A3 and up): no stop may wait on a person");
        }
    }
    out
}

/// A whole number prints as an integer, like the seconds it came from.
fn num(x: f64) -> J {
    if x.fract() == 0.0 && x.abs() < 9.0e15 { json!(x as i64) } else { json!(x) }
}
