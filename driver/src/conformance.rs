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
            if !start || !START_PLACEHOLDERS.contains(&ph.as_str()) {
                p.push(format!("{what}.argv: unknown placeholder {ph} (start may use {})", START_PLACEHOLDERS.join(", ")));
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

/// The table checked against the compiled specification (`ess compile` IR). Empty when the table
/// is accepted; otherwise every reason it is refused.
pub fn check_table(t: &J, ir: &J) -> Vec<String> {
    let mut p = vec![];
    if t["format"] != TABLE_FORMAT {
        p.push(format!("format is {}, not {TABLE_FORMAT}", t["format"]));
    }
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
            let Some(status) = o["status"].as_i64().filter(|x| (100..600).contains(x)) else {
                p.push(format!("{ow}: status is not an HTTP status"));
                continue;
            };
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
                keys.entry((s(&r["method"]).to_string(), s(&r["path"]).to_string(), status)).or_default().push((on.clone(), disc));
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
        "outcome": {"type": "string"}, "none": none, "status": {"type": "integer"},
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
            "start": {"type": "object", "required": ["argv", "ready"],
                "properties": {"argv": {"type": "array", "items": {"type": "string"}, "minItems": 1}, "ready": request}},
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
