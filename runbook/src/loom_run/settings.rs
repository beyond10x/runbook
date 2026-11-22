//! The run settings file (JSON object) and its redacted copy.

use serde_json::Value;
use std::path::Path;

/// The key that names the path of the login file. Its value never reaches the run directory.
pub const LOGIN_KEY: &str = "login_file";

/// Reads the settings file: a JSON object.
pub fn read(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("--settings {}: {e}", path.display()))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("--settings {}: not JSON: {e}", path.display()))?;
    if !v.is_object() {
        return Err(format!("--settings {}: not a JSON object", path.display()));
    }
    Ok(v)
}

/// The format name a settings file may carry in `format`.
pub const FORMAT: &str = "runbook.loom-run-settings/1";

/// The members of `loom` a settings file may carry.
const LOOM_KEYS: [&str; 5] = ["sha256", "in_process", "confinement", "cap_micro_usd", "login_file"];

fn same_number(v: &Value, want: u64, what: &str, option: &str) -> Result<(), String> {
    match v.as_u64() {
        Some(n) if n == want => Ok(()),
        Some(n) => Err(format!("{what} is {n}, the option {option} is {want}: the option decides; make them equal")),
        None => Err(format!("{what}: a whole number")),
    }
}

/// Reads the file as the one format `runbook.loom-run-settings/1` (`docs/loom-run-settings.md`) and
/// returns the settings the loop, the decider and the bindings copy read. The members that only
/// repeat an option (`format`, `run`, `loom.sha256`, `loom.confinement`, `loom.cap_micro_usd`,
/// `max_llm_calls`, `max_wall_seconds`) are checked against the options and must agree; `loom.login_file`
/// becomes `login_file`; `loom` and `format` and `run` are taken out. A `decider.principal` of a
/// model or rule decider must be the one the kind gives (`decider:model`, `decider:<verdict>`) and
/// is taken out. Everything else is passed on untouched.
pub fn normalise(raw: &Value, args: &crate::loom_run::Args) -> Result<Value, String> {
    let mut v = raw.clone();
    let top = v.as_object_mut().ok_or("settings: an object")?;
    if let Some(f) = top.remove("format") {
        if f.as_str() != Some(FORMAT) {
            return Err(format!("format: {f} is not {FORMAT}"));
        }
    }
    if let Some(r) = top.remove("run") {
        if r.as_str() != Some(args.run.as_str()) {
            return Err(format!("run: {r} is not the run named by --run"));
        }
    }
    if let Some(loom) = top.remove("loom") {
        let loom = loom.as_object().ok_or("loom: an object")?;
        for (key, value) in loom {
            match key.as_str() {
                "sha256" => match args.loom_sha256.as_deref() {
                    Some(given) => {
                        if value.as_str().map(|s| s.trim().to_ascii_lowercase()) != Some(given.trim().to_ascii_lowercase()) {
                            return Err("loom.sha256 is not the sha256 of --loom-sha256: the option decides; make them equal".to_owned());
                        }
                    }
                    None => return Err("loom.sha256: Loom is a library of this process (no --loom), so there is no digest to name; take it out".to_owned()),
                },
                "in_process" => {
                    if value != &Value::Bool(true) || !args.in_process() {
                        return Err("loom.in_process is true only when Loom is a library of this process (no --loom): make them equal".to_owned());
                    }
                }
                "confinement" => {
                    if value.as_str() != Some(args.confinement.as_arg()) {
                        return Err(format!("loom.confinement is {value}, the option --confinement is {}: make them equal", args.confinement.as_arg()));
                    }
                }
                "cap_micro_usd" => same_number(value, args.cap_micro_usd, "loom.cap_micro_usd", "--cap-micro-usd")?,
                "login_file" => {
                    if top.contains_key(LOGIN_KEY) {
                        return Err("login_file and loom.login_file both name the login file; give one".to_owned());
                    }
                    top.insert(LOGIN_KEY.to_owned(), value.clone());
                }
                "bin" => return Err("loom.bin: the binary is the option --loom, not a setting".to_owned()),
                other => return Err(format!("loom.{other}: not a setting of the format (known: {})", LOOM_KEYS.join(", "))),
            }
        }
    }
    if let Some(n) = top.get("max_llm_calls") {
        same_number(n, args.max_llm_calls, "max_llm_calls", "--max-llm-calls")?;
    }
    if let Some(n) = top.get("max_wall_seconds") {
        same_number(n, args.max_wall_seconds, "max_wall_seconds", "--max-wall-seconds")?;
    }
    if let Some(decider) = top.get_mut("decider").and_then(Value::as_object_mut) {
        let kind = decider.get("kind").and_then(Value::as_str).map(str::to_owned);
        let want = match kind.as_deref() {
            Some("model") => Some("decider:model".to_owned()),
            Some("rule") => decider.get("verdict").and_then(Value::as_str).map(|v| format!("decider:{v}")),
            _ => None,
        };
        if let Some(want) = want {
            if let Some(p) = decider.remove("principal") {
                if p.as_str() != Some(want.as_str()) {
                    return Err(format!("decider.principal is {p}; a decider of kind {} acts as {want}", kind.unwrap_or_default()));
                }
            }
        }
    }
    Ok(v)
}

/// Fills, for a model decider with none, `decider.reply_bin` (`sdc-reply` found in `path`) before
/// the decider is built: the decider does not read it, the scripted adapter of a test may, so the
/// adapter keeps getting it as `SDC_REPLY`. A `decider.program` in the file is refused: the decider
/// runs inside the runner and starts no program of the bindings copy but the adapter.
pub fn fill_decider(settings: &mut Value, path: Option<&std::ffi::OsStr>) -> Result<(), String> {
    let Some(decider) = settings.get_mut("decider").and_then(Value::as_object_mut) else {
        return Ok(());
    };
    let kind = decider.get("kind").and_then(Value::as_str).unwrap_or("").to_owned();
    if kind != "model" && kind != "rule" {
        return Ok(());
    }
    if decider.contains_key("program") {
        return Err("decider.program: not a setting; the decider runs inside the runner and starts only the adapter program (decider.adapter)".to_owned());
    }
    if kind == "model" && !decider.contains_key("reply_bin") {
        let found = path.into_iter().flat_map(std::env::split_paths).map(|d| d.join("sdc-reply")).find(|p| p.is_file());
        if let Some(found) = found {
            decider.insert("reply_bin".to_owned(), Value::String(found.display().to_string()));
        }
    }
    Ok(())
}

/// The redacted settings with what the command was given added, as `RunStarted` carries them (the
/// members `sdc-experiment check --runs-dir` compares): `loom` (`sha256` of the binary, or
/// `in_process` and the pinned `commit` of the library; `confinement`, `cap_micro_usd`),
/// `max_llm_calls`, `max_wall_seconds`, `case`, `root_protocol`. The options win over a member of
/// the same name in the file. The path of the binary is not in them.
pub fn with_run(redacted: &Value, args: &crate::loom_run::Args, loom: &crate::loom_run::host::Identity) -> Value {
    use crate::loom_run::host::Identity;
    let mut v = redacted.clone();
    if let Some(o) = v.as_object_mut() {
        let mut which = match loom {
            Identity::Binary { sha256 } => serde_json::json!({"sha256": sha256}),
            Identity::Library { commit } => serde_json::json!({"in_process": true, "commit": commit}),
        };
        if let Some(w) = which.as_object_mut() {
            w.insert("confinement".to_owned(), Value::from(args.confinement.as_arg()));
            w.insert("cap_micro_usd".to_owned(), Value::from(args.cap_micro_usd));
        }
        o.insert("loom".to_owned(), which);
        o.insert("max_llm_calls".to_owned(), Value::from(args.max_llm_calls));
        o.insert("max_wall_seconds".to_owned(), Value::from(args.max_wall_seconds));
        o.insert("case".to_owned(), Value::from(args.case.clone()));
        o.insert("root_protocol".to_owned(), Value::from(args.root_protocol.clone()));
    }
    v
}

/// The settings as written into the run directory: `login_file` replaced by the text `named`.
pub fn redacted(settings: &Value) -> Value {
    let mut v = settings.clone();
    if let Some(o) = v.as_object_mut() {
        if o.contains_key(LOGIN_KEY) {
            o.insert(LOGIN_KEY.to_owned(), Value::String("named".to_owned()));
        }
    }
    v
}
