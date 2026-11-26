//! The products of a run (`story:mcp-reads-a-run-product`): the views `products` and `product` list and serve the
//! files under the workspace of a run (its `work/`) through one reader (`runbook::engine::products`), with the
//! guards of the inspector's file route, behind the daemon's one guard layer.

use super::http::{Resp, Server};
use super::openapi;
use runbook::engine::products::{self, Bounds, LIMIT};
use serde_json::{json, Value};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const TOKEN: &str = "products-view-0123456789-abcdefghijklmn";

fn workspace(s: &Server, run: &str) -> PathBuf {
    s.fx.dir.join("state").join("workspaces").join(run)
}

fn put(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// A server with one ended stub run; its workspace is empty until a case writes into it.
fn served(tag: &str, token: Option<&str>) -> (Server, String, PathBuf) {
    let s = Server::start(tag, token);
    let run = s.fx.submit("quick");
    s.wait_state(&run, "Ended");
    let w = workspace(&s, &run);
    assert!(w.is_dir(), "the daemon made the workspace of the run");
    (s, run, w)
}

fn enc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

fn list(s: &Server, run: &str) -> Resp {
    s.get(&format!("/daemon/views/products?run_id={}", enc(run)))
}

fn read(s: &Server, run: &str, path: &str) -> Resp {
    s.get(&format!("/daemon/views/product?run_id={}&path={}", enc(run), enc(path)))
}

fn listing(s: &Server, run: &str) -> Value {
    let r = list(s, run);
    assert_eq!(r.status, 200, "{}", r.text());
    let rows = r.json()["rows"].as_array().unwrap().clone();
    assert_eq!(rows.len(), 1, "one row per run: {rows:?}");
    openapi::check_row("ProductsOfRun", &rows[0]);
    rows[0].clone()
}

fn paths(row: &Value) -> Vec<String> {
    row["files"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap().to_string()).collect()
}

fn one(s: &Server, run: &str, path: &str) -> Value {
    let r = read(s, run, path);
    assert_eq!(r.status, 200, "{path}: {}", r.text());
    let rows = r.json()["rows"].as_array().unwrap().clone();
    assert_eq!(rows.len(), 1, "{rows:?}");
    openapi::check_row("ProductOfRun", &rows[0]);
    rows[0].clone()
}

/// A refusal: its status, its `refused` word, and no byte of `secret` and no path of the host in it.
fn refused(r: &Resp, status: u16, what: &str, secret: &str, host: &Path) {
    assert_eq!(r.status, status, "{}", r.text());
    assert_eq!(r.json()["refused"], json!(what), "{}", r.text());
    assert!(!r.text().contains(secret), "the refusal holds content of a file: {}", r.text());
    assert!(!r.text().contains(host.to_str().unwrap()), "the refusal names a path of the host: {}", r.text());
}

#[test]
fn the_products_of_a_run_are_the_files_under_its_work_with_their_sizes() {
    let (s, run, w) = served("products-list", None);
    put(&w.join("idea.yaml"), b"text: x\n");
    put(&w.join("notes/a.md"), b"# a\n\nbody\n");
    put(&w.join("notes/deeper/b.txt"), b"result: valid\n");
    let row = listing(&s, &run);
    assert_eq!(row["run_id"], json!(run));
    assert_eq!(paths(&row), ["idea.yaml", "notes/a.md", "notes/deeper/b.txt"]);
    let sizes: Vec<u64> = row["files"].as_array().unwrap().iter().map(|f| f["size"].as_u64().unwrap()).collect();
    assert_eq!(sizes, [8, 10, 14]);
    assert!(row["files"][0]["modified_at"].as_str().is_some_and(|t| t.ends_with('Z')), "{row}");
    assert_eq!((row["cut"].clone(), row["hidden"].clone(), row["left_out"].clone()), (Value::Null, json!(0), json!(0)), "{row}");
    assert!(!list(&s, &run).text().contains(w.to_str().unwrap()), "no path of the host in the listing");
}

#[test]
fn a_product_is_read_as_text_up_to_the_bound_and_says_when_it_is_larger_or_not_text() {
    let (s, run, w) = served("products-read", None);
    put(&w.join("notes/a.md"), "# Ä title\n".as_bytes());
    let a = one(&s, &run, "notes/a.md");
    assert_eq!((a["path"].clone(), a["kind"].clone(), a["text"].clone(), a["size"].clone(), a["truncated"].clone()), (json!("notes/a.md"), json!("text"), json!("# Ä title\n"), json!(11), json!(false)), "{a}");
    assert_eq!(a["limit"], json!(LIMIT));
    // larger than the bound: the first LIMIT bytes, a partial character at the cut dropped, and it says so
    let mut big = vec![b'x'; LIMIT as usize - 1];
    big.extend_from_slice("é and more".as_bytes());
    put(&w.join("big.txt"), &big);
    let b = one(&s, &run, "big.txt");
    assert_eq!((b["kind"].clone(), b["truncated"].clone(), b["size"].clone()), (json!("text"), json!(true), json!(big.len())), "{}", b["size"]);
    assert_eq!(b["text"].as_str().unwrap().len(), LIMIT as usize - 1, "the cut character is dropped");
    assert_eq!(b["shown"], json!(LIMIT - 1));
    // not text: a NUL byte, or bytes that are not UTF-8
    for (name, bytes) in [("bin.dat", &b"ab\0cd"[..]), ("latin.txt", &b"caf\xe9\n"[..])] {
        put(&w.join(name), bytes);
        let r = one(&s, &run, name);
        assert_eq!((r["kind"].clone(), r.get("text").cloned()), (json!("binary"), None), "{name}: {r}");
        assert_eq!(r["size"], json!(bytes.len()));
    }
}

#[test]
fn a_path_that_leaves_the_work_directory_is_refused() {
    let (s, run, w) = served("products-leave", None);
    put(&w.join("a.txt"), b"a\n");
    let state = s.fx.dir.join("state");
    for p in ["../../journal.jsonl", "a/../../x", "..", "/etc/passwd", "", "a.txt\0x", "a\\..\\x"] {
        refused(&read(&s, &run, p), 400, "bad path", "a\n", &state);
    }
}

#[test]
fn only_a_regular_file_is_read_and_a_pipe_is_never_opened() {
    let (s, run, w) = served("products-regular", None);
    std::fs::create_dir_all(w.join("dir")).unwrap();
    let fifo = std::ffi::CString::new(w.join("pipe").to_str().unwrap()).unwrap();
    // SAFETY: a NUL-terminated path that outlives the call.
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o644) }, 0);
    let t = Instant::now();
    refused(&read(&s, &run, "pipe"), 415, "not a regular file", "no file holds this", &w);
    assert!(t.elapsed() < Duration::from_secs(5), "the read waited on a pipe");
    refused(&read(&s, &run, "dir"), 415, "not a regular file", "no file holds this", &w);
    assert_eq!(paths(&listing(&s, &run)), Vec::<String>::new(), "a pipe and a directory are no products");
}

#[test]
fn a_file_with_a_second_name_is_neither_listed_nor_read() {
    let (s, run, w) = served("products-links", None);
    let outside = s.fx.dir.join("secret.txt");
    put(&outside, b"the secret\n");
    std::fs::hard_link(&outside, w.join("hard.txt")).unwrap();
    refused(&read(&s, &run, "hard.txt"), 403, "more than one link", "the secret", &s.fx.dir);
    let row = listing(&s, &run);
    assert_eq!((paths(&row), row["left_out"].clone()), (Vec::<String>::new(), json!(1)), "{row}");
}

#[test]
fn a_name_that_starts_with_a_dot_is_neither_listed_nor_served() {
    let (s, run, w) = served("products-dots", None);
    put(&w.join(".cache/token.txt"), b"tok-4711\n");
    put(&w.join("src/.git/config"), b"cfg-4711\n");
    put(&w.join(".env"), b"KEY=v-4711\n");
    put(&w.join("src/main.rs"), b"fn main() {}\n");
    let row = listing(&s, &run);
    assert_eq!(paths(&row), ["src/main.rs"]);
    assert_eq!(row["hidden"], json!(3), "{row}");
    for p in [".cache/token.txt", "src/.git/config", ".env", "./.env"] {
        let r = read(&s, &run, p);
        refused(&r, 404, "not served", "4711", &w);
    }
}

#[test]
fn a_link_is_never_followed_whether_it_leaves_the_run_or_not() {
    let (s, run, w) = served("products-symlinks", None);
    let outside = s.fx.dir.join("outside");
    put(&outside.join("secret.txt"), b"the secret\n");
    put(&w.join("a.txt"), b"inside\n");
    symlink(outside.join("secret.txt"), w.join("out.txt")).unwrap();
    symlink(&outside, w.join("outdir")).unwrap();
    symlink("a.txt", w.join("in.txt")).unwrap();
    symlink(s.fx.dir.join("state"), w.join("state")).unwrap();
    for p in ["out.txt", "outdir/secret.txt", "in.txt", "state/journal.jsonl"] {
        refused(&read(&s, &run, p), 403, "link", "the secret", &s.fx.dir);
    }
    let row = listing(&s, &run);
    assert_eq!(paths(&row), ["a.txt"], "{row}");
    assert_eq!(row["left_out"], json!(4), "{row}");
}

#[test]
fn a_file_name_is_never_handed_to_a_program() {
    let (s, run, w) = served("products-names", None);
    // A name holds no slash, so the command a shell would run writes `pwned` in its working
    // directory: the work directory, the daemon's directory or the test's.
    let name = "$(touch pwned); `touch pwned` <b>x</b>.txt";
    put(&w.join(name), b"plain\n");
    let row = listing(&s, &run);
    assert_eq!(paths(&row), [name]);
    assert_eq!(one(&s, &run, name)["text"], json!("plain\n"));
    for dir in [w.clone(), s.fx.dir.clone(), s.fx.dir.join("state"), Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()] {
        assert!(!dir.join("pwned").exists(), "a name was run as a command in {}", dir.display());
    }
    let src = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/engine/products.rs")).unwrap();
    for word in ["std::process", "Command::new", "libc::exec", "libc::system"] {
        assert!(!src.contains(word), "the reader names {word}: it starts no program");
    }
}

#[test]
fn an_unknown_run_and_a_missing_file_are_refused_by_name_and_print_no_content() {
    let (s, run, w) = served("products-unknown", None);
    put(&w.join("a.txt"), b"the content\n");
    let none = "0190aaaa-0000-7000-8000-000000000000";
    for r in [list(&s, none), read(&s, none, "a.txt")] {
        refused(&r, 404, "no such run", "the content", &w);
        assert!(r.json()["message"].as_str().unwrap().contains(none), "{}", r.text());
    }
    let r = read(&s, &run, "b.txt");
    refused(&r, 404, "no such file", "the content", &w);
    assert!(r.json()["message"].as_str().unwrap().contains("b.txt"), "{}", r.text());
    for (view, q) in [("products", String::new()), ("product", format!("run_id={run}"))] {
        let r = s.get(&format!("/daemon/views/{view}?{q}"));
        assert_eq!(r.status, 400, "{view}: {}", r.text());
    }
}

#[test]
fn both_reads_answer_without_the_token_and_a_foreign_origin_as_every_other_read_does() {
    let (s, run, w) = served("products-token", Some(TOKEN));
    put(&w.join("a.txt"), b"the content\n");
    let bearer = format!("Bearer {TOKEN}");
    let paths = [format!("/daemon/views/products?run_id={run}"), format!("/daemon/views/product?run_id={run}&path=a.txt")];
    let other = format!("/daemon/views/run?run_id={run}");
    let no_token = s.get(&other);
    assert_eq!(no_token.status, 401);
    let origin = s.get_with(&other, &[("Authorization", &bearer), ("Origin", "http://evil.invalid")]);
    assert_eq!(origin.status, 403);
    for p in &paths {
        let r = s.get(p);
        assert_eq!((r.status, r.text()), (no_token.status, no_token.text()), "{p} without the token");
        let r = s.get_with(p, &[("Authorization", "Bearer wrong-token-0123456789-abcdefghijk")]);
        assert_eq!((r.status, r.text()), (no_token.status, no_token.text()), "{p} with a wrong token");
        let r = s.get_with(p, &[("Authorization", &bearer), ("Origin", "http://evil.invalid")]);
        assert_eq!((r.status, r.text()), (origin.status, origin.text()), "{p} from a foreign origin");
        assert!(!r.text().contains("the content"));
        assert_eq!(s.get_with(p, &[("Authorization", &bearer)]).status, 200, "{p} with the token");
    }
}

#[test]
fn the_listing_stops_at_each_of_its_bounds_and_says_which() {
    let dir = super::state::scratch("products-bounds");
    let w = dir.join("work");
    for i in 0..5 {
        put(&w.join(format!("f{i}.txt")), b"x");
    }
    put(&w.join("d1/d2/d3/deep.txt"), b"x");
    let small = Bounds { files: 3, directories: 100, entries: 100, depth: 6 };
    let l = products::list_within(&w, &small).unwrap();
    assert_eq!((l.files.len(), l.cut), (3, Some("files")));
    let l = products::list_within(&w, &Bounds { files: 100, directories: 2, entries: 100, depth: 6 }).unwrap();
    assert_eq!(l.cut, Some("directories"));
    let l = products::list_within(&w, &Bounds { files: 100, directories: 100, entries: 4, depth: 6 }).unwrap();
    assert_eq!(l.cut, Some("entries"));
    let l = products::list_within(&w, &Bounds { files: 100, directories: 100, entries: 100, depth: 1 }).unwrap();
    assert_eq!(l.cut, Some("depth"));
    assert!(!l.files.iter().any(|f| f.path.ends_with("deep.txt")));
    let l = products::list_within(&w, &Bounds::default()).unwrap();
    assert_eq!((l.files.len(), l.cut), (6, None));
    assert_eq!(Bounds::default(), Bounds { files: 500, directories: 2000, entries: 20000, depth: 6 }, "the inspector's bounds");
}
