//! The comparison of two levels, the policy of a daemon built in process, and the agreement of the specification
//! with the handler about where a level came from.

use super::PRINCIPAL;
use runbook::commands::{check_default_confinement, Config, Daemon, Policy, SubmitRun};
use runbook::runtime::{Confinement, ConfinementSource};
use runbook::stub::StubRuntime;
use serde_json::{json, Value};
use std::sync::Arc;

#[test]
fn the_comparison_of_two_levels_is_one_function_with_every_pair_written_out() {
    use Confinement::{Linux, Local};
    assert_eq!([(Local, Local), (Linux, Local), (Linux, Linux), (Local, Linux)].map(|(l, floor)| l.at_least(floor)), [true, true, true, false]);
    let refusal = check_default_confinement(Linux, Local).unwrap_err();
    assert!(refusal.contains("local") && refusal.contains("linux"), "{refusal}");
    assert_eq!(check_default_confinement(Local, Linux), Ok(()));
    assert_eq!(check_default_confinement(Local, Local), Ok(()));
}

fn daemon(dir: &std::path::Path, default: Option<Confinement>) -> Daemon {
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    let cfg = Config { state_dir: dir.join("state"), confinement_floor: Confinement::Local, context_roots: vec![dir.join("ctx")], runbook_dir: None, principal: PRINCIPAL.into() };
    let stub = StubRuntime::new();
    stub.script("quick", runbook::stub::Script::new(vec![runbook::stub::Step::Sleep(60_000)]));
    Daemon::start_with(cfg, Policy { default_confinement: default }, Arc::new(stub)).unwrap()
}

#[test]
fn the_level_and_where_it_came_from_are_read_from_the_record_after_a_restart() {
    let dir = super::state::scratch("dc-restart");
    let d = daemon(&dir, Some(Confinement::Local));
    let o = d.submit_run(SubmitRun { runbook_id: "quick".into(), intake: "x".into(), ..SubmitRun::default() }).unwrap();
    let run = o.run_id.unwrap();
    d.stop();
    drop(d);
    // restarted with another default: the run keeps what it was started with
    let d = daemon(&dir, Some(Confinement::Linux));
    let row = d.run_detail(&run)[0].clone();
    assert_eq!((row["confinement"].clone(), row["confinement_source"].clone()), (json!("local"), json!("daemon-default")), "{row}");
    d.stop();
}

#[test]
fn the_specification_and_the_handler_agree_on_the_sources_and_the_level_is_optional_on_the_wire() {
    let doc = super::openapi::doc();
    let schemas = &doc["components"]["schemas"];
    let sources: Vec<&str> = schemas["sdc.daemon.ConfinementSource"]["enum"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    assert_eq!(sources, [ConfinementSource::Submit, ConfinementSource::DaemonDefault, ConfinementSource::BuiltIn].map(ConfinementSource::as_str));
    let required: Vec<&str> = schemas["sdc.daemon.SubmitRun.Input"]["required"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    assert!(!required.contains(&"confinement"), "confinement is optional: {required:?}");
    let submit = runbook::mcp::tools::list().iter().find(|t| t["name"] == "submit_run").unwrap();
    assert_eq!(submit["inputSchema"]["required"], json!(["runbook_id", "intake"]));
    assert!(submit["inputSchema"]["properties"].get("confinement_source").is_none(), "a client never sends the source");
}
