//! Shared parts of `sdc-conform` and `sdc-conform-adapter` (docs/conformance.md): the route table
//! `sdc.conformance-table/1` and its check against the compiled specification, the templates the
//! adapter fills, a small HTTP/1.1 client for 127.0.0.1, and the ess pin (`driver/ess-pin.json`).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{json, Value as J};
use sha2::{Digest, Sha256};

pub const TABLE_FORMAT: &str = "sdc.conformance-table/1";
pub const PIN_FORMAT: &str = "sdc.ess-pin/1";
/// Programs a table may name by bare name in `build.argv` or `start.argv`; anything else must be
/// a relative path inside the delivered tree.
pub const BARE_PROGRAMS: [&str; 7] = ["cargo", "go", "node", "npm", "deno", "java", "dotnet"];
pub const METHODS: [&str; 5] = ["GET", "POST", "PUT", "PATCH", "DELETE"];
pub const START_PLACEHOLDERS: [&str; 4] = ["{host}", "{port}", "{data_dir}", "{data_file}"];

pub fn s(v: &J) -> &str {
    v.as_str().unwrap_or("")
}

pub fn sha256_bytes(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

pub fn sha256_file(p: &Path) -> Result<String, String> {
    fs::read(p).map(|b| sha256_bytes(&b)).map_err(|e| format!("{}: {e}", p.display()))
}

pub fn read_json(p: &Path) -> Result<J, String> {
    let t = fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    serde_json::from_str(&t).map_err(|e| format!("{}: not JSON: {e}", p.display()))
}

// ---- values and templates -------------------------------------------------------------------

/// The value at a dotted path (`a.b.0.c`); the empty path is the value itself.
pub fn at<'a>(v: &'a J, path: &str) -> Option<&'a J> {
    if path.is_empty() {
        return Some(v);
    }
    let mut cur = v;
    for part in path.split('.') {
        cur = match cur {
            J::Object(m) => m.get(part)?,
            J::Array(a) => a.get(part.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

/// The sources a template reads: `input`, `params`, `response`, `lookup`, `capture`.
pub type Ctx = BTreeMap<&'static str, J>;

/// The two kinds of `none`: what no client of the product's interface can reach (its scenarios
/// are not measurable) and what the product does not do (its scenarios fail).
pub const NONE_KINDS: [&str; 2] = ["not_reachable_over_the_interface", "not_implemented"];

/// `(kind, why)` of an entry's `none`, when it is well formed.
pub fn none_of(v: &J) -> Option<(&str, &str)> {
    let n = &v["none"];
    let kind = n["kind"].as_str().filter(|k| NONE_KINDS.contains(k))?;
    Some((kind, n["why"].as_str().unwrap_or("")))
}

/// A placeholder `{source.path}` that makes up a whole string: `(source, path)`.
pub fn placeholder(t: &str) -> Option<(&str, &str)> {
    let inner = t.strip_prefix('{')?.strip_suffix('}')?;
    if inner.contains('{') || inner.contains('}') {
        return None;
    }
    let (src, path) = inner.split_once('.').unwrap_or((inner, ""));
    ["input", "params", "response", "lookup", "capture"].contains(&src).then_some((src, path))
}

/// A text template filled: every placeholder in it replaced by its value as text (headers).
pub fn fill_text(t: &str, ctx: &Ctx) -> Result<String, String> {
    let mut out = t.to_string();
    for ph in path_placeholders(t) {
        let v = fill(&J::String(ph.clone()), ctx)?;
        let text = match &v {
            J::String(x) => x.clone(),
            other => other.to_string(),
        };
        out = out.replacen(&ph, &text, 1);
    }
    Ok(out)
}

/// The names of the captures a table reads anywhere (`{capture.<name>}` in any string).
pub fn capture_refs(v: &J, out: &mut BTreeSet<String>) {
    match v {
        J::String(x) => {
            for ph in path_placeholders(x) {
                if let Some(("capture", name)) = placeholder(&ph) {
                    out.insert(name.to_string());
                }
            }
        }
        J::Array(a) => a.iter().for_each(|x| capture_refs(x, out)),
        J::Object(m) => m.values().for_each(|x| capture_refs(x, out)),
        _ => {}
    }
}

/// A body or payload template filled: a string that is exactly one placeholder becomes the value
/// it names (any JSON type), objects and arrays are filled member by member, the rest is literal.
pub fn fill(t: &J, ctx: &Ctx) -> Result<J, String> {
    Ok(match t {
        J::String(x) => match placeholder(x) {
            Some((src, path)) => ctx
                .get(src)
                .and_then(|v| at(v, path))
                .cloned()
                .ok_or_else(|| format!("{x}: no such value"))?,
            None => t.clone(),
        },
        J::Array(a) => J::Array(a.iter().map(|x| fill(x, ctx)).collect::<Result<_, _>>()?),
        J::Object(m) => J::Object(m.iter().map(|(k, x)| Ok((k.clone(), fill(x, ctx)?))).collect::<Result<_, String>>()?),
        _ => t.clone(),
    })
}

fn pct(x: &str) -> String {
    x.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The placeholders of a path template, in order.
pub fn path_placeholders(p: &str) -> Vec<String> {
    let mut out = vec![];
    let mut rest = p;
    while let Some(i) = rest.find('{') {
        let Some(j) = rest[i..].find('}') else { break };
        out.push(rest[i..i + j + 1].to_string());
        rest = &rest[i + j + 1..];
    }
    out
}

/// A path template filled; each placeholder is percent-encoded.
pub fn fill_path(p: &str, ctx: &Ctx) -> Result<String, String> {
    let mut out = p.to_string();
    for ph in path_placeholders(p) {
        let v = fill(&J::String(ph.clone()), ctx)?;
        let text = match &v {
            J::String(x) => x.clone(),
            J::Number(_) | J::Bool(_) => v.to_string(),
            _ => return Err(format!("{ph}: not a string, number or boolean")),
        };
        out = out.replacen(&ph, &pct(&text), 1);
    }
    Ok(out)
}

// ---- the table check ------------------------------------------------------------------------

fn names(v: &J) -> BTreeSet<String> {
    match v {
        J::Object(m) => m.keys().cloned().collect(),
        J::Array(a) => a.iter().map(|x| s(&x["name"]).to_string()).collect(),
        _ => BTreeSet::new(),
    }
}

fn by_name<'a>(v: &'a J, name: &str) -> Option<&'a J> {
    match v {
        J::Object(m) => m.get(name),
        J::Array(a) => a.iter().find(|x| s(&x["name"]) == name),
        _ => None,
    }
}

fn field_names(v: &J) -> BTreeSet<String> {
    v["fields"].as_array().into_iter().flatten().map(|f| s(&f["name"]).to_string()).collect()
}

fn check_argv(what: &str, v: &J, start: bool, p: &mut Vec<String>) {
    let Some(a) = v.as_array().filter(|a| !a.is_empty() && a.iter().all(J::is_string)) else {
        p.push(format!("{what}.argv: not a non-empty list of strings"));
        return;
    };
    let prog = s(&a[0]);
    let bare = !prog.contains('/');
    if bare && !BARE_PROGRAMS.contains(&prog) {
        p.push(format!("{what}.argv[0] {prog:?}: a bare name must be one of {}; anything else is a relative path inside the delivered tree", BARE_PROGRAMS.join(", ")));
    }
    if !bare && (prog.starts_with('/') || Path::new(prog).components().any(|c| matches!(c, std::path::Component::ParentDir))) {
        p.push(format!("{what}.argv[0] {prog:?}: must be a relative path inside the delivered tree"));
    }
    for x in a {
        let x = s(x);
        if x.starts_with('/') || x.contains("..") {
            p.push(format!("{what}.argv: {x:?} reaches outside the delivered tree"));
        }
        for ph in path_placeholders(x) {
            if !start || !(START_PLACEHOLDERS.contains(&ph.as_str()) || ph.starts_with("{arg.")) {
                p.push(format!("{what}.argv: unknown placeholder {ph} (start may use {}, and {{arg.<name>}} of start.args)", START_PLACEHOLDERS.join(", ")));
            }
        }
    }
    if start && !a.iter().any(|x| s(x).contains("{port}")) {
        p.push(format!("{what}.argv: never passes {{port}}, so the adapter cannot bind the product to the port it chose"));
    }
}

fn check_request(what: &str, r: &J, inputs: &BTreeSet<String>, params: &BTreeSet<String>, p: &mut Vec<String>) {
    if !METHODS.contains(&s(&r["method"])) {
        p.push(format!("{what}.method {}: not one of {}", r["method"], METHODS.join(", ")));
    }
    let path = s(&r["path"]);
    if !path.starts_with('/') {
        p.push(format!("{what}.path {:?}: must start with /", r["path"]));
    }
    let mut phs = path_placeholders(path);
    collect_placeholders(&r["body"], &mut phs);
    match &r["headers"] {
        J::Null => {}
        J::Object(m) if m.values().all(J::is_string) => m.values().for_each(|v| phs.extend(path_placeholders(s(v)))),
        _ => p.push(format!("{what}.headers: not an object of strings")),
    }
    for ph in phs {
        match placeholder(&ph) {
            Some(("input", f)) if inputs.contains(f.split('.').next().unwrap_or("")) => {}
            Some(("params", f)) if params.contains(f.split('.').next().unwrap_or("")) => {}
            Some(("capture", _)) => {}
            _ => p.push(format!("{what}: placeholder {ph} names no input field, view parameter or capture")),
        }
    }
}

fn check_none(what: &str, v: &J, p: &mut Vec<String>) -> bool {
    if v["none"].is_null() {
        return false;
    }
    if none_of(v).is_none() || s(&v["none"]["why"]).trim().is_empty() {
        p.push(format!("{what}: none needs a kind ({}) and a why", NONE_KINDS.join(" or ")));
    }
    true
}

fn collect_placeholders(t: &J, out: &mut Vec<String>) {
    match t {
        J::String(x) if x.starts_with('{') && x.ends_with('}') => out.push(x.clone()),
        J::Array(a) => a.iter().for_each(|x| collect_placeholders(x, out)),
        J::Object(m) => m.values().for_each(|x| collect_placeholders(x, out)),
        _ => {}
    }
}

fn check_reads(what: &str, t: &J, inputs: &BTreeSet<String>, lookup: bool, p: &mut Vec<String>) {
    let mut phs = vec![];
    collect_placeholders(t, &mut phs);
    for ph in phs {
        match placeholder(&ph) {
            Some(("response", _)) => {}
            Some(("lookup", _)) if lookup => {}
            Some(("input", f)) if inputs.contains(f.split('.').next().unwrap_or("")) => {}
            Some(("capture", _)) => {}
            _ => p.push(format!("{what}: placeholder {ph} reads nothing the adapter has (input.<field>, response.<path>, capture.<name>, lookup.<path> after a declared lookup)")),
        }
    }
}

/// What a checked table leaves out on purpose: entries declared `none`, as
/// `<what> <name> (<kind>): <why>`.
pub fn declared_missing(table: &J) -> Vec<String> {
    let mut out = vec![];
    for c in table["commands"].as_array().into_iter().flatten() {
        if let Some((kind, why)) = none_of(c) {
            out.push(format!("command {} ({kind}): {why}", s(&c["command"])));
        }
        for o in c["outcomes"].as_array().into_iter().flatten() {
            if let Some((kind, why)) = none_of(o) {
                out.push(format!("outcome {}/{} ({kind}): {why}", s(&c["command"]), s(&o["outcome"])));
            }
        }
    }
    for v in table["views"].as_array().into_iter().flatten() {
        if let Some((kind, why)) = none_of(v) {
            out.push(format!("view {} ({kind}): {why}", s(&v["view"])));
        }
    }
    out
}

/// What the interface cannot reach (`none` of kind `not_reachable_over_the_interface`):
/// `(commands, (command, outcome) pairs, views)`.
pub fn not_reachable(table: &J) -> (BTreeSet<String>, BTreeSet<(String, String)>, BTreeSet<String>) {
    let nr = |v: &J| none_of(v).is_some_and(|(k, _)| k == NONE_KINDS[0]);
    let (mut c, mut o, mut v) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    for x in table["commands"].as_array().into_iter().flatten() {
        if nr(x) {
            c.insert(s(&x["command"]).to_string());
        }
        for y in x["outcomes"].as_array().into_iter().flatten().filter(|y| nr(y)) {
            o.insert((s(&x["command"]).to_string(), s(&y["outcome"]).to_string()));
        }
    }
    for x in table["views"].as_array().into_iter().flatten().filter(|x| nr(x)) {
        v.insert(s(&x["view"]).to_string());
    }
    (c, o, v)
}

/// Whether a suite scenario needs something the interface cannot reach: a command it executes, an
/// outcome it expects or forces, or a view it reads.
pub fn needs_unreachable(scenario: &J, nr: &(BTreeSet<String>, BTreeSet<(String, String)>, BTreeSet<String>)) -> bool {
    scenario["steps"].as_array().into_iter().flatten().any(|st| match s(&st["step"]) {
        "execute_command" | "execute_command_without_input" => nr.0.contains(s(&st["command"])),
        "expect_outcome" => nr.1.contains(&(s(&st["outcome"]["command"]).to_string(), s(&st["outcome"]["outcome"]).to_string())),
        "configure_external_outcome" | "configure_external_outcome_repeatedly" =>
            nr.1.contains(&(s(&st["force"]["command"]).to_string(), s(&st["force"]["outcome"]).to_string())),
        "query_view" | "eventually_view" => nr.2.contains(s(&st["view"])),
        _ => false,
    })
}

/// Whether a forced request goes to the command's own route: the same method and the same path
/// segments, where only a segment holding a placeholder in the command's path may differ; the
/// query string is not part of the route. Another route could answer the forced outcome's status
/// whatever the product does on the command's route.
fn same_route(command: &J, forced: &J) -> bool {
    let route = |r: &J| -> Vec<String> { s(&r["path"]).split('?').next().unwrap_or("").trim_matches('/').split('/').map(String::from).collect() };
    let (a, b) = (route(command), route(forced));
    s(&command["method"]) == s(&forced["method"]) && a.len() == b.len() && a.iter().zip(&b).all(|(x, y)| x.contains('{') || x == y)
}

fn check_captures(t: &J, ir: &J, p: &mut Vec<String>) {
    let mut declared = BTreeSet::new();
    for (i, c) in t["captures"].as_array().into_iter().flatten().enumerate() {
        let name = s(&c["name"]).to_string();
        let what = format!("captures[{i}] {name}");
        if name.is_empty() || !name.chars().all(|x| x.is_ascii_alphanumeric() || x == '_') {
            p.push(format!("{what}: name must be letters, digits and _"));
        }
        if !declared.insert(name.clone()) {
            p.push(format!("{what}: a second capture of the same name"));
        }
        if !["stdout", "stderr"].contains(&s(&c["stream"])) {
            p.push(format!("{what}: stream must be stdout or stderr"));
        }
        match regex::Regex::new(s(&c["regex"])) {
            Ok(r) if r.captures_len() == 2 => {}
            Ok(_) => p.push(format!("{what}: the regex must have exactly one group")),
            Err(e) => p.push(format!("{what}: the regex does not compile: {e}")),
        }
        if !c["timeout_s"].as_u64().is_some_and(|x| (1..=120).contains(&x)) {
            p.push(format!("{what}: timeout_s must be 1 to 120"));
        }
    }
    if !t["captures"].is_null() && !t["captures"].is_array() {
        p.push("captures: not a list".into());
    }
    let mut used = BTreeSet::new();
    let mut rest = t.clone();
    if let Some(m) = rest.as_object_mut() {
        m.remove("captures");
        m.remove("bindings");
    }
    capture_refs(&rest, &mut used);
    let inputs: BTreeSet<String> = match &ir["commands"] {
        J::Object(m) => m.values().flat_map(|c| c["input"].as_array().into_iter().flatten().map(|f| s(&f["name"]).to_string())).collect(),
        _ => BTreeSet::new(),
    };
    for (i, b) in t["bindings"].as_array().into_iter().flatten().enumerate() {
        let (field, cap) = (s(&b["input"]), s(&b["capture"]));
        if !inputs.contains(field) {
            p.push(format!("bindings[{i}]: {field:?} is no input field of a command of the specification"));
        }
        if !declared.contains(cap) {
            p.push(format!("bindings[{i}]: no declared capture {cap}"));
        }
        used.insert(cap.to_string());
    }
    for u in used.difference(&declared) {
        if !t["bindings"].as_array().into_iter().flatten().any(|b| s(&b["capture"]) == u) {
            p.push(format!("placeholder {{capture.{u}}}: no declared capture {u}"));
        }
    }
    for d in declared.difference(&used) {
        p.push(format!("capture {d} is declared and never used"));
    }
}

/// The HTTP statuses an outcome entry maps: `status` is one status or a non-empty list of them
/// (several statuses may mean one outcome).
pub fn statuses(o: &J) -> Option<Vec<i64>> {
    let ok = |x: &J| x.as_i64().filter(|x| (100..600).contains(x));
    match &o["status"] {
        J::Array(a) if !a.is_empty() => a.iter().map(ok).collect(),
        x => ok(x).map(|x| vec![x]),
    }
}

const LITERAL_WHY: &str = "an event payload or error field is filled from what the product answers (response, lookup, capture) or from the input, never written into the table, or the check compares the table with itself";

/// Whether a template reads anything: a value with no placeholder is a literal.
fn has_placeholder(v: &J) -> bool {
    match v {
        J::String(x) => path_placeholders(x).iter().any(|ph| placeholder(ph).is_some()),
        J::Array(a) => a.iter().any(has_placeholder),
        J::Object(m) => m.values().any(has_placeholder),
        _ => false,
    }
}

/// Whether a template reads the request's own input anywhere.
fn reads_input(v: &J) -> bool {
    match v {
        J::String(x) => path_placeholders(x).iter().any(|ph| matches!(placeholder(ph), Some(("input", _)))),
        J::Array(a) => a.iter().any(reads_input),
        J::Object(m) => m.values().any(reads_input),
        _ => false,
    }
}

/// The fields of what an outcome entry builds that the table fills from the request's own input:
/// of the event `event`, or of the error when `event` is None. A check comparing such a field
/// compares the suite's input with itself.
pub fn input_filled(table: &J, command: &str, outcome: &str, event: Option<&str>) -> BTreeSet<String> {
    let Some(o) = table["commands"].as_array().into_iter().flatten()
        .filter(|c| s(&c["command"]) == command)
        .flat_map(|c| c["outcomes"].as_array().into_iter().flatten())
        .find(|o| s(&o["outcome"]) == outcome) else { return BTreeSet::new() };
    let fields = match event {
        Some(e) => o["events"].as_array().into_iter().flatten().find(|x| s(&x["event"]) == e).map(|x| &x["payload"]),
        None => Some(&o["error"]["fields"]),
    };
    fields.and_then(J::as_object).into_iter().flatten().filter(|(_, v)| reads_input(v)).map(|(k, _)| k.clone()).collect()
}

/// Files a start command may be cited from: a README or a manifest of the delivered tree.
pub const START_SOURCES: [&str; 9] = ["Cargo.toml", "package.json", "go.mod", "pyproject.toml", "Procfile", "Taskfile.yml", "Makefile", "deno.json", "justfile"];

/// Words that mark a README line as describing a test, demo or mock mode.
const TEST_WORDS: [&str; 11] = ["test mode", "test-mode", "testing mode", "for tests", "for testing", "test only", "test-only", "only in tests", "demo", "mock", "fake"];

fn word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '-' || c == '_'
}

/// Whether `needle` occurs in `hay` not as part of a longer word (letters, digits, - and _).
fn contains_word(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let (first, last) = (needle.chars().next().unwrap_or(' '), needle.chars().last().unwrap_or(' '));
    hay.match_indices(needle).any(|(i, _)| {
        let before = hay[..i].chars().last();
        let after = hay[i + needle.len()..].chars().next();
        (!word_char(first) || !before.is_some_and(word_char)) && (!word_char(last) || !after.is_some_and(word_char))
    })
}

/// The literal pieces of a template, between its placeholders.
fn literal_pieces(x: &str) -> Vec<String> {
    let mut rest = x.to_string();
    for ph in path_placeholders(x) {
        rest = rest.replacen(&ph, "\u{0}", 1);
    }
    rest.split('\u{0}').map(str::trim).filter(|x| !x.is_empty()).map(String::from).collect()
}

/// The arguments of a usage line after the program `base` (or after its last `--`): for each, whether
/// it names a value the operator chooses (`<partner-id>`, `[secret]`), and the alternatives it lists
/// (`<ok|deviate>`; empty for a free value). Empty when the line names neither the program nor `--`.
fn usage_slots(line: &str, base: &str) -> Vec<Option<Vec<String>>> {
    let mut tokens: Vec<String> = vec![];
    let mut open = false;
    for w in line.split_whitespace().map(|w| w.trim_matches('`')) {
        if open {
            if let Some(last) = tokens.last_mut() {
                last.push(' ');
                last.push_str(w);
            }
        } else {
            tokens.push(w.to_string());
        }
        open = (open || w.starts_with('<') || w.starts_with('[')) && !(w.ends_with('>') || w.ends_with(']'));
    }
    let start = tokens.iter().rposition(|w| w == "--" || Path::new(w).file_name().and_then(|n| n.to_str()) == Some(base));
    start.map(|i| tokens[i + 1..].iter().map(|w| {
        let inner = w.strip_prefix('<').and_then(|x| x.strip_suffix('>')).or_else(|| w.strip_prefix('[').and_then(|x| x.strip_suffix(']')))?;
        Some(if inner.contains('|') { inner.split('|').map(String::from).collect() } else { vec![] })
    }).collect()).unwrap_or_default()
}

/// Ruling: the start command is one the delivered tree's README or manifest states for normal use.
/// `start.from` cites the file and line; the cited lines together hold the program's file name and
/// every argument without a placeholder, and none of them describes a test, demo or mock mode.
fn check_start_from(t: &J, ir: &J, tree: &Path, p: &mut Vec<String>) {
    let Some(from) = t["start"]["from"].as_array().filter(|a| !a.is_empty()) else {
        p.push("start.from: missing (the README or manifest file and line of the delivered tree that states this start command for normal use)".into());
        return;
    };
    let mut text = String::new();
    for (i, c) in from.iter().enumerate() {
        let (file, line) = (s(&c["file"]), c["line"].as_u64().unwrap_or(0));
        let name = Path::new(file).file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file.starts_with('/') || Path::new(file).components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            p.push(format!("start.from[{i}]: {file:?} is no relative path inside the delivered tree"));
            continue;
        }
        if !name.to_ascii_lowercase().starts_with("readme") && !START_SOURCES.contains(&name) {
            p.push(format!("start.from[{i}]: {file} is not a README or a manifest ({})", START_SOURCES.join(", ")));
            continue;
        }
        let Some(l) = fs::read_to_string(tree.join(file)).ok().and_then(|x| x.lines().nth((line as usize).wrapping_sub(1)).map(String::from)) else {
            p.push(format!("start.from[{i}]: {file} has no line {line}"));
            continue;
        };
        if TEST_WORDS.iter().any(|w| l.to_lowercase().contains(w)) {
            p.push(format!("start.from[{i}]: {file} line {line} describes a test, demo or mock mode: {l:?}"));
        }
        text.push_str(&l);
        text.push('\n');
    }
    check_start_args(t, ir, &text, p);
    if text.is_empty() {
        return;
    }
    let argv: Vec<&str> = t["start"]["argv"].as_array().into_iter().flatten().map(s).collect();
    let Some(prog) = argv.first() else { return };
    let base = Path::new(prog).file_name().and_then(|n| n.to_str()).unwrap_or(prog);
    if matches!(argv.as_slice(), ["cargo" | "go", "run", ..]) {
        p.push(format!("start.argv: {} run compiles at every start; start the binary the build made (e.g. target/release/<bin>) with the same arguments, and cite the manifest line naming it", argv[0]));
    }
    // An argument holding a placeholder stands for a value the README gives (a port, a data file),
    // and so does a literal at the position of an operator value (`<partner-id>`) in a cited usage.
    let slots: Vec<Vec<Option<Vec<String>>>> = text.lines().map(|l| usage_slots(l, base)).collect();
    let operator_value = |i: usize, a: &str| slots.iter().any(|u| matches!(u.get(i), Some(Some(alts)) if alts.is_empty() || alts.iter().any(|x| x == a)));
    let literal = argv.iter().enumerate().skip(1).filter(|(i, a)| path_placeholders(a).is_empty() && !operator_value(i - 1, a)).map(|(_, a)| a.to_string());
    for want in std::iter::once(base.to_string()).chain(literal) {
        if !contains_word(&text, &want) {
            p.push(format!("start.from: the cited lines do not contain {want} (they must state the program and every argument of start.argv that holds no placeholder)"));
        }
    }
}

/// Start values the README leaves to the operator, taken from the scenario: each `start.args` entry
/// `{name, kind: from_scenario, field, nth, when_absent}` fills `{arg.<name>}` in `start.argv` with
/// the `nth` (default 0) distinct value the scenario sends in the input field `field`, else
/// `when_absent`, which must be on the cited lines.
fn check_start_args(t: &J, ir: &J, cited: &str, p: &mut Vec<String>) {
    let inputs: BTreeSet<String> = match &ir["commands"] {
        J::Object(m) => m.values().flat_map(|c| c["input"].as_array().into_iter().flatten().map(|f| s(&f["name"]).to_string())).collect(),
        J::Array(a) => a.iter().flat_map(|c| c["input"].as_array().into_iter().flatten().map(|f| s(&f["name"]).to_string())).collect(),
        _ => BTreeSet::new(),
    };
    let mut names = BTreeSet::new();
    if !t["start"]["args"].is_null() && !t["start"]["args"].is_array() {
        p.push("start.args: not a list".into());
    }
    for (i, a) in t["start"]["args"].as_array().into_iter().flatten().enumerate() {
        let name = s(&a["name"]);
        let what = format!("start.args[{i}] {name}");
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') || !names.insert(name.to_string()) {
            p.push(format!("{what}: name must be letters, digits and _, once"));
        }
        if a["kind"] != "from_scenario" {
            p.push(format!("{what}: kind must be from_scenario"));
        }
        if !inputs.contains(s(&a["field"])) {
            p.push(format!("{what}: {} is no input field of a command of the specification", s(&a["field"])));
        }
        if !a["nth"].is_null() && !a["nth"].is_u64() {
            p.push(format!("{what}: nth must be a whole number"));
        }
        let absent = s(&a["when_absent"]);
        if absent.is_empty() || !contains_word(cited, absent) {
            p.push(format!("{what}: when_absent {absent} is not on the cited lines (the value the README gives for a scenario that carries none)"));
        }
    }
    for a in t["start"]["argv"].as_array().into_iter().flatten() {
        for ph in path_placeholders(s(a)) {
            if let Some(n) = ph.strip_prefix("{arg.").and_then(|x| x.strip_suffix('}')) {
                if !names.contains(n) {
                    p.push(format!("start.argv: placeholder {ph} names no start.args entry"));
                }
            }
        }
    }
}

fn command_of<'a>(ir: &'a J, command: &str) -> Option<&'a J> {
    by_name(&ir["commands"], command)
}

/// The input field naming the existing instance a command acts on: a `moves`/`updates` subject
/// whose `instance` is supplied by the caller (compiled IR).
pub fn instance_field(ir: &J, command: &str) -> Option<String> {
    command_of(ir, command)?["outcomes"].as_array()?.iter().find_map(|o| {
        let sub = &o["subject"];
        (matches!(s(&sub["effect"]), "moves" | "updates") && sub["instance"]["from"] == "supplied")
            .then(|| s(&sub["instance"]["field"]["name"]).to_string())
            .filter(|f| !f.is_empty())
    })
}

/// ESS's answer for an instance no record carries (ESS 0.97.0
/// `docs/design/typed-literals-and-unknown-instances.md` § 2 and § 2a): the command's not-found
/// outcome when it declares exactly one (an `external` refusal acting on no instance whose error
/// carries a field of the instance identity's type), otherwise its `wrong_state` outcome.
/// `(outcome, error)`; None when the command acts on no supplied instance or declares neither.
pub fn unknown_instance_answer(ir: &J, command: &str) -> Option<(String, Option<String>)> {
    let c = command_of(ir, command)?;
    let field = instance_field(ir, command)?;
    let id_type = c["input"].as_array()?.iter().find(|f| s(&f["name"]) == field).map(|f| f["type_ref"].clone())?;
    let outcomes = c["outcomes"].as_array()?;
    let not_found: Vec<&J> = outcomes.iter().filter(|o| {
        o["condition"]["kind"] == "external" && o["subject"].is_null() && o["error"].as_str().is_some_and(|e| {
            by_name(&ir["errors"], e).is_some_and(|err| err["fields"].as_array().into_iter().flatten().any(|f| f["type_ref"] == id_type))
        })
    }).collect();
    let o = if not_found.len() == 1 { not_found[0] } else { outcomes.iter().find(|o| o["condition"]["kind"] == "wrong_state")? };
    Some((s(&o["name"]).to_string(), o["error"].as_str().map(String::from)))
}

/// Ruling: a start argument or a request header that a README of the delivered tree describes as
/// a test, demo or mock mode is refused. Forced requests are exempt: forcing an external outcome is
/// what they are for.
fn check_test_mode(t: &J, tree: &Path, p: &mut Vec<String>) {
    let mut readmes: Vec<(String, Vec<String>)> = fs::read_dir(tree).into_iter().flatten().flatten()
        .filter_map(|e| e.file_name().to_str().map(String::from))
        .filter(|n| n.to_ascii_lowercase().starts_with("readme"))
        .filter_map(|n| fs::read_to_string(tree.join(&n)).ok().map(|x| (n, x.lines().map(String::from).collect())))
        .collect();
    readmes.sort();
    let marked = |token: &str| -> Option<String> {
        if token.chars().count() < 2 {
            return None;
        }
        let token = token.to_lowercase();
        readmes.iter().find_map(|(name, lines)| lines.iter().enumerate().find_map(|(i, l)| {
            let l = l.to_lowercase();
            (TEST_WORDS.iter().any(|w| l.contains(w)) && contains_word(&l, &token)).then(|| format!("{name} line {}", i + 1))
        }))
    };
    for a in t["start"]["argv"].as_array().into_iter().flatten().skip(1) {
        for piece in literal_pieces(s(a)) {
            if let Some(at) = marked(&piece) {
                p.push(format!("start.argv: {piece} is what {at} describes as a test, demo or mock mode"));
            }
        }
    }
    let mut requests: Vec<(String, &J)> = vec![("start.ready".into(), &t["start"]["ready"])];
    for c in t["commands"].as_array().into_iter().flatten() {
        requests.push((format!("{} request", s(&c["command"])), &c["request"]));
        for o in c["outcomes"].as_array().into_iter().flatten() {
            requests.push((format!("{}/{} lookup", s(&c["command"]), s(&o["outcome"])), &o["lookup"]));
        }
    }
    for v in t["views"].as_array().into_iter().flatten() {
        requests.push((format!("{} request", s(&v["view"])), &v["request"]));
    }
    for (what, r) in requests {
        for (name, value) in r["headers"].as_object().into_iter().flatten() {
            for token in std::iter::once(name.clone()).chain(literal_pieces(s(value))) {
                if let Some(at) = marked(&token) {
                    p.push(format!("{what}: header {name}: {token} is what {at} describes as a test, demo or mock mode"));
                }
            }
        }
    }
}

/// The table checked against the compiled specification (`ess compile` IR) and the delivered tree.
/// Empty when the table is accepted; otherwise every reason it is refused.
pub fn check_table(t: &J, ir: &J, tree: &Path) -> Vec<String> {
    let mut p = vec![];
    if t["format"] != TABLE_FORMAT {
        p.push(format!("format is {}, not {TABLE_FORMAT}", t["format"]));
    }
    check_start_from(t, ir, tree, &mut p);
    check_test_mode(t, tree, &mut p);
    if !t["build"].is_null() {
        check_argv("build", &t["build"]["argv"], false, &mut p);
    }
    check_argv("start", &t["start"]["argv"], true, &mut p);
    let ready = &t["start"]["ready"];
    if !ready.is_object() {
        p.push("start.ready: missing (a request the product answers once it is up)".into());
    } else {
        check_request("start.ready", ready, &BTreeSet::new(), &BTreeSet::new(), &mut p);
    }
    check_captures(t, ir, &mut p);
    let events = &ir["events"];
    let errors = &ir["errors"];
    let mut seen = BTreeSet::new();
    for (i, c) in t["commands"].as_array().into_iter().flatten().enumerate() {
        let name = s(&c["command"]).to_string();
        let what = format!("commands[{i}] {name}");
        let Some(spec) = by_name(&ir["commands"], &name) else {
            p.push(format!("{what}: no such command in the specification"));
            continue;
        };
        if !seen.insert(name.clone()) {
            p.push(format!("{what}: a second entry for the same command"));
            continue;
        }
        if check_none(&what, c, &mut p) {
            continue;
        }
        let inputs: BTreeSet<String> = spec["input"].as_array().into_iter().flatten().map(|f| s(&f["name"]).to_string()).collect();
        check_request(&format!("{what} request"), &c["request"], &inputs, &BTreeSet::new(), &mut p);
        let spec_outcomes = names(&spec["outcomes"]);
        let mut covered = BTreeSet::new();
        let mut keys: BTreeMap<(String, String, i64), Vec<(String, bool)>> = BTreeMap::new();
        let forced: Vec<&J> = c["forced"].as_array().into_iter().flatten().collect();
        for (j, f) in forced.iter().enumerate() {
            let fo = s(&f["outcome"]);
            let external = by_name(&spec["outcomes"], fo).is_some_and(|o| o["condition"]["kind"] == "external");
            if !external {
                p.push(format!("{what} forced[{j}]: {fo} is not an external outcome of the command"));
            }
            check_request(&format!("{what} forced[{j}] request"), &f["request"], &inputs, &BTreeSet::new(), &mut p);
            if !same_route(&c["request"], &f["request"]) {
                p.push(format!("{what} forced[{j}]: a forced request keeps the method and route of the command's request ({} {}); only a placeholder segment, the query, the body and the headers may differ",
                    s(&c["request"]["method"]), s(&c["request"]["path"])));
            }
        }
        for (j, o) in c["outcomes"].as_array().into_iter().flatten().enumerate() {
            let on = s(&o["outcome"]).to_string();
            let ow = format!("{what} outcomes[{j}] {on}");
            let Some(so) = by_name(&spec["outcomes"], &on) else {
                p.push(format!("{ow}: no such outcome of the command"));
                continue;
            };
            covered.insert(on.clone());
            if check_none(&ow, o, &mut p) {
                continue;
            }
            let Some(statuses) = statuses(o) else {
                p.push(format!("{ow}: status is not an HTTP status or a non-empty list of HTTP statuses"));
                continue;
            };
            if so["condition"]["kind"] == "wrong_state" && statuses.contains(&404) {
                p.push(format!("{ow}: 404 on the wrong_state outcome is the adapter's to decide: a refusal of an id the scenario never created is read as {on}, and 404 for an id it created never is"));
            }
            let lookup = o["lookup"].is_object();
            if lookup {
                check_request(&format!("{ow} lookup"), &o["lookup"], &inputs, &BTreeSet::new(), &mut p);
            }
            let disc = o["when_body"].is_object();
            if disc && (o["when_body"]["path"].as_str().is_none() || o["when_body"].get("equals").is_none()) {
                p.push(format!("{ow}: when_body needs path and equals"));
            }
            let mut requests = vec![&c["request"]];
            if so["condition"]["kind"] == "external" {
                let before = requests.len();
                requests.extend(forced.iter().filter(|f| s(&f["outcome"]) == on).map(|f| &f["request"]));
                if requests.len() == before {
                    p.push(format!("{ow}: {on} is external and needs a forced request (or none with a kind)"));
                }
            }
            for r in requests {
                for status in &statuses {
                    keys.entry((s(&r["method"]).to_string(), s(&r["path"]).to_string(), *status)).or_default().push((on.clone(), disc));
                }
            }
            let declared: BTreeSet<String> = so["emits"].as_array().into_iter().flatten().map(|e| s(e).to_string()).collect();
            let mut built = BTreeSet::new();
            for (k, e) in o["events"].as_array().into_iter().flatten().enumerate() {
                let en = s(&e["event"]).to_string();
                let Some(se) = by_name(events, &en) else {
                    p.push(format!("{ow} events[{k}]: no such event {en}"));
                    continue;
                };
                built.insert(en.clone());
                let want = field_names(se);
                let got: BTreeSet<String> = e["payload"].as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default();
                if want != got {
                    p.push(format!("{ow} events[{k}] {en}: payload fields {got:?}, the specification declares {want:?}"));
                }
                check_reads(&format!("{ow} events[{k}]"), &e["payload"], &inputs, lookup, &mut p);
                for (field, v) in e["payload"].as_object().into_iter().flatten().filter(|(_, v)| !has_placeholder(v)) {
                    p.push(format!("{ow} events[{k}] {en}: payload field {field} is the literal {v}; {LITERAL_WHY}"));
                }
            }
            if built != declared {
                p.push(format!("{ow}: builds events {built:?}, the outcome emits {declared:?}"));
            }
            match (so["error"].as_str(), o["error"].is_object()) {
                (Some(en), true) => {
                    if s(&o["error"]["error"]) != en {
                        p.push(format!("{ow}: error {}, the outcome declares {en}", o["error"]["error"]));
                    }
                    let want = by_name(errors, en).map(field_names).unwrap_or_default();
                    let got: BTreeSet<String> = o["error"]["fields"].as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default();
                    if want != got {
                        p.push(format!("{ow}: error fields {got:?}, {en} declares {want:?}"));
                    }
                    check_reads(&format!("{ow} error"), &o["error"]["fields"], &inputs, lookup, &mut p);
                    for (field, v) in o["error"]["fields"].as_object().into_iter().flatten().filter(|(_, v)| !has_placeholder(v)) {
                        p.push(format!("{ow}: error field {field} is the literal {v}; {LITERAL_WHY}"));
                    }
                }
                (Some(en), false) => p.push(format!("{ow}: the outcome declares error {en}, the entry gives none")),
                (None, true) => p.push(format!("{ow}: gives an error the outcome does not declare")),
                (None, false) => {}
            }
        }
        for missing in spec_outcomes.difference(&covered) {
            p.push(format!("{what}: outcome {missing} is not covered (map it to a status, or declare it none with a kind and why)"));
        }
        for ((m, path, st), outs) in &keys {
            let distinct: BTreeSet<&String> = outs.iter().map(|(o, _)| o).collect();
            if distinct.len() > 1 && outs.iter().any(|(_, d)| !d) {
                p.push(format!("{what}: {m} {path} status {st} maps to {distinct:?} without a when_body discriminator on each"));
            }
        }
    }
    for missing in names(&ir["commands"]).difference(&seen) {
        p.push(format!("command {missing} is not covered"));
    }
    let mut vseen = BTreeSet::new();
    let mut vreq: BTreeMap<(String, String), String> = BTreeMap::new();
    for (i, v) in t["views"].as_array().into_iter().flatten().enumerate() {
        let name = s(&v["view"]).to_string();
        let what = format!("views[{i}] {name}");
        let Some(spec) = by_name(&ir["views"], &name) else {
            p.push(format!("{what}: no such view in the specification"));
            continue;
        };
        if !vseen.insert(name.clone()) {
            p.push(format!("{what}: a second entry for the same view"));
            continue;
        }
        if check_none(&what, v, &mut p) {
            continue;
        }
        let params: BTreeSet<String> = spec["params"].as_array().into_iter().flatten().map(|f| s(&f["name"]).to_string()).collect();
        check_request(&format!("{what} request"), &v["request"], &BTreeSet::new(), &params, &mut p);
        // Two views read from one request get the same rows: at most one of them can be the view
        // the specification declares, and the other could pass a check its real view would fail.
        let key = (s(&v["request"]["method"]).to_string(), s(&v["request"]["path"]).to_string());
        if let Some(other) = vreq.get(&key) {
            p.push(format!("views {other} and {name} read the same request ({} {})", key.0, key.1));
        } else {
            vreq.insert(key, name.clone());
        }
        if !v["rows"].is_string() {
            p.push(format!("{what}: rows is not a path to the list of rows in the response (\"\" for the whole body)"));
        }
        let want = field_names(spec);
        let got: BTreeSet<String> = v["fields"].as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default();
        if want != got || !v["fields"].as_object().is_some_and(|m| m.values().all(J::is_string)) {
            p.push(format!("{what}: fields {got:?} (each a path in a row), the view declares {want:?}"));
        }
    }
    for missing in names(&ir["views"]).difference(&vseen) {
        p.push(format!("view {missing} is not covered"));
    }
    p
}

/// The response schema the table node replies with (the program check above is the real one).
pub fn table_schema() -> J {
    let request = json!({"type": "object", "required": ["method", "path"],
        "properties": {"method": {"type": "string", "enum": METHODS}, "path": {"type": "string"}, "body": {},
            "headers": {"type": "object", "additionalProperties": {"type": "string"}}}});
    let none = json!({"type": "object", "required": ["kind", "why"],
        "properties": {"kind": {"type": "string", "enum": NONE_KINDS}, "why": {"type": "string", "minLength": 1}}});
    let captures = json!({"type": "array", "items": {"type": "object", "required": ["name", "stream", "regex", "timeout_s"],
        "properties": {"name": {"type": "string"}, "stream": {"type": "string", "enum": ["stdout", "stderr"]},
            "regex": {"type": "string"}, "timeout_s": {"type": "integer", "minimum": 1, "maximum": 120}}}});
    let bindings = json!({"type": "array", "items": {"type": "object", "required": ["input", "capture"],
        "properties": {"input": {"type": "string"}, "capture": {"type": "string"}}}});
    let outcome = json!({"type": "object", "required": ["outcome"], "properties": {
        "outcome": {"type": "string"}, "none": none,
        "status": {"anyOf": [{"type": "integer"}, {"type": "array", "items": {"type": "integer"}, "minItems": 1}]},
        "when_body": {"type": "object", "required": ["path", "equals"], "properties": {"path": {"type": "string"}, "equals": {}}},
        "lookup": request,
        "events": {"type": "array", "items": {"type": "object", "required": ["event", "payload"],
            "properties": {"event": {"type": "string"}, "payload": {"type": "object"}}}},
        "error": {"type": "object", "required": ["error", "fields"],
            "properties": {"error": {"type": "string"}, "fields": {"type": "object"}}}}});
    json!({"type": "object", "additionalProperties": false,
        "required": ["format", "build", "start", "commands", "views", "notes"],
        "properties": {
            "format": {"type": "string", "enum": [TABLE_FORMAT]},
            "captures": captures,
            "bindings": bindings,
            "build": {"anyOf": [{"type": "null"}, {"type": "object", "required": ["argv"],
                "properties": {"argv": {"type": "array", "items": {"type": "string"}, "minItems": 1}}}]},
            "start": {"type": "object", "required": ["argv", "ready", "from"],
                "properties": {"argv": {"type": "array", "items": {"type": "string"}, "minItems": 1}, "ready": request,
                    "args": {"type": "array", "items": {"type": "object", "required": ["name", "kind", "field", "when_absent"],
                        "properties": {"name": {"type": "string"}, "kind": {"type": "string", "enum": ["from_scenario"]}, "field": {"type": "string"},
                            "nth": {"type": "integer", "minimum": 0}, "when_absent": {"type": "string"}}}},
                    "from": {"type": "array", "minItems": 1, "items": {"type": "object", "required": ["file", "line"],
                        "properties": {"file": {"type": "string"}, "line": {"type": "integer", "minimum": 1}}}}}},
            "commands": {"type": "array", "items": {"type": "object", "required": ["command"], "properties": {
                "command": {"type": "string"}, "none": none, "request": request,
                "forced": {"type": "array", "items": {"type": "object", "required": ["outcome", "request"],
                    "properties": {"outcome": {"type": "string"}, "request": request}}},
                "outcomes": {"type": "array", "items": outcome}}}},
            "views": {"type": "array", "items": {"type": "object", "required": ["view"], "properties": {
                "view": {"type": "string"}, "none": none, "request": request,
                "rows": {"type": "string"}, "fields": {"type": "object"}}}},
            "notes": {"type": "string"}}})
}

// ---- HTTP to 127.0.0.1 ----------------------------------------------------------------------

pub struct Reply {
    pub status: u16,
    pub body: String,
}

impl Reply {
    pub fn json(&self) -> J {
        serde_json::from_str(&self.body).unwrap_or(J::Null)
    }
}

fn dechunk(b: &[u8]) -> Vec<u8> {
    let mut out = vec![];
    let mut rest = b;
    while let Some(i) = rest.windows(2).position(|w| w == b"\r\n") {
        let size = usize::from_str_radix(String::from_utf8_lossy(&rest[..i]).split(';').next().unwrap_or("").trim(), 16).unwrap_or(0);
        if size == 0 || rest.len() < i + 2 + size {
            break;
        }
        out.extend_from_slice(&rest[i + 2..i + 2 + size]);
        rest = &rest[(i + 4 + size).min(rest.len())..];
    }
    out
}

/// One request to `127.0.0.1:<port>`, `Connection: close`, with a JSON body when given and the
/// extra headers (a header holding CR or LF is refused).
pub fn http(port: u16, method: &str, path: &str, body: Option<&J>, headers: &[(String, String)], timeout: Duration) -> Result<Reply, String> {
    if headers.iter().any(|(k, v)| k.contains(['\r', '\n', ':']) || v.contains(['\r', '\n'])) {
        return Err("a header holds a line break".into());
    }
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut st = TcpStream::connect_timeout(&addr, timeout).map_err(|e| format!("connect: {e}"))?;
    st.set_read_timeout(Some(timeout)).ok();
    st.set_write_timeout(Some(timeout)).ok();
    let payload = body.map(|b| b.to_string()).unwrap_or_default();
    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nAccept: application/json\r\n");
    if body.is_some() {
        req.push_str("Content-Type: application/json\r\n");
    }
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str(&format!("Content-Length: {}\r\n\r\n{payload}", payload.len()));
    st.write_all(req.as_bytes()).map_err(|e| format!("write: {e}"))?;
    let mut buf = vec![];
    st.read_to_end(&mut buf).map_err(|e| format!("read: {e}"))?;
    let Some(h) = buf.windows(4).position(|w| w == b"\r\n\r\n") else {
        return Err("no HTTP response head".into());
    };
    let head = String::from_utf8_lossy(&buf[..h]).to_string();
    let status = head.split_whitespace().nth(1).and_then(|x| x.parse().ok()).ok_or("no status in the response")?;
    let mut raw = buf[h + 4..].to_vec();
    let lower = head.to_ascii_lowercase();
    if lower.contains("transfer-encoding: chunked") {
        raw = dechunk(&raw);
    } else if let Some(n) = lower.lines().find_map(|l| l.strip_prefix("content-length:")).and_then(|x| x.trim().parse::<usize>().ok()) {
        raw.truncate(n);
    }
    Ok(Reply { status, body: String::from_utf8_lossy(&raw).to_string() })
}

// ---- the ess pin ----------------------------------------------------------------------------

pub fn target_triple() -> String {
    let arch = std::env::consts::ARCH;
    match std::env::consts::OS {
        "macos" => format!("{arch}-apple-darwin"),
        "linux" => format!("{arch}-unknown-linux-gnu"),
        os => format!("{arch}-{os}"),
    }
}

pub struct Ess {
    pub path: PathBuf,
    pub version: String,
    pub sha256: String,
    pub source: &'static str,
}

/// The cache path of the pinned binary: `$SDC_ESS_CACHE`, else `$XDG_CACHE_HOME/sdc/ess`, else
/// `$HOME/.cache/sdc/ess`, then `<version>/<target>/ess`.
pub fn cache_path(pin: &J) -> PathBuf {
    let base = std::env::var_os("SDC_ESS_CACHE").map(PathBuf::from).unwrap_or_else(|| {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache"))
            .join("sdc/ess")
    });
    base.join(s(&pin["version"])).join(target_triple()).join("ess")
}

/// The pinned ess: `ESS_BIN` when set, else the cache path. Refused (`Err((cause, why))`) when the
/// pin cannot be read, the binary is missing, or its digest or `--version` is not the pinned one.
/// The `ess` on PATH is never used.
pub fn resolve_ess(pin_path: &Path) -> Result<Ess, (&'static str, String)> {
    let pin = read_json(pin_path).map_err(|e| ("ess_pin_unreadable", e))?;
    if pin["format"] != PIN_FORMAT {
        return Err(("ess_pin_unreadable", format!("{}: format is not {PIN_FORMAT}", pin_path.display())));
    }
    let target = target_triple();
    let want = s(&pin["platforms"][&target]["binary_sha256"]).to_string();
    if want.is_empty() {
        return Err(("ess_pin_mismatch", format!("the pin names no binary for {target}")));
    }
    let (path, source) = match std::env::var_os("ESS_BIN").filter(|v| !v.is_empty()) {
        Some(p) => (PathBuf::from(p), "ESS_BIN"),
        None => (cache_path(&pin), "cache"),
    };
    if !path.is_file() {
        return Err(("ess_missing", format!("{} ({source}) does not exist: run `sdc-conform ess-install`", path.display())));
    }
    let got = sha256_file(&path).map_err(|e| ("ess_missing", e))?;
    if got != want {
        return Err(("ess_pin_mismatch", format!("{} ({source}) has sha256 {got}, the pin {want}", path.display())));
    }
    let out = std::process::Command::new(&path).arg("--version").output().map_err(|e| ("ess_missing", format!("{}: {e}", path.display())))?;
    let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if version != format!("ess {}", s(&pin["version"])) {
        return Err(("ess_pin_mismatch", format!("{} says {version:?}, the pin is {}", path.display(), s(&pin["version"]))));
    }
    Ok(Ess { path, version: s(&pin["version"]).to_string(), sha256: got, source })
}
