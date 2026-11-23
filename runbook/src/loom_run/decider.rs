//! The real deciders of `runbook run`: the choice from the run settings and the decider that
//! answers one decision in this process: a rule by itself, a model decision through the adapter
//! program of the settings (`decider/`: rules, the model call, the inputs and the prompt, the check
//! of the reply).
//!
//! The table of what the driver decided, what Loom can ask today and what this module does with
//! each is in `docs/loom-run-decisions.md`. Short form: an entry of `decisions[]` of kind
//! `ApproveAction` or `Approve` is an approval and one of kind `Close` is asked as a close (the
//! outcome Loom printed); everything else (a question, a finding, `Handoff`, an unknown kind) ends
//! in `CannotDecide` naming why, unless the answers file of the bindings declares the action: its
//! requests, a question or a finding each, go to [`Decider::ask`] (`decider/ask.rs`).

use crate::loom_run::answers::{self, AskRequest, Asked};
use crate::loom_run::decide::{DecisionRequest, Decider, RefusingDecider, ScriptedDecider, Verdict};
use serde_json::{json, Map, Value};
use std::io::Read;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The autonomy level at which the decider takes every decision (but the close of a case, which a
/// rule takes). At A1 to A4 it takes only the decisions the holder table gives to `decider` at that
/// level (`routing.rs`, `tools/autonomy-levels.json`); A0 and no level are a person's.
pub const AUTONOMY_DECIDES: &str = "A5";

const AUTONOMY_LEVELS: [&str; 6] = ["A0", "A1", "A2", "A3", "A4", "A5"];
const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];
const MODEL_KEYS: [&str; 15] = [
    "requester",
    "without_claims",
    "kind",
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
    "replay_plan",
];
const RULE_KEYS: [&str; 3] = ["kind", "verdict", "timeout_seconds"];
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
/// `"scripted"`: R1's test double. `"model"`: one model call per decision. `"rule"`:
/// the rule `approve` or `reject` (the driver's `SDC_DECIDER=approve|reject`; the driver's
/// `SDC_RULE_APPROVALS` is not mapped, see the docs). An unknown key or kind is refused by name.
pub fn from_settings(settings: &Value) -> Result<Box<dyn Decider>, String> {
    match settings.get("decider") {
        None => Ok(Box::new(RefusingDecider)),
        Some(decider) => match decider.get("kind").and_then(Value::as_str) {
            Some("scripted") => ScriptedDecider::from_value(decider).map(|d| Box::new(d) as Box<dyn Decider>),
            Some("model") | Some("rule") => NativeDecider::parse(settings, decider).map(|d| Box::new(d) as Box<dyn Decider>),
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
    Ok(step_principal(settings)?.and_then(|(level, principal)| (level == AUTONOMY_DECIDES).then_some(principal)))
}

/// The level (A1 to A5) and the principal of a run whose decider is of kind `model` or `rule`;
/// `None` for A0, no level, no decider or another kind. At A5 every person step is the decider's;
/// at A1 to A4 only the steps whose class the holder table gives to the decider (`Routing`).
pub fn step_principal(settings: &Value) -> Result<Option<(String, String)>, String> {
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
    Ok(level.filter(|l| l != "A0").map(|l| (l, principal)))
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
        reply_bin: Option<PathBuf>,
        replay_plan: Option<PathBuf>,
        without_claims: bool,
    },
    Rule {
        verdict: String,
    },
}

/// The decider of a run: answers a decision in this process. A rule answers by itself; a model
/// decision starts the adapter program of its settings (and nothing else) once per attempt.
pub struct NativeDecider {
    mode: Mode,
    max_calls: u64,
    max_call_micro_usd: u64,
    timeout: Duration,
    autonomy: Option<String>,
    login_file: Option<String>,
    principal: String,
    /// `decider.requester`: a decision of a requester-side class goes to the requester decider.
    requester: bool,
    /// Who decides, read once (see [`Decider::pin_routing`]) and never again: the run's own
    /// copy of the table is writable by a bound program.
    routing: std::sync::OnceLock<Result<Routing, String>>,
    /// The gateway file of the run's private directory (see [`Decider::use_gateway`]): handed to the
    /// adapter as `SDC_GATEWAY_FILE`, the path only.
    gateway_file: std::sync::OnceLock<PathBuf>,
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

fn absolute_path_key(decider: &Value, key: &str) -> Result<Option<String>, String> {
    match path_key(decider, key)? {
        Some(p) if !Path::new(&p).is_absolute() => Err(format!("decider.{key}: an absolute path")),
        other => Ok(other),
    }
}

impl NativeDecider {
    /// Reads `settings.decider` (kind `model` or `rule`) and the top-level `autonomy` and
    /// `login_file`.
    pub fn parse(settings: &Value, decider: &Value) -> Result<NativeDecider, String> {
        let object = decider.as_object().ok_or("decider: an object")?;
        let kind = decider.get("kind").and_then(Value::as_str).unwrap_or("");
        let allowed: &[&str] = if kind == "rule" { &RULE_KEYS } else { &MODEL_KEYS };
        for key in object.keys() {
            if !allowed.contains(&key.as_str()) {
                return Err(format!("decider.{key}: not a key of a decider of kind {kind}"));
            }
        }
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
                reply_bin: path_key(decider, "reply_bin")?.map(PathBuf::from),
                replay_plan: absolute_path_key(decider, "replay_plan")?.map(PathBuf::from),
                without_claims,
            };
            (mode, max_calls, max_call)
        };
        let requester = match decider.get("requester") {
            None => false,
            Some(Value::Bool(b)) => *b,
            Some(_) => return Err("decider.requester: true or false".to_owned()),
        };
        let principal = person_step_principal(&json!({"autonomy": "A5", "decider": decider}))?.ok_or("decider: no principal")?;
        Ok(NativeDecider { mode, max_calls, max_call_micro_usd, timeout, autonomy: level, login_file, principal, requester, routing: std::sync::OnceLock::new(), gateway_file: std::sync::OnceLock::new() })
    }

    /// The table this run is routed by: the read of [`Decider::pin_routing`], or the first read
    /// when nobody pinned it.
    pub(super) fn routing(&self, run_dir: &Path) -> Result<&Routing, String> {
        self.routing.get_or_init(|| Routing::load(run_dir)).as_ref().map_err(Clone::clone)
    }

    fn cannot(reason: impl Into<String>) -> Verdict {
        Verdict::CannotDecide { reason: reason.into() }
    }

    /// The level the decider works at (A1 to A5); A0 and no level are for a person.
    fn level(&self) -> Result<&str, String> {
        match self.autonomy.as_deref() {
            Some("A0") | None => {
                let level = self.autonomy.as_deref().map(|l| format!("autonomy {l}")).unwrap_or_else(|| "no autonomy level named".to_owned());
                Err(format!("reserved for a person: {level}; the decider decides at A1 to A5"))
            }
            Some(level) => Ok(level),
        }
    }

    /// Whether `escalate: final` applies: at A5 only. Below A5 an escalate stops the run for a
    /// person, whatever the settings say (holder `decider`, not `decider_final`).
    pub(super) fn final_escalate(&self) -> bool {
        matches!(self.mode, Mode::Model { final_escalate: true, .. }) && self.autonomy.as_deref() == Some(AUTONOMY_DECIDES)
    }

    /// What the level lets the decider do with a decision of `kind` (`approval`, `close`,
    /// `question` or `finding`) on `action`. `Ok(false)`: the decider takes it. `Ok(true)`: a rule
    /// takes it (the close of the root case at A5: `rule:autopilot`, no decider call). `Err`: it
    /// stays with a person, with the reason. A5 gives every decision to the decider except that
    /// close. Below A5 only a class the table gives to `decider` is taken; any other holder and a
    /// class the table does not know are a person's. At every level, A5 included, the table is
    /// loaded first, and a table that cannot be read is a person's: no rule and no model decides
    /// without it.
    fn route(&self, level: &str, run_dir: &Path, kind: &str, action: &str, outcome: Option<&str>) -> Result<bool, String> {
        let routing = self.routing(run_dir).map_err(|e| format!("reserved for a person: autonomy {level}: the table of who decides cannot be used ({e}), so a person decides"))?;
        if level == AUTONOMY_DECIDES {
            return Ok(outcome.filter(|_| kind == "close").is_some_and(|outcome| routing.closes_by_rule(level, outcome)));
        }
        let class = routing.class_of(kind, action);
        match routing.holder(level, &class) {
            Some("decider") => Ok(false),
            Some(holder) => Err(format!("reserved for a person: autonomy {level} gives the class {class} ({action}) to {holder}, not to the decider")),
            None => Err(format!("reserved for a person: autonomy {level} has no holder for the class {class} ({action})")),
        }
    }

    fn redact(&self, text: &str) -> String {
        match &self.login_file {
            Some(path) => text.replace(path.as_str(), "<login file>"),
            None => text.to_owned(),
        }
    }
}

struct Ran {
    code: i32,
    stdout: String,
    stderr: String,
}

/// Starts `program args` with exactly `env`, in its own process group, and ends the group when
/// `timeout` passes.
fn run_program(program: &Path, args: &[&str], env: &[(String, String)], cwd: &Path, timeout: Duration) -> Result<Ran, String> {
    let mut command = Command::new(program);
    command.args(args).env_clear().envs(env.iter().map(|(k, v)| (k, v))).current_dir(cwd);
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).process_group(0);
    let mut child = command.spawn().map_err(|e| format!("could not start the decider's adapter program: {}", e.kind()))?;
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
                let code = status.code().or_else(|| status.signal().map(|s| 128 + s)).unwrap_or(1);
                return Ok(Ran { code, stdout: String::from_utf8_lossy(&stdout).into_owned(), stderr: String::from_utf8_lossy(&stderr).into_owned() });
            }
            Ok(None) => {}
            Err(e) => return Err(format!("could not wait for the decider's adapter program: {}", e.kind())),
        }
        if Instant::now() >= deadline {
            let group = format!("-{}", child.id());
            let killed = Command::new("kill").args(["-KILL", "--", group.as_str()]).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
            if !killed {
                let _ = child.kill();
            }
            let _ = child.wait();
            return Err(format!("the decider's adapter program ran over its time limit of {} s and was ended", timeout.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

impl NativeDecider {
    /// The environment of the adapter program: the runner's `PATH`, a `HOME` inside the run
    /// directory, the `sdc-reply` the settings or the runner's `PATH` name (the scripted adapter of
    /// the tests reads it) and the login link. Nothing of the runner's own environment.
    fn env(&self, request: &DecisionRequest<'_>) -> Result<Vec<(String, String)>, String> {
        let path = request
            .child_env
            .iter()
            .find(|(k, _)| k == "PATH")
            .map(|(_, v)| v.clone())
            .ok_or("the runner's child environment has no PATH")?;
        let home = request.run_dir.join("home");
        std::fs::create_dir_all(&home).map_err(|e| format!("decider home directory: {}", e.kind()))?;
        let mut env = vec![("PATH".to_owned(), path), ("HOME".to_owned(), home.display().to_string())];
        if let Mode::Model { reply_bin: Some(reply_bin), .. } = &self.mode {
            env.push(("SDC_REPLY".to_owned(), reply_bin.display().to_string()));
        }
        if let Mode::Model { replay_plan: Some(plan), .. } = &self.mode {
            // A re-drive's adapter takes the plan of the run from this path and searches nowhere.
            env.push(("SDC_REPLAY".to_owned(), plan.display().to_string()));
        }
        if let (Mode::Model { .. }, Some(login)) = (&self.mode, &self.login_file) {
            // The link in the run's private directory, when there is one: the real path then
            // stays in the settings alone.
            let value = request.login_link.map_or_else(|| login.clone(), |link| link.display().to_string());
            env.push(("SDC_CLAUDE_CREDENTIALS".to_owned(), value));
        }
        if let (Mode::Model { .. }, Some(file)) = (&self.mode, self.gateway_file.get()) {
            // The path of the gateway file, never the token: the adapter reads the file.
            env.push((crate::loom_run::gateway::FILE_VARIABLE.to_owned(), file.display().to_string()));
        }
        Ok(env)
    }

    /// The reply of the decider to the request `body` in the record directory `rec`: by rule, or
    /// from one model call. The reply and what the adapter wrote on its standard error are kept in
    /// `rec` (`reply.json`, `decider-stderr.txt`), the login path replaced by `<login file>`.
    fn reply(&self, body: &Value, rec: &Path, request: &DecisionRequest<'_>, budget_micro_usd: u64) -> Result<Value, String> {
        let replied = match &self.mode {
            Mode::Rule { verdict } => Replied { reply: rules::rule_reply(verdict, body), stderr: String::new() },
            Mode::Model { model, adapter, effort, max_turns, .. } => {
                let tools = request.run_dir.join("bindings").join("tools");
                let root = instructions_root(&tools).ok_or("the instruction files are not found: the decider needs instructions/decider/ above the bindings copy")?;
                let env = self.env(request)?;
                let call_of = if self.requester && body["side"] == "requester" { requester::run } else { call::run };
                call_of(&ModelCall {
                    req: body,
                    rec,
                    tools: &tools,
                    root: &root,
                    model,
                    adapter: adapter.as_deref(),
                    effort: effort.as_deref(),
                    max_turns: *max_turns,
                    final_escalate: self.final_escalate(),
                    budget_micro_usd,
                    env: &env,
                    timeout: self.timeout,
                })
                .map_err(|reason| self.redact(&reason))?
            }
        };
        let _ = std::fs::write(rec.join("decider-stderr.txt"), self.redact(&replied.stderr));
        let _ = std::fs::write(rec.join("reply.json"), self.redact(&format!("{}\n", replied.reply)));
        Ok(replied.reply)
    }

    fn decide_rule(&self, verdict: &str, request: &DecisionRequest<'_>, approval: &Approval) -> Verdict {
        let rec = request.run_dir.join("decider").join("rule");
        if let Err(e) = std::fs::create_dir_all(&rec) {
            return Self::cannot(format!("decider record directory: {}", e.kind()));
        }
        let body = decide_request(approval, &PathBuf::new(), &rec, None);
        if let Err(e) = std::fs::write(rec.join("decide-request.json"), format!("{body}\n")) {
            return Self::cannot(format!("decider request file: {}", e.kind()));
        }
        let reply = match self.reply(&body, &rec, request, 0) {
            Ok(reply) => reply,
            Err(reason) => return Self::cannot(reason),
        };
        match reply.get("decision").and_then(Value::as_str) {
            Some(d @ ("approve" | "reject")) => Verdict::Answer {
                approve: d == "approve",
                principal: Some(self.principal.clone()),
                cost_micro_usd: Some(0),
                model_calls: 0,
                reasons: if d == "reject" { reply_reasons(&reply).map(|t| self.redact(&t)) } else { None },
            },
            _ => Self::cannot(format!("the rule decider's reply has no decision approve or reject (rule {verdict})")),
        }
    }

    fn decide_model(&self, request: &DecisionRequest<'_>, approval: &Approval) -> Verdict {
        let Mode::Model { retries, without_claims, .. } = &self.mode else {
            return Self::cannot("not a model decider");
        };
        let kind = if approval.close_outcome.is_some() { "close" } else { "approval" };
        let routed = match self.requester_class(request.run_dir, kind, &approval.action) {
            Ok(class) => class,
            Err(reason) => return Self::cannot(reason),
        };
        if !without_claims && routed.is_none() {
            // Loom's decisions[] holds no claim values (`position.claims`): the decider tells the
            // model to check claims in in/checks.json that it is not given. The setting
            // `without_claims: true` is the run's explicit acknowledgement of that. The requester
            // reads no claims.
            return Self::cannot(NO_CLAIMS);
        }
        let budget = request.budget_micro_usd.min(self.max_call_micro_usd);
        let decider_dir = request.run_dir.join("decider");
        if instructions_root(&request.run_dir.join("bindings").join("tools")).is_none() {
            return Self::cannot(NO_INSTRUCTIONS);
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
            let mut body = decide_request(approval, &case_dir, &rec, retry);
            if let Some(class) = &routed {
                requester::tag(&mut body, class, request.run_dir);
                if let Some(level) = &self.autonomy {
                    body["autonomy"] = json!(level);
                }
            }
            if let Err(e) = std::fs::write(rec.join("decide-request.json"), format!("{body}\n")) {
                return Self::cannot(format!("decider request file: {}", e.kind()));
            }
            if let Err(e) = std::fs::write(rec.join("loom-decision.json"), format!("{}\n", request.decision)) {
                return Self::cannot(format!("decider record file: {}", e.kind()));
            }
            let reply = match self.reply(&body, &rec, request, budget - spent_micro) {
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
                    if !self.final_escalate() {
                        return Self::cannot(format!("reserved for a person: the decider escalated (escalate stop): {}", self.redact(&reasons)));
                    }
                    // escalate final: the decider's `best` is taken (the driver: escalation_suppressed), reject when none.
                    reply.get("best").and_then(Value::as_str) == Some("approve")
                }
                _ => return Self::cannot("the decider's reply has no decision approve, reject or escalate"),
            };
            let reasons = if decision { None } else { reply_reasons(&reply).map(|t| self.redact(&t)) };
            return Verdict::Answer { approve: decision, principal: Some(self.principal_for(routed.is_some()).to_owned()), cost_micro_usd: Some(spent), model_calls: calls, reasons };
        }
    }
}

/// The text `decide_model` gives when the settings do not acknowledge missing claims.
const NO_CLAIMS: &str = "claims of the position are not in Loom's output: the decider tells the model to read them from in/checks.json (member `position.claims`) and decisions[] holds none; set decider.without_claims to run the model decider without them";
const NO_INSTRUCTIONS: &str = "the instruction files are not found: the decider needs instructions/decider/ above the bindings copy";

impl Decider for NativeDecider {
    fn pin_routing(&self, run_dir: &Path) {
        let _ = self.routing.get_or_init(|| Routing::load(run_dir));
    }

    fn use_gateway(&self, file: &Path) {
        let _ = self.gateway_file.set(file.to_path_buf());
    }

    fn max_call_micro_usd(&self) -> u64 {
        match self.mode {
            Mode::Rule { .. } => 0,
            Mode::Model { .. } => self.max_call_micro_usd,
        }
    }

    fn decide(&self, request: &DecisionRequest<'_>) -> Verdict {
        let level = match self.level() {
            Ok(level) => level,
            Err(reason) => return Self::cannot(reason),
        };
        let approval = match read_approval(request, Some(&request.run_dir.join("bindings"))) {
            Ok(approval) => approval,
            Err(reason) => return Self::cannot(reason),
        };
        match self.route(level, request.run_dir, if approval.close_outcome.is_some() { "close" } else { "approval" }, &approval.action, approval.close_outcome.as_deref()) {
            Err(reason) => return Self::cannot(reason),
            Ok(true) => return Verdict::Answer { approve: true, principal: Some(routing::AUTOPILOT.to_owned()), cost_micro_usd: Some(0), model_calls: 0, reasons: None },
            Ok(false) => {}
        }
        match &self.mode {
            Mode::Rule { verdict } => self.decide_rule(verdict, request, &approval),
            Mode::Model { .. } => self.decide_model(request, &approval),
        }
    }

    fn answers_requests(&self) -> bool {
        self.level().is_ok()
    }

    fn ask(&self, request: &AskRequest<'_>) -> Asked {
        self.ask_request(request)
    }
}

mod approval;
mod ask;
mod call;
mod inputs;
mod prompt;
mod requester;
mod routing;
mod rules;
mod validate;

use approval::*;
use call::{ModelCall, Replied};
pub use routing::Routing;

// The tests of this module: `src/loom_run/decider/tests.rs`. The crate is a binary, so an
// integration test (tests/cli.rs) reaches the code only through the command line.
#[cfg(test)]
mod tests;
#[cfg(test)]
mod requester_tests;
#[cfg(test)]
mod golden;

/// The directory above the bindings copy's tools directory that holds `instructions/`: the nearest
/// directory, the tools directory itself first, that has one.
pub fn instructions_root(tools: &Path) -> Option<PathBuf> {
    let mut dir = tools.to_path_buf();
    loop {
        if dir.join("instructions").is_dir() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}
