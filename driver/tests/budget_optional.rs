//! A money budget of one model call is optional: the two adapter programs of `bindings/tools/` pass
//! `--max-budget-usd` to the model CLI only when the request names `max_budget_usd`, and pass it as
//! it is when it does. The programs they start are stand-ins that write their argv and call no model.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

fn tools() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../bindings/tools")
}

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("budget-optional").join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// A stand-in for `claude` and for `metaharness`: it writes its argv, one entry a line, to `argv.txt`
/// beside it and prints the one `result` line the adapters read.
const STANDIN: &str = r#"#!/bin/sh
printf '%s\n' "$@" > "$(dirname "$(readlink -f "$0")")/argv.txt"
printf '%s\n' '{"type":"result","subtype":"success","is_error":false,"num_turns":1,"total_cost_usd":0,"duration_ms":1,"usage":{}}'
"#;

/// Starts `adapter` on a request made of `members` and the members every request has; returns what the
/// stand-in model program was given.
fn argv_of(adapter: &str, program: &str, tag: &str, members: Value, env: &[(&str, &str)]) -> Vec<String> {
    let d = scratch(&format!("{tag}-{program}"));
    let bin = d.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join(program), STANDIN).unwrap();
    fs::set_permissions(bin.join(program), fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(d.join("prompt.md"), "prompt\n").unwrap();
    fs::create_dir_all(d.join("cwd")).unwrap();
    let mut request = json!({"record_dir": d.join("rec"), "prompt_file": d.join("prompt.md"), "cwd": d.join("cwd"),
        "model": "claude-haiku-5-5", "max_turns": 3, "operations": ["file.read"], "node": "idea.dissect", "attempt": 1});
    for (k, v) in members.as_object().unwrap() {
        request[k] = v.clone();
    }
    fs::write(d.join("request.json"), request.to_string()).unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    let mut c = Command::new(tools().join(adapter));
    c.arg(d.join("request.json")).current_dir(&d).env_clear().env("PATH", path);
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c.output().unwrap();
    assert_eq!(o.status.code(), Some(0), "{adapter}: {}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    fs::read_to_string(bin.join("argv.txt")).unwrap().lines().map(str::to_string).collect()
}

fn value_after(argv: &[String], flag: &str) -> Option<String> {
    argv.iter().position(|a| a == flag).map(|i| argv[i + 1].clone())
}

#[test]
fn the_claude_cli_adapter_passes_a_budget_only_when_the_request_names_one() {
    let login = [("ANTHROPIC_API_KEY", "dummy")];
    for (tag, mode) in [("session", json!({})), ("request-mode", json!({"implement_mode": "request", "max_output_tokens": 100}))] {
        let mut none = mode.clone();
        let argv = argv_of("llm-adapter-claude-cli.sh", "claude", &format!("none-{tag}"), none.take(), &login);
        assert!(!argv.iter().any(|a| a == "--max-budget-usd"), "{tag}: no budget named, none passed: {argv:?}");
        assert_eq!(value_after(&argv, "--max-turns").as_deref(), Some("3"), "{tag}: {argv:?}");
        for (budget, want) in [(json!(0.25), "0.25"), (json!(2), "2"), (json!(0), "0")] {
            let mut named = mode.clone();
            named["max_budget_usd"] = budget;
            let argv = argv_of("llm-adapter-claude-cli.sh", "claude", &format!("named-{want}-{tag}"), named, &login);
            assert_eq!(value_after(&argv, "--max-budget-usd").as_deref(), Some(want), "{tag}: {argv:?}");
        }
    }
}

#[test]
fn the_metaharness_adapter_passes_a_budget_only_when_the_request_names_one() {
    let env = [("SDC_LLM_ENDPOINT", "http://127.0.0.1:9")];
    let argv = argv_of("llm-adapter-metaharness.sh", "metaharness", "none", json!({}), &env);
    assert!(!argv.iter().any(|a| a == "--max-budget-usd"), "no budget named, none passed: {argv:?}");
    assert_eq!(value_after(&argv, "--max-turns").as_deref(), Some("3"), "{argv:?}");
    for (budget, want) in [(json!(0.25), "0.25"), (json!(2), "2"), (json!(0), "0")] {
        let argv = argv_of("llm-adapter-metaharness.sh", "metaharness", &format!("named-{want}"), json!({"max_budget_usd": budget}), &env);
        assert_eq!(value_after(&argv, "--max-budget-usd").as_deref(), Some(want), "{argv:?}");
    }
}

/// `sdc-free-agent run --print` with stand-ins for the programs it looks up: whether the command of
/// the session carries `--max-budget-usd` and what `--max-usd` made of it.
fn free_agent_print(tag: &str, max_usd: Option<&str>) -> (i32, String) {
    let d = scratch(&format!("free-{tag}"));
    let bin = d.join("bin");
    fs::create_dir_all(&bin).unwrap();
    for p in ["claude", "ess", "slot", "bwrap"] {
        fs::write(bin.join(p), "#!/bin/sh\n").unwrap();
        fs::set_permissions(bin.join(p), fs::Permissions::from_mode(0o755)).unwrap();
    }
    fs::create_dir_all(d.join("toolchain")).unwrap();
    fs::create_dir_all(d.join("example")).unwrap();
    fs::write(d.join("prompt.md"), "prompt\n").unwrap();
    let mut c = Command::new(env!("CARGO_BIN_EXE_sdc-free-agent"));
    c.args(["run", "--run-dir"]).arg(d.join("run")).args(["--idea", "an idea", "--prompt"]).arg(d.join("prompt.md"))
        .arg("--example-spec").arg(d.join("example")).args(["--model", "m", "--effort", "high", "--max-wall-s", "60", "--max-turns", "9", "--print"])
        .arg("--toolchain").arg(d.join("toolchain"));
    if let Some(u) = max_usd {
        c.args(["--max-usd", u]);
    }
    c.env_clear().env("PATH", format!("{}:/usr/bin:/bin", bin.display())).env("HOME", &d);
    let o = c.output().unwrap();
    (o.status.code().unwrap_or(-1), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
}

#[test]
fn the_free_agent_passes_a_money_budget_only_when_one_is_named() {
    let (code, out) = free_agent_print("none", None);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("max-budget-usd") && out.contains("--max-turns"), "no budget named, none passed: {out}");
    for named in ["2.5", "0"] {
        let (code, out) = free_agent_print(named, Some(named));
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("--max-budget-usd") && out.contains(&format!("\"{named}\"")), "{named}: {out}");
    }
}
