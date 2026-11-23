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
    install_routing_table(&run);
    World { _dir: dir, run, work, adapter, reply_bin, login }
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

/// The two files the routing reads, copied from the repository beside the run directory's root.
fn install_routing_table(run: &Path) {
    for file in ["driver/examples/sdc/decision-classes.json", routing::LEVELS_FILE] {
        let to = run.join(file);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(repository_root().join(file), to).unwrap();
    }
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
fn the_adapter_of_a_re_drive_is_handed_the_plan_of_the_run_by_the_settings() {
    let w = world("r2-replay-plan", &valid("0.1"));
    let plan = w.run.join("replay/plan.json");
    let s = settings(&w, json!({"replay_plan": plan.display().to_string()}));
    ask(&w, &s, &approve_action(&w), 250_000);
    let env = std::fs::read_to_string(first_record(&w).join("env.txt")).unwrap();
    assert!(env.contains(&format!("SDC_REPLAY={}\n", plan.display())), "{env}");
    for bad in [json!(""), json!("relative/plan.json"), json!(3)] {
        let s = settings(&w, json!({"replay_plan": bad.clone()}));
        assert!(from_settings(&s).is_err(), "{bad}");
    }
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
    let release = json!({"case": "IDEA-1", "kind": "ApproveAction", "action": "release.publish", "decides": "release.publish of case IDEA-1 at revision 7", "subject": "release", "revision": "r1"});
    for level in ["A0", "A1", "A2", "A3", "A4"] {
        s["autonomy"] = json!(level);
        let d = from_settings(&s).unwrap();
        let why = reason(ask_with(d.as_ref(), &w, &release, 250_000, &env_with_path()));
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
fn below_a5_the_decider_answers_only_the_classes_the_table_gives_it_and_under_its_own_principal() {
    let w = world("r2-routing", &valid("0.1"));
    let mut s = settings(&w, json!({}));
    let table = Routing::load(&w.run).unwrap();
    let cases = [("context.approve", ["A1", "A2", "A3", "A4"]), ("roadmap.approve", ["A1", "A2", "A3", "A4"]), ("release.publish", ["A1", "A2", "A3", "A4"]), ("merge.approve", ["A1", "A2", "A3", "A4"])];
    let mut asked = 0;
    for (action, levels) in cases {
        for level in levels {
            s["autonomy"] = json!(level);
            let mut decision = approve_action(&w);
            decision["action"] = json!(action);
            let verdict = ask(&w, &s, &decision, 250_000);
            let takes = table.decider_takes(level, "approval", action);
            match verdict {
                Verdict::Answer { principal, .. } => {
                    assert!(takes, "{level} {action}: answered where the table gives {:?}", table.holder(level, &table.class_of("approval", action)));
                    assert_eq!(principal.as_deref(), Some("decider:model"), "never a person");
                    asked += 1;
                }
                Verdict::CannotDecide { reason } => {
                    assert!(!takes, "{level} {action}: {reason}");
                    assert!(reason.starts_with(&format!("reserved for a person: autonomy {level}")), "{reason}");
                }
            }
        }
    }
    assert_eq!(asked, 5, "context at A2 to A4 and roadmap at A3 and A4");
    assert_eq!(record_dirs(&w.run.join("decider")), asked, "no model call for a class held by someone else");
}

#[test]
fn below_a5_an_escalate_is_never_taken_under_final_and_a_close_is_asked_only_where_the_table_gives_it() {
    let w = world("r2-routing-final", &model(r#"{"decision":"escalate","best":"approve","reasons":["unsure"],"cites":[]}"#, "0.1"));
    context_yaml(&w);
    let mut s = settings(&w, json!({"escalate": "final"}));
    s["autonomy"] = json!("A4");
    let why = reason(ask(&w, &s, &approve_action(&w), 250_000));
    assert!(why.contains("escalat"), "{why}");
    s["autonomy"] = json!("A3");
    assert!(reason(ask(&w, &s, &close_decision(), 250_000)).starts_with("reserved for a person: autonomy A3 gives the class close"));
    let w2 = world("r2-routing-close", &valid("0.1"));
    let mut s = settings(&w2, json!({}));
    s["autonomy"] = json!("A4");
    assert!(matches!(ask(&w2, &s, &close_decision(), 250_000), Verdict::Answer { approve: true, .. }));
}

#[test]
fn at_a5_a_close_through_an_outcome_the_close_class_lists_is_taken_by_rule_without_a_call() {
    let w = world("r2-close-rule", "#!/bin/sh\ntouch \"$(dirname \"$1\")/../started\"\n");
    let mut decision = close_decision();
    decision["outcome"] = json!("operating");
    for s in [settings(&w, json!({})), json!({"autonomy": "A5", "decider": {"kind": "rule", "verdict": "reject"}})] {
        let verdict = ask(&w, &s, &decision, 250_000);
        assert_eq!(verdict, Verdict::Answer { approve: true, principal: Some("rule:autopilot".to_owned()), cost_micro_usd: Some(0), model_calls: 0, reasons: None });
    }
    assert_eq!(record_dirs(&w.run.join("decider")), 0);
    assert!(!w.run.join("started").exists());
    let w = world("r2-close-kept", &valid("0.1"));
    let s = settings(&w, json!({}));
    let kept = close_decision();
    assert!(matches!(ask(&w, &s, &kept, 250_000), Verdict::Answer { principal: Some(ref p), .. } if p == "decider:model"), "an outcome the class does not list is the decider's");
}

#[test]
fn a_table_that_is_missing_or_unusable_is_a_person_s_at_every_level_for_every_class() {
    let w = world("r2-table-broken", &valid("0.1"));
    let file = w.run.join(routing::LEVELS_FILE);
    let good = std::fs::read_to_string(&file).unwrap();
    let broken: [(&str, String); 6] = [
        ("not json", "{".to_owned()),
        ("wrong format", good.replace("sdc.autonomy-levels/1", "sdc.autonomy-levels/2")),
        ("unknown holder", good.replace("\"decider\"", "\"anyone\"")),
        ("level missing", good.replace("\"A3\":", "\"A33\":")),
        ("class missing", good.replace("\"context\": \"decider\", ", "")),
        ("empty", String::new()),
    ];
    let mut s = settings(&w, json!({}));
    for (what, text) in broken {
        std::fs::write(&file, text).unwrap();
        for level in ["A3", "A4", "A5"] {
            s["autonomy"] = json!(level);
            let mut decision = close_decision();
            decision["outcome"] = json!("operating");
            for d in [&approve_action(&w), &decision] {
                let why = reason(ask(&w, &s, d, 250_000));
                assert!(why.starts_with("reserved for a person") && why.contains("a person decides") && why.contains("autonomy-levels.json"), "{what} {level}: {why}");
            }
        }
    }
    std::fs::remove_file(&file).unwrap();
    s["autonomy"] = json!("A4");
    let why = reason(ask(&w, &s, &approve_action(&w), 250_000));
    assert!(why.contains("a person decides") && why.contains("needed above the bindings copy"), "{why}");
    assert_eq!(record_dirs(&w.run.join("decider")), 0, "no model call without a table");
}

#[test]
fn the_table_loads_and_gives_every_level_every_class_of_the_decision_classes_file() {
    let r = Routing::load(&repository_root()).unwrap();
    assert_eq!(r.holder("A3", "close"), Some("person"));
    assert_eq!(r.holder("A4", "close"), Some("decider"));
    assert_eq!(r.holder("A4", "merge"), Some("rule_and_decider"));
    assert_eq!(r.holder("A5", "close"), Some("rule"));
    assert_eq!(r.holder("A5", "nothing"), None);
    assert_eq!(r.holder("A9", "close"), None);
    assert!(r.closes_by_rule("A5", "operating") && !r.closes_by_rule("A5", "kept") && !r.closes_by_rule("A4", "operating"));
    assert!(r.decider_takes("A2", "approval", "context.approve") && !r.decider_takes("A2", "approval", "roadmap.approve"));
    assert!(!r.decider_takes("A4", "approval", "merge.approve"), "a rule with the decider is not the decider alone");
    assert!(!r.decider_takes("A4", "approval", "no.such.action"));
}

/// The cells of the table of `docs/autonomy-levels.md` § 2.2, read from the document.
#[test]
fn the_json_table_equals_the_table_of_the_document() {
    let text = std::fs::read_to_string(repository_root().join("docs/autonomy-levels.md")).unwrap();
    let start = text.find("### 2.2").expect("§ 2.2");
    let section = &text[start..];
    let rows: Vec<Vec<String>> = section
        .lines()
        .filter(|l| l.starts_with('|'))
        .map(|l| l.trim_matches('|').split('|').map(|c| c.trim().to_owned()).collect())
        .filter(|r: &Vec<String>| r.len() == 8)
        .collect();
    assert!(rows.len() > 10, "{rows:?}");
    let class_of_label = |label: &str| -> Option<&'static str> {
        Some(match label {
            "scope-changing answers" => "scope_changing_answer",
            "context" => "context",
            "vision" => "vision",
            "roadmap" => "roadmap",
            "spec acceptance" => "spec_acceptance",
            "story activation" => "story_activation",
            "code review" => "code_review",
            "merge" => "merge",
            "release" => "release",
            "operate: rollback" => "rollback",
            "operate: close" => "close",
            "budget overrun" => "budget_overrun",
            "limit hit" => "limit_hit",
            "escalation" => "escalation",
            _ => return None,
        })
    };
    let holder_of_symbol = |s: &str| -> Option<&'static str> {
        Some(match s {
            "P" => "person",
            "P+" => "advised",
            "D→P" => "decider",
            "D*" => "decider_final",
            "R" | "R, then P" => "rule",
            "R: end" => "rule_final",
            "R∧D→P" => "rule_and_decider",
            "R∧D*" => "rule_and_decider_final",
            "—" => "none",
            _ => return None,
        })
    };
    let r = Routing::load(&repository_root()).unwrap();
    let mut seen = 0;
    for row in &rows {
        let Some(class) = class_of_label(&row[0]) else { continue };
        for (i, level) in ["A0", "A1", "A2", "A3", "A4", "A5"].iter().enumerate() {
            let cell = row[2 + i].split(" (").next().unwrap().trim();
            let holder = holder_of_symbol(cell).unwrap_or_else(|| panic!("{} {level}: the symbol {cell:?} has no holder", row[0]));
            assert_eq!(r.holder(level, class), Some(holder), "{class} at {level}: the document says {cell}");
            seen += 1;
        }
    }
    assert_eq!(seen, 14 * 6, "every class at every level is in the document's table");
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
    assert!(err(json!({"program": "some-program"})).contains("decider.program"), "the decider is not a program of the bindings copy");
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

/// A `claude` that calls no model: it needs the login in its config home, writes the
/// decision and prints the result line.
const STANDIN_CLAUDE: &str = r#"#!/bin/sh
f="$CLAUDE_CONFIG_DIR/.credentials.json"
[ -f "$f" ] || { echo login-absent >&2; exit 9; }
mkdir -p out && printf '%s\n' '{"decision":"approve","reasons":["ok"],"cites":[]}' > out/decision.json
printf '%s\n' '{"type":"result","subtype":"success","is_error":false,"num_turns":1,"total_cost_usd":0,"duration_ms":1,"duration_api_ms":0,"session_id":"s","usage":{"input_tokens":0,"output_tokens":0},"result":"ok","terminal_reason":"completed","structured_output":{"decision":"approve","reasons":["ok"],"cites":[]}}'
"#;

/// The two tests of `driver/tests/questions_check.rs` that started the real claude-cli
/// adapter under the environment the model decider gets (PATH, HOME, the login) were deleted with
/// `decider.sh`; every other test of this file puts a script of its own in the adapter's place.
#[test]
fn the_real_claude_cli_adapter_finds_the_login_under_the_environment_the_decider_gives_it() {
    let w = world("adv-claude-login", "#!/bin/sh\n");
    let real = repository_root().join("bindings/tools/llm-adapter-claude-cli.sh");
    let bin = w.run.join("standin-bin");
    write_exe(&bin.join("claude"), STANDIN_CLAUDE);
    let mut s = settings(&w, json!({"adapter": real.display().to_string(), "retries": 0}));
    s["login_file"] = json!(w.login.display().to_string());
    let env = vec![("PATH".to_owned(), format!("{}:{SYSTEM_PATH}", bin.display()))];
    let decider = from_settings(&s).unwrap();
    let verdict = ask_with(decider.as_ref(), &w, &approve_action(&w), 250_000, &env);
    match verdict {
        Verdict::Answer { approve, .. } => assert!(approve, "{verdict:?}"),
        other => panic!("the real adapter did not find its login: {other:?}"),
    }
    let secret = w.login.display().to_string();
    let rec = first_record(&w);
    for kept in ["decider-stderr.txt", "reply.json", "decide-request.json", "loom-decision.json", "response.json", "status.json", "request.json", "prompt.md"] {
        let text = std::fs::read_to_string(rec.join(kept)).unwrap_or_default();
        assert!(!text.contains(&secret), "{kept} holds the login path");
    }
    // The mutant: no login file in the settings. The same adapter then finds none and the call fails.
    let w = world("adv-claude-nologin", "#!/bin/sh\n");
    write_exe(&w.run.join("standin-bin/claude"), STANDIN_CLAUDE);
    let s = settings(&w, json!({"adapter": real.display().to_string(), "retries": 0}));
    let env = vec![("PATH".to_owned(), format!("{}:{SYSTEM_PATH}", w.run.join("standin-bin").display()))];
    let decider = from_settings(&s).unwrap();
    let verdict = ask_with(decider.as_ref(), &w, &approve_action(&w), 250_000, &env);
    assert!(matches!(verdict, Verdict::CannotDecide { .. }), "a call with no login must not answer: {verdict:?}");
}

/// Not the eight files the case above names, but every file under the run
/// directory after the real adapter ran with a login: none holds the login path.
#[test]
fn no_file_under_the_run_directory_holds_the_login_path() {
    let w = world("adv-claude-login-walk", "#!/bin/sh\n");
    let real = repository_root().join("bindings/tools/llm-adapter-claude-cli.sh");
    let bin = w.run.join("standin-bin");
    write_exe(&bin.join("claude"), STANDIN_CLAUDE);
    let mut s = settings(&w, json!({"adapter": real.display().to_string(), "retries": 0}));
    s["login_file"] = json!(w.login.display().to_string());
    let env = vec![("PATH".to_owned(), format!("{}:{SYSTEM_PATH}", bin.display()))];
    let decider = from_settings(&s).unwrap();
    let verdict = ask_with(decider.as_ref(), &w, &approve_action(&w), 250_000, &env);
    assert!(matches!(verdict, Verdict::Answer { .. }), "{verdict:?}");
    let secret = w.login.display().to_string();
    let mut stack = vec![w.run.clone()];
    let mut holders = Vec::new();
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if std::fs::read(&p).is_ok_and(|b| String::from_utf8_lossy(&b).contains(&secret)) {
                holders.push(p.strip_prefix(&w.run).unwrap().display().to_string());
            }
        }
    }
    assert!(holders.is_empty(), "files that hold the login path: {holders:?}");
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
    let s = json!({"autonomy": "A1", "decider": {"kind": "rule", "verdict": "approve"}});
    let why = reason(ask(&w, &s, &approve_action(&w), 0));
    assert!(why.starts_with("reserved for a person: autonomy A1"), "{why}");
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
fn a_decider_answers_requests_at_a_level_where_the_table_gives_the_class_and_never_below_a3() {
    let w = world("ask-a4", &answer_reply("approve", ""));
    let mut below = settings(&w, json!({}));
    below["autonomy"] = json!("A2");
    let decider = from_settings(&below).unwrap();
    assert!(decider.answers_requests());
    let asked = ask_request(decider.as_ref(), &w, &scope_decision(), &request_line("question", "keep"), 1_000_000);
    let why = asked.result.unwrap_err();
    assert!(why.starts_with("reserved for a person: autonomy A2 gives the class scope_changing_answer"), "{why}");
    assert_eq!((asked.model_calls, record_dirs(&w.run.join("decider"))), (0, 0));
    below["autonomy"] = json!("A4");
    let decider = from_settings(&below).unwrap();
    let asked = ask_request(decider.as_ref(), &w, &scope_decision(), &request_line("question", "keep"), 1_000_000);
    assert_eq!(asked.result.unwrap().principal.as_deref(), Some("decider:model"));
    below["autonomy"] = json!("A0");
    let decider = from_settings(&below).unwrap();
    assert!(!decider.answers_requests());
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

// --- the cycles of the old driver that tested this code (`cycles/suite.yaml`, class d) --------------
//
// Each test is named in `docs/cycles-suite.md` for the entry it holds. The adapter is a script the
// test writes and the texts are invented.

/// An adapter whose n-th start writes the n-th of `replies` as the model's `out/decision.json` (the
/// last one for every later start) and prints a response that costs `cost`.
fn by_call(replies: &[&str], cost: &str) -> String {
    let mut arms = String::new();
    for (i, reply) in replies.iter().enumerate() {
        let pattern = if i + 1 == replies.len() { "*".to_owned() } else { (i + 1).to_string() };
        arms.push_str(&format!("  {pattern}) printf '%s\\n' '{reply}' > \"$d/cwd/out/decision.json\" ;;\n"));
    }
    format!("#!/bin/sh\nd=$(dirname \"$1\")\nn=$(cat \"$d/../count\" 2>/dev/null || echo 0)\nn=$((n + 1))\necho $n > \"$d/../count\"\ncase $n in\n{arms}esac\nprintf '%s\\n' '{}'\n", response(cost))
}

fn put(w: &World, file: &str, text: &str) {
    let path = w.work.join(file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn context_yaml(w: &World) {
    put(w, "context.yaml", "platform:\n  value: a hosted web server\n  status: set\nkind:\n  value: web\nvalid:\n  value: true\n");
}

const FITS: &str = r#"{"decision":"approve","answer":null,"best":null,"reasons":["The context fits the idea."],"cites":["context@taee57460908c89e2#platform.value","context.yaml:1-2","context.valid"]}"#;
const NOPE: &str = r#"{"decision":"approve","answer":null,"best":null,"reasons":["Cites what is not there."],"cites":["context.nope"]}"#;

fn approved(cost_micro_usd: u64, model_calls: u64) -> Verdict {
    Verdict::Answer { approve: true, principal: Some("decider:model".to_owned()), cost_micro_usd: Some(cost_micro_usd), model_calls, reasons: None }
}

fn prompts(w: &World) -> Vec<String> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(w.run.join("decider")).unwrap().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    dirs.iter().map(|d| std::fs::read_to_string(d.join("prompt.md")).unwrap()).collect()
}

#[test]
fn the_model_of_the_settings_reaches_the_call_and_the_answers_and_the_principal_is_decider_model() {
    let reply = r#"{"adapter":"claude-cli","cost_usd":0.25,"model":"claude-opus-5-5"}"#;
    let w = world("p9-model-name", &script(Some(APPROVE), reply, ""));
    let verdict = ask(&w, &settings(&w, json!({})), &approve_action(&w), 250_000);
    assert_eq!(verdict, approved(250_000, 1));
    let rec = first_record(&w);
    assert_eq!(json_file(&rec.join("adapter-request.json"))["model"], "claude-opus-5-5");
    let kept = json_file(&rec.join("reply.json"));
    assert_eq!((kept["kind"].as_str(), kept["model"].as_str()), (Some("model"), Some("claude-opus-5-5")));
    // another name in the settings is another name in the request
    let w = world("p9-model-name-other", &script(Some(APPROVE), reply, ""));
    ask(&w, &settings(&w, json!({"model": "claude-haiku-5-5"})), &approve_action(&w), 250_000);
    assert_eq!(json_file(&first_record(&w).join("adapter-request.json"))["model"], "claude-haiku-5-5");
    // a question's answer is a line of decisions.jsonl with the same model and the same principal
    let answer = r#"{"decision":"approve","answer":"the billing clerk","best":null,"reasons":["Fine."],"cites":[]}"#;
    let w = world("p9-model-name-question", &script(Some(answer), reply, ""));
    let decider = from_settings(&settings(&w, json!({}))).unwrap();
    let asked = ask_request(decider.as_ref(), &w, &scope_decision(), &request_line("question", "keep"), 1_000_000);
    assert_eq!((asked.lines[0]["principal"].as_str(), asked.lines[0]["model"].as_str()), (Some("decider:model"), Some("claude-opus-5-5")));
    assert_eq!(asked.result.unwrap().principal.as_deref(), Some("decider:model"));
}

#[test]
fn a_question_that_escalates_under_final_takes_its_default_and_the_log_says_suppressed() {
    let escalate = r#"{"decision":"escalate","answer":null,"best":null,"reasons":["The claim questions.settled is false."],"cites":[]}"#;
    let w = world("p9-scope-final", &model(escalate, "0.1"));
    let decider = from_settings(&settings(&w, json!({"escalate": "final"}))).unwrap();
    let asked = ask_request(decider.as_ref(), &w, &scope_decision(), &request_line("question", "keep"), 1_000_000);
    assert_eq!(decisions_of(&asked), ["escalation_suppressed", "approve"]);
    assert_eq!((asked.lines[1]["suppressed"].as_bool(), asked.lines[1]["question"].as_str(), asked.lines[1]["class"].as_str()), (Some(true), Some("q1"), Some("scope_changing_answer")));
    assert_eq!(asked.lines[1]["principal"], "decider:model");
    let given = asked.result.unwrap();
    assert_eq!((given.answer.as_str(), given.principal.as_deref()), ("keep", Some("decider:model")));
    assert_eq!(record_dirs(&w.run.join("decider")), 1);
}

#[test]
fn an_entry_a_line_range_and_an_id_of_the_context_resolve_and_the_first_reply_is_taken() {
    let w = world("p9-cite-address", &model(FITS, "0.1"));
    context_yaml(&w);
    let verdict = ask(&w, &settings(&w, json!({})), &approve_action(&w), 250_000);
    assert_eq!(verdict, approved(100_000, 1));
    assert_eq!(record_dirs(&w.run.join("decider")), 1);
    let kept = json_file(&first_record(&w).join("reply.json"));
    assert_eq!(kept["valid"], true);
    let forms: Vec<&str> = kept["citations"].as_array().unwrap().iter().map(|c| c["form"].as_str().unwrap()).collect();
    assert_eq!(forms, ["artifact_revision", "path_line", "id"]);
    assert_eq!(kept["citations"][0]["value"], json!({"artifact": "context", "revision": "taee57460908c89e2", "entry": "platform.value"}));
    assert_eq!(kept["citations"][1]["value"], json!({"path": "work/context.yaml", "line": 1, "line_end": 2}));
}

#[test]
fn a_cite_that_does_not_resolve_is_asked_again_with_the_reason_and_the_second_reply_is_taken() {
    let w = world("p9-cite-retry", &by_call(&[NOPE, FITS], "0.1"));
    context_yaml(&w);
    let verdict = ask(&w, &settings(&w, json!({})), &approve_action(&w), 900_000);
    assert_eq!(verdict, approved(200_000, 2));
    assert_eq!(record_dirs(&w.run.join("decider")), 2);
    let dir = |name: &str| w.run.join("decider").join(name);
    assert!(json_file(&dir("001-context.approve/decide-request.json")).get("retry").is_none());
    let again = json_file(&dir("002-context.approve/decide-request.json"));
    assert_eq!(again["retry"]["attempt"], 2);
    assert!(again["retry"]["invalid_reason"].as_str().unwrap().starts_with("cites what does not resolve: context.nope"), "{again}");
    let first = json_file(&dir("001-context.approve/reply.json"));
    assert_eq!((first["decision"].as_str(), first["valid"].as_bool()), (Some("escalate"), Some(false)));
    assert!(std::fs::read_to_string(dir("002-context.approve/prompt.md")).unwrap().contains("context.nope"), "the second prompt names the refused cite");
}

#[test]
fn three_invalid_replies_to_an_approval_end_cannot_decide_naming_the_last_reason_after_three_calls() {
    let w = world("p9-invalid-approval", &model(NOPE, "0.05"));
    context_yaml(&w);
    let why = reason(ask(&w, &settings(&w, json!({})), &approve_action(&w), 900_000));
    assert!(why.contains("invalid decider reply after 3 attempts: cites what does not resolve: context.nope"), "{why}");
    assert_eq!(record_dirs(&w.run.join("decider")), 3, "the default is two retries");
}

#[test]
fn three_invalid_replies_to_an_approval_whose_checks_are_true_stay_cannot_decide_and_are_never_approved_on_the_checks() {
    // The driver took `approved_on_checks` here. The runner has no claim values to check, and an
    // approval nobody validly gave is not recorded: the run stops for a person.
    let w = world("p9-invalid-approval-checks", &model(NOPE, "0.05"));
    context_yaml(&w);
    let verdict = ask(&w, &settings(&w, json!({})), &approve_action(&w), 900_000);
    assert!(matches!(verdict, Verdict::CannotDecide { .. }), "{verdict:?}");
    assert!(!matches!(verdict, Verdict::Answer { .. }));
    assert_eq!(record_dirs(&w.run.join("decider")), 3);
}

#[test]
fn three_invalid_replies_to_a_question_take_its_default_as_invalid_reply_default() {
    let bad = r#"{"decision":"approve","answer":"fixture","best":null,"reasons":["Cites what is not there."],"cites":["context.idea_excerpt"]}"#;
    let w = world("p9-invalid-scope", &model(bad, "0.05"));
    let decider = from_settings(&settings(&w, json!({}))).unwrap();
    let asked = ask_request(decider.as_ref(), &w, &scope_decision(), &request_line("question", "keep"), 1_000_000);
    assert_eq!(decisions_of(&asked), ["invalid_reply", "invalid_reply", "invalid_reply", "approve"]);
    assert_eq!((asked.lines[3]["fallback"].as_str(), asked.lines[3]["attempts"].as_u64()), (Some("invalid_reply_default"), Some(3)));
    assert_eq!(asked.lines[3]["principal"], "decider:model");
    let given = asked.result.unwrap();
    assert_eq!((given.answer.as_str(), given.principal.as_deref()), ("keep", Some("decider:model")));
    assert_eq!(record_dirs(&w.run.join("decider")), 3);
}

#[test]
fn an_escalate_without_best_is_refused_under_final_and_asked_again() {
    let escalate = r#"{"decision":"escalate","answer":null,"best":null,"reasons":["A person should say."],"cites":["context.kind"]}"#;
    let w = world("p9-escalate-nobest", &by_call(&[escalate, FITS], "0.1"));
    context_yaml(&w);
    let verdict = ask(&w, &settings(&w, json!({"escalate": "final"})), &approve_action(&w), 900_000);
    assert_eq!(verdict, approved(200_000, 2));
    let again = json_file(&w.run.join("decider/002-context.approve/decide-request.json"));
    assert_eq!(again["retry"]["invalid_reason"], "an escalate needs best: approve or reject (SDC_DECIDER_ESCALATE=final)");
    let first = prompts(&w).remove(0);
    assert!(first.contains("give `best`, the decision that is then taken"), "the first prompt asks for best: {first}");
}

#[test]
fn a_reject_of_a_question_is_refused_with_its_reason_asked_again_and_answered() {
    let reject = r#"{"decision":"reject","answer":null,"best":null,"reasons":["Neither option fits."],"cites":["d4"]}"#;
    let approve = r#"{"decision":"approve","answer":"the billing clerk, before it is paid","best":null,"reasons":["The idea names only the clerk."],"cites":["d4"]}"#;
    let w = world("p9-question-reject", &by_call(&[reject, approve], "0.1"));
    put(&w, "dissection.yaml", "items:\n- id: d4\n  span: the clerk\n");
    let decider = from_settings(&settings(&w, json!({}))).unwrap();
    let asked = ask_request(decider.as_ref(), &w, &scope_decision(), &request_line("question", "keep"), 1_000_000);
    assert_eq!(decisions_of(&asked), ["invalid_reply", "approve"]);
    assert!(asked.lines[0]["invalid_reason"].as_str().unwrap().contains("a question is decided with approve"), "{}", asked.lines[0]);
    assert_eq!(asked.lines[1]["attempts"], 2);
    let given = asked.result.unwrap();
    assert_eq!((given.answer.as_str(), given.principal.as_deref()), ("the billing clerk, before it is paid", Some("decider:model")));
}

fn close_decision() -> Value {
    json!({"case": "IDEA-1", "kind": "Close", "action": null, "outcome": "kept", "decides": "close case IDEA-1 through kept",
        "command": ["b10x-loom", "tree", "close", "--store", "/nonexistent/store", "--case", "IDEA-1", "--outcome", "kept"]})
}

#[test]
fn a_close_reads_the_published_readme_and_no_aep_file() {
    let w = world("p9-input-close", &valid("0.1"));
    put(&w, "published/README.md", "# The product\n");
    put(&w, "aep/initiative/roadmap.md", "---\nstatus: draft\n---\nThe roadmap.\n");
    put(&w, "idea.yaml", "text: an idea\n");
    let verdict = ask(&w, &settings(&w, json!({})), &close_decision(), 250_000);
    assert!(matches!(verdict, Verdict::Answer { approve: true, .. }), "{verdict:?}");
    let given = first_record(&w).join("cwd/in");
    assert!(given.join("published/README.md").is_file() && given.join("idea.yaml").is_file());
    assert!(!given.join("aep").exists(), "a close is not shown the planning files");
    assert!(prompts(&w)[0].contains("- in/published/README.md"));
}

#[test]
fn a_tree_input_keeps_its_cargo_lock_and_leaves_out_other_lock_files_target_and_git() {
    let w = world("p9-input-lock", &valid("0.1"));
    for file in ["impl/Cargo.lock", "impl/Cargo.toml", "impl/src/lib.rs", "impl/yarn.lock", "impl/target/debug/out", "impl/.git/HEAD"] {
        put(&w, file, "x\n");
    }
    let mut decision = approve_action(&w);
    decision["path"] = json!(w.work.join("impl").display().to_string());
    decision["action"] = json!("merge.approve");
    decision["subject"] = json!("change");
    ask(&w, &settings(&w, json!({})), &decision, 250_000);
    let given = first_record(&w).join("cwd/in/impl");
    for kept in ["Cargo.lock", "Cargo.toml", "src/lib.rs"] {
        assert!(given.join(kept).is_file(), "{kept} is not in in/impl");
    }
    for left_out in ["yarn.lock", "target", ".git"] {
        assert!(!given.join(left_out).exists(), "{left_out} is in in/impl");
    }
}

#[test]
fn the_prompt_of_an_aep_input_says_the_files_belong_to_the_planning_tool() {
    let w = world("p9-input-aep", &valid("0.1"));
    put(&w, "aep/initiative/roadmap.md", "---\nstatus: approved\nrevision: 3\n---\nThe roadmap.\n");
    let mut decision = approve_action(&w);
    decision["action"] = json!("roadmap.approve");
    decision["subject"] = json!("roadmap");
    decision["path"] = json!(w.work.join("aep/initiative/roadmap.md").display().to_string());
    ask(&w, &settings(&w, json!({})), &decision, 250_000);
    assert!(prompts(&w)[0].contains("belong to the planning tool"));
    let w = world("p9-input-no-aep", &valid("0.1"));
    put(&w, "context.yaml", "kind:\n  value: web\n");
    ask(&w, &settings(&w, json!({})), &approve_action(&w), 250_000);
    assert!(!prompts(&w)[0].contains("planning tool"), "no aep input, no note about it");
}

#[test]
fn under_final_every_prompt_calls_the_answer_the_owners_own_and_offers_no_escalation_to_a_person() {
    let w = world("p9-input-final", &valid("0.1"));
    context_yaml(&w);
    let decider = from_settings(&settings(&w, json!({"escalate": "final"}))).unwrap();
    for decision in [approve_action(&w), close_decision()] {
        let verdict = ask_with(decider.as_ref(), &w, &decision, 250_000, &env_with_path());
        assert!(matches!(verdict, Verdict::Answer { approve: true, .. }), "{verdict:?}");
    }
    let wq = world("p9-input-final-question", &answer_reply("approve", ""));
    let decider = from_settings(&settings(&wq, json!({"escalate": "final"}))).unwrap();
    let asked = ask_request(decider.as_ref(), &wq, &scope_decision(), &request_line("question", "keep"), 250_000);
    assert!(asked.result.is_ok(), "{:?}", asked.result);
    let mut all = prompts(&w);
    assert_eq!(all.len(), 2, "an approval and a close");
    all.extend(prompts(&wq));
    for text in &all {
        assert!(text.contains("is the product owner's own decision"), "{text}");
        assert!(!text.contains("Escalate to a person"), "{text}");
    }
    // under stop the prompt offers it and does not say that
    let w = world("p9-input-stop", &valid("0.1"));
    context_yaml(&w);
    ask(&w, &settings(&w, json!({"escalate": "stop"})), &approve_action(&w), 250_000);
    let text = prompts(&w).remove(0);
    assert!(text.contains("Escalate to a person") && !text.contains("is the product owner's own decision"), "{text}");
}

fn compiled_ir(name: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("protocol/compiled").join(format!("{name}.ir.json"))).unwrap()
}

/// What `prepare` leaves for an approval of `action` in a case of the compiled protocol `ir` with the
/// obligations `open` open: the checks the model reads and the prompt's text.
fn checks_and_prompt(tag: &str, ir: &str, action: &str, open: &[&str]) -> (Value, String) {
    let dir = Dir::new(tag);
    let case = dir.dir("case");
    std::fs::write(case.join("ir.json"), compiled_ir(ir)).unwrap();
    let cwd = dir.dir("cwd");
    dir.dir("cwd/in");
    let req = json!({"type": "approval", "case_dir": case.display().to_string(), "action": action, "subject": "x", "revision": "t1", "path": null,
        "position": {"claims": {}, "open_obligations": open, "legitimate_outcomes": []}});
    let renderer = prompt::Renderer { root: Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf() };
    let prepared = inputs::prepare(&req, &case, &cwd, &renderer, true).unwrap();
    let parts = prompt::Parts { req: &req, final_escalate: true, inputs: &prepared.inputs, citable: &prepared.citable, prior_rejections: prepared.prior, has_aep: false, met_after: prepared.met_after };
    (json_file(&cwd.join("in/checks.json")), prompt::prompt(&renderer, &parts).unwrap())
}

#[test]
fn an_obligation_only_a_later_step_meets_is_listed_apart_not_open_and_the_prompt_says_so() {
    let (checks, text) = checks_and_prompt("p9-later-release", "sdc-release", "release.publish", &["health.required", "docs.required"]);
    assert_eq!(checks["open_obligations"], json!(["docs.required"]));
    assert_eq!(checks["met_after_this_action"]["action"], "release.publish");
    assert_eq!(checks["met_after_this_action"]["obligations"], json!(["health.required"]));
    assert!(text.contains("lists what can only be met by a step after release.publish"), "{text}");
    let (checks, text) = checks_and_prompt("p9-later-vision", "sdc-direction", "vision.approve", &["roadmap.required"]);
    assert_eq!(checks["open_obligations"], json!([]));
    assert_eq!(checks["met_after_this_action"]["obligations"], json!(["roadmap.required"]));
    assert!(text.contains("lists what can only be met by a step after vision.approve"), "{text}");
    let (checks, text) = checks_and_prompt("p9-later-none", "sdc-release", "release.publish", &["docs.required"]);
    assert!(checks.get("met_after_this_action").is_none(), "{checks}");
    assert!(!text.contains("a step after release.publish"), "{text}");
}

// --- the files of the decider in the run directory: a program of the run may have put something at any of their names ---

/// A file outside the run that must keep its text, and a link at `at` that points at it.
fn link_to_outside(w: &World, at: &Path) -> PathBuf {
    let name = at.strip_prefix(&w.run).unwrap().display().to_string().replace('/', "-");
    let outside = w.run.parent().unwrap().join(format!("outside-{name}.txt"));
    std::fs::write(&outside, "keep\n").unwrap();
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&outside, at).unwrap();
    outside
}

/// Every link made by `link_to_outside` for `at` still points at an untouched file, and what lies at `at` is a regular file.
fn assert_replaced_not_written_through(links: &[(PathBuf, PathBuf)]) {
    for (at, outside) in links {
        assert_eq!(std::fs::read_to_string(outside).unwrap(), "keep\n", "{} was written through the link at {}", outside.display(), at.display());
        assert!(std::fs::symlink_metadata(at).unwrap().is_file(), "{} is not a regular file", at.display());
    }
}

#[test]
fn the_files_of_a_model_call_replace_a_link_a_program_left_at_their_names() {
    let w = world("r2-links-model", &valid("0.012345"));
    std::fs::write(w.work.join("context.yaml"), "context: x\n").unwrap();
    // the directory made here counts as a record directory: the call that follows is the second and writes in the one named 002
    let rec = w.run.join("decider/002-context.approve");
    let case_dir = w.run.join("decider-cases/IDEA-1.direction");
    let mut at = vec![case_dir.join("case.json"), case_dir.join("bindings.json")];
    for name in ["decide-request.json", "loom-decision.json", "reply-schema.json", "prompt.md", "request.json", "response.json", "status.json", "reply.json", "decider-stderr.txt", "cwd/in/checks.json", "cwd/in/decision.json"] {
        at.push(rec.join(name));
    }
    let links: Vec<(PathBuf, PathBuf)> = at.into_iter().map(|a| (a.clone(), link_to_outside(&w, &a))).collect();
    let dangling = w.run.parent().unwrap().join("outside-dangling-context.txt");
    std::os::unix::fs::symlink(&dangling, rec.join("cwd/in/context.yaml")).unwrap();
    let s = settings(&w, json!({}));
    let verdict = ask(&w, &s, &approve_action(&w), 250_000);
    assert!(matches!(verdict, Verdict::Answer { approve: true, .. }), "{verdict:?}");
    assert_replaced_not_written_through(&links);
    assert!(!dangling.exists(), "a copy of work/context.yaml was written through a link that points at nothing");
    assert!(std::fs::symlink_metadata(rec.join("cwd/in/context.yaml")).unwrap().is_file());
}

#[test]
fn the_files_of_a_rule_decision_replace_a_link_a_program_left_at_their_names() {
    let w = world("r2-links-rule", "#!/bin/sh\n");
    let rec = w.run.join("decider/rule");
    let links: Vec<(PathBuf, PathBuf)> =
        ["decide-request.json", "reply.json", "decider-stderr.txt"].iter().map(|n| rec.join(n)).map(|a| (a.clone(), link_to_outside(&w, &a))).collect();
    let s = json!({"autonomy": "A5", "decider": {"kind": "rule", "verdict": "approve"}});
    let verdict = ask(&w, &s, &approve_action(&w), 0);
    assert!(matches!(verdict, Verdict::Answer { approve: true, .. }), "{verdict:?}");
    assert_replaced_not_written_through(&links);
}

#[test]
fn the_files_of_a_question_replace_a_link_a_program_left_at_their_names() {
    let w = world("ask-links", &answer_reply("approve", ""));
    let rec = w.run.join("decider/002-q1");
    let links: Vec<(PathBuf, PathBuf)> = ["decide-request.json", "loom-decision.json"].iter().map(|n| rec.join(n)).map(|a| (a.clone(), link_to_outside(&w, &a))).collect();
    let decider = from_settings(&settings(&w, json!({}))).unwrap();
    let asked = ask_request(decider.as_ref(), &w, &scope_decision(), &request_line("question", "keep"), 1_000_000);
    assert_eq!(asked.result.unwrap().answer, "take it");
    assert_replaced_not_written_through(&links);
}

#[test]
fn the_request_file_of_a_rule_answer_to_a_question_replaces_a_link_a_program_left_at_its_name() {
    let w = world("ask-links-rule", "#!/bin/sh\n");
    let at = w.run.join("decider/rule/decide-request.json");
    let links = vec![(at.clone(), link_to_outside(&w, &at))];
    let decider = from_settings(&json!({"autonomy": "A5", "decider": {"kind": "rule", "verdict": "approve"}})).unwrap();
    let _ = ask_request(decider.as_ref(), &w, &scope_decision(), &request_line("question", "keep"), 0);
    assert_replaced_not_written_through(&links);
}

#[test]
fn the_status_of_a_call_is_written_past_a_link_the_adapter_left_at_the_old_temporary_name() {
    let w = world("r2-status-link", "#!/bin/sh\n");
    let target = w.run.parent().unwrap().join("outside-status.txt");
    std::fs::write(&target, "keep\n").unwrap();
    write_exe(&w.adapter, &script(Some(APPROVE), &response("0.1"), &format!("ln -s '{}' \"$d/.status.json.tmp\"\n", target.display())));
    let verdict = ask(&w, &settings(&w, json!({})), &approve_action(&w), 250_000);
    assert!(matches!(verdict, Verdict::Answer { approve: true, .. }), "{verdict:?}");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep\n", "the status of the call was written through a link at its temporary name");
    assert_eq!(json_file(&first_record(&w).join("status.json"))["state"], "finished");
}

#[test]
fn the_structured_output_of_a_call_is_copied_past_a_link_a_program_left_at_the_decision_file() {
    let w = world("r2-structured-link", "#!/bin/sh\n");
    write_exe(&w.adapter, &script(None, &response("0.1"), "printf '%s\\n' '{\"decision\":\"approve\"}' > \"$d/structured-output.json\"\n"));
    let target = w.run.parent().unwrap().join("outside-decision.txt");
    std::fs::write(&target, "keep\n").unwrap();
    let rec = w.run.join("decider/002-context.approve");
    std::fs::create_dir_all(rec.join("cwd/out")).unwrap();
    std::os::unix::fs::symlink(&target, rec.join("cwd/out/decision.json")).unwrap();
    let _ = ask(&w, &settings(&w, json!({})), &approve_action(&w), 250_000);
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep\n", "the structured output was copied through a link at the decision file");
    assert!(std::fs::symlink_metadata(rec.join("cwd/out/decision.json")).unwrap().is_file(), "the decision file is not a regular file");
}
