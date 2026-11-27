//! Adversary cases against the first slice of story `a-delegation-changes-a-repository-on-its-own-branch`
//! (`runbook/src/handback.rs`; `docs/daemon-api.md`, "A delegation that changes a repository"). Each case drives the
//! daemon through the rig of `delegate_change.rs` (bare remote, stand-in `worktree`, `claude` and `git` on the daemon's
//! `PATH`, isolated XDG directories) and asserts what the document promises.

use super::delegate_change::{git, real_git, rig, Rig};
use super::gateway::write_exe;
use super::http::send;
use super::PRINCIPAL;
use serde_json::json;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

/// A stand-in `git` that holds a call of `verb` (`commit`, `push`) until `log/release-<verb>` exists (a forge that takes
/// a moment), leaving `log/<verb>-held` when it starts and `log/<verb>-done` when the real call is over.
fn held_git(log: &Path, verb: &str) -> String {
    format!(
        r#"#!/bin/sh
log='{log}'
held=''
for a in "$@"; do [ "$a" = {verb} ] && held=1; done
if [ -n "$held" ]; then
  : > "$log/{verb}-held"
  i=0
  while [ ! -e "$log/release-{verb}" ] && [ $i -lt 400 ]; do sleep 0.05; i=$((i + 1)); done
  '{git}' "$@"
  s=$?
  : > "$log/{verb}-done"
  exit $s
fi
exec '{git}' "$@"
"#,
        log = log.display(),
        git = real_git().display()
    )
}

fn cancel(r: &Rig, run: &str) -> u16 {
    send(r.l.addr, "POST", "/daemon/commands/cancel-run", &[("Content-Type", "application/json")], Some(json!({"run_id": run}).to_string().as_bytes())).status
}

/// docs/daemon-api.md: "A run that ends any other way (a failed step, a cancel, a cap, a runtime failure) commits and
/// pushes nothing". A cancel that comes while the daemon hands the change back, before its push (here: while its commit
/// is made), is accepted, and nothing is pushed.
#[test]
fn adversary_a_run_cancelled_during_its_hand_back_before_the_push_pushes_nothing() {
    let r = rig("adv-dc-cancel", "");
    write_exe(&r.h.log.join("bin/git"), &held_git(&r.h.log, "commit"));
    let run = r.accepted(json!({"request": "Add a file.", "model_route": "default", "repository": "demo"}));
    r.wait_for("commit-held");
    assert_eq!(cancel(&r, &run), 202, "the cancel was refused while the hand-back ran");
    let end = r.until_ended(&run);
    assert_eq!(end["run_state"], json!("Cancelled"), "{end}");
    std::fs::write(r.h.log.join("release-commit"), "").unwrap();
    r.wait_for("commit-done");
    // the hand-back asks whether the run is live right after its commit; give it the time of a push
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(r.pushed(&run), None, "a cancelled run pushed its branch; the view says: {}", r.result(&run));
    assert!(r.result(&run).get("branch").is_none(), "{}", r.result(&run));
}

/// A cancel accepted while the push itself runs cannot take the push back: the run event `runbook.pushing`, written
/// while the run was live, names the branch, and the view `delegation-result` of the cancelled run names it too.
#[test]
fn adversary_a_run_cancelled_during_its_push_names_the_branch_the_push_made() {
    let r = rig("adv-dc-cancel-push", "");
    write_exe(&r.h.log.join("bin/git"), &held_git(&r.h.log, "push"));
    let run = r.accepted(json!({"request": "Add a file.", "model_route": "default", "repository": "demo"}));
    r.wait_for("push-held");
    assert_eq!(cancel(&r, &run), 202, "the cancel was refused while the push ran");
    let end = r.until_ended(&run);
    assert_eq!(end["run_state"], json!("Cancelled"), "{end}");
    std::fs::write(r.h.log.join("release-push"), "").unwrap();
    r.wait_for("push-done");
    let pushed = r.pushed(&run).expect("the push that ran when the cancel came made no branch");
    let view = r.result(&run);
    assert_eq!((view["branch"].as_str(), view["commit"].as_str()), (Some(format!("runbook/{run}").as_str()), Some(pushed.as_str())), "{view}");
}

/// docs/daemon-api.md: "no `+`, no `--force`, so a branch that exists already is refused by the remote", and
/// `handback.rs`: "pushed as `refs/heads/runbook/<run-id>`, a new ref only". A branch of that name that exists already
/// at the run's base is moved by the push (a fast-forward needs no force), instead of the push being refused.
#[test]
fn adversary_a_branch_of_the_run_that_exists_already_at_the_base_is_not_moved_by_the_push() {
    let r = rig("adv-dc-ff", "");
    let run = r.accepted(json!({"request": "MODE-WAIT", "model_route": "default", "repository": "demo"}));
    r.wait_for("waiting");
    // the run's branch exists on the remote before the hand-back, at the run's base
    git(&r.origin, &["push", "-q", r.remote.to_str().unwrap(), &format!("{}:refs/heads/runbook/{run}", r.base)]);
    std::fs::write(r.h.log.join("release"), "").unwrap();
    let end = r.until_ended(&run);
    assert_eq!(end["outcome"], json!("delivered"), "{end}");
    assert_eq!(r.pushed(&run), Some(r.base.clone()), "the push moved a branch that existed already: {end}");
    assert!(end["handback_refused"].is_string(), "no refusal: {end}");
}

/// The commit of the change: "git runs without the operator's configuration"; `handback.rs`: "the operator's identity,
/// scripts and credentials are never used". The daemon's git calls still read the configuration of the operator's
/// checkout (`<path>/.git/config`, the common directory of the run's worktree): a `url.<x>.pushInsteadOf` there sends the
/// push to another repository than the entry's `remote`. With an `https://` remote the askpass answers the token to
/// whatever host the rewritten URL names.
#[test]
fn adversary_the_operators_checkout_config_does_not_redirect_the_push_away_from_the_entrys_remote() {
    let r = rig("adv-dc-insteadof", "");
    let decoy = r.h.log.parent().unwrap().join("decoy.git");
    std::fs::create_dir_all(&decoy).unwrap();
    git(&decoy, &["init", "-q", "--bare"]);
    git(&r.origin, &["config", &format!("url.file://{}.pushInsteadOf", decoy.display()), &format!("file://{}", r.remote.display())]);
    let run = r.accepted(json!({"request": "Add a file.", "model_route": "default", "repository": "demo"}));
    let end = r.until_ended(&run);
    assert_eq!(end["outcome"], json!("delivered"), "{end}");
    assert_eq!(git(&decoy, &["for-each-ref"]), "", "the push went where the operator's checkout config sent it, not to the entry's remote: {end}");
    assert!(r.pushed(&run).is_some(), "the entry's remote has no branch of the run: {end}");
}

/// `deny_paths`: "A path ending in `/` denies everything under it". An entry `src//` (or `a/./b`) was accepted by the
/// file's check (`plain_path` saw the components `src`) and then denied nothing, as git never names a path so. Such an
/// entry now refuses the daemon's start, exit 2, naming the key; `src/` starts it.
#[test]
fn adversary_a_deny_path_with_a_doubled_slash_refuses_the_start() {
    let remote = "https://forge.invalid/org/demo.git";
    let (control, err) = serve_with_remote("adv-dc-deny-plain", remote, "deny_paths = [\"src/\"]\n");
    assert_eq!(control, None, "a plain deny path refuses the start: {err}");
    for (tag, bad) in [("adv-dc-deny-slashes", "src//"), ("adv-dc-deny-dot", "a/./b")] {
        let (status, err) = serve_with_remote(tag, remote, &format!("deny_paths = [\"{bad}\"]\n"));
        assert_eq!(status, Some(2), "the daemon started with deny_paths = [{bad}], which denies nothing: {err}");
        assert!(err.contains("deny_paths"), "{err}");
    }
}

/// A stand-in `claude` that writes `src/a.txt` and a symbolic link `src/link` whose target is outside T, then replies.
const LINK_OUT_CLAUDE: &str = r#"#!/bin/bash
mkdir -p src && printf 'a change\n' > src/a.txt && ln -s ../../outside-of-the-tree/secret src/link
printf '%s\n' '{"type":"result","subtype":"success","is_error":false,"num_turns":1,"total_cost_usd":0,"duration_ms":1,"usage":{"input_tokens":0,"output_tokens":0},"result":"Added src/a.txt.","terminal_reason":"completed"}'
"#;

/// The story's design item 4: the hand-back "also refuses a link that leaves T". A change that adds a symbolic link
/// pointing out of T is committed and pushed.
#[test]
fn adversary_a_symbolic_link_that_leaves_the_tree_is_not_pushed() {
    let r = rig("adv-dc-link", "");
    write_exe(&r.h.log.join("bin/claude"), LINK_OUT_CLAUDE);
    let run = r.accepted(json!({"request": "Add a file.", "model_route": "default", "repository": "demo"}));
    let end = r.until_ended(&run);
    assert_eq!(end["outcome"], json!("delivered"), "{end}");
    assert_eq!(r.pushed(&run), None, "a link out of the tree was pushed: {end}");
    assert!(end["handback_refused"].as_str().is_some_and(|w| w.contains("src/link")), "{end}");
}

/// The exit status of `runbook serve` on a file holding one repository entry with `remote`, or none when it started:
/// it printed its listening line, which it prints after the file was read and accepted.
fn serve_with_remote(tag: &str, remote: &str, extra: &str) -> (Option<i32>, String) {
    let dir = super::state::scratch(tag);
    let config = dir.join("runbook.toml");
    std::fs::write(
        &config,
        format!("[repositories.demo]\npath = \"demo\"\nbase = \"main\"\nremote = \"{remote}\"\ndefault_branch = \"main\"\ncommit_name = \"bot\"\ncommit_email = \"bot@example.invalid\"\n{extra}"),
    )
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_runbook"))
        .arg("serve")
        .args(["--principal", PRINCIPAL, "--listen", "127.0.0.1:0", "--min-confinement", "local", "--default-confinement", "local"])
        .arg("--state-dir")
        .arg(dir.join("state"))
        .env_remove("RUNBOOK_TOKEN")
        .env("RUNBOOK_CONFIG", &config)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // the first line of its standard output, or nothing when it ended before it listened
    let mut line = String::new();
    let _ = std::io::BufRead::read_line(&mut std::io::BufReader::new(child.stdout.take().unwrap()), &mut line);
    if line.starts_with("runbook: listening on ") {
        let _ = child.kill();
        let _ = child.wait();
        return (None, String::new());
    }
    let s = child.wait().unwrap();
    let mut err = String::new();
    let _ = std::io::Read::read_to_string(child.stderr.as_mut().unwrap(), &mut err);
    assert!(line.is_empty(), "runbook serve printed {line:?} and did not listen: {err}");
    (s.code(), err)
}

/// docs/daemon-api.md: "a remote with a user or password ... refuses the start, exit 2". `https://bot:pw@host/...` is
/// refused; `https:///bot:pw@host/...` (a third slash) is not, and curl reads `bot:pw` from it as the user and password
/// of `host` (probe: `git ls-remote https:///bot:pw@127.0.0.1:1/x` connects to 127.0.0.1:1).
#[test]
fn adversary_a_remote_with_a_user_and_password_after_a_third_slash_refuses_the_start() {
    let (control, err) = serve_with_remote("adv-dc-userinfo-2", "https://bot:pw@forge.invalid/org/demo.git", "");
    assert_eq!(control, Some(2), "the plain form is refused: {err}");
    let (status, err) = serve_with_remote("adv-dc-userinfo-3", "https:///bot:pw@forge.invalid/org/demo.git", "");
    assert_eq!(status, Some(2), "the daemon started with a remote that holds a user and a password: {err}");
    assert!(err.contains("remote"), "{err}");
}
