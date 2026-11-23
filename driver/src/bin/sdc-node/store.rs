//! The apply step of the five nodes whose store is not a plain copy: `vision.write`, `roadmap.write`, `plan.decompose` through `aep`, and
//! `context.default` and `questions.answer` on the case's own YAML. `jq` and `yq` are not started:
//! what the script read with them is read here from parsed documents, with their edge behaviour
//! where it shows (a field that is absent is `null`, an unreadable document is empty text).

use crate::ordjson::Ov;
use crate::util::{chomp, run_capture, run_cmd, Out, R};
use crate::{yaml, Ctx};
use serde_yaml::{Mapping, Value};
use std::fs;
use std::path::Path;
use std::process::Command;

/// `rm -f path`: a file or a link, never a directory; nothing to say when it is absent.
fn rm_f(path: &Path) {
    if fs::symlink_metadata(path).is_ok_and(|m| !m.is_dir()) {
        let _ = fs::remove_file(path);
    }
}

fn read_ov(path: &Path) -> Option<Ov> {
    Ov::parse(&fs::read_to_string(path).ok()?).ok()
}

/// `$(jq -r .key)`: the member as text without its trailing newlines, `null` for a missing one;
/// the empty text where jq refuses (a document that is not an object) or cannot read.
fn field(doc: Option<&Ov>, key: &str) -> String {
    match doc {
        Some(d @ Ov::Obj(_)) => chomp(&d.get(key).map_or_else(|| "null".to_string(), Ov::raw)).to_string(),
        Some(Ov::Null) => "null".to_string(),
        _ => String::new(),
    }
}

/// `jq -e '.key | length > 0'` of the document: true only for a non-empty value of an object.
fn nonempty(doc: Option<&Ov>, key: &str) -> bool {
    let Some(d @ Ov::Obj(_)) = doc else { return false };
    match d.get(key) {
        Some(Ov::Arr(a)) => !a.is_empty(),
        Some(Ov::Obj(m)) => !m.is_empty(),
        Some(Ov::Str(s)) => !s.is_empty(),
        Some(Ov::Num(n)) => n.as_f64().is_none_or(|f| f != 0.0),
        Some(Ov::Lit(_)) => true,
        _ => false,
    }
}

/// `jq -c '.key[]'` fails on a value that is neither an array nor an object.
fn iterable(doc: &Ov, key: &str) -> bool {
    matches!(doc.get(key), Some(Ov::Arr(_) | Ov::Obj(_)))
}

fn aep(c: &Ctx, args: &[&str], sink: &mut Out) -> i32 {
    run_cmd(Command::new("aep").args(args).args(["--root", "."]).current_dir(&c.work), sink)
}

/// The sink's order of output for one `aep` that prints the artifact it made: standard output and
/// standard error both go to the apply log.
fn new_artifact(c: &Ctx, kind: &str, name: &str, title: &str, summary: &str, extra: &[&str], sink: &mut Out) -> i32 {
    let mut args = vec!["plan", "artifact", "new", kind, name, "--store", "aep", "--title", title, "--summary", summary];
    args.extend_from_slice(extra);
    aep(c, &args, sink)
}

fn vision(c: &Ctx, cwd: &Path, sink: &mut Out) -> i32 {
    let (json, md) = (cwd.join("out/vision.json"), cwd.join("out/vision.md"));
    if !(json.is_file() && md.is_file()) {
        return 1;
    }
    rm_f(&c.work.join("aep/vision/product.md"));
    let doc = read_ov(&json);
    let (title, summary) = (field(doc.as_ref(), "title"), field(doc.as_ref(), "summary"));
    new_artifact(c, "vision", "product", &title, &summary, &["--from", &md.display().to_string()], sink)
}

/// `[.epics[].name] | join(" ")`: the names of the epics; the empty text where jq refuses.
fn epic_names(doc: &Ov) -> String {
    let mut names = Vec::new();
    for e in doc.get("epics").map(Ov::items).unwrap_or_default() {
        let name = match e {
            Ov::Obj(_) => e.get("name").cloned().unwrap_or(Ov::Null),
            Ov::Null => Ov::Null,
            _ => return String::new(),
        };
        names.push(match name {
            Ov::Null => String::new(),
            Ov::Arr(_) | Ov::Obj(_) => return String::new(),
            other => other.raw(),
        });
    }
    chomp(&names.join(" ")).to_string()
}

fn roadmap(c: &Ctx, cwd: &Path, sink: &mut Out) -> i32 {
    let f = cwd.join("out/roadmap.json");
    let doc = if f.is_file() { read_ov(&f) } else { None };
    if !nonempty(doc.as_ref(), "epics") {
        return 1;
    }
    let Some(doc) = doc else { return 1 };
    rm_f(&c.work.join("aep/initiative/roadmap.md"));
    if let Ok(dir) = fs::read_dir(c.work.join("aep/epic")) {
        for e in dir.flatten() {
            if e.file_name().to_string_lossy().ends_with(".md") && !e.file_name().to_string_lossy().starts_with('.') {
                rm_f(&e.path());
            }
        }
    }
    let md = cwd.join("out/roadmap.md");
    let mut extra = Vec::new();
    let from = md.display().to_string();
    if fs::metadata(&md).is_ok_and(|m| m.len() > 0) {
        extra.extend(["--from", from.as_str()]);
    }
    let summary = format!("epics: {}", epic_names(&doc));
    if new_artifact(c, "initiative", "roadmap", "Roadmap", &summary, &extra, sink) != 0 {
        return 1;
    }
    // The pipeline's status: the loop's, or jq's when it could not read the list (pipefail).
    let mut status = if iterable(&doc, "epics") { 0 } else { 5 };
    for e in doc.get("epics").map(Ov::items).unwrap_or_default() {
        let e = Some(e);
        let (name, title, summary) = (field(e, "name"), field(e, "title"), field(e, "summary"));
        if new_artifact(c, "epic", &name, &title, &summary, &[], sink) != 0 {
            status = 1;
            break;
        }
    }
    status
}

fn decompose(c: &Ctx, cwd: &Path, sink: &mut Out) -> i32 {
    let f = cwd.join("out/stories.json");
    let doc = if f.is_file() { read_ov(&f) } else { None };
    if !nonempty(doc.as_ref(), "stories") {
        return 1;
    }
    let Some(doc) = doc else { return 1 };
    let epic = c.names().0;
    let target = format!("epic:{epic}");

    // The stories already delivered to this epic go first.
    let (_, listed) = run_capture(Command::new("aep").args(["plan", "artifact", "list", "--store", "aep", "--format", "json", "--root", "."]).current_dir(&c.work), sink);
    let list = Ov::parse(&String::from_utf8_lossy(&listed)).ok();
    let mut paths = Vec::new();
    for a in list.as_ref().map(Ov::items).unwrap_or_default() {
        let delivers = a.str_of("kind") == Some("story")
            && a.get("relations").is_some_and(|r| r.items().iter().any(|rel| rel.str_of("target") == Some(target.as_str())));
        if delivers {
            paths.push(a.get("path").map_or_else(|| "null".to_string(), Ov::raw));
        }
    }
    for p in paths.iter().flat_map(|p| p.split([' ', '\t', '\n'])).filter(|p| !p.is_empty()) {
        rm_f(&c.work.join(format!("aep/{p}")));
    }

    if !iterable(&doc, "stories") {
        return 1;
    }
    let actor = format!("agent:{}", c.node);
    let relation = format!("delivers:{target}");
    for s in doc.get("stories").map(Ov::items).unwrap_or_default() {
        let s = Some(s);
        let (name, title, summary) = (field(s, "name"), field(s, "title"), field(s, "summary"));
        let id = format!("story:{name}");
        let made = new_artifact(c, "story", &name, &title, &summary, &["--relate", &relation, "--relate", "serves:vision:product"], sink) == 0
            && aep(c, &["plan", "artifact", "scope", &id, "--store", "aep", "--add", "impl"], sink) == 0
            && aep(c, &["plan", "artifact", "move", &id, "--to", "active", "--store", "aep", "--executor", &actor], sink) == 0;
        if !made {
            return 1;
        }
    }
    // The epic itself becomes active unless it is.
    let (_, listed) = run_capture(Command::new("aep").args(["plan", "artifact", "list", "--store", "aep", "--format", "json", "--root", "."]).current_dir(&c.work), sink);
    let list = Ov::parse(&String::from_utf8_lossy(&listed)).ok();
    let status: Vec<String> = list
        .as_ref()
        .map(Ov::items)
        .unwrap_or_default()
        .into_iter()
        .filter(|a| a.str_of("id") == Some(target.as_str()))
        .map(|a| a.get("status").map_or_else(|| "null".to_string(), Ov::raw))
        .collect();
    if chomp(&status.join("\n")) == "active" {
        return 0;
    }
    aep(c, &["plan", "artifact", "move", &target, "--to", "active", "--store", "aep", "--executor", &actor], sink)
}

/// `with_entries(select(.value.status == "changed" or .value.status == "accepted"))` of the old
/// context: the decisions of a person.
fn decisions(old: &Mapping) -> Mapping {
    old.iter()
        .filter(|(_, v)| matches!(yaml::get(v, "status").and_then(Value::as_str), Some("changed" | "accepted")))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// yq's `*`: mappings merge deeply, everything else (a list too) is replaced.
fn merge(into: &mut Mapping, from: Mapping) {
    for (k, v) in from {
        match (into.get_mut(&k), v) {
            (Some(Value::Mapping(a)), Value::Mapping(b)) => merge(a, b),
            (Some(slot), v) => *slot = v,
            (None, v) => {
                into.insert(k, v);
            }
        }
    }
}

/// `yq -i '. *= (load(old) | decisions)' f`. A document that cannot be read leaves the file as it
/// is, as a refused yq does; so does an old context without a decision (the bytes stay).
fn keep_decisions(f: &Path, old: &Path) {
    let (Some(Value::Mapping(mut new)), Some(Value::Mapping(before))) = (yaml::load(f), yaml::load(old)) else { return };
    let keep = decisions(&before);
    if keep.is_empty() {
        return;
    }
    merge(&mut new, keep);
    if let Ok(text) = serde_yaml::to_string(&Value::Mapping(new)) {
        let _ = fs::write(f, text);
    }
}

fn context(c: &Ctx, cwd: &Path) -> i32 {
    let f = cwd.join("out/context.yaml");
    if !f.is_file() {
        return 1;
    }
    let old = c.work.join("context.yaml");
    if old.is_file() {
        keep_decisions(&f, &old);
    }
    i32::from(fs::copy(&f, &old).is_err())
}

/// `yq -o=json -I=0 '[.questions[] | select(.scope_changing == true)]'` of the file: the questions
/// that change the scope, in order, as one text; empty where yq refuses.
fn scope_changing(path: &Path) -> String {
    let Some(doc) = yaml::load(path) else { return String::new() };
    let Value::Mapping(m) = &doc else { return String::new() };
    let questions: Vec<&Value> = match m.get("questions") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Sequence(s)) => s.iter().collect(),
        Some(Value::Mapping(q)) => q.values().collect(),
        Some(_) => return String::new(),
    };
    let kept: Vec<&Value> = questions.into_iter().filter(|q| matches!(yaml::get(q, "scope_changing"), Some(Value::Bool(true)))).collect();
    serde_yaml::to_string(&kept).unwrap_or_default()
}

fn answer(c: &Ctx, cwd: &Path, sink: &mut Out) -> R<i32> {
    let f = cwd.join("out/questions.yaml");
    if !f.is_file() {
        return Ok(1);
    }
    let own = c.work.join("questions.yaml");
    if scope_changing(&f) != scope_changing(&own) {
        let line = c.render("instructions/shared/node.answer.scope-changed.md", &[("node", &c.node)])?;
        sink.line(line.as_bytes());
        return Ok(1);
    }
    Ok(i32::from(fs::copy(&f, &own).is_err()))
}

/// Applies the output of a store node; `None` for a node that is not one.
pub fn apply(c: &Ctx, cwd: &Path, sink: &mut Out) -> R<Option<i32>> {
    Ok(Some(match c.node.as_str() {
        "vision.write" => vision(c, cwd, sink),
        "roadmap.write" => roadmap(c, cwd, sink),
        "plan.decompose" => decompose(c, cwd, sink),
        "context.default" => context(c, cwd),
        "questions.answer" => answer(c, cwd, sink)?,
        _ => return Ok(None),
    }))
}
