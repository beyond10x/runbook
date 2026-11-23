//! A decision whose action the answers file declares (`answers.rs`): the requests of the file's
//! program, the decider asked once per request, each answer applied by the file's program, the
//! `then` program, and Loom's printed command last. A child module of `run_loop.rs`, so that it
//! reaches the loop's own state without widening its visibility.

use super::*;
use crate::loom_run::answers::{self, AskRequest, Printed, Where};

fn ran_ok(argv: &[String], printed: &Printed) -> Result<(), Halt> {
    match printed.code {
        Some(0) => Ok(()),
        Some(code) => Err(Halt::Failed(format!("{} exited {code}: {}", argv[0], answers::tail(&printed.stderr)))),
        None => Err(Halt::Failed(format!("{} was ended by a signal: {}", argv[0], answers::tail(&printed.stderr)))),
    }
}

impl Runner<'_> {
    /// Takes the decision when the answers file declares its kind and action and the decider answers
    /// requests; `Ok(false)` leaves it to the decider as before (nothing declared, a decider that
    /// answers no request, or a requests program that printed none).
    pub(super) fn answer_declared(&mut self, decision: &Value, kind: &str, case: &str) -> Result<bool, Halt> {
        let action = decision.get("action").and_then(Value::as_str).unwrap_or("").to_owned();
        let Some(declaration) = self.answers.find(kind, &action).cloned() else { return Ok(false) };
        if !self.decider.answers_requests() {
            return Ok(false);
        }
        let bindings = self.bindings_dir.clone();
        let workspace = self.workspace.clone();
        let path = self.env.iter().find(|(k, _)| k == "PATH").map(|(_, v)| v.clone()).unwrap_or_default();
        let place = Where { bindings: &bindings, workspace: &workspace, path: &path, case };
        let printed = place.run(&declaration.requests, &[("case", case)])?;
        ran_ok(&declaration.requests, &printed)?;
        let requests = answers::parse_requests(&printed.stdout).map_err(|e| format!("{}: {e}", declaration.requests[0]))?;
        if requests.is_empty() {
            return Ok(false);
        }
        let mut given: Vec<(String, String)> = Vec::new();
        let mut taken_by: Option<String> = None;
        for request in &requests {
            self.check_stop()?;
            let count_key = format!("{case}\u{1f}{action}\u{1f}{}\u{1f}{}", request.kind, request.id);
            let times = self.answered.get(&count_key).copied().unwrap_or(0);
            if times >= answers::MAX_ANSWERS_PER_REQUEST {
                return Err(Halt::Failed(format!(
                    "the {} {} of case {case} came back after it was answered {times} times: {} did not take the answer",
                    request.kind, request.id, declaration.apply[0]
                )));
            }
            self.refuse_decider_path("decider answering the answers file")?;
            let want = self.decider.max_call_micro_usd();
            if self.node.is_with() && want > 0 && self.llm_calls >= self.args.max_llm_calls {
                self.emit(Event::CapReached { cap: Cap::LlmCalls, detail: format!("{} calls", self.llm_calls) })?;
                return Err(Halt::Done(Outcome::StoppedLlmCalls));
            }
            let reservation = self.reserve(want)?;
            let asked = {
                let ask = AskRequest {
                    decision,
                    request,
                    run_dir: &self.run_dir,
                    workspace: &self.workspace,
                    budget_micro_usd: reservation.micro_usd,
                    child_env: &self.env,
                    login_link: self.login_link.as_deref(),
                };
                self.decider.ask(&ask)
            };
            for line in &asked.lines {
                self.emit(Event::AnswerRecorded { line: line.clone() })?;
            }
            if !self.node.is_with() && (asked.cost_micro_usd > 0 || asked.model_calls > 0) {
                self.spent += asked.cost_micro_usd;
                self.llm_calls += asked.model_calls;
                return self.stop_model_call("decider answering the answers file", format!("the decider reported {} model call(s) and {} micro USD in a run opened without a model node; no ledger was open", asked.model_calls, asked.cost_micro_usd));
            }
            if asked.result.is_err() && asked.cost_micro_usd == 0 && asked.model_calls == 0 {
                self.release(&reservation)?;
            } else {
                self.commit(&reservation, "decider", asked.cost_micro_usd)?;
                self.spent += asked.cost_micro_usd;
                self.llm_calls += asked.model_calls;
                let model = asked.lines.iter().rev().find_map(|l| l.get("model").and_then(Value::as_str).map(str::to_owned));
                self.emit(Event::CostedCall { source: CostSource::Decider, cost_micro_usd: asked.cost_micro_usd, record: serde_json::json!({"model_calls": asked.model_calls, "model": model}) })?;
            }
            let answer = match asked.result {
                Ok(answer) => answer,
                Err(reason) => return Err(Halt::Done(Outcome::StoppedDecider { reason: format!("{action} in {case}: {reason} (decision kind {kind:?})") })),
            };
            self.answered.insert(count_key, times + 1);
            let who = answer.principal.clone().unwrap_or_else(|| self.host.principal(&Answer { decision, kind, approve: true, principal: None, result: None, reasons_file: None, deny: false }));
            let applied = place.run(&declaration.apply, &[("id", &request.id), ("answer", &answer.answer), ("principal", &who), ("case", case), ("type", &request.kind)])?;
            ran_ok(&declaration.apply, &applied)?;
            given.push((request.id.clone(), answer.answer));
            taken_by = Some(who);
        }
        let who = taken_by.unwrap_or_default();
        if let Some(then) = &declaration.then {
            let done = place.run(then, &[("principal", &who), ("case", case)])?;
            ran_ok(then, &done)?;
        }
        let principal_arg = if who.is_empty() { None } else { Some(who.as_str()) };
        let mut answer = Answer { decision, kind, approve: true, principal: principal_arg, result: None, reasons_file: None, deny: false };
        let principal = self.host.principal(&answer);
        if kind == "Handoff" {
            if self.host.runs_handoff() {
                self.run_handoff(&answer)?;
            }
        } else {
            let reasons_file = self.person_answer(&mut answer, Some(&answers::reasons_text(&given)))?;
            answer.reasons_file = reasons_file.as_deref();
            self.take_answer(&answer)?;
        }
        self.emit(Event::DeclaredTaken { decision: decision.clone(), principal })?;
        Ok(true)
    }
}
