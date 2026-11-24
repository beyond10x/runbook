//! The one writer of the records of a run directory, the one appender, the one maker of its directories and the one
//! reader of the names in it.
//!
//! The run directory is written by the programs of a run too (`--confinement local`), so no name in it is trusted: a
//! program can leave a link, a pipe, a directory or a second name of a file outside the run at any name the runner will
//! write, make or read. Every record the runner writes into the run directory after its first program started goes
//! through this file, every directory it makes there is made by [`make_dir_in`], and every file there that it reads is
//! read by [`read_regular`], [`read_regular_text`], [`read_regular_prefix`], [`read_regular_within`] or through a held
//! directory ([`Held::read`], [`Held::read_text`], [`Held::copy_to`]). The bindings copy is held for every read and
//! write after the first program started ([`Held::open`], listed by [`Held::names`]), so a link at its name ends the
//! run.
//!
//! Outside this file, and named here so that the sentence above stays true: the runner lists the call directories of
//! the run by path (`cost.rs`, `dry.rs`, `decider/approval.rs` `record_dirs`, `run_dir.rs`), which reads names, not
//! files; it reads the bindings copy (`bindings_copy.rs`, `answers.rs` `Answers::load`) and copies its protocols and
//! the idea before the first program starts; the inputs of a decider call are copied by `decider/inputs.rs` before the
//! program of that call starts; and Loom's case store of the run (`store/`) is read by the in-process engine
//! (`runbook::engine`) with Loom's own readers. The watcher (`watch/`) is its own program and holds its output
//! directory and its reads itself (`docs/loom-run-directory.md`).
//!
//! A directory the runner keeps writing to while a program runs (the directory of a decider call, which stays in use
//! while the adapter runs) is held: [`Held`] keeps the open descriptor [`make_dir_in`] returned, and every record,
//! directory, link and removal in it is made relative to that descriptor, so the writes after the program ran go to the
//! directory the runner made, whatever stands at its path by then. A read opens its file by path and holds the file, not
//! the directory above it.
//!
//! This file uses `std` and `libc` and nothing else of the crate, so that the tests of the integration-test binary (there
//! is no library target) can include it as it is, with `run_dir.rs`.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path};
use std::sync::atomic::{AtomicU64, Ordering};

/// How many temporary names one write tries before it ends with an error. No try waits, so a write ends within
/// microseconds, whatever a program left in the directory.
pub(crate) const NAMES_TRIED: usize = 16;

/// How many bytes a read of a name in the run directory takes: 64 MiB. A file that holds more is not read and the read
/// ends with an error that names the file in the run.
pub(crate) const READ_LIMIT: u64 = 64 * 1024 * 1024;

/// Makes the directory `dir` below the directory `base`, and every directory between, without following a link, and
/// returns the open directory.
///
/// `base` is a directory the runner trusts at this moment (the run directory, or a directory of it that was made by this
/// function a moment ago). Each component of `dir` below `base` is made with `mkdirat` when it is missing and is then
/// opened with `O_NOFOLLOW | O_DIRECTORY` relative to the directory before it: a link, a file or anything else that is not
/// a directory at a component ends the call with an error that names the path inside `base` and no path outside it, and
/// nothing is made beyond it. A directory that is made by the call has the mode `0o777` less the umask, as
/// `create_dir_all` makes it. `dir` that does not lie below `base` is an error.
pub(crate) fn make_dir_in(base: &Path, dir: &Path) -> Result<File, String> {
    use std::os::unix::fs::OpenOptionsExt;
    let rel = dir.strip_prefix(base).map_err(|_| format!("{}: not a directory of {}", dir.display(), base.display()))?;
    let open = OpenOptions::new().read(true).custom_flags(libc::O_DIRECTORY).open(base).map_err(|e| format!("{}: {e}", base.display()))?;
    make_dir_at(&open, base, rel)
}

/// [`make_dir_in`] below a directory that is already open: the directories `rel` below the open directory `base` are
/// made and opened as there, and the open directory is returned. `shown` is the path of `base`, for the messages only;
/// nothing is opened by it. A directory that is renamed or replaced after `base` was opened is not found again: the walk
/// starts at the descriptor.
pub(crate) fn make_dir_at(base: &File, shown: &Path, rel: &Path) -> Result<File, String> {
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    let mut current = base.try_clone().map_err(|e| format!("{}: {e}", shown.display()))?;
    let mut walked = shown.to_path_buf();
    for part in rel.components() {
        let name = match part {
            Component::Normal(name) => name,
            Component::CurDir => continue,
            _ => return Err(format!("{}: not a plain path below {}", shown.join(rel).display(), shown.display())),
        };
        walked.push(name);
        let c_name = std::ffi::CString::new(name.as_bytes()).map_err(|_| format!("{}: a name with a NUL byte", walked.display()))?;
        let fd = current.as_raw_fd();
        // SAFETY: `fd` is an open directory of this function and `c_name` is a NUL-terminated name that outlives the call.
        if unsafe { libc::mkdirat(fd, c_name.as_ptr(), 0o777) } != 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(format!("{}: {e}", walked.display()));
            }
        }
        // SAFETY: as above.
        let next = unsafe { libc::openat(fd, c_name.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC) };
        if next < 0 {
            let e = std::io::Error::last_os_error();
            return Err(match e.raw_os_error() {
                Some(libc::ELOOP | libc::ENOTDIR) => format!("{}: not a directory (a link or a file lies there)", walked.display()),
                _ => format!("{}: {e}", walked.display()),
            });
        }
        // SAFETY: `next` is a new descriptor that nothing else owns.
        current = unsafe { File::from_raw_fd(next) };
    }
    Ok(current)
}

/// Opens `path` for reading as a regular file with one name: no link is followed at its last component, a pipe is not
/// waited for, and anything that is not a regular file with one name (a pipe, a device, a directory, a second name of a
/// file that may lie outside the run) is an error that names `path` and no other path.
fn open_regular(path: &Path) -> Result<File, String> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    check_regular(file, path)
}

/// [`open_regular`] of the name `leaf` in the open directory `dir` (`openat`); `shown` is the path of the directory.
fn open_regular_at(dir: &File, shown: &Path, leaf: &std::ffi::OsStr) -> Result<File, String> {
    use std::os::fd::{AsRawFd, FromRawFd};
    let path = shown.join(leaf);
    let c_leaf = leaf_name(leaf, shown)?;
    // SAFETY: `dir` is an open directory for the call and `c_leaf` is a NUL-terminated name that outlives it.
    let raw = unsafe { libc::openat(dir.as_raw_fd(), c_leaf.as_ptr(), libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC) };
    if raw < 0 {
        return Err(format!("{}: {}", path.display(), std::io::Error::last_os_error()));
    }
    // SAFETY: `raw` is a new descriptor that nothing else owns.
    check_regular(unsafe { File::from_raw_fd(raw) }, &path)
}

fn check_regular(file: File, path: &Path) -> Result<File, String> {
    use std::os::unix::fs::MetadataExt;
    let meta = file.metadata().map_err(|e| format!("{}: {e}", path.display()))?;
    if !meta.is_file() || meta.nlink() != 1 {
        return Err(format!("{}: not a regular file with one name", path.display()));
    }
    Ok(file)
}

/// The bytes of the regular file with one name at `path`, at most [`READ_LIMIT`] of them.
pub(crate) fn read_regular(path: &Path) -> Result<Vec<u8>, String> {
    read_regular_within(path, READ_LIMIT)
}

/// The bytes of the regular file with one name at `path`: a file that holds more than `limit` bytes is not read and the
/// answer is an error that names `path` (at most `limit` plus one byte is taken from it, whatever its size becomes).
pub(crate) fn read_regular_within(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    read_open_within(open_regular(path)?, path, limit)
}

fn read_open_within(file: File, path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1)).read_to_end(&mut bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() as u64 > limit {
        return Err(format!("{}: more than {limit} bytes", path.display()));
    }
    Ok(bytes)
}

/// The text of the regular file with one name at `path` ([`read_regular`]); a file that is not UTF-8 is an error.
pub(crate) fn read_regular_text(path: &Path) -> Result<String, String> {
    String::from_utf8(read_regular(path)?).map_err(|_| format!("{}: not UTF-8 text", path.display()))
}

/// The first `limit` bytes of the regular file with one name at `path`; the rest of a longer file is not read.
pub(crate) fn read_regular_prefix(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let file = open_regular(path)?;
    let mut bytes = Vec::new();
    file.take(limit).read_to_end(&mut bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(bytes)
}

/// Writes `bytes` to `path` through a temporary file and a rename, so a reader never sees half a file.
///
/// The temporary file is created as new (`O_EXCL`: nothing at the name is followed, opened or written through) under a dot
/// name the writer chooses for this one write (the process id and a counter), and the rename replaces whatever lies at
/// `path`, a link or a pipe too. A name that is taken is not used: the next one is tried, up to [`NAMES_TRIED`] names, and
/// then the write ends with an error that names the last temporary file, which lies beside `path`. Nothing is written
/// outside the directory of `path`.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_atomic_mode(path, bytes, 0o644)
}

/// [`write_atomic`] with the permission bits `mode` for the new file, for a copy that keeps the bits of its source.
///
/// The directory of `path` is opened once and the write goes through that descriptor ([`Held::write`]); a caller that
/// holds the directory of a record itself writes through [`Held`], so that a replacement of the directory after it was made
/// is not followed.
pub(crate) fn write_atomic_mode(path: &Path, bytes: &[u8], mode: u32) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let parent = path.parent().unwrap_or(Path::new(""));
    let leaf = path.file_name().unwrap_or_default();
    let open = OpenOptions::new().read(true).custom_flags(libc::O_DIRECTORY).open(if parent.as_os_str().is_empty() { Path::new(".") } else { parent }).map_err(|e| format!("{}: {e}", path.display()))?;
    write_atomic_at(&open, parent, leaf, bytes, mode)
}

/// A name that is one plain component: not empty, no `/`, not `.` or `..`, no NUL byte.
fn leaf_name(leaf: &std::ffi::OsStr, shown: &Path) -> Result<std::ffi::CString, String> {
    use std::os::unix::ffi::OsStrExt;
    if leaf.is_empty() || leaf == "." || leaf == ".." || leaf.as_bytes().contains(&b'/') {
        return Err(format!("{}: not a plain file name", shown.join(leaf).display()));
    }
    std::ffi::CString::new(leaf.as_bytes()).map_err(|_| format!("{}: a name with a NUL byte", shown.join(leaf).display()))
}

/// The write of [`write_atomic`] into the directory `dir`, which is open: the temporary file is made with `openat` and the
/// rename is `renameat`, both relative to the descriptor. `shown` is the path of the directory, for the messages only.
fn write_atomic_at(dir: &File, shown: &Path, leaf: &std::ffi::OsStr, bytes: &[u8], mode: u32) -> Result<(), String> {
    use std::os::fd::{AsRawFd, FromRawFd};
    static WRITES: AtomicU64 = AtomicU64::new(0);
    let c_leaf = leaf_name(leaf, shown)?;
    let fd = dir.as_raw_fd();
    let mut last = String::new();
    for _ in 0..NAMES_TRIED {
        let mut tmp_name = std::ffi::OsString::from(".");
        tmp_name.push(leaf);
        tmp_name.push(format!(".{}.{}.tmp", std::process::id(), WRITES.fetch_add(1, Ordering::Relaxed)));
        let tmp = shown.join(&tmp_name);
        let c_tmp = leaf_name(&tmp_name, shown)?;
        // SAFETY: `fd` is an open directory that `dir` keeps open for the call, and `c_tmp` is a NUL-terminated name that outlives it.
        let raw = unsafe { libc::openat(fd, c_tmp.as_ptr(), libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC, mode as libc::c_uint) };
        if raw < 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                last = format!("{}: {e}", tmp.display());
                continue;
            }
            return Err(format!("{}: {e}", tmp.display()));
        }
        // SAFETY: `raw` is a new descriptor that nothing else owns.
        let mut file = unsafe { File::from_raw_fd(raw) };
        let written = file.write_all(bytes).and_then(|()| file.flush()).map_err(|e| format!("{}: {e}", tmp.display()));
        drop(file);
        let renamed = written.and_then(|()| {
            // SAFETY: as above, for both names.
            if unsafe { libc::renameat(fd, c_tmp.as_ptr(), fd, c_leaf.as_ptr()) } == 0 {
                Ok(())
            } else {
                Err(format!("{}: {}", shown.join(leaf).display(), std::io::Error::last_os_error()))
            }
        });
        if renamed.is_err() {
            // SAFETY: as above.
            unsafe { libc::unlinkat(fd, c_tmp.as_ptr(), 0) };
        }
        return renamed;
    }
    Err(last)
}

/// Copies the file `from` to `to` through [`write_atomic_mode`], with the permission bits of `from`. A link or a pipe at `to`
/// is replaced, never written through. `from` is read as every name of the run is ([`read_regular_within`] with
/// [`READ_LIMIT`]): a link, a pipe or a file with a second name at `from` is an error that names `from`, and nothing is
/// written.
#[cfg(test)]
pub(crate) fn copy_atomic(from: &Path, to: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let file = open_regular(from)?;
    let mode = file.metadata().map(|m| m.permissions().mode() & 0o777).unwrap_or(0o644);
    let bytes = read_open_within(file, from, READ_LIMIT)?;
    write_atomic_mode(to, &bytes, mode)
}

/// A directory of the run, made by [`make_dir_in`] and held open: every record the runner writes into it afterwards goes
/// through the descriptor (`openat`, `renameat`, `unlinkat`, `symlinkat` relative to it), never through its path. A program
/// of the run can rename the directory and put a link, or another directory, at its name while the runner waits for an
/// adapter; the runner's later writes then still reach the directory it made, which is a directory of the run, and nothing
/// reaches the place the link points at. The path is kept for the messages and for what the runner tells a program.
pub(crate) struct Held {
    dir: File,
    path: std::path::PathBuf,
}

impl Held {
    /// The directory `dir` below `base`, made and opened by [`make_dir_in`].
    pub(crate) fn make(base: &Path, dir: &Path) -> Result<Held, String> {
        Ok(Held { dir: make_dir_in(base, dir)?, path: dir.to_path_buf() })
    }

    /// The directory `dir`, which must exist, opened and held: its parent is opened by path and `dir` relative to it with
    /// `O_NOFOLLOW | O_DIRECTORY`, so a link, a file or anything else that is not a directory at its name is an error that
    /// names `dir`, and nothing is made.
    pub(crate) fn open(dir: &Path) -> Result<Held, String> {
        use std::os::fd::{AsRawFd, FromRawFd};
        use std::os::unix::fs::OpenOptionsExt;
        let parent = dir.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let leaf = dir.file_name().ok_or_else(|| format!("{}: not a directory name", dir.display()))?;
        let c_leaf = leaf_name(leaf, parent)?;
        let above = OpenOptions::new().read(true).custom_flags(libc::O_DIRECTORY).open(parent).map_err(|e| format!("{}: {e}", dir.display()))?;
        // SAFETY: `above` is an open directory for the call and `c_leaf` is a NUL-terminated name that outlives it.
        let raw = unsafe { libc::openat(above.as_raw_fd(), c_leaf.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC) };
        if raw < 0 {
            let e = std::io::Error::last_os_error();
            return Err(match e.raw_os_error() {
                Some(libc::ELOOP | libc::ENOTDIR) => format!("{}: not a directory (a link or a file lies there)", dir.display()),
                _ => format!("{}: {e}", dir.display()),
            });
        }
        // SAFETY: `raw` is a new descriptor that nothing else owns.
        Ok(Held { dir: unsafe { File::from_raw_fd(raw) }, path: dir.to_path_buf() })
    }

    /// The names in this directory, listed through the descriptor (`.` and `..` left out), sorted.
    pub(crate) fn names(&self) -> Result<Vec<std::ffi::OsString>, String> {
        use std::os::fd::AsRawFd;
        use std::os::unix::ffi::OsStrExt;
        // SAFETY: `dup` of a descriptor `self` keeps open; the new descriptor is handed to `fdopendir`, which owns it from then on.
        let fd = unsafe { libc::dup(self.dir.as_raw_fd()) };
        if fd < 0 {
            return Err(format!("{}: {}", self.path.display(), std::io::Error::last_os_error()));
        }
        // SAFETY: `fd` is a new descriptor of this directory; on failure it is closed here.
        let stream = unsafe { libc::fdopendir(fd) };
        if stream.is_null() {
            let e = std::io::Error::last_os_error();
            unsafe { libc::close(fd) };
            return Err(format!("{}: {e}", self.path.display()));
        }
        // SAFETY: the stream starts at the first entry whatever an earlier listing of the same directory read.
        unsafe { libc::rewinddir(stream) };
        let mut names = Vec::new();
        loop {
            // SAFETY: `stream` is open until `closedir` below; an entry is read before the next call.
            let entry = unsafe { libc::readdir(stream) };
            if entry.is_null() {
                break;
            }
            // SAFETY: `d_name` of an entry `readdir` returned is a NUL-terminated name.
            let name = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) };
            let name = std::ffi::OsStr::from_bytes(name.to_bytes());
            if name != "." && name != ".." {
                names.push(name.to_os_string());
            }
        }
        // SAFETY: `stream` is open and closed once; it closes `fd`.
        unsafe { libc::closedir(stream) };
        names.sort();
        Ok(names)
    }

    /// Whether the name `leaf` of this directory is a regular file, a link followed (`fstatat`): the answer of
    /// `Path::is_file`, asked of the descriptor.
    pub(crate) fn is_file(&self, leaf: impl AsRef<std::ffi::OsStr>) -> bool {
        use std::os::fd::AsRawFd;
        let Ok(c_leaf) = leaf_name(leaf.as_ref(), &self.path) else { return false };
        // SAFETY: as in `symlink`; `stat` is a zeroed buffer the call fills.
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        unsafe { libc::fstatat(self.dir.as_raw_fd(), c_leaf.as_ptr(), &mut stat, 0) == 0 && stat.st_mode & libc::S_IFMT == libc::S_IFREG }
    }

    /// The text of the file `leaf` of this directory ([`Held::read`]); a file that is not UTF-8 is an error.
    pub(crate) fn read_text(&self, leaf: impl AsRef<std::ffi::OsStr>) -> Result<String, String> {
        let leaf = leaf.as_ref();
        String::from_utf8(self.read(leaf)?).map_err(|_| format!("{}: not UTF-8 text", self.path.join(leaf).display()))
    }

    /// The path the directory was made at.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// The open directory, for what is set on the directory itself (its permission bits).
    pub(crate) fn file(&self) -> &File {
        &self.dir
    }

    /// The directory `rel` below this one, made when it is missing and held, by [`make_dir_at`]: the walk starts at the
    /// descriptor.
    pub(crate) fn sub(&self, rel: &Path) -> Result<Held, String> {
        Ok(Held { dir: make_dir_at(&self.dir, &self.path, rel)?, path: self.path.join(rel) })
    }

    /// [`write_atomic`] of the file `leaf` of this directory, through the descriptor.
    pub(crate) fn write(&self, leaf: impl AsRef<std::ffi::OsStr>, bytes: &[u8]) -> Result<(), String> {
        write_atomic_at(&self.dir, &self.path, leaf.as_ref(), bytes, 0o644)
    }

    /// A copy of `from` to the file `leaf` of this directory (what `copy_atomic` is by path): the source is read as every name of the run is, the
    /// file is written through the descriptor, and the bits of the source are kept.
    pub(crate) fn copy_from(&self, from: &Path, leaf: impl AsRef<std::ffi::OsStr>) -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt;
        let file = open_regular(from)?;
        let mode = file.metadata().map(|m| m.permissions().mode() & 0o777).unwrap_or(0o644);
        let bytes = read_open_within(file, from, READ_LIMIT)?;
        write_atomic_at(&self.dir, &self.path, leaf.as_ref(), &bytes, mode)
    }

    /// The file `from_leaf` of this directory, read as every name of the run is (through the descriptor, as a regular file
    /// with one name, at most [`READ_LIMIT`] bytes), written as the file `to_leaf` of the directory `to` with the bits of the
    /// source.
    pub(crate) fn copy_to(&self, from_leaf: impl AsRef<std::ffi::OsStr>, to: &Held, to_leaf: impl AsRef<std::ffi::OsStr>) -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt;
        let from = self.path.join(from_leaf.as_ref());
        let file = open_regular_at(&self.dir, &self.path, from_leaf.as_ref())?;
        let mode = file.metadata().map(|m| m.permissions().mode() & 0o777).unwrap_or(0o644);
        let bytes = read_open_within(file, &from, READ_LIMIT)?;
        write_atomic_at(&to.dir, &to.path, to_leaf.as_ref(), &bytes, mode)
    }

    /// The file `leaf` of this directory read as every name of the run is (through the descriptor).
    pub(crate) fn read(&self, leaf: impl AsRef<std::ffi::OsStr>) -> Result<Vec<u8>, String> {
        let path = self.path.join(leaf.as_ref());
        read_open_within(open_regular_at(&self.dir, &self.path, leaf.as_ref())?, &path, READ_LIMIT)
    }

    /// A link `leaf` to `target` in this directory, replacing what lies at the name (a link or a file; a directory stays and
    /// the call is an error).
    pub(crate) fn symlink(&self, target: &Path, leaf: impl AsRef<std::ffi::OsStr>) -> Result<(), String> {
        use std::os::fd::AsRawFd;
        use std::os::unix::ffi::OsStrExt;
        let leaf = leaf.as_ref();
        let c_leaf = leaf_name(leaf, &self.path)?;
        let c_target = std::ffi::CString::new(target.as_os_str().as_bytes()).map_err(|_| format!("{}: a name with a NUL byte", self.path.join(leaf).display()))?;
        let fd = self.dir.as_raw_fd();
        // SAFETY: `fd` is an open directory that `self` keeps open for the call and both names are NUL-terminated and outlive it.
        unsafe { libc::unlinkat(fd, c_leaf.as_ptr(), 0) };
        // SAFETY: as above.
        if unsafe { libc::symlinkat(c_target.as_ptr(), fd, c_leaf.as_ptr()) } != 0 {
            return Err(format!("{}: {}", self.path.join(leaf).display(), std::io::Error::last_os_error()));
        }
        Ok(())
    }

    /// Whether something is at the name `leaf` of this directory, a link followed (`fstatat`): the answer of `Path::exists`,
    /// asked of the descriptor.
    pub(crate) fn exists(&self, leaf: impl AsRef<std::ffi::OsStr>) -> bool {
        use std::os::fd::AsRawFd;
        let Ok(c_leaf) = leaf_name(leaf.as_ref(), &self.path) else { return false };
        // SAFETY: as in `symlink`; `stat` is a zeroed buffer the call fills.
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        unsafe { libc::fstatat(self.dir.as_raw_fd(), c_leaf.as_ptr(), &mut stat, 0) == 0 }
    }

    /// Removes the file or link `leaf` of this directory when there is one.
    pub(crate) fn remove(&self, leaf: impl AsRef<std::ffi::OsStr>) {
        use std::os::fd::AsRawFd;
        if let Ok(c_leaf) = leaf_name(leaf.as_ref(), &self.path) {
            // SAFETY: as in `symlink`.
            unsafe { libc::unlinkat(self.dir.as_raw_fd(), c_leaf.as_ptr(), 0) };
        }
    }
}

/// Appends `bytes` to `path` in one write, to a regular file with one name.
///
/// What lies at `path` is checked, not assumed: the open follows no link and waits for no reader of a pipe, and the
/// descriptor that is open must be a regular file with one name (a file with a second name may be a file outside the run).
/// Anything else is left alone and the answer is an error that names `path`. `create` says whether a missing file is made
/// (as an empty file of mode 644) or the append is refused.
pub(crate) fn append(path: &Path, bytes: &[u8], create: bool) -> Result<(), String> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let mut options = OpenOptions::new();
    options.append(true).create(create).mode(0o644).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    let mut file = options.open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let meta = file.metadata().map_err(|e| format!("{}: {e}", path.display()))?;
    if !meta.is_file() || meta.nlink() != 1 {
        return Err(format!("{}: not a regular file with one name", path.display()));
    }
    file.write_all(bytes).and_then(|()| file.flush()).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::Duration;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::current_exe().unwrap().parent().unwrap().join(format!("record-write-unit-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn mkfifo(path: &Path) {
        let c = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o644) }, 0, "mkfifo {}", path.display());
    }

    /// A file outside the directory of the record that must keep its text.
    fn outside(d: &Path) -> PathBuf {
        let file = d.parent().unwrap().join(format!("{}-outside.txt", d.file_name().unwrap().to_string_lossy()));
        std::fs::write(&file, "keep\n").unwrap();
        file
    }

    fn temporary(d: &Path, leaf: &str, n: usize) -> PathBuf {
        d.join(format!(".{leaf}.{}.{n}.tmp", std::process::id()))
    }

    fn names(d: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(d).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    #[test]
    fn a_temporary_name_that_is_taken_is_not_used_and_the_next_one_is_tried() {
        let d = dir("taken");
        let outside = outside(&d);
        mkfifo(&temporary(&d, "a.json", 0));
        std::fs::hard_link(&outside, temporary(&d, "a.json", 1)).unwrap();
        std::os::unix::fs::symlink(&outside, temporary(&d, "a.json", 2)).unwrap();
        std::fs::create_dir(temporary(&d, "a.json", 3)).unwrap();
        write_atomic(&d.join("a.json"), b"one\n").unwrap();
        assert_eq!(std::fs::read_to_string(d.join("a.json")).unwrap(), "one\n");
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep\n");
        assert_eq!(names(&d).len(), 5, "{:?}: the four planted names and the record, nothing else", names(&d));
    }

    #[test]
    fn a_write_that_finds_every_name_taken_ends_with_an_error_that_names_a_path_in_the_directory_and_writes_nothing_outside() {
        let d = dir("all-taken");
        let outside = outside(&d);
        for n in 0..16 {
            std::fs::hard_link(&outside, temporary(&d, "a.json", n)).unwrap();
        }
        let before = names(&d);
        let e = write_atomic(&d.join("a.json"), b"one\n").unwrap_err();
        assert!(e.contains(&d.display().to_string()) && e.contains(&format!(".a.json.{}.", std::process::id())), "{e}");
        assert!(!e.contains("outside.txt"), "{e}");
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep\n");
        assert_eq!(names(&d), before, "nothing was made, no record");
    }

    #[test]
    fn a_write_that_finds_fifteen_names_taken_uses_the_sixteenth() {
        let d = dir("last-free");
        for n in 0..15 {
            std::fs::create_dir(temporary(&d, "a.json", n)).unwrap();
        }
        write_atomic(&d.join("a.json"), b"one\n").unwrap();
        assert_eq!(std::fs::read_to_string(d.join("a.json")).unwrap(), "one\n");
    }

    #[test]
    fn a_write_whose_rename_fails_leaves_no_temporary_file() {
        let d = dir("rename-fails");
        std::fs::create_dir(d.join("a.json")).unwrap();
        std::fs::write(d.join("a.json").join("inside"), "x").unwrap();
        let before = names(&d);
        let e = write_atomic(&d.join("a.json"), b"one\n").unwrap_err();
        assert!(e.contains(&d.join("a.json").display().to_string()), "{e}");
        assert_eq!(names(&d), before, "the temporary file of the failed write is removed");
    }

    #[test]
    fn a_copy_keeps_the_permission_bits_of_its_source_and_replaces_a_link_at_its_destination() {
        let d = dir("copy");
        let outside = outside(&d);
        std::fs::write(d.join("from.sh"), "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(d.join("from.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
        std::os::unix::fs::symlink(&outside, d.join("to.sh")).unwrap();
        copy_atomic(&d.join("from.sh"), &d.join("to.sh")).unwrap();
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep\n");
        let meta = std::fs::symlink_metadata(d.join("to.sh")).unwrap();
        assert!(meta.is_file());
        assert_eq!(meta.permissions().mode() & 0o777, 0o755);
        assert_eq!(std::fs::read_to_string(d.join("to.sh")).unwrap(), "#!/bin/sh\n");
    }

    #[test]
    fn an_append_makes_a_missing_file_only_when_it_is_told_to() {
        let d = dir("create");
        let e = append(&d.join("a.jsonl"), b"one\n", false).unwrap_err();
        assert!(e.contains("a.jsonl"), "{e}");
        assert!(!d.join("a.jsonl").exists(), "a file that was not asked for was made");
        append(&d.join("a.jsonl"), b"one\n", true).unwrap();
        append(&d.join("a.jsonl"), b"two\n", false).unwrap();
        assert_eq!(std::fs::read_to_string(d.join("a.jsonl")).unwrap(), "one\ntwo\n");
    }

    #[test]
    fn an_append_does_not_follow_a_link() {
        let d = dir("link");
        let outside = outside(&d);
        std::os::unix::fs::symlink(&outside, d.join("a.jsonl")).unwrap();
        for create in [false, true] {
            let e = append(&d.join("a.jsonl"), b"x\n", create).unwrap_err();
            assert!(e.contains("a.jsonl") && !e.contains("outside.txt"), "{e}");
        }
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep\n");
    }

    #[test]
    fn an_append_refuses_a_file_that_has_a_second_name() {
        let d = dir("second-name");
        let outside = outside(&d);
        std::fs::hard_link(&outside, d.join("a.jsonl")).unwrap();
        let e = append(&d.join("a.jsonl"), b"x\n", true).unwrap_err();
        assert!(e.contains("a.jsonl"), "{e}");
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep\n");
    }

    #[test]
    fn an_append_refuses_a_named_pipe_that_has_a_reader_and_a_directory() {
        use std::io::Read;
        use std::os::unix::fs::OpenOptionsExt;
        let d = dir("pipe-reader");
        mkfifo(&d.join("a.jsonl"));
        let mut reader = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(d.join("a.jsonl")).unwrap();
        let e = append(&d.join("a.jsonl"), b"x\n", true).unwrap_err();
        assert!(e.contains("a.jsonl"), "{e}");
        let mut got = Vec::new();
        let _ = reader.read_to_end(&mut got);
        assert!(got.is_empty(), "the reader of the pipe was given {got:?}");
        std::fs::create_dir(d.join("b.jsonl")).unwrap();
        assert!(append(&d.join("b.jsonl"), b"x\n", true).is_err());
    }

    #[test]
    fn an_append_does_not_wait_for_a_reader_of_a_named_pipe() {
        let d = dir("pipe-alone");
        mkfifo(&d.join("a.jsonl"));
        let (tx, rx) = std::sync::mpsc::channel();
        let path = d.join("a.jsonl");
        std::thread::spawn(move || {
            let _ = tx.send(append(&path, b"x\n", true));
        });
        let answer = rx.recv_timeout(Duration::from_secs(5)).expect("the append waits for a reader of the pipe");
        assert!(answer.is_err());
    }

    /// An empty directory beside `d`, outside it.
    fn outdir(d: &Path) -> PathBuf {
        let out = d.parent().unwrap().join(format!("{}-outdir", d.file_name().unwrap().to_string_lossy()));
        std::fs::create_dir_all(&out).unwrap();
        out
    }

    #[test]
    fn a_directory_is_made_below_its_base_with_every_directory_between_and_again_without_error() {
        let d = dir("mk-nested");
        let made = make_dir_in(&d, &d.join("a/b/c")).unwrap();
        assert!(d.join("a/b/c").is_dir());
        drop(made);
        make_dir_in(&d, &d.join("a/b/c")).unwrap();
        make_dir_in(&d, &d.join("a/./b")).unwrap();
        make_dir_in(&d, &d).unwrap();
        assert_eq!(names(&d), vec!["a".to_owned()]);
    }

    #[test]
    fn a_directory_is_made_with_the_mode_create_dir_all_gives_it() {
        let d = dir("mk-mode");
        make_dir_in(&d, &d.join("made")).unwrap();
        std::fs::create_dir_all(d.join("plain")).unwrap();
        let mode = |name: &str| std::fs::metadata(d.join(name)).unwrap().permissions().mode() & 0o7777;
        assert_eq!(mode("made"), mode("plain"));
    }

    #[test]
    fn the_directory_the_call_returns_is_the_directory_it_made_and_takes_a_mode() {
        let d = dir("mk-returned");
        let made = make_dir_in(&d, &d.join("x/y")).unwrap();
        made.set_permissions(std::fs::Permissions::from_mode(0o710)).unwrap();
        assert_eq!(std::fs::metadata(d.join("x/y")).unwrap().permissions().mode() & 0o777, 0o710);
        assert_ne!(std::fs::metadata(d.join("x")).unwrap().permissions().mode() & 0o777, 0o710);
    }

    #[test]
    fn a_link_at_a_directory_ends_the_call_and_nothing_is_made_through_it() {
        let d = dir("mk-link");
        let out = outdir(&d);
        std::fs::create_dir(d.join("a")).unwrap();
        std::os::unix::fs::symlink(&out, d.join("a/b")).unwrap();
        let e = make_dir_in(&d, &d.join("a/b/c/e")).unwrap_err();
        assert!(e.contains(&d.join("a/b").display().to_string()) && !e.contains("-outdir"), "{e}");
        assert_eq!(names(&out), Vec::<String>::new(), "something was made in the directory the link points at");
        std::os::unix::fs::symlink(d.join("nowhere"), d.join("a/dangling")).unwrap();
        assert!(make_dir_in(&d, &d.join("a/dangling/x")).is_err());
        assert!(make_dir_in(&d, &d.join("a/dangling")).is_err());
        assert!(!d.join("nowhere").exists(), "a directory was made where a link pointed");
        std::os::unix::fs::symlink(&out, d.join("first")).unwrap();
        assert!(make_dir_in(&d, &d.join("first/x")).is_err());
        assert_eq!(names(&out), Vec::<String>::new());
    }

    #[test]
    fn a_file_at_a_directory_is_not_a_directory_of_the_run() {
        let d = dir("mk-file");
        std::fs::write(d.join("a"), "x").unwrap();
        let e = make_dir_in(&d, &d.join("a/b")).unwrap_err();
        assert!(e.contains(&d.join("a").display().to_string()), "{e}");
        assert!(make_dir_in(&d, &d.join("a")).is_err());
        assert_eq!(std::fs::read_to_string(d.join("a")).unwrap(), "x");
    }

    #[test]
    fn a_directory_that_does_not_lie_below_the_base_or_leaves_it_is_refused() {
        let d = dir("mk-outside");
        let other = dir("mk-outside-other");
        assert!(make_dir_in(&d, &other.join("x")).is_err());
        assert!(!other.join("x").exists());
        assert!(make_dir_in(&d, &d.join("a/../../mk-escaped")).is_err());
        assert!(!d.parent().unwrap().join("mk-escaped").exists());
        assert!(!d.join("mk-escaped").exists());
    }

    #[test]
    fn a_directory_that_cannot_be_made_is_an_error_that_names_it() {
        let d = dir("mk-denied");
        std::fs::create_dir(d.join("ro")).unwrap();
        std::fs::set_permissions(d.join("ro"), std::fs::Permissions::from_mode(0o555)).unwrap();
        if std::fs::write(d.join("ro/probe"), "x").is_ok() {
            return;
        }
        let e = make_dir_in(&d, &d.join("ro/x")).unwrap_err();
        assert!(e.contains(&d.join("ro/x").display().to_string()), "{e}");
        assert!(!d.join("ro/x").exists());
        std::fs::set_permissions(d.join("ro"), std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn a_read_takes_a_regular_file_and_refuses_what_is_not_one() {
        let d = dir("read-kinds");
        let outside = outside(&d);
        std::fs::write(d.join("a.txt"), "text\n").unwrap();
        assert_eq!(read_regular(&d.join("a.txt")).unwrap(), b"text\n");
        assert_eq!(read_regular_text(&d.join("a.txt")).unwrap(), "text\n");
        std::os::unix::fs::symlink(&outside, d.join("link")).unwrap();
        std::os::unix::fs::symlink(d.join("nowhere"), d.join("dangling")).unwrap();
        std::fs::hard_link(&outside, d.join("second")).unwrap();
        std::fs::create_dir(d.join("dir")).unwrap();
        for name in ["link", "dangling", "second", "dir", "absent"] {
            let e = read_regular(&d.join(name)).unwrap_err();
            assert!(e.contains(name) && !e.contains("outside.txt"), "{name}: {e}");
            assert!(read_regular_text(&d.join(name)).is_err(), "{name}");
            assert!(read_regular_within(&d.join(name), 100).is_err(), "{name}");
            assert!(read_regular_prefix(&d.join(name), 100).is_err(), "{name}");
        }
        std::fs::write(d.join("binary"), [0xff, 0xfe]).unwrap();
        assert!(read_regular_text(&d.join("binary")).unwrap_err().contains("binary"));
        assert_eq!(read_regular(&d.join("binary")).unwrap(), vec![0xff, 0xfe]);
    }

    #[test]
    fn a_read_of_a_named_pipe_does_not_wait_for_a_writer() {
        let d = dir("read-pipe");
        mkfifo(&d.join("p"));
        let path = d.join("p");
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send((read_regular(&path), read_regular_prefix(&path, 10), copy_atomic(&path, &path.with_file_name("to"))));
        });
        let (whole, prefix, copy) = rx.recv_timeout(Duration::from_secs(5)).expect("a read of a pipe waits for a writer");
        assert!(whole.is_err() && prefix.is_err() && copy.is_err());
        assert!(!d.join("to").exists());
    }

    #[test]
    fn a_file_of_exactly_the_limit_is_read_and_one_byte_more_is_refused() {
        let d = dir("read-limit");
        std::fs::write(d.join("ten"), "0123456789").unwrap();
        std::fs::write(d.join("eleven"), "0123456789a").unwrap();
        std::fs::write(d.join("empty"), "").unwrap();
        assert_eq!(read_regular_within(&d.join("ten"), 10).unwrap(), b"0123456789");
        let e = read_regular_within(&d.join("eleven"), 10).unwrap_err();
        assert!(e.contains("eleven") && e.contains("more than 10 bytes"), "{e}");
        assert!(read_regular_within(&d.join("empty"), 0).unwrap().is_empty());
        assert!(read_regular_within(&d.join("ten"), 0).is_err());
        assert!(read_regular_within(&d.join("ten"), u64::MAX).is_ok(), "a limit at the top of the range must not overflow");
    }

    #[test]
    fn the_bound_of_a_read_is_64_mib_and_a_sparse_file_one_byte_over_it_is_refused() {
        let d = dir("read-bound");
        assert_eq!(READ_LIMIT, 64 * 1024 * 1024);
        std::fs::File::create(d.join("at")).unwrap().set_len(READ_LIMIT).unwrap();
        std::fs::File::create(d.join("over")).unwrap().set_len(READ_LIMIT + 1).unwrap();
        assert_eq!(read_regular(&d.join("at")).unwrap().len() as u64, READ_LIMIT);
        let e = read_regular(&d.join("over")).unwrap_err();
        assert!(e.contains("over") && e.contains("more than 67108864 bytes"), "{e}");
        assert!(copy_atomic(&d.join("over"), &d.join("to")).is_err());
        assert!(!d.join("to").exists());
    }

    #[test]
    fn a_prefix_read_takes_the_first_bytes_of_a_longer_file_and_all_of_a_shorter_one() {
        let d = dir("read-prefix");
        std::fs::write(d.join("ten"), "0123456789").unwrap();
        assert_eq!(read_regular_prefix(&d.join("ten"), 4).unwrap(), b"0123");
        assert_eq!(read_regular_prefix(&d.join("ten"), 10).unwrap(), b"0123456789");
        assert_eq!(read_regular_prefix(&d.join("ten"), 100).unwrap(), b"0123456789");
        assert!(read_regular_prefix(&d.join("ten"), 0).unwrap().is_empty());
    }

    #[test]
    fn a_copy_refuses_a_source_that_is_a_link_or_has_a_second_name_and_writes_nothing() {
        let d = dir("copy-source");
        let outside = outside(&d);
        std::os::unix::fs::symlink(&outside, d.join("from-link")).unwrap();
        std::fs::hard_link(&outside, d.join("from-second")).unwrap();
        for name in ["from-link", "from-second", "from-absent"] {
            let e = copy_atomic(&d.join(name), &d.join("to")).unwrap_err();
            assert!(e.contains(name) && !e.contains("outside.txt"), "{name}: {e}");
            assert!(!d.join("to").exists(), "{name}: a file was made");
        }
        assert_eq!(names(&d).iter().filter(|n| n.starts_with('.')).count(), 0, "a temporary file was left");
    }
}
