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

use sdc_tools::gateway::Gateway;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The directory and, when the settings name a login file, the link in it, or, when they name a
/// gateway, the file with its address and token.
pub struct PrivateDir {
    dir: PathBuf,
    link: Option<PathBuf>,
    gateway: Option<PathBuf>,
}

/// The paths [`PrivateDir::check`] plans: the link to the login file or the gateway file, never both.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Planned {
    pub link: Option<PathBuf>,
    pub gateway: Option<PathBuf>,
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
    /// What [`PrivateDir::create_with`] would refuse, without making anything, and the paths the
    /// link and the gateway file would have (both `None` when neither a login file nor a gateway is
    /// named). A run without a model node makes neither: the runner checks here before anything is
    /// written and calls `create_with` only in a run that has a model node, after the opening decided.
    pub fn check(run: &str, login: Option<&Path>, gateway: bool) -> Result<Planned, String> {
        if login.is_some() && gateway {
            return Err("gateway and login_file: a run has one or the other, never both".to_owned());
        }
        if let Some(login) = login {
            if !login.is_absolute() || !login.is_file() {
                return Err("login_file: not an absolute path of a file that exists".to_owned());
            }
        } else if !gateway {
            return Ok(Planned::default());
        }
        let named = if gateway { "a gateway" } else { "a login file" };
        let base = match std::env::var_os("XDG_RUNTIME_DIR") {
            Some(v) if !v.is_empty() => PathBuf::from(v),
            _ => return Err(format!("{named} is named and XDG_RUNTIME_DIR is not set: what it needs lives in $XDG_RUNTIME_DIR/runbook/<run>/, not in the run directory")),
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
        Ok(if gateway { Planned { link: None, gateway: Some(dir.join("gateway")) } } else { Planned { link: Some(dir.join("login")), gateway: None } })
    }

    /// Makes the private directory of `run` when `login` is given, with the link in it; returns
    /// `None` when no login file is named (nothing is made, `XDG_RUNTIME_DIR` is not needed).
    /// Refused, with nothing made, when `XDG_RUNTIME_DIR` is unset, not absolute, not a directory
    /// or not owned by the user, when the login file is not an absolute path of a file that
    /// exists, and when the directory of this run name exists already. No message holds the login
    /// file's path.
    pub fn create(run: &str, login: Option<&Path>) -> Result<Option<PrivateDir>, String> {
        PrivateDir::create_with(run, login, None)
    }

    /// [`PrivateDir::create`] for a run that may name a gateway instead of a login: with `gateway`, the
    /// directory holds the one file `gateway` (mode 0600, written with `create_new` into the directory of
    /// mode 0700, so it never exists with wider rights) and no link. A login and a gateway together are
    /// refused; neither: nothing is made. No message holds the token.
    pub fn create_with(run: &str, login: Option<&Path>, gateway: Option<&Gateway>) -> Result<Option<PrivateDir>, String> {
        if login.is_some() && gateway.is_some() {
            return Err("gateway and login_file: a run has one or the other, never both".to_owned());
        }
        if let Some(login) = login {
            if !login.is_absolute() || !login.is_file() {
                return Err("login_file: not an absolute path of a file that exists".to_owned());
            }
        } else if gateway.is_none() {
            return Ok(None);
        }
        let named = if gateway.is_some() { "a gateway" } else { "a login file" };
        let base = match std::env::var_os("XDG_RUNTIME_DIR") {
            Some(v) if !v.is_empty() => PathBuf::from(v),
            _ => return Err(format!("{named} is named and XDG_RUNTIME_DIR is not set: what it needs lives in $XDG_RUNTIME_DIR/runbook/<run>/, not in the run directory")),
        };
        if !base.is_absolute() {
            return Err("XDG_RUNTIME_DIR is not an absolute path".to_owned());
        }
        owned_directory(&base, "XDG_RUNTIME_DIR")?;
        let parent = base.join("runbook");
        match std::fs::DirBuilder::new().mode(0o700).create(&parent) {
            Ok(()) => std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).map_err(|e| format!("{}: {}", parent.display(), e.kind()))?,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => owned_directory(&parent, "the runtime directory")?,
            Err(e) => return Err(format!("{}: {}", parent.display(), e.kind())),
        }
        let dir = parent.join(run);
        match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(format!("{}: exists (left by a run that was killed? remove it when no run of this name is on)", dir.display()));
            }
            Err(e) => return Err(format!("{}: {}", dir.display(), e.kind())),
        }
        let mut made = PrivateDir { dir, link: None, gateway: None };
        std::fs::set_permissions(&made.dir, std::fs::Permissions::from_mode(0o700)).map_err(|e| format!("{}: {}", made.dir.display(), e.kind()))?;
        if let Some(login) = login {
            let link = made.dir.join("login");
            std::os::unix::fs::symlink(login, &link).map_err(|e| format!("{}: could not link the login file: {}", link.display(), e.kind()))?;
            made.link = Some(link);
        }
        if let Some(gateway) = gateway {
            let file = made.dir.join("gateway");
            let written = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&file)
                .and_then(|mut f| std::io::Write::write_all(&mut f, gateway.render().as_bytes()));
            written.map_err(|e| format!("{}: could not write the gateway file: {}", file.display(), e.kind()))?;
            made.gateway = Some(file);
        }
        Ok(Some(made))
    }

    /// The link to the login file.
    pub fn link(&self) -> Option<&Path> {
        self.link.as_deref()
    }

    /// The gateway file: the address and the token, mode 0600.
    pub fn gateway_file(&self) -> Option<&Path> {
        self.gateway.as_deref()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::testdir::Dir;

    const TOKEN: &str = "tok-private-6e1f20ab";

    /// One test owns `XDG_RUNTIME_DIR` (the process's environment is shared by threads).
    #[test]
    fn a_gateway_file_is_mode_600_in_a_directory_of_mode_700_and_goes_with_the_directory() {
        let dir = Dir::new("private-gateway");
        let xdg = dir.path().join("xdg");
        std::fs::create_dir_all(&xdg).unwrap();
        std::fs::set_permissions(&xdg, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::env::set_var("XDG_RUNTIME_DIR", &xdg);
        let gateway = Gateway::new("http://127.0.0.1:8080", TOKEN).unwrap();
        let made = PrivateDir::create_with("g1", None, Some(&gateway)).unwrap().expect("a gateway makes the directory");
        let file = made.gateway_file().unwrap().to_path_buf();
        assert_eq!(file, xdg.join("runbook/g1/gateway"));
        assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(xdg.join("runbook/g1")).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(sdc_tools::gateway::read(&file).unwrap(), gateway);
        assert!(made.link().is_none());
        let names: Vec<_> = std::fs::read_dir(xdg.join("runbook/g1")).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, ["gateway"]);
        // the same run name is refused while the directory is there, without the token
        let again = PrivateDir::create_with("g1", None, Some(&gateway)).err().expect("a second directory of the name is refused");
        assert!(again.contains("exists") && !again.contains(TOKEN), "{again}");
        drop(made);
        assert!(!xdg.join("runbook/g1").exists(), "the directory is left");
        // neither: nothing is made; both: refused
        assert!(PrivateDir::create_with("g2", None, None).unwrap().is_none());
        assert!(!xdg.join("runbook/g2").exists());
        let login = dir.path().join("login.json");
        std::fs::write(&login, "{}").unwrap();
        let both = PrivateDir::create_with("g3", Some(&login), Some(&gateway)).err().expect("both are refused");
        assert!(both.contains("gateway") && both.contains("login_file") && !both.contains(TOKEN), "{both}");
        assert!(!xdg.join("runbook/g3").exists());
        // no runtime directory: refused by what is named, with nothing made
        std::env::remove_var("XDG_RUNTIME_DIR");
        let none = PrivateDir::create_with("g4", None, Some(&gateway)).err().expect("no XDG_RUNTIME_DIR");
        assert!(none.contains("gateway") && none.contains("XDG_RUNTIME_DIR") && !none.contains(TOKEN), "{none}");
        // a login alone works as it did
        std::env::set_var("XDG_RUNTIME_DIR", &xdg);
        let by_login = PrivateDir::create("g5", Some(&login)).unwrap().unwrap();
        assert!(by_login.link().is_some() && by_login.gateway_file().is_none());
    }
}
