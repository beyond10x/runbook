//! Loom as a child process: the `b10x-loom` binary, one call at a time, in an environment the
//! runner builds (`child.rs`). An answer is the command Loom printed for the decision, run as
//! printed with the answer added (`docs/loom-run.md`, "The decisions").

use crate::loom_run::child::{self, EndedBy};
use crate::loom_run::host::{tail_of, Answer, Call, Halt, LoomHost, Opening};
use crate::loom_run::run_loop::Clock;
use crate::loom_run::{Confinement, Outcome};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

/// The `b10x-loom` binary as a child process.
pub struct ChildHost<'a> {
    loom: PathBuf,
    env: Vec<(String, String)>,
    run_dir: PathBuf,
    store: PathBuf,
    confinement: Confinement,
    clock: &'a dyn Clock,
    cancel: &'a AtomicBool,
    deadline: u64,
}

impl<'a> ChildHost<'a> {
    /// A host that runs `loom` (a file the caller checked against its digest) with `env`, in
    /// `run_dir`, against the store `store`, and ends a running call when `cancel` is set or the
    /// clock passes `deadline`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(loom: PathBuf, env: Vec<(String, String)>, run_dir: PathBuf, store: PathBuf, confinement: Confinement, clock: &'a dyn Clock, cancel: &'a AtomicBool, deadline: u64) -> Self {
        ChildHost { loom, env, run_dir, store, confinement, clock, cancel, deadline }
    }

    fn call(&self, words: &[&str], extra: &[String]) -> Result<Call, Halt> {
        let mut argv: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
        argv.extend(extra.iter().cloned());
        self.call_argv(&argv)
    }

    fn call_argv(&self, argv: &[String]) -> Result<Call, Halt> {
        let clock = self.clock;
        let result = child::run(&self.loom, argv, &self.env, &self.run_dir, self.cancel, self.deadline, &|| clock.now_secs())?;
        match result.ended_by {
            Some(EndedBy::Signal) => Err(Halt::Done(crate::loom_run::Outcome::StoppedSignal)),
            Some(EndedBy::WallTime) => Err(Halt::Done(crate::loom_run::Outcome::StoppedWallTime)),
            None => Ok(Call { code: result.code, stdout: result.stdout, stderr: result.stderr }),
        }
    }
}

/// A call that must exit 0. One that does not and printed `b10x-loom: tree refused: <Reason>: ...`
/// ends the run `refused` with the reason as its cause, as `tree run` does (`run_loop::parse_run`);
/// any other failure is a failed call of `b10x-loom` with its exit code and the tail of its standard
/// error.
fn expect_exit0(r: &Call, what: &str) -> Result<(), Halt> {
    if r.code == Some(0) {
        return Ok(());
    }
    if let Some(refusal) = runbook::engine::refusal::in_stderr(&String::from_utf8_lossy(&r.stderr)) {
        return Err(Halt::Done(Outcome::Refused { cause: refusal.cause(), reason: format!("b10x-loom {what} refused: {refusal}"), loom_exit: r.code, stderr_tail: Some(tail_of(&r.stderr)) }));
    }
    Err(Halt::Loom { reason: format!("b10x-loom {what} exited {:?}", r.code), exit: r.code, stderr_tail: tail_of(&r.stderr) })
}

/// The words of a decision's `command`: text, the first being the program name Loom printed.
fn command_words(decision: &Value) -> Result<Vec<String>, String> {
    let words: Vec<String> = decision
        .get("command")
        .and_then(Value::as_array)
        .ok_or("a decision has no command")?
        .iter()
        .map(|w| w.as_str().map(str::to_owned).ok_or("a word of a decision's command is not text"))
        .collect::<Result<_, _>>()?;
    if words.first().map(String::as_str) != Some("b10x-loom") || words.len() < 2 {
        return Err("a decision's command does not start with b10x-loom and a subcommand".to_owned());
    }
    Ok(words)
}

/// The argv (after the program) of a decision's command. The command is data and runs as Loom
/// printed it; the answer adds to it and replaces one value here: the one after `--principal`, by
/// the decider's principal when it gave one. The principal returned is the one in the argv ("" when
/// the command has none).
fn command_argv(words: &[String], principal: Option<&str>) -> (Vec<String>, String) {
    let mut argv: Vec<String> = words[1..].to_vec();
    let mut used = String::new();
    for i in 0..argv.len().saturating_sub(1) {
        if argv[i] == "--principal" {
            if let Some(p) = principal {
                argv[i + 1] = p.to_owned();
            }
            used = argv[i + 1].clone();
        }
    }
    (argv, used)
}

impl LoomHost for ChildHost<'_> {
    fn has_exit_codes(&self) -> bool {
        true
    }

    fn open(&mut self, opening: &Opening<'_>) -> Result<(), Halt> {
        for (name, file) in opening.protocols {
            let r = self.call(&["protocols", "add", name], &["--file".to_owned(), file.display().to_string()])?;
            expect_exit0(&r, &format!("protocols add {name}"))?;
        }
        let open = [
            "--protocol".to_owned(),
            opening.root_protocol.to_owned(),
            "--store".to_owned(),
            self.store.display().to_string(),
            "--bindings".to_owned(),
            opening.root_file.display().to_string(),
            "--workspace".to_owned(),
            self.run_dir.join("work").display().to_string(),
            "--case".to_owned(),
            opening.case.to_owned(),
        ];
        let r = self.call(&["tree", "open"], &open)?;
        expect_exit0(&r, "tree open")
    }

    fn step(&mut self) -> Result<Call, Halt> {
        let argv = [
            "tree".to_owned(),
            "run".to_owned(),
            "--store".to_owned(),
            self.store.display().to_string(),
            "--confinement".to_owned(),
            self.confinement.as_arg().to_owned(),
            "--max-steps".to_owned(),
            "1".to_owned(),
        ];
        self.call_argv(&argv)
    }

    fn status(&mut self) -> Result<Call, Halt> {
        self.call(&["tree", "status"], &["--store".to_owned(), self.store.display().to_string()])
    }

    fn admit(&self, decision: &Value) -> Result<(), String> {
        command_words(decision).map(|_| ())
    }

    fn principal(&self, a: &Answer<'_>) -> String {
        command_words(a.decision).map(|words| command_argv(&words, a.principal).1).unwrap_or_default()
    }

    fn answer(&mut self, a: &Answer<'_>) -> Result<(), Halt> {
        let words = command_words(a.decision)?;
        let (mut argv, _) = command_argv(&words, a.principal);
        if a.deny {
            argv.push("--deny".to_owned());
        }
        if let Some(result) = a.result {
            argv.extend(["--result".to_owned(), result.to_owned()]);
            if let Some(file) = a.reasons_file {
                argv.extend(["--reasons-file".to_owned(), file.display().to_string()]);
            }
        }
        let r = self.call_argv(&argv)?;
        expect_exit0(&r, &format!("the printed command for a decision of kind {}", a.kind))
    }

    fn runs_handoff(&self) -> bool {
        true
    }

    fn handoff(&mut self, a: &Answer<'_>) -> Result<Call, Halt> {
        let words = command_words(a.decision)?;
        let (argv, _) = command_argv(&words, a.principal);
        self.call_argv(&argv)
    }

    fn conclude(&mut self) -> (Result<Call, String>, Result<Call, String>) {
        let never = AtomicBool::new(false);
        let clock = self.clock;
        let store = ["--store".to_owned(), self.store.display().to_string()];
        let call = |words: &[&str]| {
            let mut argv: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
            argv.extend(store.iter().cloned());
            let deadline = clock.now_secs().saturating_add(120);
            child::run(&self.loom, &argv, &self.env, &self.run_dir, &never, deadline, &|| clock.now_secs()).map(|r| Call { code: r.code, stdout: r.stdout, stderr: r.stderr })
        };
        (call(&["tree", "status"]), call(&["tree", "verify"]))
    }
}
