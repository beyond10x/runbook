//! Which files of the product tree go into the prompt, the path rule, and the writer.
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::ROOT_FILES;
use crate::reply::walk;

/// Why a file is listed without text. A label of the manifest; the prompt says it with the
/// instruction file `tree.why.<...>.md` that `assemble` picks for it.
pub(super) const NOT_UTF8: &str = "not UTF-8 text";
pub(super) const OVER_CAP: &str = "over the cap";

#[derive(Debug, Clone, PartialEq)]
/// A file of the product tree with the reason it was inlined or listed.
pub struct TreeFile {
    /// The path relative to the tree.
    pub path: String,
    /// The size in bytes.
    pub bytes: usize,
    /// The label of the rule that put it there.
    pub why: &'static str,
}

#[derive(Debug, Default)]
/// The files inlined and the files listed by the input rule.
pub struct Selection {
    /// Whether the mainline tree has no product file.
    pub first_story: bool,
    /// The files whose text goes into the prompt.
    pub inlined: Vec<TreeFile>,
    /// The files listed by path and size only.
    pub listed: Vec<TreeFile>,
    /// The bytes of the inlined files.
    pub inlined_bytes: usize,
}

/// Build output or a hidden entry: never a file of the product.
pub(super) fn not_product(rel: &str) -> bool {
    rel.split('/').next() == Some("target") || rel.split('/').any(|c| c.starts_with('.'))
}

/// The product files of a tree, sorted: no build output, no hidden entry.
pub(super) fn product_files(dir: &Path) -> Vec<String> {
    if !dir.is_dir() {
        return vec![];
    }
    walk(dir).into_iter().filter(|p| !not_product(p)).collect()
}

/// The input rule (module docs).
pub fn select_tree(tree: &Path, mainline: &Path, story_text: &str, cap: usize) -> Selection {
    let base: BTreeSet<String> = product_files(mainline).into_iter().collect();
    let first_story = base.is_empty();
    let mut ranked = Vec::new();
    let mut listed = Vec::new();
    for p in product_files(tree) {
        let bytes = fs::read(tree.join(&p)).unwrap_or_default();
        let n = bytes.len();
        if String::from_utf8(bytes.clone()).is_err() {
            listed.push(TreeFile { path: p, bytes: n, why: NOT_UTF8 });
            continue;
        }
        let (rank, why) = if story_text.contains(p.as_str()) {
            (0, "named by the story")
        } else if first_story {
            (1, "first story: the whole tree")
        } else if !base.contains(&p) || fs::read(mainline.join(&p)).ok().as_deref() != Some(bytes.as_slice()) {
            (1, "this story's change")
        } else if ROOT_FILES.contains(&p.as_str()) {
            (2, "crate root")
        } else {
            (3, "merged by an earlier story")
        };
        ranked.push((rank, TreeFile { path: p, bytes: n, why }));
    }
    ranked.sort_by(|a, b| (a.0, &a.1.path).cmp(&(b.0, &b.1.path)));
    let mut s = Selection { first_story, ..Selection::default() };
    for (_, f) in ranked {
        if s.inlined_bytes + f.bytes <= cap {
            s.inlined_bytes += f.bytes;
            s.inlined.push(f);
        } else {
            listed.push(TreeFile { why: OVER_CAP, ..f });
        }
    }
    listed.sort_by(|a, b| a.path.cmp(&b.path));
    s.listed = listed;
    s
}

/// A reply path, checked: relative, normal, inside the product tree, not build output, not
/// hidden. Ok(the path) or Err(why).
pub fn confine(p: &str) -> Result<String, String> {
    let why = |w: &str| Err(format!("path {p:?}: {w}"));
    if p.is_empty() {
        return why("an empty path");
    }
    if p.contains('\\') {
        return why("a backslash: write paths with /");
    }
    let b = p.as_bytes();
    if p.starts_with('/') || (b.len() > 1 && b[1] == b':' && b[0].is_ascii_alphabetic()) {
        return why("an absolute path: write it relative to impl/ (for example src/lib.rs)");
    }
    if p.contains('\0') {
        return why("a NUL byte");
    }
    for c in p.split('/') {
        match c {
            ".." => return why("leaves the product tree (`..`)"),
            "" | "." => return why("not a normal relative path (an empty or `.` segment)"),
            _ if c.starts_with('.') => return why("a hidden entry: .sdc, .git, .rework and the like are not the product's"),
            _ => {}
        }
    }
    if p.split('/').next() == Some("target") {
        return why("build output (target/) is not the product's");
    }
    Ok(p.to_string())
}

/// Write the files into `root` (the call's `out/impl/`). Every path is checked first (`confine`,
/// and no existing symlink or file on the way): one refused path writes nothing.
pub fn write_files(root: &Path, files: &[(String, String)]) -> Result<Vec<String>, String> {
    for (p, _) in files {
        confine(p)?;
        let mut at = root.to_path_buf();
        let parts: Vec<&str> = p.split('/').collect();
        for (i, c) in parts.iter().enumerate() {
            at.push(c);
            let Ok(m) = fs::symlink_metadata(&at) else { break };
            if m.file_type().is_symlink() {
                return Err(format!("path {p:?}: goes through a symlink ({})", parts[..=i].join("/")));
            }
            if i + 1 < parts.len() && !m.is_dir() {
                return Err(format!("path {p:?}: {} is a file, not a directory", parts[..=i].join("/")));
            }
            if i + 1 == parts.len() && m.is_dir() {
                return Err(format!("path {p:?}: is a directory"));
            }
        }
    }
    let mut written = Vec::new();
    for (p, content) in files {
        let at = root.join(p);
        if let Some(d) = at.parent() {
            fs::create_dir_all(d).map_err(|e| format!("path {p:?}: {e}"))?;
        }
        fs::write(&at, content).map_err(|e| format!("path {p:?}: {e}"))?;
        written.push(p.clone());
    }
    Ok(written)
}
