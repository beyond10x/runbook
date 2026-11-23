//! `change.review`: its inputs, the call with the check of its
//! reply and the asking again (lines 750-801). Inputs, schema and check are the library's
//! (`sdc_tools::reply`); no `sdc-reply` process is started.

use crate::call::{log_call, pretty, Request};
use crate::nodes::{Built, Inputs};
use crate::ordjson::Ov;
use crate::util::{chomp, env_set, now_utc, run_split, Exit, Out, R};
use crate::Ctx;
use sdc_tools::reply::records::{case_dir_of, Facts};
use sdc_tools::reply::review::{check_review, review_inputs, review_inputs_session};
use sdc_tools::reply::schemas::review_schema;
use sdc_tools::reply::read_json;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

/// What the review's inputs left for its call.
pub struct ReviewInputs {
    /// The document of the facts review, the only thing its model reads.
    pub doc: Option<PathBuf>,
    /// Its size, for the call log.
    pub bytes: Option<u64>,
    /// The reply schema the adapter is given.
    pub schema: PathBuf,
    /// The case directory the checks read the records from.
    pub case_dir: PathBuf,
}

const SESSION_OPS: &[&str] = &["dir.list", "file.read", "search"];

/// Writes the review's inputs into the call's `in/`, its schema into the record, and builds the
/// task. Inputs that cannot be made end the run with exit 2, recording nothing.
pub fn inputs(c: &Ctx, rec: &Path, cwd: &Path, inp: &mut Inputs) -> R<Built> {
    let story = c.names().1;
    let case_dir = env_set("SDC_CASE_DIR").map_or_else(|| case_dir_of(&c.work), PathBuf::from);
    let facts = c.facts();
    let source = Facts::open(&case_dir);
    let made = if facts { review_inputs(cwd, &source, &story) } else { review_inputs_session(cwd, &source, &story) };
    let (listed, check_err) = (rec.join("review-inputs.json"), rec.join("review-check.err"));
    let v = match made {
        Ok(v) => {
            let _ = fs::write(&listed, format!("{v}\n"));
            let _ = fs::write(&check_err, "");
            v
        }
        Err(e) => {
            let text = format!("sdc-reply review-inputs: {e}\n");
            let _ = fs::write(&listed, "");
            let _ = fs::write(&check_err, &text);
            let line = c.render("instructions/shared/node.review.no-inputs.md", &[("reason", chomp(&text))])?;
            Out::inherit().line(line.as_bytes());
            return Err(Exit(2));
        }
    };
    let schema = rec.join("reply-schema.json");
    let _ = fs::write(&schema, format!("{}\n", review_schema(&cwd.join("in"))));
    let ri = |doc: Option<PathBuf>, bytes: Option<u64>| ReviewInputs { doc, bytes, schema: schema.clone(), case_dir: case_dir.clone() };
    if facts {
        // One request, no tools: the program assembled in/review.md, the only thing the model
        // reads, and the prompt carries its text.
        let notes = c.render_part("facts_inputs", &[])?;
        inp.notes(&notes);
        let task = c.render_part("facts", &[("story", &story)])?;
        let mut b = Built::new("code_review", None, task);
        b.ops = &[];
        b.turns = 4;
        b.review = Some(ri(Some(cwd.join("in/review.md")), v["document_bytes"].as_u64()));
        return Ok(b);
    }
    let base = v.get("base").map_or_else(|| "null".to_string(), |b| b.as_str().map_or_else(|| b.to_string(), str::to_string));
    let notes = c.render_part("inputs", &[("story", &story), ("base", &base)])?;
    inp.notes(&notes);
    // The implementer's coverage document, when its tree carries one: one more input and sentence.
    let mut cov_line = String::new();
    if v["coverage"] == serde_json::json!(true) {
        let notes = c.render_part("coverage_inputs", &[])?;
        inp.notes(&notes);
        cov_line = format!("\n{}", c.render_part("coverage_note", &[])?);
    }
    let task = c.render_part("task", &[("story", &story), ("coverage_note", &cov_line)])?;
    let mut b = Built::new("code_review", None, task);
    b.ops = SESSION_OPS;
    b.review = Some(ri(None, None));
    Ok(b)
}

fn write_request(path: &Path, req: &Request) {
    let _ = fs::write(path, pretty(req));
}

/// The reply checked by the library: the exit (0 approved, 1 rejected, 2 invalid), what the check
/// prints on standard output and what it says on standard error.
fn check(c: &Ctx, cwd: &Path, case_dir: &Path, reply: &Path, principal: &str) -> R<(i32, String, String)> {
    let r = read_json(reply);
    Ok(match check_review(cwd, &Facts::open(case_dir), &r, principal) {
        Ok((approved, v)) => (i32::from(!approved), format!("{v}\n"), String::new()),
        Err(why) => {
            let what = if r.is_null() {
                let name = reply.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
                c.render("instructions/shared/node.review.unreadable.md", &[("name", &name)])?
            } else {
                why.join("; ")
            };
            let text = c.render("instructions/shared/node.review.invalid.md", &[("why", &what)])?;
            (2, String::new(), format!("{text}\n"))
        }
    })
}

fn non_empty(p: &Path) -> bool {
    fs::metadata(p).is_ok_and(|m| m.len() > 0)
}

/// The review call: the adapter, the check of its reply, and the asking again with the check's
/// reason (at most twice, each a model call). Exit 0 approved, 1 rejected (standard output is then
/// the `sdc.rejection/1` the implementer is sent back with), 2 an invalid reply or a failed call
/// (nothing is recorded), 75 the API ended.
#[allow(clippy::too_many_arguments)]
pub fn call(c: &Ctx, rec_root: &Path, rec: &Path, cwd: &Path, attempt: usize, built: &Built, req: &mut Request, feedback: bool) -> R<i32> {
    let Some(ri) = built.review.as_ref() else { return Ok(1) };
    let request = rec.join("request.json");
    req.json_schema = Some(ri.schema.display().to_string());
    req.review_attempt = Some(1);
    write_request(&request, req);
    let started = now_utc(false);
    let principal = format!("llm-review:{}", req.model);
    let (response, errors, prompt) = (rec.join("response.json"), rec.join("adapter-stderr.txt"), rec.join("prompt.md"));
    let (check_out, check_err) = (rec.join("review-check.out"), rec.join("review-check.err"));
    if c.adapter == "dry" {
        let acode = run_split(Command::new(&c.adapter_bin).arg(&request), &response, &errors);
        let code = c.run_stub(&mut Out::truncate(&rec.join("apply.log")));
        log_call(c, rec_root, rec, attempt, &started, req, built.out, acode, code, feedback, ri.bytes);
        return Ok(code);
    }
    // SDC_REVIEW_RETRIES, default 2; bash's arithmetic reads a word that is no number as 0.
    let retries: usize = env_set("SDC_REVIEW_RETRIES").map_or(2, |v| v.trim().parse().unwrap_or(0));
    let mut tries = 0;
    let (mut acode, mut code);
    loop {
        let _ = fs::remove_file(rec.join("structured-output.json"));
        let _ = fs::remove_file(cwd.join("out/review.json"));
        acode = run_split(Command::new(&c.adapter_bin).arg(&request), &response, &errors);
        if !non_empty(&response) {
            let _ = fs::write(&response, "{}\n");
        }
        if acode != 0 {
            let ended = fs::read_to_string(&response).ok().and_then(|t| Ov::parse(&t).ok()).and_then(|r| r.get("terminal_reason").cloned());
            let over = ["api_error", "recording_diverged"].iter().any(|e| ended == Some(Ov::Str((*e).into())));
            code = if over { 75 } else { 2 };
            let why = c.render("instructions/shared/node.review.call-failed.md", &[("code", &acode.to_string())])?;
            let _ = fs::write(&check_err, format!("{why}\n"));
            break;
        }
        let structured = rec.join("structured-output.json");
        let reply = if non_empty(&structured) { structured } else { cwd.join("out/review.json") };
        let (done, out, err) = check(c, cwd, &ri.case_dir, &reply, &principal)?;
        code = done;
        let _ = fs::write(&check_out, out);
        let _ = fs::write(&check_err, err);
        if code != 2 || tries >= retries {
            break;
        }
        tries += 1;
        let _ = fs::copy(&response, rec.join(format!("response.try{tries}.json")));
        if reply.is_file() {
            let _ = fs::copy(&reply, rec.join(format!("reply.try{tries}.json")));
        }
        let reason = fs::read_to_string(&check_err).unwrap_or_default();
        let retry = c.render("instructions/shared/retry-review.md", &[("reason", chomp(&reason))])?;
        if let Ok(mut f) = fs::OpenOptions::new().append(true).open(&prompt) {
            let _ = write!(f, "\n{retry}\n");
        }
        req.review_attempt = Some(tries + 1);
        write_request(&request, req);
    }
    let reason = fs::read_to_string(&check_err).unwrap_or_default();
    let line = match code {
        0 => {
            let notes = fs::read_to_string(&check_out).ok().and_then(|t| Ov::parse(&t).ok()).and_then(|d| d.get("notes").map(|n| n.items().len())).unwrap_or(0);
            Some(c.render("instructions/shared/node.review.approved.md", &[("principal", &principal), ("notes", &notes.to_string())])?)
        }
        1 => fs::read_to_string(&check_out).ok().and_then(|t| Ov::parse(&t).ok()).map(|d| d.compact()),
        75 => Some(c.render("instructions/shared/node.review.api.md", &[])?),
        _ => {
            let call = rec.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
            let vals = [("attempts", (tries + 1).to_string()), ("adapter", c.adapter.clone()), ("call", call), ("reason", chomp(&reason).to_string())];
            let vals: Vec<(&str, &str)> = vals.iter().map(|(k, v)| (*k, v.as_str())).collect();
            Some(c.render("instructions/shared/node.review.nothing-recorded.md", &vals)?)
        }
    };
    if let Some(l) = line {
        Out::inherit().line(l.as_bytes());
    }
    log_call(c, rec_root, rec, attempt, &started, req, built.out, acode, code, feedback, ri.bytes);
    Ok(code)
}

