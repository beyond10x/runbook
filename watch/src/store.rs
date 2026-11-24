//! Content: hashing, the blob store, the shadow of last known content, and the change record.

use crate::held::Held;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

pub const MAX_DIFF_BYTES: u64 = 1 << 20;

/// How many bytes of a file of the run are read: 64 MiB. Of a file that holds more, its first 64 MiB are hashed with its
/// length ([`read_content`]); the rest is not read.
pub const CONTENT_LIMIT: u64 = 64 * 1024 * 1024;

/// How many bytes of `changes.jsonl` a restarted watcher reads: 1 GiB. A record that holds more ends the start with an
/// error that names it.
pub const CHANGES_LIMIT: u64 = 1 << 30;

/// How many bytes of `snapshot.json` a restarted watcher reads; a snapshot that holds more is no snapshot.
pub const SNAPSHOT_LIMIT: u64 = 64 * 1024 * 1024;

static STOP: OnceLock<Arc<AtomicBool>> = OnceLock::new();

/// The flag SIGTERM and SIGINT set: a read of a file of the run over [`MAX_DIFF_BYTES`] ends at the next chunk once it
/// is set.
pub fn stop_flag() -> Arc<AtomicBool> {
    STOP.get_or_init(Default::default).clone()
}

fn stopping() -> bool {
    STOP.get().is_some_and(|s| s.load(Ordering::Relaxed))
}

/// A file of the run whose read ended because the watcher is stopping: what it holds is not known, and nothing is recorded
/// for it (a restarted watcher compares it with the record again).
#[derive(Debug)]
pub struct Unread;

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

#[cfg(test)]
pub fn sha(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// A regular file's content, or `None` (absent, a directory, a symlink, a pipe, unreadable).
///
/// The file is opened as the files of a call record are ([`crate::attrib::open_regular`]: no link followed, no pipe waited
/// for, a regular file) and at most [`CONTENT_LIMIT`] plus one bytes are read from the one descriptor. A file of at most
/// [`CONTENT_LIMIT`] bytes has the sha256 of its bytes; of a longer file the sha256 is that of its first
/// [`CONTENT_LIMIT`] bytes followed by its length (8 bytes, big-endian). Once the watcher is stopping, a read over
/// [`MAX_DIFF_BYTES`] ends at the next chunk with [`Unread`].
pub fn read_content(p: &Path) -> Result<Option<Content>, Unread> {
    read_content_with(p, CONTENT_LIMIT, &stopping)
}

/// [`read_content`] with the bound `limit` in place of [`CONTENT_LIMIT`] and `stop` in place of the stop flag.
fn read_content_with(p: &Path, limit: u64, stop: &dyn Fn() -> bool) -> Result<Option<Content>, Unread> {
    let Some(file) = crate::attrib::open_regular(p) else { return Ok(None) };
    content_of(file, limit, stop)
}

/// The content of the open regular file `file`: at most `limit` plus one bytes are read from it.
fn content_of(file: fs::File, limit: u64, stop: &dyn Fn() -> bool) -> Result<Option<Content>, Unread> {
    let mut from = file.take(limit + 1);
    let mut h = Sha256::new();
    let mut kept = Some(Vec::new());
    let mut total = 0u64;
    let mut buf = vec![0u8; 1 << 16];
    loop {
        if total > MAX_DIFF_BYTES && stop() {
            return Err(Unread);
        }
        let n = match from.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Ok(None),
        };
        let hashed = (n as u64).min(limit.saturating_sub(total)) as usize;
        h.update(&buf[..hashed]);
        total += n as u64;
        if total > MAX_DIFF_BYTES {
            kept = None;
        } else if let Some(k) = kept.as_mut() {
            k.extend_from_slice(&buf[..n]);
        }
    }
    if total > limit {
        let len = from.get_ref().metadata().map(|m| m.len()).unwrap_or(total);
        h.update(len.to_be_bytes());
    }
    let too_large = total > MAX_DIFF_BYTES;
    Ok(Some(Content { sha: hex(&h.finalize()), too_large, bytes: kept }))
}

pub fn is_binary(b: &[u8]) -> bool {
    b[..b.len().min(8192)].contains(&0) || std::str::from_utf8(b).is_err()
}

/// Writes `bytes` to a file that is new at `tmp`: the open is `O_CREAT | O_EXCL | O_NOFOLLOW | O_NONBLOCK`, so nothing at
/// the name is followed, opened or written through, and no pipe is waited for. The programs of a run write the run
/// directory too, so a name the watcher expects to be free may hold a link, a pipe or a file of a program: what lies at
/// `tmp` is removed (`unlink` follows nothing and opens nothing) and the open is tried once more; a name that cannot be
/// removed (a directory) is an error. The mode is the one `fs::write` makes (0o666 less the umask). Every name is one of
/// the held directory `dir`.
fn write_new(dir: &Held, tmp: &str, bytes: &[u8]) -> std::io::Result<()> {
    match write_new_once(dir, tmp, bytes) {
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            dir.remove(tmp)?;
            write_new_once(dir, tmp, bytes)
        }
        other => other,
    }
}

/// How many temporary names the store of one blob tries before it gives up.
const NAMES_TRIED: usize = 16;

/// [`write_new`] without the removal: a name that is taken is an `AlreadyExists` error and is left as it is. A file that
/// this call made and could not fill is removed again.
fn write_new_once(dir: &Held, tmp: &str, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = dir.create_new(tmp)?;
    let written = file.write_all(bytes).and_then(|()| file.flush());
    if written.is_err() {
        let _ = dir.remove(tmp);
    }
    written
}

/// Writes `bytes` to the file `name` of the held directory `dir` through the temporary file `.<name>.tmp` ([`write_new`])
/// and a rename, both relative to the descriptor.
pub fn write_through(dir: &Held, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = format!(".{name}.tmp");
    write_new(dir, &tmp, bytes)?;
    dir.rename(&tmp, name).inspect_err(|_| {
        let _ = dir.remove(&tmp);
    })
}

/// Content-addressed: `<dir>/<sha256>`, written once, through a temp file and a rename, in the blob directory the watcher
/// holds.
pub struct Blobs {
    dir: Held,
}

impl Blobs {
    pub fn new(dir: Held) -> Blobs {
        Blobs { dir }
    }
    pub fn put(&self, sha: &str, bytes: &[u8]) {
        if self.has(sha) {
            return;
        }
        // A name a program of the run took is not used and is not removed: the next of `NAMES_TRIED` names is tried, as the
        // runner's writer does, so a program that keeps taking one name does not keep the blob from being stored. The
        // rename replaces a link or a pipe at the name of the blob.
        for attempt in 0..NAMES_TRIED {
            let tmp = match attempt {
                0 => format!(".{sha}.{}", std::process::id()),
                n => format!(".{sha}.{}.{n}", std::process::id()),
            };
            match write_new_once(&self.dir, &tmp, bytes) {
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Ok(()) => {
                    if self.dir.rename(&tmp, sha).is_err() {
                        let _ = self.dir.remove(&tmp);
                    }
                }
                Err(_) => {}
            }
            return;
        }
    }
    /// A stored blob: a regular file with one name in the held directory, no link followed, no pipe waited for, at most
    /// [`MAX_DIFF_BYTES`] bytes (no larger content is stored).
    pub fn get(&self, sha: &str) -> Option<Vec<u8>> {
        self.dir.read(sha, MAX_DIFF_BYTES)
    }
    /// Whether a regular file lies at the name of the blob (a link at it is no blob).
    pub fn has(&self, sha: &str) -> bool {
        self.dir.is_file(sha)
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
    /// The file `name` of the held directory `dir`, opened once to append ([`Held::open_append`]).
    pub fn open(dir: &Held, name: &str) -> std::io::Result<Log> {
        Ok(Log { file: dir.open_append(name)? })
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
pub fn write_snapshot(out: &Held, run: &Path, shadow: &HashMap<PathBuf, Known>, seq: u64) -> std::io::Result<()> {
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
    write_through(out, "snapshot.json", &serde_json::to_vec_pretty(&v).unwrap_or_default())
}

/// Calls `line` with each line `from` yields, without its newline: at most `limit` bytes are read (more is an error), and
/// a line over `line_limit` bytes is skipped, not held.
fn each_line(from: impl Read, limit: u64, line_limit: usize, mut line: impl FnMut(&[u8])) -> std::io::Result<()> {
    let mut r = std::io::BufReader::new(from.take(limit.saturating_add(1)));
    let (mut held, mut long, mut total) = (Vec::new(), false, 0u64);
    loop {
        let buf = r.fill_buf()?;
        if buf.is_empty() {
            break;
        }
        let end = buf.iter().position(|&b| b == b'\n');
        let part = &buf[..end.unwrap_or(buf.len())];
        if !long && held.len() + part.len() > line_limit {
            long = true;
            held = Vec::new();
        } else if !long {
            held.extend_from_slice(part);
        }
        let used = part.len() + usize::from(end.is_some());
        total += used as u64;
        r.consume(used);
        if end.is_some() {
            if !long {
                line(&held);
            }
            held.clear();
            long = false;
        }
    }
    if total > limit {
        return Err(std::io::Error::other(format!("more than {limit} bytes")));
    }
    if !long && !held.is_empty() {
        line(&held);
    }
    Ok(())
}

/// What the output dir already knows: the snapshot plus every change after it, and the next seq.
/// `None` when there is no snapshot.
///
/// Both records are read from the held output directory as regular files with one name, with no link followed and no
/// pipe waited for (anything else is no record); `changes.jsonl` is read for at most [`CHANGES_LIMIT`] bytes (a record
/// that holds more is an error, so that no `seq` is given twice) and `snapshot.json` for at most [`SNAPSHOT_LIMIT`].
pub fn replay(out: &Held, run: &Path, blobs: &Blobs) -> std::io::Result<(Option<HashMap<PathBuf, Known>>, u64)> {
    let mut next = 0u64;
    let mut changes: Vec<Value> = Vec::new();
    if let Some(file) = out.open_regular("changes.jsonl") {
        each_line(file, CHANGES_LIMIT, crate::attrib::RECORD_LIMIT as usize, |l| {
            let Ok(mut v) = serde_json::from_slice::<Value>(l) else { return };
            if v["format"] != "sdc.fs-change/1" {
                return;
            }
            if let Some(o) = v.as_object_mut() {
                o.remove("diff");
                o.remove("attribution");
            }
            changes.push(v);
        }).map_err(|e| std::io::Error::other(format!("{}: {e}", out.path().join("changes.jsonl").display())))?;
    }
    for v in &changes {
        if let Some(s) = v["seq"].as_u64() {
            next = next.max(s + 1);
        }
    }
    let Some(text) = out.read("snapshot.json", SNAPSHOT_LIMIT) else { return Ok((None, next)) };
    let Ok(snap) = serde_json::from_slice::<Value>(&text) else { return Ok((None, next)) };
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
    Ok((Some(shadow), next))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_counts_and_flags() {
        let blobs = Blobs::new(Held::open(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap());
        let c = Content { sha: sha(b"a\nb\n"), too_large: false, bytes: Some(b"a\nb\n".to_vec()) };
        let ch = build_change(&blobs, 0, "r", "f", "create", None, None, Some(&c));
        assert_eq!((ch.lines_added, ch.lines_removed), (Some(2), Some(0)));
        assert!(ch.diff.unwrap().contains("+++ b/f"));
        let bin = Content { sha: sha(b"\0\x01"), too_large: false, bytes: Some(b"\0\x01".to_vec()) };
        let ch = build_change(&blobs, 0, "r", "f", "create", None, None, Some(&bin));
        assert!(ch.binary && ch.diff.is_none() && ch.lines_added.is_none());
    }

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::current_exe().unwrap().parent().unwrap().join(format!("sdc-watch-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn sparse(p: &Path, len: u64) {
        fs::File::create(p).unwrap().set_len(len).unwrap();
    }

    #[test]
    fn a_file_over_the_content_bound_is_hashed_for_its_first_bytes_and_its_length() {
        let d = scratch("content-bound");
        let limit = 2 * MAX_DIFF_BYTES;
        sparse(&d.join("one"), limit + 1);
        sparse(&d.join("two"), limit + 2);
        sparse(&d.join("same"), limit + 2);
        let read = |name: &str| read_content_with(&d.join(name), limit, &|| false);
        let (one, two, same) = (read("one"), read("two"), read("same"));
        let _ = fs::remove_dir_all(&d);
        let (one, two, same) = (one.unwrap().unwrap(), two.unwrap().unwrap(), same.unwrap().unwrap());
        assert!(one.too_large && one.bytes.is_none(), "a file over the bound is too large and not kept");
        assert_ne!(one.sha, two.sha, "two files over the bound that differ in length have different hashes");
        assert_eq!(two.sha, same.sha, "two files over the bound with the same first bytes and length have one hash");
        let mut h = Sha256::new();
        h.update(vec![0u8; limit as usize]);
        h.update((limit + 2).to_be_bytes());
        assert_eq!(two.sha, hex(&h.finalize()), "the hash is that of the first bytes followed by the length");
        assert_eq!(CONTENT_LIMIT, 64 * 1024 * 1024, "the bound of a read of a file of the run is 64 MiB");
    }

    #[test]
    fn a_file_of_the_run_is_read_for_at_most_its_bound_plus_one_byte() {
        use std::io::Seek;
        let d = scratch("read-bound");
        let limit = 2 * MAX_DIFF_BYTES;
        sparse(&d.join("long"), 3 * limit);
        let file = fs::File::open(d.join("long")).unwrap();
        let mut shared = file.try_clone().unwrap();
        let content = content_of(file, limit, &|| false);
        let read = shared.stream_position().unwrap();
        let _ = fs::remove_dir_all(&d);
        assert!(content.unwrap().unwrap().too_large);
        assert!(read <= limit + 1, "{read} bytes were read of a file of the run, more than its bound plus one ({})", limit + 1);
    }

    #[test]
    fn a_stopping_watcher_reads_a_large_file_no_further_and_a_small_file_whole() {
        let d = scratch("stopping");
        sparse(&d.join("large"), 2 * MAX_DIFF_BYTES);
        fs::write(d.join("small"), "small\n").unwrap();
        let (large, small) = (read_content_with(&d.join("large"), CONTENT_LIMIT, &|| true), read_content_with(&d.join("small"), CONTENT_LIMIT, &|| true));
        let _ = fs::remove_dir_all(&d);
        assert!(large.is_err(), "a file over the diff bound was read on while the watcher is stopping");
        assert_eq!(small.unwrap().unwrap().bytes.as_deref(), Some(&b"small\n"[..]), "a small file is read whole while the watcher is stopping");
    }

    #[test]
    fn a_record_over_its_bound_is_an_error_and_a_line_over_its_bound_is_skipped() {
        let mut lines = Vec::new();
        each_line(&b"a\nlong line\nb"[..], 100, 4, |l| lines.push(String::from_utf8(l.to_vec()).unwrap())).unwrap();
        assert_eq!(lines, vec!["a".to_owned(), "b".to_owned()]);
        assert!(each_line(&b"a\nb\n"[..], 3, 4, |_| {}).is_err(), "a record over its bound was read");
        assert!(each_line(&b"a\nb\n"[..], 4, 4, |_| {}).is_ok(), "a record of exactly its bound was refused");
    }
}
