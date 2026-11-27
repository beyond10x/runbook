//! One call: its record directory, the prompt, the request, the adapter, the apply step and the
//! call log.

use crate::nodes::{self, Inputs};
use crate::ordjson::{NumLit, Ov};
use crate::util::{chomp, now_utc, cp_r, rm_rf, run_cmd, Exit, Out, R};
use crate::{yaml, Ctx};
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The agent node of a delegation that changes a repository (`runbook/src/handback.rs`).
pub const CHANGE_NODE: &str = "delegation.change";
/// The variable the daemon names the run's worktree in.
pub const WORKTREE_VARIABLE: &str = "RUNBOOK_WORKTREE";

/// The run's worktree the daemon named: an absolute path of a directory.
fn change_tree() -> Option<PathBuf> {
    let tree = PathBuf::from(std::env::var_os(WORKTREE_VARIABLE)?);
    (tree.is_absolute() && tree.is_dir()).then_some(tree)
}

/// The text of the last result event of a call's `events.jsonl` (the CLI's final reply), if it has one.
fn final_reply(rec: &Path) -> Option<String> {
    let text = fs::read_to_string(rec.join("events.jsonl")).ok()?;
    text.lines().rev().filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok()).find(|e| e["type"] == "result").and_then(|e| e["result"].as_str().map(str::to_owned))
}

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
pub struct Request<'a> {
    pub model: &'a str,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    max_budget_usd: Option<&'a NumLit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_attempt: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implement_mode: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage_attempt: Option<usize>,
}

/// The pretty JSON text of `v` with a trailing newline, as `jq` writes a file.
pub fn pretty<T: Serialize>(v: &T) -> String {
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
pub fn tail_lines(bytes: &[u8], n: usize) -> &[u8] {
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
    // The agent node of a delegation that changes a repository works in the run's worktree, which the daemon names; the
    // call writes nothing into it (no in/, no out/). Every other node works in its call's own directory.
    let cwd = if c.node == CHANGE_NODE {
        match change_tree() {
            Some(tree) => tree,
            None => return Ok(crate::util::die(format!("llm-node: {} could not run: {WORKTREE_VARIABLE} names no directory of a worktree (an absolute path)", c.node), 126).0),
        }
    } else {
        let cwd = rec.join("cwd");
        let _ = fs::create_dir_all(cwd.join("in"));
        let _ = fs::create_dir_all(cwd.join("out"));
        cwd
    };
    let _ = fs::create_dir_all(&rec);
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
    let mut task = built.task.clone();

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
    // (One request inlines findings.yaml itself and asks for findings in the reply.)
    if c.raises() && !c.request_mode {
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

    let mut rework_limit = String::new();
    if let Some(rej) = &c.rejection {
        let doc = fs::read_to_string(rej).ok().and_then(|t| Ov::parse(&t).ok()).unwrap_or(Ov::Null);
        let show = |k: &str| doc.get(k).map(Ov::raw).unwrap_or_else(|| "null".to_string());
        let ends = fs::read_to_string(c.set.root.join("driver/examples/sdc/rule-ends.json")).ok().and_then(|t| Ov::parse(&t).ok());
        rework_limit = ends.as_ref().and_then(|e| e.get("rework_limit")).map(Ov::raw).unwrap_or_default();
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
    // One request: the program that makes it writes the prompt, the whole input.
    if !c.request_mode {
        let preamble = c.render(
            if c.node == CHANGE_NODE {
                "instructions/shared/preamble-change.md"
            } else if c.facts() {
                "instructions/shared/preamble-facts.md"
            } else {
                "instructions/shared/preamble.md"
            },
            &[("node", &c.node)],
        )?;
        let closing = if c.node == CHANGE_NODE {
            c.render("instructions/shared/closing-change.md", &[])?
        } else if c.node == "change.review" {
            match built.review.as_ref().and_then(|r| r.doc.as_ref()) {
                // The review input's exact bytes, the trailing newline kept: the file ends the paragraph.
                Some(doc) => {
                    let text = String::from_utf8_lossy(&fs::read(doc).unwrap_or_default()).into_owned();
                    c.render("instructions/shared/closing-review-facts.md", &[("review_doc", &text)])?
                }
                None => c.render("instructions/shared/closing-review.md", &[])?,
            }
        } else if c.coverage {
            c.render(if c.rejection.is_some() { "instructions/shared/closing-coverage-rework.md" } else { "instructions/shared/closing-coverage.md" }, &[])?
        } else {
            c.render(if c.rejection.is_some() { "instructions/shared/closing-rework.md" } else { "instructions/shared/closing.md" }, &[])?
        };
        let input_list = if inp.list.is_empty() { "- ".to_string() } else { chomp(&inp.list.iter().map(|i| format!("- {i}\n")).collect::<String>()).to_string() };
        let frame = c.render("instructions/shared/frame.md", &[("preamble", &preamble), ("inputs", &input_list), ("task", &task), ("closing", &closing)])?;
        let _ = fs::write(&prompt, format!("{frame}\n"));
    }

    // The model and the reasoning effort: the binding's, else the class default.
    let model = c.set.cfg.model.clone().unwrap_or_else(|| "claude-haiku-5-5".to_string());
    let default_ops: Vec<String> = built.ops.iter().map(|o| o.to_string()).collect();
    let mut operations = c.set.cfg.operations.clone().unwrap_or(default_ops);
    let mut max_turns = c.set.cfg.max_turns.clone().unwrap_or_else(|| NumLit(built.turns.to_string()));
    if c.facts() {
        // The facts review makes one request without a tool and at most 4 turns, whatever the binding gives.
        operations.clear();
        max_turns = NumLit("4".into());
    }
    if c.request_mode {
        // No operation and a turn cap for the CLI's own schema re-prompt, whatever the binding gives.
        operations.clear();
        max_turns = NumLit("3".into());
    }
    // The daemon's bound: no more turns than the run has model calls left. It only lowers the bound.
    if let Some(left) = c.set.calls_left {
        if max_turns.0.parse::<f64>().map_or(true, |t| t > left as f64) {
            max_turns = NumLit(left.to_string());
        }
    }
    // A money budget is the binding's alone: with none named the request carries none.
    let max_budget = c.set.cfg.max_budget_usd.clone();
    let case_dir = std::env::var("SDC_CASE_DIR").unwrap_or_default();
    let mut req = Request {
        model: &model,
        effort: c.effort.as_deref().filter(|e| !e.is_empty()),
        prompt_file: prompt.display().to_string(),
        cwd: cwd.display().to_string(),
        record_dir: rec.display().to_string(),
        node: &c.node,
        attempt,
        case: &c.set.case,
        case_dir: &case_dir,
        operations: &operations,
        max_turns: &max_turns,
        max_budget_usd: max_budget.as_ref(),
        json_schema: built.schema.as_ref().map(|s| s.display().to_string()),
        review_attempt: None,
        implement_mode: c.request_mode.then_some("request"),
        max_output_tokens: c.request_mode.then_some(32000),
        coverage_attempt: None,
    };
    let request = rec.join("request.json");
    let _ = fs::write(&request, pretty(&req));
    if c.node == "change.review" {
        return crate::review::call(c, rec_root, rec, cwd, attempt, &built, &mut req, feedback);
    }
    if c.request_mode {
        return crate::implement::request_call(c, rec_root, rec, cwd, attempt, &built, &req, feedback, &rework_limit);
    }

    // The adapter, and for a coverage document the check of its reply and the asking again.
    let started = now_utc(false);
    let calls = crate::implement::session_calls(c, rec_root, rec, cwd, attempt, &started, &mut req, &built, feedback)?;
    let acode = calls.acode;
    let res = fs::read_to_string(rec.join("response.json")).ok().and_then(|t| Ov::parse(&t).ok());
    let reason = |key: &str| res.as_ref().and_then(|r| r.get(key)).cloned().unwrap_or(Ov::Null);

    let mut code;
    if c.adapter == "dry" {
        code = c.run_stub(&mut Out::inherit());
        if code == 0 && !c.coverage_stub() {
            code = 126;
        }
        if code == 0 {
            c.post("", &mut Out::append(&rec.join("apply.log")))?;
        }
    } else if acode == 0 && calls.invalid {
        code = 2;
        let err = fs::read_to_string(rec.join("coverage-check.err")).unwrap_or_default();
        let call = rec.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let msg = c.render(
            "instructions/shared/node.coverage.invalid.md",
            &[("node", &c.node), ("attempts", &(calls.tries + 1).to_string()), ("call", &call), ("reason", chomp(&err))],
        )?;
        Out::inherit().line(msg.as_bytes());
    } else if acode == 0 {
        code = apply(c, built.out, rec, cwd, &calls.reply)?;
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
    if c.coverage && code == 0 && c.adapter != "dry" {
        code = crate::implement::refuse_uncovered(c);
    }
    log_call(c, rec_root, rec, attempt, &started, &req, built.out, acode, code, feedback, None);
    Ok(code)
}

/// Copies back only the declared output. `coverage` is the checked coverage document of a
/// `change.implement` that replies with one.
pub fn apply(c: &Ctx, out: &str, rec: &Path, cwd: &Path, coverage: &Path) -> R<i32> {
    let work = &c.work;
    let log = rec.join("apply.log");
    let mut sink = Out::truncate(&log);
    if let Some(code) = crate::store::apply(c, cwd, &mut sink)? {
        return Ok(code);
    }
    Ok(apply_copy(c, out, rec, cwd, work, coverage, &mut sink))
}

/// The nodes whose output is a plain copy back.
fn apply_copy(c: &Ctx, out: &str, rec: &Path, cwd: &Path, work: &Path, coverage: &Path, sink: &mut Out) -> i32 {
    let copy_back = |from: &Path, to: &Path| -> bool {
        rm_rf(to).is_ok() && to.parent().is_none_or(|p| fs::create_dir_all(p).is_ok()) && cp_r(from, to).is_ok()
    };
    match c.node.as_str() {
        "change.implement" if c.coverage => {
            // The tree, then the checked coverage document inside it (part of the revision it describes).
            if !cwd.join("out/impl").is_dir() {
                return 1;
            }
            if !copy_back(&cwd.join("out/impl"), &work.join("impl")) || fs::create_dir_all(work.join("impl/.sdc")).is_err() {
                return 1;
            }
            let doc = fs::read_to_string(coverage).ok().and_then(|t| Ov::parse(&t).ok());
            let text = doc.as_ref().map(pretty).unwrap_or_default();
            let wrote = fs::write(work.join("impl/.sdc/coverage.json"), text).is_ok();
            if doc.is_some() && wrote { 0 } else { 2 }
        }
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
            run_cmd(Command::new(c.set.here.join("sdc-tool.sh")).args(["bugfix", "repro-install"]), sink)
        }
        // The change is in the worktree: what goes back is the summary, the CLI's final reply, as result.md.
        CHANGE_NODE => match final_reply(rec) {
            Some(text) if !text.trim().is_empty() => i32::from(fs::write(work.join(out), format!("{}\n", text.trim_end())).is_err()),
            _ => 1,
        },
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
pub fn log_call(c: &Ctx, rec_root: &Path, rec: &Path, attempt: usize, started: &str, req: &Request, out: &str, acode: i32, code: i32, retry: bool, input_bytes: Option<u64>) {
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
    if let Some(budget) = req.max_budget_usd {
        // after `max_turns`, where the record always had it; with no budget named the record names none
        let after = fields.iter().position(|(k, _)| k == "max_turns").map_or(fields.len(), |i| i + 1);
        fields.insert(after, ("max_budget_usd".into(), Ov::Lit(budget.clone())));
    }
    if let Some(bytes) = input_bytes {
        fields.push(("input_bytes".into(), Ov::Num(bytes.into())));
    }
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
