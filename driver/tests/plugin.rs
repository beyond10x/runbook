//! The `runbook` plugin (`plugin/`, `.claude-plugin/marketplace.json`): its manifests carry the
//! repository version, its marketplace entry points at `./plugin`, every skill has a description
//! that says when to use it and when not, and every repository path a `SKILL.md` names exists.
//! Its delegation parts (story `claude-plugin-serves-delegation`): the hook script that denies
//! every Agent call, run on recorded hook input, and the hook and MCP configuration that start it.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn json(path: &Path) -> Value {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The first `version = "…"` line of a Cargo manifest.
fn cargo_version(manifest: &str) -> Option<String> {
    manifest.lines().find_map(|l| l.strip_prefix("version = \"")?.strip_suffix('"').map(String::from))
}

/// The manifests whose `version` differs from `want`, as `path: version`.
fn version_drift(want: &str, manifests: &[(String, Value)]) -> Vec<String> {
    manifests
        .iter()
        .filter(|(_, v)| v["version"].as_str() != Some(want))
        .map(|(p, v)| format!("{p}: {}", v["version"].as_str().unwrap_or("no version")))
        .collect()
}

/// Repository paths a text names: a token that starts with a top-level entry of the repository
/// followed by `/`, or is a top-level file. A `:line` suffix, trailing punctuation, tokens
/// holding a placeholder (`<`, `*`, `{`, `$`) and a format id such as `protocol/1` are not paths
/// to check.
fn repo_paths(text: &str, top: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for raw in text.split(|c: char| c.is_whitespace() || "`()\"',;[]".contains(c)) {
        let mut t = raw.trim_end_matches(['.', ':']);
        if t.is_empty() || t.contains(['<', '>', '*', '{', '$', '=']) {
            continue;
        }
        let version_of_a_format = t.rsplit_once('/').is_some_and(|(_, v)| !v.is_empty() && v.chars().all(|c| c.is_ascii_digit()));
        if version_of_a_format {
            continue;
        }
        if let Some((head, tail)) = t.rsplit_once(':') {
            if tail.chars().all(|c| c.is_ascii_digit() || c == '-') && !tail.is_empty() {
                t = head;
            }
        }
        let first = t.split('/').next().unwrap_or("");
        let named = top.iter().any(|e| e == first);
        if named && (t.contains('/') || Path::new(first).extension().is_some()) {
            out.push(t.to_string());
        }
    }
    out.sort();
    out.dedup();
    out
}

fn top_entries() -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(root())
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| !n.starts_with('.'))
        .collect();
    v.extend(["runs".to_string()]);
    v
}

fn skills() -> Vec<PathBuf> {
    let dir = root().join("plugin/skills");
    let mut v: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(|e| Some(e.ok()?.path()))
        .filter(|p| p.is_dir())
        .collect();
    v.sort();
    v
}

fn frontmatter(text: &str) -> serde_yaml::Value {
    let body = text.strip_prefix("---\n").expect("SKILL.md starts with ---");
    let (fm, _) = body.split_once("\n---\n").expect("frontmatter closes with ---");
    serde_yaml::from_str(fm).expect("frontmatter is YAML")
}

#[test]
fn manifests_carry_the_repository_version() {
    let version = cargo_version(&fs::read_to_string(root().join("driver/Cargo.toml")).unwrap()).unwrap();
    let manifests: Vec<(String, Value)> = [
        "plugin/.claude-plugin/plugin.json",
        "plugin/.codex-plugin/plugin.json",
    ]
    .iter()
    .map(|p| (p.to_string(), json(&root().join(p))))
    .collect();
    let drift = version_drift(&version, &manifests);
    assert!(drift.is_empty(), "plugin manifests not at {version}: {drift:?}");
}

#[test]
fn version_drift_is_found() {
    let ms = vec![
        ("a".to_string(), serde_json::json!({"version": "0.1.4"})),
        ("b".to_string(), serde_json::json!({"version": "0.1.3"})),
        ("c".to_string(), serde_json::json!({})),
    ];
    assert_eq!(version_drift("0.1.4", &ms), vec!["b: 0.1.3", "c: no version"]);
}

#[test]
fn marketplace_lists_the_plugin_at_plugin_dir() {
    let m = json(&root().join(".claude-plugin/marketplace.json"));
    assert_eq!(m["name"], "runbook");
    let plugins = m["plugins"].as_array().expect("plugins");
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0]["name"], "runbook");
    assert_eq!(plugins[0]["source"], "./plugin");
    for p in ["plugin/.claude-plugin/plugin.json", "plugin/.codex-plugin/plugin.json"] {
        assert_eq!(json(&root().join(p))["name"], "runbook", "{p}");
    }
    let codex = json(&root().join("plugin/.codex-plugin/plugin.json"));
    assert_eq!(codex["skills"], "./skills/");
}

#[test]
fn manifests_carry_the_required_fields() {
    for p in ["plugin/.claude-plugin/plugin.json", "plugin/.codex-plugin/plugin.json"] {
        let m = json(&root().join(p));
        for k in ["name", "version", "description", "license", "repository"] {
            assert!(m[k].as_str().is_some_and(|s| !s.is_empty()), "{p}: `{k}` is missing or empty");
        }
        assert!(m["author"]["name"].as_str().is_some_and(|s| !s.is_empty()), "{p}: author.name");
    }
    let m = json(&root().join(".claude-plugin/marketplace.json"));
    assert!(m["owner"]["name"].as_str().is_some_and(|s| !s.is_empty()), "marketplace owner.name");
    assert!(m["plugins"][0]["description"].as_str().is_some_and(|s| !s.is_empty()));
}

#[test]
fn the_plugin_directory_is_the_one_home_of_the_skills() {
    assert!(!root().join("skills").exists(), "a second copy of the skills under skills/");
}

#[test]
fn every_skill_has_a_name_and_a_description_with_use_and_not() {
    let dirs = skills();
    let names: Vec<String> = dirs.iter().map(|d| d.file_name().unwrap().to_string_lossy().into()).collect();
    assert_eq!(names, ["delegation", "harden", "record", "workflow-builder"]);
    for d in dirs {
        let text = fs::read_to_string(d.join("SKILL.md")).unwrap();
        let fm = frontmatter(&text);
        assert_eq!(fm["name"].as_str(), d.file_name().unwrap().to_str(), "{}", d.display());
        let desc = fm["description"].as_str().unwrap_or("");
        assert!(desc.contains("Use when"), "{}: description has no 'Use when'", d.display());
        assert!(desc.to_lowercase().contains("not for"), "{}: description has no 'Not for'", d.display());
    }
}

#[test]
fn every_repository_path_a_skill_names_exists() {
    let top = top_entries();
    let mut missing = Vec::new();
    for d in skills() {
        let text = fs::read_to_string(d.join("SKILL.md")).unwrap();
        for p in repo_paths(&text, &top) {
            if !root().join(&p).exists() {
                missing.push(format!("{}: {p}", d.file_name().unwrap().to_string_lossy()));
            }
        }
        for line in text.lines() {
            for tok in line.split(|c: char| c.is_whitespace() || "`()\"',;".contains(c)) {
                if let Some(r) = tok.strip_prefix("references/") {
                    let r = r.trim_end_matches(['.', ':']);
                    if !r.is_empty() && !d.join("references").join(r).exists() {
                        missing.push(format!("{}: references/{r}", d.file_name().unwrap().to_string_lossy()));
                    }
                }
            }
        }
    }
    assert!(missing.is_empty(), "paths named in a SKILL.md that do not exist: {missing:#?}");
}

#[test]
fn a_missing_path_is_found() {
    let top = vec!["docs".to_string(), "scripts".to_string(), "protocol".to_string(), "Taskfile.yml".to_string()];
    let text = "Run `scripts/gone.sh --x` and read docs/there.md:12-30, `docs/<id>.md`, a protocol/1 file, Taskfile.yml.";
    assert_eq!(repo_paths(text, &top), ["Taskfile.yml", "docs/there.md", "scripts/gone.sh"]);
}

#[test]
fn plugin_files_hold_no_home_path_and_no_email_address() {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out);
            } else {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(&root().join("plugin"), &mut files);
    walk(&root().join(".claude-plugin"), &mut files);
    let mut bad = Vec::new();
    for f in files {
        let text = fs::read_to_string(&f).unwrap();
        for what in leaks(&text) {
            bad.push(format!("{}: {what}", f.display()));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// A home-directory path (`/home/` and a path segment) or an email address in a text.
fn leaks(text: &str) -> Vec<&'static str> {
    let mut out = Vec::new();
    let home = text.match_indices("/home/").any(|(i, m)| {
        text[i + m.len()..].chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.')
    });
    if home {
        out.push("home path");
    }
    let email = text.match_indices('@').any(|(i, _)| {
        let before = text[..i].chars().next_back().is_some_and(|c| c.is_alphanumeric() || "._+-".contains(c));
        let domain: String = text[i + 1..].chars().take_while(|c| c.is_alphanumeric() || ".-".contains(*c)).collect();
        before && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
    });
    if email {
        out.push("email address");
    }
    out
}

#[test]
fn leaks_are_found() {
    let home = format!("see {}x/y", ["/ho", "me/"].concat());
    let mail = format!("mail a.b{}example.org now", '@');
    let neither = format!("`{}` alone, {}scope and a{}b are neither", ["/ho", "me/"].concat(), '@', '@');
    assert_eq!(leaks(&home), ["home path"]);
    assert_eq!(leaks(&mail), ["email address"]);
    assert!(leaks(&neither).is_empty());
}

// The delegation hook: `plugin/hooks/delegation.sh <event>`, fed the JSON Claude Code writes on its
// standard input (the documented `PreToolUse` and `SessionStart` input, recorded under
// `tests/fixtures/plugin/`).

const HOOK: &str = "plugin/hooks/delegation.sh";

fn fixture(name: &str) -> Vec<u8> {
    let p = root().join("driver/tests/fixtures/plugin").join(name);
    fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The first `name` found in a directory of `PATH`.
fn on_path(name: &str) -> PathBuf {
    let path = std::env::var_os("PATH").expect("PATH");
    std::env::split_paths(&path).map(|d| d.join(name)).find(|p| p.is_file()).unwrap_or_else(|| panic!("{name} not on PATH"))
}

/// A directory holding only `sed` and `tr`: the hook runs with it as its whole `PATH`, so a call
/// to any other program (a network client among them) fails.
fn narrow_path() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("plugin-hook-path-{}", std::process::id()));
    if !dir.is_dir() {
        fs::create_dir_all(&dir).unwrap();
        for tool in ["sed", "tr"] {
            std::os::unix::fs::symlink(on_path(tool), dir.join(tool)).unwrap();
        }
    }
    dir
}

/// Runs the hook with an empty environment but `PATH` (`narrow_path`) and `RUNBOOK_MODEL_ROUTE`
/// when given: its exit code and standard output.
fn hook(event: &str, input: &[u8], route: Option<&str>) -> (i32, String) {
    let mut cmd = Command::new(on_path("sh"));
    cmd.arg(root().join(HOOK)).arg(event).env_clear().env("PATH", narrow_path());
    if let Some(r) = route {
        cmd.env("RUNBOOK_MODEL_ROUTE", r);
    }
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let out = child.wait_with_output().unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8(out.stdout).expect("the hook writes UTF-8"))
}

/// The deny answer's reason; panics unless the output is exactly one `PreToolUse` deny object.
fn denied(out: &str) -> String {
    let v: Value = serde_json::from_str(out).unwrap_or_else(|e| panic!("the hook's output is not JSON ({e}): {out}"));
    let h = &v["hookSpecificOutput"];
    assert_eq!(h["hookEventName"], "PreToolUse", "{out}");
    assert_eq!(h["permissionDecision"], "deny", "{out}");
    h["permissionDecisionReason"].as_str().unwrap_or_else(|| panic!("no reason: {out}")).to_string()
}

#[test]
fn hook_denies_an_agent_call_naming_delegate_the_prompt_and_the_route() {
    let (code, out) = hook("pre-tool-use", &fixture("pre-tool-use-agent.json"), Some("qwen-local"));
    assert_eq!(code, 0, "{out}");
    let reason = denied(&out);
    assert!(reason.contains("`delegate`"), "{reason}");
    assert!(reason.contains("Find every caller of \"route_for\" in runbook/src \\ the daemon (prüfe)."), "{reason}");
    assert!(!reason.contains("Then list each"), "only the prompt's first line: {reason}");
    assert!(reason.contains("`qwen-local`") && reason.contains("RUNBOOK_MODEL_ROUTE"), "{reason}");
    assert!(!reason.contains("default route"), "{reason}");
}

#[test]
fn hook_names_the_daemons_default_route_when_the_session_has_none() {
    for route in [None, Some("")] {
        let (code, out) = hook("pre-tool-use", &fixture("pre-tool-use-agent.json"), route);
        assert_eq!(code, 0, "{out}");
        let reason = denied(&out);
        assert!(reason.contains("`delegate`"), "{reason}");
        assert!(reason.contains("the daemon's default route"), "{route:?}: {reason}");
        assert!(reason.contains("RUNBOOK_MODEL_ROUTE"), "{reason}");
    }
}

#[test]
fn hook_writes_a_route_with_quotes_and_backslashes_as_valid_json() {
    let route = "odd\"route\\x";
    let (_, out) = hook("pre-tool-use", &fixture("pre-tool-use-agent.json"), Some(route));
    assert!(denied(&out).contains(route), "{out}");
}

#[test]
fn hook_lets_a_call_of_another_tool_pass() {
    let (code, out) = hook("pre-tool-use", &fixture("pre-tool-use-bash.json"), Some("qwen-local"));
    assert_eq!((code, out.as_str()), (0, ""));
}

#[test]
fn hook_denies_input_it_cannot_read() {
    let agent = String::from_utf8(fixture("pre-tool-use-agent.json")).unwrap();
    let cut = &agent[..agent.find("\"tool_name\"").unwrap() + 8];
    let no_prompt = r#"{"hook_event_name":"PreToolUse","tool_name":"Agent","tool_input":{"description":"x"}}"#;
    let bad_bytes: &[u8] = b"{\"tool_name\":\"Agent\",\"tool_input\":{\"prompt\":\"caf\xe9\nnext\"}}";
    for input in [&b""[..], b"not json", cut.as_bytes(), no_prompt.as_bytes(), bad_bytes] {
        let (code, out) = hook("pre-tool-use", input, Some("qwen-local"));
        assert_eq!(code, 0, "{out}");
        let reason = denied(&out);
        assert!(reason.contains("`delegate`") && reason.contains("`qwen-local`"), "{reason}");
    }
}

#[test]
fn hook_cuts_a_long_first_line_at_200_characters() {
    let input = serde_json::json!({"tool_name": "Agent", "tool_input": {"prompt": format!("{}\nrest", "ä".repeat(300))}});
    let reason = denied(&hook("pre-tool-use", input.to_string().as_bytes(), None).1);
    let shown = reason.rsplit_once("The prompt begins: ").expect("the prompt's line").1;
    assert_eq!(shown, format!("{} ...", "ä".repeat(200)));
}

#[test]
fn hook_reads_pretty_printed_input() {
    let v: Value = serde_json::from_slice(&fixture("pre-tool-use-agent.json")).unwrap();
    let pretty = serde_json::to_string_pretty(&v).unwrap();
    let reason = denied(&hook("pre-tool-use", pretty.as_bytes(), None).1);
    assert!(reason.contains("Find every caller of \"route_for\""), "{reason}");
}

#[test]
fn session_start_says_delegate_the_route_and_the_catalog() {
    for (route, names) in [(Some("qwen-local"), "`qwen-local`"), (None, "the daemon's default route")] {
        let (code, out) = hook("session-start", &fixture("session-start.json"), route);
        assert_eq!(code, 0, "{out}");
        let v: Value = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}"));
        assert_eq!(v["hookSpecificOutput"]["hookEventName"], "SessionStart", "{out}");
        let text = v["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
        for want in ["`delegate`", "`describe_catalog`", names] {
            assert!(text.contains(want), "{want}: {text}");
        }
    }
}

#[test]
fn hook_names_no_network_program() {
    let text = fs::read_to_string(root().join(HOOK)).unwrap();
    let code: String = text.lines().filter(|l| !l.trim_start().starts_with('#')).collect::<Vec<_>>().join("\n");
    for word in ["curl", "wget", "nc ", "ncat", "socat", "/dev/tcp", "/dev/udp", "ssh", "telnet", "http:", "https:"] {
        assert!(!code.contains(word), "{HOOK} names `{word}`");
    }
}

#[test]
fn hooks_deny_agent_and_block_when_the_hook_fails() {
    let h = json(&root().join("plugin/hooks/hooks.json"));
    for (event, matcher, arg) in [("PreToolUse", Some("Agent"), "pre-tool-use"), ("SessionStart", None, "session-start")] {
        let groups = h["hooks"][event].as_array().unwrap_or_else(|| panic!("hooks.{event}"));
        assert_eq!(groups.len(), 1, "{event}");
        if let Some(m) = matcher {
            assert_eq!(groups[0]["matcher"], m, "{event}");
        }
        let one = &groups[0]["hooks"][0];
        assert_eq!(one["type"], "command", "{event}");
        assert_eq!(one["command"], "sh", "{event}");
        assert_eq!(one["args"], serde_json::json!(["${CLAUDE_PLUGIN_ROOT}/hooks/delegation.sh", arg]), "{event}");
        if event == "PreToolUse" {
            assert_eq!(one["onFailure"], "block");
        }
    }
    assert!(root().join(HOOK).is_file());
}

#[test]
fn mcp_configuration_starts_runbook_mcp_on_the_sessions_daemon() {
    let m = json(&root().join("plugin/.mcp.json"));
    let s = &m["mcpServers"]["runbook"];
    assert_eq!(s["command"], "runbook");
    assert_eq!(s["args"], serde_json::json!(["mcp"]));
    assert_eq!(s["env"]["RUNBOOK_DAEMON"], "${RUNBOOK_DAEMON}");
    let timeout = s["timeout"].as_u64().expect("a per-server timeout in milliseconds");
    assert!(timeout >= 1000, "Claude Code ignores a timeout under 1000 ms: {timeout}");
}

// Adversarial cases (story `claude-plugin-serves-delegation`, first part).

/// Runs the hook like `hook`, with `RUNBOOK_MODEL_ROUTE` as raw bytes: its exit code and the
/// bytes of its standard output.
fn hook_raw_route(event: &str, input: &[u8], route: &[u8]) -> (i32, Vec<u8>) {
    use std::os::unix::ffi::OsStrExt as _;
    let mut cmd = Command::new(on_path("sh"));
    cmd.arg(root().join(HOOK)).arg(event).env_clear().env("PATH", narrow_path());
    cmd.env("RUNBOOK_MODEL_ROUTE", std::ffi::OsStr::from_bytes(route));
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let out = child.wait_with_output().unwrap();
    (out.status.code().unwrap_or(-1), out.stdout)
}

/// The hook's own contract (`quote` in `delegation.sh`): text that is not UTF-8 becomes
/// "(not UTF-8)". A UTF-16 surrogate encoded as three bytes, an overlong encoding and a code
/// point above U+10FFFF are not UTF-8, and the hook copies all three into its answer.
#[test]
fn hook_answers_utf8_for_a_route_that_is_not_utf8() {
    let routes: [&[u8]; 3] = [b"a\xed\xa0\x80b", b"a\xe0\x80\x80b", b"a\xf4\x90\x80\x80b"];
    let mut bad = Vec::new();
    for route in routes {
        let (code, out) = hook_raw_route("pre-tool-use", &fixture("pre-tool-use-agent.json"), route);
        assert_eq!(code, 0);
        match String::from_utf8(out) {
            Ok(text) => assert!(denied(&text).contains("(not UTF-8)"), "{route:?}: {text}"),
            Err(_) => bad.push(format!("{route:?}")),
        }
    }
    assert!(bad.is_empty(), "the hook wrote bytes that are not UTF-8 for the routes {bad:?}");
}

/// The design (Part two): with the plugin on, the session's sub-agent work never runs on the
/// session's own model. Claude Code's `Workflow` tool "orchestrates many subagents in the
/// background" and runs each on the session's model unless the script names another
/// (code.claude.com/docs tools-reference and workflows). The `PreToolUse` matcher `Agent` is an
/// exact name and does not match `Workflow`, and the script lets any tool other than Agent pass.
#[test]
fn workflow_calls_are_denied_like_agent_calls() {
    let h = json(&root().join("plugin/hooks/hooks.json"));
    let matched: Vec<String> = h["hooks"]["PreToolUse"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|g| g["matcher"].as_str())
        .flat_map(|m| m.split(['|', ',']).map(|s| s.trim().to_string()).collect::<Vec<_>>())
        .collect();
    assert!(matched.iter().any(|m| m == "Workflow"), "no PreToolUse matcher names Workflow: {matched:?}");
    let input = br#"{"session_id":"s","hook_event_name":"PreToolUse","tool_name":"Workflow","tool_input":{"script":"agent('review the routes')"},"tool_use_id":"t"}"#;
    let (code, out) = hook("pre-tool-use", input, Some("qwen-local"));
    assert_eq!(code, 0, "{out}");
    assert!(denied(&out).contains("`delegate`"), "{out}");
}

/// Claude Code loads a `${VAR}` that is not set and has no `:-default` as its unexpanded text
/// (code.claude.com/docs mcp, "Missing environment variable"). A session without
/// `RUNBOOK_DAEMON` then starts `runbook mcp` with `RUNBOOK_DAEMON=${RUNBOOK_DAEMON}`, a URL it
/// refuses, instead of its "no daemon named" message.
#[test]
fn mcp_configuration_names_no_variable_without_a_default() {
    let m = json(&root().join("plugin/.mcp.json"));
    let mut bare = Vec::new();
    for (name, server) in m["mcpServers"].as_object().unwrap() {
        for (k, v) in server["env"].as_object().into_iter().flatten() {
            let v = v.as_str().unwrap_or("");
            let mut rest = v;
            while let Some(i) = rest.find("${") {
                let inner = &rest[i + 2..];
                let end = inner.find('}').unwrap_or(inner.len());
                let var = &inner[..end];
                if !var.contains(":-") && !var.starts_with("CLAUDE_") {
                    bare.push(format!("{name}.env.{k} = {v}"));
                }
                rest = &inner[end..];
            }
        }
    }
    assert!(bare.is_empty(), "variables Claude Code passes unexpanded when unset: {bare:?}");
}

/// The design (Part two) fills in "the prompt's first line". A prompt that opens with a line
/// break has a readable prompt, and the hook says it could not read it.
#[test]
fn hook_names_the_prompt_when_it_starts_with_a_line_break() {
    let input = serde_json::json!({"tool_name": "Agent", "tool_input": {"description": "d", "prompt": "\nRead the routes and list them."}});
    let reason = denied(&hook("pre-tool-use", input.to_string().as_bytes(), None).1);
    assert!(reason.contains("Read the routes and list them."), "{reason}");
}
