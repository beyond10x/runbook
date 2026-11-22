//! The loop of `runbook loom-run`. It exists once: it reaches Loom through a [`LoomHost`], the
//! child process or the library (`host.rs`).

use crate::loom_run::answers::Answers;
use crate::loom_run::bindings_copy;
use crate::loom_run::budget;
use crate::loom_run::child;
use crate::loom_run::child_host::ChildHost;
use crate::loom_run::cost::CostReader;
use crate::loom_run::decide::{self, Decider, DecisionRequest, Verdict};
use crate::loom_run::decider;
use crate::loom_run::events::{Cap, CostSource, Event, EventSink};
use crate::loom_run::host::{tail_of, Answer, Call, Halt, Identity, LoomHost, Opening};
use crate::loom_run::ledger::{Ledger, Reservation};
use crate::loom_run::library::{self, LibraryHost};
use crate::loom_run::private::PrivateDir;
use crate::loom_run::run_dir::RunDirWriter;
use crate::loom_run::run_files::{self, BasicFiles};
use crate::loom_run::settings;
use crate::loom_run::{Args, Outcome, Way};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// The clock the loop reads: seconds since the Unix epoch. Tests give their own.
pub trait Clock {
    /// Now, in seconds.
    fn now_secs(&self) -> u64;
}

/// The system clock.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_secs(&self) -> u64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
    }
}

/// Set by the SIGTERM and SIGINT handlers.
static SIGNALLED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: libc::c_int) {
    SIGNALLED.store(true, Ordering::SeqCst);
}

fn install_signal_handlers() {
    let handler = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
    // SAFETY: the handler only stores into an atomic.
    unsafe {
        libc::signal(libc::SIGTERM, handler);
        libc::signal(libc::SIGINT, handler);
    }
}

/// Runs the whole command. `Err` is a refusal before anything was written (exit 2); `Ok` is how
/// the run ended, written into the outcome file.
pub fn run(args: &Args) -> Result<Outcome, String> {
    install_signal_handlers();
    run_with(args, &SystemClock, &SIGNALLED)
}

/// [`run`] with the clock and the cancel flag given.
pub fn run_with(args: &Args, clock: &dyn Clock, cancel: &AtomicBool) -> Result<Outcome, String> {
    // The way to Loom is checked before anything else: one of the two, and a binary that is the
    // file its digest names, or a level the library can give.
    let (identity, library) = match args.way()? {
        Way::Child { loom, sha256 } => {
            let sha = file_sha256(loom).map_err(|e| format!("--loom: {e}"))?;
            if sha != sha256.trim().to_ascii_lowercase() {
                return Err(format!("--loom has sha256 {sha}, not the {sha256} that --loom-sha256 names"));
            }
            (Identity::Binary { sha256: sha }, None)
        }
        Way::Library => (Identity::Library { commit: library::COMMIT }, Some(library::prepare(args.confinement)?)),
    };
    check_name(&args.run)?;
    let (root_name, _) = args.root_protocol.split_once('@').filter(|(n, r)| !n.is_empty() && !r.is_empty()).ok_or("--root-protocol is not name@revision")?;
    let run_dir = std::path::absolute(args.runs_dir.join(&args.run)).map_err(|e| format!("--runs-dir: {e}"))?;
    let given = settings::normalise(&settings::read(&args.settings)?, args)?;
    let mut settings_value = given.clone();
    settings::fill_decider(&mut settings_value, &run_dir, std::env::var_os("PATH").as_deref())?;
    let decider = match decide::scripted_from_settings(&settings_value) {
        Some(d) => d?,
        None => decider::from_settings(&settings_value)?,
    };
    let protocols = protocol_files(&args.protocols)?;
    if !args.idea.is_file() {
        return Err(format!("--idea {} is not a file", args.idea.display()));
    }
    if !args.bindings.is_dir() {
        return Err(format!("--bindings {} is not a directory", args.bindings.display()));
    }
    if run_dir.exists() {
        return Err(format!("the run directory {} exists", run_dir.display()));
    }
    let login = settings_value.get(settings::LOGIN_KEY).and_then(Value::as_str).map(PathBuf::from);
    let private = PrivateDir::create(&args.run, login.as_deref())?;
    std::fs::create_dir_all(&args.runs_dir).map_err(|e| format!("--runs-dir: {e}"))?;
    std::fs::create_dir(&run_dir).map_err(|e| format!("the run directory {}: {e}", run_dir.display()))?;
    let redacted = settings::with_run(&settings::redacted(&given), args, &identity);
    let mut sinks: Vec<Box<dyn EventSink>> = Vec::new();
    let start = clock.now_secs();
    let env = child::child_env(&run_dir);
    let store = run_dir.join("store");
    let host: Box<dyn LoomHost + '_> = match (&identity, library) {
        (Identity::Binary { .. }, _) => {
            let loom = args.loom.clone().ok_or("--loom is not given")?;
            Box::new(ChildHost::new(loom, env.clone(), run_dir.clone(), store.clone(), args.confinement, clock, cancel, start.saturating_add(args.max_wall_seconds)))
        }
        (Identity::Library { .. }, Some((level, runner))) => Box::new(LibraryHost::new(store.clone(), run_dir.join("work"), level, runner, cancel)),
        (Identity::Library { .. }, None) => return Err("the library was not prepared".to_owned()),
    };
    let mut runner = Runner {
        args,
        clock,
        cancel,
        host,
        settings: settings_value,
        run_dir: run_dir.clone(),
        workspace: run_dir.join("work"),
        store,
        bindings_dir: run_dir.join("bindings"),
        env,
        decider,
        sinks: Vec::new(),
        reader: CostReader::with_home(std::env::var("HOME").ok()),
        ledger: None,
        llm_calls: 0,
        steps: 0,
        spent: 0,
        start,
        wall_cap_noted: false,
        decided: std::collections::HashMap::new(),
        answers: Answers::default(),
        answered: std::collections::HashMap::new(),
        reasons_written: 0,
        login_link: private.as_ref().and_then(|p| p.link()).map(Path::to_path_buf),
    };
    let basic = BasicFiles::open(&run_dir)?;
    sinks.push(Box::new(basic));
    sinks.push(Box::new(RunDirWriter::open(&run_dir, &redacted)?));
    runner.sinks = sinks;
    let outcome = runner.drive(root_name, &protocols, &redacted);
    Ok(runner.finish(outcome))
}

fn file_sha256(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect())
}

fn check_name(name: &str) -> Result<(), String> {
    let ok = !name.is_empty() && name != "." && name != ".." && !name.contains('/') && !name.chars().any(char::is_control);
    if ok { Ok(()) } else { Err(format!("--run {name:?} is not a plain name")) }
}

/// The registration name and file of every `*.protocol.yaml` in `dir`: `<id with dots as dashes>@<revision>`.
fn protocol_files(dir: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let mut found = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| format!("--protocols {}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| format!("--protocols: {e}"))?.path();
        let is_protocol = path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(".protocol.yaml"));
        if !is_protocol {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let doc: serde_yaml::Value = serde_yaml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let id = doc["protocol"]["id"].as_str().ok_or_else(|| format!("{}: protocol.id missing", path.display()))?;
        let revision = doc["protocol"]["revision"].as_u64().ok_or_else(|| format!("{}: protocol.revision missing", path.display()))?;
        found.push((format!("{}@{revision}", id.replace('.', "-")), path));
    }
    found.sort();
    if found.is_empty() {
        return Err(format!("--protocols {} holds no *.protocol.yaml", dir.display()));
    }
    Ok(found)
}

struct Runner<'a> {
    args: &'a Args,
    clock: &'a dyn Clock,
    cancel: &'a AtomicBool,
    host: Box<dyn LoomHost + 'a>,
    settings: Value,
    run_dir: PathBuf,
    workspace: PathBuf,
    store: PathBuf,
    bindings_dir: PathBuf,
    env: Vec<(String, String)>,
    decider: Box<dyn Decider>,
    sinks: Vec<Box<dyn EventSink>>,
    reader: CostReader,
    ledger: Option<Ledger>,
    llm_calls: u64,
    steps: u64,
    spent: u64,
    start: u64,
    wall_cap_noted: bool,
    decided: std::collections::HashMap<String, bool>,
    /// What the bindings copy's answers file declares, and how many answers each request got.
    answers: Answers,
    answered: std::collections::HashMap<String, u64>,
    reasons_written: u64,
    login_link: Option<PathBuf>,
}

/// The key of a decision for "asked before": case, action, revision and the text Loom gave (an
/// opaque string here; it is never parsed).
fn decision_key(decision: &Value) -> String {
    let get = |k: &str| decision.get(k).and_then(Value::as_str).unwrap_or("");
    format!("{}\u{1f}{}\u{1f}{}\u{1f}{}", get("case"), get("action"), get("revision"), get("decides"))
}

impl Runner<'_> {
    fn emit(&mut self, event: Event) -> Result<(), String> {
        let at = self.clock.now_secs();
        for sink in &mut self.sinks {
            sink.event(at, &event)?;
        }
        Ok(())
    }

    fn deadline(&self) -> u64 {
        self.start.saturating_add(self.args.max_wall_seconds)
    }

    /// A refusal of Loom that fails the run: a child's has an exit code and a standard error that go
    /// to the outcome; the library's has neither, so it is a plain error with its reason.
    fn loom_halt(&self, child_reason: String, library_reason: String, r: &Call) -> Halt {
        if self.host.has_exit_codes() {
            Halt::Loom { reason: child_reason, exit: r.code, stderr_tail: tail_of(&r.stderr) }
        } else {
            Halt::Failed(library_reason)
        }
    }

    fn drive(&mut self, root_name: &str, protocols: &[(String, PathBuf)], redacted: &Value) -> Outcome {
        let outcome = match self.drive_inner(root_name, protocols, redacted) {
            Ok(outcome) | Err(Halt::Done(outcome)) => outcome,
            Err(Halt::Failed(reason)) => Outcome::Error { reason },
            Err(Halt::Loom { reason, exit, stderr_tail }) => Outcome::LoomFailed { reason, exit, stderr_tail },
        };
        if outcome == Outcome::StoppedWallTime && !self.wall_cap_noted {
            let detail = format!("{} s, a call was ended", self.args.max_wall_seconds);
            if let Err(reason) = self.emit(Event::CapReached { cap: Cap::WallTime, detail }) {
                return Outcome::Error { reason };
            }
        }
        outcome
    }

    fn check_stop(&mut self) -> Result<(), Halt> {
        if self.cancel.load(Ordering::SeqCst) {
            return Err(Halt::Done(Outcome::StoppedSignal));
        }
        if self.clock.now_secs() >= self.deadline() {
            self.wall_cap_noted = true;
            self.emit(Event::CapReached { cap: Cap::WallTime, detail: format!("{} s", self.args.max_wall_seconds) })?;
            return Err(Halt::Done(Outcome::StoppedWallTime));
        }
        Ok(())
    }

    fn drive_inner(&mut self, root_name: &str, protocols: &[(String, PathBuf)], redacted: &Value) -> Result<Outcome, Halt> {
        self.emit(Event::RunStarted { run: self.args.run.clone(), settings: redacted.clone() })?;
        for d in [self.run_dir.join("home").join("data"), self.workspace.clone()] {
            std::fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        self.ledger = Some(Ledger::open(&self.args.spend_ledger)?);
        let root_file = bindings_copy::materialise(&self.args.bindings, &self.bindings_dir, root_name, &self.settings, self.login_link.as_deref())?;
        self.answers = Answers::load(&self.bindings_dir)?;
        std::fs::copy(&self.args.idea, self.workspace.join("idea.yaml")).map_err(|e| format!("--idea: {e}"))?;
        let copies = self.run_dir.join("protocols");
        std::fs::create_dir_all(&copies).map_err(|e| format!("{}: {e}", copies.display()))?;
        for (_, file) in protocols {
            let name = file.file_name().ok_or_else(|| format!("{}: no file name", file.display()))?;
            std::fs::copy(file, copies.join(name)).map_err(|e| format!("{}: {e}", file.display()))?;
        }
        self.host.open(&Opening { protocols, protocol_dir: &copies, root_protocol: &self.args.root_protocol, root_file: &root_file, case: &self.args.case })?;
        let mut index = 0u64;
        loop {
            self.check_stop()?;
            if self.llm_calls >= self.args.max_llm_calls {
                self.emit(Event::CapReached { cap: Cap::LlmCalls, detail: format!("{} calls", self.llm_calls) })?;
                return Ok(Outcome::StoppedLlmCalls);
            }
            let result = self.costed(None)?;
            index += 1;
            let (code, tree) = parse_run(&result)?;
            let stop = tree.get("stop").and_then(Value::as_str).unwrap_or("").to_owned();
            let steps = tree.get("steps").and_then(Value::as_i64).unwrap_or(0);
            self.steps += u64::try_from(steps).unwrap_or(0);
            self.refresh_status()?;
            self.emit(Event::StepMade { index, exit: code, stop, steps })?;
            match code {
                0 => {
                    let tree_outcome = tree.get("detail").and_then(Value::as_str).map(str::to_owned);
                    return Ok(Outcome::Ended { tree_outcome });
                }
                4 => return Ok(Outcome::Stuck),
                5 if steps > 0 => {}
                5 => return Err(Halt::Failed("tree run ended its budget without a step".to_owned())),
                3 => self.answer_decisions(&tree)?,
                other => return Err(self.loom_halt(format!("b10x-loom tree run exited {other}, an exit code the runner does not know"), format!("Loom stopped the tree with status {other}, which the runner does not know"), &result)),
            }
        }
    }

    /// Runs a `b10x-loom` call that may start model programs: the ledger reserves what the largest
    /// per-call budget of the bindings copy allows (at most what the cap has left), the copy is
    /// limited to it, and what the call spent is booked after it.
    fn costed(&mut self, handoff: Option<&Answer<'_>>) -> Result<Call, Halt> {
        let want = budget::largest_call_budget(&self.bindings_dir)?.unwrap_or(self.args.cap_micro_usd);
        let reservation = self.reserve(want)?;
        budget::limit_call_budgets(&self.bindings_dir, reservation.micro_usd)?;
        let result = match handoff {
            None => self.host.step(),
            Some(answer) => self.host.handoff(answer),
        };
        let cost_halt = self.book_step(&reservation);
        let result = result?;
        cost_halt?;
        Ok(result)
    }

    /// Runs `tree status` and writes `tree-status.json` (atomically, paths relative to the run
    /// directory). A refusal ends the run: `tree status` reads the protocol catalog (`HOME` and
    /// `XDG_DATA_HOME` of the run), and a store read with another catalog answers
    /// `GovernorUnavailable`.
    fn refresh_status(&mut self) -> Result<(), Halt> {
        let r = self.host.status()?;
        if r.code != Some(0) {
            let child = format!("b10x-loom tree status refused (exit {:?}); it needs the protocol catalog of the run", r.code);
            let library = format!("Loom refused the tree status: {}", tail_of(&r.stderr));
            return Err(self.loom_halt(child, library, &r));
        }
        run_files::write_status(&self.run_dir, &r.stdout)?;
        Ok(())
    }

    fn ledger(&self) -> &Ledger {
        self.ledger.as_ref().expect("the ledger is opened before the loop")
    }

    fn reserve(&mut self, want: u64) -> Result<Reservation, Halt> {
        let r = self.ledger().reserve(&self.args.run, self.args.cap_micro_usd, want)?;
        if want > 0 && r.micro_usd == 0 {
            let spent = self.ledger().spent()?;
            self.emit(Event::CapReached { cap: Cap::Spend, detail: format!("{spent} of {} micro USD spent", self.args.cap_micro_usd) })?;
            return Err(Halt::Done(Outcome::StoppedSpendCap));
        }
        Ok(r)
    }

    /// Reads what the step spent and books it. A cost that cannot be read books the whole
    /// reservation (the most the call could spend) and fails the run closed, except for a call that
    /// ended with an exit other than 0 (and 126, 127, 75): the reservation is booked for it and the
    /// run goes on, as the driver's does (the step's retry rule decides, `cost.rs`). A call
    /// directory in which no adapter was started costs nothing: the reservation is released (a
    /// commit of 0, no ledger entry) and an event notes the call and its exit.
    fn book_step(&mut self, reservation: &Reservation) -> Result<(), Halt> {
        match self.reader.read_new(&self.workspace) {
            Ok(cost) => {
                let unknown = cost.unknown.len() as u64;
                let booked = cost.micro_usd.saturating_add(reservation.micro_usd.saturating_mul(unknown));
                let what = if unknown == 0 { "step" } else { "step, cost unknown for a failed call: reservation booked" };
                self.ledger().commit(reservation, &self.args.run, what, booked)?;
                self.spent += booked;
                self.llm_calls += cost.records.len() as u64 + unknown;
                for record in cost.records {
                    let micro = record.get("cost_usd").and_then(|v| crate::loom_run::cost::usd_to_micro(v).ok()).unwrap_or(0);
                    self.emit(Event::CostedCall { source: CostSource::Step, cost_micro_usd: micro, record })?;
                }
                for mut record in cost.unknown {
                    record["cost_unknown"] = Value::Bool(true);
                    self.emit(Event::CostedCall { source: CostSource::Step, cost_micro_usd: reservation.micro_usd, record })?;
                }
                for note in cost.no_call {
                    let call = note.get("call").and_then(Value::as_str).unwrap_or("?").to_owned();
                    let exit = note.get("exit").and_then(Value::as_i64).unwrap_or(-1);
                    self.emit(Event::NoModelCall { call, exit })?;
                }
                Ok(())
            }
            Err(reason) => {
                self.ledger().commit(reservation, &self.args.run, "step, cost unknown: reservation booked", reservation.micro_usd)?;
                self.spent += reservation.micro_usd;
                Err(Halt::Failed(reason))
            }
        }
    }

    /// Answers every decision of one `tree run` that exited 3. A decision is a JSON object of which
    /// only `case`, `kind`, `decides` and `command` are stable: it goes to the decider unchanged,
    /// whatever its kind, and `command` is run as data (see `command_argv`).
    fn answer_decisions(&mut self, tree: &Value) -> Result<(), Halt> {
        let decisions = tree.get("decisions").and_then(Value::as_array).filter(|d| !d.is_empty()).ok_or_else(|| "tree run exited 3 with no decisions".to_owned())?;
        // The cases a close ended in this call: Loom prints one Close per legitimate outcome, and
        // once the case is closed the other outcomes are not decisions any more.
        let mut closed: Vec<String> = Vec::new();
        for decision in decisions.clone() {
            self.check_stop()?;
            let case = decision.get("case").and_then(Value::as_str).unwrap_or("").to_owned();
            let kind = decision.get("kind").and_then(Value::as_str).unwrap_or("").to_owned();
            if kind == "Close" && closed.contains(&case) {
                continue;
            }
            self.emit(Event::DecisionAsked { decision: decision.clone() })?;
            let key = decision_key(&decision);
            match self.decided.get(&key) {
                Some(false) => {
                    let action = decision.get("action").and_then(Value::as_str).or_else(|| decision.get("decides").and_then(Value::as_str)).unwrap_or("").to_owned();
                    return Err(Halt::Done(Outcome::Denied { action }));
                }
                Some(true) => return Err(Halt::Failed(format!("a decision of kind {kind} was approved and asked again"))),
                None => {}
            }
            self.host.admit(&decision)?;
            if self.answer_declared(&decision, &kind, &case)? {
                continue;
            }
            let want = self.decider.max_call_micro_usd();
            if want > 0 && self.llm_calls >= self.args.max_llm_calls {
                self.emit(Event::CapReached { cap: Cap::LlmCalls, detail: format!("{} calls", self.llm_calls) })?;
                return Err(Halt::Done(Outcome::StoppedLlmCalls));
            }
            let reservation = self.reserve(want)?;
            let verdict = {
                let request = DecisionRequest {
                    decision: &decision,
                    run_dir: &self.run_dir,
                    workspace: &self.workspace,
                    budget_micro_usd: reservation.micro_usd,
                    child_env: &self.env,
                    login_link: self.login_link.as_deref(),
                };
                self.decider.decide(&request)
            };
            let (approve, principal, cost, calls, reasons) = match verdict {
                Verdict::CannotDecide { reason } => {
                    self.ledger().release(&reservation)?;
                    return Err(Halt::Done(Outcome::StoppedDecider { reason: format!("{reason} (decision kind {kind:?})") }));
                }
                Verdict::Answer { cost_micro_usd: None, .. } => {
                    self.ledger().commit(&reservation, &self.args.run, "decider, cost unknown: reservation booked", reservation.micro_usd)?;
                    self.spent += reservation.micro_usd;
                    return Err(Halt::Failed("the decider answered without a cost".to_owned()));
                }
                Verdict::Answer { approve, principal, cost_micro_usd: Some(cost), model_calls, reasons } => (approve, principal, cost, model_calls, reasons),
            };
            self.ledger().commit(&reservation, &self.args.run, "decider", cost)?;
            self.spent += cost;
            self.llm_calls += calls;
            if cost > 0 || calls > 0 {
                self.emit(Event::CostedCall { source: CostSource::Decider, cost_micro_usd: cost, record: serde_json::json!({"model_calls": calls}) })?;
            }
            let mut answer = Answer { decision: &decision, kind: &kind, approve, principal: principal.as_deref(), result: None, reasons_file: None, deny: false };
            let principal = self.host.principal(&answer);
            match kind.as_str() {
                // A close the decider does not want is not run: Loom prints one per outcome.
                "Close" if !approve => {}
                "Close" => {
                    self.host.answer(&answer)?;
                    closed.push(case);
                }
                "Handoff" => {
                    if self.host.runs_handoff() {
                        self.run_handoff(&answer)?;
                    }
                }
                _ => {
                    let reasons_file = self.person_answer(&mut answer, reasons.as_deref())?;
                    answer.reasons_file = reasons_file.as_deref();
                    self.host.answer(&answer)?;
                }
            }
            self.decided.insert(key, approve);
            self.emit(Event::DecisionAnswered { decision, approve, principal, cost_micro_usd: cost, model_calls: calls })?;
        }
        Ok(())
    }

    /// A handoff's printed command is `tree run`: it runs as printed, through the ledger like every
    /// call that may start a model program, and what it printed is read like any `tree run`.
    fn run_handoff(&mut self, answer: &Answer<'_>) -> Result<(), Halt> {
        if self.llm_calls >= self.args.max_llm_calls {
            self.emit(Event::CapReached { cap: Cap::LlmCalls, detail: format!("{} calls", self.llm_calls) })?;
            return Err(Halt::Done(Outcome::StoppedLlmCalls));
        }
        let result = self.costed(Some(answer))?;
        let (_, tree) = parse_run(&result)?;
        let steps = tree.get("steps").and_then(Value::as_i64).unwrap_or(0);
        self.steps += u64::try_from(steps).unwrap_or(0);
        self.refresh_status()?;
        Ok(())
    }

    /// The results a person action declares, from the bindings copy: the keys of `results` that
    /// every `from: effect` entry of the action lists. `None`: the copy names no such action.
    fn declared_results(&self, action: &str) -> Result<Option<Vec<String>>, Halt> {
        let mut declared: Option<Vec<String>> = None;
        let entries = std::fs::read_dir(&self.bindings_dir).map_err(|e| format!("{}: {e}", self.bindings_dir.display()))?;
        let mut files: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "yaml")).collect();
        files.sort();
        for file in files {
            let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            let doc: serde_yaml::Value = serde_yaml::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
            let Some(actions) = doc.get("actions").and_then(serde_yaml::Value::as_sequence) else { continue };
            for entry in actions.iter().filter(|a| a.get("action").and_then(serde_yaml::Value::as_str) == Some(action)) {
                if entry.get("executor").and_then(|e| e.get("kind")).and_then(serde_yaml::Value::as_str) != Some("person") {
                    continue;
                }
                let evidence = entry.get("evidence").and_then(serde_yaml::Value::as_sequence).cloned().unwrap_or_default();
                for item in evidence.iter().filter(|i| i.get("from").and_then(serde_yaml::Value::as_str) == Some("effect")) {
                    let keys: Vec<String> = item
                        .get("results")
                        .and_then(serde_yaml::Value::as_mapping)
                        .map(|m| m.keys().filter_map(|k| k.as_str().map(str::to_owned)).collect())
                        .unwrap_or_default();
                    declared = Some(match declared {
                        None => keys,
                        Some(have) => have.into_iter().filter(|k| keys.contains(k)).collect(),
                    });
                }
                declared.get_or_insert_with(Vec::new);
            }
        }
        Ok(declared)
    }

    /// Names the result of a person action (`result`) where the action declares results:
    /// `performed` for an approval, `rejected` and a reasons file for a rejection (returned). A
    /// result the action does not declare ends the run before Loom is called. A decision with no
    /// action (an approval of a capability) takes no result; its rejection is a denial (`deny`).
    fn person_answer(&mut self, answer: &mut Answer<'_>, reasons: Option<&str>) -> Result<Option<PathBuf>, Halt> {
        let decision = answer.decision;
        let approve = answer.approve;
        let action = decision.get("action").and_then(Value::as_str).filter(|a| !a.is_empty());
        let Some(action) = action.filter(|_| answer.kind == "ApproveAction") else {
            answer.deny = !approve;
            return Ok(None);
        };
        let declared = self.declared_results(action)?.unwrap_or_default();
        let result = if approve { "performed" } else { "rejected" };
        if !declared.iter().any(|k| k == result) {
            if approve && declared.is_empty() {
                return Ok(None);
            }
            let listed = if declared.is_empty() { "none".to_owned() } else { declared.join(", ") };
            return Err(Halt::Failed(format!("the decider answered {result} for action {action}, but the bindings of the run declare the results {listed} for it: Loom would refuse the result")));
        }
        answer.result = Some(result);
        if approve && !reasons.is_some_and(|r| !r.trim().is_empty()) {
            return Ok(None);
        }
        Ok(Some(self.write_reasons(action, reasons)?))
    }

    /// Writes the reasons of a rejection into the workspace (Loom reads a regular file below it,
    /// at most 60 lines and 65536 bytes) and returns its path. The text has the login file's path,
    /// the run directory and the home directory taken out.
    fn write_reasons(&mut self, action: &str, reasons: Option<&str>) -> Result<PathBuf, Halt> {
        let mut text = reasons.unwrap_or("").to_owned();
        if let Some(login) = self.settings.get(settings::LOGIN_KEY).and_then(Value::as_str).filter(|l| !l.is_empty()) {
            text = text.replace(login, "<login file>");
        }
        text = text.replace(&self.run_dir.display().to_string(), "<run>");
        if let Some(home) = std::env::var("HOME").ok().filter(|h| h.len() > 1) {
            text = text.replace(&home, "~");
        }
        let mut kept: Vec<&str> = text.lines().take(60).collect();
        while kept.last().is_some_and(|l| l.trim().is_empty()) {
            kept.pop();
        }
        let mut text = kept.join("\n");
        if text.len() > 65_000 {
            let mut end = 65_000;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
        }
        if text.trim().is_empty() {
            text = "The decider rejected this and gave no reasons.".to_owned();
        }
        self.reasons_written += 1;
        let safe: String = action.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect();
        let dir = self.workspace.join(".sdc").join("reasons");
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let file = dir.join(format!("{:03}-{safe}.txt", self.reasons_written));
        std::fs::write(&file, format!("{text}\n")).map_err(|e| format!("{}: {e}", file.display()))?;
        Ok(file)
    }

    /// Stores `tree status` and `tree verify`, then writes the end of the run. Returns the outcome
    /// as it stands after `tree verify`.
    fn finish(&mut self, outcome: Outcome) -> Outcome {
        let mut outcome = outcome;
        if self.store.join("tree.json").exists() && outcome != Outcome::StoppedSignal {
            let (status, verify) = self.host.conclude();
            let (status_exit, verify_exit) = match (status, verify) {
                (Ok(s), Ok(v)) => {
                    let se = s.code.unwrap_or(-1);
                    let ve = v.code.unwrap_or(-1);
                    if let Err(e) = run_files::write_tree_files(&self.run_dir, (se == 0).then_some(s.stdout.as_slice()), ve, &v.stdout, &v.stderr) {
                        outcome = Outcome::Error { reason: e };
                    }
                    if se != 0 && !matches!(outcome, Outcome::Error { .. } | Outcome::LoomFailed { .. }) {
                        let child = format!("b10x-loom tree status refused (exit {se}) after the run ended as {}; it needs the protocol catalog of the run", outcome.label());
                        let library = format!("Loom refused the tree status after the run ended as {}: {}", outcome.label(), tail_of(&s.stderr));
                        outcome = match self.loom_halt(child, library, &s) {
                            Halt::Loom { reason, exit, stderr_tail } => Outcome::LoomFailed { reason, exit, stderr_tail },
                            Halt::Failed(reason) => Outcome::Error { reason },
                            Halt::Done(done) => done,
                        };
                    }
                    if ve != 0 && !matches!(outcome, Outcome::Error { .. } | Outcome::LoomFailed { .. }) {
                        outcome = Outcome::Error { reason: format!("tree verify exited {ve} (the run had ended: {})", outcome.label()) };
                    }
                    (se, ve)
                }
                (Err(e), _) | (_, Err(e)) => {
                    outcome = Outcome::Error { reason: format!("tree status or verify could not run: {e}") };
                    (-1, -1)
                }
            };
            if let Err(e) = self.emit(Event::TreeStored { status_exit, verify_exit }) {
                outcome = Outcome::Error { reason: e };
            }
        }
        let ended = Event::Ended {
            outcome: outcome.label().to_owned(),
            reason: outcome.reason().map(str::to_owned),
            loom_exit: match &outcome {
                Outcome::LoomFailed { exit, .. } => *exit,
                _ => None,
            },
            stderr_tail: match &outcome {
                Outcome::LoomFailed { stderr_tail, .. } => Some(stderr_tail.clone()),
                _ => None,
            },
            exit_code: outcome.exit_code(),
            steps: self.steps,
            spent_micro_usd: self.spent,
            llm_calls: self.llm_calls,
        };
        if let Err(e) = self.emit(ended) {
            eprintln!("runbook loom-run: the end of the run could not be written: {e}");
            outcome = Outcome::Error { reason: e };
        }
        outcome
    }
}

/// The exit code and the JSON object `tree run` printed; fails for an exit code the runner does
/// not know or output that is not the expected JSON.
fn parse_run(r: &Call) -> Result<(i32, Value), Halt> {
    let Some(code) = r.code else {
        return Err(Halt::Loom { reason: "b10x-loom tree run was ended by a signal it did not get from the runner".to_owned(), exit: None, stderr_tail: tail_of(&r.stderr) });
    };
    if !matches!(code, 0 | 3 | 4 | 5) {
        return Err(Halt::Loom { reason: format!("b10x-loom tree run exited {code}, an exit code the runner does not know"), exit: Some(code), stderr_tail: tail_of(&r.stderr) });
    }
    let v: Value = serde_json::from_slice(&r.stdout).map_err(|e| format!("tree run printed no JSON: {e}"))?;
    if v.get("stop").and_then(Value::as_str).is_none() || v.get("steps").and_then(Value::as_i64).is_none() {
        return Err(Halt::Failed("tree run printed JSON without stop and steps".to_owned()));
    }
    Ok((code, v))
}

mod declared;

#[cfg(test)]
#[allow(dead_code)] // fake.rs is shared with the integration binary; this unit test uses part of it
#[path = "../../tests/loom_run/fake.rs"]
mod fake;

#[cfg(test)]
mod tests {
    use super::fake;
    use super::*;
    use crate::loom_run::Confinement;

    /// The clock of the test: 100 s per call of the fake `b10x-loom`; nothing sleeps.
    struct CallClock {
        calls: PathBuf,
    }

    impl Clock for CallClock {
        fn now_secs(&self) -> u64 {
            let n: u64 = std::fs::read_to_string(&self.calls).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0);
            1_000 + 100 * n
        }
    }

    fn world(tag: &str) -> (PathBuf, fake::Fake) {
        let exe = std::env::current_exe().unwrap();
        let root = exe.parent().unwrap().join(format!("loomrun-unit-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for d in ["protocols", "bindings", "runs"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        std::fs::write(root.join("protocols/sdc.protocol.yaml"), "format: protocol/1\nprotocol:\n  id: sdc\n  revision: 8\n").unwrap();
        std::fs::write(root.join("bindings/sdc.yaml"), "format: loom.bindings/1\n").unwrap();
        std::fs::write(root.join("idea.yaml"), "text: x\n").unwrap();
        std::fs::write(root.join("settings.json"), "{}").unwrap();
        let f = fake::Fake::new(&root.join("fake"));
        f.open_ok();
        (root, f)
    }

    fn args(root: &Path, f: &fake::Fake, wall: u64) -> Args {
        Args {
            run: "r1".into(),
            runs_dir: root.join("runs"),
            loom: Some(f.bin()),
            loom_sha256: Some(f.sha256()),
            in_process: false,
            bindings: root.join("bindings"),
            protocols: root.join("protocols"),
            root_protocol: "sdc@8".into(),
            case: "IDEA-1".into(),
            idea: root.join("idea.yaml"),
            confinement: Confinement::Local,
            settings: root.join("settings.json"),
            spend_ledger: root.join("ledger.json"),
            cap_micro_usd: 1_000_000,
            max_llm_calls: 100,
            max_wall_seconds: wall,
        }
    }

    #[test]
    fn the_wall_time_cap_reads_the_injected_clock_and_stops_before_the_next_call() {
        let (root, f) = world("wall");
        f.answer("tree_run", "default", &fake::tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
        let clock = CallClock { calls: f.dir.join("calls/count") };
        // start 1000, deadline 1150: the clock reads 1200 once tree open has started.
        let outcome = run_with(&args(&root, &f, 150), &clock, &AtomicBool::new(false)).unwrap();
        assert_eq!(outcome, Outcome::StoppedWallTime);
        assert!(f.calls_of(&["tree", "run"]).is_empty(), "no step was started after the deadline");
        let text = std::fs::read_to_string(root.join("runs/r1/outcome.json")).unwrap();
        assert!(text.contains("stopped: wall time"), "{text}");
        let log = std::fs::read_to_string(root.join("runs/r1/events.jsonl")).unwrap();
        assert!(log.contains("\"cap\":\"wall_time\""), "{log}");
    }
}
