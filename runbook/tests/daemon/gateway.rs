//! The gateway of a daemon's runs (`docs/engine.md`, "A gateway"): the `[gateway]` table of the `runbook.toml` the
//! daemon reads gives the model programs of its runs an address and a token, the way `runbook run` gives them: one
//! file `gateway` in the run's private directory, the address and the token set for the model process only, and no
//! login linked or copied. A submit names no gateway, address or token.
//!
//! No test calls a model or a network beyond loopback: the model decider's adapter is the committed claude-cli
//! adapter, `claude` is a stand-in in bash that the run's copy of the bindings puts first on the decider's `PATH`,
//! and the gateway is a stand-in on a loopback port of the test's own. The token is a string the case invents.

use super::default_confinement::{live, live_env, post, rows, Live};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TOKEN: &str = "gw-daemon-7c41e9b2d05a";
const RULE: &str = "[deciders.rule]\nverdict = \"approve\"\n";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

fn write_exe(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A gateway on a loopback port: it keeps the head of every request and answers each with 200.
struct StandinGateway {
    base_url: String,
    heads: Arc<Mutex<Vec<String>>>,
}

fn standin_gateway() -> StandinGateway {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let heads = Arc::new(Mutex::new(Vec::new()));
    let kept = heads.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            while !head.ends_with(b"\r\n\r\n") && head.len() < 64 * 1024 {
                match stream.read(&mut byte) {
                    Ok(1) => head.push(byte[0]),
                    _ => break,
                }
            }
            kept.lock().unwrap().push(String::from_utf8_lossy(&head).into_owned());
            let _ = stream.write_all(b"HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\n{}");
        }
    });
    StandinGateway { base_url, heads }
}

/// A `claude` in bash: it notes whether its config directory holds a login and whether a login variable reached it
/// (`log/claude-env`), sends one request to `ANTHROPIC_BASE_URL` with `ANTHROPIC_AUTH_TOKEN` as its bearer, and
/// approves only when the gateway answered 200.
fn standin_claude(log: &Path) -> String {
    format!(
        r#"#!/bin/bash
log='{log}'
{{
  [ -e "${{CLAUDE_CONFIG_DIR:-/nonexistent}}/.credentials.json" ] && echo login=present || echo login=absent
  [ -n "${{SDC_CLAUDE_CREDENTIALS+x}}" ] && echo login_variable=set || echo login_variable=unset
  [ -n "${{SDC_GATEWAY_FILE+x}}" ] && echo gateway_variable=set || echo gateway_variable=unset
}} >> "$log/claude-env"
rest=${{ANTHROPIC_BASE_URL#http://}}
hostport=${{rest%%/*}}
exec 3<>"/dev/tcp/${{hostport%:*}}/${{hostport##*:}}" || exit 7
printf 'POST /v1/messages HTTP/1.0\r\nHost: %s\r\nAuthorization: Bearer %s\r\nContent-Length: 0\r\n\r\n' "$hostport" "${{ANTHROPIC_AUTH_TOKEN-}}" >&3
read -r status <&3
exec 3<&-
case $status in *" 200 "*) ;; *) exit 8 ;; esac
mkdir -p out && printf '%s\n' '{{"decision":"approve","reasons":["ok"],"cites":[]}}' > out/decision.json
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"num_turns":1,"total_cost_usd":0,"duration_ms":1,"duration_api_ms":0,"session_id":"s","usage":{{"input_tokens":0,"output_tokens":0}},"result":"ok","terminal_reason":"completed","structured_output":{{"decision":"approve","reasons":["ok"],"cites":[]}}}}'
"#,
        log = log.display()
    )
}

/// The home of a daemon with the model kind on the committed claude-cli adapter and a stand-in `claude` in the
/// runbook's tools, the token file outside every directory the daemon writes, and the runtime directory.
struct Home {
    home: PathBuf,
    token_file: PathBuf,
    xdg: PathBuf,
    log: PathBuf,
}

fn home(tag: &str) -> Home {
    let base = super::state::scratch(&format!("{tag}-files"));
    let home = base.join("home");
    copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/autonomy-runbook"), &home);
    copy_tree(&repo().join("instructions/decider"), &home.join("instructions/decider"));
    let log = base.join("log");
    std::fs::create_dir_all(&log).unwrap();
    write_exe(&home.join("bindings/tools/claude"), &standin_claude(&log));
    let secret = base.join("secret");
    std::fs::create_dir_all(&secret).unwrap();
    let token_file = secret.join("gateway-token");
    std::fs::write(&token_file, format!("{TOKEN}\n")).unwrap();
    std::fs::set_permissions(&token_file, std::fs::Permissions::from_mode(0o600)).unwrap();
    let xdg = base.join("xdg");
    std::fs::create_dir_all(&xdg).unwrap();
    std::fs::set_permissions(&xdg, std::fs::Permissions::from_mode(0o700)).unwrap();
    Home { home, token_file, xdg, log }
}

fn model_table(adapter: Option<&str>) -> String {
    let adapter = adapter.map(|a| format!("adapter = \"{a}\"\n")).unwrap_or_default();
    format!("[deciders.model]\nmodel = \"a-model\"\n{adapter}without_claims = true\ntimeout_seconds = 20\nretries = 0\n")
}

fn claude_cli() -> String {
    repo().join("bindings/tools/llm-adapter-claude-cli.sh").display().to_string()
}

fn gateway_table(base_url: &str, token_file: &Path) -> String {
    format!("[gateway]\nbase_url = \"{base_url}\"\ntoken_file = \"{}\"\n", token_file.display())
}

fn serve(tag: &str, h: &Home, file: &str, with_runtime_dir: bool) -> Live {
    let xdg = h.xdg.to_str().unwrap();
    let all = ["--runtime", "loom", "--runbook-home", h.home.to_str().unwrap(), "--min-confinement", "local", "--max-wall-seconds", "3600", "--max-llm-calls", "10000", "--cap-micro-usd", "1000000000"];
    live_env(tag, &all, Some(file), &[("XDG_RUNTIME_DIR", with_runtime_dir.then_some(xdg))])
}

fn submit(l: &Live, kind: &str) -> String {
    let (status, body) = post(l, &json!({"runbook_id": "autonomy-demo", "intake": "a note on tea", "confinement": "local", "autonomy": "A5", "decider": kind}));
    assert_eq!((status, body["outcome"].clone()), (202, json!("submitted")), "{body}");
    body["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned()
}

fn row(l: &Live, run: &str) -> Value {
    rows(l, &format!("/daemon/views/run?run_id={run}"))[0].clone()
}

fn until_state(l: &Live, run: &str, states: &[&str]) -> Value {
    let limit = Instant::now() + Duration::from_secs(40);
    loop {
        let r = row(l, run);
        if states.iter().any(|s| r["state"] == *s) {
            return r;
        }
        assert!(Instant::now() < limit, "gave up waiting for {states:?}: {r}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    names.sort();
    names
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let limit = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < limit, "gave up waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Every file below `dir` whose bytes hold `needle`.
fn holders(dir: &Path, needle: &str) -> Vec<PathBuf> {
    let mut hits = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push(p);
            } else if std::fs::read(&p).is_ok_and(|b| String::from_utf8_lossy(&b).contains(needle)) {
                hits.push(p);
            }
        }
    }
    hits
}

#[test]
fn a_run_of_a_daemon_whose_file_names_a_gateway_reaches_the_standin_gateway_with_the_token_and_holds_no_login() {
    let h = home("gw-daemon-reach");
    let gw = standin_gateway();
    let file = format!("{}{}", gateway_table(&gw.base_url, &h.token_file), model_table(Some(&claude_cli())));
    let l = serve("gw-daemon-reach-d", &h, &file, true);
    let run = submit(&l, "model");
    let end = until_state(&l, &run, &["Ended", "FailedToStart", "Cancelled"]);
    assert_eq!(end["state"], "Ended", "the model decider answered no decision through the gateway: {end}");
    let heads = gw.heads.lock().unwrap().clone();
    assert!(!heads.is_empty(), "the stand-in gateway was never reached");
    for head in &heads {
        assert!(head.contains(&format!("Authorization: Bearer {TOKEN}\r\n")), "a request reached the gateway without the configured token: {head}");
    }
    let seen = std::fs::read_to_string(h.log.join("claude-env")).expect("claude was never started");
    assert_eq!(seen.lines().filter(|l| *l == "login=absent").count(), heads.len(), "a call had a login in its config home: {seen}");
    assert!(!seen.contains("login=present") && !seen.contains("login_variable=set") && !seen.contains("gateway_variable=set"), "{seen}");
    // the token is in no file the daemon wrote and in no event of the run
    assert_eq!(holders(&l.dir, TOKEN), Vec::<PathBuf>::new(), "the token is in a file of the daemon's state");
    let events = rows(&l, &format!("/daemon/views/events?run_id={run}&after_position=0"));
    assert!(!events.is_empty() && !serde_json::to_string(&events).unwrap().contains(TOKEN), "the token is in an event of the run");
    assert!(!serde_json::to_string(&row(&l, &run)).unwrap().contains(TOKEN));
    // the private directory and its gateway file went with the run
    wait_until("the run's private directory to go", || entries(&h.xdg.join("runbook")).is_empty());
}

#[test]
fn the_gateway_file_of_a_daemon_run_is_mode_600_in_its_private_directory_and_the_adapter_gets_its_path_alone() {
    let h = home("gw-daemon-file");
    let gw = standin_gateway();
    // an adapter that keeps its environment and what the gateway file holds, and fails: the decision stays open
    let adapter = h.log.join("adapter.sh");
    write_exe(&adapter, &format!("#!/bin/sh\nenv > '{log}/adapter-env'\nstat -c '%a' \"$SDC_GATEWAY_FILE\" > '{log}/file-mode'\nstat -c '%a' \"$(dirname \"$SDC_GATEWAY_FILE\")\" > '{log}/dir-mode'\nls -A \"$(dirname \"$SDC_GATEWAY_FILE\")\" > '{log}/dir-ls'\ngrep -qx 'token={TOKEN}' \"$SDC_GATEWAY_FILE\" && grep -qx 'base_url={base}' \"$SDC_GATEWAY_FILE\" && echo yes > '{log}/file-content'\nexit 1\n", log = h.log.display(), base = gw.base_url));
    let file = format!("{}{}", gateway_table(&gw.base_url, &h.token_file), model_table(Some(adapter.to_str().unwrap())));
    let l = serve("gw-daemon-file-d", &h, &file, true);
    let run = submit(&l, "model");
    wait_until("the adapter to run", || h.log.join("file-content").exists() || h.log.join("adapter-env").exists());
    std::thread::sleep(Duration::from_millis(300));
    let env = std::fs::read_to_string(h.log.join("adapter-env")).unwrap();
    let file_var = env.lines().find_map(|l| l.strip_prefix("SDC_GATEWAY_FILE=")).expect("the adapter was given no gateway file").to_owned();
    assert!(Path::new(&file_var).starts_with(h.xdg.join("runbook")) && file_var.ends_with("/gateway"), "{file_var}");
    assert!(!env.contains(TOKEN) && !env.contains("ANTHROPIC_") && !env.contains("SDC_CLAUDE_CREDENTIALS"), "{env}");
    let read = |name: &str| std::fs::read_to_string(h.log.join(name)).unwrap_or_default().trim().to_owned();
    assert_eq!((read("file-mode"), read("dir-mode"), read("dir-ls"), read("file-content")), ("600".to_owned(), "700".to_owned(), "gateway".to_owned(), "yes".to_owned()));
    assert!(gw.heads.lock().unwrap().is_empty(), "the stand-in adapter calls nothing");
    let r = super::http::send(l.addr, "POST", "/daemon/commands/cancel-run", &[("Content-Type", "application/json")], Some(json!({"run_id": run}).to_string().as_bytes()));
    assert!(r.status < 300, "cancel-run: {}", r.status);
    until_state(&l, &run, &["Cancelled"]);
    wait_until("the run's private directory to go", || !Path::new(&file_var).parent().unwrap().exists());
    assert_eq!(holders(&l.dir, TOKEN), Vec::<PathBuf>::new(), "the token is in a file of the daemon's state");
}

#[test]
fn a_daemon_run_that_starts_no_model_program_writes_no_gateway_file() {
    let h = home("gw-daemon-rule");
    let gw = standin_gateway();
    let file = format!("{}{}{RULE}", gateway_table(&gw.base_url, &h.token_file), model_table(Some(&claude_cli())));
    let l = serve("gw-daemon-rule-d", &h, &file, true);
    let run = submit(&l, "rule");
    let end = until_state(&l, &run, &["Ended", "FailedToStart", "Cancelled"]);
    assert_eq!(end["state"], "Ended", "{end}");
    assert_eq!(entries(&h.xdg), Vec::<String>::new(), "a run whose decider is a rule made a private directory");
    assert!(gw.heads.lock().unwrap().is_empty() && !h.log.join("claude-env").exists());
}

#[test]
fn a_model_run_under_a_gateway_with_no_runtime_directory_fails_to_start_by_name_and_names_no_value() {
    let h = home("gw-daemon-no-xdg");
    let file = format!("{}{}", gateway_table("http://127.0.0.1:9", &h.token_file), model_table(Some(&claude_cli())));
    let l = serve("gw-daemon-no-xdg-d", &h, &file, false);
    let run = submit(&l, "model");
    let end = until_state(&l, &run, &["FailedToStart", "Ended", "Cancelled", "WaitingOnDecision"]);
    assert_eq!(end["state"], "FailedToStart", "{end}");
    let why = end["failure"].as_str().unwrap_or("");
    assert!(why.contains("gateway") && why.contains("XDG_RUNTIME_DIR"), "{why}");
    assert!(!why.contains(TOKEN) && !why.contains(&h.token_file.display().to_string()), "{why}");
}

#[test]
fn a_daemon_whose_gateway_cannot_be_used_does_not_start_and_names_no_value_of_the_file() {
    let h = home("gw-daemon-refused");
    let empty = h.log.join("empty-token");
    std::fs::write(&empty, "").unwrap();
    let missing = h.log.join("no-such-dir/token");
    let cases = [
        // the model decider's default adapter, metaharness, cannot take a gateway; neither can it named
        (format!("{}{}", gateway_table("http://127.0.0.1:8080", &h.token_file), model_table(None)), "deciders.model.adapter"),
        (format!("{}{}", gateway_table("http://127.0.0.1:8080", &h.token_file), model_table(Some("metaharness"))), "deciders.model.adapter"),
        (format!("{}{}", gateway_table("http://127.0.0.1:8080", &missing), model_table(Some(&claude_cli()))), "gateway.token_file"),
        (format!("{}{}", gateway_table("http://127.0.0.1:8080", &empty), model_table(Some(&claude_cli()))), "gateway.token_file"),
    ];
    for (n, (file, member)) in cases.iter().enumerate() {
        let dir = super::state::scratch(&format!("gw-daemon-refused-{n}"));
        std::fs::write(dir.join("runbook.toml"), file).unwrap();
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_runbook"));
        cmd.arg("serve").args(["--principal", super::PRINCIPAL, "--listen", "127.0.0.1:0", "--runtime", "loom", "--runbook-home"]).arg(&h.home).arg("--state-dir").arg(dir.join("state"));
        cmd.args(["--min-confinement", "local", "--max-wall-seconds", "3600", "--max-llm-calls", "10000", "--cap-micro-usd", "1000000000"]);
        cmd.env_remove("RUNBOOK_TOKEN").env("RUNBOOK_CONFIG", dir.join("runbook.toml")).env("XDG_RUNTIME_DIR", &h.xdg);
        cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        let (status, out, err) = super::http::exits_within(cmd, Duration::from_secs(10));
        assert_eq!(status, Some(2), "case {n}: the daemon started with a gateway it cannot use: {out} {err}");
        assert!(err.contains(member), "case {n}: the refusal does not name {member}: {err}");
        for value in [TOKEN, "127.0.0.1:8080", "gateway-token", "no-such-dir", "empty-token"] {
            assert!(!err.contains(value) && !out.contains(value), "case {n}: {value} is in the refusal: {err}");
        }
        assert!(!dir.join("state").exists(), "case {n}: nothing written");
        assert_eq!(entries(&h.xdg), Vec::<String>::new(), "case {n}: a private directory was made");
    }
}

#[test]
fn a_submit_that_names_a_gateway_an_address_or_a_token_is_refused_by_name_and_never_with_its_value() {
    let l = live("gw-daemon-submit", &["--min-confinement", "local"], None);
    let value = "gw-submit-value-3e9a51c7";
    let base = || json!({"runbook_id": runbook::serve::STUB_DECISION, "intake": "x", "confinement": "local"});
    let mut cases: Vec<(Value, &str)> = Vec::new();
    for key in ["gateway", "base_url", "token", "token_file", "gateway.base_url", "gateway.token", "gateway.token_file", "Gateway.Token"] {
        let mut body = base();
        body["settings"] = json!({key: value});
        cases.push((body, key));
    }
    for key in ["gateway", "base_url", "token", "token_file"] {
        let mut body = base();
        body[key] = json!(value);
        cases.push((body, key));
    }
    let mut body = base();
    body["gateway"] = json!({"base_url": "http://127.0.0.1:8080", "token": value});
    cases.push((body, "gateway"));
    for (body, key) in cases {
        let (status, answer) = post(&l, &body);
        let text = answer.to_string();
        assert_eq!(status, 400, "{key}: {text}");
        assert!(text.contains(key), "{key}: the refusal does not name it: {text}");
        assert!(!text.contains(value) && !text.contains("127.0.0.1:8080"), "{key}: the refusal holds the value: {text}");
    }
    assert_eq!(rows(&l, "/daemon/views/runs"), Vec::<Value>::new(), "a refused submit made a run");
    assert_eq!(holders(&l.dir, value), Vec::<PathBuf>::new(), "a refused value is in the daemon's state");
}
