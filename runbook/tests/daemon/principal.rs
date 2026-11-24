//! The principal of the daemon: given at the start, fixed for the process, the only principal the
//! runtime is handed. A label a client types (`answered_by`) is stored and shown and nothing else.

use super::commands::{config, Fx};
use super::http::{exits_within, Server};
use super::PRINCIPAL;
use runbook::commands::{check_principal, Daemon};
use runbook::stub::{AnswerGiven, StubRuntime};
use serde_json::{json, Value};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

const BIN: &str = env!("CARGO_BIN_EXE_runbook");

fn serve(tag: &str, args: &[&str]) -> (Command, std::path::PathBuf) {
    let dir = super::state::scratch(tag);
    let mut cmd = Command::new(BIN);
    cmd.arg("serve").args(["--listen", "127.0.0.1:0"]).arg("--state-dir").arg(dir.join("state")).args(args);
    cmd.env_remove("RUNBOOK_TOKEN").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    crate::isolate(&mut cmd);
    (cmd, dir)
}

#[test]
fn serve_without_a_principal_refuses_the_start_whatever_the_environment_says() {
    let (mut cmd, dir) = serve("principal-none", &[]);
    cmd.env("RUNBOOK_PRINCIPAL", "agent:from-the-environment");
    let (status, out, err) = exits_within(cmd, Duration::from_secs(5));
    assert_eq!(status, Some(1), "{err}");
    assert!(err.starts_with("runbook serve: ") && err.contains("--principal is required"), "{err}");
    assert!(out.is_empty(), "nothing was announced: {out}");
    assert!(!dir.join("state").exists(), "nothing was written");
}

#[test]
fn serve_refuses_a_principal_of_a_bad_form_with_exit_1() {
    let too_long = format!("agent:a{}", "b".repeat(63));
    for bad in ["someone", "person:", "agent:Upper", "robot:x", "person:-lead", "agent:with space", too_long.as_str()] {
        let (cmd, dir) = serve("principal-bad", &["--principal", bad]);
        let (status, out, err) = exits_within(cmd, Duration::from_secs(5));
        assert_eq!(status, Some(1), "{bad}: {err}");
        assert!(err.starts_with("runbook serve: ") && err.contains("not a principal") && err.contains(bad), "{bad}: {err}");
        assert!(out.is_empty() && !dir.join("state").exists(), "{bad}: nothing was announced or written");
    }
}

#[test]
fn the_form_of_a_principal_is_person_or_agent_and_an_id_of_at_most_63_characters() {
    let longest = format!("person:a{}", "b".repeat(62));
    for good in ["person:stand-in", "agent:a", "agent:0", "agent:build-7", "person:a.b_c-d", longest.as_str()] {
        assert_eq!(check_principal(good), Ok(()), "{good}");
    }
    for bad in ["", "person", "person:", "agent:A", "agent:-a", "agent:.a", "agent:a b", "agent:a/b", "Person:a", " agent:a", "agent:a\n", "agent:\u{e4}", format!("{longest}b").as_str()] {
        assert!(check_principal(bad).is_err(), "{bad:?}");
    }
    let dir = super::state::scratch("principal-config");
    let cfg = runbook::commands::Config { principal: "nobody".into(), ..config(&dir) };
    let err = Daemon::start(cfg, Arc::new(StubRuntime::new())).err().expect("a daemon with a bad principal does not start");
    assert!(err.to_string().contains("not a principal"), "{err}");
}

fn given(fx: &Fx) -> Vec<AnswerGiven> {
    fx.stub.answers_given()
}

#[test]
fn an_answer_reaches_the_runtime_with_the_daemons_principal_and_the_views_show_both() {
    let s = Server::start("principal-http", None);
    let run = s.fx.submit("ask");
    s.fx.wait_open_decisions(&run, 1);
    let body = json!({"run_id": run, "request_id": "q1", "answer": "A", "answered_by": "person:someone-else", "principal": "person:another-one"});
    let r = s.post("answer-decision", &body);
    assert_eq!((r.status, r.json()["outcome"].clone()), (202, json!("answered")), "{}", r.text());
    let payload = &r.json()["published"][0]["payload"];
    assert_eq!(payload["answered_by"], "person:someone-else", "the label as typed: {payload}");
    assert_eq!(payload["principal"], PRINCIPAL, "the principal is the daemon's: {payload}");
    assert_eq!(given(&s.fx), vec![AnswerGiven { run_id: run.clone(), request_id: "q1".into(), answer: "A".into(), principal: PRINCIPAL.into() }], "the runtime was given the daemon's principal and never the label");
    let rows = s.view("decisions", &format!("run_id={run}"));
    assert_eq!((rows[0]["answered_by"].clone(), rows[0]["principal"].clone()), (json!("person:someone-else"), json!(PRINCIPAL)), "{}", rows[0]);
    super::openapi::check_row("DecisionsOfRun", &rows[0]);
    let configuration = s.view("configuration", "");
    assert_eq!(configuration[0]["principal"], PRINCIPAL, "{}", configuration[0]);
    super::openapi::check_row("DaemonConfiguration", &configuration[0]);
    s.wait_state(&run, "Ended");
}

#[test]
fn the_principal_survives_in_the_record_and_a_restart_keeps_the_daemons_own() {
    let mut fx = Fx::new("principal-restart");
    let run = fx.submit("ask");
    fx.wait_open_decisions(&run, 1);
    assert_eq!(fx.answer(&run, "q1").name, "answered");
    fx.wait_state(&run, "Ended");
    fx.restart();
    let rows = fx.d().decisions_of_run(&run);
    assert_eq!(rows[0]["principal"], PRINCIPAL, "{}", rows[0]);
    assert_eq!(fx.d().daemon_configuration()[0]["principal"], PRINCIPAL);
}

#[test]
fn a_state_written_before_the_principal_existed_loads_and_the_member_is_absent() {
    let mut fx = Fx::new("principal-old-state");
    let run = fx.submit("ask");
    fx.wait_open_decisions(&run, 1);
    assert_eq!(fx.answer(&run, "q1").name, "answered");
    fx.wait_state(&run, "Ended");
    fx.restart_editing(|journal| {
        let text = std::fs::read_to_string(journal).unwrap();
        let old: Vec<String> = text
            .lines()
            .map(|line| {
                let mut fact: Value = serde_json::from_str(line).unwrap();
                fact.as_object_mut().unwrap().remove("principal");
                fact.to_string()
            })
            .collect();
        assert!(!old.iter().any(|l| l.contains("\"principal\"")), "the rewritten journal has no principal");
        std::fs::write(journal, old.join("\n") + "\n").unwrap();
    });
    let rows = fx.d().decisions_of_run(&run);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["answered_by"], "operator", "{}", rows[0]);
    assert!(rows[0].get("principal").is_none(), "the member is absent for an answer given before: {}", rows[0]);
    assert_eq!(fx.d().daemon_configuration()[0]["principal"], PRINCIPAL, "the configuration row is the process's own");
}
