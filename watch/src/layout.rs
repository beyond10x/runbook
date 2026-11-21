//! Where things are in a run directory, by relative path components.
//!
//! - case store: `store/<C>` and `store/<C>(/children/<C>)*`; its log is `events.jsonl`
//! - work root: `<case store>/work` (a child's `work` is a symlink to its parent's and is never
//!   walked, so only the tree root's real `work/` is a root)
//! - agent call record: `llm/<NNN-action>`, `decider/<NNN-id>`, or `.sdc/llm/<NNN-action>` (the
//!   default record place under work/); its stream is `events.jsonl`, its root `cwd/`

/// Never watched, at any depth.
pub const EXCLUDED_ANYWHERE: &[&str] = &["target", ".git", "node_modules", "transcripts"];
/// Not watched outside a root (the case store's own records).
pub const EXCLUDED_IN_STORE: &[&str] = &["evidence", "blobs", "evaluations"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RootKind {
    /// A case tree's `work/`; `tree` is the case store dir that owns it (relative path).
    Work { tree: String },
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

fn root_at(c: &[&str]) -> Option<Root> {
    let n = c.len();
    if n >= 2 && c[n - 1] == "cwd" && is_record_dir(&c[..n - 1]) {
        return Some(Root { rel: join(c), kind: RootKind::Record { record: join(&c[..n - 1]) } });
    }
    if n >= 3 && c[n - 1] == "work" && is_case_dir(&c[..n - 1]) {
        return Some(Root { rel: join(c), kind: RootKind::Work { tree: join(&c[..n - 1]) } });
    }
    None
}

/// The innermost root containing the entry at `c` (a file, or a dir that may be a root itself).
pub fn root_of(c: &[&str], is_dir: bool) -> Option<Root> {
    let top = if is_dir { c.len() } else { c.len().saturating_sub(1) };
    (1..=top).rev().find_map(|i| root_at(&c[..i]))
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
}
