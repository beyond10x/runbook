//! `runbook list`: the protocols of every runbook directory (`runbook.toml`'s `runbooks`, then the
//! built-in set), each with its name, kind, version, directory and title. What is read, and which
//! protocol can be started, is `runbook::runbooks`.

use runbook::runbooks::Entry;
use serde_json::{json, Value};

fn version(e: &Entry) -> String {
    e.revision.map(|r| r.to_string()).unwrap_or_else(|| "-".to_owned())
}

pub fn text(entries: &[Entry]) -> String {
    let w = entries.iter().map(|e| e.id.len()).max().unwrap_or(0);
    let wv = entries.iter().map(|e| version(e).len()).max().unwrap_or(0);
    let mut s = String::new();
    for e in entries {
        let kind = if e.startable { "runbook" } else { "child" };
        s.push_str(&format!("{:<w$}  {:<7}  {:<wv$}  {}  {}\n", e.id, kind, version(e), e.dir.display(), e.title));
    }
    s
}

pub fn json(entries: &[Entry]) -> Value {
    Value::Array(
        entries
            .iter()
            .map(|e| {
                json!({
                    "id": e.id,
                    "protocol_id": e.protocol_id,
                    "revision": e.revision,
                    "version": e.revision,
                    "kind": if e.startable { "runbook" } else { "child" },
                    "startable": e.startable,
                    "directory": e.dir,
                    "built_in": e.built_in,
                    "title": e.title,
                })
            })
            .collect(),
    )
}
