//! The text the model is told: the files of `instructions/decider/` with their `{{name}}`
//! placeholders filled, and the way the prompt is put together from them. No prompt text is in
//! this file.

use super::inputs::Inputs;
use serde_json::Value;
use std::path::PathBuf;

/// Reads and fills the instruction files below `root/instructions/decider/`.
pub(super) struct Renderer {
    pub(super) root: PathBuf,
}

/// The first `{{name}}` of `text` (name: lower case letters and `_`): where it starts and ends, and the name.
fn next_placeholder(text: &str) -> Option<(usize, usize, &str)> {
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(offset) = text[from..].find("{{") {
        let start = from + offset;
        let mut end = start + 2;
        while end < bytes.len() && (bytes[end].is_ascii_lowercase() || bytes[end] == b'_') {
            end += 1;
        }
        if end > start + 2 && text[end..].starts_with("}}") {
            return Some((start, end + 2, &text[start + 2..end]));
        }
        from = start + 1;
    }
    None
}

impl Renderer {
    /// The file with every `{{name}}` replaced by its value in one pass (a value is never searched
    /// for placeholders); the trailing newlines of the file are dropped. A file that is missing, or
    /// names a placeholder without a value, is an error.
    pub(super) fn render(&self, file: &str, vars: &[(&str, &str)]) -> Result<String, String> {
        if !file.ends_with(".md") || file.starts_with('/') || file.split('/').any(|part| part == "..") {
            return Err(format!("the instruction path {file} is not <...>.md under instructions/decider/ without a .. segment"));
        }
        let path = self.root.join("instructions").join("decider").join(file);
        // The run's copy of the instruction files is read as every name of the run is: no link followed, no pipe waited for,
        // a regular file with one name of at most 64 MiB, as UTF-8 text. What is not one is refused as a file that is not
        // there, and the message names no path outside the instruction directory.
        let text = crate::loom_run::record_write::read_regular_text(&path).map_err(|_| format!("the instruction file instructions/decider/{file} is missing or is not a regular file of text"))?;
        let mut rest = text.trim_end_matches('\n');
        let mut out = String::new();
        while let Some((start, end, name)) = next_placeholder(rest) {
            let value = vars
                .iter()
                .rev()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| *v)
                .ok_or_else(|| format!("instructions/decider/{file} names {{{{{name}}}}} and no value was given"))?;
            out.push_str(&rest[..start]);
            out.push_str(value);
            rest = &rest[end..];
        }
        out.push_str(rest);
        Ok(out)
    }
}

/// A member as `jq -r` prints it inside `$(...)`: text as it is, null as `null`, anything else as
/// compact JSON; trailing newlines dropped.
pub(super) fn raw(value: &Value) -> String {
    let text = match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    text.trim_end_matches('\n').to_owned()
}

/// `jq`'s `a // b`: `a` unless it is null or false.
pub(super) fn or<'a>(a: &'a Value, b: &'a Value) -> &'a Value {
    if a.is_null() || a == &Value::Bool(false) {
        b
    } else {
        a
    }
}

/// What the prompt of one decision is made of.
pub(super) struct Parts<'a> {
    pub(super) req: &'a Value,
    pub(super) final_escalate: bool,
    pub(super) inputs: &'a Inputs,
    pub(super) citable: &'a Value,
    pub(super) prior_rejections: usize,
    pub(super) has_aep: bool,
    pub(super) met_after: bool,
}

fn example<'a>(citable: &'a Value, name: &str) -> Option<&'a str> {
    citable["examples"][name].as_str().filter(|s| !s.is_empty())
}

/// The prompt: `model.md` filled with the decision's own text, the list of inputs, the notes, what an
/// escalate does, the keys of the reply and the forms a reply may cite.
pub(super) fn prompt(r: &Renderer, p: &Parts<'_>) -> Result<String, String> {
    let req = p.req;
    let kind = req["type"].as_str().unwrap_or("");
    let path = raw(or(&req["path"], &Value::String(String::new())));
    let rule_finding = !req["finding"]["sides"].is_null() || !req["finding"]["trace"].is_null();
    let mut what = match kind {
        "approval" => r.render("approval.md", &[("action", &raw(&req["action"])), ("subject", &raw(&req["subject"])), ("revision", &raw(&req["revision"])), ("path", &path)])?,
        "question" => r.render("question.md", &[])?,
        "finding" => {
            let how = r.render(if rule_finding { "finding-rule.md" } else { "finding-free.md" }, &[])?;
            r.render("finding.md", &[("answer_how", &how)])?
        }
        "close" => r.render("close.md", &[("outcome", &raw(&req["outcome"]))])?,
        other => return Err(format!("the request type {other:?} is not approval, question, finding or close")),
    };
    if p.prior_rejections > 0 {
        what = format!("{what}\n{}", r.render("prior-rejection.md", &[])?);
    }
    if !req["refusal"].is_null() {
        let why = raw(or(&req["refusal"]["why"], &Value::String(String::new())));
        what = format!("{what}\n{}", r.render("refusal.md", &[("why", &why)])?);
    }
    let status = raw(or(&req["question"]["status"], &Value::String("open".to_owned())));
    if kind == "question" && status != "open" {
        what = format!("{what}\n{}", r.render("answered-question.md", &[])?);
    }
    let note = raw(or(&req["note"], &Value::String(String::new())));
    if !note.is_empty() {
        what = format!("{what}\n{}", r.render("asked-again.md", &[("note", &raw(&req["note"]))])?);
    }
    let final_note = if p.final_escalate { format!("{}\n", r.render("final-note.md", &[])?) } else { String::new() };
    let line = r.render("input-decision.md", &[])?;
    let mut listed: Vec<String> = p.inputs.listed.iter().map(|i| format!("- {i}\n")).collect();
    listed.push(format!("- {line}\n"));
    let input_list = listed.concat().trim_end_matches('\n').to_owned();
    let mut notes = String::new();
    if p.met_after {
        notes.push_str(&r.render("met-after.md", &[("action", &raw(&req["action"]))])?);
        notes.push('\n');
    }
    if p.has_aep {
        notes.push_str(&r.render("aep-note.md", &[])?);
        notes.push('\n');
    }
    let escalate = if p.final_escalate {
        let taken = r.render(
            match kind {
                "question" => "taken-question.md",
                "finding" => "taken-finding.md",
                _ => "taken-other.md",
            },
            &[],
        )?;
        r.render("escalate-final.md", &[("taken", &taken)])?
    } else {
        r.render("escalate-stop.md", &[])?
    };
    let reply_keys = if kind == "finding" || kind == "question" {
        r.render(if p.final_escalate && kind == "finding" { "reply-choice-finding-final.md" } else { "reply-choice.md" }, &[])?
    } else if p.final_escalate {
        r.render("reply-other-final.md", &[])?
    } else {
        r.render("reply-other.md", &[])?
    };
    let example_id = match example(p.citable, "id") {
        Some(id) => r.render("citable-example.md", &[("example", id)])?,
        None => String::new(),
    };
    let mut forms = r.render("citable-id.md", &[("example", &example_id)])?;
    if let Some(entry) = example(p.citable, "entry") {
        forms.push('\n');
        forms.push_str(&r.render("citable-entry.md", &[("example", entry)])?);
    }
    if let Some(line) = example(p.citable, "line") {
        forms.push('\n');
        forms.push_str(&r.render("citable-line.md", &[("example", line)])?);
    }
    let one = Value::from(1);
    let attempt = or(&req["retry"]["attempt"], &one);
    let retry = if attempt.as_i64().unwrap_or(1) > 1 {
        let text = r.render("retry.md", &[("attempt", &raw(&req["retry"]["attempt"])), ("reason", &raw(&req["retry"]["invalid_reason"]))])?;
        format!("\n{text}\n")
    } else {
        String::new()
    };
    let text = r.render(
        "model.md",
        &[("final_note", &final_note), ("inputs", &input_list), ("what", &what), ("notes", &notes), ("escalate", &escalate), ("reply_keys", &reply_keys), ("citable", &forms), ("retry", &retry)],
    )?;
    Ok(format!("{text}\n"))
}
