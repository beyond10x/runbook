//! The prompts of the decider and the request of the requester, each held byte for byte against a
//! committed golden under `instructions/golden/decider/` and `instructions/golden/requester-request/`
//! (`SDC_BLESS_PROMPTS=1` rewrites them from this code). The model prompts are made by the pure
//! function `prompt::prompt` over what `inputs::prepare` leaves for a fixture case; the requester's
//! prompts are those of the calls in `requester_tests.rs`. Invented text only, no model, no program
//! is started.

use super::*;
use crate::assets::testdir::Dir;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

/// `text` is the golden `group/name`, or, when `SDC_BLESS_PROMPTS` is set, becomes it.
pub(super) fn compare(group: &str, name: &str, text: &str) {
    let golden = repo().join("instructions/golden").join(group).join(format!("{name}.md"));
    if std::env::var_os("SDC_BLESS_PROMPTS").is_some() {
        std::fs::create_dir_all(golden.parent().unwrap()).unwrap();
        std::fs::write(&golden, text).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&golden).unwrap_or_else(|_| panic!("no golden {}", golden.display()));
    assert!(text == want, "{group}/{name}: differs from {}\n--- got\n{text}\n--- want\n{want}", golden.display());
}

/// The names of the goldens of `group` on disk.
fn golden_names(group: &str) -> std::collections::BTreeSet<String> {
    std::fs::read_dir(repo().join("instructions/golden").join(group))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().trim_end_matches(".md").to_owned())
        .collect()
}

/// The prompts that `requester_tests.rs` compares; every other golden of the group is made here.
pub(super) const REQUESTER_PROMPTS: [&str; 2] = ["requester-approval", "requester-approval-retry"];

/// A case the way the first decider saw one: the intake and direction files, a vision and a roadmap.
fn case(dir: &Dir) -> PathBuf {
    let case = dir.dir("case");
    let work = dir.dir("case/work");
    let fixtures = repo().join("driver/examples/sdc/fixtures");
    for (from, to) in [("idea.yaml", "idea.yaml"), ("dissection.yaml", "dissection.yaml"), ("requirements.yaml", "requirements.yaml"), ("context.yaml", "context.yaml"), ("questions-open.yaml", "questions.yaml")] {
        std::fs::copy(fixtures.join(from), work.join(to)).unwrap();
    }
    for (path, text) in [("aep/vision/product.md", "# vision\n"), ("aep/initiative/roadmap.md", "# roadmap\n")] {
        std::fs::create_dir_all(work.join(path).parent().unwrap()).unwrap();
        std::fs::write(work.join(path), text).unwrap();
    }
    std::fs::write(case.join("case.json"), r#"{"id":"IDEA-1.direction","artifacts":{"goals":{"revision":"g1"}}}"#).unwrap();
    std::fs::write(case.join("ir.json"), r#"{"actions":{"roadmap.approve":{}},"outcomes":{}}"#).unwrap();
    case
}

/// The prompt of the model decider for `req`, `final_escalate` as the settings give it and
/// `prior` earlier rejections of the revision.
fn prompt_of(tag: &str, req: &Value, final_escalate: bool, prior: usize) -> String {
    let dir = Dir::new(tag);
    let case = case(&dir);
    let cwd = dir.dir("cwd");
    dir.dir("cwd/in");
    let renderer = prompt::Renderer { root: repo() };
    let prepared = inputs::prepare(req, &case, &Held::make(cwd.parent().unwrap(), &cwd).unwrap(), &renderer, final_escalate).unwrap();
    let has_aep = cwd.join("in").join("aep").exists();
    // What the citable set and the later-action note say is fixed here, not read from the fixture:
    // the examples of the citable forms and a later action that meets the open obligations of
    // every request but a close.
    let citable = json!({"artifacts": [], "examples": {"id": "r1", "entry": "requirements@1#r1", "line": "idea.yaml:1"}});
    let met_after = req["type"] != "close";
    let parts = prompt::Parts { req, final_escalate, inputs: &prepared.inputs, citable: &citable, prior_rejections: prior, has_aep, met_after };
    prompt::prompt(&renderer, &parts).unwrap()
}

fn with(mut v: Value, k: &str, x: Value) -> Value {
    v[k] = x;
    v
}

#[test]
fn the_prompt_of_each_model_decider_is_its_golden_byte_for_byte() {
    let position = json!({"claims": {"roadmap.required": {"value": "false"}, "health.required": {"value": "unknown"}}, "open_obligations": ["roadmap.required", "o1"], "legitimate_outcomes": []});
    let approval = json!({"type": "approval", "action": "roadmap.approve", "capability": "approve", "subject": "roadmap", "path": "aep/initiative/roadmap.md", "revision": "r7", "position": position});
    let question = json!({"type": "question", "action": "scope.answer", "path": "questions.yaml", "position": position,
        "question": {"id": "q1", "item": "d3", "question": "Which states does an invoice go through?", "options": ["a", "b"], "default": "a", "status": "open"}});
    let finding = |sides: bool| {
        let mut f = json!({"id": "f1", "says": "Two readings.", "options": ["x", "y"], "default": "x", "cites": ["r1"]});
        if sides {
            f["sides"] = json!(["r1", "r2"]);
        }
        json!({"type": "finding", "action": "amend.upstream", "position": position, "finding": f})
    };
    let close = json!({"type": "close", "action": "close", "outcome": "delivered", "position": position});
    let answered = with(question.clone(), "question", json!({"id": "q1", "item": "d3", "question": "Q?", "options": ["a"], "default": "a", "status": "accepted", "answer": "a"}));
    let asked_again = with(with(approval.clone(), "note", json!("The run stopped here.")), "refusal", json!({"why": "the check refused it"}));
    let retried = with(approval.clone(), "retry", json!({"attempt": 2, "invalid_reason": "cites what does not resolve"}));
    let cases: Vec<(&str, Value, bool, usize)> = vec![
        ("approval", approval.clone(), false, 0),
        ("approval-final", approval.clone(), true, 0),
        ("approval-prior-rejection", retried, false, 1),
        ("approval-asked-again", asked_again, false, 0),
        ("question", question.clone(), false, 0),
        ("question-final", question, true, 0),
        ("question-answered", answered, false, 0),
        ("finding", finding(false), false, 0),
        ("finding-final", finding(false), true, 0),
        ("finding-rule", finding(true), false, 0),
        ("close", close.clone(), false, 0),
        ("close-final", close, true, 0),
    ];
    let mut seen: std::collections::BTreeSet<String> = REQUESTER_PROMPTS.iter().map(|n| (*n).to_owned()).collect();
    for (name, req, final_escalate, prior) in cases {
        seen.insert(name.to_owned());
        compare("decider", name, &prompt_of(&format!("golden-{name}"), &req, final_escalate, prior));
    }
    if std::env::var_os("SDC_BLESS_PROMPTS").is_none() {
        assert_eq!(seen, golden_names("decider"), "a golden no test reads, or a prompt with no golden");
    }
}

#[test]
fn the_request_of_the_requester_is_its_golden_byte_for_byte() {
    let dir = Dir::new("golden-requester-request");
    let case = case(&dir);
    let renderer = prompt::Renderer { root: repo() };
    let position = json!({"claims": {}, "open_obligations": [], "legitimate_outcomes": []});
    let approval = json!({"type": "approval", "action": "roadmap.approve", "capability": "approve", "subject": "roadmap", "path": "aep/initiative/roadmap.md", "revision": "r7", "class": "approval", "position": position});
    let close = json!({"type": "close", "action": "close", "outcome": "delivered", "class": "close", "position": position});
    let bare = json!({"type": "approval", "action": "plan.approve", "subject": "plan", "position": position});
    for (name, req) in [("approval", approval), ("approval-no-revision", bare), ("close", close)] {
        let request = requester::approval_request(&renderer, &req, &case, "A3").unwrap();
        compare("requester-request", name, &format!("{}\n", serde_json::to_string(&request).unwrap()));
    }
    if std::env::var_os("SDC_BLESS_PROMPTS").is_none() {
        assert_eq!(golden_names("requester-request"), ["approval", "approval-no-revision", "close"].map(String::from).into_iter().collect());
    }
}
