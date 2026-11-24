//! The trees a bug-fix run hands to a check command are git trees (docs/bugfix.md, "The trees a
//! check runs in"): `repro-run`, `tests-run` and `gate-run` run each command of checks.yaml in a copy
//! under `.sdc/bugfix/`, and the copy is a git repository with one commit of the copied files, made
//! with a fixed identity and none of the host's git configuration. The fix's diff, the artifacts and
//! their bytes stay as they were.
//!
//! The case's commands are the `bugfix-script` fixture's, each behind `checks/probe`, a POSIX sh
//! stand-in that writes what git says of its working directory into `PROBE_OUT` (outside the tree)
//! and then runs the command it wraps. No build pool: `slot` is taken off the `PATH` of the steps.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const PROBE: &str = r#"#!/bin/sh
# probe: what git says of this tree, into $PROBE_OUT/<n>.txt and <n>.files; then the wrapped command.
out=${PROBE_OUT:?}
n=$(( $(cat "$out/count" 2>/dev/null || echo 0) + 1 ))
echo "$n" > "$out/count"
up=$(dirname "$PWD")
g() { env -i PATH="$PATH" HOME=/nonexistent GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CEILING_DIRECTORIES="$up" git "$@"; }
seen=no
[ -e .git/probe-seen ] && seen=yes
{
  echo "dir=$PWD"
  echo "toplevel=$(g rev-parse --show-toplevel 2>&1)"
  echo "head=$(g rev-parse HEAD 2>&1)"
  echo "count=$(g rev-list --count HEAD 2>&1)"
  echo "status=$(g status --porcelain --ignored 2>&1 | tr '\n' ';')"
  echo "status-plain=$(g status --porcelain 2>&1 | tr '\n' ';')"
  echo "remotes=$(g remote 2>&1 | tr '\n' ';')"
  echo "who=$(g log -1 --format='%an <%ae> %at|%cn <%ce> %ct' 2>&1)"
  echo "branch=$(g symbolic-ref --short HEAD 2>&1)"
  echo "maintenance.auto=$(g config --local maintenance.auto 2>&1)"
  echo "gc.auto=$(g config --local gc.auto 2>&1)"
  echo "core.hooksPath=$(g config --local core.hooksPath 2>&1)"
  echo "seen=$seen"
} > "$out/$n.txt"
g ls-files > "$out/$n.files" 2>&1
[ -d .git ] && : > .git/probe-seen
if [ "${PROBE_SLOT:-}" = 1 ]; then
  mkdir -p .slot .git/info
  echo job > .slot/job
  echo '.slot/' >> .git/info/exclude
fi
exec "$@"
"#;

const CHECKS: &str = "format: sdc.checks/1
test: [./checks/probe, ./checks/run-all]
repro: [./checks/probe, ./checks/run-one, \"{file}\"]
gate:
- name: suite
  argv: [./checks/probe, ./checks/run-all]
test_file: '^tests/[a-z][a-z0-9_]*\\.test\\.sh$'
test_name: '^[a-z][a-z0-9_]*$'
env:
  LC_ALL: C
";

/// The identity and instant of every commit a run makes in a check tree.
const WHO: &str = "runbook <runbook@localhost> 946684800|runbook <runbook@localhost> 946684800";

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/sdc/fixtures/bugfix-script")
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dst = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &dst);
        } else {
            fs::copy(e.path(), dst).unwrap();
        }
    }
}

fn exec(p: &Path, text: &str) {
    fs::write(p, text).unwrap();
    fs::set_permissions(p, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
}

/// The PATH of the test with no `slot` on it, so no command becomes a pool job: a directory that
/// holds one is replaced by a directory under `root` of links to its other entries.
fn path_without_pool(root: &Path) -> String {
    let p = std::env::var_os("PATH").unwrap_or_default();
    let dirs: Vec<PathBuf> = std::env::split_paths(&p)
        .enumerate()
        .map(|(i, d)| {
            if !d.join("slot").exists() {
                return d;
            }
            let links = root.join(format!("path-{i}"));
            fs::create_dir_all(&links).unwrap();
            for e in fs::read_dir(&d).into_iter().flatten().flatten().filter(|e| e.file_name() != "slot") {
                let _ = std::os::unix::fs::symlink(e.path(), links.join(e.file_name()));
            }
            links
        })
        .collect();
    std::env::join_paths(dirs).unwrap().into_string().unwrap()
}

/// A case: a code tree (the fixture's repo/ with checks/probe), checks.yaml beside it, the probe's
/// output directory and the work directory, seeded and with the reproducing test installed.
struct Case {
    work: PathBuf,
    probe: PathBuf,
    path: String,
    env: Vec<(String, String)>,
}

impl Case {
    fn new(name: &str, env: &[(&str, &str)]) -> Case {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bugfix-git-trees").join(name);
        let _ = fs::remove_dir_all(&root);
        let src = root.join("src");
        copy_dir(&fixture().join("repo"), &src.join("repo"));
        exec(&src.join("repo/checks/probe"), PROBE);
        // A file of the code that its own .gitignore names is copied, so it is in the commit too.
        fs::write(src.join("repo/.gitignore"), "*.log\n").unwrap();
        fs::write(src.join("repo/kept.log"), "a file of the code\n").unwrap();
        fs::write(src.join("checks.yaml"), CHECKS).unwrap();
        fs::copy(fixture().join("bug.yaml"), src.join("bug.yaml")).unwrap();
        let (work, probe) = (root.join("work"), root.join("probe"));
        fs::create_dir_all(&work).unwrap();
        fs::create_dir_all(&probe).unwrap();
        let path = path_without_pool(&root);
        let mut c = Case { work, probe, path, env: Vec::new() };
        let (code, out) = c.bugfix(&["seed", "--from", src.to_str().unwrap(), "--checks", src.join("checks.yaml").to_str().unwrap()]);
        assert_eq!(code, 0, "{out}");
        copy_dir(&fixture().join("stub-repro"), &c.work.join("repro"));
        let (code, out) = c.bugfix(&["repro-install"]);
        assert_eq!(code, 0, "{out}");
        // `env` is the environment of the steps that run checks, not of the seed.
        c.env = env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        c
    }

    /// `sdc-bugfix <args>` in the work directory: (exit code, stdout and stderr).
    fn bugfix(&self, args: &[&str]) -> (i32, String) {
        let mut c = Command::new(env!("CARGO_BIN_EXE_sdc-bugfix"));
        for (k, _) in std::env::vars().filter(|(k, _)| k.starts_with("SDC_") || k.starts_with("GIT_")) {
            c.env_remove(k);
        }
        c.args(args).current_dir(&self.work).env("PATH", &self.path).env("PROBE_OUT", &self.probe);
        for (k, v) in &self.env {
            c.env(k, v);
        }
        let o = c.output().unwrap();
        (o.status.code().unwrap_or(-1), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
    }

    /// The fix of the fixture in impl/.
    fn fix(&self) {
        fs::copy(fixture().join("stub-fix/bin/total"), self.work.join("impl/bin/total")).unwrap();
    }

    /// What the probe wrote on its `n`-th call: its keys, and the files `git ls-files` listed.
    fn record(&self, n: usize) -> (BTreeMap<String, String>, Vec<String>) {
        let t = fs::read_to_string(self.probe.join(format!("{n}.txt"))).unwrap_or_else(|_| panic!("no probe call {n}"));
        let keys = t.lines().filter_map(|l| l.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let files = fs::read_to_string(self.probe.join(format!("{n}.files"))).unwrap();
        (keys, files.lines().map(str::to_string).collect())
    }

    fn calls(&self) -> usize {
        fs::read_to_string(self.probe.join("count")).map_or(0, |s| s.trim().parse().unwrap())
    }
}

/// Every file under `dir` but the relative paths in `skip`, relative, with its bytes.
fn snapshot(dir: &Path, skip: &[&str]) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, skip: &[&str], out: &mut BTreeMap<String, Vec<u8>>) {
        for e in fs::read_dir(dir).unwrap().flatten() {
            let rel = e.path().strip_prefix(root).unwrap().to_string_lossy().into_owned();
            if skip.contains(&rel.as_str()) {
                continue;
            }
            if e.file_type().unwrap().is_dir() {
                walk(root, &e.path(), skip, out);
            } else {
                out.insert(rel, fs::read(e.path()).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, skip, &mut out);
    out
}

/// The relative paths of the files under `dir`, sorted.
fn listing(dir: &Path) -> Vec<String> {
    snapshot(dir, &[]).into_keys().collect()
}

/// The artifacts of the case and the files beside them: everything in the work directory but the
/// copies the commands run in and their logs.
fn artifacts(work: &Path) -> BTreeMap<String, Vec<u8>> {
    snapshot(work, &[".sdc/bugfix"])
}

/// Every `.git` under `dir`, relative.
fn git_dirs(dir: &Path, root: &Path, out: &mut Vec<String>) {
    for e in fs::read_dir(dir).unwrap().flatten() {
        if e.file_type().unwrap().is_dir() {
            let rel = e.path().strip_prefix(root).unwrap().to_string_lossy().into_owned();
            if e.file_name() == ".git" {
                out.push(rel);
            } else {
                git_dirs(&e.path(), root, out);
            }
        }
    }
}

/// The probe's record `r` of a call in `copy` (`tree` or `base`) is a git tree with one commit of
/// exactly the files of `source`, with no remote and none of the machine's configuration.
fn assert_git_tree(c: &Case, n: usize, copy: &str, source: &Path) {
    let (r, files) = c.record(n);
    let dir = fs::canonicalize(c.work.join(".sdc/bugfix").join(copy)).unwrap();
    let at = format!("probe call {n} ({copy}): {r:?}");
    assert_eq!(r["dir"], dir.to_string_lossy(), "{at}");
    assert_eq!(r["toplevel"], dir.to_string_lossy(), "{at}");
    assert_eq!(r["count"], "1", "{at}");
    assert_eq!(r["status"], "", "{at}");
    assert_eq!(r["remotes"], "", "{at}");
    assert_eq!(r["branch"], "main", "{at}");
    assert_eq!(r["who"], WHO, "{at}");
    assert_eq!(r["maintenance.auto"], "false", "{at}");
    assert_eq!(r["gc.auto"], "0", "{at}");
    assert_eq!(r["core.hooksPath"], "/dev/null", "{at}");
    assert_eq!(r["head"].len(), 40, "{at}");
    assert_eq!(files, listing(source), "{at}: the commit holds exactly the copied files");
}

#[test]
fn every_tree_a_check_command_gets_is_a_git_tree_with_one_commit_of_the_copy() {
    let t = std::time::Instant::now();
    let c = Case::new("one-commit", &[]);
    // repro-run: the reproducing test in the copy of impl/, then the suite in the copy of the base.
    let (code, out) = c.bugfix(&["repro-run"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("repro: red:"), "{out}");
    assert_eq!(c.calls(), 2);
    assert_git_tree(&c, 1, "tree", &c.work.join("impl"));
    assert_git_tree(&c, 2, "base", &c.work.join("mainline"));

    // tests-run: two commands in one copy, made once (the second sees the mark the first left in
    // .git); gate-run: a copy made again, a repository of its own.
    c.fix();
    let (code, out) = c.bugfix(&["tests-run"]);
    assert_eq!(code, 0, "{out}");
    let (code, out) = c.bugfix(&["gate-run"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(c.calls(), 5);
    for n in 3..=5 {
        assert_git_tree(&c, n, "tree", &c.work.join("impl"));
    }
    let seen: Vec<String> = (3..=5).map(|n| c.record(n).0["seen"].clone()).collect();
    assert_eq!(seen, ["no", "yes", "no"], "made once per copy, and again for the next copy");
    // The same files give the same commit, whichever copy and step.
    let heads: Vec<String> = (3..=5).map(|n| c.record(n).0["head"].clone()).collect();
    assert!(heads.iter().all(|h| *h == heads[0]), "{heads:?}");
    assert_ne!(c.record(1).0["head"], heads[0], "another tree, another commit");
    eprintln!("bugfix-git-trees one-commit: {:.2} s", t.elapsed().as_secs_f64());
}

#[test]
fn the_hosts_git_identity_configuration_and_hooks_are_not_used() {
    let t = std::time::Instant::now();
    let plain = Case::new("plain", &[]);
    let (code, out) = plain.bugfix(&["repro-run"]);
    assert_eq!(code, 0, "{out}");

    // A hostile git configuration: another identity, signing through a program that fails, a hooks
    // directory and a template directory whose hooks fail and leave a mark, another default branch,
    // and the same through the variables git reads.
    let host = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bugfix-git-trees/hostile-config");
    let _ = fs::remove_dir_all(&host);
    let (hooks, template, marks) = (host.join("hooks"), host.join("template"), host.join("marks"));
    for d in [&hooks, &template.join("hooks"), &marks] {
        fs::create_dir_all(d).unwrap();
    }
    for name in ["pre-commit", "prepare-commit-msg", "commit-msg", "post-commit", "pre-auto-gc", "reference-transaction"] {
        let hook = format!("#!/bin/sh\n: > '{}/{name}'\nexit 1\n", marks.display());
        exec(&hooks.join(name), &hook);
        exec(&template.join("hooks").join(name), &hook);
    }
    let config = host.join("gitconfig");
    fs::write(&config, format!("[user]\n\tname = Host Person\n\temail = host@example.invalid\n[commit]\n\tgpgsign = true\n\
        [gpg]\n\tprogram = false\n[core]\n\thooksPath = {}\n[init]\n\tdefaultBranch = hostile\n\ttemplateDir = {}\n\
        [maintenance]\n\tauto = true\n[gc]\n\tauto = 1\n", hooks.display(), template.display())).unwrap();
    let cfg = config.to_str().unwrap();
    let tpl = template.to_str().unwrap();
    let hostile = Case::new("hostile", &[
        ("GIT_CONFIG_GLOBAL", cfg), ("GIT_CONFIG_SYSTEM", cfg), ("GIT_TEMPLATE_DIR", tpl),
        ("GIT_AUTHOR_NAME", "Env Author"), ("GIT_AUTHOR_EMAIL", "author@example.invalid"),
        ("GIT_COMMITTER_NAME", "Env Committer"), ("GIT_COMMITTER_EMAIL", "committer@example.invalid"),
        ("GIT_AUTHOR_DATE", "2020-02-02T02:02:02Z"), ("GIT_COMMITTER_DATE", "2020-02-02T02:02:02Z"),
        ("GIT_CONFIG_COUNT", "1"), ("GIT_CONFIG_KEY_0", "user.name"), ("GIT_CONFIG_VALUE_0", "Env Person"),
    ]);
    let (code, out) = hostile.bugfix(&["repro-run"]);
    assert_eq!(code, 0, "{out}");
    assert_git_tree(&hostile, 1, "tree", &hostile.work.join("impl"));
    assert_git_tree(&hostile, 2, "base", &hostile.work.join("mainline"));
    for n in 1..=2 {
        assert_eq!(hostile.record(n).0["head"], plain.record(n).0["head"], "probe call {n}: the commit does not depend on the host");
    }
    assert_eq!(listing(&marks), Vec::<String>::new(), "no hook of the host ran");
    eprintln!("bugfix-git-trees hostile: {:.2} s", t.elapsed().as_secs_f64());
}

#[test]
fn the_git_tree_never_enters_the_diff_an_artifact_or_their_bytes() {
    let t = std::time::Instant::now();
    let c = Case::new("no-diff", &[]);
    let before = artifacts(&c.work);
    let (code, out) = c.bugfix(&["repro-run"]);
    assert_eq!(code, 0, "{out}");
    assert!(c.work.join(".sdc/bugfix/tree/.git").is_dir() && c.work.join(".sdc/bugfix/base/.git").is_dir(), "the copies are git trees");
    assert_eq!(artifacts(&c.work), before, "repro-run changed no artifact");

    // The diff of a changed impl/ names the changed code file and nothing of a .git.
    c.fix();
    let fixed = artifacts(&c.work);
    let (code, out) = c.bugfix(&["repro-run"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("repro: not red: the code is changed, not only the test added: bin/total\n"), "{out}");
    for step in ["tests-run", "gate-run"] {
        let (code, out) = c.bugfix(&[step]);
        assert_eq!(code, 0, "{step}: {out}");
        assert!(c.work.join(".sdc/bugfix/tree/.git").is_dir(), "{step}");
        assert_eq!(artifacts(&c.work), fixed, "{step} changed no artifact");
    }
    let mut gits = Vec::new();
    git_dirs(&c.work, &c.work, &mut gits);
    gits.sort();
    assert_eq!(gits, [".sdc/bugfix/base/.git", ".sdc/bugfix/tree/.git"], "no .git outside the copies");
    eprintln!("bugfix-git-trees no-diff: {:.2} s", t.elapsed().as_secs_f64());
}

#[test]
fn a_slot_directory_a_check_command_writes_stays_out_of_the_diff() {
    let t = std::time::Instant::now();
    let c = Case::new("slot-dir", &[("PROBE_SLOT", "1")]);
    c.fix();
    let fixed = artifacts(&c.work);
    // tests-run: the first command writes .slot/ and excludes it, as the pool's client does.
    let (code, out) = c.bugfix(&["tests-run"]);
    assert_eq!(code, 0, "{out}");
    let (second, _) = c.record(2);
    assert_eq!(second["status-plain"], "", "{second:?}");
    assert_eq!(second["status"], "!! .slot/;", "{second:?}");
    assert!(c.work.join(".sdc/bugfix/tree/.slot/job").is_file());
    assert_eq!(artifacts(&c.work), fixed, "no .slot/ in an artifact");
    assert!(!c.work.join("impl/.slot").exists() && !c.work.join("mainline/.slot").exists());
    // The next step's copy is made again, without the .slot/ of the last one.
    let (code, out) = c.bugfix(&["gate-run"]);
    assert_eq!(code, 0, "{out}");
    assert_git_tree(&c, 3, "tree", &c.work.join("impl"));
    let (code, out) = c.bugfix(&["repro-run"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("the code is changed, not only the test added: bin/total\n"), "{out}");
    eprintln!("bugfix-git-trees slot-dir: {:.2} s", t.elapsed().as_secs_f64());
}
