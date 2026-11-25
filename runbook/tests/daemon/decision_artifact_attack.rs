//! Adversary cases against `story:decision-options-come-from-a-case-artifact`: a request read from its case
//! that no answer can take, a citation that resolves outside the workspace, an answer that names no option
//! committed by the run's judge, and the declared option `free` taken with no text.

use super::commands::Fx;
use super::engine::{copy_tree, until};
use super::state::scratch;
use runbook::commands::{AnswerDecision, Config, Daemon, Outcome, Policy, SubmitRun};
use runbook::engine::arbiter::{Arbiter, Judgement, Opened, Opening, RunArbiter};
use runbook::engine::Engine;
use runbook::runtime::{Autonomy, Confinement, RequestArtifact};
use runbook::stub::{Ask, Script, Step};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

const RUNBOOK: &str = "engine-demo";

/// The writer of the fixture, which also leaves the case's artifact (`artifact.in` with the draft's revision for `@REV@`).
const WRITER_WITH_ARTIFACT: &str = "#!/bin/sh\n[ -z \"$LOOM_RETRY_FEEDBACK\" ] || cp \"$LOOM_RETRY_FEEDBACK\" feedback.json\necho draft >> draft.txt\nrev=\"t$(sha256sum draft.txt | cut -c1-16)\"\nsed \"s/@REV@/$rev/\" artifact.in > decision-request.json\n";

/// The writer of a draft that is a YAML document with a `question` member.
const WRITER_OF_A_QUESTION: &str = "#!/bin/sh\nprintf 'question: Is the draft about tea?\\n' > draft.txt\n";

fn options() -> Vec<Value> {
    vec![
        json!({"letter": "A", "does": "ship the draft", "cost": "none", "consequence": {"admits": ["draft.review"], "blocks": [], "answer": "performed"}}),
        json!({"letter": "B", "does": "send it back", "cost": "one more round", "consequence": {"admits": [], "blocks": ["draft.review"], "answer": "rejected"}}),
        json!({"letter": "C", "does": "leave it for later", "cost": "the release waits", "consequence": {"admits": [], "blocks": []}}),
    ]
}

fn case_artifact(citation: Value) -> Value {
    json!({
        "request_id": "DR-draft-review-1", "case_id": "draft", "question": "Which draft goes out?",
        "subject_artifact": "draft", "subject_revision": "@REV@",
        "unblocks": {"kind": "approval", "action": "draft.review"},
        "options": options(), "recommended_option": "B", "recommendation_strength": "moderate",
        "grounds": [{"source": "idea", "citation": {"form": "path_line", "value": citation}, "supports": "B", "how": "the idea asks for tea"}],
    })
}

struct Rig {
    dir: PathBuf,
    d: Daemon,
}

impl Rig {
    /// The fixture runbook of `engine.rs` with `writer` as the writer's program, `edit` applied to the draft's
    /// binding, `questions` as `bindings/person-questions.json` when given, and `arbiter` as the engine's
    /// deciders (its one kind `test`) when given.
    fn new(tag: &str, writer: &str, edit: impl Fn(String) -> String, questions: Option<&str>, arbiter: Option<Arc<dyn Arbiter>>) -> Rig {
        let dir = scratch(tag);
        let home = dir.join("home");
        copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &home);
        let binding = home.join("bindings/engine-draft.yaml");
        let text = std::fs::read_to_string(&binding).unwrap().replace("principal: reviewer", &format!("principal: {}", super::PRINCIPAL));
        std::fs::write(&binding, edit(text)).unwrap();
        std::fs::write(home.join("bindings/tools/draft-write.sh"), writer).unwrap();
        if let Some(q) = questions {
            std::fs::write(home.join("bindings/person-questions.json"), q).unwrap();
        }
        std::fs::create_dir_all(dir.join("ctx/project")).unwrap();
        let mut engine = Engine::with_levels(home, dir.join("state"), &[Confinement::Local], super::policy());
        let mut policy = Policy::default();
        if let Some(a) = arbiter {
            engine = engine.with_arbiter(a);
            policy.kinds = vec!["test".into()];
        }
        let cfg = Config { state_dir: dir.join("state"), confinement_floor: Confinement::Local, context_roots: vec![dir.join("ctx")], runbook_dir: None, principal: super::PRINCIPAL.into() };
        let d = Daemon::start_with(cfg, policy, Arc::new(engine)).unwrap();
        Rig { dir, d }
    }

    fn submit(&self, artifact: Option<&Value>, decider: bool) -> String {
        let ctx = self.dir.join("ctx/project");
        if let Some(a) = artifact {
            std::fs::write(ctx.join("artifact.in"), a.to_string()).unwrap();
        }
        let (autonomy, decider) = if decider { (Some(Autonomy::A3), Some("test".to_string())) } else { (None, None) };
        let o = self
            .d
            .submit_run(SubmitRun { runbook_id: RUNBOOK.into(), intake: "a note on tea".into(), confinement: Some(Confinement::Local), context_path: Some(ctx.display().to_string()), autonomy, decider, ..SubmitRun::default() })
            .unwrap();
        assert_eq!(o.name, "submitted", "{o:?}");
        o.run_id.unwrap()
    }

    fn state_of(&self, run: &str) -> String {
        self.d.run_detail(run)[0]["state"].as_str().unwrap().to_string()
    }

    fn answer(&self, run: &str, request: &str, text: &str) -> Outcome {
        self.d.answer_decision(AnswerDecision { run_id: run.into(), request_id: request.into(), answer: text.into(), answered_by: "reviewer".into() }).unwrap()
    }

    /// Answers `approve` to every open request that is not the review until a review request is open (its
    /// row) or the run is over (none).
    fn review_or_end(&self, run: &str) -> Option<Value> {
        let mut found = None;
        let mut answered: BTreeSet<String> = BTreeSet::new();
        until(&format!("a review request of run {run}, or its end"), 40, || {
            if matches!(self.state_of(run).as_str(), "Ended" | "Cancelled" | "FailedToStart") {
                return true;
            }
            for row in self.d.open_decisions_of_run(run) {
                let id = row["request_id"].as_str().unwrap().to_string();
                if row["action"] == "draft.review" {
                    found = Some(row);
                    return true;
                } else if answered.insert(id.clone()) {
                    self.answer(run, &id, "approve");
                }
            }
            false
        });
        found
    }

    fn reason(&self, run: &str) -> String {
        until("the run to end", 40, || self.state_of(run) == "Ended");
        self.d.events_after(run, 0).into_iter().find(|e| e["kind"] == "runbook.runtime-failed").map(|e| e["record"]["reason"].as_str().unwrap_or("").to_owned()).unwrap_or_default()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.d.stop();
    }
}

/// The contract refuses a request of fewer than two options (`ess/domains/decision.yaml`, `too-few-options`:
/// "nothing was opened"). An artifact of none opens a request whose every answer is refused `unknown-option`,
/// and the run waits on it until a cancel.
#[test]
fn a_case_artifact_with_no_option_is_refused_by_its_cause_and_opens_no_request_no_answer_can_take() {
    let rig = Rig::new("adv-art-no-option", WRITER_WITH_ARTIFACT, |t| t, None, None);
    let mut a = case_artifact(json!({"path": "idea.yaml", "line": 1}));
    a["options"] = json!([]);
    a.as_object_mut().unwrap().remove("recommended_option");
    let run = rig.submit(Some(&a), false);
    if let Some(row) = rig.review_or_end(&run) {
        let id = row["request_id"].as_str().unwrap();
        let o = rig.answer(&run, id, "A");
        panic!("a request was opened from an artifact of no option, and the answer A was {:?}: {row}", o.name);
    }
    let reason = rig.reason(&run);
    assert!(reason.contains("DecisionArtifactRefused"), "{reason}");
}

/// The design (item 9): where the bindings name a question member, the options are the results the action
/// declares. An action whose `from: effect` entries share no result declares none: the request then has no
/// option and refuses every answer, and the run waits on it until a cancel.
#[test]
fn a_question_member_named_for_an_action_that_declares_no_result_opens_no_request_no_answer_can_take() {
    let disjoint = |t: String| {
        let old = "        results: {performed: approved, rejected: rejected}\n";
        assert!(t.contains(old), "the fixture's review evidence changed");
        let new = "        results: {A: approved}\n      - kind: draft_review\n        subject: draft\n        producer: reviewer\n        from: effect\n        results: {B: rejected}\n";
        t.replace(old, new)
    };
    let questions = r#"{"format": "runbook.person-questions/1", "actions": {"draft.review": "question"}}"#;
    let rig = Rig::new("adv-art-no-result", WRITER_OF_A_QUESTION, disjoint, Some(questions), None);
    let run = rig.submit(None, false);
    let row = rig.review_or_end(&run).unwrap_or_else(|| panic!("the run ended before its review: {}", rig.reason(&run)));
    let id = row["request_id"].as_str().unwrap();
    let takes_free = row["artifact"]["free_answer"] == json!(true);
    let n = row["options"].as_array().map_or(0, Vec::len);
    let tried: Vec<(String, &str)> = ["A", "B", "approve", "performed"].iter().map(|t| (t.to_string(), rig.answer(&run, id, t).name)).collect();
    assert!(n > 0 || takes_free, "an open request no answer can take ({tried:?}): {row}");
}

/// The design (item 4): every `path_line` citation names a file inside the workspace. A directory link the
/// case's program leaves in `work/` makes a citation below it resolve outside, and the check passes on a file
/// of another directory (its lines and its digest are read).
#[test]
fn a_citation_below_a_directory_link_that_leaves_the_workspace_is_refused() {
    let probe = scratch("adv-art-outside");
    std::fs::write(probe.join("secret.txt"), "secret\n").unwrap();
    let writer = format!("{WRITER_WITH_ARTIFACT}ln -sfn '{}' out\n", probe.display());
    let rig = Rig::new("adv-art-link", &writer, |t| t, None, None);
    let run = rig.submit(Some(&case_artifact(json!({"path": "out/secret.txt", "line": 1}))), false);
    if let Some(row) = rig.review_or_end(&run) {
        panic!("a request was opened on a citation of a file outside the workspace: {row}");
    }
    let reason = rig.reason(&run);
    assert!(reason.contains("DecisionArtifactRefused"), "{reason}");
}

/// A judge that answers the review the way the decider of `runbook run` writes an approval of a person action
/// (`performed`), and leaves everything else to a person.
#[derive(Debug)]
struct Performs;

impl Arbiter for Performs {
    fn kinds(&self) -> Vec<String> {
        vec!["test".into()]
    }
    fn open(&self, o: &Opening<'_>) -> Result<Opened, String> {
        Ok(Opened { bindings: o.bindings.to_path_buf(), judge: Box::new(Judge) })
    }
}

struct Judge;

impl RunArbiter for Judge {
    fn judge(&mut self, decision: &Value) -> Judgement {
        if decision["action"] == "draft.review" {
            Judgement::Answer { answer: "performed".into(), by: "decider:test".into(), principal: "decider:test".into() }
        } else {
            Judgement::Open("left to a person".into())
        }
    }
}

/// Outcome 3 and the design (item 2): an answer that names no option of a request read from its case is
/// refused before it is committed. The answer of the run's judge goes to the journal without that check.
#[test]
fn an_answer_of_the_runs_judge_that_names_no_option_of_a_case_request_is_not_committed() {
    let rig = Rig::new("adv-art-judge", WRITER_WITH_ARTIFACT, |t| t, None, Some(Arc::new(Performs)));
    let run = rig.submit(Some(&case_artifact(json!({"path": "idea.yaml", "line": 1}))), true);
    let mut answered: BTreeSet<String> = BTreeSet::new();
    let mut seen: Option<Instant> = None;
    let limit = Instant::now() + Duration::from_secs(30);
    until(&format!("the review of run {run} to be judged"), 40, || {
        let state = rig.state_of(&run);
        assert!(Instant::now() < limit, "run {run} is {state}; decisions {:?}; events {:?}", rig.d.decisions_of_run(&run), rig.d.events_after(&run, 0).iter().map(|e| e["kind"].clone()).collect::<Vec<_>>());
        assert!(!matches!(state.as_str(), "Ended" | "Cancelled" | "FailedToStart"), "run {run} is {state}");
        for row in rig.d.open_decisions_of_run(&run) {
            let id = row["request_id"].as_str().unwrap().to_string();
            if row["action"] != "draft.review" && answered.insert(id.clone()) {
                rig.answer(&run, &id, "approve");
            }
        }
        let reviews: Vec<Value> = rig.d.decisions_of_run(&run).into_iter().filter(|r| r.get("artifact").is_some()).collect();
        if reviews.iter().any(|r| r.get("answer").is_some()) {
            return true;
        }
        if !reviews.is_empty() {
            let at = *seen.get_or_insert_with(Instant::now);
            return at.elapsed() > Duration::from_secs(2);
        }
        false
    });
    for r in rig.d.decisions_of_run(&run).into_iter().filter(|r| r.get("artifact").is_some()) {
        let letters: Vec<&str> = r["options"].as_array().unwrap().iter().filter_map(|o| o["letter"].as_str()).collect();
        if let Some(answer) = r["answer"].as_str() {
            let named = letters.iter().any(|l| answer == *l || answer.strip_prefix(l).is_some_and(|rest| rest.starts_with(':')));
            assert!(named, "the request {} offers {letters:?} and holds the committed answer {answer:?} by {}: {r}", r["request_id"], r["answered_by"]);
        }
    }
}

// ---- the declared option `free` over the stub runtime ----

/// The options a binding's declared results A, B and `free` give (design item 9).
fn declared() -> Vec<Value> {
    let free = "take the action with the result free; this option takes text: answer free: and the text, which goes with it as the reasons";
    vec![
        json!({"letter": "A", "does": "take the action with the result A", "cost": "", "consequence": {"admits": [], "blocks": [], "answer": "A"}}),
        json!({"letter": "B", "does": "take the action with the result B", "cost": "", "consequence": {"admits": [], "blocks": [], "answer": "B"}}),
        json!({"letter": "free", "does": free, "cost": "", "consequence": {"admits": [], "blocks": [], "answer": "free"}}),
    ]
}

/// The option `free` says it takes text (`free: <text>`), and the UI's button for it sends its letter alone
/// (`ui/daemon/decide.js`, `data-answer` is `o.letter`). An answer `free` or `free:` with no text is handed on as
/// the result `free` with no reasons.
#[test]
fn the_declared_option_free_is_not_taken_without_its_text() {
    let fx = Fx::new("adv-art-free");
    let ask = Ask {
        request_id: "{run}-q".into(),
        case_id: "case-1".into(),
        action: "decision.answer".into(),
        capability: "".into(),
        question: "Which tea goes in?".into(),
        options: declared(),
        artifact: Some(RequestArtifact { recommendation_strength: "none".into(), ..RequestArtifact::default() }),
    };
    fx.stub.script("declared", Script::new(vec![Step::Ask(vec![ask]), Step::End("approved".into())]));
    let run = fx.submit("declared");
    fx.wait_open_decisions(&run, 1);
    let request = format!("{run}-q");
    for text in ["free", "free:", "free:   "] {
        let o = fx.d().answer_decision(AnswerDecision { run_id: run.clone(), request_id: request.clone(), answer: text.into(), answered_by: "operator".into() }).unwrap();
        assert_eq!((o.name, o.status), ("unknown-option", 422), "{text:?} without text was taken: {o:?}; handed on: {:?}", fx.stub.answers_given());
    }
}
