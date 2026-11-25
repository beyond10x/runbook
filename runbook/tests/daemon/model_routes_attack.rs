//! The adversary's cases against the view `routes` (story `daemon-declares-model-routes`, `docs/model-routes.md`,
//! sections 1 and 6): a gateway that answers one byte at a time, and a gateway whose status routes stall while many
//! clients ask for the view at once.
//!
//! Every gateway is a stand-in on a loopback port of the test's own: no model call, no network beyond loopback.

use super::default_confinement::live;
use super::http::try_send;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

/// Reads a request head, at most 64 KiB.
fn read_head(stream: &mut TcpStream) -> String {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") && head.len() < 64 * 1024 {
        match stream.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            _ => break,
        }
    }
    String::from_utf8_lossy(&head).into_owned()
}

fn answer(stream: &mut TcpStream, body: &str) {
    let _ = stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
}

/// A gateway that answers every request with a status line and then one byte of a header every half second, for
/// `seconds`: each read of the daemon gets a byte long before its read timeout, and the answer is never whole in
/// that time.
fn dripping(seconds: u64) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                read_head(&mut stream);
                if stream.write_all(b"HTTP/1.1 200 OK\r\nX-Drip: ").is_err() {
                    return;
                }
                for _ in 0..seconds * 2 {
                    std::thread::sleep(Duration::from_millis(500));
                    if stream.write_all(b"a").is_err() {
                        return;
                    }
                }
                let _ = stream.write_all(b"\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}");
            });
        }
    });
    base_url
}

/// A gateway that answers `/ready` and `/v1/models` (listing `aliases` on the wire chat) at once and holds every
/// other request open without a byte for `seconds`.
fn stalling(aliases: &[&str], seconds: u64) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let models = serde_json::json!({"object": "list", "data": aliases.iter().map(|a| serde_json::json!({"id": a, "wires": ["chat"]})).collect::<Vec<_>>()}).to_string();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let models = models.clone();
            std::thread::spawn(move || {
                let head = read_head(&mut stream);
                let line = head.lines().next().unwrap_or("").to_owned();
                if line.starts_with("GET /ready ") {
                    answer(&mut stream, "{\"status\":\"ready\"}");
                } else if line.starts_with("GET /v1/models ") {
                    answer(&mut stream, &models);
                } else {
                    std::thread::sleep(Duration::from_secs(seconds));
                }
            });
        }
    });
    base_url
}

#[test]
fn a_gateway_that_drips_its_answer_one_byte_at_a_time_does_not_hold_the_view_past_its_read_bound() {
    // each GET of a gateway ends within 3 s, its whole answer included (`routes::GET_BOUND`), and a read of the view
    // within 5 s (`routes::READ_BOUND`); before, the 3 s bound held each socket read, and a gateway that
    // sends a byte every half second never trips a per-read timeout and holds the view for as long as it drips
    let base_url = dripping(45);
    let file = format!("[gateways.drip]\nbase_url = \"{base_url}\"\n[routes.r]\ngateway = \"drip\"\nmodel = \"m\"\nkind = \"hosted\"\n");
    let l = live("routes-attack-drip", &[], Some(&file));
    let started = Instant::now();
    let got = try_send(l.addr, "GET", "/daemon/views/routes", &[], None);
    let took = started.elapsed();
    let resp = got.unwrap_or_else(|e| panic!("the view routes gave no answer in {took:?} while one gateway drips its answer: {e}"));
    assert!(took < runbook::routes::READ_BOUND, "the view routes took {took:?} for one gateway that drips its answer");
    assert_eq!(resp.status, 200, "{}", resp.text());
    assert_eq!(resp.json()["rows"][0]["status"], "gateway-unreachable", "{}", resp.text());
}

#[test]
fn a_gateway_whose_status_routes_stall_does_not_hold_the_other_views_of_the_daemon_while_clients_ask_for_routes() {
    // each status read stalls for its full 3 s bound: /v1/status, then the status route of each of three managed
    // routes, about 12 s for one read of the view, every read inside its bound
    let base_url = stalling(&["m1", "m2", "m3"], 30);
    let file = format!(
        "[gateways.slow]\nbase_url = \"{base_url}\"\n[routes.m1]\ngateway = \"slow\"\nmodel = \"m1\"\nkind = \"managed\"\n[routes.m2]\ngateway = \"slow\"\nmodel = \"m2\"\nkind = \"managed\"\n[routes.m3]\ngateway = \"slow\"\nmodel = \"m3\"\nkind = \"managed\"\n"
    );
    let l = live("routes-attack-stall", &[], Some(&file));
    let addr = l.addr;
    // a page and some MCP clients asking for the routes at once
    let askers: Vec<_> = (0..96).map(|_| std::thread::spawn(move || try_send(addr, "GET", "/daemon/views/routes", &[], None).map(|r| r.status))).collect();
    std::thread::sleep(Duration::from_secs(1));
    let started = Instant::now();
    let got = try_send(addr, "GET", "/daemon/views/sessions", &[], None);
    let took = started.elapsed();
    let resp = got.unwrap_or_else(|e| panic!("the view sessions gave no answer in {took:?} while clients wait for the view routes of a slow gateway: {e}"));
    assert_eq!(resp.status, 200, "{}", resp.text());
    assert!(took < Duration::from_secs(3), "the view sessions took {took:?} while clients wait for the view routes of a slow gateway");
    drop(askers);
}
