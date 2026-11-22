//! One call: its record directory, the prompt, the request, the adapter, the apply step and the
//! call log (lines 387-416, 600-736, 792-960 of `llm-node.sh`).

use crate::nodes::{self, Inputs};
use crate::ordjson::{NumLit, Ov};
use crate::util::{chomp, now_utc, cp_r, rm_rf, run_cmd, run_to_file, Exit, Out, R};
use crate::{yaml, Ctx};
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize)]
struct Status<'a> {
    format: &'a str,
    state: &'a str,
    pid: u32,
    started_at: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    finished_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exit: Option<i32>,
}

#[derive(Serialize)]
struct Request<'a> {
    model: &'a str,
    effort: Option<&'a str>,
    prompt_file: String,
    cwd: String,
    record_dir: String,
    node: &'a str,
    attempt: usize,
    case: &'a str,
    case_dir: &'a str,
    operations: &'a [String],
    max_turns: &'a NumLit,
    max_budget_usd: &'a NumLit,
    #[serde(skip_serializing_if = "Option::is_none")]
    json_schema: Option<String>,
}

fn pretty<T: Serialize>(v: &T) -> String {
    let mut t = serde_json::to_string_pretty(v).unwrap_or_default();
    t.push('\n');
    t
}

/// The status file of a call (`sdc.call-status/1`): running now, finished or failed.
fn write_status(rec: &Path, state: &str, exit: Option<i32>, started: &str) {
    let s = Status {
        format: "sdc.call-status/1",
        state,
        pid: std::process::id(),
        started_at: started,
        finished_at: exit.map(|_| now_utc(true)),
        exit,
    };
    let tmp = rec.join(format!(".status.json.{}", std::process::id()));
    if fs::write(&tmp, pretty(&s)).is_ok() {
        let _ = fs::rename(&tmp, rec.join("status.json"));
    }
}

fn count_dirs(dir: &Path, suffix: &str) -> usize {
    fs::read_dir(dir)
        .map(|d| {
            d.flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()) && e.file_name().to_string_lossy().ends_with(suffix))
                .count()
        })
        .unwrap_or(0)
}

/// The last `n` lines of `bytes`, as `tail -n` keeps them.
fn tail_lines(bytes: &[u8], n: usize) -> &[u8] {
    let body = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let mut seen = 0;
    for (i, b) in body.iter().enumerate().rev() {
        if *b == b'\n' {
            seen += 1;
            if seen == n {
                return &bytes[i + 1..];
            }
        }
    }
    bytes
}

/// Makes the call's record directory, runs the node and records how it ended.
pub fn run(c: &Ctx) -> R<i32> {
    let rec_root = c.work.join(".sdc/llm");
    let _ = fs::create_dir_all(&rec_root);
    let n = count_dirs(&rec_root, "") + 1;
    let rec = rec_root.join(format!("{n:03}-{}", c.node));
    let cwd = rec.join("cwd");
    let _ = fs::create_dir_all(cwd.join("in"));
    let _ = fs::create_dir_all(cwd.join("out"));
    let started = now_utc(true);
    write_status(&rec, "running", None, &started);
    let attempt = count_dirs(&rec_root, &format!("-{}", c.node));
    let code = match body(c, &rec_root, &rec, &cwd, attempt) {
        Ok(code) => code,
        Err(Exit(code)) => code,
    };
    write_status(&rec, if code == 0 { "finished" } else { "failed" }, Some(code), &started);
    Ok(code)
}

fn append_task(c: &Ctx, task: &mut String, rel: &str, vals: &[(&str, &str)]) -> R<()> {
    let a = c.render(rel, vals)?;
    task.push_str("\n\n");
    task.push_str(&a);
    Ok(())
}

/// `.log` of a feedback record as a path text, `""` for none.
fn log_of(r: &Ov) -> String {
    r.field("log").map(Ov::raw).unwrap_or_default()
}

/// The check's own output: the last 80 lines of its log without the build pool's `slot: job` lines.
fn log_tail(path: &str) -> String {
    let p = Path::new(path);
    if path.is_empty() || !p.is_file() {
        return String::new();
    }
    let bytes = fs::read(p).unwrap_or_default();
    let text = String::from_utf8_lossy(&bytes);
    let mut lines: Vec<&str> = text.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    let kept: Vec<&str> = lines.into_iter().filter(|l| !l.starts_with("slot: job ")).collect();
    let tail = &kept[kept.len().saturating_sub(80)..];
    chomp(&tail.join("\n")).to_string()
}

fn tostr(v: Option<&Ov>) -> String {
    match v {
        Some(Ov::Str(s)) => s.clone(),
        Some(other) => other.compact(),
        None => "null".to_string(),
    }
}

fn body(c: &Ctx, rec_root: &Path, rec: &Path, cwd: &Path, attempt: usize) -> R<i32> {
    let work = &c.work;
    let mut inp = Inputs::new(work, cwd);
    let built = nodes::build(c, rec, cwd, &mut inp)?;
    let mut task = built.task;

    // A run seeded from a ticket: the ticket's constraints bind the code writers.
    if matches!(c.node.as_str(), "test.reproduce" | "change.fix") && c.has_ticket() {
        let bug = yaml::load(&work.join("bug.yaml"));
        let constraints: Vec<String> = bug
            .as_ref()
            .and_then(|d| yaml::get(d, "constraints"))
            .and_then(serde_yaml::Value::as_sequence)
            .map(|s| s.iter().map(|i| format!("- {}", yaml::text(Some(i)))).collect())
            .unwrap_or_default();
        if !constraints.is_empty() {
            append_task(c, &mut task, "instructions/shared/constraints.md", &[("constraints", &constraints.join("\n"))])?;
        }
    }
    if c.raises() {
        if work.join("findings.yaml").is_file() {
            inp.input("findings.yaml");
        }
        append_task(c, &mut task, "instructions/shared/findings.md", &[])?;
    }

    // The failed checks of the previous attempt, in in/feedback.md.
    let mut feedback = false;
    let rejection_text = c.rejection.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
    let records: Vec<Ov> = c
        .feedback_records()
        .into_iter()
        .filter(|r| r.str_of("result") != Some("rejected") || log_of(r) != rejection_text)
        .collect();
    if !records.is_empty() {
        let head = c.render("instructions/shared/feedback.heading.md", &[])?;
        let none = c.render("instructions/shared/feedback.no-output.md", &[])?;
        let mut doc = format!("{head}\n");
        for r in &records {
            let action = r.field("action").map_or_else(|| "?".to_string(), |v| tostr(Some(v)));
            let exit = r.field("exit").map_or_else(|| "?".to_string(), |v| tostr(Some(v)));
            let line = c.render(
                "instructions/shared/feedback.check.md",
                &[("kind", &tostr(r.get("kind"))), ("result", &tostr(r.get("result"))), ("action", &action), ("exit", &exit)],
            )?;
            doc.push_str(&format!("\n{line}\n"));
            let own = log_tail(&log_of(r));
            if own.chars().any(|ch| !ch.is_whitespace()) {
                let fenced = c.render("instructions/shared/feedback.log.md", &[("log", &own)])?;
                doc.push_str(&format!("{fenced}\n"));
            } else {
                doc.push_str(&format!("{none}\n"));
            }
        }
        let _ = fs::write(cwd.join("in/feedback.md"), doc);
        let notes = c.render("instructions/shared/feedback.inputs.md", &[])?;
        inp.notes(&notes);
        feedback = true;
    }

    if let Some(rej) = &c.rejection {
        let doc = fs::read_to_string(rej).ok().and_then(|t| Ov::parse(&t).ok()).unwrap_or(Ov::Null);
        let show = |k: &str| doc.get(k).map(Ov::raw).unwrap_or_else(|| "null".to_string());
        let ends = fs::read_to_string(c.set.root.join("driver/examples/sdc/rule-ends.json")).ok().and_then(|t| Ov::parse(&t).ok());
        let rework_limit = ends.as_ref().and_then(|e| e.get("rework_limit")).map(Ov::raw).unwrap_or_default();
        let _ = fs::copy(rej, cwd.join("in/rejection.json"));
        let notes = c.render(
            "instructions/shared/rework.inputs.md",
            &[("approval", &show("approval")), ("revision", &show("revision")), ("principal", &show("principal"))],
        )?;
        inp.notes(&notes);
        append_task(
            c,
            &mut task,
            "instructions/shared/rework.md",
            &[("approval", &show("approval")), ("count", &show("count")), ("rework_limit", &rework_limit)],
        )?;
    }

    let prompt = rec.join("prompt.md");
    let preamble = c.render("instructions/shared/preamble.md", &[("node", &c.node)])?;
    let closing = c.render(if c.rejection.is_some() { "instructions/shared/closing-rework.md" } else { "instructions/shared/closing.md" }, &[])?;
    let input_list = if inp.list.is_empty() { "- ".to_string() } else { chomp(&inp.list.iter().map(|i| format!("- {i}\n")).collect::<String>()).to_string() };
    let frame = c.render("instructions/shared/frame.md", &[("preamble", &preamble), ("inputs", &input_list), ("task", &task), ("closing", &closing)])?;
    let _ = fs::write(&prompt, format!("{frame}\n"));

    // The model and the reasoning effort: the binding's, else the class default.
    let model = c.set.cfg.model.clone().unwrap_or_else(|| "claude-haiku-5-5".to_string());
    let default_ops: Vec<String> = ["dir.list", "file.edit", "file.read", "file.write", "search"].map(String::from).to_vec();
    let operations = c.set.cfg.operations.clone().unwrap_or(default_ops);
    let max_turns = c.set.cfg.max_turns.clone().unwrap_or_else(|| NumLit("30".into()));
    let max_budget = c.set.cfg.max_budget_usd.clone().unwrap_or_else(|| NumLit("1".into()));
    let req = Request {
        model: &model,
        effort: c.effort.as_deref().filter(|e| !e.is_empty()),
        prompt_file: prompt.display().to_string(),
        cwd: cwd.display().to_string(),
        record_dir: rec.display().to_string(),
        node: &c.node,
        attempt,
        case: &c.set.case,
        case_dir: "",
        operations: &operations,
        max_turns: &max_turns,
        max_budget_usd: &max_budget,
        json_schema: built.schema.as_ref().map(|s| s.display().to_string()),
    };
    let request = rec.join("request.json");
    let _ = fs::write(&request, pretty(&req));

    // The adapter.
    let started = now_utc(false);
    let response = rec.join("response.json");
    let _ = fs::remove_file(rec.join("structured-output.json"));
    let acode = run_to_file(Command::new(&c.adapter_bin).arg(&request), &response);
    if fs::metadata(&response).map_or(true, |m| m.len() == 0) {
        let _ = fs::write(&response, "{}\n");
    }
    let res = fs::read_to_string(&response).ok().and_then(|t| Ov::parse(&t).ok());
    let reason = |key: &str| res.as_ref().and_then(|r| r.get(key)).cloned().unwrap_or(Ov::Null);

    let code;
    if c.adapter == "dry" {
        code = c.run_stub(&mut Out::inherit());
        if code == 0 {
            c.post("", &mut Out::append(&rec.join("apply.log")))?;
        }
    } else if acode == 0 {
        code = apply(c, built.out, rec, cwd);
        if code != 0 {
            let head = c.render("instructions/shared/node.not-applied.md", &[("node", &c.node), ("code", &code.to_string())])?;
            Out::inherit().line(head.as_bytes());
            let log = fs::read(rec.join("apply.log")).unwrap_or_default();
            let _ = std::io::stdout().write_all(tail_lines(&log, 20));
        }
        if code == 0 {
            let findings = cwd.join("out/findings.yaml");
            let raised = if findings.is_file() { findings.display().to_string() } else { String::new() };
            c.post(&raised, &mut Out::append(&rec.join("apply.log")))?;
        }
    } else if reason("terminal_reason") == Ov::Str("api_error".into()) {
        // The model's API refused or broke off: the node could not run, which says nothing about
        // its subject. Exit 75: resume later on the same model.
        let status = match reason("api_error_status") {
            v if v.truthy() => format!(" {}", v.raw()),
            _ => String::new(),
        };
        let msg = c.render("instructions/shared/node.api-ended.md", &[("node", &c.node), ("status", &status)])?;
        Out::inherit().line(msg.as_bytes());
        code = 75;
    } else {
        code = acode;
        let why = match (reason("terminal_reason"), reason("error")) {
            (t, _) if t.truthy() => t.raw(),
            (_, e) if e.truthy() => e.raw(),
            _ => "no reason given".to_string(),
        };
        let mut shown = format!("{why}\n").into_bytes();
        shown.truncate(300);
        let why = String::from_utf8_lossy(&shown).into_owned();
        let msg = c.render("instructions/shared/node.call-exit.md", &[("node", &c.node), ("code", &acode.to_string()), ("reason", chomp(&why))])?;
        Out::inherit().line(msg.as_bytes());
    }
    log_call(c, rec_root, rec, attempt, &started, &req, built.out, acode, code, feedback);
    Ok(code)
}

/// Copies back only the declared output.
fn apply(c: &Ctx, out: &str, rec: &Path, cwd: &Path) -> i32 {
    let work = &c.work;
    let log = rec.join("apply.log");
    let mut sink = Out::truncate(&log);
    let copy_back = |from: &Path, to: &Path| -> bool {
        rm_rf(to).is_ok() && to.parent().is_none_or(|p| fs::create_dir_all(p).is_ok()) && cp_r(from, to).is_ok()
    };
    match c.node.as_str() {
        "bug.dissect" => {
            // The reply under the response schema, when the adapter had one, else the written file.
            let mut f: PathBuf = cwd.join("out/bug-dissection.json");
            if fs::metadata(rec.join("structured-output.json")).is_ok_and(|m| m.len() > 0) {
                f = rec.join("structured-output.json");
            }
            if !f.is_file() {
                return 1;
            }
            i32::from(fs::copy(&f, work.join("bug-dissection.json")).is_err())
        }
        "test.reproduce" => {
            if !cwd.join("out/repro").is_dir() {
                return 1;
            }
            if !copy_back(&cwd.join("out/repro"), &work.join("repro")) {
                return 1;
            }
            run_cmd(Command::new(c.set.here.join("sdc-tool.sh")).args(["bugfix", "repro-install"]), &mut sink)
        }
        _ => {
            if !cwd.join("out").join(out).exists() {
                return 1;
            }
            i32::from(!copy_back(&cwd.join("out").join(out), &work.join(out)))
        }
    }
}

/// One line per call, for the run log and the viewer.
#[allow(clippy::too_many_arguments)]
fn log_call(c: &Ctx, rec_root: &Path, rec: &Path, attempt: usize, started: &str, req: &Request, out: &str, acode: i32, code: i32, retry: bool) {
    let Some(res) = fs::read_to_string(rec.join("response.json")).ok().and_then(|t| Ov::parse(&t).ok()).filter(|r| matches!(r, Ov::Obj(_))) else {
        return;
    };
    let r = |k: &str| res.get(k).cloned().unwrap_or(Ov::Null);
    let s = |v: &str| Ov::Str(v.to_string());
    let n = |v: i32| Ov::Num(v.into());
    let mut fields: Vec<(String, Ov)> = vec![
        ("action".into(), s(&c.node)),
        ("case".into(), s(&c.set.case)),
        ("started".into(), s(started)),
        ("attempt".into(), Ov::Num((attempt as u64).into())),
        ("retry".into(), Ov::Bool(retry)),
        ("adapter".into(), r("adapter")),
        ("model".into(), s(req.model)),
        ("effort".into(), s(req.effort.unwrap_or("default"))),
        ("class".into(), s(&c.class)),
        ("allowed_operations".into(), Ov::Arr(req.operations.iter().map(|o| s(o)).collect())),
        ("max_turns".into(), Ov::Lit(req.max_turns.clone())),
        ("max_budget_usd".into(), Ov::Lit(req.max_budget_usd.clone())),
        ("exit".into(), n(acode)),
        ("applied_exit".into(), n(code)),
        ("output".into(), s(out)),
        ("duration_ms".into(), r("duration_ms")),
        ("cost_usd".into(), r("cost_usd")),
        ("num_turns".into(), r("num_turns")),
        ("usage".into(), r("usage")),
        ("offered_tools".into(), r("offered_tools")),
        ("tool_calls".into(), r("tool_calls")),
        ("dry".into(), if r("dry").truthy() { r("dry") } else { Ov::Bool(false) }),
        ("record".into(), s(&rec.display().to_string())),
    ];
    if r("implement_mode").truthy() {
        for k in ["implement_mode", "requests", "continuations", "reply_retries", "cause"] {
            fields.push((k.into(), r(k)));
        }
    }
    let mut line = Ov::Obj(fields).compact();
    line.push('\n');
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(rec_root.join("calls.jsonl")) {
        let _ = f.write_all(line.as_bytes());
    }
}
