//! What leaves the engine toward a client names no path of the daemon's state directory and no path of
//! the runbook home. A reason, a run-log record or a question is written by code that is not the
//! engine's (Loom, the YAML reader, the operating system) and names the file it read by the path it
//! read it at; this is the one place that takes those paths out: an entry of the run's copy, of the
//! run's directory or of the home is named relative to it, and a directory itself is named `.`.

use crate::runtime::{DecisionRequest, EventSink};
use super::clean;
use serde_json::Value;
use std::path::Path;

/// The directories whose paths a client never reads, longest first.
pub struct Hide {
    spots: Vec<String>,
}

impl Hide {
    /// The paths of `dirs`, as given (only where absolute: a relative word is no spot, the absolute and
    /// the canonical forms cover it) and as the file system names them, each in every way a text can
    /// spell it by the time it is read here: as it is, with a control character read as a space (the
    /// engine does that to a failure before it is hidden), and as Debug escapes it (Loom names a refused
    /// file that way).
    pub fn new(dirs: &[&Path]) -> Hide {
        let mut spots: Vec<String> = Vec::new();
        for dir in dirs {
            let mut forms = Vec::new();
            if dir.is_absolute() {
                forms.push(dir.to_path_buf());
            }
            if let Ok(real) = std::fs::canonicalize(dir) {
                forms.push(real);
            }
            if let Ok(abs) = std::path::absolute(dir) {
                forms.push(abs);
            }
            for form in forms {
                for text in spellings(&form) {
                    let text = text.trim_end_matches('/').to_string();
                    if text.len() > 1 && !spots.contains(&text) {
                        spots.push(text);
                    }
                }
            }
        }
        spots.sort_by_key(|s| std::cmp::Reverse(s.len()));
        Hide { spots }
    }

    /// `text` without the paths: `<dir>/entry` becomes `entry`, `<dir>` alone becomes `.`.
    pub fn text(&self, text: &str) -> String {
        let mut out = text.to_string();
        for spot in &self.spots {
            let mut rest = String::with_capacity(out.len());
            let mut from = 0;
            while let Some(at) = out[from..].find(spot.as_str()) {
                let at = from + at;
                let end = at + spot.len();
                rest.push_str(&out[from..at]);
                let after = out[end..].chars().next();
                let before = out[..at].chars().next_back();
                if before.is_some_and(is_path_char) {
                    rest.push_str(spot);
                    from = end;
                } else if after == Some('/') {
                    from = end + 1;
                } else if after.is_some_and(is_path_char) {
                    rest.push_str(spot);
                    from = end;
                } else {
                    rest.push('.');
                    from = end;
                }
            }
            rest.push_str(&out[from..]);
            out = rest;
        }
        out
    }

    /// `value` with every text in it taken through `text`.
    pub fn value(&self, value: Value) -> Value {
        match value {
            Value::String(s) => Value::String(self.text(&s)),
            Value::Array(items) => Value::Array(items.into_iter().map(|v| self.value(v)).collect()),
            Value::Object(map) => Value::Object(map.into_iter().map(|(k, v)| (k, self.value(v))).collect()),
            other => other,
        }
    }
}

/// The texts that spell `path`: as the file system names it (lossy where it is not UTF-8), with each
/// control character replaced by a space, as `Debug` escapes it without its quotes, and each of those
/// read as a failure text is (control characters replaced).
fn spellings(path: &Path) -> Vec<String> {
    fn escaped(text: &str) -> String {
        let quoted = format!("{text:?}");
        quoted[1..quoted.len() - 1].to_string()
    }
    let lossy = path.to_string_lossy().into_owned();
    let cleaned = clean(&lossy);
    let debug_of_path = format!("{path:?}");
    let mut texts = vec![lossy.clone(), cleaned.clone(), escaped(&lossy), escaped(&cleaned), debug_of_path[1..debug_of_path.len() - 1].to_string()];
    let cleaned_after: Vec<String> = texts.iter().map(|t| clean(t)).collect();
    texts.extend(cleaned_after);
    texts
}

fn is_path_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '-' | '_' | '.')
}

/// The sink the runtime reports through, with every record and question taken through [`Hide`].
pub struct HidingSink<'a> {
    pub inner: &'a dyn EventSink,
    pub hide: &'a Hide,
}

impl EventSink for HidingSink<'_> {
    fn started(&self) {
        self.inner.started()
    }

    fn record(&self, kind: &str, record: Value) {
        self.inner.record(kind, self.hide.value(record))
    }

    fn decision_requested(&self, request: DecisionRequest) {
        let hide = self.hide;
        self.inner.decision_requested(DecisionRequest {
            question: hide.text(&request.question),
            options: request.options.into_iter().map(|o| hide.value(o)).collect(),
            ..request
        })
    }

    fn resumed(&self) {
        self.inner.resumed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStrExt;
    use std::path::PathBuf;

    #[test]
    fn a_path_below_a_directory_is_named_relative_to_it_and_the_directory_itself_is_a_dot() {
        let hide = Hide::new(&[&PathBuf::from("/s/runs/r1/runbook"), &PathBuf::from("/s/runs/r1"), &PathBuf::from("/s"), &PathBuf::from("/h/home")]);
        assert_eq!(hide.text("/s/runs/r1/runbook/bindings/a.yaml: bad at line 2"), "bindings/a.yaml: bad at line 2");
        assert_eq!(hide.text("file \"/s/runs/r1/protocol/x\" and /s/workspaces/r1/out"), "file \"protocol/x\" and workspaces/r1/out");
        assert_eq!(hide.text("/h/home/bindings/tools/leak.sh: a link"), "bindings/tools/leak.sh: a link");
        assert_eq!(hide.text("in /s/runs/r1: no"), "in .: no");
        assert_eq!(hide.text("/s-other/x and /sx and /h/homework/y"), "/s-other/x and /sx and /h/homework/y");
        assert_eq!(hide.text("no path here"), "no path here");
        let record = hide.value(serde_json::json!({"a": ["/h/home/b/c", 3], "b": {"c": "/s/runs/r1/x"}}));
        assert_eq!(record, serde_json::json!({"a": ["b/c", 3], "b": {"c": "x"}}));
    }

    #[derive(Default)]
    struct Catch {
        records: std::sync::Mutex<Vec<(String, Value)>>,
        requests: std::sync::Mutex<Vec<DecisionRequest>>,
    }

    impl EventSink for Catch {
        fn started(&self) {}
        fn record(&self, kind: &str, record: Value) {
            self.records.lock().unwrap().push((kind.to_string(), record));
        }
        fn decision_requested(&self, request: DecisionRequest) {
            self.requests.lock().unwrap().push(request);
        }
        fn resumed(&self) {}
    }

    #[test]
    fn a_record_and_the_options_of_a_question_reach_the_sink_without_the_directory() {
        let hide = Hide::new(&[&PathBuf::from("/s/runs/r1"), &PathBuf::from("/h/home")]);
        let catch = Catch::default();
        let sink = HidingSink { inner: &catch, hide: &hide };
        sink.record("note", serde_json::json!({"file": "/s/runs/r1/protocol/x.yaml", "list": ["/h/home/bindings/a"], "n": 2}));
        sink.decision_requested(DecisionRequest {
            request_id: "q1".into(),
            case_id: "c".into(),
            action: "act".into(),
            capability: "approve".into(),
            question: "is /s/runs/r1/out right".into(),
            options: vec![serde_json::json!({"answer": "approve", "label": "keep /h/home/bindings/a"}), serde_json::json!("/s/runs/r1")],
        });
        assert_eq!(*catch.records.lock().unwrap(), vec![("note".to_string(), serde_json::json!({"file": "protocol/x.yaml", "list": ["bindings/a"], "n": 2}))]);
        let requests = catch.requests.lock().unwrap();
        assert_eq!(requests[0].question, "is out right");
        assert_eq!(requests[0].options, vec![serde_json::json!({"answer": "approve", "label": "keep bindings/a"}), serde_json::json!(".")]);
        assert_eq!((requests[0].request_id.as_str(), requests[0].action.as_str()), ("q1", "act"));
    }

    #[test]
    fn a_directory_whose_name_has_a_control_character_a_quote_or_a_mark_is_hidden_as_every_step_spells_it() {
        let dir = PathBuf::from("/s/st\tate\"\\/x\u{301}\n");
        let hide = Hide::new(&[&dir]);
        assert_eq!(hide.text("read /s/st\tate\"\\/x\u{301}\n/runs/r/f"), "read runs/r/f");
        assert_eq!(hide.text("read /s/st ate\"\\/x\u{301} /runs/r/f"), "read runs/r/f");
        assert_eq!(hide.text("file \"/s/st\\tate\\\"\\\\/x\\u{301}\\n/runs/r/f\": bad"), "file \"runs/r/f\": bad");
        let bytes = PathBuf::from(std::ffi::OsStr::from_bytes(b"/s/st\xFFate"));
        let hide = Hide::new(&[&bytes]);
        assert_eq!(hide.text("a /s/st\u{FFFD}ate/f and \"/s/st\\xFFate/g\""), "a f and \"g\"");
    }

    #[test]
    fn a_linked_directory_and_one_given_with_a_slash_is_hidden_in_each_spelling() {
        let base = std::env::current_exe().unwrap().parent().unwrap().join(format!("hide-links-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let real = base.join("real");
        std::fs::create_dir_all(real.join("runs")).unwrap();
        let link = base.join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let given = PathBuf::from(format!("{}/", link.display()));
        let hide = Hide::new(&[&given]);
        let token = base.file_name().unwrap().to_str().unwrap().to_string();
        let texts = [
            format!("{}/runs/f", real.display()),
            format!("{}/runs/f", link.display()),
            format!("{}//runs/f", link.display()),
            format!("{}/./runs/f", link.display()),
            format!("{}/runs/../runs/f", link.display()),
            format!("{}/", real.display()),
            format!("in {}: no", real.display()),
            format!("file://{}/runs/f", real.display()),
            format!("{:?}", real.join("runs/f")),
            format!("{:?}", link.join("runs/f").display().to_string()),
        ];
        let kept: Vec<String> = texts.iter().filter_map(|t| hide.text(t).contains(&token).then(|| format!("{t:?} -> {:?}", hide.text(t)))).collect();
        let _ = std::fs::remove_dir_all(&base);
        assert!(kept.is_empty(), "{} texts keep the path:\n{}", kept.len(), kept.join("\n"));
    }

    #[test]
    fn a_directory_given_as_a_relative_word_rewrites_no_word_of_a_reason() {
        let hide = Hide::new(&[&PathBuf::from("home"), &PathBuf::from("state")]);
        assert_eq!(hide.text("a link that leaves the runbook home in the state of things"), "a link that leaves the runbook home in the state of things");
    }
}
