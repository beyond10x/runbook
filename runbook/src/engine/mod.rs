//! The engine: a second implementation of `Runtime` that runs a case tree through Loom's library
//! calls in the daemon's process. This is the shape the tests compile against; nothing runs yet.

use crate::runtime::{AnswerFailure, Confinement, EventSink, Finish, Recovery, Runtime, RuntimeFailure, StartRequest};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

/// What a run's store on disk is, as a restarted daemon finds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreState {
    Absent,
    Idle,
    StaleHolder(u32),
    Held(Option<u32>),
}

pub struct Engine {
    levels: Vec<Confinement>,
}

impl Engine {
    pub fn new(home: PathBuf, state_dir: PathBuf) -> Engine {
        Engine::with_levels(home, state_dir, &[Confinement::Local])
    }

    pub fn with_levels(_home: PathBuf, _state_dir: PathBuf, levels: &[Confinement]) -> Engine {
        Engine { levels: levels.to_vec() }
    }

    pub fn levels(&self) -> &[Confinement] {
        &self.levels
    }

    pub fn store_state(&self, _run_id: &str) -> StoreState {
        StoreState::Absent
    }
}

impl Runtime for Engine {
    fn recover(&self, _run_id: &str) -> Recovery {
        Recovery::Gone
    }

    fn knows_runbook(&self, _runbook: &str, _version: Option<&str>) -> bool {
        false
    }

    fn start(&self, _request: StartRequest, _sink: &dyn EventSink, _cancel: &AtomicBool) -> Result<Finish, RuntimeFailure> {
        Err(RuntimeFailure { reason: "the engine is not built".to_string() })
    }

    fn answer(&self, _run_id: &str, _request_id: &str, _answer: &str, _principal: &str) -> Result<(), AnswerFailure> {
        Err(AnswerFailure::UnknownRun)
    }

    fn cancel(&self, _run_id: &str) {}
}
