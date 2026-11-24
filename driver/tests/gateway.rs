//! The gateway of a run (`docs/loom-run-settings.md`, "`[gateway]`") as the programs of the bindings take it:
//! `sdc-node --gateway-file <file>` hands the file's path to the adapters (`SDC_GATEWAY_FILE`), and the
//! claude-cli adapter reads the file and gives `claude` the address and the token in the environment of that
//! one start, with a fresh config directory that holds no login. Nothing here calls a model: `claude` and
//! `metaharness` are stand-ins in POSIX sh, and the token is a string the case invents.
//!
//! A stand-in writes what it was given into a log directory beside the case's roots, never into the
//! workspace; the case reads it.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const TOKEN: &str = "gw-token-7c41e9d05b2a";
const OTHER_TOKEN: &str = "caller-token-3d8f1a66";
const BASE: &str = "http://127.0.0.1:8080";

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn tools() -> PathBuf {
    manifest().join("../bindings/tools")
}

fn fx() -> PathBuf {
    manifest().join("examples/sdc/fixtures")
}

/// `<tmp>/gateway/<name>/` with `work/`, `bin/`, `log/` and `private/` (mode 700).
struct Case {
    base: PathBuf,
}

impl Case {
    fn new(name: &str) -> Case {
        let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gateway").join(name);
        let _ = fs::remove_dir_all(&base);
        for d in ["work", "bin", "log", "private"] {
            fs::create_dir_all(base.join(d)).unwrap();
        }
        fs::set_permissions(base.join("private"), fs::Permissions::from_mode(0o700)).unwrap();
        Case { base }
    }

    fn work(&self) -> PathBuf {
        self.base.join("work")
    }

    fn log(&self) -> PathBuf {
        self.base.join("log")
    }

    fn private(&self) -> PathBuf {
        self.base.join("private")
    }

    fn program(&self, name: &str, text: &str) {
        let p = self.base.join("bin").join(name);
        fs::write(&p, text).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// The gateway file in the private directory, mode 600, holding `BASE` and `token`.
    fn gateway_file(&self, token: &str) -> PathBuf {
        let p = self.private().join("gateway");
        fs::write(&p, format!("base_url={BASE}\ntoken={token}\n")).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
        p
    }

    /// PATH of a program Loom starts: the stand-ins first, then the driver's binaries and the host's.
    fn path(&self) -> String {
        let bins = Path::new(env!("CARGO_BIN_EXE_sdc-findings")).parent().unwrap();
        format!("{}:{}:{}", self.base.join("bin").display(), bins.display(), std::env::var("PATH").unwrap_or_default())
    }

    fn logged(&self, name: &str) -> Option<String> {
        fs::read_to_string(self.log().join(name)).ok()
    }

    fn listing(&self, dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }
}

/// A `claude` that writes which of the variables it got and whether its config directory holds a login
/// into `log/claude-env` (the token is compared with the case's, never printed), its argv into
/// `log/claude-argv`, and the dissection the node expects. `exit` is its exit code.
fn claude(case: &Case, exit: i32) -> String {
    let log = case.log().display().to_string();
    format!(
        r#"#!/bin/sh
log='{log}'
{{
  echo "base=${{ANTHROPIC_BASE_URL-unset}}"
  if [ "${{ANTHROPIC_AUTH_TOKEN-}}" = '{TOKEN}' ]; then echo token=gateway
  elif [ -n "${{ANTHROPIC_AUTH_TOKEN-}}" ]; then echo token=other
  else echo token=unset; fi
  [ -n "${{ANTHROPIC_API_KEY+x}}" ] && echo api_key=set || echo api_key=unset
  [ -n "${{CLAUDE_CODE_OAUTH_TOKEN+x}}" ] && echo oauth=set || echo oauth=unset
  [ -e "$CLAUDE_CONFIG_DIR/.credentials.json" ] && echo login=present || echo login=absent
  echo "config=$CLAUDE_CONFIG_DIR"
}} > "$log/claude-env"
printf '%s\n' "$@" > "$log/claude-argv"
[ {exit} -eq 0 ] || exit {exit}
mkdir -p out && cp '{fixture}' out/dissection.yaml
echo '{{"type":"result","subtype":"success","is_error":false,"num_turns":1,"total_cost_usd":0,"duration_ms":1,"usage":{{}}}}'
"#,
        fixture = fx().join("dissection.yaml").display()
    )
}

fn value(text: &str, key: &str) -> String {
    text.lines().find_map(|l| l.strip_prefix(&format!("{key}="))).unwrap_or_else(|| panic!("no {key}= in {text}")).to_owned()
}

struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
    files: BTreeMap<String, String>,
}

fn files(work: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            files(work, &p, out);
        } else {
            out.insert(p.strip_prefix(work).unwrap().display().to_string(), String::from_utf8_lossy(&fs::read(&p).unwrap()).into_owned());
        }
    }
}

fn run(case: &Case, mut c: Command, stdin: &str) -> Run {
    let work = case.work();
    c.current_dir(&work).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = c.spawn().unwrap();
    child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    let mut f = BTreeMap::new();
    files(&work, &work, &mut f);
    Run { code: o.status.code(), stdout: String::from_utf8_lossy(&o.stdout).into_owned(), stderr: String::from_utf8_lossy(&o.stderr).into_owned(), files: f }
}

/// A request for an adapter, as sdc-node writes it.
fn adapter_request(case: &Case) -> PathBuf {
    let work = case.work();
    fs::write(work.join("prompt.md"), "prompt\n").unwrap();
    fs::create_dir_all(work.join("cwd")).unwrap();
    let request = serde_json::json!({"record_dir": work.join("rec"), "prompt_file": work.join("prompt.md"), "cwd": work.join("cwd"),
        "model": "claude-haiku-5-5", "max_turns": 1, "max_budget_usd": 0.1, "operations": ["file.read"], "node": "idea.dissect", "attempt": 1});
    fs::write(work.join("request.json"), request.to_string()).unwrap();
    work.join("request.json")
}

/// The adapter alone, with the environment `extra` adds to `PATH`.
fn adapter(case: &Case, name: &str, extra: &[(&str, &str)]) -> Run {
    let mut c = Command::new(tools().join(name));
    c.arg(adapter_request(case)).env_clear().env("PATH", case.path());
    for (k, v) in extra {
        c.env(k, v);
    }
    run(case, c, "")
}

/// The model step as Loom starts it: `PATH` and `LOOM_CASE` (and `extra`, the caller's own variables, which a
/// real Loom would not pass: the adapter must not take them either way).
fn node(case: &Case, options: &[&str], extra: &[(&str, &str)]) -> Run {
    fs::write(case.work().join("idea.yaml"), fs::read_to_string(fx().join("idea.yaml")).unwrap()).unwrap();
    let mut c = Command::new(env!("CARGO_BIN_EXE_sdc-node"));
    c.args(["idea.dissect", "--tools-dir"]).arg(tools());
    c.args(["--model", "claude-haiku-5-5", "--max-turns", "15", "--max-budget-usd", "0.5", "--adapter", "claude-cli"]);
    c.args(options);
    c.args(["--instructions", r#"{"task":"instructions/idea.dissect.md"}"#, "--stub", "stub-file", "dissection.yaml", "dissection.yaml"]);
    c.env_clear().env("PATH", case.path()).env("LOOM_CASE", "IDEA-1.intake");
    for (k, v) in extra {
        c.env(k, v);
    }
    run(case, c, "{}")
}

fn token_holders(r: &Run) -> Vec<String> {
    let mut holders: Vec<String> = r.files.iter().filter(|(_, text)| text.contains(TOKEN)).map(|(k, _)| k.clone()).collect();
    if r.stdout.contains(TOKEN) {
        holders.push("stdout".into());
    }
    if r.stderr.contains(TOKEN) {
        holders.push("stderr".into());
    }
    holders
}

const CALLER: [(&str, &str); 4] = [
    ("ANTHROPIC_API_KEY", "caller-api-key-marker"),
    ("CLAUDE_CODE_OAUTH_TOKEN", "caller-oauth-marker"),
    ("ANTHROPIC_AUTH_TOKEN", OTHER_TOKEN),
    ("ANTHROPIC_BASE_URL", "http://caller.invalid"),
];

#[test]
fn a_node_on_the_claude_cli_adapter_gives_claude_the_gateway_and_neither_a_login_nor_the_callers_key() {
    let case = Case::new("gateway-node");
    case.program("claude", &claude(&case, 0));
    let file = case.gateway_file(TOKEN);
    let r = node(&case, &["--gateway-file", file.to_str().unwrap()], &CALLER);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let seen = case.logged("claude-env").expect("claude was not started");
    assert_eq!(value(&seen, "base"), BASE);
    assert_eq!(value(&seen, "token"), "gateway", "{seen}");
    assert_eq!(value(&seen, "api_key"), "unset", "the caller's ANTHROPIC_API_KEY reached claude");
    assert_eq!(value(&seen, "oauth"), "unset", "the caller's oauth token reached claude");
    assert_eq!(value(&seen, "login"), "absent", "the config directory held a login file");
    // The config directory lay in the private directory of the gateway file, and is gone with the call.
    assert!(value(&seen, "config").starts_with(case.private().to_str().unwrap()), "{seen}");
    assert_eq!(case.listing(&case.private()), ["gateway"]);
    // The token is in no file of the workspace, no stream and no argv `claude` recorded.
    assert_eq!(token_holders(&r), Vec::<String>::new());
    assert!(!case.logged("claude-argv").unwrap().contains(TOKEN));
    assert!(!case.logged("claude-argv").unwrap().contains(BASE));
}

#[test]
fn the_claude_cli_adapter_takes_the_gateway_before_a_login_and_a_key_that_are_in_its_environment_too() {
    let case = Case::new("gateway-first");
    case.program("claude", &claude(&case, 0));
    let file = case.gateway_file(TOKEN);
    let home = case.base.join("home/.claude");
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join(".credentials.json"), "{}").unwrap();
    let mut extra: Vec<(&str, &str)> = CALLER.to_vec();
    let file_text = file.display().to_string();
    let home_text = case.base.join("home").display().to_string();
    let login_text = home.join(".credentials.json").display().to_string();
    extra.extend([("SDC_GATEWAY_FILE", file_text.as_str()), ("HOME", home_text.as_str()), ("SDC_CLAUDE_CREDENTIALS", login_text.as_str())]);
    let r = adapter(&case, "llm-adapter-claude-cli.sh", &extra);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let seen = case.logged("claude-env").expect("claude was not started");
    assert_eq!((value(&seen, "base").as_str(), value(&seen, "token").as_str()), (BASE, "gateway"), "{seen}");
    assert_eq!((value(&seen, "api_key").as_str(), value(&seen, "oauth").as_str(), value(&seen, "login").as_str()), ("unset", "unset", "absent"));
    assert_eq!(token_holders(&r), Vec::<String>::new());
    let argv = fs::read_to_string(case.work().join("rec/argv.json")).unwrap();
    assert!(!argv.contains(TOKEN), "the adapter recorded the token in argv.json");
}

#[test]
fn a_call_that_fails_leaves_only_the_gateway_file_in_the_private_directory() {
    let case = Case::new("gateway-failed-call");
    case.program("claude", &claude(&case, 3));
    let file = case.gateway_file(TOKEN);
    let r = node(&case, &["--gateway-file", file.to_str().unwrap()], &[]);
    assert_ne!(r.code, Some(0), "{}", r.stdout);
    assert!(case.logged("claude-env").is_some(), "claude was not started");
    assert_eq!(case.listing(&case.private()), ["gateway"]);
    assert_eq!(token_holders(&r), Vec::<String>::new());
}

#[test]
fn a_gateway_file_the_adapter_cannot_read_ends_it_with_127_before_claude_starts_and_names_no_value() {
    for (tag, make) in [("missing", None), ("malformed", Some("token only\n".to_owned())), ("no-token", Some(format!("base_url={BASE}\n"))), ("empty-token", Some(format!("base_url={BASE}\ntoken=\n")))] {
        let case = Case::new(&format!("gateway-unreadable-{tag}"));
        case.program("claude", &claude(&case, 0));
        let file = case.private().join("gateway");
        if let Some(text) = make {
            fs::write(&file, text + &format!("# {TOKEN}\n")).unwrap();
        }
        let r = adapter(&case, "llm-adapter-claude-cli.sh", &[("SDC_GATEWAY_FILE", file.to_str().unwrap()), ("ANTHROPIC_API_KEY", "caller-api-key-marker")]);
        assert_eq!(r.code, Some(127), "{tag}: {}", r.stderr);
        assert!(r.stderr.contains("SDC_GATEWAY_FILE"), "{tag}: {}", r.stderr);
        assert!(case.logged("claude-env").is_none(), "{tag}: claude was started with no gateway");
        assert!(!r.stderr.contains(TOKEN) && !r.stdout.contains(TOKEN), "{tag}: the token is in the output");
    }
}

#[test]
fn the_metaharness_adapter_refuses_a_gateway_and_starts_no_model_process() {
    let case = Case::new("gateway-metaharness");
    let mark = case.log().join("metaharness-ran");
    case.program("metaharness", &format!("#!/bin/sh\ntouch '{}'\nexit 0\n", mark.display()));
    let file = case.gateway_file(TOKEN);
    let r = adapter(&case, "llm-adapter-metaharness.sh", &[("SDC_GATEWAY_FILE", file.to_str().unwrap())]);
    assert_eq!(r.code, Some(127), "{}", r.stderr);
    assert!(r.stderr.contains("SDC_GATEWAY_FILE") && r.stderr.contains("claude-cli"), "{}", r.stderr);
    assert!(!mark.exists(), "metaharness was started under a gateway");
    assert!(!r.stderr.contains(TOKEN) && !r.stdout.contains(TOKEN));
}

#[test]
fn sdc_node_refuses_a_gateway_file_with_a_login_and_one_it_cannot_read_by_the_option_and_names_no_value() {
    let case = Case::new("gateway-node-refusals");
    case.program("claude", &claude(&case, 0));
    let file = case.gateway_file(TOKEN);
    let login = case.base.join("login-file");
    fs::write(&login, "{}").unwrap();
    let both = node(&case, &["--gateway-file", file.to_str().unwrap(), "--claude-credentials", login.to_str().unwrap()], &[]);
    assert_eq!(both.code, Some(64), "{}", both.stderr);
    assert!(both.stderr.contains("--gateway-file") && both.stderr.contains("--claude-credentials"), "{}", both.stderr);
    assert!(case.logged("claude-env").is_none());
    let missing = case.private().join("not-there");
    let r = node(&case, &["--gateway-file", missing.to_str().unwrap()], &[]);
    assert_eq!(r.code, Some(64), "{}", r.stderr);
    assert!(r.stderr.contains("--gateway-file") && !r.stderr.contains("not-there"), "{}", r.stderr);
    fs::write(&file, format!("junk {TOKEN}\n")).unwrap();
    let r = node(&case, &["--gateway-file", file.to_str().unwrap()], &[]);
    assert_eq!(r.code, Some(64), "{}", r.stderr);
    assert!(r.stderr.contains("--gateway-file") && !r.stderr.contains(TOKEN) && !r.stdout.contains(TOKEN), "{}", r.stderr);
    assert!(case.logged("claude-env").is_none(), "claude was started");
}

#[test]
fn sdc_node_hands_the_adapter_the_path_of_the_gateway_file_and_never_the_login_path_beside_it() {
    let case = Case::new("gateway-node-env");
    let seen = case.log().join("adapter-env");
    case.program(
        "claude",
        &format!("#!/bin/sh\nenv | grep -E '^(SDC_GATEWAY_FILE|SDC_CLAUDE_CREDENTIALS|ANTHROPIC_)' | sort > '{}'\nexit 3\n", seen.display()),
    );
    let file = case.gateway_file(TOKEN);
    let _ = node(&case, &["--gateway-file", file.to_str().unwrap()], &[("SDC_CLAUDE_CREDENTIALS", "/caller/login-path")]);
    let text = fs::read_to_string(&seen).unwrap();
    // claude itself is given the two variables of its own start; the path variable of the adapter is not exported on.
    assert!(!text.contains("SDC_CLAUDE_CREDENTIALS"), "{text}");
    assert!(!text.contains("SDC_GATEWAY_FILE"), "the gateway file's path reached claude: {text}");
    assert!(text.contains("ANTHROPIC_BASE_URL=") && text.contains("ANTHROPIC_AUTH_TOKEN="), "{text}");
}
