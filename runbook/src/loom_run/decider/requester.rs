//! The requester decider (`decider.requester: true` of a model decider, principal
//! `decider:requester`): a decision of a requester-side class (`driver/examples/sdc/
//! decision-classes.json`) is answered from the requester's goals document, `work/goals.yaml`
//! (`--goals`), by one model call of its own. What the driver did with `decider.sh requester`
//! (`scripts/run-trial.sh`, `decide`, lines 799 to 822) is done here, file for file: the same
//! call directory, the same inputs under `in/`, the same reply.
//!
//! The text the model reads is in `instructions/decider/requester*.md`; this file holds none. The
//! grounding check stays the program it is (`tools/decision-grounds.sh`, run on the proposal), and
//! so does the request of a scope question (`sdc-decision-request`, beside `sdc-reply`).
//!
//! The routing (`route`, `tag`) and the call (`run`) are used by `decider.rs` and `ask.rs`: the
//! retries, the caps, the `escalate` handling and the lines of `decisions.jsonl` are the model
//! decider's, applied to the requester's reply.

use super::call::{adapter_path, call_status, now, start, ModelCall, Replied};
use super::inputs::write_pretty;
use super::prompt::{or, raw, Renderer};
use super::rules::rule;
use super::run_program;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

/// The principal of every answer the requester gives.
pub(super) const PRINCIPAL: &str = "decider:requester";
/// The person a request that the requester cannot answer is addressed to (`SDC_REQUESTER_PERSON`).
const REQUESTER_PERSON: &str = "person:requester";
const CLASSES_FILE: &str = "driver/examples/sdc/decision-classes.json";
const DRY_WHY: &str = "dry run: no LLM was called; the approve rule decided";

/// The decision class of a decision (`decision-classes.json`, `sdc.decision-classes/1`) and the side
/// it belongs to.
pub(super) struct Route {
    pub(super) class: String,
    pub(super) side: String,
}

/// The class of a request of `kind` (`approval`, `question`, `finding` or `close`) on `action`: a
/// question is a `scope_changing_answer`, a finding a `finding`, a close a `close`, an approval the
/// first class that lists the action (`<protocol>/<action>`), else `approval`. A class that is not
/// in the file is on the delivering side.
pub(super) fn route(root: &Path, kind: &str, action: &str) -> Result<Route, String> {
    let file = root.join(CLASSES_FILE);
    let text = std::fs::read_to_string(&file).map_err(|e| format!("the requester decider needs {CLASSES_FILE} above the bindings copy: {}", e.kind()))?;
    let doc: Value = serde_json::from_str(&text).map_err(|_| format!("{CLASSES_FILE} is not JSON"))?;
    let classes = doc["classes"].as_object().ok_or_else(|| format!("{CLASSES_FILE} has no classes"))?;
    let class = match kind {
        "question" => "scope_changing_answer".to_owned(),
        "finding" => "finding".to_owned(),
        "close" => "close".to_owned(),
        _ => {
            let tail = format!("/{action}");
            classes
                .iter()
                .find(|(_, v)| v["actions"].as_array().is_some_and(|a| a.iter().filter_map(Value::as_str).any(|x| x.ends_with(&tail))))
                .map_or_else(|| "approval".to_owned(), |(k, _)| k.clone())
        }
    };
    let side = classes.get(&class).and_then(|c| c["side"].as_str()).unwrap_or("delivering").to_owned();
    Ok(Route { class, side })
}

/// Adds to a request body what the requester reads of the run: the class and side, the level, and
/// the decisions log of the run (its own earlier decisions are read from there).
pub(super) fn tag(body: &mut Value, class: &str, run_dir: &Path) {
    body["class"] = json!(class);
    body["side"] = json!("requester");
    body["autonomy"] = json!(super::AUTONOMY_DECIDES);
    body["decisions_log"] = json!(run_dir.join("decisions.jsonl").display().to_string());
}

impl super::NativeDecider {
    /// The class of a decision when the requester answers it: `Some` only with `decider.requester`
    /// and a class on the requester side of `decision-classes.json`, read above the bindings copy
    /// like the instruction files.
    pub(super) fn requester_class(&self, run_dir: &Path, kind: &str, action: &str) -> Result<Option<String>, String> {
        if !self.requester {
            return Ok(None);
        }
        let root = super::instructions_root(&run_dir.join("bindings").join("tools")).ok_or(super::NO_INSTRUCTIONS)?;
        let found = route(&root, kind, action)?;
        Ok((found.side == "requester").then_some(found.class))
    }

    /// The principal of an answer: the requester's when it answered, else the decider's.
    pub(super) fn principal_for(&self, routed: bool) -> &str {
        if routed {
            PRINCIPAL
        } else {
            &self.principal
        }
    }
}

enum Failed {
    /// The call could not be made.
    Program(String),
    /// A program was ended at the time limit.
    Ended(String),
}

/// The call: prepares what the requester reads, asks the adapter once and checks the reply.
pub(super) fn run(c: &ModelCall<'_>) -> Result<Replied, String> {
    let adapter = c.adapter.unwrap_or("metaharness");
    if adapter == "stub" {
        let reply = json!({"decision": "escalate", "answer": null, "kind": "requester", "valid": false, "invalid_reason": "the stub adapter has no LLM",
            "reasons": ["no LLM: SDC_LLM_ADAPTER=stub"], "cites": []});
        return Ok(Replied { reply, stderr: String::new() });
    }
    let cwd = c.rec.join("cwd");
    for dir in ["in", "out"] {
        std::fs::create_dir_all(cwd.join(dir)).map_err(|e| format!("decider call directory: {}", e.kind()))?;
    }
    let started = now();
    call_status(c.rec, "running", &started, None);
    match ask(c, adapter, &cwd) {
        Ok((replied, exit)) => {
            call_status(c.rec, if exit == 0 { "finished" } else { "failed" }, &started, Some(exit));
            Ok(replied)
        }
        Err(Failed::Program(reason)) => {
            call_status(c.rec, "failed", &started, Some(126));
            Err(reason)
        }
        Err(Failed::Ended(reason)) => Err(reason),
    }
}

/// The present time as `YYYY-MM-DDTHH:MM:SSZ`.
fn instant() -> String {
    format!("{}Z", &now()[..19])
}

/// The `sdc-decision-request` of a scope question: the `request` member of what it prints, None when
/// it refuses (the call then ends as an invalid reply).
fn question_request(c: &ModelCall<'_>, case_dir: &Path, level: &str) -> Result<(Option<Value>, String), Failed> {
    let qid = raw(&c.req["question"]["id"]);
    let bin = c.env.iter().find(|(k, _)| k == "SDC_REPLY").and_then(|(_, v)| Path::new(v).parent().map(|d| d.join("sdc-decision-request")));
    let Some(bin) = bin else {
        return Err(Failed::Program("the requester decider needs sdc-decision-request beside sdc-reply (decider.reply_bin)".to_owned()));
    };
    let eligible = format!("{PRINCIPAL},{REQUESTER_PERSON}");
    let case = case_dir.display().to_string();
    let stamp = instant();
    let args = [case.as_str(), qid.as_str(), "--level", level, "--holder", "decider", "--eligible", eligible.as_str(), "--opened-at", stamp.as_str()];
    let ran = run_program(&bin, &args, c.env, c.rec, c.timeout).map_err(Failed::Ended)?;
    if ran.code != 0 {
        return Ok((None, ran.stderr));
    }
    let request = serde_json::from_str::<Value>(ran.stdout.trim()).ok().and_then(|v| v.get("request").cloned()).filter(|r| !r.is_null());
    Ok((request, ran.stderr))
}

/// The request of an approval or a close, with the question and the two options rendered from the
/// instruction files.
fn approval_request(r: &Renderer, req: &Value, case_dir: &Path, level: &str) -> Result<Value, String> {
    let close = req["type"] == "close";
    let (question, a, b) = if close {
        let outcome = raw(&req["outcome"]);
        (
            r.render("requester.close.question.md", &[("outcome", &outcome)])?,
            r.render("requester.close.option-a.md", &[("outcome", &outcome)])?,
            r.render("requester.close.option-b.md", &[])?,
        )
    } else {
        (
            r.render("requester.approval.question.md", &[("action", &raw(&req["action"])), ("subject", &raw(&req["subject"])), ("revision", &raw(&req["revision"]))])?,
            r.render("requester.approval.option-a.md", &[])?,
            r.render("requester.approval.option-b.md", &[])?,
        )
    };
    let path = raw(or(&req["path"], &json!("")));
    let cid = raw(&sdc_tools::reply::read_json(&case_dir.join("case.json"))["id"]);
    let revision = raw(or(&req["revision"], &json!("none")));
    let action = raw(&req["action"]);
    Ok(json!({
        "request_id": format!("DR-{cid}-{action}-{revision}"),
        "state": "Open",
        "case_id": cid,
        "question_key": format!("{cid}|{action}"),
        "unblocks": {"kind": req["type"], "action": req["action"]},
        "subject_artifact": or(&req["subject"], &json!("case")),
        "subject_revision": req["revision"],
        "subject_path": if path.is_empty() { Value::Null } else { json!(format!("work/{path}")) },
        "question": question,
        "options": [{"letter": "A", "does": a}, {"letter": "B", "does": b}],
        "autonomy_level": level,
        "decision_class": req["class"],
        "holder": "decider",
        "eligible": [PRINCIPAL],
        "addressed_to": PRINCIPAL,
        "grounds": [],
        "decision_reasons": [],
    }))
}

/// Copies `work/<p>` to `in/<p>` and lists it, unless it is not a file, is there already, or `p` leaves `work/`.
fn input(work: &Path, cwd: &Path, p: &str, listed: &mut Vec<String>) {
    let plain = !p.is_empty() && Path::new(p).components().all(|c| matches!(c, std::path::Component::Normal(_)));
    if !plain {
        return;
    }
    let (from, to) = (work.join(p), cwd.join("in").join(p));
    if !from.is_file() || to.exists() {
        return;
    }
    if let Some(dir) = to.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::copy(&from, &to);
    listed.push(format!("in/{p}"));
}

/// `in/decisions.json`: the earlier decisions of the requester in this run (`valid` not false), read
/// from the decisions log; `[]` when the log cannot be read.
fn earlier_decisions(log: &Path) -> Value {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(entry) = serde_json::from_str::<Value>(line) else { return json!([]) };
        if !entry.is_object() && !entry.is_null() {
            return json!([]);
        }
        if entry["principal"] == PRINCIPAL && entry["valid"] != false {
            let mut one = Map::new();
            for key in ["case", "action", "question", "revision", "decision", "answer", "reasons", "grounds"] {
                one.insert(key.to_owned(), entry[key].clone());
            }
            out.push(Value::Object(one));
        }
    }
    Value::Array(out)
}

fn ask(c: &ModelCall<'_>, adapter: &str, cwd: &Path) -> Result<(Replied, i64), Failed> {
    let program = Failed::Program;
    let req = c.req;
    let case_dir = PathBuf::from(raw(&req["case_dir"]));
    let work = case_dir.join("work");
    let level = raw(or(&req["autonomy"], &json!("A3")));
    let renderer = Renderer { root: c.root.to_path_buf() };
    let mut tool_stderr = String::new();
    let request = if req["type"] == "question" {
        let (found, stderr) = question_request(c, &case_dir, &level)?;
        tool_stderr = stderr;
        match found {
            Some(request) => request,
            None => {
                let reply = json!({"decision": "escalate", "answer": null, "best": null,
                    "reasons": [format!("no decision request for {} (sdc-decision-request refused it)", raw(&req["question"]["id"]))],
                    "cites": [], "kind": "requester", "valid": false, "invalid_reason": "no decision request"});
                return Ok((Replied { reply, stderr: tool_stderr }, 0));
            }
        }
    } else {
        approval_request(&renderer, req, &case_dir, &level).map_err(program)?
    };
    let write = |path: &Path, text: String| std::fs::write(path, text).map_err(|e| Failed::Program(format!("{}: {}", path.display(), e.kind())));
    write(&cwd.join("in").join("request.json"), format!("{}\n", serde_json::to_string(&request).unwrap_or_default()))?;
    let mut listed = vec![renderer.render("requester.input-request.md", &[]).map_err(Failed::Program)?];
    let subject_path = request["subject_path"].as_str().unwrap_or("");
    input(&work, cwd, subject_path.strip_prefix("work/").unwrap_or(subject_path), &mut listed);
    for ground in request["grounds"].as_array().into_iter().flatten() {
        if ground["citation"]["form"] == "path_line" {
            if let Some(path) = ground["citation"]["value"]["path"].as_str() {
                input(&work, cwd, path.strip_prefix("work/").unwrap_or(path), &mut listed);
            }
        }
    }
    for name in ["idea.yaml", "dissection.yaml", "goals.yaml"] {
        input(&work, cwd, name, &mut listed);
    }
    let log = PathBuf::from(raw(or(&req["decisions_log"], &json!("/dev/null"))));
    let decisions = earlier_decisions(&log);
    write_pretty(&cwd.join("in").join("decisions.json"), &decisions).map_err(Failed::Program)?;
    listed.push(renderer.render("requester.input-decisions.md", &[]).map_err(Failed::Program)?);

    let attempt = or(&req["retry"]["attempt"], &json!(1)).clone();
    let retry = if attempt.as_i64().unwrap_or(1) > 1 {
        let text = renderer.render("requester-retry.md", &[("attempt", &raw(&attempt)), ("reason", &raw(&req["retry"]["invalid_reason"]))]).map_err(Failed::Program)?;
        format!("\n{text}\n")
    } else {
        String::new()
    };
    let case_json = sdc_tools::reply::read_json(&case_dir.join("case.json"));
    let goals_revision = raw(or(&case_json["artifacts"]["goals"]["revision"], &json!("none")));
    let inputs = listed.iter().map(|i| format!("- {i}\n")).collect::<String>();
    let text = renderer
        .render("requester.md", &[("inputs", inputs.trim_end_matches('\n')), ("goals_revision", &goals_revision), ("retry", &retry)])
        .map_err(Failed::Program)?;
    let prompt_file = c.rec.join("prompt.md");
    write(&prompt_file, format!("{text}\n"))?;

    let effort = c.effort.map(str::to_owned).or_else(|| requester_effort(c.tools));
    let usd_literal = super::approval::usd_text(c.budget_micro_usd);
    let usd = usd_literal.parse::<f64>().unwrap_or(0.0);
    let adapter_request = json!({
        "model": c.model, "effort": effort, "prompt_file": prompt_file.display().to_string(), "cwd": cwd.display().to_string(),
        "record_dir": c.rec.display().to_string(), "node": format!("requester.{}", raw(or(&req["question"]["id"], &req["action"]))),
        "attempt": attempt, "case": or(&case_json["id"], &json!("")), "case_dir": case_dir.display().to_string(),
        "operations": ["dir.list", "file.read", "file.write"], "max_turns": c.max_turns.unwrap_or(8), "max_budget_usd": usd,
    });
    let request_file = c.rec.join("request.json");
    let text = format!("{}\n", serde_json::to_string_pretty(&adapter_request).unwrap_or_default()).replacen(&format!("\"max_budget_usd\": {}", Value::from(usd)), &format!("\"max_budget_usd\": {usd_literal}"), 1);
    write(&request_file, text)?;

    let ran = start(&adapter_path(c.tools, adapter), &request_file, c).map_err(Failed::Ended)?;
    let exit = i64::from(ran.code);
    let response_file = c.rec.join("response.json");
    write(&response_file, ran.stdout.clone())?;
    let response = match serde_json::from_str::<Value>(&ran.stdout) {
        Ok(v) if v.is_object() => v,
        _ => {
            write(&response_file, "{}\n".to_owned())?;
            json!({})
        }
    };
    let mut meta = Map::new();
    meta.insert("kind".to_owned(), json!("requester"));
    for key in ["model", "effort", "adapter", "usage", "cost_usd", "duration_ms"] {
        meta.insert(key.to_owned(), response[key].clone());
    }
    meta.insert("exit".to_owned(), json!(exit));
    meta.insert("record".to_owned(), json!(c.rec.display().to_string()));
    let merged = |mut base: Map<String, Value>| {
        for (k, v) in &meta {
            base.insert(k.clone(), v.clone());
        }
        Value::Object(base)
    };
    let done = |reply: Value| Ok((Replied { reply, stderr: format!("{tool_stderr}{}", ran.stderr) }, exit));
    if adapter == "dry" {
        let mut reply = merged(rule("requester", "approve", DRY_WHY, true, req).as_object().cloned().unwrap_or_default());
        reply["dry"] = json!(true);
        return done(reply);
    }
    let invalid = |why: &str| {
        let mut base = Map::new();
        base.insert("decision".to_owned(), json!("escalate"));
        base.insert("answer".to_owned(), Value::Null);
        base.insert("best".to_owned(), Value::Null);
        base.insert("reasons".to_owned(), json!([format!("invalid requester reply: {why}")]));
        base.insert("cites".to_owned(), json!([]));
        base.insert("valid".to_owned(), json!(false));
        base.insert("invalid_reason".to_owned(), json!(why));
        merged(base)
    };
    let reply_file = cwd.join("out").join("reply.json");
    let proposed = std::fs::read_to_string(&reply_file).ok().filter(|t| !t.is_empty()).and_then(|t| serde_json::from_str::<Value>(&t).ok()).filter(Value::is_object);
    let Some(proposed) = proposed else {
        return done(invalid("out/reply.json is missing or not a JSON object"));
    };
    if proposed["no_ground_in_goals"] == true {
        let mut reasons = vec![json!("no ground in the goals")];
        match or(&proposed["reasons"], &json!([])) {
            Value::Array(own) => reasons.extend(own.iter().cloned()),
            _ => return Err(Failed::Program("out/reply.json: reasons is not a list".to_owned())),
        }
        let mut base = Map::new();
        base.insert("decision".to_owned(), json!("escalate"));
        base.insert("answer".to_owned(), Value::Null);
        base.insert("best".to_owned(), if req["type"] == "question" { Value::Null } else { json!("approve") });
        base.insert("reasons".to_owned(), Value::Array(reasons));
        base.insert("cites".to_owned(), json!([]));
        base.insert("grounds".to_owned(), json!([]));
        base.insert("strength".to_owned(), or(&proposed["strength"], &json!("weak")).clone());
        base.insert("no_ground_in_goals".to_owned(), json!(true));
        base.insert("valid".to_owned(), json!(true));
        return done(merged(base));
    }
    let mut recommend = Map::new();
    recommend.insert("strength".to_owned(), proposed["strength"].clone());
    recommend.insert("default_on_silence".to_owned(), json!(false));
    recommend.insert("grounds".to_owned(), or(&proposed["grounds"], &json!([])).clone());
    recommend.insert("proposed_by".to_owned(), json!("llm"));
    if proposed["option"].is_null() {
        recommend.insert("recommended_answer".to_owned(), proposed["answer"].clone());
    } else {
        recommend.insert("recommended_option".to_owned(), proposed["option"].clone());
    }
    let proposal_file = c.rec.join("proposal.json");
    write_pretty(&proposal_file, &json!({"request": request, "proposal": recommend})).map_err(Failed::Program)?;
    let mut at = Vec::new();
    if request["opened_at"].as_str().unwrap_or("").is_empty() {
        at = vec!["--at".to_owned(), instant()];
    }
    let grounds_tool = c.root.join("tools").join("decision-grounds.sh");
    let mut args = vec![proposal_file.display().to_string(), "--case".to_owned(), case_dir.display().to_string()];
    args.extend(at);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let ran_check = run_program(&grounds_tool, &refs, c.env, c.rec, c.timeout).map_err(Failed::Ended)?;
    let checked = ran_check.stdout.trim_end_matches('\n').to_owned();
    write(&c.rec.join("grounds-check.json"), format!("{checked}\n"))?;
    write(&c.rec.join("grounds-check.stderr"), ran_check.stderr.clone())?;
    let check = serde_json::from_str::<Value>(&checked).ok().filter(Value::is_object);
    let Some(check) = check.filter(|_| ran_check.code == 0 || ran_check.code == 1) else {
        let tail: Vec<&str> = ran_check.stderr.lines().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect();
        return Err(Failed::Program(format!("decision-grounds.sh exit {}, its output could not be read: {}", ran_check.code, tail.join("\n"))));
    };
    if or(&check["recommend"]["check_verdict"], &json!("refused")) != "verified" {
        let report = check["recommend"]["check_report"].as_str().unwrap_or("");
        return done(invalid(&format!("the grounding check refused it: {report}")));
    }
    let letter = proposed["option"].as_str().unwrap_or("");
    let question = req["type"] == "question";
    let (decision, answer) = if question {
        let answer = if letter.is_empty() {
            raw(&proposed["answer"])
        } else {
            request["options"].as_array().into_iter().flatten().find(|o| o["letter"] == letter).map(|o| raw(or(&o["consequence"]["answer"], &o["does"]))).unwrap_or_default()
        };
        ("approve", answer)
    } else {
        (if letter == "B" { "reject" } else { "approve" }, String::new())
    };
    let grounds = check["recommend"]["grounds"].clone();
    let citations: Vec<Value> = grounds
        .as_array()
        .into_iter()
        .flatten()
        .map(|g| {
            let value = &g["citation"]["value"];
            let mut cite = raw(or(&value["artifact"], &value["path"]));
            if !value["entry"].is_null() && value["entry"] != false {
                cite.push_str(&format!("#{}", raw(&value["entry"])));
            }
            json!({"cite": cite, "form": g["citation"]["form"], "value": value})
        })
        .collect();
    let mut base = Map::new();
    base.insert("decision".to_owned(), json!(decision));
    base.insert("answer".to_owned(), if question { json!(answer) } else { Value::Null });
    base.insert("best".to_owned(), Value::Null);
    base.insert("reasons".to_owned(), proposed["reasons"].clone());
    base.insert("cites".to_owned(), json!([]));
    base.insert("grounds".to_owned(), grounds);
    base.insert("citations".to_owned(), Value::Array(citations));
    base.insert("strength".to_owned(), proposed["strength"].clone());
    base.insert("no_ground_in_goals".to_owned(), json!(false));
    base.insert("option".to_owned(), proposed["option"].clone());
    base.insert("grounds_check".to_owned(), check["recommend"]["check_report"].clone());
    base.insert("valid".to_owned(), json!(true));
    done(merged(base))
}

/// The effort when the settings name none: the default of the requester class in the copy's `node-classes.json`.
fn requester_effort(tools: &Path) -> Option<String> {
    let classes = sdc_tools::reply::read_json(&tools.join("node-classes.json"));
    classes["default_efforts"]["requester"].as_str().filter(|e| !e.is_empty()).map(str::to_owned)
}
