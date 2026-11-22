//! Tests of the decider against a FAKE adapter (a script the test writes; no model): the decider
//! runs in this process and starts the adapter program of the settings, nothing else. The rules
//! (`approve`, `reject`) need no program at all.

use super::*;
use crate::assets::testdir::Dir;
use std::os::unix::fs::PermissionsExt;

const SYSTEM_PATH: &str = "/usr/bin:/bin";

struct World {
    _dir: Dir,
    run: PathBuf,
    work: PathBuf,
    adapter: PathBuf,
    reply_bin: PathBuf,
    login: PathBuf,
}

fn write_exe(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// The instruction files of the repository, the text the decider fills its prompts from.
fn instructions() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("instructions/decider")
}

/// A run directory with `home/`, `work/`, `instructions/decider/` (the repository's files) and a fake
/// adapter `bindings/tools/adapter.sh` holding `script`.
fn world(tag: &str, script: &str) -> World {
    let dir = Dir::new(tag);
    let run = dir.dir("run");
    dir.dir("run/home");
    let work = dir.dir("run/work");
    dir.dir("run/instructions");
    std::os::unix::fs::symlink(instructions(), run.join("instructions/decider")).unwrap();
    let adapter = run.join("bindings/tools/adapter.sh");
    write_exe(&adapter, script);
    let reply_bin = dir.path().join("sdc-reply");
    write_exe(&reply_bin, "#!/bin/sh\n");
    let login = dir.path().join("secret-login-dir/credentials.json");
    std::fs::create_dir_all(login.parent().unwrap()).unwrap();
    std::fs::write(&login, "{}").unwrap();
    World { _dir: dir, run, work, adapter, reply_bin, login }
}

/// An adapter that records its environment and its request beside the request, writes `decision`
/// as the model's `out/decision.json` (none: no file) and prints `response`; `more` runs before.
fn script(decision: Option<&str>, response: &str, more: &str) -> String {
    let write = decision.map(|d| format!("printf '%s\\n' '{d}' > \"$d/cwd/out/decision.json\"\n")).unwrap_or_default();
    format!("#!/bin/sh\nd=$(dirname \"$1\")\nenv > \"$d/env.txt\"\ncp \"$1\" \"$d/adapter-request.json\"\n{more}{write}printf '%s\\n' '{response}'\n")
}

fn response(cost: &str) -> String {
    format!(r#"{{"adapter":"claude-cli","cost_usd":{cost}}}"#)
}

const APPROVE: &str = r#"{"decision":"approve","reasons":["fine"],"cites":[]}"#;
const UNKNOWN: &str = r#"{"decision":"maybe","reasons":["x"],"cites":[]}"#;
const UNKNOWN_WHY: &str = "decision is not approve, reject or escalate";

/// An adapter whose model approves, at `cost`.
fn valid(cost: &str) -> String {
    script(Some(APPROVE), &response(cost), "")
}

/// An adapter whose model writes `decision`, at `cost`.
fn model(decision: &str, cost: &str) -> String {
    script(Some(decision), &response(cost), "")
}

fn settings(w: &World, decider: Value) -> Value {
    let mut base = json!({
        "kind": "model", "model": "claude-opus-5-5", "adapter": w.adapter.display().to_string(),
        "reply_bin": w.reply_bin.display().to_string(), "without_claims": true, "escalate": "final", "max_calls": 5,
    });
    for (k, v) in decider.as_object().unwrap() {
        base[k] = v.clone();
    }
    json!({"autonomy": "A5", "decider": base})
}

fn approve_action(w: &World) -> Value {
    json!({
        "case": "IDEA-1.direction", "kind": "ApproveAction", "action": "context.approve", "capability": "owner.approve",
        "decides": "context.approve of case IDEA-1.direction at revision 7", "subject": "context", "revision": "taee57460908c89e2",
        "path": w.work.join("context.yaml").display().to_string(),
        "command": ["b10x-loom", "tree", "approve", "--store", "/nonexistent/store", "--case", "IDEA-1.direction", "--principal", "person:owner"],
    })
}

fn env_with_path() -> Vec<(String, String)> {
    vec![("PATH".to_owned(), SYSTEM_PATH.to_owned())]
}

fn ask_with(decider: &dyn Decider, w: &World, decision: &Value, budget: u64, env: &[(String, String)]) -> Verdict {
    decider.decide(&DecisionRequest { decision, run_dir: &w.run, workspace: &w.work, budget_micro_usd: budget, child_env: env, login_link: None })
}

fn ask(w: &World, settings: &Value, decision: &Value, budget: u64) -> Verdict {
    let decider = from_settings(settings).unwrap();
    ask_with(decider.as_ref(), w, decision, budget, &env_with_path())
}

fn reason(verdict: Verdict) -> String {
    match verdict {
        Verdict::CannotDecide { reason } => reason,
        other => panic!("expected CannotDecide, got {other:?}"),
    }
}

fn first_record(w: &World) -> PathBuf {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(w.run.join("decider")).unwrap().filter_map(Result::ok).map(|e| e.path()).collect();
    dirs.sort();
    dirs.remove(0)
}

fn json_file(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The request the decider built for the first call (what it put before the adapter).
fn seen(w: &World) -> Value {
    json_file(&first_record(w).join("decide-request.json"))
}

// --- one test per kind of decisions[] entry -----------------------------------------------------

#[test]
fn approve_action_entry_is_asked_and_answered() {
    let w = world("r2-approve-action", &valid("0.012345"));
    let s = settings(&w, json!({}));
    let verdict = ask(&w, &s, &approve_action(&w), 250_000);
    assert_eq!(verdict, Verdict::Answer { approve: true, principal: Some("decider:model".to_owned()), cost_micro_usd: Some(12_345), model_calls: 1, reasons: None });
    let rec = first_record(&w);
    assert!(rec.file_name().unwrap().to_str().unwrap().starts_with("001-context.approve"));
    let seen = seen(&w);
    assert_eq!(seen["type"], "approval");
    assert_eq!(seen["action"], "context.approve");
    assert_eq!(seen["subject"], "context");
    assert_eq!(seen["revision"], "taee57460908c89e2");
    assert_eq!(seen["path"], "context.yaml", "the path is relative to the workspace");
    assert_eq!(seen["capability"], "owner.approve");
    let case_dir = PathBuf::from(seen["case_dir"].as_str().unwrap());
    let case = json_file(&case_dir.join("case.json"));
    assert_eq!(case["id"], "IDEA-1.direction");
    assert_eq!(case["artifacts"]["context"]["revision"], "taee57460908c89e2");
    assert_eq!(std::fs::read_link(case_dir.join("work")).unwrap(), w.work);
    assert!(rec.join("loom-decision.json").is_file() && rec.join("reply.json").is_file());
    let asked = json_file(&rec.join("adapter-request.json"));
    assert_eq!((asked["model"].as_str(), asked["node"].as_str(), asked["attempt"].as_u64()), (Some("claude-opus-5-5"), Some("decide.context.approve"), Some(1)));
    assert!(rec.join("prompt.md").is_file() && rec.join("reply-schema.json").is_file() && rec.join("response.json").is_file() && rec.join("status.json").is_file());
}

#[test]
fn approve_kind_entry_without_a_path_is_asked() {
    let w = world("r2-approve-kind", &valid("0.5"));
    let s = settings(&w, json!({}));
    let mut decision = approve_action(&w);
    decision["kind"] = json!("Approve");
    decision["action"] = json!("release.publish");
    decision["capability"] = json!("release.approve");
    decision.as_object_mut().unwrap().remove("path");
    let verdict = ask(&w, &s, &decision, 1_000_000);
    assert_eq!(verdict, Verdict::Answer { approve: true, principal: Some("decider:model".to_owned()), cost_micro_usd: Some(500_000), model_calls: 1, reasons: None });
    assert_eq!(seen(&w)["path"], Value::Null);
}

#[test]
fn a_reject_is_an_answer_that_does_not_approve() {
    let w = world("r2-reject", &model(r#"{"decision":"reject","reasons":["no"],"cites":[]}"#, "0.1"));
    let verdict = ask(&w, &settings(&w, json!({})), &approve_action(&w), 250_000);
    assert_eq!(verdict, Verdict::Answer { approve: false, principal: Some("decider:model".to_owned()), cost_micro_usd: Some(100_000), model_calls: 1, reasons: Some("no".to_owned()) });
    let reply = std::fs::read_to_string(first_record(&w).join("reply.json")).unwrap();
    assert!(reply.contains("\"no\""), "the reasons stay in the run directory record");
}

#[test]
fn a_close_entry_is_asked_as_a_close_with_the_outcome_loom_printed() {
    let w = world("r2-close", &valid("0.1"));
    let s = settings(&w, json!({}));
    let close = json!({
        "case": "IDEA-1", "kind": "Close", "action": null, "outcome": "kept", "decides": "close case IDEA-1 through kept",
        "command": ["b10x-loom", "tree", "close", "--store", "/nonexistent/store", "--case", "IDEA-1", "--outcome", "kept"],
    });
    let verdict = ask(&w, &s, &close, 250_000);
    assert!(matches!(verdict, Verdict::Answer { approve: true, .. }), "{verdict:?}");
    let seen = seen(&w);
    assert_eq!((seen["type"].as_str(), seen["outcome"].as_str()), (Some("close"), Some("kept")));
    assert_eq!(seen["action"], Value::Null);
    let mut no_outcome = close.clone();
    no_outcome.as_object_mut().unwrap().remove("outcome");
    assert!(reason(ask(&w, &s, &no_outcome, 250_000)).contains("names no outcome"));
}

#[test]
fn handoff_unknown_and_question_entries_cannot_be_decided() {
    let w = world("r2-kinds", &valid("0.1"));
    let s = settings(&w, json!({}));
    let mut handoff = approve_action(&w);
    handoff["kind"] = json!("Handoff");
    assert!(reason(ask(&w, &s, &handoff, 250_000)).contains("kind Handoff"));
    let mut unknown = approve_action(&w);
    unknown["kind"] = json!("Wish");
    assert_eq!(reason(ask(&w, &s, &unknown, 250_000)), "kind Wish not known");
    let mut question = approve_action(&w);
    question["action"] = json!("scope.answer");
    std::fs::write(w.run.join("bindings/sdc.yaml"), "format: loom.bindings/1\nactions:\n  - action: scope.answer\n    executor: {kind: person, principal: person:owner}\n").unwrap();
    let why = reason(ask(&w, &s, &question, 250_000));
    assert!(why.contains("scope.answer is a step whose outcome is a person's text") && why.contains("tools/answers.yaml"), "{why}");
    assert!(!w.run.join("decider").exists(), "none of these started the program");
}

#[test]
fn absent_members_are_named_not_guessed() {
    let w = world("r2-absent", &valid("0.1"));
    let s = settings(&w, json!({}));
    for member in ["action", "subject", "revision"] {
        let mut decision = approve_action(&w);
        decision.as_object_mut().unwrap().remove(member);
        let why = reason(ask(&w, &s, &decision, 250_000));
        assert!(why.contains(&format!("member `{member}` absent")), "{why}");
    }
    let mut no_kind = approve_action(&w);
    no_kind.as_object_mut().unwrap().remove("kind");
    assert_eq!(reason(ask(&w, &s, &no_kind, 250_000)), "the decision has no kind");
    let mut outside = approve_action(&w);
    outside["path"] = json!("/elsewhere/context.yaml");
    assert!(reason(ask(&w, &s, &outside, 250_000)).contains("not inside the workspace"));
    let mut up = approve_action(&w);
    up["path"] = json!(w.work.join("../x").display().to_string());
    assert!(reason(ask(&w, &s, &up, 250_000)).contains("not a plain name"));
}

#[test]
fn artifacts_of_the_case_file_reach_case_json() {
    let w = world("r2-casefile", &valid("0.1"));
    let store = w.run.join("store");
    std::fs::create_dir_all(&store).unwrap();
    let file = store.join(format!("case-{}.json", hex("IDEA-1.direction")));
    std::fs::write(&file, r#"{"id":"IDEA-1.direction","artifacts":[{"artifact":"requirements","revision":"te23630e115159d67"},{"artifact":"context","revision":"old"}]}"#).unwrap();
    let mut decision = approve_action(&w);
    decision["command"] = json!(["b10x-loom", "tree", "approve", "--store", store.display().to_string()]);
    ask(&w, &settings(&w, json!({})), &decision, 250_000);
    let case = json_file(&Path::new(seen(&w)["case_dir"].as_str().unwrap()).join("case.json"));
    assert_eq!(case["artifacts"]["requirements"]["revision"], "te23630e115159d67");
    assert_eq!(case["artifacts"]["context"]["revision"], "taee57460908c89e2", "the decision's own revision wins");
}

#[test]
fn without_claims_is_an_explicit_acknowledgement() {
    let w = world("r2-claims", &valid("0.1"));
    let s = settings(&w, json!({"without_claims": false}));
    let why = reason(ask(&w, &s, &approve_action(&w), 250_000));
    assert!(why.contains("claims of the position are not in Loom's output"), "{why}");
    assert!(!w.run.join("decider").exists());
}

// --- environment, budget, answer parsing, limits ---------------------------------------------------

#[test]
fn the_adapter_gets_an_environment_built_here_and_nothing_inherited() {
    std::env::set_var("RUNBOOK_R2_MARKER", "leaked");
    std::env::set_var("SDC_LLM_ADAPTER", "leaked-adapter");
    let w = world("r2-env", &valid("0.1"));
    let s = settings(&w, json!({"effort": "high", "max_turns": 6}));
    ask(&w, &s, &approve_action(&w), 250_000);
    let env = std::fs::read_to_string(first_record(&w).join("env.txt")).unwrap();
    assert!(!env.contains("RUNBOOK_R2_MARKER") && !env.contains("leaked"), "{env}");
    let mut names: Vec<&str> = env.lines().filter_map(|l| l.split('=').next()).filter(|n| !["PWD", "SHLVL", "_", "OLDPWD"].contains(n)).collect();
    names.sort_unstable();
    assert_eq!(names, ["HOME", "PATH", "SDC_REPLY"]);
    assert!(env.contains(&format!("PATH={SYSTEM_PATH}\n")));
    assert!(env.contains(&format!("HOME={}\n", w.run.join("home").display())));
    assert!(env.contains(&format!("SDC_REPLY={}\n", w.reply_bin.display())));
    let asked = json_file(&first_record(&w).join("adapter-request.json"));
    assert_eq!((asked["model"].as_str(), asked["effort"].as_str(), asked["max_turns"].as_u64()), (Some("claude-opus-5-5"), Some("high"), Some(6)));
    std::env::remove_var("RUNBOOK_R2_MARKER");
    std::env::remove_var("SDC_LLM_ADAPTER");
}

#[test]
fn a_child_environment_without_path_is_refused() {
    let w = world("r2-nopath", &valid("0.1"));
    let s = settings(&w, json!({}));
    let decider = from_settings(&s).unwrap();
    let why = reason(ask_with(decider.as_ref(), &w, &approve_action(&w), 250_000, &[]));
    assert!(why.contains("no PATH"), "{why}");
}

#[test]
fn the_per_call_budget_is_the_max_budget_in_the_adapters_request() {
    let w = world("r2-budget", &valid("0.1"));
    let s = settings(&w, json!({}));
    ask(&w, &s, &approve_action(&w), 123_456);
    let asked = json_file(&first_record(&w).join("adapter-request.json"));
    assert_eq!(asked["max_budget_usd"].as_f64(), Some(0.123456), "{asked}");
    // never more than the decider's own limit (max_budget_usd), even when more is handed
    let w = world("r2-budget-cap", &valid("0.01"));
    let s = settings(&w, json!({"max_budget_usd": 0.05}));
    let decider = from_settings(&s).unwrap();
    assert_eq!(decider.max_call_micro_usd(), 50_000);
    ask(&w, &s, &approve_action(&w), 900_000);
    let asked = json_file(&first_record(&w).join("adapter-request.json"));
    assert_eq!(asked["max_budget_usd"].as_f64(), Some(0.05), "{asked}");
    // the default limit is the driver's 0.25 USD
    assert_eq!(from_settings(&settings(&w, json!({}))).unwrap().max_call_micro_usd(), 250_000);
}

#[test]
fn an_adapter_answer_that_is_not_json_is_cannot_decide() {
    let w = world("r2-notjson", "#!/bin/sh\necho 'approved, thanks'\n");
    let why = reason(ask(&w, &settings(&w, json!({"retries": 0})), &approve_action(&w), 250_000));
    assert!(why.contains("invalid decider reply after 1 attempts: out/decision.json is missing"), "{why}");
}

#[test]
fn an_answer_without_a_cost_is_cannot_decide() {
    for response in [r#"{"adapter":"claude-cli"}"#, r#"{"adapter":"claude-cli","cost_usd":null}"#, r#"{"adapter":"claude-cli","cost_usd":-1}"#] {
        let w = world("r2-nocost", &script(Some(APPROVE), response, ""));
        let why = reason(ask(&w, &settings(&w, json!({})), &approve_action(&w), 250_000));
        assert!(why.contains("has no cost"), "{response}: {why}");
    }
}

#[test]
fn an_answer_without_a_known_decision_is_cannot_decide() {
    let w = world("r2-nodecision", &model(UNKNOWN, "0.1"));
    let why = reason(ask(&w, &settings(&w, json!({"retries": 0})), &approve_action(&w), 250_000));
    assert!(why.contains(&format!("invalid decider reply after 1 attempts: {UNKNOWN_WHY}")), "{why}");
}

#[test]
fn a_non_zero_exit_is_cannot_decide_with_the_exit_in_the_record() {
    let w = world("r2-exit", "#!/bin/sh\necho 'adapter: no login' >&2\nexit 127\n");
    let why = reason(ask(&w, &settings(&w, json!({"retries": 0})), &approve_action(&w), 250_000));
    assert!(why.contains("out/decision.json is missing"), "{why}");
    let rec = first_record(&w);
    assert_eq!(json_file(&rec.join("response.json"))["exit"], 127);
    assert_eq!(json_file(&rec.join("status.json"))["exit"], 127);
    assert!(std::fs::read_to_string(rec.join("decider-stderr.txt")).unwrap().contains("adapter: no login"));
}

#[test]
fn an_adapter_that_cannot_start_is_cannot_decide() {
    let w = world("r2-nostart", "#!/bin/sh\n");
    std::fs::remove_file(&w.adapter).unwrap();
    let why = reason(ask(&w, &settings(&w, json!({"retries": 0})), &approve_action(&w), 250_000));
    assert!(why.contains("out/decision.json is missing"), "{why}");
    let rec = first_record(&w);
    assert_eq!(json_file(&rec.join("response.json"))["exit"], 127);
    assert!(std::fs::read_to_string(rec.join("decider-stderr.txt")).unwrap().contains("No such file or directory"));
}

#[test]
fn an_adapter_over_its_time_limit_is_ended_with_its_children() {
    let w = world("r2-timeout", "#!/bin/sh\nsleep 30 &\necho $! > \"$(dirname \"$1\")/child.pid\"\nwait\n");
    let s = settings(&w, json!({"timeout_seconds": 1}));
    let mut d = NativeDecider::parse(&s, &s["decider"]).unwrap();
    d.timeout = Duration::from_millis(2000);
    let started = Instant::now();
    let why = reason(ask_with(&d, &w, &approve_action(&w), 250_000, &env_with_path()));
    assert!(why.contains("over its time limit"), "{why}");
    assert!(started.elapsed() < Duration::from_secs(10), "{:?}", started.elapsed());
    let pid = std::fs::read_to_string(first_record(&w).join("child.pid")).unwrap();
    let mut alive = true;
    for _ in 0..50 {
        alive = Command::new("kill").args(["-0", pid.trim()]).stderr(Stdio::null()).status().unwrap().success();
        if !alive {
            break;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    assert!(!alive, "the child of the adapter was left running");
}

#[test]
fn the_call_limit_stops_the_decider_before_the_next_adapter_start() {
    let w = world("r2-limit", &valid("0.1"));
    let s = settings(&w, json!({"max_calls": 2}));
    let decider = from_settings(&s).unwrap();
    let env = env_with_path();
    for _ in 0..2 {
        assert!(matches!(ask_with(decider.as_ref(), &w, &approve_action(&w), 250_000, &env), Verdict::Answer { .. }));
    }
    let why = reason(ask_with(decider.as_ref(), &w, &approve_action(&w), 250_000, &env));
    assert!(why.contains("decider budget spent: 2 decider calls"), "{why}");
    assert_eq!(record_dirs(&w.run.join("decider")), 2);
}

#[test]
fn an_invalid_reply_is_asked_again_with_its_reason_then_cannot_decide() {
    let w = world("r2-invalid", &model(UNKNOWN, "0.2"));
    let s = settings(&w, json!({"retries": 1}));
    let why = reason(ask(&w, &s, &approve_action(&w), 900_000));
    assert!(why.contains(&format!("invalid decider reply after 2 attempts: {UNKNOWN_WHY}")), "{why}");
    assert_eq!(record_dirs(&w.run.join("decider")), 2);
    let second = json_file(&w.run.join("decider").join("002-context.approve/decide-request.json"));
    assert_eq!(second["retry"], json!({"attempt": 2, "invalid_reason": UNKNOWN_WHY}));
    let prompt = std::fs::read_to_string(w.run.join("decider").join("002-context.approve/prompt.md")).unwrap();
    assert!(prompt.contains(UNKNOWN_WHY), "the retry prompt names why the first reply was refused");
}

#[test]
fn an_invalid_reply_then_a_valid_one_adds_both_costs() {
    let more = "if [ -e \"$d/../first\" ]; then\n printf '%s\\n' '{\"decision\":\"approve\",\"reasons\":[\"x\"],\"cites\":[]}' > \"$d/cwd/out/decision.json\"\n printf '%s\\n' '{\"adapter\":\"claude-cli\",\"cost_usd\":0.3}'\n exit 0\nfi\ntouch \"$d/../first\"\n";
    let script = script(Some(UNKNOWN), &response("0.2"), more);
    let w = world("r2-invalid-then-valid", &script);
    let verdict = ask(&w, &settings(&w, json!({})), &approve_action(&w), 900_000);
    assert_eq!(verdict, Verdict::Answer { approve: true, principal: Some("decider:model".to_owned()), cost_micro_usd: Some(500_000), model_calls: 2, reasons: None });
}

#[test]
fn an_escalate_takes_best_under_final_and_waits_for_a_person_under_stop() {
    let decision = r#"{"decision":"escalate","best":"approve","reasons":["unsure"],"cites":[]}"#;
    let w = world("r2-escalate-final", &model(decision, "0.1"));
    let verdict = ask(&w, &settings(&w, json!({"escalate": "final"})), &approve_action(&w), 250_000);
    assert_eq!(verdict, Verdict::Answer { approve: true, principal: Some("decider:model".to_owned()), cost_micro_usd: Some(100_000), model_calls: 1, reasons: None });
    let w = world("r2-escalate-stop", &model(decision, "0.1"));
    let why = reason(ask(&w, &settings(&w, json!({"escalate": "stop"})), &approve_action(&w), 250_000));
    assert!(why.starts_with("reserved for a person: the decider escalated"), "{why}");
}

// --- the autonomy rule ---------------------------------------------------------------------------

#[test]
fn only_autonomy_a5_lets_the_decider_answer() {
    let w = world("r2-autonomy", &valid("0.1"));
    let mut s = settings(&w, json!({}));
    let decider = from_settings(&s).unwrap();
    assert!(matches!(ask_with(decider.as_ref(), &w, &approve_action(&w), 250_000, &env_with_path()), Verdict::Answer { .. }));
    for level in ["A0", "A1", "A2", "A3", "A4"] {
        s["autonomy"] = json!(level);
        let d = from_settings(&s).unwrap();
        let why = reason(ask_with(d.as_ref(), &w, &approve_action(&w), 250_000, &env_with_path()));
        assert!(why.starts_with(&format!("reserved for a person: autonomy {level}")), "{why}");
    }
    s.as_object_mut().unwrap().remove("autonomy");
    let d = from_settings(&s).unwrap();
    assert!(reason(ask_with(d.as_ref(), &w, &approve_action(&w), 250_000, &env_with_path())).starts_with("reserved for a person: no autonomy level named"));
    s["autonomy"] = json!("A9");
    assert!(from_settings(&s).err().unwrap().contains("autonomy"));
    assert_eq!(record_dirs(&w.run.join("decider")), 1, "only the A5 decision started the adapter");
}

#[test]
fn the_principal_of_person_steps_follows_the_autonomy_rule() {
    let w = world("r2-principal", &valid("0.1"));
    let mut s = settings(&w, json!({}));
    assert_eq!(person_step_principal(&s).unwrap().as_deref(), Some("decider:model"));
    s["autonomy"] = json!("A3");
    assert_eq!(person_step_principal(&s).unwrap(), None, "a step reserved for a person keeps its person");
    assert_eq!(person_step_principal(&json!({"autonomy": "A5"})).unwrap(), None);
    assert_eq!(person_step_principal(&json!({"autonomy": "A5", "decider": {"kind": "scripted"}})).unwrap(), None);
    let rule = json!({"autonomy": "A5", "decider": {"kind": "rule", "verdict": "approve"}});
    assert_eq!(person_step_principal(&rule).unwrap().as_deref(), Some("decider:approve"));
}

// --- the settings --------------------------------------------------------------------------------

#[test]
fn settings_are_checked_and_unknown_keys_are_refused_by_name() {
    let w = world("r2-settings", &valid("0.1"));
    assert!(from_settings(&json!({})).is_ok(), "no decider key: the refusing default");
    let refusing = from_settings(&json!({})).unwrap();
    assert!(reason(ask_with(refusing.as_ref(), &w, &approve_action(&w), 0, &env_with_path())).contains("no decider configured"));
    let err = |decider: Value| from_settings(&settings(&w, decider)).err().expect("refused");
    assert!(err(json!({"colour": "blue"})).contains("decider.colour"));
    assert!(err(json!({"program": "decider.sh"})).contains("decider.program"), "the decider is not a program of the bindings copy");
    assert!(err(json!({"model": ""})).contains("decider.model"));
    assert!(err(json!({"escalate": "sometimes"})).contains("decider.escalate"));
    assert!(err(json!({"effort": "extreme"})).contains("decider.effort"));
    assert!(err(json!({"max_calls": 0})).contains("decider.max_calls"));
    assert!(err(json!({"max_budget_usd": 0})).contains("decider.max_budget_usd"));
    assert!(err(json!({"max_decider_calls": 3})).contains("same limit"));
    assert!(from_settings(&json!({"decider": {"kind": "oracle"}})).err().unwrap().contains("oracle"));
    assert!(from_settings(&json!({"decider": {"model": "m"}})).err().unwrap().contains("decider.kind"));
    assert!(from_settings(&json!({"decider": {"kind": "rule"}})).err().unwrap().contains("decider.verdict"));
    assert!(from_settings(&json!({"decider": {"kind": "rule", "verdict": "approve", "model": "m"}})).err().unwrap().contains("decider.model"));
    let mut alias = settings(&w, json!({"max_decider_calls": 40}));
    alias["decider"].as_object_mut().unwrap().remove("max_calls");
    assert!(from_settings(&alias).is_ok(), "the experiment record's name for the limit is accepted");
}

// --- the login file never leaves the child's environment ------------------------------------------

#[test]
fn the_login_path_reaches_the_adapter_environment_only_and_is_redacted_from_what_is_kept() {
    let w = world("r2-login", "#!/bin/sh\n");
    let secret = w.login.display().to_string();
    let more = "echo \"login $SDC_CLAUDE_CREDENTIALS\" >&2\n".to_owned();
    let cites = format!(r#"{{"decision":"approve","reasons":["fine"],"cites":["{secret}"]}}"#);
    std::fs::write(&w.adapter, script(Some(&cites), &response("0.1"), &more)).unwrap();
    let mut s = settings(&w, json!({"retries": 0}));
    s["login_file"] = json!(secret);
    let why = reason(ask(&w, &s, &approve_action(&w), 250_000));
    assert!(!why.contains(&secret) && why.contains("<login file>"), "{why}");
    let rec = first_record(&w);
    assert!(std::fs::read_to_string(rec.join("env.txt")).unwrap().contains(&format!("SDC_CLAUDE_CREDENTIALS={secret}\n")));
    for kept in ["decider-stderr.txt", "reply.json", "decide-request.json", "loom-decision.json", "response.json", "status.json", "request.json", "prompt.md"] {
        let text = std::fs::read_to_string(rec.join(kept)).unwrap_or_default();
        assert!(!text.contains(&secret), "{kept} holds the login path");
    }
    assert!(std::fs::read_to_string(rec.join("decider-stderr.txt")).unwrap().contains("login <login file>"));
    // without a login file in the settings the variable is not set
    let w = world("r2-nologin", &valid("0.1"));
    ask(&w, &settings(&w, json!({})), &approve_action(&w), 250_000);
    assert!(!std::fs::read_to_string(first_record(&w).join("env.txt")).unwrap().contains("SDC_CLAUDE_CREDENTIALS"));
}

// --- the rule decider --------------------------------------------------------------------------------

#[test]
fn the_rule_decider_answers_through_this_code_and_starts_no_program() {
    let w = world("r2-rule", "#!/bin/sh\ntouch \"$(dirname \"$1\")/../started\"\n");
    for (verdict, approve) in [("approve", true), ("reject", false)] {
        let s = json!({"autonomy": "A5", "decider": {"kind": "rule", "verdict": verdict}});
        let decider = from_settings(&s).unwrap();
        assert_eq!(decider.max_call_micro_usd(), 0);
        let env = vec![("PATH".to_owned(), SYSTEM_PATH.to_owned())];
        let started = Instant::now();
        let verdict_out = ask_with(decider.as_ref(), &w, &approve_action(&w), 0, &env);
        assert_eq!(verdict_out, Verdict::Answer { approve, principal: Some(format!("decider:{verdict}")), cost_micro_usd: Some(0), model_calls: 0, reasons: (!approve).then(|| "rule decider: refuse every decision".to_owned()) });
        assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
    }
    let reply = json_file(&w.run.join("decider/rule/reply.json"));
    assert_eq!(reply["adapter"], "rule");
    assert_eq!(record_dirs(&w.run.join("decider")), 0, "a rule decision is no model call and counts against no limit");
    assert!(!w.run.join("decider/started").exists() && !w.run.join("started").exists());
}

#[test]
fn the_rule_decider_obeys_the_autonomy_rule_too() {
    let w = world("r2-rule-autonomy", "#!/bin/sh\n");
    let s = json!({"autonomy": "A2", "decider": {"kind": "rule", "verdict": "approve"}});
    let why = reason(ask(&w, &s, &approve_action(&w), 0));
    assert!(why.starts_with("reserved for a person: autonomy A2"), "{why}");
}

// --- requests of the answers file ---------------------------------------------------------------

fn scope_decision() -> Value {
    json!({"case": "IDEA-1.direction", "kind": "ApproveAction", "action": "scope.answer", "capability": "owner.answer", "command": ["b10x-loom", "tree", "approve"]})
}

fn request_line(kind: &str, default: &str) -> crate::loom_run::answers::Request {
    let line = json!({"type": kind, "id": "q1", kind: {"default": default}, "subject": "intake", "path": "intake.yaml"});
    crate::loom_run::answers::parse_requests(&line.to_string()).unwrap().remove(0)
}

fn ask_request(decider: &dyn Decider, w: &World, decision: &Value, request: &crate::loom_run::answers::Request, budget: u64) -> Asked {
    let env = env_with_path();
    decider.ask(&AskRequest { decision, request, run_dir: &w.run, workspace: &w.work, budget_micro_usd: budget, child_env: &env, login_link: None })
}

/// An adapter whose model writes the reply `decision` with the answer "take it" (and `extra`), at 0.25.
fn answer_reply(decision: &str, extra: &str) -> String {
    model(&format!(r#"{{"decision":"{decision}","answer":"take it","reasons":["fine"],"cites":[]{extra}}}"#), "0.25")
}

/// An approval without the answer a question needs: an invalid reply, at 0.1.
fn invalid() -> String {
    model(APPROVE, "0.1")
}

const NO_ANSWER_WHY: &str = "an approved question needs an answer";

fn decisions_of(asked: &Asked) -> Vec<String> {
    asked.lines.iter().map(|l| l["decision"].as_str().unwrap().to_owned()).collect()
}

#[test]
fn a_question_is_asked_with_the_members_of_the_drivers_request_and_answered() {
    let w = world("ask-question", &answer_reply("approve", ""));
    let s = settings(&w, json!({}));
    let decider = from_settings(&s).unwrap();
    assert!(decider.answers_requests());
    let asked = ask_request(decider.as_ref(), &w, &scope_decision(), &request_line("question", "keep"), 1_000_000);
    assert_eq!((asked.cost_micro_usd, asked.model_calls), (250_000, 1));
    assert_eq!(asked.lines.len(), 1);
    assert_eq!((asked.lines[0]["decision"].as_str(), asked.lines[0]["class"].as_str(), asked.lines[0]["question"].as_str()), (Some("approve"), Some("scope_changing_answer"), Some("q1")));
    let given = asked.result.unwrap();
    assert_eq!((given.answer.as_str(), given.principal.as_deref()), ("take it", Some("decider:model")));
    let rec = first_record(&w);
    assert!(rec.file_name().unwrap().to_str().unwrap().starts_with("001-q1"));
    let seen = seen(&w);
    assert_eq!((seen["type"].as_str(), seen["class"].as_str(), seen["side"].as_str(), seen["autonomy"].as_str()), (Some("question"), Some("scope_changing_answer"), Some("delivering"), Some("A5")));
    assert_eq!((seen["subject"].as_str(), seen["path"].as_str(), seen["action"].as_str()), (Some("intake"), Some("intake.yaml"), Some("scope.answer")));
    assert_eq!(seen["question"]["default"], "keep");
    assert!(seen["decisions_log"].as_str().unwrap().ends_with("decisions.jsonl"));
}

#[test]
fn an_invalid_reply_is_retried_then_takes_the_default_under_final() {
    let w = world("ask-invalid-final", &invalid());
    let s = settings(&w, json!({"retries": 1}));
    let decider = from_settings(&s).unwrap();
    let asked = ask_request(decider.as_ref(), &w, &scope_decision(), &request_line("question", "keep"), 1_000_000);
    assert_eq!(decisions_of(&asked), ["invalid_reply", "invalid_reply", "approve"]);
    assert_eq!(asked.lines[2]["fallback"], "invalid_reply_default");
    assert_eq!((asked.cost_micro_usd, asked.model_calls), (200_000, 2));
    assert_eq!(asked.result.unwrap().answer, "keep");
    assert_eq!(record_dirs(&w.run.join("decider")), 2);
}

#[test]
fn an_invalid_reply_under_stop_escalates_and_names_why() {
    let w = world("ask-invalid-stop", &invalid());
    let s = settings(&w, json!({"retries": 0, "escalate": "stop"}));
    let decider = from_settings(&s).unwrap();
    let asked = ask_request(decider.as_ref(), &w, &scope_decision(), &request_line("question", "keep"), 1_000_000);
    assert_eq!(decisions_of(&asked), ["escalate"]);
    assert!(asked.result.unwrap_err().contains(&format!("invalid decider reply after 1 attempts: {NO_ANSWER_WHY}")));
}

#[test]
fn a_finding_that_escalates_under_final_takes_the_best_answer_else_its_default() {
    for (extra, want) in [(r#","best":"fix it""#, "fix it"), ("", "defer")] {
        let w = world("ask-finding-final", &answer_reply("escalate", extra));
        let s = settings(&w, json!({}));
        let decider = from_settings(&s).unwrap();
        let asked = ask_request(decider.as_ref(), &w, &scope_decision(), &request_line("finding", "defer"), 1_000_000);
        assert_eq!(decisions_of(&asked), ["escalation_suppressed", "approve"]);
        assert_eq!(asked.lines[1]["suppressed"], true);
        assert_eq!(asked.lines[0]["class"], "finding");
        assert_eq!(asked.result.unwrap().answer, want);
    }
}

#[test]
fn an_escalation_without_a_default_to_take_is_an_error() {
    let w = world("ask-no-default", &answer_reply("escalate", ""));
    let s = settings(&w, json!({}));
    let decider = from_settings(&s).unwrap();
    let line = json!({"type": "question", "id": "q1", "question": {}});
    let request = crate::loom_run::answers::parse_requests(&line.to_string()).unwrap().remove(0);
    let asked = ask_request(decider.as_ref(), &w, &scope_decision(), &request, 1_000_000);
    assert!(asked.result.unwrap_err().contains("names no default"));
}

#[test]
fn a_reject_of_a_question_is_an_invalid_reply_and_never_an_answer() {
    let w = world("ask-reject", &answer_reply("reject", ""));
    let s = settings(&w, json!({"retries": 0, "escalate": "stop"}));
    let decider = from_settings(&s).unwrap();
    let asked = ask_request(decider.as_ref(), &w, &scope_decision(), &request_line("question", "keep"), 1_000_000);
    assert_eq!(decisions_of(&asked), ["escalate"]);
    assert!(asked.result.unwrap_err().contains("a question is decided with approve and an answer, or escalate"));
}

#[test]
fn the_call_cap_counts_the_record_directories_of_requests_too() {
    let w = world("ask-cap", &answer_reply("approve", ""));
    let s = settings(&w, json!({"max_calls": 1}));
    let decider = from_settings(&s).unwrap();
    assert!(ask_request(decider.as_ref(), &w, &scope_decision(), &request_line("question", "keep"), 1_000_000).result.is_ok());
    let again = ask_request(decider.as_ref(), &w, &scope_decision(), &request_line("question", "keep"), 1_000_000);
    assert_eq!(again.result.unwrap_err(), "decider budget spent: 1 decider calls (max_calls)");
}

#[test]
fn only_a_decider_at_a5_answers_requests() {
    let w = world("ask-a4", &answer_reply("approve", ""));
    let mut below = settings(&w, json!({}));
    below["autonomy"] = json!("A4");
    let decider = from_settings(&below).unwrap();
    assert!(!decider.answers_requests());
    let asked = ask_request(decider.as_ref(), &w, &scope_decision(), &request_line("question", "keep"), 1_000_000);
    assert!(asked.result.unwrap_err().starts_with("reserved for a person"));
    assert!(from_settings(&settings(&w, json!({}))).unwrap().answers_requests());
}

#[test]
fn the_rule_decider_answers_a_request_with_its_default_and_no_call_and_refuses_one_without() {
    let w = world("ask-rule", "#!/bin/sh\n");
    let s = json!({"autonomy": "A5", "decider": {"kind": "rule", "verdict": "approve"}});
    let decider = from_settings(&s).unwrap();
    let asked = ask_request(decider.as_ref(), &w, &scope_decision(), &request_line("question", "keep"), 0);
    assert_eq!((asked.cost_micro_usd, asked.model_calls), (0, 0));
    assert_eq!(record_dirs(&w.run.join("decider")), 0, "a rule decision counts against no call limit");
    let given = asked.result.unwrap();
    assert_eq!((given.answer.as_str(), given.principal.as_deref(), asked.lines.len()), ("keep", Some("decider:approve"), 1));
    let line = json!({"type": "question", "id": "q1", "question": {}});
    let request = crate::loom_run::answers::parse_requests(&line.to_string()).unwrap().remove(0);
    let asked = ask_request(decider.as_ref(), &w, &scope_decision(), &request, 0);
    assert_eq!(asked.result.unwrap_err(), "the rule decider's reply has no answer");
}
