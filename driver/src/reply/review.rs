//! Review inputs and the check of a review reply (`change.review`): the story's scope, the inputs
//! written into `in/`, and `check_review`.
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::{json, Map, Value};

use super::citations::{ctx, resolve};
use super::coverage::COVERAGE;
use super::records::Facts;
use super::review_doc::{changed_files, product_files, review_document, test_summary, ReviewFacts};
use super::schemas::review_targets;
use super::{pretty, read_data, read_json, sorted, walk};

// --- change.review (story:review-node-is-a-model) ------------------------------------------------

/// The front matter's `summary` of an AEP story file, and the line it starts on (1-based).
pub fn story_summary(text: &str) -> Option<(usize, String)> {
    let mut lines = text.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let front: Vec<&str> = lines.take_while(|l| l.trim() != "---").collect();
    let v: serde_yaml::Value = serde_yaml::from_str(&front.join("\n")).ok()?;
    let summary = v.get("summary")?.as_str()?.to_string();
    let at = front.iter().position(|l| l.starts_with("summary:"))? + 2;
    Some((at, summary))
}

/// The commands a story covers: the words after `covers` in its summary.
pub fn covers(summary: &str) -> Vec<String> {
    summary.split_once("covers ").map(|(_, c)| c.split_whitespace()
        .map(|w| w.trim_end_matches(['.', ',']).to_string()).filter(|w| !w.is_empty()).collect()).unwrap_or_default()
}

/// `todo.task.AddTask` -> `todo.task`.
pub fn domain_of(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(d, _)| d)
}

/// Whether a requirement is delivered by a story covering `cov`: a traced command it covers, or a
/// traced view or actor of a covered command's domain; without a trace entry, a `spec_domains`
/// entry that is a covered command's domain.
pub fn delivered_by(req: &Value, traces: &Value, commands: &Value, cov: &[String]) -> bool {
    let doms: BTreeSet<&str> = cov.iter().map(|c| domain_of(c)).collect();
    match traces[req["id"].as_str().unwrap_or("")].as_array() {
        Some(t) => t.iter().filter_map(|x| x.as_str()).any(|x| cov.iter().any(|c| c == x)
            || (commands.get(x).is_none() && doms.contains(domain_of(x)))),
        None => req["spec_domains"].as_array().into_iter().flatten().filter_map(|d| d.as_str()).any(|d| doms.contains(d)),
    }
}

/// The requirements of the story (`cov`) and the requirements no story of the plan covers
/// (`all_cov`: what every story covers). A constraint or a dropped requirement is in neither.
pub fn review_scope(reqs: &[Value], traces: &Value, commands: &Value, cov: &[String], all_cov: &[Vec<String>]) -> (Vec<Value>, Vec<Value>) {
    let mut mine = Vec::new();
    let mut uncovered = Vec::new();
    for r in reqs {
        if matches!(r["behaviour"].as_str(), Some("constraint" | "dropped")) {
            continue;
        }
        let cites = traces.get(r["id"].as_str().unwrap_or("")).cloned().unwrap_or_else(|| r["spec_domains"].clone());
        let e = json!({"id": r["id"], "text": r["text"], "cites": cites});
        if delivered_by(r, traces, commands, cov) {
            mine.push(e);
        } else if !all_cov.iter().any(|c| delivered_by(r, traces, commands, c)) {
            uncovered.push(e);
        }
    }
    (mine, uncovered)
}

/// SDC_REVIEW_MODE: `session` (unset or empty, the default: main's review, a session with read
/// tools over the whole change tree) or `facts` (story:review-minimal-facts: one request on one
/// document of facts). True for facts.
pub fn review_mode(v: Option<&str>) -> Result<bool, String> {
    match v.unwrap_or("") {
        "" | "session" => Ok(false),
        "facts" => Ok(true),
        m => Err(format!("SDC_REVIEW_MODE={m}: neither session nor facts")),
    }
}

/// Copy a tree without `target`, `.sdc` and `.derived-from`.
pub fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)?.flatten() {
        let name = e.file_name();
        if name == "target" || name == ".sdc" || name == ".derived-from" {
            continue;
        }
        let (s, d) = (e.path(), to.join(&name));
        if s.is_dir() { copy_tree(&s, &d)? } else { fs::copy(&s, &d).map(|_| ())? }
    }
    Ok(())
}

/// A story's scope in the case's `work/`: its file, its acceptance lines, the commands it covers,
/// the requirements it delivers (`mine`) and those no story of the plan covers.
pub struct Scope {
    /// The story file's path under the work directory.
    pub rel: String,
    /// The story file's text.
    pub text: String,
    /// The commands the story covers.
    pub cov: Vec<String>,
    /// The acceptance lines, each `{cite, text}`.
    pub acceptance: Vec<Value>,
    /// The requirements the story delivers.
    pub mine: Vec<Value>,
    /// The requirements no story of the plan covers.
    pub uncovered: Vec<Value>,
    /// The requirement traces of `spec/traces.json`, or null.
    pub traces: Value,
    /// The compiled specification of `.sdc/gate/ir.json`, or null.
    pub ir: Value,
}

/// The scope of a story in the work directory `work`.
pub fn scope_of(work: &Path, story: &str) -> Result<Scope, String> {
    let rel = format!("aep/story/{story}.md");
    let text = fs::read_to_string(work.join(&rel)).map_err(|e| format!("{rel}: {e}"))?;
    let (sl, summary) = story_summary(&text).ok_or(format!("{rel} has no summary"))?;
    let cov = covers(&summary);
    let mut acceptance = vec![json!({"cite": format!("{rel}:{sl}"), "text": summary})];
    let mut in_acc = false;
    for (i, l) in text.lines().enumerate() {
        if l.starts_with("## ") { in_acc = l.trim() == "## Acceptance"; continue }
        if in_acc && !l.trim().is_empty() { acceptance.push(json!({"cite": format!("{rel}:{}", i + 1), "text": l.trim()})) }
    }
    let mut all_cov = Vec::new();
    for f in walk(&work.join("aep/story")) {
        if let Some((_, s)) = fs::read_to_string(work.join("aep/story").join(&f)).ok().as_deref().and_then(story_summary) {
            all_cov.push(covers(&s));
        }
    }
    let reqs = read_data(&work.join("requirements.yaml")).map(|v| v["requirements"].as_array().cloned().unwrap_or_default()).unwrap_or_default();
    let traces = read_json(&work.join("spec/traces.json"))["traces"].clone();
    let ir = read_json(&work.join(".sdc/gate/ir.json"));
    let (mine, uncovered) = review_scope(&reqs, &traces, &ir["commands"], &cov, &all_cov);
    Ok(Scope { rel, text, cov, acceptance, mine, uncovered, traces, ir })
}

/// The log of the conformance run on the change revision under review, and the gate's log on the
/// specification revision (None without one). sdc.story (revision 7) admits change.review only once
/// the conformance run passed at the revision under review, so its log is always there; without it
/// the review has no input.
pub fn run_logs(facts: &Facts) -> Result<(String, Option<String>), String> {
    let tests = facts.record_log("conformance_run", "change").ok_or(format!(
        "no conformance run recorded on change revision {} (sdc.story reviews only after tests.pass)", facts.revision("change")))?;
    Ok((tests, facts.record_log("ess_gate", "spec")))
}

/// The session review's inputs (SDC_REVIEW_MODE=session, the default), as before
/// story:review-minimal-facts: written into `<cwd>/in/` and returned. The story, its
/// acceptance lines (the summary line and the lines of an `## Acceptance` section), the
/// requirements it delivers and those no story covers, the covered commands' IR entries, the
/// change tree, its diff against the mainline (empty for the first story), the gate and test logs.
pub fn review_inputs_session(cwd: &Path, facts: &Facts, story: &str) -> Result<Value, String> {
    let work = facts.work();
    let in_dir = cwd.join("in");
    let Scope { rel, text, cov, acceptance, mine, uncovered, traces, ir } = scope_of(&work, story)?;
    fs::create_dir_all(in_dir.join("aep/story")).map_err(|e| e.to_string())?;
    fs::write(in_dir.join(&rel), &text).map_err(|e| e.to_string())?;
    let commands: Map<String, Value> = cov.iter().map(|c| (c.clone(), ir["commands"][c].clone())).collect();
    let w = |name: &str, v: &Value| fs::write(in_dir.join(name), pretty(v)).map_err(|e| e.to_string());
    w("scope.json", &json!({"story": story, "covers": cov, "acceptance": acceptance, "requirements": mine,
        "traced_by": if traces.is_null() { "spec_domains (no spec/traces.json)" } else { "spec/traces.json" }}))?;
    w("uncovered.json", &json!({"about": "requirements no story of the plan covers: outside this story", "requirements": uncovered}))?;
    w("commands.json", &Value::Object(commands))?;
    copy_tree(&work.join("impl"), &in_dir.join("impl")).map_err(|e| format!("impl: {e}"))?;
    // The implementer's coverage document, read-only: where it says each requirement is built.
    let coverage = work.join("impl").join(COVERAGE);
    let has_coverage = coverage.is_file();
    if has_coverage {
        fs::copy(&coverage, in_dir.join("coverage.json")).map_err(|e| format!("coverage: {e}"))?;
    }
    let base = work.join("mainline");
    let empty = cwd.join("base-empty");
    let base = if base.is_dir() { base } else { fs::create_dir_all(&empty).map_err(|e| e.to_string())?; empty };
    let out = std::process::Command::new("diff").args(["-ruN", "-x", "target", "-x", ".sdc", "-x", ".derived-from"]).arg(&base).arg(work.join("impl"))
        .output().map_err(|e| format!("diff: {e}"))?;
    let (b, a) = (format!("{}/", base.display()), format!("{}/", work.join("impl").display()));
    let diff: String = String::from_utf8_lossy(&out.stdout).lines().map(|l| {
        let l = if l.starts_with("diff ") || l.starts_with("--- ") || l.starts_with("+++ ") { l.replace(&b, "mainline/").replace(&a, "impl/") } else { l.to_string() };
        l + "\n"
    }).collect();
    fs::write(in_dir.join("change.diff"), &diff).map_err(|e| e.to_string())?;
    let (tests, gate) = run_logs(facts)?;
    fs::write(in_dir.join("tests.txt"), &tests).map_err(|e| e.to_string())?;
    fs::write(in_dir.join("gate.txt"), gate.clone().unwrap_or("no gate run recorded on this specification revision\n".into()))
        .map_err(|e| e.to_string())?;
    let changed: Vec<&str> = diff.lines().filter_map(|l| l.strip_prefix("+++ impl/")).map(|l| l.split('\t').next().unwrap_or(l)).collect();
    let mut out = json!({"story": story, "covers": cov, "requirements": mine.iter().map(|r| r["id"].clone()).collect::<Vec<_>>(),
        "uncovered": uncovered.iter().map(|r| r["id"].clone()).collect::<Vec<_>>(), "changed_files": changed,
        "gate": gate.is_some(), "base": if base.ends_with("mainline") { "mainline" } else { "empty" }});
    // `coverage` only when the tree carries the document (SDC_IMPLEMENT_COVERAGE=on): without it
    // the output is main's.
    if has_coverage {
        out["coverage"] = json!(true);
    }
    Ok(sorted(out))
}

/// Write the review's inputs into `<cwd>/in/` and return what was written. `review.md` is the
/// document the model reads (`review_document`): the requirements the story delivers and the
/// commands it covers, its acceptance lines, the test summary, the diff against the mainline (empty
/// tree for the first story) and the full text of only the files the diff touches, plus the
/// build constraints when the tree has them, and a listing of every other product file (path, size,
/// top-level items; `listing_section`) so that what the tree lacks can be seen. Beside it, for the
/// programs only: `scope.json` and `uncovered.json` (what a finding may name), `change.diff` and the
/// touched files under `impl/` (the lines `at` may cite), `listing.json` (the files `at` may cite by
/// path alone). No other file's text, no other story, no log.
pub fn review_inputs(cwd: &Path, facts: &Facts, story: &str) -> Result<Value, String> {
    let work = facts.work();
    let in_dir = cwd.join("in");
    let rel = format!("aep/story/{story}.md");
    let text = fs::read_to_string(work.join(&rel)).map_err(|e| format!("{rel}: {e}"))?;
    fs::create_dir_all(&in_dir).map_err(|e| e.to_string())?;
    let (sl, summary) = story_summary(&text).ok_or(format!("{rel} has no summary"))?;
    let cov = covers(&summary);
    let mut acceptance = vec![json!({"cite": format!("{rel}:{sl}"), "text": summary})];
    let mut in_acc = false;
    for (i, l) in text.lines().enumerate() {
        if l.starts_with("## ") { in_acc = l.trim() == "## Acceptance"; continue }
        if in_acc && !l.trim().is_empty() { acceptance.push(json!({"cite": format!("{rel}:{}", i + 1), "text": l.trim()})) }
    }
    let mut all_cov = Vec::new();
    for f in walk(&work.join("aep/story")) {
        if let Some((_, s)) = fs::read_to_string(work.join("aep/story").join(&f)).ok().as_deref().and_then(story_summary) {
            all_cov.push(covers(&s));
        }
    }
    let reqs = read_data(&work.join("requirements.yaml")).map(|v| v["requirements"].as_array().cloned().unwrap_or_default()).unwrap_or_default();
    let traces = read_json(&work.join("spec/traces.json"))["traces"].clone();
    let ir = read_json(&work.join(".sdc/gate/ir.json"));
    let (mine, uncovered) = review_scope(&reqs, &traces, &ir["commands"], &cov, &all_cov);
    let mut by_name = cov.clone();
    by_name.sort();
    let commands: Map<String, Value> = by_name.iter().map(|c| (c.clone(), ir["commands"][c].clone())).collect();
    let w = |name: &str, v: &Value| fs::write(in_dir.join(name), pretty(v)).map_err(|e| e.to_string());
    w("scope.json", &json!({"story": story, "covers": cov, "acceptance": acceptance, "requirements": mine,
        "traced_by": if traces.is_null() { "spec_domains (no spec/traces.json)" } else { "spec/traces.json" }}))?;
    w("uncovered.json", &json!({"about": "requirements no story of the plan covers: outside this story", "requirements": uncovered}))?;
    let base = work.join("mainline");
    let empty = cwd.join("base-empty");
    let base = if base.is_dir() { base } else { fs::create_dir_all(&empty).map_err(|e| e.to_string())?; empty };
    let out = std::process::Command::new("diff").args(["-ruN", "-x", "target", "-x", ".sdc", "-x", ".derived-from"]).arg(&base).arg(work.join("impl"))
        .output().map_err(|e| format!("diff: {e}"))?;
    let (b, a) = (format!("{}/", base.display()), format!("{}/", work.join("impl").display()));
    let diff: String = String::from_utf8_lossy(&out.stdout).lines().map(|l| {
        let l = if l.starts_with("diff ") || l.starts_with("--- ") || l.starts_with("+++ ") { l.replace(&b, "mainline/").replace(&a, "impl/") } else { l.to_string() };
        l + "\n"
    }).collect();
    fs::write(in_dir.join("change.diff"), &diff).map_err(|e| e.to_string())?;
    let changed = changed_files(&diff);
    let mut files: Vec<(String, Option<String>)> = Vec::new();
    for p in &changed {
        let src = work.join("impl").join(p);
        if p.contains("..") || !src.is_file() {
            files.push((p.clone(), None));
            continue;
        }
        let dst = in_dir.join("impl").join(p);
        fs::create_dir_all(dst.parent().unwrap_or(&in_dir)).map_err(|e| e.to_string())?;
        fs::copy(&src, &dst).map_err(|e| format!("impl/{p}: {e}"))?;
        files.push((p.clone(), Some(String::from_utf8_lossy(&fs::read(&src).map_err(|e| e.to_string())?).into_owned())));
    }
    let listing = product_files(&work.join("impl"), &changed);
    w("listing.json", &json!({"about": "product files the diff does not touch: what `at` may cite by path alone",
        "files": listing.iter().map(|f| json!({"path": f.path, "bytes": f.bytes, "items": f.items})).collect::<Vec<_>>()}))?;
    let (tests, gate) = run_logs(facts)?;
    let tests = test_summary(&tests);
    let constraints = fs::read_to_string(work.join("build-constraints.yaml")).ok();
    let doc = review_document(&ReviewFacts { story, base: if base.ends_with("mainline") { "mainline" } else { "empty" },
        acceptance: &acceptance, requirements: &mine, uncovered: &uncovered, commands: &commands,
        constraints: constraints.as_deref(), tests: &tests, diff: &diff, files: &files, listing: &listing });
    fs::write(in_dir.join("review.md"), &doc).map_err(|e| e.to_string())?;
    Ok(sorted(json!({"story": story, "covers": cov, "requirements": mine.iter().map(|r| r["id"].clone()).collect::<Vec<_>>(),
        "uncovered": uncovered.iter().map(|r| r["id"].clone()).collect::<Vec<_>>(), "changed_files": changed,
        "listed_files": listing.len(), "gate": gate.is_some(), "base": if base.ends_with("mainline") { "mainline" } else { "empty" },
        "build_constraints": constraints.is_some(), "document": "in/review.md", "document_bytes": doc.len()})))
}

/// The review reply checked by program. Ok((approved, rejection or note)), or Err(reasons) when
/// the reply is invalid: an unknown decision, a finding whose place is not a line of the change
/// tree, a violation of something outside the story's set, an `outside` finding about a
/// requirement a story covers, an empty `says`, or a rejection with no violation finding.
pub fn check_review(cwd: &Path, source: &Facts, reply: &Value, principal: &str) -> Result<(bool, Value), Vec<String>> {
    let c = ctx(cwd, source);
    let (inside, outside) = review_targets(&c.in_dir);
    let changed: BTreeSet<String> = changed_files(&fs::read_to_string(c.in_dir.join("change.diff")).unwrap_or_default()).into_iter().collect();
    // A facts review (SDC_REVIEW_MODE=facts) has listing.json: a listed file may be cited by path.
    let facts = c.in_dir.join("listing.json").is_file();
    let listed: BTreeSet<String> = read_json(&c.in_dir.join("listing.json"))["files"].as_array().into_iter().flatten()
        .filter_map(|f| f["path"].as_str().map(String::from)).collect();
    let mut why = Vec::new();
    let decision = reply["decision"].as_str().unwrap_or("");
    if decision != "approved" && decision != "rejected" {
        why.push(format!("decision {:?} is neither approved nor rejected", reply["decision"]));
    }
    let mut reasons = Vec::new();
    let mut notes = Vec::new();
    let mut citations = Vec::new();
    for (i, f) in reply["findings"].as_array().into_iter().flatten().enumerate() {
        let n = i + 1;
        let (kind, at, v, says) = (f["kind"].as_str().unwrap_or(""), f["at"].as_str().unwrap_or(""),
            f["violates"].as_str().unwrap_or(""), f["says"].as_str().unwrap_or("").trim());
        // A file by path alone: a product file the listing names, or a changed file.
        let by_path = at.strip_prefix("impl/").filter(|p| facts && !p.contains(':'));
        let r = if by_path.is_some() { json!({"ok": true}) } else { resolve(&c, at) };
        if let Some(p) = by_path {
            if p.contains("..") || !(listed.contains(p) || changed.contains(p)) {
                why.push(format!("finding {n}: {at:?} is neither a listed product file nor a changed file (cite a listed file by path, or <path>:<line> of a changed file)"));
            }
        } else if !at.starts_with("impl/") || r["ok"] != true {
            why.push(format!("finding {n}: {at:?} is not a line of the change tree ({})", r["why"].as_str().unwrap_or("not under impl/")));
        }
        match kind {
            "violation" if !inside.iter().any(|x| x == v) =>
                why.push(format!("finding {n}: {v:?} is not a requirement, acceptance line or command of this story ({})", inside.join(", "))),
            "outside" if !outside.iter().any(|x| x == v) =>
                why.push(format!("finding {n}: {v:?} is not a requirement no story covers ({})", if outside.is_empty() { "none".into() } else { outside.join(", ") })),
            "violation" | "outside" => {}
            _ => why.push(format!("finding {n}: kind {kind:?} is neither violation nor outside")),
        }
        if says.is_empty() {
            why.push(format!("finding {n}: says nothing"));
        }
        let rest = at.strip_prefix("impl/").unwrap_or("");
        let file = rest.rsplit_once(':').map_or(rest, |(p, _)| p);
        let line = format!("{at}: {says} ({} {v})", if kind == "outside" { "outside this story:" } else { "violates" });
        if kind == "outside" { notes.push(line) } else { reasons.push(line) }
        let mut citation = json!({"at": at, "violates": v, "kind": kind, "in_diff": changed.contains(file)});
        if facts {
            citation["listed"] = json!(listed.contains(file));
        }
        citations.push(citation);
    }
    if decision == "rejected" && reasons.is_empty() {
        why.push("a rejection names no violation finding: reject only for a requirement, acceptance line or command of this story".into());
    }
    if !why.is_empty() {
        return Err(why);
    }
    if decision == "approved" {
        return Ok((true, sorted(json!({"approved": true, "principal": principal, "notes": notes, "citations": citations}))));
    }
    let count = source.rejections("change.review").as_array().map_or(0, |a| a.len()) + 1;
    let rejection = json!({"format": "sdc.rejection/1", "approval": "change.review", "case": source.case_id(), "subject": "change",
        "revision": source.revision_value("change"), "principal": principal, "reasons": reasons,
        "notes": notes, "citations": citations, "count": count, "summary": reply["summary"]});
    source.note_rejection("change.review", &rejection).map_err(|e| vec![format!("the rejection could not be kept: {e}")])?;
    Ok((false, sorted(rejection)))
}
