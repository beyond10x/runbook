//! The gateway of a run (`docs/loom-run-settings.md`, "`[gateway]`"): an address and a token for the model
//! programs, given by the settings file and handed over through one file in the runner's private directory
//! (`<XDG_RUNTIME_DIR>/runbook/<run>/gateway`, mode 0600, in a directory of mode 0700) and one option,
//! `--gateway-file`, in each model node of the copy of the bindings. The token is never in an argv, an
//! environment, a log line, an event, the run directory or the copy; the directory is gone when the run
//! ends, also when it fails; and the refusals name the member, never a value.
//!
//! No test calls a model or the network: `sdc-node` and `claude` are stand-ins in POSIX sh that sit first on
//! the `PATH` of the world, and the token is a string the case invents. The adapter that starts `claude` is
//! the committed `bindings/tools/llm-adapter-claude-cli.sh`.

use super::core::{code, grep_tree, stderr, toml_text, World};
use super::fake::tree_run_json;
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const TOKEN: &str = "gw-token-4be7a1d9c350";
const BASE: &str = "http://127.0.0.1:8080";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn private_dir(w: &World, run: &str) -> PathBuf {
    w.root.join("xdg").join("runbook").join(run)
}

fn gateway() -> Value {
    json!({"base_url": BASE, "token": TOKEN})
}

fn scripted() -> Value {
    json!({"kind": "scripted", "max_call_micro_usd": 0, "answers": [{"approve": true, "principal": "person:owner", "cost_micro_usd": 0, "model_calls": 0}]})
}

/// The settings file of the world: a scripted decider, no login file, and `extra`.
fn settings(w: &World, extra: Value) {
    let mut v = json!({"decider": scripted()});
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    std::fs::write(w.root.join("settings.toml"), toml_text(&v)).unwrap();
}

fn executable(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn log(w: &World, name: &str) -> PathBuf {
    w.root.join("log").join(name)
}

/// A `claude` that writes which of the variables it was given and whether its config directory holds a login
/// into `log/claude-env`, and its argv into `log/claude-argv`.
fn standin_claude(w: &World) {
    let dir = w.root.join("log");
    std::fs::create_dir_all(&dir).unwrap();
    let text = format!(
        r#"#!/bin/sh
log='{log}'
{{
  echo "base=${{ANTHROPIC_BASE_URL-unset}}"
  if [ "${{ANTHROPIC_AUTH_TOKEN-}}" = '{TOKEN}' ]; then echo token=gateway
  elif [ "${{ANTHROPIC_AUTH_TOKEN-}}" = 'runbook-gateway-no-token' ]; then echo token=placeholder
  elif [ -n "${{ANTHROPIC_AUTH_TOKEN-}}" ]; then echo token=other
  else echo token=unset; fi
  [ -n "${{ANTHROPIC_API_KEY+x}}" ] && echo api_key=set || echo api_key=unset
  [ -e "$CLAUDE_CONFIG_DIR/.credentials.json" ] && echo login=present || echo login=absent
}} > "$log/claude-env"
printf '%s\n' "$@" > "$log/claude-argv"
echo '{{"type":"result","subtype":"success","is_error":false,"num_turns":1,"total_cost_usd":0,"duration_ms":1,"usage":{{}}}}'
"#,
        log = dir.display()
    );
    executable(&w.root.join("claude"), &text);
}

/// An `sdc-node` that records its argv into `log/sdc-node-argv` and starts the committed claude-cli adapter
/// the way the real one does: the path of the `--gateway-file` option in `SDC_GATEWAY_FILE`, nothing else.
fn standin_sdc_node(w: &World) {
    let dir = w.root.join("log");
    let text = format!(
        r#"#!/bin/sh
log='{log}'
printf '%s\n' "$@" >> "$log/sdc-node-argv"
gw=; prev=
for a in "$@"; do [ "$prev" = --gateway-file ] && gw=$a; prev=$a; done
d="$PWD/work/node-call"; mkdir -p "$d/cwd"
echo prompt > "$d/prompt.md"
jq -n --arg d "$d" '{{record_dir: ($d + "/rec"), prompt_file: ($d + "/prompt.md"), cwd: ($d + "/cwd"), model: "m", max_turns: 1, max_budget_usd: 0.1, operations: ["file.read"], node: "node.write", attempt: 1}}' > "$d/request.json"
SDC_GATEWAY_FILE="$gw" '{adapter}' "$d/request.json" > "$d/response.json" 2> "$d/stderr.txt"
"#,
        log = dir.display(),
        adapter = repo().join("bindings/tools/llm-adapter-claude-cli.sh").display()
    );
    executable(&w.root.join("sdc-node"), &text);
}

/// The hook of the first `tree run`: looks at the private directory while the run is on, then plays Loom: it
/// starts `sdc-node` with the `--gateway-file` of the model node of the copy and `PATH` and `LOOM_CASE` alone.
fn look_and_start_the_node(w: &World, run: &str, token: &str) -> String {
    let p = private_dir(w, run).display().to_string();
    format!(
        "stat -c '%a' \"{p}\" > \"$D/private-mode\"\nstat -c '%a' \"{p}/gateway\" > \"$D/file-mode\"\nls -A \"{p}\" > \"$D/private-ls\"\n\
         if grep -qx 'base_url={BASE}' \"{p}/gateway\" && grep -qx 'token={token}' \"{p}/gateway\"; then echo yes > \"$D/file-content\"; fi\n\
         gw=$(grep -A1 -e '--gateway-file' bindings/sdc.yaml | tail -1 | sed \"s/^ *- //; s/['\\\"]//g\")\n\
         echo \"$gw\" > \"$D/option-value\"\n\
         env -i PATH=\"$PATH\" LOOM_CASE=root sdc-node node.write --gateway-file \"$gw\"\n"
    )
}

fn seen(w: &World, name: &str) -> String {
    std::fs::read_to_string(w.fake.dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}")).trim().to_owned()
}

fn value(text: &str, key: &str) -> String {
    text.lines().find_map(|l| l.strip_prefix(&format!("{key}="))).unwrap_or_else(|| panic!("no {key}= in {text}")).to_owned()
}

fn holders(dir: &Path, needle: &str) -> Vec<PathBuf> {
    let mut hits = Vec::new();
    grep_tree(dir, needle, &mut hits);
    hits
}

fn chain_world(tag: &str) -> World {
    let w = World::new(tag);
    w.one_decision_tree();
    standin_claude(&w);
    standin_sdc_node(&w);
    w
}

#[test]
fn a_gateway_reaches_claude_through_a_file_in_the_private_directory_and_leaves_no_token_behind() {
    let w = chain_world("gateway-chain");
    w.fake.record_env();
    settings(&w, json!({"gateway": gateway()}));
    w.fake.hook("tree_run", "1", &look_and_start_the_node(&w, "r1", TOKEN));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    // While the run was on: the directory has mode 0700 and holds the one file, mode 0600, with the two values.
    assert_eq!(seen(&w, "private-mode"), "700");
    assert_eq!(seen(&w, "file-mode"), "600");
    assert_eq!(seen(&w, "private-ls"), "gateway");
    assert_eq!(seen(&w, "file-content"), "yes", "the file does not hold the address and the token");
    // The copy names that file in the model node, and no login.
    let copy = std::fs::read_to_string(w.run_dir("r1/bindings/sdc.yaml")).unwrap();
    let file = private_dir(&w, "r1").join("gateway").display().to_string();
    assert!(copy.contains("--gateway-file") && copy.contains(&file), "{copy}");
    assert!(!copy.contains("--claude-credentials") && !copy.contains(TOKEN), "{copy}");
    assert_eq!(seen(&w, "option-value"), file);
    // `claude` got the two values and no login; the caller's key (the runner's own environment holds one) did not reach it.
    let env = std::fs::read_to_string(log(&w, "claude-env")).expect("claude was not started");
    assert_eq!(value(&env, "base"), BASE);
    assert_eq!(value(&env, "token"), "gateway", "{env}");
    assert_eq!((value(&env, "api_key").as_str(), value(&env, "login").as_str()), ("unset", "absent"));
    // The token is in no file of the run directory, no argv or environment of a call to Loom, no argv the
    // stand-ins recorded and no line the runner printed.
    assert_eq!(holders(&w.run_dir("r1"), TOKEN), Vec::<PathBuf>::new());
    assert_eq!(holders(&w.fake.dir.join("calls"), TOKEN), Vec::<PathBuf>::new());
    for name in ["sdc-node-argv", "claude-argv"] {
        let text = std::fs::read_to_string(log(&w, name)).unwrap();
        assert!(!text.contains(TOKEN), "{name} holds the token");
    }
    assert!(!String::from_utf8_lossy(&o.stdout).contains(TOKEN) && !stderr(&o).contains(TOKEN));
    // Loom's contract: nothing of the gateway in the environment of a program Loom starts.
    for call in w.fake.calls() {
        assert!(call.env().iter().all(|(k, v)| !k.starts_with("ANTHROPIC_") && !k.starts_with("SDC_GATEWAY") && !v.contains(TOKEN)), "{:?}", call.env());
    }
    // The record says only that a gateway was named; the directory is gone.
    let recorded: Value = serde_json::from_str(&std::fs::read_to_string(w.run_dir("r1/settings.json")).unwrap()).unwrap();
    assert_eq!(recorded["gateway"], "named");
    assert!(recorded.get("login_file").is_none());
    assert!(!private_dir(&w, "r1").exists(), "the private directory is left");
}

#[test]
fn a_token_in_a_file_is_read_from_the_file_and_neither_its_path_nor_its_line_end_is_kept() {
    let w = chain_world("gateway-token-file");
    let file = w.root.join("token-holder");
    std::fs::write(&file, format!("{TOKEN}\n")).unwrap();
    settings(&w, json!({"gateway": {"base_url": BASE, "token_file": file.display().to_string()}}));
    w.fake.hook("tree_run", "1", &look_and_start_the_node(&w, "r1", TOKEN));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(seen(&w, "file-content"), "yes", "the file holds the token without its line end");
    assert_eq!(value(&std::fs::read_to_string(log(&w, "claude-env")).unwrap(), "token"), "gateway");
    assert_eq!(holders(&w.run_dir("r1"), &file.display().to_string()), Vec::<PathBuf>::new(), "the path of the token file is in the run directory");
    assert_eq!(holders(&w.run_dir("r1"), TOKEN), Vec::<PathBuf>::new());
    assert!(!String::from_utf8_lossy(&o.stdout).contains(TOKEN) && !stderr(&o).contains(TOKEN));
    assert!(file.is_file(), "the token file of the user was removed");
}

#[test]
fn the_private_directory_and_its_file_are_gone_after_a_run_that_fails() {
    let w = World::new("gateway-failed");
    w.fake.answer("tree_open", "default", "", 1);
    let p = private_dir(&w, "r1").display().to_string();
    w.fake.hook("tree_open", "default", &format!("stat -c '%a' \"{p}/gateway\" > \"$D/had-file-mode\"\n"));
    settings(&w, json!({"gateway": gateway()}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 1, "{}", stderr(&o));
    assert_eq!(seen(&w, "had-file-mode"), "600", "the file did not exist while the run was on");
    assert!(!private_dir(&w, "r1").exists(), "the private directory is left after an error");
    assert_eq!(holders(&w.run_dir("r1"), TOKEN), Vec::<PathBuf>::new());
}

#[test]
fn the_private_directory_and_its_file_are_gone_after_sigterm() {
    let w = World::new("gateway-signal");
    w.fake.answer("tree_run", "1", "", 0);
    w.fake.hook("tree_run", "1", "echo $$ > \"$D/pid\"\nsleep 30\n");
    settings(&w, json!({"gateway": gateway()}));
    let mut child = w.command(&w.args("r1", 1_000_000)).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let started = std::time::Instant::now();
    while !w.fake.dir.join("pid").exists() {
        assert!(started.elapsed().as_secs() < 20, "the fake tree run did not start");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(private_dir(&w, "r1").join("gateway").exists(), "the file is there while the run is on");
    assert!(Command::new("kill").args(["-TERM", &child.id().to_string()]).status().unwrap().success());
    let end = std::time::Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        assert!(end.elapsed().as_secs() < 8, "the runner did not end after SIGTERM");
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(7));
    assert!(!private_dir(&w, "r1").exists(), "the private directory is left after SIGTERM");
}

/// A run that is refused: exit 2, the member named, no value printed, nothing written, Loom never called.
fn refused(tag: &str, extra: Value, members: &[&str], absent: &[&str]) {
    refused_with(tag, extra, members, absent, &|_| {});
}

fn refused_with(tag: &str, extra: Value, members: &[&str], absent: &[&str], more: &dyn Fn(&mut Vec<String>)) {
    let w = World::new(tag);
    w.one_decision_tree();
    settings(&w, extra);
    let mut args = w.args("r1", 1_000_000);
    more(&mut args);
    let o = w.run(&args);
    let said = stderr(&o);
    assert_eq!(code(&o), 2, "{tag}: {said}");
    for m in members {
        assert!(said.contains(m), "{tag}: the refusal does not name {m}: {said}");
    }
    for value in absent.iter().chain(&[TOKEN]) {
        assert!(!said.contains(value) && !String::from_utf8_lossy(&o.stdout).contains(value), "{tag}: a value is in the refusal: {said}");
    }
    assert!(!w.run_dir("r1").exists(), "{tag}: a run directory was made");
    assert!(!private_dir(&w, "r1").exists(), "{tag}: a private directory was made");
    assert!(!w.ledger().exists(), "{tag}: the ledger was made");
    assert_eq!(w.fake.calls_so_far(), 0, "{tag}: Loom was called");
}

#[test]
fn a_gateway_table_without_a_base_url_is_refused_by_name() {
    refused("gateway-no-base", json!({"gateway": {"token": TOKEN}}), &["gateway.base_url"], &[]);
}

#[test]
fn a_gateway_with_both_a_token_and_a_token_file_is_refused_by_name() {
    refused("gateway-both", json!({"gateway": {"base_url": BASE, "token": TOKEN, "token_file": "/absolute/token-file"}}), &["gateway.token", "gateway.token_file"], &["/absolute/token-file"]);
}

#[test]
fn a_gateway_without_a_token_reaches_claude_with_the_address_and_the_placeholder_and_no_login() {
    // a gateway without ingress auth: neither `token` nor `token_file`
    let w = chain_world("gateway-no-token");
    settings(&w, json!({"gateway": {"base_url": BASE}}));
    let p = private_dir(&w, "r1").display().to_string();
    let hook = format!(
        "ls -A \"{p}\" > \"$D/private-ls\"\ncat \"{p}/gateway\" > \"$D/file-text\"\n\
         gw=$(grep -A1 -e '--gateway-file' bindings/sdc.yaml | tail -1 | sed \"s/^ *- //; s/['\\\"]//g\")\n\
         env -i PATH=\"$PATH\" LOOM_CASE=root sdc-node node.write --gateway-file \"$gw\"\n"
    );
    w.fake.hook("tree_run", "1", &hook);
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(seen(&w, "private-ls"), "gateway", "the private directory holds the gateway file alone, no login");
    assert_eq!(seen(&w, "file-text"), format!("base_url={BASE}"), "the file holds the address and no token line");
    let env = std::fs::read_to_string(log(&w, "claude-env")).expect("claude was not started");
    assert_eq!(value(&env, "base"), BASE);
    assert_eq!(value(&env, "token"), "placeholder", "{env}");
    assert_eq!((value(&env, "api_key").as_str(), value(&env, "login").as_str()), ("unset", "absent"));
    let recorded: Value = serde_json::from_str(&std::fs::read_to_string(w.run_dir("r1/settings.json")).unwrap()).unwrap();
    assert_eq!(recorded["gateway"], "named");
    assert!(!private_dir(&w, "r1").exists(), "the private directory is left");
}

#[test]
fn a_token_file_that_is_relative_missing_a_directory_empty_or_not_one_line_is_refused_by_name_and_value() {
    refused("gateway-tf-relative", json!({"gateway": {"base_url": BASE, "token_file": "relative/token-holder"}}), &["gateway.token_file"], &["relative/token-holder"]);
    refused("gateway-tf-missing", json!({"gateway": {"base_url": BASE, "token_file": "/nonexistent-dir/token-holder"}}), &["gateway.token_file"], &["nonexistent-dir"]);
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gateway-token-files-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let dir = root.join("a-directory");
    std::fs::create_dir_all(&dir).unwrap();
    refused("gateway-tf-directory", json!({"gateway": {"base_url": BASE, "token_file": dir.display().to_string()}}), &["gateway.token_file"], &[&dir.display().to_string()]);
    let empty = root.join("empty-holder");
    std::fs::write(&empty, "\n  \n").unwrap();
    refused("gateway-tf-empty", json!({"gateway": {"base_url": BASE, "token_file": empty.display().to_string()}}), &["gateway.token_file", "empty"], &[&empty.display().to_string()]);
    let two = root.join("two-lines-holder");
    std::fs::write(&two, "first-line-secret\nsecond-line-secret\n").unwrap();
    refused("gateway-tf-lines", json!({"gateway": {"base_url": BASE, "token_file": two.display().to_string()}}), &["gateway.token_file"], &["first-line-secret", "second-line-secret"]);
}

#[test]
fn an_empty_token_or_one_with_a_control_character_is_refused_by_name() {
    refused("gateway-token-empty", json!({"gateway": {"base_url": BASE, "token": "  "}}), &["gateway.token", "empty"], &[]);
    refused("gateway-token-newline", json!({"gateway": {"base_url": BASE, "token": "line-one-secret\nline-two-secret"}}), &["gateway.token"], &["line-one-secret", "line-two-secret"]);
}

#[test]
fn a_base_url_that_is_not_http_or_https_with_a_host_is_refused_by_name_and_never_printed() {
    for (tag, url) in [("ftp", "ftp://gw-host-name"), ("bare", "gw-host-name:8080"), ("no-host", "http://"), ("empty", ""), ("userinfo", "http://user:gw-secret-pass@gw-host-name"), ("query", "http://gw-host-name/?k=v")] {
        refused(&format!("gateway-url-{tag}"), json!({"gateway": {"base_url": url, "token": TOKEN}}), &["gateway.base_url"], &["gw-secret-pass", "gw-host-name"]);
    }
    refused("gateway-url-type", json!({"gateway": {"base_url": 8080, "token": TOKEN}}), &["gateway.base_url"], &[]);
}

#[test]
fn a_gateway_table_that_is_no_table_or_has_another_member_is_refused_by_name() {
    refused("gateway-text", json!({"gateway": "http://127.0.0.1:8080"}), &["gateway"], &["127.0.0.1"]);
    refused("gateway-extra", json!({"gateway": {"base_url": BASE, "token": TOKEN, "model": "x"}}), &["gateway.model"], &[]);
}

#[test]
fn a_gateway_with_a_login_file_in_either_spelling_is_refused_and_a_login_file_alone_works_as_before() {
    let login = "/nonexistent-login-dir/login-holder";
    refused("gateway-login", json!({"gateway": gateway(), "login_file": login}), &["gateway", "login_file"], &["nonexistent-login-dir"]);
    refused("gateway-login-old", json!({"gateway": gateway(), "loom": {"login_file": login}}), &["gateway", "login_file"], &["nonexistent-login-dir"]);
    // a login file alone: the run goes as it always did, no gateway file, the link in the private directory
    let w = World::new("gateway-login-alone");
    w.one_decision_tree();
    w.scripted_approve();
    let p = private_dir(&w, "r1").display().to_string();
    w.fake.hook("tree_run", "1", &format!("ls -A \"{p}\" > \"$D/private-ls\"\n"));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(seen(&w, "private-ls"), "login");
    assert!(!std::fs::read_to_string(w.run_dir("r1/bindings/sdc.yaml")).unwrap().contains("--gateway-file"));
}

#[test]
fn a_gateway_with_an_adapter_that_cannot_take_it_is_refused_by_the_setting_that_names_it() {
    refused("gateway-adapter", json!({"gateway": gateway(), "adapter": "metaharness"}), &["adapter", "metaharness"], &[]);
    let model_decider = json!({"kind": "model", "model": "claude-opus-5-5", "without_claims": true});
    refused("gateway-decider-default", json!({"gateway": gateway(), "decider": model_decider.clone(), "autonomy": "A5"}), &["decider.adapter"], &[]);
    let mut explicit = model_decider;
    explicit["adapter"] = json!("metaharness");
    refused("gateway-decider-metaharness", json!({"gateway": gateway(), "decider": explicit, "autonomy": "A5"}), &["decider.adapter"], &[]);
}

#[test]
fn a_gateway_under_the_substrate_level_is_refused_before_a_run_directory_exists() {
    refused_with("gateway-substrate", json!({"gateway": gateway()}), &["gateway", "substrate"], &[], &|args| {
        let i = args.iter().position(|a| a == "--confinement").unwrap();
        args[i + 1] = "substrate".to_owned();
    });
}

#[test]
fn a_dry_run_names_a_gateway_without_using_it_and_still_checks_its_shape() {
    let w = World::new("gateway-dry");
    let f = &w.fake;
    f.answer("tree_run", "1", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    settings(&w, json!({"gateway": {"base_url": BASE, "token_file": "/nonexistent-dir/token-holder"}, "decider": {"kind": "rule", "verdict": "approve"}, "autonomy": "A5"}));
    let mut args = w.args("r1", 5_000_000);
    args.push("--dry".into());
    let mut c = w.command(&args);
    c.env_remove("XDG_RUNTIME_DIR");
    let o = c.output().unwrap();
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(!w.root.join("xdg/runbook").exists(), "a dry run made a private directory");
    let copy = std::fs::read_to_string(w.run_dir("r1/bindings/sdc.yaml")).unwrap();
    assert!(!copy.contains("--gateway-file"), "{copy}");
    let recorded: Value = serde_json::from_str(&std::fs::read_to_string(w.run_dir("r1/settings.json")).unwrap()).unwrap();
    assert_eq!(recorded["gateway"], "named");
    refused_with("gateway-dry-shape", json!({"gateway": {"base_url": "ftp://x", "token": TOKEN}, "decider": {"kind": "rule", "verdict": "approve"}, "autonomy": "A5"}), &["gateway.base_url"], &[], &|args| args.push("--dry".into()));
}

#[test]
fn a_gateway_without_a_runtime_directory_is_refused_by_name_before_anything_is_written() {
    let w = World::new("gateway-no-xdg");
    w.one_decision_tree();
    settings(&w, json!({"gateway": gateway()}));
    let mut c = w.command(&w.args("r1", 1_000_000));
    c.env_remove("XDG_RUNTIME_DIR");
    let o = c.output().unwrap();
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(stderr(&o).contains("XDG_RUNTIME_DIR") && stderr(&o).contains("gateway") && !stderr(&o).contains(TOKEN), "{}", stderr(&o));
    assert!(!w.run_dir("r1").exists());
    assert_eq!(w.fake.calls_so_far(), 0);
}

#[test]
fn the_model_decider_starts_its_adapter_with_the_gateway_file_and_neither_a_login_nor_the_token() {
    let w = World::new("gateway-decider");
    super::decider::install_decider(&w, super::decider::APPROVE);
    super::decider::tree_with_approval(&w, "0.30");
    let mut v = super::decider::fixture_settings(&w, 5_000_000);
    v.as_object_mut().unwrap().remove("login_file");
    v["adapter"] = json!("claude-cli");
    v["gateway"] = gateway();
    super::decider::write_settings(&w, &v);
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let env = std::fs::read_to_string(w.run_dir("r1/bindings/tools/env.txt")).expect("the decider's adapter was not started");
    let file = private_dir(&w, "r1").join("gateway").display().to_string();
    assert!(env.contains(&format!("SDC_GATEWAY_FILE={file}\n")), "{env}");
    for absent in ["SDC_CLAUDE_CREDENTIALS", "ANTHROPIC_", TOKEN] {
        assert!(!env.contains(absent), "{absent} is in the decider adapter's environment: {env}");
    }
    assert_eq!(holders(&w.run_dir("r1"), TOKEN), Vec::<PathBuf>::new());
    assert!(!private_dir(&w, "r1").exists());
}

#[test]
fn under_a_gateway_the_model_decider_asks_its_adapter_for_the_model_decider_model_names() {
    // a gateway that routes by model name: `decider.model` is the name the decider's adapter is asked for,
    // and the adapter passes a request's model to `claude` as `--model` (driver/tests/gateway_model_and_no_token.rs)
    let w = World::new("gateway-decider-model");
    super::decider::install_decider(&w, super::decider::APPROVE);
    super::decider::tree_with_approval(&w, "0.30");
    let mut v = super::decider::fixture_settings(&w, 5_000_000);
    v.as_object_mut().unwrap().remove("login_file");
    v["adapter"] = json!("claude-cli");
    v["gateway"] = gateway();
    v["decider"]["model"] = json!("qwen");
    super::decider::write_settings(&w, &v);
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let request: Value = serde_json::from_str(&std::fs::read_to_string(w.run_dir("r1/bindings/tools/adapter-request.json")).expect("the decider's adapter was not started")).unwrap();
    assert_eq!(request["model"], "qwen", "{request}");
    let env = std::fs::read_to_string(w.run_dir("r1/bindings/tools/env.txt")).unwrap();
    assert!(env.contains("SDC_GATEWAY_FILE=") && !env.contains(TOKEN), "{env}");
}

