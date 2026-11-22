//! The node's whole input as one prompt and the manifest of what went in.
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::{json, Map, Value};

use super::tree::{product_files, select_tree, TreeFile, NOT_UTF8, OVER_CAP};
use super::MAX_CONTINUATIONS;
use crate::instructions;
use crate::reply::review::{covers, domain_of, review_scope, story_summary};
use crate::reply::{read_data, read_json, sorted};

/// What `assemble` needs.
pub struct Inputs<'a> {
    /// The case directory whose `work/` holds the story and the specification.
    pub case_dir: &'a Path,
    /// The tree whose current text is shown (the call's `out/impl/`).
    pub tree: &'a Path,
    /// The story's id.
    pub story: &'a str,
    /// The feedback text of the last run, when there is one.
    pub feedback: Option<String>,
    /// The last rejection of the change (sdc.rejection/1), when this is a rework.
    pub rejection: Option<Value>,
    /// The rework limit named in the rework section.
    pub rework_limit: Option<u64>,
    /// The cap on inlined bytes (`INLINE_CAP`).
    pub cap: usize,
    /// A continuation: the paths written so far and the paths asked for now.
    pub continuation: Option<(Vec<String>, Vec<String>)>,
}

/// The assembled prompt and the manifest of what went in.
pub struct Assembly {
    /// The whole prompt text.
    pub prompt: String,
    /// The `sdc.implement-inputs/1` manifest.
    pub manifest: Value,
}

/// A Markdown fence longer than any run of backticks in `text`.
fn fence(text: &str) -> String {
    let (mut longest, mut cur) = (0, 0);
    for ch in text.chars() {
        if ch == '`' {
            cur += 1;
            longest = longest.max(cur);
        } else {
            cur = 0;
        }
    }
    "`".repeat(longest.max(2) + 1)
}

/// An IR entry without its empty `naming` objects (they say nothing and cost tokens), and a type
/// reference `{kind: primitive|declared, name}` written as its name.
fn strip_naming(v: &Value) -> Value {
    match v {
        Value::Object(m) if m.len() == 2 && matches!(m.get("kind").and_then(|k| k.as_str()), Some("primitive" | "declared"))
            && m.get("name").is_some_and(|n| n.is_string()) => m["name"].clone(),
        Value::Object(m) => Value::Object(m.iter()
            .filter(|(k, x)| !(k.as_str() == "naming" && x.as_object().is_some_and(|o| o.is_empty())))
            .map(|(k, x)| (k.clone(), strip_naming(x))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(strip_naming).collect()),
        _ => v.clone(),
    }
}

fn strings_in(v: &Value, out: &mut BTreeSet<String>) {
    match v {
        Value::String(s) => {
            out.insert(s.clone());
        }
        Value::Array(a) => a.iter().for_each(|x| strings_in(x, out)),
        Value::Object(m) => m.values().for_each(|x| strings_in(x, out)),
        _ => {}
    }
}

/// The specification a story must serve: its covered commands (with their outcomes), the views
/// its requirements trace to and the views of its commands' domains, and the entities, events,
/// errors and types those name, followed name by name. Also the covered names the IR lacks.
fn spec_facts(ir: &Value, traces: &Value, cov: &[String], mine: &[Value]) -> (Value, Vec<String>) {
    let mut out = Map::new();
    let commands: Map<String, Value> = cov.iter().filter_map(|c| ir["commands"].get(c).map(|x| (c.clone(), strip_naming(x)))).collect();
    let missing: Vec<String> = cov.iter().filter(|c| ir["commands"].get(c.as_str()).is_none()).cloned().collect();
    let doms: BTreeSet<&str> = cov.iter().map(|c| domain_of(c)).collect();
    let traced: BTreeSet<String> = mine.iter()
        .flat_map(|r| traces[r["id"].as_str().unwrap_or("")].as_array().cloned().unwrap_or_default())
        .filter_map(|x| x.as_str().map(String::from)).collect();
    let views: Map<String, Value> = ir["views"].as_object().into_iter().flatten()
        .filter(|(k, _)| traced.contains(k.as_str()) || doms.contains(domain_of(k)))
        .map(|(k, v)| (k.clone(), strip_naming(v))).collect();
    let mut names = BTreeSet::new();
    strings_in(&Value::Object(commands.clone()), &mut names);
    strings_in(&Value::Object(views.clone()), &mut names);
    out.insert("commands".into(), Value::Object(commands));
    out.insert("views".into(), Value::Object(views));
    let kinds = ["entities", "events", "errors", "types"];
    let mut picked: Vec<Map<String, Value>> = vec![Map::new(); kinds.len()];
    loop {
        let before: usize = picked.iter().map(|m| m.len()).sum();
        for (i, k) in kinds.iter().enumerate() {
            for (name, v) in ir[*k].as_object().into_iter().flatten() {
                if names.contains(name) && !picked[i].contains_key(name) {
                    let v = strip_naming(v);
                    strings_in(&v, &mut names);
                    picked[i].insert(name.clone(), v);
                }
            }
        }
        if picked.iter().map(|m| m.len()).sum::<usize>() == before {
            break;
        }
    }
    for (k, m) in kinds.iter().zip(picked) {
        if !m.is_empty() {
            out.insert((*k).into(), Value::Object(m));
        }
    }
    (sorted(Value::Object(out)), missing)
}

/// The node's whole input as one prompt, and a manifest of what went in (sections and bytes,
/// the files inlined and listed with the rank that put them there).
pub fn assemble(i: &Inputs) -> Result<Assembly, String> {
    let work = i.case_dir.join("work");
    let rel = format!("aep/story/{}.md", i.story);
    let story_text = fs::read_to_string(work.join(&rel)).map_err(|e| format!("{rel}: {e}"))?;
    let (sl, summary) = story_summary(&story_text).ok_or(format!("{rel} has no summary"))?;
    let cov = covers(&summary);
    let mut acceptance = vec![(format!("{rel}:{sl}"), summary.clone())];
    let mut in_acc = false;
    for (n, l) in story_text.lines().enumerate() {
        if l.starts_with("## ") {
            in_acc = l.trim() == "## Acceptance";
            continue;
        }
        if in_acc && !l.trim().is_empty() {
            acceptance.push((format!("{rel}:{}", n + 1), l.trim().to_string()));
        }
    }
    let reqs = read_data(&work.join("requirements.yaml")).map(|v| v["requirements"].as_array().cloned().unwrap_or_default()).unwrap_or_default();
    let traces = read_json(&work.join("spec/traces.json"))["traces"].clone();
    let ir = read_json(&work.join(".sdc/gate/ir.json"));
    let (mine, _) = review_scope(&reqs, &traces, &ir["commands"], &cov, std::slice::from_ref(&cov));
    let (spec, missing) = spec_facts(&ir, &traces, &cov, &mine);
    let sel = select_tree(i.tree, &work.join("mainline"), &story_text, i.cap);

    let mut sections: Vec<(&str, String)> = vec![("head", instructions::render("instructions/change.implement.request.head.md", &[])? + "\n")];
    let f = fence(&story_text);
    sections.push(("story", instructions::render("instructions/change.implement.request.story.md",
        &[("story", i.story), ("fence", &f), ("text", story_text.trim_end())])? + "\n"));
    let mut r = instructions::render("instructions/change.implement.request.requirements.heading.md", &[])? + "\n\n";
    if mine.is_empty() {
        r.push_str(&instructions::render("instructions/change.implement.request.requirements.none.md", &[])?);
        r.push('\n');
    }
    for q in &mine {
        r.push_str(&format!("- {}: {}\n", q["id"].as_str().unwrap_or("?"), q["text"].as_str().unwrap_or("")));
    }
    r.push('\n');
    r.push_str(&instructions::render("instructions/change.implement.request.requirements.acceptance.md", &[])?);
    r.push_str("\n\n");
    for (c, t) in &acceptance {
        r.push_str(&format!("- {t} ({c})\n"));
    }
    sections.push(("requirements", r));
    let mut s = instructions::render("instructions/change.implement.request.spec.heading.md", &[])? + "\n\n";
    if !missing.is_empty() {
        s.push_str(&instructions::render("instructions/change.implement.request.spec.missing.md", &[("names", &missing.join(", "))])?);
        s.push_str("\n\n");
    }
    // One compact JSON line per declaration (a type reference is written as its type's name).
    for (kind, decls) in spec.as_object().into_iter().flatten() {
        s.push_str(&format!("{kind}:\n"));
        for (name, d) in decls.as_object().into_iter().flatten() {
            s.push_str(&format!("- {name}: {d}\n"));
        }
        s.push('\n');
    }
    sections.push(("spec", s));
    let bc = read_data(&work.join("build-constraints.yaml"));
    let constraints = match &bc {
        Some(v) => {
            let mut t = v["text"].as_str().unwrap_or("").trim().to_string();
            for k in ["language", "edition", "dependencies"] {
                if !v[k].is_null() {
                    t.push_str(&format!("\n{k}: {}", if v[k].is_string() { v[k].as_str().unwrap_or("").to_string() } else { v[k].to_string() }));
                }
            }
            t
        }
        None => instructions::render("instructions/change.implement.request.build.md", &[])?,
    };
    let heading = if bc.is_some() { "instructions/change.implement.request.constraints.heading.run.md" } else { "instructions/change.implement.request.constraints.heading.md" };
    sections.push(("constraints", format!("{}\n\n{constraints}\n", instructions::render(heading, &[])?)));
    let mut t = instructions::render("instructions/change.implement.request.tree.heading.md", &[])? + "\n\n";
    if sel.inlined.is_empty() && sel.listed.is_empty() {
        t.push_str(&instructions::render("instructions/change.implement.request.tree.empty.md", &[])?);
        t.push('\n');
    } else {
        let listed = if sel.listed.is_empty() { String::new() } else {
            instructions::render("instructions/change.implement.request.tree.count.listed.md", &[("count", &sel.listed.len().to_string())])?
        };
        t.push_str(&instructions::render("instructions/change.implement.request.tree.count.md",
            &[("files", &sel.inlined.len().to_string()), ("bytes", &sel.inlined_bytes.to_string()), ("listed", &listed)])?);
        t.push_str("\n\n");
        for f in &sel.inlined {
            let text = fs::read_to_string(i.tree.join(&f.path)).unwrap_or_default();
            let body = format!("{text}{}", if text.ends_with('\n') || text.is_empty() { "" } else { "\n" });
            t.push_str(&instructions::render("instructions/change.implement.request.tree.file.md",
                &[("path", &f.path), ("bytes", &f.bytes.to_string()), ("fence", &fence(&text)), ("body", &body)])?);
            t.push_str("\n\n");
        }
        if !sel.listed.is_empty() {
            t.push_str(&instructions::render("instructions/change.implement.request.tree.listed.md", &[])?);
            t.push_str("\n\n");
            for f in &sel.listed {
                let why = match f.why {
                    NOT_UTF8 => instructions::render("instructions/change.implement.request.tree.why.not-utf8.md", &[])?,
                    OVER_CAP => instructions::render("instructions/change.implement.request.tree.why.over-cap.md", &[])?,
                    other => return Err(format!("no instruction text for the reason {other:?} to list a file without text")),
                };
                t.push_str(&instructions::render("instructions/change.implement.request.tree.listed.item.md",
                    &[("path", &f.path), ("bytes", &f.bytes.to_string()), ("why", &why)])?);
                t.push('\n');
            }
            t.push('\n');
        }
    }
    sections.push(("tree", t));
    if let Ok(ft) = fs::read_to_string(work.join("findings.yaml")) {
        sections.push(("findings", instructions::render("instructions/change.implement.request.findings.md",
            &[("fence", &fence(&ft)), ("text", ft.trim_end())])? + "\n"));
    }
    if let Some(fb) = &i.feedback {
        sections.push(("feedback", instructions::render("instructions/change.implement.request.feedback.md", &[("feedback", fb.trim_end())])? + "\n"));
    }
    if let Some(rj) = &i.rejection {
        let (count, limit) = (rj["count"].to_string(), i.rework_limit.map_or("?".to_string(), |n| n.to_string()));
        let mut x = instructions::render("instructions/change.implement.request.rework.md", &[("approval", rj["approval"].as_str().unwrap_or("?")),
            ("revision", rj["revision"].as_str().unwrap_or("?")), ("count", &count), ("limit", &limit),
            ("principal", rj["principal"].as_str().unwrap_or("its approver"))])? + "\n\n";
        for (n, reason) in rj["reasons"].as_array().into_iter().flatten().enumerate() {
            x.push_str(&format!("{}. {}\n", n + 1, reason.as_str().unwrap_or("")));
        }
        sections.push(("rejection", x));
    }
    let max_continuations = MAX_CONTINUATIONS.to_string();
    sections.push(("task", instructions::render("instructions/change.implement.request.task.md", &[("story", i.story), ("max_continuations", &max_continuations)])? + "\n"));
    if let Some((written, asked)) = &i.continuation {
        sections.push(("continuation", instructions::render("instructions/change.implement.request.continuation.md",
            &[("written", &written.join(", ")), ("asked", &asked.join(", "))])? + "\n"));
    }
    let prompt = sections.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>().join("\n");
    let tf = |v: &Vec<TreeFile>| v.iter().map(|f| json!({"path": f.path, "bytes": f.bytes, "why": f.why})).collect::<Vec<_>>();
    let manifest = json!({"format": "sdc.implement-inputs/1", "story": i.story,
        "requirements": mine.iter().map(|r| r["id"].clone()).collect::<Vec<_>>(),
        "commands": spec["commands"].as_object().map(|m| m.keys().cloned().collect::<Vec<_>>()).unwrap_or_default(),
        "views": spec["views"].as_object().map(|m| m.keys().cloned().collect::<Vec<_>>()).unwrap_or_default(),
        "missing_commands": missing, "constraints": if bc.is_some() { "build-constraints.yaml" } else { "default" },
        "tree": {"rule": "named by the story, this story's change (all of it for a first story), crate root, merged by an earlier story; inlined in that order up to the cap, the rest listed",
            "cap": i.cap, "first_story": sel.first_story, "inlined_bytes": sel.inlined_bytes, "inlined": tf(&sel.inlined), "listed": tf(&sel.listed)},
        "sections": sections.iter().map(|(k, s)| ((*k).to_string(), json!(s.len()))).collect::<Map<String, Value>>(),
        "prompt_bytes": prompt.len()});
    Ok(Assembly { prompt, manifest: sorted(manifest) })
}

/// A reply of every product file of a tree (fixture adapter, tests).
pub fn pack(tree: &Path, note: &str) -> Value {
    let files: Vec<Value> = product_files(tree).into_iter()
        .filter_map(|p| fs::read_to_string(tree.join(&p)).ok().map(|c| json!({"path": p, "content": c}))).collect();
    sorted(json!({"files": files, "note": note, "findings": [], "remaining": []}))
}
