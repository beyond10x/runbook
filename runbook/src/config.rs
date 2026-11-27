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
/// The variable Loom gives every bound program of a run (its program contract): a `runbook` that has it is
/// started inside a run.
pub const ENV_PROGRAM_OF_A_RUN: &str = "LOOM_CASE";
/// The most bytes of a file.
const MAX_BYTES: u64 = 256 * 1024;

/// The keys that are not run settings.
const OWN_KEYS: [&str; 9] = ["runs_dir", "spend_ledger", "runbooks", "deciders", runbook::routes::GATEWAYS, runbook::routes::ROUTES, runbook::routes::POLICY, DELEGATION, runbook::handback::REPOSITORIES];
/// The table of the daemon's delegations (`classify_threshold`).
const DELEGATION: &str = "delegation";
/// The keys that are an option of `runbook run` too; the file's value is the option's default.
const OPTION_KEYS: [&str; 4] = ["confinement", "cap_micro_usd", "max_llm_calls", "max_wall_seconds"];
/// The run settings members a file may carry (`docs/loom-run-settings.md`): `format`, `run`,
/// `loom_sha256`, `in_process` and `replay` belong to one run and are not defaults.
/// `gateway` takes `base_url` and `token_file` here, never `token`: the file holds no token.
const MEMBER_KEYS: [&str; 8] = ["login_file", "gateway", "autonomy", "decider", "adapter", "models", "efforts", "review"];

/// The keys a file found by a program of a run that has no run directory above it (by the walk or in the user's
/// directory) does not have: the levels (`confinement`, `autonomy`), the caps, the spend ledger, the directories
/// runbooks are loaded from, the gateway of the model programs and the model routes. The floor of the daemon has no
/// key in any file.
const NESTED_REFUSED_KEYS: [&str; 12] =
    ["confinement", "autonomy", "cap_micro_usd", "max_llm_calls", "max_wall_seconds", "spend_ledger", "runbooks", "gateway", runbook::routes::GATEWAYS, runbook::routes::ROUTES, runbook::routes::POLICY, runbook::handback::REPOSITORIES];

/// The starter `runbook init` writes (and the README shows): no comment, the stronger confinement, and no cap, call
/// limit or ledger (`AGENTS.md`: no default that requires or implies a cap).
pub const STARTER: &str = "runs_dir = \"runs\"\nconfinement = \"substrate\"\nmax_wall_seconds = 3600\n";

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
    /// Record the spend in this ledger file, shared by every run that names it (`spend_ledger`); without it none is opened.
    #[arg(long, value_name = "FILE")]
    pub spend_ledger: Option<PathBuf>,
    /// Stop the run when this many micro USD are spent (over all runs that share the ledger, or over this run alone without one); 0 spends nothing (`cap_micro_usd`). Without it, no spend limit.
    #[arg(long, value_name = "N")]
    pub cap_micro_usd: Option<u64>,
    /// Stop the run after this many model calls (steps and decisions together); 0 allows none (`max_llm_calls`). Without it, no limit.
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
    /// The process is a program of a run: Loom gives every bound program `LOOM_CASE`.
    pub nested: bool,
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
            nested: var(ENV_PROGRAM_OF_A_RUN).is_some(),
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
    /// The file was found by a program of a run that has no run directory above its working directory: the
    /// file lies in the tree a model wrote into.
    nested: bool,
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
    Ok(Some(Found { path: path.to_path_buf(), via, text, nested: false }))
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
    // a program of a run with no run directory above it (the work directory bound at another path): every file
    // it finds lies in the tree a model wrote into; with the run directory above it the walk skips that tree
    let nested = l.nested && !l.cwd.as_ref().is_some_and(|cwd| cwd.ancestors().any(is_run_dir));
    locate_file(l).map(|found| found.map(|f| Found { nested, ..f }))
}

fn locate_file(l: &Lookup) -> Result<Option<Found>, String> {
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
    /// `[deciders.<kind>]`: the deciders `runbook serve` offers its runs, one table per kind. `runbook run`
    /// does not read them (it has the one `decider` of its settings); the daemon's arbiter refuses a table
    /// it cannot take, by name, when it starts.
    pub deciders: Map<String, Value>,
    /// The run settings members, in file order, as `loom_run/settings.rs` reads them.
    pub members: Map<String, Value>,
    /// `[gateways.<name>]`, `[routes.<name>]` and `[route_policy]`, with a lone `[gateway]` as the gateway `default`:
    /// the model routes `runbook serve` declares. Checked here, its token files not read: the daemon reads them when
    /// it starts. `runbook run` does not read them yet.
    pub routes: runbook::routes::RouteTable,
    /// `[delegation] classify_threshold`: the confidence, from 0 to 1, at or above which a classified delegation runs the
    /// runbook the classifier picked (`docs/model-routes.md`, section 9.1). `runbook serve` reads it.
    pub classify_threshold: Option<f64>,
    /// `[repositories.<name>]`: the repositories a delegation may change (`runbook::handback`), none by default. `runbook
    /// serve` reads them; their token files are read when it starts.
    pub repositories: runbook::handback::Repositories,
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
            && self.routes.is_empty()
            && self.classify_threshold.is_none()
            && self.repositories.is_empty()
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
    locate(&Lookup::from_process(source)).map_err(|e| mask(&e))?.map(|f| parse(&f)).transpose()
}

/// The one wrapper every refusal passes through before a command prints it (`config`, `list`, `check`, `run`,
/// `doctor`, `serve`): a word with the form of a key or a token is replaced, whatever joins it to the text around it.
pub fn printable(message: &str) -> String {
    mask(message)
}

/// A character of a word of a text: a key, a token and a path segment are words.
fn word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+' | '=')
}

/// Whether a word ends in `bearer`, in any case (`AUTH=Bearer` does): the word after it is a credential.
fn is_bearer(word: &str) -> bool {
    word.split('=').rfind(|part| !part.is_empty()).is_some_and(|part| part.eq_ignore_ascii_case("bearer"))
}

/// Whether the text has a word `bearer` (any case) that another word follows, wherever it stands, or the
/// word `bearer` and a value joined by an equals sign (`Bearer=<value>`).
fn bearer_value(s: &str) -> bool {
    let words: Vec<&str> = s.split(|c: char| !word_char(c)).filter(|w| !w.is_empty()).collect();
    words.windows(2).any(|pair| is_bearer(pair[0]))
        || words.iter().any(|w| w.split('=').collect::<Vec<_>>().windows(2).any(|pair| pair[0].eq_ignore_ascii_case("bearer") && !pair[1].is_empty()))
}

/// Whether the text begins with the form of a key or a token, or holds a private key block.
fn form_at_start(s: &str) -> bool {
    const PREFIXES: [&str; 14] = ["sk-ant-", "ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_", "glpat-", "xoxb-", "xoxp-", "xoxa-", "xoxs-", "xapp-", "npm_"];
    let t = s.trim();
    let aws = (t.starts_with("AKIA") || t.starts_with("ASIA")) && t.len() == 20 && t.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    let jwt = t.starts_with("eyJ") && t.matches('.').count() == 2;
    let generic = t.starts_with("sk-") && t.len() >= 20;
    let google = t.starts_with("AIza") && t.len() == 39;
    PREFIXES.iter().any(|p| t.starts_with(p)) || t.contains("sk-ant-") || t.contains("-----BEGIN") || aws || jwt || generic || google
}

/// Whether the text has the form of a key or a token: the forms of the vendors' keys, a private key
/// block, a JSON web token, a bearer value (the word `bearer` and the word after it, anywhere in the text),
/// each as the whole text and as any part of it that follows a name and an equals sign (`GH_TOKEN=<token>`)
/// or a dot.
pub fn token_form(s: &str) -> bool {
    form_at_start(s) || bearer_value(s) || joined_form(s)
}

/// The most segments a form spans: a web token has three, joined by dots.
const FORM_SEGMENTS: usize = 3;

/// Whether a form begins at the start of any segment of the text, the segments being what stands between dots and
/// equals signs, and spans up to `FORM_SEGMENTS` of them (`a.b=c.<web token>`).
fn joined_form(s: &str) -> bool {
    let ends: Vec<usize> = s.match_indices(['.', '=']).map(|(i, _)| i).chain(std::iter::once(s.len())).collect();
    let starts = std::iter::once(0).chain(ends.iter().map(|e| e + 1));
    starts.zip(0..ends.len()).any(|(from, k)| (k..ends.len().min(k + FORM_SEGMENTS)).any(|e| s.get(from..ends[e]).is_some_and(form_at_start)))
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
                if mask(key) != *key {
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
    let chars: Vec<char> = message.chars().collect();
    let mut out = String::new();
    let mut at = 0;
    // the word `bearer` makes what follows it hidden: after the break, the colon and the quotes between the
    // two, the value is everything up to the next break, quote or end of an item (a slash or a colon is in it)
    let mut after_bearer = false;
    while at < chars.len() {
        let c = chars[at];
        if after_bearer {
            if let Some(n) = value_end(&chars, at) {
                out.extend(&chars[at..at + n]);
                at += n;
            } else if c == ':' {
                out.push(c);
                at += 1;
            } else {
                while at < chars.len() && value_end(&chars, at).is_none() {
                    at += if is_escaped_backslash(&chars, at) { 2 } else { 1 };
                }
                out.push_str(HIDDEN);
                after_bearer = false;
            }
        } else if is_escaped_backslash(&chars, at) {
            // the character after it is the next character of the text, not the end of a break (`\\n` is a backslash and an `n`)
            out.extend(&chars[at..at + 2]);
            at += 2;
        } else if let Some(n) = escape_len(&chars, at) {
            out.extend(&chars[at..at + n]);
            at += n;
        } else if word_char(c) {
            let end = (at..chars.len()).find(|&i| !word_char(chars[i])).unwrap_or(chars.len());
            let word: String = chars[at..end].iter().collect();
            out.push_str(if token_form(&word) { HIDDEN } else { &word });
            after_bearer = is_bearer(&word);
            at = end;
        } else {
            out.push(c);
            at += 1;
        }
    }
    out
}

/// Whether two backslashes stand at `at`: a JSON string writes one backslash so.
fn is_escaped_backslash(chars: &[char], at: usize) -> bool {
    chars.get(at) == Some(&'\\') && chars.get(at + 1) == Some(&'\\')
}

/// The length of a break a JSON string writes as an escape (`\n`, `\t`, `\r`, `\b`, `\f`, `\u0000`): it ends a
/// word and is no part of one.
fn escape_len(chars: &[char], at: usize) -> Option<usize> {
    match (chars.get(at), chars.get(at + 1)) {
        (Some('\\'), Some('n' | 't' | 'r' | 'b' | 'f')) => Some(2),
        (Some('\\'), Some('u')) if chars.get(at + 2..at + 6).is_some_and(|hex| hex.iter().all(char::is_ascii_hexdigit)) => Some(6),
        _ => None,
    }
}

/// The length of what ends the value after a bearer word, or stands between the word and its value: white
/// space, a quote (escaped or not), a break, the end of an item.
fn value_end(chars: &[char], at: usize) -> Option<usize> {
    match (chars.get(at), chars.get(at + 1)) {
        (Some('\\'), Some('"' | '\'')) => Some(2),
        (Some(c), _) if c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ']' | '}') => Some(1),
        _ => escape_len(chars, at),
    }
}

/// The leading key of a message of the settings readers: those can print a value, so only the key
/// they begin with is kept.
fn scrub(message: String) -> String {
    let key: String = message.split(|c: char| c == ':' || c.is_whitespace()).next().unwrap_or("").chars().take(120).collect();
    let key = key.split('.').map(segment).collect::<Vec<_>>().join(".");
    mask(&format!("{key}: refused (the reader's own words are not printed here, they can hold a value; the form: docs/loom-run-settings.md)"))
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

/// The table `[delegation]` of the file: its one key `classify_threshold`, a number from 0 to 1. A refusal names the key.
fn delegation_table(table: Option<Value>) -> Result<Option<f64>, String> {
    let Some(table) = table else { return Ok(None) };
    let table = table.as_object().ok_or_else(|| format!("{DELEGATION}: a table with the key classify_threshold"))?;
    if let Some(k) = table.keys().find(|k| k.as_str() != "classify_threshold") {
        let k = if runbook::routes::is_name(k) { k.as_str() } else { "<key>" };
        return Err(format!("{DELEGATION}.{k}: not a key of the table; it takes classify_threshold"));
    }
    match table.get("classify_threshold") {
        None => Ok(None),
        Some(v) => v.as_f64().filter(|t| (0.0..=1.0).contains(t)).map(Some).ok_or_else(|| format!("{DELEGATION}.classify_threshold: a number from 0 to 1")),
    }
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
    read_text(found).map_err(|e| mask(&e))
}

fn read_text(found: &Found) -> Result<File, String> {
    let name = found.path.display();
    let fail = |e: String| format!("{name}: {e}");
    let dir = found.path.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut top = match settings::parse(&found.text).map_err(|e| fail(mask(&e)))? {
        Value::Object(m) => m,
        _ => return Err(fail("a table".to_owned())),
    };
    scan(&Value::Object(top.clone()), "").map_err(fail)?;
    if found.nested && !found.via.chosen() {
        if let Some(key) = NESTED_REFUSED_KEYS.iter().find(|k| top.contains_key(**k)) {
            return Err(fail(format!("{key}: not a key of runbook.toml found by a program of a run (a file in the tree of a run has no key for a floor, a level, a cap, the spend ledger, the runbook directories, the gateway, the model routes or the repositories; name the file with --config to use it)")));
        }
    }
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
    if let Some(gateway) = top.get_mut(crate::loom_run::gateway::KEY).and_then(Value::as_object_mut) {
        if gateway.contains_key("token") {
            return Err(fail("gateway.token: not a key of runbook.toml (the file holds no token: name gateway.token_file, or put the token in a --settings file)".to_owned()));
        }
        match gateway.get_mut("token_file") {
            None => {}
            Some(Value::String(s)) if !s.is_empty() && !content_like(s) => {
                if !Path::new(s.as_str()).is_absolute() {
                    *s = dir.join(s.as_str()).display().to_string();
                }
            }
            Some(_) => return Err(fail("gateway.token_file: a path as text (a path, never content)".to_owned())),
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
    let deciders = match top.shift_remove("deciders") {
        None => Map::new(),
        Some(Value::Object(kinds)) if kinds.values().all(Value::is_object) => kinds,
        Some(_) => return Err(fail("deciders: a table of tables, one per decider kind ([deciders.rule])".to_owned())),
    };
    let mut gateways = top.shift_remove(runbook::routes::GATEWAYS);
    // a relative token file is relative to the file, as the one of `[gateway]`
    for (name, table) in gateways.as_mut().and_then(Value::as_object_mut).into_iter().flat_map(|g| g.iter_mut()) {
        match table.as_object_mut().and_then(|t| t.get_mut("token_file")) {
            Some(Value::String(s)) if !s.is_empty() && !content_like(s) => {
                if !Path::new(s.as_str()).is_absolute() {
                    *s = dir.join(s.as_str()).display().to_string();
                }
            }
            Some(Value::String(_)) => {
                let name = if runbook::routes::is_name(name) { name.as_str() } else { "<name>" };
                return Err(fail(format!("{}.{name}.token_file: a path as text (a path, never content)", runbook::routes::GATEWAYS)));
            }
            _ => {}
        }
    }
    let routes = top.shift_remove(runbook::routes::ROUTES);
    let route_policy = top.shift_remove(runbook::routes::POLICY);
    let classify_threshold = delegation_table(top.shift_remove(DELEGATION)).map_err(fail)?;
    let repositories = runbook::handback::Repositories::declare(top.shift_remove(runbook::handback::REPOSITORIES).as_ref(), &dir).map_err(|e| fail(mask(&e)))?;
    check_members(&top).map_err(fail)?;
    let tables = runbook::routes::Tables { gateway: top.get(crate::loom_run::gateway::KEY), gateways: gateways.as_ref(), routes: routes.as_ref(), route_policy: route_policy.as_ref() };
    let routes = runbook::routes::RouteTable::declare(&tables).map_err(|e| fail(mask(&e)))?;
    Ok(File { path: found.path.clone(), via: found.via, runs_dir, spend_ledger, confinement, cap_micro_usd, max_llm_calls, max_wall_seconds, runbooks, deciders, members: top, routes, classify_threshold, repositories })
}

/// The members through the readers of a run: the keys of every table, the kinds, the decider. The
/// login file is named by path and is not opened here.
fn check_members(members: &Map<String, Value>) -> Result<(), String> {
    // the shape refusals name the member and never a value
    crate::loom_run::gateway::check_shape(&Value::Object(members.clone())).map_err(|e| mask(&e))?;
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
    /// `None`: no ledger is opened (a cap that is stated is counted in the run itself).
    pub spend_ledger: Item<PathBuf>,
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
    let spend_ledger = pick(options.spend_ledger.clone(), "--spend-ledger", file.and_then(|f| f.spend_ledger.clone()), path);
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
pub(crate) fn segment(key: &str) -> &str {
    if mask(key) != key {
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
    out.push(item("spend_ledger", "--spend-ledger", "no ledger is opened without it", values.spend_ledger.as_ref().map(|(p, o)| (shown(&Value::from(p.display().to_string())), o))));
    out.push(match (&values.file, values.runbooks.is_empty()) {
        (Some(file), false) => format!("runbooks = {}  (file {})", shown(&Value::from(values.runbooks.iter().map(|d| d.display().to_string()).collect::<Vec<_>>())), file.display()),
        _ => "runbooks = []  (built-in: the built-in set only; `runbooks` in runbook.toml adds directories before it)".to_owned(),
    });
    out.push(item("confinement", "--confinement", "no built-in default, a run is refused without it", values.confinement.as_ref().map(|(c, o)| (shown(&Value::from(c.as_arg())), o))));
    out.push(item("max_wall_seconds", "--max-wall-seconds", "no built-in default, a run is refused without it", values.max_wall_seconds.as_ref().map(|(n, o)| (n.to_string(), o))));
    out.push(item("cap_micro_usd", "--cap-micro-usd", "no spend limit without it", values.cap_micro_usd.as_ref().map(|(n, o)| (n.to_string(), o))));
    out.push(item("max_llm_calls", "--max-llm-calls", "no limit on model calls without it", values.max_llm_calls.as_ref().map(|(n, o)| (n.to_string(), o))));
    // a gateway token of a settings file is named, never printed, whatever its form (`docs/loom-run-settings.md`)
    let mut merged = merged.clone();
    if let Some(token) = merged.get_mut(crate::loom_run::gateway::KEY).and_then(|g| g.get_mut("token")) {
        *token = Value::String("named".to_owned());
    }
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
    show_text(source, options).map_err(|e| mask(&e))
}

fn show_text(source: &Source, options: &ValueOptions) -> Result<String, String> {
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
    fn the_token_forms_after_a_name_and_an_equals_sign_or_a_dot() {
        let yes: Vec<String> = [["GH_TOKEN=gh", "p_x runner"], ["runner --token=gl", "pat-x"], ["x.gh", "p_x"], ["AUTH=Bear", "er abc"], ["Bear", "er=abc"], ["a=b c=Bear", "er d"], ["API_TOKEN=ey", "Ja.eyJb.c"]].iter().map(|p| p.concat()).collect();
        for yes in &yes {
            assert!(token_form(yes), "{yes}");
        }
        for no in ["max_wall_seconds=1", "a=b", "key=value.x", "name=bearer", "x.y.z", "a=", "=b", "bearer="] {
            assert!(!token_form(no), "{no}");
        }
    }

    #[test]
    fn a_web_token_is_hidden_after_any_run_of_dots_and_equals_signs_and_a_key_with_a_token_in_it_is_hidden_whole() {
        let yes: Vec<String> = [["name.ey", "Ja.eyJb.c"], ["a.b=c.ey", "Ja.eyJb.c"], ["a.b.c.ey", "Ja.eyJb.c"], ["x=y=ey", "Ja.eyJb.c"], ["n.gh", "p_x.tail"]].iter().map(|p| p.concat()).collect();
        for yes in &yes {
            assert!(token_form(yes), "{yes}");
            assert_eq!(mask(yes), HIDDEN, "{yes}");
        }
        for no in ["name.eyJa.b", "a.b.c.d", "x=y=z.w", "see.eyJ"] {
            assert!(!token_form(no), "{no}");
        }
        for key in [["see/gh", "p_x"], ["see:gh", "p_x"], ["see gh", "p_x"], ["see\\gh", "p_x"], ["see,Bear", "er x"]].iter().map(|p| p.concat()) {
            assert_eq!(segment(&key), HIDDEN, "{key}");
        }
        for key in ["see/words", "a b", "models", "claude-sonnet-5-5"] {
            assert_eq!(segment(key), key);
        }
    }

    #[test]
    fn mask_reads_an_escaped_backslash_as_one_character_and_not_as_the_start_of_a_break() {
        // the text a JSON string has for the value `C:\dir\<token>`: the backslash is written twice, and the `n` after the second is no `\n`
        let token = ["n", "pm_x"].concat();
        let shown = format!("\"C:\\\\dir\\\\{token}\"");
        assert_eq!(mask(&shown), "\"C:\\\\dir\\\\<a key or token>\"");
        // a break after an escaped backslash is still a break, and the value after a bearer word spans the pair
        assert_eq!(mask("\"a\\\\\\nBearer x\""), "\"a\\\\\\nBearer <a key or token>\"");
        assert_eq!(mask("\"Bearer abc\\\\ndef ghi\""), "\"Bearer <a key or token> ghi\"");
    }

    #[test]
    fn mask_hides_the_word_after_bearer_in_a_header_after_a_break_and_nothing_else() {
        assert_eq!(mask("Authorization: Bearer hunter2"), "Authorization: Bearer <a key or token>");
        assert_eq!(mask("Bearer\n  hunter2"), "Bearer\n  <a key or token>");
        assert_eq!(mask("Bearer: hunter2 and tea"), "Bearer: <a key or token> and tea");
        assert_eq!(mask("a Bearer b c"), "a Bearer <a key or token> c");
        assert_eq!(mask("a bear hunter2"), "a bear hunter2");
        assert_eq!(mask("Bearer"), "Bearer");
        assert_eq!(mask(""), "");
        // the line a JSON string has: the break is the two characters backslash and n
        assert_eq!(mask("\"Authorization: Bearer\\n  hunter2\""), "\"Authorization: Bearer\\n  <a key or token>\"");
        assert_eq!(mask("\"x\\nBearer\\thunter2\""), "\"x\\nBearer\\t<a key or token>\"");
    }

    #[test]
    fn mask_ends_a_word_at_every_break_a_json_string_writes_and_a_bearer_value_at_every_end_of_an_item() {
        let token = ["gh", "p_x"].concat();
        for brk in ["\\n", "\\t", "\\r", "\\b", "\\f", "\\u000a", "\\u0009"] {
            assert_eq!(mask(&format!("\"a{brk}{token}\"")), format!("\"a{brk}{HIDDEN}\""), "{brk}");
        }
        // `\u` that is no escape of a character is no break
        let plain = format!("a\\u00zz{token}");
        assert_eq!(mask(&plain), plain, "a bad escape is no break: the word after the backslash is one plain word");
        for (text, want) in [
            ("Bearer abc,tea", "Bearer <a key or token>,tea"),
            ("[Bearer abc]", "[Bearer <a key or token>]"),
            ("{x: Bearer abc}", "{x: Bearer <a key or token>}"),
            ("Bearer 'abc' x", "Bearer '<a key or token>' x"),
            ("Bearer \\\"abc\\\" x", "Bearer \\\"<a key or token>\\\" x"),
            ("Bearer abc\\\\def x", "Bearer <a key or token> x"),
        ] {
            assert_eq!(mask(text), want, "{text}");
        }
    }

    #[test]
    fn every_refusal_of_the_file_is_masked_where_it_is_made_and_the_printed_text_is_masked_again() {
        let token = ["gh", "p_x"].concat();
        let path = PathBuf::from(format!("/nonexistent/see/{token}/runbook.toml"));
        let source = Source { config: Some(path.clone()), no_config: false };
        for e in [load(&source).unwrap_err(), show(&source, &ValueOptions::default()).unwrap_err()] {
            assert!(e.contains(HIDDEN) && !e.contains(&token), "{e}");
        }
        let found = Found { path, via: Via::Option, text: "unknown_key = 1\n".to_owned(), nested: false };
        let e = parse(&found).unwrap_err();
        assert!(e.contains(HIDDEN) && !e.contains(&token) && e.contains("unknown_key"), "{e}");
        assert_eq!(printable(&format!("x/{token}")), format!("x/{HIDDEN}"));
    }

    #[test]
    fn the_scrubbed_message_and_the_refusal_of_a_key_are_masked_by_themselves_and_not_only_by_the_caller() {
        // a form that no one segment of a dotted key has: only the whole text shows it
        let jwt = ["ey", "Ja.eyJb.c"].concat();
        let said = scrub(format!("{jwt}: a value"));
        assert!(said.contains(HIDDEN) && !said.contains("eyJa"), "{said}");
        // a key with a token after a space: the word-aware mask hides it, the form at the start of the key does not
        let key = ["see gh", "p_x"].concat();
        assert!(!token_form(&key));
        let object: Value = serde_json::from_str(&format!("{{\"{key}\": 1}}")).unwrap();
        let refused = scan(&object, "").unwrap_err();
        assert!(refused.contains("a key has the form of a key or a token") && !refused.contains("p_x"), "{refused}");
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
    fn a_gateway_in_runbook_toml_takes_an_address_and_a_token_file_never_a_token_and_a_file_in_a_run_names_none() {
        let token = "tok-config-5c1e93a0";
        let found = |text: &str, nested: bool, via: Via| Found { path: PathBuf::from("/x/runbook.toml"), via, text: text.to_owned(), nested };
        let f = parse(&found("[gateway]\nbase_url = \"http://127.0.0.1:8080\"\ntoken_file = \"gw-token\"\n", false, Via::Walk)).unwrap();
        assert_eq!(f.members["gateway"], serde_json::json!({"base_url": "http://127.0.0.1:8080", "token_file": "/x/gw-token"}), "a relative token file is relative to the file");
        let inline = parse(&found(&format!("[gateway]\nbase_url = \"http://127.0.0.1:8080\"\ntoken = \"{token}\"\n"), false, Via::Option)).unwrap_err();
        assert!(inline.contains("gateway.token") && !inline.contains(token), "{inline}");
        let both = parse(&found("login_file = \"/l\"\n[gateway]\nbase_url = \"http://127.0.0.1:8080\"\ntoken_file = \"/t\"\n", false, Via::Option)).unwrap_err();
        assert!(both.contains("gateway") && both.contains("login_file") && !both.contains("/t"), "{both}");
        let shape = parse(&found("[gateway]\nbase_url = \"ftp://h\"\ntoken_file = \"/secret-place\"\n", false, Via::Option)).unwrap_err();
        assert!(shape.contains("gateway.base_url") && !shape.contains("ftp") && !shape.contains("secret-place"), "{shape}");
        // a file a program of a run finds by the walk has no gateway; a file it names does
        let text = "[gateway]\nbase_url = \"http://127.0.0.1:8080\"\ntoken_file = \"/t\"\n";
        let nested = parse(&found(text, true, Via::Walk)).unwrap_err();
        assert!(nested.contains("gateway: not a key of runbook.toml found by a program of a run"), "{nested}");
        assert!(parse(&found(text, true, Via::Option)).is_ok());
    }

    #[test]
    fn the_model_routes_of_runbook_toml_take_a_token_file_relative_to_the_file_and_a_file_in_a_run_names_none() {
        let found = |text: &str, nested: bool, via: Via| Found { path: PathBuf::from("/x/runbook.toml"), via, text: text.to_owned(), nested };
        let text = "[gateways.gw]\nbase_url = \"http://127.0.0.1:8080\"\ntoken_file = \"gw-token\"\n[routes.qwen]\ngateway = \"gw\"\nmodel = \"qwen\"\nkind = \"managed\"\n[route_policy]\ndefault = \"qwen\"\n";
        let f = parse(&found(text, false, Via::Walk)).unwrap();
        assert_eq!((f.routes.names(), f.routes.default_route()), (vec!["qwen"], Some("qwen")));
        assert!(!f.states_nothing() && f.members.is_empty(), "the tables are the file's own keys, not run settings");
        assert!(format!("{:?}", f.routes.gateway("gw").unwrap()).contains("<named>"));
        // the relative token file is made absolute against the file: the read names its member, never the path
        let e = f.routes.clone().read_tokens().unwrap_err();
        assert!(e.contains("gateways.gw.token_file") && !e.contains("/x") && !e.contains("gw-token"), "{e}");
        for key in ["gateways", "routes", "route_policy"] {
            let only = match key {
                "gateways" => "[gateways.gw]\nbase_url = \"http://127.0.0.1:8080\"\n".to_owned(),
                "routes" => "[routes.qwen]\ngateway = \"default\"\nmodel = \"qwen\"\nkind = \"hosted\"\n".to_owned(),
                _ => "[route_policy]\n".to_owned(),
            };
            let nested = parse(&found(&only, true, Via::Walk)).unwrap_err();
            assert!(nested.contains(&format!("{key}: not a key of runbook.toml found by a program of a run")), "{nested}");
        }
        let lone = parse(&found("[gateway]\nbase_url = \"http://127.0.0.1:8080\"\n[routes.r]\ngateway = \"default\"\nmodel = \"m\"\nkind = \"local\"\n", false, Via::Option)).unwrap();
        assert_eq!(lone.routes.resolve("r").map(|r| r.base_url), Some("http://127.0.0.1:8080"), "a lone [gateway] is the gateway default");
        assert_eq!(lone.members["gateway"]["base_url"], "http://127.0.0.1:8080", "and stays the run setting it was");
    }

    /// `[delegation] classify_threshold` is a number from 0 to 1 (`docs/model-routes.md`, section 9.1); a value out of
    /// range, of another type, or another key of the table is refused by name, never by its value.
    #[test]
    fn the_classify_threshold_of_runbook_toml_is_a_number_from_0_to_1_and_a_refusal_names_the_key() {
        let found = |text: &str| Found { path: PathBuf::from("/x/runbook.toml"), via: Via::Option, text: text.to_owned(), nested: false };
        let f = parse(&found("[delegation]\nclassify_threshold = 0.95\n")).unwrap();
        assert_eq!(f.classify_threshold, Some(0.95));
        assert!(!f.states_nothing() && f.members.is_empty(), "the table is the file's own key, not a run setting");
        assert_eq!(parse(&found("[delegation]\nclassify_threshold = 0\n")).unwrap().classify_threshold, Some(0.0));
        assert_eq!(parse(&found("runs_dir = \"r\"\n")).unwrap().classify_threshold, None);
        for (text, said) in [
            ("[delegation]\nclassify_threshold = 1.5\n", "delegation.classify_threshold: a number from 0 to 1"),
            ("[delegation]\nclassify_threshold = -0.1\n", "delegation.classify_threshold: a number from 0 to 1"),
            ("[delegation]\nclassify_threshold = \"high\"\n", "delegation.classify_threshold: a number from 0 to 1"),
            ("[delegation]\nthreshold = 0.5\n", "delegation.threshold: not a key of the table; it takes classify_threshold"),
            ("delegation = 0.5\n", "delegation: a table with the key classify_threshold"),
        ] {
            let e = parse(&found(text)).unwrap_err();
            assert!(e.contains(said) && !e.contains("1.5") && !e.contains("high"), "{text}: {e}");
        }
    }

    #[test]
    fn the_starter_is_read() {
        let found = Found { path: PathBuf::from("/x/runbook.toml"), via: Via::Option, text: STARTER.to_owned(), nested: false };
        let f = parse(&found).unwrap();
        assert_eq!((f.confinement, f.max_wall_seconds, f.cap_micro_usd, f.max_llm_calls), (Some(Confinement::Substrate), Some(3600), None, None));
        assert_eq!(f.spend_ledger, None, "the starter names no ledger");
        assert_eq!(f.runs_dir, Some(PathBuf::from("/x/runs")));
    }
}
