//! The rejection a writer addresses (lines 298-305) and the note a stub writes into the rejected
//! artifact (lines 329-359).

use crate::ordjson::Ov;
use crate::util::chomp;
use crate::{yaml, Ctx};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// `- ` before each line of `text` (`sed 's/^/- /'`), without the trailing newlines.
fn dashed(text: &str) -> String {
    let body = text.strip_suffix('\n').unwrap_or(text);
    if text.is_empty() {
        return String::new();
    }
    chomp(&body.split('\n').map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n")).to_string()
}

impl Ctx {
    /// The records of the feedback file: its entries, none when it cannot be read.
    pub fn feedback_records(&self) -> Vec<Ov> {
        let Some(p) = self.set.feedback.as_ref().filter(|p| p.is_file()) else { return Vec::new() };
        fs::read_to_string(p).ok().and_then(|t| Ov::parse(&t).ok()).map(|d| d.items().into_iter().cloned().collect()).unwrap_or_default()
    }

    /// A rejected approval of this writer's output (`approve --result rejected`): the feedback
    /// names the record, and its log is the rejection (`sdc.rejection/1`). `None` when there is
    /// none, or when its log is not such a document.
    pub fn find_rejection(&self) -> Option<PathBuf> {
        let log = self
            .feedback_records()
            .iter()
            .filter(|r| r.str_of("result") == Some("rejected"))
            .filter_map(|r| r.field("log"))
            .next_back()
            .map(Ov::raw)?;
        let path = PathBuf::from(log);
        let doc = fs::read_to_string(&path).ok().and_then(|t| Ov::parse(&t).ok())?;
        (doc.str_of("format") == Some("sdc.rejection/1")).then_some(path)
    }

    /// The stub addresses a rejection by recording it in the rejected artifact (the approval's
    /// subject, at its `$artifacts` path), so the rework is a new revision: a tree gets `.rework`,
    /// a YAML or Markdown file the same lines as a comment.
    pub fn stub_rework(&self, code: i32) -> crate::util::R<()> {
        let (mut approval, mut subject, mut lines) = (String::new(), String::new(), String::new());
        let file_ok = self.set.feedback.as_ref().is_some_and(|p| p.is_file());
        if self.rejection.is_none() && file_ok {
            // A person's rejection: the record names the person's action and the subject, and its
            // log is the person's reasons as text.
            let recs = self.feedback_records();
            let last = recs.iter().rfind(|r| {
                let some = |k: &str| r.get(k).is_some_and(|v| *v != Ov::Null);
                r.str_of("result") == Some("rejected") && some("action") && some("log")
            });
            let field = |k: &str| last.and_then(|r| r.field(k)).map(Ov::raw).unwrap_or_default();
            approval = field("action");
            subject = field("subject");
            let log = field("log");
            if !approval.is_empty() && PathBuf::from(&log).is_file() {
                let reasons = dashed(&String::from_utf8_lossy(&fs::read(&log).unwrap_or_default()));
                let text = self.render("instructions/shared/rework.stub.person.md", &[("action", &approval), ("reasons", &reasons)])?;
                lines = chomp(&text).to_string();
            }
        } else if let Some(rej) = self.rejection.as_ref() {
            let doc = fs::read_to_string(rej).ok().and_then(|t| Ov::parse(&t).ok()).unwrap_or(Ov::Null);
            let show = |k: &str| doc.get(k).map(Ov::raw).unwrap_or_else(|| "null".to_string());
            approval = doc.field("approval").map(Ov::raw).unwrap_or_default();
            subject = doc.field("subject").map(Ov::raw).unwrap_or_default();
            let reasons = match doc.get("reasons") {
                Some(Ov::Arr(a)) => a.iter().map(|r| format!("- {}", r.raw())).collect::<Vec<_>>().join("\n"),
                _ => String::new(),
            };
            let text = self.render(
                "instructions/shared/rework.stub.md",
                &[("count", &show("count")), ("approval", &show("approval")), ("revision", &show("revision")), ("reasons", &reasons)],
            )?;
            lines = chomp(&text).to_string();
        }
        if approval.is_empty() || subject.is_empty() {
            return Ok(());
        }
        // The subject's path in the bindings file beside these tools that binds the approval.
        let mut found: Vec<String> = Vec::new();
        let mut files: Vec<PathBuf> = fs::read_dir(self.set.here.join(".."))
            .map(|d| d.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "yaml")).collect())
            .unwrap_or_default();
        files.sort();
        for y in files {
            let Some(doc) = yaml::load(&y) else { continue };
            let binds = yaml::get(&doc, "actions")
                .and_then(serde_yaml::Value::as_sequence)
                .is_some_and(|a| a.iter().any(|x| yaml::get(x, "action").and_then(serde_yaml::Value::as_str) == Some(&approval)));
            if !binds {
                continue;
            }
            let path = yaml::get(&doc, "artifacts").and_then(|a| yaml::get(a, &subject)).map(|v| yaml::text(Some(v))).unwrap_or_default();
            if !path.is_empty() && !found.contains(&path) {
                found.push(path);
            }
        }
        let [t] = found.as_slice() else { return Ok(()) };
        let target = self.work.join(t);
        if code != 0 || lines.is_empty() || !target.exists() {
            return Ok(());
        }
        let append = |text: String| {
            if let Ok(mut f) = fs::OpenOptions::new().append(true).open(&target) {
                let _ = f.write_all(text.as_bytes());
            }
        };
        if t.ends_with(".yaml") || t.ends_with(".yml") {
            if !target.is_dir() {
                append(lines.split('\n').map(|l| format!("# {l}\n")).collect());
            }
        } else if t.ends_with(".md") {
            append(format!("\n<!--\n{lines}\n-->\n"));
        }
        if target.is_dir() {
            let _ = fs::write(target.join(".rework"), format!("{lines}\n"));
        }
        Ok(())
    }
}
