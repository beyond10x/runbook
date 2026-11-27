//! A delegation that changes a repository on its own branch, first slice (story
//! `a-delegation-changes-a-repository-on-its-own-branch`, `## Decision`; `docs/daemon-api.md`, "A delegation that
//! changes a repository"): a delegation names a repository of `[repositories.<name>]` in `runbook.toml`, the daemon
//! makes a worktree T of it with the host's `worktree` command, the agent node of `delegation-change` runs with cwd T,
//! and after the outcome `delivered` the daemon commits T's change under the entry's identity and pushes the new ref
//! `refs/heads/runbook/<run-id>`, never forced, never a path of `deny_paths`.
//!
//! Everything is local: the entry's remote is a bare repository of the test (a `file://` URL), its `path` a checkout of
//! the test, `worktree` a stand-in in sh first on the daemon's `PATH` that records its arguments and makes a linked
//! worktree with `git worktree add` (the forge's runner has no `worktree` command, and the host's keeps its registry in
//! the user's state directory), `claude` a stand-in in bash that changes files of its working directory, and `git` a
//! stand-in that records each call of the daemon (its arguments and the variables that keep the operator's
//! configuration out) and runs the real one. The push token is a file with a dummy value. No model is called; the
//! gateway is a stand-in on a loopback port of the test that is only asked whether it is ready.

use super::default_confinement::{live_env, rows, Live};
use super::delegate::ROUTE;
use super::gateway::{gateway_table, home, repo, write_exe, Home};
use super::http::send;
use super::relay::{gateway_listing, Gateway};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 56\r\nConnection: close\r\n\r\n{\"id\":\"m\",\"usage\":{\"input_tokens\":11,\"output_tokens\":5}}";
/// The dummy push token: it must appear in no argv, log, file of the daemon's state or environment of the run.
const PUSH_TOKEN: &str = "push-token-6f1c0d9a2b7e44c1";
const BOT_NAME: &str = "runbook-bot";
const BOT_EMAIL: &str = "runbook-bot@example.invalid";
/// What the stand-in `claude` replies as its last message: the change's summary.
const SUMMARY: &str = "Added src/a.txt.";

/// The real `git`, which the stand-ins call by path.
pub(super) fn real_git() -> PathBuf {
    ["/usr/bin/git", "/bin/git", "/usr/local/bin/git"].iter().map(PathBuf::from).find(|p| p.is_file()).expect("no git on this host")
}

/// `git` of the test itself, with no configuration of the host.
pub(super) fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new(real_git())
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=test", "-c", "user.email=test@example.invalid", "-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null"])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("RUNBOOK_NO_CONFIG", "1")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(o.status.success(), "git {args:?} in {}: {}", dir.display(), String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_owned()
}

/// `git` of the test that may fail: its stdout when it succeeded.
pub(super) fn git_ok(dir: &Path, args: &[&str]) -> Option<String> {
    let o = Command::new(real_git()).arg("-C").arg(dir).args(args).env("RUNBOOK_NO_CONFIG", "1").env("GIT_CONFIG_GLOBAL", "/dev/null").env("GIT_CONFIG_NOSYSTEM", "1").output().unwrap();
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim().to_owned())
}

/// The stand-in `claude`: it records its working directory, its environment and its arguments, then does what the
/// request asks by a word of the prompt, and replies [`SUMMARY`] in a stream-json result event.
fn claude(log: &Path) -> String {
    format!(
        r#"#!/bin/bash
log='{log}'
prompt=''
prev=''
for a in "$@"; do [ "$prev" = -p ] && prompt=$a; prev=$a; done
pwd -P > "$log/claude-cwd"
env > "$log/claude-env"
printf '%s\n' "$@" > "$log/claude-argv"
case $prompt in
  *MODE-FAIL*) exit 8 ;;
  *MODE-WAIT*)
    mkdir -p src && printf 'a change\n' > src/a.txt
    : > "$log/waiting"
    i=0
    while [ ! -e "$log/release" ] && [ $i -lt 400 ]; do sleep 0.05; i=$((i + 1)); done ;;
  *MODE-CI*) mkdir -p .github/workflows src && printf 'on: push\n' > .github/workflows/x.yml && printf 'a change\n' > src/a.txt ;;
  *MODE-GITFILE*) printf 'gitdir: /elsewhere\n' > .git && mkdir -p src && printf 'a change\n' > src/a.txt ;;
  *MODE-FREE*) mkdir -p out && printf 'a free answer\n' > out/result.md ;;
  *) mkdir -p src && printf 'a change\n' > src/a.txt ;;
esac
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"num_turns":1,"total_cost_usd":0,"duration_ms":1,"usage":{{"input_tokens":0,"output_tokens":0}},"result":"{SUMMARY}","terminal_reason":"completed"}}'
"#,
        log = log.display()
    )
}

/// The stand-in `worktree`: each call is a line of `log/worktree`; `create` makes a detached linked worktree of `--repo`
/// at `--base` under `trees/<id>` with the real `git` (no hook of the repository runs) and prints its path.
fn worktree(log: &Path, trees: &Path, empty: &Path) -> String {
    format!(
        r#"#!/bin/sh
log='{log}'
printf '%s\n' "$*" >> "$log/worktree"
cmd=$1
shift
case $cmd in
  create)
    repo=.
    base=HEAD
    id=tree
    while [ $# -gt 0 ]; do
      case $1 in
        --repo) repo=$2; shift ;;
        --base) base=$2; shift ;;
        --id) id=$2; shift ;;
      esac
      shift
    done
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 '{git}' -c core.hooksPath='{empty}' -C "$repo" worktree add -q --detach '{trees}'/"$id" "$base" >> "$log/worktree.err" 2>&1 || exit 1
    printf '%s\n' '{trees}'/"$id" ;;
  *) exit 0 ;;
esac
"#,
        log = log.display(),
        trees = trees.display(),
        empty = empty.display(),
        git = real_git().display()
    )
}

/// The stand-in `git` of the daemon: one line per call with its arguments and the variables that keep the operator's
/// configuration and credentials out, then the real `git`.
pub(super) fn git_recorder(log: &Path) -> String {
    format!(
        r#"#!/bin/sh
{{ printf 'argv:'; for a in "$@"; do printf ' [%s]' "$a"; done; printf ' env: GIT_CONFIG_GLOBAL=%s GIT_CONFIG_NOSYSTEM=%s GIT_TERMINAL_PROMPT=%s GIT_ASKPASS=%s\n' "${{GIT_CONFIG_GLOBAL-}}" "${{GIT_CONFIG_NOSYSTEM-}}" "${{GIT_TERMINAL_PROMPT-}}" "${{GIT_ASKPASS-}}"; }} >> '{log}/git'
exec '{git}' "$@"
"#,
        log = log.display(),
        git = real_git().display()
    )
}

/// A hook that leaves a marker named after itself in `log`.
fn marker_hook(log: &Path) -> String {
    format!("#!/bin/sh\n: > '{}/hook-ran-'\"$(basename \"$0\")\"\n", log.display())
}

pub(super) struct Rig {
    pub(super) h: Home,
    pub(super) l: Live,
    _gw: Gateway,
    /// The entry's `path`: the operator's checkout, which the run never changes.
    pub(super) origin: PathBuf,
    /// The entry's remote, a bare repository.
    pub(super) remote: PathBuf,
    /// Where the stand-in `worktree` makes the trees.
    pub(super) trees: PathBuf,
    /// The commit of the entry's `base` when the test started.
    pub(super) base: String,
}

impl Rig {
    pub(super) fn log(&self, name: &str) -> String {
        std::fs::read_to_string(self.h.log.join(name)).unwrap_or_default()
    }

    pub(super) fn tree(&self, run: &str) -> PathBuf {
        self.trees.join(format!("runbook-{run}"))
    }

    /// The commit of `refs/heads/runbook/<run>` on the remote, if it has one.
    pub(super) fn pushed(&self, run: &str) -> Option<String> {
        git_ok(&self.remote, &["rev-parse", "--verify", "-q", &format!("refs/heads/runbook/{run}")])
    }

    pub(super) fn delegate(&self, body: Value) -> (u16, Value) {
        let r = send(self.l.addr, "POST", "/daemon/commands/delegate", &[("Content-Type", "application/json")], Some(body.to_string().as_bytes()));
        (r.status, r.json())
    }

    /// A delegation on the route `default` that the daemon accepts: its run id.
    pub(super) fn accepted(&self, body: Value) -> String {
        let (status, answer) = self.delegate(body);
        assert_eq!(status, 202, "{answer}");
        answer["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned()
    }

    pub(super) fn result(&self, run: &str) -> Value {
        rows(&self.l, &format!("/daemon/views/delegation-result?run_id={run}"))[0].clone()
    }

    pub(super) fn until_ended(&self, run: &str) -> Value {
        let limit = Instant::now() + Duration::from_secs(40);
        loop {
            let r = self.result(run);
            if ["Ended", "FailedToStart", "Cancelled"].iter().any(|s| r["run_state"] == *s) {
                return r;
            }
            assert!(Instant::now() < limit, "gave up waiting for the end of the delegation: {r}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    pub(super) fn events(&self, run: &str, kind: &str) -> Vec<Value> {
        rows(&self.l, &format!("/daemon/views/events?run_id={run}&after_position=0")).into_iter().filter(|e| e["kind"] == kind).map(|e| e["record"].clone()).collect()
    }

    pub(super) fn wait_for(&self, file: &str) {
        let limit = Instant::now() + Duration::from_secs(30);
        while !self.h.log.join(file).exists() {
            assert!(Instant::now() < limit, "gave up waiting for {file}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// The daemon of these cases: the Loom runtime at the level `local`, the route `default` on a stand-in gateway, the
/// repository entry `demo` (and `tables` after it), the stand-ins first on its `PATH`, and an operator's git
/// configuration (`GIT_CONFIG_GLOBAL`) with a hook directory, a signing rule and a credential helper the daemon must
/// not use. The entry's checkout holds hooks of its own that leave a marker when they run.
pub(super) fn rig(tag: &str, tables: &str) -> Rig {
    let h = home(tag);
    let base_dir = h.log.parent().unwrap().to_path_buf();
    let (origin, remote, trees, empty) = (base_dir.join("origin"), base_dir.join("remote.git"), base_dir.join("trees"), base_dir.join("empty-hooks"));
    for d in [&origin, &remote, &trees, &empty] {
        std::fs::create_dir_all(d).unwrap();
    }
    git(&remote, &["init", "-q", "--bare"]);
    git(&origin, &["init", "-q", "-b", "main"]);
    std::fs::write(origin.join("README.md"), "a repository of the test\n").unwrap();
    git(&origin, &["add", "README.md"]);
    git(&origin, &["commit", "-q", "-m", "first"]);
    let base = git(&origin, &["rev-parse", "HEAD"]);
    for hook in ["pre-commit", "post-commit", "commit-msg", "pre-push", "post-checkout", "reference-transaction"] {
        write_exe(&origin.join(".git/hooks").join(hook), &marker_hook(&h.log));
    }
    // the operator's own configuration: a hook directory, signed commits (no key: a commit that reads it fails) and a
    // credential helper, each of which leaves a marker or fails the hand-back when the daemon reads it
    let operator_hooks = base_dir.join("operator-hooks");
    std::fs::create_dir_all(&operator_hooks).unwrap();
    for hook in ["pre-commit", "post-commit", "pre-push"] {
        write_exe(&operator_hooks.join(hook), &marker_hook(&h.log));
    }
    let helper = base_dir.join("credential-helper");
    write_exe(&helper, &format!("#!/bin/sh\n: > '{}/credential-helper-ran'\n", h.log.display()));
    let operator = base_dir.join("operator.gitconfig");
    std::fs::write(&operator, format!("[user]\n\tname = Operator\n\temail = operator@example.invalid\n[core]\n\thooksPath = {}\n[commit]\n\tgpgsign = true\n[credential]\n\thelper = {}\n", operator_hooks.display(), helper.display())).unwrap();
    let secret = base_dir.join("push-secret");
    std::fs::create_dir_all(&secret).unwrap();
    let push_token = secret.join("push-token");
    std::fs::write(&push_token, format!("{PUSH_TOKEN}\n")).unwrap();
    std::fs::set_permissions(&push_token, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
    let bin = h.log.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    write_exe(&bin.join("claude"), &claude(&h.log));
    write_exe(&bin.join("worktree"), &worktree(&h.log, &trees, &empty));
    write_exe(&bin.join("git"), &git_recorder(&h.log));
    let gw = gateway_listing(OK, &["a-model"]);
    let entry = format!(
        "[repositories.demo]\npath = \"{}\"\nbase = \"main\"\nremote = \"file://{}\"\ndefault_branch = \"main\"\ncommit_name = \"{BOT_NAME}\"\ncommit_email = \"{BOT_EMAIL}\"\npush_token_file = \"{}\"\n",
        origin.display(),
        remote.display(),
        push_token.display()
    );
    let file = format!("{}{ROUTE}{entry}{tables}", gateway_table(&gw.base_url, &h.token_file));
    let path = format!("{}:{}:/usr/local/bin:/usr/bin:/bin", bin.display(), repo().join("driver/target/debug").display());
    let args = ["--runtime", "loom", "--runbook-home", h.home.to_str().unwrap(), "--min-confinement", "local", "--default-confinement", "local", "--max-wall-seconds", "3600", "--max-llm-calls", "100", "--cap-micro-usd", "1000000000"];
    let state_home = base_dir.join("xdg-state");
    let config_home = base_dir.join("xdg-config");
    let l = live_env(
        &format!("{tag}-d"),
        &args,
        Some(&file),
        &[
            ("XDG_RUNTIME_DIR", Some(h.xdg.to_str().unwrap())),
            ("XDG_STATE_HOME", Some(state_home.to_str().unwrap())),
            ("XDG_CONFIG_HOME", Some(config_home.to_str().unwrap())),
            ("GIT_CONFIG_GLOBAL", Some(operator.to_str().unwrap())),
            ("PATH", Some(&path)),
        ],
    );
    Rig { h, l, _gw: gw, origin, remote, trees, base }
}

/// The lines of the stand-in `worktree`'s log that name the run.
fn worktree_calls(r: &Rig, run: &str) -> Vec<String> {
    r.log("worktree").lines().filter(|l| l.contains(run)).map(str::to_owned).collect()
}

/// Case 1: the run's agent node works in T, and after `delivered` the remote holds `refs/heads/runbook/<run-id>` with
/// one commit by the entry's identity whose parent is the entry's base and whose change is the file the agent wrote;
/// the event `runbook.handed-back` and the view `delegation-result` carry the branch and the commit. T is finished
/// after the push. The operator's checkout is unchanged, and T's path is in no view.
#[test]
fn a_delegation_naming_a_listed_repository_pushes_its_own_branch_with_one_commit_by_the_entrys_identity() {
    let started = Instant::now();
    let r = rig("dlg-change", "");
    let (status, body) = r.delegate(json!({"request": "Add a file.", "model_route": "default", "repository": "demo"}));
    assert_eq!(status, 202, "{body}");
    assert_eq!((body["published"][1]["payload"]["path"].clone(), body["published"][1]["payload"]["protocol"].clone()), (json!("change"), json!("delegation-change")), "{body}");
    let run = body["published"][0]["payload"]["run_id"].as_str().unwrap().to_owned();
    let end = r.until_ended(&run);
    assert_eq!((end["run_state"].clone(), end["outcome"].clone()), (json!("Ended"), json!("delivered")), "{end}");
    let branch = format!("runbook/{run}");
    let commit = r.pushed(&run).unwrap_or_else(|| panic!("the remote has no {branch}: git log {}", r.log("git")));
    assert_eq!((end["repository"].clone(), end["branch"].clone(), end["commit"].clone()), (json!("demo"), json!(branch), json!(commit)), "{end}");
    assert_eq!(end["handback_refused"], Value::Null, "{end}");
    assert_eq!(end["result_text"], json!(format!("{SUMMARY}\n")), "{end}");
    super::openapi::check_row("DelegationResult", &end);
    let handed = r.events(&run, "runbook.handed-back");
    assert_eq!(handed.len(), 1, "{handed:?}");
    assert_eq!((handed[0]["branch"].clone(), handed[0]["commit"].clone(), handed[0]["base"].clone()), (json!(branch), json!(commit), json!(r.base)), "{handed:?}");
    let delegated = r.events(&run, "runbook.delegated");
    assert_eq!((delegated[0]["repository"].clone(), delegated[0]["base"].clone()), (json!("demo"), json!(r.base)), "{delegated:?}");
    let pushing = r.events(&run, "runbook.pushing");
    assert_eq!((pushing.len(), pushing[0]["branch"].clone(), pushing[0]["commit"].clone()), (1, json!(branch), json!(commit)), "{pushing:?}");
    // one push, of the commit to the run's new ref, never forced (case 5)
    let calls = r.log("git");
    let pushes: Vec<&str> = calls.lines().filter(|l| l.contains("[push]")).collect();
    assert_eq!(pushes.len(), 1, "{pushes:?}");
    assert!(!pushes[0].contains("--force") && !pushes[0].contains("[+") && pushes[0].contains(&format!("[{commit}:refs/heads/{branch}]")), "{pushes:?}");
    // one commit on the base, by the entry's identity, whose change is the agent's file
    let remote = &r.remote;
    assert_eq!(git(remote, &["rev-list", "--count", &format!("{}..{commit}", r.base)]), "1");
    assert_eq!(git(remote, &["rev-parse", &format!("{commit}^")]), r.base);
    assert_eq!(git(remote, &["log", "-1", "--format=%an|%ae|%cn|%ce", &commit]), format!("{BOT_NAME}|{BOT_EMAIL}|{BOT_NAME}|{BOT_EMAIL}"));
    assert_eq!(git(remote, &["diff-tree", "--no-commit-id", "--name-only", "-r", &commit]), "src/a.txt");
    // the agent ran in T
    let tree = r.tree(&run);
    assert_eq!(r.log("claude-cwd").trim(), std::fs::canonicalize(&tree).unwrap().display().to_string(), "the agent node did not run in the run's worktree");
    // the stand-in worktree saw create with the run's id and the recorded base, then finish and gc after the push
    let calls = worktree_calls(&r, &run);
    assert!(calls[0].starts_with("create ") && calls[0].contains(&format!("--id runbook-{run}")) && calls[0].contains(&format!("--base {}", r.base)), "{calls:?}");
    assert!(calls.iter().any(|c| c == &format!("finish runbook-{run}")), "{calls:?}");
    assert!(calls.iter().any(|c| c.starts_with("gc ") && c.contains(&format!("runbook-{run}"))), "{calls:?}");
    // the operator's checkout is unchanged
    assert_eq!(git(&r.origin, &["rev-parse", "HEAD"]), r.base);
    assert_eq!(git(&r.origin, &["status", "--porcelain"]), "");
    // T's path is in no view and no event
    let tree_text = tree.display().to_string();
    for row in [end.clone(), json!(rows(&r.l, &format!("/daemon/views/run?run_id={run}"))), json!(rows(&r.l, &format!("/daemon/views/events?run_id={run}&after_position=0")))] {
        assert!(!row.to_string().contains(&tree_text), "a view names the worktree's path: {row}");
    }
    // the push token is in no argv, no log, no file of the daemon's state and no environment of the run's model process
    for dir in [r.l.dir.clone(), r.h.log.clone(), r.trees.clone()] {
        assert_eq!(super::gateway::holders(&dir, PUSH_TOKEN), Vec::<PathBuf>::new(), "the push token is in a file under {}", dir.display());
    }
    assert!(!r.log("claude-env").contains("push-token"), "the model's process got the token file");
    assert!(started.elapsed() < Duration::from_secs(5), "the case took {:?}", started.elapsed());
}

/// Case 2: a repository that `runbook.toml` does not list, or a path in place of a name, is refused at the submit with
/// the refusal `repository-refused`; no run exists and `worktree` is never called. A delegation without `repository`
/// runs as before and calls no `worktree` either.
#[test]
fn a_repository_that_is_not_listed_or_a_path_is_refused_at_the_submit() {
    let r = rig("dlg-change-refused", "");
    for name in ["other", "/tmp/demo", "../origin", "demo/..", ""] {
        let (status, body) = r.delegate(json!({"request": "Add a file.", "model_route": "default", "repository": name}));
        assert_eq!((status, body["outcome"].clone()), (422, json!("repository-refused")), "{name:?}: {body}");
        assert_eq!(body["error"], json!("sdc.daemon.RepositoryRefused"), "{body}");
        super::openapi::check_response("delegate", "repository-refused", &body);
    }
    // a protocol beside a repository is refused the same way: the change runs its own protocol
    let (status, body) = r.delegate(json!({"request": "Add a file.", "model_route": "default", "repository": "demo", "protocol": "delegation-free"}));
    assert_eq!((status, body["outcome"].clone()), (422, json!("repository-refused")), "{body}");
    assert_eq!(rows(&r.l, "/daemon/views/runs").len(), 0, "a refused delegation left a run");
    let free = r.accepted(json!({"request": "MODE-FREE", "model_route": "default"}));
    let end = r.until_ended(&free);
    assert_eq!((end["outcome"].clone(), end["path"].clone(), end["branch"].clone()), (json!("delivered"), json!("free"), Value::Null), "{end}");
    assert_eq!(r.log("worktree"), "", "worktree was called");
}

/// Case 3: a change under `.github/` (a default of `deny_paths`) is not pushed: the run delivers, its hand-back
/// records the refusal naming the path, the remote holds no ref of the run, and T is kept.
#[test]
fn a_change_of_a_denied_path_is_not_pushed_and_the_hand_back_records_the_refusal() {
    let r = rig("dlg-change-deny", "");
    let run = r.accepted(json!({"request": "MODE-CI", "model_route": "default", "repository": "demo"}));
    let end = r.until_ended(&run);
    assert_eq!(end["outcome"], json!("delivered"), "{end}");
    let refused = end["handback_refused"].as_str().unwrap_or_else(|| panic!("no refusal: {end}"));
    assert!(refused.contains(".github/workflows/x.yml"), "{refused}");
    assert_eq!((end["branch"].clone(), end["commit"].clone()), (Value::Null, Value::Null), "{end}");
    assert_eq!(r.events(&run, "runbook.hand-back-refused").len(), 1);
    assert_eq!(git(&r.remote, &["for-each-ref"]), "", "the remote holds a ref");
    assert!(r.tree(&run).join(".github/workflows/x.yml").is_file(), "T was not kept");
    assert!(!worktree_calls(&r, &run).iter().any(|c| c.starts_with("finish")), "a refused hand-back finished T");
}

/// Case 4: a run whose agent fails, a run cancelled while its agent works and a run stopped by its wall cap push
/// nothing; their T is kept and not finished.
#[test]
fn a_run_that_fails_is_cancelled_or_stops_on_a_cap_pushes_nothing_and_keeps_its_tree() {
    let r = rig("dlg-change-ends", "");
    let failed = r.accepted(json!({"request": "MODE-FAIL", "model_route": "default", "repository": "demo"}));
    let end = r.until_ended(&failed);
    assert_ne!(end["outcome"], json!("delivered"), "{end}");
    let cancelled = r.accepted(json!({"request": "MODE-WAIT", "model_route": "default", "repository": "demo"}));
    r.wait_for("waiting");
    let c = send(r.l.addr, "POST", "/daemon/commands/cancel-run", &[("Content-Type", "application/json")], Some(json!({"run_id": cancelled}).to_string().as_bytes()));
    assert_eq!(c.status, 202, "{}", c.text());
    assert_eq!(r.until_ended(&cancelled)["run_state"], json!("Cancelled"));
    std::fs::remove_file(r.h.log.join("waiting")).unwrap();
    let capped = r.accepted(json!({"request": "MODE-WAIT", "model_route": "default", "repository": "demo", "settings": {"max_wall_seconds": "1"}}));
    let end = r.until_ended(&capped);
    // The wall cap ends the agent node's program; the run then ends `runtime-failure` (the killed call left no cost record),
    // as the free run's does on the same cap today, not `stopped: wall time`: either way it did not deliver.
    assert_eq!(end["run_state"], json!("Ended"), "{end}");
    assert_ne!(end["outcome"], json!("delivered"), "{end}");
    for run in [&failed, &cancelled, &capped] {
        assert_eq!(r.pushed(run), None, "{run} pushed");
        assert!(r.tree(run).is_dir(), "the tree of {run} was not kept");
        assert!(!worktree_calls(&r, run).iter().any(|c| c.starts_with("finish") || c.starts_with("gc")), "{run}: {:?}", worktree_calls(&r, run));
        assert_eq!(r.events(run, "runbook.handed-back").len(), 0);
    }
    assert_eq!(git(&r.remote, &["for-each-ref"]), "");
}

/// Case 5: the push never forces: when `refs/heads/runbook/<run-id>` exists on the remote before the hand-back, the
/// hand-back asks the remote first and pushes nothing, records it, and the remote's ref is the one it had. (The push of
/// case 1 names no `--force` and no `+`.)
#[test]
fn the_push_never_forces_an_existing_branch_of_the_run() {
    let r = rig("dlg-change-noforce", "");
    let run = r.accepted(json!({"request": "MODE-WAIT", "model_route": "default", "repository": "demo"}));
    r.wait_for("waiting");
    // someone else's commit on the run's branch, made while the agent works
    let other = r.h.log.parent().unwrap().join("other");
    git(r.h.log.parent().unwrap(), &["clone", "-q", r.remote.to_str().unwrap(), other.to_str().unwrap()]);
    std::fs::write(other.join("theirs.txt"), "theirs\n").unwrap();
    git(&other, &["add", "theirs.txt"]);
    git(&other, &["commit", "-q", "-m", "theirs"]);
    git(&other, &["push", "-q", r.remote.to_str().unwrap(), &format!("HEAD:refs/heads/runbook/{run}")]);
    let theirs = git(&other, &["rev-parse", "HEAD"]);
    std::fs::write(r.h.log.join("release"), "").unwrap();
    let end = r.until_ended(&run);
    assert_eq!(end["outcome"], json!("delivered"), "{end}");
    let refused = end["handback_refused"].as_str().unwrap_or_else(|| panic!("no refusal: {end}"));
    assert!(refused.starts_with("push") && refused.contains("exists already"), "{refused}");
    assert_eq!(r.pushed(&run), Some(theirs), "the remote's branch was moved");
    let calls = r.log("git");
    assert!(calls.lines().any(|l| l.contains("[ls-remote]") && l.contains(&format!("[refs/heads/runbook/{run}]"))), "{calls}");
    let pushes: Vec<&str> = calls.lines().filter(|l| l.contains("[push]")).collect();
    assert!(pushes.is_empty(), "a push over a branch that exists: {pushes:?}");
}

/// Case 6: the daemon's git calls use none of the operator's configuration, hooks or credential helpers: each runs with
/// `GIT_CONFIG_GLOBAL=/dev/null` and `GIT_CONFIG_NOSYSTEM=1` and `core.hooksPath=/dev/null`, so the operator's signing rule
/// does not fail the commit and no hook of the repository or of the operator leaves its marker.
#[test]
fn the_operators_git_configuration_hooks_and_credential_helpers_are_not_used() {
    let r = rig("dlg-change-hooks", "");
    let run = r.accepted(json!({"request": "Add a file.", "model_route": "default", "repository": "demo"}));
    let end = r.until_ended(&run);
    assert!(r.pushed(&run).is_some(), "nothing was pushed: {end}");
    let calls: Vec<String> = r.log("git").lines().map(str::to_owned).collect();
    assert!(calls.iter().any(|c| c.contains("[commit]")), "{calls:?}");
    for c in &calls {
        assert!(c.contains("GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 GIT_TERMINAL_PROMPT=0"), "a git call of the daemon reads the operator's configuration: {c}");
        assert!(c.contains("[core.hooksPath="), "a git call of the daemon runs hooks: {c}");
    }
    let markers: Vec<String> = super::gateway::entries(&r.h.log).into_iter().filter(|n| n.starts_with("hook-ran-") || n == "credential-helper-ran").collect();
    assert_eq!(markers, Vec::<String>::new(), "a hook or helper of the operator ran");
}

/// Case 7: T's `.git` file changed by the model gets no push: the hand-back is refused by name and the remote holds
/// no ref of the run.
#[test]
fn a_git_file_changed_by_the_model_refuses_the_hand_back() {
    let r = rig("dlg-change-gitfile", "");
    let run = r.accepted(json!({"request": "MODE-GITFILE", "model_route": "default", "repository": "demo"}));
    let end = r.until_ended(&run);
    assert_eq!(end["outcome"], json!("delivered"), "{end}");
    let refused = end["handback_refused"].as_str().unwrap_or_else(|| panic!("no refusal: {end}"));
    assert!(refused.contains(".git"), "{refused}");
    assert_eq!(git(&r.remote, &["for-each-ref"]), "", "the remote holds a ref");
    assert!(!r.log("git").contains("[push]"), "the daemon pushed");
}

/// The askpass of the push (design item 4): git asks the `runbook` binary itself, with `RUNBOOK_ASKPASS_FILE` naming the
/// entry's token file, and gets the token for a password and the bot user for a user name. The token is read from the
/// file then, never passed in an argument or a variable. It answers only a prompt for the host of the entry's remote
/// (`RUNBOOK_ASKPASS_ORIGIN`): a push that a URL rewrite or a redirect sent elsewhere gets no token.
#[test]
fn the_runbook_binary_answers_gits_askpass_from_the_token_file() {
    let dir = super::state::scratch("askpass");
    let file = dir.join("push-token");
    std::fs::write(&file, format!("{PUSH_TOKEN}\n")).unwrap();
    std::fs::set_permissions(&file, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
    let ask = |prompt: &str| {
        let o = Command::new(env!("CARGO_BIN_EXE_runbook")).arg(prompt).env("RUNBOOK_ASKPASS_FILE", &file).env("RUNBOOK_ASKPASS_ORIGIN", "https://forge.invalid").env("RUNBOOK_NO_CONFIG", "1").output().unwrap();
        (o.status.code(), String::from_utf8_lossy(&o.stdout).into_owned())
    };
    assert_eq!(ask("Password for 'https://x-access-token@forge.invalid': "), (Some(0), format!("{PUSH_TOKEN}\n")));
    assert_eq!(ask("Username for 'https://forge.invalid': "), (Some(0), "x-access-token\n".to_owned()));
    assert_eq!(ask("Password for 'https://x-access-token@elsewhere.invalid': "), (Some(1), String::new()), "a prompt for another host");
    let unreadable = Command::new(env!("CARGO_BIN_EXE_runbook")).arg("Password for 'https://forge.invalid': ").env("RUNBOOK_NO_CONFIG", "1").env("RUNBOOK_ASKPASS_ORIGIN", "https://forge.invalid").env("RUNBOOK_ASKPASS_FILE", dir.join("none")).output().unwrap();
    assert_ne!(unreadable.status.code(), Some(0));
    assert!(unreadable.stdout.is_empty());
}
