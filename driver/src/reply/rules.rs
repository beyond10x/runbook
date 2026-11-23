//! Rule checks: whether `rule:autopilot` takes an approval on the strength of program checks, and
//! what can only come after an action (`sdc-reply rule-approval`, `sdc-reply later`).
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use serde_json::{json, Value};

use super::records::rejections;
use super::{read_json, sorted};

/// The principal of an approval taken by the rule.
pub const RULE_PRINCIPAL: &str = "rule:autopilot";

/// Whether the argv of binding `b` starts a model's program: `sdc-node`.
pub fn model_step_argv(b: &Value) -> bool {
    b["argv"].as_array().is_some_and(|a| {
        a.iter().any(|x| x.as_str().and_then(|s| s.rsplit('/').next()) == Some("sdc-node"))
    })
}

/// Every `(claim id, is)` a condition names.
pub fn claim_conditions(f: &Value, out: &mut Vec<(String, String)>) {
    match f {
        Value::Object(m) => {
            if let Some(c) = m.get("claim").filter(|c| c.is_object()) {
                out.push((c["id"].as_str().unwrap_or("").into(), c["is"].as_str().unwrap_or("true").into()));
            }
            m.values().for_each(|x| claim_conditions(x, out));
        }
        Value::Array(a) => a.iter().for_each(|x| claim_conditions(x, out)),
        _ => {}
    }
}

/// Whether binding `b` is a program check of the evidence `w` (a claim's `true_when.evidence`):
/// bound, not a person's, not a model's, producing that kind about that subject.
pub fn program_check(b: &Value, w: &Value) -> Result<(), String> {
    if !b.is_object() {
        return Err("it has no binding".into());
    }
    if b["manual"] == true {
        return Err("it is a person's action, not a program check".into());
    }
    if !b["llm"].is_null() || model_step_argv(b) {
        return Err("it is run by a model, not a program check".into());
    }
    let kind = w["kind"].as_str().unwrap_or("");
    let produces = match &b["produces"] {
        Value::String(k) => vec![k.as_str()],
        Value::Array(a) => a.iter().filter_map(|k| k.as_str()).collect(),
        _ => vec![],
    };
    if !produces.contains(&kind) {
        return Err(format!("it does not produce {kind} (it produces {})", if produces.is_empty() { "nothing".into() } else { produces.join(", ") }));
    }
    if b["subject"] != w["subject"] {
        return Err(format!("it checks {}, the claim is about {}", b["subject"], w["subject"]));
    }
    Ok(())
}

/// The decision of `rule:autopilot` on an approval, or why the decider is asked: the data file
/// (rule-ends.json `approvals.<protocol>/<action>.by_rule`) names, for each claim the approval
/// rests on, the program check that records it. Each guard needs the claim true in the request's
/// position and a passing record of the check about the exact revision now (the evidence the
/// claim's `true_when` names, produced by that check, not set aside, with the upstream revisions it
/// was observed against still current), and no earlier rejection of this approval may exist.
pub fn rule_checks(req: &Value, ends: &Value) -> Result<(String, String, Vec<Value>), String> {
    let case_dir = PathBuf::from(req["case_dir"].as_str().ok_or("the request names no case_dir")?);
    let action = req["action"].as_str().ok_or("the request names no action")?;
    let (ir, bindings, case) = (read_json(&case_dir.join("ir.json")), read_json(&case_dir.join("bindings.json")), read_json(&case_dir.join("case.json")));
    let proto = ir["protocol"]["id"].as_str().or(ir["protocol"].as_str()).or(case["protocol"].as_str()).unwrap_or("");
    let key = format!("{proto}/{action}");
    let rule = &ends["approvals"][&key]["by_rule"];
    let guards = rule["guards"].as_array().filter(|g| !g.is_empty()).ok_or_else(|| format!("no rule takes {key}: it has no program check to rest on"))?;
    let revision = req["revision"].as_str().unwrap_or("");
    let position = &req["position"];
    // Every claim the action requires true is true now.
    let mut cond = Vec::new();
    claim_conditions(&ir["actions"][action]["precondition"], &mut cond);
    for (c, _) in cond.iter().filter(|(_, is)| is == "true") {
        let v = position["claims"][c]["value"].as_str().unwrap_or("unknown");
        if v != "true" {
            return Err(format!("{c} is {v}, not true"));
        }
    }
    let mut checks = Vec::new();
    for g in guards {
        let (claim, check) = (g["claim"].as_str().unwrap_or(""), g["check"].as_str().unwrap_or(""));
        if !cond.iter().any(|(c, is)| c == claim && is == "true") {
            return Err(format!("the rule's guard {claim} is not a condition of {action}"));
        }
        let w = &ir["claims"][claim]["true_when"]["evidence"];
        program_check(&bindings[check], w).map_err(|e| format!("{check}, which should record {claim}: {e}"))?;
        let (kind, subject) = (w["kind"].as_str().unwrap_or(""), w["subject"].as_str().unwrap_or(""));
        let current = case["artifacts"][subject]["revision"].as_str().unwrap_or("none");
        if req["subject"] == subject && current != revision {
            return Err(format!("the request is about {subject}@{revision}, the case is at {current}"));
        }
        let aside: Vec<&str> = position["claims"][claim]["excluded_evidence"].as_array().into_iter().flatten()
            .filter_map(|x| x["evidence"].as_str()).collect();
        let mut files: Vec<PathBuf> = fs::read_dir(case_dir.join("evidence")).into_iter().flatten().filter_map(|e| e.ok().map(|e| e.path())).collect();
        files.sort();
        let found = files.iter().map(|p| read_json(p)).rfind(|r| {
            let id = r["id"].as_str().unwrap_or("");
            r["kind"] == kind && r["subject"] == subject && r["result"] == w["result"] && r["subject_revision"] == current
                && id.ends_with(&format!("-{check}")) && !aside.contains(&id)
                && r["upstream_revisions"].as_object().into_iter().flatten()
                    .all(|(a, v)| case["artifacts"][a]["revision"].is_null() || &case["artifacts"][a]["revision"] == v)
        });
        let Some(r) = found else {
            return Err(format!("{check} has recorded no passing {kind} ({}) about {subject}@{current}", w["result"].as_str().unwrap_or("any result")));
        };
        checks.push(json!({"claim": claim, "check": check, "evidence": r["id"], "kind": kind, "result": r["result"],
            "subject": subject, "revision": current}));
    }
    let prior = rejections(&case_dir, action);
    if prior.as_array().is_some_and(|p| !p.is_empty()) {
        return Err(format!("an earlier rejection of {action} exists ({}): the decider checks the rework against it", prior[0]["record"].as_str().unwrap_or("?")));
    }
    Ok((key, rule["rule"].as_str().unwrap_or("program-checks").to_string(), checks))
}

/// `sdc-reply rule-approval`: see `Cmd::RuleApproval`. `{take: true, decision, rule}` where
/// `decision` is a decider reply (decisions.jsonl's shape) and `rule` names the rule, the checks and
/// the evidence ids; `{take: false, why}` leaves the approval to the decider.
pub fn rule_approval(req: &Value, ends: &Value) -> Value {
    sorted(match rule_checks(req, ends) {
        Err(why) => json!({"take": false, "covered": !why.starts_with("no rule takes"), "why": why}),
        Ok((key, name, checks)) => {
            let ids: Vec<&Value> = checks.iter().map(|c| &c["evidence"]).collect();
            let said: Vec<String> = checks.iter().map(|c| format!("{} by {} on {} ({}={} about {}@{})", c["claim"].as_str().unwrap_or(""),
                c["check"].as_str().unwrap_or(""), c["evidence"].as_str().unwrap_or(""), c["kind"].as_str().unwrap_or(""),
                c["result"].as_str().unwrap_or("recorded"), c["subject"].as_str().unwrap_or(""), c["revision"].as_str().unwrap_or(""))).collect();
            let rule = json!({"principal": RULE_PRINCIPAL, "rule": name, "approval": key, "checks": checks});
            json!({"take": true, "rule": rule, "decision": {"decision": "approve", "answer": null, "best": null,
                "reasons": [format!("taken by {RULE_PRINCIPAL} ({name}): the program checks passed on the exact revision: {}; no earlier rejection; no model was asked", said.join("; "))],
                "cites": ids, "kind": "rule", "model": null, "adapter": "rule", "usage": null, "cost_usd": 0, "valid": true, "dry": false,
                "rule": rule}})
        }
    })
}

/// The actions that may produce evidence `kind`.
pub fn producers<'a>(ir: &'a Value, kind: &Value) -> Vec<&'a str> {
    ir["actions"].as_object().into_iter().flatten()
        .filter(|(_, a)| a["may_produce"].as_array().is_some_and(|m| m.iter().any(|p| &p["evidence"] == kind)))
        .map(|(k, _)| k.as_str()).collect()
}

/// Claim `id` can be made true only through actions in `set`: its evidence has producers, all in
/// `set`; a claim defined over claims is read as a formula; a children claim never.
pub fn claim_needs(ir: &Value, id: &str, set: &BTreeSet<String>, depth: u8) -> bool {
    let w = &ir["claims"][id]["true_when"];
    if depth > 32 {
        return false;
    }
    if w["evidence"].is_object() {
        let p = producers(ir, &w["evidence"]["kind"]);
        !p.is_empty() && p.iter().all(|a| set.contains(*a))
    } else {
        formula_needs(ir, w, set, depth + 1)
    }
}

/// Formula `f` (a precondition, a discharge condition) holds only once a claim it requires true
/// is made true, which only actions in `set` can do: `all` when one part needs that, `any` when
/// every part does; a claim required to be anything but true (unknown, false) never needs it.
pub fn formula_needs(ir: &Value, f: &Value, set: &BTreeSet<String>, depth: u8) -> bool {
    if let Some(c) = f["claim"].as_object() {
        c["is"] == "true" && c["id"].as_str().is_some_and(|id| claim_needs(ir, id, set, depth))
    } else if let Some(all) = f["all"].as_array() {
        all.iter().any(|g| formula_needs(ir, g, set, depth))
    } else if let Some(any) = f["any"].as_array() {
        !any.is_empty() && any.iter().all(|g| formula_needs(ir, g, set, depth))
    } else {
        false
    }
}

/// `sdc-reply later`: see `Cmd::Later`.
pub fn later(ir: &Value, action: &str) -> Value {
    let mut reach: BTreeSet<String> = BTreeSet::from([action.to_string()]);
    loop {
        let add: Vec<String> = ir["actions"].as_object().into_iter().flatten()
            .filter(|(k, a)| !reach.contains(*k) && formula_needs(ir, &a["precondition"], &reach, 0))
            .map(|(k, _)| k.clone()).collect();
        if add.is_empty() {
            break;
        }
        reach.extend(add);
    }
    reach.remove(action);
    let mut claims: Vec<&String> = ir["claims"].as_object().into_iter().flatten()
        .filter(|(k, _)| claim_needs(ir, k, &reach, 0)).map(|(k, _)| k).collect();
    let mut obligations: Vec<&String> = ir["obligations"].as_object().into_iter().flatten()
        .filter(|(_, o)| formula_needs(ir, &o["discharged_when"], &reach, 0)).map(|(k, _)| k).collect();
    claims.sort();
    obligations.sort();
    sorted(json!({"action": action, "after_actions": reach, "claims": claims, "obligations": obligations}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn only_sdc_node_starts_a_model_step() {
        assert!(model_step_argv(&json!({"argv": ["sdc-node", "spec.author"]})));
        assert!(model_step_argv(&json!({"argv": ["/x/bin/sdc-node", "spec.author"]})));
        let old = ["llm-node", "sh"].join(".");
        assert!(!model_step_argv(&json!({"argv": [format!("{{dir}}/../../../bindings/tools/{old}"), "spec.author"]})));
        assert!(!model_step_argv(&json!({"argv": [old, "spec.author"]})));
        assert!(!model_step_argv(&json!({"argv": ["tools/sdc-tool.sh", "spec-domain"]})));
    }
    use serde_json::{json, Map, Value};
    use crate::reply::read_json;

    fn ir(name: &str) -> Value {
        read_json(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../protocol/compiled").join(format!("{name}.ir.json")))
    }

    #[test]
    fn health_is_met_after_release_publish() {
        let l = later(&ir("sdc-release"), "release.publish");
        assert_eq!(l["obligations"], json!(["health.required"]));
        assert_eq!(l["claims"], json!(["release.healthy", "release.observed", "rollback.complete"]));
        assert_eq!(l["after_actions"], json!(["release.observe", "release.rollback"]));
    }

    #[test]
    fn what_the_action_itself_settles_is_not_later() {
        // merge.approve makes merge.approved true itself (settled by the decision, not later);
        // change.merge needs it, so change.merged comes after.
        let l = later(&ir("sdc-story"), "merge.approve");
        assert!(!l["claims"].as_array().unwrap().iter().any(|c| c == "merge.approved"), "{l}");
        assert!(l["claims"].as_array().unwrap().iter().any(|c| c == "change.merged"), "{l}");
        assert_eq!(l["obligations"], json!([]));
    }

    // ---- rule-approval ---------------------------------------------------------------------------

    fn repo(p: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(p)
    }

    /// A case dir of protocol `proto` (its compiled IR and bindings from the repository) with the
    /// given artifact revisions and evidence records `(id, kind, result, subject, subject_revision)`.
    fn rule_case(name: &str, proto: &str, revisions: Value, evidence: &[(&str, &str, Value, &str, &str)]) -> PathBuf {
        let case = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-scratch/rule-approval").join(name);
        let _ = fs::remove_dir_all(&case);
        fs::create_dir_all(case.join("evidence")).unwrap();
        fs::copy(repo(&format!("protocol/compiled/{proto}.ir.json")), case.join("ir.json")).unwrap();
        fs::copy(repo(&format!("driver/examples/sdc/{proto}.bindings.json")), case.join("bindings.json")).unwrap();
        let arts: Map<String, Value> = revisions.as_object().unwrap().iter().map(|(k, v)| (k.clone(), json!({"revision": v}))).collect();
        fs::write(case.join("case.json"), json!({"id": "IDEA-1.x", "protocol": proto.replacen('-', ".", 1), "artifacts": arts}).to_string()).unwrap();
        for (id, kind, result, subject, rev) in evidence {
            fs::write(case.join("evidence").join(format!("{id}.json")), json!({"format": "canon-evidence/1", "id": id, "kind": kind,
                "result": result, "subject": subject, "subject_revision": rev, "upstream_revisions": null}).to_string()).unwrap();
        }
        case
    }

    fn request(case: &Path, action: &str, subject: &str, revision: &str, claims: &[(&str, &str)]) -> Value {
        let claims: Map<String, Value> = claims.iter().map(|(k, v)| (k.to_string(), json!({"value": v}))).collect();
        json!({"type": "approval", "case_dir": case, "action": action, "subject": subject, "revision": revision,
            "position": {"claims": claims, "open_obligations": []}})
    }

    fn ends() -> Value {
        read_json(&repo("driver/examples/sdc/rule-ends.json"))
    }

    fn context_case(name: &str, evidence: &[(&str, &str, Value, &str, &str)]) -> (PathBuf, Value) {
        let case = rule_case(name, "sdc-direction", json!({"context": "t1", "requirements": "r1"}), evidence);
        let req = request(&case, "context.approve", "context", "t1", &[("context.valid", "true"), ("context.decided", "unknown")]);
        (case, req)
    }

    #[test]
    fn a_passed_check_at_the_exact_revision_is_taken_by_the_rule() {
        let (_, req) = context_case("taken", &[("e0004-context.check", "context_defaults", json!("valid"), "context", "t1")]);
        let v = rule_approval(&req, &ends());
        assert_eq!(v["take"], true, "{v}");
        assert_eq!(v["decision"]["decision"], "approve");
        assert_eq!(v["decision"]["model"], Value::Null);
        assert_eq!(v["decision"]["cost_usd"], 0);
        assert_eq!(v["decision"]["cites"], json!(["e0004-context.check"]));
        let r = &v["decision"]["rule"];
        assert_eq!(r["principal"], "rule:autopilot");
        assert_eq!(r["checks"][0]["check"], "context.check");
        assert_eq!(r["checks"][0]["claim"], "context.valid");
        assert_eq!(r["checks"][0]["evidence"], "e0004-context.check");
        assert_eq!(r["checks"][0]["revision"], "t1");
    }

    #[test]
    fn a_failed_missing_or_stale_check_leaves_it_to_the_decider() {
        // The check ran and found something: no passing record.
        let (_, req) = context_case("failed", &[("e0004-context.check", "context_defaults", json!("finding"), "context", "t1")]);
        let v = rule_approval(&req, &ends());
        assert_eq!(v["take"], false, "{v}");
        assert!(v["why"].as_str().unwrap().contains("context.check"), "{v}");
        assert_eq!(v["covered"], true, "a rule exists, it declined: the line says why");
        // No record at all.
        let (_, req) = context_case("missing", &[]);
        assert_eq!(rule_approval(&req, &ends())["take"], false);
        // The only passing record is about an earlier revision: the revision changed after the check.
        let (_, req) = context_case("stale", &[("e0004-context.check", "context_defaults", json!("valid"), "context", "t0")]);
        let v = rule_approval(&req, &ends());
        assert_eq!(v["take"], false, "{v}");
        assert!(v["why"].as_str().unwrap().contains("t1"), "{v}");
        // The request's revision is not the case's current one.
        let (case, _) = context_case("moved", &[("e0004-context.check", "context_defaults", json!("valid"), "context", "t1")]);
        let req = request(&case, "context.approve", "context", "t0", &[("context.valid", "true")]);
        assert_eq!(rule_approval(&req, &ends())["take"], false);
        // A passing record Canon set aside is no record either.
        let (case, mut req) = context_case("excluded", &[("e0004-context.check", "context_defaults", json!("valid"), "context", "t1")]);
        req["position"]["claims"]["context.valid"]["excluded_evidence"] = json!([{"evidence": "e0004-context.check", "reason": "invalidated"}]);
        assert_eq!(rule_approval(&req, &ends())["take"], false, "{case:?}");
    }

    #[test]
    fn the_claims_the_action_requires_must_be_true() {
        let (case, _) = context_case("claim", &[("e0004-context.check", "context_defaults", json!("valid"), "context", "t1")]);
        let req = request(&case, "context.approve", "context", "t1", &[("context.valid", "unknown")]);
        let v = rule_approval(&req, &ends());
        assert_eq!(v["take"], false, "{v}");
        assert!(v["why"].as_str().unwrap().contains("context.valid"), "{v}");
    }

    #[test]
    fn an_earlier_rejection_is_asked_of_the_decider() {
        let (case, req) = context_case("rejected", &[("e0004-context.check", "context_defaults", json!("valid"), "context", "t1"),
            ("e0009-context.approve", "context_approval", json!("rejected"), "context", "t0")]);
        fs::write(case.join("events.jsonl"), json!({"event": "action_finished", "action": "context.approve", "evidence": ["e0009-context.approve"]}).to_string() + "\n").unwrap();
        let v = rule_approval(&req, &ends());
        assert_eq!(v["take"], false, "{v}");
        assert!(v["why"].as_str().unwrap().contains("rejection"), "{v}");
    }

    #[test]
    fn a_check_that_is_a_model_or_a_person_is_no_program_check() {
        let (case, req) = context_case("model", &[("e0004-context.check", "context_defaults", json!("valid"), "context", "t1")]);
        for (patch, word) in [(json!({"manual": true}), "person"), (json!({"llm": {"model": "m"}}), "model"),
            (json!({"produces": "other_kind"}), "other_kind")] {
            let mut b = read_json(&case.join("bindings.json"));
            for (k, x) in patch.as_object().unwrap() {
                b["context.check"][k] = x.clone();
            }
            fs::write(case.join("bindings.json"), b.to_string()).unwrap();
            let v = rule_approval(&req, &ends());
            assert_eq!(v["take"], false, "{v}");
            assert!(v["why"].as_str().unwrap().contains(word), "{word}: {v}");
            fs::copy(repo("driver/examples/sdc/sdc-direction.bindings.json"), case.join("bindings.json")).unwrap();
        }
    }

    #[test]
    fn the_vision_has_no_program_check_and_is_never_taken() {
        let (case, _) = context_case("vision", &[("e0004-context.check", "context_defaults", json!("valid"), "context", "t1")]);
        let req = request(&case, "vision.approve", "vision", "v1", &[("context.valid", "true"), ("context.approved", "true")]);
        let v = rule_approval(&req, &ends());
        assert_eq!(v["take"], false, "{v}");
        assert!(v["why"].as_str().unwrap().contains("no rule"), "{v}");
        assert_eq!(v["covered"], false);
        assert!(ends()["approvals"]["sdc.direction/vision.approve"]["by_rule"].is_null());
    }

    #[test]
    fn the_roadmap_is_taken_on_its_validation() {
        let case = rule_case("roadmap", "sdc-direction", json!({"context": "t1", "vision": "v1", "roadmap": "m1"}),
            &[("e0008-roadmap.validate", "roadmap_validation", json!("valid"), "roadmap", "m1")]);
        let claims = [("context.valid", "true"), ("vision.approved", "true"), ("roadmap.valid", "true"), ("roadmap.decided", "unknown")];
        let v = rule_approval(&request(&case, "roadmap.approve", "roadmap", "m1", &claims), &ends());
        assert_eq!(v["take"], true, "{v}");
        assert_eq!(v["decision"]["rule"]["checks"][0]["check"], "roadmap.validate");
    }

    #[test]
    fn the_release_needs_its_gate_its_build_and_its_acceptance() {
        let all = [("e0004-spec.gate", "ess_gate", json!("passed"), "spec", "s1"),
            ("e0008-release.build", "build_provenance", Value::Null, "release", "d1"),
            ("e0013-acceptance.run", "release_conformance", json!("passed"), "release", "d1")];
        let claims = [("gate.passed", "true"), ("release.built", "true"), ("release.accepted", "true")];
        let case = rule_case("release", "sdc-release", json!({"spec": "s1", "release": "d1"}), &all);
        let v = rule_approval(&request(&case, "release.publish", "release", "d1", &claims), &ends());
        assert_eq!(v["take"], true, "{v}");
        let checks: Vec<&str> = v["decision"]["rule"]["checks"].as_array().unwrap().iter().filter_map(|c| c["check"].as_str()).collect();
        assert_eq!(checks, vec!["spec.gate", "release.build", "acceptance.run"]);
        assert_eq!(v["decision"]["cites"], json!(["e0004-spec.gate", "e0008-release.build", "e0013-acceptance.run"]));
        // The conformance run failed: the decider is asked.
        let bad = [all[0].clone(), all[1].clone(), ("e0013-acceptance.run", "release_conformance", json!("failed"), "release", "d1")];
        let case = rule_case("release-failed", "sdc-release", json!({"spec": "s1", "release": "d1"}), &bad);
        let v = rule_approval(&request(&case, "release.publish", "release", "d1", &claims), &ends());
        assert_eq!(v["take"], false, "{v}");
        assert!(v["why"].as_str().unwrap().contains("acceptance.run"), "{v}");
        // The release was rebuilt after its acceptance run: that run is about the old revision.
        let case = rule_case("release-stale", "sdc-release", json!({"spec": "s1", "release": "d2"}), &all);
        let v = rule_approval(&request(&case, "release.publish", "release", "d2", &claims), &ends());
        assert_eq!(v["take"], false, "{v}");
    }

    #[test]
    fn every_rule_in_the_data_names_a_program_check_of_its_protocol() {
        // The verification the data file stands on: for each approval with `by_rule`, every guard's
        // claim is a condition of the action, and the check is a bound program action (no model,
        // no person) producing that claim's evidence kind about that claim's subject.
        let e = ends();
        let mut seen = 0;
        for (qa, end) in e["approvals"].as_object().unwrap() {
            let Some(rule) = end.get("by_rule").filter(|r| !r.is_null()) else { continue };
            let (proto, action) = qa.split_once('/').unwrap();
            let name = proto.replacen("sdc.", "sdc-", 1);
            let ir = read_json(&repo(&format!("protocol/compiled/{name}.ir.json")));
            let b = read_json(&repo(&format!("driver/examples/sdc/{name}.bindings.json")));
            let mut cond = Vec::new();
            claim_conditions(&ir["actions"][action]["precondition"], &mut cond);
            assert!(rule["guards"].as_array().is_some_and(|g| !g.is_empty()), "{qa}: no guards");
            for g in rule["guards"].as_array().unwrap() {
                let (claim, check) = (g["claim"].as_str().unwrap(), g["check"].as_str().unwrap());
                assert!(cond.iter().any(|(c, is)| c == claim && is == "true"), "{qa}: {claim} is no condition of {action}");
                let w = &ir["claims"][claim]["true_when"]["evidence"];
                assert!(program_check(&b[check], w).is_ok(), "{qa}: {check}: {:?}", program_check(&b[check], w));
                seen += 1;
            }
        }
        assert!(seen >= 5, "the data covers context, roadmap and release: {seen} guards");
    }
}
