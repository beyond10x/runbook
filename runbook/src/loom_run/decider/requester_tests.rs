//! Tests of the requester decider (`requester.rs`) against stand-ins the test writes: a fake adapter
//! (the model, which writes `out/reply.json`), a stand-in for the grounding check
//! (`tools/decision-grounds.sh`) and for `sdc-decision-request`. Invented text only, no model, no
//! network. The decider runs in this process, as it does in `runbook run`.

use super::*;
use crate::assets::testdir::Dir;
use crate::loom_run::answers::{parse_requests, Asked};
use std::os::unix::fs::PermissionsExt;

const SYSTEM_PATH: &str = "/usr/bin:/bin";

const VERIFIED: &str = r#"{"recommend":{"check_verdict":"verified","check_report":"every ground holds","grounds":[{"citation":{"form":"artifact_revision","value":{"artifact":"goals","revision":"g1","entry":"G2"}},"supports":"A"}]}}"#;
const REFUSED: &str = r#"{"recommend":{"check_verdict":"refused","check_report":"goals@g1#G9: no such entry","grounds":[]}}"#;
const GOALS: &str = "goals:\n  - id: G2\n    text: The viewer works without an account.\n";
const IDEA: &str = "idea: A pocket viewer for harbour tide tables.\n";
const DISSECTION: &str = "items:\n  - id: i1\n    text: Which harbours are shown first?\n";
const QUESTIONS: &str = "questions:\n  - id: q1\n    item: i1\n    question: Sign in how?\n    status: open\n    default: a one-time code\n";

struct World {
    _dir: Dir,
    run: PathBuf,
    work: PathBuf,
    adapter: PathBuf,
    reply_bin: PathBuf,
}

fn write_exe(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

/// The fake adapter: counts its starts in `n`, keeps the adapter's request as `adapter-request.json`,
/// writes the model's `out/reply.json` from `reply.<n>.json` (else `reply.json`; none: no file) and
/// prints `response.json`.
const ADAPTER: &str = "#!/bin/sh\nd=$(dirname \"$0\")\nrec=$(dirname \"$1\")\nn=$(cat \"$d/n\" 2>/dev/null || echo 0)\nn=$((n + 1))\necho $n > \"$d/n\"\ncp \"$1\" \"$d/adapter-request.json\"\nf=\"$d/reply.$n.json\"\n[ -f \"$f\" ] || f=\"$d/reply.json\"\n[ -f \"$f\" ] && cp \"$f\" \"$rec/cwd/out/reply.json\" && cp \"$f\" \"$rec/cwd/out/decision.json\"\ncat \"$d/response.json\"\n";
/// The stand-in grounding check: prints `verdict.json` and keeps its arguments in `grounds-args.txt`.
const GROUNDS: &str = "#!/bin/sh\nd=$(dirname \"$0\")/..\nprintf '%s\\n' \"$@\" >> \"$d/grounds-args.txt\"\ncat \"$d/verdict.json\"\n";
/// The stand-in `sdc-decision-request`: prints `question-request.json`, or refuses when there is none.
const QUESTION_REQUEST: &str = "#!/bin/sh\nd=$(dirname \"$0\")\nprintf '%s\\n' \"$@\" > \"$d/question-args.txt\"\n[ -f \"$d/question-request.json\" ] || { echo refused >&2; exit 2; }\ncat \"$d/question-request.json\"\n";

fn world(tag: &str) -> World {
    let dir = Dir::new(tag);
    let run = dir.dir("run");
    dir.dir("run/home");
    let work = dir.dir("run/work");
    for (name, text) in [("goals.yaml", GOALS), ("idea.yaml", IDEA), ("dissection.yaml", DISSECTION), ("questions.yaml", QUESTIONS), ("aep/vision/product.md", "# Tide viewer\n")] {
        let file = work.join(name);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
    }
    dir.dir("run/instructions");
    std::os::unix::fs::symlink(repo().join("instructions/decider"), run.join("instructions/decider")).unwrap();
    std::fs::create_dir_all(run.join("driver/examples/sdc")).unwrap();
    std::fs::copy(repo().join("driver/examples/sdc/decision-classes.json"), run.join("driver/examples/sdc/decision-classes.json")).unwrap();
    std::fs::create_dir_all(run.join("tools")).unwrap();
    std::fs::copy(repo().join(routing::LEVELS_FILE), run.join(routing::LEVELS_FILE)).unwrap();
    std::fs::create_dir_all(run.join("bindings/tools")).unwrap();
    std::fs::copy(repo().join("bindings/tools/node-classes.json"), run.join("bindings/tools/node-classes.json")).unwrap();
    let adapter = run.join("bindings/tools/adapter.sh");
    write_exe(&adapter, ADAPTER);
    write_exe(&run.join("tools/decision-grounds.sh"), GROUNDS);
    let reply_bin = run.join("bin/sdc-reply");
    write_exe(&reply_bin, "#!/bin/sh\n");
    write_exe(&run.join("bin/sdc-decision-request"), QUESTION_REQUEST);
    std::fs::write(run.join("verdict.json"), VERIFIED).unwrap();
    let tools = run.join("bindings/tools");
    std::fs::write(tools.join("response.json"), r#"{"adapter":"claude-cli","model":"claude-opus-5-5","cost_usd":0.05}"#).unwrap();
    std::fs::write(tools.join("reply.json"), reply("A", "the goal G2 supports it")).unwrap();
    World { _dir: dir, run, work, adapter, reply_bin }
}

fn reply(option: &str, reason: &str) -> String {
    format!(r#"{{"option":"{option}","answer":null,"reasons":["{reason}"],"grounds":[{{"source":"goals","citation":{{"form":"artifact_revision","value":{{"artifact":"goals","revision":"g1","entry":"G2"}}}},"supports":"{option}","how":"it says so"}}],"strength":"strong","no_ground_in_goals":false}}"#)
}

const NO_GROUND: &str = r#"{"option":null,"answer":null,"reasons":["no entry covers it"],"grounds":[],"strength":"weak","no_ground_in_goals":true}"#;

fn tools(w: &World) -> PathBuf {
    w.run.join("bindings/tools")
}

fn set_reply(w: &World, n: Option<u32>, text: &str) {
    let name = n.map_or("reply.json".to_owned(), |n| format!("reply.{n}.json"));
    std::fs::write(tools(w).join(name), text).unwrap();
}

fn settings(w: &World, extra: Value) -> Value {
    let mut decider = json!({
        "kind": "model", "model": "claude-opus-5-5", "adapter": w.adapter.display().to_string(), "reply_bin": w.reply_bin.display().to_string(),
        "escalate": "final", "max_calls": 6, "requester": true,
    });
    for (k, v) in extra.as_object().unwrap() {
        decider[k] = v.clone();
    }
    json!({"autonomy": "A5", "decider": decider})
}

fn env() -> Vec<(String, String)> {
    vec![("PATH".to_owned(), SYSTEM_PATH.to_owned())]
}

/// The decision Loom prints for the approval of `action`; its store names the goals revision `g1`.
fn approval(w: &World, action: &str, subject: &str) -> Value {
    let store = w.run.join("store");
    std::fs::create_dir_all(&store).unwrap();
    let case = "IDEA-1.direction";
    std::fs::write(store.join(format!("case-{}.json", super::approval::hex(case))), r#"{"artifacts":[{"artifact":"goals","revision":"g1"}]}"#).unwrap();
    json!({
        "case": case, "kind": "ApproveAction", "action": action, "capability": "owner.approve", "decides": format!("{action} of case {case}"),
        "subject": subject, "revision": "tvis0001", "path": w.work.join("aep/vision/product.md").display().to_string(),
        "command": ["b10x-loom", "tree", "approve", "--store", store.display().to_string(), "--case", case, "--principal", "person:owner"],
    })
}

fn decide_with(w: &World, settings: &Value, decision: &Value) -> Verdict {
    let decider = from_settings(settings).unwrap();
    decider.decide(&DecisionRequest { decision, run_dir: &w.run, workspace: &w.work, budget_micro_usd: 1_000_000, child_env: &env(), login_link: None })
}

fn ask_with(w: &World, settings: &Value, request: &crate::loom_run::answers::Request) -> Asked {
    let decider = from_settings(settings).unwrap();
    let decision = approval(w, "scope.answer", "questions");
    decider.ask(&AskRequest { decision: &decision, request, run_dir: &w.run, workspace: &w.work, budget_micro_usd: 1_000_000, child_env: &env(), login_link: None })
}

fn json_file(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn records(w: &World) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(w.run.join("decider")).unwrap().filter_map(Result::ok).map(|e| e.path()).collect();
    dirs.sort();
    dirs
}

/// The files below `dir`, relative, sorted.
fn files_below(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, at: &Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(at).unwrap().filter_map(Result::ok) {
            if e.path().is_dir() {
                walk(base, &e.path(), out);
            } else {
                out.push(e.path().strip_prefix(base).unwrap().display().to_string());
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

fn reason(verdict: Verdict) -> String {
    match verdict {
        Verdict::CannotDecide { reason } => reason,
        other => panic!("expected CannotDecide, got {other:?}"),
    }
}

fn question_line() -> crate::loom_run::answers::Request {
    let line = json!({"type": "question", "id": "q1", "question": {"id": "q1", "item": "i1", "default": "a one-time code", "status": "open"}});
    parse_requests(&line.to_string()).unwrap().remove(0)
}

fn question_request() -> String {
    json!({"request": {
        "request_id": "DR-IDEA-1.direction-scope.answer-none", "state": "Open", "question_key": "IDEA-1.direction|scope.answer",
        "subject_path": "work/questions.yaml", "opened_at": "2026-01-02T03:04:05Z", "decision_class": "scope_changing_answer",
        "options": [{"letter": "A", "does": "a one-time code", "consequence": {"answer": "Sign in with a one-time code"}}, {"letter": "B", "does": "a password"}],
        "grounds": [{"citation": {"form": "path_line", "value": {"path": "work/dissection.yaml", "line": 2}}}],
    }})
    .to_string()
}

fn sorted(mut names: Vec<&str>) -> Vec<String> {
    names.sort_unstable();
    names.into_iter().map(str::to_owned).collect()
}

// --- the routing ----------------------------------------------------------------------------------

#[test]
fn the_class_of_a_decision_is_read_from_decision_classes_and_its_side_routes_it() {
    let root = repo();
    for (kind, action, class, side) in [
        ("approval", "vision.approve", "vision", "requester"),
        ("approval", "context.approve", "context", "requester"),
        ("approval", "release.publish", "release", "requester"),
        ("approval", "merge.approve", "merge", "delivering"),
        ("approval", "change.merge", "merge", "delivering"),
        ("approval", "something.else", "approval", "delivering"),
        ("close", "close", "close", "requester"),
        ("question", "scope.answer", "scope_changing_answer", "requester"),
        ("finding", "amend.upstream", "finding", "delivering"),
    ] {
        let found = requester::route(&root, kind, action).unwrap();
        assert_eq!((found.class.as_str(), found.side.as_str()), (class, side), "{kind} {action}");
    }
}

#[test]
fn the_requester_setting_is_a_flag_of_a_model_decider_only() {
    let w = world("requester-settings");
    assert!(from_settings(&settings(&w, json!({}))).is_ok());
    let err = from_settings(&settings(&w, json!({"requester": "yes"}))).err().unwrap();
    assert_eq!(err, "decider.requester: true or false");
    let rule = json!({"autonomy": "A5", "decider": {"kind": "rule", "verdict": "approve", "requester": true}});
    assert!(from_settings(&rule).err().unwrap().contains("decider.requester: not a key of a decider of kind rule"));
}

#[test]
fn a_run_without_the_class_file_cannot_route_and_says_so() {
    let w = world("requester-no-classes");
    std::fs::remove_file(w.run.join("driver/examples/sdc/decision-classes.json")).unwrap();
    let why = reason(decide_with(&w, &settings(&w, json!({})), &approval(&w, "vision.approve", "vision")));
    assert!(why.contains("decision-classes.json"), "{why}");
}

// --- an approval of a requester-side class --------------------------------------------------------

#[test]
fn a_requester_side_approval_is_answered_from_the_goals_by_the_requester() {
    let w = world("requester-approve");
    let verdict = decide_with(&w, &settings(&w, json!({})), &approval(&w, "vision.approve", "vision"));
    assert_eq!(verdict, Verdict::Answer { approve: true, principal: Some("decider:requester".to_owned()), cost_micro_usd: Some(50_000), model_calls: 1, reasons: None });
    let rec = &records(&w)[0];
    assert!(rec.file_name().unwrap().to_str().unwrap().starts_with("001-vision.approve"));
    assert_eq!(files_below(&rec.join("cwd/in")), sorted(vec!["aep/vision/product.md", "decisions.json", "dissection.yaml", "goals.yaml", "idea.yaml", "request.json"]), "exactly what the requester may read");
    assert_eq!(json_file(&rec.join("cwd/in/decisions.json")), json!([]));
    let request = json_file(&rec.join("cwd/in/request.json"));
    assert_eq!(request["question"], "Approve vision.approve: the vision at revision tvis0001?");
    assert_eq!((request["options"][0]["does"].as_str(), request["options"][1]["does"].as_str()), (Some("approve"), Some("reject: the writer reworks it")));
    assert_eq!((request["decision_class"].as_str(), request["addressed_to"].as_str(), request["subject_path"].as_str()), (Some("vision"), Some("decider:requester"), Some("work/aep/vision/product.md")));
    let prompt = std::fs::read_to_string(rec.join("prompt.md")).unwrap();
    assert!(prompt.contains("You answer for the requester") && prompt.contains("The goals revision is g1"), "{prompt}");
    assert!(prompt.contains("- in/goals.yaml") && !prompt.contains("This is attempt"), "{prompt}");
    let asked = json_file(&rec.join("request.json"));
    assert_eq!((asked["node"].as_str(), asked["effort"].as_str(), asked["attempt"].as_u64(), asked["model"].as_str()), (Some("requester.vision.approve"), Some("xhigh"), Some(1), Some("claude-opus-5-5")));
    assert!(asked.get("json_schema").is_none(), "the requester's reply is checked by the grounding program, not by a schema");
    let record = json_file(&rec.join("decide-request.json"));
    assert_eq!((record["class"].as_str(), record["side"].as_str(), record["autonomy"].as_str()), (Some("vision"), Some("requester"), Some("A5")));
    assert!(rec.join("proposal.json").is_file() && rec.join("grounds-check.json").is_file() && rec.join("status.json").is_file());
    let args = std::fs::read_to_string(w.run.join("grounds-args.txt")).unwrap();
    assert!(args.contains("proposal.json") && args.contains("--case") && args.contains("--at"), "{args}");
}

#[test]
fn a_delivering_side_approval_stays_with_the_model_decider_of_the_run() {
    let w = world("requester-delivering");
    let decision = approval(&w, "merge.approve", "merge");
    let no_claims = reason(decide_with(&w, &settings(&w, json!({})), &decision));
    assert!(no_claims.contains("claims of the position are not in Loom's output"), "{no_claims}");
    set_reply(&w, None, r#"{"decision":"approve","reasons":["fine"],"cites":[]}"#);
    let s = settings(&w, json!({"without_claims": true}));
    let verdict = decide_with(&w, &s, &decision);
    assert!(matches!(&verdict, Verdict::Answer { approve: true, principal: Some(p), .. } if p == "decider:model"), "{verdict:?}");
    assert!(w.run.join("decider").read_dir().unwrap().next().unwrap().unwrap().path().join("cwd/in/checks.json").is_file());
}

#[test]
fn option_b_is_a_reject_with_the_reasons_of_the_requester() {
    let w = world("requester-reject");
    set_reply(&w, None, &reply("B", "G2 rules out an account"));
    let verdict = decide_with(&w, &settings(&w, json!({})), &approval(&w, "roadmap.approve", "roadmap"));
    assert_eq!(verdict, Verdict::Answer { approve: false, principal: Some("decider:requester".to_owned()), cost_micro_usd: Some(50_000), model_calls: 1, reasons: Some("G2 rules out an account".to_owned()) });
}

#[test]
fn a_close_is_decided_on_the_outcome_loom_printed() {
    let w = world("requester-close");
    let close = json!({"case": "IDEA-1", "kind": "Close", "action": null, "outcome": "kept", "decides": "close case IDEA-1 through kept",
        "command": ["b10x-loom", "tree", "close", "--store", "/nonexistent/store", "--case", "IDEA-1", "--outcome", "kept"]});
    let verdict = decide_with(&w, &settings(&w, json!({})), &close);
    assert!(matches!(&verdict, Verdict::Answer { approve: true, principal: Some(p), .. } if p == "decider:requester"), "{verdict:?}");
    let request = json_file(&records(&w)[0].join("cwd/in/request.json"));
    assert_eq!((request["question"].as_str(), request["options"][0]["does"].as_str()), (Some("Close the idea through the outcome kept?"), Some("close through kept")));
    assert_eq!(request["decision_class"], "close");
}

// --- no ground, a refused check, a bad reply ------------------------------------------------------

#[test]
fn no_ground_in_the_goals_escalates_and_the_run_waits_unless_final_takes_the_best() {
    let w = world("requester-no-ground");
    set_reply(&w, None, NO_GROUND);
    let decision = approval(&w, "vision.approve", "vision");
    let stop = reason(decide_with(&w, &settings(&w, json!({"escalate": "stop"})), &decision));
    assert!(stop.starts_with("reserved for a person: the decider escalated (escalate stop): no ground in the goals no entry covers it"), "{stop}");
    assert!(!records(&w)[0].join("grounds-check.json").exists(), "no ground: the grounding check is not asked");
    let w = world("requester-no-ground-final");
    set_reply(&w, None, NO_GROUND);
    let verdict = decide_with(&w, &settings(&w, json!({})), &approval(&w, "vision.approve", "vision"));
    assert!(matches!(&verdict, Verdict::Answer { approve: true, principal: Some(p), .. } if p == "decider:requester"), "{verdict:?}");
}

#[test]
fn a_refused_grounding_check_is_an_invalid_reply_asked_again_with_its_reason() {
    let w = world("requester-refused");
    std::fs::write(w.run.join("verdict.json"), REFUSED).unwrap();
    let why = reason(decide_with(&w, &settings(&w, json!({})), &approval(&w, "vision.approve", "vision")));
    assert_eq!(why, "invalid decider reply after 3 attempts: the grounding check refused it: goals@g1#G9: no such entry");
    let dirs = records(&w);
    assert_eq!(dirs.len(), 3);
    let second = std::fs::read_to_string(dirs[1].join("prompt.md")).unwrap();
    assert!(second.contains("This is attempt 2.") && second.contains("the grounding check refused it: goals@g1#G9"), "{second}");
    assert_eq!(json_file(&dirs[1].join("request.json"))["attempt"], 2);
}

#[test]
fn a_reply_that_is_missing_or_not_an_object_is_an_invalid_reply_and_a_failing_check_is_an_error() {
    let w = world("requester-missing");
    std::fs::remove_file(tools(&w).join("reply.json")).unwrap();
    let why = reason(decide_with(&w, &settings(&w, json!({"retries": 0})), &approval(&w, "vision.approve", "vision")));
    assert_eq!(why, "invalid decider reply after 1 attempts: out/reply.json is missing or not a JSON object");
    let w = world("requester-array");
    set_reply(&w, None, "[1]");
    let why = reason(decide_with(&w, &settings(&w, json!({"retries": 0})), &approval(&w, "vision.approve", "vision")));
    assert!(why.ends_with("out/reply.json is missing or not a JSON object"), "{why}");
    let w = world("requester-check-fails");
    write_exe(&w.run.join("tools/decision-grounds.sh"), "#!/bin/sh\necho 'no sdc-goals' >&2\nexit 5\n");
    let why = reason(decide_with(&w, &settings(&w, json!({})), &approval(&w, "vision.approve", "vision")));
    assert!(why.contains("decision-grounds.sh exit 5, its output could not be read: no sdc-goals"), "{why}");
}

#[test]
fn the_stub_adapter_has_no_model_and_the_requester_says_so() {
    let w = world("requester-stub");
    let s = settings(&w, json!({"adapter": "stub", "retries": 0, "escalate": "stop"}));
    let why = reason(decide_with(&w, &s, &approval(&w, "vision.approve", "vision")));
    assert_eq!(why, "invalid decider reply after 1 attempts: the stub adapter has no LLM");
}

// --- a scope question -----------------------------------------------------------------------------

#[test]
fn a_scope_question_is_answered_with_the_answer_of_the_chosen_option_and_the_line_names_the_requester() {
    let w = world("requester-question");
    std::fs::write(w.run.join("bin/question-request.json"), question_request()).unwrap();
    std::fs::write(
        w.run.join("decisions.jsonl"),
        concat!(
            "{\"principal\":\"decider:requester\",\"case\":\"IDEA-1.direction\",\"question\":\"q0\",\"decision\":\"approve\",\"answer\":\"earlier\",\"valid\":true}\n",
            "{\"principal\":\"decider:model\",\"case\":\"IDEA-1.direction\",\"question\":\"q9\",\"decision\":\"approve\",\"valid\":true}\n",
            "{\"principal\":\"decider:requester\",\"case\":\"IDEA-1.direction\",\"question\":\"q8\",\"decision\":\"escalate\",\"valid\":false}\n",
        ),
    )
    .unwrap();
    let asked = ask_with(&w, &settings(&w, json!({})), &question_line());
    let given = asked.result.as_ref().unwrap();
    assert_eq!((given.answer.as_str(), given.principal.as_deref()), ("Sign in with a one-time code", Some("decider:requester")));
    assert_eq!((asked.cost_micro_usd, asked.model_calls, asked.lines.len()), (50_000, 1, 1));
    let line = &asked.lines[0];
    assert_eq!((line["principal"].as_str(), line["side"].as_str(), line["class"].as_str(), line["decision"].as_str(), line["question"].as_str()), (Some("decider:requester"), Some("requester"), Some("scope_changing_answer"), Some("approve"), Some("q1")));
    let rec = &records(&w)[0];
    assert_eq!(files_below(&rec.join("cwd/in")), sorted(vec!["dissection.yaml", "decisions.json", "goals.yaml", "idea.yaml", "questions.yaml", "request.json"]));
    let earlier = json_file(&rec.join("cwd/in/decisions.json"));
    assert_eq!(earlier.as_array().unwrap().len(), 1, "only its own earlier valid decisions: {earlier}");
    assert_eq!((earlier[0]["question"].as_str(), earlier[0]["answer"].as_str()), (Some("q0"), Some("earlier")));
    let args = std::fs::read_to_string(w.run.join("bin/question-args.txt")).unwrap();
    assert!(args.contains("q1") && args.contains("decider:requester,person:requester") && args.contains("--holder\ndecider"), "{args}");
    assert!(std::fs::read_to_string(rec.join("prompt.md")).unwrap().contains("- in/dissection.yaml"));
    assert_eq!(json_file(&rec.join("request.json"))["node"], "requester.q1");
}

#[test]
fn a_scope_question_with_no_ground_takes_its_default_under_final_and_waits_under_stop() {
    let w = world("requester-question-no-ground");
    std::fs::write(w.run.join("bin/question-request.json"), question_request()).unwrap();
    set_reply(&w, None, NO_GROUND);
    let asked = ask_with(&w, &settings(&w, json!({})), &question_line());
    assert_eq!(asked.lines.iter().map(|l| l["decision"].as_str().unwrap()).collect::<Vec<_>>(), ["escalation_suppressed", "approve"]);
    assert!(asked.lines.iter().all(|l| l["principal"] == "decider:requester" && l["side"] == "requester"));
    assert_eq!(asked.lines[1]["suppressed"], true);
    let given = asked.result.unwrap();
    assert_eq!((given.answer.as_str(), given.principal.as_deref()), ("a one-time code", Some("decider:requester")));
    let w = world("requester-question-no-ground-stop");
    std::fs::write(w.run.join("bin/question-request.json"), question_request()).unwrap();
    set_reply(&w, None, NO_GROUND);
    let asked = ask_with(&w, &settings(&w, json!({"escalate": "stop"})), &question_line());
    assert!(asked.result.unwrap_err().starts_with("reserved for a person: the decider escalated (escalate stop): no ground in the goals"));
}

#[test]
fn a_question_that_sdc_decision_request_refuses_is_an_invalid_reply_and_takes_the_default_under_final() {
    let w = world("requester-question-refused");
    let asked = ask_with(&w, &settings(&w, json!({"retries": 0})), &question_line());
    assert_eq!(asked.lines.iter().map(|l| l["decision"].as_str().unwrap()).collect::<Vec<_>>(), ["invalid_reply", "approve"]);
    assert_eq!(asked.lines[0]["invalid_reason"], "no decision request");
    assert_eq!(asked.result.unwrap().answer, "a one-time code");
    assert!(!tools(&w).join("adapter-request.json").exists(), "no model call without a request");
}
