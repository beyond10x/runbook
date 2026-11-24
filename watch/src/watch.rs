//! The watcher: inotify over the run directory, a per-path debounce, the shadow, and the record.

use crate::attrib::{self, Attributor};
use crate::held::Held;
use crate::layout::{self, Root};
use crate::store::{self, Blobs, Change, Content, Known, Log};
use crate::timefmt::{fmt_ms, now_ms};
use inotify::{EventMask, Inotify, WatchDescriptor, WatchMask};
use serde_json::json;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::OsString;
use std::fs;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// A path is written when it has been quiet this long.
pub const DEBOUNCE: Duration = Duration::from_millis(100);
/// ... or when it has been busy this long.
pub const MAX_DELAY: Duration = Duration::from_secs(2);
/// How long a change in an agent call waits for the call's stream to reach its time.
pub const HOLD: Duration = Duration::from_millis(1500);
/// How long a change written without a tool can still get one from a late stream.
pub const LATE: Duration = Duration::from_secs(600);

fn rel_of(run: &Path, p: &Path) -> Option<String> {
    let r = p.strip_prefix(run).ok()?.to_string_lossy().into_owned();
    (!r.is_empty()).then_some(r)
}

fn comps(rel: &str) -> Vec<&str> {
    rel.split('/').collect()
}

/// Visits every non-excluded directory (before listing it) and file under `start`, without
/// following symlinks.
pub fn walk(run: &Path, start: &Path, out_rel: Option<&str>, f: &mut dyn FnMut(&Path, &str, bool)) {
    let rel = rel_of(run, start).unwrap_or_default();
    if !rel.is_empty() && layout::is_excluded(&comps(&rel), out_rel) {
        return;
    }
    f(start, &rel, true);
    let Ok(rd) = fs::read_dir(start) else { return };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let Ok(ft) = e.file_type() else { continue };
        let p = e.path();
        if ft.is_dir() {
            walk(run, &p, out_rel, f);
        } else if ft.is_file() {
            if let Some(r) = rel_of(run, &p) {
                if !layout::is_excluded(&comps(&r), out_rel) {
                    f(&p, &r, false);
                }
            }
        }
    }
}

/// What a file event means for the watcher.
enum Source {
    Case(String),
    Stream(String),
    /// Loom run: `rounds.jsonl`, one line per finished step.
    Rounds,
    /// Loom run: a call record's `status.json`.
    Status(String),
}

fn source_of(rel: &str) -> Option<Source> {
    let loom = layout::kind() == layout::Kind::Loom;
    if loom && rel == "rounds.jsonl" {
        return Some(Source::Rounds);
    }
    let (parent, name) = rel.rsplit_once('/')?;
    let c = comps(parent);
    match name {
        // a Loom run's case logs are a projection written whole, not read
        "events.jsonl" if layout::is_case_dir(&c) => (!loom).then(|| Source::Case(parent.to_string())),
        "events.jsonl" if layout::is_record_dir(&c) => Some(Source::Stream(parent.to_string())),
        "status.json" if loom && layout::is_record_dir(&c) => Some(Source::Status(parent.to_string())),
        _ => None,
    }
}

struct Dirty {
    first_ord: u64,
    first_at: i64,
    first_seen: Instant,
    last: Instant,
    from: Option<PathBuf>,
}

struct Pending {
    change: Change,
    root: Root,
    ord: u64,
    file_name: String,
    deadline: Instant,
}

struct Late {
    seq: u64,
    record: String,
    root: Root,
    ord: u64,
    at: i64,
    file_name: String,
    until: Instant,
}

pub struct Watcher {
    run: PathBuf,
    out_dir: Held,
    out_rel: Option<String>,
    ino: Inotify,
    wds: HashMap<WatchDescriptor, PathBuf>,
    pub shadow: HashMap<PathBuf, Known>,
    blobs: Blobs,
    log: Log,
    pub seq: u64,
    dirty: HashMap<PathBuf, Dirty>,
    moves: HashMap<u32, (PathBuf, bool)>,
    pending: VecDeque<Pending>,
    late: Vec<Late>,
    attr: Attributor,
    ord: u64,
    touched: HashSet<String>,
    pub written: u64,
}

const MASK: WatchMask = WatchMask::CREATE.union(WatchMask::MODIFY).union(WatchMask::CLOSE_WRITE)
    .union(WatchMask::DELETE).union(WatchMask::MOVED_FROM).union(WatchMask::MOVED_TO)
    .union(WatchMask::DONT_FOLLOW).union(WatchMask::ONLYDIR).union(WatchMask::EXCL_UNLINK);

pub fn out_rel(run: &Path, out: &Path) -> Option<String> {
    rel_of(run, out)
}

/// One pass over the run dir: the content of every file in a root (stored as blobs), without
/// watching. For `scan`.
pub fn snapshot(run: &Path, out: &Path, blobs: &Blobs) -> HashMap<PathBuf, Known> {
    let orel = out_rel(run, out);
    let mut disk = HashMap::new();
    walk(run, run, orel.as_deref(), &mut |p, rel, is_dir| {
        if !is_dir && layout::root_of(&comps(rel), false).is_some() {
            if let Ok(Some(c)) = store::read_content(p) {
                disk.insert(p.to_path_buf(), blobs.keep(&c));
            }
        }
    });
    disk
}

impl Watcher {
    /// Watches, reads the case logs and streams, scans the roots, and replays what `out` knows. `out_dir` is the output
    /// directory `out`, held: every record and blob is read and written through it.
    pub fn start(run: PathBuf, out: PathBuf, out_dir: Held) -> std::io::Result<Watcher> {
        let blobs = Blobs::new(out_dir.sub("blobs")?);
        let (base, seq) = store::replay(&out_dir, &run, &blobs)?;
        let log = Log::open(&out_dir, "changes.jsonl")?;
        let mut w = Watcher {
            out_rel: out_rel(&run, &out), ino: Inotify::init()?, wds: HashMap::new(), shadow: HashMap::new(),
            blobs, log, seq, dirty: HashMap::new(), moves: HashMap::new(), pending: VecDeque::new(), late: Vec::new(),
            attr: Attributor::new(run.clone()), ord: 0, touched: HashSet::new(), written: 0, run, out_dir };
        let mut disk = HashMap::new();
        let run = w.run.clone();
        w.walk_live(&run, &mut |w, p, rel| {
            if layout::root_of(&comps(rel), false).is_some() {
                if let Ok(Some(c)) = store::read_content(p) {
                    disk.insert(p.to_path_buf(), w.blobs.keep(&c));
                }
            }
        });
        w.attr.associate_all(0);
        match base {
            None => {
                w.shadow = disk;
                store::write_snapshot(&w.out_dir, &w.run, &w.shadow, w.seq)?;
            }
            Some(base) => {
                // What changed while nobody watched is recorded at start.
                w.shadow = base;
                let mut paths: Vec<PathBuf> = w.shadow.keys().chain(disk.keys()).cloned().collect::<HashSet<_>>().into_iter().collect();
                paths.sort();
                for p in paths {
                    if w.shadow.get(&p).map(|k| &k.sha) != disk.get(&p).map(|k| &k.sha) {
                        w.mark(p, 0, None);
                    }
                }
            }
        }
        Ok(w)
    }

    pub fn tracked(&self) -> (usize, usize) {
        (self.shadow.len(), self.wds.len())
    }

    pub fn fd(&self) -> i32 {
        self.ino.as_raw_fd()
    }

    /// Walks `start`, watching every dir, registering agent call records, reading case logs and
    /// streams; `file` gets every other file.
    fn walk_live(&mut self, start: &Path, file: &mut dyn FnMut(&mut Watcher, &Path, &str)) {
        let out_rel = self.out_rel.clone();
        let mut seen: Vec<(PathBuf, String, bool)> = Vec::new();
        let run = self.run.clone();
        // Watch each dir before it is listed, so nothing created meanwhile is missed.
        walk(&run, start, out_rel.as_deref(), &mut |p, rel, is_dir| {
            if is_dir {
                match self.ino.watches().add(p, MASK) {
                    Ok(wd) => {
                        self.wds.insert(wd, p.to_path_buf());
                    }
                    Err(e) => eprintln!("sdc-watch: cannot watch {}: {e}", p.display()),
                }
            }
            seen.push((p.to_path_buf(), rel.to_string(), is_dir));
        });
        let ord = self.ord;
        for (p, rel, is_dir) in seen {
            if is_dir {
                if layout::is_record_dir(&comps(&rel)) {
                    self.attr.record(&rel, ord, ord > 0);
                }
                continue;
            }
            match source_of(&rel) {
                Some(Source::Case(d)) => self.attr.case_update(&d, ord),
                Some(Source::Stream(r)) => {
                    if self.attr.stream_update(&r, ord) {
                        self.touched.insert(r);
                    }
                }
                Some(Source::Rounds) => self.attr.steps_update(ord),
                Some(Source::Status(r)) => self.attr.status_update(&r, ord),
                None => file(self, &p, &rel),
            }
        }
    }

    fn mark(&mut self, p: PathBuf, ord: u64, from: Option<PathBuf>) {
        let now = Instant::now();
        let d = self.dirty.entry(p).or_insert_with(|| Dirty { first_ord: ord, first_at: now_ms(), first_seen: now, last: now, from: None });
        d.last = now;
        if from.is_some() {
            d.from = from;
        }
    }

    fn mark_under(&mut self, dir: &Path) {
        let ord = self.ord;
        let under: Vec<PathBuf> = self.shadow.keys().filter(|k| k.starts_with(dir)).cloned().collect();
        for k in under {
            self.mark(k, ord, None);
        }
    }

    fn forget_under(&mut self, dir: &Path, remove_watches: bool) {
        let gone: Vec<WatchDescriptor> = self.wds.iter().filter(|(_, p)| p.starts_with(dir)).map(|(w, _)| w.clone()).collect();
        for wd in gone {
            self.wds.remove(&wd);
            if remove_watches {
                let _ = self.ino.watches().remove(wd);
            }
        }
    }

    fn walk_new(&mut self, dir: &Path) {
        let ord = self.ord;
        self.walk_live(dir, &mut |w, p, rel| {
            if layout::root_of(&comps(rel), false).is_some() {
                w.mark(p.to_path_buf(), ord, None);
            }
        });
    }

    fn dir_moved(&mut self, src: &Path, dst: &Path) {
        let src_known = self.wds.values().any(|p| p == src);
        let dst_rel = rel_of(&self.run, dst).unwrap_or_default();
        if !src_known || layout::is_excluded(&comps(&dst_rel), self.out_rel.as_deref()) {
            if src_known {
                self.forget_under(src, true);
                self.mark_under(src);
            }
            self.walk_new(dst);
            return;
        }
        for p in self.wds.values_mut() {
            if let Ok(rest) = p.strip_prefix(src) {
                *p = dst.join(rest);
            }
        }
        let ord = self.ord;
        let moved: Vec<PathBuf> = self.shadow.keys().filter(|k| k.starts_with(src)).cloned().collect();
        for old in moved {
            let new = dst.join(old.strip_prefix(src).unwrap());
            self.mark(new, ord, Some(old.clone()));
            self.mark(old, ord, None);
        }
        self.walk_new(dst);
    }

    /// Reads every queued inotify event. Returns false when the queue overflowed (then rescans).
    pub fn drain(&mut self) {
        let mut buf = vec![0u8; 256 * 1024];
        let mut overflow = false;
        loop {
            let evs: Vec<(WatchDescriptor, EventMask, u32, Option<OsString>)> = match self.ino.read_events(&mut buf) {
                Ok(evs) => evs.map(|e| (e.wd, e.mask, e.cookie, e.name.map(|n| n.to_os_string()))).collect(),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    eprintln!("sdc-watch: inotify read: {e}");
                    break;
                }
            };
            if evs.is_empty() {
                break;
            }
            for (wd, mask, cookie, name) in evs {
                self.ord += 1;
                if mask.contains(EventMask::Q_OVERFLOW) {
                    overflow = true;
                    continue;
                }
                if mask.contains(EventMask::IGNORED) {
                    self.wds.remove(&wd);
                    continue;
                }
                let (Some(dir), Some(name)) = (self.wds.get(&wd).cloned(), name) else { continue };
                self.event(dir.join(name), mask, cookie);
            }
        }
        // A move whose other half never came left or entered the watched tree.
        for (_, (p, is_dir)) in std::mem::take(&mut self.moves) {
            if is_dir {
                self.forget_under(&p, true);
                self.mark_under(&p);
            }
        }
        if overflow {
            eprintln!("sdc-watch: inotify queue overflowed; rescanning");
            self.rescan();
        }
    }

    fn event(&mut self, p: PathBuf, mask: EventMask, cookie: u32) {
        let Some(rel) = rel_of(&self.run, &p) else { return };
        let c = comps(&rel);
        if layout::is_excluded(&c, self.out_rel.as_deref()) {
            return;
        }
        let ord = self.ord;
        if mask.contains(EventMask::ISDIR) {
            if mask.contains(EventMask::MOVED_FROM) {
                self.moves.insert(cookie, (p, true));
            } else if mask.contains(EventMask::MOVED_TO) {
                match self.moves.remove(&cookie) {
                    Some((src, true)) => self.dir_moved(&src, &p),
                    _ => self.walk_new(&p),
                }
            } else if mask.contains(EventMask::CREATE) {
                self.walk_new(&p);
            } else if mask.contains(EventMask::DELETE) {
                self.forget_under(&p, false);
                self.mark_under(&p);
            }
            return;
        }
        match source_of(&rel) {
            Some(Source::Case(d)) => return self.attr.case_update(&d, ord),
            Some(Source::Stream(r)) => {
                if self.attr.stream_update(&r, ord) {
                    self.touched.insert(r);
                }
                return;
            }
            Some(Source::Rounds) => return self.attr.steps_update(ord),
            Some(Source::Status(r)) => return self.attr.status_update(&r, ord),
            None => {}
        }
        if layout::root_of(&c, false).is_none() {
            return;
        }
        if mask.contains(EventMask::MOVED_FROM) {
            self.moves.insert(cookie, (p.clone(), false));
            self.mark(p, ord, None);
        } else if mask.contains(EventMask::MOVED_TO) {
            let from = match self.moves.remove(&cookie) {
                Some((src, false)) => Some(src),
                _ => None,
            };
            self.mark(p, ord, from);
        } else {
            self.mark(p, ord, None);
        }
    }

    fn rescan(&mut self) {
        let run = self.run.clone();
        let all: Vec<PathBuf> = self.shadow.keys().cloned().collect();
        let ord = self.ord;
        for k in all {
            self.mark(k, ord, None);
        }
        let cases: Vec<String> = self.attr.cases.keys().cloned().collect();
        for c in cases {
            self.attr.case_update(&c, ord);
        }
        self.attr.steps_update(ord);
        self.walk_new(&run);
    }

    /// When the next timer is due (`None`: nothing pending).
    pub fn next_wake(&self) -> Option<Instant> {
        let d = self.dirty.values().map(|d| (d.last + DEBOUNCE).min(d.first_seen + MAX_DELAY)).min();
        let p = self.pending.front().map(|p| p.deadline);
        match (d, p) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Writes every path that has settled (`all`: every dirty path).
    pub fn flush(&mut self, all: bool) {
        let now = Instant::now();
        let due: HashSet<PathBuf> = self.dirty.iter()
            .filter(|(_, d)| all || now >= d.last + DEBOUNCE || now >= d.first_seen + MAX_DELAY)
            .map(|(p, _)| p.clone()).collect();
        if due.is_empty() {
            return;
        }
        // A rename's source waits for its destination.
        let held: HashSet<PathBuf> = self.dirty.iter().filter(|(p, _)| !due.contains(*p))
            .filter_map(|(_, d)| d.from.clone()).collect();
        let mut batch: Vec<(PathBuf, Dirty)> = due.into_iter().filter(|p| !held.contains(p))
            .filter_map(|p| self.dirty.remove(&p).map(|d| (p, d))).collect();
        batch.sort_by(|a, b| (a.1.first_ord, &a.0).cmp(&(b.1.first_ord, &b.0)));
        let dest_of: HashMap<PathBuf, usize> = batch.iter().enumerate()
            .filter_map(|(i, (_, d))| d.from.clone().map(|f| (f, i))).collect();
        let index: HashMap<PathBuf, usize> = batch.iter().enumerate().map(|(i, (p, _))| (p.clone(), i)).collect();
        let mut done = vec![false; batch.len()];
        for i in 0..batch.len() {
            if done[i] {
                continue;
            }
            // A rename source is settled through its destination, in the source's place.
            let target = dest_of.get(&batch[i].0).copied().filter(|&j| !done[j]).unwrap_or(i);
            done[target] = true;
            let consumed = self.settle(&batch[target].0, &batch[target].1);
            if consumed {
                if let Some(&k) = batch[target].1.from.as_ref().and_then(|f| index.get(f)) {
                    done[k] = true;
                }
            }
            if !done[i] {
                done[i] = true;
                self.settle(&batch[i].0, &batch[i].1);
            }
        }
    }

    fn emit(&mut self, change: Change, root: &Root, ord: u64, p: &Path) {
        self.pending.push_back(Pending { change, root: root.clone(), ord, file_name: attrib::file_name(p),
            deadline: Instant::now() + HOLD });
    }

    /// Compares `p` with its shadow and queues the change, if any. Returns whether it consumed a
    /// rename source.
    fn settle(&mut self, p: &Path, d: &Dirty) -> bool {
        let Some(rel) = rel_of(&self.run, p) else { return false };
        let c = comps(&rel);
        let Some(root) = layout::root_of(&c, false) else { return false };
        let in_root = |r: &str| r.strip_prefix(&format!("{}/", root.rel)).unwrap_or(r).to_string();
        let path = in_root(&rel);
        let Ok(new) = store::read_content(p) else { return false };
        let new: Option<Content> = new;
        let old = self.shadow.get(p).cloned();
        if let (Some(src), Some(content)) = (&d.from, &new) {
            let src_rel = rel_of(&self.run, src).unwrap_or_default();
            let same_root = layout::root_of(&comps(&src_rel), false).is_some_and(|r| r == root);
            if let (true, Some(sk), false) = (same_root, self.shadow.get(src).cloned(), src.exists()) {
                if let Some(o) = &old {
                    let ch = store::build_change(&self.blobs, d.first_at, &root.rel, &path, "delete", None, Some(o), None);
                    self.emit(ch, &root, d.first_ord, p);
                }
                let from = in_root(&src_rel);
                let ch = store::build_change(&self.blobs, d.first_at, &root.rel, &path, "rename", Some(&from), Some(&sk), Some(content));
                self.emit(ch, &root, d.first_ord, p);
                self.shadow.remove(src);
                let k = self.blobs.keep(content);
                self.shadow.insert(p.to_path_buf(), k);
                self.dirty.remove(src);
                return true;
            }
        }
        let op = match (&old, &new) {
            (None, None) => return false,
            (None, Some(_)) => "create",
            (Some(_), None) => "delete",
            (Some(o), Some(n)) if o.sha != n.sha => "modify",
            _ => return false,
        };
        let ch = store::build_change(&self.blobs, d.first_at, &root.rel, &path, op, None, old.as_ref(), new.as_ref());
        self.emit(ch, &root, d.first_ord, p);
        match &new {
            Some(n) => {
                let k = self.blobs.keep(n);
                self.shadow.insert(p.to_path_buf(), k);
            }
            None => {
                self.shadow.remove(p);
            }
        }
        false
    }

    /// Writes queued changes in order, each once its attribution is settled or its hold is over.
    pub fn write(&mut self, force: bool) {
        while let Some(p) = self.pending.front() {
            let f = self.attr.attribute(&p.root, p.ord, p.change.at, &p.file_name);
            if !f.resolved && !force && Instant::now() < p.deadline {
                break;
            }
            let p = self.pending.pop_front().unwrap();
            let seq = self.seq;
            self.seq += 1;
            self.written += 1;
            self.log.append(&store::change_json(seq, &p.change, f.attribution));
            if let (false, Some(record)) = (f.resolved, f.record) {
                self.late.push(Late { seq, record, root: p.root, ord: p.ord, at: p.change.at, file_name: p.file_name,
                    until: Instant::now() + LATE });
            }
        }
    }

    /// A stream that caught up after a change was written: append its tool attribution.
    pub fn late(&mut self) {
        let touched = std::mem::take(&mut self.touched);
        if self.late.is_empty() {
            return;
        }
        let now = Instant::now();
        let mut keep = Vec::new();
        for l in std::mem::take(&mut self.late) {
            if now > l.until {
                continue;
            }
            if !touched.contains(&l.record) {
                keep.push(l);
                continue;
            }
            let f = self.attr.attribute(&l.root, l.ord, l.at, &l.file_name);
            if f.has_tool {
                self.log.append(&json!({"format": "sdc.fs-attribution/1", "change": l.seq, "at": fmt_ms(now_ms()),
                    "attribution": f.attribution}));
            } else if !f.resolved {
                keep.push(l);
            }
        }
        self.late = keep;
    }

    /// Everything still in flight goes out.
    pub fn finish(&mut self) {
        self.drain();
        self.flush(true);
        self.write(true);
        self.late();
    }
}
