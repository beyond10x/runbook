//! The private directory of a run: `$XDG_RUNTIME_DIR/runbook/<run name>/`, mode 0700.
//!
//! It holds the one link to the login file that the model nodes of the bindings copy and the
//! model decider read (`--claude-credentials`, `SDC_CLAUDE_CREDENTIALS`), and the private homes the
//! adapters make beside it for the length of a call (they hold a copy of the login; never in the run
//! directory, where a SIGKILL would leave it). The run directory is
//! copied, archived and read by other programs; the login file's path must not be in it, and
//! neither a link to it. The private directory lives only as long as the run: [`PrivateDir`]
//! removes it when it is dropped, which covers the end of a run, an error, and the signals the
//! runner handles (they make the run end through its normal return). A SIGKILL leaves it; the next
//! run of the same name is refused by name until someone removes it (`docs/loom-run.md`).
//!
//! A link, not a copy of the file: no second copy of the credential is written to disk, and a
//! token the CLI refreshes lands in the real file.

use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The directory and, when the settings name a login file, the link in it.
pub struct PrivateDir {
    dir: PathBuf,
    link: Option<PathBuf>,
}

fn owned_directory(path: &Path, what: &str) -> Result<(), String> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| format!("{what} {}: {}", path.display(), e.kind()))?;
    // SAFETY: geteuid has no preconditions.
    let me = unsafe { libc::geteuid() };
    if !meta.is_dir() || meta.uid() != me {
        return Err(format!("{what} {} is not a directory owned by the user running the command", path.display()));
    }
    Ok(())
}

impl PrivateDir {
    /// What [`PrivateDir::create`] would refuse, without making anything, and the path the link
    /// would have (`None` when no login file is named). A run without a model node makes no link:
    /// the runner checks here before anything is written and calls `create` only in a run that
    /// has a model node, after the opening decided.
    pub fn check(run: &str, login: Option<&Path>) -> Result<Option<PathBuf>, String> {
        let Some(login) = login else {
            return Ok(None);
        };
        if !login.is_absolute() || !login.is_file() {
            return Err("login_file: not an absolute path of a file that exists".to_owned());
        }
        let base = match std::env::var_os("XDG_RUNTIME_DIR") {
            Some(v) if !v.is_empty() => PathBuf::from(v),
            _ => return Err("a login file is named and XDG_RUNTIME_DIR is not set: the link to it lives in $XDG_RUNTIME_DIR/runbook/<run>/, not in the run directory".to_owned()),
        };
        if !base.is_absolute() {
            return Err("XDG_RUNTIME_DIR is not an absolute path".to_owned());
        }
        owned_directory(&base, "XDG_RUNTIME_DIR")?;
        let parent = base.join("runbook");
        if std::fs::symlink_metadata(&parent).is_ok() {
            owned_directory(&parent, "the runtime directory")?;
        }
        let dir = parent.join(run);
        if std::fs::symlink_metadata(&dir).is_ok() {
            return Err(format!("{}: exists (left by a run that was killed? remove it when no run of this name is on)", dir.display()));
        }
        Ok(Some(dir.join("login")))
    }

    /// Makes the private directory of `run` when `login` is given, with the link in it; returns
    /// `None` when no login file is named (nothing is made, `XDG_RUNTIME_DIR` is not needed).
    /// Refused, with nothing made, when `XDG_RUNTIME_DIR` is unset, not absolute, not a directory
    /// or not owned by the user, when the login file is not an absolute path of a file that
    /// exists, and when the directory of this run name exists already. No message holds the login
    /// file's path.
    pub fn create(run: &str, login: Option<&Path>) -> Result<Option<PrivateDir>, String> {
        let Some(login) = login else {
            return Ok(None);
        };
        if !login.is_absolute() || !login.is_file() {
            return Err("login_file: not an absolute path of a file that exists".to_owned());
        }
        let base = match std::env::var_os("XDG_RUNTIME_DIR") {
            Some(v) if !v.is_empty() => PathBuf::from(v),
            _ => return Err("a login file is named and XDG_RUNTIME_DIR is not set: the link to it lives in $XDG_RUNTIME_DIR/runbook/<run>/, not in the run directory".to_owned()),
        };
        if !base.is_absolute() {
            return Err("XDG_RUNTIME_DIR is not an absolute path".to_owned());
        }
        owned_directory(&base, "XDG_RUNTIME_DIR")?;
        let parent = base.join("runbook");
        match std::fs::create_dir(&parent) {
            Ok(()) => std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).map_err(|e| format!("{}: {}", parent.display(), e.kind()))?,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => owned_directory(&parent, "the runtime directory")?,
            Err(e) => return Err(format!("{}: {}", parent.display(), e.kind())),
        }
        let dir = parent.join(run);
        match std::fs::create_dir(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(format!("{}: exists (left by a run that was killed? remove it when no run of this name is on)", dir.display()));
            }
            Err(e) => return Err(format!("{}: {}", dir.display(), e.kind())),
        }
        let mut made = PrivateDir { dir, link: None };
        std::fs::set_permissions(&made.dir, std::fs::Permissions::from_mode(0o700)).map_err(|e| format!("{}: {}", made.dir.display(), e.kind()))?;
        let link = made.dir.join("login");
        std::os::unix::fs::symlink(login, &link).map_err(|e| format!("{}: could not link the login file: {}", link.display(), e.kind()))?;
        made.link = Some(link);
        Ok(Some(made))
    }

    /// The link to the login file.
    pub fn link(&self) -> Option<&Path> {
        self.link.as_deref()
    }
}

impl Drop for PrivateDir {
    fn drop(&mut self) {
        // The link first, as a link: `remove_file` removes the symlink and never its target. Then the
        // directory with all it holds: a call of an adapter that was killed with SIGKILL leaves its
        // private home there (`.claude/.credentials.json`, a link to a program). `remove_dir_all`
        // does not follow a link either: a link inside is removed, the file it names stays.
        if let Some(link) = &self.link {
            let _ = std::fs::remove_file(link);
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
