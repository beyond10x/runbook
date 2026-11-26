//! A daemon run on a route (`docs/model-routes.md`, section 11; `crate::relay`): the run's place on the daemon's
//! relay, its route file, and the two wrappers that hand the route to the run's processes and tell the relay which
//! step is in flight.
//!
//! Every bound program of the run starts with `RUNBOOK_ROUTE_FILE` naming the route file and nothing else of the
//! route ([`RouteEnv`]); the decider gets the same path from the arbiter. No process of the run gets the gateway's
//! address or credential: those stay in the daemon's relay.

use crate::relay::{Relay, RouteFile, RouteTable, Ticket, ROUTE_FILE_VARIABLE};
use crate::runtime::{StartTarget, Started};
use b10x_loom_intake_slice::confinement::{Backend, ChildCommand, ConfinementError, TestExecution, TestRunner};
use b10x_loom_intake_slice::executor::TestCommand;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The name of a run's private directory under `$XDG_RUNTIME_DIR/runbook/`: the daemon's process and the run's
/// own name (the last part of its directory), the name the arbiter gives the directory of a login link.
pub fn private_name(run_dir: &Path) -> String {
    let run: String = run_dir.file_name().and_then(|n| n.to_str()).unwrap_or("run").chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    format!("daemon-{}-{run}", std::process::id())
}

/// A run on the relay: its ticket (the token is void when this is dropped) and its route file (removed with its
/// directory when this is dropped).
pub struct RunRoute {
    // The file goes first, then the token: a process that reads the file after it is gone finds no route.
    file: RouteFile,
    pub ticket: Ticket,
    /// The step in flight, written by [`StepNote`] and read by the relay for the line of a call.
    pub step: Arc<Mutex<Option<String>>>,
    /// The role names the route file holds, in order.
    pub roles: Vec<String>,
}

impl RunRoute {
    /// Takes the run onto `relay` with `table` and writes its route file into the run's private directory.
    pub fn open(relay: &Arc<Relay>, run_id: &str, run_dir: &Path, table: &RouteTable) -> Result<RunRoute, String> {
        let ticket = relay.open(run_id, table.clone())?;
        let file = RouteFile::write(&private_name(run_dir), &ticket, table)?;
        let roles = table.roles().into_iter().map(str::to_owned).collect();
        Ok(RunRoute { file, ticket, step: Arc::new(Mutex::new(None)), roles })
    }

    /// The route file.
    pub fn file(&self) -> &Path {
        self.file.path()
    }
}

/// The runner of a run on a route: every program starts with [`ROUTE_FILE_VARIABLE`] in its environment, the path
/// of the route file.
pub struct RouteEnv {
    inner: Arc<dyn TestRunner>,
    file: PathBuf,
}

impl RouteEnv {
    pub fn wrap(inner: Arc<dyn TestRunner>, file: &Path) -> Arc<dyn TestRunner> {
        Arc::new(RouteEnv { inner, file: file.to_path_buf() })
    }
}

impl TestRunner for RouteEnv {
    fn run(&self, command: &TestCommand, root: &Path) -> Result<TestExecution, ConfinementError> {
        self.inner.run(command, root)
    }

    fn backend(&self) -> Backend {
        self.inner.backend()
    }

    fn label(&self) -> String {
        self.inner.label()
    }

    fn local_reason(&self) -> Option<&str> {
        self.inner.local_reason()
    }

    fn run_child(&self, child: &ChildCommand) -> Result<TestExecution, ConfinementError> {
        let mut environment = child.environment().clone();
        environment.set.insert(ROUTE_FILE_VARIABLE.to_owned(), self.file.display().to_string());
        self.inner.run_child(&child.clone().with_environment(environment))
    }

    fn preflight_child(&self, root: &Path) -> Result<(), ConfinementError> {
        self.inner.preflight_child(root)
    }
}

/// A [`StartTarget`] that keeps the step in flight for the relay (the action of the program, else its case) and
/// tells every start and end on to the target it wraps.
pub struct StepNote {
    inner: Arc<dyn StartTarget>,
    step: Arc<Mutex<Option<String>>>,
}

impl StepNote {
    pub fn wrap(inner: Arc<dyn StartTarget>, step: Arc<Mutex<Option<String>>>) -> Arc<dyn StartTarget> {
        Arc::new(StepNote { inner, step })
    }
}

impl StartTarget for StepNote {
    fn started(&self, start: &Started) {
        if start.role == "program" {
            *self.step.lock().unwrap_or_else(|e| e.into_inner()) = start.action.clone().or_else(|| start.case.clone());
        }
        self.inner.started(start);
    }

    fn ended(&self, start: &Started) {
        if start.role == "program" {
            *self.step.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
        self.inner.ended(start);
    }
}
