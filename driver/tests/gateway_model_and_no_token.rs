//! The gateway as a gateway that routes by model name takes it (`docs/loom-run-settings.md`, "`[gateway]`"):
//! the model a node names reaches `claude` as `--model` under a gateway. Nothing here calls a model: `claude`
//! is a stand-in in POSIX sh that writes what it was given into a log directory beside the case's roots.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const TOKEN: &str = "gw-token-0e5b7d21c4a9";
const BASE: &str = "http://127.0.0.1:8080";

fn tools() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("bindings/tools")
}

fn fx() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/sdc/fixtures")
}

/// `<tmp>/gateway-model/<name>/` with `work/`, `bin/`, `log/` and `private/` (mode 700).
struct Case {
    base: PathBuf,
}

impl Case {
    fn new(name: &str) -> Case {
        let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gateway-model").join(name);
        let _ = fs::remove_dir_all(&base);
        for d in ["work", "bin", "log", "private"] {
            fs::create_dir_all(base.join(d)).unwrap();
        }
        fs::set_permissions(base.join("private"), fs::Permissions::from_mode(0o700)).unwrap();
        let case = Case { base };
        case.claude();
        case
    }

    /// The gateway file in the private directory, mode 600, with `text` as `runbook run` writes it.
    fn gateway_file(&self, text: &str) -> PathBuf {
        let p = self.base.join("private/gateway");
        fs::write(&p, text).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
        p
    }

    /// A `claude` that writes the address, which kind of auth token it got (the case's token is compared,
    /// never printed) and whether its config directory holds a login into `log/claude-env`, and its argv into
    /// `log/claude-argv`.
    fn claude(&self) {
        let log = self.base.join("log").display().to_string();
        let text = format!(
            r#"#!/bin/sh
log='{log}'
{{
  echo "base=${{ANTHROPIC_BASE_URL-unset}}"
  if [ "${{ANTHROPIC_AUTH_TOKEN-}}" = '{TOKEN}' ]; then echo token=gateway
  elif [ -n "${{ANTHROPIC_AUTH_TOKEN+x}}" ]; then echo "token=other"
  else echo token=unset; fi
  [ -n "${{ANTHROPIC_API_KEY+x}}" ] && echo api_key=set || echo api_key=unset
  [ -e "$CLAUDE_CONFIG_DIR/.credentials.json" ] && echo login=present || echo login=absent
}} > "$log/claude-env"
printf '%s\n' "$@" > "$log/claude-argv"
mkdir -p out && cp '{fixture}' out/dissection.yaml
echo '{{"type":"result","subtype":"success","is_error":false,"num_turns":1,"total_cost_usd":0,"duration_ms":1,"usage":{{}}}}'
"#,
            fixture = fx().join("dissection.yaml").display()
        );
        let p = self.base.join("bin/claude");
        fs::write(&p, text).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn logged(&self, name: &str) -> String {
        fs::read_to_string(self.base.join("log").join(name)).unwrap_or_else(|e| panic!("{name}: {e} (claude was not started)"))
    }

    /// The model step as Loom starts it, `PATH` and `LOOM_CASE` alone, with `--model <model>` and the gateway file.
    fn node(&self, model: &str, gateway_file: &Path) -> (Option<i32>, String) {
        let work = self.base.join("work");
        fs::write(work.join("idea.yaml"), fs::read_to_string(fx().join("idea.yaml")).unwrap()).unwrap();
        let bins = Path::new(env!("CARGO_BIN_EXE_sdc-findings")).parent().unwrap();
        let path = format!("{}:{}:{}", self.base.join("bin").display(), bins.display(), std::env::var("PATH").unwrap_or_default());
        let o = Command::new(env!("CARGO_BIN_EXE_sdc-node"))
            .args(["idea.dissect", "--tools-dir"])
            .arg(tools())
            .args(["--model", model, "--max-turns", "15", "--adapter", "claude-cli", "--gateway-file"])
            .arg(gateway_file)
            .args(["--instructions", r#"{"task":"instructions/idea.dissect.md"}"#, "--stub", "stub-file", "dissection.yaml", "dissection.yaml"])
            .env_clear()
            .env("PATH", path)
            .env("LOOM_CASE", "IDEA-1.intake")
            .current_dir(&work)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        (o.status.code(), String::from_utf8_lossy(&o.stderr).into_owned())
    }
}

/// The value after `--model` in the argv `claude` recorded, one argument per line.
fn model_of(argv: &str) -> Option<String> {
    let args: Vec<&str> = argv.lines().collect();
    args.windows(2).find(|w| w[0] == "--model").map(|w| w[1].to_owned())
}

#[test]
fn a_node_under_a_gateway_hands_claude_the_model_it_names() {
    let case = Case::new("model");
    let file = case.gateway_file(&format!("base_url={BASE}\ntoken={TOKEN}\n"));
    let (code, stderr) = case.node("qwen", &file);
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(model_of(&case.logged("claude-argv")).as_deref(), Some("qwen"));
    let env = case.logged("claude-env");
    assert!(env.contains(&format!("base={BASE}\n")) && env.contains("token=gateway\n"), "{env}");
}
