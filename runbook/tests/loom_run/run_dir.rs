//! The run directory writer (`src/loom_run/run_dir.rs`): the files of a Loom run that
//! `sdc-experiment report` and `runbook parity summarize` read.
//!
//! The crate has no library target, so the writer and the events it receives are compiled into this
//! test binary from their source files (`events.rs` and `run_dir.rs` use nothing else of the
//! crate). `read_run` below reads a run directory by the rules of the contract table
//! (`driver/src/run_metrics.rs` `load`, as `docs/loom-run-directory.md` repeats them); the driver's
//! own code is not a dependency of this crate.

#[allow(dead_code)]
#[path = "../../src/loom_run/events.rs"]
mod events;
#[allow(dead_code)]
#[path = "../../src/loom_run/run_dir.rs"]
mod writer;

use crate::{path_str, pfx, stdout, Timed, BIN};
use events::{Cap, CostSource, Event, EventSink};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use writer::RunDirWriter;

const T0: u64 = 1_795_000_000;
const HOME: &str = concat!("/ho", "me/");
const ROOT: &str = "IDEA-1";
const CHILD: &str = "IDEA-1.intake";

fn hex(s: &str) -> String {
    s.bytes().map(|b| format!("{b:02x}")).collect()
}

fn scratch(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("run-dir-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn put(p: &Path, text: &str) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn rfc(secs: u64) -> String {
    // The same civil-date arithmetic the reader below inverts.
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

fn epoch_ms(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 20 || !s.ends_with('Z') {
        return None;
    }
    let n = |a: usize, c: usize| s.get(a..c)?.parse::<i64>().ok();
    let (y, mo, d, hh, mm, ss) = (n(0, 4)?, n(5, 7)?, n(8, 10)?, n(11, 13)?, n(14, 16)?, n(17, 19)?);
    let frac = if b[19] == b'.' { s.get(20..s.len() - 1)?.parse::<i64>().ok()? } else { 0 };
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some((days * 86_400 + hh * 3600 + mm * 60 + ss) * 1000 + frac)
}

// ------------------------------------------------------------------------- the fake Loom side

const PROTOCOL_SDC: &str = "format: protocol/1\nprotocol:\n  id: sdc\n  revision: 8\nactions:\n  intake.open:\n    effect: write\n";
const PROTOCOL_INTAKE: &str = "format: protocol/1\nprotocol:\n  id: sdc.intake\n  revision: 9\nactions:\n  idea.check:\n    effect: read\n  idea.dissect:\n    effect: write\n  dissection.check:\n    effect: read\n  scope.approve:\n    effect: write\n";
const BINDINGS_SDC: &str = "format: loom.bindings/1\nprotocol: sdc@8\nprogram_root: bindings\nactions:\n  - action: intake.open\n    executor:\n      kind: open\n";
const BINDINGS_INTAKE: &str = "format: loom.bindings/1\nprotocol: sdc-intake@9\nprogram_root: bindings\nactions:\n  - action: idea.check\n    executor:\n      kind: program\n      argv: [tools/sdc-tool.sh, check-idea, --action, idea.check]\n  - action: idea.dissect\n    executor:\n      kind: program\n      argv: [tools/llm-node.sh, idea.dissect, --model, claude-sonnet-5-5]\n  - action: dissection.check\n    executor:\n      kind: program\n      argv: [tools/sdc-tool.sh, check-dissection, --action, dissection.check]\n  - action: scope.approve\n    executor:\n      kind: person\n      principal: person:owner\n";

fn rl(seq: u64, kind: &str, extra: Value) -> Value {
    let mut v = json!({"seq": seq, "at": "2026-11-22T10:00:00Z", "kind": kind});
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

fn program(seq: u64, action: &str, exit: i64, evidence: &[&str]) -> Value {
    rl(seq, "ActionFinished", json!({"action": action, "exit": exit, "blob_digest": "d", "evidence": evidence, "revisions": []}))
}

/// A run directory as R1 leaves it, the writer opened on it, and the scripted Loom store.
struct World {
    dir: PathBuf,
    w: Option<RunDirWriter>,
    root_lines: u64,
    child_lines: u64,
}

fn settings() -> Value {
    json!({
        "login_file": "named",
        "models": {"writer": {"model": "claude-sonnet-5-5"}, "implement": {"model": "claude-sonnet-5-5"},
                   "review": {"model": "claude-sonnet-5-5"}, "other": {"model": "claude-haiku-5-5"}},
        "decider": {"kind": "model", "model": "claude-haiku-5-5"},
        "loom": {"sha256": "ab12", "confinement": "local", "cap_micro_usd": 20_000_000u64},
        "max_llm_calls": 150, "max_wall_seconds": 5400,
        "case": ROOT, "root_protocol": "sdc@8",
    })
}

impl World {
    fn new(tag: &str, settings: &Value) -> World {
        let dir = scratch(tag);
        put(&dir.join("settings.json"), &serde_json::to_string(settings).unwrap());
        put(&dir.join("protocols/sdc.protocol.yaml"), PROTOCOL_SDC);
        put(&dir.join("protocols/sdc-intake.protocol.yaml"), PROTOCOL_INTAKE);
        put(&dir.join("bindings/sdc.yaml"), BINDINGS_SDC);
        put(&dir.join("bindings/sdc-intake.yaml"), BINDINGS_INTAKE);
        let w = RunDirWriter::open(&dir, settings).unwrap();
        World { dir, w: Some(w), root_lines: 0, child_lines: 0 }
    }

    fn ev(&mut self, secs: u64, e: Event) {
        self.w.as_mut().unwrap().event(secs, &e).unwrap();
    }

    fn log(&self, case: &str, lines: &[Value]) {
        let p = self.dir.join(format!("store/runlog-case-{}.jsonl", hex(case)));
        let mut t = std::fs::read_to_string(&p).unwrap_or_default();
        for l in lines {
            t.push_str(&serde_json::to_string(l).unwrap());
            t.push('\n');
        }
        put(&p, &t);
    }

    fn root(&mut self, lines: Vec<Value>) {
        let base = self.root_lines;
        let numbered: Vec<Value> = lines.into_iter().enumerate().map(|(i, mut l)| { l["seq"] = json!(base + i as u64); l }).collect();
        self.root_lines += numbered.len() as u64;
        self.log(ROOT, &numbered);
    }

    fn child(&mut self, lines: Vec<Value>) {
        let base = self.child_lines;
        let numbered: Vec<Value> = lines.into_iter().enumerate().map(|(i, mut l)| { l["seq"] = json!(base + i as u64); l }).collect();
        self.child_lines += numbered.len() as u64;
        self.log(CHILD, &numbered);
    }

    fn step(&mut self, secs: u64, index: u64, stop: &str) {
        self.ev(secs, Event::StepMade { index, exit: 0, stop: stop.into(), steps: 1 });
    }

    fn model_call(&mut self, secs: u64, micro: u64, started: u64, action: &str) {
        let record = json!({
            "action": action, "case": CHILD, "started": rfc(started), "attempt": 1, "retry": false,
            "adapter": "claude-cli", "model": "claude-sonnet-5-5", "effort": "default", "exit": 0, "duration_ms": 15000,
            "cost_usd": micro as f64 / 1e6, "num_turns": 3,
            "usage": {"input_tokens": 100, "output_tokens": 50, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0},
            "record": format!("{}/work/.sdc/llm/001-{action}", self.dir.display()),
        });
        self.ev(secs, Event::CostedCall { source: CostSource::Step, cost_micro_usd: micro, record });
    }

    fn start(&mut self) {
        self.ev(T0, Event::RunStarted { run: "r".into(), settings: settings() });
    }

    /// Steps 1 to 5 of the small complete run: two model steps with costs.
    fn first_steps(&mut self) {
        self.start();
        self.first_steps_after_start();
    }

    /// `first_steps` for a world whose `RunStarted` was sent already.
    fn first_steps_after_start(&mut self) {
        self.root(vec![
            rl(0, "CaseOpened", json!({"case": ROOT, "protocol": "sdc@8"})),
            rl(1, "Evaluated", json!({})),
            rl(2, "ActionFinished", json!({"action": "intake.open", "exit": 0, "blob_digest": "d", "evidence": ["ev-open"], "revisions": []})),
            rl(3, "ChildOpened", json!({"child": CHILD, "protocol": "sdc-intake@9"})),
        ]);
        self.child(vec![rl(0, "CaseOpened", json!({"case": CHILD, "protocol": "sdc-intake@9"}))]);
        self.step(T0 + 10, 1, "Continue");
        self.child(vec![program(1, "idea.check", 0, &["c1"])]);
        self.step(T0 + 20, 2, "Continue");
        self.model_call(T0 + 50, 120_000, T0 + 30, "idea.dissect");
        self.child(vec![program(2, "idea.dissect", 0, &[])]);
        self.step(T0 + 50, 3, "Continue");
        self.child(vec![program(3, "dissection.check", 1, &["c2"])]);
        self.step(T0 + 60, 4, "Continue");
        self.model_call(T0 + 90, 80_000, T0 + 70, "idea.dissect");
        self.child(vec![program(4, "idea.dissect", 0, &[])]);
        self.step(T0 + 90, 5, "Continue");
    }

    /// The rest of the small complete run: a decision answered by the model decider, the outcome.
    fn last_steps(&mut self) {
        self.child(vec![program(5, "dissection.check", 0, &["c3"]), rl(6, "Stopped", json!({"stop": "AwaitingPerson", "next": ["scope.approve"]}))]);
        self.step(T0 + 100, 6, "AwaitingPerson");
        let decision = json!({"case": CHILD, "capability": "owner.approve", "action": "scope.approve", "kind": "ApproveAction",
            "path": format!("{}/work/idea.yaml", self.dir.display()), "revision": 7, "subject": "idea",
            "command": ["b10x-loom", "tree", "approve", "--store", format!("{}/store", self.dir.display())]});
        self.ev(T0 + 100, Event::DecisionAsked { decision: decision.clone() });
        self.child(vec![
            rl(7, "AuthorityRecorded", json!({"capability": "owner.approve", "decision": "Allow", "principal": "decider:model"})),
            rl(8, "ActionFinished", json!({"action": "scope.approve", "exit": 0, "evidence": ["c4"], "revisions": []})),
        ]);
        self.ev(T0 + 118, Event::CostedCall { source: CostSource::Decider, cost_micro_usd: 30_000, record: json!({"note": "decider", "model": "claude-haiku-5-5"}) });
        self.ev(
            T0 + 120,
            Event::DecisionAnswered { decision, approve: true, principal: "decider:model".into(), cost_micro_usd: 30_000, model_calls: 1 },
        );
        self.child(vec![rl(9, "Evaluated", json!({})), rl(10, "Terminated", json!({"reason": "passed"}))]);
        self.root(vec![rl(4, "Terminated", json!({"reason": "operating"}))]);
        self.step(T0 + 130, 7, "Terminated");
        let status = status_doc(&self.dir);
        put(&self.dir.join("tree-status.json"), &serde_json::to_string(&status).unwrap());
        put(&self.dir.join("tree-verify.json"), &json!({"exit": 0, "stdout": "verified", "stderr": ""}).to_string());
        self.ev(T0 + 131, Event::TreeStored { status_exit: 0, verify_exit: 0 });
        self.ev(
            T0 + 131,
            Event::Ended { outcome: "ended".into(), reason: Some("operating".into()), loom_exit: None, stderr_tail: None, exit_code: 0, steps: 7, spent_micro_usd: 230_000, llm_calls: 3 },
        );
    }
}

fn status_doc(dir: &Path) -> Value {
    let ev = |id: &str, kind: &str, result: &str, producer: &str, action: &str| {
        json!({"evidence_id": id, "index": 0, "kind": kind, "result": result, "subject": "idea", "subject_revision": "r1",
               "upstream_revisions": {}, "producer": producer, "action": action})
    };
    json!({
        "format": "loom.tree-status/1", "version": "0.58.0",
        "store": format!("{}/store", dir.display()), "workspace": format!("{}/work", dir.display()), "root": ROOT,
        "cases": [
            {"case": ROOT, "protocol": "sdc@8", "protocol_id": "sdc", "protocol_revision": "8", "revision": 2, "outcome": "operating",
             "open_obligations": [], "stop_next": [], "evidence": [ev("ev-open", "intake_opened", "", "sdc-drive", "intake.open")],
             "runs": [{"action": "intake.open", "attempt": 1, "exit": 0, "retried": false, "retry_of": []}], "approvals": [], "copies": [],
             "case_file": format!("{}/store/case-x.json", dir.display()), "run_log": format!("{}/store/runlog-x.jsonl", dir.display())},
            {"case": CHILD, "name": "intake", "parent": ROOT, "protocol": "sdc-intake@9", "protocol_id": "sdc-intake", "protocol_revision": "9",
             "revision": 9, "outcome": "passed", "open_obligations": [], "stop_next": [],
             "evidence": [ev("c1", "idea_shape", "valid", "sdc-check", "idea.check"),
                          ev("c2", "dissection_shape", "invalid", "sdc-check", "dissection.check"),
                          ev("c3", "dissection_shape", "valid", "sdc-check", "dissection.check"),
                          ev("c4", "scope_approval", "approved", "decider:model", "scope.approve")],
             "runs": [{"action": "idea.check", "attempt": 1, "exit": 0, "retried": false, "retry_of": []},
                      {"action": "idea.dissect", "attempt": 1, "exit": 0, "retried": false, "retry_of": []},
                      {"action": "dissection.check", "attempt": 1, "exit": 1, "retried": true, "retry_of": []},
                      {"action": "idea.dissect", "attempt": 2, "exit": 0, "retried": false, "retry_of": ["dissection_shape"]},
                      {"action": "dissection.check", "attempt": 1, "exit": 0, "retried": false, "retry_of": []}],
             "approvals": [{"action": "scope.approve", "actor": "decider:model", "answer": "Allow", "at": "2026-11-22T10:00:00Z", "capability": "owner.approve"}],
             "copies": [], "case_file": format!("{}/store/case-y.json", dir.display()), "run_log": format!("{}/store/runlog-y.jsonl", dir.display())},
        ],
        "protocols": [],
    })
}

// ------------------------------------------------------------- the reader (the contract's rules)

#[derive(Debug, Default)]
struct Take {
    action: String,
    llm: bool,
    retry_of: Vec<String>,
    exit: String,
    principal: String,
    evidence: Vec<(String, String)>,
    t_start: Option<i64>,
    t_end: Option<i64>,
    cost: f64,
    done: bool,
}

#[derive(Debug, Default)]
struct CaseRead {
    id: String,
    stage: String,
    outcome: Option<String>,
    takes: Vec<Take>,
    effect: BTreeMap<String, String>,
    manual: Vec<String>,
}

#[derive(Debug, Default)]
struct Metrics {
    end: String,
    end_kind: String,
    finished: bool,
    closed: bool,
    delivered: bool,
    clean: bool,
    outcome: Option<String>,
    stop_cause: Option<String>,
    llm_calls: Option<usize>,
    llm_cost: f64,
    decider_calls: Option<usize>,
    decider_cost: f64,
    cost_usd: f64,
    cases: Vec<CaseRead>,
    settings: BTreeMap<String, String>,
    driver_s: Option<f64>,
    wall_s: Option<i64>,
    findings_raised: usize,
    malformed: usize,
    unmatched_llm_calls: usize,
}

fn jsonl(p: &Path, malformed: &mut usize) -> Option<Vec<Value>> {
    let t = std::fs::read_to_string(p).ok()?;
    let mut out = vec![];
    for l in t.lines().filter(|l| !l.trim().is_empty()) {
        match serde_json::from_str(l) {
            Ok(v) => out.push(v),
            Err(_) => *malformed += 1,
        }
    }
    Some(out)
}

fn json_opt(p: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

fn js(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        o => o.to_string(),
    }
}

fn flat(v: &Value, prefix: &str, out: &mut BTreeMap<String, String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                if matches!(k.as_str(), "at" | "format" | "nodes" | "llm_calls_before") {
                    continue;
                }
                flat(x, &if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") }, out);
            }
        }
        Value::Null => {
            out.insert(prefix.into(), "null".into());
        }
        Value::Array(_) => {
            out.insert(prefix.into(), v.to_string());
        }
        o => {
            out.insert(prefix.into(), js(o));
        }
    }
}

fn case_dirs(root: &Path, out: &mut Vec<PathBuf>) {
    if root.join("events.jsonl").is_file() {
        out.push(root.to_path_buf());
    }
    let Ok(rd) = std::fs::read_dir(root.join("children")) else { return };
    let mut subs: Vec<PathBuf> = rd.flatten().filter(|e| e.path().is_dir()).map(|e| e.path()).collect();
    subs.sort();
    for d in subs {
        case_dirs(&d, out);
    }
}

fn root_case_dir(dir: &Path) -> PathBuf {
    let idea = dir.join("store/IDEA-1");
    if idea.join("events.jsonl").is_file() {
        return idea;
    }
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir.join("store")).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.join("events.jsonl").is_file()).collect();
    found.sort();
    if found.len() == 1 { found.remove(0) } else { idea }
}

/// What `run_metrics::load` makes of the directory, for the members the tests check.
fn read_run(dir: &Path) -> Result<Metrics, String> {
    assert!(!dir.join("free-agent.json").exists(), "free-agent.json makes the report read a free-agent run");
    let root = root_case_dir(dir);
    if !root.join("events.jsonl").is_file() {
        return Err(format!("cannot read {}: no such file", root.join("events.jsonl").display()));
    }
    let mut m = Metrics::default();
    let mut dirs = vec![];
    case_dirs(&root, &mut dirs);
    for d in &dirs {
        let events = jsonl(&d.join("events.jsonl"), &mut m.malformed).unwrap();
        let observed = jsonl(&d.join("observed.jsonl"), &mut m.malformed).unwrap_or_default();
        let mut times: Vec<Option<i64>> = vec![None; events.len()];
        for o in &observed {
            if let (Some(i), Some(t)) = (o["index"].as_u64(), o["observed_at"].as_str().and_then(epoch_ms)) {
                times[i as usize] = Some(t);
            }
        }
        let protocol = events.iter().find(|e| e["event"] == "opened").map(|e| js(&e["protocol"]["id"])).unwrap_or_default();
        let stage = match protocol.strip_prefix("sdc.") {
            Some(s) => s.to_string(),
            None if protocol == "sdc" => "idea".into(),
            None => protocol.clone(),
        };
        let mut c = CaseRead { id: d.file_name().unwrap().to_string_lossy().into_owned(), stage, ..Default::default() };
        if let Some(ir) = json_opt(&d.join("ir.json")) {
            for (k, v) in ir["actions"].as_object().into_iter().flatten() {
                c.effect.insert(k.clone(), js(&v["effect"]));
            }
        }
        if let Some(b) = json_opt(&d.join("bindings.json")) {
            for (k, v) in b.as_object().into_iter().flatten() {
                if v["manual"] == true {
                    c.manual.push(k.clone());
                }
            }
        }
        let mut open: Option<usize> = None;
        for (i, e) in events.iter().enumerate() {
            match e["event"].as_str().unwrap_or("") {
                "action_started" => {
                    c.takes.push(Take {
                        action: js(&e["action"]),
                        llm: e["llm"] == true,
                        retry_of: e["retry_of"].as_array().into_iter().flatten().map(js).collect(),
                        t_start: times[i],
                        ..Default::default()
                    });
                    open = Some(c.takes.len() - 1);
                }
                "action_finished" => {
                    let Some(k) = open.take().filter(|k| c.takes[*k].action == js(&e["action"])) else { continue };
                    let t = &mut c.takes[k];
                    t.done = true;
                    t.exit = js(&e["exit"]);
                    t.principal = js(&e["principal"]);
                    t.t_end = times[i];
                    for id in e["evidence"].as_array().into_iter().flatten().map(js) {
                        let rec = json_opt(&d.join("evidence").join(format!("{id}.json"))).map(|v| (js(&v["kind"]), js(&v["result"]))).unwrap_or_default();
                        t.evidence.push(rec);
                    }
                }
                "terminated" => c.outcome = Some(js(&e["outcome"])),
                _ => {}
            }
        }
        m.cases.push(c);
    }
    let root_id = root.file_name().unwrap().to_string_lossy().into_owned();

    let calls = jsonl(&dir.join("llm-calls.jsonl"), &mut m.malformed);
    let decisions = jsonl(&dir.join("decisions.jsonl"), &mut m.malformed);
    for c in m.cases.iter_mut() {
        let actions: Vec<String> = c.takes.iter().map(|t| t.action.clone()).collect();
        for a in actions.iter().collect::<std::collections::BTreeSet<_>>() {
            let lines: Vec<&Value> = calls.iter().flatten().filter(|l| js(&l["case"]) == c.id && js(&l["action"]) == *a).collect();
            let idx: Vec<usize> = (0..c.takes.len()).filter(|&i| c.takes[i].action == *a && c.takes[i].llm && c.takes[i].done).collect();
            let mut cursor = 0;
            for l in lines {
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
                        cursor += if started.is_some() && c.takes[i].t_start.is_some() { p } else { p + 1 };
                    }
                    None => m.unmatched_llm_calls += 1,
                }
            }
        }
    }
    m.llm_calls = calls.as_ref().map(Vec::len);
    m.llm_cost = calls.iter().flatten().map(|l| l["cost_usd"].as_f64().unwrap_or(0.0)).sum();
    let is_call = |l: &Value| l["model"].as_str().is_some_and(|x| !x.is_empty()) && l["suppressed"] != true && l.get("fallback").is_none_or(|f| f.is_null());
    m.decider_calls = decisions.as_ref().map(|d| d.iter().filter(|l| is_call(l)).count());
    m.decider_cost = decisions.iter().flatten().filter(|l| is_call(l)).map(|l| l["cost_usd"].as_f64().unwrap_or(0.0)).sum();
    m.cost_usd = m.llm_cost + m.decider_cost;

    let status = json_opt(&dir.join("status.json"));
    let stopped = json_opt(&dir.join("run-stopped.json"));
    let reopens = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.starts_with("run-stopped.") && n.ends_with(".json") && n != "run-stopped.json"
        })
        .count();
    let defects = jsonl(&dir.join("defects.jsonl"), &mut m.malformed);
    assert!(defects.is_none(), "defects.jsonl is not written by a Loom run");
    let root_case = m.cases.iter().find(|c| c.id == root_id);
    let closed_outcome = root_case.and_then(|c| c.outcome.clone()).or_else(|| status.as_ref().filter(|s| s["state"] == "closed").map(|s| js(&s["outcome"])));
    (m.end, m.end_kind) = if let Some(o) = &closed_outcome {
        (format!("closed: {o}"), "closed".into())
    } else if let Some(s) = &stopped {
        let cause = Some(js(&s["cause"])).filter(|c| !c.is_empty()).unwrap_or_else(|| js(&s["verdict_at_stop"]));
        (format!("stopped: {cause} in {}", js(&s["case"])), "stopped".into())
    } else {
        ("open: no close and no stop record".into(), "open".into())
    };
    m.stop_cause = stopped.as_ref().map(|s| js(&s["cause"]));
    m.finished = m.end_kind != "open";
    m.closed = m.end_kind == "closed";
    m.delivered = match (&closed_outcome, status.as_ref().and_then(|s| s["delivered"].as_bool())) {
        (Some(_), Some(d)) => d,
        (Some(o), None) => o == "operating",
        _ => false,
    };
    m.outcome = closed_outcome;
    m.clean = m.closed && reopens == 0;
    let rounds = jsonl(&dir.join("rounds.jsonl"), &mut m.malformed).unwrap_or_default();
    m.driver_s = (!rounds.is_empty()).then(|| rounds.iter().filter_map(|r| r["seconds"].as_f64()).sum());
    let mut times: Vec<i64> = vec![];
    for d in &dirs {
        for o in jsonl(&d.join("observed.jsonl"), &mut m.malformed).unwrap_or_default() {
            times.extend(o["observed_at"].as_str().and_then(epoch_ms));
        }
    }
    m.wall_s = match (times.iter().min(), times.iter().max()) {
        (Some(a), Some(b)) => Some((b - a) / 1000),
        _ => None,
    };
    if let Some(last) = jsonl(&dir.join("run-settings.jsonl"), &mut m.malformed).and_then(|v| v.last().cloned()) {
        flat(&last, "", &mut m.settings);
    }
    m.findings_raised = std::fs::read_to_string(root.join("work/findings.yaml")).map(|t| t.matches("- id:").count()).unwrap_or(0);
    Ok(m)
}

fn all_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            all_files(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

// --------------------------------------------------------------------------------------- tests

#[test]
fn a_small_complete_run_reads_as_closed_delivered_and_clean_with_the_cost_of_the_events() {
    let _t = Timed::start("a_small_complete_run_reads_as_closed_delivered_and_clean_with_the_cost_of_the_events");
    let mut w = World::new("complete", &settings());
    w.first_steps();
    w.last_steps();
    let m = read_run(&w.dir).unwrap();
    let take = |case: &str, action: &str| -> Vec<&Take> {
        m.cases.iter().find(|c| c.id == case).unwrap().takes.iter().filter(|t| t.action == action).collect()
    };

    // Table 2: closed, delivered, clean, finished, the end text.
    assert_eq!((m.finished, m.closed, m.delivered, m.clean), (true, true, true, true));
    assert_eq!(m.end, "closed: operating");
    assert_eq!(m.outcome.as_deref(), Some("operating"));
    // status.json: the tree's outcome and the delivered flag, no counters.
    let status = json_opt(&w.dir.join("status.json")).unwrap();
    assert_eq!((status["state"].as_str(), status["outcome"].as_str(), status["delivered"].as_bool()), (Some("closed"), Some("operating"), Some(true)));
    // The cost is the cost of the events: two step calls and one decider call.
    assert_eq!(m.llm_calls, Some(2));
    assert!(near(m.llm_cost, 0.2), "{}", m.llm_cost);
    assert_eq!(m.decider_calls, Some(1));
    assert!(near(m.decider_cost, 0.03));
    assert!(near(m.cost_usd, 0.23), "{}", m.cost_usd);
    // The store: root case and its child, the stage of each from the protocol id.
    let ids: Vec<(&str, &str)> = m.cases.iter().map(|c| (c.id.as_str(), c.stage.as_str())).collect();
    assert_eq!(ids, vec![(ROOT, "idea"), (CHILD, "intake")]);
    assert!(w.dir.join(format!("store/{ROOT}/children/{CHILD}/events.jsonl")).is_file());
    assert_eq!(m.cases[0].outcome.as_deref(), Some("operating"));
    assert_eq!(m.cases[1].outcome.as_deref(), Some("passed"));
    // Takes: the model writer is marked llm, the checks are not; a retry names the evidence kind.
    let dissect = take(CHILD, "idea.dissect");
    assert_eq!(dissect.len(), 2);
    assert!(dissect.iter().all(|t| t.llm && t.done && t.exit == "0"));
    assert!(dissect[0].retry_of.is_empty());
    assert_eq!(dissect[1].retry_of, vec!["dissection_shape".to_string()]);
    assert!(take(CHILD, "idea.check").iter().all(|t| !t.llm));
    let checks = take(CHILD, "dissection.check");
    assert_eq!(checks.iter().map(|t| t.exit.as_str()).collect::<Vec<_>>(), vec!["1", "0"]);
    assert_eq!(checks[0].evidence, vec![("dissection_shape".to_string(), "invalid".to_string())]);
    // A decision: the take carries the principal; the node is an approval (bindings.json).
    let approve = take(CHILD, "scope.approve");
    assert_eq!(approve[0].principal, "decider:model");
    assert!(m.cases[1].manual.contains(&"scope.approve".to_string()));
    // Node kinds: ir.json holds the protocol's effects.
    assert_eq!(m.cases[1].effect.get("idea.dissect").map(String::as_str), Some("write"));
    assert_eq!(m.cases[1].effect.get("idea.check").map(String::as_str), Some("read"));
    // The calls join to the takes by their start time, each to its own take.
    assert_eq!(m.unmatched_llm_calls, 0);
    assert!(near(dissect[0].cost, 0.12) && near(dissect[1].cost, 0.08), "{} {}", dissect[0].cost, dissect[1].cost);
    // Times: the span of the events, the sum of the steps' seconds.
    assert_eq!(m.wall_s, Some(130));
    assert_eq!(m.driver_s, Some(110.0));
    // Settings: the keys `check --runs-dir` compares.
    for (k, v) in [
        ("loom.sha256", "ab12"), ("loom.confinement", "local"), ("loom.cap_micro_usd", "20000000"), ("max_llm_calls", "150"),
        ("max_wall_seconds", "5400"), ("models.writer.model", "claude-sonnet-5-5"), ("models.implement.model", "claude-sonnet-5-5"),
        ("models.review.model", "claude-sonnet-5-5"), ("models.other.model", "claude-haiku-5-5"), ("decider.model", "claude-haiku-5-5"),
    ] {
        assert_eq!(m.settings.get(k).map(String::as_str), Some(v), "run-settings.jsonl {k}");
    }
    // Members a Loom run does not record stay absent.
    for f in ["free-agent.json", "defects.jsonl", "oracle.json", "run-stopped.json", "decision.json"] {
        assert!(!w.dir.join(f).exists(), "{f} must not be written");
    }
    assert_eq!(m.malformed, 0);
    // The loom/ files for `parity summarize`.
    assert!(w.dir.join("loom/status.json").is_file() && w.dir.join("loom/verify.json").is_file());
}

#[test]
fn a_run_stopped_by_the_spend_cap_is_finished_not_closed_and_not_delivered() {
    let _t = Timed::start("a_run_stopped_by_the_spend_cap_is_finished_not_closed_and_not_delivered");
    let mut w = World::new("capped", &settings());
    w.first_steps();
    w.ev(T0 + 95, Event::CapReached { cap: Cap::Spend, detail: "the spend ledger holds 20.0 USD, the cap is 20.0 USD".into() });
    let status = status_doc(&w.dir);
    put(&w.dir.join("tree-status.json"), &status.to_string());
    w.ev(T0 + 96, Event::TreeStored { status_exit: 0, verify_exit: 0 });
    w.ev(
        T0 + 96,
        Event::Ended { outcome: "stopped: spend cap".into(), reason: None, loom_exit: None, stderr_tail: None, exit_code: 4, steps: 5, spent_micro_usd: 200_000, llm_calls: 2 },
    );
    let m = read_run(&w.dir).unwrap();
    assert_eq!((m.finished, m.closed, m.delivered, m.clean), (true, false, false, false));
    assert_eq!(m.end_kind, "stopped");
    assert_eq!(m.end, format!("stopped: spend_cap in {CHILD}"));
    assert_eq!(m.stop_cause.as_deref(), Some("spend_cap"));
    assert_eq!(m.outcome, None);
    assert!(!w.dir.join("status.json").exists());
    let stop = json_opt(&w.dir.join("run-stopped.json")).unwrap();
    assert_eq!(stop["format"], "sdc.run-stopped/1");
    assert_eq!(stop["by"], "rule:loom-run");
    assert!(stop["at"].as_str().and_then(epoch_ms).is_some());
    assert!(stop["reason"].as_str().unwrap().contains("cap"));
    // No terminated event in any case: the tree did not end.
    for c in &m.cases {
        assert_eq!(c.outcome, None, "{}", c.id);
    }
    // What was spent before the stop stays in the record.
    assert_eq!(m.llm_calls, Some(2));
    assert!(near(m.cost_usd, 0.2), "{}", m.cost_usd);
}

#[test]
fn a_run_that_is_killed_leaves_a_directory_the_reader_takes_as_open() {
    let _t = Timed::start("a_run_that_is_killed_leaves_a_directory_the_reader_takes_as_open");
    let mut w = World::new("killed", &settings());
    w.first_steps();
    let dir = w.dir.clone();
    // The process dies: the writer is dropped with no end event.
    drop(w.w.take());
    let m = read_run(&dir).unwrap();
    assert_eq!((m.finished, m.closed, m.delivered), (false, false, false));
    assert_eq!(m.end_kind, "open");
    assert_eq!(m.malformed, 0, "every line written is whole");
    assert_eq!(m.llm_calls, Some(2));
    assert!(near(m.cost_usd, 0.2));
    assert_eq!(m.cases.len(), 2);
    assert_eq!(m.settings.get("decider.model").map(String::as_str), Some("claude-haiku-5-5"));
    // No half-written file stays behind.
    let mut files = vec![];
    all_files(&dir, &mut files);
    assert!(files.iter().all(|f| f.extension().is_none_or(|e| e != "tmp")), "{files:?}");
}

#[test]
fn a_run_killed_before_its_first_step_still_has_a_root_case_log_when_the_run_log_exists() {
    let _t = Timed::start("a_run_killed_before_its_first_step_still_has_a_root_case_log_when_the_run_log_exists");
    let mut w = World::new("early", &settings());
    w.start();
    w.root(vec![rl(0, "CaseOpened", json!({"case": ROOT, "protocol": "sdc@8"}))]);
    w.step(T0 + 5, 1, "Continue");
    drop(w.w.take());
    let m = read_run(&w.dir).unwrap();
    assert_eq!(m.cases.len(), 1);
    assert_eq!(m.end_kind, "open");
    let events = std::fs::read_to_string(w.dir.join(format!("store/{ROOT}/events.jsonl"))).unwrap();
    assert!(events.contains("\"opened\"") && events.contains("\"sdc\""));
}

#[test]
fn a_run_dir_holds_no_login_path_no_marker_no_home_path_and_no_environment() {
    let _t = Timed::start("a_run_dir_holds_no_login_path_no_marker_no_home_path_and_no_environment");
    std::env::set_var("LOOM_RUN_DIR_TEST_ENV_MARKER", "ENV-MARKER-4c1d");
    let login = format!("{HOME}tester/.claude/LOGIN-MARKER-7f3a/credentials.json");
    let mut raw = settings();
    // R1 gives `named`; the writer holds even when a raw path or a secret gets through.
    raw["login_file"] = json!(login.clone());
    raw["claude_credentials"] = json!(login);
    raw["api_token"] = json!("TOKEN-MARKER-91be");
    raw["max_tokens"] = json!(4000);
    let mut w = World::new("secrets", &raw);
    put(&w.dir.join("login-file-content.txt"), "LOGIN-CONTENT-MARKER-2e90");
    w.ev(T0, Event::RunStarted { run: "r".into(), settings: raw.clone() });
    w.first_steps_after_start();
    w.last_steps();
    // Cost records and decisions as the model tool and Loom print them: absolute paths everywhere.
    let mut files = vec![];
    all_files(&w.dir, &mut files);
    let skip = ["login-file-content.txt", "settings.json"];
    let mut checked = 0;
    for f in files {
        let name = f.file_name().unwrap().to_string_lossy().into_owned();
        // R1's files (`settings.json`, `tree-status.json`, `tree-verify.json`) and the fake Loom's
        // own store are not the writer's; its files are the rest.
        let rel = f.strip_prefix(&w.dir).unwrap().to_string_lossy().into_owned();
        if skip.contains(&name.as_str()) || rel.starts_with("store/runlog-") || rel == "tree-status.json" || rel == "tree-verify.json" || rel.starts_with("bindings/") || rel.starts_with("protocols/") {
            continue;
        }
        let text = std::fs::read_to_string(&f).unwrap();
        for bad in ["LOGIN-MARKER-7f3a", "TOKEN-MARKER-91be", "LOGIN-CONTENT-MARKER-2e90", "ENV-MARKER-4c1d", HOME, "credentials.json"] {
            assert!(!text.contains(bad), "{rel} holds {bad}");
        }
        let run_dir_text = w.dir.to_string_lossy().into_owned();
        assert!(!text.contains(&run_dir_text), "{rel} holds the run directory's absolute path");
        checked += 1;
    }
    assert!(checked >= 10, "only {checked} files were checked");
    // The setting that is a count survives: `max_tokens` is not a token.
    let line = std::fs::read_to_string(w.dir.join("run-settings.jsonl")).unwrap();
    let v: Value = serde_json::from_str(line.lines().last().unwrap()).unwrap();
    assert_eq!(v["max_tokens"], 4000);
    assert_eq!(v["login_file"], "named");
    assert_eq!(v["api_token"], "named");
}

#[test]
fn the_files_are_appended_line_by_line_as_the_events_arrive() {
    let _t = Timed::start("the_files_are_appended_line_by_line_as_the_events_arrive");
    let mut w = World::new("append", &settings());
    let lines = |dir: &Path, f: &str| std::fs::read_to_string(dir.join(f)).map(|t| t.lines().count()).unwrap_or(0);
    w.start();
    assert_eq!(lines(&w.dir, "run-settings.jsonl"), 1);
    assert_eq!((lines(&w.dir, "llm-calls.jsonl"), lines(&w.dir, "decisions.jsonl")), (0, 0));
    w.model_call(T0 + 5, 10_000, T0 + 2, "idea.dissect");
    assert_eq!(lines(&w.dir, "llm-calls.jsonl"), 1);
    w.step(T0 + 6, 1, "Continue");
    assert_eq!(lines(&w.dir, "rounds.jsonl"), 1);
    w.model_call(T0 + 9, 20_000, T0 + 7, "idea.dissect");
    assert_eq!(lines(&w.dir, "llm-calls.jsonl"), 2);
    let text = std::fs::read_to_string(w.dir.join("llm-calls.jsonl")).unwrap();
    let costs: Vec<f64> = text.lines().map(|l| serde_json::from_str::<Value>(l).unwrap()["cost_usd"].as_f64().unwrap()).collect();
    assert_eq!(costs, vec![0.01, 0.02]);
}

#[test]
fn a_decision_without_a_model_call_is_not_a_decider_call() {
    let _t = Timed::start("a_decision_without_a_model_call_is_not_a_decider_call");
    let mut w = World::new("rule", &settings());
    w.start();
    let decision = json!({"case": CHILD, "capability": "owner.approve", "action": "scope.approve", "kind": "ApproveAction", "revision": 3, "subject": "idea"});
    w.ev(T0 + 5, Event::DecisionAnswered { decision, approve: true, principal: "rule:autopilot".into(), cost_micro_usd: 0, model_calls: 0 });
    let m = read_run_without_store(&w.dir);
    assert_eq!(m.0, Some(0), "a rule's answer is a line without a model");
    let line: Value = serde_json::from_str(std::fs::read_to_string(w.dir.join("decisions.jsonl")).unwrap().lines().next().unwrap()).unwrap();
    assert_eq!((line["decision"].as_str(), line["principal"].as_str(), line["model"].is_null()), (Some("approve"), Some("rule:autopilot"), true));
}

/// decider_calls of a directory that has no case store yet.
fn read_run_without_store(dir: &Path) -> (Option<usize>, f64) {
    let mut bad = 0;
    let d = jsonl(&dir.join("decisions.jsonl"), &mut bad);
    let calls = d.as_ref().map(|d| d.iter().filter(|l| l["model"].as_str().is_some_and(|x| !x.is_empty())).count());
    (calls, 0.0)
}

#[test]
fn a_decider_call_without_a_model_name_is_counted_with_the_word_unknown_not_dropped() {
    let _t = Timed::start("a_decider_call_without_a_model_name_is_counted_with_the_word_unknown_not_dropped");
    let mut raw = settings();
    raw["decider"] = json!({"kind": "model"});
    let mut w = World::new("nomodel", &raw);
    w.ev(T0, Event::RunStarted { run: "r".into(), settings: raw });
    let decision = json!({"case": CHILD, "capability": "owner.approve", "action": "scope.approve", "kind": "ApproveAction", "revision": 3, "subject": "idea"});
    w.ev(T0 + 5, Event::DecisionAnswered { decision, approve: false, principal: "decider:model".into(), cost_micro_usd: 5_000, model_calls: 2 });
    let line: Value = serde_json::from_str(std::fs::read_to_string(w.dir.join("decisions.jsonl")).unwrap().lines().next().unwrap()).unwrap();
    assert_eq!(line["model"], "unknown");
    assert_eq!(line["decision"], "reject");
    assert_eq!(line["model_calls"], 2);
    assert_eq!(line["cost_usd"].as_f64(), Some(0.005));
}

#[test]
fn the_run_directory_names_the_files_of_parity_summarize_and_the_counts_line_is_the_fixtures() {
    let _t = Timed::start("the_run_directory_names_the_files_of_parity_summarize_and_the_counts_line_is_the_fixtures");
    let mut w = World::new("parity", &json!({"login_file": "named"}));
    // The committed status of the parity fixture, with its paths made absolute under the run
    // directory as Loom prints them, and the product tree in `work/`.
    let mut status: Value = serde_json::from_str(&std::fs::read_to_string(pfx().join("loom-status.json")).unwrap()).unwrap();
    let abs = |rel: &str| format!("{}/{rel}", w.dir.display());
    status["store"] = json!(abs("store"));
    status["workspace"] = json!(abs("work"));
    for c in status["cases"].as_array_mut().unwrap() {
        let case_file = format!("{}/{}", abs(""), c["case_file"].as_str().unwrap());
        let run_log = format!("{}/{}", abs(""), c["run_log"].as_str().unwrap());
        c["case_file"] = json!(case_file.replace("//", "/"));
        c["run_log"] = json!(run_log.replace("//", "/"));
    }
    put(&w.dir.join("tree-status.json"), &status.to_string());
    copy_tree(&pfx().join("product"), &w.dir.join("work"));
    w.ev(T0, Event::RunStarted { run: "r".into(), settings: json!({"login_file": "named"}) });
    w.ev(T0 + 1, Event::TreeStored { status_exit: 0, verify_exit: 0 });

    let written: Value = serde_json::from_str(&std::fs::read_to_string(w.dir.join("loom/status.json")).unwrap()).unwrap();
    let text = written.to_string();
    assert!(!text.contains(&w.dir.to_string_lossy().into_owned()) && !text.contains(HOME), "an absolute path in loom/status.json");
    assert_eq!(written["store"], "store");
    assert_eq!(written["workspace"], "work");
    assert_eq!(written["cases"][0]["case_file"], "store/case-root.json");
    assert_eq!(written["format"], "loom.tree-status/1");

    let out = w.dir.join("summary.json");
    let o = std::process::Command::new(BIN)
        .args(["parity", "summarize", "--loom-status"])
        .arg(w.dir.join("loom/status.json"))
        .arg("--product-tree")
        .arg(w.dir.join("work"))
        .arg("--output")
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(stdout(&o).trim(), "summary: 3 cases, 9 evidence records, 5 program runs, 2 person steps, 2 product files");
    // The same status, summarized from the fixture itself, gives the same file.
    let direct = std::process::Command::new(BIN)
        .args(["parity", "summarize", "--loom-status"])
        .arg(pfx().join("loom-status.json"))
        .arg("--product-tree")
        .arg(pfx().join("product"))
        .output()
        .unwrap();
    assert_eq!(std::fs::read_to_string(&out).unwrap(), stdout(&direct));
    let _ = path_str(&out);
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let p = e.path();
        let dst = to.join(e.file_name());
        if p.is_dir() {
            copy_tree(&p, &dst);
        } else {
            std::fs::copy(&p, &dst).unwrap();
        }
    }
}

#[test]
fn the_writer_refuses_a_case_id_that_is_not_a_directory_name() {
    let _t = Timed::start("the_writer_refuses_a_case_id_that_is_not_a_directory_name");
    let mut w = World::new("badid", &settings());
    w.start();
    w.root(vec![
        rl(0, "CaseOpened", json!({"case": ROOT, "protocol": "sdc@8"})),
        rl(1, "ChildOpened", json!({"child": "../../escape", "protocol": "sdc-intake@9"})),
    ]);
    let err = w.w.as_mut().unwrap().event(T0 + 5, &Event::StepMade { index: 1, exit: 0, stop: "Continue".into(), steps: 1 }).unwrap_err();
    assert!(err.contains("not a directory name"), "{err}");
}

#[test]
fn a_settings_value_that_is_not_an_object_or_a_missing_run_directory_is_refused() {
    let _t = Timed::start("a_settings_value_that_is_not_an_object_or_a_missing_run_directory_is_refused");
    let dir = scratch("refuse");
    assert!(RunDirWriter::open(&dir, &json!("settings")).is_err());
    assert!(RunDirWriter::open(&dir.join("nothing"), &json!({})).is_err());
    put(&dir.join("free-agent.json"), "{}");
    assert!(RunDirWriter::open(&dir, &json!({})).is_err());
}

#[test]
fn the_launchers_stop_record_is_not_overwritten() {
    let _t = Timed::start("the_launchers_stop_record_is_not_overwritten");
    let mut w = World::new("launcher", &settings());
    w.start();
    put(&w.dir.join("run-stopped.json"), "{\"by\":\"person:operator\",\"cause\":\"stopped by the launcher\"}");
    w.ev(T0 + 3, Event::CapReached { cap: Cap::WallTime, detail: "wall time".into() });
    let v = json_opt(&w.dir.join("run-stopped.json")).unwrap();
    assert_eq!(v["by"], "person:operator");
}
