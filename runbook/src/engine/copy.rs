//! The one routine that copies a directory of a runbook: `runbook run` copies its bindings with it
//! (links stay links, `Links::Keep`) and the daemon copies the runbook of a run with it (only
//! regular files whose real path lies inside the runbook home, `Links::Within`). The digest of a
//! directory is here too: the daemon names it in the run's record and checks the copy against it.

use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// How deep a copied directory may nest, links followed (the daemon's copy; a link that points back
/// up is refused by it). The copy of `runbook run` keeps links as links and has no depth bound.
const MAX_DEPTH: usize = 24;

/// The most entries (directories and files, a link counted as what it is copied as) the daemon's copy
/// takes. The runbook home of this repository holds 661 (`bindings/`, `protocol/`,
/// `instructions/`, `views/`, `driver/examples/`, `tools/`): the bound is about six times that.
pub const MAX_ENTRIES: usize = 4096;

/// The most bytes (the lengths of the files) the daemon's copy takes. The same home holds about
/// 1.1 MB, its largest file 37 KB: the bound is about fourteen times the home.
pub const MAX_BYTES: u64 = 16 << 20;

/// What a link in the source is.
#[derive(Clone, Copy)]
pub enum Links<'a> {
    /// A link is made again as a link, whatever it points at. Files keep the mode they had (and are
    /// writable); nothing is bounded.
    Keep,
    /// A link is followed, and only a file or directory whose real path lies below this canonical
    /// directory is copied, as what it is. Anything that is not a regular file or a directory is
    /// refused. The copy is bounded (`MAX_ENTRIES`, `MAX_BYTES`, `MAX_DEPTH`), it is private to its
    /// owner (directories `0o700`, files `0o600`, or `0o700` when the source has an executable bit),
    /// and a source that is not the file the plan saw refuses the whole copy.
    Within(&'a Path),
}

/// What the plan saw of a file it will copy.
#[derive(Clone, Copy)]
struct Seen {
    dev: u64,
    ino: u64,
    len: u64,
    exec: bool,
}

enum Entry {
    Dir(PathBuf),
    File { from: PathBuf, to: PathBuf, seen: Option<Seen> },
    Link { target: PathBuf, to: PathBuf },
}

/// What the plan has counted.
struct Budget {
    bounded: bool,
    entries: usize,
    bytes: u64,
}

impl Budget {
    fn charge(&mut self, bytes: u64) -> Result<(), String> {
        self.entries += 1;
        self.bytes = self.bytes.saturating_add(bytes);
        if self.bounded && (self.entries > MAX_ENTRIES || self.bytes > MAX_BYTES) {
            return Err(format!("the runbook is over the bound of {MAX_ENTRIES} entries or {MAX_BYTES} bytes"));
        }
        Ok(())
    }
}

/// Opens a file for reading without following a link in its last component and without waiting on a
/// pipe: what is opened is checked by the caller to be a regular file.
fn open_regular(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(path)
}

/// How a refusal names `path`: for the daemon's copy relative to the home (the directory itself is
/// `.`; a path that is not below the home is named by its last component), for `runbook run` as given.
fn named(links: Links<'_>, path: &Path) -> String {
    match links {
        Links::Keep => path.display().to_string(),
        Links::Within(root) => match path.strip_prefix(root) {
            Ok(rel) if rel.as_os_str().is_empty() => ".".to_string(),
            Ok(rel) => rel.display().to_string(),
            Err(_) => path.file_name().map_or_else(|| ".".to_string(), |name| name.to_string_lossy().into_owned()),
        },
    }
}

/// Copies each `(source, destination)` directory. Every refusal of the plan comes before the first
/// write: when it refuses, no destination was created. A `Within` copy that fails while it writes
/// removes the destinations it created.
pub fn copy_dirs(jobs: &[(&Path, &Path)], links: Links<'_>) -> Result<(), String> {
    let mut entries = Vec::new();
    let mut budget = Budget { bounded: matches!(links, Links::Within(_)), entries: 0, bytes: 0 };
    for (src, dst) in jobs {
        match links {
            Links::Keep => plan(src, dst, links, 0, &mut budget, &mut entries)?,
            Links::Within(root) => {
                let real = std::fs::canonicalize(src).map_err(|e| format!("{}: {e}", named(links, src)))?;
                if !real.starts_with(root) {
                    return Err(format!("{}: a link that leaves the runbook home", named(links, src)));
                }
                plan(&real, dst, links, 0, &mut budget, &mut entries)?
            }
        }
    }
    let created: Vec<&Path> = jobs.iter().map(|(_, dst)| *dst).filter(|dst| std::fs::symlink_metadata(dst).is_err()).collect();
    let written = write(entries, links);
    if written.is_err() && matches!(links, Links::Within(_)) {
        for dst in created {
            let _ = std::fs::remove_dir_all(dst);
        }
    }
    written
}

fn write(entries: Vec<Entry>, links: Links<'_>) -> Result<(), String> {
    for entry in entries {
        match entry {
            Entry::Dir(dir) => match links {
                Links::Keep => std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?,
                Links::Within(_) => std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&dir).map_err(|e| format!("the copy cannot be written: {e}"))?,
            },
            Entry::Link { target, to } => std::os::unix::fs::symlink(&target, &to).map_err(|e| format!("{}: {e}", to.display()))?,
            Entry::File { from, to, seen: None } => {
                std::fs::copy(&from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
                // A read-only source is copied writable: the copy is the run's.
                let mut permissions = std::fs::metadata(&to).map_err(|e| format!("{}: {e}", to.display()))?.permissions();
                permissions.set_mode(permissions.mode() | 0o200);
                std::fs::set_permissions(&to, permissions).map_err(|e| format!("{}: {e}", to.display()))?;
            }
            Entry::File { from, to, seen: Some(seen) } => copy_seen(&from, &to, seen)?,
        }
    }
    Ok(())
}

/// Copies the file the plan saw: the source is opened without following a link and must be that
/// file (same device, inode and length); the copy is private to its owner.
fn copy_seen(from: &Path, to: &Path, seen: Seen) -> Result<(), String> {
    let changed = || "a file of the runbook home changed while it was copied".to_string();
    let mut source = open_regular(from).map_err(|_| changed())?;
    let meta = source.metadata().map_err(|_| changed())?;
    if !meta.is_file() || meta.dev() != seen.dev || meta.ino() != seen.ino || meta.len() != seen.len {
        return Err(changed());
    }
    let mode = if seen.exec { 0o700 } else { 0o600 };
    let mut out = OpenOptions::new().write(true).create_new(true).mode(mode).open(to).map_err(|e| format!("the copy cannot be written: {e}"))?;
    let copied = std::io::copy(&mut (&mut source).take(seen.len), &mut out).map_err(|e| format!("the copy cannot be written: {e}"))?;
    if copied != seen.len {
        return Err(changed());
    }
    out.set_permissions(std::fs::Permissions::from_mode(mode)).map_err(|e| format!("the copy cannot be written: {e}"))
}

fn plan(src: &Path, dst: &Path, links: Links<'_>, depth: usize, budget: &mut Budget, out: &mut Vec<Entry>) -> Result<(), String> {
    if matches!(links, Links::Within(_)) && depth > MAX_DEPTH {
        return Err(format!("{}: nested deeper than {MAX_DEPTH} levels, or a link that loops", named(links, src)));
    }
    budget.charge(0)?;
    out.push(Entry::Dir(dst.to_path_buf()));
    let entries = std::fs::read_dir(src).map_err(|e| format!("{}: {e}", named(links, src)))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", named(links, src)))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let kind = entry.file_type().map_err(|e| format!("{}: {e}", named(links, &from)))?;
        match links {
            Links::Keep if kind.is_symlink() => {
                // A link stays a link: the fixtures hold links to directories and to programs.
                let target = std::fs::read_link(&from).map_err(|e| format!("{}: {e}", from.display()))?;
                out.push(Entry::Link { target, to });
            }
            Links::Keep if kind.is_dir() => plan(&from, &to, links, depth + 1, budget, out)?,
            Links::Keep => out.push(Entry::File { from, to, seen: None }),
            Links::Within(root) => {
                let real = if kind.is_symlink() {
                    let real = std::fs::canonicalize(&from).map_err(|e| format!("{}: {e}", named(links, &from)))?;
                    if !real.starts_with(root) {
                        return Err(format!("{}: a link that leaves the runbook home", named(links, &from)));
                    }
                    real
                } else {
                    from.clone()
                };
                let meta = std::fs::symlink_metadata(&real).map_err(|e| format!("{}: {e}", named(links, &from)))?;
                if meta.is_dir() {
                    plan(&real, &to, links, depth + 1, budget, out)?;
                } else if meta.is_file() {
                    budget.charge(meta.len())?;
                    out.push(Entry::File { from: real, to, seen: Some(Seen { dev: meta.dev(), ino: meta.ino(), len: meta.len(), exec: meta.mode() & 0o111 != 0 }) });
                } else {
                    return Err(format!("{}: not a regular file or a directory", named(links, &from)));
                }
            }
        }
    }
    Ok(())
}

/// The sha256 (hex) over a directory: for each entry in name order its kind, its relative path
/// (the raw bytes of the names) and, for a file, its length, whether it has an executable bit (any of
/// the three, so that the group and other bits of a source change nothing) and its bytes, streamed
/// through the hasher; for a link, its target (raw bytes). A file edited, added, removed, made
/// executable or replaced by a link changes it. A failure names the entry relative to `dir`.
pub fn digest(dir: &Path) -> Result<String, String> {
    match std::fs::symlink_metadata(dir) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => return Err(".: not a directory".to_string()),
        Err(e) => return Err(format!(".: {e}")),
    }
    let mut hasher = Sha256::new();
    walk(dir, dir, &mut hasher)?;
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn shown(base: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(base).unwrap_or(path);
    if rel.as_os_str().is_empty() {
        ".".to_string()
    } else {
        rel.display().to_string()
    }
}

fn walk(base: &Path, dir: &Path, hasher: &mut Sha256) -> Result<(), String> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", shown(base, dir)))? {
        names.push(entry.map_err(|e| format!("{}: {e}", shown(base, dir)))?.file_name());
    }
    names.sort();
    for name in names {
        let path = dir.join(&name);
        let rel = path.strip_prefix(base).unwrap_or(&path);
        let at = |e: std::io::Error| format!("{}: {e}", rel.display());
        let meta = std::fs::symlink_metadata(&path).map_err(at)?;
        let kind: &[u8] = if meta.file_type().is_symlink() {
            b"L"
        } else if meta.is_dir() {
            b"D"
        } else if meta.is_file() {
            b"F"
        } else {
            b"O"
        };
        hasher.update(kind);
        hasher.update([0]);
        hasher.update(rel.as_os_str().as_bytes());
        hasher.update([0]);
        if meta.file_type().is_symlink() {
            let target = std::fs::read_link(&path).map_err(at)?;
            hasher.update(target.as_os_str().as_bytes());
        } else if meta.is_dir() {
            walk(base, &path, hasher)?;
        } else if meta.is_file() {
            hash_file(&path, rel, hasher)?;
        }
        hasher.update([0]);
    }
    Ok(())
}

/// A file of the directory, read through the descriptor it was opened with, in blocks.
fn hash_file(path: &Path, rel: &Path, hasher: &mut Sha256) -> Result<(), String> {
    let bad = |what: &str| format!("{}: {what}", rel.display());
    let mut file = open_regular(path).map_err(|e| bad(&e.to_string()))?;
    let meta = file.metadata().map_err(|e| bad(&e.to_string()))?;
    if !meta.is_file() {
        return Err(bad("not a regular file"));
    }
    let len = meta.len();
    hasher.update(len.to_string().as_bytes());
    hasher.update([0, u8::from(meta.mode() & 0o111 != 0), 0]);
    let mut block = vec![0u8; 64 << 10];
    let mut left = len;
    while left > 0 {
        let want = usize::try_from(left).unwrap_or(usize::MAX).min(block.len());
        let n = file.read(&mut block[..want]).map_err(|e| bad(&e.to_string()))?;
        if n == 0 {
            return Err(bad("changed while it was read"));
        }
        hasher.update(&block[..n]);
        left -= n as u64;
    }
    let mut more = [0u8; 1];
    if file.read(&mut more).map_err(|e| bad(&e.to_string()))? != 0 {
        return Err(bad("changed while it was read"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::current_exe().unwrap().parent().unwrap().join(format!("engine-copy-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::canonicalize(d).unwrap()
    }

    fn home(tag: &str) -> PathBuf {
        let home = dir(tag);
        std::fs::create_dir_all(home.join("bindings/tools")).unwrap();
        std::fs::write(home.join("bindings/a.yaml"), "a: 1\n").unwrap();
        std::fs::write(home.join("bindings/tools/run.sh"), "#!/bin/sh\n").unwrap();
        home
    }

    #[test]
    fn the_digest_changes_with_an_edit_an_addition_a_removal_and_a_link_and_not_otherwise() {
        let home = home("digest");
        let at = |home: &Path| digest(&home.join("bindings")).unwrap();
        let first = at(&home);
        assert_eq!(first, at(&home));
        std::fs::write(home.join("bindings/a.yaml"), "a: 2\n").unwrap();
        let edited = at(&home);
        assert_ne!(first, edited);
        std::fs::write(home.join("bindings/b.yaml"), "").unwrap();
        let added = at(&home);
        assert_ne!(edited, added);
        std::fs::remove_file(home.join("bindings/b.yaml")).unwrap();
        assert_eq!(edited, at(&home));
        std::fs::remove_file(home.join("bindings/tools/run.sh")).unwrap();
        let removed = at(&home);
        assert_ne!(edited, removed);
        std::os::unix::fs::symlink("a.yaml", home.join("bindings/tools/run.sh")).unwrap();
        assert_ne!(removed, at(&home));
    }

    #[test]
    fn a_link_that_leaves_the_home_is_refused_before_anything_is_written() {
        let home = home("leaves");
        let outside = dir("leaves-outside");
        std::fs::write(outside.join("secret"), "x").unwrap();
        std::os::unix::fs::symlink(outside.join("secret"), home.join("bindings/tools/leak")).unwrap();
        let target = dir("leaves-target");
        let (a, b) = (target.join("bindings"), target.join("again"));
        let refused = copy_dirs(&[(&home.join("bindings"), &a), (&home.join("bindings"), &b)], Links::Within(&home)).unwrap_err();
        assert!(refused.contains("leaves the runbook home"), "{refused}");
        assert!(!a.exists() && !b.exists(), "a destination was made");
        // `runbook run` keeps a link as it is.
        copy_dirs(&[(&home.join("bindings"), &a)], Links::Keep).unwrap();
        assert!(std::fs::symlink_metadata(a.join("tools/leak")).unwrap().file_type().is_symlink());
    }

    #[test]
    fn a_link_inside_the_home_is_copied_as_the_file_and_a_loop_is_refused() {
        let home = home("inside");
        std::os::unix::fs::symlink("run.sh", home.join("bindings/tools/again.sh")).unwrap();
        let target = dir("inside-target");
        let dst = target.join("bindings");
        copy_dirs(&[(&home.join("bindings"), &dst)], Links::Within(&home)).unwrap();
        let meta = std::fs::symlink_metadata(dst.join("tools/again.sh")).unwrap();
        assert!(meta.is_file() && !meta.file_type().is_symlink());
        std::os::unix::fs::symlink("..", home.join("bindings/tools/up")).unwrap();
        let refused = copy_dirs(&[(&home.join("bindings"), &target.join("loop"))], Links::Within(&home)).unwrap_err();
        assert!(refused.contains("loops"), "{refused}");
        assert!(!target.join("loop").exists());
    }

    #[test]
    fn a_source_that_is_a_link_out_of_the_home_is_refused() {
        let home = home("source");
        let outside = dir("source-outside");
        std::fs::remove_dir_all(home.join("bindings")).unwrap();
        std::os::unix::fs::symlink(&outside, home.join("bindings")).unwrap();
        let target = dir("source-target");
        let refused = copy_dirs(&[(&home.join("bindings"), &target.join("bindings"))], Links::Within(&home)).unwrap_err();
        assert!(refused.contains("leaves the runbook home"), "{refused}");
        assert!(!target.join("bindings").exists());
    }
}
