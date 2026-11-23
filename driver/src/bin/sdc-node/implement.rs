//! `change.implement`: its inputs and task, the coverage document
//! its reply carries and the asking again (lines 792-834), the stub's coverage document (lines
//! 187-227), the call as one request (lines 903-931) and the check after the writer (lines
//! 962-969). The requirements, the schema and the checks are the library's (`sdc_tools::reply`);
//! no `sdc-reply` process is started.

use crate::call::{apply, log_call, pretty, Request};
use crate::nodes::{Built, Inputs};
use crate::ordjson::Ov;
use crate::util::{chomp, die, env_set, now_utc, run_split, run_to_file, Out, R};
use crate::Ctx;
use sdc_tools::reply::coverage::{check_coverage_text, coverage_stub, story_arg, story_requirements, work_arg};
use sdc_tools::reply::implement::{self, INLINE_CAP};
use sdc_tools::reply::records::case_dir_of;
use sdc_tools::reply::schemas::coverage_schema;
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

fn non_empty(p: &Path) -> bool {
    fs::metadata(p).is_ok_and(|m| m.len() > 0)
}

/// The story's requirements as `sdc-reply coverage` prints them, and the reply schema over their
/// ids as `sdc-reply coverage --schema` prints it.
fn requirements(c: &Ctx) -> Result<(Value, Value), String> {
    let story = story_arg(None)?;
    let reqs = story_requirements(&work_arg(Some(c.work.clone()), None)?, &story)?;
    let ids: Vec<String> = reqs.iter().filter_map(|x| x["id"].as_str().map(String::from)).collect();
    let list: Vec<Value> = reqs.iter().map(|x| json!({"id": x["id"], "text": x["text"]})).collect();
    Ok((Value::Array(list), coverage_schema(&ids)))
}

/// The inputs and the task: the session's, with the coverage document when the switch is on, or
/// none of either for one request (the program that makes it reads the case itself).
pub fn inputs(c: &Ctx, rec: &Path, cwd: &Path, inp: &mut Inputs) -> R<Built> {
    if c.request_mode {
        inp.seed("impl");
        let mut b = Built::new("impl", None, String::new());
        b.ops = &[];
        return Ok(b);
    }
    let story = c.names().1;
    for i in ["requirements.yaml", "context.yaml", "spec"] {
        inp.input(i);
    }
    inp.input(&format!("aep/story/{story}.md"));
    inp.input("impl");
    inp.seed("impl");
    let mut task = c.render_part("task", &[("story", &story)])?;
    let mut schema = None;
    if c.coverage {
        // The story's requirements and the reply schema, by program: the reply is checked against
        // the schema and asked again at most twice.
        let (err, file) = (rec.join("coverage-check.err"), rec.join("coverage-schema.json"));
        match requirements(c) {
            Ok((list, doc)) => {
                let _ = fs::write(cwd.join("in/story-requirements.json"), format!("{list}\n"));
                let _ = fs::write(&file, format!("{doc}\n"));
                let _ = fs::write(&err, "");
            }
            Err(e) => {
                let _ = fs::write(cwd.join("in/story-requirements.json"), "");
                let _ = fs::write(&err, format!("sdc-reply coverage: {e}\n"));
                let why = fs::read_to_string(&err).unwrap_or_default();
                return Err(die(format!("llm-node: {} could not run: no story requirements: {}", c.node, chomp(&why)), 126));
            }
        }
        let compact = fs::read_to_string(&file).ok().and_then(|t| Ov::parse(&t).ok()).map(|v| v.compact()).unwrap_or_default();
        inp.notes(&c.render_part("coverage_inputs", &[])?);
        let cov = c.render_part("coverage", &[("schema", &compact)])?;
        task = format!("{task}\n\n{cov}");
        schema = Some(file);
    }
    Ok(Built::new("impl", schema, task))
}

impl Ctx {
    /// The stub implementer's coverage document, unless the stub tree carries one (a fixture's own).
    /// False when it cannot be made.
    pub fn coverage_stub(&self) -> bool {
        let tree = self.work.join("impl");
        let doc = tree.join(".sdc/coverage.json");
        if !(self.coverage && tree.is_dir() && !doc.is_file()) {
            return true;
        }
        if fs::create_dir_all(tree.join(".sdc")).is_err() {
            return false;
        }
        let ids = story_arg(None).and_then(|s| {
            let reqs = story_requirements(&work_arg(Some(self.work.clone()), None)?, &s)?;
            Ok(reqs.iter().filter_map(|x| x["id"].as_str().map(String::from)).collect::<Vec<String>>())
        });
        match ids {
            Ok(ids) => fs::write(&doc, format!("{}\n", coverage_stub(&tree, &ids))).is_ok(),
            Err(e) => {
                let _ = fs::write(&doc, "");
                eprintln!("sdc-reply coverage-stub: {e}");
                false
            }
        }
    }
}

/// What the calls of a session left.
pub struct Calls {
    /// The last call's exit.
    pub acode: i32,
    /// The coverage document stayed invalid after the last asking.
    pub invalid: bool,
    /// How many times the model was asked again.
    pub tries: usize,
    /// The reply the coverage document was read from.
    pub reply: PathBuf,
}

/// The session's call. With a coverage document the reply (the adapter's structured output, else
/// out/coverage.json) is checked against its schema; an invalid one is asked again with the check's
/// reason (`SDC_COVERAGE_RETRIES`, default 2), each a model call with its own line in the call log,
/// the files under out/ kept.
#[allow(clippy::too_many_arguments)]
pub fn session_calls(c: &Ctx, rec_root: &Path, rec: &Path, cwd: &Path, attempt: usize, started: &str, req: &mut Request, built: &Built, feedback: bool) -> R<Calls> {
    let (request, response, structured) = (rec.join("request.json"), rec.join("response.json"), rec.join("structured-output.json"));
    // bash's arithmetic reads a word that is no number as 0.
    let retries: usize = env_set("SDC_COVERAGE_RETRIES").map_or(2, |v| v.trim().parse().unwrap_or(0));
    let mut tries = 0;
    loop {
        let _ = fs::remove_file(&structured);
        let acode = run_to_file(Command::new(&c.adapter_bin).arg(&request), &response);
        if !non_empty(&response) {
            let _ = fs::write(&response, "{}\n");
        }
        let done = |invalid: bool, reply: PathBuf| Ok(Calls { acode, invalid, tries, reply });
        if !(c.coverage && acode == 0 && c.adapter != "dry") {
            return done(false, PathBuf::new());
        }
        let reply = if non_empty(&structured) { structured.clone() } else { cwd.join("out/coverage.json") };
        let (code, _, err) = check_coverage_text(Some(c.work.clone()), None, None, &cwd.join("out/impl"), Some(reply.clone()), true)
            .unwrap_or_else(|e| (2, String::new(), format!("sdc-reply check-coverage: {e}\n")));
        let _ = fs::write(rec.join("coverage-check.err"), &err);
        if code == 0 {
            return done(false, reply);
        }
        if tries >= retries {
            return done(true, reply);
        }
        tries += 1;
        log_call(c, rec_root, rec, attempt, started, req, built.out, acode, 2, feedback, None);
        let _ = fs::copy(&response, rec.join(format!("response.try{tries}.json")));
        if reply.is_file() {
            let _ = fs::copy(&reply, rec.join(format!("coverage.try{tries}.json")));
        }
        let retry = c.render("instructions/shared/retry-coverage.md", &[("reason", chomp(&err))])?;
        if let Ok(mut f) = fs::OpenOptions::new().append(true).open(rec.join("prompt.md")) {
            let _ = write!(f, "\n{retry}\n");
        }
        req.coverage_attempt = Some(tries + 1);
        let _ = fs::write(&request, pretty(req));
    }
}

/// After the writer, under Loom no driver runs the binding's postcondition, so the same program
/// check runs here (the one copy of the rule). Exit 1 is this run's exit 8, which the binding
/// records as `implementation_change` `coverage_refused` (the writer is sent back, no conformance
/// run is due); the check's output is the run's output and so the next run's feedback.
pub fn refuse_uncovered(c: &Ctx) -> i32 {
    match check_coverage_text(Some(c.work.clone()), None, None, &c.work.join("impl"), None, false) {
        Ok((status, out, err)) => {
            let _ = std::io::stdout().write_all(out.as_bytes());
            let _ = std::io::stderr().write_all(err.as_bytes());
            match status {
                0 => 0,
                1 => 8,
                _ => 126,
            }
        }
        Err(e) => {
            eprintln!("sdc-reply check-coverage: {e}");
            126
        }
    }
}

/// The call as one request: the library assembles the input from the case, makes the requests
/// (one as a rule; at most two retries of an invalid reply, at most two continuations by rule),
/// writes the reply's files into out/impl/ and the summed response.json; apply copies out/impl/ to
/// impl/. Exit 2: the reply stayed invalid or incomplete, nothing written; 75 the API ended. Dry
/// assembles the prompt, then runs the stub.
#[allow(clippy::too_many_arguments)]
pub fn request_call(c: &Ctx, rec_root: &Path, rec: &Path, cwd: &Path, attempt: usize, built: &Built, req: &Request, feedback: bool, rework_limit: &str) -> R<i32> {
    let started = now_utc(false);
    let story = c.names().1;
    let case_dir = env_set("SDC_CASE_DIR").map_or_else(|| case_dir_of(&c.work), PathBuf::from);
    let args = |adapter: Option<PathBuf>, assemble_only: bool| implement::RunArgs {
        rec: rec.to_path_buf(),
        cwd: cwd.to_path_buf(),
        case_dir: case_dir.clone(),
        story: story.clone(),
        adapter,
        rework_limit: c.rejection.as_ref().and_then(|_| rework_limit.trim().parse().ok()),
        assemble_only,
        cap: INLINE_CAP,
    };
    let (out, response) = (rec.join("implement-run.out"), rec.join("response.json"));
    if c.adapter == "dry" {
        let (_, line) = implement::run(&args(None, true));
        let _ = fs::write(&out, format!("{line}\n"));
        let acode = run_split(Command::new(&c.adapter_bin).arg(rec.join("request.json")), &response, &rec.join("adapter-stderr.txt"));
        let code = c.run_stub(&mut Out::truncate(&rec.join("apply.log")));
        if code == 0 {
            c.post("", &mut Out::append(&rec.join("apply.log")))?;
        }
        log_call(c, rec_root, rec, attempt, &started, req, built.out, acode, code, feedback, None);
        return Ok(code);
    }
    let (acode, line) = implement::run(&args(Some(c.adapter_bin.clone()), false));
    let acode = i32::from(acode);
    let _ = fs::write(&out, format!("{line}\n"));
    if !non_empty(&response) {
        let _ = fs::write(&response, "{}\n");
    }
    let mut code = acode;
    match acode {
        0 => {
            Out::inherit().line(line.as_bytes());
            code = apply(c, built.out, rec, cwd, Path::new(""))?;
            if code != 0 {
                let head = c.render("instructions/shared/node.not-applied.md", &[("node", &c.node), ("code", &code.to_string())])?;
                Out::inherit().line(head.as_bytes());
                let log = fs::read(rec.join("apply.log")).unwrap_or_default();
                let _ = std::io::stdout().write_all(crate::call::tail_lines(&log, 20));
            }
            if code == 0 {
                let findings = cwd.join("out/findings.yaml");
                let raised = if findings.is_file() { findings.display().to_string() } else { String::new() };
                c.post(&raised, &mut Out::append(&rec.join("apply.log")))?;
            }
        }
        75 => {
            let msg = c.render("instructions/shared/node.implement.api-ended.md", &[("node", &c.node), ("reason", chomp(&line))])?;
            Out::inherit().line(msg.as_bytes());
        }
        _ => {
            let call = rec.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
            let msg = c.render(
                "instructions/shared/node.implement.nothing-recorded.md",
                &[("node", &c.node), ("call", &call), ("adapter", &c.adapter), ("reason", chomp(&line))],
            )?;
            Out::inherit().line(msg.as_bytes());
        }
    }
    log_call(c, rec_root, rec, attempt, &started, req, built.out, acode, code, feedback, None);
    Ok(code)
}
