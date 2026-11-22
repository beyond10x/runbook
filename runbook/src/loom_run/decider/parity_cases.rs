//! The cases of the parity lane (`parity.rs`): invented fixtures (a case directory and its work
//! files about an imaginary tide-table viewer), the requests and the replies of the scripted adapter.
//! Deleted with `decider.sh`, with the lane.

use serde_json::{json, Value};
use std::path::Path;

/// The case directory's name (its last member names the story whose `aep/story/<s>.md` is read).
pub const CASE: &str = "IDEA-1.direction.s1";

pub enum Kind {
    Rule(&'static str),
    Model { adapter: &'static str, final_escalate: bool },
}

pub struct Case {
    pub name: String,
    pub kind: Kind,
    /// The request members except `case_dir` and `record_dir`, which the lane sets.
    pub body: Value,
    /// What the scripted adapter writes as the model's `out/decision.json` (none: nothing).
    pub decision: Option<String>,
    /// What the scripted adapter writes as `structured-output.json` (none: nothing).
    pub structured: Option<String>,
    /// What the scripted adapter prints.
    pub response: String,
    /// A shell line the scripted adapter runs last (an exit code).
    pub tail: &'static str,
    /// The file name of the scripted adapter (a name ending `llm-adapter-fixture.sh` has its own schema note).
    pub adapter_file: &'static str,
}

const WORK: [(&str, &str); 14] = [
    ("idea.yaml", "idea: A pocket viewer for harbour tide tables.\naudience: Small-boat sailors who check tides before leaving the mooring.\nconstraint: Works offline; one screen; no account.\n"),
    ("dissection.yaml", "items:\n  - id: i1\n    text: Which harbours are shown first?\n    span: Small-boat sailors who check tides before leaving the mooring.\n  - id: i2\n    text: Is the viewer offline only?\n    span: Works offline; one screen; no account.\n"),
    ("questions.yaml", "questions:\n  - id: q1\n    item: i1\n    question: Which harbours are listed on first start?\n    status: open\n    default: the three nearest\n    scope_changing: true\n  - id: q2\n    item: i2\n    question: May the viewer fetch fresh tables when online?\n    answer: no, tables ship with the app\n    status: accepted\n    answered_by: person:owner\n"),
    ("requirements.yaml", "requirements:\n  - id: r1\n    text: The viewer lists the next four tides of one harbour.\n    cites: [q2]\n  - id: r2\n    text: The viewer works without a network.\n    cites: [q2]\n"),
    ("context.yaml", "scope:\n  value: one screen, one harbour at a time\n  status: accepted\nplatform:\n  value: a phone browser\n  status: accepted\n"),
    ("findings.yaml", "findings:\n  - id: f1\n    status: needs_person\n    default: defer\n    cites: [r1, context.scope]\n    text: The requirement r1 and the scope disagree on how many harbours show at once.\n"),
    ("aep/vision/product.md", "# Tide viewer\n\nA small offline viewer for tide tables.\n"),
    ("aep/initiative/roadmap.md", "# Roadmap\n\n- first screen\n- second harbour\n"),
    ("aep/epic/e1.md", "# First screen\n\nShows the next four tides.\n"),
    ("aep/story/s1.md", "# Show four tides\n\nAs a sailor I see the next four tides.\n"),
    ("published/README.md", "# Tide viewer\n\nInstall by opening the page once.\n"),
    ("notes/extra.txt", "line one of the extra note\nline two of the extra note\n"),
    ("Cargo.lock", "# fixture lock file\n"),
    ("target/skip.txt", "left out of the copy\n"),
];

const CASE_FILES: [(&str, &str); 7] = [
    ("case.json", r#"{"id":"IDEA-1.direction.s1","artifacts":{"context":{"revision":"taee57460908c89e2"},"requirements":{"revision":"te23630e115159d67"}}}"#),
    ("ir.json", r#"{"actions":{"context.approve":{"precondition":{"all":[{"claim":{"id":"context.valid","is":"true"}},{"claim":{"id":"requirements.valid","is":"true"}}]}},"scope.answer":{"precondition":{"claim":{"id":"questions.settled","is":"false"}}}},"outcomes":{"kept":{"requires":{"claim":{"id":"roadmap.approved","is":"true"}}}}}"#),
    ("bindings.json", r#"{"context.approve":{"produces":"context_approval"}}"#),
    ("events.jsonl", "{\"event\":\"action_finished\",\"action\":\"context.approve\",\"evidence\":[\"e1\"],\"log\":\"l1.json\"}\n"),
    ("evidence/e1.json", r#"{"kind":"context_approval","result":"rejected","subject_revision":"told00000000000000","observed_at":"2026-01-02T03:04:05Z","producer":"decider:model"}"#),
    ("blobs/l1.json", r#"{"reasons":["the scope names two harbours at once"],"citations":[]}"#),
    ("evidence/e2.json", r#"{"kind":"context_approval","result":"approved","subject_revision":"taee57460908c89e2","observed_at":"2026-01-03T03:04:05Z","producer":"person:owner"}"#),
];

/// Writes the case directory `<dir>/<CASE>/` with its work files and returns nothing else to know.
pub fn write_fixture(dir: &Path) {
    let case = dir.join(CASE);
    for (name, text) in CASE_FILES {
        put(&case.join(name), text);
    }
    for (name, text) in WORK {
        put(&case.join("work").join(name), text);
    }
}

fn put(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn position() -> Value {
    json!({"claims": {"context.valid": {"value": "true", "checked_by": "context.check"}, "requirements.valid": {"value": "true"},
        "questions.settled": {"value": "false"}, "roadmap.approved": {"value": "true"}, "release.healthy": {"value": "unknown"}},
        "open_obligations": ["obligation.review"], "legitimate_outcomes": ["kept", "dropped"]})
}

fn approval(action: &str, subject: &str, path: Option<&str>) -> Value {
    json!({"type": "approval", "action": action, "capability": "owner.approve", "subject": subject, "path": path,
        "revision": "taee57460908c89e2", "position": position()})
}

fn question(status: &str) -> Value {
    json!({"type": "question", "action": "scope.answer", "capability": "owner.answer", "subject": "questions", "path": "questions.yaml",
        "revision": "te23630e115159d67", "position": position(),
        "question": {"id": "q1", "item": "i1", "question": "Which harbours are listed on first start?", "default": "the three nearest", "status": status}})
}

fn finding(rule: bool) -> Value {
    let mut f = json!({"id": "f1", "status": "needs_person", "default": "defer", "cites": ["r1", "context.scope"],
        "text": "The requirement r1 and the scope disagree on how many harbours show at once."});
    if rule {
        f["sides"] = json!([{"claim": "r1", "says": "four tides"}, {"claim": "context.scope", "says": "one harbour"}]);
        f["options"] = json!(["keep-r1", "keep-scope"]);
        f["default"] = json!("keep-scope");
    }
    json!({"type": "finding", "action": "amend.upstream", "capability": "owner.approve", "subject": "requirements", "path": "findings.yaml",
        "revision": "te23630e115159d67", "position": position(), "finding": f})
}

fn close() -> Value {
    json!({"type": "close", "action": null, "capability": null, "subject": null, "path": null, "revision": null,
        "outcome": "kept", "position": position()})
}

fn with(mut body: Value, members: Value) -> Value {
    for (k, v) in members.as_object().unwrap() {
        body[k] = v.clone();
    }
    body
}

const RESPONSE: &str = r#"{"model":"claude-opus-5-5","effort":"xhigh","adapter":"claude-cli","usage":{"input_tokens":1200,"output_tokens":340},"cost_usd":0.0731,"duration_ms":4210}"#;
const CONSTRAINED: &str = r#"{"model":"claude-opus-5-5","effort":"high","adapter":"claude-cli","usage":null,"cost_usd":0.25,"duration_ms":90,"schema_constrained":true,"schema_note":"a schema was passed"}"#;

const APPROVE: &str = r#"{"decision":"approve","reasons":["the context fits the idea"],"cites":["context@taee57460908c89e2","idea.yaml:1"]}"#;
const APPROVE_BARE: &str = r#"{"decision":"approve","reasons":["nothing in the inputs speaks against it"]}"#;
const REJECT: &str = r#"{"decision":"reject","reasons":["the scope names two harbours at once"],"cites":["context@taee57460908c89e2"]}"#;
const ESCALATE_BEST: &str = r#"{"decision":"escalate","best":"approve","reasons":["the checks are silent on offline use"],"cites":[]}"#;
const ESCALATE_BARE: &str = r#"{"decision":"escalate","reasons":["the checks are silent on offline use"],"cites":[]}"#;
const BAD_CITE: &str = r#"{"decision":"approve","reasons":["x"],"cites":["context@tffffffffffffffff","nowhere.txt:9"]}"#;
const UNKNOWN_KEY: &str = r#"{"decision":"approve","reasons":["x"],"cites":[],"confidence":0.9}"#;
const NO_REASONS: &str = r#"{"decision":"approve","reasons":[],"cites":[]}"#;
const NOT_JSON: &str = "{not json at all";
const Q_APPROVE: &str = r#"{"decision":"approve","answer":"the three nearest","reasons":["the idea names small-boat sailors"],"cites":["idea.yaml:2"]}"#;
const Q_NO_ANSWER: &str = r#"{"decision":"approve","reasons":["fine"],"cites":[]}"#;
const Q_REJECT: &str = r#"{"decision":"reject","reasons":["no"],"cites":[]}"#;
const F_FREE: &str = r#"{"decision":"approve","answer":"keep r1 and show four tides of the chosen harbour","reasons":["r1 is the narrower claim"],"cites":["r1","context.scope"]}"#;
const F_RULE: &str = r#"{"decision":"approve","answer":"keep-r1","reasons":["r1 is the narrower claim"],"cites":["r1"]}"#;
const F_RULE_OFF: &str = r#"{"decision":"approve","answer":"keep neither","reasons":["x"],"cites":[]}"#;
const F_BEST_OPTION: &str = r#"{"decision":"escalate","best":"keep-scope","reasons":["both sides read the idea"],"cites":[]}"#;
const F_BEST_OFF: &str = r#"{"decision":"escalate","best":"keep neither","reasons":["x"],"cites":[]}"#;

fn model(name: &str, adapter: &'static str, final_escalate: bool, body: Value, decision: Option<&str>) -> Case {
    Case {
        name: name.to_owned(),
        kind: Kind::Model { adapter, final_escalate },
        body,
        decision: decision.map(str::to_owned),
        structured: None,
        response: RESPONSE.to_owned(),
        tail: "",
        adapter_file: "scripted-adapter.sh",
    }
}

/// Every case: each kind (rules, the three adapters) on each request type, the scripted adapter's
/// valid, invalid and escalating replies, and the members that change the prompt.
pub fn cases() -> Vec<Case> {
    let mut all = Vec::new();
    let types: [(&str, Value); 6] = [
        ("approval", approval("context.approve", "context", Some("context.yaml"))),
        ("question", question("open")),
        ("finding-free", finding(false)),
        ("finding-rule", finding(true)),
        ("close", close()),
        ("approval-no-prior", approval("requirements.approve", "requirements", Some("requirements.yaml"))),
    ];
    for (name, body) in &types {
        for verdict in ["approve", "reject"] {
            all.push(Case { kind: Kind::Rule(verdict), ..model(&format!("rule-{verdict}-{name}"), "", false, body.clone(), None) });
        }
        for adapter in ["dry", "stub"] {
            all.push(model(&format!("{adapter}-{name}"), adapter, false, body.clone(), None));
            all.push(model(&format!("{adapter}-final-{name}"), adapter, true, body.clone(), None));
        }
    }
    let ap = &types[0].1;
    for (label, decision) in [("ok", Some(APPROVE)), ("bare", Some(APPROVE_BARE)), ("reject", Some(REJECT)), ("escalate-best", Some(ESCALATE_BEST)), ("escalate-bare", Some(ESCALATE_BARE)),
        ("bad-cite", Some(BAD_CITE)), ("unknown-key", Some(UNKNOWN_KEY)), ("no-reasons", Some(NO_REASONS)), ("not-json", Some(NOT_JSON)), ("missing", None)] {
        for final_escalate in [false, true] {
            all.push(model(&format!("scripted-approval-{label}-{}", if final_escalate { "final" } else { "stop" }), "scripted", final_escalate, ap.clone(), decision));
        }
    }
    let q = &types[1].1;
    for (label, decision) in [("approve", Q_APPROVE), ("no-answer", Q_NO_ANSWER), ("reject", Q_REJECT), ("escalate", ESCALATE_BARE), ("escalate-best", ESCALATE_BEST)] {
        for final_escalate in [false, true] {
            all.push(model(&format!("scripted-question-{label}-{}", if final_escalate { "final" } else { "stop" }), "scripted", final_escalate, q.clone(), Some(decision)));
        }
    }
    for (label, body, decisions) in [("free", &types[2].1, [F_FREE, F_RULE_OFF, Q_REJECT, ESCALATE_BARE, F_BEST_OFF]), ("rule", &types[3].1, [F_RULE, F_RULE_OFF, Q_REJECT, F_BEST_OPTION, F_BEST_OFF])] {
        for (n, decision) in decisions.iter().enumerate() {
            all.push(model(&format!("scripted-finding-{label}-{n}"), "scripted", n % 2 == 1, body.clone(), Some(decision)));
        }
    }
    for (label, decision) in [("approve", APPROVE_BARE), ("escalate", ESCALATE_BEST), ("answer-given", Q_APPROVE)] {
        all.push(model(&format!("scripted-close-{label}"), "scripted", false, types[4].1.clone(), Some(decision)));
    }
    let variants: [(&str, Value); 6] = [
        ("retry", with(ap.clone(), json!({"retry": {"attempt": 2, "invalid_reason": "cites what does not resolve: nowhere.txt:9 (no file nowhere.txt in in/)"}}))),
        ("note", with(ap.clone(), json!({"note": "the run stopped here twice"}))),
        ("refusal", with(types[2].1.clone(), json!({"refusal": {"why": "the answer was not one of the options"}}))),
        ("answered-question", question("accepted")),
        ("path-extra", approval("context.approve", "context", Some("notes/extra.txt"))),
        ("path-none", approval("release.publish", "release", None)),
    ];
    for (label, body) in variants {
        all.push(model(&format!("scripted-variant-{label}"), "scripted", label == "retry", body, Some(if label.starts_with("answered") { Q_APPROVE } else if label == "refusal" { F_FREE } else { APPROVE })));
    }
    let mut constrained = model("scripted-constrained-response", "scripted", false, ap.clone(), Some(APPROVE));
    constrained.response = CONSTRAINED.to_owned();
    all.push(constrained);
    let mut structured = model("scripted-structured-output", "scripted", false, ap.clone(), None);
    structured.structured = Some(APPROVE.to_owned());
    all.push(structured);
    let mut failing = model("scripted-adapter-exit-3", "scripted", false, ap.clone(), None);
    failing.tail = "exit 3\n";
    all.push(failing);
    let mut not_object = model("scripted-response-not-an-object", "scripted", false, ap.clone(), Some(APPROVE));
    not_object.response = "no response today".to_owned();
    all.push(not_object);
    let mut fixture_named = model("scripted-fixture-named-adapter", "scripted", false, ap.clone(), Some(APPROVE));
    fixture_named.adapter_file = "llm-adapter-fixture.sh";
    all.push(fixture_named);
    all
}
