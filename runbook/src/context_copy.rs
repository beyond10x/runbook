//! The copy of a run's context into its workspace (`context_path` of a submit, `workspace` of a delegation): directories
//! and regular files only, never an entry named `.git` (a directory, or the file of a linked worktree), never an entry
//! the exclude list names, within a bound of entries and bytes. A symbolic link and anything that is neither a
//! directory nor a regular file is skipped, so a link inside a context root cannot carry a file from outside it into a
//! run. Every refusal of the plan comes before the first write: a refused copy writes nothing into the workspace, and a
//! copy that fails while it writes removes what it wrote.
//!
//! The exclude list is the daemon's (`[delegation] copy_exclude` of `runbook.toml`, [`DEFAULT_EXCLUDE`] when the key is
//! absent) and what a submit or a delegation adds (`workspace_exclude`); a run cannot remove an entry of the daemon's.
//! An entry is a name (`target/`, `fixtures`), matched at any depth, or a relative path from the context root
//! (`docs/big/`). An entry that ends in `/` matches a directory only; one without it matches a directory or a file.
//! A name matches a whole name only: `target/` leaves out `target` and `a/target`, never `targets` or `target_x`.

use serde_json::{json, Value};
use std::ffi::{OsStr, OsString};
use std::fs::{File, OpenOptions, Permissions};
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The exclude list of a daemon whose `runbook.toml` has no `[delegation] copy_exclude`.
pub const DEFAULT_EXCLUDE: [&str; 2] = ["target/", "node_modules/"];

/// The name left out of every copy, whatever the exclude list says.
pub const ALWAYS_EXCLUDED: &str = ".git";

/// The most entries a copy reads (`[delegation] copy_max_entries`): the directories and files it copies and the entries
/// it skips, an excluded directory counted once and not entered. This repository's checkout without `.git`, `target/`
/// and `node_modules/` holds 4,212 entries (2026-11-27): the bound is about twelve times that.
pub const DEFAULT_MAX_ENTRIES: u64 = 50_000;

/// The most bytes (the lengths of the files) a copy takes (`[delegation] copy_max_bytes`). The same checkout holds about
/// 32.7 MB, its largest file 3.1 MB: the bound is about eight times the checkout.
pub const DEFAULT_MAX_BYTES: u64 = 256 << 20;

/// The most entries of one exclude list, and the most characters of one entry.
pub const MAX_PATTERNS: usize = 64;
pub const MAX_PATTERN_CHARS: usize = 200;

/// The run event a copy records: the exclude list it applied, the entries and bytes it copied and the entries it skipped.
pub const COPIED_EVENT: &str = "runbook.context-copied";

/// The word a run's failure starts with when its context is over a bound.
pub const OVER_BOUND: &str = "context-over-bound";

/// What the daemon's operator sets for every copy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyRules {
    pub exclude: Vec<String>,
    pub max_entries: u64,
    pub max_bytes: u64,
}

impl Default for CopyRules {
    fn default() -> CopyRules {
        CopyRules { exclude: DEFAULT_EXCLUDE.iter().map(|e| (*e).to_owned()).collect(), max_entries: DEFAULT_MAX_ENTRIES, max_bytes: DEFAULT_MAX_BYTES }
    }
}

/// Why `entry` is no entry of an exclude list, or nothing. The reason never quotes the entry.
pub fn check_pattern(entry: &str) -> Result<(), &'static str> {
    if entry.chars().count() > MAX_PATTERN_CHARS {
        return Err("longer than 200 characters");
    }
    let body = entry.strip_suffix('/').unwrap_or(entry);
    if body.is_empty() {
        return Err("empty");
    }
    if body.starts_with('/') {
        return Err("an absolute path");
    }
    if body.contains('\0') {
        return Err("holds a NUL character");
    }
    if body.split('/').any(|part| part.is_empty() || part == "." || part == "..") {
        return Err("a path with an empty, . or .. part");
    }
    Ok(())
}

/// Why `list` is no exclude list, or nothing: the line names the entry by its place (from 1), never by its value.
pub fn check_list(list: &[String]) -> Result<(), String> {
    if list.len() > MAX_PATTERNS {
        return Err(format!("more than {MAX_PATTERNS} entries"));
    }
    for (i, entry) in list.iter().enumerate() {
        check_pattern(entry).map_err(|why| format!("entry {} is no name or relative path: {why}", i + 1))?;
    }
    Ok(())
}

/// What a copy did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Copied {
    /// The exclude list it applied: `.git`, the daemon's entries, then the run's own that the daemon's do not hold.
    pub exclude: Vec<String>,
    /// The directories and files it copied.
    pub entries: u64,
    /// The bytes of the files it copied.
    pub bytes: u64,
    /// The entries it left out: `.git`, an excluded entry (counted once, not entered), a link, anything that is neither a
    /// directory nor a regular file.
    pub skipped: u64,
}

impl Copied {
    /// The record of [`COPIED_EVENT`]: counts, never the paths it skipped.
    pub fn record(&self) -> Value {
        json!({"exclude": self.exclude, "entries": self.entries, "bytes": self.bytes, "skipped": self.skipped})
    }
}

/// Why a copy wrote nothing into the workspace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CopyError {
    /// The context is over a bound: the key of `[delegation]` that sets it, and its value.
    OverBound { key: &'static str, bound: u64 },
    /// The context could not be read, or the workspace not written.
    Failed(String),
}

impl CopyError {
    /// The run's failure to start.
    pub fn reason(&self) -> String {
        match self {
            CopyError::OverBound { key, bound } => {
                let unit = if *key == "copy_max_bytes" { "bytes" } else { "entries" };
                format!("{OVER_BOUND}: the context is over the bound of {bound} {unit} ([delegation] {key}); nothing of it was copied")
            }
            CopyError::Failed(why) => format!("the context could not be copied into the workspace: {why}"),
        }
    }
}

/// One entry of the exclude list, read.
struct Pattern {
    parts: Vec<Vec<u8>>,
    dir_only: bool,
}

impl Pattern {
    fn of(entry: &str) -> Pattern {
        let dir_only = entry.ends_with('/');
        let body = entry.strip_suffix('/').unwrap_or(entry);
        Pattern { parts: body.split('/').map(|p| p.as_bytes().to_vec()).collect(), dir_only }
    }

    /// Whether the entry at `rel` (its parts from the context root) is left out.
    fn leaves_out(&self, rel: &[OsString], is_dir: bool) -> bool {
        if self.dir_only && !is_dir {
            return false;
        }
        match self.parts.as_slice() {
            [name] => rel.last().is_some_and(|last| last.as_bytes() == name.as_slice()),
            parts => parts.len() == rel.len() && parts.iter().zip(rel).all(|(p, r)| p.as_slice() == r.as_bytes()),
        }
    }
}

/// The exclude list a copy applies: `.git`, the daemon's entries, then each entry of the run's own that is not one of them.
pub fn applied(rules: &CopyRules, extra: &[String]) -> Vec<String> {
    let mut out = vec![ALWAYS_EXCLUDED.to_owned()];
    for entry in rules.exclude.iter().chain(extra) {
        if !out.contains(entry) {
            out.push(entry.clone());
        }
    }
    out
}

enum Planned {
    Dir(PathBuf),
    File(PathBuf),
}

/// Copies the context `from` into the workspace `to` (which exists) under `rules`, with the run's own entries `extra`.
pub fn copy(from: &Path, to: &Path, rules: &CopyRules, extra: &[String]) -> Result<Copied, CopyError> {
    let exclude = applied(rules, extra);
    let patterns: Vec<Pattern> = exclude.iter().skip(1).map(|e| Pattern::of(e)).collect();
    let (planned, copied) = plan(from, rules, &patterns, exclude)?;
    std::fs::create_dir_all(to).map_err(|e| CopyError::Failed(format!("the workspace: {e}")))?;
    let mut tops: Vec<PathBuf> = Vec::new();
    let written = write(from, to, &planned, rules.max_bytes, &mut tops);
    if written.is_err() {
        for top in tops {
            let path = to.join(top);
            let _ = match std::fs::symlink_metadata(&path) {
                Ok(m) if m.is_dir() => std::fs::remove_dir_all(&path),
                Ok(_) => std::fs::remove_file(&path),
                Err(_) => Ok(()),
            };
        }
    }
    written.map(|()| copied)
}

/// The walk: what is copied, in an order where a directory comes before what it holds. Nothing is written.
fn plan(from: &Path, rules: &CopyRules, patterns: &[Pattern], exclude: Vec<String>) -> Result<(Vec<Planned>, Copied), CopyError> {
    let mut planned = Vec::new();
    let mut copied = Copied { exclude, entries: 0, bytes: 0, skipped: 0 };
    let mut stack: Vec<Vec<OsString>> = vec![Vec::new()];
    while let Some(dir) = stack.pop() {
        let at = join(from, &dir);
        let named = shown(&dir);
        let mut listed = Vec::new();
        for entry in std::fs::read_dir(&at).map_err(|e| CopyError::Failed(format!("{named}: {e}")))? {
            let entry = entry.map_err(|e| CopyError::Failed(format!("{named}: {e}")))?;
            listed.push(entry);
        }
        listed.sort_by_key(|e| e.file_name());
        for entry in listed {
            let name = entry.file_name();
            let mut rel = dir.clone();
            rel.push(name.clone());
            let kind = entry.file_type().map_err(|e| CopyError::Failed(format!("{}: {e}", shown(&rel))))?;
            let seen = copied.entries + copied.skipped + 1;
            if seen > rules.max_entries {
                return Err(CopyError::OverBound { key: "copy_max_entries", bound: rules.max_entries });
            }
            let left_out = name.as_os_str() == OsStr::new(ALWAYS_EXCLUDED)
                || !(kind.is_dir() || kind.is_file())
                || patterns.iter().any(|p| p.leaves_out(&rel, kind.is_dir()));
            if left_out {
                copied.skipped += 1;
                continue;
            }
            copied.entries += 1;
            if kind.is_dir() {
                planned.push(Planned::Dir(join(Path::new(""), &rel)));
                stack.push(rel);
            } else {
                let len = entry.metadata().map_err(|e| CopyError::Failed(format!("{}: {e}", shown(&rel))))?.len();
                copied.bytes = copied.bytes.saturating_add(len);
                if copied.bytes > rules.max_bytes {
                    return Err(CopyError::OverBound { key: "copy_max_bytes", bound: rules.max_bytes });
                }
                planned.push(Planned::File(join(Path::new(""), &rel)));
            }
        }
    }
    Ok((planned, copied))
}

fn join(base: &Path, rel: &[OsString]) -> PathBuf {
    let mut p = base.to_path_buf();
    for part in rel {
        p.push(part);
    }
    p
}

/// A relative path as a failure names it; the context root is `.`.
fn shown(rel: &[OsString]) -> String {
    if rel.is_empty() {
        ".".to_owned()
    } else {
        join(Path::new(""), rel).display().to_string()
    }
}

/// Writes what the plan saw. A file read is held to the bytes left of the bound, so a context that grows during the copy
/// is refused as one over the bound; a file that is no longer a regular file fails the copy.
fn write(from: &Path, to: &Path, planned: &[Planned], max_bytes: u64, tops: &mut Vec<PathBuf>) -> Result<(), CopyError> {
    let mut left = max_bytes;
    for p in planned {
        let rel = match p {
            Planned::Dir(rel) | Planned::File(rel) => rel,
        };
        if rel.components().count() == 1 {
            tops.push(rel.clone());
        }
        let failed = |e: std::io::Error| CopyError::Failed(format!("{}: {e}", rel.display()));
        match p {
            Planned::Dir(_) => std::fs::create_dir_all(to.join(rel)).map_err(failed)?,
            Planned::File(_) => {
                let src = OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(from.join(rel)).map_err(failed)?;
                let meta = src.metadata().map_err(failed)?;
                if !meta.is_file() {
                    return Err(CopyError::Failed(format!("{}: no longer a regular file", rel.display())));
                }
                let mut dst = OpenOptions::new().write(true).create(true).truncate(true).custom_flags(libc::O_NOFOLLOW).open(to.join(rel)).map_err(failed)?;
                let n = std::io::copy(&mut (&src).take(left.saturating_add(1)), &mut dst).map_err(failed)?;
                if n > left {
                    return Err(CopyError::OverBound { key: "copy_max_bytes", bound: max_bytes });
                }
                left -= n;
                set_mode(&dst, meta.permissions().mode())?;
            }
        }
    }
    Ok(())
}

/// The copy keeps the permission bits of its source (as `std::fs::copy` did), without set-id or sticky bits.
fn set_mode(dst: &File, mode: u32) -> Result<(), CopyError> {
    dst.set_permissions(Permissions::from_mode(mode & 0o777)).map_err(|e| CopyError::Failed(format!("the workspace: {e}")))
}
