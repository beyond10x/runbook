//! The slow lane of the engine: the `sdc` stub cycle through the daemon over `runbook::engine`, on
//! the repository's real protocols and bindings. Ignored in the gate lane; `task test:engine-real`
//! runs it with `RUNBOOK_ENGINE_WORK` (a directory outside the repository) and with `PATH` holding
//! the built bound programs (`driver/target/debug`). No model call: every approval is given by this
//! test, as `person:owner`.

use runbook::commands::{AnswerDecision, Config, Daemon, SubmitRun};
use runbook::engine::Engine;
use runbook::runtime::Confinement;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[test]
#[ignore = "slow lane: `task test:engine-real`"]
fn the_stub_cycle_through_the_engine_is_operating() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let base = PathBuf::from(std::env::var_os("RUNBOOK_ENGINE_WORK").expect("RUNBOOK_ENGINE_WORK names a directory outside the repository"));
    assert!(base.is_absolute() && !base.starts_with(&root), "RUNBOOK_ENGINE_WORK {} is inside the repository", base.display());
    let work = base.join(format!("engine-stub-cycle-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(work.join("ctx")).unwrap();
    let state = work.join("state");
    let started = Instant::now();
    let engine = Arc::new(Engine::with_levels(root.clone(), state.clone(), &[Confinement::Local]));
    let cfg = Config { state_dir: state.clone(), confinement_floor: Confinement::Local, context_roots: vec![work.join("ctx")], runbook_dir: None };
    let d = Daemon::start(cfg, engine.clone()).unwrap();
    let idea = std::fs::read_to_string(root.join("driver/examples/sdc/fixtures/idea.yaml")).unwrap();
    let o = d.submit_run(SubmitRun { runbook_id: "sdc".into(), intake: idea, confinement: Some(Confinement::Local), ..SubmitRun::default() }).unwrap();
    assert_eq!(o.name, "submitted", "{o:?}");
    let run = o.run_id.unwrap();
    let state_of = |d: &Daemon| d.run_detail(&run)[0]["state"].as_str().unwrap().to_string();
    let mut answered: BTreeSet<String> = BTreeSet::new();
    let limit = Instant::now() + Duration::from_secs(25 * 60);
    while !matches!(state_of(&d).as_str(), "Ended" | "Cancelled" | "FailedToStart") {
        assert!(Instant::now() < limit, "the run did not end in 25 minutes");
        for q in d.open_decisions_of_run(&run) {
            let id = q["request_id"].as_str().unwrap().to_string();
            if answered.insert(id.clone()) {
                let o = d.answer_decision(AnswerDecision { run_id: run.clone(), request_id: id, answer: "approve".into(), answered_by: "person:owner".into() }).unwrap();
                assert!(!o.is_refusal(), "{o:?}");
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let row = d.run_detail(&run)[0].clone();
    eprintln!("engine stub cycle: {} decisions answered, {:?} wall", answered.len(), started.elapsed());
    assert_eq!((row["state"].as_str(), row["outcome"].as_str()), (Some("Ended"), Some("operating")), "{row}");
    d.stop();

    let store = state.join("runs").join(&run).join("tree");
    let catalog = b10x_loom_tree::stored_catalog(&store).unwrap().expect("the tree keeps its protocols");
    let status = b10x_loom_tree::tree_status(&store, &catalog).unwrap();
    let status_file = work.join("tree-status.json");
    std::fs::write(&status_file, b10x_loom_tree::status_json(&status).to_string()).unwrap();
    let bin = env!("CARGO_BIN_EXE_runbook");
    let summary = work.join("summary-engine.json");
    let s = Command::new(bin).args(["parity", "summarize", "--loom-status"]).arg(&status_file).arg("--product-tree").arg(state.join("workspaces").join(&run)).arg("--output").arg(&summary).output().unwrap();
    eprintln!("parity summarize: exit {:?}: {}{}", s.status.code(), String::from_utf8_lossy(&s.stdout), String::from_utf8_lossy(&s.stderr));
    assert_eq!(s.status.code(), Some(0), "summarize failed");
    let driver = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parity/stub-cycle-driver-summary.json");
    let c = Command::new(bin).args(["parity", "compare", "--product-digests"]).arg(&driver).arg(&summary).output().unwrap();
    eprintln!("parity compare: exit {:?}:\n{}", c.status.code(), String::from_utf8_lossy(&c.stdout));
}
