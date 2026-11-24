//! `runbook.toml`: the file the binary finds by itself and the defaults it carries
//! (`docs/loom-run.md`, "Where a value comes from").
//!
//! The file is found in this order, the first hit wins and no two files are merged: `--config <file>`;
//! the variable `RUNBOOK_CONFIG`; the working directory and its parents; the user's configuration
//! directory (`$XDG_CONFIG_HOME/runbook/runbook.toml`, else `<home>/.config/runbook/runbook.toml`).
//! `--no-config` (and `RUNBOOK_NO_CONFIG`, which the runner gives every program of a run) reads none.
//!
//! A value comes from, in this order: an option of the command line; a `--settings` file (for the
//! members it states); `runbook.toml`; a built-in default where a wrong value does no harm. The run
//! settings members of the file (`autonomy`, `decider`, `models`, ...) are read by the one reader of
//! `loom_run/settings.rs`; this module reads only the keys that are not settings.
//!
//! What the file can never do, each held by a case in `tests/loom_run/config_file.rs`: print a line, a
//! value or a part of a value in a refusal; hold a key or a token (a login file is named by path, never
//! by content); lower what an option sets; be read from a tree a model wrote into.

use crate::loom_run::{bindings_copy, decider, settings, Confinement};
use clap::{Args, ValueEnum};
use serde_json::{Map, Value};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// The name of the file.
pub const FILE_NAME: &str = "runbook.toml";
/// The variable that names a file: the same checks as `--config`.
pub const ENV_CONFIG: &str = "RUNBOOK_CONFIG";
/// The variable that reads no file: what the runner gives every program it starts.
pub const ENV_NO_CONFIG: &str = "RUNBOOK_NO_CONFIG";
/// The most bytes of a file.
const MAX_BYTES: u64 = 256 * 1024;

/// The keys that are not run settings.
const OWN_KEYS: [&str; 3] = ["runs_dir", "spend_ledger", "runbooks"];
/// The keys that are an option of `runbook run` too; the file's value is the option's default.
const OPTION_KEYS: [&str; 4] = ["confinement", "cap_micro_usd", "max_llm_calls", "max_wall_seconds"];
/// The run settings members a file may carry (`docs/loom-run-settings.md`): `format`, `run`,
/// `loom_sha256`, `in_process` and `replay` belong to one run and are not defaults.
const MEMBER_KEYS: [&str; 7] = ["login_file", "autonomy", "decider", "adapter", "models", "efforts", "review"];

/// The starter `runbook init` writes (and the README shows): no comment, the stronger confinement.
pub const STARTER: &str = "runs_dir = \"runs\"\nconfinement = \"substrate\"\nmax_wall_seconds = 3600\ncap_micro_usd = 5000000\nmax_llm_calls = 200\n";

/// Where the file is named: the two options every command takes.
#[derive(Args, Clone, Debug, Default)]
pub struct Source {
    /// The `runbook.toml` to read (`RUNBOOK_CONFIG` also names one). Without both: the working directory and its parents, then the user's configuration directory.
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// Read no `runbook.toml` (`RUNBOOK_NO_CONFIG` does the same).
    #[arg(long, global = true)]
    pub no_config: bool,
}

/// The options of `runbook run` that have a default in the file, and that `runbook config` takes to
/// show where a value comes from.
#[derive(Args, Clone, Debug, Default)]
pub struct ValueOptions {
    /// Name of the run; the run directory is `<runs-dir>/<name>` and must not exist. Default: generated, unique, printed.
    #[arg(long, value_name = "NAME")]
    pub run: Option<String>,
    /// Directory that holds the run directories. Default: `runs/` beside the file (`runs_dir`).
    #[arg(long, value_name = "DIR")]
    pub runs_dir: Option<PathBuf>,
    /// The confinement of every bound program: `local` or `substrate`. No default (`confinement`).
    #[arg(long, value_enum, value_name = "LEVEL")]
    pub confinement: Option<Confinement>,
    /// A run settings file (TOML, `docs/loom-run-settings.md`); its members win over the file's.
    #[arg(long, value_name = "FILE")]
    pub settings: Option<PathBuf>,
    /// The spend ledger shared by every run that counts against the same cap. Default: in the directory of the runs (`spend_ledger`).
    #[arg(long, value_name = "FILE")]
    pub spend_ledger: Option<PathBuf>,
    /// The spend cap in micro USD over all runs that share the ledger (`cap_micro_usd`).
    #[arg(long, value_name = "N")]
    pub cap_micro_usd: Option<u64>,
    /// The most model calls of this run (`max_llm_calls`).
    #[arg(long, value_name = "N")]
    pub max_llm_calls: Option<u64>,
    /// The most wall seconds of this run. No default (`max_wall_seconds`).
    #[arg(long, value_name = "N")]
    pub max_wall_seconds: Option<u64>,
}

impl ValueOptions {
    /// The first option given, by its flag.
    pub fn first_given(&self) -> Option<&'static str> {
        [
            (self.run.is_some(), "--run"),
            (self.runs_dir.is_some(), "--runs-dir"),
            (self.confinement.is_some(), "--confinement"),
            (self.settings.is_some(), "--settings"),
            (self.spend_ledger.is_some(), "--spend-ledger"),
            (self.cap_micro_usd.is_some(), "--cap-micro-usd"),
            (self.max_llm_calls.is_some(), "--max-llm-calls"),
            (self.max_wall_seconds.is_some(), "--max-wall-seconds"),
        ]
        .into_iter()
        .find_map(|(given, flag)| given.then_some(flag))
    }
}

/// Everything the lookup reads from the process, so a test can give it a world of its own.
#[derive(Clone, Debug, Default)]
pub struct Lookup {
    pub option: Option<PathBuf>,
    pub no_config: bool,
    pub env_file: Option<PathBuf>,
    pub env_no_config: bool,
    pub cwd: Option<PathBuf>,
    pub xdg_config_home: Option<PathBuf>,
    pub home: Option<PathBuf>,
    /// The user the binary runs as; a file found by the walk or in the user's directory is theirs.
    pub uid: u32,
}

impl Lookup {
    pub fn from_process(source: &Source) -> Lookup {
        let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
        Lookup {
            option: source.config.clone(),
            no_config: source.no_config,
            env_file: var(ENV_CONFIG),
            env_no_config: var(ENV_NO_CONFIG).is_some(),
            cwd: std::env::current_dir().ok(),
            xdg_config_home: var("XDG_CONFIG_HOME"),
            home: var("HOME"),
            // SAFETY: geteuid(2) has no preconditions and cannot fail.
            uid: unsafe { libc::geteuid() },
        }
    }
}

/// How the file was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Via {
    Option,
    Variable,
    Walk,
    UserDirectory,
}

impl Via {
    pub fn describe(self) -> &'static str {
        match self {
            Via::Option => "--config",
            Via::Variable => "RUNBOOK_CONFIG",
            Via::Walk => "found in the working directory or a parent",
            Via::UserDirectory => "found in the user's configuration directory",
        }
    }

    /// A file the caller named: the same checks, except the owner.
    fn chosen(self) -> bool {
        matches!(self, Via::Option | Via::Variable)
    }
}

/// A file that passed the checks, with its text read from the handle the checks ran on.
#[derive(Clone, Debug)]
pub struct Found {
    pub path: PathBuf,
    pub via: Via,
    text: String,
}

/// The directory is a run's: the files a run holds (`events.jsonl`, `work/`, `bindings/`).
pub fn is_run_dir(dir: &Path) -> bool {
    dir.join("events.jsonl").is_file() && dir.join("work").is_dir() && dir.join("bindings").is_dir()
}

/// The most entries of a directory that are looked at for a run directory.
const RUNS_SCANNED: usize = 4096;

/// Whether the directory holds a run directory.
fn holds_run(dir: &Path) -> bool {
    dir.read_dir().is_ok_and(|entries| entries.take(RUNS_SCANNED).any(|e| e.is_ok_and(|e| is_run_dir(&e.path()))))
}

/// The daemon's state directory: the two files it writes there (`State::open`, `state.rs`).
fn is_daemon_state(dir: &Path) -> bool {
    dir.join("daemon.lock").is_file() && dir.join("journal.jsonl").is_file()
}

/// Why the walk does not read a directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skip {
    /// The directory is a run directory or lies inside one.
    InRun,
    /// The directory holds the run directory the working directory is in.
    RunsAbove,
    /// The directory holds a run directory and is the working directory.
    RunsHere,
    /// The directory is a workspace or a run of the daemon, or lies inside one.
    Daemon,
}

impl Skip {
    pub fn describe(self) -> &'static str {
        match self {
            Skip::InRun => "inside a run directory (it holds events.jsonl, work/ and bindings/)",
            Skip::RunsAbove => "the directory of runs that holds the run directory it is in",
            Skip::RunsHere => "the directory of runs it is started in (it holds a run directory)",
            Skip::Daemon => "inside a workspace or a run of the daemon (its state directory holds daemon.lock and journal.jsonl)",
        }
    }
}

/// The directories, from the first of `dirs` (the working directory) up, that the walk does not read,
/// each with why: the topmost run directory among them and the directory of runs above it; the working
/// directory when it holds a run; the topmost `<state>/workspaces/<id>` or `<state>/runs/<id>` of a daemon's
/// state directory and its `workspaces` or `runs` directory.
fn blocks(dirs: &[&Path]) -> Vec<Skip> {
    let run = dirs.iter().rposition(|d| is_run_dir(d)).map_or(0, |i| i + 2);
    let daemon = (0..dirs.len().saturating_sub(2))
        .rev()
        .find(|&i| dirs[i + 1].file_name().is_some_and(|n| n == "workspaces" || n == "runs") && is_daemon_state(dirs[i + 2]))
        .map_or(0, |i| i + 2);
    let here = usize::from(dirs.first().is_some_and(|d| holds_run(d)));
    (0..run.max(daemon).max(here).min(dirs.len()))
        .map(|i| {
            if i < daemon {
                Skip::Daemon
            } else if i + 1 == run {
                Skip::RunsAbove
            } else if i < run {
                Skip::InRun
            } else {
                Skip::RunsHere
            }
        })
        .collect()
}

/// Why the directory of `path` is not read, when it lies in a run's tree, in a directory of runs or in a
/// tree of the daemon.
fn tree_skip(path: &Path) -> Option<Skip> {
    let dirs: Vec<&Path> = path.parent().into_iter().flat_map(Path::ancestors).collect();
    blocks(&dirs).first().copied()
}

/// Whether `path` lies in a tree the walk never reads.
fn in_run_tree(path: &Path) -> bool {
    tree_skip(path).is_some()
}

/// The file at `path`, or `None` when a file found by the walk or in the user's directory is not
/// there. Every check runs on the opened handle: a regular file with one name, not writable by group
/// or others, owned by `uid` (not for a file the caller named), at most `MAX_BYTES`, UTF-8 text. A
/// file that is there and fails a check is refused by name, never skipped.
fn read_file(path: &Path, via: Via, uid: u32) -> Result<Option<Found>, String> {
    let refuse = |why: String| format!("{}: {why}", path.display());
    if !via.chosen() {
        match std::fs::symlink_metadata(path) {
            Ok(m) if m.file_type().is_symlink() => return Err(refuse("is a link: a file found by the walk or in the user's directory is a regular file with one name (name it with --config to use it)".to_owned())),
            Ok(_) => {}
            Err(e) if matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied) || e.raw_os_error() == Some(libc::ENOTDIR) => return Ok(None),
            Err(e) => return Err(refuse(e.to_string())),
        }
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true).custom_flags(libc::O_NONBLOCK | if via.chosen() { 0 } else { libc::O_NOFOLLOW });
    let file = options.open(path).map_err(|e| refuse(e.to_string()))?;
    let meta = file.metadata().map_err(|e| refuse(e.to_string()))?;
    if !meta.is_file() {
        return Err(refuse("is not a regular file".to_owned()));
    }
    if meta.nlink() != 1 {
        return Err(refuse(format!("has {} names (hard links): a configuration file has one name", meta.nlink())));
    }
    if meta.mode() & 0o022 != 0 {
        return Err(refuse(format!("is writable by group or others (mode {:04o}): `chmod go-w` it", meta.mode() & 0o7777)));
    }
    if !via.chosen() && meta.uid() != uid {
        return Err(refuse("is owned by another user: a file found by the walk or in the user's directory is the user's own (name it with --config to use it)".to_owned()));
    }
    if meta.len() > MAX_BYTES {
        return Err(refuse(format!("is larger than {MAX_BYTES} bytes")));
    }
    let mut bytes = Vec::new();
    (&file).take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|e| refuse(e.to_string()))?;
    let text = String::from_utf8(bytes).map_err(|_| refuse("is not UTF-8 text".to_owned()))?;
    Ok(Some(Found { path: path.to_path_buf(), via, text }))
}

/// The user's file: `$XDG_CONFIG_HOME/runbook/runbook.toml` (an absolute path only), else
/// `<home>/.config/runbook/runbook.toml`.
pub fn user_file(l: &Lookup) -> Option<PathBuf> {
    match (&l.xdg_config_home, &l.home) {
        (Some(x), _) if x.is_absolute() => Some(x.join("runbook").join(FILE_NAME)),
        (_, Some(h)) => Some(h.join(".config/runbook").join(FILE_NAME)),
        _ => None,
    }
}

/// The file to read, or `None`.
pub fn locate(l: &Lookup) -> Result<Option<Found>, String> {
    if l.option.is_some() && l.no_config {
        return Err("--config and --no-config exclude each other".to_owned());
    }
    if let Some(path) = &l.option {
        return read_file(path, Via::Option, l.uid);
    }
    if l.no_config {
        return Ok(None);
    }
    if let Some(path) = &l.env_file {
        return read_file(path, Via::Variable, l.uid);
    }
    if l.env_no_config {
        return Ok(None);
    }
    if let Some(cwd) = &l.cwd {
        let dirs: Vec<&Path> = cwd.ancestors().collect();
        for dir in dirs.iter().skip(blocks(&dirs).len()) {
            if let Some(found) = read_file(&dir.join(FILE_NAME), Via::Walk, l.uid)? {
                return Ok(Some(found));
            }
        }
    }
    match user_file(l) {
        Some(path) if !in_run_tree(&path) => read_file(&path, Via::UserDirectory, l.uid),
        _ => Ok(None),
    }
}

/// The files the walk found and did not read, each as a line with why (`runbook config` prints them): a
/// file in a run's tree, in a directory of runs or in a tree of the daemon.
pub fn skipped(l: &Lookup) -> Vec<String> {
    if l.option.is_some() || l.no_config || l.env_file.is_some() || l.env_no_config {
        return Vec::new();
    }
    let line = |file: PathBuf, why: Skip| std::fs::symlink_metadata(&file).ok().map(|_| format!("{}: not read: {}", file.display(), why.describe()));
    let walked: Vec<String> = l.cwd.iter().flat_map(|cwd| {
        let dirs: Vec<&Path> = cwd.ancestors().collect();
        blocks(&dirs).into_iter().zip(dirs.iter()).filter_map(|(why, dir)| line(dir.join(FILE_NAME), why)).collect::<Vec<_>>()
    }).collect();
    let user = user_file(l).and_then(|path| tree_skip(&path).and_then(|why| line(path, why)));
    walked.into_iter().chain(user).collect()
}

/// A file that is read, as the run reads it.
#[derive(Clone, Debug)]
pub struct File {
    pub path: PathBuf,
    /// How the file was found.
    pub via: Via,
    pub runs_dir: Option<PathBuf>,
    pub spend_ledger: Option<PathBuf>,
    pub confinement: Option<Confinement>,
    pub cap_micro_usd: Option<u64>,
    pub max_llm_calls: Option<u64>,
    pub max_wall_seconds: Option<u64>,
    /// The runbook directories the file names, in its order, each made absolute against the file.
    pub runbooks: Vec<PathBuf>,
    /// The run settings members, in file order, as `loom_run/settings.rs` reads them.
    pub members: Map<String, Value>,
}

impl File {
    /// Whether the file states no key: it took no value for a run, so a run does not name it.
    pub fn states_nothing(&self) -> bool {
        self.runs_dir.is_none()
            && self.spend_ledger.is_none()
            && self.confinement.is_none()
            && self.cap_micro_usd.is_none()
            && self.max_llm_calls.is_none()
            && self.max_wall_seconds.is_none()
            && self.runbooks.is_empty()
            && self.members.is_empty()
    }

    /// What `runbook run` says on standard error before anything starts: the file and how it was found.
    /// The path of a file and nothing of its content; `None` for a file that took no value.
    pub fn said(&self) -> Option<String> {
        (!self.states_nothing()).then(|| format!("values from runbook.toml {} ({})", self.path.display(), self.via.describe()))
    }

    /// The same for the start record of a run: name and origin, never the path (a record leaves the
    /// machine with the run).
    pub fn noted(&self) -> Option<String> {
        (!self.states_nothing()).then(|| format!("{FILE_NAME} ({})", self.via.describe()))
    }
}

/// Locates and reads the file the process names or finds.
pub fn load(source: &Source) -> Result<Option<File>, String> {
    locate(&Lookup::from_process(source))?.map(|f| parse(&f)).transpose()
}

/// A character of a word of a text: a key, a token and a path segment are words.
fn word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+' | '=')
}

/// Whether a word is `bearer`, in any case: the word after it is a credential.
fn is_bearer(word: &str) -> bool {
    word.eq_ignore_ascii_case("bearer")
}

/// Whether the text has a word `bearer` (any case) that another word follows, wherever it stands.
fn bearer_value(s: &str) -> bool {
    let words: Vec<&str> = s.split(|c: char| !word_char(c)).filter(|w| !w.is_empty()).collect();
    words.windows(2).any(|pair| is_bearer(pair[0]))
}

/// Whether the text has the form of a key or a token: the forms of the vendors' keys, a private key
/// block, a JSON web token, a bearer value (the word `bearer` and the word after it, anywhere in the text).
pub fn token_form(s: &str) -> bool {
    const PREFIXES: [&str; 14] = ["sk-ant-", "ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_", "glpat-", "xoxb-", "xoxp-", "xoxa-", "xoxs-", "xapp-", "npm_"];
    let t = s.trim();
    let aws = (t.starts_with("AKIA") || t.starts_with("ASIA")) && t.len() == 20 && t.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    let jwt = t.starts_with("eyJ") && t.matches('.').count() == 2;
    let generic = t.starts_with("sk-") && t.len() >= 20;
    let google = t.starts_with("AIza") && t.len() == 39;
    PREFIXES.iter().any(|p| t.starts_with(p)) || t.contains("sk-ant-") || t.contains("-----BEGIN") || bearer_value(t) || aws || jwt || generic || google
}

/// A path as text, not content: no control character (a newline) and no JSON or array opening.
fn content_like(s: &str) -> bool {
    s.chars().any(char::is_control) || s.trim_start().starts_with(['{', '['])
}

/// Walks every key and string of the file; a refusal names the table and the key, never the value.
fn scan(v: &Value, at: &str) -> Result<(), String> {
    match v {
        Value::Object(map) => {
            for (key, x) in map {
                let here = if at.is_empty() { key.clone() } else { format!("{at}.{key}") };
                if token_form(key) {
                    let table = if at.is_empty() { "the top level".to_owned() } else { at.to_owned() };
                    return Err(format!("{table}: a key has the form of a key or a token: the file names a login file by path, never by content"));
                }
                scan(x, &here)?;
            }
            Ok(())
        }
        Value::Array(items) => items.iter().enumerate().try_for_each(|(i, x)| scan(x, &format!("{at}[{i}]"))),
        Value::String(s) if token_form(s) => Err(format!("{at}: the value has the form of a key or a token: the file names a login file by path, never by content")),
        _ => Ok(()),
    }
}

/// What is printed in place of a word or a value that has the form of a key or a token.
const HIDDEN: &str = "<a key or token>";

/// The message with every word that has the form of a key or a token replaced: a key of the file can
/// be one, and the reader names the key it stops at.
fn mask(message: &str) -> String {
    let mut out = String::new();
    let mut current = String::new();
    // the word `bearer` makes the next word hidden, whatever stands between the two
    let mut after_bearer = false;
    for c in message.chars().chain(std::iter::once(' ')) {
        if word_char(c) {
            current.push(c);
            continue;
        }
        if current.is_empty() {
            out.push(c);
            continue;
        }
        let word = std::mem::take(&mut current);
        out.push_str(if after_bearer || token_form(&word) { HIDDEN } else { &word });
        after_bearer = is_bearer(&word);
        out.push(c);
    }
    out.pop();
    out
}

/// The leading key of a message of the settings readers: those can print a value, so only the key
/// they begin with is kept.
fn scrub(message: String) -> String {
    let key: String = message.split(|c: char| c == ':' || c.is_whitespace()).next().unwrap_or("").chars().take(120).collect();
    format!("{key}: refused (the reader's own words are not printed here, they can hold a value; the form: docs/loom-run-settings.md)")
}

fn path_in(top: &mut Map<String, Value>, key: &str, dir: &Path) -> Result<Option<PathBuf>, String> {
    match top.shift_remove(key) {
        None => Ok(None),
        Some(Value::String(s)) if !s.is_empty() && !content_like(&s) => Ok(Some(if Path::new(&s).is_absolute() { PathBuf::from(s) } else { dir.join(s) })),
        Some(_) => Err(format!("{key}: a path as text (a path, never content)")),
    }
}

/// The most runbook directories a file names.
const MAX_RUNBOOK_DIRS: usize = 64;

/// `runbooks = ["dir", ...]`: a list of paths as text, each absolute or relative to the file.
fn runbook_dirs(top: &mut Map<String, Value>, dir: &Path) -> Result<Vec<PathBuf>, String> {
    let wrong = || "runbooks: a list of directories as text (a path, never content)".to_owned();
    let Some(listed) = top.shift_remove("runbooks") else { return Ok(Vec::new()) };
    let Value::Array(items) = listed else { return Err(wrong()) };
    if items.len() > MAX_RUNBOOK_DIRS {
        return Err(format!("runbooks: at most {MAX_RUNBOOK_DIRS} directories"));
    }
    items
        .into_iter()
        .map(|item| match item {
            Value::String(s) if !s.is_empty() && !content_like(&s) => Ok(if Path::new(&s).is_absolute() { PathBuf::from(s) } else { dir.join(s) }),
            _ => Err(wrong()),
        })
        .collect()
}

fn whole(top: &mut Map<String, Value>, key: &str) -> Result<Option<u64>, String> {
    match top.shift_remove(key) {
        None => Ok(None),
        Some(v) => v.as_u64().map(Some).ok_or_else(|| format!("{key}: a whole number")),
    }
}

/// Reads the text of a found file: the one settings reader for the syntax and the members, then the
/// keys of its own. An error names the file, the table and the key, and a line at most.
pub fn parse(found: &Found) -> Result<File, String> {
    let name = found.path.display();
    let fail = |e: String| format!("{name}: {e}");
    let dir = found.path.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut top = match settings::parse(&found.text).map_err(|e| fail(mask(&e)))? {
        Value::Object(m) => m,
        _ => return Err(fail("a table".to_owned())),
    };
    scan(&Value::Object(top.clone()), "").map_err(fail)?;
    for key in top.keys() {
        if !OWN_KEYS.contains(&key.as_str()) && !OPTION_KEYS.contains(&key.as_str()) && !MEMBER_KEYS.contains(&key.as_str()) {
            let known: Vec<&str> = OWN_KEYS.iter().chain(&OPTION_KEYS).chain(&MEMBER_KEYS).copied().collect();
            return Err(fail(format!("{key}: not a key of runbook.toml (known: {})", known.join(", "))));
        }
    }
    let runs_dir = path_in(&mut top, "runs_dir", &dir).map_err(fail)?;
    let spend_ledger = path_in(&mut top, "spend_ledger", &dir).map_err(fail)?;
    let confinement = match top.shift_remove("confinement") {
        None => None,
        Some(Value::String(s)) => Some(Confinement::from_str(&s, false).map_err(|_| fail("confinement: not a level (local or substrate)".to_owned()))?),
        Some(_) => return Err(fail("confinement: not a level (local or substrate)".to_owned())),
    };
    let cap_micro_usd = whole(&mut top, "cap_micro_usd").map_err(fail)?;
    let max_llm_calls = whole(&mut top, "max_llm_calls").map_err(fail)?;
    let max_wall_seconds = whole(&mut top, "max_wall_seconds").map_err(fail)?;
    if let Some(login) = top.get("login_file").cloned() {
        match login {
            Value::String(s) if !s.is_empty() && !content_like(&s) => {
                if !Path::new(&s).is_absolute() {
                    top.insert("login_file".to_owned(), Value::String(dir.join(&s).display().to_string()));
                }
            }
            _ => return Err(fail("login_file: a path as text (a path, never content)".to_owned())),
        }
    }
    if let Some(reply) = top.get_mut("decider").and_then(Value::as_object_mut).and_then(|d| d.get_mut("reply_bin")) {
        match reply {
            Value::String(s) if !s.is_empty() && !content_like(s) => {
                if !Path::new(s.as_str()).is_absolute() {
                    *s = dir.join(s.as_str()).display().to_string();
                }
            }
            _ => return Err(fail("decider.reply_bin: a path as text".to_owned())),
        }
    }
    let runbooks = runbook_dirs(&mut top, &dir).map_err(fail)?;
    check_members(&top).map_err(fail)?;
    Ok(File { path: found.path.clone(), via: found.via, runs_dir, spend_ledger, confinement, cap_micro_usd, max_llm_calls, max_wall_seconds, runbooks, members: top })
}

/// The members through the readers of a run: the keys of every table, the kinds, the decider. The
/// login file is named by path and is not opened here.
fn check_members(members: &Map<String, Value>) -> Result<(), String> {
    let mut v = members.clone();
    v.shift_remove(settings::LOGIN_KEY);
    let mut v = Value::Object(v);
    bindings_copy::check(&v).map_err(scrub)?;
    settings::fill_decider(&mut v, None).map_err(scrub)?;
    decider::from_settings(&v).map(|_| ()).map_err(scrub)
}

/// Where a value comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    Option(&'static str),
    Settings(PathBuf),
    File(PathBuf),
    BuiltIn(&'static str),
}

impl Origin {
    pub fn text(&self) -> String {
        match self {
            Origin::Option(flag) => format!("option {flag}"),
            Origin::Settings(path) => format!("settings {}", path.display()),
            Origin::File(path) => format!("file {}", path.display()),
            Origin::BuiltIn(what) => format!("built-in: {what}"),
        }
    }
}

type Item<T> = Option<(T, Origin)>;

/// The effective values of the options of a run, each with its origin; `None` is "not stated", a state
/// of its own.
#[derive(Clone, Debug)]
pub struct Values {
    pub file: Option<PathBuf>,
    /// `None`: the name is generated when the run starts.
    pub run: Item<String>,
    pub runs_dir: (PathBuf, Origin),
    pub spend_ledger: (PathBuf, Origin),
    pub confinement: Item<Confinement>,
    pub cap_micro_usd: Item<u64>,
    pub max_llm_calls: Item<u64>,
    pub max_wall_seconds: Item<u64>,
    /// The runbook directories of the file; the built-in set follows them.
    pub runbooks: Vec<PathBuf>,
    pub settings: Option<PathBuf>,
    /// The members of the file, for the settings reader to merge under `--settings`.
    pub members: Map<String, Value>,
}

fn pick<T: Clone>(option: Option<T>, flag: &'static str, file: Option<T>, path: Option<&Path>) -> Item<T> {
    match (option, file, path) {
        (Some(v), _, _) => Some((v, Origin::Option(flag))),
        (None, Some(v), Some(p)) => Some((v, Origin::File(p.to_path_buf()))),
        _ => None,
    }
}

/// The values: an option, else the file, else the built-in default where there is one.
pub fn resolve(options: &ValueOptions, file: Option<&File>, cwd: &Path) -> Values {
    let path = file.map(|f| f.path.as_path());
    let runs_dir = pick(options.runs_dir.clone(), "--runs-dir", file.and_then(|f| f.runs_dir.clone()), path).unwrap_or_else(|| match path.and_then(Path::parent) {
        Some(dir) => (dir.join("runs"), Origin::BuiltIn("runs/ beside the file")),
        None => (cwd.join("runs"), Origin::BuiltIn("runs/ in the working directory")),
    });
    let spend_ledger = pick(options.spend_ledger.clone(), "--spend-ledger", file.and_then(|f| f.spend_ledger.clone()), path)
        .unwrap_or_else(|| (runs_dir.0.join("spend-ledger.json"), Origin::BuiltIn("in the directory of the runs")));
    Values {
        file: path.map(Path::to_path_buf),
        run: options.run.clone().map(|r| (r, Origin::Option("--run"))),
        runs_dir,
        spend_ledger,
        confinement: pick(options.confinement, "--confinement", file.and_then(|f| f.confinement), path),
        cap_micro_usd: pick(options.cap_micro_usd, "--cap-micro-usd", file.and_then(|f| f.cap_micro_usd), path),
        max_llm_calls: pick(options.max_llm_calls, "--max-llm-calls", file.and_then(|f| f.max_llm_calls), path),
        max_wall_seconds: pick(options.max_wall_seconds, "--max-wall-seconds", file.and_then(|f| f.max_wall_seconds), path),
        runbooks: file.map(|f| f.runbooks.clone()).unwrap_or_default(),
        settings: options.settings.clone(),
        members: file.map(|f| f.members.clone()).unwrap_or_default(),
    }
}

/// The settings a run reads: the members of `runbook.toml`, each replaced as a whole by the member of
/// the `--settings` file that has the same name (a table is not merged with a table), in the order of
/// the file, the new members after them.
pub fn merge(file: &Map<String, Value>, settings_file: Option<&Value>) -> Value {
    let mut merged = file.clone();
    if let Some(over) = settings_file.and_then(Value::as_object) {
        for (key, member) in over {
            merged.insert(key.clone(), member.clone());
        }
    }
    Value::Object(merged)
}

/// A name for a run that has none: the runbook, a time and a random part.
pub fn generated_run_name(runbook: &str) -> String {
    let plain: String = runbook.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') { c } else { '-' }).collect();
    let id = uuid::Uuid::now_v7().simple().to_string();
    format!("{plain}-{}-{}", &id[..12], &id[20..])
}

/// The text a value has in `runbook config`. The one place a value is printed: a string that has the form
/// of a key or a token, wherever it is in the value and whichever source it comes from, is replaced by
/// `HIDDEN`; the key and the origin stay.
fn shown(v: &Value) -> String {
    fn hide(v: &Value) -> Value {
        match v {
            Value::String(s) if token_form(s) => Value::String(HIDDEN.to_owned()),
            Value::Array(items) => Value::Array(items.iter().map(hide).collect()),
            Value::Object(map) => Value::Object(map.iter().map(|(k, x)| (k.clone(), hide(x))).collect()),
            other => other.clone(),
        }
    }
    hide(v).to_string()
}

/// A key of a member as one segment of the dotted name printed for a value: a key that has the form of a
/// token is replaced by `HIDDEN` (the prefix before a dot would hide it from `mask`).
fn segment(key: &str) -> &str {
    if token_form(key) {
        HIDDEN
    } else {
        key
    }
}

fn leaves(prefix: &str, v: &Value, origin: &Origin, out: &mut Vec<String>) {
    match v {
        Value::Object(map) if !map.is_empty() => map.iter().for_each(|(k, x)| leaves(&format!("{prefix}.{}", segment(k)), x, origin, out)),
        Value::Object(_) => {}
        other => out.push(format!("{prefix} = {}  ({})", shown(other), origin.text())),
    }
}

/// The text `runbook config` prints: the file and how it was found, then every value with its origin.
pub fn text(found: Option<(&Path, Via)>, skipped: &[String], source: &Source, values: &Values, merged: &Value, settings_origin: &dyn Fn(&str) -> Origin) -> String {
    let mut out = Vec::new();
    out.push(match (found, source.no_config) {
        (Some((path, via)), _) => format!("runbook.toml: {} ({})", path.display(), via.describe()),
        (None, true) => "runbook.toml: none read (--no-config)".to_owned(),
        (None, false) => "runbook.toml: none found (the working directory and its parents, the user's configuration directory; `runbook init` writes one)".to_owned(),
    });
    out.extend(skipped.iter().map(|s| format!("runbook.toml skipped: {s}")));
    let item = |key: &str, flag: &str, what: &str, v: Option<(String, &Origin)>| match v {
        Some((value, origin)) => format!("{key} = {value}  ({})", origin.text()),
        None => format!("{key} = not stated  ({what}: {flag}, or `{key}` in runbook.toml)"),
    };
    out.push(match &values.run {
        Some((r, o)) => format!("run = {}  ({})", shown(&Value::from(r.as_str())), o.text()),
        None => "run = generated when the run starts  (built-in: unique, printed)".to_owned(),
    });
    out.push(format!("runs_dir = {}  ({})", shown(&Value::from(values.runs_dir.0.display().to_string())), values.runs_dir.1.text()));
    out.push(format!("spend_ledger = {}  ({})", shown(&Value::from(values.spend_ledger.0.display().to_string())), values.spend_ledger.1.text()));
    out.push(match (&values.file, values.runbooks.is_empty()) {
        (Some(file), false) => format!("runbooks = {}  (file {})", shown(&Value::from(values.runbooks.iter().map(|d| d.display().to_string()).collect::<Vec<_>>())), file.display()),
        _ => "runbooks = []  (built-in: the built-in set only; `runbooks` in runbook.toml adds directories before it)".to_owned(),
    });
    out.push(item("confinement", "--confinement", "no built-in default, a run is refused without it", values.confinement.as_ref().map(|(c, o)| (shown(&Value::from(c.as_arg())), o))));
    out.push(item("max_wall_seconds", "--max-wall-seconds", "no built-in default, a run is refused without it", values.max_wall_seconds.as_ref().map(|(n, o)| (n.to_string(), o))));
    out.push(item("cap_micro_usd", "--cap-micro-usd", "a run is refused without it", values.cap_micro_usd.as_ref().map(|(n, o)| (n.to_string(), o))));
    out.push(item("max_llm_calls", "--max-llm-calls", "a run is refused without it", values.max_llm_calls.as_ref().map(|(n, o)| (n.to_string(), o))));
    if let Some(members) = merged.as_object() {
        for (key, v) in members {
            leaves(segment(key), v, &settings_origin(key), &mut out);
        }
    }
    // a key of a settings file and a path of an option are no values and can have the form too
    out.iter().map(|line| mask(line)).collect::<Vec<_>>().join("\n") + "\n"
}

/// `runbook config`: the effective values and where each comes from. Never opens a login file.
pub fn show(source: &Source, options: &ValueOptions) -> Result<String, String> {
    let lookup = Lookup::from_process(source);
    let found = locate(&lookup)?;
    let file = found.as_ref().map(parse).transpose()?;
    let cwd = lookup.cwd.clone().unwrap_or_default();
    let values = resolve(options, file.as_ref(), &cwd);
    let over = values.settings.as_deref().map(settings::read).transpose()?;
    let merged = merge(&values.members, over.as_ref());
    let stated: Vec<String> = over.as_ref().and_then(Value::as_object).map(|m| m.keys().cloned().collect()).unwrap_or_default();
    let file_path = values.file.clone();
    let settings_path = values.settings.clone();
    let origin = move |key: &str| match (&settings_path, stated.iter().any(|k| k == key), &file_path) {
        (Some(s), true, _) => Origin::Settings(s.clone()),
        (_, _, Some(f)) => Origin::File(f.clone()),
        _ => Origin::BuiltIn("none"),
    };
    Ok(text(found.as_ref().map(|f| (f.path.as_path(), f.via)), &skipped(&lookup), source, &values, &merged, &origin))
}

/// `runbook init`: writes the starter, mode 600, and refuses to overwrite.
pub fn init(path: Option<&Path>) -> Result<PathBuf, String> {
    use std::io::Write;
    let path = path.map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from(FILE_NAME));
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            format!("{} exists: it is not overwritten (take it out first, or edit it)", path.display())
        } else {
            format!("{}: {e}", path.display())
        }
    })?;
    file.write_all(STARTER.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::testdir::Dir;
    use std::os::unix::fs::PermissionsExt;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    fn lookup(t: &Dir) -> Lookup {
        Lookup { cwd: Some(t.dir("cwd")), home: Some(t.dir("home")), xdg_config_home: None, uid: unsafe { libc::geteuid() }, ..Lookup::default() }
    }

    #[test]
    fn a_file_found_by_the_walk_or_in_the_users_directory_is_the_users_own() {
        let t = Dir::new("config-owner");
        write(&t.path().join("cwd/runbook.toml"), "max_wall_seconds = 1\n");
        let mut l = lookup(&t);
        assert!(locate(&l).unwrap().is_some());
        l.uid += 1;
        let e = locate(&l).unwrap_err();
        assert!(e.contains("owned by another user") && e.contains("runbook.toml"), "{e}");
        // a file the caller names is not held to the owner
        l.option = Some(t.path().join("cwd/runbook.toml"));
        assert!(locate(&l).unwrap().is_some());
        // the user's directory is the user's too
        std::fs::remove_file(t.path().join("cwd/runbook.toml")).unwrap();
        l.option = None;
        write(&t.path().join("home/.config/runbook/runbook.toml"), "max_wall_seconds = 2\n");
        let e = locate(&l).unwrap_err();
        assert!(e.contains("owned by another user"), "{e}");
    }

    #[test]
    fn the_token_forms_and_the_plain_texts() {
        let yes: Vec<String> = [["sk-", "ant-api03-x"], ["gh", "p_x"], ["-----BEGIN ", "PRIVATE KEY-----"], ["AK", "IAABCDEFGHIJKLMNOP"], ["ey", "Ja.eyJb.c"], ["Bear", "er abc"], ["Authorization: Bear", "er abc"], ["curl -H 'x: BEAR", "ER abc'"], ["s", "k-aaaaaaaaaaaaaaaaaaaaaaaa"]].iter().map(|p| p.concat()).collect();
        for yes in &yes {
            assert!(token_form(yes), "{yes}");
        }
        for no in ["claude-sonnet-5-5", "/srv/x/.claude/login.json", "runs", "A5", "substrate", "sk-short", "ask-ant", "bearer", "bearers abc", "a bear of tea"] {
            assert!(!token_form(no), "{no}");
        }
    }

    #[test]
    fn the_run_names_are_unique_plain_and_carry_the_runbook() {
        let a = generated_run_name("sdc");
        let b = generated_run_name("sdc");
        assert_ne!(a, b);
        assert!(a.starts_with("sdc-") && a.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'), "{a}");
        assert!(generated_run_name("a/b c").starts_with("a-b-c-"));
    }

    #[test]
    fn the_settings_file_replaces_a_member_as_a_whole_and_keeps_the_order() {
        let file: Map<String, Value> = serde_json::from_str(r#"{"autonomy": "A5", "decider": {"kind": "rule", "verdict": "approve"}, "adapter": "dry"}"#).unwrap();
        let over: Value = serde_json::from_str(r#"{"decider": {"kind": "rule"}, "models": {"writer": {"model": "m"}}}"#).unwrap();
        let merged = merge(&file, Some(&over));
        let keys: Vec<&String> = merged.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["autonomy", "decider", "adapter", "models"]);
        assert_eq!(merged["decider"], serde_json::json!({"kind": "rule"}));
    }

    #[test]
    fn the_starter_is_read() {
        let found = Found { path: PathBuf::from("/x/runbook.toml"), via: Via::Option, text: STARTER.to_owned() };
        let f = parse(&found).unwrap();
        assert_eq!((f.confinement, f.max_wall_seconds, f.cap_micro_usd, f.max_llm_calls), (Some(Confinement::Substrate), Some(3600), Some(5_000_000), Some(200)));
        assert_eq!(f.runs_dir, Some(PathBuf::from("/x/runs")));
    }
}
