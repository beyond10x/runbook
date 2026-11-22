//! sdc-check-routes: the reject and cap routes of an unattended run, checked statically (D148,
//! docs/reject-routes.md). Canon's `simulate` exhausts a protocol's states; what it cannot see
//! lives in the bindings and the runner: which result of which action sends
//! which writer back, the caps, the reject limit and the ends by rule. This reads the compiled IR
//! of every protocol, every bindings file, node-classes.json, decision-classes.json and the
//! declared rule ends (driver/examples/sdc/rule-ends.json), and prints one line per state where
//! nothing points to a next step:
//!
//!   negative-result  a check result no claim accepts, which no writer's `retry_when` names, no
//!                    rule end covers and from which no outcome or write action follows
//!   stale-retry      a `retry_when` kind no bound action of the protocol produces
//!   not-handed       a `retry_when` result whose producer's log does not reach the writer
//!   own-failure      an LLM writer whose failed run's log does not reach its next run
//!   shared-cap       one `max_runs` shared by different retry kinds, or by reworks
//!   inert-cap        a `max_runs` on a node the driver does not cap
//!   unrecorded-exit  an LLM observation with exits its `results` do not map (no record: stuck)
//!   rework-limit     an approval with no end after its last rework
//!   no-reject-route  a decision class whose reject has no route
//!
//!   sdc-check-routes [--root <repository>] [--doc <markdown>]
//!
//! Exit 0 with no finding, 1 with findings. With `--doc`, the findings must equal the fenced
//! block under the heading "## Check output" of that file, line by line: exit 0 when they do,
//! 1 (with a diff) when they do not, so a fix that removes a finding must remove its line too.
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn read(p: &Path) -> Result<Value, String> {
    let s = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    serde_json::from_str(&s).map_err(|e| format!("{}: {e}", p.display()))
}

/// One protocol: its name (`sdc.story`), its compiled IR and its bindings.
struct Protocol {
    name: String,
    ir: Value,
    bindings: Value,
}

/// Every `{claim: {id, is}}` in a condition, as (id, is).
fn claim_refs(v: &Value, out: &mut Vec<(String, String)>) {
    match v {
        Value::Object(m) => {
            if let Some(c) = m.get("claim").filter(|c| c.is_object()) {
                out.push((c["id"].as_str().unwrap_or("").into(), c["is"].as_str().unwrap_or("true").into()));
            }
            m.values().for_each(|x| claim_refs(x, out));
        }
        Value::Array(a) => a.iter().for_each(|x| claim_refs(x, out)),
        _ => {}
    }
}

fn kinds_of(b: &Value) -> Vec<String> {
    match &b["produces"] {
        Value::String(k) => vec![k.clone()],
        Value::Array(a) => a.iter().filter_map(|k| k.as_str().map(String::from)).collect(),
        _ => vec![],
    }
}

/// The results a binding records for kind `k` (a map per kind when it produces several).
fn results_of(b: &Value, k: &str, several: bool) -> BTreeSet<String> {
    b["results"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(_, r)| if several { r[k].as_str() } else { r.as_str() }.map(String::from))
        .collect()
}

fn bound(p: &Protocol) -> impl Iterator<Item = (&String, &Value)> {
    p.bindings.as_object().into_iter().flatten().filter(|(a, _)| !a.starts_with('$'))
}

fn is_llm_node(b: &Value) -> bool {
    sdc_tools::reply::rules::model_step_argv(b)
}

fn check(protocols: &[Protocol], classes: &Value, ends: &Value) -> Vec<String> {
    let mut f = Vec::new();
    let drv = &ends["driver"];
    let capped: Vec<&str> = drv["capped_effects"].as_array().into_iter().flatten().filter_map(|e| e.as_str()).collect();
    for p in protocols {
        let ir = &p.ir;
        let effect = |a: &str| ir["actions"][a]["effect"].as_str().unwrap_or("").to_string();
        let claim_kind = |c: &str| ir["claims"][c]["true_when"]["evidence"]["kind"].as_str().map(String::from);
        let producer = |k: &str| bound(p).find(|(_, b)| kinds_of(b).iter().any(|x| x == k)).map(|(a, b)| (a.clone(), b.clone()));
        // negative-result
        for (a, b) in bound(p) {
            if b["manual"] == true {
                continue; // a person's or the decider's result: no-reject-route below
            }
            let ks = kinds_of(b);
            for k in &ks {
                let wanted: Vec<&str> = ir["claims"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .filter(|(_, c)| c["true_when"]["evidence"]["kind"] == k.as_str())
                    .filter_map(|(_, c)| c["true_when"]["evidence"]["result"].as_str())
                    .collect();
                if wanted.is_empty() {
                    continue; // no claim turns on a result of this kind
                }
                for r in results_of(b, k, ks.len() > 1) {
                    if wanted.contains(&r.as_str()) {
                        continue;
                    }
                    let retried = bound(p).any(|(_, w)| w["retry_when"][k].as_array().is_some_and(|rs| rs.iter().any(|x| x == r.as_str())));
                    let finding = r == "finding" && p.bindings["amend.upstream"]["handoff"].is_string();
                    let declared = ends["checks"].as_array().into_iter().flatten().any(|e| {
                        e["protocol"] == p.name.as_str() && e["kind"] == k.as_str() && e["result"] == r.as_str()
                    });
                    let false_claim = |cond: &Value| {
                        let mut refs = Vec::new();
                        claim_refs(cond, &mut refs);
                        refs.iter().any(|(c, is)| is == "false" && claim_kind(c).as_deref() == Some(k.as_str()))
                    };
                    let outcome = ir["outcomes"].as_object().into_iter().flatten().any(|(_, o)| false_claim(&o["requires"]));
                    let writer = bound(p).any(|(w, _)| effect(w) == "write" && false_claim(&ir["actions"][w.as_str()]["precondition"]));
                    if !(retried || finding || declared || outcome || writer) {
                        f.push(format!("negative-result {} {a} {k}={r}: no writer's retry_when names it, no rule end covers it, no outcome or write follows", p.name));
                    }
                }
            }
        }
        for (w, b) in bound(p) {
            let Some(rw) = b["retry_when"].as_object() else { continue };
            // stale-retry, not-handed
            for (k, rs) in rw {
                let Some((pa, pb)) = producer(k) else {
                    f.push(format!("stale-retry {} {w}: retry_when names {k}, which no bound action produces", p.name));
                    continue;
                };
                for r in rs.as_array().into_iter().flatten().filter_map(|r| r.as_str()) {
                    let handed = is_llm_node(b)
                        && (r != "rejected" || pb["manual"] == true || pb.get("llm").is_some() || drv["rejection_logs_fed_back"] == true);
                    if !handed {
                        f.push(format!("not-handed {} {w}: retry_when {k}={r}, but the log of {pa} does not reach it", p.name));
                    }
                }
            }
            // shared-cap
            if effect(w) == "write" {
                let kinds: Vec<&String> = rw.keys().collect();
                let checks = kinds.iter().filter(|k| producer(k).is_some_and(|(_, pb)| pb["manual"] != true)).count();
                if checks > 1 && drv["cap_per_kind"] != true {
                    f.push(format!("shared-cap {} {w}: {checks} retry kinds share max_runs {}", p.name, b["max_runs"].as_u64().unwrap_or(8)));
                }
                for k in kinds.iter().filter(|k| producer(k).is_some_and(|(_, pb)| pb["manual"] == true)) {
                    if drv["rejection_is_arrival"] != true {
                        f.push(format!("shared-cap {} {w}: reworks after a rejected {k} count against max_runs {}", p.name, b["max_runs"].as_u64().unwrap_or(8)));
                    }
                }
            }
        }
        for (a, b) in bound(p) {
            // own-failure
            if is_llm_node(b) && effect(a) == "write" && drv["own_failure_fed_back"] != true {
                f.push(format!("own-failure {} {a}: a failed run's log does not reach its next run", p.name));
            }
            // inert-cap
            if b.get("max_runs").is_some() && !capped.contains(&effect(a).as_str()) {
                f.push(format!("inert-cap {} {a}: max_runs {} on a node with effect {}, which the driver does not cap", p.name, b["max_runs"], effect(a)));
            }
            // unrecorded-exit
            if is_llm_node(b) && effect(a) != "write" && !kinds_of(b).is_empty() && b["results"].get("*").is_none() {
                f.push(format!("unrecorded-exit {} {a}: an exit its results do not map records nothing, and the case ends stuck", p.name));
            }
        }
    }
    // rework-limit and no-reject-route, per decision class
    for (cls, c) in classes["classes"].as_object().into_iter().flatten() {
        let ty = c["type"].as_str().unwrap_or("");
        for qa in c["actions"].as_array().into_iter().flatten().filter_map(|x| x.as_str()) {
            let (pname, act) = qa.split_once('/').unwrap_or(("", qa));
            if ty == "close" {
                if ends["close"][act].is_null() {
                    f.push(format!("no-reject-route {qa} ({cls}): the decider is asked and its reject has no route"));
                }
                continue;
            }
            let Some(p) = protocols.iter().find(|p| p.name == pname) else { continue };
            let b = &p.bindings[act];
            if b.is_null() {
                continue; // unbound: never offered
            }
            if ty == "question" {
                if act == "scope.answer" && ends["questions"]["reject"].is_null() {
                    f.push(format!("no-reject-route {qa} ({cls}): a question's reject has no route"));
                }
                if act != "scope.answer" {
                    continue;
                }
            }
            let manual = b["manual"] == true;
            let caps = p.ir["actions"][act]["requires"].as_array().is_some_and(|r| !r.is_empty());
            if !(manual || caps || ty == "review") {
                continue; // not a decision point (change.merge runs on the approval)
            }
            let ks = kinds_of(b);
            let k = ks.first().cloned().unwrap_or_default();
            let recordable = results_of(b, &k, false).contains("rejected");
            let reworked = recordable && bound(p).any(|(_, w)| w["retry_when"][&k].as_array().is_some_and(|rs| rs.iter().any(|x| x == "rejected")));
            let end = &ends["approvals"][qa];
            let rule_end = end["reject"]["outcome"].as_str().is_some_and(|o| {
                p.ir["outcomes"][o]["requires"]["decision"] == end["reject"]["decision"]
            });
            if !reworked && !rule_end {
                f.push(format!("no-reject-route {qa} ({cls}): a reject is {} and no rule end covers it",
                    if recordable { "recorded but reworked by no writer" } else { "not recordable" }));
            } else if reworked && manual && end["after_limit"].is_null() {
                f.push(format!("rework-limit {qa} ({cls}): rejection {} ends in the stop rework_spent, no end by rule",
                    ends["rework_limit"].as_u64().unwrap_or(2) + 1));
            }
        }
    }
    // rule-guard: an approval taken by rule (`by_rule`) rests on program checks; each guard's claim
    // must be a condition of the action and its check a bound program action (no person, no model)
    // that produces the claim's evidence kind about the claim's subject.
    for (qa, end) in ends["approvals"].as_object().into_iter().flatten() {
        let Some(guards) = end["by_rule"]["guards"].as_array() else { continue };
        let (pname, act) = qa.split_once('/').unwrap_or(("", qa));
        let Some(p) = protocols.iter().find(|p| p.name == pname) else {
            f.push(format!("rule-guard {qa}: no protocol {pname}"));
            continue;
        };
        if p.ir["actions"][act].is_null() {
            f.push(format!("rule-guard {qa}: the protocol has no action {act}"));
            continue;
        }
        let mut cond = Vec::new();
        claim_refs(&p.ir["actions"][act]["precondition"], &mut cond);
        if guards.is_empty() {
            f.push(format!("rule-guard {qa}: a rule with no guard rests on nothing"));
        }
        for g in guards {
            let (claim, check) = (g["claim"].as_str().unwrap_or(""), g["check"].as_str().unwrap_or(""));
            let w = &p.ir["claims"][claim]["true_when"]["evidence"];
            let b = &p.bindings[check];
            let why = if !cond.iter().any(|(c, is)| c == claim && is == "true") {
                Some(format!("{claim} is not a condition of {act}"))
            } else if !w.is_object() {
                Some(format!("{claim} is no evidence claim"))
            } else if !b.is_object() {
                Some(format!("{check} is not bound"))
            } else if b["manual"] == true || is_llm_node(b) || !b["llm"].is_null() {
                Some(format!("{check} is a person's or a model's action, not a program check"))
            } else if !kinds_of(b).iter().any(|k| w["kind"] == k.as_str()) {
                Some(format!("{check} does not produce {}", w["kind"].as_str().unwrap_or("")))
            } else if b["subject"] != w["subject"] {
                Some(format!("{check} checks {}, {claim} is about {}", b["subject"], w["subject"]))
            } else {
                None
            };
            if let Some(why) = why {
                f.push(format!("rule-guard {qa}: {why}"));
            }
        }
    }
    if ends["findings"]["refused_by_check"].is_null() {
        f.push("no-reject-route amend.upstream (finding): a decision the program check refuses ends in the stop decision_not_takeable".into());
    }
    f
}

/// The lines of the fenced block under "## Check output".
fn doc_block(doc: &str) -> Option<Vec<String>> {
    let rest = doc.split("\n## Check output").nth(1)?;
    let start = rest.find("```")?;
    let body = &rest[start + 3..];
    let body = &body[body.find('\n')? + 1..];
    let end = body.find("```")?;
    Some(body[..end].lines().map(String::from).filter(|l| !l.is_empty()).collect())
}

fn load(root: &Path) -> Result<(Vec<Protocol>, Value, Value), String> {
    let ex = root.join("driver/examples/sdc");
    let mut protocols = Vec::new();
    let mut files: Vec<PathBuf> = std::fs::read_dir(root.join("protocol/compiled"))
        .map_err(|e| format!("protocol/compiled: {e}"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().ends_with(".ir.json"))
        .collect();
    files.sort();
    for irp in files {
        let stem = irp.file_name().unwrap().to_string_lossy().trim_end_matches(".ir.json").to_string();
        let bp = ex.join(format!("{stem}.bindings.json"));
        if !bp.exists() {
            continue;
        }
        let ir = read(&irp)?;
        let name = ir["protocol"].as_str().map(String::from).unwrap_or_else(|| stem.replacen('-', ".", 1));
        protocols.push(Protocol { name, ir, bindings: read(&bp)? });
    }
    Ok((protocols, read(&ex.join("decision-classes.json"))?, read(&ex.join("rule-ends.json"))?))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let opt = |n: &str| args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned();
    let root = opt("--root").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".."));
    let (protocols, classes, ends) = match load(&root) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("sdc-check-routes: {e}");
            return ExitCode::from(2);
        }
    };
    let found = check(&protocols, &classes, &ends);
    let Some(doc) = opt("--doc") else {
        found.iter().for_each(|l| println!("{l}"));
        return if found.is_empty() { ExitCode::SUCCESS } else { ExitCode::from(1) };
    };
    let text = std::fs::read_to_string(&doc).unwrap_or_default();
    let Some(listed) = doc_block(&text) else {
        eprintln!("sdc-check-routes: {doc} has no fenced block under \"## Check output\"");
        return ExitCode::from(2);
    };
    let extra: Vec<&String> = found.iter().filter(|l| !listed.contains(l)).collect();
    let gone: Vec<&String> = listed.iter().filter(|l| !found.contains(l)).collect();
    for l in &extra {
        println!("+ {l}  (found, not in {doc})");
    }
    for l in &gone {
        println!("- {l}  (in {doc}, no longer found: remove its line)");
    }
    if extra.is_empty() && gone.is_empty() {
        println!("routes: {} findings, all listed in {doc}", found.len());
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proto(ir: Value, bindings: Value) -> Vec<Protocol> {
        vec![Protocol { name: "t.p".into(), ir, bindings }]
    }
    fn ends(extra: Value) -> Value {
        let mut e = json!({"driver": {"capped_effects": ["write"], "cap_per_kind": true, "rejection_is_arrival": true,
            "own_failure_fed_back": true, "rejection_logs_fed_back": true}, "rework_limit": 2,
            "questions": {"reject": "refused"}, "findings": {"refused_by_check": "retry"}, "close": {}, "approvals": {}, "checks": []});
        if let (Some(m), Some(x)) = (e.as_object_mut(), extra.as_object()) {
            for (k, v) in x {
                m.insert(k.clone(), v.clone());
            }
        }
        e
    }
    fn ir() -> Value {
        json!({"protocol": "t.p",
            "claims": {"doc.ok": {"true_when": {"evidence": {"kind": "doc_check", "result": "valid"}}},
                       "doc.approved": {"true_when": {"evidence": {"kind": "doc_approval", "result": "approved"}}}},
            "actions": {"doc.write": {"effect": "write"}, "doc.check": {"effect": "read"},
                        "doc.approve": {"effect": "write", "requires": [{"capability": "owner.approve"}]}},
            "outcomes": {"passed": {"requires": {"claim": {"id": "doc.ok", "is": "true"}}}}})
    }
    const LLM: &str = "sdc-node";
    fn no_classes() -> Value {
        json!({"classes": {}})
    }

    #[test]
    fn a_failed_check_nobody_reworks_is_a_finding() {
        let b = json!({"doc.write": {"argv": [LLM, "doc.write"], "llm": {}},
                       "doc.check": {"produces": "doc_check", "results": {"0": "valid", "*": "invalid"}}});
        let f = check(&proto(ir(), b), &no_classes(), &ends(json!({})));
        assert_eq!(f, vec!["negative-result t.p doc.check doc_check=invalid: no writer's retry_when names it, no rule end covers it, no outcome or write follows"]);
    }

    #[test]
    fn retry_when_or_a_rule_end_routes_it() {
        let b = json!({"doc.write": {"argv": [LLM, "doc.write"], "llm": {}, "retry_when": {"doc_check": ["invalid"]}},
                       "doc.check": {"produces": "doc_check", "results": {"0": "valid", "*": "invalid"}}});
        assert!(check(&proto(ir(), b), &no_classes(), &ends(json!({}))).is_empty());
        let b = json!({"doc.check": {"produces": "doc_check", "results": {"0": "valid", "*": "invalid"}}});
        let e = ends(json!({"checks": [{"protocol": "t.p", "kind": "doc_check", "result": "invalid", "end": "fixture"}]}));
        assert!(check(&proto(ir(), b), &no_classes(), &e).is_empty());
    }

    #[test]
    fn caps_feedback_and_inert_caps() {
        let b = json!({"doc.write": {"argv": [LLM, "doc.write"], "llm": {}, "max_runs": 3,
                         "retry_when": {"doc_check": ["invalid"], "doc_review": ["rejected"], "gone": ["x"]}},
                       "doc.check": {"produces": "doc_check", "results": {"0": "valid", "*": "invalid"}, "max_runs": 3},
                       "doc.review": {"argv": ["sdc-tool.sh", "review"], "produces": "doc_review", "results": {"0": "approved", "*": "rejected"}}});
        let mut i = ir();
        i["actions"]["doc.review"] = json!({"effect": "read"});
        let e = ends(json!({"driver": {"capped_effects": ["write"], "cap_per_kind": false, "rejection_is_arrival": true,
            "own_failure_fed_back": false, "rejection_logs_fed_back": false}}));
        let f = check(&proto(i, b), &no_classes(), &e);
        let want = [
            "stale-retry t.p doc.write: retry_when names gone, which no bound action produces",
            "not-handed t.p doc.write: retry_when doc_review=rejected, but the log of doc.review does not reach it",
            "shared-cap t.p doc.write: 2 retry kinds share max_runs 3",
            "own-failure t.p doc.write: a failed run's log does not reach its next run",
            "inert-cap t.p doc.check: max_runs 3 on a node with effect read, which the driver does not cap",
        ];
        for w in want {
            assert!(f.iter().any(|l| l == w), "missing {w} in {f:?}");
        }
        assert_eq!(f.len(), want.len(), "{f:?}");
    }

    #[test]
    fn approvals_need_a_rework_or_an_end() {
        let classes = json!({"classes": {"doc": {"type": "approval", "actions": ["t.p/doc.approve"]}}});
        let approve = json!({"manual": true, "produces": "doc_approval", "results": {"0": "approved", "6": "rejected"}});
        let b = json!({"doc.approve": approve});
        assert_eq!(check(&proto(ir(), b), &classes, &ends(json!({}))),
            vec!["no-reject-route t.p/doc.approve (doc): a reject is recorded but reworked by no writer and no rule end covers it"]);
        let b = json!({"doc.approve": approve, "doc.write": {"argv": [LLM, "doc.write"], "llm": {}, "retry_when": {"doc_approval": ["rejected"]}}});
        assert_eq!(check(&proto(ir(), b.clone()), &classes, &ends(json!({}))),
            vec!["rework-limit t.p/doc.approve (doc): rejection 3 ends in the stop rework_spent, no end by rule"]);
        let e = ends(json!({"approvals": {"t.p/doc.approve": {"after_limit": {"decision": "explicitly_dropped"}}}}));
        assert!(check(&proto(ir(), b), &classes, &e).is_empty());
    }

    #[test]
    fn a_capability_with_a_rule_end_is_routed() {
        let classes = json!({"classes": {"doc": {"type": "approval", "actions": ["t.p/doc.approve"]}}});
        let mut i = ir();
        i["outcomes"]["withheld"] = json!({"requires": {"decision": "explicitly_withheld"}});
        let b = json!({"doc.approve": {"argv": ["publish"], "produces": "doc_approval", "results": {"0": null}}});
        assert_eq!(check(&proto(i.clone(), b.clone()), &classes, &ends(json!({}))).len(), 1);
        let e = ends(json!({"approvals": {"t.p/doc.approve": {"reject": {"decision": "explicitly_withheld", "outcome": "withheld"}}}}));
        assert!(check(&proto(i, b), &classes, &e).is_empty());
    }

    #[test]
    fn a_rule_guard_must_be_a_program_check_of_a_condition() {
        let classes = json!({"classes": {}});
        let guard = |claim: &str, check: &str| ends(json!({"approvals": {"t.p/doc.approve": {"by_rule": {"guards": [{"claim": claim, "check": check}]}}}}));
        let mut i = ir();
        i["actions"]["doc.approve"]["precondition"] = json!({"claim": {"id": "doc.ok", "is": "true"}});
        i["claims"]["doc.ok"]["true_when"]["evidence"]["subject"] = json!("doc");
        let b = json!({"doc.check": {"argv": ["sdc-tool.sh", "check"], "produces": "doc_check", "subject": "doc", "results": {"0": "valid", "*": "invalid"}}});
        let run = |i: &Value, b: &Value, e: &Value| check(&proto(i.clone(), b.clone()), &classes, e).into_iter().filter(|l| l.starts_with("rule-guard")).collect::<Vec<_>>();
        assert!(run(&i, &b, &guard("doc.ok", "doc.check")).is_empty());
        assert_eq!(run(&i, &b, &guard("doc.approved", "doc.check")), vec!["rule-guard t.p/doc.approve: doc.approved is not a condition of doc.approve"]);
        assert_eq!(run(&i, &b, &guard("doc.ok", "doc.nope")), vec!["rule-guard t.p/doc.approve: doc.nope is not bound"]);
        let mut manual = b.clone();
        manual["doc.check"]["manual"] = json!(true);
        assert_eq!(run(&i, &manual, &guard("doc.ok", "doc.check")), vec!["rule-guard t.p/doc.approve: doc.check is a person's or a model's action, not a program check"]);
        let model = json!({"doc.check": {"argv": [LLM, "doc.check"], "llm": {}, "produces": "doc_check", "subject": "doc"}});
        assert_eq!(run(&i, &model, &guard("doc.ok", "doc.check")).len(), 1);
        let mut other = b.clone();
        other["doc.check"]["subject"] = json!("elsewhere");
        assert_eq!(run(&i, &other, &guard("doc.ok", "doc.check")).len(), 1);
    }

    #[test]
    fn the_doc_block_is_read() {
        let d = "x\n## Check output\n\ntext\n```text\na b\nc d\n```\n## Next\n";
        assert_eq!(doc_block(d), Some(vec!["a b".to_string(), "c d".to_string()]));
    }
}
