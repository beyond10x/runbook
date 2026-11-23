//! What leaves the engine toward a client names no path of the daemon's state directory and no path of
//! the runbook home. A reason, a run-log record or a question is written by code that is not the
//! engine's (Loom, the YAML reader, the operating system) and names the file it read by the path it
//! read it at; this is the one place that takes those paths out: an entry of the run's copy, of the
//! run's directory or of the home is named relative to it, and a directory itself is named `.`.

use crate::runtime::{DecisionRequest, EventSink};
use serde_json::Value;
use std::path::Path;

/// The directories whose paths a client never reads, longest first.
pub struct Hide {
    spots: Vec<String>,
}

impl Hide {
    /// The paths of `dirs`, as given and as the file system names them.
    pub fn new(dirs: &[&Path]) -> Hide {
        let mut spots: Vec<String> = Vec::new();
        for dir in dirs {
            let mut forms = vec![dir.to_path_buf()];
            if let Ok(real) = std::fs::canonicalize(dir) {
                forms.push(real);
            }
            if let Ok(abs) = std::path::absolute(dir) {
                forms.push(abs);
            }
            for form in forms {
                let text = form.to_string_lossy().trim_end_matches('/').to_string();
                if text.len() > 1 && !spots.contains(&text) {
                    spots.push(text);
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
}
