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

/// A free-running agent's run (sdc-free-agent): the stream of `claude -p` with partial messages,
/// in the shape the CLI writes it. Seconds: init 0.0; request 1 sent 0.5 (status requesting),
/// first token 1.0, a Read tool call 2.0-2.5 while it streams, stop 3.0; request 2 sent 3.2
/// (the tool result was at 2.5, the status line says 3.2), stop 5.0; result at 5.2.
fn free_run(run: &Path, outcome: Option<Value>) {
    let ev = |t: u64, e: Value| (t, e);
    let usage = |i: i64, o: i64, cr: i64, cw: i64| json!({"input_tokens": i, "output_tokens": o, "cache_read_input_tokens": cr, "cache_creation_input_tokens": cw});
    let lines = vec![
        ev(0, json!({"type": "system", "subtype": "init", "cwd": "/opt/private-tree/work"})),
        ev(500, json!({"type": "system", "subtype": "status", "status": "requesting"})),
        ev(1000, json!({"type": "stream_event", "event": {"type": "message_start", "message": {"id": "m1", "model": "claude-sonnet-5-5", "usage": usage(2, 16, 0, 5000)}}})),
        ev(2000, json!({"type": "assistant", "message": {"id": "m1", "content": [{"type": "tool_use", "id": "u1", "name": "Read", "input": {"file_path": "/opt/private-tree/secret"}}]}})),
        ev(2500, json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "u1", "content": "file text that must not leak"}]}})),
        ev(2900, json!({"type": "stream_event", "event": {"type": "message_delta", "usage": usage(2, 300, 0, 5000)}})),
        ev(3000, json!({"type": "stream_event", "event": {"type": "message_stop"}})),
        ev(3200, json!({"type": "system", "subtype": "status", "status": "requesting"})),
        ev(3600, json!({"type": "stream_event", "event": {"type": "message_start", "message": {"id": "m2", "model": "claude-sonnet-5-5", "usage": usage(2, 10, 5000, 400)}}})),
        ev(4900, json!({"type": "stream_event", "event": {"type": "message_delta", "usage": usage(2, 50, 5000, 400)}})),
        ev(5000, json!({"type": "stream_event", "event": {"type": "message_stop"}})),
        ev(5200, json!({"type": "result", "subtype": "success", "is_error": false, "total_cost_usd": 0.42, "num_turns": 2,
            "usage": usage(4, 350, 5000, 5400), "result": "a reply that must not leak",
            "modelUsage": {"claude-sonnet-5-5": {"inputTokens": 4, "outputTokens": 350, "cacheReadInputTokens": 5000, "cacheCreationInputTokens": 5400, "costUSD": 0.42}}})),
    ];
    jsonl(&run.join("events.jsonl"), &lines.iter().map(|(_, e)| e.clone()).collect::<Vec<_>>());
    jsonl(&run.join("received.jsonl"), &lines.iter().enumerate().map(|(i, (t, _))| json!({"index": i, "received_at": at(*t)})).collect::<Vec<_>>());
    write_json(&run.join("free-agent.json"), &json!({"format": "sdc.free-agent-run/1", "run": "free-1", "end": "completed", "capped": null, "exit": 0,
        "model": "claude-sonnet-5-5", "effort": "high", "adapter": "claude-cli", "tools": ["Read", "Bash"],
        "caps": {"max_usd": 25.0, "max_wall_s": 5400, "max_turns": 1000}, "started_at": at(0), "ended_at": at(5300),
        "confinement": {"summary": "bubblewrap", "argv": []}}));
    if let Some(o) = outcome {
        write_json(&run.join("outcome.json"), &o);
    }
}

#[test]
fn a_free_agent_run_is_one_model_span_per_request_and_one_tool_span_per_call() {
    let run = scratch("free").join("free-1");
    free_run(&run, None);
    let (code, doc, err) = timeline(&run, &["--check"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(doc["kind"], "free-agent");
    assert_eq!(doc["settings"]["model"], "claude-sonnet-5-5");
    let models = lane(&doc, "model");
    assert_eq!(models.len(), 2);
    assert!(close(&models[0]["t0_s"], 0.5) && close(&models[0]["t1_s"], 3.0));
    assert!(close(&models[0]["ttft_s"], 0.5));
    assert_eq!(models[0]["tokens"], json!({"input": 2, "output": 300, "cache_read": 0, "cache_write": 5000}));
    assert_eq!(models[0]["tool_calls"], json!({"Read": 1}));
    assert!(models[0]["cost_usd"].is_null());
    assert!(close(&models[1]["t0_s"], 3.2) && close(&models[1]["t1_s"], 5.0));
    let tools = lane(&doc, "tool");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["action"], "Read");
    assert!(close(&tools[0]["t0_s"], 2.0) && close(&tools[0]["t1_s"], 2.5));
    let t = &doc["totals"];
    assert!(close(&t["cost_usd"], 0.42));
    assert_eq!(t["api_requests"], 2);
    assert_eq!(t["model_tool_calls"], 1);
    assert_eq!(t["tokens"], json!({"input": 4, "output": 350, "cache_read": 5000, "cache_write": 5400}));
    assert!(close(&t["wall_s"], 5.3));
    // 0-0.5 before the first request, 3.0-3.2 between requests, 5.0-5.3 after the last.
    assert!(close(&t["idle_s"], 1.0), "{}", t["idle_s"]);
    assert!(close(&t["overlap_s"], 0.5), "the tool ran while its response streamed");
    assert_eq!(doc["session"]["usage_beyond_streamed_requests"], json!({"input": 0, "output": 0, "cache_read": 0, "cache_write": 0}));
    let text = doc.to_string();
    assert!(!text.contains("/opt/private-tree") && !text.contains("must not leak"), "{text}");
}

fn sha(text: &str) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// A prospective record comparing runners: the free agent against two SDC variants whose runs
/// another record made (`from`), on two primary metrics.
fn runner_record(free_settings: &str) -> String {
    let idea = "A to-do list.";
    format!(r#"format: sdc.experiment/1
id: cmp
kind: prospective
status: running
created: 2026-11-21
title: Free agent against the SDC
purpose: Whether one free agent session costs more than the SDC.
hypothesis: A free-running agent costs more (run.cost_usd and run.tokens_total) than the SDC.
inputs:
- name: todo
  kind: idea
  text: {idea}
  sha256: {}
tree_commit: 0123456789abcdef0123456789abcdef01234567
target:
  kind: run
  name: run.cost_usd
variable: runner
known_confounds:
- key: decider
  note: the SDC settings differ in the decider
variants:
- id: strong
  from: base
  settings:
    models.writer.model: s
    models.implement.model: s
    models.review.model: s
    models.other.model: s
    decider.kind: haiku
    decider.model: claude-opus-5-5
    decider.escalate: final
    decider.max_decider_calls: '40'
    adapter: metaharness
    max_llm_calls: '150'
    autonomy: A5
  runs: [sdc-a]
- id: mixed
  from: base
  settings:
    models.writer.model: s
    models.implement.model: s
    models.review.model: s
    models.other.model: h
    decider.kind: haiku
    decider.model: claude-opus-5-5
    decider.escalate: final
    decider.max_decider_calls: '40'
    adapter: metaharness
    max_llm_calls: '150'
    autonomy: A5
  runs: [sdc-b]
- id: sonnet
  runner: free-agent
  run_name: free-{{n}}
  settings:
{free_settings}
  runs: [free-1]
baseline: [strong, mixed]
treatment: sonnet
runs_per_variant: 1
pass_rule: clean-delivery
primary:
- metric: run.cost_usd
  direction: increase
  min_effect: 0
- metric: run.tokens_total
  direction: increase
  min_effect: 0
guards:
- metric: run.delivered
  worse: decrease
  max_worsening: 0
decision_rule:
  rule: complete-separation
  text: held when on both metrics against both SDC variants every free run lies above every SDC run.
adoption:
  decision: pending
  commit: null
"#, sha(idea))
}

const FREE_SETTINGS: &str = "    free.model: claude-sonnet-5-5\n    free.effort: high\n    free.adapter: claude-cli\n    free.max_usd: '25'\n    free.max_wall_s: '5400'\n    free.max_turns: '1000'";

#[test]
fn a_runner_comparison_record_is_checked_launched_from_its_free_variant_only_and_reported_per_baseline() {
    let base = scratch("runner-record");
    let runs = base.join("runs");
    let exp = base.join("experiments");
    fs::create_dir_all(&exp).unwrap();
    for (name, other) in [("sdc-a", "s"), ("sdc-b", "h")] {
        sdc_run(&runs.join(name));
        jsonl(&runs.join(name).join("run-settings.jsonl"), &[json!({"format": "sdc.run-settings/1", "at": at_whole(0), "adapter": "metaharness",
            "autonomy": "A5", "max_llm_calls": 150,
            "decider": {"kind": "haiku", "model": "claude-opus-5-5", "escalate": "final", "max_decider_calls": 40},
            "models": {"writer": {"model": "s"}, "implement": {"model": "s"}, "review": {"model": "s"}, "other": {"model": other}, "decider": {"model": "claude-opus-5-5"}}})]);
    }
    free_run(&runs.join("free-1"), Some(json!({"ended_by_itself": true, "delivered": true})));
    fs::write(exp.join("cmp.yaml"), runner_record(FREE_SETTINGS)).unwrap();
    let exe = env!("CARGO_BIN_EXE_sdc-experiment");
    let o = Command::new(exe).args(["check"]).arg(exp.join("cmp.yaml")).arg("--runs-dir").arg(&runs).output().unwrap();
    assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    // Only the free variant is launched, under its run_name, by sdc-free-agent.
    let o = Command::new(exe).args(["run"]).arg(exp.join("cmp.yaml")).args(["--print", "--runs-dir"]).arg(&runs).output().unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success(), "{text}");
    assert!(text.contains("free-1 (") && text.contains("sdc-free-agent run") && text.contains("--max-usd 25"), "{text}");
    assert!(!text.contains("cmp.strong"), "{text}");
    // A free variant with an SDC setting is refused.
    fs::write(exp.join("cmp.yaml"), runner_record(&format!("{FREE_SETTINGS}\n    autonomy: A5"))).unwrap();
    let o = Command::new(exe).args(["check"]).arg(exp.join("cmp.yaml")).output().unwrap();
    assert!(String::from_utf8_lossy(&o.stdout).contains("autonomy is no free-agent launch setting"));
    fs::write(exp.join("cmp.yaml"), runner_record(FREE_SETTINGS)).unwrap();
    // The report: per metric and baseline; the free run here is cheaper, so contradicted.
    let o = Command::new(exe).args(["report"]).arg(exp.join("cmp.yaml")).arg("--runs-dir").arg(&runs).output().unwrap();
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success(), "{out}{}", String::from_utf8_lossy(&o.stderr));
    let rec: serde_yaml::Value = serde_yaml::from_str(&fs::read_to_string(exp.join("cmp.yaml")).unwrap()).unwrap();
    let res = &rec["result"];
    assert_eq!(res["verdict"].as_str(), Some("contradicted"), "{out}");
    let comps = res["comparisons"].as_sequence().unwrap();
    assert_eq!(comps.len(), 4);
    assert!(comps.iter().all(|c| c["separation"].as_str() == Some("reversed")));
    let report = fs::read_to_string(exp.join("cmp/report.md")).unwrap();
    assert!(report.contains("`run.tokens_total` (increase), sonnet against mixed: contradicted"), "{report}");
    let summary = fs::read_to_string(exp.join("cmp/runs/free-1.json")).unwrap();
    assert!(summary.contains("sdc.free-agent-metrics/1") && !summary.contains("/home/"));
    let o = Command::new(exe).args(["check", "--all", "--experiments"]).arg(&exp).output().unwrap();
    let all = String::from_utf8_lossy(&o.stdout);
    assert!(!all.contains("cmp.yaml") && !all.contains("cmp/report.md"), "{all}");
}

#[test]
fn a_free_agent_run_is_reported_in_the_metric_shape_once_its_outcome_is_measured() {
    let base = scratch("free-exp");
    free_run(&base.join("free-1"), None);
    // Without outcome.json the run is not finished; with it, delivered as measured.
    let rec = |dir: &Path| -> Value {
        let out = Command::new(env!("CARGO_BIN_EXE_sdc-run-timeline")).arg(dir).args(["--metrics", "-"]).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        serde_json::from_slice(&out.stdout).unwrap()
    };
    let m = rec(&base.join("free-1"));
    assert_eq!(m["metrics"]["finished"], false);
    free_run(&base.join("free-2"), Some(json!({"ended_by_itself": true, "delivered": true})));
    let m = rec(&base.join("free-2"));
    assert_eq!(m["metrics"]["finished"], true);
    assert_eq!(m["metrics"]["delivered"], true);
    assert_eq!(m["metrics"]["clean"], true);
    assert!(close(&m["metrics"]["cost_usd"], 0.42));
    assert_eq!(m["settings"]["free.max_usd"], "25");
    assert_eq!(m["metrics"]["tokens"]["cache_creation"], 5400);
}

#[test]
fn a_product_tree_is_sized_by_its_tests_its_product_lines_and_whether_its_readme_says_how_to_start_it() {
    let p = scratch("size").join("product");
    fs::create_dir_all(p.join("src")).unwrap();
    fs::create_dir_all(p.join("tests")).unwrap();
    fs::create_dir_all(p.join("target/debug")).unwrap();
    fs::write(p.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();
    fs::write(
        p.join("src/lib.rs"),
        "// a comment\n\npub fn add(a: u32, b: u32) -> u32 {\n    a + b\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn adds() {\n        assert_eq!(add(1, 2), 3);\n    }\n}\n",
    )
    .unwrap();
    fs::write(p.join("src/main.rs"), "fn main() {\n    println!(\"{}\", demo::add(1, 1));\n}\n").unwrap();
    fs::write(p.join("tests/api.rs"), "#[test]\nfn one() {\n    assert!(true);\n}\n\n#[test]\nfn two() {}\n").unwrap();
    fs::write(p.join("target/debug/junk.rs"), "#[test]\nfn not_counted() {}\n").unwrap();
    fs::write(p.join("src/index.html"), "<html>\n\n<body></body>\n</html>\n").unwrap();
    fs::write(p.join("src/sw.js"), "self.addEventListener('fetch', () => {});\n").unwrap();
    fs::write(p.join("target/debug/x.js"), "not counted\n").unwrap();
    fs::write(p.join("README.md"), "# Demo\n\n## Run\n\n    cargo run --locked -- 8080\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_sdc-free-agent")).arg("size").arg(&p).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["format"], "sdc.product-size/1");
    assert_eq!(v["rust_files"], 3);
    assert_eq!(v["tests"], 3);
    // lib.rs outside its test module: 3 lines (the comment and blank lines do not count); main.rs: 3.
    assert_eq!(v["product_lines"], 6);
    // lib.rs's test module (8 lines, from #[cfg(test)] to its closing brace) and tests/api.rs (6).
    assert_eq!(v["test_lines"], 14);
    // Web assets kept beside the Rust (a product may embed them in Rust strings instead).
    assert_eq!(v["asset_lines"], 4);
    // Files of the tree outside target/: Cargo.toml, README.md, 3 .rs, index.html, sw.js.
    assert_eq!(v["files"], 7);
    assert_eq!(v["requirements"], Value::Null);
    let req = p.parent().unwrap().join("requirements.yaml");
    fs::write(&req, "requirements:\n- id: r1\n  text: one\n- id: r2\n  text: two\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_sdc-free-agent")).arg("size").arg(&p).arg("--requirements").arg(&req).output().unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["requirements"], 2);
    assert_eq!(v["readme"], true);
    assert_eq!(v["readme_says_how_to_start"], true);
    fs::write(p.join("README.md"), "# Demo\n\nA demo.\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_sdc-free-agent")).arg("size").arg(&p).output().unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["readme_says_how_to_start"], false);
}

// ---- a run directory of `runbook loom-run` ------------------------------------------------------

/// `tests/fixtures/loom-run/` is written by hand in the shape `runbook loom-run` writes (the member
/// table of docs/loom-run-directory.md), with invented text only: no stub run made it and no real
/// run directory was copied. Seconds from 09:00:00: root `idea.dissect` 0-20 (model, call record
/// missing), child `spec.author` 21-50 (model, call record in `work/.sdc/llm`, 2 requests, Read and
/// Write), `spec.approve` 50-53 by a person, a decider call 53-54 (1 request), root `idea.close`
/// 54-55 by the decider. Cost 0.25 + 0.5 + 0.125.
fn loom_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/loom-run")
}

fn keys(v: &Value) -> Vec<String> {
    v.as_object().unwrap().keys().cloned().collect()
}

#[test]
fn a_loom_run_directory_gives_the_same_document_shape_as_a_driver_run() {
    let (code, doc, err) = timeline(&loom_fixture(), &["--check"]);
    assert_eq!(code, 0, "{err}");
    let run = scratch("shape").join("demo-run");
    sdc_run(&run);
    let (_, driver, _) = timeline(&run, &[]);
    assert_eq!(keys(&doc), keys(&driver));
    assert_eq!(keys(&doc["totals"]), keys(&driver["totals"]));
    assert_eq!(keys(&doc["settings"]), keys(&driver["settings"]));
    assert_eq!(doc["format"], "sdc.run-timeline/1");
    assert_eq!(doc["kind"], "sdc");
    assert_eq!(doc["run"], "loom-run");
    assert_eq!(doc["end"], json!({"state": "closed", "outcome": "operating", "delivered": true}));
    assert_eq!(doc["settings"]["models"]["writer"], "claude-sonnet-5-5");
    assert_eq!(doc["settings"]["autonomy"], "A5");
    assert_eq!(doc["settings"]["decider"]["kind"], "model");
    let tree = doc["tree"].as_array().unwrap();
    assert_eq!(tree.len(), 2);
    assert_eq!(tree[1]["case"], "IDEA-1.spec");
    assert_eq!(tree[1]["parent"], "IDEA-1");
    assert_eq!(tree[1]["stage"], "spec");
    assert!(doc["notes"].as_array().unwrap().iter().any(|n| n.as_str().unwrap().starts_with("a runbook loom-run directory")));
    assert!(driver["notes"].as_array().unwrap().iter().all(|n| !n.as_str().unwrap().starts_with("a runbook loom-run directory")));
}

#[test]
fn a_loom_run_is_read_into_spans_with_null_for_what_it_does_not_record() {
    let (code, doc, err) = timeline(&loom_fixture(), &[]);
    assert_eq!(code, 0, "{err}");
    let models = lane(&doc, "model");
    assert_eq!(models.len(), 2);
    // The call record of `spec.author` is in work/.sdc/llm: requests and tool calls are read from it.
    let author = models.iter().find(|s| s["action"] == "spec.author").unwrap();
    assert!(close(&author["t0_s"], 21.0) && close(&author["t1_s"], 50.0));
    assert_eq!(author["api_requests"], 2);
    assert_eq!(author["tool_calls"], json!({"Read": 1, "Write": 1}));
    assert!(close(&author["cost_usd"], 0.5));
    // `idea.dissect` has a costed line and no record directory: its counts are null, its cost is not.
    let dissect = models.iter().find(|s| s["action"] == "idea.dissect").unwrap();
    assert_eq!(dissect["api_requests"], Value::Null);
    assert_eq!(dissect["tool_calls"], Value::Null);
    assert!(close(&dissect["cost_usd"], 0.25));
    // No evaluation record in Loom's run log: no take has a cause.
    for s in lane(&doc, "program").into_iter().chain(models) {
        assert_eq!(s["cause"], Value::Null, "{s}");
    }
    let programs = lane(&doc, "program");
    assert_eq!(programs.len(), 2);
    assert_eq!(programs[0]["principal"], "person:reviewer-1");
    let deciders = lane(&doc, "decider");
    assert_eq!(deciders.len(), 1);
    assert!(close(&deciders[0]["t0_s"], 53.0) && close(&deciders[0]["t1_s"], 54.0));
    assert_eq!(deciders[0]["api_requests"], 1);
    assert_eq!(deciders[0]["tool_calls"], json!({"StructuredOutput": 1}));
    let t = &doc["totals"];
    assert!(close(&t["cost_usd"], 0.875));
    assert!(close(&t["wall_s"], 55.0));
    assert_eq!(t["model_sessions"], 2);
    assert_eq!(t["api_requests"], 3);
    assert_eq!(t["api_requests_unrecorded_spans"], 1);
    assert_eq!(t["waiting_on_person_s"], 0.0);
}

#[test]
fn a_directory_of_neither_kind_is_refused_with_exit_2_naming_both_files() {
    let d = scratch("neither");
    let out = Command::new(env!("CARGO_BIN_EXE_sdc-run-timeline")).arg(&d).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("settings.json") && err.contains("events.jsonl"), "{err}");
}
