//! Where things are in a run directory, by relative path components.
//!
//! Two kinds of run directory (`Kind`, as `driver/src/run_kind.rs` tells them apart):
//!
//! - run of the removed driver:
//!   - case store: `store/<C>` and `store/<C>(/children/<C>)*`; its log is `events.jsonl`
//!   - work root: `<case store>/work` (a child's `work` is a symlink to its parent's and is never
//!     walked, so only the tree root's real `work/` is a root)
//!   - agent call record: `llm/<NNN-action>`, `decider/<NNN-id>`, or `.sdc/llm/<NNN-action>` (the
//!     default record place under work/); its stream is `events.jsonl`, its root `cwd/`
//! - Loom run (`runbook run`; `settings.json` marks it):
//!   - work root: `work/`, the product tree of the whole run. `store/` is Loom's own case store and
//!     the projection of it; neither is a root, and the projection's logs are not read.
//!   - agent call record: `work/.sdc/llm/<NNN-action>` and `decider/<NNN-id>`; its root is `cwd/`.
//!     The record's own files (`request.json`, `status.json`, `response.json`, ...) and
//!     `work/.sdc/llm/calls.jsonl` are the runner's transport, not product: never a root.
//!   - steps: `rounds.jsonl`, one line per finished `tree run` step.

use std::sync::OnceLock;

/// The kind of run directory the process watches, set once at start; a driver run when never set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Driver,
    Loom,
}

static KIND: OnceLock<Kind> = OnceLock::new();

/// Fixes the kind for this process. The first call wins.
pub fn set_kind(k: Kind) {
    let _ = KIND.set(k);
}

pub fn kind() -> Kind {
    KIND.get().copied().unwrap_or(Kind::Driver)
}

/// The file that marks a Loom run directory.
pub const LOOM_MARK: &str = "settings.json";

/// Never watched, at any depth.
pub const EXCLUDED_ANYWHERE: &[&str] = &["target", ".git", "node_modules", "transcripts"];
/// Not watched outside a root (the case store's own records).
pub const EXCLUDED_IN_STORE: &[&str] = &["evidence", "blobs", "evaluations"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RootKind {
    /// A case tree's `work/`; `tree` is the case store dir that owns it (relative path).
    Work { tree: String },
    /// A Loom run's `work/`, the product tree of the whole run.
    Loom,
    /// An agent call's `cwd/`; `record` is the record dir (relative path).
    Record { record: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Root {
    pub rel: String,
    pub kind: RootKind,
}

pub fn join(c: &[&str]) -> String {
    c.join("/")
}

pub fn is_case_dir(c: &[&str]) -> bool {
    c.len() >= 2 && c[0] == "store" && c.len().is_multiple_of(2) && (2..c.len()).step_by(2).all(|i| c[i] == "children")
}

pub fn is_record_dir(c: &[&str]) -> bool {
    let n = c.len();
    if n < 2 || c[n - 1].is_empty() {
        return false;
    }
    match (n, c[n - 2]) {
        (2, "llm") | (2, "decider") => true,
        (_, "llm") => n >= 3 && c[n - 3] == ".sdc",
        _ => false,
    }
}

pub fn record_kind_is_decider(c: &[&str]) -> bool {
    c.len() == 2 && c[0] == "decider"
}

/// `015-change.implement` → `change.implement`; a name without a number prefix stays as is.
pub fn record_action(name: &str) -> String {
    match name.split_once('-') {
        Some((n, rest)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => rest.to_string(),
        _ => name.to_string(),
    }
}

fn root_at(kind: Kind, c: &[&str]) -> Option<Root> {
    let n = c.len();
    if n >= 2 && c[n - 1] == "cwd" && is_record_dir(&c[..n - 1]) {
        return Some(Root { rel: join(c), kind: RootKind::Record { record: join(&c[..n - 1]) } });
    }
    match kind {
        Kind::Driver if n >= 3 && c[n - 1] == "work" && is_case_dir(&c[..n - 1]) => {
            Some(Root { rel: join(c), kind: RootKind::Work { tree: join(&c[..n - 1]) } })
        }
        Kind::Loom if n == 1 && c[0] == "work" => Some(Root { rel: join(c), kind: RootKind::Loom }),
        _ => None,
    }
}

/// In a Loom run: whether `c` is in `work/.sdc/llm/` outside the `cwd/` of a call record, that
/// is a record's own file or the call log.
fn in_call_record_outside_cwd(c: &[&str]) -> bool {
    let Some(i) = (0..c.len().saturating_sub(2)).find(|&i| c[i] == "work" && c[i + 1] == ".sdc" && c[i + 2] == "llm") else {
        return false;
    };
    c.get(i + 4) != Some(&"cwd") && c.len() > i + 3
}

/// The innermost root containing the entry at `c` (a file, or a dir that may be a root itself).
pub fn root_of(c: &[&str], is_dir: bool) -> Option<Root> {
    root_of_in(kind(), c, is_dir)
}

pub fn root_of_in(kind: Kind, c: &[&str], is_dir: bool) -> Option<Root> {
    if kind == Kind::Loom && in_call_record_outside_cwd(c) {
        return None;
    }
    let top = if is_dir { c.len() } else { c.len().saturating_sub(1) };
    (1..=top).rev().find_map(|i| root_at(kind, &c[..i]))
}

/// Not watched and never recorded.
pub fn is_excluded(c: &[&str], out_rel: Option<&str>) -> bool {
    if let Some(o) = out_rel {
        let s = join(c);
        if s == o || s.starts_with(&format!("{o}/")) {
            return true;
        }
    }
    if c.iter().any(|x| EXCLUDED_ANYWHERE.contains(x)) {
        return true;
    }
    root_of(c, true).is_none() && c.iter().any(|x| EXCLUDED_IN_STORE.contains(x))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(p: &str) -> Vec<&str> {
        p.split('/').collect()
    }

    #[test]
    fn classifies() {
        assert!(is_case_dir(&s("store/IDEA-1")));
        assert!(is_case_dir(&s("store/IDEA-1/children/IDEA-1.todolist")));
        assert!(!is_case_dir(&s("store/IDEA-1/work")));
        let r = root_of(&s("store/IDEA-1/work/impl/src/lib.rs"), false).unwrap();
        assert_eq!(r.rel, "store/IDEA-1/work");
        assert_eq!(r.kind, RootKind::Work { tree: "store/IDEA-1".into() });
        let r = root_of(&s("llm/015-change.implement/cwd/out/impl/src/lib.rs"), false).unwrap();
        assert_eq!(r.kind, RootKind::Record { record: "llm/015-change.implement".into() });
        let r = root_of(&s("store/IDEA-1/work/.sdc/llm/001-x.y/cwd/out/a"), false).unwrap();
        assert_eq!(r.rel, "store/IDEA-1/work/.sdc/llm/001-x.y/cwd");
        assert!(root_of(&s("llm/015-change.implement/events.jsonl"), false).is_none());
        assert!(root_of(&s("store/IDEA-1/events.jsonl"), false).is_none());
        assert_eq!(record_action("015-change.implement"), "change.implement");
        assert_eq!(record_action("rule"), "rule");
        assert!(is_excluded(&s("store/IDEA-1/work/impl/target"), Some("fs")));
        assert!(is_excluded(&s("store/IDEA-1/evidence"), Some("fs")));
        assert!(!is_excluded(&s("store/IDEA-1/work/blobs"), Some("fs")));
        assert!(is_excluded(&s("fs/blobs"), Some("fs")));
        assert!(is_excluded(&s("llm/001-a/transcripts"), Some("fs")));
    }

    #[test]
    fn a_loom_run_has_one_work_root_and_the_call_roots_in_it() {
        let l = |p: &str| root_of_in(Kind::Loom, &s(p), false);
        assert_eq!(l("work/impl/src/lib.rs").unwrap().kind, RootKind::Loom);
        assert_eq!(l("work/idea.yaml").unwrap().rel, "work");
        let r = l("work/.sdc/llm/001-change.implement/cwd/out/a.rs").unwrap();
        assert_eq!(r.rel, "work/.sdc/llm/001-change.implement/cwd");
        assert_eq!(r.kind, RootKind::Record { record: "work/.sdc/llm/001-change.implement".into() });
        for transport in ["work/.sdc/llm/001-change.implement/request.json", "work/.sdc/llm/001-change.implement/status.json", "work/.sdc/llm/calls.jsonl"] {
            assert!(l(transport).is_none(), "{transport}");
        }
        assert_eq!(l("work/.sdc/records/x.json").unwrap().kind, RootKind::Loom);
        assert!(l("store/IDEA-1/work/findings.yaml").is_none(), "the projection of the store is no root");
        assert!(l("store/runlog-case-ab.jsonl").is_none());
        assert!(l("bindings/sdc.yaml").is_none());
        assert!(root_of_in(Kind::Driver, &s("work/idea.yaml"), false).is_none(), "a driver run has no top-level work root");
    }
}
