//! The engine: a second implementation of `Runtime` that runs a case tree through Loom's library
//! calls in the daemon's process (`docs/engine.md`). It starts no `b10x-loom`, `loom` or
//! `canon` process; the bound programs of the bindings are started by Loom's runner, as always.

pub mod announce;
pub mod arbiter;
pub mod calls;
pub mod copy;
mod decision;
mod gate;
mod hide;
pub mod layout;
pub mod pin;
pub mod products;
pub mod refusal;
pub mod shape;
mod tail;

pub use decision::Parsed;
pub use gate::WallClock;

use crate::caps::{CapPolicy, Caps, Usage};
use crate::runtime::{AnswerFailure, Autonomy, Confinement, EventSink, Finish, Recovery, RunbookInfo, Runtime, RuntimeFailure, StartRequest};
use arbiter::{Arbiter, Judgement, Opening as ArbiterOpening, RunArbiter};
use b10x_loom_tree::model::{DecisionKind, TreeStop};
use b10x_loom_tree::{StoreLock, HOLDER_FILE, MANIFEST};
use decision::{parse_answer, parse_option, plan_with, read_case, Planned};
use hide::{Hide, HidingSink};
use pin::Pin;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use gate::{Cap, Gate, Meter, Stopped};
use tail::LogTail;

/// The directory of a run's tree store, inside the run's own directory.
pub const STORE_DIR: &str = "tree";

/// The kind of the event that names the copy of the runbook a run took at its submit.
pub const PINNED_EVENT: &str = "runbook.pinned";

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
    meter: Mutex<Option<Arc<Meter>>>,
}

/// How a run whose own flag is set ends.
#[derive(Debug, PartialEq, Eq)]
enum Halt {
    Cancelled,
    Wall,
}

/// The daemon's cancel wins over the wall cap: the run ends at the wall cap only when it was not
/// cancelled and the wall cap set the flag.
fn halt_of(cancelled: bool, wall_hit: bool) -> Halt {
    if cancelled || !wall_hit {
        Halt::Cancelled
    } else {
        Halt::Wall
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub struct Engine {
    /// The directories a runbook name is looked up in, in order: the configured ones, then the home.
    homes: Vec<PathBuf>,
    state_dir: PathBuf,
    levels: Vec<Confinement>,
    caps: CapPolicy,
    runs: Mutex<HashMap<String, Arc<Slot>>>,
    rounds: AtomicU64,
    epoch: u128,
    /// The deciders the operator configured; none: no run takes a level or a kind.
    arbiter: Option<Arc<dyn Arbiter>>,
    /// What the wall caps of its runs are held against: the host's clock unless a test gave another.
    wall: WallClock,
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
    pub fn new(home: PathBuf, state_dir: PathBuf, caps: CapPolicy) -> Engine {
        Engine::with_levels(home, state_dir, &calls::host_levels(), caps)
    }

    /// An engine that offers exactly these levels. Every run it starts has the caps of `caps`: there
    /// is no engine without them.
    pub fn with_levels(home: PathBuf, state_dir: PathBuf, levels: &[Confinement], caps: CapPolicy) -> Engine {
        let epoch = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        let absolute = |p: PathBuf| std::path::absolute(&p).unwrap_or(p);
        Engine { homes: vec![absolute(home)], state_dir: absolute(state_dir), levels: levels.to_vec(), caps, runs: Mutex::new(HashMap::new()), rounds: AtomicU64::new(0), epoch, arbiter: None, wall: WallClock::default() }
    }

    /// The deciders that answer the waits of the runs that have an autonomy level and a decider kind
    /// (`arbiter.rs`). Without one, a run that asks for a kind is refused when it starts.
    pub fn with_arbiter(mut self, arbiter: Arc<dyn Arbiter>) -> Engine {
        self.arbiter = Some(arbiter);
        self
    }

    /// The clock the wall caps of this engine's runs are held against, for a test that builds the
    /// engine in process (`WallClock::standing`); without one it is the host's monotonic clock.
    pub fn with_wall_clock(mut self, wall: WallClock) -> Engine {
        self.wall = wall;
        self
    }

    /// The runbook directories of `runbook.toml` (`runbooks.rs`), looked up before the home, in
    /// the order given, each made absolute like the home. The set was accepted by `runbooks::load`
    /// when the daemon started.
    pub fn with_runbook_dirs(mut self, dirs: Vec<PathBuf>) -> Engine {
        let home = self.homes.pop().into_iter().collect::<Vec<_>>();
        self.homes = dirs.into_iter().map(|p| std::path::absolute(&p).unwrap_or(p)).chain(home).collect();
        self
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
        let caps = self.caps.resolve(&request.settings).map_err(failure)?;
        let home = layout::home_of(&self.homes, &request.runbook).map_err(failure)?;
        let in_home = layout::resolve(&home, &request.runbook, request.version.as_deref()).map_err(failure)?;
        let runner = calls::runner(request.confinement).map_err(failure)?;
        let store = request.state_dir.join(STORE_DIR);
        let workspace = std::path::absolute(&request.workspace_dir).map_err(|e| failure(format!("{}: {e}", request.workspace_dir.display())))?;
        let resume = store.join(MANIFEST).is_file();
        // The run takes its runbook now: it reads the copy in its own directory until it ends.
        let (resolved, pin) = if resume {
            let pin = Pin::load(&request.state_dir).map_err(failure)?;
            (in_home, pin)
        } else {
            let (pin, copied) = Pin::take(&home, &request.state_dir, &request.runbook, request.version.as_deref()).map_err(failure)?;
            (copied, Some(pin))
        };
        let pinned = pin.as_ref();
        // What each program reserves, from the bindings the run reads (the copy's members are kept
        // beside it, `budget::seal`); a runbook whose programs could call a model with no budget
        // anywhere is refused here.
        let read_bindings = pinned.map_or_else(|| resolved.bindings.clone(), |p| p.copy.join("bindings").join(format!("{}.yaml", request.runbook)));
        let reserves = crate::budget::reserves(&read_bindings).map_err(failure)?;
        // Every program the runner starts is told to the sink before it starts: the daemon records the step in
        // flight. The actions are read from the bindings the run reads, those of its own copy.
        let runner: Arc<dyn b10x_loom_tree::TestRunner> = match (sink.steps(), resolved.bindings.parent()) {
            (Some(target), Some(dir)) => Arc::new(announce::Announcing::to(runner, target, dir.to_path_buf())),
            _ => runner,
        };
        // A level and a kind: the arbiter answers what the level gives to the decider. A0, or neither: every
        // decision waits for a client, as it always did. A kind this engine has none for ends the run here.
        let terms = &request.terms;
        let mut judge: Option<Box<dyn RunArbiter>> = None;
        let mut tree_bindings = resolved.bindings.clone();
        if let (Some(level), Some(kind)) = (terms.autonomy, terms.decider.as_deref()) {
            if level != Autonomy::A0 {
                let arbiter = self.arbiter.as_ref().ok_or_else(|| failure(format!("the run asks for the decider kind {kind} and this engine has no decider configured")))?;
                if !arbiter.kinds().iter().any(|k| k == kind) {
                    return Err(failure(format!("the decider kind {kind} is not configured in this engine")));
                }
                let opened = arbiter
                    .open(&ArbiterOpening { run_dir: &request.state_dir, workspace: &workspace, bindings: &resolved.bindings, terms, resume })
                    .map_err(failure)?;
                tree_bindings = opened.bindings;
                judge = Some(opened.judge);
            }
        }
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
                bindings: &tree_bindings,
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
            if let Some(p) = pinned {
                sink.record(PINNED_EVENT, p.record());
            }
            tail.sync_all().map_err(failure)?;
        }

        let before = if resume { gate::read_usage(&request.state_dir).unwrap_or_default() } else { Usage::default() };
        let meter = Arc::new(Meter::on(before, self.wall.clone()));
        *lock(&slot.meter) = Some(meter.clone());
        let mut gate = Gate::open(caps, &self.state_dir, &request.runbook, &request.run_id, &request.state_dir, &workspace, resume, meter, reserves).map_err(failure)?;
        gate.persist();

        // The run's own flag: the daemon's cancel or the wall cap sets it, and it ends a step that sleeps.
        let local = AtomicBool::new(false);
        let wall_hit = AtomicBool::new(false);
        let done = AtomicBool::new(false);
        let wall_cap = caps.max_wall_seconds;
        let watched = gate.meter.clone();
        let result = std::thread::scope(|scope| {
            scope.spawn(|| {
                while !done.load(Ordering::SeqCst) {
                    if cancel.load(Ordering::SeqCst) || lock(&slot.state).cancelled {
                        local.store(true, Ordering::SeqCst);
                    } else if watched.usage().wall_seconds >= wall_cap {
                        wall_hit.store(true, Ordering::SeqCst);
                        local.store(true, Ordering::SeqCst);
                    }
                    std::thread::sleep(POLL);
                }
            });
            let asking = Asking { workspace: workspace.clone(), bindings: resolved.bindings.parent().map(Path::to_path_buf) };
            let result = self.steps(request, pinned, &store, &catalog, runner, sink, cancel, &local, &wall_hit, slot, &mut tail, &mut gate, &mut judge, &asking);
            done.store(true, Ordering::SeqCst);
            result
        });
        gate.persist();
        result
    }

    /// Whether the daemon cancelled the run (the wall cap sets the run's own flag, not this).
    fn cancelled(cancel: &AtomicBool, slot: &Slot) -> bool {
        cancel.load(Ordering::SeqCst) || lock(&slot.state).cancelled
    }

    /// The run's own flag is set: a cancel by the daemon wins over the wall cap.
    fn halted(cancel: &AtomicBool, wall_hit: &AtomicBool, slot: &Slot, gate: &Gate, sink: &dyn EventSink) -> Finish {
        match halt_of(Self::cancelled(cancel, slot), wall_hit.load(Ordering::SeqCst)) {
            Halt::Cancelled => Finish::Cancelled,
            Halt::Wall => {
                let used = gate.meter.usage().wall_seconds;
                Finish::Outcome(gate.reached(Cap::Wall, &format!("{used} of {} seconds", gate.caps.max_wall_seconds), sink))
            }
        }
    }

    /// The steps of a run, one at a time, each held against the caps before and booked after.
    #[allow(clippy::too_many_arguments)]
    fn steps(
        &self,
        request: &StartRequest,
        pinned: Option<&Pin>,
        store: &Path,
        catalog: &loom_protocols::ProtocolCatalog,
        runner: Arc<dyn b10x_loom_tree::TestRunner>,
        sink: &dyn EventSink,
        cancel: &AtomicBool,
        local: &AtomicBool,
        wall_hit: &AtomicBool,
        slot: &Slot,
        tail: &mut LogTail<'_>,
        gate: &mut Gate,
        judge: &mut Option<Box<dyn RunArbiter>>,
        asking: &Asking,
    ) -> Result<Finish, RuntimeFailure> {
        // The programs of the steps and those the answers of a wait start go through the gate's runner,
        // which reserves the budget of each before it starts; each is booked after.
        let reserving = gate.runner(runner);
        let settings = calls::run_settings(reserving.clone(), request.confinement, Some(1));
        // The digest of the run's copy is read before each round, as it is for a run that takes all the
        // steps of a round in one call: a round is the steps between two waits, each step a `run_tree`
        // of one step, and the check stands at the top of the round's first.
        let mut new_round = true;
        loop {
            if gate.wall_over() {
                wall_hit.store(true, Ordering::SeqCst);
                local.store(true, Ordering::SeqCst);
            }
            if local.load(Ordering::SeqCst) {
                return Ok(Self::halted(cancel, wall_hit, slot, gate, sink));
            }
            if let Some((cap, detail)) = gate.admit().map_err(failure)? {
                return Ok(Finish::Outcome(gate.reached(cap, &detail, sink)));
            }
            if std::mem::take(&mut new_round) {
                check_copy(pinned)?;
            }
            let ran = {
                let mut observer = tail.observer();
                calls::run(store, catalog, &settings, Some(&mut observer), Some(local))
            };
            let booked = gate.book(sink);
            let ran = ran.map_err(failure)?;
            booked.map_err(failure)?;
            tail.sync_all().map_err(failure)?;
            if let Some(f) = tail.failure.take() {
                return Err(failure(f));
            }
            // A program that would have started over what the run has left was not started.
            if let Some(end) = Self::stop_of(gate, cancel, slot, sink) {
                return end;
            }
            match ran.stop {
                TreeStop::Terminal => {
                    check_copy(pinned)?;
                    let status = calls::status(store, catalog).map_err(failure)?;
                    let outcome = status.cases.iter().find(|c| c.case == status.root).and_then(|c| c.outcome.clone()).or(ran.detail);
                    return outcome.map(Finish::Outcome).ok_or_else(|| failure("the root case ended with no outcome"));
                }
                // one step done: the caps are held against the use again before the next
                TreeStop::BudgetSpent if ran.steps > 0 && !local.load(Ordering::SeqCst) => {}
                TreeStop::BudgetSpent => {
                    if local.load(Ordering::SeqCst) {
                        return Ok(Self::halted(cancel, wall_hit, slot, gate, sink));
                    }
                    return Err(failure(format!("the run spent a budget at case {}: {}", ran.case, ran.detail.unwrap_or_default())));
                }
                TreeStop::Cancelled => return Ok(Self::halted(cancel, wall_hit, slot, gate, sink)),
                TreeStop::Stuck => {
                    return Err(failure(match refusal::program_refused(store) {
                        Some(refused) => format!("the tree is stuck at case {}: {}", ran.case, refused.text),
                        None => match refusal::stuck_on(store) {
                            Some(lead) => format!("the tree is stuck at case {}: no action is useful and nothing waits on a person ({lead})", ran.case),
                            None => format!("the tree is stuck at case {}: no action is useful and nothing waits on a person", ran.case),
                        },
                    }));
                }
                TreeStop::AwaitingPerson | TreeStop::AwaitingAuthority => {
                    let waited = Waited { local, cancel, slot, asking };
                    match self.round(&ran, pinned, store, catalog, &reserving, sink, &waited, tail, gate, judge)? {
                        RoundEnd::Resumed => new_round = true,
                        RoundEnd::Halted => return Ok(Self::halted(cancel, wall_hit, slot, gate, sink)),
                        RoundEnd::Ended(end) => return Ok(end),
                    }
                }
            }
        }
    }

    /// How the run ends when a program of the step or answer just taken was not started: the
    /// ledger could not be read (a failure), or the reservation was short (the spend cap, unless the
    /// daemon cancelled the run). `None` when every program started.
    fn stop_of(gate: &Gate, cancel: &AtomicBool, slot: &Slot, sink: &dyn EventSink) -> Option<Result<Finish, RuntimeFailure>> {
        Some(match stop_end(gate.take_stopped(), Self::cancelled(cancel, slot))? {
            StopEnd::Failed(why) => Err(failure(why)),
            StopEnd::Cancelled => Ok(Finish::Cancelled),
            StopEnd::Spend(detail) => Ok(Finish::Outcome(gate.reached(Cap::Spend, &detail, sink))),
        })
    }

    /// One wait: a request per decision, then the answers, each a library call. The run's judge, when it
    /// has one, answers first what its level gives to the decider; the rest waits for a client. A program
    /// an answer starts, a judge's or a client's, goes through `runner`, the gate's: reserved before it
    /// starts, booked after.
    #[allow(clippy::too_many_arguments)]
    fn round(
        &self,
        ran: &calls::Ran,
        pinned: Option<&Pin>,
        store: &Path,
        catalog: &loom_protocols::ProtocolCatalog,
        runner: &Arc<dyn b10x_loom_tree::TestRunner>,
        sink: &dyn EventSink,
        waited: &Waited<'_>,
        tail: &mut LogTail<'_>,
        gate: &mut Gate,
        judge: &mut Option<Box<dyn RunArbiter>>,
    ) -> Result<RoundEnd, RuntimeFailure> {
        let (cancel, slot) = (waited.local, waited.slot);
        let mut wait = Wait {
            round: self.rounds.fetch_add(1, Ordering::SeqCst),
            epoch: self.epoch,
            pinned,
            store,
            catalog,
            runner,
            sink,
            slot,
            daemon_cancel: waited.cancel,
            asked: HashMap::new(),
            unanswered: BTreeSet::new(),
            closed: BTreeSet::new(),
            replaced: 0,
        };
        // Each request reads its case first: an artifact that fails the check ends the run by its cause, and a
        // question member that gave no question is recorded and the request asks the generated one.
        let mut plans: Vec<Planned> = Vec::new();
        for (n, d) in ran.decisions.iter().enumerate() {
            let found = read_case(d, &waited.asking.workspace, waited.asking.bindings.as_deref()).map_err(failure)?;
            if let Some(cause) = &found.unread {
                sink.record(decision::QUESTION_UNREAD, serde_json::json!({"case": d.case, "action": d.action, "cause": cause}));
            }
            plans.push(plan_with(format!("d{:x}-{}-{n}", self.epoch, wait.round), d, None, &found));
        }
        let stuck = plans.iter().any(|p| p.unanswerable.is_some());
        match judge.as_mut() {
            // What the level gives to the decider: judged in the order Loom printed the decisions, each
            // request raised just before it is judged, so that a close of a case that an earlier
            // answer of this wait already closed is never raised (as `runbook run` does not ask it:
            // docs/loom-run-decisions.md). A judgement that is not an answer, and an answer the
            // daemon does not take into its record, leave the request open.
            Some(judge) if !stuck => {
                let printed = ran.document.get("decisions").and_then(Value::as_array);
                for (n, planned) in plans.into_iter().enumerate() {
                    if cancel.load(Ordering::SeqCst) {
                        return Ok(RoundEnd::Halted);
                    }
                    if planned.decision.kind == DecisionKind::Close && wait.closed.contains(&planned.decision.case) {
                        continue;
                    }
                    let id = planned.request.request_id.clone();
                    wait.raise(planned.clone());
                    let Some(printed) = printed.and_then(|p| p.get(n)) else { continue };
                    let Judgement::Answer { answer, by, principal } = judge.judge(printed) else { continue };
                    // A request read from its case takes only an answer that names one of its options (or a free
                    // answer where it takes one); any other judgement is not saved and leaves it open for a person.
                    if planned.request.artifact.as_ref().is_some_and(|a| !a.takes(&planned.request.options, &answer)) {
                        continue;
                    }
                    if !sink.decision_answered(&id, &answer, &by, &principal) {
                        continue;
                    }
                    lock(&slot.state).open.remove(&id);
                    wait.unanswered.remove(&id);
                    if let Some(end) = wait.settle(tail, gate, &planned, &id, &answer, &principal)? {
                        return Ok(RoundEnd::Ended(end));
                    }
                }
            }
            _ => {
                for planned in plans {
                    wait.raise(planned);
                }
            }
        }
        while !wait.unanswered.is_empty() {
            let Some(answered) = next_answer(slot, cancel) else { return Ok(RoundEnd::Halted) };
            lock(&slot.state).open.remove(&answered.request_id);
            wait.unanswered.remove(&answered.request_id);
            let Some(planned) = wait.asked.get(&answered.request_id).cloned() else { continue };
            if stuck || planned.unanswerable.is_some() {
                // Nothing can be done with it; the run waits for a cancel.
                wait.unanswered.insert(answered.request_id.clone());
                lock(&slot.state).open.insert(answered.request_id);
                continue;
            }
            if let Some(end) = wait.settle(tail, gate, &planned, &answered.request_id, &answered.answer, &answered.principal)? {
                return Ok(RoundEnd::Ended(end));
            }
        }
        if cancel.load(Ordering::SeqCst) {
            return Ok(RoundEnd::Halted);
        }
        sink.resumed();
        Ok(RoundEnd::Resumed)
    }
}

/// What one wait holds while its requests are answered.
struct Wait<'a> {
    round: u64,
    epoch: u128,
    pinned: Option<&'a Pin>,
    store: &'a Path,
    catalog: &'a loom_protocols::ProtocolCatalog,
    /// The gate's runner: a program an answer starts is reserved before it starts.
    runner: &'a Arc<dyn b10x_loom_tree::TestRunner>,
    sink: &'a dyn EventSink,
    slot: &'a Slot,
    /// The daemon's cancel, which wins over a spend stop.
    daemon_cancel: &'a AtomicBool,
    asked: HashMap<String, Planned>,
    unanswered: BTreeSet<String>,
    /// Closing a case through one outcome settles the others of that case.
    closed: BTreeSet<String>,
    replaced: usize,
}

impl Wait<'_> {
    /// The run stops on one decision: its request is open, announced and waits for an answer.
    fn raise(&mut self, planned: Planned) {
        let id = planned.request.request_id.clone();
        lock(&self.slot.state).open.insert(id.clone());
        self.unanswered.insert(id.clone());
        self.sink.decision_requested(planned.request.clone());
        self.asked.insert(id, planned);
    }

    /// An answer, from a client or from the run's judge, is one library call, booked against the caps after
    /// it. The daemon holds the answer as given; a call Loom refuses is asked again as a new request that
    /// quotes the refusal. `Some` when the run ends here: a program the answer would have started over what
    /// the run has left was not started.
    fn settle(&mut self, tail: &mut LogTail<'_>, gate: &mut Gate, planned: &Planned, request_id: &str, answer: &str, principal: &str) -> Result<Option<Finish>, RuntimeFailure> {
        let d = &planned.decision;
        let (result, booked) = if d.kind == DecisionKind::Close && self.closed.contains(&d.case) {
            (Ok(()), Ok(()))
        } else {
            // An answer is a library call that reads the bindings and may run a program of them.
            check_copy(self.pinned)?;
            let applied = parsed_answer(planned, answer).and_then(|parsed| calls::apply(self.store, self.catalog, d, &parsed, principal, self.runner));
            (applied, gate.book(self.sink))
        };
        tail.sync_all().map_err(failure)?;
        if let Some(f) = tail.failure.take() {
            return Err(failure(f));
        }
        booked.map_err(failure)?;
        if let Some(end) = Engine::stop_of(gate, self.daemon_cancel, self.slot, self.sink) {
            return end.map(Some);
        }
        match result {
            Ok(()) => {
                if d.kind == DecisionKind::Close && !self.closed.contains(&d.case) && parsed_answer(planned, answer).is_ok_and(|p| !p.deny) {
                    self.closed.insert(d.case.clone());
                }
            }
            Err(why) => {
                self.replaced += 1;
                let again = plan_with(format!("d{:x}-{}-{}r{}", self.epoch, self.round, request_id.rsplit('-').next().unwrap_or("0"), self.replaced), d, Some(&why), &planned.found);
                let id = again.request.request_id.clone();
                lock(&self.slot.state).open.insert(id.clone());
                self.unanswered.insert(id.clone());
                self.sink.decision_requested(again.request.clone());
                self.asked.insert(id, again);
            }
        }
        Ok(None)
    }
}

/// What a wait reads: the run's own flag (a cancel or the wall cap), the daemon's cancel and the
/// run's slot.
struct Waited<'a> {
    local: &'a AtomicBool,
    cancel: &'a AtomicBool,
    slot: &'a Slot,
    asking: &'a Asking,
}

/// Where a request is read from: the run's workspace (the cases' `work/`) and the run's bindings directory.
struct Asking {
    workspace: PathBuf,
    bindings: Option<PathBuf>,
}

/// How a wait ended.
enum RoundEnd {
    /// Every request was answered: the run goes on with a new round.
    Resumed,
    /// The run's own flag was set while it waited.
    Halted,
    /// The run ended in the wait (a program an answer would have started was not).
    Ended(Finish),
}

/// How a run ends when a program was not started.
#[derive(Debug, PartialEq, Eq)]
enum StopEnd {
    Failed(String),
    Cancelled,
    Spend(String),
}

/// A ledger that cannot be read fails the run; a short reservation is the spend cap, and a cancel
/// by the daemon wins over it.
fn stop_end(stopped: Option<Stopped>, cancelled: bool) -> Option<StopEnd> {
    Some(match stopped? {
        Stopped::Ledger(why) => StopEnd::Failed(why),
        Stopped::Spend(_) if cancelled => StopEnd::Cancelled,
        Stopped::Spend(detail) => StopEnd::Spend(detail),
    })
}

/// What `answer` is for the request it answers: a letter of its options for a request read from its case
/// (`decision::parse_option`), else an answer as `decision::parse_answer` reads it.
fn parsed_answer(planned: &Planned, answer: &str) -> Result<Parsed, String> {
    match &planned.request.artifact {
        Some(_) => parse_option(&planned.request, planned.decision.kind, answer),
        None => parse_answer(answer),
    }
}

/// The run's copy of the runbook is what it was at the submit, or the run ends with the refusal.
fn check_copy(pinned: Option<&Pin>) -> Result<(), RuntimeFailure> {
    pinned.map_or(Ok(()), |p| p.check()).map_err(failure)
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
        layout::resolve_in(&self.homes, runbook, version).is_ok()
    }

    fn runbooks(&self) -> Vec<RunbookInfo> {
        layout::list_in(&self.homes)
    }

    fn kind(&self) -> &'static str {
        "loom"
    }

    fn resolve_caps(&self, settings: &std::collections::BTreeMap<String, String>) -> Result<Option<Caps>, String> {
        self.caps.resolve(settings).map(Some)
    }

    fn check_runbook(&self, runbook: &str, version: Option<&str>) -> Result<(), String> {
        let resolved = layout::resolve_in(&self.homes, runbook, version)?;
        crate::budget::reserve_for_unnamed(&resolved.bindings).map(|_| ())
    }

    fn usage(&self, run_id: &str) -> Option<Usage> {
        if let Some(meter) = self.slot(run_id).and_then(|s| lock(&s.meter).clone()) {
            return Some(meter.usage());
        }
        gate::read_usage(&self.state_dir.join("runs").join(run_id))
    }

    fn start(&self, mut request: StartRequest, sink: &dyn EventSink, cancel: &AtomicBool) -> Result<Finish, RuntimeFailure> {
        // Loom takes only an absolute store: the run's directory is made absolute here, once, before
        // any path is built from it.
        request.state_dir = std::path::absolute(&request.state_dir).map_err(|_| RuntimeFailure { reason: "the run's directory cannot be made absolute: the daemon has no current directory".into() })?;
        let slot = Arc::new(Slot::default());
        {
            let mut runs = lock(&self.runs);
            if runs.contains_key(&request.run_id) {
                return Err(failure(format!("run {} is live in this engine already", request.run_id)));
            }
            runs.insert(request.run_id.clone(), slot.clone());
        }
        let _registered = Registered { engine: self, run_id: request.run_id.clone() };
        // Everything that leaves the engine toward a client, a failure and what the run reports, goes
        // through one `Hide`: no path of the run's directory, the state directory or the home.
        let copy_dir = request.state_dir.join(pin::COPY_DIR);
        let mut hidden: Vec<&Path> = vec![&copy_dir, &request.state_dir, &request.workspace_dir, &self.state_dir];
        hidden.extend(self.homes.iter().map(PathBuf::as_path));
        let hide = Hide::new(&hidden);
        self.drive(&request, &HidingSink { inner: sink, hide: &hide }, cancel, &slot).map_err(|f| RuntimeFailure { reason: hide.text(&f.reason) })
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

#[cfg(test)]
mod halt_tests {
    use super::*;

    #[test]
    fn a_client_cancel_wins_over_the_wall_cap_whichever_came_first() {
        assert_eq!(halt_of(true, true), Halt::Cancelled, "the wall cap was hit and the client had cancelled");
        assert_eq!(halt_of(true, false), Halt::Cancelled);
        assert_eq!(halt_of(false, false), Halt::Cancelled, "the flag was set by neither");
        assert_eq!(halt_of(false, true), Halt::Wall);
    }

    #[test]
    fn a_client_cancel_wins_over_a_spend_stop_and_a_ledger_failure_wins_over_both() {
        assert_eq!(stop_end(Some(Stopped::Spend("d".into())), true), Some(StopEnd::Cancelled), "the reservation was short and the client had cancelled");
        assert_eq!(stop_end(Some(Stopped::Spend("d".into())), false), Some(StopEnd::Spend("d".into())));
        assert_eq!(stop_end(Some(Stopped::Ledger("l".into())), true), Some(StopEnd::Failed("l".into())));
        assert_eq!(stop_end(None, true), None);
    }
}
