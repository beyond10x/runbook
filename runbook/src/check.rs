//! `runbook check`: the consistency of the text of a runbook under the asset root.
//!
//! A runbook is text: protocols, two forms of bindings, instruction files, views and the cycles
//! list. This reads that text and reports each inconsistency as one finding: the file, what is
//! wrong, and the fix. It does not run anything and calls no program: the checks that need `canon`,
//! `ess` or the generated site data stay in `scripts/check-bindings.sh`.
//!
//! The checks, in the order of their findings:
//! 1. `layout`: every part of `assets::LAYOUT` is present (the parts `runbook doctor` lists).
//! 2. `bindings`: each `bindings/*.yaml` and `driver/examples/sdc/*.bindings.json` parses; each
//!    instruction file a binding names (`--instructions` of an action of the YAML form,
//!    `llm.instructions` of the JSON form) is `instructions/<...>.md` without a `..` segment and
//!    exists; the two forms name the same files per action; each program a YAML action names with a
//!    `/` is an executable `tools/<file>` under `bindings/`, and no argument holds a `..` segment.
//! 3. `instructions`: each `{{placeholder}}` is `{{[a-z_]+}}`; a file nothing names is reported.
//!    `instructions/golden/` (rendered prompts kept as test data) is left out of both.
//! 4. `views`: each `views/*.view.json` parses, is an object and has `"format": "runbook.view/1"`.
//! 5. `cycles`: `cycles/tree-suite.yaml` parses, has `loom.tree-suite/1`, unique cycle names and
//!    the seeds each cycle names; each name is one of the entries of the old suite, listed in
//!    `cycles/old-suite-names.txt`; and, while `docs/cycles-suite.md` exists, its table has one row
//!    per listed entry, and a row that says converted or pending is a cycle of the suite file.

use crate::assets::LAYOUT;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const VIEW_FORMAT: &str = "runbook.view/1";
pub const CYCLES_FORMAT: &str = "loom.tree-suite/1";
pub const CYCLES_SUITE: &str = "cycles/tree-suite.yaml";
pub const CYCLES_NAMES: &str = "cycles/old-suite-names.txt";
pub const CYCLES_TABLE: &str = "docs/cycles-suite.md";

const LAYOUT_FIX: &str = "restore it in the asset root: it holds every part of the text layout, as the repository does";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub check: &'static str,
    /// Relative to the asset root.
    pub file: String,
    pub what: String,
    pub fix: String,
}

#[derive(Debug, Default)]
pub struct Report {
    pub root: PathBuf,
    pub findings: Vec<Finding>,
    pub notes: Vec<String>,
    /// What was read, for the summary line.
    pub summary: String,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.findings.is_empty()
    }

    fn add(&mut self, check: &'static str, file: &str, what: impl Into<String>, fix: impl Into<String>) {
        self.findings.push(Finding { check, file: file.to_string(), what: what.into(), fix: fix.into() });
    }
}

pub fn run(root: &Path) -> Report {
    let mut r = Report { root: root.to_path_buf(), ..Report::default() };
    let mut seen = Seen::default();
    layout(root, &mut r);
    bindings(root, &mut r, &mut seen);
    instructions(root, &mut r, &mut seen);
    views(root, &mut r, &mut seen);
    cycles(root, &mut r, &mut seen);
    r.summary = seen.line();
    r
}

#[derive(Default)]
struct Seen {
    yaml: usize,
    json: usize,
    instructions: usize,
    views: usize,
    cycles: usize,
    table: Option<usize>,
}

impl Seen {
    fn line(&self) -> String {
        let table = match self.table {
            Some(n) => format!(", {n} compared with {CYCLES_TABLE}"),
            None => format!(", not compared ({CYCLES_TABLE} absent)"),
        };

        format!(
            "{} bindings files and {} JSON bindings, {} instruction files, {} views, {} cycles{table}",
            self.yaml, self.json, self.instructions, self.views, self.cycles
        )
    }
}

// ---------------------------------------------------------------------------------------- layout

fn layout(root: &Path, r: &mut Report) {
    for part in LAYOUT.iter().filter(|p| !p.present(root)) {
        r.add("layout", part.name, format!("missing ({})", part.absent_reason()), LAYOUT_FIX);
    }
}

// -------------------------------------------------------------------------------------- helpers

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

/// The files directly in `dir` whose name ends with `suffix`, sorted. A missing directory has none.
fn files_ending(dir: &Path, suffix: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| d.flatten().map(|e| e.path()).filter(|p| p.is_file() && p.file_name().is_some_and(|n| n.to_string_lossy().ends_with(suffix))).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// Every file below `dir` ending with `suffix`, sorted, leaving out the top-level directories in `skip`.
fn files_below(dir: &Path, suffix: &str, skip: &[&str]) -> Vec<PathBuf> {
    fn walk(d: &Path, suffix: &str, skip: &[&str], top: bool, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if p.is_dir() {
                if !(top && skip.contains(&name.as_str())) {
                    walk(&p, suffix, skip, false, out);
                }
            } else if p.is_file() && name.ends_with(suffix) {
                out.push(p);
            }
        }
    }
    let mut v = Vec::new();
    walk(dir, suffix, skip, true, &mut v);
    v.sort();
    v
}

fn yaml_to_json(v: serde_yaml::Value) -> Value {
    use serde_yaml::Value as Y;
    match v {
        Y::Null => Value::Null,
        Y::Bool(b) => Value::Bool(b),
        Y::Number(n) => serde_json::to_value(n).unwrap_or(Value::Null),
        Y::String(s) => Value::String(s),
        Y::Sequence(s) => Value::Array(s.into_iter().map(yaml_to_json).collect()),
        Y::Mapping(m) => {
            let mut o = Map::new();
            for (k, v) in m {
                let key = match k {
                    Y::String(s) => s,
                    other => serde_yaml::to_string(&other).unwrap_or_default().trim_end().to_string(),
                };
                o.insert(key, yaml_to_json(v));
            }
            Value::Object(o)
        }
        Y::Tagged(t) => yaml_to_json(t.value),
    }
}

fn read_yaml(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_yaml::from_str::<serde_yaml::Value>(&text).map(yaml_to_json).map_err(|e| e.to_string())
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str::<Value>(&text).map_err(|e| e.to_string())
}

/// Whether some `/`-separated segment of `p` is `..`.
fn has_dotdot(p: &str) -> bool {
    p.split('/').any(|s| s == "..")
}

fn instruction_path_ok(root: &Path, p: &str) -> bool {
    p.strip_prefix("instructions/").is_some_and(|t| t.len() > ".md".len() && t.ends_with(".md")) && !has_dotdot(p) && root.join(p).is_file()
}

// ------------------------------------------------------------------------------------- bindings

/// The `--instructions` value of an action's argv: `None` without the option, `Some(Err)` when it is
/// no JSON object.
fn argv_instructions(argv: &[&str]) -> Option<Result<Map<String, Value>, String>> {
    let i = (0..argv.len().saturating_sub(1)).find(|&i| argv[i] == "--instructions")?;
    Some(match serde_json::from_str::<Value>(argv[i + 1]) {
        Ok(Value::Object(m)) => Ok(m),
        _ => Err(argv[i + 1].to_string()),
    })
}

fn strings(v: &Value) -> Vec<&str> {
    v.as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default()
}

fn bindings(root: &Path, r: &mut Report, seen: &mut Seen) {
    let jsons = files_ending(&root.join("driver/examples/sdc"), ".bindings.json");
    let yamls = files_ending(&root.join("bindings"), ".yaml");
    seen.json = jsons.len();
    seen.yaml = yamls.len();
    let mut json_docs: BTreeMap<String, Value> = BTreeMap::new();
    for p in &jsons {
        let f = rel(root, p);
        match read_json(p) {
            Ok(d) if d.is_object() => {
                let name = p.file_name().unwrap().to_string_lossy().trim_end_matches(".bindings.json").to_string();
                json_docs.insert(name, d);
            }
            Ok(_) => r.add("bindings", &f, "is not a JSON object", "make the top level an object of actions"),
            Err(e) => r.add("bindings", &f, format!("does not parse: {e}"), "fix the JSON"),
        }
    }
    let mut yaml_docs: BTreeMap<String, Value> = BTreeMap::new();
    for p in &yamls {
        let f = rel(root, p);
        match read_yaml(p) {
            Ok(d) => {
                let name = p.file_stem().unwrap().to_string_lossy().into_owned();
                yaml_docs.insert(name, d);
            }
            Err(e) => r.add("bindings", &f, format!("does not parse: {e}"), "fix the YAML"),
        }
    }
    for name in json_docs.keys().filter(|n| !yamls.iter().any(|p| p.file_stem().is_some_and(|s| s.to_string_lossy() == **n))) {
        r.add("bindings", &format!("driver/examples/sdc/{name}.bindings.json"), format!("has no bindings/{name}.yaml"), format!("write bindings/{name}.yaml (loom.bindings/1) beside it"));
    }
    for (name, doc) in &yaml_docs {
        yaml_bindings(root, r, name, doc, json_docs.get(name));
    }
    for (name, doc) in &json_docs {
        json_instructions(root, r, name, doc);
    }
}

fn yaml_bindings(root: &Path, r: &mut Report, name: &str, doc: &Value, json: Option<&Value>) {
    let file = format!("bindings/{name}.yaml");
    let Some(actions) = doc.get("actions").and_then(Value::as_array) else {
        r.add("bindings", &file, "has no actions list", "list the actions under `actions:`");
        return;
    };
    for a in actions {
        let Some(action) = a.get("action").and_then(Value::as_str) else {
            r.add("bindings", &file, "an entry of actions has no `action` name", "name it with `action:`");
            continue;
        };
        let argv = a.pointer("/executor/argv").map(strings).unwrap_or_default();
        let from = a.get("arguments_from").map(strings).unwrap_or_default();
        for p in argv.iter().chain(from.iter()).filter(|p| has_dotdot(p)) {
            r.add("bindings", &file, format!("{action} names {p}: a path with .."), "name the program as tools/<file>, with no .. segment");
        }
        for (member, v) in [("executor.argv[0]", argv.first()), ("arguments_from[0]", from.first())] {
            let Some(p) = v.filter(|p| p.contains('/')) else { continue };
            let ok = p.strip_prefix("tools/").is_some_and(|t| !t.is_empty() && !t.contains('/') && crate::assets::is_executable(&root.join("bindings").join(p)));
            if !ok {
                r.add("bindings", &file, format!("{action} {member} names {p}: not an executable bindings/tools/<file>"), "add the program to bindings/tools/ with the execute bit, or name one that is there");
            }
        }
        if let Some(defect) = budget_defect(a) {
            r.add("bindings", &file, format!("{action}: {defect}"), "write the budget as docs/loom-bindings.md (\"The budget\") says: Loom refuses it at load");
        }
        let yaml_names = match argv_instructions(&argv) {
            None => None,
            Some(Ok(m)) => Some(Value::Object(m)),
            Some(Err(_)) => {
                r.add("bindings", &file, format!("{action}: --instructions is not a JSON object"), "give --instructions a JSON object of file names, {\"task\":\"instructions/<name>.md\"}");
                continue;
            }
        };
        if let Some(Value::Object(m)) = &yaml_names {
            for p in m.values() {
                instruction_file(root, r, &file, action, p);
            }
        }
        if let Some(j) = json.and_then(|j| j.get(action)) {
            let d = j.pointer("/llm/instructions").cloned();
            if yaml_names != d {
                let show = |v: &Option<Value>| v.as_ref().map(Value::to_string).unwrap_or_else(|| "none".into());
                r.add(
                    "bindings",
                    &file,
                    format!("{action}: --instructions {} differs from llm.instructions {} of driver/examples/sdc/{name}.bindings.json", show(&yaml_names), show(&d)),
                    "make the two forms name the same instruction files",
                );
            }
        }
    }
}

/// What a value is, for a message, as Loom's decoder says it.
fn shown(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::String(s) => format!("{s:?}"),
        Value::Array(_) => "a list".into(),
        Value::Object(_) => "an object".into(),
        other => other.to_string(),
    }
}

/// The first defect of the rework members of an action's `budget` that Loom refuses at load, in
/// Loom's words after the member (`story:rework-allowance-after-a-rejection`): `runs_per` is `case`
/// or `rejection`; `rework_limit` a whole number from 1; `rework_on` a non-empty map of evidence
/// kinds to non-empty lists of results; `rework_limit` and `rework_on` are read only under
/// `runs_per: rejection`, which needs a `retry_when` that lists a result, and each pair of
/// `rework_on` is one `retry_when` lists; the three members belong to the program and person
/// executors. `scripts/check-bindings.sh` (`check_budget`) says the same in `jq`.
fn budget_defect(a: &Value) -> Option<String> {
    let member = |k: &str| a.pointer("/budget").and_then(|b| b.get(k));
    let rejection = member("runs_per").and_then(Value::as_str) == Some("rejection");
    if let Some(v) = member("runs_per").filter(|v| !matches!(v.as_str(), Some("case" | "rejection"))) {
        return Some(format!("budget.runs_per is {}; it is case or rejection", shown(v)));
    }
    if let Some(v) = member("rework_limit").filter(|v| !v.as_i64().is_some_and(|n| n >= 1)) {
        return Some(format!("budget.rework_limit is {}; it is a whole number from 1", shown(v)));
    }
    if let Some(defect) = member("rework_on").and_then(rework_on_defect) {
        return Some(defect);
    }
    let retry: Vec<(&str, Vec<&str>)> = a.get("retry_when").and_then(Value::as_object).map(|m| m.iter().map(|(k, v)| (k.as_str(), strings(v))).collect()).unwrap_or_default();
    if !rejection {
        for m in ["rework_limit", "rework_on"] {
            if member(m).is_some() {
                return Some(format!("budget.{m} is read only under runs_per: rejection"));
            }
        }
    } else if retry.iter().all(|(_, results)| results.is_empty()) {
        return Some("budget.runs_per is rejection and retry_when lists no result, so no record could open an allowance".into());
    } else if let Some(on) = member("rework_on").and_then(Value::as_object) {
        for (kind, results) in on {
            for result in strings(results) {
                if !retry.iter().any(|(k, listed)| k == kind && listed.contains(&result)) {
                    return Some(format!("budget.rework_on lists {kind} {result}, which retry_when does not list; a rejection is a record that sends the action back"));
                }
            }
        }
    }
    let kind = a.pointer("/executor/kind").and_then(Value::as_str).filter(|k| !matches!(*k, "program" | "person"))?;
    let named: String = kind.chars().take(1).flat_map(char::to_uppercase).chain(kind.chars().skip(1)).collect();
    ["rework_limit", "rework_on", "runs_per"].into_iter().find(|m| member(m).is_some()).map(|m| format!("budget.{m} belongs to the Program and Person executors, not {named}"))
}

/// The structure of `budget.rework_on`: a map of kinds to lists of strings, none empty, and itself
/// not empty (Loom's `rework_on`, after the shape of `retry_when`).
fn rework_on_defect(v: &Value) -> Option<String> {
    const M: &str = "budget.rework_on";
    let Some(map) = v.as_object() else {
        return Some(format!("{M} is {}; it is an object", shown(v)));
    };
    for (kind, results) in map {
        let Some(list) = results.as_array() else {
            return Some(format!("{M}.{kind} is {}; it is a list of strings", shown(results)));
        };
        if let Some((i, item)) = list.iter().enumerate().find(|(_, item)| !item.is_string()) {
            return Some(format!("{M}.{kind}[{i}] is {}; it is a string", shown(item)));
        }
        if let Some(i) = list.iter().position(|item| item.as_str() == Some("")) {
            return Some(format!("{M}.{kind}[{i}] is empty"));
        }
    }
    if map.is_empty() {
        return Some(format!("{M} is empty; it names the kinds and results that are rejections"));
    }
    map.iter().find(|(_, results)| results.as_array().is_some_and(Vec::is_empty)).map(|(kind, _)| format!("{M}.{kind} is empty; it lists the results of the kind that are rejections"))
}

fn json_instructions(root: &Path, r: &mut Report, name: &str, doc: &Value) {
    let file = format!("driver/examples/sdc/{name}.bindings.json");
    let Some(o) = doc.as_object() else { return };
    for (action, v) in o.iter().filter(|(k, _)| !k.starts_with('$')) {
        match v.pointer("/llm/instructions") {
            None | Some(Value::Null) => {}
            Some(Value::Object(m)) => m.values().for_each(|p| instruction_file(root, r, &file, action, p)),
            Some(_) => r.add("bindings", &file, format!("{action}: llm.instructions is not an object"), "give llm.instructions an object of file names"),
        }
    }
}

fn instruction_file(root: &Path, r: &mut Report, file: &str, action: &str, p: &Value) {
    match p.as_str() {
        Some(s) if instruction_path_ok(root, s) => {}
        Some(s) => r.add("bindings", file, format!("{action}: instruction file {s} is not an existing instructions/<...>.md without .."), "name an instruction file that exists under instructions/, or add it"),
        None => r.add("bindings", file, format!("{action}: an instruction file name is not a string ({p})"), "name each instruction file as a string, instructions/<...>.md"),
    }
}

// ------------------------------------------------------------------------------- instructions

/// The placeholders of `text` that are not `{{[a-z_]+}}`: each `{{` opens one that ends at the next
/// `}}` of the same line.
pub fn bad_placeholders(text: &str) -> Vec<String> {
    let mut bad = Vec::new();
    for line in text.lines() {
        let mut rest = line;
        while let Some(i) = rest.find("{{") {
            let after = &rest[i + 2..];
            match after.find("}}") {
                Some(j) => {
                    let name = &after[..j];
                    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') {
                        bad.push(format!("{{{{{}}}}}", name.chars().take(40).collect::<String>()));
                    }
                    rest = &after[j + 2..];
                }
                None => {
                    bad.push(format!("{{{{{}", after.chars().take(20).collect::<String>()));
                    rest = after;
                }
            }
        }
    }
    bad
}

/// Whether `name` occurs in `text` as a whole file name: not inside a longer name.
fn names_file(text: &str, name: &str) -> bool {
    let before_ok = |c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    let after_ok = |c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-'));
    text.match_indices(name).any(|(i, m)| {
        text[..i].chars().next_back().is_none_or(before_ok) && text[i + m.len()..].chars().next().is_none_or(after_ok)
    })
}

fn instructions(root: &Path, r: &mut Report, seen: &mut Seen) {
    let dir = root.join("instructions");
    let files = files_below(&dir, ".md", &["golden"]);
    seen.instructions = files.len();
    let mut texts: Vec<(String, String)> = Vec::new();
    for p in &files {
        let f = rel(root, p);
        match std::fs::read_to_string(p) {
            Ok(t) => {
                for b in bad_placeholders(&t) {
                    r.add("instructions", &f, format!("placeholder {b} is not of the form {{{{[a-z_]+}}}}"), "rename it to lower-case letters and underscores, or take the braces out");
                }
                texts.push((f, t));
            }
            Err(e) => r.add("instructions", &f, format!("cannot be read: {e}"), "make it a readable UTF-8 file"),
        }
    }
    // What can name an instruction file: the bindings (both forms), the programs of the bindings,
    // the Rust sources of the driver crate (`driver/src`, where `sdc-reply` and `sdc-conform`
    // read theirs) and of the runner (`runbook/src`, where its deciders read theirs), and the other
    // instruction files. A file is reported only when
    // none of them holds its name as a whole token; a name a program builds at run time would not
    // be seen (none does today).
    let mut corpus: Vec<(String, String)> = Vec::new();
    for p in files_ending(&root.join("bindings"), ".yaml")
        .into_iter()
        .chain(files_ending(&root.join("driver/examples/sdc"), ".bindings.json"))
        .chain(files_below(&root.join("bindings/tools"), "", &[]))
        .chain(files_below(&root.join("driver/src"), ".rs", &[]))
        .chain(files_below(&root.join("runbook/src"), ".rs", &[]))
    {
        if let Ok(t) = std::fs::read_to_string(&p) {
            corpus.push((rel(root, &p), t));
        }
    }
    corpus.extend(texts.iter().cloned());
    for (f, _) in &texts {
        let base = f.rsplit('/').next().unwrap();
        let named = corpus.iter().any(|(other, t)| other != f && names_file(t, base));
        if !named {
            r.add("instructions", f, "no binding, program or other instruction file names it", "name it in a binding or a program, or delete it");
        }
    }
}

// ---------------------------------------------------------------------------------------- views

/// What is wrong with the `product` member of a view (the main file of a run, which the inspector links
/// from its first screen): it is one string, a path relative to the run directory that stays inside the
/// run and has no component that starts with a dot (a tool's cache or login, never listed or served).
/// The inspector refuses the same views (ui/web/view.js `productProblems`).
fn product_problem(v: &Value) -> Option<String> {
    let Some(p) = v.as_str() else {
        return Some("must be a string: one path relative to the run directory".to_string());
    };
    if p.contains(['\0', '\\']) {
        return Some(format!("{p:?} holds a character that no path of a run holds (NUL or a backslash)"));
    }
    if p.starts_with('/') {
        return Some(format!("{p:?} is absolute: the product is a path relative to the run directory"));
    }
    let parts: Vec<&str> = p.split('/').filter(|n| !n.is_empty() && *n != ".").collect();
    if parts.is_empty() {
        return Some("is empty".to_string());
    }
    if parts.contains(&"..") {
        return Some(format!("{p:?} leaves the run"));
    }
    if parts.iter().any(|n| n.starts_with('.')) {
        return Some(format!("{p:?} has a component that starts with a dot: a tool's cache or login is never listed or served"));
    }
    None
}

fn views(root: &Path, r: &mut Report, seen: &mut Seen) {
    let files = files_ending(&root.join("views"), ".view.json");
    seen.views = files.len();
    for p in files {
        let f = rel(root, &p);
        match read_json(&p) {
            Err(e) => r.add("views", &f, format!("does not parse: {e}"), "fix the JSON"),
            Ok(Value::Object(o)) => {
                match o.get("format").and_then(Value::as_str) {
                    Some(VIEW_FORMAT) => {}
                    Some(other) => r.add("views", &f, format!("format is \"{other}\", not \"{VIEW_FORMAT}\""), format!("set \"format\": \"{VIEW_FORMAT}\" or convert the file to it")),
                    None => r.add("views", &f, "has no \"format\" member", format!("add \"format\": \"{VIEW_FORMAT}\"")),
                }
                if let Some(product) = o.get("product") {
                    if let Some(problem) = product_problem(product) {
                        r.add("views", &f, format!("product {problem}"), "name one file as a path relative to the run directory, for example \"work/<runbook>/<runbook>.md\"");
                    }
                }
            }
            Ok(_) => r.add("views", &f, "is not a JSON object", "make the top level an object with a \"format\" member"),
        }
    }
}

// --------------------------------------------------------------------------------------- cycles

/// The state words of the table of `docs/cycles-suite.md` that say the entry is a cycle of the
/// suite file: `converted` runs as expected, `pending` is marked `pending: true`.
const TABLE_IN_SUITE: [&str; 2] = ["converted", "pending"];

/// The four words the `state` cell of a row of `docs/cycles-suite.md` may say, as the page defines them.
const TABLE_STATES: [&str; 4] = ["converted", "pending", "held by a Rust test", "dropped with the driver"];

/// The ids of the list of old entries: one per line, a line of white space or `#` is skipped.
fn names_of(text: &str) -> Vec<String> {
    text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(String::from).collect()
}

/// The `(entry, state)` rows of the tables of `docs/cycles-suite.md` headed `| entry | state |`.
pub fn table_rows(text: &str) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    let mut in_table = false;
    for line in text.lines() {
        if !line.starts_with('|') {
            in_table = false;
            continue;
        }
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        if cells.len() >= 2 && cells[0] == "entry" && cells[1] == "state" {
            in_table = true;
            continue;
        }
        if in_table && cells.len() >= 2 && !cells[0].starts_with("---") {
            rows.push((cells[0].to_string(), cells[1].to_string()));
        }
    }
    rows
}

/// The seeds a `loom.tree-suite/1` document names: `(cycle or "defaults", from)`.
fn suite_seeds(doc: &Value) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    let mut take = |who: &str, v: &Value| {
        for s in v.get("seed").and_then(Value::as_array).into_iter().flatten() {
            out.push((who.to_string(), s.get("from").cloned().unwrap_or(Value::Null)));
        }
    };
    if let Some(d) = doc.get("defaults") {
        take("defaults", d);
    }
    for e in doc.get("cycles").and_then(Value::as_array).into_iter().flatten() {
        take(e.get("name").and_then(Value::as_str).unwrap_or("?"), e);
    }
    out
}

fn cycles(root: &Path, r: &mut Report, seen: &mut Seen) {
    let suite_path = root.join(CYCLES_SUITE);
    if !suite_path.is_file() {
        r.add("cycles", CYCLES_SUITE, "is missing", "restore cycles/tree-suite.yaml (docs/cycles-suite.md)");
        return;
    }
    let doc = match read_yaml(&suite_path) {
        Ok(d) => d,
        Err(e) => {
            r.add("cycles", CYCLES_SUITE, format!("does not parse: {e}"), "fix the YAML");
            return;
        }
    };
    match doc.get("format").and_then(Value::as_str) {
        Some(CYCLES_FORMAT) => {}
        other => r.add("cycles", CYCLES_SUITE, format!("format is {}, not \"{CYCLES_FORMAT}\"", other.map_or("missing".to_string(), |f| format!("\"{f}\""))), format!("set `format: {CYCLES_FORMAT}`")),
    }
    let Some(entries) = doc.get("cycles").and_then(Value::as_array) else {
        r.add("cycles", CYCLES_SUITE, "has no cycles list", "list the cycles under `cycles:`");
        return;
    };
    seen.cycles = entries.len();
    let mut in_suite: BTreeMap<String, bool> = BTreeMap::new();
    let mut dup = BTreeSet::new();
    for (n, e) in entries.iter().enumerate() {
        let Some(name) = e.get("name").and_then(Value::as_str).filter(|s| !s.is_empty()) else {
            r.add("cycles", CYCLES_SUITE, format!("entry {} has no `name`", n + 1), "give each cycle a `name:`");
            continue;
        };
        if in_suite.insert(name.to_string(), e.get("pending").and_then(Value::as_bool) == Some(true)).is_some() {
            dup.insert(name.to_string());
        }
    }
    for name in &dup {
        r.add("cycles", CYCLES_SUITE, format!("cycle name {name} appears more than once"), "keep one entry per name");
    }
    for (who, from) in suite_seeds(&doc) {
        match from.as_str() {
            Some(s) if !s.starts_with('/') && !has_dotdot(s) && root.join("cycles").join(s).exists() => {}
            Some(s) => r.add("cycles", CYCLES_SUITE, format!("cycle {who}: seed {s} does not exist under cycles/"), "restore the seed or name the one the cycle uses"),
            None => r.add("cycles", CYCLES_SUITE, format!("cycle {who}: a seed has no `from` path string ({from})"), "name each seed's `from` as a path string"),
        }
    }
    let names_path = root.join(CYCLES_NAMES);
    let names = match std::fs::read_to_string(&names_path) {
        Ok(t) => names_of(&t),
        Err(_) => {
            r.add("cycles", CYCLES_NAMES, "is missing or unreadable", "restore the committed list of the entries of the old suite, one name per line");
            return;
        }
    };
    let listed: BTreeSet<&str> = names.iter().map(String::as_str).collect();
    if listed.len() != names.len() {
        r.add("cycles", CYCLES_NAMES, "names an entry more than once", "keep one line per entry");
    }
    for name in in_suite.keys().filter(|n| !listed.contains(n.as_str())) {
        r.add("cycles", CYCLES_SUITE, format!("cycle {name} is not an entry of {CYCLES_NAMES}"), "add its name to the list and a row for it to docs/cycles-suite.md");
    }
    let table_path = root.join(CYCLES_TABLE);
    let Ok(table) = std::fs::read_to_string(&table_path) else {
        r.notes.push(format!("{CYCLES_TABLE} does not exist: the entries of {CYCLES_NAMES} are not compared with it"));
        return;
    };
    let rows = table_rows(&table);
    seen.table = Some(listed.len());
    let mut rowed: BTreeMap<&str, &str> = BTreeMap::new();
    for (name, state) in &rows {
        if rowed.insert(name, state).is_some() {
            r.add("cycles", CYCLES_TABLE, format!("entry {name} has more than one row"), "keep one row per entry");
        }
        if !listed.contains(name.as_str()) {
            r.add("cycles", CYCLES_TABLE, format!("entry {name} is not in {CYCLES_NAMES}"), "take the row out, or add the name to the list");
        }
    }
    for name in names.iter().filter(|n| !rowed.contains_key(n.as_str())) {
        r.add("cycles", CYCLES_TABLE, format!("entry {name} of {CYCLES_NAMES} has no row"), "add a row for it: what holds it now");
    }
    for (name, state) in rowed {
        let pending = in_suite.get(name).copied();
        if !TABLE_STATES.contains(&state) {
            r.add(
                "cycles",
                CYCLES_TABLE,
                format!("entry {name}: the state \"{state}\" is none of {}", TABLE_STATES.join(", ")),
                "write one of those words in the state cell and any qualification in the third cell",
            );
            continue;
        }
        match (state, pending) {
            ("converted", Some(false)) | ("pending", Some(true)) => {}
            (s, _) if TABLE_IN_SUITE.contains(&s) => r.add(
                "cycles",
                CYCLES_TABLE,
                format!("entry {name} is {s} in the table and {} in {CYCLES_SUITE}", match pending { None => "not there", Some(true) => "pending", Some(false) => "converted" }),
                "make the table and the suite file say the same",
            ),
            (s, Some(_)) => r.add("cycles", CYCLES_TABLE, format!("entry {name} is a cycle of {CYCLES_SUITE} and its row says {s}"), "say converted or pending, or take the cycle out of the suite file"),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------------------- output

pub fn text(r: &Report) -> String {
    let mut s = String::new();
    for f in &r.findings {
        s.push_str(&format!("{}: {}; fix: {}\n", f.file, f.what, f.fix));
    }
    for n in &r.notes {
        s.push_str(&format!("note: {n}\n"));
    }
    if r.ok() {
        s.push_str(&format!("check: ok ({})\n", r.summary));
    } else {
        s.push_str(&format!("check: {} findings ({})\n", r.findings.len(), r.summary));
    }
    s
}

pub fn json(r: &Report) -> Value {
    json!({
        "ok": r.ok(),
        "root": r.root,
        "summary": r.summary,
        "findings": r.findings.iter().map(|f| json!({"check": f.check, "file": f.file, "what": f.what, "fix": f.fix})).collect::<Vec<_>>(),
        "notes": r.notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_of_the_right_form_pass_and_others_are_named() {
        assert!(bad_placeholders("a {{story}} and {{test_command}}\n{\"a\": {\"b\": 1}}").is_empty());
        assert_eq!(bad_placeholders("x {{Story}} y {{a-b}} {{}}"), ["{{Story}}", "{{a-b}}", "{{}}"]);
        assert_eq!(bad_placeholders("open {{story"), ["{{story"]);
    }

    #[test]
    fn a_file_name_is_not_found_inside_a_longer_one() {
        assert!(names_file("render x close.md y", "close.md"));
        assert!(names_file("$shared/close.md", "close.md"));
        assert!(names_file("\"instructions/close.md\"", "close.md"));
        assert!(!names_file("closing-close.md", "close.md"));
        assert!(!names_file("x.close.md", "close.md"));
        assert!(!names_file("close.md-x", "close.md"));
    }

    #[test]
    fn the_rows_of_the_entry_tables_are_read_and_other_tables_are_not() {
        let t = "| entry | why |\n|---|---|\n| a, b | waits |\n\n| entry | state | reason |\n|---|---|---|\n| c | converted | |\n| d | held by a Rust test | `x` |\n\ntext\n| e | pending | not a table of entries |\n";
        assert_eq!(table_rows(t), [("c".to_string(), "converted".to_string()), ("d".to_string(), "held by a Rust test".to_string())]);
    }

    #[test]
    fn the_list_of_old_entries_skips_blank_and_comment_lines() {
        assert_eq!(names_of("a\n\n# note\n  b  \n"), ["a", "b"]);
    }

    /// The findings of the `bindings` check for a one-action bindings file: `executor` and `rest` are
    /// the lines of the action after its name (2 spaces and 4 spaces of indent).
    fn budget_findings(executor: &str, rest: &str) -> Vec<String> {
        let text = format!("format: loom.bindings/1\nactions:\n  - action: a.write\n    executor: {executor}\n{rest}");
        let doc = yaml_to_json(serde_yaml::from_str::<serde_yaml::Value>(&text).unwrap());
        let mut r = Report::default();
        yaml_bindings(Path::new("."), &mut r, "x", &doc, None);
        r.findings.into_iter().map(|f| format!("{}: {}", f.file, f.what)).collect()
    }

    const PROGRAM: &str = "{kind: program, argv: [sdc-node, a.write]}";
    const RETRY: &str = "    retry_when:\n      code_review: [rejected]\n      conformance_run: [failed]\n";

    #[test]
    fn a_budget_that_loom_reads_passes() {
        for budget in [
            "{max_runs: 3}",
            "{max_runs: 3, runs_per: case}",
            "{runs_per: rejection}",
            "{max_runs: 3, runs_per: rejection, rework_limit: 2}",
            "{max_runs: 3, runs_per: rejection, rework_limit: 1, rework_on: {code_review: [rejected]}}",
        ] {
            assert_eq!(budget_findings(PROGRAM, &format!("    budget: {budget}\n{RETRY}")), Vec::<String>::new(), "{budget}");
        }
        let person = "{kind: person, principal: person:owner}";
        let rest = "    budget: {runs_per: rejection, rework_limit: 2}\n    retry_when:\n      merge_approval: [rejected]\n";
        assert_eq!(budget_findings(person, rest), Vec::<String>::new(), "a person's action takes the members too");
    }

    #[test]
    fn each_budget_defect_loom_refuses_at_load_is_one_finding_naming_the_member() {
        for (what, budget, retry, member, words) in [
            ("a runs_per that is neither value", "{runs_per: window}", RETRY, "budget.runs_per", "is \"window\"; it is case or rejection"),
            ("a runs_per that is no text", "{runs_per: 3}", RETRY, "budget.runs_per", "it is case or rejection"),
            ("rejection beside a retry_when with no result", "{runs_per: rejection}", "    retry_when: {code_review: []}\n", "budget.runs_per", "lists no result"),
            ("rejection with no retry_when", "{runs_per: rejection}", "", "budget.runs_per", "lists no result"),
            ("a rework_limit of 0", "{runs_per: rejection, rework_limit: 0}", RETRY, "budget.rework_limit", "it is a whole number from 1"),
            ("a rework_limit that is no number", "{runs_per: rejection, rework_limit: many}", RETRY, "budget.rework_limit", "it is a whole number from 1"),
            ("a rework_limit beside runs_per case", "{runs_per: case, rework_limit: 2}", RETRY, "budget.rework_limit", "is read only under runs_per: rejection"),
            ("a rework_limit with no runs_per", "{rework_limit: 2}", RETRY, "budget.rework_limit", "is read only under runs_per: rejection"),
            ("a rework_on beside runs_per case", "{runs_per: case, rework_on: {code_review: [rejected]}}", RETRY, "budget.rework_on", "is read only under runs_per: rejection"),
            ("an empty rework_on", "{runs_per: rejection, rework_on: {}}", RETRY, "budget.rework_on", "is empty"),
            ("a rework_on kind with no result", "{runs_per: rejection, rework_on: {code_review: []}}", RETRY, "budget.rework_on.code_review", "is empty"),
            ("a rework_on that is no mapping", "{runs_per: rejection, rework_on: [code_review]}", RETRY, "budget.rework_on", "it is an object"),
            ("a rework_on pair retry_when does not list", "{runs_per: rejection, rework_on: {code_review: [approved]}}", RETRY, "budget.rework_on", "lists code_review approved, which retry_when does not list"),
            ("a rework_on kind retry_when does not list", "{runs_per: rejection, rework_on: {merge_approval: [rejected]}}", RETRY, "budget.rework_on", "lists merge_approval rejected, which retry_when does not list"),
        ] {
            let found = budget_findings(PROGRAM, &format!("    budget: {budget}\n{retry}"));
            assert_eq!(found.len(), 1, "{what}: {found:?}");
            assert!(found[0].starts_with("bindings/x.yaml: a.write: ") && found[0].contains(member) && found[0].contains(words), "{what}: {found:?}");
        }
    }

    #[test]
    fn the_rework_members_belong_to_the_program_and_person_executors_only() {
        let budget = "    budget: {runs_per: rejection}\n    retry_when:\n      code_review: [rejected]\n";
        for executor in ["{kind: agent, adapter: claude-code, model: m}", "{kind: open, name: c, bindings: c.yaml}", "{kind: copy, evidence: e, from: p@1}"] {
            let found = budget_findings(executor, budget);
            assert_eq!(found.len(), 1, "{executor}: {found:?}");
            assert!(found[0].contains("budget.runs_per") && found[0].contains("belongs to the Program and Person executors"), "{executor}: {found:?}");
        }
        let found = budget_findings("{kind: open, name: c, bindings: c.yaml}", "    budget: {max_runs: 3}\n");
        assert_eq!(found, Vec::<String>::new(), "max_runs is read on every executor");
    }
}
