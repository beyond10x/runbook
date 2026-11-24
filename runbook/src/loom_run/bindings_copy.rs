//! The run's copy of the bindings: `bindings/` copied into the run directory with the run's
//! principal for the person steps the decider answers, the model per role, turns and budgets, the
//! adapter and the login file applied to the COPY, and the tools directory of the copy handed to
//! every model node (`--tools-dir <copy>/tools`: `sdc-node` is found on PATH and has no checkout
//! beside it). The committed `bindings/*.yaml` are never
//! written. The table of every setting and where it lands is in `docs/loom-run-decisions.md`.
//!
//! The YAML is edited structurally (`serde_yaml::Value`), never by text replacement. A file with no
//! model node that no setting touches is copied byte for byte.

use crate::loom_run::answers::Answers;
use crate::loom_run::decider;
use crate::loom_run::record_write::Held;
use runbook::engine::copy::{copy_dirs, Links};
use serde_json::Value;
use serde_yaml::{Mapping, Value as Yaml};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The top-level keys of the settings that `materialise` knows. `login_file` is R1's; `autonomy`
/// and `decider` are read by the decider as well; `max_llm_calls` and `max_wall_seconds` belong to
/// the runner's caps and are accepted and ignored here.
pub const SETTINGS_KEYS: [&str; 10] = ["login_file", "autonomy", "decider", "adapter", "models", "efforts", "review", "max_llm_calls", "max_wall_seconds", "replay"];
/// The roles whose model, turns and budget a run sets (`bindings/tools/node-classes.json`).
pub const ROLES: [&str; 4] = ["writer", "implement", "review", "other"];
const ROLE_KEYS: [&str; 3] = ["model", "max_turns", "max_budget_usd"];
/// The reasoning efforts a model node takes (`efforts` of `bindings/tools/node-classes.json`).
const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];
/// The member of `efforts.<role>` that names the level.
const EFFORT_KEY: &str = "effort";
const ADAPTERS: [&str; 3] = ["metaharness", "claude-cli", "dry"];
/// What a checkout holds beside `bindings/` that the tools of the copy read from their root
/// (`$here/../..`): the instruction files, the fixtures and the rule file of the driver, and the
/// checks the requester decider runs (`tools/decision-grounds.sh`).
const COMPANIONS: [&str; 3] = ["instructions", "driver/examples", "tools"];
/// The program of a model node, found on PATH.
pub const MODEL_TOOL: &str = "sdc-node";
/// The option that names the tools directory of the copy.
const TOOLS_DIR_OPTION: &str = "--tools-dir";

#[cfg(test)]
mod tests;

#[derive(Default)]
struct RoleSet {
    model: Option<String>,
    max_turns: Option<u64>,
    max_budget_usd: Option<String>,
    effort: Option<String>,
}

struct Plan {
    principal: Option<String>,
    /// Below A5: the level, the decider's principal and the holder table; a person step is the
    /// decider's only where the table gives its class to the decider. `None`: no such run, or a table
    /// that cannot be read (every step keeps its person).
    routed: Option<(String, String, decider::Routing)>,
    roles: BTreeMap<String, RoleSet>,
    adapter: Option<String>,
    login: Option<PathBuf>,
    replay: Option<crate::loom_run::replay::BindingPlan>,
}

impl Plan {
    /// The principal the person step `action` is given: the decider's at A5, below A5 only where the
    /// holder table gives the class of the action to the decider.
    fn principal_of(&self, action: &str) -> Option<&str> {
        if let Some(principal) = &self.principal {
            return Some(principal);
        }
        let (level, principal, routing) = self.routed.as_ref()?;
        routing.decider_takes(level, "approval", action).then_some(principal.as_str())
    }
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

/// The reasoning effort of each role the settings name (`efforts.<role>.effort`). A role that is
/// none, a member that is not `effort`, and a level outside the allowed set are refused with the
/// setting's name: both model CLIs ignore an unknown level with a warning and run at their own
/// default. The run checks this before it creates its directory.
pub fn efforts(settings: &Value) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    let Some(efforts) = settings.get("efforts") else { return Ok(out) };
    for (role, set) in object(efforts, "settings.efforts")? {
        if !ROLES.contains(&role.as_str()) {
            return Err(format!("settings.efforts.{role}: not a role ({})", ROLES.join(", ")));
        }
        for (key, value) in object(set, &format!("settings.efforts.{role}"))? {
            let what = format!("settings.efforts.{role}.{key}");
            if key != EFFORT_KEY {
                return Err(format!("{what}: not a setting a run knows under Loom (known: {EFFORT_KEY})"));
            }
            match value.as_str() {
                Some(level) if EFFORTS.contains(&level) => out.insert(role.clone(), level.to_owned()),
                _ => return Err(format!("{what}: {value} is not a reasoning effort ({})", EFFORTS.join(", "))),
            };
        }
    }
    Ok(out)
}

/// Whether the settings' members are ones the copy reads: every table, key and kind, nothing written.
pub fn check(settings: &Value) -> Result<(), String> {
    read_plan(settings).map(|_| ())
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
    for (role, level) in efforts(settings)? {
        roles.entry(role).or_default().effort = Some(level);
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
    let replay = crate::loom_run::replay::binding_plan(settings)?;
    Ok(Plan { principal, routed: None, roles, adapter, login, replay })
}

/// Copies the bindings directory `src` into `dst` (which does not exist yet) and applies
/// `settings` to the copy; returns the root bindings file `dst/<root_protocol_name>.yaml` that
/// `tree open --bindings` is given.
///
/// Besides `dst`, things are made beside it, because the tools find their root two levels
/// above their own directory (`dst/tools/..` is `dst`, its parent is the run directory):
/// `<dst>/../instructions` and `<dst>/../driver/examples` (copies of `<src>/../instructions` and
/// `<src>/../driver/examples`, when they exist: the fixtures and `rule-ends.json` the tools read)
/// and, when the settings name a login file, nothing: `login_link` (made by the caller outside
/// the run directory, see `private.rs`) is what the `--claude-credentials` option of the model
/// nodes holds, and the settings must come with one. An unknown key in `settings` is refused by
/// name before anything is written. The login file's path is in no error and no return value.
pub fn materialise(src: &Path, dst: &Path, root_protocol_name: &str, settings: &Value, login_link: Option<&Path>) -> Result<PathBuf, String> {
    let mut plan = read_plan(settings)?;
    if let Some((level, principal)) = decider::step_principal(settings)?.filter(|(level, _)| level != decider::AUTONOMY_DECIDES) {
        plan.routed = src.parent().and_then(|root| decider::Routing::load(root).ok()).map(|routing| (level, principal, routing));
    }
    if dst.exists() {
        return Err(format!("{}: exists", dst.display()));
    }
    let dst = std::path::absolute(dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    let beside = dst.parent().ok_or("the bindings copy has no parent directory")?.to_path_buf();
    if plan.login.is_some() && login_link.is_none() {
        return Err("login_file is named and no link to it was given (the runner makes it in its private directory)".to_owned());
    }
    let companions: Vec<(PathBuf, PathBuf)> = COMPANIONS
        .iter()
        .filter_map(|rel| {
            let from = src.parent()?.join(rel);
            from.is_dir().then(|| (from, beside.join(rel)))
        })
        .collect();
    if let Some((_, to)) = companions.iter().find(|(_, to)| to.exists()) {
        return Err(format!("{}: exists", to.display()));
    }
    // One routine copies a runbook's files, for this command and for the daemon's engine.
    let mut jobs: Vec<(&Path, &Path)> = vec![(src, dst.as_path())];
    jobs.extend(companions.iter().map(|(from, to)| (from.as_path(), to.as_path())));
    copy_dirs(&jobs, Links::Keep)?;
    // The members `max_budget_usd` leave the copy as written, before the settings touch an argv: Loom
    // refuses a member it does not know, and the reservation reads them from beside the copy.
    runbook::budget::seal(&dst.join(format!("{root_protocol_name}.yaml")))?;
    let login_argument = login_link.map(|l| l.display().to_string());
    rewrite_files(&dst, &plan, login_argument.as_deref())?;
    let root = dst.join(format!("{root_protocol_name}.yaml"));
    if !root.is_file() {
        return Err(format!("{}: the root bindings file is missing in the copy", root.display()));
    }
    Ok(root)
}

/// Whether the bindings in `dir` (a bindings directory or the run's copy of it) hold a model node,
/// and why. `None`: none of the forms below was found. It reads every `*.yaml` and `*.yml` file
/// below `dir` (the bindings files, `tools/answers.yaml`) and fails closed: a form that cannot be
/// read or is not one the rule knows counts as a model node. The forms that count:
///
/// - a word of an `argv`, or any other text of a file, that names the model tool: its last path
///   part is `sdc-node`, or `sdc-node` is a part of the text (so `sh -c "sdc-node ..."`, `env X=1
///   sdc-node ...` and `timeout 60 sdc-node ...` count, as does a program whose first word is the
///   tool, the rule `rewrite_document` applies);
/// - an `argv` that is not a non-empty list of texts;
/// - an executor of a kind other than `person`, `copy` (Loom copies evidence of another case, it starts no program) and `program`, or an action without an executor;
/// - `actions` that is not a list, an action that is not a mapping;
/// - a file that is not valid YAML or cannot be read (a `*.yaml` name that is a link, a pipe or a file
///   with a second name is not read and counts), a directory that cannot be read, and a link to a
///   directory or to anything that cannot be read.
///
/// The copy is read while the programs of the run may write it, so it is read as every name of the
/// run is (`record_write`): `dir` is held ([`Held::open`]: a link at its name counts), its names are
/// listed through the descriptor, a directory below it is held from it ([`Held::sub`]) and a file is
/// read through it ([`Held::read_text`]).
///
/// What it cannot see: a bound program that starts a model by a name this function does not read
/// (a script that calls a service by itself). It is not covered by a cap either.
pub fn model_node_in(dir: &Path) -> Option<String> {
    let held = match Held::open(dir) {
        Ok(h) => h,
        Err(e) => return Some(format!("{e} (it cannot be read)")),
    };
    let mut stack = vec![(held, PathBuf::new())];
    while let Some((d, rel)) = stack.pop() {
        let names = match d.names() {
            Ok(n) => n,
            Err(e) => return Some(format!("{e} (it cannot be listed)")),
        };
        for name in names {
            let shown = rel.join(&name);
            if !d.exists(&name) {
                return Some(format!("{} is a link to nothing", shown.display()));
            }
            if !d.is_file(&name) {
                // a directory is held from this one, without following a link: a link to a directory, a pipe or another
                // kind of name is refused there and counts
                match d.sub(Path::new(&name)) {
                    Ok(sub) => stack.push((sub, shown)),
                    Err(_) => return Some(format!("{} is a link to a directory or not a file or a directory", shown.display())),
                }
                continue;
            }
            if !Path::new(&name).extension().is_some_and(|x| x == "yaml" || x == "yml") {
                continue;
            }
            let text = match d.read_text(&name) {
                Ok(t) => t,
                Err(e) => return Some(format!("{} cannot be read ({e})", shown.display())),
            };
            let doc: Yaml = match serde_yaml::from_str(&text) {
                Ok(d) => d,
                Err(_) => return Some(format!("{} is not valid YAML", shown.display())),
            };
            if let Some(why) = model_node_in_document(&doc) {
                return Some(format!("{}: {why}", shown.display()));
            }
        }
    }
    None
}

/// The form of one document that counts as a model node, see [`model_node_in`].
fn model_node_in_document(doc: &Yaml) -> Option<String> {
    if let Some(actions) = doc.get("actions") {
        let Some(actions) = actions.as_sequence() else {
            return Some("actions is not a list".to_owned());
        };
        for action in actions {
            let name = action.get("action").and_then(Yaml::as_str).unwrap_or("?");
            if !action.is_mapping() {
                return Some("an action that is not a mapping".to_owned());
            }
            let Some(executor) = action.get("executor").filter(|e| e.is_mapping()) else {
                return Some(format!("{name}: no executor"));
            };
            match executor.get("kind").and_then(Yaml::as_str) {
                Some("person" | "copy") => {}
                Some("program") => {
                    let argv = executor.get("argv").and_then(Yaml::as_sequence);
                    if !argv.is_some_and(|a| !a.is_empty() && a.iter().all(Yaml::is_string)) {
                        return Some(format!("{name}: argv is not a list of texts"));
                    }
                }
                other => return Some(format!("{name}: an executor kind the rule does not know ({})", other.unwrap_or("none"))),
            }
        }
    }
    names_model_tool(doc).map(|text| format!("{MODEL_TOOL} is named in {text:?}"))
}

/// The first text in `value`, at any depth, that names the model tool.
fn names_model_tool(value: &Yaml) -> Option<String> {
    match value {
        Yaml::String(s) => s.contains(MODEL_TOOL).then(|| s.chars().take(80).collect()),
        Yaml::Sequence(items) => items.iter().find_map(names_model_tool),
        Yaml::Mapping(map) => map.iter().find_map(|(k, v)| names_model_tool(k).or_else(|| names_model_tool(v))),
        Yaml::Tagged(tagged) => names_model_tool(&tagged.value),
        _ => None,
    }
}

fn class_of(classes: &Value, action: &str) -> String {
    classes
        .get("classes")
        .and_then(Value::as_object)
        .and_then(|map| map.iter().find(|(_, names)| names.as_array().is_some_and(|a| a.iter().any(|n| n.as_str() == Some(action)))).map(|(k, _)| k.clone()))
        .unwrap_or_else(|| "other".to_owned())
}

fn rewrite_files(dst: &Path, plan: &Plan, login: Option<&str>) -> Result<(), String> {
    let classes: Value = std::fs::read_to_string(dst.join("tools").join("node-classes.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null);
    let answers = Answers::load(dst)?;
    let tools = dst.join("tools").display().to_string();
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
        let mut changed = rewrite_document(&mut doc, plan, &classes, login, &tools, &answers);
        if let Some(replay) = &plan.replay {
            changed |= replay.apply(&mut doc, dst);
        }
        if changed {
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
fn rewrite_document(doc: &mut Yaml, plan: &Plan, classes: &Value, login: Option<&str>, tools: &str, answers: &Answers) -> bool {
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
            // A step whose outcome is a person's own text (an answer) keeps its person unless the
            // answers file declares it: only the file says what the decider answers there.
            if let Some(principal) = plan.principal_of(&name).filter(|_| answers.decider_takes(&name, action)) {
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
            changed |= set_option(argv, TOOLS_DIR_OPTION, tools);
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
                if let Some(effort) = &set.effort {
                    changed |= set_option(argv, "--effort", effort);
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
pub(super) fn set_option(argv: &mut Vec<Yaml>, name: &str, value: &str) -> bool {
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
