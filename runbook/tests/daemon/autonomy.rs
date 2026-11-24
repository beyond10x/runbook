//! A run of the daemon that takes an autonomy level and a decider kind: the engine answers a wait through the
//! decider and the routing `runbook run` uses, and a decision the level leaves to a person stays open.
//!
//! The runbook is `tests/fixtures/autonomy-runbook`: a staging step that needs a capability (an approval), a
//! reviewer's step (a person), and a close with two outcomes. The daemon is the binary, on a loopback port,
//! with the Loom runtime and a rule decider (`[deciders.rule]`) that approves.

use super::default_confinement::{live, post, rows, Live};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const RULE: &str = "[deciders.rule]\nverdict = \"approve\"\n";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/autonomy-runbook")
}

fn serve(tag: &str, args: &[&str], file: &str) -> Live {
    serve_home(tag, &fixture(), args, file)
}

fn serve_home(tag: &str, home: &Path, args: &[&str], file: &str) -> Live {
    let mut all = vec!["--runtime", "loom", "--runbook-home", home.to_str().unwrap(), "--min-confinement", "local"];
    all.extend_from_slice(args);
    live(tag, &all, Some(file))
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// The fixture copied below a scratch directory of its own, to be broken.
fn broken_copy(tag: &str) -> PathBuf {
    let home = super::state::scratch(tag).join("home");
    copy_tree(&fixture(), &home);
    home
}

fn submit(l: &Live, extra: Value) -> (u16, Value) {
    let mut body = json!({"runbook_id": "autonomy-demo", "intake": "a note on tea", "confinement": "local"});
    body.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    post(l, &body)
}

fn run_of(body: &Value) -> String {
    assert_eq!(body["outcome"], "submitted", "{body}");
    body["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned()
}

fn row(l: &Live, run: &str) -> Value {
    rows(l, &format!("/daemon/views/run?run_id={run}"))[0].clone()
}

fn until_state(l: &Live, run: &str, states: &[&str]) -> Value {
    let limit = Instant::now() + Duration::from_secs(40);
    loop {
        let r = row(l, run);
        if states.iter().any(|s| r["state"] == *s) {
            return r;
        }
        assert!(Instant::now() < limit, "gave up waiting for {states:?}: {r}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The request id, the state and the label of each decision request, in the order they were made (the view has no action).
fn answers(l: &Live, run: &str) -> Vec<(String, Value, Value)> {
    rows(l, &format!("/daemon/views/decisions?run_id={run}")).iter().map(|r| (r["request_id"].as_str().unwrap_or("").to_owned(), r["state"].clone(), r["answered_by"].clone())).collect()
}

fn open(l: &Live, run: &str) -> Vec<Value> {
    rows(l, &format!("/daemon/views/open-decisions?run_id={run}"))
}

/// The open requests once one for `action` is among them; the run passes through states with none open between two answers.
fn until_open(l: &Live, run: &str, action: &str) -> Vec<Value> {
    let limit = Instant::now() + Duration::from_secs(40);
    loop {
        let now = open(l, run);
        if now.iter().any(|r| r["action"] == action) {
            return now;
        }
        let state = row(l, run);
        assert!(!["Ended", "FailedToStart", "Cancelled"].iter().any(|s| state["state"] == *s), "the run ended with no request for {action}: {state}");
        assert!(Instant::now() < limit, "gave up waiting for a request for {action}: {now:?} {state}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn at_a5_with_the_rule_a_run_goes_to_its_end_with_no_open_request_and_every_answer_names_the_rule() {
    let l = serve("autonomy-a5", &[], RULE);
    let (status, body) = submit(&l, json!({"autonomy": "A5", "decider": "rule"}));
    assert_eq!(status, 202, "{body}");
    let run = run_of(&body);
    let ended = until_state(&l, &run, &["Ended", "FailedToStart", "Cancelled"]);
    assert_eq!((ended["state"].clone(), ended["outcome"].clone()), (json!("Ended"), json!("operating")), "{ended}");
    assert_eq!((ended["autonomy"].clone(), ended["decider"].clone()), (json!("A5"), json!("rule")), "the row shows level and kind as text: {ended}");
    assert!(open(&l, &run).is_empty(), "no request is open");
    let given = answers(&l, &run);
    assert!(given.len() >= 3, "a staging approval, the reviewer's step and a close: {given:?}");
    for (action, state, by) in &given {
        assert_eq!(state, "Answered", "{action}");
        let by = by.as_str().unwrap_or("");
        assert!(by == "rule:approve" || by == "rule:autopilot", "{action} was answered by {by:?}, not by a rule");
    }
    let events = rows(&l, &format!("/daemon/views/events?run_id={run}&after_position=0"));
    assert_eq!((events[0]["record"]["autonomy"].clone(), events[0]["record"]["decider"].clone()), (json!("A5"), json!("rule")), "the start event records level and kind: {:?}", events[0]);
}

#[test]
fn the_same_submit_without_a_level_stops_at_the_first_decision() {
    let l = serve("autonomy-none", &[], RULE);
    let (status, body) = submit(&l, json!({}));
    assert_eq!(status, 202, "{body}");
    let run = run_of(&body);
    let waiting = until_state(&l, &run, &["WaitingOnDecision", "Ended", "FailedToStart", "Cancelled"]);
    assert_eq!(waiting["state"], "WaitingOnDecision", "{waiting}");
    let open = open(&l, &run);
    assert_eq!(open.len(), 1, "{open:?}");
    assert_eq!(open[0]["action"], "notes.stage", "{:?}", open[0]);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(row(&l, &run)["state"], "WaitingOnDecision", "nothing answered it");
    assert!(waiting["autonomy"].is_null() && waiting["decider"].is_null(), "no level, no kind: {waiting}");
}

#[test]
fn at_a3_a_decision_the_level_leaves_to_a_person_stays_open_and_the_others_are_answered() {
    let l = serve("autonomy-a3", &[], RULE);
    let (status, body) = submit(&l, json!({"autonomy": "A3", "decider": "rule"}));
    assert_eq!(status, 202, "{body}");
    let run = run_of(&body);
    let open = until_open(&l, &run, "notes.review");
    assert_eq!(open.len(), 1, "{open:?}");
    assert_eq!(row(&l, &run)["state"], "WaitingOnDecision");
    let given = answers(&l, &run);
    assert_eq!(given.len(), 2, "{given:?}");
    assert_eq!((given[0].1.clone(), given[0].2.clone()), (json!("Answered"), json!("rule:approve")), "the capability approval, asked first, was answered by the rule: {given:?}");
    assert_eq!((given[1].0.as_str(), given[1].1.clone()), (open[0]["request_id"].as_str().unwrap(), json!("Open")), "the open one is the person's step: {given:?}");
}

#[test]
fn a_model_decider_against_a_daemon_with_none_configured_is_refused_before_anything_is_written() {
    let l = serve("autonomy-nomodel", &[], RULE);
    let (status, body) = submit(&l, json!({"autonomy": "A5", "decider": "model"}));
    assert_eq!((status, body["refused"].clone()), (400, json!("bad request")), "{body}");
    let text = body["message"].as_str().unwrap_or("").to_owned();
    assert!(text.contains("model") && text.contains("rule") && text.contains("configured"), "it names the kind asked for and the configured kinds: {text}");
    assert!(!text.contains("scripted"), "and no other kind: {text}");
    assert!(rows(&l, "/daemon/views/runs").is_empty(), "no run is written");
    assert!(!l.dir.join("state/runs").read_dir().map(|mut d| d.next().is_some()).unwrap_or(false), "no run directory either");
}

#[test]
fn at_a4_the_close_stays_a_persons_although_the_level_answers_the_rest() {
    let l = serve("autonomy-a4", &[], RULE);
    let (status, body) = submit(&l, json!({"autonomy": "A4", "decider": "rule"}));
    assert_eq!(status, 202, "{body}");
    let run = run_of(&body);
    let open = until_open(&l, &run, "close");
    assert!(open.iter().all(|r| r["action"] == "close"), "the table gives the close to a person at A4: {open:?}");
    assert_eq!(row(&l, &run)["state"], "WaitingOnDecision");
    let given = answers(&l, &run);
    let answered: Vec<_> = given.iter().filter(|(_, state, _)| state == "Answered").collect();
    assert_eq!(answered.len(), given.len() - open.len(), "every request that is not open was answered: {given:?}");
    assert!(answered.len() >= 2 && answered.iter().all(|(_, _, by)| by == "rule:approve"), "the staging approval and the person's step are the decider's at A4: {given:?}");
}

#[test]
fn a_holder_table_that_cannot_be_read_fails_closed_and_every_decision_waits() {
    for (tag, spoil) in [("garbage", Some("not json")), ("missing", None)] {
        let home = broken_copy(&format!("autonomy-table-{tag}-home"));
        match spoil {
            Some(text) => std::fs::write(home.join("tools/autonomy-levels.json"), text).unwrap(),
            None => std::fs::remove_file(home.join("tools/autonomy-levels.json")).unwrap(),
        }
        let l = serve_home(&format!("autonomy-table-{tag}"), &home, &[], RULE);
        let (status, body) = submit(&l, json!({"autonomy": "A5", "decider": "rule"}));
        assert_eq!(status, 202, "{tag}: {body}");
        let run = run_of(&body);
        let waiting = until_state(&l, &run, &["WaitingOnDecision", "Ended", "FailedToStart", "Cancelled"]);
        assert_eq!(waiting["state"], "WaitingOnDecision", "{tag}: {waiting}");
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(open(&l, &run).len(), 1, "{tag}: the first decision waits");
        assert!(answers(&l, &run).iter().all(|(_, state, _)| state == "Open"), "{tag}: nothing was answered");
    }
}

#[test]
fn a_level_that_is_not_one_is_refused_at_submit_and_writes_nothing() {
    let l = serve("autonomy-unknown", &[], RULE);
    for level in ["A6", "a5", "5", "", "A5 "] {
        let (status, body) = submit(&l, json!({"autonomy": level, "decider": "rule"}));
        assert_eq!((status, body["refused"].clone()), (400, json!("bad request")), "{level:?}: {body}");
    }
    assert!(rows(&l, "/daemon/views/runs").is_empty(), "no run");
    assert!(!l.dir.join("state/runs").read_dir().map(|mut d| d.next().is_some()).unwrap_or(false), "no run directory");
}

#[test]
fn a_decider_that_cannot_decide_leaves_the_decision_open_and_a_client_may_answer_it() {
    let file = "[deciders.model]\nmodel = \"a-model\"\n";
    let l = serve("autonomy-model-stuck", &[], file);
    let (status, body) = submit(&l, json!({"autonomy": "A5", "decider": "model"}));
    assert_eq!(status, 202, "{body}");
    let run = run_of(&body);
    let waiting = until_state(&l, &run, &["WaitingOnDecision", "Ended", "FailedToStart", "Cancelled"]);
    assert_eq!((waiting["state"].clone(), waiting["decider"].clone()), (json!("WaitingOnDecision"), json!("model")), "{waiting}");
    let open = open(&l, &run);
    assert_eq!((open.len(), open[0]["action"].clone()), (1, json!("notes.stage")), "{open:?}");
    let a = send_answer(&l, &run, open[0]["request_id"].as_str().unwrap(), "approve");
    assert_eq!(a["outcome"], "answered", "{a}");
    let given = answers(&l, &run);
    assert_eq!(given[0].2, json!(PRINCIPAL_LABEL), "the answer of a client is under the label it gave: {given:?}");
}

#[test]
fn level_a0_and_a_kind_with_no_level_leave_every_decision_to_a_client() {
    let l = serve("autonomy-a0", &[], RULE);
    for (tag, extra) in [("a0", json!({"autonomy": "A0", "decider": "rule"})), ("kind-only", json!({"decider": "rule"}))] {
        let run = run_of(&submit(&l, extra).1);
        let waiting = until_state(&l, &run, &["WaitingOnDecision", "Ended", "FailedToStart", "Cancelled"]);
        assert_eq!(waiting["state"], "WaitingOnDecision", "{tag}: {waiting}");
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(open(&l, &run).len(), 1, "{tag}: the first decision waits");
        assert!(answers(&l, &run).iter().all(|(_, state, _)| state == "Open"), "{tag}: nothing was answered by a decider");
    }
}

#[test]
fn below_a5_a_class_the_table_gives_to_another_holder_is_not_taken_by_the_decider() {
    for holder in ["decider_final", "rule", "advised"] {
        let home = broken_copy(&format!("autonomy-holder-{holder}"));
        let table = home.join("tools/autonomy-levels.json");
        let text = std::fs::read_to_string(&table).unwrap();
        let a4 = r#""A4": {"staging": "decider", "review": "decider", "close": "person"}"#;
        assert!(text.contains(a4));
        std::fs::write(&table, text.replace(a4, &format!(r#""A4": {{"staging": "decider", "review": "{holder}", "close": "person"}}"#))).unwrap();
        let l = serve_home(&format!("autonomy-holder-{holder}-d"), &home, &[], RULE);
        let run = run_of(&submit(&l, json!({"autonomy": "A4", "decider": "rule"})).1);
        let open = until_open(&l, &run, "notes.review");
        assert!(open.iter().any(|r| r["action"] == "notes.review"), "{holder}: the review step waits for a client: {open:?}");
    }
}

#[test]
fn at_a5_a_close_of_a_case_that_one_outcome_already_closed_is_not_asked_of_the_decider() {
    let l = serve("autonomy-a5-close", &[], RULE);
    let run = run_of(&submit(&l, json!({"autonomy": "A5", "decider": "rule"})).1);
    let ended = until_state(&l, &run, &["Ended", "FailedToStart", "Cancelled"]);
    assert_eq!(ended["state"], "Ended", "{ended}");
    let given = answers(&l, &run);
    let autopilot = given.iter().filter(|(_, _, by)| by == "rule:autopilot").count();
    assert_eq!(autopilot, 1, "{given:?}");
    // `runbook run` asks nothing after one close ran (docs/loom-run-decisions.md): the staging approval, the review and the one close
    assert_eq!(given.len(), 3, "the decider was asked about a close the case no longer needed, and the record says it answered: {given:?}");
}

fn write_exe(path: &Path, text: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A daemon with the model kind whose adapter is a stand-in that writes its own environment and fails; the pairs it saw, and the login path the file names.
fn environment_of_the_model_child(tag: &str) -> (Vec<(String, String)>, String) {
    let (pairs, login, _, _) = model_child_run(tag);
    (pairs, login)
}

/// The same run, kept: the pairs the adapter saw, the login path, the daemon and the run (the run waits on a decision the adapter could not make).
fn model_child_run(tag: &str) -> (Vec<(String, String)>, String, Live, String) {
    let base = super::state::scratch(tag);
    let home = base.join("home");
    copy_tree(&fixture(), &home);
    copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("instructions/decider"), &home.join("instructions/decider"));
    let adapter = base.join("adapter.sh");
    write_exe(&adapter, "#!/bin/sh\nd=$(dirname \"$1\")\nenv > \"$d/env.txt\"\nexit 1\n");
    let login = base.join("login-dir/credentials.json");
    std::fs::create_dir_all(login.parent().unwrap()).unwrap();
    std::fs::write(&login, "{}").unwrap();
    let file = format!("login_file = \"{}\"\n[deciders.model]\nmodel = \"a-model\"\nadapter = \"{}\"\nwithout_claims = true\ntimeout_seconds = 20\n", login.display(), adapter.display());
    std::env::set_var("DAEMON_CANARY", "canary-value");
    let runtime_dir = base.join("xdg");
    std::fs::create_dir_all(&runtime_dir).unwrap();
    std::env::set_var("XDG_RUNTIME_DIR", &runtime_dir);
    let l = serve_home(&format!("{tag}-d"), &home, &[], &file);
    let run = run_of(&submit(&l, json!({"autonomy": "A5", "decider": "model"})).1);
    until_state(&l, &run, &["WaitingOnDecision", "Ended", "FailedToStart", "Cancelled"]);
    let state = l.dir.join("state");
    // the decision is open before the decider's child starts: wait for the file, not for the state
    let limit = Instant::now() + Duration::from_secs(30);
    let found = loop {
        if let Some(path) = find_named(&state, "env.txt") {
            break path;
        }
        assert!(Instant::now() < limit, "the stand-in adapter was never started: {:?} {}", answers(&l, &run), row(&l, &run));
        std::thread::sleep(Duration::from_millis(100));
    };
    std::thread::sleep(Duration::from_millis(300));
    let pairs = std::fs::read_to_string(found).unwrap().lines().filter_map(|l| l.split_once('=')).map(|(k, v)| (k.to_owned(), v.to_owned())).collect();
    (pairs, login.display().to_string(), l, run)
}

fn find_named(dir: &Path, name: &str) -> Option<PathBuf> {
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.file_name().is_some_and(|n| n == name) {
            return Some(path);
        }
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            if let Some(found) = find_named(&path, name) {
                return Some(found);
            }
        }
    }
    None
}

#[test]
fn the_model_child_holds_nothing_of_the_daemons_environment() {
    let (env, _login) = environment_of_the_model_child("autonomy-model-env");
    let keys: Vec<&str> = env.iter().map(|(k, _)| k.as_str()).collect();
    assert!(!keys.contains(&"DAEMON_CANARY"), "a variable of the daemon's own reached the model's adapter: {keys:?}");
    assert!(!keys.iter().any(|k| k.contains("TOKEN") || *k == "RUNBOOK_CONFIG"), "{keys:?}");
    assert!(keys.iter().all(|k| ["PATH", "HOME", "PWD", "SHLVL", "_", "SDC_CLAUDE_CREDENTIALS"].contains(k)), "{keys:?}");
}

#[test]
fn the_model_child_is_given_no_path_to_the_login_file_itself() {
    let (env, login) = environment_of_the_model_child("autonomy-model-login");
    let held: Vec<_> = env.iter().filter(|(_, v)| v.contains(&login)).map(|(k, _)| k.clone()).collect();
    assert!(held.is_empty(), "the real path of the operator's login file is in the environment of the model's adapter ({held:?}); `runbook run` hands it a link in a private directory");
}

#[test]
fn the_login_link_of_a_run_is_in_its_private_directory_and_is_removed_when_the_run_ends() {
    let (env, login, l, run) = model_child_run("autonomy-model-link");
    let link = PathBuf::from(&env.iter().find(|(k, _)| k == "SDC_CLAUDE_CREDENTIALS").expect("the adapter was given no login").1);
    let runtime = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap());
    assert!(link.starts_with(runtime.join("runbook")), "the link is not in the private directory below $XDG_RUNTIME_DIR/runbook: {link:?}");
    assert_eq!(std::fs::read_link(&link).unwrap().display().to_string(), login, "the link does not lead to the login file");
    assert!(!link.starts_with(l.dir.join("state")), "the link is in the daemon's state, which is copied and read by others");
    let (status, _) = {
        let r = super::http::send(l.addr, "POST", "/daemon/commands/cancel-run", &[("Content-Type", "application/json")], Some(json!({"run_id": run}).to_string().as_bytes()));
        (r.status, r.json())
    };
    assert!(status < 300, "cancel-run: {status}");
    until_state(&l, &run, &["Cancelled"]);
    let gone = Instant::now() + Duration::from_secs(10);
    while std::fs::symlink_metadata(&link).is_ok() && Instant::now() < gone {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(std::fs::symlink_metadata(&link).is_err(), "the link is still there after the run ended");
    assert!(!link.parent().unwrap().exists(), "the private directory is still there after the run ended");
    assert!(Path::new(&login).is_file(), "the login file itself was removed");
}

#[test]
fn a_refused_decider_table_ends_the_daemon_and_names_no_value_of_the_file() {
    let files = [
        "[deciders.rule]\nverdict = \"SECRETVALUE1\"\n",
        "[deciders.rule]\nverdict = \"approve\"\nzz = \"SECRETVALUE2\"\n",
        "[deciders.model]\nmodel = \"SECRETVALUE3 x\"\n",
        "[deciders.scripted]\nanswers = \"SECRETVALUE4\"\n",
        "[deciders.rule]\nkind = \"SECRETVALUE5\"\nverdict = \"approve\"\n",
        "[deciders]\nrule = \"SECRETVALUE6\"\n",
        "[deciders.model]\nmodel = \"a-model\"\nadapter = \"/SECRETVALUE7/adapter\"\nmax_budget_usd = \"SECRETVALUE7\"\n",
    ];
    for (n, file) in files.iter().enumerate() {
        let dir = super::state::scratch(&format!("autonomy-table-refused-{n}"));
        std::fs::write(dir.join("runbook.toml"), file).unwrap();
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_runbook"));
        cmd.arg("serve").args(["--principal", super::PRINCIPAL, "--listen", "127.0.0.1:0"]).arg("--state-dir").arg(dir.join("state"));
        cmd.env_remove("RUNBOOK_TOKEN").env("RUNBOOK_CONFIG", dir.join("runbook.toml")).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        let (status, out, err) = super::http::exits_within(cmd, Duration::from_secs(10));
        assert!(status.is_some_and(|s| s != 0), "file {n}: the daemon started with a table it cannot take: {out} {err}");
        assert!(!err.contains("SECRETVALUE") && !out.contains("SECRETVALUE"), "file {n}: a value of the file is in the refusal: {err}");
        assert!(!dir.join("state").exists(), "file {n}: nothing written");
    }
}

const PRINCIPAL_LABEL: &str = "a-client";

fn send_answer(l: &Live, run: &str, request: &str, answer: &str) -> Value {
    let body = json!({"run_id": run, "request_id": request, "answer": answer, "answered_by": PRINCIPAL_LABEL});
    let r = super::http::send(l.addr, "POST", "/daemon/commands/answer-decision", &[("Content-Type", "application/json")], Some(body.to_string().as_bytes()));
    r.json()
}
