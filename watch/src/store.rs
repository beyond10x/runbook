//! Content: hashing, the blob store, the shadow of last known content, and the change record.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const MAX_DIFF_BYTES: u64 = 1 << 20;

/// What a regular file holds now.
pub struct Content {
    pub sha: String,
    pub too_large: bool,
    /// The bytes, unless too large.
    pub bytes: Option<Vec<u8>>,
}

/// The last known content of a path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Known {
    pub sha: String,
    pub too_large: bool,
}

pub fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sha(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// A regular file's content, or `None` (absent, a directory, a symlink, unreadable).
pub fn read_content(p: &Path) -> Option<Content> {
    let md = fs::symlink_metadata(p).ok()?;
    if !md.file_type().is_file() {
        return None;
    }
    if md.len() > MAX_DIFF_BYTES {
        let mut f = fs::File::open(p).ok()?;
        let mut h = Sha256::new();
        let mut buf = vec![0u8; 1 << 16];
        let mut total = 0u64;
        loop {
            let n = f.read(&mut buf).ok()?;
            if n == 0 {
                break;
            }
            total += n as u64;
            h.update(&buf[..n]);
        }
        return Some(Content { sha: hex(&h.finalize()), too_large: total > MAX_DIFF_BYTES, bytes: None });
    }
    let bytes = fs::read(p).ok()?;
    if bytes.len() as u64 > MAX_DIFF_BYTES {
        return Some(Content { sha: sha(&bytes), too_large: true, bytes: None });
    }
    Some(Content { sha: sha(&bytes), too_large: false, bytes: Some(bytes) })
}

pub fn is_binary(b: &[u8]) -> bool {
    b[..b.len().min(8192)].contains(&0) || std::str::from_utf8(b).is_err()
}

/// Writes `bytes` to a file that is new at `tmp`: the open is `O_CREAT | O_EXCL | O_NOFOLLOW | O_NONBLOCK`, so nothing at
/// the name is followed, opened or written through, and no pipe is waited for. The programs of a run write the run
/// directory too, so a name the watcher expects to be free may hold a link, a pipe or a file of a program: what lies at
/// `tmp` is removed (`unlink` follows nothing and opens nothing) and the open is tried once more; a name that cannot be
/// removed (a directory) is an error. The mode is the one `fs::write` makes (0o666 less the umask).
fn write_new(tmp: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let open = || fs::OpenOptions::new().write(true).create_new(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(tmp);
    let mut file = match open() {
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            fs::remove_file(tmp)?;
            open()?
        }
        other => other?,
    };
    file.write_all(bytes)?;
    file.flush()
}

/// Content-addressed: `<dir>/<sha256>`, written once, through a temp file and a rename.
pub struct Blobs {
    pub dir: PathBuf,
}

impl Blobs {
    pub fn put(&self, sha: &str, bytes: &[u8]) {
        let p = self.dir.join(sha);
        if p.exists() {
            return;
        }
        let tmp = self.dir.join(format!(".{sha}.{}", std::process::id()));
        if write_new(&tmp, bytes).is_ok() && fs::rename(&tmp, &p).is_err() {
            let _ = fs::remove_file(&tmp);
        }
    }
    pub fn get(&self, sha: &str) -> Option<Vec<u8>> {
        fs::read(self.dir.join(sha)).ok()
    }
    pub fn has(&self, sha: &str) -> bool {
        self.dir.join(sha).exists()
    }
    /// Stores the content (unless too large) and returns what to remember of it.
    pub fn keep(&self, c: &Content) -> Known {
        if let Some(b) = &c.bytes {
            self.put(&c.sha, b);
        }
        Known { sha: c.sha.clone(), too_large: c.too_large }
    }
}

/// One effective change, without its sequence number and attribution.
pub struct Change {
    pub at: i64,
    pub root: String,
    pub path: String,
    pub op: &'static str,
    pub from_path: Option<String>,
    pub before: Option<String>,
    pub after: Option<String>,
    pub binary: bool,
    pub too_large: bool,
    pub lines_added: Option<u64>,
    pub lines_removed: Option<u64>,
    pub diff: Option<String>,
}

/// Builds the change from the old and new content (`None`: absent), with a unified diff.
#[allow(clippy::too_many_arguments)]
pub fn build_change(blobs: &Blobs, at: i64, root: &str, path: &str, op: &'static str, from_path: Option<&str>,
                    before: Option<&Known>, after: Option<&Content>) -> Change {
    let too_large = before.is_some_and(|k| k.too_large) || after.is_some_and(|c| c.too_large);
    let old: Option<Vec<u8>> = match before {
        None => Some(Vec::new()),
        Some(k) if k.too_large => None,
        Some(k) => blobs.get(&k.sha),
    };
    let new: Option<Vec<u8>> = match after {
        None => Some(Vec::new()),
        Some(c) => c.bytes.clone(),
    };
    let binary = !too_large && [&old, &new].iter().any(|b| b.as_deref().is_some_and(is_binary));
    let mut ch = Change { at, root: root.into(), path: path.into(), op, from_path: from_path.map(String::from),
        before: before.map(|k| k.sha.clone()), after: after.map(|c| c.sha.clone()), binary, too_large,
        lines_added: None, lines_removed: None, diff: None };
    if too_large || binary {
        return ch;
    }
    let (Some(old), Some(new)) = (old, new) else { return ch };
    if old == new {
        ch.lines_added = Some(0);
        ch.lines_removed = Some(0);
        return ch;
    }
    let (o, n) = (String::from_utf8_lossy(&old), String::from_utf8_lossy(&new));
    let diff = similar::TextDiff::configure().timeout(Duration::from_secs(2)).diff_lines(o.as_ref(), n.as_ref());
    let (mut add, mut rem) = (0u64, 0u64);
    for c in diff.iter_all_changes() {
        match c.tag() {
            similar::ChangeTag::Insert => add += 1,
            similar::ChangeTag::Delete => rem += 1,
            similar::ChangeTag::Equal => {}
        }
    }
    let a = if before.is_none() { "/dev/null".to_string() } else { format!("a/{}", from_path.unwrap_or(path)) };
    let b = if after.is_none() { "/dev/null".to_string() } else { format!("b/{path}") };
    ch.diff = Some(diff.unified_diff().context_radius(3).header(&a, &b).to_string());
    ch.lines_added = Some(add);
    ch.lines_removed = Some(rem);
    ch
}

pub fn change_json(seq: u64, c: &Change, attribution: Value) -> Value {
    json!({
        "format": "sdc.fs-change/1",
        "seq": seq,
        "at": crate::timefmt::fmt_ms(c.at),
        "root": c.root,
        "path": c.path,
        "op": c.op,
        "from_path": c.from_path,
        "before": c.before,
        "after": c.after,
        "binary": c.binary,
        "too_large": c.too_large,
        "lines_added": c.lines_added,
        "lines_removed": c.lines_removed,
        "diff": c.diff,
        "attribution": attribution,
    })
}

/// Append-only JSON lines.
pub struct Log {
    file: fs::File,
}

impl Log {
    pub fn open(p: &Path) -> std::io::Result<Log> {
        Ok(Log { file: fs::OpenOptions::new().create(true).append(true).open(p)? })
    }
    pub fn append(&mut self, v: &Value) {
        let mut line = serde_json::to_string(v).unwrap_or_default();
        line.push('\n');
        if let Err(e) = self.file.write_all(line.as_bytes()) {
            eprintln!("sdc-watch: cannot append a record: {e}");
        }
    }
}

/// The snapshot (`snapshot.json`): the content of every tracked file when it was taken, and the
/// first change `seq` it does not yet include.
pub fn write_snapshot(out: &Path, run: &Path, shadow: &HashMap<PathBuf, Known>, seq: u64) -> std::io::Result<()> {
    let mut files: Vec<Value> = shadow.iter().filter_map(|(p, k)| {
        let rel = p.strip_prefix(run).ok()?.to_string_lossy().into_owned();
        let c: Vec<&str> = rel.split('/').collect();
        let root = crate::layout::root_of(&c, false)?;
        let path = rel.strip_prefix(&format!("{}/", root.rel))?.to_string();
        Some(json!({"root": root.rel, "path": path, "sha256": k.sha, "too_large": k.too_large}))
    }).collect();
    files.sort_by(|a, b| (a["root"].as_str(), a["path"].as_str()).cmp(&(b["root"].as_str(), b["path"].as_str())));
    let v = json!({"format": "sdc.fs-snapshot/1", "at": crate::timefmt::fmt_ms(crate::timefmt::now_ms()),
        "seq": seq, "files": files});
    let tmp = out.join(".snapshot.json.tmp");
    write_new(&tmp, &serde_json::to_vec_pretty(&v).unwrap_or_default())?;
    fs::rename(&tmp, out.join("snapshot.json"))
}

/// What the output dir already knows: the snapshot plus every change after it, and the next seq.
/// `None` when there is no snapshot.
pub fn replay(out: &Path, run: &Path, blobs: &Blobs) -> (Option<HashMap<PathBuf, Known>>, u64) {
    let mut next = 0u64;
    let changes: Vec<Value> = fs::read_to_string(out.join("changes.jsonl")).unwrap_or_default().lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["format"] == "sdc.fs-change/1").collect();
    for v in &changes {
        if let Some(s) = v["seq"].as_u64() {
            next = next.max(s + 1);
        }
    }
    let Ok(text) = fs::read_to_string(out.join("snapshot.json")) else { return (None, next) };
    let Ok(snap) = serde_json::from_str::<Value>(&text) else { return (None, next) };
    let from = snap["seq"].as_u64().unwrap_or(0);
    let mut shadow = HashMap::new();
    let abs = |root: &Value, path: &Value| run.join(root.as_str().unwrap_or("")).join(path.as_str().unwrap_or(""));
    for f in snap["files"].as_array().into_iter().flatten() {
        if let Some(sha) = f["sha256"].as_str() {
            shadow.insert(abs(&f["root"], &f["path"]), Known { sha: sha.into(), too_large: f["too_large"] == true });
        }
    }
    for v in changes.iter().filter(|v| v["seq"].as_u64().is_some_and(|s| s >= from)) {
        let p = abs(&v["root"], &v["path"]);
        if v["op"] == "rename" {
            shadow.remove(&abs(&v["root"], &v["from_path"]));
        }
        match v["after"].as_str() {
            Some(sha) => {
                shadow.insert(p, Known { sha: sha.into(), too_large: !blobs.has(sha) });
            }
            None => {
                shadow.remove(&p);
            }
        }
    }
    (Some(shadow), next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_counts_and_flags() {
        let blobs = Blobs { dir: PathBuf::from("/nonexistent") };
        let c = Content { sha: sha(b"a\nb\n"), too_large: false, bytes: Some(b"a\nb\n".to_vec()) };
        let ch = build_change(&blobs, 0, "r", "f", "create", None, None, Some(&c));
        assert_eq!((ch.lines_added, ch.lines_removed), (Some(2), Some(0)));
        assert!(ch.diff.unwrap().contains("+++ b/f"));
        let bin = Content { sha: sha(b"\0\x01"), too_large: false, bytes: Some(b"\0\x01".to_vec()) };
        let ch = build_change(&blobs, 0, "r", "f", "create", None, None, Some(&bin));
        assert!(ch.binary && ch.diff.is_none() && ch.lines_added.is_none());
    }
}
