//! A delegation that changes a repository on its own branch, first slice (story
//! `a-delegation-changes-a-repository-on-its-own-branch`, its `## Design` items 1, 3, 4 and 8 and its `## Decision`;
//! `docs/daemon-api.md`, "A delegation that changes a repository").
//!
//! The operator lists the repositories a delegation may change in `runbook.toml` (`[repositories.<name>]`, none by
//! default). A delegation names one by its name, never by a path. When its run starts, the daemon makes the run's
//! worktree T with the host's `worktree` command at the entry's base, as it was resolved at the submit; the agent node
//! of the protocol `delegation-change` works in T. When the run's outcome is `delivered`, the daemon commits T's change
//! under the entry's identity and pushes it as the new ref `refs/heads/runbook/<run-id>`: no force, never over a branch
//! of that name, never a path of the entry's `deny_paths` or a link out of T, never when T's `.git` file differs from
//! the one it was made with, never after a cancel. Then it finishes T. A run that ends any other way pushes nothing and
//! keeps T.
//!
//! Every git call of the daemon runs with `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_CONFIG_NOSYSTEM=1`, `/dev/null` as git's
//! script directory (no script of the repository runs), no credential helper and no prompt, and is killed after
//! [`CALL_SECONDS`]: the operator's identity, scripts and credentials are never used. The push runs from a bare
//! repository of the daemon's own, so the checkout's configuration does not steer it either. The push credential is the
//! entry's `push_token_file`, read by the `runbook` binary as git's askpass ([`ASKPASS_VARIABLE`], [`askpass`]) for the
//! remote's host only: the token is in no argument, no variable, no log and no file of the run.

use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

/// The table of `runbook.toml` that lists the repositories a delegation may change.
pub const REPOSITORIES: &str = "repositories";
/// The variable the daemon gives every bound program of a run that changes a repository: the path of the run's
/// worktree, where its agent node works.
pub const WORKTREE_VARIABLE: &str = "RUNBOOK_WORKTREE";
/// The variable that makes the `runbook` binary git's askpass: the path of the entry's token file.
pub const ASKPASS_VARIABLE: &str = "RUNBOOK_ASKPASS_FILE";
/// The variable that names the one origin (`https://<host>`) the askpass answers the token for.
pub const ASKPASS_ORIGIN_VARIABLE: &str = "RUNBOOK_ASKPASS_ORIGIN";
/// The run event the daemon writes right before its push, while the run is still live: the repository, the branch and
/// the commit. A cancel accepted after it does not stop the push.
pub const PUSHING_EVENT: &str = "runbook.pushing";
/// The most seconds one git or `worktree` call of the daemon takes before it is killed.
pub const CALL_SECONDS: u64 = 120;
/// The user name the askpass answers: a token is the password of any user on the forges this slice pushes to.
pub const ASKPASS_USER: &str = "x-access-token";
/// The run event of a hand-back that pushed: the repository, the branch, the commit and the base.
pub const HANDED_BACK_EVENT: &str = "runbook.handed-back";
/// The run event of a hand-back the daemon refused or that failed: the repository and the reason.
pub const REFUSED_EVENT: &str = "runbook.hand-back-refused";
/// The file of the run's directory that records the run's worktree: never shown in a view.
pub const RECORD_FILE: &str = "worktree.json";
/// The paths a change may not touch when the entry names none: a branch of the same repository runs its CI with the
/// repository's secrets.
pub const DEFAULT_DENY: [&str; 3] = [".github/", ".gitlab-ci.yml", ".gitmodules"];
/// The keys of an entry.
const KEYS: [&str; 8] = ["path", "base", "remote", "default_branch", "commit_name", "commit_email", "push_token_file", "deny_paths"];
/// The most bytes of a reason the daemon records from git's own words.
const REASON_LIMIT: usize = 300;

/// One entry `[repositories.<name>]` of `runbook.toml`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repository {
    pub name: String,
    /// The operator's checkout the worktree is made from (absolute).
    pub path: PathBuf,
    /// The revision of `path` a run's branch starts from; resolved to a commit at the submit.
    pub base: String,
    /// Where the branch is pushed: an `https://` or `file://` URL with no user or password in it.
    pub remote: String,
    /// The branch a later merge request would target; the push never names it.
    pub default_branch: String,
    pub commit_name: String,
    pub commit_email: String,
    /// The file of the push credential, read by the askpass; none: the remote needs none (a `file://` remote).
    pub push_token_file: Option<PathBuf>,
    /// A path ending in `/` denies everything under it, any other path itself and everything under it.
    pub deny_paths: Vec<String>,
}

/// The entries of `[repositories.*]`, by name: the allow-list of the repositories a delegation may change.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Repositories {
    entries: BTreeMap<String, Repository>,
}

fn text_of(table: &Map<String, Value>, at: &str, key: &str, required: bool) -> Result<Option<String>, String> {
    match table.get(key) {
        None if required => Err(format!("{at}.{key}: missing")),
        None => Ok(None),
        Some(Value::String(s)) if !s.trim().is_empty() && !s.contains(['\n', '\r', '\0']) => Ok(Some(s.clone())),
        Some(_) => Err(format!("{at}.{key}: one line of text")),
    }
}

/// A revision or branch name the daemon passes to git: no option, no space, no `..`.
fn plain_revision(text: &str) -> bool {
    !text.starts_with('-') && !text.contains("..") && text.bytes().all(|b| b.is_ascii_graphic() && !matches!(b, b'~' | b'^' | b':' | b'?' | b'*' | b'[' | b'\\'))
}

/// A remote the daemon pushes to: `https://host/...` or `file:///path`, with no `@` anywhere (a user or a password
/// hides behind one, also after a third slash, which git's transport reads as the host's).
fn plain_remote(text: &str) -> bool {
    if text.contains('@') || !text.bytes().all(|b| b.is_ascii_graphic()) {
        return false;
    }
    if let Some(rest) = text.strip_prefix("https://") {
        return !rest.split('/').next().unwrap_or("").is_empty();
    }
    text.strip_prefix("file://").is_some_and(|rest| rest.starts_with('/'))
}

/// The scheme and host of an `https://` remote: the one origin the askpass answers the token for.
fn origin(remote: &str) -> Option<String> {
    let rest = remote.strip_prefix("https://")?;
    Some(format!("https://{}", rest.split('/').next().unwrap_or("")))
}

/// A path of `deny_paths` as git names a path: relative, segments joined by one `/`, no `.` or `..`, at most one
/// trailing `/`.
fn plain_path(text: &str) -> bool {
    let t = text.strip_suffix('/').unwrap_or(text);
    !t.is_empty() && t.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
}

/// Whether a link at `path` (relative to T) whose target is `target` leads out of T: an absolute target, or one whose
/// `..` climbs above T's root.
fn leaves_tree(path: &str, target: &Path) -> bool {
    let mut depth = path.split('/').count().saturating_sub(1);
    for c in target.components() {
        match c {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir if depth > 0 => depth -= 1,
            _ => return true,
        }
    }
    false
}

impl Repositories {
    /// The table `[repositories]` of a file whose directory is `dir`: every key checked, nothing read. A relative `path`
    /// or `push_token_file` is relative to the file. A refusal names the entry and the key, never a value.
    pub fn declare(table: Option<&Value>, dir: &Path) -> Result<Repositories, String> {
        let Some(table) = table else { return Ok(Repositories::default()) };
        let table = table.as_object().ok_or_else(|| format!("{REPOSITORIES}: a table of tables, one per repository ([{REPOSITORIES}.<name>])"))?;
        let mut entries = BTreeMap::new();
        for (name, entry) in table {
            if !crate::routes::is_name(name) {
                return Err(format!("{REPOSITORIES}.<name>: a name of letters, digits, '.', '_' and '-', at most 64 bytes"));
            }
            let at = format!("{REPOSITORIES}.{name}");
            let entry = entry.as_object().ok_or_else(|| format!("{at}: a table"))?;
            if let Some(k) = entry.keys().find(|k| !KEYS.contains(&k.as_str())) {
                let k = if crate::routes::is_name(k) { k.as_str() } else { "<key>" };
                return Err(format!("{at}.{k}: not a key of a repository (known: {})", KEYS.join(", ")));
            }
            let absolute = |p: String| if Path::new(&p).is_absolute() { PathBuf::from(p) } else { dir.join(p) };
            let path = absolute(text_of(entry, &at, "path", true)?.unwrap_or_default());
            let base = text_of(entry, &at, "base", true)?.unwrap_or_default();
            if !plain_revision(&base) {
                return Err(format!("{at}.base: a branch, tag or commit of the repository"));
            }
            let remote = text_of(entry, &at, "remote", true)?.unwrap_or_default();
            if !plain_remote(&remote) {
                return Err(format!("{at}.remote: an https://<host>/ or file:/// URL with no user, password or '@' in it (the credential is push_token_file)"));
            }
            let default_branch = text_of(entry, &at, "default_branch", true)?.unwrap_or_default();
            if !plain_revision(&default_branch) || default_branch.starts_with("runbook/") {
                return Err(format!("{at}.default_branch: a branch name outside runbook/"));
            }
            let commit_name = text_of(entry, &at, "commit_name", true)?.unwrap_or_default();
            let commit_email = text_of(entry, &at, "commit_email", true)?.unwrap_or_default();
            if commit_name.contains(['<', '>']) || commit_email.contains(['<', '>']) || !commit_email.contains('@') {
                return Err(format!("{at}.commit_name, {at}.commit_email: a name, and an address with @, neither with < or >"));
            }
            let push_token_file = text_of(entry, &at, "push_token_file", false)?.map(absolute);
            let deny_paths = match entry.get("deny_paths") {
                None => DEFAULT_DENY.iter().map(|p| (*p).to_owned()).collect(),
                Some(Value::Array(items)) => items
                    .iter()
                    .map(|i| i.as_str().filter(|s| plain_path(s)).map(str::to_owned).ok_or_else(|| format!("{at}.deny_paths: a list of paths inside the repository")))
                    .collect::<Result<Vec<_>, _>>()?,
                Some(_) => return Err(format!("{at}.deny_paths: a list of paths inside the repository")),
            };
            entries.insert(name.clone(), Repository { name: name.clone(), path, base, remote, default_branch, commit_name, commit_email, push_token_file, deny_paths });
        }
        Ok(Repositories { entries })
    }

    pub fn get(&self, name: &str) -> Option<&Repository> {
        self.entries.get(name)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn names(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }

    /// Each entry's token file read as a route's is (`crate::routes::read_token_file`), when the daemon starts: a file
    /// that is no one-line token refuses the start, naming the key. The token is not kept: the askpass reads the file.
    pub fn check_token_files(&self) -> Result<(), String> {
        for e in self.entries.values() {
            if let Some(file) = &e.push_token_file {
                crate::routes::read_token_file(&file.display().to_string(), &format!("{REPOSITORIES}.{}.push_token_file", e.name))?;
            }
        }
        Ok(())
    }
}

/// Why a delegation's `repository` is refused at the submit, or none: it names an entry of the file.
pub fn refusal(repositories: &Repositories, named: &str) -> Option<String> {
    if !crate::routes::is_name(named) {
        return Some("repository: a delegation names a repository by the name of its entry in runbook.toml, never a path".to_owned());
    }
    match repositories.get(named) {
        Some(_) => None,
        None if repositories.is_empty() => Some(format!("repository: runbook.toml lists no repository ([{REPOSITORIES}.<name>]); a delegation changes none")),
        None => Some(format!("repository: {named} is not an entry of runbook.toml; it lists {}", repositories.names().join(", "))),
    }
}

/// The text of the last bytes of `bytes` that git wrote, one line, at most [`REASON_LIMIT`] bytes.
fn said(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let line = text.lines().map(str::trim).filter(|l| !l.is_empty()).filter(|l| !l.starts_with("hint:")).collect::<Vec<_>>().join("; ");
    line.chars().take(REASON_LIMIT).collect()
}

/// A git call of the daemon: no configuration but the command's own, no script of the repository (its directory is
/// `/dev/null`), no credential helper, no prompt and no signing.
fn git() -> Command {
    let mut c = Command::new("git");
    c.env_clear();
    if let Some(path) = std::env::var_os("PATH") {
        c.env("PATH", path);
    }
    c.env("GIT_CONFIG_GLOBAL", "/dev/null").env("GIT_CONFIG_NOSYSTEM", "1").env("GIT_TERMINAL_PROMPT", "0").env("LC_ALL", "C");
    c.args(["-c", "core.hooksPath=/dev/null"]);
    c.args(["-c", "credential.helper=", "-c", "commit.gpgsign=false", "-c", "core.fsmonitor=false", "-c", "protocol.file.allow=always"]);
    c.stdin(Stdio::null());
    c
}

/// Runs `c` in a process group of its own and kills the group after [`CALL_SECONDS`]: its stdout, or why it failed.
fn bounded(mut c: Command) -> Result<Vec<u8>, String> {
    use std::io::Read;
    use std::os::unix::process::CommandExt;
    use std::time::{Duration, Instant};
    c.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).process_group(0);
    let mut child = c.spawn().map_err(|e| format!("the program could not be started ({})", e.kind()))?;
    let (mut out, mut err) = (child.stdout.take(), child.stderr.take());
    let out = std::thread::spawn(move || {
        let mut b = Vec::new();
        if let Some(o) = out.as_mut() {
            let _ = o.read_to_end(&mut b);
        }
        b
    });
    let err = std::thread::spawn(move || {
        let mut b = Vec::new();
        if let Some(e) = err.as_mut() {
            let _ = e.read_to_end(&mut b);
        }
        b
    });
    let until = Instant::now() + Duration::from_secs(CALL_SECONDS);
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                // SAFETY: a plain kill(2) of the child's own process group; no memory is shared.
                unsafe { libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL) };
                let _ = child.wait();
                break None;
            }
        }
    };
    let (out, err) = (out.join().unwrap_or_default(), err.join().unwrap_or_default());
    match status {
        Some(s) if s.success() => Ok(out),
        Some(_) => Err(said(&err)),
        None => Err(format!("killed after {CALL_SECONDS} s")),
    }
}

fn output(c: Command, what: &str) -> Result<Vec<u8>, String> {
    bounded(c).map_err(|why| format!("{what}: {why}"))
}

/// The commit `entry.base` names in the entry's checkout, read at the submit.
pub fn resolve_base(entry: &Repository) -> Result<String, String> {
    let mut c = git();
    c.arg("-C").arg(&entry.path).args(["rev-parse", "--verify", "--quiet", "--end-of-options", &format!("{}^{{commit}}", entry.base)]);
    let out = output(c, "base").map_err(|_| format!("repository: the base of {} does not name a commit of its checkout", entry.name))?;
    let commit = String::from_utf8_lossy(&out).trim().to_owned();
    if commit.len() >= 40 && commit.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(commit)
    } else {
        Err(format!("repository: the base of {} does not name a commit of its checkout", entry.name))
    }
}

/// The run's worktree as the daemon made it: recorded in the run's directory ([`RECORD_FILE`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tree {
    pub repository: String,
    /// T: the worktree's root, as the `worktree` command made it.
    pub path: PathBuf,
    /// The bytes of T's `.git` file when it was made.
    pub git_file: String,
    /// The worktree's own git directory, read from that file: every git call of the hand-back names it.
    pub git_dir: PathBuf,
    /// The commit the branch starts from.
    pub base: String,
}

impl Tree {
    fn record(&self) -> Value {
        json!({"repository": self.repository, "path": self.path, "git_file": self.git_file, "git_dir": self.git_dir, "base": self.base})
    }

    /// The tree recorded in the run's directory `run_dir`, if the run has one.
    pub fn read(run_dir: &Path) -> Option<Tree> {
        let v: Value = serde_json::from_slice(&std::fs::read(run_dir.join(RECORD_FILE)).ok()?).ok()?;
        let s = |k: &str| v[k].as_str().map(str::to_owned);
        Some(Tree { repository: s("repository")?, path: PathBuf::from(s("path")?), git_file: s("git_file")?, git_dir: PathBuf::from(s("git_dir")?), base: s("base")? })
    }
}

/// The id the `worktree` command knows the run's tree by.
pub fn tree_id(run_id: &str) -> String {
    format!("runbook-{run_id}")
}

/// The branch of the run.
pub fn branch(run_id: &str) -> String {
    format!("runbook/{run_id}")
}

fn worktree(args: &[&str]) -> Result<Vec<u8>, String> {
    let mut c = Command::new("worktree");
    c.args(args);
    bounded(c).map_err(|why| format!("worktree {}: {why}", args[0]))
}

/// Makes the run's worktree of `entry` at `base` with the host's `worktree` command, takes its lease and records it in
/// the run's directory `run_dir`. A run that has its record already (it was taken up again) keeps that tree.
pub fn make(entry: &Repository, run_id: &str, base: &str, run_dir: &Path) -> Result<Tree, String> {
    if let Some(tree) = Tree::read(run_dir) {
        return Ok(tree);
    }
    let id = tree_id(run_id);
    let repo = entry.path.display().to_string();
    let purpose = format!("runbook run {run_id}");
    let out = worktree(&["create", "--repo", &repo, "--base", base, "--id", &id, "--purpose", &purpose])?;
    let printed = String::from_utf8_lossy(&out);
    let path = PathBuf::from(printed.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or(""));
    if !path.is_absolute() || !path.is_dir() {
        return Err("worktree create: it printed no directory".to_owned());
    }
    let git_file = std::fs::read_to_string(path.join(".git")).map_err(|_| "worktree create: the tree has no .git file".to_owned())?;
    let git_dir = git_file.strip_prefix("gitdir:").map(str::trim).filter(|d| !d.is_empty()).ok_or("worktree create: the tree's .git file names no git directory")?;
    let git_dir = if Path::new(git_dir).is_absolute() { PathBuf::from(git_dir) } else { path.join(git_dir) };
    let tree = Tree { repository: entry.name.clone(), path, git_file, git_dir, base: base.to_owned() };
    let _ = worktree(&["hook", "session-start", "--session", &id, "--path", &tree.path.display().to_string()]);
    let text = serde_json::to_vec_pretty(&tree.record()).map_err(|e| e.to_string())?;
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(run_dir.join(RECORD_FILE)).map_err(|e| format!("the run's record of its worktree: {}", e.kind()))?;
        f.write_all(&text).map_err(|e| format!("the run's record of its worktree: {}", e.kind()))?;
    }
    Ok(tree)
}

/// The lease of the run on its tree ends: the run is over, whatever its end.
pub fn release(run_id: &str, tree: &Tree) {
    let _ = worktree(&["hook", "session-end", "--session", &tree_id(run_id), "--path", &tree.path.display().to_string()]);
}

/// What a hand-back that pushed made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handed {
    pub branch: String,
    pub commit: String,
}

/// Whether `path`, a path of the change, is one `deny` names.
fn denied<'a>(deny: &'a [String], path: &str) -> Option<&'a str> {
    deny.iter().map(String::as_str).find(|d| match d.strip_suffix('/') {
        Some(dir) => path.starts_with(&format!("{dir}/")),
        None => path == *d || path.starts_with(&format!("{d}/")),
    })
}

/// The hand-back of a run whose outcome is `delivered`: T's change committed under the entry's identity on the base and
/// pushed as `refs/heads/runbook/<run-id>`, a new ref only (a branch of that name on the remote refuses the push).
/// `scratch` is the run's directory, where the daemon's push repository is made and removed again. `before_push` is
/// asked right before the push and refuses it when the run is no longer live. `Err` is why nothing was pushed: T's
/// `.git` file changed, no change, a denied path, a link out of T, the branch exists, a cancel, or git's own words.
pub fn hand_back(entry: &Repository, run_id: &str, tree: &Tree, scratch: &Path, before_push: &dyn Fn(&Handed) -> bool) -> Result<Handed, String> {
    let git_file = std::fs::symlink_metadata(tree.path.join(".git")).ok().filter(std::fs::Metadata::is_file).and_then(|_| std::fs::read_to_string(tree.path.join(".git")).ok());
    if git_file.as_deref() != Some(tree.git_file.as_str()) {
        return Err("the worktree's .git file was changed by the run: the hand-back takes no tree whose .git the run changed".to_owned());
    }
    let in_tree = || {
        let mut c = git();
        c.arg("--git-dir").arg(&tree.git_dir).arg("--work-tree").arg(&tree.path);
        c
    };
    let mut add = in_tree();
    add.args(["add", "--all", "--", "."]);
    output(add, "add")?;
    let mut diff = in_tree();
    diff.args(["diff", "--cached", "--name-only", "--no-renames", "-z"]);
    let names = output(diff, "diff")?;
    let paths: Vec<String> = names.split(|b| *b == 0).filter(|p| !p.is_empty()).map(|p| String::from_utf8_lossy(p).into_owned()).collect();
    if paths.is_empty() {
        return Err("no change: the run changed no file of the worktree".to_owned());
    }
    let cut = |text: String| -> String { text.chars().take(REASON_LIMIT).collect() };
    let refused: Vec<String> = paths.iter().filter_map(|p| denied(&entry.deny_paths, p).map(|d| format!("{p} (denied by {d})"))).collect();
    if !refused.is_empty() {
        return Err(cut(format!("deny_paths: the change touches {}", refused.join(", "))));
    }
    let links: Vec<&str> = paths
        .iter()
        .filter(|p| {
            let at = tree.path.join(p);
            std::fs::symlink_metadata(&at).is_ok_and(|m| m.file_type().is_symlink()) && std::fs::read_link(&at).map_or(true, |target| leaves_tree(p, &target))
        })
        .map(String::as_str)
        .collect();
    if !links.is_empty() {
        return Err(cut(format!("links: the change adds a link that leads out of the worktree: {}", links.join(", "))));
    }
    let mut commit = in_tree();
    commit.arg("-c").arg(format!("user.name={}", entry.commit_name)).arg("-c").arg(format!("user.email={}", entry.commit_email));
    commit.env("GIT_AUTHOR_NAME", &entry.commit_name).env("GIT_AUTHOR_EMAIL", &entry.commit_email).env("GIT_COMMITTER_NAME", &entry.commit_name).env("GIT_COMMITTER_EMAIL", &entry.commit_email);
    commit.args(["commit", "--quiet", "--no-verify", "-m", &format!("runbook run {run_id}"), "-m", "The change of a delegation, committed by the daemon after its run delivered."]);
    output(commit, "commit")?;
    let mut head = in_tree();
    head.args(["rev-parse", "--verify", "HEAD"]);
    let sha = String::from_utf8_lossy(&output(head, "commit")?).trim().to_owned();
    let mut parent = in_tree();
    parent.args(["rev-parse", "--verify", "HEAD^"]);
    if String::from_utf8_lossy(&output(parent, "commit")?).trim() != tree.base {
        return Err("commit: the commit's parent is not the run's base".to_owned());
    }
    let handed = Handed { branch: branch(run_id), commit: sha };
    // The push runs from a repository of the daemon's own beside the run's record: no configuration but the daemon's
    // (the checkout's own, with its URL rewrites, push URLs and http settings, is never read), the checkout's objects
    // through an alternate.
    let pusher = scratch.join("handback-push.git");
    let _ = std::fs::remove_dir_all(&pusher);
    let pushed = push_from(entry, tree, &pusher, &handed, before_push);
    let _ = std::fs::remove_dir_all(&pusher);
    // git names the remote in its words: the reason says <remote>, never a URL or a path of the host
    pushed.map_err(|why| why.replace(&entry.remote, "<remote>").replace(entry.remote.trim_start_matches("file://"), "<remote>"))?;
    Ok(handed)
}

fn push_from(entry: &Repository, tree: &Tree, pusher: &Path, handed: &Handed, before_push: &dyn Fn(&Handed) -> bool) -> Result<(), String> {
    let mut init = git();
    init.args(["init", "--quiet", "--bare", "--template="]).arg(pusher);
    output(init, "push")?;
    let common = match std::fs::read_to_string(tree.git_dir.join("commondir")) {
        Ok(dir) if Path::new(dir.trim()).is_absolute() => PathBuf::from(dir.trim()),
        Ok(dir) => tree.git_dir.join(dir.trim()),
        Err(_) => tree.git_dir.clone(),
    };
    let objects = std::fs::canonicalize(common.join("objects")).map_err(|e| format!("push: the checkout's objects ({})", e.kind()))?;
    std::fs::write(pusher.join("objects/info/alternates"), format!("{}\n", objects.display())).map_err(|e| format!("push: the alternate ({})", e.kind()))?;
    let exe = match entry.push_token_file {
        Some(_) => Some(std::env::current_exe().map_err(|e| format!("push: the daemon's own program for the askpass ({})", e.kind()))?),
        None => None,
    };
    let remote_git = || {
        let mut c = git();
        c.args(["-c", "http.followRedirects=false"]).arg("--git-dir").arg(pusher);
        if let (Some(file), Some(exe)) = (&entry.push_token_file, &exe) {
            c.env("GIT_ASKPASS", exe).env(ASKPASS_VARIABLE, file).env(ASKPASS_ORIGIN_VARIABLE, origin(&entry.remote).unwrap_or_default());
        }
        c
    };
    let target = format!("refs/heads/{}", handed.branch);
    let mut probe = remote_git();
    probe.args(["ls-remote", "--refs", &entry.remote, &target]);
    if !output(probe, "push")?.is_empty() {
        return Err(format!("push: the branch {} exists already on the remote; the hand-back only makes a new branch", handed.branch));
    }
    if !before_push(handed) {
        return Err("cancelled: the run was cancelled before its push; nothing was pushed".to_owned());
    }
    let mut push = remote_git();
    push.args(["push", "--quiet", "--no-verify", &entry.remote, &format!("{}:{target}", handed.commit)]);
    output(push, "push").map(|_| ())
}

/// A tree whose change was pushed is finished (`worktree finish`) and removed (`worktree gc --apply`), after its lease
/// ended. Whether both took it.
pub fn finish(entry: &Repository, run_id: &str) -> bool {
    let id = tree_id(run_id);
    worktree(&["finish", &id]).is_ok() && worktree(&["gc", "--apply", "--repo", &entry.path.display().to_string(), "--id", &id]).is_ok()
}

/// Whether git's prompt (`Username for 'https://host': `, `Password for 'https://user@host': `) asks for `origin`
/// (`https://<host>`), the user left out.
fn asks_origin(prompt: &str, origin: &str) -> bool {
    let Some(host) = origin.strip_prefix("https://").filter(|h| !h.is_empty()) else { return false };
    let Some(url) = prompt.split('\'').nth(1) else { return false };
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let authority = rest.split('/').next().unwrap_or("");
    let asked = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    asked == host
}

/// The `runbook` binary as git's askpass: started with [`ASKPASS_VARIABLE`] naming a token file and git's prompt as its
/// one argument, it prints the bot user for a user name and the token for anything else. The file is checked as a
/// route's token file is. `None`: the process is no askpass (the variable is not set).
pub fn askpass() -> Option<std::process::ExitCode> {
    use std::io::Write;
    let file = std::env::var_os(ASKPASS_VARIABLE)?;
    let prompt = std::env::args().nth(1).unwrap_or_default();
    let origin = std::env::var(ASKPASS_ORIGIN_VARIABLE).unwrap_or_default();
    let answer = if !asks_origin(&prompt, &origin) {
        Err("the prompt names no URL of the entry's remote".to_owned())
    } else if prompt.trim_start().starts_with("Username") {
        Ok(ASKPASS_USER.to_owned())
    } else {
        crate::routes::read_token_file(&file.to_string_lossy(), ASKPASS_VARIABLE)
    };
    Some(match answer {
        Ok(text) if writeln!(std::io::stdout(), "{text}").is_ok() => std::process::ExitCode::SUCCESS,
        Ok(_) => std::process::ExitCode::FAILURE,
        Err(why) => {
            let _ = writeln!(std::io::stderr(), "runbook askpass: {why}");
            std::process::ExitCode::FAILURE
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(extra: &str) -> Result<Repositories, String> {
        let text = format!("[repositories.demo]\npath = \"demo\"\nbase = \"main\"\nremote = \"https://forge.invalid/org/demo.git\"\ndefault_branch = \"main\"\ncommit_name = \"bot\"\ncommit_email = \"bot@example.invalid\"\n{extra}");
        let v: Value = toml::from_str(&text).unwrap();
        Repositories::declare(v.get(REPOSITORIES), Path::new("/etc/runbook"))
    }

    #[test]
    fn an_entry_is_checked_key_by_key_and_its_paths_are_relative_to_the_file() {
        let r = entry("").unwrap();
        let e = r.get("demo").unwrap();
        assert_eq!((e.path.as_path(), e.deny_paths.clone()), (Path::new("/etc/runbook/demo"), DEFAULT_DENY.iter().map(|p| p.to_string()).collect::<Vec<_>>()));
        assert_eq!(Repositories::declare(None, Path::new("/")).unwrap(), Repositories::default());
        for (extra, word) in [
            ("token = \"x\"\n", "token"),
            ("deny_paths = [\"../x\"]\n", "deny_paths"),
            ("deny_paths = \"x\"\n", "deny_paths"),
            ("push_token_file = 3\n", "push_token_file"),
        ] {
            assert!(entry(extra).unwrap_err().contains(word), "{extra}");
        }
        let swap = |from: &str, to: &str| {
            let text = "[repositories.demo]\npath = \"demo\"\nbase = \"main\"\nremote = \"https://forge.invalid/org/demo.git\"\ndefault_branch = \"main\"\ncommit_name = \"bot\"\ncommit_email = \"bot@example.invalid\"\n".replace(from, to);
            let v: Value = toml::from_str(&text).unwrap();
            Repositories::declare(v.get(REPOSITORIES), Path::new("/")).unwrap_err()
        };
        assert!(swap("https://forge.invalid", "https://user:pw@forge.invalid").contains("remote"));
        assert!(swap("https://forge.invalid", "https:///bot:pw@forge.invalid").contains("remote"), "a user and password after a third slash");
        assert!(swap("https://forge.invalid/org/demo.git", "https:///demo.git").contains("remote"), "no host");
        assert!(swap("https://forge.invalid/org/demo.git", "file://host/demo.git").contains("remote"), "a file URL with a host");
        for bad in ["src//", "a/./b", "./a", "/a", "a//b/"] {
            assert!(entry(&format!("deny_paths = [\"{bad}\"]\n")).unwrap_err().contains("deny_paths"), "{bad}");
        }
        assert!(swap("https://forge.invalid/org/demo.git", "ssh://forge.invalid/demo").contains("remote"));
        assert!(swap("base = \"main\"", "base = \"--upload-pack=x\"").contains("base"));
        assert!(swap("default_branch = \"main\"", "default_branch = \"runbook/x\"").contains("default_branch"));
        assert!(swap("bot@example.invalid", "bot").contains("commit_email"));
        assert!(swap("[repositories.demo]", "[repositories.\"a/b\"]").contains("<name>"));
    }

    #[test]
    fn a_repository_is_named_by_its_entry_never_by_a_path() {
        let r = entry("").unwrap();
        assert_eq!(refusal(&r, "demo"), None);
        for named in ["/tmp/demo", "../demo", "demo/..", "", "other"] {
            assert!(refusal(&r, named).is_some(), "{named:?}");
        }
        assert!(refusal(&Repositories::default(), "demo").unwrap().contains("lists no repository"));
    }

    #[test]
    fn a_denied_path_is_a_directory_with_a_slash_or_a_path_and_what_is_under_it() {
        let deny: Vec<String> = DEFAULT_DENY.iter().map(|p| p.to_string()).collect();
        assert_eq!(denied(&deny, ".github/workflows/x.yml"), Some(".github/"));
        assert_eq!(denied(&deny, ".gitlab-ci.yml"), Some(".gitlab-ci.yml"));
        assert_eq!(denied(&deny, ".gitmodules"), Some(".gitmodules"));
        assert_eq!(denied(&deny, ".githubx"), None);
        assert_eq!(denied(&deny, "src/.github/x"), None);
        assert_eq!(denied(&["ci".to_owned()], "ci/x"), Some("ci"));
        assert_eq!(denied(&["ci".to_owned()], "cix"), None);
    }

    #[test]
    fn a_link_leaves_the_tree_when_its_target_is_absolute_or_climbs_above_the_root() {
        for (path, target) in [("a", "/etc/passwd"), ("a", "../x"), ("src/a", "../../x"), ("src/a", "./../../x"), ("src/b/a", "../../../x")] {
            assert!(leaves_tree(path, Path::new(target)), "{path} -> {target}");
        }
        for (path, target) in [("a", "b"), ("src/a", "../b"), ("src/a", "./b"), ("src/b/a", "../../c/d"), ("a", "b/../c")] {
            assert!(!leaves_tree(path, Path::new(target)), "{path} -> {target}");
        }
    }

    #[test]
    fn the_askpass_answers_only_for_the_host_of_the_entrys_remote() {
        let origin = "https://forge.invalid";
        assert!(asks_origin("Username for 'https://forge.invalid': ", origin));
        assert!(asks_origin("Password for 'https://x-access-token@forge.invalid': ", origin));
        for prompt in ["Password for 'https://x-access-token@elsewhere.invalid': ", "Password for 'https://forge.invalid.elsewhere.invalid': ", "Password: ", "Password for 'http://forge.invalid': "] {
            assert!(!asks_origin(prompt, origin), "{prompt}");
        }
        assert!(!asks_origin("Password for 'https://forge.invalid': ", ""), "no origin named");
        assert_eq!(super::origin("https://forge.invalid/org/demo.git"), Some(origin.to_owned()));
    }
}
