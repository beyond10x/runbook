//! The adversary's cases against the reads of a run's products and its tree (stories `mcp-reads-a-run-product` and
//! `daemon-page-shows-a-run-tree-and-products`): links and swaps under `work/`, a file far over the bound, names a
//! terminal or a page could misread, a pipe in the middle of a path, a large listing, and readers without the token.

use super::http::{Resp, Server};
use serde_json::json;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const TOKEN: &str = "products-attack-0123456789-abcdefghijk";

fn put(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn enc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

fn served(tag: &str, token: Option<&str>) -> (Server, String, PathBuf) {
    let s = Server::start(tag, token);
    let run = s.fx.submit("quick");
    s.wait_state(&run, "Ended");
    let w = s.fx.dir.join("state").join("workspaces").join(&run);
    (s, run, w)
}

fn read(s: &Server, run: &str, path: &str) -> Resp {
    s.get(&format!("/daemon/views/product?run_id={}&path={}", enc(run), enc(path)))
}

fn list(s: &Server, run: &str) -> Resp {
    s.get(&format!("/daemon/views/products?run_id={}", enc(run)))
}

#[test]
fn a_work_directory_that_is_itself_a_link_is_neither_listed_nor_read() {
    let (s, run, w) = served("attack-work-link", None);
    let outside = s.fx.dir.join("outside");
    put(&outside.join("secret.txt"), b"the secret\n");
    std::fs::remove_dir_all(&w).unwrap();
    symlink(&outside, &w).unwrap();
    let l = list(&s, &run);
    assert_eq!((l.status, l.json()["refused"].clone()), (403, json!("link")), "{}", l.text());
    let r = read(&s, &run, "secret.txt");
    assert_eq!((r.status, r.json()["refused"].clone()), (403, json!("link")), "{}", r.text());
    for t in [l.text(), r.text()] {
        assert!(!t.contains("the secret") && !t.contains(s.fx.dir.to_str().unwrap()), "{t}");
    }
}

#[test]
fn a_file_swapped_for_a_link_while_it_is_read_never_gives_the_target() {
    let (s, run, w) = served("attack-swap", None);
    let outside = s.fx.dir.join("secret.txt");
    put(&outside, b"the secret\n");
    put(&w.join("a.txt"), b"plain\n");
    let stop = Arc::new(AtomicBool::new(false));
    let swapper = {
        let (stop, w, outside) = (stop.clone(), w.clone(), outside.clone());
        std::thread::spawn(move || {
            let (file, link, tmp) = (w.join("a.txt"), w.join("a.link"), w.join("a.tmp"));
            while !stop.load(Ordering::Relaxed) {
                let _ = std::fs::remove_file(&link);
                let _ = symlink(&outside, &link);
                let _ = std::fs::rename(&link, &file);
                let _ = std::fs::write(&tmp, b"plain\n");
                let _ = std::fs::rename(&tmp, &file);
            }
        })
    };
    let (mut texts, mut links) = (0, 0);
    let until = Instant::now() + Duration::from_secs(2);
    while Instant::now() < until {
        let r = read(&s, &run, "a.txt");
        assert!(!r.text().contains("the secret"), "a swap gave the target of a link: {}", r.text());
        match r.status {
            200 => texts += 1,
            403 => links += 1,
            404 => {}
            other => panic!("{other}: {}", r.text()),
        }
    }
    stop.store(true, Ordering::Relaxed);
    swapper.join().unwrap();
    assert!(texts > 0 && links > 0, "the swap was seen both ways: {texts} texts, {links} links");
}

#[test]
fn a_sparse_file_of_eight_gibibytes_is_read_to_the_bound_at_once() {
    let (s, run, w) = served("attack-sparse", None);
    let f = std::fs::File::create(w.join("huge.bin")).unwrap();
    f.set_len(8 << 30).unwrap();
    drop(f);
    let t = Instant::now();
    let r = read(&s, &run, "huge.bin");
    assert!(t.elapsed() < Duration::from_secs(5), "the read took {:?}", t.elapsed());
    assert_eq!(r.status, 200, "{}", r.text());
    let row = r.json()["rows"][0].clone();
    // eight GiB of NUL bytes: not text, the size said, nothing shown
    assert_eq!((row["kind"].clone(), row["size"].clone(), row["truncated"].clone(), row["shown"].clone()), (json!("binary"), json!(8u64 << 30), json!(true), json!(0)), "{row}");
    assert!(r.text().len() < 4096, "the answer carried the file: {} bytes", r.text().len());
}

#[test]
fn names_with_a_newline_an_escape_and_a_direction_override_are_listed_and_read_back_by_the_same_path() {
    let (s, run, w) = served("attack-names", None);
    let names = ["line\nbreak.txt", "esc\u{1b}[31mred.txt", "rtl\u{202e}txt.exe", "tab\there.txt", &"n".repeat(250)];
    for n in names {
        put(&w.join(n), n.as_bytes());
    }
    let row = list(&s, &run).json()["rows"][0].clone();
    let mut listed: Vec<String> = row["files"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap().to_string()).collect();
    listed.sort();
    let mut want: Vec<String> = names.iter().map(|n| n.to_string()).collect();
    want.sort();
    assert_eq!(listed, want);
    for n in names {
        let r = read(&s, &run, n);
        assert_eq!((r.status, r.json()["rows"][0]["text"].clone()), (200, json!(n)), "{n:?}: {}", r.text());
    }
}

#[test]
fn a_refusal_repeats_at_most_three_hundred_characters_of_a_path_or_a_run_id() {
    let (s, run, _w) = served("attack-long", None);
    let long = format!("{}/x.txt", "d".repeat(5000));
    let r = read(&s, &run, &long);
    assert_eq!(r.status, 404, "{}", r.text());
    assert!(r.json()["message"].as_str().unwrap().len() < 500, "{}", r.text().len());
    let id = "r".repeat(5000);
    for view in ["products", "cases"] {
        let r = s.get(&format!("/daemon/views/{view}?run_id={id}"));
        assert_eq!(r.status, 404, "{view}");
        assert!(r.json()["message"].as_str().unwrap().len() < 500, "{view}: {}", r.text().len());
    }
}

#[test]
fn a_pipe_in_the_middle_of_a_path_is_never_waited_for() {
    let (s, run, w) = served("attack-mid-pipe", None);
    let fifo = std::ffi::CString::new(w.join("pipe").to_str().unwrap()).unwrap();
    // SAFETY: a NUL-terminated path that outlives the call.
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o644) }, 0);
    let t = Instant::now();
    let r = read(&s, &run, "pipe/x.txt");
    assert!(t.elapsed() < Duration::from_secs(5), "the read waited on a pipe");
    assert!(matches!(r.status, 403 | 404 | 415), "{}: {}", r.status, r.text());
}

#[test]
fn a_run_with_six_hundred_files_lists_five_hundred_and_says_it_cut() {
    let (s, run, w) = served("attack-many", None);
    for i in 0..600 {
        put(&w.join(format!("f{i:04}.txt")), b"x");
    }
    let t = Instant::now();
    let row = list(&s, &run).json()["rows"][0].clone();
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
    assert_eq!((row["files"].as_array().unwrap().len(), row["cut"].clone()), (500, json!("files")), "{}", row["cut"]);
}

#[test]
fn a_cookie_of_another_daemon_opens_none_of_the_three_reads() {
    let (s, run, w) = served("attack-cookie", Some(TOKEN));
    put(&w.join("a.txt"), b"the content\n");
    // the cookie value of a daemon whose token is another (docs/daemon-api.md, "The web UI": SHA-256 of
    // `runbook-ui-read/1`, NUL, token)
    use sha2::Digest;
    let mut h = sha2::Sha256::new();
    h.update(b"runbook-ui-read/1\0another-token-0123456789-abcdefghij");
    let cookie = format!("runbook_ui={}", h.finalize().iter().map(|b| format!("{b:02x}")).collect::<String>());
    for p in [format!("/daemon/views/products?run_id={run}"), format!("/daemon/views/product?run_id={run}&path=a.txt"), format!("/daemon/views/cases?run_id={run}")] {
        let r = s.get_with(&p, &[("Cookie", &cookie)]);
        assert_eq!(r.status, 401, "{p}: {}", r.text());
        assert!(!r.text().contains("the content"), "{p}");
    }
}

