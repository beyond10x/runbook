//! One implement call: the requests, the retries and the continuations.
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

use super::assemble::{assemble, Inputs};
use super::reply::{check_reply, schema};
use super::tree::write_files;
use super::{MAX_CONTINUATIONS, MAX_RETRIES};
use crate::instructions;
use crate::reply::{pretty, read_json, sorted, text};

/// The arguments of `sdc-reply implement-run`.
pub struct RunArgs {
    /// The call record directory.
    pub rec: PathBuf,
    /// The call directory (`in/`, `out/`).
    pub cwd: PathBuf,
    /// The case directory.
    pub case_dir: PathBuf,
    /// The story's id.
    pub story: String,
    /// The adapter program; none stops after assembling.
    pub adapter: Option<PathBuf>,
    /// The rework limit shown to the model.
    pub rework_limit: Option<u64>,
    /// Write the prompt and the manifest and stop.
    pub assemble_only: bool,
    /// The cap on inlined bytes.
    pub cap: usize,
}

/// Numbers summed key by key (a usage object, nested); anything else keeps the last value.
fn add_numbers(acc: &mut Value, v: &Value) {
    match (acc.as_object_mut(), v) {
        (Some(a), Value::Object(m)) => {
            for (k, x) in m {
                let cur = a.entry(k.clone()).or_insert(Value::Null);
                match (cur.as_i64(), x.as_i64(), cur.as_f64(), x.as_f64()) {
                    (Some(p), Some(q), _, _) => *cur = json!(p + q),
                    (_, _, Some(p), Some(q)) => *cur = json!(p + q),
                    _ if x.is_object() && (cur.is_object() || cur.is_null()) => {
                        if cur.is_null() {
                            *cur = json!({});
                        }
                        add_numbers(cur, x);
                    }
                    _ => *cur = x.clone(),
                }
            }
        }
        _ => *acc = v.clone(),
    }
}

/// The call's response.json: the requests' responses summed (cost, duration, turns, usage), the
/// last one's adapter, model and reasons, and the request counts.
fn aggregate(responses: &[Value], exit: u8, requests: usize, continuations: usize, retries: usize, cause: Option<&str>) -> Value {
    let last = responses.last().cloned().unwrap_or(json!({}));
    let sum = |k: &str| {
        let xs: Vec<f64> = responses.iter().filter_map(|r| r[k].as_f64()).collect();
        if xs.is_empty() { Value::Null } else { json!(xs.iter().sum::<f64>()) }
    };
    let mut usage = Value::Null;
    for r in responses.iter().filter(|r| r["usage"].is_object()) {
        if usage.is_null() {
            usage = json!({});
        }
        add_numbers(&mut usage, &r["usage"]);
    }
    let tool_calls: Vec<Value> = responses.iter().flat_map(|r| r["tool_calls"].as_array().cloned().unwrap_or_default()).collect();
    sorted(json!({"adapter": last["adapter"], "model": last["model"], "effort": last["effort"], "exit": exit,
        "duration_ms": sum("duration_ms"), "wall_ms": sum("wall_ms"), "cost_usd": sum("cost_usd"), "num_turns": sum("num_turns"),
        "usage": usage, "tool_calls": tool_calls, "offered_tools": last["offered_tools"], "observed_models": last["observed_models"],
        "is_error": last["is_error"], "subtype": last["subtype"], "terminal_reason": last["terminal_reason"],
        "api_error_status": last["api_error_status"], "schema_constrained": last["schema_constrained"],
        "implement_mode": "request", "requests": requests, "continuations": continuations, "reply_retries": retries, "cause": cause}))
}

/// One implement call's requests. Returns (exit, the line for the step's log): 0 the files are
/// written into `<cwd>/out/impl/`; 2 the reply stayed invalid after `MAX_RETRIES` retries
/// (`implement_reply_invalid`) or files stayed `remaining` after `MAX_CONTINUATIONS`
/// continuations (`implement_reply_incomplete`), or the input could not be assembled
/// (`implement_inputs_missing`); 75 the model's API ended a request or a re-driven recording
/// diverged; 126/127 the adapter could not run; 64 a usage error.
pub fn run(a: &RunArgs) -> (u8, String) {
    let tree = a.cwd.join("out/impl");
    let feedback = fs::read_to_string(a.cwd.join("in/feedback.md")).ok().filter(|s| !s.trim().is_empty());
    let rejection: Option<Value> = fs::read(a.cwd.join("in/rejection.json")).ok().and_then(|b| serde_json::from_slice(&b).ok());
    let inputs = |continuation| Inputs { case_dir: &a.case_dir, tree: &tree, story: &a.story, feedback: feedback.clone(),
        rejection: rejection.clone(), rework_limit: a.rework_limit, cap: a.cap, continuation };
    let first = match assemble(&inputs(None)) {
        Ok(x) => x,
        Err(e) => return (2, format!("implement_inputs_missing: {e}")),
    };
    let w = |name: &str, text: &str| fs::write(a.rec.join(name), text).map_err(|e| format!("{name}: {e}"));
    if let Err(e) = fs::create_dir_all(&a.rec).map_err(|e| e.to_string())
        .and_then(|_| w("prompt.md", &first.prompt))
        .and_then(|_| w("reply-schema.json", &schema().to_string()))
        .and_then(|_| w("implement-inputs.json", &pretty(&first.manifest))) {
        return (127, format!("the call record could not be written: {e}"));
    }
    let summary = format!("{} bytes of prompt: {} file(s) of impl/ inlined ({} bytes), {} listed", first.prompt.len(),
        first.manifest["tree"]["inlined"].as_array().map_or(0, |v| v.len()), first.manifest["tree"]["inlined_bytes"],
        first.manifest["tree"]["listed"].as_array().map_or(0, |v| v.len()));
    if a.assemble_only {
        return (0, format!("assembled: {summary}"));
    }
    let Some(adapter) = &a.adapter else { return (64, "implement-run: no --adapter".into()) };
    let base = read_json(&a.rec.join("request.json"));
    if !base.is_object() {
        return (127, format!("no request.json in {}", a.rec.display()));
    }
    let stderr = || fs::OpenOptions::new().create(true).append(true).open(a.rec.join("adapter-stderr.txt"));
    let (mut slot, mut prompt) = (first.prompt.clone(), first.prompt);
    let mut asked: Option<Vec<String>> = None;
    let (mut written, mut findings, mut notes): (Vec<String>, Vec<Value>, Vec<String>) = (vec![], vec![], vec![]);
    let (mut retries, mut conts, mut k, mut kind) = (0usize, 0usize, 0usize, "first");
    let mut responses = Vec::new();
    let mut log = String::new();
    let (code, line, cause): (u8, String, Option<&str>) = loop {
        k += 1;
        let dir = a.rec.join(format!("req-{k}"));
        let mut req = base.clone();
        req["record_dir"] = json!(dir);
        req["prompt_file"] = json!(dir.join("prompt.md"));
        req["json_schema"] = json!(a.rec.join("reply-schema.json"));
        req["reply_attempt"] = json!(k);
        req["request_kind"] = json!(kind);
        if let Err(e) = fs::create_dir_all(&dir).and_then(|_| fs::write(dir.join("prompt.md"), &prompt))
            .and_then(|_| fs::write(dir.join("request.json"), pretty(&req))) {
            break (127, format!("request {k} could not be written: {e}"), None);
        }
        let err = match stderr() { Ok(f) => Stdio::from(f), Err(_) => Stdio::null() };
        let out = match Command::new(adapter).arg(dir.join("request.json")).stdin(Stdio::null()).stderr(err).output() {
            Ok(o) => o,
            Err(e) => break (127, format!("the adapter {} did not start: {e}", adapter.display()), None),
        };
        let exit = out.status.code().unwrap_or(1);
        let resp: Value = serde_json::from_slice(&out.stdout).unwrap_or(json!({}));
        let _ = fs::write(dir.join("response.json"), text(&resp));
        responses.push(resp.clone());
        let term = resp["terminal_reason"].as_str().unwrap_or("");
        if term == "api_error" || term == "recording_diverged" {
            break (75, format!("request {k}: {term}{}", resp["api_error_status"].as_i64().map_or(String::new(), |s| format!(" {s}"))), None);
        }
        if exit == 126 || exit == 127 {
            break (exit as u8, format!("request {k}: the adapter could not run (exit {exit})"), None);
        }
        let said = |file: &str, values: &[(&str, &str)]| instructions::render(file, values).map_err(|e| (2, format!("implement_inputs_missing: {e}"), Some("implement_inputs_missing")));
        let checked = if exit != 0 {
            let (code, subtype) = (exit.to_string(), resp["subtype"].as_str().map_or(String::new(), |s| format!(", {s}")));
            match said("instructions/change.implement.request.request-failed.md",&[("exit", &code), ("subtype", &subtype)]) {
                Ok(why) => Err(vec![why]),
                Err(stop) => break stop,
            }
        } else {
            match fs::read(dir.join("structured-output.json")).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()) {
                None => match said("instructions/change.implement.request.no-reply.md",&[]) {
                    Ok(why) => Err(vec![why]),
                    Err(stop) => break stop,
                },
                Some(v) => check_reply(&v, asked.as_deref()).and_then(|r| write_files(&tree, &r.files).map(|w| (r, w)).map_err(|e| vec![e])),
            }
        };
        match checked {
            Err(why) => {
                let why = why.join("; ");
                log.push_str(&format!("{}\n", text(&json!({"request": k, "kind": kind, "valid": false, "error": why}))));
                if retries >= MAX_RETRIES {
                    break (2, format!("implement_reply_invalid after {k} request(s), {retries} retries: {why}"), Some("implement_reply_invalid"));
                }
                retries += 1;
                kind = "retry";
                match said("instructions/change.implement.request.retry.md",&[("why", &why)]) {
                    Ok(t) => prompt = format!("{slot}\n{t}\n"),
                    Err(stop) => break stop,
                }
            }
            Ok((r, w)) => {
                log.push_str(&format!("{}\n", text(&json!({"request": k, "kind": kind, "valid": true, "files": w, "remaining": r.remaining}))));
                written.extend(w);
                findings.extend(r.findings);
                notes.push(r.note);
                if r.remaining.is_empty() {
                    break (0, format!("change.implement wrote {} file(s) in {k} request(s) ({retries} retries, {conts} continuations; {summary}): {}",
                        written.len(), notes.join(" / ").replace('\n', " ")), None);
                }
                if conts >= MAX_CONTINUATIONS {
                    break (2, format!("implement_reply_incomplete after {k} request(s): {} still remaining after {MAX_CONTINUATIONS} continuations",
                        r.remaining.join(", ")), Some("implement_reply_incomplete"));
                }
                conts += 1;
                kind = "continuation";
                asked = Some(r.remaining.clone());
                match assemble(&inputs(Some((written.clone(), r.remaining)))) {
                    Ok(x) => {
                        slot = x.prompt;
                        let _ = fs::write(a.rec.join(format!("implement-inputs.continuation-{conts}.json")),
                            pretty(&x.manifest));
                    }
                    Err(e) => break (2, format!("implement_inputs_missing: {e}"), Some("implement_inputs_missing")),
                }
                prompt = slot.clone();
            }
        }
    };
    if code == 0 && !findings.is_empty() {
        let y = serde_yaml::to_string(&sorted(json!({"findings": findings}))).unwrap_or_default();
        if let Err(e) = fs::write(a.cwd.join("out/findings.yaml"), y) {
            return (127, format!("out/findings.yaml: {e}"));
        }
    }
    let _ = fs::write(a.rec.join("requests.jsonl"), &log);
    let mut events = Vec::new();
    for n in 1..=k {
        if let Ok(b) = fs::read(a.rec.join(format!("req-{n}/events.jsonl"))) {
            events.extend(b);
            if !events.ends_with(b"\n") && !events.is_empty() {
                events.push(b'\n');
            }
        }
    }
    if !events.is_empty() {
        let _ = fs::File::create(a.rec.join("events.jsonl")).and_then(|mut f| f.write_all(&events));
    }
    let agg = aggregate(&responses, code, k, conts, retries, cause);
    let _ = fs::write(a.rec.join("response.json"), text(&agg));
    (code, line)
}
