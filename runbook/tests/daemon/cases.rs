//! The case tree of a run over HTTP (`story:daemon-page-shows-a-run-tree-and-products`): the view `cases` is a read
//! behind the daemon's one guard layer. The stub runtime has no tree, so a stub run answers no case; the Loom runtime's
//! tree is held by `daemon::engine` (`the_cases_of_a_run_are_its_tree_...`).

use super::http::Server;
use serde_json::json;

const TOKEN: &str = "cases-view-0123456789-abcdefghijklmnopq";

#[test]
fn a_run_of_the_stub_runtime_has_no_case_and_an_unknown_run_is_refused() {
    let s = Server::start("cases-stub", None);
    let run = s.fx.submit("quick");
    s.wait_state(&run, "Ended");
    let r = s.get(&format!("/daemon/views/cases?run_id={run}"));
    assert_eq!((r.status, r.json()), (200, json!({"rows": []})), "{}", r.text());
    let r = s.get("/daemon/views/cases?run_id=no-such-run-0001");
    assert_eq!((r.status, r.json()["refused"].clone()), (404, json!("no such run")), "{}", r.text());
    assert!(r.json()["message"].as_str().unwrap().contains("no-such-run-0001"), "{}", r.text());
    assert_eq!(s.get("/daemon/views/cases").status, 400);
}

#[test]
fn the_tree_answers_without_the_token_and_a_foreign_origin_as_every_other_read_does() {
    let s = Server::start("cases-guard", Some(TOKEN));
    let run = s.fx.submit("quick");
    let bearer = format!("Bearer {TOKEN}");
    let path = format!("/daemon/views/cases?run_id={run}");
    let other = format!("/daemon/views/run?run_id={run}");
    let no_token = s.get(&other);
    assert_eq!(no_token.status, 401);
    let origin = s.get_with(&other, &[("Authorization", &bearer), ("Origin", "http://evil.invalid")]);
    assert_eq!(origin.status, 403);
    let r = s.get(&path);
    assert_eq!((r.status, r.text()), (no_token.status, no_token.text()), "without the token");
    let r = s.get_with(&path, &[("Authorization", "Bearer wrong-token-0123456789-abcdefghijk")]);
    assert_eq!((r.status, r.text()), (no_token.status, no_token.text()), "with a wrong token");
    let r = s.get_with(&path, &[("Authorization", &bearer), ("Origin", "http://evil.invalid")]);
    assert_eq!((r.status, r.text()), (origin.status, origin.text()), "from a foreign origin");
    assert_eq!(s.get_with(&path, &[("Authorization", &bearer)]).status, 200, "with the token");
}
