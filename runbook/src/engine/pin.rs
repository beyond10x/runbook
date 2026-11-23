//! A run takes its runbook at submit: the files a run reads are copied into the run's own
//! directory (`<state-dir>/runs/<run id>/runbook/`, outside the run's workspace) and the tree is
//! opened on the copy. A revision, an edit or a removal in the runbook home afterwards changes
//! nothing for the run. The copy is checked against a digest before every call into Loom that can
//! read it or run a program of it.

use super::copy::{copy_dirs, digest, Links};
use super::layout::{self, Resolved};
use serde_json::{json, Value};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// The directory of the copy, in the run's directory.
pub const COPY_DIR: &str = "runbook";
/// The file in the run's directory (beside the copy, not in it) that names what the copy was.
const PIN_FILE: &str = "runbook-pin.json";
/// The most bytes of the pin file `load` reads: the file holds five short members.
const PIN_FILE_MAX: u64 = 64 << 10;
/// The name a refusal of a changed copy starts with.
pub const CHANGED: &str = "RunbookCopyChanged";
/// What the tools of the bindings read beside `bindings/` (`$here/../..`): the instruction files,
/// the views, the fixtures and rule file of the driver, the checks of the decider. Copied when the
/// home has them.
const COMPANIONS: [&str; 4] = ["instructions", "views", "driver/examples", "tools"];

/// What the run's copy of the runbook was when it was made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    /// The copy: a directory with `protocol/` and `bindings/`, laid out as the home is.
    pub copy: PathBuf,
    pub runbook: String,
    /// The id the root protocol declares.
    pub protocol: String,
    pub revision: i64,
    /// The sha256 of the copied `bindings/` directory.
    pub bindings_sha256: String,
    /// The sha256 of the whole copy.
    pub copy_sha256: String,
}

/// The refusal of a run whose pin file is not what the submit wrote: it names no path and holds no
/// byte of the file.
fn pin_file_changed() -> String {
    format!("{CHANGED}: the pin file of the run's copy of the runbook is missing or is not what the submit wrote; the run ends and nothing of the copy runs")
}

impl Pin {
    /// Copies the runbook `name` of `home` into `run_dir` and describes the copy. A refusal (a link
    /// that leaves the home, a runbook that is not there, a revision that is not the one asked for)
    /// leaves nothing of the runbook in `run_dir`. Returns the runbook as the copy holds it.
    pub fn take(home: &Path, run_dir: &Path, name: &str, version: Option<&str>) -> Result<(Pin, Resolved), String> {
        let root = std::fs::canonicalize(home).map_err(|e| format!("RunbookCopyRefused: the runbook home cannot be read: {e}"))?;
        let copy = run_dir.join(COPY_DIR);
        let refused = |why: String| format!("RunbookCopyRefused: {why}");
        if copy.exists() {
            std::fs::remove_dir_all(&copy).map_err(|e| refused(format!("{}: {e}", copy.display())))?;
        }
        let mut jobs: Vec<(PathBuf, PathBuf)> = vec![(root.join("protocol"), copy.join("protocol")), (root.join("bindings"), copy.join("bindings"))];
        for rel in COMPANIONS {
            let from = root.join(rel);
            if from.is_dir() {
                jobs.push((from, copy.join(rel)));
            }
        }
        let borrowed: Vec<(&Path, &Path)> = jobs.iter().map(|(from, to)| (from.as_path(), to.as_path())).collect();
        let made = copy_dirs(&borrowed, Links::Within(&root)).and_then(|()| {
            let resolved = layout::resolve(&copy, name, version)?;
            let pin = Pin {
                bindings_sha256: digest(&copy.join("bindings"))?,
                copy_sha256: digest(&copy)?,
                copy: copy.clone(),
                runbook: name.to_owned(),
                protocol: resolved.protocol_id.clone(),
                revision: resolved.revision,
            };
            let mut file = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(run_dir.join(PIN_FILE)).map_err(|e| format!("{PIN_FILE}: {e}"))?;
            std::io::Write::write_all(&mut file, pin.record().to_string().as_bytes()).map_err(|e| format!("{PIN_FILE}: {e}"))?;
            Ok::<_, String>((pin, resolved))
        });
        if made.is_err() {
            let _ = std::fs::remove_dir_all(&copy);
        }
        made.map_err(refused)
    }

    /// The pin of a run whose copy was made by an earlier process. `None` only for a run that has
    /// neither a copy nor a pin file (a run opened before the copy existed reads the runbook home, as
    /// it always did). A copy that stands without a pin file that is a regular file of one name, text,
    /// and what the submit wrote is a changed copy: refused by the error a changed copy has. The pin
    /// file is opened without following a link and without waiting on a pipe.
    pub fn load(run_dir: &Path) -> Result<Option<Pin>, String> {
        let copy = run_dir.join(COPY_DIR);
        let stands = std::fs::symlink_metadata(&copy).is_ok();
        let mut file = match std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(run_dir.join(PIN_FILE)) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && !stands => return Ok(None),
            Err(_) => return Err(pin_file_changed()),
        };
        let meta = file.metadata().map_err(|_| pin_file_changed())?;
        if !meta.is_file() || meta.nlink() != 1 || meta.len() > PIN_FILE_MAX {
            return Err(pin_file_changed());
        }
        let mut text = String::new();
        file.read_to_string(&mut text).map_err(|_| pin_file_changed())?;
        let doc: Value = serde_json::from_str(&text).map_err(|_| pin_file_changed())?;
        let text_of = |k: &str| doc.get(k).and_then(Value::as_str).map(str::to_owned).ok_or_else(pin_file_changed);
        let pin = Pin {
            copy,
            runbook: text_of("runbook")?,
            protocol: text_of("protocol")?,
            revision: doc.get("revision").and_then(Value::as_i64).ok_or_else(pin_file_changed)?,
            bindings_sha256: text_of("bindings_sha256")?,
            copy_sha256: text_of("copy_sha256")?,
        };
        pin.check()?;
        Ok(Some(pin))
    }

    /// Whether the copy is what it was. A copy that is not is refused by name: nothing in it runs. A
    /// reason names an entry relative to the copy, never a path of the host.
    pub fn check(&self) -> Result<(), String> {
        match digest(&self.copy) {
            Ok(now) if now == self.copy_sha256 => Ok(()),
            Ok(_) => Err(format!("{CHANGED}: the run's copy of the runbook {} changed after the submit; the run ends and nothing of the changed copy runs", self.runbook)),
            Err(e) => Err(format!("{CHANGED}: the run's copy of the runbook {} cannot be read ({e}); the run ends", self.runbook)),
        }
    }

    /// The record of the run's event `runbook.pinned` and the content of the pin file: no path,
    /// only what the copy is.
    pub fn record(&self) -> Value {
        json!({"runbook": self.runbook, "protocol": self.protocol, "revision": self.revision, "bindings_sha256": self.bindings_sha256, "copy_sha256": self.copy_sha256})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::current_exe().unwrap().parent().unwrap().join(format!("engine-pin-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::canonicalize(d).unwrap()
    }

    fn home(tag: &str) -> PathBuf {
        let home = dir(tag);
        std::fs::create_dir_all(home.join("protocol")).unwrap();
        std::fs::create_dir_all(home.join("bindings/tools")).unwrap();
        std::fs::write(home.join("protocol/demo.protocol.yaml"), "protocol:\n  id: demo\n  revision: 3\n").unwrap();
        std::fs::write(home.join("bindings/demo.yaml"), "protocol: demo@3\n").unwrap();
        std::fs::write(home.join("bindings/tools/run.sh"), "#!/bin/sh\n").unwrap();
        home
    }

    #[test]
    fn a_copy_is_the_home_at_the_time_and_is_checked_and_loaded_again() {
        let home = home("take");
        let run = dir("take-run");
        let (pin, resolved) = Pin::take(&home, &run, "demo", Some("3")).unwrap();
        assert_eq!((pin.protocol.as_str(), pin.revision, resolved.revision), ("demo", 3, 3));
        assert!(resolved.bindings.starts_with(run.join(COPY_DIR)) && resolved.protocol_dir.starts_with(run.join(COPY_DIR)));
        assert_eq!(pin.bindings_sha256, digest(&home.join("bindings")).unwrap());
        assert!(!pin.record().to_string().contains(run.to_str().unwrap()), "the record names no path");
        pin.check().unwrap();
        assert_eq!(Pin::load(&run).unwrap(), Some(pin.clone()));
        // The home changes: the copy does not.
        std::fs::remove_dir_all(home.join("bindings")).unwrap();
        pin.check().unwrap();
        // The copy changes: it is refused by name, also when the run is loaded again.
        std::fs::write(pin.copy.join("bindings/tools/run.sh"), "#!/bin/sh\nexit 1\n").unwrap();
        assert!(pin.check().unwrap_err().starts_with(CHANGED));
        assert!(Pin::load(&run).unwrap_err().starts_with(CHANGED));
    }

    #[test]
    fn a_refused_copy_leaves_nothing_in_the_run_directory() {
        let home = home("refused");
        let run = dir("refused-run");
        let outside = dir("refused-outside");
        std::os::unix::fs::symlink(&outside, home.join("bindings/tools/out")).unwrap();
        let refused = Pin::take(&home, &run, "demo", None).unwrap_err();
        assert!(refused.starts_with("RunbookCopyRefused") && refused.contains("leaves the runbook home"), "{refused}");
        assert_eq!(std::fs::read_dir(&run).unwrap().count(), 0);
        std::fs::remove_file(home.join("bindings/tools/out")).unwrap();
        // A version that is not the revision is refused after the copy, and takes the copy with it.
        let refused = Pin::take(&home, &run, "demo", Some("4")).unwrap_err();
        assert!(refused.contains("revision 3, not 4"), "{refused}");
        assert_eq!(std::fs::read_dir(&run).unwrap().count(), 0);
        assert_eq!(Pin::load(&run).unwrap(), None);
    }
}
