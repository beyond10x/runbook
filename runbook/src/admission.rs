//! The route entries of a runbook and their admission (`docs/model-routes.md`, section 2).
//!
//! A runbook may carry, beside its bindings, the file `bindings/<runbook>.routes.yaml` (format `runbook.routes/1`): per
//! role an entry that names a daemon route (`{route: qwen}`) or states what a route must meet (`{needs: {wire, min_context,
//! tool_calling, kind}}`), and an optional `routes_allowed` that narrows which declared routes may serve the runbook. It
//! is Runbook's own file, not part of Loom's bindings.
//!
//! [`parse`] reads the text, [`admit`] matches each entry against the routes the daemon declares and what their gateway
//! listed (`crate::routes`, the view `routes`): a named route must be declared and allowed; needs are met by the daemon's
//! default route when it meets them, else by the one allowed route that meets them, and none or several is a refusal.
//! Every refusal names the role and the route or the requirement; a text that is not a name (an address, a token) is
//! never echoed. The catalog (`Daemon::catalog`) lists the result. A submit and a delegation that names the runbook ask
//! [`check`] and [`for_run`], which add the run's word (it comes first) and name the runbook in every refusal; nothing here
//! reads a gateway or writes a file.

use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

/// The format of the file.
pub const FORMAT: &str = "runbook.routes/1";
/// The most bytes of the file that are read.
pub const FILE_LIMIT: u64 = 64 * 1024;
/// The most roles a file may hold.
const MAX_ROLES: usize = 64;
const MEMBERS: [&str; 3] = ["format", "model_routes", "routes_allowed"];
const NEEDS: [&str; 4] = ["wire", "min_context", "tool_calling", "kind"];
const KINDS: [&str; 3] = ["managed", "hosted", "local"];

/// The file of the runbook `runbook`, relative to its runbook directory, as a message names it.
pub fn file_name(runbook: &str) -> String {
    format!("bindings/{runbook}.routes.yaml")
}

/// The text of the route entries file at `path` (the file of [`file_name`] in the runbook's directory): none when there is
/// none; an error, naming the file by [`file_name`] and never by its path, when it is a link, not a regular file, larger
/// than [`FILE_LIMIT`] or not UTF-8 text.
pub fn read(path: &Path, runbook: &str) -> Option<Result<String, String>> {
    let named = file_name(runbook);
    let file = match std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC).open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(_) => return Some(Err(format!("{named}: not a readable regular file (a link is not followed)"))),
    };
    if !file.metadata().is_ok_and(|m| m.is_file()) {
        return Some(Err(format!("{named}: not a regular file")));
    }
    let mut bytes = Vec::new();
    if file.take(FILE_LIMIT + 1).read_to_end(&mut bytes).is_err() {
        return Some(Err(format!("{named}: not readable")));
    }
    if bytes.len() as u64 > FILE_LIMIT {
        return Some(Err(format!("{named}: larger than {FILE_LIMIT} bytes")));
    }
    Some(String::from_utf8(bytes).map_err(|_| format!("{named}: not UTF-8 text")))
}

/// What a route entry asks of a route when it names none: each present member is a requirement.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Needs {
    pub wire: Option<String>,
    pub min_context: Option<u64>,
    pub tool_calling: Option<bool>,
    pub kind: Option<String>,
}

impl Needs {
    fn to_json(&self) -> Value {
        let mut m = Map::new();
        if let Some(w) = &self.wire {
            m.insert("wire".into(), json!(w));
        }
        if let Some(c) = self.min_context {
            m.insert("min_context".into(), json!(c));
        }
        if let Some(t) = self.tool_calling {
            m.insert("tool_calling".into(), json!(t));
        }
        if let Some(k) = &self.kind {
            m.insert("kind".into(), json!(k));
        }
        Value::Object(m)
    }

    /// The requirements in words: `wire messages, min_context 100000, tool_calling, kind managed`.
    fn words(&self) -> String {
        let mut w = Vec::new();
        if let Some(x) = &self.wire {
            w.push(format!("wire {x}"));
        }
        if let Some(c) = self.min_context {
            w.push(format!("min_context {c}"));
        }
        if self.tool_calling == Some(true) {
            w.push("tool_calling".to_owned());
        }
        if let Some(k) = &self.kind {
            w.push(format!("kind {k}"));
        }
        if w.is_empty() {
            "no requirement".to_owned()
        } else {
            w.join(", ")
        }
    }

    fn met_by(&self, f: &Facts) -> bool {
        self.wire.as_ref().is_none_or(|w| f.wires.iter().any(|x| x == w))
            && self.min_context.is_none_or(|c| f.max_model_len.is_some_and(|m| m >= c))
            && (self.tool_calling != Some(true) || f.tool_calling == Some(true))
            && self.kind.as_ref().is_none_or(|k| *k == f.kind)
    }
}

/// One role of the file.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Entry {
    Named(String),
    Needs(Needs),
    /// The entry is not one: why.
    Invalid(String),
}

/// A file as read: its entries by role and its `routes_allowed`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declared {
    entries: BTreeMap<String, Entry>,
    /// The names of `routes_allowed`, or why one of them is not a name.
    allowed: Option<Result<Vec<String>, String>>,
}

/// What a declared route is, as a need is checked against it: its name and kind from the daemon's file, its wires, context
/// length and tool calling as its gateway listed them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub route: String,
    pub kind: String,
    pub wires: Vec<String>,
    pub max_model_len: Option<u64>,
    pub tool_calling: Option<bool>,
}

impl Facts {
    /// The facts of a row of the view `routes`.
    pub fn of_row(row: &Value) -> Facts {
        Facts {
            route: row["route"].as_str().unwrap_or_default().to_owned(),
            kind: row["kind"].as_str().unwrap_or_default().to_owned(),
            wires: row["wires"].as_array().map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect()).unwrap_or_default(),
            max_model_len: row["max_model_len"].as_u64(),
            tool_calling: row["tool_calling"].as_bool(),
        }
    }
}

fn is_name(text: &str) -> bool {
    crate::routes::is_name(text)
}

/// The entry of `role` from its value.
fn entry_of(role: &str, v: &serde_yaml::Value) -> Entry {
    let Some(m) = v.as_mapping() else {
        return Entry::Invalid(format!("role {role}: an entry is a mapping with `route` (a daemon route by name) or `needs`"));
    };
    let keys: Vec<Option<&str>> = m.keys().map(serde_yaml::Value::as_str).collect();
    if let Some(key) = keys.iter().find(|k| !matches!(k, Some("route" | "needs"))) {
        let shown = key.filter(|k| is_name(k)).map_or_else(|| "a member".to_owned(), |k| format!("`{k}`"));
        return Entry::Invalid(format!("role {role}: {shown} is not a member of an entry: an entry names a daemon route by `route` or states `needs`, never an address, a token or an alias"));
    }
    match (m.get("route"), m.get("needs")) {
        (Some(_), Some(_)) => Entry::Invalid(format!("role {role}: an entry has `route` or `needs`, not both")),
        (Some(r), None) => match r.as_str() {
            Some(name) if is_name(name) => Entry::Named(name.to_owned()),
            _ => Entry::Invalid(format!("role {role}: `route` is a daemon route by name (1 to 64 letters, digits, '.', '_' or '-'), never an address, a token or an alias")),
        },
        (None, Some(n)) => needs_of(role, n).map_or_else(Entry::Invalid, Entry::Needs),
        (None, None) => Entry::Invalid(format!("role {role}: an entry has `route` (a daemon route by name) or `needs`")),
    }
}

fn needs_of(role: &str, v: &serde_yaml::Value) -> Result<Needs, String> {
    let m = v.as_mapping().ok_or_else(|| format!("role {role}: `needs` is a mapping of {}", NEEDS.join(", ")))?;
    let mut needs = Needs::default();
    for (k, x) in m {
        match k.as_str() {
            Some("wire") => needs.wire = Some(x.as_str().filter(|w| is_name(w)).ok_or_else(|| format!("role {role}: `needs.wire` is a wire by name (chat, responses, messages)"))?.to_owned()),
            Some("min_context") => needs.min_context = Some(x.as_u64().ok_or_else(|| format!("role {role}: `needs.min_context` is a whole number of tokens"))?),
            Some("tool_calling") => needs.tool_calling = Some(x.as_bool().ok_or_else(|| format!("role {role}: `needs.tool_calling` is true or false"))?),
            Some("kind") => needs.kind = Some(x.as_str().filter(|k| KINDS.contains(k)).ok_or_else(|| format!("role {role}: `needs.kind` is one of {}", KINDS.join(", ")))?.to_owned()),
            other => {
                let shown = other.filter(|k| is_name(k)).map_or_else(|| "a member".to_owned(), |k| format!("`{k}`"));
                return Err(format!("role {role}: {shown} is not a requirement (known: {})", NEEDS.join(", ")));
            }
        }
    }
    Ok(needs)
}

/// Reads the text of the file of the runbook `runbook`. An error names the file and what is wrong with it as a whole; an
/// entry that is not one is kept and refused by [`admit`] with its role.
pub fn parse(runbook: &str, text: &str) -> Result<Declared, String> {
    let named = file_name(runbook);
    let doc: serde_yaml::Value = serde_yaml::from_str(text).map_err(|_| format!("{named}: not YAML"))?;
    let top = doc.as_mapping().ok_or_else(|| format!("{named}: a mapping with format, model_routes and routes_allowed"))?;
    for k in top.keys() {
        if !k.as_str().is_some_and(|k| MEMBERS.contains(&k)) {
            let shown = k.as_str().filter(|k| is_name(k)).map_or_else(|| "a member".to_owned(), |k| format!("`{k}`"));
            return Err(format!("{named}: {shown} is not a member (known: {})", MEMBERS.join(", ")));
        }
    }
    if top.get("format").and_then(serde_yaml::Value::as_str) != Some(FORMAT) {
        return Err(format!("{named}: the format is not {FORMAT}"));
    }
    let mut entries = BTreeMap::new();
    match top.get("model_routes") {
        None | Some(serde_yaml::Value::Null) => {}
        Some(serde_yaml::Value::Mapping(m)) => {
            if m.len() > MAX_ROLES {
                return Err(format!("{named}: more than {MAX_ROLES} roles"));
            }
            for (k, v) in m {
                let role = k.as_str().filter(|r| is_name(r)).ok_or_else(|| format!("{named}: a role is 1 to 64 letters, digits, '.', '_' or '-'"))?;
                let entry = match crate::relay::prefixed_role_refusal(role, &format!("model_routes.{role}")) {
                    Some(why) => Entry::Invalid(why),
                    None => entry_of(role, v),
                };
                entries.insert(role.to_owned(), entry);
            }
        }
        Some(_) => return Err(format!("{named}: model_routes is a mapping of roles")),
    }
    let allowed = match top.get("routes_allowed") {
        None | Some(serde_yaml::Value::Null) => None,
        Some(serde_yaml::Value::Sequence(s)) => Some(
            s.iter()
                .map(|r| r.as_str().filter(|n| is_name(n)).map(str::to_owned).ok_or_else(|| "routes_allowed holds an entry that is not a route name".to_owned()))
                .collect::<Result<Vec<_>, _>>(),
        ),
        Some(_) => Some(Err("routes_allowed is a list of route names".to_owned())),
    };
    Ok(Declared { entries, allowed })
}

impl Declared {
    /// Whether the file has an entry for `role`.
    pub fn has_role(&self, role: &str) -> bool {
        self.entries.contains_key(role)
    }
}

/// The setting that chose the route of a role of a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum By {
    /// The submit named it (`model_route`, `model_routes.<role>`).
    Run,
    /// The runbook's entry for the role: the route it names, or the route that meets its needs.
    Protocol,
    /// The daemon's `route_policy.default`.
    Default,
}

/// What a submit names of its routes, and the daemon's default route.
#[derive(Clone, Copy, Debug)]
pub struct Asked<'a> {
    /// The route of the role `default` (`model_route`).
    pub route: Option<&'a str>,
    /// The route of each further role (`model_routes`).
    pub roles: &'a BTreeMap<String, String>,
    /// The daemon's `route_policy.default`.
    pub default: Option<&'a str>,
}

/// The `routes_allowed` of `declared`, each name a declared route; `Err` names the runbook and what is wrong with it.
fn allowed_of<'d>(runbook: &str, declared: &'d Declared, names: &[&str]) -> Result<Option<&'d Vec<String>>, String> {
    match &declared.allowed {
        None => Ok(None),
        Some(Err(why)) => Err(format!("runbook {runbook}: {why}")),
        Some(Ok(list)) => match list.iter().find(|r| !names.contains(&r.as_str())) {
            Some(r) => Err(format!("runbook {runbook}: routes_allowed names the route {r}, which the daemon does not declare (declared: {})", listed(names))),
            None => Ok(Some(list)),
        },
    }
}

fn listed(names: &[&str]) -> String {
    if names.is_empty() {
        "none".to_owned()
    } else {
        names.join(", ")
    }
}

fn may(allowed: Option<&Vec<String>>, route: &str) -> bool {
    allowed.is_none_or(|a| a.iter().any(|r| r == route))
}

/// What [`check`] refuses of the file of `runbook` whatever routes the daemon declares: a `routes_allowed` that is not a
/// list of route names, then the first entry that is not one. `Err` names the runbook and, for an entry, the role. The
/// daemon asks it through [`check`] at every submit and delegation; `runbook check` asks it with no daemon.
pub fn shape(runbook: &str, declared: &Declared) -> Result<(), String> {
    if let Some(Err(why)) = &declared.allowed {
        return Err(format!("runbook {runbook}: {why}"));
    }
    match declared.entries.values().find_map(|e| if let Entry::Invalid(why) = e { Some(why) } else { None }) {
        Some(why) => Err(format!("runbook {runbook}: {why}")),
        None => Ok(()),
    }
}

/// Whether the file of `runbook` can be admitted against the declared routes `routes`, whatever a run names: `Err` is the
/// first reason it cannot, naming the runbook and the role and the route or the requirement. An entry that is not one, a
/// named route the daemon does not declare or `routes_allowed` does not hold, needs that no allowed route meets and a
/// `routes_allowed` that names an undeclared route are refused; needs that several routes meet are left to the run
/// ([`for_run`]).
pub fn check(runbook: &str, declared: &Declared, routes: &[Facts]) -> Result<(), String> {
    shape(runbook, declared)?;
    let names: Vec<&str> = routes.iter().map(|f| f.route.as_str()).collect();
    let allowed = allowed_of(runbook, declared, &names)?;
    for (role, entry) in &declared.entries {
        match entry {
            Entry::Invalid(why) => return Err(format!("runbook {runbook}: {why}")),
            Entry::Named(r) if !names.contains(&r.as_str()) => return Err(format!("runbook {runbook}: role {role}: the route {r} is not one the daemon declares (declared: {})", listed(&names))),
            Entry::Named(r) if !may(allowed, r) => return Err(format!("runbook {runbook}: role {role}: the route {r} is not in routes_allowed ({})", allowed.map(|a| a.join(", ")).unwrap_or_default())),
            Entry::Needs(n) if !routes.iter().any(|f| may(allowed, &f.route) && n.met_by(f)) => return Err(format!("runbook {runbook}: role {role}: no declared route meets {}", n.words())),
            _ => {}
        }
    }
    Ok(())
}

/// The route of each role of a run of `runbook` and the setting that chose it (`docs/model-routes.md`, section 2): the
/// run's word first (`asked.route` for the role `default`, `asked.roles` for the others), then the runbook's entry (a
/// named route, or for needs the run's route or the daemon's default when it meets them, else the one allowed route that
/// does), then the daemon's default for the role `default`. Every route must be in `routes_allowed`, and a route the run
/// names for a role with needs must meet them. `Err` is the reason of `route-refused`: it names the runbook, and the role,
/// the route and the member or the setting that chose it, or the requirement. The role `default` is absent when neither
/// the run, the runbook nor the daemon names one.
pub fn for_run(runbook: &str, declared: &Declared, routes: &[Facts], asked: Asked<'_>) -> Result<BTreeMap<String, (String, By)>, String> {
    check(runbook, declared, routes)?;
    let names: Vec<&str> = routes.iter().map(|f| f.route.as_str()).collect();
    let allowed = allowed_of(runbook, declared, &names)?;
    let narrows = |route: &str, said: &str| -> Result<(), String> {
        if may(allowed, route) {
            Ok(())
        } else {
            Err(format!("runbook {runbook}: the route {route} ({said}) is not in routes_allowed ({})", allowed.map(|a| a.join(", ")).unwrap_or_default()))
        }
    };
    let mut out = BTreeMap::new();
    for (role, entry) in &declared.entries {
        let (member, word) = if role == "default" { ("model_route".to_owned(), asked.route) } else { (format!("model_routes.{role}"), asked.roles.get(role).map(String::as_str)) };
        let chosen = match (word, entry) {
            (Some(w), Entry::Needs(n)) => {
                narrows(w, &member)?;
                if !routes.iter().any(|f| f.route == w && n.met_by(f)) {
                    return Err(format!("runbook {runbook}: role {role}: the route {w} ({member}) does not meet {}", n.words()));
                }
                (w.to_owned(), By::Run)
            }
            (Some(w), _) => {
                narrows(w, &member)?;
                (w.to_owned(), By::Run)
            }
            (None, Entry::Named(r)) => (r.clone(), By::Protocol),
            (None, Entry::Needs(n)) => {
                let meet: Vec<&str> = routes.iter().filter(|f| may(allowed, &f.route) && n.met_by(f)).map(|f| f.route.as_str()).collect();
                // the run's route when it meets them, else the daemon's default when it does, else the one route that does
                match ([asked.route, asked.default].into_iter().flatten().find(|d| meet.contains(d)), meet.as_slice()) {
                    (Some(d), _) => (d.to_owned(), By::Protocol),
                    (None, [one]) => ((*one).to_owned(), By::Protocol),
                    (None, []) => return Err(format!("runbook {runbook}: role {role}: no declared route meets {}", n.words())),
                    (None, several) => return Err(format!("runbook {runbook}: role {role}: several routes meet {}: {}; the runbook names one with `route`, or the submit in {member}", n.words(), several.join(", "))),
                }
            }
            (None, Entry::Invalid(why)) => return Err(format!("runbook {runbook}: {why}")),
        };
        out.insert(role.clone(), chosen);
    }
    if !out.contains_key("default") {
        match (asked.route, asked.default) {
            (Some(w), _) => {
                narrows(w, "model_route")?;
                out.insert("default".to_owned(), (w.to_owned(), By::Run));
            }
            (None, Some(d)) => {
                narrows(d, "route_policy.default")?;
                out.insert("default".to_owned(), (d.to_owned(), By::Default));
            }
            (None, None) => {}
        }
    }
    for (role, w) in asked.roles {
        if !out.contains_key(role) {
            narrows(w, &format!("model_routes.{role}"))?;
            out.insert(role.clone(), (w.clone(), By::Run));
        }
    }
    Ok(out)
}

/// What the admission made of a file: one JSON entry per role (the view's `RouteEntry`), the `routes_allowed` as given, and
/// every reason the runbook is not admitted, in role order after the reasons of `routes_allowed`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Admission {
    pub entries: Vec<Value>,
    pub routes_allowed: Option<Vec<String>>,
    pub reasons: Vec<String>,
}

/// Admits the entries of `declared` against the declared routes `routes` (in order of name) with the daemon's default route
/// `default`.
pub fn admit(declared: &Declared, routes: &[Facts], default: Option<&str>) -> Admission {
    let names: Vec<&str> = routes.iter().map(|f| f.route.as_str()).collect();
    let listed = if names.is_empty() { "none".to_owned() } else { names.join(", ") };
    let mut out = Admission::default();
    let allowed: Option<&Vec<String>> = match &declared.allowed {
        None => None,
        Some(Err(why)) => {
            out.reasons.push(why.clone());
            None
        }
        Some(Ok(list)) => {
            for r in list.iter().filter(|r| !names.contains(&r.as_str())) {
                out.reasons.push(format!("routes_allowed names the route {r}, which the daemon does not declare (declared: {listed})"));
            }
            out.routes_allowed = Some(list.clone());
            Some(list)
        }
    };
    let may = |route: &str| allowed.is_none_or(|a| a.iter().any(|r| r == route));
    for (role, entry) in &declared.entries {
        let mut m = Map::new();
        m.insert("role".into(), json!(role));
        let (route, reason) = match entry {
            Entry::Invalid(why) => (None, Some(why.clone())),
            Entry::Named(r) if !names.contains(&r.as_str()) => (None, Some(format!("role {role}: the route {r} is not one the daemon declares (declared: {listed})"))),
            Entry::Named(r) if !may(r) => (None, Some(format!("role {role}: the route {r} is not in routes_allowed ({})", allowed.map(|a| a.join(", ")).unwrap_or_default()))),
            Entry::Named(r) => (Some(r.clone()), None),
            Entry::Needs(n) => {
                m.insert("needs".into(), n.to_json());
                let meet: Vec<&str> = routes.iter().filter(|f| may(&f.route) && n.met_by(f)).map(|f| f.route.as_str()).collect();
                match (default.filter(|d| meet.contains(d)), meet.as_slice()) {
                    (Some(d), _) => (Some(d.to_owned()), None),
                    (None, [one]) => (Some((*one).to_owned()), None),
                    (None, []) => (None, Some(format!("role {role}: no declared route meets {}", n.words()))),
                    (None, several) => (None, Some(format!("role {role}: several routes meet {}: {}; name one with `route`", n.words(), several.join(", ")))),
                }
            }
        };
        if let Some(r) = &route {
            m.insert("route".into(), json!(r));
        }
        m.insert("admitted".into(), json!(reason.is_none()));
        if let Some(why) = reason {
            m.insert("reason".into(), json!(why));
            out.reasons.push(why);
        }
        out.entries.push(Value::Object(m));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(route: &str, kind: &str, wires: &[&str], len: Option<u64>, tools: Option<bool>) -> Facts {
        Facts { route: route.into(), kind: kind.into(), wires: wires.iter().map(|w| (*w).to_owned()).collect(), max_model_len: len, tool_calling: tools }
    }

    fn two() -> Vec<Facts> {
        vec![facts("claude", "hosted", &["messages"], Some(200_000), Some(true)), facts("qwen", "managed", &["chat", "messages"], Some(32_768), None)]
    }

    fn admitted(text: &str, default: Option<&str>) -> Admission {
        admit(&parse("r", text).unwrap(), &two(), default)
    }

    #[test]
    fn needs_take_the_default_route_when_it_meets_them_else_the_one_route_and_several_or_none_are_refused() {
        let a = admitted("format: runbook.routes/1\nmodel_routes:\n  w: {needs: {wire: messages}}\n", Some("qwen"));
        assert_eq!(a.entries, vec![json!({"role": "w", "needs": {"wire": "messages"}, "route": "qwen", "admitted": true})]);
        let a = admitted("format: runbook.routes/1\nmodel_routes:\n  w: {needs: {wire: messages}}\n", None);
        assert_eq!(a.reasons, ["role w: several routes meet wire messages: claude, qwen; name one with `route`"]);
        let a = admitted("format: runbook.routes/1\nmodel_routes:\n  w: {needs: {min_context: 100000, tool_calling: true}}\n", Some("qwen"));
        assert_eq!(a.entries[0]["route"], "claude", "the default does not meet them: the one route that does");
        let a = admitted("format: runbook.routes/1\nmodel_routes:\n  w: {needs: {kind: local}}\n", Some("qwen"));
        assert_eq!(a.reasons, ["role w: no declared route meets kind local"]);
        let a = admitted("format: runbook.routes/1\nmodel_routes:\n  w: {needs: {wire: messages}}\nroutes_allowed: [claude]\n", Some("qwen"));
        assert_eq!((a.entries[0]["route"].clone(), a.routes_allowed), (json!("claude"), Some(vec!["claude".to_owned()])), "the default is not allowed");
    }

    fn for_run_of(text: &str, route: Option<&str>, roles: &[(&str, &str)], default: Option<&str>) -> Result<BTreeMap<String, (String, By)>, String> {
        let roles: BTreeMap<String, String> = roles.iter().map(|(r, n)| ((*r).to_owned(), (*n).to_owned())).collect();
        for_run("r", &parse("r", text).unwrap(), &two(), Asked { route, roles: &roles, default })
    }

    fn chosen(pairs: &[(&str, &str, By)]) -> BTreeMap<String, (String, By)> {
        pairs.iter().map(|(role, route, by)| ((*role).to_owned(), ((*route).to_owned(), *by))).collect()
    }

    #[test]
    fn a_run_takes_its_own_word_then_the_protocols_then_the_default_and_every_route_must_be_allowed() {
        let file = "format: runbook.routes/1\nmodel_routes:\n  writer: {route: qwen}\n  review: {needs: {wire: messages, tool_calling: true}}\n";
        assert_eq!(for_run_of(file, None, &[], Some("qwen")), Ok(chosen(&[("default", "qwen", By::Default), ("review", "claude", By::Protocol), ("writer", "qwen", By::Protocol)])));
        assert_eq!(for_run_of(file, Some("claude"), &[("writer", "claude"), ("extra", "qwen")], None), Ok(chosen(&[("default", "claude", By::Run), ("extra", "qwen", By::Run), ("review", "claude", By::Protocol), ("writer", "claude", By::Run)])));
        assert_eq!(for_run_of(file, None, &[("review", "qwen")], None), Err("runbook r: role review: the route qwen (model_routes.review) does not meet wire messages, tool_calling".to_owned()));
        assert_eq!(for_run_of("format: runbook.routes/1\nmodel_routes:\n  default: {route: claude}\n", None, &[], Some("qwen")), Ok(chosen(&[("default", "claude", By::Protocol)])));
        let narrow = "format: runbook.routes/1\nroutes_allowed: [claude]\n";
        assert_eq!(for_run_of(narrow, None, &[], Some("qwen")), Err("runbook r: the route qwen (route_policy.default) is not in routes_allowed (claude)".to_owned()));
        assert_eq!(for_run_of(narrow, Some("qwen"), &[], None), Err("runbook r: the route qwen (model_route) is not in routes_allowed (claude)".to_owned()));
        assert_eq!(for_run_of(narrow, None, &[("w", "qwen")], Some("claude")), Err("runbook r: the route qwen (model_routes.w) is not in routes_allowed (claude)".to_owned()));
        assert_eq!(for_run_of(narrow, None, &[], None), Ok(BTreeMap::new()), "no route named anywhere: the caller decides");
        let several = "format: runbook.routes/1\nmodel_routes:\n  w: {needs: {wire: messages}}\n";
        assert!(for_run_of(several, None, &[], None).is_err_and(|e| e.contains("role w: several routes meet wire messages: claude, qwen")));
        assert_eq!(for_run_of(several, Some("claude"), &[], None), Ok(chosen(&[("default", "claude", By::Run), ("w", "claude", By::Protocol)])), "the run's route meets the needs");
    }

    #[test]
    fn check_refuses_what_no_run_can_admit_and_leaves_several_routes_to_the_run() {
        let ok = |text: &str| check("r", &parse("r", text).unwrap(), &two());
        assert_eq!(ok("format: runbook.routes/1\nmodel_routes:\n  w: {needs: {wire: messages}}\n"), Ok(()));
        assert_eq!(ok("format: runbook.routes/1\nmodel_routes:\n  w: {route: gpt}\n"), Err("runbook r: role w: the route gpt is not one the daemon declares (declared: claude, qwen)".to_owned()));
        assert_eq!(ok("format: runbook.routes/1\nmodel_routes:\n  w: {needs: {kind: local}}\n"), Err("runbook r: role w: no declared route meets kind local".to_owned()));
        assert_eq!(ok("format: runbook.routes/1\nmodel_routes:\n  w: {needs: {kind: managed}}\nroutes_allowed: [claude]\n"), Err("runbook r: role w: no declared route meets kind managed".to_owned()));
        assert_eq!(ok("format: runbook.routes/1\nroutes_allowed: [nope]\n"), Err("runbook r: routes_allowed names the route nope, which the daemon does not declare (declared: claude, qwen)".to_owned()));
        let none = check("r", &parse("r", "format: runbook.routes/1\nmodel_routes:\n  w: {route: qwen}\n").unwrap(), &[]);
        assert_eq!(none, Err("runbook r: role w: the route qwen is not one the daemon declares (declared: none)".to_owned()));
    }

    #[test]
    fn a_fact_the_gateway_did_not_give_meets_no_requirement() {
        let unread = vec![facts("qwen", "managed", &[], None, None)];
        let a = admit(&parse("r", "format: runbook.routes/1\nmodel_routes:\n  w: {needs: {min_context: 1}}\n").unwrap(), &unread, Some("qwen"));
        assert_eq!(a.reasons, ["role w: no declared route meets min_context 1"]);
    }

    #[test]
    fn named_routes_must_be_declared_and_allowed_and_routes_allowed_names_declared_routes() {
        let a = admitted("format: runbook.routes/1\nmodel_routes:\n  a: {route: gpt}\n  b: {route: claude}\n  c: {route: qwen}\nroutes_allowed: [qwen, nope]\n", None);
        assert_eq!(
            a.reasons,
            [
                "routes_allowed names the route nope, which the daemon does not declare (declared: claude, qwen)",
                "role a: the route gpt is not one the daemon declares (declared: claude, qwen)",
                "role b: the route claude is not in routes_allowed (qwen, nope)",
            ]
        );
        assert_eq!(a.entries[2], json!({"role": "c", "route": "qwen", "admitted": true}));
    }

    #[test]
    fn an_address_a_token_or_an_unknown_member_is_refused_by_name_and_never_echoed() {
        let a = admitted("format: runbook.routes/1\nmodel_routes:\n  a: {route: \"http://10.0.0.1:9/v1\"}\n  b: {token: sk-secret-1}\n  c: {needs: {base_url: \"http://h\"}}\n  d: {route: qwen, needs: {}}\n  e: {}\n", None);
        let text = format!("{:?}", a.reasons);
        assert!(!text.contains("10.0.0.1") && !text.contains("sk-secret") && !text.contains("http://h"), "{text}");
        assert_eq!(a.reasons.len(), 5, "{text}");
        assert!(a.reasons[1].contains("`token` is not a member") && a.reasons[2].contains("`base_url` is not a requirement"), "{text}");
        for text in ["format: other/1\n", "- a\n", "format: runbook.routes/1\nextra: 1\n", "format: runbook.routes/1\nmodel_routes: [a]\n", "format: runbook.routes/1\nmodel_routes:\n  \"a b\": {route: qwen}\n", ": :\n  - ["] {
            let why = parse("r", text).unwrap_err();
            assert!(why.starts_with("bindings/r.routes.yaml: "), "{why}");
        }
        let a = admitted("format: runbook.routes/1\nroutes_allowed: [\"http://h\"]\n", None);
        assert_eq!(a.reasons, ["routes_allowed holds an entry that is not a route name"]);
    }

    /// The relay takes the model `runbook-<role>` for the role `<role>`: a role whose name starts with `runbook-` would be
    /// taken for another, and the entry is refused naming its member.
    #[test]
    fn a_role_whose_name_starts_with_runbook_dash_is_refused_naming_its_member() {
        let declared = parse("r", "format: runbook.routes/1\nmodel_routes:\n  runbook-x: {route: claude}\n  x: {route: qwen}\n").unwrap();
        let a = admit(&declared, &two(), None);
        assert_eq!(a.reasons.len(), 1, "{:?}", a.reasons);
        assert!(a.reasons[0].contains("model_routes.runbook-x") && a.reasons[0].contains("`runbook-`"), "{:?}", a.reasons);
        let why = check("r", &declared, &two()).unwrap_err();
        assert!(why.starts_with("runbook r: ") && why.contains("model_routes.runbook-x"), "{why}");
    }
}
