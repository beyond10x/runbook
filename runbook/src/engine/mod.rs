//! The engine: a second implementation of `Runtime` that runs a case tree through Loom's library
//! calls in the daemon's process (`docs/engine.md`). It starts no `b10x-loom`, `loom` or
//! `canon` process; the bound programs of the bindings are started by Loom's runner, as always.

pub mod calls;
mod decision;
pub mod layout;
mod tail;

pub use decision::Parsed;

use crate::runtime::{AnswerFailure, Confinement, EventSink, Finish, Recovery, RunbookInfo, Runtime, RuntimeFailure, StartRequest};
use b10x_loom_tree::model::{DecisionKind, TreeStop};
use b10x_loom_tree::{StoreLock, HOLDER_FILE, MANIFEST};
use decision::{parse_answer, plan, Planned};
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use tail::LogTail;

/// The directory of a run's tree store, inside the run's own directory.
pub const STORE_DIR: &str = "tree";

const POLL: Duration = Duration::from_millis(20);

/// What a run's store on disk is, as a restarted daemon finds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreState {
    /// No tree store of this run.
    Absent,
    /// A tree store that no process holds: nothing runs it.
    Idle,
    /// A tree store that no process holds, with a holder file that names a process that is gone.
    StaleHolder(u32),
    /// Another process holds the store now (the process id is the one it names).
    Held(Option<u32>),
}

struct Answered {
    request_id: String,
    answer: String,
    principal: String,
}

#[derive(Default)]
struct SlotState {
    open: BTreeSet<String>,
    answers: VecDeque<Answered>,
    cancelled: bool,
}

/// What `answer` and `cancel` reach of a live run.
#[derive(Default)]
struct Slot {
    state: Mutex<SlotState>,
    cv: Condvar,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub struct Engine {
    home: PathBuf,
    state_dir: PathBuf,
    levels: Vec<Confinement>,
    runs: Mutex<HashMap<String, Arc<Slot>>>,
    rounds: AtomicU64,
    epoch: u128,
}

struct Registered<'a> {
    engine: &'a Engine,
    run_id: String,
}

impl Drop for Registered<'_> {
    fn drop(&mut self) {
        lock(&self.engine.runs).remove(&self.run_id);
    }
}

fn clean(text: &str) -> String {
    text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect()
}

fn failure(reason: impl Into<String>) -> RuntimeFailure {
    RuntimeFailure { reason: clean(&reason.into()) }
}

impl Engine {
    /// An engine for the runbooks under `home` (a directory with `protocol/` and `bindings/`),
    /// keeping each run's store under that run's directory below `state_dir`. It offers the levels
    /// this host can give: `local`, and `linux` where the process is in a delegated cgroup.
    pub fn new(home: PathBuf, state_dir: PathBuf) -> Engine {
        Engine::with_levels(home, state_dir, &calls::host_levels())
    }

    /// An engine that offers exactly these levels.
    pub fn with_levels(home: PathBuf, state_dir: PathBuf, levels: &[Confinement]) -> Engine {
        let epoch = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        Engine { home, state_dir, levels: levels.to_vec(), runs: Mutex::new(HashMap::new()), rounds: AtomicU64::new(0), epoch }
    }

    pub fn levels(&self) -> &[Confinement] {
        &self.levels
    }

    fn store_of(&self, run_id: &str) -> PathBuf {
        self.state_dir.join("runs").join(run_id).join(STORE_DIR)
    }

    /// What the store of `run_id` is on disk. It takes the store's lock for an instant, as any
    /// Loom command does, and lets go.
    pub fn store_state(&self, run_id: &str) -> StoreState {
        let store = self.store_of(run_id);
        if !store.join(MANIFEST).is_file() {
            return StoreState::Absent;
        }
        let named = std::fs::read_to_string(store.join(HOLDER_FILE)).ok().and_then(|t| t.trim().parse::<u32>().ok());
        match StoreLock::acquire(&store) {
            Ok(held) => {
                drop(held);
                match named {
                    Some(pid) if !Path::new("/proc").join(pid.to_string()).exists() => StoreState::StaleHolder(pid),
                    _ => StoreState::Idle,
                }
            }
            Err(refused) => StoreState::Held(refused.0.holder.and_then(|h| u32::try_from(h).ok())),
        }
    }

    fn slot(&self, run_id: &str) -> Option<Arc<Slot>> {
        lock(&self.runs).get(run_id).cloned()
    }

    fn drive(&self, request: &StartRequest, sink: &dyn EventSink, cancel: &AtomicBool, slot: &Slot) -> Result<Finish, RuntimeFailure> {
        // Everything that can refuse the start comes before anything is written.
        if !self.levels.contains(&request.confinement) {
            return Err(failure(format!(
                "the engine cannot give the confinement level {}; it offers {}",
                request.confinement.as_str(),
                self.levels.iter().map(|l| l.as_str()).collect::<Vec<_>>().join(", ")
            )));
        }
        let resolved = layout::resolve(&self.home, &request.runbook, request.version.as_deref()).map_err(failure)?;
        let runner = calls::runner(request.confinement).map_err(failure)?;
        let store = request.state_dir.join(STORE_DIR);
        let workspace = std::path::absolute(&request.workspace_dir).map_err(|e| failure(format!("{}: {e}", request.workspace_dir.display())))?;
        let resume = store.join(MANIFEST).is_file();
        let idea = if resume || request.intake.is_empty() {
            None
        } else {
            let rel = layout::idea_path(&resolved.bindings).map_err(failure)?.ok_or_else(|| failure("the root bindings place no artifact named idea: the intake has nowhere to go"))?;
            let path = Path::new(&rel);
            if path.is_absolute() || path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                return Err(failure(format!("the idea artifact path {rel:?} leaves the workspace")));
            }
            Some(workspace.join(path))
        };

        if !resume {
            let opening = calls::Opening {
                store: &store,
                workspace: &workspace,
                protocols: &resolved.protocol_dir,
                root: calls::Root::Protocol { id: &resolved.protocol_id, revision: resolved.revision },
                bindings: &resolved.bindings,
                case: &request.run_id,
            };
            calls::open(&opening, || {
                if let Some(path) = &idea {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
                    }
                    std::fs::write(path, layout::idea_document(&request.intake)).map_err(|e| format!("{}: {e}", path.display()))?;
                }
                Ok(())
            })
            .map_err(failure)?;
        }
        let catalog = calls::catalog(&store).map_err(failure)?;

        let mut tail = LogTail::new(&store, sink);
        if resume {
            tail.mark_all_seen().map_err(failure)?;
            sink.started();
        } else {
            sink.started();
            tail.sync_all().map_err(failure)?;
        }

        let settings = calls::run_settings(runner, request.confinement, None);
        loop {
            let ran = {
                let mut observer = tail.observer();
                calls::run(&store, &catalog, &settings, Some(&mut observer), Some(cancel))
            };
            let ran = ran.map_err(failure)?;
            tail.sync_all().map_err(failure)?;
            if let Some(f) = tail.failure.take() {
                return Err(failure(f));
            }
            match ran.stop {
                TreeStop::Terminal => {
                    let status = calls::status(&store, &catalog).map_err(failure)?;
                    let outcome = status.cases.iter().find(|c| c.case == status.root).and_then(|c| c.outcome.clone()).or(ran.detail);
                    return outcome.map(Finish::Outcome).ok_or_else(|| failure("the root case ended with no outcome"));
                }
                TreeStop::BudgetSpent => {
                    return if cancel.load(Ordering::SeqCst) {
                        Ok(Finish::Cancelled)
                    } else {
                        Err(failure(format!("the run spent a budget at case {}: {}", ran.case, ran.detail.unwrap_or_default())))
                    };
                }
                TreeStop::Cancelled => return Ok(Finish::Cancelled),
                TreeStop::Stuck => return Err(failure(format!("the tree is stuck at case {}: no action is useful and nothing waits on a person", ran.case))),
                TreeStop::AwaitingPerson | TreeStop::AwaitingAuthority => {
                    if !self.round(&ran.decisions, request, &store, &catalog, &settings.runner, sink, cancel, slot, &mut tail)? {
                        return Ok(Finish::Cancelled);
                    }
                }
            }
        }
    }

    /// One wait: a request per decision, then the answers, each a library call. `false` when the
    /// run was cancelled while it waited.
    #[allow(clippy::too_many_arguments)]
    fn round(
        &self,
        decisions: &[b10x_loom_tree::model::NextDecision],
        _request: &StartRequest,
        store: &Path,
        catalog: &loom_protocols::ProtocolCatalog,
        runner: &Arc<dyn b10x_loom_tree::TestRunner>,
        sink: &dyn EventSink,
        cancel: &AtomicBool,
        slot: &Slot,
        tail: &mut LogTail<'_>,
    ) -> Result<bool, RuntimeFailure> {
        let round = self.rounds.fetch_add(1, Ordering::SeqCst);
        let mut asked: HashMap<String, Planned> = HashMap::new();
        let mut unanswered: BTreeSet<String> = BTreeSet::new();
        let mut replaced = 0usize;
        let mut stuck = false;
        for (n, d) in decisions.iter().enumerate() {
            let planned = plan(format!("d{:x}-{round}-{n}", self.epoch), d, None);
            stuck |= planned.unanswerable.is_some();
            let id = planned.request.request_id.clone();
            lock(&slot.state).open.insert(id.clone());
            unanswered.insert(id.clone());
            sink.decision_requested(planned.request.clone());
            asked.insert(id, planned);
        }
        // Closing a case through one outcome settles the others of that case.
        let mut closed: BTreeSet<String> = BTreeSet::new();
        while !unanswered.is_empty() {
            let Some(answered) = next_answer(slot, cancel) else { return Ok(false) };
            lock(&slot.state).open.remove(&answered.request_id);
            unanswered.remove(&answered.request_id);
            let Some(planned) = asked.get(&answered.request_id).cloned() else { continue };
            if stuck || planned.unanswerable.is_some() {
                // Nothing can be done with it; the run waits for a cancel.
                unanswered.insert(answered.request_id.clone());
                lock(&slot.state).open.insert(answered.request_id);
                continue;
            }
            let d = &planned.decision;
            let result = if d.kind == DecisionKind::Close && closed.contains(&d.case) {
                Ok(())
            } else {
                parse_answer(&answered.answer).and_then(|parsed| calls::apply(store, catalog, d, &parsed, &answered.principal, runner))
            };
            tail.sync_all().map_err(failure)?;
            if let Some(f) = tail.failure.take() {
                return Err(failure(f));
            }
            match result {
                Ok(()) => {
                    if d.kind == DecisionKind::Close && !closed.contains(&d.case) && parse_answer(&answered.answer).is_ok_and(|p| !p.deny) {
                        closed.insert(d.case.clone());
                    }
                }
                Err(why) => {
                    // The daemon holds the answer as given; the engine asks again, with the refusal.
                    replaced += 1;
                    let again = plan(format!("d{:x}-{round}-{}r{replaced}", self.epoch, answered.request_id.rsplit('-').next().unwrap_or("0")), d, Some(&why));
                    let id = again.request.request_id.clone();
                    lock(&slot.state).open.insert(id.clone());
                    unanswered.insert(id.clone());
                    sink.decision_requested(again.request.clone());
                    asked.insert(id, again);
                }
            }
        }
        if cancel.load(Ordering::SeqCst) {
            return Ok(false);
        }
        sink.resumed();
        Ok(true)
    }
}

fn next_answer(slot: &Slot, cancel: &AtomicBool) -> Option<Answered> {
    let mut st = lock(&slot.state);
    loop {
        if cancel.load(Ordering::SeqCst) || st.cancelled {
            return None;
        }
        if let Some(a) = st.answers.pop_front() {
            return Some(a);
        }
        st = slot.cv.wait_timeout(st, POLL).unwrap_or_else(|e| e.into_inner()).0;
    }
}

impl Runtime for Engine {
    /// A run lives on one thread of one process; a restarted daemon has none, whatever the store
    /// holds. The store stays on disk (`store_state`), and a `start` with the same request goes on
    /// from it. A store that a live process holds is that process's run, not this runtime's.
    fn recover(&self, _run_id: &str) -> Recovery {
        Recovery::Gone
    }

    fn knows_runbook(&self, runbook: &str, version: Option<&str>) -> bool {
        layout::resolve(&self.home, runbook, version).is_ok()
    }

    fn runbooks(&self) -> Vec<RunbookInfo> {
        layout::list(&self.home)
    }

    fn kind(&self) -> &'static str {
        "loom"
    }

    fn start(&self, request: StartRequest, sink: &dyn EventSink, cancel: &AtomicBool) -> Result<Finish, RuntimeFailure> {
        let slot = Arc::new(Slot::default());
        {
            let mut runs = lock(&self.runs);
            if runs.contains_key(&request.run_id) {
                return Err(failure(format!("run {} is live in this engine already", request.run_id)));
            }
            runs.insert(request.run_id.clone(), slot.clone());
        }
        let _registered = Registered { engine: self, run_id: request.run_id.clone() };
        self.drive(&request, sink, cancel, &slot)
    }

    fn answer(&self, run_id: &str, request_id: &str, answer: &str, principal: &str) -> Result<(), AnswerFailure> {
        let slot = self.slot(run_id).ok_or(AnswerFailure::UnknownRun)?;
        let mut st = lock(&slot.state);
        if !st.open.contains(request_id) {
            return Err(AnswerFailure::Refused(format!("run {run_id} waits on no request {request_id}")));
        }
        st.answers.push_back(Answered { request_id: request_id.to_string(), answer: answer.to_string(), principal: principal.to_string() });
        slot.cv.notify_all();
        Ok(())
    }

    fn cancel(&self, run_id: &str) {
        if let Some(slot) = self.slot(run_id) {
            lock(&slot.state).cancelled = true;
            slot.cv.notify_all();
        }
    }
}

