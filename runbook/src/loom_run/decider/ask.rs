//! One request of the answers file (`answers.rs`) put to the program decider: a scope-changing
//! question or a finding, asked as the removed driver asked it: the same request members, the same retries of an invalid reply,
//! the same `escalate` handling (`stop`: the run waits for a person; `final`: the default of a
//! question, the `best` of a finding, else its default) and the same lines in `decisions.jsonl`.
//!
//! This is a child module of `decider.rs` so that it uses what the approval path uses (the case
//! directory, the record directories, the reply check) without widening any visibility.

use super::*;
use crate::loom_run::answers::{decision_line, Given};

impl NativeDecider {
    pub(super) fn ask_request(&self, r: &AskRequest<'_>) -> Asked {
        let level = match self.level() {
            Ok(level) => level,
            Err(reason) => return Asked::cannot(reason),
        };
        let dreq = DecisionRequest { decision: r.decision, run_dir: r.run_dir, workspace: r.workspace, budget_micro_usd: r.budget_micro_usd, child_env: r.child_env, login_link: r.login_link };
        let approval = match ask_approval(r) {
            Ok(approval) => approval,
            Err(reason) => return Asked::cannot(reason),
        };
        if let Err(reason) = self.route(level, r.run_dir, &r.request.kind, &approval.action, None) {
            return Asked::cannot(reason);
        }
        match &self.mode {
            Mode::Rule { .. } => self.ask_rule(r, &dreq, &approval),
            Mode::Model { .. } => self.ask_model(r, &dreq, &approval),
        }
    }

    fn ask_rule(&self, r: &AskRequest<'_>, dreq: &DecisionRequest<'_>, approval: &Approval) -> Asked {
        let rec = r.run_dir.join("decider").join("rule");
        if let Err(e) = make_dir_in(r.run_dir, &rec) {
            return Asked::cannot(format!("decider record directory: {e}"));
        }
        let body = ask_body(r, approval, &PathBuf::new(), &rec, None);
        if let Err(e) = write_atomic(&rec.join("decide-request.json"), format!("{body}\n").as_bytes()) {
            return Asked::cannot(format!("decider request file: {e}"));
        }
        let reply = match self.reply(&body, &rec, dreq, 0) {
            Ok(reply) => reply,
            Err(reason) => return Asked::cannot(reason),
        };
        match (reply.get("decision").and_then(Value::as_str), reply.get("answer").and_then(Value::as_str).filter(|a| !a.is_empty())) {
            (Some("approve"), Some(answer)) => Asked {
                lines: vec![decision_line(r.decision, r.request, &approval.revision, "approve", &reply, &self.principal, json!({}))],
                cost_micro_usd: 0,
                model_calls: 0,
                result: Ok(Given { answer: answer.to_owned(), principal: Some(self.principal.clone()) }),
            },
            (Some("approve"), None) => Asked::cannot("the rule decider's reply has no answer"),
            _ => Asked::cannot("the rule decider does not approve: a question or a finding takes an answer, not a rejection"),
        }
    }

    fn ask_model(&self, r: &AskRequest<'_>, dreq: &DecisionRequest<'_>, approval: &Approval) -> Asked {
        let Mode::Model { without_claims, .. } = &self.mode else {
            return Asked::cannot("not a model decider");
        };
        let routed = match self.requester_class(r.run_dir, &r.request.kind, &approval.action) {
            Ok(class) => class,
            Err(reason) => return Asked::cannot(reason),
        };
        if !without_claims && routed.is_none() {
            return Asked::cannot(NO_CLAIMS);
        }
        let mut asked = self.ask_model_routed(r, dreq, approval, routed.as_deref());
        if let Some(class) = &routed {
            for line in &mut asked.lines {
                line["class"] = json!(class);
                line["side"] = json!("requester");
            }
        }
        asked
    }

    fn ask_model_routed(&self, r: &AskRequest<'_>, dreq: &DecisionRequest<'_>, approval: &Approval, routed: Option<&str>) -> Asked {
        let Mode::Model { retries, .. } = &self.mode else {
            return Asked::cannot("not a model decider");
        };
        let final_escalate = self.final_escalate();
        let principal = self.principal_for(routed.is_some());
        if instructions_root(&r.run_dir.join("bindings").join("tools")).is_none() {
            return Asked::cannot(NO_INSTRUCTIONS);
        }
        let budget = r.budget_micro_usd.min(self.max_call_micro_usd);
        let decider_dir = r.run_dir.join("decider");
        let case_dir = match make_case_dir(dreq, approval) {
            Ok(dir) => dir,
            Err(reason) => return Asked::cannot(reason),
        };
        let mut asked = Asked { lines: Vec::new(), cost_micro_usd: 0, model_calls: 0, result: Err(String::new()) };
        let mut attempt = 1u64;
        let mut invalid_reason = String::new();
        loop {
            let number = record_dirs(&decider_dir) + 1;
            if number > self.max_calls {
                asked.result = Err(format!("decider budget spent: {} decider calls (max_calls)", self.max_calls));
                return asked;
            }
            if budget <= asked.cost_micro_usd {
                asked.result = Err(format!("the per-call budget of {budget} micro USD is spent"));
                return asked;
            }
            let rec = decider_dir.join(format!("{number:03}-{}", safe_part(&r.request.id)));
            if let Err(e) = make_dir_in(r.run_dir, &rec) {
                asked.result = Err(format!("decider record directory: {e}"));
                return asked;
            }
            let retry = (attempt > 1).then(|| (attempt, invalid_reason.clone()));
            let mut body = ask_body(r, approval, &case_dir, &rec, retry);
            if let Some(class) = routed {
                requester::tag(&mut body, class, r.run_dir);
            }
            if let Some(level) = &self.autonomy {
                body["autonomy"] = json!(level);
            }
            if let Err(e) = write_atomic(&rec.join("decide-request.json"), format!("{body}\n").as_bytes()).and_then(|()| write_atomic(&rec.join("loom-decision.json"), format!("{}\n", r.decision).as_bytes())) {
                asked.result = Err(format!("decider request file: {e}"));
                return asked;
            }
            let reply = match self.reply(&body, &rec, dreq, budget - asked.cost_micro_usd) {
                Ok(reply) => reply,
                Err(reason) => {
                    asked.result = Err(reason);
                    return asked;
                }
            };
            if reply.get("adapter").and_then(Value::as_str) != Some("rule") && reply.get("dry") != Some(&Value::Bool(true)) {
                asked.model_calls += 1;
            }
            let line = |outcome: &str, reply: &Value, extra: Value| decision_line(r.decision, r.request, &approval.revision, outcome, reply, principal, extra);
            if reply.get("valid") == Some(&Value::Bool(false)) {
                invalid_reason = reply.get("invalid_reason").and_then(Value::as_str).unwrap_or("invalid").to_owned();
                if let Some(cost) = cost_micro(&reply) {
                    asked.cost_micro_usd = asked.cost_micro_usd.saturating_add(cost);
                }
                if attempt > *retries {
                    if final_escalate {
                        // The fallback of a question or a finding: its default (`invalid_reply_default`).
                        asked.lines.push(line("invalid_reply", &reply, json!({"attempt": attempt})));
                        let Some(answer) = default_of(r) else {
                            asked.result = Err(format!("invalid decider reply after {attempt} attempts ({}) and the request names no default to take", self.redact(&invalid_reason)));
                            return asked;
                        };
                        let taken = with_answer(&reply, &answer, vec![format!("invalid decider reply after {retries} retries ({}): the default taken", self.redact(&invalid_reason))]);
                        asked.lines.push(line("approve", &taken, json!({"fallback": "invalid_reply_default", "attempts": attempt})));
                        asked.result = Ok(Given { answer, principal: Some(principal.to_owned()) });
                    } else {
                        asked.lines.push(line("escalate", &reply, json!({"attempts": attempt})));
                        asked.result = Err(format!("invalid decider reply after {attempt} attempts: {}", self.redact(&invalid_reason)));
                    }
                    return asked;
                }
                asked.lines.push(line("invalid_reply", &reply, json!({"attempt": attempt})));
                attempt += 1;
                continue;
            }
            let Some(cost) = cost_micro(&reply) else {
                asked.result = Err("the decider's reply has no cost (cost_usd is absent, null or not a non-negative number)".to_owned());
                return asked;
            };
            asked.cost_micro_usd = asked.cost_micro_usd.saturating_add(cost);
            let attempts = if attempt > 1 { json!({"attempts": attempt}) } else { json!({}) };
            match reply.get("decision").and_then(Value::as_str) {
                Some("approve") => {
                    let Some(answer) = reply.get("answer").and_then(Value::as_str).filter(|a| !a.trim().is_empty()) else {
                        asked.result = Err("the decider approved and gave no answer".to_owned());
                        return asked;
                    };
                    asked.lines.push(line("approve", &reply, attempts));
                    asked.result = Ok(Given { answer: answer.to_owned(), principal: Some(principal.to_owned()) });
                }
                Some("escalate") => {
                    let reasons = reply.get("reasons").and_then(Value::as_array).map(|r| r.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")).unwrap_or_default();
                    if !final_escalate {
                        asked.lines.push(line("escalate", &reply, attempts));
                        asked.result = Err(format!("reserved for a person: the decider escalated (escalate stop): {}", self.redact(&reasons)));
                        return asked;
                    }
                    // escalate final: a question takes its default, a finding the `best` answer, else its default.
                    asked.lines.push(line("escalation_suppressed", &reply, json!({})));
                    let best = reply.get("best").and_then(Value::as_str).filter(|b| !b.is_empty()).filter(|_| r.request.kind == "finding");
                    let note = if best.is_some() { "best answer taken" } else { "the default taken" };
                    let Some(answer) = best.map(str::to_owned).or_else(|| default_of(r)) else {
                        asked.result = Err("the decider escalated and the request names no default to take (escalate final)".to_owned());
                        return asked;
                    };
                    let mut taken = with_answer(&reply, &answer, vec![format!("escalation suppressed (SDC_DECIDER_ESCALATE=final); {note}")]);
                    taken["usage"] = Value::Null;
                    taken["cost_usd"] = Value::Null;
                    asked.lines.push(line("approve", &taken, json!({"suppressed": true})));
                    asked.result = Ok(Given { answer, principal: Some(principal.to_owned()) });
                }
                Some("reject") => {
                    asked.lines.push(line("reject", &reply, attempts));
                    asked.result = Err("the decider rejected: a question or a finding takes an answer or an escalation, not a rejection".to_owned());
                }
                _ => asked.result = Err("the decider's reply has no decision approve or escalate".to_owned()),
            }
            return asked;
        }
    }
}

/// What the decider is asked about, read as an approval is: the case from Loom's decision, the
/// action, and the members the request line adds (`capability`, `subject`, `path`, `revision`).
fn ask_approval(r: &AskRequest<'_>) -> Result<Approval, String> {
    let case = text_member(r.decision, "case").ok_or("the decision names no case")?.to_owned();
    if !plain_name(&case) {
        return Err("the case name is not a plain name".to_owned());
    }
    let action = text_member(r.decision, "action").ok_or("the decision names no action (member `action` absent)")?.to_owned();
    let member = |name: &str| r.request.body.get(name).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned);
    let subject = member("subject").unwrap_or_default();
    let revision = member("revision")
        .or_else(|| case_artifacts(r.decision, &case).into_iter().find(|(name, _)| *name == subject).map(|(_, revision)| revision))
        .unwrap_or_else(|| "none".to_owned());
    Ok(Approval { case, action, capability: member("capability").or_else(|| text_member(r.decision, "capability").map(str::to_owned)), subject, revision, path: member("path"), close_outcome: None })
}

fn class_name(kind: &str) -> &'static str {
    if kind == "question" {
        "scope_changing_answer"
    } else {
        "finding"
    }
}

/// The request the decider reads for a question or a finding: the members the removed driver's request had,
/// then every other member
/// of the request line, which may override them but never `type`, `case_dir` or `record_dir`.
fn ask_body(r: &AskRequest<'_>, approval: &Approval, case_dir: &Path, record_dir: &Path, retry: Option<(u64, String)>) -> Value {
    let kind = r.request.kind.as_str();
    let mut request = json!({
        "case_dir": case_dir.display().to_string(),
        "action": approval.action,
        "capability": approval.capability,
        "subject": approval.subject,
        "path": approval.path,
        "revision": approval.revision,
        "question": null,
        "outcome": null,
        "position": {"claims": {}, "open_obligations": [], "legitimate_outcomes": []},
        "class": class_name(kind),
        "side": "delivering",
        "autonomy": AUTONOMY_DECIDES,
        "decisions_log": r.run_dir.join("decisions.jsonl").display().to_string(),
    });
    if let (Some(map), Some(line)) = (request.as_object_mut(), r.request.body.as_object()) {
        for (name, value) in line {
            if !matches!(name.as_str(), "type" | "id" | "case_dir" | "record_dir") {
                map.insert(name.clone(), value.clone());
            }
        }
    }
    request["type"] = json!(kind);
    request["case_dir"] = json!(case_dir.display().to_string());
    request["record_dir"] = json!(record_dir.display().to_string());
    if let Some((attempt, why)) = retry {
        request["retry"] = json!({"attempt": attempt, "invalid_reason": why});
    }
    request
}

/// The default of the question or finding asked about (`question.default`, `finding.default`).
fn default_of(r: &AskRequest<'_>) -> Option<String> {
    match r.request.body.get(&r.request.kind).and_then(|o| o.get("default")) {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.clone()),
        _ => None,
    }
}

/// `reply` with `answer` set and `reasons` put before its own.
fn with_answer(reply: &Value, answer: &str, mut reasons: Vec<String>) -> Value {
    let mut out = reply.clone();
    if let Some(own) = reply.get("reasons").and_then(Value::as_array) {
        reasons.extend(own.iter().filter_map(Value::as_str).map(str::to_owned));
    }
    out["answer"] = json!(answer);
    out["valid"] = json!(true);
    out["reasons"] = json!(reasons);
    out
}
