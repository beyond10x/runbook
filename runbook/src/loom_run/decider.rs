//! The real deciders of `runbook loom-run`: the choice from the run settings and the decider that
//! starts `bindings/tools/decider.sh` of the bindings copy for one decision.
//!
//! The table of what the driver decided, what Loom can ask today and what this module does with
//! each is in `docs/loom-run-decisions.md`. Short form: an entry of `decisions[]` of kind
//! `ApproveAction` or `Approve` is an approval and one of kind `Close` is asked as a close (the
//! outcome Loom printed); everything else (a question, a finding, `Handoff`, an unknown kind) ends
//! in `CannotDecide` naming why.

use crate::loom_run::decide::{DecisionRequest, Decider, RefusingDecider, ScriptedDecider, Verdict};
use serde_json::{json, Map, Value};
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The only autonomy level at which the decider answers; every other level (and none) is reserved
/// for a person. The driver (`scripts/run-trial.sh`, `SDC_AUTONOMY`) hands every decision to the
/// decider at A5 and to a person at A0; A1 to A4 have no per-class routing there ("not built") and
/// a person holds some decisions of each, so the runner stops for a person.
pub const AUTONOMY_DECIDES: &str = "A5";

const AUTONOMY_LEVELS: [&str; 6] = ["A0", "A1", "A2", "A3", "A4", "A5"];
const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];
const MODEL_KEYS: [&str; 14] = [
    "without_claims",
    "kind",
    "program",
    "model",
    "adapter",
    "effort",
    "max_turns",
    "max_budget_usd",
    "max_calls",
    "max_decider_calls",
    "escalate",
    "retries",
    "timeout_seconds",
    "reply_bin",
];
const RULE_KEYS: [&str; 4] = ["kind", "program", "verdict", "timeout_seconds"];
/// The kinds of `decisions[]` entries that are approvals; `Close` is asked as a close.
const APPROVAL_KINDS: [&str; 3] = ["ApproveAction", "Approve", "Close"];
/// Driver default of `SDC_MAX_DECIDER_CALLS`.
const DEFAULT_MAX_CALLS: u64 = 20;
/// Driver default of `SDC_DECIDER_RETRIES`.
const DEFAULT_RETRIES: u64 = 2;
/// Driver default of `SDC_DECIDER_MAX_BUDGET_USD` (0.25), micro USD.
const DEFAULT_MAX_CALL_MICRO_USD: u64 = 250_000;
const DEFAULT_TIMEOUT_SECONDS: u64 = 900;
const OUTPUT_CAP: u64 = 1 << 20;

/// Chooses the decider from the run settings. No `decider` key: the refusing default.
/// `"scripted"`: R1's test double. `"model"`: one `decider.sh model` call per decision. `"rule"`:
/// `decider.sh approve` or `decider.sh reject` (the driver's `SDC_DECIDER=approve|reject`; the
/// driver's `SDC_RULE_APPROVALS` is not mapped, see the docs). An unknown key or kind is refused
/// by name.
pub fn from_settings(settings: &Value) -> Result<Box<dyn Decider>, String> {
    match settings.get("decider") {
        None => Ok(Box::new(RefusingDecider)),
        Some(decider) => match decider.get("kind").and_then(Value::as_str) {
            Some("scripted") => ScriptedDecider::from_value(decider).map(|d| Box::new(d) as Box<dyn Decider>),
            Some("model") | Some("rule") => ProgramDecider::parse(settings, decider).map(|d| Box::new(d) as Box<dyn Decider>),
            Some(other) => Err(format!("decider.kind {other}: not known (model, rule, scripted)")),
            None => Err("decider.kind: missing".to_owned()),
        },
    }
}

/// The principal the person steps of the bindings copy are given, when the settings let a decider
/// take them: `autonomy` is `A5` and `decider.kind` is `model` (`decider:model`) or `rule`
/// (`decider:approve` or `decider:reject`, as the driver names its rule deciders). `None`: the
/// steps keep the principal the bindings name (a person). A decider kind that is not one of these
/// (a scripted double, no decider) leaves the steps alone.
pub fn person_step_principal(settings: &Value) -> Result<Option<String>, String> {
    let level = autonomy(settings)?;
    let decider = match settings.get("decider") {
        Some(decider) => decider,
        None => return Ok(None),
    };
    let principal = match decider.get("kind").and_then(Value::as_str) {
        Some("model") => "decider:model".to_owned(),
        Some("rule") => format!("decider:{}", rule_verdict(decider)?),
        _ => return Ok(None),
    };
    Ok((level.as_deref() == Some(AUTONOMY_DECIDES)).then_some(principal))
}

fn autonomy(settings: &Value) -> Result<Option<String>, String> {
    match settings.get("autonomy") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(level)) if AUTONOMY_LEVELS.contains(&level.as_str()) => Ok(Some(level.clone())),
        Some(_) => Err("autonomy: one of A0, A1, A2, A3, A4, A5".to_owned()),
    }
}

fn rule_verdict(decider: &Value) -> Result<&str, String> {
    match decider.get("verdict").and_then(Value::as_str) {
        Some(v @ ("approve" | "reject")) => Ok(v),
        _ => Err("decider.verdict: approve or reject (kind rule)".to_owned()),
    }
}

enum Mode {
    Model {
        model: String,
        adapter: Option<String>,
        effort: Option<String>,
        max_turns: Option<u64>,
        final_escalate: bool,
        retries: u64,
        reply_bin: PathBuf,
        without_claims: bool,
    },
    Rule {
        verdict: String,
    },
}

/// A decider that starts the bindings copy's `decider.sh` once per decision.
pub struct ProgramDecider {
    mode: Mode,
    program: PathBuf,
    max_calls: u64,
    max_call_micro_usd: u64,
    timeout: Duration,
    autonomy: Option<String>,
    login_file: Option<String>,
    principal: String,
}

fn number_key(decider: &Value, key: &str, min: u64) -> Result<Option<u64>, String> {
    match decider.get(key) {
        None => Ok(None),
        Some(v) => match v.as_u64() {
            Some(n) if n >= min => Ok(Some(n)),
            _ => Err(format!("decider.{key}: a whole number of at least {min}")),
        },
    }
}

fn text_key(decider: &Value, key: &str) -> Result<Option<String>, String> {
    match decider.get(key) {
        None => Ok(None),
        Some(Value::String(s)) if !s.is_empty() && !s.chars().any(char::is_whitespace) => Ok(Some(s.clone())),
        Some(_) => Err(format!("decider.{key}: a non-empty text without white space")),
    }
}

fn path_key(decider: &Value, key: &str) -> Result<Option<String>, String> {
    match decider.get(key) {
        None => Ok(None),
        Some(Value::String(s)) if !s.is_empty() => Ok(Some(s.clone())),
        Some(_) => Err(format!("decider.{key}: a path")),
    }
}

impl ProgramDecider {
    /// Reads `settings.decider` (kind `model` or `rule`) and the top-level `autonomy` and
    /// `login_file`.
    pub fn parse(settings: &Value, decider: &Value) -> Result<ProgramDecider, String> {
        let object = decider.as_object().ok_or("decider: an object")?;
        let kind = decider.get("kind").and_then(Value::as_str).unwrap_or("");
        let allowed: &[&str] = if kind == "rule" { &RULE_KEYS } else { &MODEL_KEYS };
        for key in object.keys() {
            if !allowed.contains(&key.as_str()) {
                return Err(format!("decider.{key}: not a key of a decider of kind {kind}"));
            }
        }
        let program = path_key(decider, "program")?.ok_or("decider.program: missing (the decider.sh of the bindings copy)")?;
        let timeout = Duration::from_secs(number_key(decider, "timeout_seconds", 1)?.unwrap_or(DEFAULT_TIMEOUT_SECONDS));
        let level = autonomy(settings)?;
        let login_file = match settings.get(crate::loom_run::settings::LOGIN_KEY) {
            None | Some(Value::Null) => None,
            Some(Value::String(path)) if !path.is_empty() => Some(path.clone()),
            Some(_) => return Err(format!("{}: a path", crate::loom_run::settings::LOGIN_KEY)),
        };
        let (mode, max_calls, max_call_micro_usd) = if kind == "rule" {
            (Mode::Rule { verdict: rule_verdict(decider)?.to_owned() }, 0, 0)
        } else {
            if decider.get("max_calls").is_some() && decider.get("max_decider_calls").is_some() {
                return Err("decider: max_calls and max_decider_calls name the same limit; give one".to_owned());
            }
            let max_calls = number_key(decider, "max_calls", 1)?.or(number_key(decider, "max_decider_calls", 1)?).unwrap_or(DEFAULT_MAX_CALLS);
            let escalate = match decider.get("escalate") {
                None => false,
                Some(Value::String(s)) if s == "stop" => false,
                Some(Value::String(s)) if s == "final" => true,
                Some(_) => return Err("decider.escalate: final or stop".to_owned()),
            };
            let without_claims = match decider.get("without_claims") {
                None => false,
                Some(Value::Bool(b)) => *b,
                Some(_) => return Err("decider.without_claims: true or false".to_owned()),
            };
            let effort = text_key(decider, "effort")?;
            if let Some(effort) = &effort {
                if !EFFORTS.contains(&effort.as_str()) {
                    return Err(format!("decider.effort: {effort} is not a reasoning effort ({})", EFFORTS.join(", ")));
                }
            }
            let max_call = match decider.get("max_budget_usd") {
                None => DEFAULT_MAX_CALL_MICRO_USD,
                Some(v) => match v.as_f64() {
                    Some(usd) if usd.is_finite() && usd > 0.0 => (usd * 1_000_000.0).round() as u64,
                    _ => return Err("decider.max_budget_usd: a number above 0".to_owned()),
                },
            };
            let mode = Mode::Model {
                model: text_key(decider, "model")?.ok_or("decider.model: missing")?,
                adapter: text_key(decider, "adapter")?,
                effort,
                max_turns: number_key(decider, "max_turns", 1)?,
                final_escalate: escalate,
                retries: number_key(decider, "retries", 0)?.unwrap_or(DEFAULT_RETRIES),
                reply_bin: PathBuf::from(path_key(decider, "reply_bin")?.ok_or("decider.reply_bin: missing (the sdc-reply binary decider.sh reads the reply schema from)")?),
                without_claims,
            };
            (mode, max_calls, max_call)
        };
        let principal = person_step_principal(&json!({"autonomy": "A5", "decider": decider}))?.ok_or("decider: no principal")?;
        Ok(ProgramDecider { mode, program: PathBuf::from(program), max_calls, max_call_micro_usd, timeout, autonomy: level, login_file, principal })
    }

    fn cannot(reason: impl Into<String>) -> Verdict {
        Verdict::CannotDecide { reason: reason.into() }
    }

    fn redact(&self, text: &str) -> String {
        match &self.login_file {
            Some(path) => text.replace(path.as_str(), "<login file>"),
            None => text.to_owned(),
        }
    }
}

/// The approval a decision entry asks for, read with an explicit path for every absent member.
struct Approval {
    case: String,
    action: String,
    capability: Option<String>,
    subject: String,
    revision: String,
    path: Option<String>,
    /// The outcome of a `Close` entry; the decision is then a close, not an approval.
    close_outcome: Option<String>,
}

fn text_member<'a>(decision: &'a Value, name: &str) -> Option<&'a str> {
    decision.get(name).and_then(Value::as_str).filter(|s| !s.is_empty())
}

fn plain_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains('/') && !name.contains('\0')
}

fn read_approval(request: &DecisionRequest<'_>) -> Result<Approval, String> {
    let decision = request.decision;
    let kind = text_member(decision, "kind").ok_or("the decision has no kind")?;
    if !APPROVAL_KINDS.contains(&kind) {
        return Err(match kind {
            "Handoff" => "kind Handoff: a writer's handoff text names work for a person; the printed `tree run` changes nothing until that work is done, and the decider has no reply for it".to_owned(),
            other => format!("kind {other} not known"),
        });
    }
    let case = text_member(decision, "case").ok_or("the decision names no case")?.to_owned();
    if !plain_name(&case) {
        return Err("the case name is not a plain name".to_owned());
    }
    if kind == "Close" {
        let outcome = text_member(decision, "outcome").ok_or("the Close decision names no outcome (member `outcome` absent)")?.to_owned();
        return Ok(Approval { case, action: "close".to_owned(), capability: None, subject: String::new(), revision: String::new(), path: None, close_outcome: Some(outcome) });
    }
    let action = text_member(decision, "action").ok_or("the decision names no action (member `action` absent)")?.to_owned();
    if action == "scope.answer" {
        return Err("scope.answer is a question in the driver (an answer per question, written into questions.yaml); Loom's approval of it carries no answer and has no way to take one".to_owned());
    }
    let subject = text_member(decision, "subject").ok_or("the decision names no subject (member `subject` absent): decider.sh cannot say what is decided")?.to_owned();
    let revision = text_member(decision, "revision").ok_or("the decision names no revision (member `revision` absent): decider.sh cannot say which revision is decided")?.to_owned();
    let path = match text_member(decision, "path") {
        None => None,
        Some(path) => {
            let relative = Path::new(path)
                .strip_prefix(request.workspace)
                .map_err(|_| "the decision's path is not inside the workspace".to_owned())?;
            if relative.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
                return Err("the decision's path has a segment that is not a plain name".to_owned());
            }
            Some(relative.to_string_lossy().into_owned())
        }
    };
    Ok(Approval { case, action, capability: text_member(decision, "capability").map(str::to_owned), subject, revision, path, close_outcome: None })
}

fn hex(text: &str) -> String {
    text.bytes().map(|b| format!("{b:02x}")).collect()
}

/// The artifacts of a case with their revisions, from Loom's case file (`case_file` of
/// `tree status`: `<store>/case-<hex of the case id>.json`, store named by `--store` of the
/// decision's `command`). Empty when the file cannot be read.
fn case_artifacts(decision: &Value, case: &str) -> Vec<(String, String)> {
    let words: Vec<&str> = decision.get("command").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let store = match words.iter().position(|w| *w == "--store").and_then(|i| words.get(i + 1)) {
        Some(store) => PathBuf::from(store),
        None => return Vec::new(),
    };
    let file = store.join(format!("case-{}.json", hex(case)));
    let document: Value = match std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()) {
        Some(document) => document,
        None => return Vec::new(),
    };
    document
        .get("artifacts")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| Some((item.get("artifact")?.as_str()?.to_owned(), item.get("revision")?.as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default()
}

/// The directory `decider.sh` takes as `case_dir`: `case.json` (`id` from the decision's `case`,
/// `artifacts` from Loom's case file and the decision's own subject) and `work` (a link to the
/// workspace). Under `<run dir>/decider-cases/<case>/`.
fn make_case_dir(request: &DecisionRequest<'_>, approval: &Approval) -> Result<PathBuf, String> {
    let dir = request.run_dir.join("decider-cases").join(&approval.case);
    std::fs::create_dir_all(&dir).map_err(|e| format!("decider case directory: {e}"))?;
    let mut artifacts = Map::new();
    for (name, revision) in case_artifacts(request.decision, &approval.case) {
        artifacts.insert(name, json!({ "revision": revision }));
    }
    if !approval.subject.is_empty() {
        artifacts.insert(approval.subject.clone(), json!({ "revision": approval.revision }));
    }
    let case = json!({"id": approval.case, "artifacts": artifacts});
    std::fs::write(dir.join("case.json"), format!("{case}\n")).map_err(|e| format!("decider case file: {e}"))?;
    let work = dir.join("work");
    if std::fs::symlink_metadata(&work).is_ok() {
        std::fs::remove_file(&work).map_err(|e| format!("decider case directory: {e}"))?;
    }
    std::os::unix::fs::symlink(request.workspace, &work).map_err(|e| format!("decider case directory: {e}"))?;
    Ok(dir)
}

fn record_dirs(base: &Path) -> u64 {
    std::fs::read_dir(base)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false) && e.file_name() != "rule")
                .count() as u64
        })
        .unwrap_or(0)
}

fn safe_part(text: &str) -> String {
    text.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' }).collect()
}

fn usd_text(micro: u64) -> String {
    format!("{}.{:06}", micro / 1_000_000, micro % 1_000_000)
}

struct Ran {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

/// Starts `program args` with exactly `env`, in its own process group, and ends the group when
/// `timeout` passes.
fn run_program(program: &Path, args: &[&str], env: &[(String, String)], cwd: &Path, timeout: Duration) -> Result<Ran, String> {
    let mut command = Command::new(program);
    command.args(args).env_clear().envs(env.iter().map(|(k, v)| (k, v))).current_dir(cwd);
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).process_group(0);
    let mut child = command.spawn().map_err(|e| format!("could not start the decider program: {}", e.kind()))?;
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
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = out_reader.join().unwrap_or_default();
                let stderr = err_reader.join().unwrap_or_default();
                return Ok(Ran { code: status.code(), stdout: String::from_utf8_lossy(&stdout).into_owned(), stderr: String::from_utf8_lossy(&stderr).into_owned() });
            }
            Ok(None) => {}
            Err(e) => return Err(format!("could not wait for the decider program: {}", e.kind())),
        }
        if Instant::now() >= deadline {
            let group = format!("-{}", child.id());
            let killed = Command::new("kill").args(["-KILL", "--", group.as_str()]).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
            if !killed {
                let _ = child.kill();
            }
            let _ = child.wait();
            return Err(format!("the decider program ran over its time limit of {} s and was ended", timeout.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn tail(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    all[all.len().saturating_sub(lines)..].join(" | ")
}

impl ProgramDecider {
    fn env(&self, request: &DecisionRequest<'_>, budget_micro_usd: u64) -> Result<Vec<(String, String)>, String> {
        let path = request
            .child_env
            .iter()
            .find(|(k, _)| k == "PATH")
            .map(|(_, v)| v.clone())
            .ok_or("the runner's child environment has no PATH")?;
        let home = request.run_dir.join("home");
        std::fs::create_dir_all(&home).map_err(|e| format!("decider home directory: {}", e.kind()))?;
        let mut env = vec![("PATH".to_owned(), path), ("HOME".to_owned(), home.display().to_string())];
        if let Mode::Model { model, adapter, effort, max_turns, final_escalate, reply_bin, .. } = &self.mode {
            env.push(("SDC_DECIDER_ESCALATE".to_owned(), if *final_escalate { "final" } else { "stop" }.to_owned()));
            env.push(("SDC_LLM_MODEL_DECIDER".to_owned(), model.clone()));
            env.push(("SDC_DECIDER_MAX_BUDGET_USD".to_owned(), usd_text(budget_micro_usd)));
            env.push(("SDC_REPLY".to_owned(), reply_bin.display().to_string()));
            if let Some(adapter) = adapter {
                env.push(("SDC_DECIDER_ADAPTER".to_owned(), adapter.clone()));
            }
            if let Some(effort) = effort {
                env.push(("SDC_LLM_EFFORT_DECIDER".to_owned(), effort.clone()));
            }
            if let Some(turns) = max_turns {
                env.push(("SDC_DECIDER_MAX_TURNS".to_owned(), turns.to_string()));
            }
            if let Some(login) = &self.login_file {
                // The link in the run's private directory, when there is one: the real path then
                // stays in the settings alone.
                let value = request.login_link.map_or_else(|| login.clone(), |link| link.display().to_string());
                env.push(("SDC_CLAUDE_CREDENTIALS".to_owned(), value));
            }
        }
        Ok(env)
    }

    fn decide_rule(&self, verdict: &str, request: &DecisionRequest<'_>, approval: &Approval) -> Verdict {
        let rec = request.run_dir.join("decider").join("rule");
        if let Err(e) = std::fs::create_dir_all(&rec) {
            return Self::cannot(format!("decider record directory: {}", e.kind()));
        }
        let body = decide_request(approval, &PathBuf::new(), &rec, None);
        let req_file = rec.join("decide-request.json");
        if let Err(e) = std::fs::write(&req_file, format!("{body}\n")) {
            return Self::cannot(format!("decider request file: {}", e.kind()));
        }
        let env = match self.env(request, 0) {
            Ok(env) => env,
            Err(reason) => return Self::cannot(reason),
        };
        let ran = match run_program(&self.program, &[verdict, &req_file.display().to_string()], &env, &rec, self.timeout) {
            Ok(ran) => ran,
            Err(reason) => return Self::cannot(reason),
        };
        match self.read_reply(&ran, &rec) {
            Err(reason) => Self::cannot(reason),
            Ok(reply) => {
                if reply.get("valid") == Some(&Value::Bool(false)) {
                    return Self::cannot("the rule decider's reply is not valid");
                }
                match reply.get("decision").and_then(Value::as_str) {
                    Some(d @ ("approve" | "reject")) => Verdict::Answer {
                        approve: d == "approve",
                        principal: Some(self.principal.clone()),
                        cost_micro_usd: Some(0),
                        model_calls: 0,
                        reasons: if d == "reject" { reply_reasons(&reply).map(|t| self.redact(&t)) } else { None },
                    },
                    _ => Self::cannot("the rule decider's reply has no decision approve or reject"),
                }
            }
        }
    }

    /// Checks the exit and parses stdout as one JSON object; writes redacted copies into `rec`.
    fn read_reply(&self, ran: &Ran, rec: &Path) -> Result<Value, String> {
        let _ = std::fs::write(rec.join("decider-stderr.txt"), self.redact(&ran.stderr));
        let _ = std::fs::write(rec.join("reply.json"), self.redact(&ran.stdout));
        match ran.code {
            Some(0) => {}
            Some(code) => return Err(format!("decider.sh exited {code}: {}", self.redact(&tail(&ran.stderr, 3)))),
            None => return Err(format!("decider.sh was ended by a signal: {}", self.redact(&tail(&ran.stderr, 3)))),
        }
        match serde_json::from_str::<Value>(ran.stdout.trim()) {
            Ok(reply) if reply.is_object() => Ok(reply),
            _ => Err("decider.sh printed no single JSON object".to_owned()),
        }
    }

    fn decide_model(&self, request: &DecisionRequest<'_>, approval: &Approval) -> Verdict {
        let Mode::Model { final_escalate, retries, reply_bin, without_claims, .. } = &self.mode else {
            return Self::cannot("not a model decider");
        };
        if !without_claims {
            // Loom's decisions[] holds no claim values (`position.claims`): decider.sh tells the
            // model to check claims in in/checks.json that it is not given. The setting
            // `without_claims: true` is the run's explicit acknowledgement of that.
            return Self::cannot("claims of the position are not in Loom's output: decider.sh tells the model to read them from in/checks.json (member `position.claims`) and decisions[] holds none; set decider.without_claims to run the model decider without them");
        }
        if !reply_bin.is_file() {
            return Self::cannot("sdc-reply (decider.reply_bin) is not a file: decider.sh needs it for the reply schema, the citable artifacts and the prior rejections");
        }
        let budget = request.budget_micro_usd.min(self.max_call_micro_usd);
        let decider_dir = request.run_dir.join("decider");
        if instructions_root(&self.program).is_none() {
            return Self::cannot("the instruction files are not found: decider.sh needs instructions/decider/ above the bindings copy");
        }
        let case_dir = match make_case_dir(request, approval) {
            Ok(dir) => dir,
            Err(reason) => return Self::cannot(reason),
        };
        let mut spent_micro = 0u64;
        let mut calls = 0u64;
        let mut attempt = 1u64;
        let mut invalid_reason = String::new();
        loop {
            let number = record_dirs(&decider_dir) + 1;
            if number > self.max_calls {
                return Self::cannot(format!("decider budget spent: {} decider calls (max_calls)", self.max_calls));
            }
            if budget <= spent_micro {
                return Self::cannot(format!("the per-call budget of {} micro USD is spent", budget));
            }
            let rec = decider_dir.join(format!("{number:03}-{}", safe_part(&approval.action)));
            if let Err(e) = std::fs::create_dir_all(&rec) {
                return Self::cannot(format!("decider record directory: {}", e.kind()));
            }
            let retry = (attempt > 1).then(|| (attempt, invalid_reason.clone()));
            let body = decide_request(approval, &case_dir, &rec, retry);
            let req_file = rec.join("decide-request.json");
            if let Err(e) = std::fs::write(&req_file, format!("{body}\n")) {
                return Self::cannot(format!("decider request file: {}", e.kind()));
            }
            if let Err(e) = std::fs::write(rec.join("loom-decision.json"), format!("{}\n", request.decision)) {
                return Self::cannot(format!("decider record file: {}", e.kind()));
            }
            let env = match self.env(request, budget - spent_micro) {
                Ok(env) => env,
                Err(reason) => return Self::cannot(reason),
            };
            let ran = match run_program(&self.program, &["model", &req_file.display().to_string()], &env, &rec, self.timeout) {
                Ok(ran) => ran,
                Err(reason) => return Self::cannot(self.redact(&reason)),
            };
            let reply = match self.read_reply(&ran, &rec) {
                Ok(reply) => reply,
                Err(reason) => return Self::cannot(reason),
            };
            let model_call = reply.get("adapter").and_then(Value::as_str) != Some("rule") && reply.get("dry") != Some(&Value::Bool(true));
            if model_call {
                calls += 1;
            }
            if reply.get("valid") == Some(&Value::Bool(false)) {
                invalid_reason = reply.get("invalid_reason").and_then(Value::as_str).unwrap_or("invalid").to_owned();
                if let Some(cost) = cost_micro(&reply) {
                    spent_micro = spent_micro.saturating_add(cost);
                }
                if attempt > *retries {
                    return Self::cannot(format!("invalid decider reply after {attempt} attempts: {}", self.redact(&invalid_reason)));
                }
                attempt += 1;
                continue;
            }
            let Some(cost) = cost_micro(&reply) else {
                return Self::cannot("the decider's reply has no cost (cost_usd is absent, null or not a non-negative number)");
            };
            let spent = spent_micro.saturating_add(cost);
            let decision = match reply.get("decision").and_then(Value::as_str) {
                Some("approve") => true,
                Some("reject") => false,
                Some("escalate") => {
                    let reasons = reply.get("reasons").and_then(Value::as_array).map(|r| r.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")).unwrap_or_default();
                    if !*final_escalate {
                        return Self::cannot(format!("reserved for a person: the decider escalated (escalate stop): {}", self.redact(&reasons)));
                    }
                    // escalate final: the decider's `best` is taken (the driver: escalation_suppressed), reject when none.
                    reply.get("best").and_then(Value::as_str) == Some("approve")
                }
                _ => return Self::cannot("the decider's reply has no decision approve, reject or escalate"),
            };
            let reasons = if decision { None } else { reply_reasons(&reply).map(|t| self.redact(&t)) };
            return Verdict::Answer { approve: decision, principal: Some(self.principal.clone()), cost_micro_usd: Some(spent), model_calls: calls, reasons };
        }
    }
}

impl Decider for ProgramDecider {
    fn max_call_micro_usd(&self) -> u64 {
        match self.mode {
            Mode::Rule { .. } => 0,
            Mode::Model { .. } => self.max_call_micro_usd,
        }
    }

    fn decide(&self, request: &DecisionRequest<'_>) -> Verdict {
        if self.autonomy.as_deref() != Some(AUTONOMY_DECIDES) {
            let level = self.autonomy.as_deref().map(|l| format!("autonomy {l}")).unwrap_or_else(|| "no autonomy level named".to_owned());
            return Self::cannot(format!("reserved for a person: {level}; the decider decides only at {AUTONOMY_DECIDES}"));
        }
        let approval = match read_approval(request) {
            Ok(approval) => approval,
            Err(reason) => return Self::cannot(reason),
        };
        match &self.mode {
            Mode::Rule { verdict } => self.decide_rule(verdict, request, &approval),
            Mode::Model { .. } => self.decide_model(request, &approval),
        }
    }
}

// The tests of this module: `src/loom_run/decider/tests.rs`. The crate is a binary, so an
// integration test (tests/cli.rs) reaches the code only through the command line.
#[cfg(test)]
mod tests;

fn cost_micro(reply: &Value) -> Option<u64> {
    let usd = reply.get("cost_usd")?.as_f64()?;
    (usd.is_finite() && usd >= 0.0).then(|| (usd * 1_000_000.0).round() as u64)
}

/// The directory above the bindings copy's tools directory that holds `instructions/`, found the
/// way `decider.sh` finds it (the nearest directory above its own that has one).
pub fn instructions_root(program: &Path) -> Option<PathBuf> {
    let mut dir = program.parent()?.to_path_buf();
    loop {
        if dir.join("instructions").is_dir() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// The request `decider.sh` reads (`decider.sh <kind> <request.json>`; its header names the
/// members), built from the decision entry: `type` approval, the case directory, the subject, and
/// an empty `position` (Loom's output holds no claim values).
fn decide_request(approval: &Approval, case_dir: &Path, record_dir: &Path, retry: Option<(u64, String)>) -> Value {
    let mut request = json!({
        "type": "approval",
        "case_dir": case_dir.display().to_string(),
        "action": approval.action,
        "capability": approval.capability,
        "subject": approval.subject,
        "path": approval.path,
        "revision": approval.revision,
        "position": {"claims": {}, "open_obligations": [], "legitimate_outcomes": []},
        "record_dir": record_dir.display().to_string(),
    });
    if let Some(outcome) = &approval.close_outcome {
        request["type"] = json!("close");
        request["outcome"] = json!(outcome);
        for member in ["action", "capability", "subject", "path", "revision"] {
            request[member] = Value::Null;
        }
    }
    if let Some((attempt, why)) = retry {
        request["retry"] = json!({"attempt": attempt, "invalid_reason": why});
    }
    request
}

/// The decider's reasons of a reply: the `reasons` text members joined by line breaks.
fn reply_reasons(reply: &Value) -> Option<String> {
    let items = reply.get("reasons").and_then(Value::as_array)?;
    let text = items.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n");
    (!text.trim().is_empty()).then_some(text)
}
