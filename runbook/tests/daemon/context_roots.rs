//! The daemon's context roots (story `context-roots-are-keys-of-runbook-toml`; `docs/daemon-api.md`, "Context roots"):
//! `context_roots` of `runbook.toml` and the option `--context-root` add up; a root that is missing is created with mode
//! 0700, its missing parents too; an existing root is never changed: one with group or other write bits is refused, one
//! wider than 0700 without them is named in the start log. A refusal names the key or the option and the path, and exits
//! 2. A relative root of the file is taken against the file's directory.
//!
//! The submit cases run the stub runtime (a submit's `context_path` meets the same check as a delegation's `workspace`);
//! the first case runs a delegation on the Loom runtime against a stand-in gateway.

use super::default_confinement::{live, post, Live};
use super::delegate::{daemon_rooted, delegate, run_of, until_ended, OK, ROUTE};
use super::gateway::{claude_cli, model_table};
use super::http::exits_within;
use super::relay::gateway_listing;
use super::state::scratch;
use super::PRINCIPAL;
use runbook::serve::STUB_DECISION;
use serde_json::json;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const BIN: &str = env!("CARGO_BIN_EXE_runbook");

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o7777
}

fn set_mode(path: &Path, mode: u32) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

/// A submit of the stub's preset with `context` as its context path: the status and the outcome.
fn submit_under(l: &Live, context: &Path) -> (u16, String) {
    let (status, body) = post(l, &json!({"runbook_id": STUB_DECISION, "intake": "x", "confinement": "local", "context_path": context.to_str().unwrap()}));
    (status, body["outcome"].as_str().unwrap_or_default().to_owned())
}

/// What the daemon wrote on standard error until it was stopped.
fn stop_and_read_stderr(mut l: Live) -> String {
    let _ = l.child.kill();
    let _ = l.child.wait();
    let mut err = String::new();
    l.child.stderr.take().unwrap().read_to_string(&mut err).unwrap();
    err
}

/// `runbook serve` with `file` as its `runbook.toml` at `<dir>/conf/runbook.toml`, started in `<dir>/elsewhere`, that is
/// expected to stop at its start: the exit code and standard error.
fn refused_start(dir: &Path, file: &str, args: &[&str]) -> (Option<i32>, String) {
    std::fs::create_dir_all(dir.join("conf")).unwrap();
    std::fs::create_dir_all(dir.join("elsewhere")).unwrap();
    let config = dir.join("conf/runbook.toml");
    std::fs::write(&config, file).unwrap();
    let mut cmd = Command::new(BIN);
    cmd.current_dir(dir.join("elsewhere")).arg("serve").args(["--listen", "127.0.0.1:0"]).arg("--state-dir").arg(dir.join("state")).args(args);
    cmd.env_remove("RUNBOOK_TOKEN").env("RUNBOOK_CONFIG", &config).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let (code, out, err) = exits_within(cmd, Duration::from_secs(20));
    assert!(!out.contains("listening"), "the daemon listened: {out}");
    (code, err)
}

fn toml_list(paths: &[&Path]) -> String {
    let items: Vec<String> = paths.iter().map(|p| format!("\"{}\"", p.display())).collect();
    format!("context_roots = [{}]\n", items.join(", "))
}

/// Acceptance 1: `context_roots = ["ctx"]` and no `ctx`: the daemon starts, `ctx` exists with mode 0700 beside the file,
/// and a delegation whose `workspace` lies under it is taken and runs; one outside it is refused.
#[test]
fn a_missing_root_of_runbook_toml_is_created_0700_and_a_delegation_under_it_is_taken() {
    let gw = gateway_listing(OK, &["a-model"]);
    let tables = format!("{}{ROUTE}", model_table(Some(&claude_cli())));
    let (h, l, roots) = daemon_rooted("ctxroot-dlg", &gw.base_url, "context_roots = [\"ctx\"]\n", &tables, "100", false);
    let ctx = l.dir.join("ctx");
    assert!(ctx.is_dir(), "the root of the file was not created beside the file");
    assert_eq!(mode(&ctx), 0o700);
    std::fs::create_dir_all(ctx.join("project")).unwrap();
    std::fs::write(ctx.join("project/notes.txt"), "a file of the workspace\n").unwrap();
    let (status, body, _) = delegate(&l, &json!({"request": "green or black", "model_route": "default", "protocol": "answer-demo", "workspace": ctx.join("project").to_str().unwrap()}));
    assert_eq!(status, 202, "{body}");
    let end = until_ended(&l, &run_of(&body));
    assert_eq!((end["run_state"].clone(), end["outcome"].clone()), (json!("Ended"), json!("answered")), "{end}");
    // the option was not given: the helper's own root is no root of this daemon
    let (status, body, _) = delegate(&l, &json!({"request": "x", "model_route": "default", "protocol": "answer-demo", "workspace": roots.join("project").to_str().unwrap()}));
    assert_eq!((status, body["outcome"].clone()), (422, json!("context-outside-roots")), "{body}");
    drop(h);
}

/// Acceptance 2: the file's roots and the option's add up: a path under either is taken, a path under neither is refused
/// `context-outside-roots`; the start log names each root with its source.
#[test]
fn the_roots_of_the_file_and_of_the_option_add_up() {
    let mine = scratch("ctxroot-both-files");
    let (from_file, from_option, neither) = (mine.join("from-file"), mine.join("from-option"), mine.join("neither"));
    for d in [&from_file, &from_option, &neither] {
        std::fs::create_dir_all(d).unwrap();
    }
    let l = live("ctxroot-both", &["--min-confinement", "local", "--context-root", from_option.to_str().unwrap()], Some(&toml_list(&[&from_file])));
    assert_eq!(submit_under(&l, &from_file), (202, "submitted".to_owned()));
    assert_eq!(submit_under(&l, &from_option), (202, "submitted".to_owned()));
    assert_eq!(submit_under(&l, &neither), (422, "context-outside-roots".to_owned()));
    let err = stop_and_read_stderr(l);
    assert!(err.contains(&format!("context root {} (context_roots of runbook.toml)", from_file.display())), "{err}");
    assert!(err.contains(&format!("context root {} (--context-root)", from_option.display())), "{err}");
}

/// The option alone works as before: a path under its root is taken, one outside it is refused.
#[test]
fn a_context_root_option_alone_works_as_before() {
    let mine = scratch("ctxroot-option-files");
    let (root, outside) = (mine.join("root"), mine.join("outside"));
    std::fs::create_dir_all(root.join("project")).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    let l = live("ctxroot-option", &["--min-confinement", "local", "--context-root", root.to_str().unwrap()], None);
    assert_eq!(submit_under(&l, &root.join("project")), (202, "submitted".to_owned()));
    assert_eq!(submit_under(&l, &outside), (422, "context-outside-roots".to_owned()));
}

/// A relative root of the file is taken against the file's directory, never the working directory; its missing parents
/// are created 0700 with it; `~` is no home.
#[test]
fn a_relative_root_is_taken_against_the_files_directory_and_its_missing_parents_are_created_0700() {
    let l = live("ctxroot-rel", &["--min-confinement", "local"], Some("context_roots = [\"g5-rel-deep/ctx\", \"~/g5-tilde\"]\n"));
    let (deep, ctx) = (l.dir.join("g5-rel-deep"), l.dir.join("g5-rel-deep/ctx"));
    assert_eq!((mode(&deep), mode(&ctx)), (0o700, 0o700));
    assert!(l.dir.join("~/g5-tilde").is_dir(), "`~` is a directory name beside the file");
    let cwd = std::env::current_dir().unwrap();
    assert!(!cwd.join("g5-rel-deep").exists() && !cwd.join("~").exists(), "a root was made in the working directory");
    std::fs::create_dir_all(ctx.join("project")).unwrap();
    assert_eq!(submit_under(&l, &ctx.join("project")), (202, "submitted".to_owned()));
    let err = stop_and_read_stderr(l);
    assert!(err.contains(&format!("context root {} (context_roots of runbook.toml): created with mode 0700", ctx.display())), "{err}");
}

/// An existing root is never changed: one wider than 0700 without group or other write bits starts the daemon and its
/// mode is named in the start log; one with them stops the start, naming the mode. Neither mode is changed.
#[test]
fn an_existing_root_keeps_its_mode_and_one_writable_by_group_or_others_stops_the_start() {
    let mine = scratch("ctxroot-mode-files");
    let (open, shared) = (mine.join("open"), mine.join("shared"));
    std::fs::create_dir_all(&open).unwrap();
    std::fs::create_dir_all(&shared).unwrap();
    set_mode(&open, 0o755);
    set_mode(&shared, 0o777);
    let l = live("ctxroot-mode-open", &["--min-confinement", "local"], Some(&toml_list(&[&open])));
    assert_eq!(submit_under(&l, &open), (202, "submitted".to_owned()));
    let err = stop_and_read_stderr(l);
    assert!(err.contains(&format!("context root {} (context_roots of runbook.toml) has mode 0755, wider than 0700: it is not changed", open.display())), "{err}");
    assert_eq!(mode(&open), 0o755);
    let dir = scratch("ctxroot-mode-shared");
    let (code, err) = refused_start(&dir, &toml_list(&[&shared]), &["--principal", PRINCIPAL]);
    assert_eq!(code, Some(2), "{err}");
    assert!(err.contains(&format!("context_roots {}: is writable by group or others (mode 0777)", shared.display())), "{err}");
    assert_eq!(mode(&shared), 0o777, "the mode of an existing root was changed");
    assert!(!dir.join("state").exists(), "the daemon wrote its state before it refused the root");
    let (code, err) = refused_start(&dir, "", &["--principal", PRINCIPAL, "--context-root", shared.to_str().unwrap()]);
    assert_eq!(code, Some(2), "{err}");
    assert!(err.contains(&format!("--context-root {}: is writable by group or others (mode 0777)", shared.display())), "{err}");
}

/// A root that is a file stops the start, naming the key and the path; so does one under a file, with the OS error, and
/// one that cannot be created. The option's root is held to the same.
#[test]
fn a_root_that_is_a_file_or_cannot_be_created_stops_the_start_naming_the_key_and_the_path() {
    let mine = scratch("ctxroot-file-files");
    let file = mine.join("a-file");
    std::fs::write(&file, "not a directory\n").unwrap();
    let dir = scratch("ctxroot-file");
    let (code, err) = refused_start(&dir, &toml_list(&[&file]), &["--principal", PRINCIPAL]);
    assert_eq!(code, Some(2), "{err}");
    assert!(err.contains(&format!("context_roots {}: is not a directory", file.display())), "{err}");
    let (code, err) = refused_start(&dir, "", &["--principal", PRINCIPAL, "--context-root", file.to_str().unwrap()]);
    assert_eq!(code, Some(2), "{err}");
    assert!(err.contains(&format!("--context-root {}: is not a directory", file.display())), "{err}");
    let under = file.join("ctx");
    let (code, err) = refused_start(&dir, &toml_list(&[&under]), &["--principal", PRINCIPAL]);
    assert_eq!(code, Some(2), "{err}");
    assert!(err.contains(&format!("context_roots {}: ", under.display())) && err.contains("Not a directory"), "{err}");
    // a parent the daemon may not write into: refused with the OS error (a user that writes anywhere has nothing to show)
    let locked = mine.join("locked");
    std::fs::create_dir_all(&locked).unwrap();
    set_mode(&locked, 0o500);
    let writes_anywhere = std::fs::create_dir(locked.join("probe")).is_ok();
    if !writes_anywhere {
        let blocked = locked.join("ctx");
        let (code, err) = refused_start(&dir, &toml_list(&[&blocked]), &["--principal", PRINCIPAL]);
        assert_eq!(code, Some(2), "{err}");
        assert!(err.contains(&format!("context_roots {}: cannot be created: ", blocked.display())) && err.contains("Permission denied"), "{err}");
    }
    set_mode(&locked, 0o700);
    assert!(!dir.join("state").exists(), "the daemon wrote its state before it refused a root");
}

/// An unknown key beside `context_roots` is refused as before, and nothing is created; a start refused at its floor (no
/// principal) creates no root either.
#[test]
fn an_unknown_key_beside_context_roots_is_refused_and_a_refused_start_creates_no_root() {
    let dir = scratch("ctxroot-unknown");
    let (code, err) = refused_start(&dir, "context_roots = [\"ctx\"]\nnot_a_key = 1\n", &["--principal", PRINCIPAL]);
    assert_eq!(code, Some(2), "{err}");
    assert!(err.contains("not_a_key: not a key of runbook.toml"), "{err}");
    assert!(!dir.join("conf/ctx").exists(), "a root was created by a file that was refused");
    let (code, err) = refused_start(&dir, "context_roots = [\"ctx\"]\n", &[]);
    assert_eq!(code, Some(1), "{err}");
    assert!(err.contains("--principal is required"), "{err}");
    assert!(!dir.join("conf/ctx").exists(), "a root was created before the start's floor held");
}

/// `runbook config` prints the roots of the file with their origin, each made absolute against the file.
#[test]
fn runbook_config_prints_the_roots_of_the_file_with_their_origin() {
    let dir = scratch("ctxroot-config");
    let config = dir.join("runbook.toml");
    std::fs::write(&config, "context_roots = [\"ctx\", \"/srv/ctx-b\"]\n").unwrap();
    let out = Command::new(BIN).arg("config").env("RUNBOOK_CONFIG", &config).current_dir(&dir).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{text} {}", String::from_utf8_lossy(&out.stderr));
    let want: PathBuf = dir.join("ctx");
    assert!(text.contains(&format!("context_roots = [\"{}\",\"/srv/ctx-b\"]  (file {})", want.display(), config.display())), "{text}");
    assert!(!want.exists(), "runbook config created a root");
}
