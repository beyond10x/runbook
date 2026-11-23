//! Cases of the claude-cli adapter's output filter, its call environment and descriptors, the two readers of the
//! gateway file and the lower bound of a token, beside `gateway.rs` and `gateway_hardening.rs`. Invented strings
//! only; `claude` is a stand-in in POSIX sh; no model, no network. Every wait on a process has a bound.

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TOKEN: &str = "gwk-gw-token-5e8d1c93b7f4";
const BASE: &str = "http://127.0.0.1:8080";
const RESULT: &str = r#"{"type":"result","subtype":"success","is_error":false,"num_turns":1,"total_cost_usd":0,"duration_ms":1,"usage":{}}"#;

fn tools() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../bindings/tools")
}

struct Case {
    base: PathBuf,
}

impl Case {
    fn new(name: &str) -> Case {
        let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gateway-readers").join(name);
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

    fn request(&self, schema: bool) -> PathBuf {
        let work = self.base.join("work");
        fs::write(work.join("prompt.md"), "prompt\n").unwrap();
        fs::create_dir_all(work.join("cwd")).unwrap();
        let mut request = serde_json::json!({"record_dir": work.join("rec"), "prompt_file": work.join("prompt.md"), "cwd": work.join("cwd"),
            "model": "claude-haiku-5-5", "max_turns": 1, "max_budget_usd": 0.1, "operations": ["file.read"], "node": "idea.dissect", "attempt": 1});
        if schema {
            fs::write(work.join("schema.json"), r#"{"type":"object"}"#).unwrap();
            request["json_schema"] = serde_json::json!(work.join("schema.json"));
        }
        fs::write(work.join("request.json"), request.to_string()).unwrap();
        work.join("request.json")
    }

    fn command(&self, token: &str, schema: bool, extra: &[(String, String)]) -> Command {
        let path = format!("{}:{}", self.base.join("bin").display(), std::env::var("PATH").unwrap_or_default());
        let mut c = Command::new(tools().join("llm-adapter-claude-cli.sh"));
        c.arg(self.request(schema)).env_clear().env("PATH", path).env("SDC_GATEWAY_FILE", self.gateway_file(token));
        for (k, v) in extra {
            c.env(k, v);
        }
        c.current_dir(self.base.join("work")).stdin(Stdio::null());
        c
    }

    fn adapter(&self, token: &str, schema: bool) -> std::process::Output {
        self.bounded(self.command(token, schema, &[]))
    }

    /// `c` run to its end for at most 40 s, its two streams in files of the case (no pipe a process left behind could
    /// hold open): a command that is still there is killed and the case fails.
    fn bounded(&self, mut c: Command) -> std::process::Output {
        let (out, err) = (self.base.join("log/bounded-stdout"), self.base.join("log/bounded-stderr"));
        c.stdout(fs::File::create(&out).unwrap()).stderr(fs::File::create(&err).unwrap());
        let mut child = c.spawn().unwrap();
        let start = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if start.elapsed().as_secs() >= 40 {
                let _ = child.kill();
                let _ = child.wait();
                panic!("the command did not end within 40 s");
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        std::process::Output { status, stdout: fs::read(&out).unwrap(), stderr: fs::read(&err).unwrap() }
    }

    /// The files under `dir` that hold any of `needles`.
    fn holders(&self, dir: &Path, needles: &[&str], out: &mut Vec<String>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if fs::symlink_metadata(&p).unwrap().file_type().is_symlink() {
                continue;
            }
            if p.is_dir() {
                self.holders(&p, needles, out);
            } else if fs::read(&p).map(|b| needles.iter().any(|n| b.windows(n.len()).any(|w| w == n.as_bytes()))).unwrap_or(false) {
                out.push(p.strip_prefix(&self.base).unwrap().display().to_string());
            }
        }
    }
}

fn real(name: &str) -> PathBuf {
    std::env::var("PATH").unwrap().split(':').map(|d| Path::new(d).join(name)).find(|p| p.is_file()).expect(name)
}

fn json_text(s: &str) -> String {
    let j = serde_json::to_string(s).unwrap();
    j[1..j.len() - 1].to_owned()
}

fn said(o: &std::process::Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

// ---- A(4): structured-output.json, the printed summary, argv.json ----

#[test]
fn guard_structured_output_the_printed_summary_and_argv_json_hold_no_token() {
    let case = Case::new("derived");
    case.program(
        "claude",
        &format!(
            "#!/bin/sh\njq -cn --arg t \"$ANTHROPIC_AUTH_TOKEN\" '{{type:\"assistant\",message:{{content:[{{type:\"tool_use\",name:\"Read\",input:{{file_path:$t,more:(\"x\"+$t)}}}}]}}}}'\n\
             jq -cn --arg t \"$ANTHROPIC_AUTH_TOKEN\" '{RESULT_OBJ} + {{structured_output:{{answer:$t,list:[$t,(\"a\"+$t+\"b\")],nested:(\"{{\\\"k\\\":\\\"\"+$t+\"\\\"}}\")}}}}'\n",
            RESULT_OBJ = r#"{type:"result",subtype:"success",is_error:false,num_turns:1,total_cost_usd:0,duration_ms:1,usage:{}}"#
        ),
    );
    let o = case.adapter(TOKEN, true);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    let rec = case.base.join("work/rec");
    let structured = fs::read_to_string(rec.join("structured-output.json")).expect("no structured-output.json: the schema path was not taken");
    assert!(structured.contains("<gateway token>"), "{structured}");
    let mut hits = Vec::new();
    case.holders(&case.base.join("work"), &[TOKEN], &mut hits);
    assert_eq!(hits, Vec::<String>::new(), "a file of the call holds the token");
    assert!(!said(&o).contains(TOKEN), "the printed summary or the error holds the token");
    assert!(!fs::read_to_string(rec.join("argv.json")).unwrap().contains(TOKEN));
}

// ---- A(2): forms of the token ----

#[test]
fn guard_a_token_of_metacharacters_adjacent_or_at_the_ends_of_a_line_is_replaced() {
    let case = Case::new("metacharacters");
    let token = "-a.b*c+d?[e](f)|g$^h{1}~&";
    case.program(
        "claude",
        &format!(
            "#!/bin/sh\njq -cn --arg t \"$ANTHROPIC_AUTH_TOKEN\" '{{a:($t+$t),b:(\"x\"+$t),c:($t[0:4]+$t),d:$t,e:($t+\"/\"+$t)}}'\n\
             printf '%s%s\\n' \"$ANTHROPIC_AUTH_TOKEN\" \"$ANTHROPIC_AUTH_TOKEN\" >&2\nprintf '%s' \"$ANTHROPIC_AUTH_TOKEN\" >&2\necho '{RESULT}'\n"
        ),
    );
    let o = case.adapter(token, false);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    let mut hits = Vec::new();
    case.holders(&case.base.join("work"), &[token, &json_text(token)], &mut hits);
    assert_eq!(hits, Vec::<String>::new(), "a form of the token with metacharacters is kept");
    let events = fs::read_to_string(case.base.join("work/rec/events.jsonl")).unwrap();
    let first: serde_json::Value = serde_json::from_str(events.lines().next().unwrap()).unwrap();
    assert_eq!(first["d"], "<gateway token>");
    assert_eq!(first["a"], "<gateway token><gateway token>");
    assert_eq!(first["b"], "x<gateway token>");
    assert_eq!(first["e"], "<gateway token>/<gateway token>");
    assert_eq!(fs::read_to_string(case.base.join("work/rec/stderr.txt")).unwrap(), "<gateway token><gateway token>\n<gateway token>\n");
}

/// What the filter leaves of a token that the program prints in a form other than the plain one or its JSON text.
/// Measured and reported as notes: the case passes when each of these forms is kept (the docs claim only the two).
#[test]
fn note_the_forms_the_filter_does_not_replace_are_exactly_the_documented_two_not_more() {
    let case = Case::new("other-forms");
    let b64 = "c2VjMi1ndy10b2tlbi01ZThkMWM5M2I3ZjQ=";
    assert_eq!(b64.len(), 36);
    let url = TOKEN.replace('-', "%2D");
    let uni = format!("\\u0073{}", &TOKEN[1..]);
    case.program(
        "claude",
        &format!(
            "#!/bin/sh\nt=\"$ANTHROPIC_AUTH_TOKEN\"\nh=$(printf '%s' \"$t\" | cut -c1-12)\nr=$(printf '%s' \"$t\" | cut -c13-)\n\
             printf '%s\\n' \"split: $h\"\nprintf '%s\\n' \"split: $r\"\nprintf '%s\\n' 'b64: {b64}' 'url: {url}' 'uni: {uni}' >&2\necho '{RESULT}'\n"
        ),
    );
    let o = case.adapter(TOKEN, false);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    let events = fs::read_to_string(case.base.join("work/rec/events.jsonl")).unwrap();
    let stderr = fs::read_to_string(case.base.join("work/rec/stderr.txt")).unwrap();
    // Two lines, each half of the token: no replacement can see the whole token.
    assert!(events.contains(&format!("split: {}", &TOKEN[..12])) && events.contains(&format!("split: {}", &TOKEN[12..])));
    assert!(stderr.contains(b64) && stderr.contains(&url) && stderr.contains(&uni));
}

#[test]
fn guard_a_token_that_is_a_prefix_of_the_replacement_text_cannot_make_the_filter_loop_or_leak() {
    let case = Case::new("prefix-of-replacement");
    let token = "<gateway"; // 8 bytes, visible
    case.program("claude", &format!("#!/bin/sh\necho 'a <gateway b <gateway token> c'\necho 'e' >&2\necho '{RESULT}'\n"));
    let o = case.adapter(token, false);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    let events = fs::read_to_string(case.base.join("work/rec/events.jsonl")).unwrap();
    assert_eq!(events.lines().next().unwrap(), "a <gateway token> b <gateway token> token> c");
}

// ---- A(3): a filter that dies midway ----

fn dying_jq(case: &Case, victim: u32) {
    let counter = case.base.join("log/rawfile-starts");
    let jq = real("jq");
    case.program(
        "jq",
        &format!(
            "#!/bin/sh\ncase \" $* \" in *\" --rawfile \"*)\n  if mkdir '{c}.lock' 2>/dev/null; then n=1; else n=2; fi\n  if [ \"$n\" = {victim} ]; then\n    IFS= read -r first\n    printf '%s\\n' \"$first\" | '{jq}' \"$@\"\n    kill -9 $$\n  fi ;;\nesac\nexec '{jq}' \"$@\"\n",
            c = counter.display(),
            jq = jq.display()
        ),
    );
}

fn filter_death(victim: u32, tag: &str) {
    let case = Case::new(tag);
    dying_jq(&case, victim);
    case.program(
        "claude",
        &format!("#!/bin/sh\nfor i in 1 2 3; do echo \"out $i $ANTHROPIC_AUTH_TOKEN\"; echo \"err $i $ANTHROPIC_AUTH_TOKEN\" >&2; done\necho '{RESULT}'\n"),
    );
    let o = case.adapter(TOKEN, true);
    assert_eq!(o.status.code(), Some(127), "{}", said(&o));
    let rec = case.base.join("work/rec");
    let mut left: Vec<String> = fs::read_dir(&rec).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    left.sort();
    assert_eq!(left, ["argv.json"], "a file of the call is left after the filter died: {left:?}");
    let mut hits = Vec::new();
    case.holders(&case.base.join("work"), &[TOKEN], &mut hits);
    assert_eq!(hits, Vec::<String>::new());
    assert!(!said(&o).contains(TOKEN));
    // nothing but the gateway file itself holds the token anywhere in the case
    let mut all = Vec::new();
    case.holders(&case.base, &[TOKEN], &mut all);
    all.retain(|p| !p.ends_with("private/gateway") && !p.starts_with("bin/") && !p.starts_with("log/"));
    assert_eq!(all, Vec::<String>::new());
}

#[test]
fn guard_the_events_filter_that_dies_after_writing_leaves_no_file() {
    filter_death(1, "die-events");
}

#[test]
fn guard_the_stderr_filter_that_dies_after_writing_leaves_no_file() {
    filter_death(2, "die-stderr");
}

// ---- B: the environment of the call ----

#[test]
fn guard_claude_has_no_prefixed_name_of_the_caller_whatever_its_form_and_how_many() {
    let case = Case::new("environ-names");
    let log = case.base.join("log/claude-environ");
    case.program("claude", &format!("#!/bin/sh\ncat /proc/self/environ > '{}'\necho '{RESULT}'\n", log.display()));
    let mut extra: Vec<(String, String)> = vec![
        ("ANTHROPIC_NL\nTAIL".into(), "v".into()),
        ("CLAUDE_CODE_NL\nTAIL".into(), "v".into()),
        ("ANTHROPIC_".into(), "v".into()),
        ("CLAUDE_CODE_".into(), "v".into()),
        ("CLAUDE_CODE_DISABLE_AUTO_MEMORY".into(), "0".into()),
        ("CLAUDE_CODE_DISABLE_GIT_INSTRUCTIONS".into(), "0".into()),
        ("CLAUDE_CONFIG_DIR".into(), "/caller-config-home".into()),
        ("ANTHROPIC_BASE_URL".into(), "http://caller.invalid".into()),
        ("ANTHROPIC_AUTH_TOKEN".into(), "caller-token-marker".into()),
        ("SDC_GATEWAY_BASE_URL".into(), "http://caller.invalid".into()),
        ("SDC_GATEWAY_TOKEN".into(), "caller-exported-marker".into()),
        ("SDC_CLAUDE_CREDENTIALS".into(), "/caller/credentials".into()),
        ("anthropic_lower".into(), "v".into()),
    ];
    for i in 0..10_000 {
        extra.push((format!("ANTHROPIC_BULK_{i}"), "v".into()));
    }
    let started = Instant::now();
    let o = case.bounded(case.command(TOKEN, false, &extra));
    let took = started.elapsed();
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    assert!(took.as_secs() < 30, "10,000 names took {took:?}");
    let environ = fs::read(&log).expect("claude did not start");
    let entries: Vec<String> = environ.split(|b| *b == 0).filter(|e| !e.is_empty()).map(|e| String::from_utf8_lossy(e).into_owned()).collect();
    let prefixed: BTreeSet<String> = entries.iter().filter(|e| e.starts_with("ANTHROPIC_") || e.starts_with("CLAUDE_CODE_")).cloned().collect();
    let want: BTreeSet<String> = [
        format!("ANTHROPIC_AUTH_TOKEN={TOKEN}"),
        format!("ANTHROPIC_BASE_URL={BASE}"),
        "CLAUDE_CODE_DISABLE_AUTO_MEMORY=1".to_owned(),
        "CLAUDE_CODE_DISABLE_GIT_INSTRUCTIONS=1".to_owned(),
    ]
    .into_iter()
    .collect();
    assert_eq!(prefixed, want, "claude holds a prefixed name of the caller, or lacks one of the gateway's and the adapter's own");
    let names: Vec<&str> = entries.iter().map(|e| e.split('=').next().unwrap()).collect();
    for gone in ["SDC_GATEWAY_BASE_URL", "SDC_GATEWAY_TOKEN", "SDC_GATEWAY_FILE", "SDC_CLAUDE_CREDENTIALS"] {
        assert!(!names.contains(&gone), "{gone} reached claude");
    }
    let config = entries.iter().find(|e| e.starts_with("CLAUDE_CONFIG_DIR=")).expect("claude has no config home");
    assert!(!config.contains("caller-config-home"), "the caller's config home reached claude");
}

// ---- A(1), B: no process but claude holds the token, at any sampled moment of the call ----

fn tree_of(root: u32) -> Vec<u32> {
    let mut parent: Vec<(u32, u32)> = Vec::new();
    if let Ok(dir) = fs::read_dir("/proc") {
        for e in dir.flatten() {
            let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() else { continue };
            // stat only: the argv and the environment are read for the processes of the tree alone.
            let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else { continue };
            let Some(rest) = stat.rsplit_once(')').map(|(_, r)| r) else { continue };
            if let Some(ppid) = rest.split_whitespace().nth(1).and_then(|p| p.parse::<u32>().ok()) {
                parent.push((pid, ppid));
            }
        }
    }
    let mut tree = vec![root];
    let mut i = 0;
    while i < tree.len() {
        let at = tree[i];
        for (pid, ppid) in &parent {
            if *ppid == at && !tree.contains(pid) {
                tree.push(*pid);
            }
        }
        i += 1;
    }
    tree
}

#[test]
fn no_process_of_the_call_but_claude_holds_the_token_in_argv_or_environment_at_any_sampled_moment() {
    let case = Case::new("sampled");
    let pidfile = case.base.join("log/claude-pid");
    let jq = real("jq");
    // A slow stand-in jq keeps the filters and the readers alive long enough to be sampled.
    case.program("jq", &format!("#!/bin/sh\nsleep 0.05\nexec '{}' \"$@\"\n", jq.display()));
    case.program("claude", &format!("#!/bin/sh\necho $$ > '{}'\necho \"out $ANTHROPIC_AUTH_TOKEN\"\necho \"err $ANTHROPIC_AUTH_TOKEN\" >&2\nexec sleep 1.5\n", pidfile.display()));
    let mut c = case.command(TOKEN, true, &[("SDC_GATEWAY_TOKEN".to_owned(), "caller-exported-marker".to_owned())]);
    c.stdout(Stdio::null()).stderr(Stdio::null());
    let mut child = c.spawn().unwrap();
    let root = child.id();
    let stop = Arc::new(AtomicBool::new(false));
    let seen: Arc<Mutex<BTreeSet<String>>> = Arc::default();
    let sampler = {
        let (stop, seen, base) = (stop.clone(), seen.clone(), case.base.clone());
        std::thread::spawn(move || {
            let mut samples = 0u32;
            let mut programs = BTreeSet::new();
            while !stop.load(Ordering::SeqCst) {
                for pid in tree_of(root) {
                    let comm = fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default().trim().to_owned();
                    for what in ["cmdline", "environ"] {
                        if let Ok(b) = fs::read(format!("/proc/{pid}/{what}")) {
                            if b.windows(TOKEN.len()).any(|w| w == TOKEN.as_bytes()) {
                                seen.lock().unwrap().insert(format!("{what} of {comm} #{pid}"));
                            }
                        }
                    }
                    programs.insert(comm);
                }
                // files of the case: only the gateway file itself may hold the token
                let mut files = Vec::new();
                walk(&base, &mut files);
                for f in files {
                    if f.ends_with("private/gateway") || f.starts_with(base.join("bin")) || f.starts_with(base.join("log")) {
                        continue;
                    }
                    if fs::read(&f).map(|b| b.windows(TOKEN.len()).any(|w| w == TOKEN.as_bytes())).unwrap_or(false) {
                        seen.lock().unwrap().insert(format!("file {}", f.strip_prefix(&base).unwrap().display()));
                    }
                }
                samples += 1;
                std::thread::sleep(Duration::from_millis(2));
            }
            seen.lock().unwrap().insert(format!("(samples {samples}; programs {programs:?})"));
        })
    };
    let status = child.wait().unwrap();
    stop.store(true, Ordering::SeqCst);
    sampler.join().unwrap();
    assert!(status.success());
    let seen = seen.lock().unwrap().clone();
    // The process that claude is (the stand-in writes its own pid, then execs): read once the call is over.
    let claude: Option<u32> = fs::read_to_string(&pidfile).ok().and_then(|t| t.trim().parse().ok());
    let seen: Vec<String> = seen.into_iter().filter(|s| claude.is_none_or(|c| !s.ends_with(&format!(" #{c}")))).collect();
    let (info, holders): (Vec<_>, Vec<_>) = seen.into_iter().partition(|s| s.starts_with('('));
    assert!(info[0].contains("jq") && info[0].contains("bash"), "the sampler did not see the filters: {info:?}");
    assert_eq!(holders, Vec::<String>::new(), "a process or a file of the call holds the token ({info:?})");
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        match fs::symlink_metadata(&p) {
            Ok(m) if m.is_dir() => walk(&p, out),
            Ok(m) if m.is_file() => out.push(p),
            _ => {}
        }
    }
}

// ---- C: the two readers agree ----

fn shell_takes(path: &Path, lc_all: &str) -> bool {
    let mut child = Command::new("bash")
        .args(["-c", "source \"$1\" && sdc_gateway_read \"$2\"", "_"])
        .arg(tools().join("gateway.sh"))
        .arg(path)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("LC_ALL", lc_all)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    loop {
        if let Some(s) = child.try_wait().unwrap() {
            return s.success();
        }
        if start.elapsed().as_secs() > 20 {
            let _ = child.kill();
            panic!("the shell reader did not end within 20 s");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn gw(base: &str, token: &str) -> Vec<u8> {
    format!("base_url={base}\ntoken={token}\n").into_bytes()
}

#[test]
fn the_two_readers_agree_on_every_form_of_a_gateway_file() {
    let case = Case::new("parity");
    let dir = case.base.join("work");
    let t8 = "tok-1234";
    let long_ok = "x".repeat(8192);
    let long_bad = "x".repeat(8193);
    let big_base = format!("http://h/{}", "p".repeat(20_000));
    let forms: Vec<(&str, Vec<u8>)> = vec![
        ("good", gw(BASE, t8)),
        ("https with a path", gw("https://h/a/b", t8)),
        ("no final end of line", b"base_url=http://h\ntoken=tok-12345".to_vec()),
        ("crlf", b"base_url=http://h\r\ntoken=tok-12345\r\n".to_vec()),
        ("a lone cr at the end of the token", b"base_url=http://h\ntoken=tok-12345\r".to_vec()),
        ("a blank line first", b"\nbase_url=http://h\ntoken=tok-12345\n".to_vec()),
        ("blank around the equal sign", b"base_url = http://h\ntoken = tok-12345\n".to_vec()),
        ("upper case keys", b"BASE_URL=http://h\nTOKEN=tok-12345\n".to_vec()),
        ("two base_url lines", b"base_url=http://h\nbase_url=http://i\ntoken=tok-12345\n".to_vec()),
        ("the lines swapped", b"token=tok-12345\nbase_url=http://h\n".to_vec()),
        ("an equal sign inside the token", gw("http://h", "a=b=c=d=e=f")),
        ("quotes, backticks, a dollar and a backslash in the token", gw("http://h", "a'b\"c`d$(e)\\f;g")),
        ("a token of 7 bytes", gw("http://h", "tok-123")),
        ("a token of 8192 bytes", gw("http://h", &long_ok)),
        ("a token of 8193 bytes", gw("http://h", &long_bad)),
        ("a tab in the token", gw("http://h", "tok\t12345")),
        ("a DEL in the token", gw("http://h", "tok\u{7f}12345")),
        ("a byte over 127 that is no UTF-8", [b"base_url=http://h\ntoken=tok".as_slice(), &[0x80], b"12345\n"].concat()),
        ("a two-byte character in the token", gw("http://h", "t\u{e9}k-12345")),
        ("a NUL in the token", b"base_url=http://h\ntoken=tok\0-12345\n".to_vec()),
        ("a NUL in the address", b"base_url=http://h\0x\ntoken=tok-12345\n".to_vec()),
        ("a NUL after the last end of line", b"base_url=http://h\ntoken=tok-12345\n\0".to_vec()),
        ("an empty file", Vec::new()),
        ("one end of line", b"\n".to_vec()),
        ("a byte order mark", [b"\xef\xbb\xbf".as_slice(), b"base_url=http://h\ntoken=tok-12345\n"].concat()),
        ("a trailing blank in the token", gw("http://h", "tok-12345 ")),
        ("a token of blanks", gw("http://h", "        ")),
        ("an address with an IPv6 host and a port", gw("http://[::1]:8080", t8)),
        ("an address with an IPv6 host and no port", gw("http://[::1]", t8)),
        ("an address with an IPv6 host and an empty port", gw("http://[::1]:", t8)),
        ("an IPv6 address that is not closed", gw("http://[::1", t8)),
        ("an IPv6 address with no host", gw("http://[]", t8)),
        ("an IPv6 address with a stray tail", gw("http://[::1]x", t8)),
        ("an IPv6 address with a letter in the port", gw("http://[::1]:80a", t8)),
        ("an empty port", gw("http://h:", t8)),
        ("a port with a letter", gw("http://h:80x", t8)),
        ("two ports", gw("http://h:80:80", t8)),
        ("a port over 65535", gw("http://h:99999", t8)),
        ("an upper case scheme", gw("HTTP://h", t8)),
        ("a scheme with one slash", gw("http:/h", t8)),
        ("an address with a user", gw("http://u@h", t8)),
        ("an address with a user and a port", gw("http://h:80@h", t8)),
        ("an address with an at sign in the path", gw("http://h/a@b", t8)),
        ("an address with a query mark at the end", gw("http://h/p?", t8)),
        ("an address with a fragment mark", gw("http://h#", t8)),
        ("an address with a blank", gw("http://h /x", t8)),
        ("an address with a non-ASCII character", gw("http://h\u{e9}", t8)),
        ("an address with a bracket inside a name", gw("http://h[x", t8)),
        ("an address of 20,000 bytes", gw(&big_base, t8)),
        ("an address with only a port", gw("http://:80", t8)),
        ("an address with no host and a path", gw("http:///p", t8)),
    ];
    assert!(forms.len() >= 30);
    let mut differ = Vec::new();
    for (what, bytes) in &forms {
        let file = dir.join("gateway");
        fs::write(&file, bytes).unwrap();
        let rust = sdc_tools::gateway::read(&file).is_ok();
        let c = shell_takes(&file, "C");
        let utf8 = shell_takes(&file, "en_US.utf8");
        if rust != c || c != utf8 {
            differ.push(format!("{what}: rust={rust} shell(C)={c} shell(en_US.utf8)={utf8}"));
        }
    }
    assert_eq!(differ, Vec::<String>::new(), "the readers disagree");
}

#[test]
fn an_address_of_2048_bytes_is_taken_and_one_of_2049_is_refused_by_both_readers_without_a_byte_of_it() {
    let case = Case::new("address-bound");
    let file = case.base.join("work/gateway");
    let address = |n: usize| format!("http://h/{}", "p".repeat(n - "http://h/".len()));
    assert_eq!(sdc_tools::gateway::ADDRESS_LIMIT, 2048);
    assert_eq!(sdc_tools::gateway::check_base_url(&address(2048)), Ok(()));
    assert_eq!(sdc_tools::gateway::check_base_url(&address(2049)), Err("is longer than 2048 bytes"));
    for (n, taken) in [(2048, true), (2049, false)] {
        fs::write(&file, gw(&address(n), "tok-12345")).unwrap();
        assert_eq!(sdc_tools::gateway::read(&file).is_ok(), taken, "the Rust reader, {n} bytes");
        for locale in ["C", "en_US.utf8"] {
            assert_eq!(shell_takes(&file, locale), taken, "the shell reader in {locale}, {n} bytes");
        }
    }
    let said_rust = sdc_tools::gateway::read(&file).unwrap_err();
    let mut reader = Command::new("bash");
    reader
        .args(["-c", "source \"$1\" && sdc_gateway_read \"$2\"", "_"])
        .arg(tools().join("gateway.sh"))
        .arg(&file)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default());
    let said_shell = said(&case.bounded(reader));
    assert!(!said_rust.contains("pppp") && !said_shell.contains("pppp"), "{said_rust} {said_shell}");
    assert!(!said_shell.is_empty(), "the shell reader refused without a word");
}

#[test]
fn the_shell_reader_refuses_the_kinds_of_file_that_are_no_regular_file_and_does_not_hang() {
    let case = Case::new("parity-kinds");
    let dir = case.base.join("work");
    let good = dir.join("good");
    fs::write(&good, gw(BASE, "tok-12345")).unwrap();
    let link = dir.join("link");
    symlink(&good, &link).unwrap();
    let fifo = dir.join("fifo");
    assert!(Command::new("mkfifo").arg(&fifo).status().unwrap().success());
    let dangling = dir.join("dangling");
    symlink(dir.join("absent"), &dangling).unwrap();
    for (what, path, expect) in [
        ("a link to a good file", &link, true),
        ("a dangling link", &dangling, false),
        ("a FIFO", &fifo, false),
        ("a directory", &dir, false),
        ("/dev/null", &PathBuf::from("/dev/null"), false),
        ("/dev/zero", &PathBuf::from("/dev/zero"), false),
    ] {
        assert_eq!(shell_takes(path, "C"), expect, "the shell reader and {what}");
    }
}

#[test]
fn the_rust_reader_ends_on_a_fifo_as_the_shell_reader_does() {
    let case = Case::new("rust-fifo");
    let fifo = case.base.join("work/fifo");
    assert!(Command::new("mkfifo").arg(&fifo).status().unwrap().success());
    let (tx, rx) = std::sync::mpsc::channel();
    let p = fifo.clone();
    std::thread::spawn(move || {
        let _ = tx.send(sdc_tools::gateway::read(&p).is_ok());
    });
    let answer = rx.recv_timeout(Duration::from_secs(5));
    // Let the blocked open end, whatever the answer, so that no thread outlives the case.
    let _ = fs::OpenOptions::new().write(true).custom_flags_nonblock().open(&fifo);
    assert_eq!(answer, Ok(false), "the Rust reader did not end within 5 s on a FIFO (the shell reader refuses it at once)");
}

trait NonBlock {
    fn custom_flags_nonblock(&mut self) -> &mut Self;
}

impl NonBlock for fs::OpenOptions {
    fn custom_flags_nonblock(&mut self) -> &mut Self {
        std::os::unix::fs::OpenOptionsExt::custom_flags(self, 0o4000)
    }
}

// ---- D: the lower bound ----

#[test]
fn no_refusal_of_a_short_token_prints_a_byte_of_it_or_its_length() {
    let case = Case::new("short");
    let dir = case.base.join("work");
    let marker = "zq9k"; // the only bytes the stand-in tokens share: none of them may come back
    for n in 1..8usize {
        let token: String = marker.chars().cycle().take(n).collect();
        // The Rust checks, inline and through the constructor.
        let e = sdc_tools::gateway::check_token(&token).unwrap_err();
        assert_eq!(e, "is shorter than 8 bytes", "{n} bytes");
        let e = sdc_tools::gateway::Gateway::new("http://h", &token).unwrap_err();
        assert_eq!(e, "gateway.token: is shorter than 8 bytes", "{n} bytes");
        // The Rust file reader and the shell reader.
        let file = dir.join("gateway");
        fs::write(&file, gw("http://h", &token)).unwrap();
        let e = sdc_tools::gateway::read(&file).unwrap_err();
        assert!(!e.contains(marker) && !e.contains(&format!("{n} byte")) && !e.contains(&n.to_string()), "{e}");
        let mut reader = Command::new("bash");
        reader
            .args(["-c", "source \"$1\" && sdc_gateway_read \"$2\"", "_"])
            .arg(tools().join("gateway.sh"))
            .arg(&file)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default());
        let sh = case.bounded(reader);
        assert!(!sh.status.success());
        let said = said(&sh);
        assert!(!said.contains(marker) && !said.contains(&n.to_string()), "{said}");
        // The adapter, with that file.
        let o = case.adapter(&token, false);
        assert_eq!(o.status.code(), Some(127));
        let said = said_of(&o);
        assert!(!said.contains(marker) && !said.contains(&format!("{n} byte")), "{said}");
    }
    // Bytes, not characters: a token of two characters of three bytes each is no short token, it is not visible text.
    assert_eq!(sdc_tools::gateway::check_token("\u{20ac}\u{20ac}"), Err("is not one line of visible text"));
    assert_eq!(sdc_tools::gateway::check_token("1234567"), Err("is shorter than 8 bytes"));
    assert_eq!(sdc_tools::gateway::check_token("12345678"), Ok(()));
}

fn said_of(o: &std::process::Output) -> String {
    said(o)
}

// ---- A(3): the descriptors of the filters ----

#[test]
fn claude_inherits_no_descriptor_of_the_filters_beyond_its_three_streams() {
    let case = Case::new("claude-fds");
    let log = case.base.join("log/claude-fds");
    case.program("claude", &format!("#!/bin/sh\nfor f in /proc/self/fd/*; do readlink \"$f\"; done > '{}'\necho '{RESULT}'\n", log.display()));
    let o = case.adapter(TOKEN, false);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    let fds = fs::read_to_string(&log).expect("claude did not start");
    let pipes = fds.lines().filter(|l| l.starts_with("pipe:")).count();
    // fd 1 is the log file here (the loop's redirect), fd 2 is the pipe of the stderr filter: one pipe and no other.
    assert_eq!(pipes, 1,"claude holds descriptors of the filters beyond its stdout and stderr: {fds}");
}

#[test]
fn a_process_that_claude_leaves_behind_does_not_hold_the_adapter_after_claude_ended() {
    let case = Case::new("claude-orphan");
    // The orphan closes its own 0, 1 and 2; only a descriptor it inherited beside them can keep a filter alive.
    case.program("claude", &format!("#!/bin/sh\nsh -c 'exec sleep 7' </dev/null >/dev/null 2>&1 &\necho '{RESULT}'\n"));
    let started = Instant::now();
    let o = case.adapter(TOKEN, false);
    let took = started.elapsed();
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    assert!(took.as_secs() < 4, "the adapter waited {took:?} for a process claude left behind");
}
