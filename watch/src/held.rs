//! The output directory of the watcher, held by descriptor.
//!
//! The programs of a run write the run directory too, and the output directory (`fs/`) lies in it, so a program can rename
//! it and put a link at its name while the watcher runs. The watcher opens it once, at start, with `O_NOFOLLOW`, and makes,
//! reads, renames and removes every file of it relative to that descriptor (`openat`, `renameat`, `unlinkat`): what it
//! writes reaches the directory it opened, and nothing reaches the place a link points at. A file of it is read as a
//! regular file with one name, with no link followed and no pipe waited for, and at most a bound of bytes is taken.

use std::ffi::CString;
use std::fs::File;
use std::io::{Error, ErrorKind, Read, Result};
use std::os::fd::{AsRawFd, FromRawFd};
use std::path::{Path, PathBuf};

/// A directory held open: every name in it is reached through the descriptor, never through the path.
pub struct Held {
    dir: File,
    path: PathBuf,
}

fn c_name(name: &str) -> Result<CString> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') {
        return Err(Error::new(ErrorKind::InvalidInput, format!("{name:?}: not a plain file name")));
    }
    CString::new(name).map_err(|_| Error::new(ErrorKind::InvalidInput, format!("{name:?}: a name with a NUL byte")))
}

fn fd_result(raw: libc::c_int) -> Result<File> {
    if raw < 0 {
        return Err(Error::last_os_error());
    }
    // SAFETY: `raw` is a new descriptor that nothing else owns.
    Ok(unsafe { File::from_raw_fd(raw) })
}

fn zero_result(r: libc::c_int) -> Result<()> {
    if r == 0 { Ok(()) } else { Err(Error::last_os_error()) }
}

/// The bytes `from` yields when they are at most `limit`; at most `limit + 1` bytes are asked of it.
pub fn read_within(from: impl Read, limit: u64) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    from.take(limit.saturating_add(1)).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= limit).then_some(bytes)
}

impl Held {
    /// Opens the directory at `path` once, with `O_NOFOLLOW | O_DIRECTORY`: a link or anything else that is not a directory
    /// at its last component is an error.
    pub fn open(path: &Path) -> Result<Held> {
        use std::os::unix::fs::OpenOptionsExt;
        let dir = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC).open(path)?;
        Ok(Held { dir, path: path.to_path_buf() })
    }

    /// The directory `name` of this one, made when it is missing (`mkdirat`, mode 0o777 less the umask) and opened relative
    /// to the descriptor with `O_NOFOLLOW | O_DIRECTORY`.
    pub fn sub(&self, name: &str) -> Result<Held> {
        let c = c_name(name)?;
        // SAFETY: `self.dir` is an open directory and `c` a NUL-terminated name, both alive for the call.
        if unsafe { libc::mkdirat(self.dir.as_raw_fd(), c.as_ptr(), 0o777) } != 0 {
            let e = Error::last_os_error();
            if e.kind() != ErrorKind::AlreadyExists {
                return Err(e);
            }
        }
        // SAFETY: as above.
        let dir = fd_result(unsafe { libc::openat(self.dir.as_raw_fd(), c.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC) })?;
        Ok(Held { dir, path: self.path.join(name) })
    }

    /// A second descriptor of the same directory.
    pub fn try_clone(&self) -> Result<Held> {
        Ok(Held { dir: self.dir.try_clone()?, path: self.path.clone() })
    }

    /// The path the directory was opened at, for messages.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The file `name` opened for reading as a regular file with one name: no link is followed, a pipe is not waited for,
    /// and anything else (a pipe, a device, a directory, a second name of a file) is `None`.
    pub fn open_regular(&self, name: &str) -> Option<File> {
        use std::os::unix::fs::MetadataExt;
        let c = c_name(name).ok()?;
        // SAFETY: as in `sub`.
        let file = fd_result(unsafe { libc::openat(self.dir.as_raw_fd(), c.as_ptr(), libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC) }).ok()?;
        let meta = file.metadata().ok()?;
        (meta.is_file() && meta.nlink() == 1).then_some(file)
    }

    /// The bytes of the file `name` ([`Held::open_regular`]) when it holds at most `limit` of them.
    pub fn read(&self, name: &str, limit: u64) -> Option<Vec<u8>> {
        read_within(self.open_regular(name)?, limit)
    }

    /// Whether a regular file lies at `name`, no link followed (`fstatat` with `AT_SYMLINK_NOFOLLOW`).
    pub fn is_file(&self, name: &str) -> bool {
        let Ok(c) = c_name(name) else { return false };
        // SAFETY: as in `sub`; `stat` is a zeroed buffer the call fills.
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        unsafe { libc::fstatat(self.dir.as_raw_fd(), c.as_ptr(), &mut stat, libc::AT_SYMLINK_NOFOLLOW) == 0 && stat.st_mode & libc::S_IFMT == libc::S_IFREG }
    }

    /// A file that is new at `name`: `O_CREAT | O_EXCL | O_NOFOLLOW | O_NONBLOCK`, so nothing at the name is followed,
    /// opened or written through; a taken name is an `AlreadyExists` error. The mode is 0o666 less the umask.
    pub fn create_new(&self, name: &str) -> Result<File> {
        let c = c_name(name)?;
        // SAFETY: as in `sub`.
        fd_result(unsafe { libc::openat(self.dir.as_raw_fd(), c.as_ptr(), libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC, 0o666 as libc::c_uint) })
    }

    /// The file `name` opened to append, made when it is missing: no link is followed, no reader of a pipe is waited for,
    /// and anything that is not a regular file with one name is an error.
    pub fn open_append(&self, name: &str) -> Result<File> {
        use std::os::unix::fs::MetadataExt;
        let c = c_name(name)?;
        // SAFETY: as in `sub`.
        let file = fd_result(unsafe { libc::openat(self.dir.as_raw_fd(), c.as_ptr(), libc::O_WRONLY | libc::O_APPEND | libc::O_CREAT | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC, 0o666 as libc::c_uint) })?;
        let meta = file.metadata()?;
        if !meta.is_file() || meta.nlink() != 1 {
            return Err(Error::other(format!("{}: not a regular file with one name", self.path.join(name).display())));
        }
        Ok(file)
    }

    /// Renames `from` to `to`, both names of this directory (`renameat`); what lies at `to` is replaced.
    pub fn rename(&self, from: &str, to: &str) -> Result<()> {
        let (f, t) = (c_name(from)?, c_name(to)?);
        // SAFETY: as in `sub`, for both names.
        zero_result(unsafe { libc::renameat(self.dir.as_raw_fd(), f.as_ptr(), self.dir.as_raw_fd(), t.as_ptr()) })
    }

    /// Removes the file or link `name` (`unlinkat`: nothing is followed or opened).
    pub fn remove(&self, name: &str) -> Result<()> {
        let c = c_name(name)?;
        // SAFETY: as in `sub`.
        zero_result(unsafe { libc::unlinkat(self.dir.as_raw_fd(), c.as_ptr(), 0) })
    }
}
