//! The answers file of a runbook (`<bindings>/tools/answers.yaml`, format
//! `runbook.loom-run-answers/1`) and the programs it names.
//!
//! The file says, per action, what the runner does before it runs the command Loom printed for a
//! decision: which program prints the requests (one JSON object per line, a question or a finding
//! for the decider), which program writes one answer into the workspace, and which program takes
//! the step once every request is answered. This module knows the format of the file and of a
//! request line, nothing about what a request is about: the names of the work files and of the
//! actions are the file's and its programs'.
//!
//! ```yaml
//! format: runbook.loom-run-answers/1
//! answers:
//! - action: <action id>
//!   on: ApproveAction | Handoff        # the kind of the decision Loom printed
//!   requests: [<program>, <arg>...]    # prints one request per line
//!   apply: [<program>, <arg>...]       # writes one answer; {id} {answer} {principal} {case} {type}
//!   then: [<program>, <arg>...]        # optional, once after the last answer; {principal} {case}
//! ```
//!
//! A request line is a JSON object with `type` (`question` or `finding`), `id`, and a member of the
//! same name as `type` holding the object the decider is asked about (`default` and, for a finding,
//! `options` in it); every other member is passed on to the decider's request unchanged. A word of
//! a program that holds a `/` names a file of the bindings copy; any other word is looked up on the
//! `PATH` of the runner's child environment. A program starts in the workspace, with `PATH` and
//! `LOOM_CASE` and nothing else, as Loom starts a bound program.

use serde_json::{json, Value};
use serde_yaml::Value as Yaml;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The file in the bindings copy.
pub const FILE: &str = "tools/answers.yaml";
/// Its format id.
pub const FORMAT: &str = "runbook.loom-run-answers/1";
/// The kinds of decision a declaration may be for.
const KINDS: [&str; 2] = ["ApproveAction", "Handoff"];
/// The names a program word may hold between braces.
const PLACEHOLDERS: [&str; 5] = ["principal", "case", "id", "answer", "type"];
/// The most answers one request gets in a run: the same request coming back means the program that
/// reads the answer refused it (the driver stops a finding at the third).
pub const MAX_ANSWERS_PER_REQUEST: u64 = 3;
const PROGRAM_SECONDS: u64 = 300;
const OUTPUT_CAP: u64 = 1 << 20;

/// What the runner does for one action.
#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
    pub action: String,
    pub on: String,
    pub requests: Vec<String>,
    pub apply: Vec<String>,
    pub then: Option<Vec<String>>,
}

/// The declarations of a bindings copy; empty when it has no file.
#[derive(Clone, Debug, Default)]
pub struct Answers {
    declarations: Vec<Declaration>,
}

fn words(value: Option<&Yaml>, what: &str, allowed: &[&str]) -> Result<Vec<String>, String> {
    let list = value.and_then(Yaml::as_sequence).ok_or_else(|| format!("{what}: a list of words"))?;
    let mut out = Vec::new();
    for item in list {
        let word = item.as_str().filter(|w| !w.is_empty()).ok_or_else(|| format!("{what}: every word is a non-empty text"))?;
        let mut rest = word;
        while let Some(open) = rest.find('{') {
            let close = rest[open..].find('}').ok_or_else(|| format!("{what}: {word:?} opens a brace and never closes it"))? + open;
            let name = &rest[open + 1..close];
            if !allowed.contains(&name) {
                return Err(format!("{what}: {{{name}}} is not a name here (known: {})", allowed.join(", ")));
            }
            rest = &rest[close + 1..];
        }
        out.push(word.to_owned());
    }
    if out.is_empty() {
        return Err(format!("{what}: names no program"));
    }
    Ok(out)
}

impl Answers {
    /// Reads `FILE` of the bindings directory `dir`. A missing file is no declaration; a file that
    /// does not parse, names another format, has a key or a kind this format does not know, or
    /// declares an action twice for one kind is refused.
    pub fn load(dir: &Path) -> Result<Answers, String> {
        let file = dir.join(FILE);
        let text = match std::fs::read_to_string(&file) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Answers::default()),
            Err(e) => return Err(format!("{FILE}: {}", e.kind())),
        };
        let doc: Yaml = serde_yaml::from_str(&text).map_err(|e| format!("{FILE}: {e}"))?;
        let top = doc.as_mapping().ok_or_else(|| format!("{FILE}: a mapping"))?;
        for key in top.keys() {
            if !matches!(key.as_str(), Some("format" | "answers")) {
                return Err(format!("{FILE}: {key:?} is not a key of {FORMAT} (format, answers)"));
            }
        }
        if doc.get("format").and_then(Yaml::as_str) != Some(FORMAT) {
            return Err(format!("{FILE}: format is not {FORMAT}"));
        }
        let list = doc.get("answers").and_then(Yaml::as_sequence).ok_or_else(|| format!("{FILE}: answers: a list"))?;
        let mut declarations: Vec<Declaration> = Vec::new();
        for (n, entry) in list.iter().enumerate() {
            let at = format!("{FILE}: answers[{n}]");
            let map = entry.as_mapping().ok_or_else(|| format!("{at}: a mapping"))?;
            for key in map.keys() {
                if !matches!(key.as_str(), Some("action" | "on" | "requests" | "apply" | "then")) {
                    return Err(format!("{at}: {key:?} is not a key (action, on, requests, apply, then)"));
                }
            }
            let action = entry.get("action").and_then(Yaml::as_str).filter(|a| !a.is_empty()).ok_or_else(|| format!("{at}: action: a non-empty text"))?.to_owned();
            let on = entry.get("on").and_then(Yaml::as_str).filter(|k| KINDS.contains(k)).ok_or_else(|| format!("{at}: on: one of {}", KINDS.join(", ")))?.to_owned();
            if declarations.iter().any(|d| d.action == action && d.on == on) {
                return Err(format!("{at}: {action} is declared twice for {on}"));
            }
            declarations.push(Declaration {
                requests: words(entry.get("requests"), &format!("{at}: requests"), &["case"])?,
                apply: words(entry.get("apply"), &format!("{at}: apply"), &PLACEHOLDERS)?,
                then: match entry.get("then") {
                    None => None,
                    Some(_) => Some(words(entry.get("then"), &format!("{at}: then"), &["principal", "case"])?),
                },
                action,
                on,
            });
        }
        Ok(Answers { declarations })
    }

    /// The declaration for a decision of `kind` on `action`.
    pub fn find(&self, kind: &str, action: &str) -> Option<&Declaration> {
        self.declarations.iter().find(|d| d.on == kind && d.action == action)
    }

    /// Whether any declaration names `action`.
    pub fn declares(&self, action: &str) -> bool {
        self.declarations.iter().any(|d| d.action == action)
    }

    /// Whether the decider's principal may take the person step `entry` of the bindings copy: the
    /// file declares it, or its outcome is a recorded result and not the person's own text (see
    /// [`is_answer_step`]).
    pub fn decider_takes(&self, name: &str, entry: &Yaml) -> bool {
        self.declares(name) || !is_answer_step(entry)
    }
}

/// A person step that lists no `from: effect` result: whatever the person does there is text of
/// their own (an answer), not one of the results the bindings name, so an approval says nothing
/// about it. Only the file of the runbook can say what the decider answers there.
pub fn is_answer_step(entry: &Yaml) -> bool {
    let person = entry.get("executor").and_then(|e| e.get("kind")).and_then(Yaml::as_str) == Some("person");
    let results = entry.get("evidence").and_then(Yaml::as_sequence).is_some_and(|items| items.iter().any(|i| i.get("from").and_then(Yaml::as_str) == Some("effect")));
    person && !results
}

/// Whether `action` is such a step in some `*.yaml` of the bindings directory `dir`.
pub fn is_answer_step_in(dir: &Path, action: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else { return false };
    let mut files: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "yaml") && p.is_file()).collect();
    files.sort();
    for file in files {
        let Some(doc) = crate::loom_run::record_write::read_regular_text(&file).ok().and_then(|t| serde_yaml::from_str::<Yaml>(&t).ok()) else { continue };
        let Some(actions) = doc.get("actions").and_then(Yaml::as_sequence) else { continue };
        if actions.iter().any(|a| a.get("action").and_then(Yaml::as_str) == Some(action) && is_answer_step(a)) {
            return true;
        }
    }
    false
}

/// One request a program printed.
#[derive(Clone, Debug)]
pub struct Request {
    pub id: String,
    /// `question` or `finding`.
    pub kind: String,
    /// The whole line.
    pub body: Value,
}

/// Parses the lines a requests program printed. A line that is not an object with a `type`, an
/// `id` and the member of that type is an error naming the line.
pub fn parse_requests(stdout: &str) -> Result<Vec<Request>, String> {
    let mut out = Vec::new();
    for (n, line) in stdout.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        let body: Value = serde_json::from_str(line).map_err(|e| format!("request line {}: not JSON: {e}", n + 1))?;
        let kind = body.get("type").and_then(Value::as_str).filter(|k| matches!(*k, "question" | "finding")).ok_or_else(|| format!("request line {}: type is question or finding", n + 1))?;
        let id = body.get("id").and_then(Value::as_str).filter(|i| !i.is_empty()).ok_or_else(|| format!("request line {}: id: a non-empty text", n + 1))?;
        if !body.get(kind).is_some_and(Value::is_object) {
            return Err(format!("request line {}: member {kind} is not an object", n + 1));
        }
        out.push(Request { id: id.to_owned(), kind: kind.to_owned(), body: body.clone() });
    }
    Ok(out)
}

/// What the decider is given for one request.
#[derive(Debug)]
pub struct AskRequest<'a> {
    /// The decision Loom printed, unchanged.
    pub decision: &'a Value,
    /// The request line of the program.
    pub request: &'a Request,
    pub run_dir: &'a Path,
    pub workspace: &'a Path,
    pub budget_micro_usd: u64,
    pub child_env: &'a [(String, String)],
    pub login_link: Option<&'a Path>,
}

/// The decider's answer to one request: the lines it makes in `decisions.jsonl` (an invalid reply
/// and a suppressed escalation make their own), what the calls cost and how many were model calls,
/// and the answer or the reason the run stops.
#[derive(Debug)]
pub struct Asked {
    pub lines: Vec<Value>,
    pub cost_micro_usd: u64,
    pub model_calls: u64,
    pub result: Result<Given, String>,
}

/// An answer, and the principal it was given as (`None`: the one in Loom's printed command).
#[derive(Debug)]
pub struct Given {
    pub answer: String,
    pub principal: Option<String>,
}

impl Asked {
    /// A decider that answers no requests.
    pub fn cannot(reason: impl Into<String>) -> Asked {
        Asked { lines: Vec::new(), cost_micro_usd: 0, model_calls: 0, result: Err(reason.into()) }
    }
}

/// The text of the reasons file of an answered approval: one line per answer.
pub fn reasons_text(given: &[(String, String)]) -> String {
    given.iter().map(|(id, answer)| format!("{id}: {}", answer.replace('\n', " "))).collect::<Vec<_>>().join("\n")
}

/// The place a declared program runs in.
pub struct Where<'a> {
    pub bindings: &'a Path,
    pub workspace: &'a Path,
    pub path: &'a str,
    pub case: &'a str,
}

/// What a declared program printed.
pub struct Printed {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

fn substitute(word: &str, values: &[(&str, &str)]) -> String {
    let mut out = String::new();
    let mut rest = word;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}').map(|c| c + open) else { break };
        out.push_str(&rest[..open]);
        let name = &rest[open + 1..close];
        match values.iter().find(|(k, _)| *k == name) {
            Some((_, v)) => out.push_str(v),
            None => out.push_str(&rest[open..=close]),
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}

impl Where<'_> {
    /// Starts `argv` (placeholders filled from `values`) with `PATH` and `LOOM_CASE` alone, in the
    /// workspace, standard input empty, in its own process group, ended after five minutes.
    pub fn run(&self, argv: &[String], values: &[(&str, &str)]) -> Result<Printed, String> {
        let argv: Vec<String> = argv.iter().map(|w| substitute(w, values)).collect();
        let program = if argv[0].contains('/') { self.bindings.join(&argv[0]).display().to_string() } else { argv[0].clone() };
        let name = argv[0].clone();
        let mut command = Command::new(&program);
        command.args(&argv[1..]).env_clear().env("PATH", self.path).env("LOOM_CASE", self.case).current_dir(self.workspace);
        command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).process_group(0);
        let mut child = command.spawn().map_err(|e| format!("could not start {name}: {}", e.kind()))?;
        let mut out = child.stdout.take().ok_or("no stdout")?;
        let mut err = child.stderr.take().ok_or("no stderr")?;
        let out_reader = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = (&mut out).take(OUTPUT_CAP).read_to_end(&mut bytes);
            bytes
        });
        let err_reader = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = (&mut err).take(OUTPUT_CAP).read_to_end(&mut bytes);
            bytes
        });
        let deadline = Instant::now() + Duration::from_secs(PROGRAM_SECONDS);
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let stdout = out_reader.join().unwrap_or_default();
                    let stderr = err_reader.join().unwrap_or_default();
                    return Ok(Printed { code: status.code(), stdout: String::from_utf8_lossy(&stdout).into_owned(), stderr: String::from_utf8_lossy(&stderr).into_owned() });
                }
                Ok(None) => {}
                Err(e) => return Err(format!("could not wait for {name}: {}", e.kind())),
            }
            if Instant::now() >= deadline {
                let group = format!("-{}", child.id());
                let killed = Command::new("kill").args(["-KILL", "--", group.as_str()]).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
                if !killed {
                    let _ = child.kill();
                }
                let _ = child.wait();
                return Err(format!("{name} ran over {PROGRAM_SECONDS} s and was ended"));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// The last lines of a program's standard error, for a reason.
pub fn tail(text: &str) -> String {
    let all: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    all[all.len().saturating_sub(3)..].join(" | ")
}

/// The decisions.jsonl line of a request answered or escalated: the members the removed driver's lines
/// had, so `sdc-run-metrics` counts them by class.
pub fn decision_line(decision: &Value, request: &Request, revision: &str, outcome: &str, reply: &Value, principal: &str, extra: Value) -> Value {
    let class = if request.kind == "question" { "scope_changing_answer" } else { "finding" };
    let mut line = json!({
        "case": decision.get("case").cloned().unwrap_or(Value::Null),
        "type": request.kind,
        "class": class,
        "action": decision.get("action").cloned().unwrap_or(Value::Null),
        "question": if request.kind == "question" { json!(request.id) } else { Value::Null },
        "finding": if request.kind == "finding" { json!(request.id) } else { Value::Null },
        "capability": request.body.get("capability").cloned().or_else(|| decision.get("capability").cloned()).unwrap_or(Value::Null),
        "outcome": Value::Null,
        "revision": revision,
        "decision": outcome,
        "answer": reply.get("answer").cloned().unwrap_or(Value::Null),
        "principal": principal,
        "reasons": reply.get("reasons").cloned().unwrap_or(Value::Null),
        "cites": reply.get("cites").cloned().unwrap_or(Value::Null),
        "model": reply.get("model").cloned().unwrap_or(Value::Null),
        "effort": reply.get("effort").cloned().unwrap_or(Value::Null),
        "adapter": reply.get("adapter").cloned().unwrap_or(Value::Null),
        "tokens": reply.get("usage").cloned().unwrap_or(Value::Null),
        "cost_usd": reply.get("cost_usd").cloned().unwrap_or(Value::Null),
        "duration_ms": reply.get("duration_ms").cloned().unwrap_or(Value::Null),
        "valid": reply.get("valid").cloned().unwrap_or(json!(true)),
        "invalid_reason": reply.get("invalid_reason").cloned().unwrap_or(Value::Null),
        "dry": reply.get("dry").cloned().unwrap_or(json!(false)),
        "record": reply.get("record").cloned().unwrap_or(Value::Null),
    });
    if let (Some(map), Some(more)) = (line.as_object_mut(), extra.as_object()) {
        for (k, v) in more {
            map.insert(k.clone(), v.clone());
        }
    }
    line
}

#[cfg(test)]
mod tests;
