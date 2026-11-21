//! Test double for `sdc-conform` (driver/tests/conformance.rs), two roles in one binary.
//!
//! Product: `conformance-stub-product <port> <data dir> [ok|deviate|nostart|failbuild|token|token-silent]`
//! serves the `chores` fixture specification (driver/tests/fixtures/conformance/spec) on 127.0.0.1
//! over HTTP: `POST /chores {"title"}` 201, `POST /chores/<id>/finish [{"board":"down"}]` 200, 503
//! or 409 `{"error","state"}`, `GET /boards/<code>` 200 `{"code"}`, `GET /chores/open`,
//! `GET /chores/done`, `GET /health` (a query string is ignored). `deviate` answers an unknown chore 404 instead of 409 (one scenario of the suite
//! fails); `nostart` exits at once; `failbuild` exits 1 (a build step that fails). `token` mints a
//! token at start and prints `board token: <token>` on stdout; every route but `/health` then needs
//! the header `x-token: <token>` (else 401) and `/boards/<code>` knows only the token (else 404).
//! `token-silent` does the same without printing the token.
//!
//! Table node: with `SDC_STUB_TABLE_NODE=<slot>[;<slot>...]`, each slot `<table.json>[,<table.json>...]`,
//! called as an LLM adapter with one argument (the request file), it copies the table for the
//! request's `slot` and `attempt` (the last slot and the last table for later ones) to
//! `<record_dir>/structured-output.json` and prints a response record. `-` writes no table.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use serde_json::{json, Value as J};

fn table_node(req_path: &str, tables: &str) {
    let req: J = serde_json::from_str(&std::fs::read_to_string(req_path).unwrap()).unwrap();
    let slots: Vec<&str> = tables.split(';').collect();
    let slot = req["slot"].as_u64().unwrap_or(1) as usize;
    let files: Vec<&str> = slots[(slot.max(1) - 1).min(slots.len() - 1)].split(',').collect();
    let attempt = req["attempt"].as_u64().unwrap_or(1) as usize;
    let file = files[(attempt.max(1) - 1).min(files.len() - 1)];
    let rec = std::path::PathBuf::from(req["record_dir"].as_str().unwrap());
    if !file.is_empty() && file != "-" {
        std::fs::copy(file, rec.join("structured-output.json")).unwrap();
    }
    println!("{}", json!({"adapter": "stub", "model": req["model"], "effort": req["effort"], "exit": 0, "cost_usd": 0.0}));
}

struct Chore {
    title: String,
    done: bool,
}

fn respond(st: &mut TcpStream, status: u16, body: &J) {
    let b = body.to_string();
    let _ = write!(st, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len());
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Ok(tables) = std::env::var("SDC_STUB_TABLE_NODE") {
        table_node(&args[0], &tables);
        return;
    }
    let mode = args.get(2).map(String::as_str).unwrap_or("ok");
    match mode {
        "failbuild" => {
            eprintln!("error: could not compile `stub`");
            std::process::exit(1);
        }
        "nostart" => {
            eprintln!("stub: refusing to start");
            std::process::exit(3);
        }
        _ => {}
    }
    let port: u16 = args[0].parse().unwrap();
    let l = TcpListener::bind(("127.0.0.1", port)).unwrap();
    // `token`: a secret minted at start, printed on stdout, required as the header `x-token` and as
    // the board code; `token-silent` mints it and prints nothing.
    let token = mode.starts_with("token").then(|| format!("tok{port}x{}", std::process::id()));
    if let (Some(t), "token") = (&token, mode) {
        println!("listening on 127.0.0.1:{port}");
        println!("board token: {t}");
        let _ = std::io::stdout().flush();
    }
    let mut chores: BTreeMap<String, Chore> = BTreeMap::new();
    let mut n = 0u64;
    for st in l.incoming() {
        let Ok(mut st) = st else { continue };
        let mut buf = vec![];
        let mut chunk = [0u8; 4096];
        let head_end = loop {
            if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break p;
            }
            match st.read(&mut chunk) {
                Ok(0) | Err(_) => break usize::MAX,
                Ok(k) => buf.extend_from_slice(&chunk[..k]),
            }
        };
        if head_end == usize::MAX {
            continue;
        }
        let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
        let len: usize = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0))).unwrap_or(0);
        while buf.len() < head_end + 4 + len {
            match st.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(k) => buf.extend_from_slice(&chunk[..k]),
            }
        }
        let body: J = serde_json::from_slice(&buf[head_end + 4..]).unwrap_or(J::Null);
        let mut first = head.lines().next().unwrap_or("").split_whitespace();
        let (method, path) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
        let parts: Vec<&str> = path.split('?').next().unwrap_or("").trim_matches('/').split('/').collect();
        let header = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("x-token:").map(|v| v.trim().to_string()));
        if let Some(t) = &token {
            if parts != ["health"] && header.as_deref() != Some(t.to_ascii_lowercase().as_str()) {
                respond(&mut st, 401, &json!({"error": "x-token"}));
                continue;
            }
        }
        match (method, parts.as_slice()) {
            ("GET", ["health"]) => respond(&mut st, 200, &json!({"ok": true})),
            ("GET", ["boards", code]) => match &token {
                Some(t) if t != code => respond(&mut st, 404, &json!({"error": "no such board"})),
                _ => respond(&mut st, 200, &json!({"code": code})),
            },
            ("POST", ["chores"]) => match body["title"].as_str() {
                Some(t) => {
                    n += 1;
                    let id = format!("00000000-0000-4000-8000-{n:012x}");
                    chores.insert(id.clone(), Chore { title: t.to_string(), done: false });
                    respond(&mut st, 201, &json!({"chore_id": id, "title": t, "state": "Open"}));
                }
                None => respond(&mut st, 400, &json!({"error": "title"})),
            },
            ("GET", ["chores", "open"]) => {
                let rows: Vec<J> = chores.iter().filter(|(_, c)| !c.done).map(|(k, c)| json!({"chore_id": k, "title": c.title})).collect();
                respond(&mut st, 200, &J::Array(rows));
            }
            ("GET", ["chores", "done"]) => {
                let rows: Vec<J> = chores.iter().filter(|(_, c)| c.done).map(|(k, c)| json!({"chore_id": k, "title": c.title})).collect();
                respond(&mut st, 200, &J::Array(rows));
            }
            ("POST", ["chores", id, "finish"]) => {
                let down = body["board"] == "down";
                match chores.get_mut(*id) {
                    _ if down => respond(&mut st, 503, &json!({"error": "board unreachable"})),
                    None if mode == "deviate" => respond(&mut st, 404, &json!({"error": "no such chore", "state": "Open"})),
                    None => respond(&mut st, 409, &json!({"error": "no such chore", "state": "Open"})),
                    Some(c) if c.done => respond(&mut st, 409, &json!({"error": "already done", "state": "Done"})),
                    Some(c) => {
                        c.done = true;
                        respond(&mut st, 200, &json!({"chore_id": id, "state": "Done"}));
                    }
                }
            }
            _ => respond(&mut st, 404, &json!({"error": "not found"})),
        }
    }
}
