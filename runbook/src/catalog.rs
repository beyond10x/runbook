//! The runbooks of this host as `list`, `run`, `serve`, `check` and `doctor --for` see them: the
//! directories `runbook.toml` names, then the built-in set (`assets.rs`), read by the one function
//! `runbook::runbooks::load`.

use crate::assets::{Assets, Lookup};
use crate::config;
use runbook::runbooks::{self, Entry};
use std::path::PathBuf;

/// What was found: the directories the file names, the asset root and every protocol of them.
pub struct World {
    pub configured: Vec<PathBuf>,
    pub assets: Option<Assets>,
    /// Why there is no asset root, when there is none.
    pub missing: Option<String>,
    pub entries: Vec<Entry>,
}

/// Why it could not be opened: the exit code (2 for the configuration file, 1 for the directories) and
/// the words.
pub type Failure = (u8, String);

/// Reads `runbook.toml` as the process finds it, then the directories.
pub fn open(source: &config::Source) -> Result<World, Failure> {
    let file = config::load(source).map_err(|e| (2, e))?;
    open_with(file, &Lookup::from_process())
}

/// The directories of `file` and the built-in set found by `lookup`. A built-in set that is not there
/// is no error here (`World::need_some` makes it one where that matters): the directories of the file
/// are enough for a host that has no installed assets.
pub fn open_with(file: Option<config::File>, lookup: &Lookup) -> Result<World, Failure> {
    let configured = file.as_ref().map(|f| f.runbooks.clone()).unwrap_or_default();
    let (assets, missing) = match lookup.locate() {
        Ok(a) => (Some(a), None),
        Err(e) => (None, Some(e)),
    };
    let entries = runbooks::load(&configured, assets.as_ref().map(|a| a.root.as_path())).map_err(|e| (1, e))?;
    Ok(World { configured, assets, missing, entries })
}

impl World {
    /// An error when there is neither a configured directory nor a built-in set.
    pub fn need_some(self) -> Result<World, Failure> {
        match (&self.missing, self.configured.is_empty()) {
            (Some(why), true) => Err((1, why.clone())),
            _ => Ok(self),
        }
    }

    /// The directories searched, for a message.
    pub fn searched(&self) -> String {
        runbooks::searched(&self.configured, self.assets.as_ref().map(|a| a.root.as_path()))
    }

    /// The runbooks a person can start, by name.
    pub fn startable(&self) -> Vec<&str> {
        runbooks::startable(&self.entries)
    }
}
