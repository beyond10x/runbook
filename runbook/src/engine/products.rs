//! The products of a run: the files under its workspace, which is the run's `work/` (the cases of a run share it).
//! The one reader of a run's files for the daemon: the views `products` and `product` (`commands.rs`), and through
//! them the MCP tools `list_products` and `read_product` and the daemon's page, call it.
//!
//! The guards are the inspector's (`ui/products.mjs`, `ui/runfiles.mjs`), held through a directory descriptor
//! (`record_write::Held`) instead of a real path:
//! - only a name below the workspace: a path is relative to it, and one that is empty, absolute, holds a NUL or a
//!   backslash or has a `..` component is refused before anything is opened;
//! - no link is followed: the workspace is opened with `O_NOFOLLOW | O_DIRECTORY`, each directory below it with
//!   `openat` and the same flags, and the file with `openat` and `O_NOFOLLOW | O_NONBLOCK`; a link at any component is
//!   refused, whether it points into the workspace or out of it, and the listing names no link;
//! - a regular file with one name only: the type and the link count are read from the open descriptor before a byte
//!   is read, so a pipe is never waited for and a second name of a file outside the run (a hard link) is not read;
//! - a name that starts with a dot (a tool's cache or login) is neither listed nor served, at any component;
//! - bounds: a read answers at most [`LIMIT`] bytes and says when the file is longer; a listing stops at the
//!   [`Bounds`] and says which one cut it.
//!
//! Everything is read in the daemon's process: no program is started and no name is handed to one. No answer and no
//! refusal names a path of the host: a path is said as the client asked it, relative to the workspace.

use crate::record_write::{stat_open, Held, Stat};
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// The most bytes of one file a read answers: the inspector's display limit (`ui/products.mjs`, `LIMIT`).
pub const LIMIT: u64 = 1024 * 1024;

/// The most characters of a path a refusal repeats.
const SAID: usize = 300;

/// How far a listing walks: the inspector's `MAX_FILES`, `MAX_DIRS`, `MAX_ENTRIES` and `MAX_DEPTH`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub files: usize,
    pub directories: usize,
    pub entries: usize,
    pub depth: usize,
}

impl Default for Bounds {
    fn default() -> Bounds {
        Bounds { files: 500, directories: 2000, entries: 20000, depth: 6 }
    }
}

/// One listed file: its path relative to the workspace, its length and when it was last written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Product {
    pub path: String,
    pub size: u64,
    pub modified_millis: i64,
}

/// What a listing found and what it left out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Listing {
    /// The regular files with one name, by path.
    pub files: Vec<Product>,
    /// The bound that cut the listing: `files`, `directories`, `entries` or `depth`.
    pub cut: Option<&'static str>,
    /// Entries whose name starts with a dot.
    pub hidden: u64,
    /// Links and files with more than one name: neither listed nor served.
    pub left_out: u64,
}

/// One file read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read1 {
    /// The path as the listing names it.
    pub path: String,
    pub size: u64,
    pub modified_millis: i64,
    /// The text of the first [`LIMIT`] bytes; none when they are not UTF-8 text (a NUL byte or a sequence that is not
    /// UTF-8; a partial character at the cut does not count).
    pub text: Option<String>,
    /// The bytes of the text.
    pub shown: u64,
    /// Whether the file holds more than [`LIMIT`] bytes.
    pub truncated: bool,
}

/// Why a read or a listing is refused. Each names the path as it was asked, cut to a few hundred characters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Empty, absolute, a NUL, a backslash or a `..` component.
    BadPath(String),
    /// A name that starts with a dot.
    NotServed(String),
    Missing(String),
    /// A link at a component.
    Link(String),
    /// A regular file with more than one name.
    SecondName(String),
    NotRegular { path: String, kind: &'static str },
    /// The file system refused the read.
    Unreadable(String),
}

impl Refusal {
    /// The HTTP status of the refusal.
    pub fn status(&self) -> u16 {
        match self {
            Refusal::BadPath(_) => 400,
            Refusal::NotServed(_) | Refusal::Missing(_) => 404,
            Refusal::Link(_) | Refusal::SecondName(_) => 403,
            Refusal::NotRegular { .. } => 415,
            Refusal::Unreadable(_) => 500,
        }
    }

    /// The word of the refusal (`refused` of the answer).
    pub fn word(&self) -> &'static str {
        match self {
            Refusal::BadPath(_) => "bad path",
            Refusal::NotServed(_) => "not served",
            Refusal::Missing(_) => "no such file",
            Refusal::Link(_) => "link",
            Refusal::SecondName(_) => "more than one link",
            Refusal::NotRegular { .. } => "not a regular file",
            Refusal::Unreadable(_) => "unreadable",
        }
    }

    /// The sentence of the refusal. It holds no byte of a file and no path of the host.
    pub fn message(&self) -> String {
        match self {
            Refusal::BadPath(p) => format!("{p:?}: name a path relative to the run's work/ directory, with no `..`, no NUL and no backslash"),
            Refusal::NotServed(p) => format!("{p:?}: a name that starts with a dot is neither listed nor served"),
            Refusal::Missing(p) => format!("{p:?}: no such file under the run's work/ directory"),
            Refusal::Link(p) => format!("{p:?}: a link lies on the path; no link is followed, so it is not read"),
            Refusal::SecondName(p) => format!("{p:?}: the file has more than one link (a hard link may be a file outside the run), so it is not read"),
            Refusal::NotRegular { path, kind } => format!("{path:?}: not a regular file ({kind}), so it is not opened"),
            Refusal::Unreadable(p) => format!("{p:?}: the file could not be read"),
        }
    }
}

fn said(path: &str) -> String {
    path.chars().take(SAID).collect()
}

/// The workspace, held; a workspace that is not there yet is `None`.
fn open_workspace(workspace: &Path, asked: &str) -> Result<Option<Held>, Refusal> {
    match Held::open(workspace) {
        Ok(held) => Ok(Some(held)),
        Err(_) => match std::fs::symlink_metadata(workspace) {
            Err(_) => Ok(None),
            Ok(m) if m.file_type().is_symlink() => Err(Refusal::Link(said(asked))),
            Ok(_) => Err(Refusal::Unreadable(said(asked))),
        },
    }
}

/// The files under `workspace` within the inspector's bounds.
pub fn list(workspace: &Path) -> Result<Listing, Refusal> {
    list_within(workspace, &Bounds::default())
}

/// The files under `workspace`, walked within `bounds`: the regular files with one name, by path; a link is not
/// listed and not entered, a name that starts with a dot is counted and neither listed nor entered, and the walk stops
/// at the first bound it meets and names it. A directory is read an entry at a time, at most as many as the entry
/// bound has left.
pub fn list_within(workspace: &Path, bounds: &Bounds) -> Result<Listing, Refusal> {
    let mut out = Listing::default();
    let Some(root) = open_workspace(workspace, ".")? else { return Ok(out) };
    let mut walk = Walk { bounds, out: &mut out, directories: 0, entries: 0, deeper: false };
    walk.dir(&root, "", 0);
    let deeper = walk.deeper;
    out.files.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    if out.cut.is_none() && deeper {
        out.cut = Some("depth");
    }
    Ok(out)
}

struct Walk<'a> {
    bounds: &'a Bounds,
    out: &'a mut Listing,
    directories: usize,
    entries: usize,
    deeper: bool,
}

impl Walk<'_> {
    fn dir(&mut self, held: &Held, prefix: &str, depth: usize) {
        self.directories += 1;
        if self.directories > self.bounds.directories {
            self.out.cut.get_or_insert("directories");
            return;
        }
        let left = self.bounds.entries.saturating_sub(self.entries).saturating_add(1);
        let Ok(names) = held.names_within(left) else { return };
        for name in names {
            if self.out.cut.is_some() {
                return;
            }
            if self.out.files.len() >= self.bounds.files {
                self.out.cut = Some("files");
                return;
            }
            self.entries += 1;
            if self.entries > self.bounds.entries {
                self.out.cut = Some("entries");
                return;
            }
            let Some(stat) = held.stat(&name) else { continue };
            if name.as_bytes().first() == Some(&b'.') {
                self.out.hidden += 1;
                continue;
            }
            let Ok(text) = name.into_string() else {
                // a name that is not UTF-8 cannot be asked for by a client: not listed
                self.out.left_out += 1;
                continue;
            };
            let path = if prefix.is_empty() { text.clone() } else { format!("{prefix}/{text}") };
            if stat.is_link() {
                self.out.left_out += 1;
            } else if stat.is_dir() {
                if depth < self.bounds.depth {
                    if let Ok(child) = held.child(&text) {
                        self.dir(&child, &path, depth + 1);
                    }
                } else {
                    self.deeper = true;
                }
            } else if stat.is_file() {
                if stat.links > 1 {
                    self.out.left_out += 1;
                } else {
                    self.out.files.push(Product { path, size: stat.size, modified_millis: stat.modified_millis });
                }
            }
        }
    }
}

/// The components of a path a client asked for, or why it is refused: `.` and empty components are dropped.
fn components(asked: &str) -> Result<Vec<&str>, Refusal> {
    let bad = || Refusal::BadPath(said(asked));
    if asked.is_empty() || asked.starts_with('/') || asked.contains('\0') || asked.contains('\\') {
        return Err(bad());
    }
    let parts: Vec<&str> = asked.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    if parts.is_empty() || parts.contains(&"..") {
        return Err(bad());
    }
    if parts.iter().any(|p| p.starts_with('.')) {
        return Err(Refusal::NotServed(said(asked)));
    }
    Ok(parts)
}

/// The file `asked` (a path relative to `workspace`), at most [`LIMIT`] bytes of it.
pub fn read(workspace: &Path, asked: &str) -> Result<Read1, Refusal> {
    read_within(workspace, asked, LIMIT)
}

/// [`read`] with another bound, for a case that proves the bound with a small file.
pub fn read_within(workspace: &Path, asked: &str, limit: u64) -> Result<Read1, Refusal> {
    let parts = components(asked)?;
    let missing = || Refusal::Missing(said(asked));
    let Some(mut dir) = open_workspace(workspace, asked)? else { return Err(missing()) };
    let (leaf, above) = parts.split_last().ok_or_else(missing)?;
    for name in above {
        dir = match dir.stat(name) {
            None => return Err(missing()),
            Some(s) if s.is_link() => return Err(Refusal::Link(said(asked))),
            Some(s) if !s.is_dir() => return Err(missing()),
            Some(_) => dir.child(name).map_err(|_| Refusal::Link(said(asked)))?,
        };
    }
    // The type is read before the open, so a pipe, a socket or a device is never opened; and again from the
    // descriptor, so a file swapped for one between the two is refused too.
    let refuse = |s: &Stat| -> Option<Refusal> {
        if s.is_link() {
            Some(Refusal::Link(said(asked)))
        } else if !s.is_file() {
            Some(Refusal::NotRegular { path: said(asked), kind: s.kind() })
        } else if s.links > 1 {
            Some(Refusal::SecondName(said(asked)))
        } else {
            None
        }
    };
    match dir.stat(leaf) {
        None => return Err(missing()),
        Some(s) => {
            if let Some(r) = refuse(&s) {
                return Err(r);
            }
        }
    }
    let file = dir.open_as_is(leaf).map_err(|e| match e.raw_os_error() {
        Some(libc::ELOOP) => Refusal::Link(said(asked)),
        Some(libc::ENOENT) => missing(),
        _ => Refusal::Unreadable(said(asked)),
    })?;
    let now = stat_open(&file).map_err(|_| Refusal::Unreadable(said(asked)))?;
    if let Some(r) = refuse(&now) {
        return Err(r);
    }
    let mut bytes = Vec::new();
    // The bound is the read's own: a file a program makes longer after the check is read for `limit` + 1 bytes at most.
    file.take(limit.saturating_add(1)).read_to_end(&mut bytes).map_err(|_| Refusal::Unreadable(said(asked)))?;
    let truncated = bytes.len() as u64 > limit || now.size > limit;
    bytes.truncate(limit as usize);
    let text = utf8_text(&bytes, truncated);
    let shown = text.as_ref().map_or(0, |t| t.len() as u64);
    Ok(Read1 { path: parts.join("/"), size: now.size.max(bytes.len() as u64), modified_millis: now.modified_millis, text, shown, truncated })
}

/// The text of bytes that may have been cut off a longer file, or none when they are not text: a NUL byte or a
/// sequence that is not UTF-8. With `cut`, a partial character at the end is dropped (`ui/products.mjs`, `utf8Text`).
pub fn utf8_text(bytes: &[u8], cut: bool) -> Option<String> {
    let mut end = bytes.len();
    if cut {
        let mut i = end;
        let mut back = 0;
        while i > 0 && back < 3 && bytes[i - 1] & 0xc0 == 0x80 {
            i -= 1;
            back += 1;
        }
        if i > 0 && bytes[i - 1] >= 0xc0 {
            let lead = bytes[i - 1];
            let need = if lead >= 0xf0 { 4 } else if lead >= 0xe0 { 3 } else { 2 };
            if end - (i - 1) < need {
                end = i - 1;
            }
        }
    }
    let part = &bytes[..end];
    if part.contains(&0) {
        return None;
    }
    std::str::from_utf8(part).ok().map(str::to_owned)
}
