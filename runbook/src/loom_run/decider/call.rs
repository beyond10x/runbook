//! One model call of the decider: the files of the call directory, the adapter program and the
//! check of its reply. The adapter is the only program the decider starts.
//!
//! The call directory `rec` holds `request.json` (what the adapter is given), `prompt.md`,
//! `reply-schema.json`, `response.json` (what the adapter printed, with the call's meta), `status.json`
//! (`sdc.call-status/1`) and `cwd/` with `in/` (what the model reads) and `out/decision.json`.

use super::inputs::{prepare, write_pretty};
use super::prompt::{or, prompt, raw, Parts, Renderer};
use super::rules::dry_reply;
use super::validate::{validate, Check};
use super::{run_program, Ran};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// What one call needs besides its request.
pub(super) struct ModelCall<'a> {
    pub(super) req: &'a Value,
    pub(super) rec: &'a Path,
    pub(super) tools: &'a Path,
    pub(super) root: &'a Path,
    pub(super) model: &'a str,
    pub(super) adapter: Option<&'a str>,
    pub(super) effort: Option<&'a str>,
    pub(super) max_turns: Option<u64>,
    pub(super) final_escalate: bool,
    pub(super) budget_micro_usd: u64,
    pub(super) env: &'a [(String, String)],
    pub(super) timeout: Duration,
}

/// The reply and what the adapter wrote on its standard error.
pub(super) struct Replied {
    pub(super) reply: Value,
    pub(super) stderr: String,
}

const DEFAULT_MODEL_TURNS: u64 = 8;

/// The present time as `YYYY-MM-DDTHH:MM:SS.mmmZ`.
pub(super) fn now() -> String {
    let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let (days, rest) = ((d.as_secs() / 86_400) as i64, d.as_secs() % 86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z", rest / 3600, rest % 3600 / 60, rest % 60, d.subsec_millis())
}

/// `status.json` of the call, written whole: running now, finished or failed.
pub(super) fn call_status(rec: &Path, state: &str, started: &str, exit: Option<i64>) {
    let mut doc = json!({"format": "sdc.call-status/1", "state": state, "pid": std::process::id(), "started_at": started});
    if let (Some(exit), Some(map)) = (exit, doc.as_object_mut()) {
        map.insert("finished_at".to_owned(), json!(now()));
        map.insert("exit".to_owned(), json!(exit));
    }
    let tmp = rec.join(".status.json.tmp");
    if write_pretty(&tmp, &doc).is_ok() {
        let _ = std::fs::rename(&tmp, rec.join("status.json"));
    }
}

/// The adapter program of the name the settings give: one of the three of the bindings copy, or a path.
pub(super) fn adapter_path(tools: &Path, adapter: &str) -> PathBuf {
    match adapter {
        "metaharness" | "claude-cli" | "dry" => tools.join(format!("llm-adapter-{adapter}.sh")),
        path => PathBuf::from(path),
    }
}

/// The effort when the settings name none: the default of the decider class in the copy's `node-classes.json`.
pub(super) fn default_effort(tools: &Path) -> Option<String> {
    let classes = sdc_tools::reply::read_json(&tools.join("node-classes.json"));
    classes["default_efforts"]["decider"].as_str().filter(|e| !e.is_empty()).map(str::to_owned)
}

/// Starts the adapter with the request file; a program that is missing or cannot be run ends with
/// the exit a shell gives (127, 126) and says why on its standard error.
pub(super) fn start(adapter: &Path, request_file: &Path, c: &ModelCall<'_>) -> Result<Ran, String> {
    use std::os::unix::fs::PermissionsExt;
    let shown = adapter.display();
    match std::fs::metadata(adapter) {
        Err(_) => return Ok(Ran { code: 127, stdout: String::new(), stderr: format!("{shown}: No such file or directory\n") }),
        Ok(m) if !m.is_file() || m.permissions().mode() & 0o111 == 0 => return Ok(Ran { code: 126, stdout: String::new(), stderr: format!("{shown}: Permission denied\n") }),
        Ok(_) => {}
    }
    run_program(adapter, &[&request_file.display().to_string()], c.env, c.rec, c.timeout)
}

/// The note on a response that was not made under a response schema.
fn schema_note(adapter: &str) -> &'static str {
    if adapter == "metaharness" {
        "not constrained: `metaharness run claude` has no response-schema option; the program check and the retry hold the reply"
    } else if adapter == "fixture" || adapter.ends_with("llm-adapter-fixture.sh") {
        "not constrained: a scripted reply, no model"
    } else {
        "not constrained: the adapter passed no response schema"
    }
}

/// The call: prepares what the model reads, asks the adapter once and checks the reply.
pub(super) fn run(c: &ModelCall<'_>) -> Result<Replied, String> {
    let adapter = c.adapter.unwrap_or("metaharness");
    if adapter == "stub" {
        let reply = json!({"decision": "escalate", "answer": null, "kind": "model", "valid": false, "invalid_reason": "the stub adapter has no LLM",
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

enum Failed {
    /// The call could not be made (an instruction file is missing, a file cannot be written).
    Program(String),
    /// The adapter was ended at the time limit.
    Ended(String),
}

fn ask(c: &ModelCall<'_>, adapter: &str, cwd: &Path) -> Result<(Replied, i64), Failed> {
    let program = Failed::Program;
    let req = c.req;
    let case_dir = PathBuf::from(raw(&req["case_dir"]));
    let renderer = Renderer { root: c.root.to_path_buf() };
    let prepared = prepare(req, &case_dir, cwd, &renderer, c.final_escalate).map_err(program)?;
    let schema_file = c.rec.join("reply-schema.json");
    std::fs::write(&schema_file, super::inputs::schema_text(&prepared.schema)).map_err(|e| program(format!("reply schema: {}", e.kind())))?;
    let has_aep = cwd.join("in").join("aep").exists();
    let text = prompt(&renderer, &Parts { req, final_escalate: c.final_escalate, inputs: &prepared.inputs, citable: &prepared.citable, prior_rejections: prepared.prior, has_aep, met_after: prepared.met_after }).map_err(program)?;
    let prompt_file = c.rec.join("prompt.md");
    std::fs::write(&prompt_file, text).map_err(|e| program(format!("prompt file: {}", e.kind())))?;

    let effort = c.effort.map(str::to_owned).or_else(|| default_effort(c.tools));
    let case = sdc_tools::reply::read_json(&case_dir.join("case.json"));
    let usd_literal = super::approval::usd_text(c.budget_micro_usd);
    let usd = usd_literal.parse::<f64>().unwrap_or(0.0);
    let request = json!({
        "model": c.model, "effort": effort, "prompt_file": prompt_file.display().to_string(), "cwd": cwd.display().to_string(),
        "record_dir": c.rec.display().to_string(), "node": format!("decide.{}", raw(&req["action"])),
        "attempt": or(&req["retry"]["attempt"], &json!(1)), "json_schema": format!("{}/reply-schema.json", c.rec.display()),
        "case": or(&case["id"], &json!("")), "case_dir": case_dir.display().to_string(), "operations": ["dir.list", "file.read", "file.write"],
        "max_turns": c.max_turns.unwrap_or(DEFAULT_MODEL_TURNS), "max_budget_usd": usd,
    });
    let request_file = c.rec.join("request.json");
    // `jq` keeps the text of a number it was given as text (`0.250000`): so does the file.
    let text = format!("{}\n", serde_json::to_string_pretty(&request).unwrap_or_default()).replacen(&format!("\"max_budget_usd\": {}", Value::from(usd)), &format!("\"max_budget_usd\": {usd_literal}"), 1);
    std::fs::write(&request_file, text).map_err(|e| program(format!("{}: {}", request_file.display(), e.kind())))?;

    let ran = start(&adapter_path(c.tools, adapter), &request_file, c).map_err(Failed::Ended)?;
    let exit = i64::from(ran.code);
    let mut response = match serde_json::from_str::<Value>(ran.stdout.trim()) {
        Ok(v) if v.is_object() => v,
        _ => json!({}),
    };
    let structured = c.rec.join("structured-output.json");
    if std::fs::read(&structured).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).is_some_and(|v| v.is_object()) {
        let _ = std::fs::copy(&structured, cwd.join("out").join("decision.json"));
    }
    if let Some(map) = response.as_object_mut() {
        let constrained = map.get("schema_constrained").is_some_and(|v| !v.is_null() && v != &json!(false));
        if !constrained {
            map.insert("schema_constrained".to_owned(), json!(false));
            if map.get("schema_note").is_none_or(|v| v.is_null() || v == &json!(false)) {
                map.insert("schema_note".to_owned(), json!(schema_note(adapter)));
            }
        }
        map.insert("exit".to_owned(), json!(exit));
    }
    write_pretty(&c.rec.join("response.json"), &response).map_err(program)?;
    let reply = if adapter == "dry" {
        dry_reply(c.rec, &response, req)
    } else {
        validate(&Check { req, cwd, case_dir: &case_dir, rec: c.rec, final_escalate: c.final_escalate }, &response)
    };
    Ok((Replied { reply, stderr: ran.stderr }, exit))
}

