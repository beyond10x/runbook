//! `sdc-run-timeline` on small run directories written by the tests (docs/llm-is-not-the-runtime.md).
//! Each fixture is built here with known times, so every span and total can be counted by hand.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("run-timeline").join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn jsonl(path: &Path, lines: &[Value]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    fs::write(path, text).unwrap();
}

fn write_json(path: &Path, v: &Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, v.to_string()).unwrap();
}

/// `2026-11-21T10:MM:SS.mmmZ` for millisecond `ms` after 10:00:00.
fn at(ms: u64) -> String {
    let s = ms / 1000;
    format!("2026-11-21T10:{:02}:{:02}.{:03}Z", s / 60, s % 60, ms % 1000)
}

fn at_whole(s: u64) -> String {
    format!("2026-11-21T10:{:02}:{:02}Z", s / 60, s % 60)
}

const EPOCH_10H: i64 = 1_795_255_200; // 2026-11-21T10:00:00Z

/// One case: events with observed times (ms), evaluations, evidence.
struct CaseW {
    dir: PathBuf,
    events: Vec<Value>,
    observed: Vec<Value>,
    ev_n: usize,
}

impl CaseW {
    fn new(dir: PathBuf, protocol: &str, ir: Value, manual: &[&str], t: u64) -> Self {
        write_json(&dir.join("ir.json"), &ir);
        let b: serde_json::Map<String, Value> = ir["actions"].as_object().unwrap().keys()
            .map(|a| (a.clone(), if manual.contains(&a.as_str()) { json!({"manual": true, "argv": ["true"]}) } else { json!({"argv": ["x"]}) }))
            .collect();
        write_json(&dir.join("bindings.json"), &Value::Object(b));
        let mut c = CaseW { dir, events: vec![], observed: vec![], ev_n: 0 };
        c.push(t, json!({"event": "opened", "at": "-", "protocol": {"id": protocol, "revision": 1}}));
        c
    }

    fn push(&mut self, t: u64, mut e: Value) {
        let i = self.events.len();
        e["seq"] = json!(i);
        if e.get("at").is_none() {
            e["at"] = json!(at_whole(t / 1000));
        }
        self.observed.push(json!({"format": "sdc.observed/1", "index": i, "event": e["event"], "observed_at": at(t)}));
        self.events.push(e);
    }

    /// An `evaluated` event whose decision holds `claims` (id -> (value, evidence ids)).
    fn evaluate(&mut self, t: u64, claims: &[(&str, &str, &[&str])]) {
        let file = format!("evaluations/{:04}.json", self.events.len());
        let mut c = serde_json::Map::new();
        let mut ex = serde_json::Map::new();
        for (id, v, ev) in claims {
            c.insert(id.to_string(), json!({"value": v}));
            let because: Vec<Value> = ev.iter().map(|e| json!({"evidence": e, "status": "applied"})).collect();
            ex.insert(id.to_string(), json!({"value": v, "because": because}));
        }
        write_json(&self.dir.join(&file), &json!({"format": "canon-decision/1", "claims": c, "explanation": {"claims": ex}}));
        self.push(t, json!({"event": "evaluated", "file": file}));
    }

    /// One take from `t0` to `t1` (ms); an evidence record of `kind` when not empty.
    fn take(&mut self, action: &str, llm: bool, t0: u64, t1: u64, kind: &str, principal: Option<&str>) {
        self.push(t0, json!({"event": "action_started", "action": action, "llm": llm, "retry_of": [],
            "argv": ["/opt/private-tree/tool.sh"]}));
        let mut evidence = vec![];
        if !kind.is_empty() {
            self.ev_n += 1;
            let id = format!("e{:04}-{action}", self.ev_n);
            write_json(&self.dir.join("evidence").join(format!("{id}.json")), &json!({"id": id, "kind": kind, "result": "passed"}));
            evidence.push(id);
        }
        self.push(t1, json!({"event": "action_finished", "action": action, "exit": "0", "evidence": evidence, "principal": principal}));
    }

    fn finish(mut self, t: u64, outcome: &str, principal: Option<&str>) {
        self.push(t, json!({"event": "terminated", "outcome": outcome, "principal": principal}));
        jsonl(&self.dir.join("events.jsonl"), &self.events);
        jsonl(&self.dir.join("observed.jsonl"), &self.observed);
    }
}

/// A run: root case IDEA-1 (sdc) with one child IDEA-1.spec (sdc.spec).
///
/// Child, t in seconds: 0.0-1.0 program `spec.check` (admissible on spec.checked unknown, no
/// evidence), 2.0-12.0 model `spec.author` (one session 2.1-11.9, cost 0.5, 3 API requests,
/// tool calls Read 2 and Write 1), 13.0-14.0 program `spec.check` again (spec.written true from
/// e0001), 30.0-30.0 approval `spec.approve` by the decider; decider call 20.0-26.0 (cost 0.25,
/// 2 API requests, 1 tool call StructuredOutput). Root: terminated at 31.0 by rule:autopilot.
/// Rounds: 0-15 s and 30-31 s; the gap 15-30 s lies between rounds.
fn sdc_run(run: &Path) {
    let root_dir = run.join("store/IDEA-1");
    let ir_root = json!({"actions": {"spec.open": {"effect": "write", "precondition": {"claim": {"id": "idea.ok", "is": "true"}}}}});
    let mut root = CaseW::new(root_dir.clone(), "sdc", ir_root, &[], 0);
    let ir = json!({"actions": {
        "spec.check": {"effect": "read", "precondition": {"claim": {"id": "spec.checked", "is": "unknown"}}},
        "spec.author": {"effect": "write", "precondition": {"all": [
            {"any": [{"claim": {"id": "spec.valid", "is": "false"}}, {"claim": {"id": "spec.valid", "is": "unknown"}}]},
            {"claim": {"id": "requirements.valid", "is": "true"}}]}},
        "spec.approve": {"effect": "write", "precondition": {"claim": {"id": "spec.valid", "is": "true"}}}}});
    let mut c = CaseW::new(root_dir.join("children/IDEA-1.spec"), "sdc.spec", ir, &["spec.approve"], 0);
    c.evaluate(0, &[("spec.checked", "unknown", &[]), ("spec.valid", "unknown", &[]), ("requirements.valid", "true", &["e9-requirements.check"])]);
    c.take("spec.check", false, 0, 1000, "", None);
    c.evaluate(1500, &[("spec.checked", "unknown", &[]), ("spec.valid", "unknown", &[]), ("requirements.valid", "true", &["e9-requirements.check"])]);
    c.take("spec.author", true, 2000, 12000, "", None);
    c.evaluate(12500, &[("spec.checked", "unknown", &[]), ("spec.written", "true", &["e0001"])]);
    c.take("spec.check", false, 13000, 14000, "spec_shape", None);
    c.evaluate(29000, &[("spec.valid", "true", &["e0001-spec.check"])]);
    c.take("spec.approve", false, 30000, 30000, "", Some("decider:haiku"));
    c.finish(30500, "passed", None);
    root.push(0, json!({"event": "revised", "artifacts": {"idea": "t1"}}));
    root.finish(31000, "operating", Some("rule:autopilot"));

    let rec = run.join("llm/001-spec.author");
    write_json(&rec.join("status.json"), &json!({"format": "sdc.call-status/1", "state": "finished", "started_at": at(2100), "finished_at": at(11900), "exit": 0}));
    let mh = |seq: u64, e: Value| { let mut e = e; e["format"] = json!("metaharness.event/1"); e["seq"] = json!(seq); e };
    jsonl(&rec.join("events.jsonl"), &[
        mh(1, json!({"event": "session.started", "model": "claude-sonnet-5-5", "cwd": "/opt/private-tree/cwd"})),
        mh(2, json!({"event": "tool.requested", "at": at(3000), "call_id": "t1", "name": "Read"})),
        mh(3, json!({"event": "usage", "at": at(3000), "request_id": "r1", "usage": {"input_tokens": 2}})),
        mh(4, json!({"event": "tool.requested", "at": at(5000), "call_id": "t2", "name": "Read"})),
        mh(5, json!({"event": "usage", "at": at(5000), "request_id": "r2", "usage": {"input_tokens": 2}})),
        mh(6, json!({"event": "tool.requested", "at": at(8000), "call_id": "t3", "name": "Write"})),
        mh(7, json!({"event": "usage", "at": at(8000), "request_id": "r2", "usage": {"input_tokens": 2}})),
        mh(8, json!({"event": "usage", "at": at(11000), "request_id": "r3", "usage": {"input_tokens": 2}})),
        mh(9, json!({"event": "session.ended", "num_turns": 3})),
    ]);
    jsonl(&run.join("llm-calls.jsonl"), &[json!({"action": "spec.author", "case": "IDEA-1.spec", "started": at_whole(2),
        "attempt": 1, "adapter": "metaharness", "model": "claude-sonnet-5-5", "effort": "high", "class": "writer",
        "exit": 0, "duration_ms": 9800, "cost_usd": 0.5, "num_turns": 3,
        "usage": {"input_tokens": 6, "output_tokens": 700, "cache_read_input_tokens": 50000, "cache_creation_input_tokens": 4000},
        "record": rec.display().to_string()})]);

    let drec = run.join("decider/001-spec.approve");
    write_json(&drec.join("status.json"), &json!({"format": "sdc.call-status/1", "state": "finished", "started_at": at(20000), "finished_at": at(26000), "exit": 0}));
    jsonl(&drec.join("events.jsonl"), &[
        json!({"type": "system", "subtype": "init"}),
        json!({"type": "assistant", "request_id": "q1", "message": {"id": "m1", "content": [{"type": "thinking"}]}}),
        json!({"type": "assistant", "request_id": "q1", "message": {"id": "m1", "content": [{"type": "text"}]}}),
        json!({"type": "assistant", "request_id": "q2", "message": {"id": "m2", "content": [{"type": "tool_use", "id": "u1", "name": "StructuredOutput"}]}}),
        json!({"type": "result", "num_turns": 3, "total_cost_usd": 0.25}),
    ]);
    jsonl(&run.join("decisions.jsonl"), &[
        json!({"time": at_whole(26), "case": "IDEA-1.spec", "type": "approval", "class": "approval", "action": "spec.approve",
            "decision": "approve", "principal": "decider:haiku", "model": "claude-opus-5-5", "effort": "xhigh", "adapter": "claude-cli",
            "reasons": ["a reason that must not leak"],
            "tokens": {"input_tokens": 10, "output_tokens": 300, "cache_read_input_tokens": 20000, "cache_creation_input_tokens": 5000},
            "cost_usd": 0.25, "duration_ms": 6000, "valid": true, "record": drec.display().to_string()}),
        json!({"time": at_whole(27), "case": "IDEA-1.spec", "type": "approval", "class": "approval", "action": "spec.approve",
            "decision": "approve", "model": "claude-opus-5-5", "suppressed": true, "cost_usd": 0.25}),
    ]);
    jsonl(&run.join("rounds.jsonl"), &[
        json!({"started_epoch": EPOCH_10H, "ended_epoch": EPOCH_10H + 15, "seconds": 15, "verdict": "awaiting_authority"}),
        json!({"started_epoch": EPOCH_10H + 30, "ended_epoch": EPOCH_10H + 31, "seconds": 1, "verdict": "terminable"}),
    ]);
    jsonl(&run.join("run-settings.jsonl"), &[json!({"format": "sdc.run-settings/1", "at": at_whole(0), "adapter": "metaharness",
        "decider": {"kind": "haiku", "model": "claude-opus-5-5"},
        "models": {"writer": {"model": "claude-sonnet-5-5"}, "decider": {"model": "claude-opus-5-5"}},
        "efforts": {"writer": {"effort": "high"}, "decider": {"effort": "xhigh"}}})]);
    write_json(&run.join("status.json"), &json!({"format": "sdc.run-status/1", "state": "closed", "outcome": "operating", "delivered": true}));
}

fn timeline(run: &Path, extra: &[&str]) -> (i32, Value, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_sdc-run-timeline")).arg(run).args(["--json", "-"]).args(extra).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let doc = serde_json::from_str(&text).unwrap_or(Value::Null);
    (out.status.code().unwrap_or(-1), doc, String::from_utf8_lossy(&out.stderr).into_owned())
}

fn lane<'a>(doc: &'a Value, l: &str) -> Vec<&'a Value> {
    doc["spans"].as_array().unwrap().iter().filter(|s| s["lane"] == l).collect()
}

fn close(a: &Value, b: f64) -> bool {
    a.as_f64().is_some_and(|x| (x - b).abs() < 1e-6)
}

#[test]
fn an_sdc_run_is_programs_model_sessions_decider_calls_a_rule_step_and_idle_time() {
    let run = scratch("sdc").join("demo-run");
    sdc_run(&run);
    let (code, doc, err) = timeline(&run, &[]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(doc["format"], "sdc.run-timeline/1");
    assert_eq!(doc["kind"], "sdc");
    assert_eq!(doc["run"], "demo-run");
    assert_eq!(doc["settings"]["models"]["writer"], "claude-sonnet-5-5");
    assert_eq!(doc["settings"]["efforts"]["decider"], "xhigh");
    let tree = doc["tree"].as_array().unwrap();
    assert_eq!(tree.len(), 2);
    assert_eq!(tree[1]["case"], "IDEA-1.spec");
    assert_eq!(tree[1]["parent"], "IDEA-1");
    assert_eq!(tree[1]["stage"], "spec");

    let programs = lane(&doc, "program");
    assert_eq!(programs.len(), 3, "two spec.check takes and the approved spec.approve");
    assert!(close(&programs[0]["t0_s"], 0.0) && close(&programs[0]["t1_s"], 1.0));
    // The cause: the claims of the action's precondition that held in the decision before it.
    assert_eq!(programs[0]["cause"]["claims"], json!([{"claim": "spec.checked", "value": "unknown", "evidence": []}]));
    assert_eq!(programs[2]["action"], "spec.approve");
    assert_eq!(programs[2]["principal"], "decider:haiku");

    let models = lane(&doc, "model");
    assert_eq!(models.len(), 1);
    let m = models[0];
    assert_eq!(m["case"], "IDEA-1.spec");
    assert_eq!(m["action"], "spec.author");
    assert!(close(&m["t0_s"], 2.0) && close(&m["t1_s"], 12.0));
    assert_eq!(m["model"], "claude-sonnet-5-5");
    assert_eq!(m["effort"], "high");
    assert_eq!(m["api_requests"], 3);
    assert_eq!(m["tool_calls"], json!({"Read": 2, "Write": 1}));
    assert_eq!(m["tokens"], json!({"input": 6, "output": 700, "cache_read": 50000, "cache_write": 4000}));
    assert!(close(&m["cost_usd"], 0.5));
    assert_eq!(m["cause"]["claims"], json!([
        {"claim": "spec.valid", "value": "unknown", "evidence": []},
        {"claim": "requirements.valid", "value": "true", "evidence": ["e9-requirements.check"]}]));

    let deciders = lane(&doc, "decider");
    assert_eq!(deciders.len(), 1, "the suppressed repeat line is no call");
    let d = deciders[0];
    assert!(close(&d["t0_s"], 20.0) && close(&d["t1_s"], 26.0));
    assert_eq!(d["asked"], "approval/approval");
    assert_eq!(d["decision"], "approve");
    assert_eq!(d["api_requests"], 2);
    assert_eq!(d["tool_calls"], json!({"StructuredOutput": 1}));
    assert!(close(&d["cost_usd"], 0.25));

    let rules = lane(&doc, "rule");
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0]["case"], "IDEA-1");
    assert_eq!(rules[0]["action"], "terminated:operating");

    // Idle: 1-2 driver, 12-13 driver, 14-15 driver, 15-20 between rounds, 26-30 between rounds,
    // 30-31 driver (the close after the approval): 13 s.
    let idle = lane(&doc, "idle");
    let idle_s: f64 = idle.iter().map(|s| s["t1_s"].as_f64().unwrap() - s["t0_s"].as_f64().unwrap()).sum();
    assert!((idle_s - 13.0).abs() < 1e-6, "idle {idle_s}");
    assert!(idle.iter().any(|s| s["cause"] == "between rounds"));
    assert!(idle.iter().any(|s| s["cause"] == "driver"));

    let t = &doc["totals"];
    assert!(close(&t["wall_s"], 31.0));
    assert!(close(&t["cost_usd"], 0.75));
    assert_eq!(t["program_steps"], 3);
    assert_eq!(t["model_sessions"], 1);
    assert_eq!(t["api_requests"], 5);
    assert_eq!(t["model_tool_calls"], 3);
    assert_eq!(t["decider_calls"], 1);
    assert_eq!(t["rule_steps"], 1);
    assert!(close(&t["model_s"], 10.0));
    assert!(close(&t["program_s"], 2.0));
    assert!(close(&t["decider_s"], 6.0));
    assert!(close(&t["idle_s"], 13.0));
    assert!(close(&t["waiting_on_person_s"], 0.0));
    assert_eq!(t["tokens"], json!({"input": 16, "output": 1000, "cache_read": 70000, "cache_write": 9000}));

    // Sanitized: no absolute path, no reason text.
    let text = doc.to_string();
    assert!(!text.contains("/home/") && !text.contains("/opt/private-tree"), "{text}");
    assert!(!text.contains("must not leak"));
}

#[test]
fn the_check_holds_totals_to_the_spans_and_cost_to_sdc_run_metrics() {
    let run = scratch("check").join("demo-run");
    sdc_run(&run);
    let (code, _, err) = timeline(&run, &["--check"]);
    assert_eq!(code, 0, "{err}");
    let out = Command::new(env!("CARGO_BIN_EXE_sdc-run-metrics")).arg(&run).args(["--json", "-"]).output().unwrap();
    let metrics: Value = serde_json::from_slice(&out.stdout).unwrap();
    let (_, doc, _) = timeline(&run, &[]);
    assert!(close(&doc["totals"]["cost_usd"], metrics["metrics"]["cost_usd"].as_f64().unwrap()));

    // A stale document: a total that is not the sum of its spans fails the check.
    let mut stale = doc.clone();
    stale["totals"]["model_s"] = json!(99.0);
    let f = run.parent().unwrap().join("stale.json");
    fs::write(&f, stale.to_string()).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_sdc-run-timeline")).args(["--check-doc"]).arg(&f).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("model_s"));
}

#[test]
fn the_same_run_gives_the_same_bytes_and_a_missing_run_is_exit_2() {
    let run = scratch("bytes").join("demo-run");
    sdc_run(&run);
    let a = Command::new(env!("CARGO_BIN_EXE_sdc-run-timeline")).arg(&run).args(["--json", "-"]).output().unwrap();
    let b = Command::new(env!("CARGO_BIN_EXE_sdc-run-timeline")).arg(&run).args(["--json", "-"]).output().unwrap();
    assert_eq!(a.stdout, b.stdout);
    let out = Command::new(env!("CARGO_BIN_EXE_sdc-run-timeline")).arg(run.join("nothing")).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}
