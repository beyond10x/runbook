//! `scripts/install.sh --prefix <dir>`: the layout it writes, the refusals, the second install, and an
//! installed `runbook` that runs from an empty directory with `<prefix>/bin` on `PATH` and nothing else.
//!
//! The fast lane makes one prefix from the debug builds (`--from-built debug`) once per test run: a directory
//! under the target directory keyed by a digest of the script, the built files and the asset sources, made by
//! the first test that needs it under a file lock and read by the others (nextest runs each test in its own
//! process, so a global in the process would build it once per test). The slow lane (`task test:install`) runs the
//! `--release` build into a prefix of its own and the same commands, and the acceptance run `runbook run sdc --dry`
//! on the prefix of the fast lane (`slow.rs`). `guards.rs` and `rules.rs` hold the cases of what is copied, the
//! modes, the printed lines, the marker of a half install, the checks of the arguments and the commit.

mod caller_environment;
mod during_the_build;
mod guards;
mod half_install;
mod installed_path;
mod modes;
mod refused_prefixes;
mod rules;
mod slow;
mod stale_build;
mod usage_text;
mod version_file;
mod what_is_copied;
mod while_the_files_are_copied;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use super::{repo, BIN, LAYOUT_DIRS};

/// The programs of `driver/` and `watch/` a run starts: the ones `runbook doctor` requires of every runbook, and the
/// three that `runbook run` or its tools start besides them.
const HELPERS: [&str; 11] = [
    "sdc-answers",
    "sdc-bugfix",
    "sdc-check-routes",
    "sdc-conform",
    "sdc-conform-adapter",
    "sdc-decision-request",
    "sdc-findings",
    "sdc-node",
    "sdc-replay",
    "sdc-reply",
    "sdc-watch",
];

const SYSTEM_PATH: &str = "/usr/bin:/bin";

/// Added to every `install.sh` command that runs after `env_clear()`: git of the child reads no system configuration
/// file. `env_clear()` and a scratch `HOME` hide the user's configuration only; a system file of the machine
/// (`safe.directory = *`, `core.fsmonitor`, `core.hooksPath`, a credential helper) still applies, and a case that
/// needs "git refuses this checkout for its owner" was red on a forge runner image that trusts every directory and
/// green on a host without that file. A case that needs a system file sets `GIT_CONFIG_SYSTEM` instead.
const NO_SYSTEM_GIT: [(&str, &str); 1] = [("GIT_CONFIG_NOSYSTEM", "1")];

fn script() -> PathBuf {
    repo().join("scripts/install.sh")
}

fn built(profile: &str, name: &str) -> PathBuf {
    let krate = match name {
        "runbook" => "runbook",
        "sdc-watch" => "watch",
        _ => "driver",
    };
    repo().join(krate).join("target").join(profile).join(name)
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

/// A directory of a case that is removed when the case ends (the test thread drops it). `RUNBOOK_INSTALL_KEEP_SCRATCH`
/// set to anything keeps every directory, to look at what a case left.
struct ScratchDir(PathBuf);

impl Drop for ScratchDir {
    fn drop(&mut self) {
        if std::env::var_os("RUNBOOK_INSTALL_KEEP_SCRATCH").is_none() {
            let _ = Command::new("chmod").arg("-R").arg("u+rwx").arg(&self.0).output();
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

thread_local! {
    static SCRATCH_DIRS: std::cell::RefCell<Vec<ScratchDir>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn scratch(tag: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("install-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    SCRATCH_DIRS.with(|dirs| dirs.borrow_mut().push(ScratchDir(d.clone())));
    d
}

/// Every file and symbolic link below `dir`, relative, sorted, with its size and mode: what a script that
/// writes nowhere else leaves unchanged.
fn listing(dir: &Path) -> Vec<String> {
    fn walk(dir: &Path, base: &Path, into: &mut Vec<String>) {
        for e in fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            let rel = p.strip_prefix(base).unwrap().display().to_string();
            let m = fs::symlink_metadata(&p).unwrap();
            if m.is_dir() {
                into.push(format!("{rel}/"));
                walk(&p, base, into);
            } else {
                into.push(format!("{rel} {} {:o}", m.len(), m.permissions().mode() & 0o777));
            }
        }
    }
    let mut v = Vec::new();
    walk(dir, dir, &mut v);
    v.sort();
    v
}

fn files_below(dir: &Path) -> BTreeSet<String> {
    listing(dir).into_iter().filter(|l| !l.ends_with('/')).map(|l| l.rsplitn(3, ' ').last().unwrap().to_owned()).collect()
}

fn install(script: &Path, prefix: &Path, extra: &[&str]) -> Output {
    Command::new("sh")
        .arg(script)
        .arg("--prefix")
        .arg(prefix)
        .args(extra)
        .env_clear().envs(NO_SYSTEM_GIT)
        .env("PATH", SYSTEM_PATH)
        .env("HOME", prefix.parent().unwrap().join("home"))
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

// ---- the shared prefix of the fast lane ----

fn stat_line(p: &Path) -> String {
    let m = fs::metadata(p).unwrap_or_else(|e| panic!("{}: {e} (build the driver binaries and `watch`: `task test:runbook` does)", p.display()));
    let ns = m.modified().unwrap().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    format!("{} {} {ns}\n", p.display(), m.len())
}

/// The key of the shared prefix: the script, the built files it takes (path, size, time) and the asset sources.
fn key() -> String {
    let mut h = Sha256::new();
    h.update(fs::read(script()).unwrap());
    h.update(stat_line(Path::new(BIN)));
    for n in HELPERS {
        h.update(stat_line(&built("debug", n)));
    }
    h.update(assets_digest(&repo()));
    h.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// The directories whose files the script installs as assets.
const ASSET_DIRS: [&str; 9] = ["protocol", "scripts", "driver/examples/sdc", "bindings", "instructions", "views", "cycles", "tools", "ui"];

/// The asset sources of the checkout at `root`, as they enter the key: the files of `ASSET_DIRS` that git tracks (the
/// script copies those and no other), each by its name and its bytes (a link by its target), read from the work tree as
/// the script reads them.
fn assets_digest(root: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let o = Command::new("git")
        .args(["ls-files", "-z", "--"])
        .args(ASSET_DIRS)
        .current_dir(root)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .envs(NO_SYSTEM_GIT)
        .output()
        .unwrap();
    assert!(o.status.success(), "git ls-files in {}: {}", root.display(), err(&o));
    let mut h = Sha256::new();
    for name in o.stdout.split(|b| *b == 0).filter(|n| !n.is_empty()) {
        let p = root.join(std::ffi::OsStr::from_bytes(name));
        h.update(name);
        h.update([0]);
        let body = match fs::symlink_metadata(&p) {
            Ok(m) if m.file_type().is_symlink() => [b"link ".as_slice(), fs::read_link(&p).unwrap().as_os_str().as_bytes()].concat(),
            Ok(m) if m.is_file() => [b"file ".as_slice(), &fs::read(&p).unwrap()].concat(),
            _ => b"absent".to_vec(),
        };
        h.update((body.len() as u64).to_le_bytes());
        h.update(&body);
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// The key moves with what the script would copy and with nothing else, so no case removes the prefix another case is
/// reading while the gate's other steps write beside the tests: a file under `ui/screenshots` (git-ignored, written by
/// the UI step), an untracked file and a tracked file written again with the same bytes and a new time (the protocol
/// checks write `protocol/compiled` on every run) leave it; a change of the bytes of a tracked file moves it.
#[test]
fn the_key_of_the_shared_prefix_moves_with_the_bytes_of_the_tracked_assets_only() {
    let root = scratch("key-assets");
    let git = |args: &[&str]| {
        let o = Command::new("git").args(args).current_dir(&root).env_remove("GIT_DIR").env_remove("GIT_WORK_TREE").envs(NO_SYSTEM_GIT).output().unwrap();
        assert!(o.status.success(), "git {args:?}: {}", err(&o));
    };
    let put = |rel: &str, text: &str| {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    };
    git(&["init", "-q"]);
    put(".gitignore", "ui/screenshots/\n");
    put("ui/web/app.js", "app\n");
    put("protocol/compiled/a.ir.json", "{}\n");
    git(&["add", "-A"]);
    let before = assets_digest(&root);
    put("ui/screenshots/390x844-mobile.png", "png");
    assert_eq!(assets_digest(&root), before, "a git-ignored file written under ui/");
    put("ui/web/new.js", "new\n");
    assert_eq!(assets_digest(&root), before, "an untracked file written under ui/");
    put("protocol/compiled/a.ir.json", "{}\n");
    let file = fs::File::options().write(true).open(root.join("protocol/compiled/a.ir.json")).unwrap();
    file.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(86_400)).unwrap();
    assert_eq!(assets_digest(&root), before, "a tracked file written again with the same bytes and another time");
    put("protocol/compiled/a.ir.json", "{\"a\":1}\n");
    assert_ne!(assets_digest(&root), before, "a tracked file with other bytes");
}

/// The prefix made from the debug builds, built by the first caller under a lock, read by every other.
fn shared_prefix() -> PathBuf {
    shared_prefix_of("whole", &|partial| {
        let o = install(&script(), &partial.join("prefix"), &["--from-built", "debug"]);
        assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    })
}

/// `<base>/<key>-<kind>/prefix`, made by `make(<directory with an empty prefix/ and home/>)` by the first caller under
/// the lock; the directories of an older key are removed. Each kind is made once per run of the tests, not once per test.
fn shared_prefix_of(kind: &str, make: &dyn Fn(&Path)) -> PathBuf {
    let base = Path::new(env!("CARGO_TARGET_TMPDIR")).join("install-prefix");
    fs::create_dir_all(&base).unwrap();
    let lock = fs::OpenOptions::new().create(true).truncate(false).write(true).open(base.join("lock")).unwrap();
    lock.lock().unwrap();
    let key = key();
    let dir = base.join(format!("{key}-{kind}"));
    if !dir.join("prefix").is_dir() {
        for e in fs::read_dir(&base).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.path().is_dir() && (!name.starts_with(&key) || name == format!("{key}-{kind}.partial")) {
                fs::remove_dir_all(e.path()).unwrap();
            }
        }
        let partial = base.join(format!("{key}-{kind}.partial"));
        fs::create_dir_all(partial.join("prefix")).unwrap();
        fs::create_dir_all(partial.join("home")).unwrap();
        make(&partial);
        fs::rename(&partial, &dir).unwrap();
    }
    drop(lock);
    dir.join("prefix")
}

// ---- running the installed runbook ----

/// The toolchain the tests run under, for a child whose `HOME` is a scratch directory. `cargo`, `cargo-clippy` and
/// `rustfmt` of `/usr/bin` are rustup proxies, and `runbook doctor` starts each: with a scratch `HOME` and the
/// pinned `rust-toolchain.toml` above it a proxy finds no toolchain and tries to download one (each probe then waits
/// 5 s for its answer, and a download of 200 MB into `HOME` may follow). The child gets the rustup home of this
/// process, the toolchain it runs under, and no automatic install.
fn with_toolchain(cmd: &mut Command) -> &mut Command {
    let real_home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let rustup = std::env::var_os("RUSTUP_HOME").map(PathBuf::from).unwrap_or_else(|| real_home.join(".rustup"));
    cmd.env("RUSTUP_HOME", rustup).env("RUSTUP_AUTO_INSTALL", "0");
    if let Some(t) = std::env::var_os("RUSTUP_TOOLCHAIN") {
        cmd.env("RUSTUP_TOOLCHAIN", t);
    }
    cmd
}

/// `runbook <args>` from `cwd` with `PATH` = `<prefix>/bin` and the system directories, no `RUNBOOK_HOME`.
pub(crate) fn from_prefix(prefix: &Path, cwd: &Path, args: &[&str]) -> Output {
    let path = format!("{}:{SYSTEM_PATH}", prefix.join("bin").display());
    let mut c = Command::new("runbook");
    c.args(args).env_clear();
    crate::isolate(&mut c);
    with_toolchain(&mut c).env("PATH", path).env("HOME", cwd).current_dir(cwd).stdin(Stdio::null()).output().unwrap()
}

/// The same command with the build of the checkout.
pub(crate) fn from_checkout(cwd: &Path, args: &[&str]) -> Output {
    let mut c = Command::new(BIN);
    c.args(args).env_clear();
    crate::isolate(&mut c);
    with_toolchain(&mut c).env("PATH", SYSTEM_PATH).env("HOME", cwd).current_dir(cwd).stdin(Stdio::null()).output().unwrap()
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().flatten() {
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_tree(&e.path(), &target);
        } else {
            fs::copy(e.path(), &target).unwrap();
        }
    }
}

/// An empty working directory holding the hand-written runbook (`protocol/`, `bindings/`, `intake.yaml`,
/// `settings.toml`) of `tests/fixtures/install-runbook`.
pub(crate) fn world(tag: &str) -> PathBuf {
    let w = scratch(tag);
    copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/install-runbook"), &w);
    fs::write(w.join("intake.yaml"), "text: kettle and teacup\n").unwrap();
    fs::write(w.join("settings.toml"), "in_process = true\nconfinement = \"local\"\n\n[decider]\nkind = \"scripted\"\nmax_call_micro_usd = 0\nanswers = []\n").unwrap();
    w
}

/// The run of the example runbook in `w`, by the command `with`; the `outcome.json` it wrote.
pub(crate) fn run_example(with: &dyn Fn(&Path, &[&str]) -> Output, w: &Path) -> (Value, String) {
    let args = [
        "run",
        "example",
        "--run",
        "r1",
        "--runs-dir",
        "runs",
        "--protocols",
        "protocol",
        "--bindings",
        "bindings",
        "--root-protocol",
        "example@1",
        "--case",
        "CASE-1",
        "--intake",
        "intake.yaml",
        "--confinement",
        "local",
        "--settings",
        "settings.toml",
        "--spend-ledger",
        "ledger.json",
        "--cap-micro-usd",
        "0",
        "--max-llm-calls",
        "1",
        "--max-wall-seconds",
        "50",
    ];
    let o = with(w, &args);
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    let text = fs::read_to_string(w.join("runs/r1/outcome.json")).unwrap_or_else(|e| panic!("no outcome.json: {e}\n{}{}", out(&o), err(&o)));
    let where_found = fs::read_to_string(w.join("runs/r1/work/where-sdc-reply.txt")).unwrap_or_default();
    (serde_json::from_str(&text).unwrap(), where_found.trim().to_owned())
}

/// `outcome.json` without what differs between two runs: times, durations and the paths of the two worlds.
fn comparable(outcome: &Value, world: &Path) -> String {
    fn scrub(v: &mut Value, world: &str) {
        match v {
            Value::Object(m) => {
                m.retain(|k, _| !(k.ends_with("_at") || k.ends_with("_ms") || k.ends_with("_seconds") || k == "started" || k == "finished" || k == "elapsed"));
                m.values_mut().for_each(|x| scrub(x, world));
            }
            Value::Array(a) => a.iter_mut().for_each(|x| scrub(x, world)),
            Value::String(s) => *s = s.replace(world, "<world>"),
            _ => {}
        }
    }
    let mut v = outcome.clone();
    scrub(&mut v, &world.display().to_string());
    serde_json::to_string_pretty(&v).unwrap()
}

// ---- tests ----

#[test]
fn the_prefix_holds_the_binary_the_helpers_and_the_files_and_its_manifest_lists_every_file_it_wrote() {
    let prefix = shared_prefix();
    assert!(Path::new(&prefix.join("bin/runbook")).metadata().unwrap().permissions().mode() & 0o111 != 0);
    for h in HELPERS {
        assert!(
            prefix
                .join("libexec/runbook")
                .join(h)
                .metadata()
                .unwrap()
                .permissions()
                .mode()
                & 0o111
                != 0,
            "libexec/runbook/{h}"
        );
    }
    let share = prefix.join("share/runbook");
    for d in LAYOUT_DIRS {
        assert!(share.join(d).is_dir(), "share/runbook/{d}");
    }
    assert!(share.join("tools/autonomy-levels.json").is_file());
    assert!(share.join("bindings/tools/sdc-tool.sh").metadata().unwrap().permissions().mode() & 0o111 != 0);
    let manifest = fs::read_to_string(share.join("MANIFEST")).unwrap();
    let listed: Vec<&str> = manifest.lines().collect();
    let mut sorted = listed.clone();
    sorted.sort();
    assert_eq!(listed, sorted, "MANIFEST is sorted");
    assert_eq!(listed.iter().copied().collect::<BTreeSet<_>>(), files_below(&prefix).iter().map(String::as_str).collect::<BTreeSet<_>>(), "MANIFEST lists every file of the prefix, and only those");
    let version = fs::read_to_string(share.join("VERSION")).unwrap();
    let said = from_prefix(&prefix, &prefix, &["--version"]);
    assert_eq!(code(&said), 0);
    let marked = version.trim().strip_suffix(" dirty");
    assert_eq!(marked.unwrap_or(version.trim()), out(&said).trim(), "VERSION is what `runbook --version` prints, and ` dirty` after it when a tracked file differs from HEAD");
    let cargo_version = env!("CARGO_PKG_VERSION");
    assert!(version.contains(cargo_version), "{version}");
    let head = Command::new("git").args(["rev-parse", "HEAD"]).current_dir(repo()).output().unwrap();
    let head = out(&head).trim().to_owned();
    assert!(version.contains(&head[..12]), "{version} does not carry the commit {head}");
}

/// `runbook list` of the prefix from an empty directory: the protocols of the repository, as the checkout lists them.
pub(crate) fn list_from(prefix: &Path) {
    let cwd = scratch("list");
    let o = from_prefix(prefix, &cwd, &["list"]);
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    let mine = out(&o);
    assert!(mine.contains("sdc") && mine.contains("sdc-bugfix"), "{mine}");
    let theirs = from_checkout(&cwd, &["list"]);
    // the column of the runbook directory differs by design: the share directory of the prefix against the checkout
    let share = format!("{}", prefix.join("bin/../share/runbook").display());
    let rows = |text: String, home: &str| text.replace(home, "<home>");
    assert!(mine.contains(&share), "the installed list names the share directory of the prefix: {mine}");
    assert!(out(&theirs).contains(repo().to_str().unwrap()), "the checkout's list names the checkout: {}", out(&theirs));
    assert_eq!(rows(mine, &share), rows(out(&theirs), repo().to_str().unwrap()), "the installed list equals the checkout's");
}

#[test]
fn the_installed_runbook_lists_the_protocols_from_an_empty_directory() {
    list_from(&shared_prefix());
}

#[test]
fn the_installed_doctor_finds_the_assets_and_the_helpers_in_the_prefix() {
    doctor_from(&shared_prefix());
}

/// `runbook doctor` of the prefix from an empty directory: the assets are the installed ones, every layout part
/// and every helper is found, the helpers in `libexec`.
pub(crate) fn doctor_from(prefix: &Path) {
    let cwd = scratch("doctor");
    let o = from_prefix(prefix, &cwd, &["doctor"]);
    let text = out(&o);
    let line = |name: &str| text.lines().find(|l| l.split("  ").map(str::trim).filter(|s| !s.is_empty()).nth(1) == Some(name)).unwrap_or_else(|| panic!("no {name} line:\n{text}\n{}", err(&o))).to_owned();
    let assets = line("assets");
    assert!(assets.contains("installed (share/runbook)"), "{assets}");
    assert!(
        assets.contains(&prefix.join("bin/../share/runbook").display().to_string())
            || assets.contains(&prefix.join("share/runbook").display().to_string()),
        "{assets}"
    );
    let install = line("install");
    assert!(
        install.trim_start().starts_with("pass") && install.contains("whole"),
        "a whole prefix passes the install line: {install}"
    );
    for part in LAYOUT_DIRS {
        let l = line(&format!("{part}/"));
        assert!(l.trim_start().starts_with("pass"), "{l}");
    }
    for h in HELPERS {
        let l = line(h);
        assert!(l.trim_start().starts_with("pass"), "{l}");
        assert!(
            l.contains("libexec/runbook"),
            "{h} is not found in libexec/runbook: {l}"
        );
    }
}

#[test]
fn a_run_from_the_prefix_ends_with_the_outcome_of_the_same_run_from_the_checkout_build() {
    run_from(&shared_prefix());
}

/// One run with sh programs only from the prefix, and the same command from the checkout build: the same
/// `outcome.json` but for times and paths.
pub(crate) fn run_from(prefix: &Path) {
    let installed = world("run-installed");
    let checkout = world("run-checkout");
    let (a, where_a) = run_example(&|cwd, args| from_prefix(prefix, cwd, args), &installed);
    let (b, where_b) = run_example(&|cwd, args| from_checkout(cwd, args), &checkout);
    assert_eq!(comparable(&a, &installed), comparable(&b, &checkout));
    assert_eq!(a["reason"], json!("done"), "{a}");
    assert!(
        where_a.ends_with("libexec/runbook/sdc-reply"),
        "a program the run starts finds the helpers of the prefix on PATH: {where_a:?}"
    );
    assert_eq!(where_b, "", "the checkout build adds nothing to PATH");
}

// ---- the refusals ----

#[test]
fn the_script_refuses_a_prefix_that_is_relative_the_root_missing_or_inside_the_cargo_directory_and_writes_nothing() {
    let w = scratch("refuse");
    let home = w.join("home");
    fs::create_dir_all(home.join(".cargo/bin")).unwrap();
    fs::create_dir_all(w.join("elsewhere")).unwrap();
    std::os::unix::fs::symlink(home.join(".cargo/bin"), w.join("link-to-cargo")).unwrap();
    let before = listing(&w);
    let cases: Vec<(String, &str)> = vec![
        ("relative/prefix".into(), "absolute"),
        (".".into(), "absolute"),
        ("/".into(), "root"),
        (w.join("missing").display().to_string(), "exist"),
        (home.join(".cargo").display().to_string(), ".cargo"),
        (home.join(".cargo/bin").display().to_string(), ".cargo"),
        (format!("{}/", home.join(".cargo/bin").display()), ".cargo"),
        (w.join("link-to-cargo").display().to_string(), ".cargo"),
        (w.join("elsewhere/../home/.cargo/bin").display().to_string(), ".cargo"),
    ];
    for (prefix, word) in &cases {
        let o = Command::new("sh")
            .arg(script())
            .args(["--prefix", prefix, "--from-built", "debug"])
            .env_clear()
            .env("PATH", SYSTEM_PATH)
            .env("HOME", &home)
            .current_dir(&w)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(code(&o), 2, "{prefix}: {}{}", out(&o), err(&o));
        assert!(err(&o).contains(word), "{prefix}: stderr names {word:?}: {}", err(&o));
        assert_eq!(listing(&w), before, "{prefix}: the refusal wrote something");
    }
    for args in [vec![], vec!["--prefix"], vec!["--from-built", "debug"], vec!["--prefix", "/x", "--from-built", "fast"], vec!["--nothing"]] {
        let o = Command::new("sh").arg(script()).args(&args).env_clear().env("PATH", SYSTEM_PATH).env("HOME", &home).stdin(Stdio::null()).output().unwrap();
        assert_eq!(code(&o), 2, "{args:?}: {}{}", out(&o), err(&o));
        assert!(err(&o).contains("usage"), "{args:?}: {}", err(&o));
    }
    let o = Command::new("sh").arg(script()).arg("--help").env_clear().env("PATH", SYSTEM_PATH).stdin(Stdio::null()).output().unwrap();
    assert_eq!(code(&o), 0);
    assert!(out(&o).contains("--prefix") && out(&o).contains("--from-built"), "{}", out(&o));
}

/// `git <args>` in `dir` with an identity of its own, for a fake checkout.
pub(crate) fn git_in(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.org",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .unwrap();
    assert!(o.status.success(), "git {args:?}: {}", err(&o));
    out(&o).trim().to_owned()
}

/// The version line of the stub programs of a fake checkout whose commit is `commit`: what `runbook --version` prints.
pub(crate) fn stub_version(commit: &str) -> String {
    format!("runbook 9.9.9 ({commit})")
}

/// The shell stub of a program that prints `version` for `--version` (`--from-built` runs the one of `runbook`).
pub(crate) fn write_stub(path: &Path, version: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, format!("#!/bin/sh\necho \"{version}\"\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// A tree that has the shape of a checkout: one file in each directory the script copies, a copy of the script, and
/// a shell stub for every built file `--from-built` takes. In a git work tree of its own, committed: what the script
/// copies is what git tracks, and the stubs print the commit of HEAD. See `fake_archive` for the same without git.
pub(crate) fn fake_checkout(tag: &str, skip: Option<&str>) -> PathBuf {
    let root = fake_tree(tag);
    git_in(&root, &["init", "-q"]);
    git_in(&root, &["add", "-A"]);
    git_in(&root, &["commit", "-q", "-m", "fake checkout"]);
    let head: String = git_in(&root, &["rev-parse", "HEAD"])
        .chars()
        .take(12)
        .collect();
    stubs(&root, skip, &stub_version(&head));
    root
}

/// The same tree without git (an unpacked source archive); its stubs print the commit `unknown`.
pub(crate) fn fake_archive(tag: &str) -> PathBuf {
    let root = fake_tree(tag);
    stubs(&root, None, &stub_version("unknown"));
    root
}

fn fake_tree(tag: &str) -> PathBuf {
    let root = scratch(tag).join("checkout");
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::copy(script(), root.join("scripts/install.sh")).unwrap();
    for d in LAYOUT_DIRS.iter().chain(&["tools", "ui/web"]) {
        fs::create_dir_all(root.join(d)).unwrap();
        fs::write(root.join(d).join("file.txt"), format!("{d}\n")).unwrap();
    }
    fs::write(root.join("ui/server.mjs"), "// the page's server\n").unwrap();
    fs::write(root.join("ui/package.json"), "{}\n").unwrap();
    fs::write(root.join("bindings/tools/sdc-tool.sh"), "#!/bin/sh\n").unwrap();
    fs::set_permissions(
        root.join("bindings/tools/sdc-tool.sh"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    root
}

/// The built files of the debug profile, as stubs, except `skip`.
pub(crate) fn stubs(root: &Path, skip: Option<&str>, version: &str) {
    for n in std::iter::once("runbook").chain(HELPERS) {
        if Some(n) == skip {
            continue;
        }
        write_stub(
            &root.join(built("debug", n).strip_prefix(repo()).unwrap()),
            version,
        );
    }
}

#[test]
fn a_missing_built_file_with_from_built_is_named_and_nothing_is_written() {
    let root = fake_checkout("missing-built", Some("sdc-reply"));
    let sandbox = root.parent().unwrap().to_path_buf();
    let prefix = sandbox.join("prefix");
    fs::create_dir_all(&prefix).unwrap();
    fs::create_dir_all(sandbox.join("home")).unwrap();
    let before = listing(&sandbox);
    let o = install(&root.join("scripts/install.sh"), &prefix, &["--from-built", "debug"]);
    assert_ne!(code(&o), 0, "{}{}", out(&o), err(&o));
    assert!(err(&o).contains("driver/target/debug/sdc-reply"), "{}", err(&o));
    assert_eq!(listing(&sandbox), before, "a refused install wrote something");
    let o = install(&root.join("scripts/install.sh"), &prefix, &["--from-built", "release"]);
    assert_ne!(code(&o), 0);
    assert!(err(&o).contains("runbook/target/release/runbook"), "the first missing file is named: {}", err(&o));
}

#[test]
fn the_script_writes_only_under_the_prefix() {
    let root = fake_checkout("only-prefix", None);
    let sandbox = root.parent().unwrap().to_path_buf();
    let prefix = sandbox.join("prefix");
    fs::create_dir_all(&prefix).unwrap();
    fs::create_dir_all(sandbox.join("home")).unwrap();
    let before_root = listing(&root);
    let before_home = listing(&sandbox.join("home"));
    let o = install(&root.join("scripts/install.sh"), &prefix, &["--from-built", "debug"]);
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    assert_eq!(listing(&root), before_root, "the checkout changed");
    assert_eq!(listing(&sandbox.join("home")), before_home, "the home directory changed");
    assert_eq!(fs::read_dir(&sandbox).unwrap().count(), 3, "checkout, home and prefix only");
    let said = out(&o);
    assert!(said.contains(&format!("export PATH=\"{}/bin:$PATH\"", prefix.display())), "{said}");
    assert_eq!(said.lines().filter(|l| l.contains(&format!("{}/bin", prefix.display()))).count(), 3, "three PATH lines: {said}");
}

#[test]
fn a_second_install_removes_the_file_a_dropped_helper_left_and_keeps_a_file_the_script_never_wrote() {
    let root = fake_checkout("second", None);
    let sandbox = root.parent().unwrap().to_path_buf();
    let prefix = sandbox.join("prefix");
    fs::create_dir_all(&prefix).unwrap();
    fs::create_dir_all(sandbox.join("home")).unwrap();
    let copy = root.join("scripts/install.sh");
    let o = install(&copy, &prefix, &["--from-built", "debug"]);
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    assert!(prefix.join("libexec/runbook/sdc-answers").is_file());
    let manifest = fs::read_to_string(prefix.join("share/runbook/MANIFEST")).unwrap();
    assert!(manifest.lines().any(|l| l == "libexec/runbook/sdc-answers"));
    fs::write(prefix.join("share/runbook/protocol/mine.txt"), "mine\n").unwrap();
    fs::write(prefix.join("libexec/runbook/foreign"), "foreign\n").unwrap();
    fs::write(prefix.join("notes.txt"), "notes\n").unwrap();
    let outside = sandbox.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("file.txt"), "outside\n").unwrap();
    let text = fs::read_to_string(&copy).unwrap();
    let dropped = text.replacen("\nsdc-answers\n", "\n", 1);
    assert_ne!(text, dropped, "the helper list of the script has a line `sdc-answers`");
    fs::write(&copy, dropped).unwrap();
    let o = install(&copy, &prefix, &["--from-built", "debug"]);
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    assert!(
        !prefix.join("libexec/runbook/sdc-answers").exists(),
        "the dropped helper stays"
    );
    assert!(prefix.join("libexec/runbook/sdc-node").is_file());
    for kept in [
        "share/runbook/protocol/mine.txt",
        "libexec/runbook/foreign",
        "notes.txt",
    ] {
        assert!(prefix.join(kept).is_file(), "{kept} was removed");
    }
    let manifest = fs::read_to_string(prefix.join("share/runbook/MANIFEST")).unwrap();
    assert!(!manifest.lines().any(|l| l == "libexec/runbook/sdc-answers"));
    assert!(!manifest
        .lines()
        .any(|l| l.ends_with("mine.txt") || l.ends_with("foreign") || l.ends_with("notes.txt")));
    assert_eq!(files_below(&outside).len(), 1);
}

#[test]
fn a_manifest_path_that_leaves_the_prefix_or_goes_through_a_link_is_never_removed() {
    let root = fake_checkout("links", None);
    let sandbox = root.parent().unwrap().to_path_buf();
    let prefix = sandbox.join("prefix");
    fs::create_dir_all(&prefix).unwrap();
    fs::create_dir_all(sandbox.join("home")).unwrap();
    let copy = root.join("scripts/install.sh");
    assert_eq!(code(&install(&copy, &prefix, &["--from-built", "debug"])), 0);
    let outside = sandbox.join("outside");
    fs::create_dir_all(&outside).unwrap();
    for n in ["a.txt", "b.txt", "c.txt"] {
        fs::write(outside.join(n), "outside\n").unwrap();
    }
    fs::write(sandbox.join("elsewhere.txt"), "elsewhere\n").unwrap();
    std::os::unix::fs::symlink(&outside, prefix.join("share/runbook/linked")).unwrap();
    let manifest_path = prefix.join("share/runbook/MANIFEST");
    let mut manifest = fs::read_to_string(&manifest_path).unwrap();
    manifest.push_str("share/runbook/linked/a.txt\n../elsewhere.txt\nshare/runbook/../../elsewhere.txt\n/etc/hostname-not-ours\n");
    fs::write(&manifest_path, manifest).unwrap();
    let o = install(&copy, &prefix, &["--from-built", "debug"]);
    assert_eq!(code(&o), 0, "{}{}", out(&o), err(&o));
    assert_eq!(files_below(&outside).len(), 3, "a file outside the prefix was removed");
    assert!(sandbox.join("elsewhere.txt").is_file());
}

// ---- helpers of the cases of several files ----

fn sandbox_of(root: &Path) -> (PathBuf, PathBuf) {
    let sandbox = root.parent().unwrap().to_path_buf();
    let prefix = sandbox.join("prefix");
    fs::create_dir_all(&prefix).unwrap();
    fs::create_dir_all(sandbox.join("home")).unwrap();
    (sandbox, prefix)
}

fn from_built(root: &Path, prefix: &Path) -> Output {
    install(&root.join("scripts/install.sh"), prefix, &["--from-built", "debug"])
}

fn head12(root: &Path) -> String {
    git_in(root, &["rev-parse", "HEAD"]).chars().take(12).collect()
}

fn executable(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn from_built_with(root: &Path, prefix: &Path, path: &str, vars: &[(&str, &Path)]) -> Output {
    let mut cmd = Command::new("sh");
    cmd.arg(root.join("scripts/install.sh")).arg("--prefix").arg(prefix).args(["--from-built", "debug"]).env_clear().envs(NO_SYSTEM_GIT).env("PATH", path).env("HOME", prefix.parent().unwrap().join("home")).stdin(Stdio::null());
    for (k, v) in vars {
        cmd.env(k, v);
    }
    cmd.output().unwrap()
}
