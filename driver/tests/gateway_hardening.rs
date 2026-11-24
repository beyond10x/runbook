//! Cases of the gateway file's readers and of the claude-cli adapter under a gateway, beside the cases of
//! `gateway.rs`. Invented strings only; `claude` is a stand-in in POSIX sh; no model, no network.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const TOKEN: &str = "review-gw-token-7c41e9d05b2a";
const BASE: &str = "http://127.0.0.1:8080";

fn tools() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../bindings/tools")
}

struct Case {
    base: PathBuf,
}

impl Case {
    fn new(name: &str) -> Case {
        let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("review-gateway").join(name);
        let _ = fs::remove_dir_all(&base);
        for d in ["work", "bin", "log", "private"] {
            fs::create_dir_all(base.join(d)).unwrap();
        }
        fs::set_permissions(base.join("private"), fs::Permissions::from_mode(0o700)).unwrap();
        Case { base }
    }

    fn program(&self, name: &str, text: &str) {
        let p = self.base.join("bin").join(name);
        fs::write(&p, text).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn gateway_file(&self, token: &str) -> PathBuf {
        let p = self.base.join("private/gateway");
        fs::write(&p, format!("base_url={BASE}\ntoken={token}\n")).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
        p
    }

    fn request(&self) -> PathBuf {
        let work = self.base.join("work");
        fs::write(work.join("prompt.md"), "prompt\n").unwrap();
        fs::create_dir_all(work.join("cwd")).unwrap();
        let request = serde_json::json!({"record_dir": work.join("rec"), "prompt_file": work.join("prompt.md"), "cwd": work.join("cwd"),
            "model": "claude-haiku-5-5", "max_turns": 1, "max_budget_usd": 0.1, "operations": ["file.read"], "node": "idea.dissect", "attempt": 1});
        fs::write(work.join("request.json"), request.to_string()).unwrap();
        work.join("request.json")
    }

    /// The claude-cli adapter with the gateway file and `extra` on a cleared environment.
    fn adapter(&self, extra: &[(&str, &str)]) -> std::process::Output {
        self.adapter_of(Some(TOKEN), extra)
    }

    /// The claude-cli adapter on a cleared environment: with a gateway file that holds `token`, or with none.
    fn adapter_of(&self, token: Option<&str>, extra: &[(&str, &str)]) -> std::process::Output {
        let path = format!("{}:{}", self.base.join("bin").display(), std::env::var("PATH").unwrap_or_default());
        let mut c = Command::new(tools().join("llm-adapter-claude-cli.sh"));
        c.arg(self.request()).env_clear().env("PATH", path);
        if let Some(token) = token {
            c.env("SDC_GATEWAY_FILE", self.gateway_file(token));
        }
        for (k, v) in extra {
            c.env(k, v);
        }
        c.current_dir(self.base.join("work")).stdin(Stdio::null()).output().unwrap()
    }

    /// The files under `dir` that hold any of `needles`.
    fn holders(&self, dir: &Path, needles: &[&str], out: &mut Vec<String>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                self.holders(&p, needles, out);
            } else if fs::read(&p).map(|b| needles.iter().any(|n| b.windows(n.len()).any(|w| w == n.as_bytes()))).unwrap_or(false) {
                out.push(p.strip_prefix(&self.base).unwrap().display().to_string());
            }
        }
    }
}

const RESULT: &str = r#"{"type":"result","subtype":"success","is_error":false,"num_turns":1,"total_cost_usd":0,"duration_ms":1,"usage":{}}"#;

#[test]
fn no_other_anthropic_variable_of_the_caller_reaches_claude_under_a_gateway() {
    let case = Case::new("other-anthropic");
    let log = case.base.join("log/claude-names");
    case.program("claude", &format!("#!/bin/sh\nenv | cut -d= -f1 | sort > '{}'\necho '{RESULT}'\n", log.display()));
    let caller = [
        ("ANTHROPIC_CUSTOM_HEADERS", "x-caller: marker"),
        ("ANTHROPIC_MODEL", "caller-model"),
        ("ANTHROPIC_SMALL_FAST_MODEL", "caller-model"),
        ("ANTHROPIC_BEDROCK_BASE_URL", "http://caller.invalid"),
        ("ANTHROPIC_FOUNDRY_API_KEY", "caller-marker"),
        ("ANTHROPIC_VERTEX_PROJECT_ID", "caller-marker"),
    ];
    let o = case.adapter(&caller);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let names = fs::read_to_string(&log).expect("claude was not started");
    let extra: Vec<&str> = names.lines().filter(|n| n.starts_with("ANTHROPIC_") && *n != "ANTHROPIC_BASE_URL" && *n != "ANTHROPIC_AUTH_TOKEN").collect();
    assert_eq!(extra, Vec::<&str>::new(), "variables of the caller reached claude beside the gateway's two");
}

#[test]
fn what_the_model_program_prints_is_checked_for_the_token_before_it_is_kept_in_the_records() {
    let case = Case::new("echo-token");
    case.program(
        "claude",
        &format!("#!/bin/sh\necho \"{{\\\"type\\\":\\\"user\\\",\\\"tool_result\\\":\\\"$ANTHROPIC_AUTH_TOKEN\\\"}}\"\necho \"claude: $ANTHROPIC_AUTH_TOKEN\" >&2\necho '{RESULT}'\n"),
    );
    let o = case.adapter(&[]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let mut hits = Vec::new();
    case.holders(&case.base.join("work"), &[TOKEN], &mut hits);
    hits.sort();
    assert_eq!(hits, Vec::<String>::new(), "the token the model program printed is kept in the records of the call");
}

/// A `claude` that writes the names of its environment into `log/claude-names` and ends with a result.
fn names_logger(case: &Case) -> PathBuf {
    let log = case.base.join("log/claude-names");
    case.program("claude", &format!("#!/bin/sh\nenv | cut -d= -f1 | sort > '{}'\necho '{RESULT}'\n", log.display()));
    log
}

/// The names of `names` that begin `ANTHROPIC_` or `CLAUDE_CODE_`, sorted.
fn claude_names(names: &str) -> Vec<&str> {
    names.lines().filter(|n| n.starts_with("ANTHROPIC_") || n.starts_with("CLAUDE_CODE_")).collect()
}

#[test]
fn under_a_gateway_claude_has_no_anthropic_or_claude_code_name_of_the_caller_beside_the_adapters_own() {
    let case = Case::new("prefixed-names");
    let log = names_logger(&case);
    let caller = [
        ("ANTHROPIC_API_KEY", "caller-marker"),
        ("CLAUDE_CODE_OAUTH_TOKEN", "caller-marker"),
        ("CLAUDE_CODE_USE_BEDROCK", "1"),
        ("CLAUDE_CODE_EFFORT_LEVEL", "high"),
        ("CLAUDE_CODE_INVENTED_NAME_7", "caller-marker"),
        ("ANTHROPIC_INVENTED_NAME_7", "caller-marker"),
    ];
    let o = case.adapter(&caller);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let names = fs::read_to_string(&log).expect("claude was not started");
    assert_eq!(
        claude_names(&names),
        ["ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL", "CLAUDE_CODE_DISABLE_AUTO_MEMORY", "CLAUDE_CODE_DISABLE_GIT_INSTRUCTIONS"],
        "claude has a name of the caller beside the gateway's two and the adapter's own"
    );
}

#[test]
fn without_a_gateway_the_anthropic_and_claude_code_names_of_the_caller_reach_claude_as_before() {
    let case = Case::new("prefixed-names-no-gateway");
    let log = names_logger(&case);
    let caller = [
        ("ANTHROPIC_API_KEY", "caller-marker"),
        ("ANTHROPIC_MODEL", "caller-model"),
        ("CLAUDE_CODE_USE_BEDROCK", "1"),
        ("ANTHROPIC_INVENTED_NAME_7", "caller-marker"),
    ];
    let o = case.adapter_of(None, &caller);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let names = fs::read_to_string(&log).expect("claude was not started");
    assert_eq!(
        claude_names(&names),
        ["ANTHROPIC_API_KEY", "ANTHROPIC_INVENTED_NAME_7", "ANTHROPIC_MODEL", "CLAUDE_CODE_DISABLE_AUTO_MEMORY", "CLAUDE_CODE_DISABLE_GIT_INSTRUCTIONS", "CLAUDE_CODE_USE_BEDROCK"]
    );
}

/// The path of the program `name` on the host's `PATH`.
fn real(name: &str) -> PathBuf {
    std::env::var("PATH").unwrap().split(':').map(|d| Path::new(d).join(name)).find(|p| p.is_file()).expect(name)
}

/// A `jq` ahead of the real one that logs each start into `log/jq-runs`, and each start whose environment holds `needle`
/// into `log/jq-hits`; with `fail_on` it ends 1 where its argv holds that word.
fn jq_watch(case: &Case, needle: &str, fail_on: Option<&str>) -> (PathBuf, PathBuf) {
    let (runs, hits) = (case.base.join("log/jq-runs"), case.base.join("log/jq-hits"));
    let fail = fail_on.map(|w| format!("case \" $* \" in *\" {w} \"*) exit 1 ;; esac\n")).unwrap_or_default();
    case.program(
        "jq",
        &format!("#!/bin/sh\necho run >> '{}'\ngrep -a -q -F -e '{needle}' \"/proc/$$/environ\" && echo hit >> '{}'\n{fail}exec '{}' \"$@\"\n", runs.display(), hits.display(), real("jq").display()),
    );
    (runs, hits)
}

#[test]
fn no_process_but_claude_holds_the_token_in_its_environment() {
    let case = Case::new("environ-scan");
    let (runs, hits) = jq_watch(&case, TOKEN, None);
    case.program("claude", &format!("#!/bin/sh\necho '{RESULT}'\n"));
    // The caller exported the name that the reader fills: the value stays out of every environment but claude's.
    let o = case.adapter(&[("SDC_GATEWAY_TOKEN", "caller-exported-marker")]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(fs::read_to_string(&runs).unwrap_or_default().lines().count() >= 3, "the stand-in jq was not started by the adapter");
    assert_eq!(fs::read_to_string(&hits).unwrap_or_default().lines().count(), 0, "a process other than claude has the token in its environment");
}

#[test]
fn a_token_filter_that_cannot_run_ends_the_call_with_exit_127_and_keeps_nothing_the_model_program_printed() {
    let case = Case::new("filter-fails");
    jq_watch(&case, TOKEN, Some("--rawfile"));
    case.program("claude", &format!("#!/bin/sh\necho \"claude: $ANTHROPIC_AUTH_TOKEN\"\necho \"claude: $ANTHROPIC_AUTH_TOKEN\" >&2\necho '{RESULT}'\n"));
    let o = case.adapter(&[]);
    assert_eq!(o.status.code(), Some(127), "{}", String::from_utf8_lossy(&o.stderr));
    let mut hits = Vec::new();
    case.holders(&case.base.join("work"), &[TOKEN], &mut hits);
    assert_eq!(hits, Vec::<String>::new(), "the token is kept in the records of a call whose filter failed");
    assert!(!String::from_utf8_lossy(&o.stderr).contains(TOKEN) && !String::from_utf8_lossy(&o.stdout).contains(TOKEN));
}

#[test]
fn every_occurrence_of_the_token_is_replaced_by_the_fixed_text_and_the_rest_of_each_line_is_kept() {
    let case = Case::new("replaced");
    case.program(
        "claude",
        &format!("#!/bin/sh\necho \"{{\\\"type\\\":\\\"assistant\\\",\\\"text\\\":\\\"a $ANTHROPIC_AUTH_TOKEN b $ANTHROPIC_AUTH_TOKEN c\\\"}}\"\necho \"claude: $ANTHROPIC_AUTH_TOKEN\" >&2\necho '{RESULT}'\n"),
    );
    let o = case.adapter(&[]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let rec = case.base.join("work/rec");
    let events = fs::read_to_string(rec.join("events.jsonl")).unwrap();
    assert_eq!(events, format!("{{\"type\":\"assistant\",\"text\":\"a <gateway token> b <gateway token> c\"}}\n{RESULT}\n"));
    assert_eq!(fs::read_to_string(rec.join("stderr.txt")).unwrap(), "claude: <gateway token>\n");
}

#[test]
fn a_token_with_a_quote_or_a_backslash_is_replaced_also_where_the_program_printed_it_as_json_text() {
    let case = Case::new("replaced-json");
    let token = r#"qu"ote\back-0123"#;
    case.program(
        "claude",
        &format!("#!/bin/sh\njq -cn --arg t \"$ANTHROPIC_AUTH_TOKEN\" '{{type: \"user\", tool_result: $t}}'\necho \"claude: $ANTHROPIC_AUTH_TOKEN\" >&2\necho '{RESULT}'\n"),
    );
    let o = case.adapter_of(Some(token), &[]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let escaped = serde_json::to_string(token).unwrap();
    let escaped = &escaped[1..escaped.len() - 1];
    assert_ne!(escaped, token);
    let mut hits = Vec::new();
    case.holders(&case.base.join("work"), &[token, escaped], &mut hits);
    assert_eq!(hits, Vec::<String>::new(), "the token the program printed, as text or as JSON text, is kept in the records");
    assert_eq!(fs::read_to_string(case.base.join("work/rec/events.jsonl")).unwrap().lines().next(), Some(r#"{"type":"user","tool_result":"<gateway token>"}"#));
}

#[test]
fn guard_no_process_alive_during_the_claude_call_holds_the_token_in_its_argv() {
    let case = Case::new("argv-scan");
    let log = case.base.join("log/argv-hits");
    // `[1]` in the pattern: the scan's own argv does not match its own pattern.
    let pattern = TOKEN.replacen("1", "[1]", 1);
    case.program("claude", &format!("#!/bin/sh\ngrep -l -a -e '{pattern}' /proc/[0-9]*/cmdline 2>/dev/null > '{}'\necho '{RESULT}'\n", log.display()));
    let o = case.adapter(&[]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let hits = fs::read_to_string(&log).expect("claude was not started");
    assert_eq!(hits.trim(), "", "a process holds the token in its argv while claude runs");
}

/// What the Rust reader says of the bytes of a file, and what the shell reader says of the same file.
fn readers(dir: &Path, bytes: &[u8]) -> (bool, bool) {
    let file = dir.join("gateway");
    fs::write(&file, bytes).unwrap();
    let rust = std::str::from_utf8(bytes).map_err(|_| ()).and_then(|t| sdc_tools::gateway::parse(t).map(|_| ()).map_err(|_| ())).is_ok();
    let sh = Command::new("bash")
        .args(["-c", "source \"$1\" && sdc_gateway_read \"$2\"", "_"])
        .arg(tools().join("gateway.sh"))
        .arg(&file)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success();
    (rust, sh)
}

fn shell_takes(path: &Path) -> bool {
    Command::new("bash")
        .args(["-c", "source \"$1\" && sdc_gateway_read \"$2\"", "_"])
        .arg(tools().join("gateway.sh"))
        .arg(path)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success()
}

#[test]
fn guard_the_shell_reader_takes_two_lines_and_refuses_a_missing_file_a_directory_and_every_malformed_pair() {
    let case = Case::new("shell-guard");
    let dir = case.base.join("work");
    let file = dir.join("gateway");
    let good = |text: &str| {
        fs::write(&file, text).unwrap();
        shell_takes(&file)
    };
    assert!(good("base_url=http://127.0.0.1:8080\ntoken=tok-12345\n"));
    assert!(good("base_url=https://h/v1\ntoken=tok-12345"), "no end of line at the end");
    assert!(good("base_url=http://h\ntoken=tok-1234\n"), "a token of 8 bytes");
    assert!(good("base_url=http://h\n"), "the address alone: a gateway without a token");
    assert!(!good("base_url=http://h\n\n"), "the address and a blank line");
    for (what, text) in [
        ("a third line", "base_url=http://h\ntoken=tok-12345\nextra=1\n"),
        ("a blank third line", "base_url=http://h\ntoken=tok-12345\n\n"),
        ("the lines swapped", "token=tok-12345\nbase_url=http://h\n"),
        ("no scheme", "base_url=h:80\ntoken=tok-12345\n"),
        ("another scheme", "base_url=ftp://h\ntoken=tok-12345\n"),
        ("an empty address", "base_url=\ntoken=tok-12345\n"),
        ("an empty token", "base_url=http://h\ntoken=\n"),
        ("a blank in the token", "base_url=http://h\ntoken=to k\n"),
        ("a tab in the address", "base_url=http://h\t/x\ntoken=tok-12345\n"),
        ("a first key that is none of the two", "url=http://h\ntoken=tok-12345\n"),
        ("a second key that is none of the two", "base_url=http://h\nauth=tok-12345\n"),
        ("a carriage return", "base_url=http://h\ntoken=tok-12345\r\n"),
        ("a token of 7 bytes", "base_url=http://h\ntoken=tok-123\n"),
    ] {
        assert!(!good(text), "the shell reader took a file with {what}");
    }
    assert!(!shell_takes(&dir.join("absent")), "the shell reader took a missing file");
    assert!(!shell_takes(&dir), "the shell reader took a directory");
}

#[test]
fn the_shell_reader_refuses_every_file_the_rust_reader_refuses() {
    let case = Case::new("reader-parity");
    let dir = case.base.join("work");
    let good = |base: &str, token: &str| format!("base_url={base}\ntoken={token}\n").into_bytes();
    let long = "x".repeat(sdc_tools::gateway::TOKEN_LIMIT + 1);
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("a good file", good(BASE, "tok-12345")),
        ("no end of line at the end", b"base_url=http://h\ntoken=tok-12345".to_vec()),
        ("a NUL inside the token", b"base_url=http://h\ntoken=ab\0cd\n".to_vec()),
        ("a non-ASCII token", "base_url=http://h\ntoken=t\u{f6}k\n".as_bytes().to_vec()),
        ("a token longer than the bound", good("http://h", &long)),
        ("an address without a host", good("http:///path", "tok-12345")),
        ("an address with only a port", good("http://:80", "tok-12345")),
        ("an address with user information", good("http://u:p@h", "tok-12345")),
        ("an address with a query", good("http://h/?k=v", "tok-12345")),
        ("an address with a fragment", good("http://h/#f", "tok-12345")),
        ("an address with a NUL", b"base_url=http://h\0x\ntoken=tok-12345\n".to_vec()),
        ("an upper case scheme", good("HTTP://h", "tok-12345")),
        ("a token of 7 bytes", good(BASE, "tok-123")),
        ("a token of 8 bytes", good(BASE, "tok-1234")),
        ("a third line", b"base_url=http://h\ntoken=tok-12345\nextra=1\n".to_vec()),
        ("a blank third line", b"base_url=http://h\ntoken=tok-12345\n\n".to_vec()),
        ("the lines in the other order", b"token=tok-12345\nbase_url=http://h\n".to_vec()),
        ("carriage returns", b"base_url=http://h\r\ntoken=tok-12345\r\n".to_vec()),
    ];
    let mut wider = Vec::new();
    for (what, bytes) in &cases {
        let (rust, sh) = readers(&dir, bytes);
        if sh && !rust {
            wider.push(*what);
        }
    }
    assert_eq!(wider, Vec::<&str>::new(), "the shell reader takes files that the Rust reader refuses");
}
