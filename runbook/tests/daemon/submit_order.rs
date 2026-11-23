//! The order of the outcomes of `SubmitRun` (`story:submitrun-outcome-order`) against the handler and
//! the table, the floor as a guard in every session state, the token before all of it.

use super::commands::Fx;
use super::http::{send, Server};
use runbook::api;
use runbook::commands::{CloseSession, Daemon, OpenSession, Outcome, SubmitRun, MAX_INTAKE_BYTES, OUTCOMES};
use runbook::mcp;
use runbook::runtime::Confinement;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

const TOKEN: &str = "adv-token-0123456789-abcdefghijklmnopqrstu";

fn declared_submit_order() -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ess/domains/daemon.yaml");
    let doc: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    for c in doc["commands"].as_sequence().unwrap() {
        if c["naming"]["wire"].as_str() == Some("submit-run") {
            return c["outcomes"].as_sequence().unwrap().iter().map(|o| o["name"].as_str().unwrap().to_string()).collect();
        }
    }
    panic!("no submit-run in the domain file");
}

/// Everything the daemon stores for a refused submit to change: the journal, the run and workspace
/// directories, the runs, the daemon's events.
fn stored(fx: &Fx) -> (Vec<u8>, Vec<String>, String, String) {
    let state = fx.dir.join("state");
    let journal = std::fs::read(state.join("journal.jsonl")).unwrap_or_default();
    let mut tree = vec![];
    for sub in ["runs", "workspaces"] {
        if let Ok(rd) = std::fs::read_dir(state.join(sub)) {
            for e in rd {
                tree.push(format!("{sub}/{}", e.unwrap().file_name().to_string_lossy()));
            }
        }
    }
    tree.sort();
    (journal, tree, serde_json::to_string(&fx.d().runs()).unwrap(), serde_json::to_string(&fx.d().events_of_daemon_after(0)).unwrap())
}

fn base() -> SubmitRun {
    SubmitRun { runbook_id: "quick".into(), intake: "x".into(), ..SubmitRun::default() }
}

fn open(fx: &Fx, id: &str) {
    assert_eq!(fx.d().open_session(OpenSession { session_id: id.into(), client: "adv".into() }).unwrap().name, "opened");
}

fn close(fx: &Fx, id: &str) {
    assert_eq!(fx.d().close_session(CloseSession { session_id: id.into() }).unwrap().name, "closed");
}

/// The condition of each outcome of `SubmitRun`, by the name the domain file gives it: how to make a
/// request in which that outcome's condition holds (the daemon id it is sent to is the second member).
struct Setup {
    fx: Fx,
    inside: PathBuf,
}

impl Setup {
    fn new(tag: &str) -> Setup {
        let fx = Fx::new(tag);
        open(&fx, "open-s");
        open(&fx, "closed-s");
        close(&fx, "closed-s");
        let inside = fx.dir.join("ctx/sub");
        std::fs::create_dir_all(&inside).unwrap();
        // The key "used" belongs to a run that exists.
        let mut first = base();
        first.idempotency_key = Some("used".into());
        fx.submit_with(first);
        Setup { fx, inside }
    }

    fn make(&self, outcome: &str, req: &mut SubmitRun, daemon: &mut String) {
        match outcome {
            "intake-too-large" => req.intake = "a".repeat(MAX_INTAKE_BYTES + 1),
            "no-runbook" => req.runbook_id = "nothing".into(),
            "context-outside-roots" => req.context_path = Some(self.fx.dir.to_string_lossy().into_owned()),
            "no-daemon" => *daemon = "another-daemon".into(),
            "no-session" => req.session_id = Some("none".into()),
            "below-floor" => req.confinement = Some(Confinement::Local),
            "session-closed" => req.session_id = Some("closed-s".into()),
            "already-submitted" => req.idempotency_key = Some("used".into()),
            "submitted" => {}
            other => panic!("the domain file declares {other} and this test does not know it"),
        }
    }

    fn send(&self, daemon: &str, req: SubmitRun) -> Outcome {
        self.fx.d().submit_run_for(daemon, req).unwrap()
    }
}

fn daemon_id(s: &Setup) -> String {
    s.fx.d().daemon_id().to_string()
}

#[test]
fn every_pair_of_conditions_of_submit_run_answers_the_earlier_declared_outcome() {
    let s = Setup::new("adv-order");
    let own = daemon_id(&s);
    let order = declared_submit_order();
    assert_eq!(
        order,
        ["intake-too-large", "no-runbook", "context-outside-roots", "no-daemon", "no-session", "below-floor", "session-closed", "already-submitted", "submitted"]
    );
    // alone
    let mut report = vec![];
    let mut differs = vec![];
    for name in &order {
        let (mut req, mut daemon) = (base(), own.clone());
        s.make(name, &mut req, &mut daemon);
        let o = s.send(&daemon, req);
        if o.name != name.as_str() {
            differs.push(format!("alone {name}: got {}", o.name));
        }
    }
    for (i, a) in order.iter().enumerate() {
        for b in &order[i + 1..] {
            // one session id cannot be missing and closed at once
            if (a == "no-session" && b == "session-closed") || b == "submitted" {
                continue;
            }
            let (mut req, mut daemon) = (base(), own.clone());
            s.make(a, &mut req, &mut daemon);
            s.make(b, &mut req, &mut daemon);
            let o = s.send(&daemon, req);
            let adjacent = order.iter().position(|x| x == b) == Some(i + 1);
            let verdict = if o.name == a.as_str() { "holds" } else { "differs" };
            report.push(format!("{}pair {a} > {b}: {verdict} (got {})", if adjacent { "ADJ " } else { "    " }, o.name));
            if o.name != a.as_str() {
                differs.push(format!("{a} > {b}: got {}", o.name));
            }
        }
    }
    // the adjacent pair whose second member is the accepting outcome: only the first holds, the rest is accepted
    let (mut req, mut daemon) = (base(), own.clone());
    s.make("already-submitted", &mut req, &mut daemon);
    let o = s.send(&daemon, req);
    report.push(format!("ADJ pair already-submitted > submitted: {} (got {})", if o.name == "already-submitted" { "holds" } else { "differs" }, o.name));
    for l in &report {
        println!("{l}");
    }
    assert!(differs.is_empty(), "{differs:#?}");
}

#[test]
fn no_request_below_the_floor_is_accepted_or_changes_what_the_daemon_stores() {
    let s = Setup::new("adv-floor");
    let own = daemon_id(&s);
    // control: the same request at the floor with an open session is accepted, so a refusal below is the floor's work
    let mut ok = base();
    ok.session_id = Some("open-s".into());
    assert_eq!(s.send(&own, ok).name, "submitted");
    // the runs of the setup end on their own threads: wait, so the journal is still while the refusals run
    for row in s.fx.d().runs() {
        s.fx.wait_state(row["run_id"].as_str().unwrap(), "Ended");
    }
    std::thread::sleep(std::time::Duration::from_millis(200));

    let sessions: [Option<&str>; 4] = [None, Some("none"), Some("open-s"), Some("closed-s")];
    let daemons = [own.clone(), "another-daemon".to_string()];
    let runbooks = ["quick", "nothing"];
    let intakes = [false, true];
    let contexts = [0, 1, 2];
    let keys: [Option<&str>; 3] = [None, Some("fresh"), Some("used")];
    let mut n = 0;
    let mut wrong = vec![];
    let before = stored(&s.fx);
    for session in sessions {
        for daemon in &daemons {
            for runbook in runbooks {
                for big in intakes {
                    for ctx in contexts {
                        for key in keys {
                            let mut req = base();
                            req.confinement = Some(Confinement::Local);
                            req.session_id = session.map(String::from);
                            req.runbook_id = runbook.into();
                            if big {
                                req.intake = "a".repeat(MAX_INTAKE_BYTES + 1);
                            }
                            req.context_path = match ctx {
                                0 => None,
                                1 => Some(s.inside.to_string_lossy().into_owned()),
                                _ => Some(s.fx.dir.to_string_lossy().into_owned()),
                            };
                            req.idempotency_key = key.map(String::from);
                            let want = if big {
                                "intake-too-large"
                            } else if runbook == "nothing" {
                                "no-runbook"
                            } else if ctx == 2 {
                                "context-outside-roots"
                            } else if daemon != &own {
                                "no-daemon"
                            } else if session == Some("none") {
                                "no-session"
                            } else {
                                "below-floor"
                            };
                            let label = format!("session {session:?} daemon {daemon} runbook {runbook} big {big} ctx {ctx} key {key:?}");
                            let o = s.send(daemon, req);
                            n += 1;
                            if o.name != want || !o.is_refusal() || o.run_id.is_some() || !o.published.is_empty() {
                                wrong.push(format!("{label}: got {} want {want}", o.name));
                            }
                            if stored(&s.fx) != before {
                                wrong.push(format!("{label}: stored state changed"));
                            }
                        }
                    }
                }
            }
        }
    }
    println!("{n} combinations below the floor");
    assert_eq!(n, 288);
    assert!(wrong.is_empty(), "{} of {n}: {:#?}", wrong.len(), &wrong[..wrong.len().min(10)]);
}

type Post<'a> = &'a dyn Fn(&Value, Option<&str>) -> (u16, String);

fn without_auth_is_one_answer(post: Post, probe: &dyn Fn() -> (Vec<u8>, usize)) {
    let before = probe();
    let missing_low = json!({"runbook_id": "quick", "intake": "x", "confinement": "local", "session_id": "none"});
    let open_low = json!({"runbook_id": "quick", "intake": "x", "confinement": "local", "session_id": "open-s"});
    let missing_ok = json!({"runbook_id": "quick", "intake": "x", "session_id": "none"});
    let mut answers = vec![];
    for auth in [None, Some("Bearer nope"), Some("Bearer ")] {
        for body in [&missing_low, &open_low, &missing_ok] {
            answers.push(post(body, auth));
        }
    }
    for a in &answers {
        assert_eq!(a.0, 401, "{a:?}");
        assert!(!a.1.contains("NoSuch") && !a.1.contains("Floor") && !a.1.contains("session") && !a.1.contains("floor"), "{a:?}");
    }
    assert!(answers.iter().all(|a| a == &answers[0]), "an unauthenticated caller tells the requests apart: {answers:#?}");
    assert_eq!(probe(), before, "a refused-by-token request changed the stored state");
}

#[test]
fn the_token_is_checked_before_the_order_on_the_http_route() {
    let s = Server::start("adv-http-token", Some(TOKEN));
    let auth = format!("Bearer {TOKEN}");
    s.fx.d().open_session(OpenSession { session_id: "open-s".into(), client: "adv".into() }).unwrap();
    let probe = || (std::fs::read(s.fx.dir.join("state/journal.jsonl")).unwrap(), s.fx.d().runs().len());
    let post = |body: &Value, a: Option<&str>| {
        let mut h = vec![("Content-Type", "application/json")];
        if let Some(a) = a {
            h.push(("Authorization", a));
        }
        let r = s.send("POST", "/daemon/commands/submit-run", &h, Some(body.to_string().as_bytes()));
        (r.status, r.text())
    };
    without_auth_is_one_answer(&post, &probe);
    // with the token the order is the declared one
    let h = [("Content-Type", "application/json"), ("Authorization", auth.as_str())];
    let go = |body: Value| {
        let r = s.send("POST", "/daemon/commands/submit-run", &h, Some(body.to_string().as_bytes()));
        (r.status, r.json()["error"].clone())
    };
    assert_eq!(go(json!({"runbook_id": "quick", "intake": "x", "confinement": "local", "session_id": "none"})), (409, json!("sdc.daemon.NoSuchSession")));
    assert_eq!(go(json!({"runbook_id": "quick", "intake": "x", "confinement": "local", "session_id": "open-s"})), (409, json!("sdc.daemon.BelowConfinementFloor")));
    // the sessions are in a view the same token reads: the probe tells an authenticated caller nothing new
    let r = s.send("GET", "/daemon/views/sessions", &[("Authorization", auth.as_str())], None);
    assert!(r.text().contains("open-s"));
}

#[test]
fn the_token_is_checked_before_the_order_on_the_mcp_tool() {
    let fx = Fx::new("adv-mcp-token");
    fx.d().open_session(OpenSession { session_id: "open-s".into(), client: "adv".into() }).unwrap();
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let listener = rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
    let addr = SocketAddr::from(([127, 0, 0, 1], listener.local_addr().unwrap().port()));
    let daemon: Daemon = fx.d().clone();
    rt.spawn(async move {
        let extra = mcp::router(daemon.clone());
        let _ = api::serve_extra(listener, daemon, Some(TOKEN), api::Streams::default(), Some(extra), std::future::pending()).await;
    });
    let probe = || (std::fs::read(fx.dir.join("state/journal.jsonl")).unwrap(), fx.d().runs().len());
    let call = |args: &Value, a: Option<&str>| {
        let body = json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "submit_run", "arguments": args}});
        let mut h = vec![("Content-Type", "application/json"), ("Accept", "application/json, text/event-stream"), ("MCP-Protocol-Version", "2025-11-25")];
        if let Some(a) = a {
            h.push(("Authorization", a));
        }
        send(addr, "POST", "/mcp", &h, Some(body.to_string().as_bytes()))
    };
    let post = |body: &Value, a: Option<&str>| {
        let r = call(body, a);
        (r.status, r.text())
    };
    without_auth_is_one_answer(&post, &probe);
    let auth = format!("Bearer {TOKEN}");
    let r = call(&json!({"runbook_id": "quick", "intake": "x", "confinement": "local", "session_id": "none"}), Some(&auth));
    assert_eq!(r.status, 200, "{}", r.text());
    let res = r.json()["result"].clone();
    assert_eq!((res["isError"].clone(), res["structuredContent"]["error"].clone()), (json!(true), json!("sdc.daemon.NoSuchSession")), "{res}");
    let r = call(&json!({"runbook_id": "quick", "intake": "x", "confinement": "local", "session_id": "open-s"}), Some(&auth));
    assert_eq!(r.json()["result"]["structuredContent"]["error"], json!("sdc.daemon.BelowConfinementFloor"));
    rt.shutdown_background();
}

#[test]
fn the_outcome_table_lists_the_submit_outcomes_in_the_declared_order() {
    let have: Vec<&str> = OUTCOMES.iter().filter(|s| s.command == "submit-run").map(|s| s.outcome).collect();
    assert_eq!(have, declared_submit_order(), "runbook/src/commands.rs OUTCOMES keeps the order of the domain file");
}
