//! The guards of the autonomy level and the decider kind of a run, built in process: what the daemon records,
//! what it refuses before it writes, and who an answer may be recorded under. The runtime here is a script of its
//! own that asks and tries to answer as a decider would; the engine and the decider of the binary are the cases of
//! `autonomy.rs` (the binary) and of `src/loom_run/arbiter.rs`.

use super::commands::{config, Fx};
use super::http::Server;
use super::stub::ask;
use runbook::commands::{client_label, AnswerDecision, CancelRun, Daemon, Fault, Policy, SubmitRun};
use runbook::runtime::{AnswerFailure, Autonomy, Confinement, DecisionRequest, EventSink, Finish, Recovery, RunbookInfo, Runtime, RuntimeFailure, StartRequest};
use runbook::stub::{Script, Step, StubRuntime};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Asks one request for each try, then tries to answer each as `(by, principal)` and keeps what the daemon said.
struct Trying {
    tries: Vec<(&'static str, &'static str, &'static str)>,
    taken: Mutex<Vec<bool>>,
    started: Mutex<Vec<StartRequest>>,
}

impl Trying {
    fn new(tries: Vec<(&'static str, &'static str, &'static str)>) -> Arc<Trying> {
        Arc::new(Trying { tries, taken: Mutex::new(Vec::new()), started: Mutex::new(Vec::new()) })
    }
}

struct Rt(Arc<Trying>);

impl Runtime for Rt {
    fn recover(&self, _run_id: &str) -> Recovery {
        Recovery::Gone
    }
    fn knows_runbook(&self, _runbook: &str, _version: Option<&str>) -> bool {
        true
    }
    fn runbooks(&self) -> Vec<RunbookInfo> {
        Vec::new()
    }
    fn kind(&self) -> &'static str {
        "stub"
    }
    fn start(&self, request: StartRequest, sink: &dyn EventSink, cancel: &AtomicBool) -> Result<Finish, RuntimeFailure> {
        self.0.started.lock().unwrap().push(request);
        sink.started();
        for (id, _, _) in &self.0.tries {
            sink.decision_requested(DecisionRequest { request_id: (*id).into(), case_id: "c".into(), action: "notes.stage".into(), capability: "notes.stage".into(), question: "q".into(), options: vec![] });
        }
        for (id, by, principal) in &self.0.tries {
            let taken = sink.decision_answered(id, "approve", by, principal);
            self.0.taken.lock().unwrap().push(taken);
        }
        while !cancel.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(Finish::Cancelled)
    }
    fn answer(&self, _run_id: &str, _request_id: &str, _answer: &str, _principal: &str) -> Result<(), AnswerFailure> {
        Ok(())
    }
    fn cancel(&self, _run_id: &str) {}
}

fn policy(autonomy: Option<Autonomy>, decider: Option<&str>, kinds: &[&str]) -> Policy {
    Policy { autonomy, decider: decider.map(str::to_owned), kinds: kinds.iter().map(|k| (*k).to_owned()).collect(), ..Policy::default() }
}

fn start(dir: &std::path::Path, runtime: &Arc<Trying>, policy: Policy) -> Daemon {
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    Daemon::start_with(config(dir), policy, Arc::new(Rt(runtime.clone()))).unwrap()
}

fn submit(d: &Daemon, autonomy: Option<Autonomy>, decider: Option<&str>) -> Result<String, Fault> {
    d.submit_run(SubmitRun { runbook_id: "any".into(), intake: "x".into(), confinement: Some(Confinement::Linux), autonomy, decider: decider.map(str::to_owned), ..SubmitRun::default() }).map(|o| {
        assert_eq!(o.body()["outcome"], "submitted", "{}", o.body());
        o.run_id.unwrap()
    })
}

fn until(what: &str, mut done: impl FnMut() -> bool) {
    let limit = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < limit, "gave up waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn decisions(d: &Daemon, run: &str) -> Vec<Value> {
    d.decisions_of_run(run)
}

#[test]
fn an_answer_of_a_decider_is_recorded_under_its_own_name_and_never_a_person_or_a_label_a_client_typed() {
    let dir = super::state::scratch("auto-labels");
    let rt = Trying::new(vec![("q1", "rule:approve", "decider:approve"), ("q2", "person:alice", "person:alice"), ("q3", "operator", "operator"), ("q4", "decider:model", "decider:model"), ("q5", "rule:", "x"), ("q6", "decider:", "x")]);
    let d = start(&dir, &rt, policy(Some(Autonomy::A5), Some("rule"), &["rule"]));
    let run = submit(&d, None, None).unwrap();
    until("the six tries", || rt.taken.lock().unwrap().len() == 6);
    assert_eq!(*rt.taken.lock().unwrap(), [true, false, false, true, false, false], "only a name of a rule or of a decider is taken");
    let rows = decisions(&d, &run);
    let row = |id: &str| rows.iter().find(|r| r["request_id"] == id).unwrap().clone();
    assert_eq!((row("q1")["state"].clone(), row("q1")["answered_by"].clone(), row("q1")["principal"].clone()), (json!("Answered"), json!("rule:approve"), json!("decider:approve")));
    assert_eq!((row("q4")["answered_by"].clone(), row("q4")["principal"].clone()), (json!("decider:model"), json!("decider:model")));
    for id in ["q2", "q3", "q5", "q6"] {
        assert_eq!((row(id)["state"].clone(), row(id)["answered_by"].clone()), (json!("Open"), Value::Null), "{id} stays open for a client");
    }
    // the client's answer is the label it typed and the daemon's principal, and a request a decider answered takes no second answer
    for label in ["rule:approve", "decider:model", "rule:"] {
        match d.answer_decision(AnswerDecision { run_id: run.clone(), request_id: "q2".into(), answer: "approve".into(), answered_by: label.into() }) {
            Err(Fault::Invalid(why)) => assert!(why.contains("rule:") && why.contains("decider:"), "{why}"),
            other => panic!("a client typed the label {label:?} of a decider and it was taken: {other:?}"),
        }
    }
    assert_eq!(decisions(&d, &run).into_iter().find(|r| r["request_id"] == "q2").unwrap()["state"], "Open", "a refused label answers nothing");
    let client = d.answer_decision(AnswerDecision { run_id: run.clone(), request_id: "q2".into(), answer: "approve".into(), answered_by: "operator".into() }).unwrap();
    assert_eq!(client.body()["outcome"], "answered", "{}", client.body());
    let row2 = decisions(&d, &run).into_iter().find(|r| r["request_id"] == "q2").unwrap();
    assert_eq!((row2["answered_by"].clone(), row2["principal"].clone()), (json!("operator"), json!(super::PRINCIPAL)), "a client's answer carries the daemon's principal and the label it typed");
    let again = d.answer_decision(AnswerDecision { run_id: run.clone(), request_id: "q1".into(), answer: "deny".into(), answered_by: "op".into() }).unwrap();
    assert_eq!(again.body()["outcome"], "already-answered", "{}", again.body());
    d.cancel_run(CancelRun { run_id: run }).unwrap();
    d.stop();
}

#[test]
fn a_submit_takes_the_level_and_the_kind_of_the_daemon_and_names_win_and_the_start_event_records_both() {
    let dir = super::state::scratch("auto-defaults");
    let rt = Trying::new(vec![]);
    let d = start(&dir, &rt, policy(Some(Autonomy::A3), Some("rule"), &["model", "rule"]));
    let row = |run: &str| d.run_detail(run)[0].clone();
    let plain = submit(&d, None, None).unwrap();
    assert_eq!((row(&plain)["autonomy"].clone(), row(&plain)["decider"].clone()), (json!("A3"), json!("rule")), "{}", row(&plain));
    let named = submit(&d, Some(Autonomy::A5), Some("model")).unwrap();
    assert_eq!((row(&named)["autonomy"].clone(), row(&named)["decider"].clone()), (json!("A5"), json!("model")), "the submit wins");
    let lower = submit(&d, Some(Autonomy::A0), None).unwrap();
    assert_eq!((row(&lower)["autonomy"].clone(), row(&lower)["decider"].clone()), (json!("A0"), json!("rule")));
    let record = d.events_after(&named, 0)[0]["record"].clone();
    assert_eq!((record["autonomy"].clone(), record["decider"].clone()), (json!("A5"), json!("model")), "the start event names level and kind: {record}");
    until("the runtime has the three requests", || rt.started.lock().unwrap().len() == 3);
    let given: Vec<_> = rt.started.lock().unwrap().iter().map(|r| (r.terms.autonomy, r.terms.decider.clone())).collect();
    assert!(given.contains(&(Some(Autonomy::A5), Some("model".to_owned()))) && given.contains(&(Some(Autonomy::A3), Some("rule".to_owned()))), "{given:?}");
    for run in [plain, named, lower] {
        d.cancel_run(CancelRun { run_id: run }).unwrap();
    }
    d.stop();
}

#[test]
fn a_daemon_with_no_level_and_a_submit_with_none_start_a_run_that_every_decision_waits_in() {
    let dir = super::state::scratch("auto-none");
    let rt = Trying::new(vec![]);
    let d = start(&dir, &rt, policy(None, None, &[]));
    let run = submit(&d, None, None).unwrap();
    let row = d.run_detail(&run)[0].clone();
    assert!(row.get("autonomy").is_none() && row.get("decider").is_none(), "no level, no kind: {row}");
    until("the start", || rt.started.lock().unwrap().len() == 1);
    let terms = rt.started.lock().unwrap()[0].terms.clone();
    assert_eq!((terms.autonomy, terms.decider), (None, None));
    d.cancel_run(CancelRun { run_id: run }).unwrap();
    d.stop();
}

#[test]
fn a_kind_the_operator_did_not_configure_is_refused_before_a_run_directory_or_a_record_exists() {
    let dir = super::state::scratch("auto-unconfigured");
    let rt = Trying::new(vec![]);
    let d = start(&dir, &rt, policy(None, None, &["rule"]));
    let refused = |autonomy: Option<Autonomy>, decider: Option<&str>| match submit(&d, autonomy, decider) {
        Err(Fault::Invalid(message)) => message,
        other => panic!("not refused: {other:?}"),
    };
    let message = refused(Some(Autonomy::A5), Some("model"));
    assert!(message.contains("model") && message.contains("rule") && message.contains("configured"), "{message}");
    assert!(!message.contains("scripted"), "only the configured kinds: {message}");
    let message = refused(None, Some("../bin/anything"));
    assert!(message.contains("rule"), "{message}");
    let message = refused(Some(Autonomy::A3), None);
    assert!(message.contains("level") && message.contains("rule"), "a level with no kind to take it: {message}");
    assert!(d.runs().is_empty(), "no run");
    assert!(!dir.join("state/runs").read_dir().map(|mut r| r.next().is_some()).unwrap_or(false), "no run directory");
    assert!(d.events_of_daemon_after(0).iter().all(|e| e["kind"] != "runbook.submitted"), "no record of a submit");
    assert!(rt.started.lock().unwrap().is_empty(), "the runtime was not asked");
    d.stop();
}

#[test]
fn the_level_and_the_kind_of_a_run_are_read_from_its_record_after_a_restart_not_from_the_daemons_policy() {
    let dir = super::state::scratch("auto-restart");
    let rt = Trying::new(vec![]);
    let d = start(&dir, &rt, policy(Some(Autonomy::A5), Some("rule"), &["rule", "model"]));
    let run = submit(&d, None, Some("model")).unwrap();
    d.stop();
    drop(d);
    let d = start(&dir, &rt, policy(Some(Autonomy::A0), Some("rule"), &["rule"]));
    let row = d.run_detail(&run)[0].clone();
    assert_eq!((row["autonomy"].clone(), row["decider"].clone()), (json!("A5"), json!("model")), "{row}");
    d.stop();
}

#[test]
fn a_default_level_needs_a_kind_to_take_it_or_the_daemon_does_not_start() {
    let dir = super::state::scratch("auto-nokind");
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    let rt = Trying::new(vec![]);
    let started = |p: Policy| Daemon::start_with(config(&dir), p, Arc::new(Rt(rt.clone()))).err().map(|e| e.to_string());
    let why = started(policy(Some(Autonomy::A3), None, &["rule"])).unwrap();
    assert!(why.contains("A3") && why.contains("decider"), "{why}");
    let why = started(policy(None, Some("model"), &["rule"])).unwrap();
    assert!(why.contains("model") && why.contains("rule"), "the default kind is one that is configured: {why}");
    assert!(!dir.join("state").exists(), "nothing was written");
    assert!(started(policy(Some(Autonomy::A3), Some("rule"), &["rule"])).is_none());
}

fn stub_with_quick() -> Arc<StubRuntime> {
    let stub = Arc::new(StubRuntime::new());
    stub.script("quick", Script::new(vec![Step::End("done".into())]));
    stub.script("hold", Script::new(vec![Step::Sleep(60_000)]));
    stub
}

#[test]
fn a_reserved_prefix_is_refused_in_every_spelling_of_a_client_label() {
    let fx = Fx::new("auto-labels");
    let spellings: Vec<String> = vec![
        "rule:approve".into(),
        "decider:model".into(),
        "Rule:approve".into(),
        "RULE:approve".into(),
        "Decider:model".into(),
        " rule:approve".into(),
        "\trule:approve".into(),
        "\u{a0}rule:approve".into(),
        "\u{200b}rule:approve".into(),
        "rule\u{200b}:approve".into(),
        "\u{ff52}ule:approve".into(),
        "rule\u{ff1a}approve".into(),
        "r\u{200d}ule:approve".into(),
        "\u{0440}ule:approve".into(),
        "rule:\u{200b}approve".into(),
    ];
    let asks: Vec<_> = (0..spellings.len()).map(|n| ask(&format!("s{n}"))).collect();
    fx.stub.script("labels-ask", Script::new(vec![Step::Ask(asks), Step::End("approved".into())]));
    let run = fx.submit("labels-ask");
    fx.wait_open_decisions(&run, spellings.len());
    let mut taken = Vec::new();
    for (n, label) in spellings.iter().enumerate() {
        let answered = fx.d().answer_decision(AnswerDecision { run_id: run.clone(), request_id: format!("s{n}"), answer: "A".into(), answered_by: label.clone() });
        if answered.is_ok_and(|o| o.name == "answered") {
            taken.push(label.escape_unicode().to_string());
        }
    }
    // the same prefix as a JSON escape arrives decoded: the body of a request, parsed as the route parses it
    let escaped: AnswerDecision = serde_json::from_str(r#"{"run_id":"r","request_id":"q","answer":"A","answered_by":"rule:approve"}"#).unwrap();
    assert_eq!(escaped.answered_by, "rule:approve");
    assert!(taken.is_empty(), "a client label that reads as a rule's or a decider's was recorded under the daemon's principal; the exact `rule:` and `decider:` are refused, and the ones that were taken: {taken:?}");
}

/// Spellings that read as a reserved name or hold a character outside printable ASCII, each with the text `MARK` after the prefix.
fn refused_labels() -> Vec<String> {
    let mut labels: Vec<String> = Vec::new();
    for prefix in ["rule:", "decider:", "router:"] {
        let mut title = prefix.to_owned();
        title[..1].make_ascii_uppercase();
        let alternating: String = prefix.chars().enumerate().map(|(i, c)| if i % 2 == 0 { c.to_ascii_uppercase() } else { c }).collect();
        for spelling in [prefix.to_owned(), prefix.to_ascii_uppercase(), title, alternating] {
            for pad in ["", " ", "\t", "\n", "\r\n", "\u{c}", "   \t ", " \n\t\r\u{c}"] {
                labels.push(format!("{pad}{spelling}MARK{pad}"));
            }
            labels.push(format!("{spelling} MARK"));
            labels.push(spelling);
        }
    }
    for odd in [
        "\u{b}rule:MARK", "\u{a0}rule:MARK", "\u{85}rule:MARK", "\u{2028}rule:MARK", "\u{2003}rule:MARK", "\u{3000}rule:MARK", "\u{feff}rule:MARK",
        "\u{200b}rule:MARK", "\u{200d}rule:MARK", "\u{2060}rule:MARK", "\u{ad}rule:MARK", "\u{202e}rule:MARK", "\u{e0072}rule:MARK", "ru\u{200b}le:MARK",
        "rule:\u{200b}MARK", "r\u{0}ule:MARK", "\u{0}rule:MARK", "rule:\u{0}MARK", "\u{7f}rule:MARK", "rule:\u{1b}[31mMARK", "\u{ff52}ule:MARK",
        "\u{0440}ule:MARK", "rule\u{ff1a}MARK", "de\u{0441}ider:MARK", "rou\u{0442}er:MARK", "ca\u{e9}MARK", "\u{1f600}MARK", "\u{a0}operatorMARK", "operatorMARK\u{3000}",
    ] {
        labels.push(odd.to_owned());
    }
    labels
}

#[test]
fn a_client_label_that_reads_as_a_rule_a_decider_or_a_router_is_refused_without_printing_a_byte_of_it() {
    let mut taken = Vec::new();
    let mut echoed = Vec::new();
    for label in refused_labels() {
        match client_label("answered_by", &label) {
            Ok(kept) => taken.push((label.escape_unicode().to_string(), kept)),
            Err(why) if why.contains("MARK") => echoed.push(why),
            Err(_) => {}
        }
    }
    assert!(taken.is_empty(), "labels that read as a reserved name or hold a character outside printable ASCII were kept: {taken:?}");
    assert!(echoed.is_empty(), "a refusal printed bytes of the label: {echoed:?}");
}

#[test]
fn a_client_label_is_trimmed_bounded_and_otherwise_kept_as_typed() {
    for (typed, kept) in [("operator", "operator"), ("  operator\t", "operator"), ("\n\u{c}a b\r", "a b"), ("rules:x", "rules:x"), ("rule", "rule"), ("router", "router"), ("decide:x", "decide:x"), ("x rule:y", "x rule:y"), ("a-client_1.2", "a-client_1.2")] {
        assert_eq!(client_label("answered_by", typed), Ok(kept.to_owned()), "{typed:?}");
    }
    assert!(client_label("answered_by", &"a".repeat(200)).is_ok());
    assert!(client_label("answered_by", &"a".repeat(201)).is_err());
    assert!(client_label("answered_by", &format!("{}rule:x", " ".repeat(150))).is_err(), "padding does not hide the prefix");
    // the same text as a JSON escape arrives decoded before the one function sees it
    for text in [r#"{"run_id":"r","request_id":"q","answer":"A","answered_by":"RULE:x"}"#, r#"{"run_id":"r","request_id":"q","answer":"A","answered_by":" rule:x"}"#, r#"{"run_id":"r","request_id":"q","answer":"A","answered_by":"\tDecider:"}"#] {
        let input: AnswerDecision = serde_json::from_str(text).unwrap();
        assert!(client_label("answered_by", &input.answered_by).is_err(), "{:?}", input.answered_by);
    }
}

#[test]
fn the_mcp_document_names_every_prefix_a_client_label_may_not_start_with() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let text = std::fs::read_to_string(root.join("docs/mcp.md")).unwrap();
    let line = text.lines().find(|l| l.contains("an `answered_by` that starts with")).expect("the sentence about the answered_by the tool refuses is gone");
    for prefix in ["`rule:`", "`decider:`", "`router:`"] {
        assert!(line.contains(prefix), "docs/mcp.md: the sentence on the `answered_by` the tool refuses does not name {prefix}, which `client_label` refuses");
    }
}

#[test]
fn a_resubmit_with_the_same_key_changes_neither_the_level_nor_the_kind_and_a_hostile_kind_is_not_echoed_whole() {
    let dir = super::state::scratch("auto-terms");
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    let policy = Policy { autonomy: None, decider: None, kinds: vec!["rule".into()], ..Policy::default() };
    let mut cfg = config(&dir);
    cfg.confinement_floor = Confinement::Local;
    let d = Daemon::start_with(cfg, policy, stub_with_quick()).unwrap();
    let first = d.submit_run(SubmitRun { runbook_id: "hold".into(), intake: "x".into(), confinement: Some(Confinement::Local), autonomy: Some(Autonomy::A5), decider: Some("rule".into()), idempotency_key: Some("k1".into()), ..SubmitRun::default() }).unwrap();
    let run = first.run_id.clone().unwrap();
    let again = d.submit_run(SubmitRun { runbook_id: "hold".into(), intake: "x".into(), confinement: Some(Confinement::Local), autonomy: Some(Autonomy::A0), decider: Some("rule".into()), idempotency_key: Some("k1".into()), ..SubmitRun::default() }).unwrap();
    assert_eq!((again.name, again.run_id.clone()), ("already-submitted", Some(run.clone())));
    let row = d.run_detail(&run)[0].clone();
    assert_eq!((row["autonomy"].clone(), row["decider"].clone()), (json!("A5"), json!("rule")), "{row}");
    let runs_before = d.runs().len();
    for hostile in ["x".repeat(5000), "/etc/passwd".to_owned(), "rule\nmodel".to_owned()] {
        let message = match d.submit_run(SubmitRun { runbook_id: "hold".into(), intake: "x".into(), confinement: Some(Confinement::Local), decider: Some(hostile.clone()), ..SubmitRun::default() }) {
            Err(Fault::Invalid(m)) => m,
            other => panic!("a kind that is not configured was not refused: {other:?}"),
        };
        assert!(message.len() < 400 && !message.contains(&"x".repeat(65)) && !message.contains('\n'), "the refusal echoes the client's text whole or on several lines: {message:?}");
        assert!(!message.contains("runbook.toml"), "{message}");
    }
    assert_eq!(d.runs().len(), runs_before, "no run");
    d.stop();
}

#[test]
fn the_new_members_of_a_submit_pass_the_one_token_check_and_origin_guard() {
    const TOKEN: &str = "guard-token-0123456789-abcdefghijklmnopqrstuv";
    let s = Server::start("auto-guard", Some(TOKEN));
    let body = json!({"runbook_id": "quick", "intake": "x", "confinement": "linux", "autonomy": "A5", "decider": "rule"});
    let tool = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "submit_run", "arguments": body}});
    let json_header = ("Content-Type", "application/json");
    let bearer = format!("Bearer {TOKEN}");
    let routes: [(&str, &Value); 2] = [("/daemon/commands/submit-run", &body), ("/mcp", &tool)];
    for (path, payload) in routes {
        let bytes = payload.to_string();
        let none = s.send("POST", path, &[json_header], Some(bytes.as_bytes()));
        assert_eq!(none.status, 401, "{path} without the token");
        let wrong = s.send("POST", path, &[json_header, ("Authorization", "Bearer not-the-token-0123456789-abcdefghijklmnop")], Some(bytes.as_bytes()));
        assert_eq!(wrong.status, 401, "{path} with another token");
        let foreign = s.send("POST", path, &[json_header, ("Authorization", &bearer), ("Origin", "http://evil.example")], Some(bytes.as_bytes()));
        assert_eq!(foreign.status, 403, "{path} from a foreign origin with the token");
    }
    assert!(s.fx.d().runs().is_empty(), "a refused request wrote no run");
    let ok = s.send("POST", "/daemon/commands/submit-run", &[json_header, ("Authorization", &bearer)], Some(body.to_string().as_bytes()));
    assert_eq!(ok.status, 400, "with the token a kind nobody configured is refused: {}", ok.text());
    assert!(s.fx.d().runs().is_empty());
}
