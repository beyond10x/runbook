//! The view `runbooks` (`GET /daemon/views/runbooks`): the runbooks the daemon can start, by name and
//! version, from the runtime's own table or the runbook home and from `--runbook-dir`. It sits behind the
//! daemon's one token check, and it names runbooks only: no path of the host is in an answer.

use super::http::send;
use super::openapi;
use super::state::scratch;
use runbook::api;
use runbook::commands::{Config, Daemon};
use runbook::engine::Engine;
use runbook::runtime::{Confinement, Runtime};
use runbook::stub::{Script, Step, StubRuntime};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const TOKEN: &str = "runbooks-view-0123456789-abcdefghijklmn";

struct Live {
    addr: SocketAddr,
    dir: PathBuf,
    _rt: tokio::runtime::Runtime,
    _daemon: Daemon,
}

fn live(tag: &str, runtime: Arc<dyn Runtime>, runbook_dir: Option<PathBuf>, token: Option<&str>, dir: PathBuf) -> Live {
    let cfg = Config { state_dir: dir.join("state"), confinement_floor: Confinement::Linux, context_roots: vec![], runbook_dir, principal: super::PRINCIPAL.into() };
    let daemon = Daemon::start(cfg, runtime).unwrap_or_else(|e| panic!("{tag}: {e}"));
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let listener = rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
    let addr = listener.local_addr().unwrap();
    let (served, token) = (daemon.clone(), token.map(String::from));
    rt.spawn(async move {
        let _ = api::serve(listener, served, token.as_deref(), std::future::pending()).await;
    });
    Live { addr, dir, _rt: rt, _daemon: daemon }
}

impl Live {
    fn rows(&self) -> Vec<Value> {
        let r = send(self.addr, "GET", "/daemon/views/runbooks", &[("Authorization", &format!("Bearer {TOKEN}"))], None);
        assert_eq!(r.status, 200, "{}", r.text());
        let body = r.json();
        assert!(!r.text().contains(self.dir.to_str().unwrap()), "no path of the host in the answer: {}", r.text());
        body["rows"].as_array().unwrap().clone()
    }
}

fn names(rows: &[Value]) -> Vec<(String, Option<String>)> {
    rows.iter().map(|r| (r["runbook_id"].as_str().unwrap().to_string(), r["runbook_version"].as_str().map(String::from))).collect()
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

#[test]
fn the_stub_runtime_lists_its_scripts_and_the_runbooks_of_the_runbook_dir() {
    let dir = scratch("runbooks-stub");
    let stub = Arc::new(StubRuntime::new());
    stub.script("example", Script::new(vec![Step::End("done".into())]).version("2"));
    stub.script("scripted", Script::new(vec![Step::End("done".into())]));
    let books = dir.join("books");
    // a directory is a runbook, and so is `<name>.protocol.yaml`
    write(&books.join("from-dir/protocol.yaml"), "x: 1\n");
    write(&books.join("from-file.protocol.yaml"), "x: 1\n");
    // none of these is a runbook the daemon would accept in a submit
    write(&books.join("notes.txt"), "text\n");
    write(&books.join("plain-file"), "text\n");
    write(&books.join(".hidden/protocol.yaml"), "x: 1\n");
    write(&books.join(".dotted.protocol.yaml"), "x: 1\n");
    write(&books.join("with space/protocol.yaml"), "x: 1\n");
    // a name the script table has already is listed once
    write(&books.join("scripted/protocol.yaml"), "x: 1\n");
    let s = live("stub", stub, Some(books), None, dir);
    let rows = s.rows();
    rows.iter().for_each(|r| openapi::check_row("Runbooks", r));
    assert_eq!(
        names(&rows),
        [("example", Some("2")), ("from-dir", None), ("from-file", None), ("scripted", None)].map(|(n, v)| (n.to_string(), v.map(String::from))),
        "{rows:?}"
    );
    assert!(rows.iter().all(|r| r["runtime"] == json!("stub")), "{rows:?}");
}

#[test]
fn a_runbook_home_lists_the_runbooks_that_have_a_protocol_and_bindings() {
    let dir = scratch("runbooks-loom");
    let home = dir.join("home");
    write(&home.join("protocol/alpha.protocol.yaml"), "format: protocol/1\nprotocol:\n  id: alpha\n  revision: 3\n");
    write(&home.join("bindings/alpha.yaml"), "format: loom.bindings/1\n");
    write(&home.join("protocol/beta.protocol.yaml"), "format: protocol/1\nprotocol:\n  id: beta\n  revision: 1\n");
    write(&home.join("protocol/notes.txt"), "text\n");
    write(&home.join("bindings/gamma.yaml"), "format: loom.bindings/1\n");
    let engine = Arc::new(Engine::new(home.clone(), dir.join("state")));
    let s = live("loom", engine, None, None, dir);
    let rows = s.rows();
    rows.iter().for_each(|r| openapi::check_row("Runbooks", r));
    assert_eq!(names(&rows), [("alpha".to_string(), Some("3".to_string()))], "{rows:?}");
    assert_eq!(rows[0]["runtime"], json!("loom"));
    assert!(!rows[0].to_string().contains("protocol/"), "{rows:?}");
}

#[test]
fn the_runbook_directories_of_the_file_are_listed_before_the_runbook_home() {
    let dir = scratch("runbooks-dirs");
    let home = dir.join("home");
    write(&home.join("protocol/alpha.protocol.yaml"), "format: protocol/1\nprotocol:\n  id: alpha\n  revision: 3\n");
    write(&home.join("bindings/alpha.yaml"), "format: loom.bindings/1\n");
    let books = dir.join("books");
    write(&books.join("protocol/delta.protocol.yaml"), "format: protocol/1\nprotocol:\n  id: delta\n  revision: 7\n");
    write(&books.join("bindings/delta.yaml"), "format: loom.bindings/1\n");
    let engine = Arc::new(Engine::new(home, dir.join("state")).with_runbook_dirs(vec![books]));
    let s = live("dirs", engine, None, None, dir);
    let rows = s.rows();
    rows.iter().for_each(|r| openapi::check_row("Runbooks", r));
    assert_eq!(names(&rows), [("alpha".to_string(), Some("3".to_string())), ("delta".to_string(), Some("7".to_string()))], "{rows:?}");
}

#[test]
fn every_runbook_the_view_lists_is_one_a_submit_accepts() {
    let dir = scratch("runbooks-submit");
    let stub = Arc::new(StubRuntime::new());
    stub.script("example", Script::new(vec![Step::End("done".into())]));
    write(&dir.join("books/from-dir/protocol.yaml"), "x: 1\n");
    let s = live("submit", stub, Some(dir.join("books")), None, dir);
    let rows = s.rows();
    assert_eq!(rows.len(), 2, "{rows:?}");
    for r in rows {
        let body = json!({"runbook_id": r["runbook_id"], "intake": "text", "confinement": "linux"}).to_string();
        let got = send(s.addr, "POST", "/daemon/commands/submit-run", &[("Content-Type", "application/json")], Some(body.as_bytes()));
        assert_ne!(got.json()["outcome"], json!("no-runbook"), "{r}: {}", got.text());
    }
}

#[test]
fn the_view_is_behind_the_token_check_of_the_daemon() {
    let dir = scratch("runbooks-token");
    let stub = Arc::new(StubRuntime::new());
    stub.script("example", Script::new(vec![Step::End("done".into())]));
    let s = live("token", stub, None, Some(TOKEN), dir);
    assert_eq!(send(s.addr, "GET", "/daemon/views/runbooks", &[], None).status, 401);
    assert_eq!(send(s.addr, "GET", "/daemon/views/runbooks", &[("Authorization", "Bearer wrong")], None).status, 401);
    assert_eq!(names(&s.rows()), [("example".to_string(), None)]);
    assert_eq!(send(s.addr, "POST", "/daemon/views/runbooks", &[("Authorization", &format!("Bearer {TOKEN}"))], Some(b"{}")).status, 405);
}
