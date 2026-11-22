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
//! 5. `cycles`: `cycles/suite.yaml` parses, has `runbook.cycles/1`, unique ids, a known state per
//!    entry and the fixtures each entry names; and, while `scripts/run-finding-cycles.sh` exists,
//!    the ids and states equal the script's `expected` and `pending` arrays.

use crate::assets::LAYOUT;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const VIEW_FORMAT: &str = "runbook.view/1";
pub const CYCLES_FORMAT: &str = "runbook.cycles/1";
pub const CYCLE_STATES: [&str; 3] = ["expected", "pending", "unlisted"];
pub const CYCLES_SUITE: &str = "cycles/suite.yaml";
pub const CYCLES_SCRIPT: &str = "scripts/run-finding-cycles.sh";

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
    script: Option<usize>,
}

impl Seen {
    fn line(&self) -> String {
        let script = match self.script {
            Some(n) => format!(", {n} compared with {CYCLES_SCRIPT}"),
            None => format!(", not compared ({CYCLES_SCRIPT} absent)"),
        };
        format!(
            "{} bindings files and {} JSON bindings, {} instruction files, {} views, {} cycles{script}",
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
    // What can name an instruction file: the bindings (both forms), the programs of the bindings
    // and the other instruction files. A file is reported only when none of them holds its name;
    // a name a program builds at run time would not be seen (none does today).
    let mut corpus: Vec<(String, String)> = Vec::new();
    for p in files_ending(&root.join("bindings"), ".yaml")
        .into_iter()
        .chain(files_ending(&root.join("driver/examples/sdc"), ".bindings.json"))
        .chain(files_below(&root.join("bindings/tools"), "", &[]))
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

fn views(root: &Path, r: &mut Report, seen: &mut Seen) {
    let files = files_ending(&root.join("views"), ".view.json");
    seen.views = files.len();
    for p in files {
        let f = rel(root, &p);
        match read_json(&p) {
            Err(e) => r.add("views", &f, format!("does not parse: {e}"), "fix the JSON"),
            Ok(Value::Object(o)) => match o.get("format").and_then(Value::as_str) {
                Some(VIEW_FORMAT) => {}
                Some(other) => r.add("views", &f, format!("format is \"{other}\", not \"{VIEW_FORMAT}\""), format!("set \"format\": \"{VIEW_FORMAT}\" or convert the file to it")),
                None => r.add("views", &f, "has no \"format\" member", format!("add \"format\": \"{VIEW_FORMAT}\"")),
            },
            Ok(_) => r.add("views", &f, "is not a JSON object", "make the top level an object with a \"format\" member"),
        }
    }
}

// --------------------------------------------------------------------------------------- cycles

/// The (id, state) pairs of the `expected` and `pending` arrays of the script, in order. An array is
/// `expected=(`, `expected+=(`, `pending=(` or `pending+=(` up to the next `)`, wherever it stands in
/// the text (a conditional append counts); words are separated by white space. A word that is not
/// a plain cycle id is returned in the second list.
pub fn script_arrays(text: &str) -> (Vec<(String, &'static str)>, Vec<String>) {
    let mut pairs = Vec::new();
    let mut odd = Vec::new();
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let mut hit = None;
        for (word, state) in [("expected", "expected"), ("pending", "pending")] {
            if b[i..].starts_with(word.as_bytes()) && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) {
                let mut j = i + word.len();
                if text[j..].starts_with('+') {
                    j += 1;
                }
                if text[j..].starts_with("=(") {
                    hit = Some((j + 2, state));
                }
            }
        }
        match hit {
            Some((start, state)) => {
                let end = text[start..].find(')').map_or(text.len(), |e| start + e);
                for w in text[start..end].split_whitespace() {
                    if w.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.')) {
                        pairs.push((w.to_string(), state));
                    } else {
                        odd.push(w.to_string());
                    }
                }
                i = end;
            }
            None => i += 1,
        }
    }
    (pairs, odd)
}

fn cycles(root: &Path, r: &mut Report, seen: &mut Seen) {
    let suite_path = root.join(CYCLES_SUITE);
    if !suite_path.is_file() {
        r.add("cycles", CYCLES_SUITE, "is missing", "restore cycles/suite.yaml (docs/cycles-suite.md)");
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
    let mut states: BTreeMap<String, String> = BTreeMap::new();
    let mut dup = BTreeSet::new();
    for (n, e) in entries.iter().enumerate() {
        let Some(id) = e.get("id").and_then(Value::as_str).filter(|s| !s.is_empty()) else {
            r.add("cycles", CYCLES_SUITE, format!("entry {} has no `id`", n + 1), "give each cycle an `id:`");
            continue;
        };
        match e.get("state").and_then(Value::as_str) {
            Some(s) if CYCLE_STATES.contains(&s) => {
                if states.insert(id.to_string(), s.to_string()).is_some() {
                    dup.insert(id.to_string());
                }
            }
            other => {
                r.add("cycles", CYCLES_SUITE, format!("cycle {id} has state {}, not one of {}", other.map_or("none".to_string(), |s| format!("\"{s}\"")), CYCLE_STATES.join(", ")), "set `state:` to one of them (docs/cycles-suite.md)");
                if states.insert(id.to_string(), String::new()).is_some() {
                    dup.insert(id.to_string());
                }
            }
        }
        for p in e.pointer("/stub/fixtures").map(|f| f.as_array().cloned().unwrap_or_default()).unwrap_or_default() {
            match p.as_str() {
                Some(s) if !s.starts_with('/') && !has_dotdot(s) && root.join(s).exists() => {}
                Some(s) => r.add("cycles", CYCLES_SUITE, format!("cycle {id}: fixture {s} does not exist under the asset root"), "restore the fixture or name the one the cycle uses"),
                None => r.add("cycles", CYCLES_SUITE, format!("cycle {id}: a fixture name is not a string ({p})"), "name each fixture as a path string"),
            }
        }
    }
    for id in &dup {
        r.add("cycles", CYCLES_SUITE, format!("cycle id {id} appears more than once"), "keep one entry per id");
    }
    let script_path = root.join(CYCLES_SCRIPT);
    if !script_path.is_file() {
        r.notes.push(format!("{CYCLES_SCRIPT} does not exist: the ids and states of {CYCLES_SUITE} are not compared with it"));
        return;
    }
    let script = match std::fs::read_to_string(&script_path) {
        Ok(t) => t,
        Err(e) => {
            r.add("cycles", CYCLES_SCRIPT, format!("cannot be read: {e}"), "make it a readable file");
            return;
        }
    };
    let (pairs, odd) = script_arrays(&script);
    for w in odd {
        r.add("cycles", CYCLES_SCRIPT, format!("a word of the expected or pending arrays is not a plain cycle id: {w}"), "write each cycle id as a plain word, so the suite can be compared with it");
    }
    if pairs.is_empty() {
        r.add("cycles", CYCLES_SCRIPT, "holds no `expected` or `pending` arrays of cycle ids", "keep the arrays the suite is compared with, or update the check");
        return;
    }
    let mut script_states: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (id, st) in &pairs {
        script_states.entry(id).or_default().insert(st);
    }
    seen.script = Some(script_states.len());
    let mut unlisted = 0;
    for (id, st) in &states {
        if st == "unlisted" {
            unlisted += 1;
            if script_states.contains_key(id.as_str()) {
                r.add("cycles", CYCLES_SUITE, format!("cycle {id} is unlisted but the script lists it in its arrays"), "set its state to the array it is in");
            }
            continue;
        }
        match script_states.get(id.as_str()) {
            None => r.add("cycles", CYCLES_SUITE, format!("cycle {id} ({st}) is in the suite, not in the arrays of {CYCLES_SCRIPT}"), format!("add it to the {st} array of the script, or take it out of the suite")),
            Some(s) if !s.contains(st.as_str()) => r.add(
                "cycles",
                CYCLES_SUITE,
                format!("cycle {id} is {st} in the suite and {} in {CYCLES_SCRIPT}", s.iter().copied().collect::<Vec<_>>().join(" and ")),
                format!("set its state to {} in the suite, or move the id between the arrays of the script", s.iter().next().unwrap()),
            ),
            Some(_) => {}
        }
    }
    for id in script_states.keys().filter(|id| !states.contains_key(**id)) {
        r.add("cycles", CYCLES_SUITE, format!("cycle {id} is in the arrays of {CYCLES_SCRIPT}, not in the suite"), "add an entry for it to cycles/suite.yaml, or take it out of the script");
    }
    if unlisted > 0 {
        r.notes.push(format!("{unlisted} cycles of {CYCLES_SUITE} are unlisted: the script runs them outside both arrays and they are not compared"));
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
    fn the_arrays_of_the_script_are_read_wherever_they_stand() {
        let t = "expected=(a b\n  c)\nx() { :; }\n  expected+=(d)\npending=(e f)\nif x; then expected+=(g $h); fi\nunexpected=(z)\n";
        let (p, odd) = script_arrays(t);
        let got: Vec<String> = p.iter().map(|(i, s)| format!("{i}:{s}")).collect();
        assert_eq!(got, ["a:expected", "b:expected", "c:expected", "d:expected", "e:pending", "f:pending", "g:expected"]);
        assert_eq!(odd, ["$h"]);
    }
}
