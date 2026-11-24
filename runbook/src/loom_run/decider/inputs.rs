//! What the model is shown: the files the decision turns on, copied into the call's `in/`, the
//! claims it reads (`in/checks.json`) and the decision itself (`in/decision.json`).

use super::prompt::{or, raw, Renderer};
use crate::loom_run::record_write::{read_regular, read_regular_text, Held};
use sdc_tools::reply::rules::later as protocol_later;
use sdc_tools::reply::sorted;
use serde_json::{json, Map, Value};
use std::path::Path;

/// The inputs listed in the prompt, in the order they were put into `in/`.
#[derive(Default)]
pub(super) struct Inputs {
    pub(super) listed: Vec<String>,
}

/// A name `rsync -a --exclude target --exclude .git --include Cargo.lock --exclude '*.lock'` leaves out.
fn left_out(name: &str) -> bool {
    name == "target" || name == ".git" || (name.ends_with(".lock") && name != "Cargo.lock")
}

/// A JSON file as a value with sorted keys, read as every name of the run is (`record_write::read_regular`); null when it
/// is missing, refused (a link, a pipe, a file with a second name or over the bound) or not JSON.
pub(super) fn read_json(p: &Path) -> Value {
    sorted(read_regular(p).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null))
}

/// Copies `from` to the name `name` of the held directory `parent`: every directory of the copy is made below the descriptor
/// without following a link (`Held::sub`), a file is copied through `Held::copy_from` and a link as a link, all relative to
/// the descriptor of the directory they go into.
fn copy_entry(parent: &Held, from: &Path, name: &std::ffi::OsStr) -> Result<(), String> {
    let kind = |e: std::io::Error| e.kind().to_string();
    let meta = std::fs::symlink_metadata(from).map_err(kind)?;
    if meta.file_type().is_symlink() {
        return parent.symlink(&std::fs::read_link(from).map_err(kind)?, name);
    }
    if meta.is_dir() {
        let made = parent.sub(Path::new(name))?;
        let mut names: Vec<std::ffi::OsString> = std::fs::read_dir(from).map_err(kind)?.filter_map(Result::ok).map(|e| e.file_name()).collect();
        names.sort();
        for child in names {
            if !left_out(&child.to_string_lossy()) {
                copy_entry(&made, &from.join(&child), &child)?;
            }
        }
        return made.file().set_permissions(meta.permissions()).map_err(kind);
    }
    if meta.is_file() {
        parent.copy_from(from, name)?;
    }
    Ok(())
}

impl Inputs {
    /// Copies `work/<p>` to `in/<p>` and lists it; nothing when it does not exist. The directories of the copy are made
    /// below the held directory `cwd` without following a link, the file goes in through the descriptor of its directory,
    /// and a file of `work/` is read as a regular file with one name.
    pub(super) fn add(&mut self, work: &Path, cwd: &Held, p: &str) -> Result<(), String> {
        let from = work.join(p);
        if !from.exists() {
            return Ok(());
        }
        let to = Path::new("in").join(p);
        let name = to.file_name().map(|n| n.to_os_string()).unwrap_or_default();
        let dir = cwd.sub(to.parent().unwrap_or(Path::new("in"))).map_err(|e| format!("input {p}: {e}"))?;
        if !left_out(&name.to_string_lossy()) {
            copy_entry(&dir, &from, &name).map_err(|e| format!("input {p}: {e}"))?;
        }
        self.listed.push(format!("in/{p}"));
        Ok(())
    }
}

/// The third field of the case id (`cut -d. -f3`): the whole id when it holds no dot.
fn story_of(id: &str) -> String {
    if !id.contains('.') {
        return id.to_owned();
    }
    id.split('.').nth(2).unwrap_or("").to_owned()
}

/// The inputs of a decision: the idea, the intake's results, the direction's artifacts and the
/// subject under review (a close reads the published result instead of the plan).
pub(super) fn collect(req: &Value, case_dir: &Path, cwd: &Held) -> Result<Inputs, String> {
    let work = case_dir.join("work");
    let mut inputs = Inputs::default();
    for p in ["idea.yaml", "dissection.yaml", "questions.yaml", "requirements.yaml", "context.yaml"] {
        inputs.add(&work, cwd, p)?;
    }
    if req["type"] == "close" {
        inputs.add(&work, cwd, "published/README.md")?;
    } else {
        for p in ["aep/vision/product.md", "aep/initiative/roadmap.md", "aep/epic"] {
            inputs.add(&work, cwd, p)?;
        }
    }
    let id = match read_regular(&case_dir.join("case.json")).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()) {
        Some(case) => raw(&case["id"]),
        None => String::new(),
    };
    let story = story_of(&id);
    if !story.is_empty() {
        inputs.add(&work, cwd, &format!("aep/story/{story}.md"))?;
    }
    let path = raw(or(&req["path"], &Value::String(String::new())));
    if !["", "idea.yaml", "dissection.yaml", "questions.yaml", "requirements.yaml", "context.yaml"].contains(&path.as_str()) && !path.starts_with("aep/") {
        inputs.add(&work, cwd, &path)?;
    }
    Ok(inputs)
}

/// The names of the claims a formula mentions: `[.. | objects | .claim? | objects | .id] | unique`.
fn claim_ids(formula: &Value) -> Vec<Value> {
    fn walk(v: &Value, out: &mut Vec<Value>) {
        match v {
            Value::Object(map) => {
                if let Some(claim) = map.get("claim").filter(|c| c.is_object()) {
                    out.push(claim["id"].clone());
                }
                map.values().for_each(|x| walk(x, out));
            }
            Value::Array(items) => items.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(formula, &mut out);
    out.sort_by_key(|v| v.to_string());
    out.dedup();
    out
}

fn precondition(ir: &Value, action: &str) -> Value {
    or(&ir["actions"][action]["precondition"], &json!({})).clone()
}

fn names(list: &Value) -> Vec<String> {
    list.as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_owned)).collect()
}

/// `in/checks.json`: the claims the decision turns on. An approval's claims are those of the
/// action's precondition, a close's those of its outcome, a question's or a finding's those of the
/// action that takes its answer (all of them when it has none). What only a step after the action
/// can meet is listed apart; the claims the action is there to settle are marked.
pub(super) fn checks(req: &Value, case_dir: &Path) -> Value {
    let ir_file = case_dir.join("ir.json");
    let has_ir = std::fs::symlink_metadata(&ir_file).map(|m| m.is_file() && m.len() > 0).unwrap_or(false);
    let ir = if has_ir { read_json(&ir_file) } else { Value::Null };
    let kind = req["type"].as_str().unwrap_or("");
    let action = req["action"].as_str().unwrap_or("");
    let mut reads: Option<Vec<Value>> = None;
    let mut settles: Vec<Value> = Vec::new();
    if has_ir {
        reads = match kind {
            "approval" => Some(claim_ids(&precondition(&ir, action))),
            "question" | "finding" => Some(claim_ids(&precondition(&ir, action))).filter(|c| !c.is_empty()),
            "close" => Some(claim_ids(or(&ir["outcomes"][raw(&req["outcome"])]["requires"], &json!({})))),
            _ => None,
        };
        let mut conditions: Vec<(Value, Value)> = Vec::new();
        fn collect(v: &Value, out: &mut Vec<(Value, Value)>) {
            match v {
                Value::Object(map) => {
                    if let Some(claim) = map.get("claim").filter(|c| c.is_object()) {
                        out.push((claim["id"].clone(), claim["is"].clone()));
                    }
                    map.values().for_each(|x| collect(x, out));
                }
                Value::Array(items) => items.iter().for_each(|x| collect(x, out)),
                _ => {}
            }
        }
        collect(&precondition(&ir, action), &mut conditions);
        let mut ids: Vec<Value> = conditions.iter().map(|(id, _)| id.clone()).collect();
        ids.sort_by_key(|v| v.to_string());
        ids.dedup();
        for id in ids {
            if conditions.iter().filter(|(i, _)| *i == id).all(|(_, is)| is != "true") {
                settles.push(id);
            }
        }
    }
    let later = if has_ir && kind != "close" && ir["actions"].get(action).is_some() { sorted(protocol_later(&ir, action)) } else { json!({"claims": [], "obligations": []}) };
    let later_claims = names(&later["claims"]);
    let later_obligations = names(&later["obligations"]);
    let position = &req["position"];
    let all_claims: Map<String, Value> = position["claims"].as_object().cloned().unwrap_or_default();
    let mut claims = Map::new();
    for (key, value) in &all_claims {
        if reads.as_ref().is_some_and(|r| !r.iter().any(|x| x.as_str() == Some(key))) || later_claims.contains(key) {
            continue;
        }
        let mut value = value.clone();
        if settles.iter().any(|s| s.as_str() == Some(key)) {
            match value.as_object_mut() {
                Some(map) => {
                    map.insert("settled_by_this_decision".to_owned(), json!(true));
                }
                None => value = json!({"settled_by_this_decision": true}),
            }
        }
        claims.insert(key.clone(), value);
    }
    let obligations: Vec<&Value> = position["open_obligations"].as_array().into_iter().flatten().collect();
    let open: Vec<&Value> = obligations.iter().copied().filter(|o| !o.as_str().is_some_and(|s| later_obligations.iter().any(|l| l == s))).collect();
    let met: Vec<&Value> = obligations.iter().copied().filter(|o| o.as_str().is_some_and(|s| later_obligations.iter().any(|l| l == s))).collect();
    let mut keys: Vec<&String> = all_claims.keys().collect();
    keys.sort();
    let met_claims: Vec<&String> = keys.into_iter().filter(|k| later_claims.contains(k)).collect();
    let mut out = Map::new();
    out.insert("claims".to_owned(), Value::Object(claims));
    out.insert("open_obligations".to_owned(), json!(open));
    out.insert("legitimate_outcomes".to_owned(), position["legitimate_outcomes"].clone());
    if !met.is_empty() || !met_claims.is_empty() {
        let after = raw(&later["action"]);
        out.insert("met_after_this_action".to_owned(), json!({"action": later["action"], "obligations": met, "claims": met_claims, "note": format!("met after {after}, not a condition of this decision")}));
    }
    Value::Object(out)
}

/// A YAML file as JSON with its keys in file order; None when it is missing or not YAML.
fn yaml(path: &Path) -> Option<Value> {
    let text = read_regular_text(path).ok()?;
    serde_yaml::from_str::<serde_yaml::Value>(&text).ok().and_then(|v| serde_json::to_value(v).ok())
}

/// The dissection item a question was asked about.
pub(super) fn item_of(req: &Value, work: &Path) -> Value {
    if req["type"] != "question" {
        return Value::Null;
    }
    let wanted = raw(or(&req["question"]["item"], &Value::String(String::new())));
    yaml(&work.join("dissection.yaml"))
        .and_then(|d| d["items"].as_array().and_then(|items| items.iter().find(|i| i["id"].as_str() == Some(wanted.as_str())).cloned()))
        .unwrap_or(Value::Null)
}

/// What a finding touches: the requirements it cites, the prior decisions (answered questions) it or
/// those requirements cite, and the context fields it cites.
pub(super) fn touches(req: &Value, work: &Path) -> Value {
    if req["type"] != "finding" {
        return Value::Null;
    }
    let finding = &req["finding"];
    let cites: Vec<Value> = or(&finding["cites"], &json!([])).as_array().cloned().unwrap_or_default();
    let requirements = yaml(&work.join("requirements.yaml")).unwrap_or(Value::Null);
    let questions = yaml(&work.join("questions.yaml")).unwrap_or(Value::Null);
    let context = yaml(&work.join("context.yaml")).unwrap_or(Value::Null);
    let rs: Vec<Value> = requirements["requirements"].as_array().into_iter().flatten().filter(|r| cites.contains(&r["id"])).cloned().collect();
    let mut qids = cites.clone();
    for r in &rs {
        qids.extend(r["cites"].as_array().cloned().unwrap_or_default());
    }
    let decisions: Vec<Value> = questions["questions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|q| qids.contains(&q["id"]) && (q["status"] == "accepted" || q["status"] == "changed"))
        .map(|q| json!({"id": q["id"], "question": q["question"], "answer": q["answer"], "status": q["status"], "answered_by": q["answered_by"]}))
        .collect();
    let fields: Vec<Value> = cites
        .iter()
        .filter_map(|c| c.as_str()?.strip_prefix("context.").map(str::to_owned))
        .map(|k| json!({"field": format!("context.{k}"), "value": context[&k]["value"], "status": context[&k]["status"]}))
        .collect();
    json!({"requirements": rs, "decisions": decisions, "context": fields})
}

/// `in/decision.json`: the decision as the model reads it.
pub(super) fn decision(req: &Value, item: &Value, touches: &Value, citable: &Value, prior: &Value) -> Value {
    let case = req["case_dir"].as_str().unwrap_or("").rsplit('/').next().unwrap_or("").to_owned();
    let mut d = json!({
        "type": req["type"], "case": case, "action": req["action"], "capability": req["capability"], "subject": req["subject"],
        "artifact_path": req["path"], "revision": req["revision"], "question": req["question"], "item": item,
        "idea_excerpt": or(&item["span"], &Value::Null), "finding": or(&req["finding"], &Value::Null), "touches": touches,
        "outcome": req["outcome"], "citable_artifacts": citable["artifacts"], "prior_rejections": prior,
    });
    if let Some(map) = d.as_object_mut() {
        if !req["note"].is_null() && req["note"] != false {
            map.insert("note".to_owned(), req["note"].clone());
        }
        if !req["refusal"].is_null() && req["refusal"] != false {
            map.insert("refusal".to_owned(), req["refusal"].clone());
        }
    }
    d
}

/// Writes a document as `jq` does, indented and then a newline, as the file `leaf` of the held directory `dir`.
pub(super) fn write_pretty(dir: &Held, leaf: &str, value: &Value) -> Result<(), String> {
    dir.write(leaf, format!("{}\n", serde_json::to_string_pretty(value).unwrap_or_default()).as_bytes())
}

/// Everything below `in/` the model is shown, and the prompt's pieces that come from it.
pub(super) struct Prepared {
    pub(super) inputs: Inputs,
    pub(super) citable: Value,
    pub(super) prior: usize,
    pub(super) met_after: bool,
    pub(super) schema: Value,
}

pub(super) fn prepare(req: &Value, case_dir: &Path, cwd: &Held, r: &Renderer, final_escalate: bool) -> Result<Prepared, String> {
    let work = case_dir.join("work");
    let mut inputs = collect(req, case_dir, cwd)?;
    let checks = checks(req, case_dir);
    write_pretty(&cwd.sub(Path::new("in"))?, "checks.json", &checks)?;
    inputs.listed.push(r.render("input-checks.md", &[])?);
    let item = item_of(req, &work);
    if req["type"] == "finding" {
        inputs.add(&work, cwd, "findings.yaml")?;
    }
    let touches = touches(req, &work);
    let facts = sdc_tools::reply::records::Facts::driver(case_dir);
    let context = sdc_tools::reply::citations::ctx(cwd.path(), &facts);
    let citable = sdc_tools::reply::citations::citable_set(&context);
    let prior = if req["type"] == "approval" { sdc_tools::reply::records::rejections(case_dir, req["action"].as_str().unwrap_or("")) } else { json!([]) };
    write_pretty(&cwd.sub(Path::new("in"))?, "decision.json", &decision(req, &item, &touches, &citable, &prior))?;
    let finding = &req["finding"];
    let options: Vec<String> = names(or(&finding["options"], &json!([])));
    let rule = !finding["sides"].is_null() || !finding["trace"].is_null();
    let schema = sdc_tools::reply::schemas::decider_schema(&context, req["type"].as_str().unwrap_or(""), final_escalate, &if req["type"] == "finding" { options } else { Vec::new() }, req["type"] == "finding" && rule);
    Ok(Prepared { inputs, citable, prior: prior.as_array().map_or(0, Vec::len), met_after: !checks["met_after_this_action"].is_null() && checks["met_after_this_action"] != false, schema })
}

/// The reply schema as `sdc-reply schema` prints it: compact, a newline after.
pub(super) fn schema_text(schema: &Value) -> String {
    format!("{}\n", sdc_tools::reply::text(schema))
}

