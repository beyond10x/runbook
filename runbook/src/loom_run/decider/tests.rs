//! Tests of the decider against a FAKE `decider.sh` (a script the test writes; no model), and one
//! against the real `bindings/tools/decider.sh` in its rule mode.

use super::*;
use crate::assets::testdir::Dir;
use std::os::unix::fs::PermissionsExt;

const SYSTEM_PATH: &str = "/usr/bin:/bin";

struct World {
    _dir: Dir,
    run: PathBuf,
    work: PathBuf,
    program: PathBuf,
    reply_bin: PathBuf,
    login: PathBuf,
}

fn write_exe(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A run directory with `home/`, `work/`, `instructions/decider/` and a fake `bindings/tools/decider.sh`
/// holding `script`.
fn world(tag: &str, script: &str) -> World {
    let dir = Dir::new(tag);
    let run = dir.dir("run");
    dir.dir("run/home");
    let work = dir.dir("run/work");
    dir.dir("run/instructions/decider");
    let program = run.join("bindings/tools/decider.sh");
    write_exe(&program, script);
    let reply_bin = dir.path().join("sdc-reply");
    write_exe(&reply_bin, "#!/bin/sh\n");
    let login = dir.path().join("secret-login-dir/credentials.json");
    std::fs::create_dir_all(login.parent().unwrap()).unwrap();
    std::fs::write(&login, "{}").unwrap();
    World { _dir: dir, run, work, program, reply_bin, login }
}

fn valid(cost: &str) -> String {
    format!(r#"{{"decision":"approve","reasons":["fine"],"cites":[],"valid":true,"adapter":"claude-cli","cost_usd":{cost}}}"#)
}

/// A fake that records its environment and its request beside the request, and prints `reply`.
fn fake(reply: &str) -> String {
    format!("#!/bin/sh\nd=$(dirname \"$2\")\nenv > \"$d/env.txt\"\ncp \"$2\" \"$d/seen.json\"\nprintf '%s\\n' '{reply}'\n")
}

fn settings(w: &World, decider: Value) -> Value {
    let mut base = json!({
        "kind": "model", "model": "claude-opus-5-5", "program": w.program.display().to_string(),
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

fn ask_with(decider: &dyn Decider, w: &World, _settings: &Value, decision: &Value, budget: u64, env: &[(String, String)]) -> Verdict {
    decider.decide(&DecisionRequest { decision, run_dir: &w.run, workspace: &w.work, budget_micro_usd: budget, child_env: env })
}

fn ask(w: &World, settings: &Value, decision: &Value, budget: u64) -> Verdict {
    let decider = from_settings(settings).unwrap();
    ask_with(decider.as_ref(), w, settings, decision, budget, &env_with_path())
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

// --- one test per kind of decisions[] entry -----------------------------------------------------

#[test]
fn approve_action_entry_is_asked_and_answered() {
    let w = world("r2-approve-action", &fake(&valid("0.012345")));
    let s = settings(&w, json!({}));
    let verdict = ask(&w, &s, &approve_action(&w), 250_000);
    assert_eq!(verdict, Verdict::Answer { approve: true, principal: Some("decider:model".to_owned()), cost_micro_usd: Some(12_345), model_calls: 1 });
    let rec = first_record(&w);
    assert!(rec.file_name().unwrap().to_str().unwrap().starts_with("001-context.approve"));
    let seen: Value = serde_json::from_str(&std::fs::read_to_string(rec.join("seen.json")).unwrap()).unwrap();
    assert_eq!(seen["type"], "approval");
    assert_eq!(seen["action"], "context.approve");
    assert_eq!(seen["subject"], "context");
    assert_eq!(seen["revision"], "taee57460908c89e2");
    assert_eq!(seen["path"], "context.yaml", "the path is relative to the workspace");
    assert_eq!(seen["capability"], "owner.approve");
    let case_dir = PathBuf::from(seen["case_dir"].as_str().unwrap());
    let case: Value = serde_json::from_str(&std::fs::read_to_string(case_dir.join("case.json")).unwrap()).unwrap();
    assert_eq!(case["id"], "IDEA-1.direction");
    assert_eq!(case["artifacts"]["context"]["revision"], "taee57460908c89e2");
    assert_eq!(std::fs::read_link(case_dir.join("work")).unwrap(), w.work);
    assert!(rec.join("loom-decision.json").is_file() && rec.join("reply.json").is_file());
}

#[test]
fn approve_kind_entry_without_a_path_is_asked() {
    let w = world("r2-approve-kind", &fake(&valid("0.5")));
    let s = settings(&w, json!({}));
    let mut decision = approve_action(&w);
    decision["kind"] = json!("Approve");
    decision["action"] = json!("release.publish");
    decision["capability"] = json!("release.approve");
    decision.as_object_mut().unwrap().remove("path");
    let verdict = ask(&w, &s, &decision, 1_000_000);
    assert_eq!(verdict, Verdict::Answer { approve: true, principal: Some("decider:model".to_owned()), cost_micro_usd: Some(500_000), model_calls: 1 });
    let seen: Value = serde_json::from_str(&std::fs::read_to_string(first_record(&w).join("seen.json")).unwrap()).unwrap();
    assert_eq!(seen["path"], Value::Null);
}

#[test]
fn a_reject_is_an_answer_that_does_not_approve() {
    let w = world("r2-reject", &fake(r#"{"decision":"reject","reasons":["no"],"cites":[],"valid":true,"cost_usd":0.1}"#));
    let verdict = ask(&w, &settings(&w, json!({})), &approve_action(&w), 250_000);
    assert_eq!(verdict, Verdict::Answer { approve: false, principal: Some("decider:model".to_owned()), cost_micro_usd: Some(100_000), model_calls: 1 });
    let reply = std::fs::read_to_string(first_record(&w).join("reply.json")).unwrap();
    assert!(reply.contains("\"no\""), "the reasons stay in the run directory record");
}

#[test]
fn close_handoff_unknown_and_question_entries_cannot_be_decided() {
    let w = world("r2-kinds", &fake(&valid("0.1")));
    let s = settings(&w, json!({}));
    let mut close = approve_action(&w);
    close["kind"] = json!("Close");
    assert!(reason(ask(&w, &s, &close, 250_000)).contains("kind Close"));
    let mut handoff = approve_action(&w);
    handoff["kind"] = json!("Handoff");
    assert!(reason(ask(&w, &s, &handoff, 250_000)).contains("kind Handoff"));
    let mut unknown = approve_action(&w);
    unknown["kind"] = json!("Wish");
    assert_eq!(reason(ask(&w, &s, &unknown, 250_000)), "kind Wish not known");
    let mut question = approve_action(&w);
    question["action"] = json!("scope.answer");
    assert!(reason(ask(&w, &s, &question, 250_000)).contains("scope.answer is a question"));
    assert!(!w.run.join("decider").exists(), "none of these started the program");
}

#[test]
fn absent_members_are_named_not_guessed() {
    let w = world("r2-absent", &fake(&valid("0.1")));
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
    let w = world("r2-casefile", &fake(&valid("0.1")));
    let store = w.run.join("store");
    std::fs::create_dir_all(&store).unwrap();
    let file = store.join(format!("case-{}.json", hex("IDEA-1.direction")));
    std::fs::write(&file, r#"{"id":"IDEA-1.direction","artifacts":[{"artifact":"requirements","revision":"te23630e115159d67"},{"artifact":"context","revision":"old"}]}"#).unwrap();
    let mut decision = approve_action(&w);
    decision["command"] = json!(["b10x-loom", "tree", "approve", "--store", store.display().to_string()]);
    ask(&w, &settings(&w, json!({})), &decision, 250_000);
    let seen: Value = serde_json::from_str(&std::fs::read_to_string(first_record(&w).join("seen.json")).unwrap()).unwrap();
    let case: Value = serde_json::from_str(&std::fs::read_to_string(Path::new(seen["case_dir"].as_str().unwrap()).join("case.json")).unwrap()).unwrap();
    assert_eq!(case["artifacts"]["requirements"]["revision"], "te23630e115159d67");
    assert_eq!(case["artifacts"]["context"]["revision"], "taee57460908c89e2", "the decision's own revision wins");
}

#[test]
fn without_claims_is_an_explicit_acknowledgement() {
    let w = world("r2-claims", &fake(&valid("0.1")));
    let s = settings(&w, json!({"without_claims": false}));
    let why = reason(ask(&w, &s, &approve_action(&w), 250_000));
    assert!(why.contains("claims of the position are not in Loom's output"), "{why}");
    assert!(!w.run.join("decider").exists());
}

// --- environment, budget, answer parsing, limits ---------------------------------------------------

#[test]
fn the_program_gets_an_environment_built_here_and_nothing_inherited() {
    std::env::set_var("RUNBOOK_R2_MARKER", "leaked");
    std::env::set_var("SDC_LLM_ADAPTER", "leaked-adapter");
    let w = world("r2-env", &fake(&valid("0.1")));
    let s = settings(&w, json!({"adapter": "claude-cli", "effort": "high", "max_turns": 6}));
    ask(&w, &s, &approve_action(&w), 250_000);
    let env = std::fs::read_to_string(first_record(&w).join("env.txt")).unwrap();
    assert!(!env.contains("RUNBOOK_R2_MARKER") && !env.contains("leaked"), "{env}");
    let mut names: Vec<&str> = env.lines().filter_map(|l| l.split('=').next()).filter(|n| !["PWD", "SHLVL", "_", "OLDPWD"].contains(n)).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        ["HOME", "PATH", "SDC_DECIDER_ADAPTER", "SDC_DECIDER_ESCALATE", "SDC_DECIDER_MAX_BUDGET_USD", "SDC_DECIDER_MAX_TURNS", "SDC_LLM_EFFORT_DECIDER", "SDC_LLM_MODEL_DECIDER", "SDC_REPLY"]
    );
    assert!(env.contains(&format!("PATH={SYSTEM_PATH}\n")));
    assert!(env.contains(&format!("HOME={}\n", w.run.join("home").display())));
    assert!(env.contains("SDC_DECIDER_ESCALATE=final\n") && env.contains("SDC_LLM_MODEL_DECIDER=claude-opus-5-5\n"));
    assert!(env.contains("SDC_DECIDER_ADAPTER=claude-cli\n") && env.contains("SDC_DECIDER_MAX_TURNS=6\n"));
    std::env::remove_var("RUNBOOK_R2_MARKER");
    std::env::remove_var("SDC_LLM_ADAPTER");
}

#[test]
fn a_child_environment_without_path_is_refused() {
    let w = world("r2-nopath", &fake(&valid("0.1")));
    let s = settings(&w, json!({}));
    let decider = from_settings(&s).unwrap();
    let why = reason(ask_with(decider.as_ref(), &w, &s, &approve_action(&w), 250_000, &[]));
    assert!(why.contains("no PATH"), "{why}");
}

#[test]
fn the_per_call_budget_is_the_scripts_budget_option() {
    let w = world("r2-budget", &fake(&valid("0.1")));
    let s = settings(&w, json!({}));
    ask(&w, &s, &approve_action(&w), 123_456);
    let env = std::fs::read_to_string(first_record(&w).join("env.txt")).unwrap();
    assert!(env.contains("SDC_DECIDER_MAX_BUDGET_USD=0.123456\n"), "{env}");
    // never more than the decider's own limit (max_budget_usd), even when more is handed
    let w = world("r2-budget-cap", &fake(&valid("0.01")));
    let s = settings(&w, json!({"max_budget_usd": 0.05}));
    let decider = from_settings(&s).unwrap();
    assert_eq!(decider.max_call_micro_usd(), 50_000);
    ask(&w, &s, &approve_action(&w), 900_000);
    let env = std::fs::read_to_string(first_record(&w).join("env.txt")).unwrap();
    assert!(env.contains("SDC_DECIDER_MAX_BUDGET_USD=0.050000\n"), "{env}");
    // the default limit is the driver's 0.25 USD
    assert_eq!(from_settings(&settings(&w, json!({}))).unwrap().max_call_micro_usd(), 250_000);
}

#[test]
fn an_answer_that_is_not_json_is_cannot_decide() {
    let w = world("r2-notjson", "#!/bin/sh\necho 'approved, thanks'\n");
    let why = reason(ask(&w, &settings(&w, json!({})), &approve_action(&w), 250_000));
    assert!(why.contains("no single JSON object"), "{why}");
}

#[test]
fn an_answer_without_a_cost_is_cannot_decide() {
    for reply in [r#"{"decision":"approve","reasons":["x"],"valid":true}"#, r#"{"decision":"approve","reasons":["x"],"valid":true,"cost_usd":null}"#, r#"{"decision":"approve","reasons":["x"],"valid":true,"cost_usd":-1}"#] {
        let w = world("r2-nocost", &fake(reply));
        let why = reason(ask(&w, &settings(&w, json!({})), &approve_action(&w), 250_000));
        assert!(why.contains("has no cost"), "{reply}: {why}");
    }
}

#[test]
fn an_answer_without_a_known_decision_is_cannot_decide() {
    let w = world("r2-nodecision", &fake(r#"{"decision":"maybe","valid":true,"cost_usd":0.1}"#));
    let why = reason(ask(&w, &settings(&w, json!({})), &approve_action(&w), 250_000));
    assert!(why.contains("no decision approve, reject or escalate"), "{why}");
}

#[test]
fn a_non_zero_exit_is_cannot_decide_with_the_reason() {
    let w = world("r2-exit", "#!/bin/sh\necho 'decider.sh: yq is not on PATH' >&2\nexit 127\n");
    let why = reason(ask(&w, &settings(&w, json!({})), &approve_action(&w), 250_000));
    assert!(why.contains("exited 127") && why.contains("yq is not on PATH"), "{why}");
}

#[test]
fn a_program_that_cannot_start_is_cannot_decide() {
    let w = world("r2-nostart", "#!/bin/sh\n");
    std::fs::remove_file(&w.program).unwrap();
    let why = reason(ask(&w, &settings(&w, json!({})), &approve_action(&w), 250_000));
    assert!(why.contains("could not start"), "{why}");
}

#[test]
fn a_program_over_its_time_limit_is_ended_with_its_children() {
    let w = world("r2-timeout", "#!/bin/sh\nsleep 30 &\necho $! > \"$(dirname \"$2\")/child.pid\"\nwait\n");
    let s = settings(&w, json!({"timeout_seconds": 1}));
    let mut d = ProgramDecider::parse(&s, &s["decider"]).unwrap();
    d.timeout = Duration::from_millis(400);
    let started = Instant::now();
    let why = reason(ask_with(&d, &w, &s, &approve_action(&w), 250_000, &env_with_path()));
    assert!(why.contains("over its time limit"), "{why}");
    assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
    let pid = std::fs::read_to_string(first_record(&w).join("child.pid")).unwrap();
    let mut alive = true;
    for _ in 0..50 {
        alive = Command::new("kill").args(["-0", pid.trim()]).stderr(Stdio::null()).status().unwrap().success();
        if !alive {
            break;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    assert!(!alive, "the child of the program was left running");
}

#[test]
fn the_call_limit_stops_the_decider_before_the_next_program_start() {
    let w = world("r2-limit", &fake(&valid("0.1")));
    let s = settings(&w, json!({"max_calls": 2}));
    let decider = from_settings(&s).unwrap();
    let env = env_with_path();
    for _ in 0..2 {
        assert!(matches!(ask_with(decider.as_ref(), &w, &s, &approve_action(&w), 250_000, &env), Verdict::Answer { .. }));
    }
    let why = reason(ask_with(decider.as_ref(), &w, &s, &approve_action(&w), 250_000, &env));
    assert!(why.contains("decider budget spent: 2 decider calls"), "{why}");
    assert_eq!(record_dirs(&w.run.join("decider")), 2);
}

#[test]
fn an_invalid_reply_is_asked_again_with_its_reason_then_cannot_decide() {
    let w = world("r2-invalid", &fake(r#"{"decision":"escalate","valid":false,"invalid_reason":"cites what does not resolve","cost_usd":0.2}"#));
    let s = settings(&w, json!({"retries": 1}));
    let why = reason(ask(&w, &s, &approve_action(&w), 900_000));
    assert!(why.contains("invalid decider reply after 2 attempts: cites what does not resolve"), "{why}");
    assert_eq!(record_dirs(&w.run.join("decider")), 2);
    let second: Value = serde_json::from_str(&std::fs::read_to_string(w.run.join("decider").join("002-context.approve/decide-request.json")).unwrap()).unwrap();
    assert_eq!(second["retry"], json!({"attempt": 2, "invalid_reason": "cites what does not resolve"}));
}

#[test]
fn an_invalid_reply_then_a_valid_one_adds_both_costs() {
    let script = "#!/bin/sh\nd=$(dirname \"$2\")\nif [ -e \"$d/../first\" ]; then\n printf '%s\\n' '{\"decision\":\"approve\",\"reasons\":[\"x\"],\"valid\":true,\"cost_usd\":0.3}'\nelse\n touch \"$d/../first\"\n printf '%s\\n' '{\"decision\":\"escalate\",\"valid\":false,\"invalid_reason\":\"bad\",\"cost_usd\":0.2}'\nfi\n";
    let w = world("r2-invalid-then-valid", script);
    let verdict = ask(&w, &settings(&w, json!({})), &approve_action(&w), 900_000);
    assert_eq!(verdict, Verdict::Answer { approve: true, principal: Some("decider:model".to_owned()), cost_micro_usd: Some(500_000), model_calls: 2 });
}

#[test]
fn an_escalate_takes_best_under_final_and_waits_for_a_person_under_stop() {
    let reply = r#"{"decision":"escalate","best":"approve","reasons":["unsure"],"valid":true,"cost_usd":0.1}"#;
    let w = world("r2-escalate-final", &fake(reply));
    let verdict = ask(&w, &settings(&w, json!({"escalate": "final"})), &approve_action(&w), 250_000);
    assert_eq!(verdict, Verdict::Answer { approve: true, principal: Some("decider:model".to_owned()), cost_micro_usd: Some(100_000), model_calls: 1 });
    let w = world("r2-escalate-stop", &fake(reply));
    let why = reason(ask(&w, &settings(&w, json!({"escalate": "stop"})), &approve_action(&w), 250_000));
    assert!(why.starts_with("reserved for a person: the decider escalated"), "{why}");
}

// --- the autonomy rule ---------------------------------------------------------------------------

#[test]
fn only_autonomy_a5_lets_the_decider_answer() {
    let w = world("r2-autonomy", &fake(&valid("0.1")));
    let mut s = settings(&w, json!({}));
    let decider = from_settings(&s).unwrap();
    assert!(matches!(ask_with(decider.as_ref(), &w, &s, &approve_action(&w), 250_000, &env_with_path()), Verdict::Answer { .. }));
    for level in ["A0", "A1", "A2", "A3", "A4"] {
        s["autonomy"] = json!(level);
        let d = from_settings(&s).unwrap();
        let why = reason(ask_with(d.as_ref(), &w, &s, &approve_action(&w), 250_000, &env_with_path()));
        assert!(why.starts_with(&format!("reserved for a person: autonomy {level}")), "{why}");
    }
    s.as_object_mut().unwrap().remove("autonomy");
    let d = from_settings(&s).unwrap();
    assert!(reason(ask_with(d.as_ref(), &w, &s, &approve_action(&w), 250_000, &env_with_path())).starts_with("reserved for a person: no autonomy level named"));
    s["autonomy"] = json!("A9");
    assert!(from_settings(&s).err().unwrap().contains("autonomy"));
    assert_eq!(record_dirs(&w.run.join("decider")), 1, "only the A5 decision started the program");
}

#[test]
fn the_principal_of_person_steps_follows_the_autonomy_rule() {
    let w = world("r2-principal", &fake(&valid("0.1")));
    let mut s = settings(&w, json!({}));
    assert_eq!(person_step_principal(&s).unwrap().as_deref(), Some("decider:model"));
    s["autonomy"] = json!("A3");
    assert_eq!(person_step_principal(&s).unwrap(), None, "a step reserved for a person keeps its person");
    assert_eq!(person_step_principal(&json!({"autonomy": "A5"})).unwrap(), None);
    assert_eq!(person_step_principal(&json!({"autonomy": "A5", "decider": {"kind": "scripted"}})).unwrap(), None);
    let rule = json!({"autonomy": "A5", "decider": {"kind": "rule", "verdict": "approve", "program": "x"}});
    assert_eq!(person_step_principal(&rule).unwrap().as_deref(), Some("decider:approve"));
}

// --- the settings --------------------------------------------------------------------------------

#[test]
fn settings_are_checked_and_unknown_keys_are_refused_by_name() {
    let w = world("r2-settings", &fake(&valid("0.1")));
    assert!(from_settings(&json!({})).is_ok(), "no decider key: the refusing default");
    let refusing = from_settings(&json!({})).unwrap();
    assert!(reason(ask_with(refusing.as_ref(), &w, &json!({}), &approve_action(&w), 0, &env_with_path())).contains("no decider configured"));
    let err = |decider: Value| from_settings(&settings(&w, decider)).err().expect("refused");
    assert!(err(json!({"colour": "blue"})).contains("decider.colour"));
    assert!(err(json!({"model": ""})).contains("decider.model"));
    assert!(err(json!({"escalate": "sometimes"})).contains("decider.escalate"));
    assert!(err(json!({"effort": "extreme"})).contains("decider.effort"));
    assert!(err(json!({"max_calls": 0})).contains("decider.max_calls"));
    assert!(err(json!({"max_budget_usd": 0})).contains("decider.max_budget_usd"));
    assert!(err(json!({"max_decider_calls": 3})).contains("same limit"));
    assert!(from_settings(&json!({"decider": {"kind": "oracle"}})).err().unwrap().contains("oracle"));
    assert!(from_settings(&json!({"decider": {"model": "m"}})).err().unwrap().contains("decider.kind"));
    assert!(from_settings(&json!({"decider": {"kind": "rule", "program": "p"}})).err().unwrap().contains("decider.verdict"));
    assert!(from_settings(&json!({"decider": {"kind": "rule", "program": "p", "verdict": "approve", "model": "m"}})).err().unwrap().contains("decider.model"));
    let alias = settings(&w, json!({"max_decider_calls": 40}));
    let mut alias = alias;
    alias["decider"].as_object_mut().unwrap().remove("max_calls");
    assert!(from_settings(&alias).is_ok(), "the experiment record's name for the limit is accepted");
}

// --- the login file never leaves the child's environment ------------------------------------------

#[test]
fn the_login_path_reaches_the_program_environment_only_and_is_redacted_from_what_is_kept() {
    let w = world("r2-login", "#!/bin/sh\nd=$(dirname \"$2\")\nenv > \"$d/env.txt\"\necho \"no login ($SDC_CLAUDE_CREDENTIALS) and no ANTHROPIC_API_KEY\" >&2\nexit 127\n");
    let mut s = settings(&w, json!({"adapter": "claude-cli"}));
    s["login_file"] = json!(w.login.display().to_string());
    let why = reason(ask(&w, &s, &approve_action(&w), 250_000));
    let secret = w.login.display().to_string();
    assert!(!why.contains(&secret) && why.contains("<login file>"), "{why}");
    let rec = first_record(&w);
    assert!(std::fs::read_to_string(rec.join("env.txt")).unwrap().contains(&format!("SDC_CLAUDE_CREDENTIALS={secret}\n")));
    for kept in ["decider-stderr.txt", "reply.json", "decide-request.json", "loom-decision.json"] {
        let text = std::fs::read_to_string(rec.join(kept)).unwrap_or_default();
        assert!(!text.contains(&secret), "{kept} holds the login path");
    }
    // without a login file in the settings the variable is not set
    let w = world("r2-nologin", &fake(&valid("0.1")));
    ask(&w, &settings(&w, json!({"adapter": "claude-cli"})), &approve_action(&w), 250_000);
    assert!(!std::fs::read_to_string(first_record(&w).join("env.txt")).unwrap().contains("SDC_CLAUDE_CREDENTIALS"));
}

// --- the rule decider and the real decider.sh --------------------------------------------------------

fn real_decider_sh() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("bindings/tools/decider.sh")
}

#[test]
fn the_real_decider_sh_in_rule_mode_answers_through_this_code() {
    let on_path = |tool: &str| std::env::var("PATH").unwrap_or_default().split(':').any(|d| Path::new(d).join(tool).is_file());
    if !on_path("jq") || !on_path("yq") {
        eprintln!("skipped: decider.sh needs jq and yq on PATH");
        return;
    }
    let w = world("r2-real", "#!/bin/sh\n");
    for (verdict, approve) in [("approve", true), ("reject", false)] {
        let s = json!({"autonomy": "A5", "decider": {"kind": "rule", "verdict": verdict, "program": real_decider_sh().display().to_string()}});
        let decider = from_settings(&s).unwrap();
        assert_eq!(decider.max_call_micro_usd(), 0);
        let env = vec![("PATH".to_owned(), std::env::var("PATH").unwrap())];
        let started = Instant::now();
        let verdict_out = ask_with(decider.as_ref(), &w, &s, &approve_action(&w), 0, &env);
        assert_eq!(verdict_out, Verdict::Answer { approve, principal: Some(format!("decider:{verdict}")), cost_micro_usd: Some(0), model_calls: 0 });
        assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
    }
    let reply: Value = serde_json::from_str(&std::fs::read_to_string(w.run.join("decider/rule/reply.json")).unwrap()).unwrap();
    assert_eq!(reply["adapter"], "rule");
    assert_eq!(record_dirs(&w.run.join("decider")), 0, "a rule decision is no model call and counts against no limit");
}

#[test]
fn the_rule_decider_obeys_the_autonomy_rule_too() {
    let w = world("r2-rule-autonomy", "#!/bin/sh\n");
    let s = json!({"autonomy": "A2", "decider": {"kind": "rule", "verdict": "approve", "program": real_decider_sh().display().to_string()}});
    let why = reason(ask(&w, &s, &approve_action(&w), 0));
    assert!(why.starts_with("reserved for a person: autonomy A2"), "{why}");
}
