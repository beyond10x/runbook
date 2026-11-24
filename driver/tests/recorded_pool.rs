//! `sdc-recorded-pool`, the stand-in `slot` of the cycles suite (`docs/cycles-suite.md`, "Recorded
//! build and test results"): a command is answered from a recording keyed by the digest of its argv
//! and of the tree it runs in; a command with no recording is a program error naming the digest; in
//! record mode the command runs through the next `slot` on PATH in a copy of the tree, and what it
//! printed is written as the recording with the paths of the copy taken out.

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("recorded-pool").join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// A small crate tree in `d/tree`.
fn tree(d: &Path) -> PathBuf {
    let t = d.join("tree");
    fs::create_dir_all(t.join("src")).unwrap();
    fs::write(t.join("Cargo.toml"), "[package]\nname = \"billing\"\nversion = \"0.1.0\"\n").unwrap();
    fs::write(t.join("src/lib.rs"), "pub fn create_invoice() {}\n").unwrap();
    t
}

/// `d/pool/slot`, a link to the stand-in; with `record`, the directory holds the file that turns
/// record mode on.
fn pool(d: &Path, record: bool) -> PathBuf {
    let p = d.join("pool");
    fs::create_dir_all(&p).unwrap();
    symlink(env!("CARGO_BIN_EXE_sdc-recorded-pool"), p.join("slot")).unwrap();
    if record {
        fs::write(p.join("record"), "").unwrap();
    }
    p
}

/// `slot <argv>` in `cwd` with PATH = `path` and no HOME, as Loom starts a bound program.
fn slot(d: &Path, path: &[&Path], cwd: &Path, argv: &[&str], target: Option<&Path>) -> Output {
    let path = std::env::join_paths(path).unwrap();
    let mut c = Command::new("slot");
    c.args(argv)
        .current_dir(cwd)
        .env_clear()
        .env("PATH", path)
        .env("SDC_RECORDINGS", d.join("recordings"))
        .env("SDC_RECORDING_WORK", d.join("recording-work"));
    if let Some(t) = target {
        c.env("CARGO_TARGET_DIR", t);
    }
    c.output().unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// The digest a miss names: `no recording <digest>`.
fn missed(o: &Output) -> String {
    let err = text(&o.stderr);
    assert_eq!(o.status.code(), Some(127), "a command with no recording is a program error: {err}");
    assert!(o.stdout.is_empty(), "a miss prints nothing on stdout: {}", text(&o.stdout));
    let at = err.find("no recording ").unwrap_or_else(|| panic!("the miss names the recording: {err}"));
    err[at + "no recording ".len()..].chars().take_while(|c| c.is_ascii_hexdigit()).collect()
}

#[test]
fn a_command_with_no_recording_names_its_digest_and_a_recorded_one_replays_output_and_exit() {
    let d = scratch("replay");
    let t = tree(&d);
    let p = pool(&d, false);
    let o = slot(&d, &[&p], &t, &["cargo", "test", "--offline"], None);
    let key = missed(&o);
    assert_eq!(key.len(), 64, "a sha256 in hex: {key}");
    let err = text(&o.stderr);
    assert!(err.starts_with("error: "), "the line starts with `error`, which the impl gate shows: {err}");
    assert!(err.contains("cargo test --offline") && err.contains("task test:cycles-record"), "it names the command and the lane that records it: {err}");

    fs::create_dir_all(d.join("recordings")).unwrap();
    let rec = serde_json::json!({
        "format": "sdc.recorded-pool/1",
        "argv": ["cargo", "test", "--offline"],
        "exit": 101,
        "stdout": "test create_invoice ... FAILED\n\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n",
        "stderr": "   Compiling billing v0.1.0 ({dir})\n     Running tests ({target}/debug/deps/billing)\n",
    });
    fs::write(d.join("recordings").join(format!("{key}.json")), serde_json::to_string_pretty(&rec).unwrap()).unwrap();
    let target = d.join("tgt");
    let o = slot(&d, &[&p], &t, &["cargo", "test", "--offline"], Some(&target));
    assert_eq!(o.status.code(), Some(101), "the recorded exit: {}", text(&o.stderr));
    assert_eq!(text(&o.stdout), rec["stdout"].as_str().unwrap());
    assert_eq!(
        text(&o.stderr),
        format!("   Compiling billing v0.1.0 ({})\n     Running tests ({}/debug/deps/billing)\n", t.display(), target.display()),
        "{{dir}} is the tree the command runs in and {{target}} its CARGO_TARGET_DIR"
    );
}

#[test]
fn the_digest_follows_the_tree_and_the_command_and_leaves_the_target_directory_out() {
    let d = scratch("digest");
    let t = tree(&d);
    let p = pool(&d, false);
    let run = |argv: &[&str]| missed(&slot(&d, &[&p], &t, argv, None));
    let first = run(&["cargo", "test", "--offline"]);
    assert_eq!(run(&["cargo", "test", "--offline"]), first, "the same tree and command, the same digest");
    assert_eq!(run(&["--timeout", "30m", "cargo", "test", "--offline"]), first, "slot's own options are not the command");
    assert_ne!(run(&["cargo", "build", "--offline"]), first, "another command");
    fs::create_dir_all(t.join("target/debug")).unwrap();
    fs::write(t.join("target/debug/x"), "built").unwrap();
    assert_eq!(run(&["cargo", "test", "--offline"]), first, "target/ is build output, not the tree");
    fs::write(t.join("src/lib.rs"), "pub fn create_invoice() { todo!() }\n").unwrap();
    assert_ne!(run(&["cargo", "test", "--offline"]), first, "a changed file");
}

/// The git repository a bug-fix check tree carries (docs/bugfix.md, "The trees a check runs in are
/// git trees") is not the tree: its index and objects change with every copy, so it is left out of
/// the digest, and out of the copy record mode runs the command in.
#[test]
fn the_git_repository_of_a_check_tree_is_left_out_of_the_digest_and_the_recording_copy() {
    let t0 = std::time::Instant::now();
    let d = scratch("git-tree");
    let t = tree(&d);
    let p = pool(&d, false);
    let run = || missed(&slot(&d, &[&p], &t, &["cargo", "test", "--offline"], None));
    let first = run();
    fs::create_dir_all(t.join(".git/objects")).unwrap();
    fs::write(t.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    fs::write(t.join(".git/index"), "an index of this copy\n").unwrap();
    assert_eq!(run(), first, "a .git at the top of the tree is not in the digest");
    fs::write(t.join(".git/index"), "an index of another copy\n").unwrap();
    assert_eq!(run(), first, "nor its bytes");

    let rec = pool(&scratch("git-tree-record"), true);
    let real = d.join("real");
    fs::create_dir_all(&real).unwrap();
    let seen = d.join("seen-entries");
    // Shell builtins only: the PATH holds the two pools.
    let probe = format!("#!/bin/sh\nfor f in .git Cargo.toml src/lib.rs; do [ -e \"$f\" ] && echo \"$f\"; done > '{}'\nexit 0\n", seen.display());
    fs::write(real.join("slot"), probe).unwrap();
    fs::set_permissions(real.join("slot"), fs::Permissions::from_mode(0o755)).unwrap();
    let o = slot(&d, &[&rec, &real], &t, &["cargo", "test", "--offline"], None);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
    assert_eq!(fs::read_to_string(&seen).unwrap(), "Cargo.toml\nsrc/lib.rs\n", "the copy record mode runs in has no .git");
    let recs: Vec<PathBuf> = fs::read_dir(d.join("recordings")).unwrap().flatten().map(|e| e.path()).collect();
    assert_eq!(recs.len(), 1, "{recs:?}");
    assert_eq!(recs[0].file_stem().unwrap().to_string_lossy(), first, "recorded under the digest of the tree without its .git");
    eprintln!("recorded-pool git-tree: {:.2} s", t0.elapsed().as_secs_f64());
}

/// The build pool behind the stand-in in record mode: prints where it ran and the pool's own lines,
/// writes its HOME into `seen-home` beside the tree's parent, exits 101.
const REAL_SLOT: &str = r#"#!/bin/sh
printf '%s\n' "$HOME" > "$SEEN_HOME_FILE"
echo "slot: job 7 queued for 1 slot(s): 0 free, 0 request(s) ahead" >&2
echo "   Compiling billing v0.1.0 ($PWD)" >&2
echo "    Finished \`test\` profile [unoptimized] target(s) in 0.68s" >&2
echo "slot: job 7 waited 8s for a build slot, ran 0s" >&2
printf 'ran: %s\n' "$*"
printf 'test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.37s\n'
exit 101
"#;

#[test]
fn record_mode_runs_the_next_slot_in_a_copy_and_writes_the_recording_without_its_paths() {
    let d = scratch("record");
    let t = tree(&d);
    let p = pool(&d, true);
    let real = d.join("real");
    fs::create_dir_all(&real).unwrap();
    fs::write(real.join("slot"), REAL_SLOT.replace("$SEEN_HOME_FILE", &d.join("seen-home").display().to_string())).unwrap();
    fs::set_permissions(real.join("slot"), fs::Permissions::from_mode(0o755)).unwrap();
    let o = slot(&d, &[&p, &real], &t, &["cargo", "test", "--offline"], None);
    let err = text(&o.stderr);
    assert_eq!(o.status.code(), Some(101), "the command's exit: {err}");
    assert_eq!(text(&o.stdout), "ran: cargo test --offline\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n");
    assert_eq!(err, format!("   Compiling billing v0.1.0 ({})\n    Finished `test` profile [unoptimized] target(s) in 0.00s\n", t.display()), "printed as a replay prints it");
    let home = fs::read_to_string(d.join("seen-home")).unwrap();
    assert!(home.trim().starts_with('/'), "the pool got a HOME though the stand-in had none: {home:?}");

    let recs: Vec<PathBuf> = fs::read_dir(d.join("recordings")).unwrap().flatten().map(|e| e.path()).collect();
    assert_eq!(recs.len(), 1, "{recs:?}");
    let rec: Value = serde_json::from_str(&fs::read_to_string(&recs[0]).unwrap()).unwrap();
    assert_eq!(rec["format"], "sdc.recorded-pool/1");
    assert_eq!(rec["exit"], 101);
    assert_eq!(rec["argv"], serde_json::json!(["cargo", "test", "--offline"]));
    let stderr = rec["stderr"].as_str().unwrap();
    assert_eq!(stderr, "   Compiling billing v0.1.0 ({dir})\n    Finished `test` profile [unoptimized] target(s) in 0.00s\n", "the copy's path is {{dir}}, no pool line, no duration");
    assert!(!fs::read_to_string(&recs[0]).unwrap().contains(&d.display().to_string()), "no path of this machine in the recording");
    let left: Vec<_> = fs::read_dir(d.join("recording-work")).map(|r| r.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    assert!(left.is_empty(), "the copy is removed: {left:?}");

    let key = recs[0].file_stem().unwrap().to_string_lossy().into_owned();
    fs::remove_file(d.join("seen-home")).unwrap();
    let replay = scratch("record-replay");
    let p2 = pool(&replay, false);
    let t2 = tree(&replay);
    fs::create_dir_all(replay.join("recordings")).unwrap();
    fs::copy(&recs[0], replay.join("recordings").join(format!("{key}.json"))).unwrap();
    let o = slot(&replay, &[&p2, &real], &t2, &["cargo", "test", "--offline"], None);
    assert_eq!(o.status.code(), Some(101), "the same tree elsewhere replays the recording: {}", text(&o.stderr));
    assert!(!d.join("seen-home").exists(), "a replay starts no pool job");
}
