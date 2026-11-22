//! The UI in the binary (story `ui-embedded-in-the-binary`, slice 1): the files of `ui/web/` are
//! compiled in and `runbook serve --ui` serves them beside the API, behind the daemon's token rule.

use super::commands::Fx;
use super::http::{send, Resp};
use runbook::api;
use runbook::ui_embed;
use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_runbook");
const TOKEN: &str = "t0ken-for-the-ui-tests-0123456789-abcdef";

fn web_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/web")
}

fn files_on_disk() -> Vec<(String, Vec<u8>)> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, base, out);
            } else {
                out.push((p.strip_prefix(base).unwrap().to_str().unwrap().to_string(), std::fs::read(&p).unwrap()));
            }
        }
    }
    let mut out = Vec::new();
    walk(&web_dir(), &web_dir(), &mut out);
    out.sort();
    out
}

/// The daemon of a fixture on port 0, with or without the UI.
struct Srv {
    _fx: Fx,
    addr: SocketAddr,
    rt: Option<tokio::runtime::Runtime>,
}

impl Srv {
    fn start(tag: &str, token: Option<&str>, ui: bool) -> Srv {
        let fx = Fx::new(tag);
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        let listener = rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
        let addr = listener.local_addr().unwrap();
        let (daemon, token) = (fx.d().clone(), token.map(String::from));
        rt.spawn(async move {
            let _ = api::serve_ui(listener, daemon, token.as_deref(), ui, std::future::pending()).await;
        });
        Srv { _fx: fx, addr, rt: Some(rt) }
    }

    fn get(&self, path: &str, headers: &[(&str, &str)]) -> Resp {
        send(self.addr, "GET", path, headers, None)
    }
}

impl Drop for Srv {
    fn drop(&mut self) {
        if let Some(rt) = self.rt.take() {
            rt.shutdown_background();
        }
    }
}

fn bearer() -> String {
    format!("Bearer {TOKEN}")
}

#[test]
fn every_file_of_ui_web_is_served_at_its_path_byte_for_byte() {
    let s = Srv::start("ui-files", None, true);
    let disk = files_on_disk();
    assert!(disk.len() > 20, "ui/web has {} files", disk.len());
    for (path, bytes) in &disk {
        let r = s.get(&format!("/{path}"), &[]);
        assert_eq!(r.status, 200, "{path}");
        assert_eq!(&r.body, bytes, "{path}");
    }
    let root = s.get("/", &[]);
    assert_eq!(root.status, 200);
    assert_eq!(root.body, std::fs::read(web_dir().join("index.html")).unwrap());
    assert_eq!(root.header("content-type"), Some("text/html; charset=utf-8"));
}

#[test]
fn the_embedded_list_is_the_directory_and_the_committed_files() {
    let embedded: Vec<String> = ui_embed::paths().iter().map(|p| p.to_string()).collect();
    let disk: Vec<String> = files_on_disk().into_iter().map(|(p, _)| p).collect();
    assert_eq!(embedded, disk, "a file of ui/web/ is not embedded, or an embedded file is gone");
    // the tracked files only: an untracked file in ui/web/ would be compiled into a binary that no commit holds
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let out = Command::new("git").args(["ls-files", "ui/web"]).current_dir(root).output().expect("git runs");
    assert!(out.status.success(), "git ls-files ui/web failed");
    let mut tracked: Vec<String> = String::from_utf8(out.stdout).unwrap().lines().map(|l| l.strip_prefix("ui/web/").unwrap().to_string()).collect();
    tracked.sort();
    assert_eq!(embedded, tracked, "the embedded set is the committed files of ui/web/, nothing else");
}

#[test]
fn content_types_follow_the_extension() {
    let s = Srv::start("ui-types", None, true);
    for (path, want) in [
        ("/index.html", "text/html; charset=utf-8"),
        ("/app.js", "text/javascript; charset=utf-8"),
        ("/style.css", "text/css; charset=utf-8"),
        ("/fonts/inter-variable-latin.woff2", "font/woff2"),
        ("/fonts/OFL-Inter.txt", "text/plain; charset=utf-8"),
        ("/vendor/graph-layout.js", "text/javascript; charset=utf-8"),
    ] {
        assert_eq!(s.get(path, &[]).header("content-type"), Some(want), "{path}");
    }
}

#[test]
fn without_ui_the_root_and_every_file_answer_404() {
    let s = Srv::start("ui-off", None, false);
    for path in ["/", "/index.html", "/app.js"] {
        let r = s.get(path, &[]);
        assert_eq!(r.status, 404, "{path}");
        assert_eq!(r.json()["refused"], "not found");
    }
    assert_eq!(s.get("/daemon/views/runs", &[]).status, 200);
}

#[test]
fn embedded_files_carry_the_security_headers_and_a_digest_cache_rule() {
    let s = Srv::start("ui-headers", None, true);
    let r = s.get("/app.js", &[]);
    assert_eq!(r.header("x-content-type-options"), Some("nosniff"));
    assert_eq!(r.header("server-timing"), Some("runbookd"), "the page tells that the daemon served it");
    assert_eq!(r.header("server-timing"), Some("runbookd"), "the page tells the daemon served it");
    let csp = r.header("content-security-policy").expect("a content security policy").to_string();
    for part in [
        "default-src 'none'",
        "style-src 'self'",
        "img-src 'self' data:",
        "font-src 'self'",
        "connect-src 'self'",
        "base-uri 'none'",
        "frame-ancestors 'none'",
    ] {
        assert!(csp.contains(part), "{part} in {csp}");
    }
    assert!(!csp.contains("script-src 'unsafe-inline'") && !csp.contains("'unsafe-eval'"), "{csp}");
    // the one inline script of index.html is allowed by its hash, and no other
    let index = String::from_utf8(std::fs::read(web_dir().join("index.html")).unwrap()).unwrap();
    let inline: Vec<&str> = index.split("<script>").skip(1).map(|t| t.split("</script>").next().unwrap()).collect();
    assert_eq!(inline.len(), 1, "index.html has {} inline scripts", inline.len());
    let hash = ui_embed::csp_hash(inline[0].as_bytes());
    assert!(csp.contains(&format!("script-src 'self' '{hash}'")), "{hash} in {csp}");
    // the cache rule: revalidate by the digest of the bytes, 304 when it still matches
    assert_eq!(r.header("cache-control"), Some("private, no-cache"));
    let etag = r.header("etag").expect("an etag").to_string();
    assert_eq!(etag, format!("\"{}\"", ui_embed::digest(&r.body)));
    let again = s.get("/app.js", &[("If-None-Match", &etag)]);
    assert_eq!(again.status, 304);
    assert!(again.body.is_empty());
    assert_eq!(s.get("/app.js", &[("If-None-Match", "\"other\"")]).status, 200);
    assert_ne!(s.get("/style.css", &[]).header("etag"), Some(etag.as_str()));
}

#[test]
fn the_csp_hash_is_base64_of_the_sha256() {
    assert_eq!(ui_embed::csp_hash(b""), "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=");
    assert_eq!(ui_embed::csp_hash(b"a"), "sha256-ypeBEsobvcr6wjGzmiPcTaeG7/gUfE5yuYB3ha/uSLs=");
}

#[test]
fn no_directory_listing_and_no_path_outside_the_embedded_list() {
    let s = Srv::start("ui-paths", None, true);
    for path in [
        "/fonts",
        "/fonts/",
        "/vendor/",
        "/..",
        "/../Cargo.toml",
        "/../../Cargo.toml",
        "/%2e%2e/Cargo.toml",
        "/%2E%2E/Cargo.toml",
        "/..%2fCargo.toml",
        "/%2e%2e%2fCargo.toml",
        "/fonts/..%2f..%2fCargo.toml",
        "/vendor/../app.js",
        "/./app.js",
        "//app.js",
        "/app.js/",
        "/app.js%00",
        "/app%2ejs",
        "/App.js",
        "/%2findex.html",
        "/ui/web/app.js",
    ] {
        let r = s.get(path, &[]);
        assert_eq!(r.status, 404, "{path} answered {}", r.status);
        assert!(!r.text().contains("[package]"), "{path}");
    }
    // a method that is not a read, on a file that exists
    assert_eq!(send(s.addr, "POST", "/app.js", &[], Some(b"x")).status, 405);
}

#[test]
fn a_query_does_not_change_the_file_served() {
    let s = Srv::start("ui-query", None, true);
    let a = s.get("/", &[]);
    let b = s.get("/?tab=events&run=x", &[]);
    assert_eq!(a.body, b.body);
    assert_eq!(b.status, 200);
}

#[test]
fn the_token_rule_guards_the_ui_through_the_same_check_as_the_api() {
    let s = Srv::start("ui-token", Some(TOKEN), true);
    for path in ["/", "/index.html", "/app.js", "/fonts/inter-variable-latin.woff2", "/nothing", "/daemon/views/runs"] {
        let r = s.get(path, &[]);
        assert_eq!(r.status, 401, "{path}");
        assert_eq!(r.header("www-authenticate"), Some("Bearer"), "{path}");
    }
    assert_eq!(s.get("/", &[("Authorization", "Bearer wrong")]).status, 401);
    let ok = s.get("/", &[("Authorization", &bearer())]);
    assert_eq!(ok.status, 200);
    assert_eq!(ok.body, std::fs::read(web_dir().join("index.html")).unwrap());
    assert_eq!(s.get("/app.js", &[("Authorization", &bearer())]).status, 200);
    assert_eq!(s.get("/daemon/views/runs", &[("Authorization", &bearer())]).status, 200);
}

#[test]
fn a_browser_hands_over_the_token_in_a_form_body_and_keeps_it_in_a_cookie() {
    let s = Srv::start("ui-login", Some(TOKEN), true);
    // the navigation of a browser: 401 with a form that posts the token (never in a URL)
    let page = s.get("/", &[("Accept", "text/html,application/xhtml+xml")]);
    assert_eq!(page.status, 401);
    assert_eq!(page.header("content-type"), Some("text/html; charset=utf-8"));
    assert!(page.text().contains("method=\"post\"") && page.text().contains("action=\"/ui/login\""), "{}", page.text());
    assert!(!page.text().contains(TOKEN));
    let form = |token: &str, extra: &[(&str, &str)]| {
        let mut h = vec![("Content-Type", "application/x-www-form-urlencoded")];
        h.extend_from_slice(extra);
        send(s.addr, "POST", "/ui/login", &h, Some(format!("token={token}").as_bytes()))
    };
    // a wrong token: refused, no cookie
    let bad = form("nope", &[]);
    assert_eq!(bad.status, 401);
    assert!(bad.header("set-cookie").is_none());
    // a form from another origin is refused whatever it carries
    let cross = form(TOKEN, &[("Origin", "http://evil.example")]);
    assert_eq!(cross.status, 403);
    assert!(cross.header("set-cookie").is_none());
    // the right token: a redirect to / and a cookie that script cannot read
    let good = form(TOKEN, &[]);
    assert_eq!(good.status, 303);
    assert_eq!(good.header("location"), Some("/"));
    let cookie = good.header("set-cookie").expect("a cookie").to_string();
    assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict") && cookie.contains("Path=/"), "{cookie}");
    assert!(!cookie.contains(TOKEN), "the cookie holds the token encoded: {cookie}");
    let pair = cookie.split(';').next().unwrap().to_string();
    // reads with the cookie: the page, a file, a view and a stream's route
    let with = [("Cookie", pair.as_str())];
    let root = s.get("/", &with);
    assert_eq!(root.status, 200);
    assert_eq!(root.body, std::fs::read(web_dir().join("index.html")).unwrap());
    assert_eq!(s.get("/daemon/views/runs", &with).status, 200);
    // a wrong cookie, and a cookie on a write: refused
    assert_eq!(s.get("/", &[("Cookie", "runbook_ui=00")]).status, 401);
    let post = send(s.addr, "POST", "/daemon/commands/open-session", &[("Cookie", &pair), ("Content-Type", "application/json")], Some(br#"{"session_id":"x","client":"c"}"#));
    assert_eq!(post.status, 401, "a cookie is for reads: {}", post.text());
}

#[test]
fn without_ui_a_cookie_is_not_a_token() {
    let s = Srv::start("ui-nocookie", Some(TOKEN), false);
    let hex: String = TOKEN.bytes().map(|b| format!("{b:02x}")).collect();
    assert_eq!(s.get("/daemon/views/runs", &[("Cookie", &format!("runbook_ui={hex}"))]).status, 401);
    assert_eq!(send(s.addr, "POST", "/ui/login", &[], Some(b"token=x")).status, 401);
}

// ---- the binary: `runbook serve --ui` from an empty directory ----

struct Daemon {
    child: Child,
    port: u16,
}

impl Daemon {
    fn start(tag: &str, args: &[&str], token: Option<&str>) -> (Daemon, PathBuf) {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("empty")).unwrap();
        let mut cmd = Command::new(BIN);
        cmd.arg("serve").args(["--principal", super::PRINCIPAL]).args(args).arg("--state-dir").arg(dir.join("state"));
        cmd.current_dir(dir.join("empty")).env_remove("RUNBOOK_TOKEN").env("HOME", &dir).stdout(Stdio::piped()).stderr(Stdio::null());
        if let Some(t) = token {
            cmd.env("RUNBOOK_TOKEN", t);
        }
        let mut child = cmd.spawn().unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.as_mut().unwrap()).read_line(&mut line).unwrap();
        let port: u16 = line.trim().rsplit(':').next().and_then(|p| p.parse().ok()).unwrap_or_else(|| panic!("no listening line: {line:?}"));
        (Daemon { child, port }, dir.join("empty"))
    }

    fn get(&self, path: &str, headers: &[(&str, &str)]) -> Resp {
        send(SocketAddr::from(([127, 0, 0, 1], self.port)), "GET", path, headers, None)
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn serve_ui_from_an_empty_directory_answers_the_index_and_without_ui_404() {
    let (d, cwd) = Daemon::start("ui-bin", &["--listen", "127.0.0.1:0", "--ui"], None);
    assert!(!cwd.join("ui").exists());
    let index = std::fs::read(web_dir().join("index.html")).unwrap();
    let r = d.get("/", &[]);
    assert_eq!((r.status, &r.body), (200, &index));
    let app = std::fs::read(web_dir().join("app.js")).unwrap();
    assert_eq!(d.get("/app.js", &[]).body, app);
    assert_eq!(d.get("/daemon/views/configuration", &[]).status, 200);

    let (plain, _) = Daemon::start("ui-bin-off", &["--listen", "127.0.0.1:0"], None);
    assert_eq!(plain.get("/", &[]).status, 404);
}

#[test]
fn a_non_loopback_listener_gives_the_ui_to_the_token_only() {
    let (d, _) = Daemon::start("ui-bin-wide", &["--listen", "0.0.0.0:0", "--ui"], Some(TOKEN));
    assert_eq!(d.get("/", &[]).status, 401);
    assert_eq!(d.get("/app.js", &[]).status, 401);
    let r = d.get("/", &[("Authorization", &bearer())]);
    assert_eq!(r.status, 200);
    assert_eq!(r.body, std::fs::read(web_dir().join("index.html")).unwrap());
}
