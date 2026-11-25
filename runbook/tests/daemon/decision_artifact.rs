//! `story:decision-options-come-from-a-case-artifact`: a person action's question, options,
//! recommendation and grounds come from its case. Two halves: the daemon over the stub runtime (the
//! rows of a request that carries an artifact, and the refusal of an answer that names none of its
//! options), and the engine over the fixture runbook of `engine.rs` (the artifact the writer leaves in
//! the case, its check, and the declared results of a binding that names a question member).

use super::commands::Fx;
use super::engine::{copy_tree, until};
use super::openapi::check_row;
use super::state::scratch;
use runbook::commands::{AnswerDecision, Config, Daemon, Outcome, SubmitRun};
use runbook::engine::Engine;
use runbook::runtime::{Confinement, RequestArtifact};
use runbook::stub::{Ask, Script, Step};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn options() -> Vec<Value> {
    vec![
        json!({"letter": "A", "does": "ship the draft", "cost": "none", "consequence": {"admits": ["draft.review"], "blocks": [], "answer": "performed"}}),
        json!({"letter": "B", "does": "send it back to the writer", "cost": "one more round", "consequence": {"admits": [], "blocks": ["draft.review"], "answer": "rejected"}}),
        json!({"letter": "C", "does": "leave it for later", "cost": "the release waits", "consequence": {"admits": [], "blocks": []}}),
    ]
}

fn grounds() -> Vec<Value> {
    vec![
        json!({"source": "idea", "citation": {"form": "path_line", "value": {"path": "idea.yaml", "line": 1}}, "supports": "B", "how": "the idea asks for a note on tea"}),
        json!({"source": "policy", "citation": {"form": "record_id", "value": {"store": "sdc_decisions", "record_id": "DR-0"}}, "supports": "B", "how": "the last round sent a short draft back"}),
    ]
}

fn artifact(free_answer: bool) -> RequestArtifact {
    RequestArtifact { recommended_option: Some("B".into()), recommended_answer: None, recommendation_strength: "moderate".into(), grounds: grounds(), free_answer }
}

pub(super) fn ask_with(free_answer: bool) -> Ask {
    Ask {
        request_id: "{run}-q".into(),
        case_id: "case-1".into(),
        action: "draft.review".into(),
        capability: "".into(),
        question: "Which draft goes out?".into(),
        options: options(),
        artifact: Some(artifact(free_answer)),
    }
}

/// A fixture whose runbook `art` asks one request read from a case, `art-free` one that takes a free answer.
fn fx(tag: &str) -> Fx {
    let fx = Fx::new(tag);
    fx.stub.script("art", Script::new(vec![Step::Ask(vec![ask_with(false)]), Step::End("approved".into())]));
    fx.stub.script("art-free", Script::new(vec![Step::Ask(vec![ask_with(true)]), Step::End("approved".into())]));
    fx
}

fn answer(fx: &Fx, run: &str, request: &str, text: &str) -> Outcome {
    fx.d().answer_decision(AnswerDecision { run_id: run.into(), request_id: request.into(), answer: text.into(), answered_by: "operator".into() }).unwrap()
}

fn journal_answers(fx: &Fx) -> usize {
    std::fs::read_to_string(fx.dir.join("state/journal.jsonl")).unwrap().lines().filter(|l| l.contains("\"fact\":\"decision_answered\"")).count()
}

#[test]
fn the_rows_of_a_request_read_from_its_case_carry_its_options_its_recommendation_and_its_grounds() {
    let fx = fx("art-rows");
    let run = fx.submit("art");
    fx.wait_open_decisions(&run, 1);
    let open = fx.d().open_decisions_of_run(&run);
    let row = &open[0];
    assert_eq!(row["question"], "Which draft goes out?");
    assert_eq!(row["options"], json!(options()));
    let want = json!({"recommended_option": "B", "recommendation_strength": "moderate", "grounds": grounds(), "free_answer": false});
    assert_eq!(row["artifact"], want, "{row}");
    check_row("OpenDecisionsOfRun", row);
    let all = fx.d().decisions_of_run(&run);
    assert_eq!((all[0]["artifact"].clone(), all[0]["options"].clone()), (want, json!(options())), "{}", all[0]);
    check_row("DecisionsOfRun", &all[0]);
    // A request that read nothing has no such member, in either view.
    let plain = fx.submit("ask");
    fx.wait_open_decisions(&plain, 1);
    assert!(fx.d().open_decisions_of_run(&plain)[0].get("artifact").is_none());
    assert!(fx.d().decisions_of_run(&plain)[0].get("artifact").is_none());
}

#[test]
fn an_answer_that_names_no_option_of_the_artifact_is_refused_by_name_before_anything_is_delivered() {
    let fx = fx("art-refuse");
    let run = fx.submit("art");
    fx.wait_open_decisions(&run, 1);
    let request = format!("{run}-q");
    let before = journal_answers(&fx);
    for text in ["approve", "Z", "", "a"] {
        let o = answer(&fx, &run, &request, text);
        assert_eq!((o.name, o.status, o.error), ("unknown-option", 422, Some("UnknownOption")), "{text:?}: {o:?}");
        assert_eq!(o.payload.as_ref().unwrap()["request_id"], json!(request));
    }
    assert!(fx.stub.answers_given().is_empty(), "the runtime was handed a refused answer: {:?}", fx.stub.answers_given());
    assert_eq!(journal_answers(&fx), before, "a refused answer is on the journal");
    assert_eq!(fx.d().open_decisions_of_run(&run).len(), 1, "the request stays Open");
    // Through the route: the same refusal with its status.
    let body = json!({"run_id": run, "request_id": request, "answer": "approve", "answered_by": "operator"}).to_string();
    let http = runbook::api::routes::command_answer("answer-decision", fx.d(), body.as_bytes());
    assert_eq!((http.status.as_u16(), http.body["outcome"].clone(), http.body["error"].clone()), (422, json!("unknown-option"), json!("sdc.daemon.UnknownOption")), "{}", http.body);
    // The letter B is taken and handed on as it was given; a second answer is refused as before.
    let o = answer(&fx, &run, &request, "B: too short");
    assert_eq!(o.name, "answered", "{o:?}");
    let given: Vec<String> = fx.stub.answers_given().into_iter().map(|a| a.answer).collect();
    assert_eq!(given, ["B: too short"]);
    assert_eq!(answer(&fx, &run, &request, "A").name, "already-answered");
    fx.wait_state(&run, "Ended");
}

#[test]
fn a_request_that_takes_a_free_answer_takes_text_that_names_no_option_and_a_request_without_an_artifact_refuses_nothing_new() {
    let fx = fx("art-free");
    let run = fx.submit("art-free");
    fx.wait_open_decisions(&run, 1);
    assert_eq!(answer(&fx, &run, &format!("{run}-q"), "green tea, not black").name, "answered");
    fx.wait_state(&run, "Ended");
    let plain = fx.submit("ask");
    fx.wait_open_decisions(&plain, 1);
    assert_eq!(answer(&fx, &plain, "q1", "approve").name, "answered");
}

// ---- the engine over the fixture runbook ----

const RUNBOOK: &str = "engine-demo";

/// The writer of the fixture, which also leaves the case's artifact: `artifact.in` of the context with
/// the draft's revision put in for `@REV@` (Loom's revision of a file is `t` and the first 16 hex digits
/// of the SHA-256 of its bytes, as Loom's own tests compute it: `loom-cli/tests/tree_run/person_program.rs`).
const WRITER_WITH_ARTIFACT: &str = "#!/bin/sh\n[ -z \"$LOOM_RETRY_FEEDBACK\" ] || cp \"$LOOM_RETRY_FEEDBACK\" feedback.json\necho draft >> draft.txt\nrev=\"t$(sha256sum draft.txt | cut -c1-16)\"\nsed \"s/@REV@/$rev/\" artifact.in > decision-request.json\n";

/// The writer of a draft that is a YAML document with a `question` member, a new one each round.
const WRITER_OF_A_QUESTION: &str = "#!/bin/sh\nn=$(( $(cat count 2>/dev/null || echo 0) + 1 ))\necho $n > count\nprintf 'question: Is draft %s about tea?\\n' $n > draft.txt\n";

struct Rig {
    dir: PathBuf,
    d: Daemon,
}

impl Rig {
    /// The fixture runbook with `writer` as the writer's program, the review's results replaced by
    /// `results` and `questions` as `bindings/person-questions.json` when given.
    fn new(tag: &str, writer: Option<&str>, results: Option<&str>, questions: Option<&str>) -> Rig {
        let dir = scratch(tag);
        let home = dir.join("home");
        copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &home);
        let binding = home.join("bindings/engine-draft.yaml");
        let mut text = std::fs::read_to_string(&binding).unwrap().replace("principal: reviewer", &format!("principal: {}", super::PRINCIPAL));
        if let Some(r) = results {
            let old = "results: {performed: approved, rejected: rejected}";
            assert!(text.contains(old));
            text = text.replace(old, &format!("results: {r}"));
        }
        std::fs::write(&binding, text).unwrap();
        if let Some(w) = writer {
            std::fs::write(home.join("bindings/tools/draft-write.sh"), w).unwrap();
        }
        if let Some(q) = questions {
            std::fs::write(home.join("bindings/person-questions.json"), q).unwrap();
        }
        std::fs::create_dir_all(dir.join("ctx/project")).unwrap();
        let engine = Arc::new(Engine::with_levels(home, dir.join("state"), &[Confinement::Local], super::policy()));
        let cfg = Config { state_dir: dir.join("state"), confinement_floor: Confinement::Local, context_roots: vec![dir.join("ctx")], runbook_dir: None, principal: super::PRINCIPAL.into() };
        let d = Daemon::start(cfg, engine).unwrap();
        Rig { dir, d }
    }

    /// Submits a run whose context holds `artifact` as `artifact.in`.
    fn submit(&self, artifact: Option<&Value>) -> String {
        let ctx = self.dir.join("ctx/project");
        if let Some(a) = artifact {
            std::fs::write(ctx.join("artifact.in"), a.to_string()).unwrap();
        }
        let o = self.d.submit_run(SubmitRun { runbook_id: RUNBOOK.into(), intake: "a note on tea".into(), confinement: Some(Confinement::Local), context_path: Some(ctx.display().to_string()), ..SubmitRun::default() }).unwrap();
        assert_eq!(o.name, "submitted", "{o:?}");
        o.run_id.unwrap()
    }

    fn state_of(&self, run: &str) -> String {
        self.d.run_detail(run)[0]["state"].as_str().unwrap().to_string()
    }

    fn workspace(&self, run: &str) -> PathBuf {
        self.dir.join("state/workspaces").join(run)
    }

    fn answer(&self, run: &str, request: &str, text: &str) -> Outcome {
        self.d.answer_decision(AnswerDecision { run_id: run.into(), request_id: request.into(), answer: text.into(), answered_by: "reviewer".into() }).unwrap()
    }

    /// Answers every open request of the run that is not the review with `approve` until the review is
    /// open, and returns its row; `seen` holds the reviews already taken.
    fn next_review(&self, run: &str, seen: &mut BTreeSet<String>) -> Value {
        let mut found = None;
        let mut answered: BTreeSet<String> = BTreeSet::new();
        until(&format!("a review request of run {run}"), 40, || {
            let state = self.state_of(run);
            assert!(!matches!(state.as_str(), "Ended" | "Cancelled" | "FailedToStart"), "run {run} is {state}: {}", self.d.run_detail(run)[0]);
            for row in self.d.open_decisions_of_run(run) {
                let id = row["request_id"].as_str().unwrap().to_string();
                if row["action"] == "draft.review" {
                    if !seen.contains(&id) {
                        seen.insert(id);
                        found = Some(row);
                        return true;
                    }
                } else if answered.insert(id.clone()) {
                    assert_eq!(self.answer(run, &id, "approve").name, "answered");
                }
            }
            false
        });
        found.unwrap()
    }

    /// Answers `approve` to everything until the run ends.
    fn finish(&self, run: &str) {
        let mut answered: BTreeSet<String> = BTreeSet::new();
        until(&format!("run {run} to be over"), 40, || {
            if matches!(self.state_of(run).as_str(), "Ended" | "Cancelled" | "FailedToStart") {
                return true;
            }
            for row in self.d.open_decisions_of_run(run) {
                let id = row["request_id"].as_str().unwrap().to_string();
                if row["action"] != "draft.review" && answered.insert(id.clone()) {
                    self.answer(run, &id, "approve");
                }
            }
            false
        });
    }

    fn reason(&self, run: &str) -> String {
        until("the run to end", 40, || self.state_of(run) == "Ended");
        let failed = self.d.events_after(run, 0).into_iter().find(|e| e["kind"] == "runbook.runtime-failed").expect("no runbook.runtime-failed event");
        failed["record"]["reason"].as_str().unwrap().to_owned()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.d.stop();
    }
}

fn case_artifact(revision: &str, citation: Value) -> Value {
    let mut g = grounds();
    g[0]["citation"]["value"] = citation;
    json!({
        "request_id": "DR-draft-review-1", "case_id": "draft", "question": "Which draft goes out?",
        "subject_artifact": "draft", "subject_revision": revision,
        "unblocks": {"kind": "approval", "action": "draft.review"},
        "options": options(), "recommended_option": "B", "recommendation_strength": "moderate", "grounds": g,
    })
}

#[test]
fn the_engine_opens_the_request_of_the_case_artifact_and_hands_loom_the_effect_of_the_letter() {
    let rig = Rig::new("art-engine", Some(WRITER_WITH_ARTIFACT), None, None);
    let run = rig.submit(Some(&case_artifact("@REV@", json!({"path": "idea.yaml", "line": 1}))));
    let mut seen = BTreeSet::new();
    let row = rig.next_review(&run, &mut seen);
    let id = row["request_id"].as_str().unwrap().to_string();
    assert_eq!(row["question"], "Which draft goes out?", "{row}");
    assert_eq!(row["options"], json!(options()));
    let want = json!({"recommended_option": "B", "recommendation_strength": "moderate", "grounds": grounds(), "free_answer": false});
    assert_eq!(row["artifact"], want);
    check_row("OpenDecisionsOfRun", &row);
    // The open request keeps what it opened with when the case's file changes afterwards.
    std::fs::write(rig.workspace(&run).join("decision-request.json"), case_artifact("x", json!({"path": "idea.yaml", "line": 1})).to_string().replace("Which draft", "Another")).unwrap();
    let now = rig.d.open_decisions_of_run(&run).into_iter().find(|r| r["request_id"] == id.as_str()).unwrap();
    assert_eq!(now, row);
    // `approve` names no option: refused, the request stays open, the writer does not run again.
    let o = rig.answer(&run, &id, "approve");
    assert_eq!((o.name, o.status), ("unknown-option", 422), "{o:?}");
    assert!(rig.d.open_decisions_of_run(&run).iter().any(|r| r["request_id"] == id.as_str()));
    // B's consequence is the result `rejected`: the writer is sent back and writes a second draft.
    assert_eq!(rig.answer(&run, &id, "B: add the tea").name, "answered");
    until("the writer's second draft", 40, || std::fs::read_to_string(rig.workspace(&run).join("draft.txt")).is_ok_and(|t| t == "draft\ndraft\n"));
    let feedback = std::fs::read_to_string(rig.workspace(&run).join("feedback.json")).unwrap();
    assert!(feedback.contains("add the tea"), "{feedback}");
    let second = rig.next_review(&run, &mut seen);
    assert_eq!(second["options"], json!(options()), "the second draft's artifact is read anew");
    assert_eq!(rig.answer(&run, second["request_id"].as_str().unwrap(), "A").name, "answered");
    rig.finish(&run);
    let detail = rig.d.run_detail(&run)[0].clone();
    assert_eq!((detail["state"].as_str(), detail["outcome"].as_str()), (Some("Ended"), Some("delivered")), "{detail}");
}

#[test]
fn an_artifact_of_another_revision_or_with_a_citation_that_does_not_resolve_ends_the_run_by_its_named_cause() {
    let rig = Rig::new("art-engine-stale", Some(WRITER_WITH_ARTIFACT), None, None);
    let run = rig.submit(Some(&case_artifact("sha256:00", json!({"path": "idea.yaml", "line": 1}))));
    let reason = rig.reason(&run);
    assert!(reason.contains("DecisionArtifactRefused") && reason.contains("subject_revision sha256:00"), "{reason}");
    assert!(rig.d.decisions_of_run(&run).iter().all(|r| r["question"] != "Which draft goes out?"), "a request opened with the artifact's words");
    let rig = Rig::new("art-engine-cite", Some(WRITER_WITH_ARTIFACT), None, None);
    let run = rig.submit(Some(&case_artifact("@REV@", json!({"path": "idea.yaml", "line": 99}))));
    let reason = rig.reason(&run);
    assert!(reason.contains("DecisionArtifactRefused") && reason.contains("lines 99-99 outside idea.yaml"), "{reason}");
    assert!(!reason.contains(&rig.dir.display().to_string()), "the reason names a path of the state directory: {reason}");
}

const QUESTIONS: &str = r#"{"format": "runbook.person-questions/1", "actions": {"draft.review": "question"}}"#;
const DECLARED: &str = "{A: approved, B: rejected, free: approved}";

#[test]
fn a_binding_that_names_a_question_member_gives_its_text_as_the_question_and_the_declared_results_as_the_options() {
    let rig = Rig::new("art-engine-declared", Some(WRITER_OF_A_QUESTION), Some(DECLARED), Some(QUESTIONS));
    let run = rig.submit(None);
    let mut seen = BTreeSet::new();
    let row = rig.next_review(&run, &mut seen);
    let id = row["request_id"].as_str().unwrap().to_string();
    assert_eq!(row["question"], "Is draft 1 about tea?", "{row}");
    let letters: Vec<&str> = row["options"].as_array().unwrap().iter().map(|o| o["letter"].as_str().unwrap()).collect();
    assert_eq!(letters, ["A", "B", "free"], "{row}");
    assert_eq!(row["artifact"], json!({"recommendation_strength": "none", "grounds": [], "free_answer": false}));
    check_row("OpenDecisionsOfRun", &row);
    let o = rig.answer(&run, &id, "Z");
    assert_eq!((o.name, o.status), ("unknown-option", 422), "{o:?}");
    assert_eq!(rig.answer(&run, &id, "B").name, "answered");
    let second = rig.next_review(&run, &mut seen);
    assert_eq!(second["question"], "Is draft 2 about tea?", "B is the result `rejected` of the evidence: the writer ran again");
    assert_eq!(rig.answer(&run, second["request_id"].as_str().unwrap(), "free: green tea").name, "answered");
    rig.finish(&run);
    assert_eq!(rig.d.run_detail(&run)[0]["outcome"], "delivered");
}

#[test]
fn a_question_member_the_subject_lacks_leaves_the_question_of_before_and_the_record_says_why_and_no_opt_in_keeps_the_row_of_before() {
    // The writer of the fixture writes `draft\n`: no member `question`.
    let rig = Rig::new("art-engine-unread", None, Some(DECLARED), Some(QUESTIONS));
    let run = rig.submit(None);
    let row = rig.next_review(&run, &mut BTreeSet::new());
    assert!(row["question"].as_str().unwrap().contains("\nrevision: ") && row["question"] != "question", "the generated question: {row}");
    let letters: Vec<&str> = row["options"].as_array().unwrap().iter().map(|o| o["letter"].as_str().unwrap()).collect();
    assert_eq!(letters, ["A", "B", "free"]);
    let unread = rig.d.events_after(&run, 0).into_iter().find(|e| e["kind"] == "runbook.question-field-unread").expect("no runbook.question-field-unread event");
    assert_eq!(unread["record"]["action"], "draft.review", "{unread}");
    assert!(unread["record"]["cause"].as_str().unwrap().contains("question"), "{unread}");
    assert_eq!(rig.answer(&run, row["request_id"].as_str().unwrap(), "A").name, "answered");
    rig.finish(&run);
    // No question file: the review's row is the one of before, byte for byte.
    let rig = Rig::new("art-engine-plain", None, None, None);
    let run = rig.submit(None);
    let row = rig.next_review(&run, &mut BTreeSet::new());
    assert!(row.get("artifact").is_none(), "{row}");
    assert_eq!(
        serde_json::to_string(&row["options"]).unwrap(),
        r#"[{"answer":"approve","effect":"take the action with the result performed, as the principal who answers"},{"answer":"{\"result\":\"<result>\",\"reasons\":\"<text>\"}","effect":"take the action with a result its binding names and the reasons (the result alone may be given as the answer)"}]"#
    );
}

#[test]
fn a_journal_line_of_a_request_from_before_the_member_still_loads_and_a_request_with_it_comes_back_whole() {
    use runbook::state::{Fact, State};
    let dir = scratch("art-journal");
    std::fs::create_dir_all(&dir).unwrap();
    let old = r#"{"fact":"decision_requested","request_id":"q1","run_id":"r1","case_id":"c","action":"a","capability":"k","question":"?","options":[{"answer":"approve","effect":"e"}],"opened_at":"t","state":"Open","answer":null,"answered_by":null,"answered_at":null}"#;
    std::fs::write(dir.join("journal.jsonl"), format!("{old}\n")).unwrap();
    let q1 = {
        let mut st = State::open(&dir).unwrap();
        let q1 = st.decision("q1").expect("the line of before loads").clone();
        assert!(q1.artifact.is_none());
        let mut q2 = q1.clone();
        q2.request_id = "q2".into();
        q2.artifact = Some(artifact(true));
        st.commit(Fact::DecisionRequested(q2)).unwrap();
        q1
    };
    assert!(!serde_json::to_string(&Fact::DecisionRequested(q1)).unwrap().contains("artifact"), "a request that read nothing writes no member");
    let st = State::open(&dir).unwrap();
    assert_eq!(st.decision("q2").unwrap().artifact, Some(artifact(true)));
}
