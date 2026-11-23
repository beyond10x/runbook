//! Loom as a library in this process: the calls of `runbook::engine::calls`, the same ones the
//! daemon's engine makes. No `b10x-loom`, `loom` or `canon` process is started; only
//! the bound programs run as processes, started by Loom's runner with the environment Loom itself
//! gives them (`docs/loom-run.md`, "The environment of a bound program").
//!
//! A decision is answered from its structured members; the printed `command` is never read.

use crate::loom_run::host::{Answer, Call, Halt, LoomHost, Opening};
use crate::loom_run::{Confinement, Outcome};
use b10x_loom_tree::{RunSettings, TestRunner};
use runbook::engine::calls;
use runbook::engine::refusal;
use runbook::engine::Parsed;
use runbook::runtime::Confinement as Level;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// The Loom commit this binary links, read from `Cargo.lock` by `build.rs`.
pub const COMMIT: &str = env!("RUNBOOK_LOOM_COMMIT");

/// The exit status `b10x-loom tree` ends with for a refusal.
const REFUSED: i32 = 1;
/// The exit status `b10x-loom tree verify` ends with when a log does not verify.
const LOG_FAILED: i32 = 3;

/// The level of the engine a `--confinement` value is.
pub fn level_of(c: Confinement) -> Level {
    match c {
        Confinement::Local => Level::Local,
        Confinement::Substrate => Level::Linux,
    }
}

/// The refusal of a level `offered` does not hold; nothing is written before it.
pub fn check_level(c: Confinement, offered: &[Level]) -> Result<Level, String> {
    let level = level_of(c);
    if offered.contains(&level) {
        return Ok(level);
    }
    Err(format!(
        "--confinement {} is not a level the library can give here: it offers {} (the linux level needs a cgroup this process may delegate)",
        c.as_arg(),
        offered.iter().map(|l| l.as_str()).collect::<Vec<_>>().join(", ")
    ))
}

/// The level and its runner, or the refusal of the level; nothing is written before it.
pub fn prepare(c: Confinement) -> Result<(Level, Arc<dyn TestRunner>), String> {
    let level = check_level(c, &calls::host_levels())?;
    let runner = calls::runner(level)?;
    Ok((level, runner))
}

/// Loom as a library.
pub struct LibraryHost<'a> {
    store: PathBuf,
    settings: RunSettings,
    catalog: Option<loom_protocols::ProtocolCatalog>,
    cancel: &'a AtomicBool,
    workspace: PathBuf,
    /// The run's copy of the bindings (the directory of the root file), set by `open`.
    bindings_dir: PathBuf,
}

/// The principal Loom gives an approval whose bound action names none (`DEFAULT_PRINCIPAL`,
/// `loom-tree/src/run.rs:70`; the crate does not re-export it).
const DEFAULT_PRINCIPAL: &str = "operator";

impl<'a> LibraryHost<'a> {
    /// A host for the store `store` that runs bound programs through `runner` at `level`.
    pub fn new(store: PathBuf, workspace: PathBuf, level: Level, runner: Arc<dyn TestRunner>, cancel: &'a AtomicBool) -> Self {
        LibraryHost { store, settings: calls::run_settings(runner, level, Some(1)), catalog: None, cancel, workspace, bindings_dir: PathBuf::new() }
    }

    /// The principal Loom would print for `decision`: the `executor.principal` of the bound action in
    /// the run's copy of the bindings (the member the printed command is made from), else Loom's
    /// default. Read from the bindings, never from the printed command.
    fn bound_principal(&self, decision: &Value) -> Result<String, String> {
        let Some(action) = decision.get("action").and_then(Value::as_str) else {
            return Ok(DEFAULT_PRINCIPAL.to_owned());
        };
        let mut found: Vec<String> = Vec::new();
        let entries = std::fs::read_dir(&self.bindings_dir).map_err(|e| format!("{}: {e}", self.bindings_dir.display()))?;
        let mut files: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "yaml")).collect();
        files.sort();
        for file in files {
            let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            let doc: serde_yaml::Value = serde_yaml::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
            let Some(actions) = doc.get("actions").and_then(serde_yaml::Value::as_sequence) else { continue };
            for entry in actions.iter().filter(|a| a.get("action").and_then(serde_yaml::Value::as_str) == Some(action)) {
                if let Some(p) = entry.get("executor").and_then(|e| e.get("principal")).and_then(serde_yaml::Value::as_str) {
                    if !found.iter().any(|f| f == p) {
                        found.push(p.to_owned());
                    }
                }
            }
        }
        match found.as_slice() {
            [] => Ok(DEFAULT_PRINCIPAL.to_owned()),
            [one] => Ok(one.clone()),
            many => Err(format!("the bindings of the run bind the action {action} to the principals {}: the decider has to name the one", many.join(", "))),
        }
    }

    /// The principal of an answer: the decider's, else the bound one; a close has none.
    fn principal_of(&self, a: &Answer<'_>) -> Result<String, String> {
        match (a.principal, a.kind) {
            (Some(p), _) => Ok(p.to_owned()),
            (None, "Close" | "Handoff") => Ok(String::new()),
            (None, _) => self.bound_principal(a.decision),
        }
    }

    fn catalog(&self) -> Result<&loom_protocols::ProtocolCatalog, Halt> {
        self.catalog.as_ref().ok_or_else(|| Halt::Failed("the tree is not open".to_owned()))
    }

    /// A refusal of Loom's call `what`. Loom words a refusal `<Reason>: ...`: one that leads with a
    /// reason of the pinned Loom ends the run `refused` with that reason as its cause; any other
    /// failure (a document that did not parse, a file that did not open) fails the run closed.
    fn refused(&self, what: &str, why: String) -> Halt {
        let text = format!("Loom refused {what}: {why}");
        match refusal::named(&why) {
            Some(reason) => Halt::Done(Outcome::Refused { cause: refusal::cause(reason), reason: text, loom_exit: None, stderr_tail: None }),
            None => Halt::Failed(text),
        }
    }
}

impl LoomHost for LibraryHost<'_> {
    fn has_exit_codes(&self) -> bool {
        false
    }

    fn open(&mut self, o: &Opening<'_>) -> Result<(), Halt> {
        let opening = calls::Opening {
            store: &self.store,
            workspace: &self.workspace,
            protocols: o.protocol_dir,
            root: calls::Root::Registration(o.root_protocol),
            bindings: o.root_file,
            case: o.case,
        };
        calls::open(&opening, || Ok(())).map_err(|e| self.refused("tree open", e))?;
        self.bindings_dir = o.root_file.parent().map(Path::to_path_buf).unwrap_or_default();
        self.catalog = Some(calls::catalog(&self.store)?);
        Ok(())
    }

    fn step(&mut self) -> Result<Call, Halt> {
        if self.cancel.load(Ordering::SeqCst) {
            return Err(Halt::Done(Outcome::StoppedSignal));
        }
        let catalog = self.catalog()?;
        let ran = calls::run(&self.store, catalog, &self.settings, None, Some(self.cancel)).map_err(|refusal| {
            Halt::Done(Outcome::Refused { cause: refusal.cause(), reason: format!("Loom refused tree run: {refusal}"), loom_exit: None, stderr_tail: None })
        })?;
        let code = calls::stop_status(ran.stop);
        if code == 5 && ran.steps == 0 && self.cancel.load(Ordering::SeqCst) {
            return Err(Halt::Done(Outcome::StoppedSignal));
        }
        Ok(Call { code: Some(code), stdout: ran.document.to_string().into_bytes(), stderr: Vec::new() })
    }

    fn status(&mut self) -> Result<Call, Halt> {
        let catalog = self.catalog()?;
        Ok(match calls::status_document(&self.store, catalog) {
            Ok(doc) => Call { code: Some(0), stdout: doc.to_string().into_bytes(), stderr: Vec::new() },
            Err(e) => Call { code: Some(REFUSED), stdout: Vec::new(), stderr: e.into_bytes() },
        })
    }

    fn admit(&self, decision: &Value) -> Result<(), String> {
        calls::decision_from_json(decision).map(|_| ())
    }

    fn principal(&self, a: &Answer<'_>) -> String {
        self.principal_of(a).unwrap_or_default()
    }

    fn answer(&mut self, a: &Answer<'_>) -> Result<(), Halt> {
        let decision = calls::decision_from_json(a.decision)?;
        let principal = self.principal_of(a)?;
        let parsed = Parsed { deny: a.deny, result: a.result.map(str::to_owned), reasons: None, reasons_file: a.reasons_file.map(Path::to_path_buf) };
        let catalog = self.catalog()?;
        calls::apply(&self.store, catalog, &decision, &parsed, &principal, &self.settings.runner).map_err(|e| self.refused(&format!("the answer to a decision of kind {}", a.kind), e))
    }

    fn runs_handoff(&self) -> bool {
        false
    }

    fn handoff(&mut self, _: &Answer<'_>) -> Result<Call, Halt> {
        Err(Halt::Failed("the library runs no call for a handoff".to_owned()))
    }

    fn conclude(&mut self) -> (Result<Call, String>, Result<Call, String>) {
        let Some(catalog) = self.catalog.as_ref() else {
            return (Err("the tree is not open".to_owned()), Err("the tree is not open".to_owned()));
        };
        let status = Ok(match calls::status_document(&self.store, catalog) {
            Ok(doc) => Call { code: Some(0), stdout: doc.to_string().into_bytes(), stderr: Vec::new() },
            Err(e) => Call { code: Some(REFUSED), stdout: Vec::new(), stderr: e.into_bytes() },
        });
        let verify = Ok(match calls::verify_document(&self.store, catalog) {
            Ok((doc, true)) => Call { code: Some(0), stdout: doc.to_string().into_bytes(), stderr: Vec::new() },
            Ok((doc, false)) => Call { code: Some(LOG_FAILED), stdout: doc.to_string().into_bytes(), stderr: Vec::new() },
            Err(e) => Call { code: Some(REFUSED), stdout: Vec::new(), stderr: e.into_bytes() },
        });
        (status, verify)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_the_library_does_not_offer_is_refused_by_name() {
        assert_eq!(check_level(Confinement::Local, &[Level::Local]), Ok(Level::Local));
        let refusal = check_level(Confinement::Substrate, &[Level::Local]).unwrap_err();
        // the whole text: `docs/loom-run.md` states it and `tests/loom_run/pages.rs` reads the same literal
        assert_eq!(refusal, "--confinement substrate is not a level the library can give here: it offers local (the linux level needs a cgroup this process may delegate)");        assert_eq!(check_level(Confinement::Substrate, &[Level::Local, Level::Linux]), Ok(Level::Linux));
    }

    #[test]
    fn the_pinned_commit_is_forty_hex_digits() {
        assert!(COMMIT.len() == 40 && COMMIT.chars().all(|c| c.is_ascii_hexdigit()), "{COMMIT}");
    }
}
