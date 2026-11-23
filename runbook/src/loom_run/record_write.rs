//! The one writer of the records of a run directory, and the one appender.
//!
//! The run directory is written by the programs of a run too (`--confinement local`), so no name in it is trusted: a
//! program can leave a link, a pipe, a directory or a second name of a file outside the run at any name the runner will
//! write. Every record the runner writes into the run directory after its first program started goes through this file,
//! and nothing else of the runner opens such a name for writing.
//!
//! This file uses `std` and `libc` and nothing else of the crate, so that the tests of the integration-test binary (there
//! is no library target) can include it as it is, with `run_dir.rs`.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// How many temporary names one write tries before it ends with an error. No try waits, so a write ends within
/// microseconds, whatever a program left in the directory.
pub(crate) const NAMES_TRIED: usize = 16;

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
pub(crate) fn write_atomic_mode(path: &Path, bytes: &[u8], mode: u32) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    static WRITES: AtomicU64 = AtomicU64::new(0);
    let leaf = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut last = String::new();
    for _ in 0..NAMES_TRIED {
        let tmp = path.with_file_name(format!(".{leaf}.{}.{}.tmp", std::process::id(), WRITES.fetch_add(1, Ordering::Relaxed)));
        let mut file = match OpenOptions::new().write(true).create_new(true).mode(mode).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(&tmp) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                last = format!("{}: {e}", tmp.display());
                continue;
            }
            Err(e) => return Err(format!("{}: {e}", tmp.display())),
        };
        let written = file.write_all(bytes).and_then(|()| file.flush()).map_err(|e| format!("{}: {e}", tmp.display()));
        drop(file);
        let renamed = written.and_then(|()| std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display())));
        if renamed.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        return renamed;
    }
    Err(last)
}

/// Copies the file `from` to `to` through [`write_atomic_mode`], with the permission bits of `from`. A link or a pipe at `to`
/// is replaced, never written through.
pub(crate) fn copy_atomic(from: &Path, to: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let bytes = std::fs::read(from).map_err(|e| format!("{}: {e}", from.display()))?;
    let mode = std::fs::metadata(from).map(|m| m.permissions().mode() & 0o777).unwrap_or(0o644);
    write_atomic_mode(to, &bytes, mode)
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
}
