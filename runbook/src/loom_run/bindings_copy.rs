//! The run's copy of the bindings: `bindings/` copied into the run directory with the run's
//! principal for the person steps the decider answers, the model per role, turns and budgets, the
//! adapter and the login file applied to the COPY. The committed `bindings/*.yaml` are never
//! written. The table of every setting and where it lands is in `docs/loom-run-decisions.md`.
//!
//! The YAML is edited structurally (`serde_yaml::Value`), never by text replacement. A file that no
//! setting touches is copied byte for byte.

// Nothing calls this yet: R1's loop wires it (interfaces.md). Remove the allow with that change.
#![allow(dead_code)]

use crate::loom_run::decider;
use serde_json::Value;
use serde_yaml::{Mapping, Value as Yaml};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The top-level keys of the settings that `materialise` knows. `login_file` is R1's; `autonomy`
/// and `decider` are read by the decider as well; `max_llm_calls` and `max_wall_seconds` belong to
/// the runner's caps and are accepted and ignored here.
pub const SETTINGS_KEYS: [&str; 8] = ["login_file", "autonomy", "decider", "adapter", "models", "review", "max_llm_calls", "max_wall_seconds"];
/// The roles whose model, turns and budget a run sets (`bindings/tools/node-classes.json`).
pub const ROLES: [&str; 4] = ["writer", "implement", "review", "other"];
const ROLE_KEYS: [&str; 3] = ["model", "max_turns", "max_budget_usd"];
const ADAPTERS: [&str; 2] = ["metaharness", "claude-cli"];
/// The program of a model node.
const MODEL_TOOL: &str = "llm-node.sh";
/// The person action whose answer the decider cannot give (a question; see the decisions page).
const KEEPS_PERSON: [&str; 1] = ["scope.answer"];

#[cfg(test)]
mod tests;

#[derive(Default)]
struct RoleSet {
    model: Option<String>,
    max_turns: Option<u64>,
    max_budget_usd: Option<String>,
}

struct Plan {
    principal: Option<String>,
    roles: BTreeMap<String, RoleSet>,
    adapter: Option<String>,
    login: Option<PathBuf>,
}

fn object<'a>(value: &'a Value, what: &str) -> Result<&'a serde_json::Map<String, Value>, String> {
    value.as_object().ok_or_else(|| format!("{what}: an object"))
}

fn plain(value: &Value, what: &str) -> Result<String, String> {
    match value {
        Value::String(s) if !s.is_empty() && !s.chars().any(char::is_whitespace) => Ok(s.clone()),
        _ => Err(format!("{what}: a non-empty text without white space")),
    }
}

fn read_plan(settings: &Value) -> Result<Plan, String> {
    let top = object(settings, "settings")?;
    for key in top.keys() {
        if !SETTINGS_KEYS.contains(&key.as_str()) {
            return Err(format!("settings.{key}: not a setting a run knows under Loom (known: {})", SETTINGS_KEYS.join(", ")));
        }
    }
    let principal = decider::person_step_principal(settings)?;
    let adapter = match top.get("adapter") {
        None => None,
        Some(v) => {
            let a = plain(v, "settings.adapter")?;
            if !ADAPTERS.contains(&a.as_str()) {
                return Err(format!("settings.adapter: {} (metaharness or claude-cli; the other adapters are the driver's)", a));
            }
            Some(a)
        }
    };
    if let Some(review) = top.get("review") {
        let review = object(review, "settings.review")?;
        for (key, value) in review {
            if key != "adapter" {
                return Err(format!("settings.review.{key}: not a setting a run knows under Loom"));
            }
            if value.as_str() != Some("claude-cli") {
                return Err("settings.review.adapter: only claude-cli is expressible under Loom (it is what the review class takes when the writers run on metaharness; an option to change it does not exist)".to_owned());
            }
        }
    }
    let mut roles = BTreeMap::new();
    if let Some(models) = top.get("models") {
        for (role, set) in object(models, "settings.models")? {
            if !ROLES.contains(&role.as_str()) {
                return Err(format!("settings.models.{role}: not a role ({})", ROLES.join(", ")));
            }
            let mut out = RoleSet::default();
            for (key, value) in object(set, &format!("settings.models.{role}"))? {
                let what = format!("settings.models.{role}.{key}");
                match key.as_str() {
                    "model" => out.model = Some(plain(value, &what)?),
                    "max_turns" => out.max_turns = Some(value.as_u64().filter(|n| *n >= 1).ok_or_else(|| format!("{what}: a whole number of at least 1"))?),
                    "max_budget_usd" => {
                        let usd = value.as_f64().filter(|u| u.is_finite() && *u > 0.0).ok_or_else(|| format!("{what}: a number above 0"))?;
                        out.max_budget_usd = Some(format!("{usd}"));
                    }
                    other => return Err(format!("settings.models.{role}.{other}: not a setting a run knows under Loom (known: {})", ROLE_KEYS.join(", "))),
                }
            }
            roles.insert(role.clone(), out);
        }
    }
    let login = match top.get(crate::loom_run::settings::LOGIN_KEY) {
        None | Some(Value::Null) => None,
        Some(Value::String(path)) if !path.is_empty() => {
            let path = PathBuf::from(path);
            if !path.is_absolute() || !path.is_file() {
                return Err("login_file: not an absolute path of a file that exists".to_owned());
            }
            Some(path)
        }
        Some(_) => return Err("login_file: a path".to_owned()),
    };
    Ok(Plan { principal, roles, adapter, login })
}

/// Copies the bindings directory `src` into `dst` (which does not exist yet) and applies
/// `settings` to the copy; returns the root bindings file `dst/<root_protocol_name>.yaml` that
/// `tree open --bindings` is given.
///
/// Besides `dst`, two things are made beside it, because the tools find their root two levels
/// above their own directory (`dst/tools/..` is `dst`, its parent is the run directory):
/// `<dst>/../instructions` (a copy of `<src>/../instructions`, when that exists) and, when the
/// settings name a login file, the link `<dst>/../login` to it; the link's path is what the
/// `--claude-credentials` option of the model nodes holds. An unknown key in `settings` is refused
/// by name before anything is written. The login file's path is in no error and no return value.
pub fn materialise(src: &Path, dst: &Path, root_protocol_name: &str, settings: &Value) -> Result<PathBuf, String> {
    let plan = read_plan(settings)?;
    if dst.exists() {
        return Err(format!("{}: exists", dst.display()));
    }
    let dst = std::path::absolute(dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    let beside = dst.parent().ok_or("the bindings copy has no parent directory")?.to_path_buf();
    if plan.login.is_some() && std::fs::symlink_metadata(beside.join("login")).is_ok() {
        return Err(format!("{}: exists", beside.join("login").display()));
    }
    let instructions_src = src.parent().map(|p| p.join("instructions")).filter(|p| p.is_dir());
    if instructions_src.is_some() && beside.join("instructions").exists() {
        return Err(format!("{}: exists", beside.join("instructions").display()));
    }
    copy_dir(src, &dst)?;
    if let Some(from) = &instructions_src {
        copy_dir(from, &beside.join("instructions"))?;
    }
    let login_argument = match &plan.login {
        None => None,
        Some(real) => {
            let link = beside.join("login");
            std::os::unix::fs::symlink(real, &link).map_err(|e| format!("{}: could not link the login file: {}", link.display(), e.kind()))?;
            Some(link.display().to_string())
        }
    };
    rewrite_files(&dst, &plan, login_argument.as_deref())?;
    let root = dst.join(format!("{root_protocol_name}.yaml"));
    if !root.is_file() {
        return Err(format!("{}: the root bindings file is missing in the copy", root.display()));
    }
    Ok(root)
}

fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    let entries = std::fs::read_dir(src).map_err(|e| format!("{}: {e}", src.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", src.display()))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let kind = entry.file_type().map_err(|e| format!("{}: {e}", from.display()))?;
        if kind.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
            // A read-only source is copied writable: the copy is the run's.
            let mut permissions = std::fs::metadata(&to).map_err(|e| format!("{}: {e}", to.display()))?.permissions();
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(permissions.mode() | 0o200);
            std::fs::set_permissions(&to, permissions).map_err(|e| format!("{}: {e}", to.display()))?;
        }
    }
    Ok(())
}

fn class_of(classes: &Value, action: &str) -> String {
    classes
        .get("classes")
        .and_then(Value::as_object)
        .and_then(|map| map.iter().find(|(_, names)| names.as_array().is_some_and(|a| a.iter().any(|n| n.as_str() == Some(action)))).map(|(k, _)| k.clone()))
        .unwrap_or_else(|| "other".to_owned())
}

fn rewrite_files(dst: &Path, plan: &Plan, login: Option<&str>) -> Result<(), String> {
    if plan.principal.is_none() && plan.roles.is_empty() && plan.adapter.is_none() && login.is_none() {
        return Ok(());
    }
    let classes: Value = std::fs::read_to_string(dst.join("tools").join("node-classes.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null);
    let mut files: Vec<PathBuf> = std::fs::read_dir(dst)
        .map_err(|e| format!("{}: {e}", dst.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "yaml") && p.is_file())
        .collect();
    files.sort();
    for file in files {
        let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let mut doc: Yaml = serde_yaml::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
        if rewrite_document(&mut doc, plan, &classes, login) {
            let out = serde_yaml::to_string(&doc).map_err(|e| format!("{}: {e}", file.display()))?;
            std::fs::write(&file, out).map_err(|e| format!("{}: {e}", file.display()))?;
        }
    }
    Ok(())
}

fn key(name: &str) -> Yaml {
    Yaml::String(name.to_owned())
}

fn text(map: &Mapping, name: &str) -> Option<String> {
    map.get(name).and_then(Yaml::as_str).map(str::to_owned)
}

/// Applies the plan to one bindings document; true when it changed anything.
fn rewrite_document(doc: &mut Yaml, plan: &Plan, classes: &Value, login: Option<&str>) -> bool {
    let mut changed = false;
    let Some(actions) = doc.get_mut("actions").and_then(Yaml::as_sequence_mut) else {
        return false;
    };
    for action in actions {
        let Some(name) = action.get("action").and_then(Yaml::as_str).map(str::to_owned) else {
            continue;
        };
        let kind = action.get("executor").and_then(|e| e.get("kind")).and_then(Yaml::as_str).map(str::to_owned);
        if kind.as_deref() == Some("person") {
            if let Some(principal) = plan.principal.as_deref().filter(|_| !KEEPS_PERSON.contains(&name.as_str())) {
                let old = action.get("executor").and_then(Yaml::as_mapping).and_then(|m| text(m, "principal"));
                if let Some(executor) = action.get_mut("executor").and_then(Yaml::as_mapping_mut) {
                    executor.insert(key("principal"), key(principal));
                    changed = true;
                }
                if let (Some(old), Some(evidence)) = (old, action.get_mut("evidence").and_then(Yaml::as_sequence_mut)) {
                    for entry in evidence.iter_mut().filter_map(Yaml::as_mapping_mut) {
                        if text(entry, "producer").as_deref() == Some(old.as_str()) {
                            entry.insert(key("producer"), key(principal));
                        }
                    }
                }
            }
        } else if kind.as_deref() == Some("program") {
            let Some(argv) = action.get_mut("executor").and_then(|e| e.get_mut("argv")).and_then(Yaml::as_sequence_mut) else {
                continue;
            };
            let is_model_node = argv.first().and_then(Yaml::as_str).is_some_and(|tool| tool.rsplit('/').next() == Some(MODEL_TOOL));
            if !is_model_node {
                continue;
            }
            let set = plan.roles.get(&class_of(classes, &name));
            if let Some(set) = set {
                if let Some(model) = &set.model {
                    changed |= set_option(argv, "--model", model);
                }
                if let Some(turns) = set.max_turns {
                    changed |= set_option(argv, "--max-turns", &turns.to_string());
                }
                if let Some(usd) = &set.max_budget_usd {
                    changed |= set_option(argv, "--max-budget-usd", usd);
                }
            }
            if let Some(adapter) = &plan.adapter {
                changed |= set_option(argv, "--adapter", adapter);
            }
            if let Some(login) = login {
                changed |= set_option(argv, "--claude-credentials", login);
            }
        }
    }
    changed
}

/// Sets option `name` of the model tool's argv (`tool action [--name value]... [--stub ...]`):
/// replaces its value, or inserts the pair right after the action name (the options precede
/// `--stub`, which takes the rest of the argv). True when the argv changed.
fn set_option(argv: &mut Vec<Yaml>, name: &str, value: &str) -> bool {
    let stub = argv.iter().position(|a| a.as_str() == Some("--stub")).unwrap_or(argv.len());
    let mut i = 2;
    while i + 1 < stub {
        if argv[i].as_str() == Some(name) {
            let same = argv[i + 1].as_str() == Some(value);
            argv[i + 1] = key(value);
            return !same;
        }
        i += 2;
    }
    argv.insert(2.min(argv.len()), key(value));
    argv.insert(2.min(argv.len()), key(name));
    true
}
