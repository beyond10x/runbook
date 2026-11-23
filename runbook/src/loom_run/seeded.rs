//! What the run was seeded with: `seeded.json` and `seeded-files.jsonl` in the run directory.
//!
//! The runner lists `work/` once, right after the seed ends (the intake file, the bug-fix seed, the
//! goals copy). A file the seed wrote is a `create` of step 1 in `fs/changes.jsonl`, the same as a
//! file the first step writes, because the watcher starts before `work/` exists and no record marks
//! the end of the seed. This record is that mark: a reader that holds both tells a seeded file (its
//! path is listed, its content hash is the one the change record ends at) from a written one.
//!
//! - `seeded.json` (`runbook.seeded/1`): `at` (seconds since the Unix epoch), `complete`, `files` (the
//!   number of files listed) and `entries`, one per top-level entry of `work/` with its `kind`
//!   (`dir`, `file`, `link` or `other`) and the number of regular files below it.
//! - `seeded-files.jsonl`: one line per regular file, `{"path": <relative to work/>, "sha256": <hex>}`,
//!   sorted by path. The hash is the SHA-256 of the content, the form of `after` in `fs/changes.jsonl`.
//!
//! Regular files only, links are not followed, an entry whose name starts with a dot is skipped, as
//! the inspector skips it. The listing stops at [`MAX_FILES`] files and says `complete: false`; so it
//! does for an entry it could not read. The record holds no path outside `work/`, no environment, no
//! argument and no file content.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

/// The summary record.
pub const RECORD: &str = "seeded.json";
/// The list of seeded files.
pub const LIST: &str = "seeded-files.jsonl";
/// The format member of the summary.
pub const FORMAT: &str = "runbook.seeded/1";
/// The listing stops at this many files.
pub const MAX_FILES: usize = 20_000;
/// It also stops at this many entries of any kind (a directory of a million links costs no more).
const MAX_ENTRIES: usize = 200_000;

/// Lists `workspace` and writes the record into `run_dir`.
pub fn write(run_dir: &Path, workspace: &Path, at_secs: u64) -> Result<(), String> {
    write_with(run_dir, workspace, at_secs, MAX_FILES)
}

struct Listing {
    files: Vec<(String, String)>,
    entries: Vec<Value>,
    complete: bool,
}

fn sha256_of(path: &Path) -> Option<String> {
    let mut f = fs::File::open(path).ok()?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Some(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn names_of(dir: &Path) -> Option<Vec<String>> {
    let mut names = Vec::new();
    for e in fs::read_dir(dir).ok()? {
        names.push(e.ok()?.file_name().into_string().ok()?);
    }
    names.sort();
    Some(names)
}

fn list(workspace: &Path, max_files: usize, max_entries: usize) -> Listing {
    let mut out = Listing { files: Vec::new(), entries: Vec::new(), complete: true };
    let mut seen = 0usize;
    let Some(top) = names_of(workspace) else {
        out.complete = false;
        return out;
    };
    for name in top.into_iter().filter(|n| !n.starts_with('.')) {
        let path = workspace.join(&name);
        let Ok(md) = fs::symlink_metadata(&path) else {
            out.complete = false;
            continue;
        };
        let before = out.files.len();
        let kind = if md.file_type().is_symlink() {
            "link"
        } else if md.is_file() {
            if out.files.len() >= max_files {
                out.complete = false;
            } else if let Some(sha) = sha256_of(&path) {
                out.files.push((name.clone(), sha));
            } else {
                out.complete = false;
            }
            "file"
        } else if md.is_dir() {
            let mut stack = vec![(path, name.clone())];
            while let Some((dir, rel)) = stack.pop() {
                let Some(names) = names_of(&dir) else {
                    out.complete = false;
                    continue;
                };
                // popped last-in-first-out: push in reverse so the walk reads names in order
                let mut sub = Vec::new();
                for n in names.into_iter().filter(|n| !n.starts_with('.')) {
                    seen += 1;
                    if seen > max_entries {
                        out.complete = false;
                        break;
                    }
                    let p = dir.join(&n);
                    let r = format!("{rel}/{n}");
                    let Ok(m) = fs::symlink_metadata(&p) else {
                        out.complete = false;
                        continue;
                    };
                    if m.is_dir() {
                        sub.push((p, r));
                    } else if m.is_file() {
                        if out.files.len() >= max_files {
                            out.complete = false;
                        } else if let Some(sha) = sha256_of(&p) {
                            out.files.push((r, sha));
                        } else {
                            out.complete = false;
                        }
                    }
                }
                stack.extend(sub.into_iter().rev());
            }
            "dir"
        } else {
            "other"
        };
        out.entries.push(json!({"path": name, "kind": kind, "files": out.files.len() - before}));
    }
    out.files.sort();
    out
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    // What lies at the temporary name is removed (a link is removed, never followed) and the file is then created new: the
    // open fails on anything that is there, so the record is never written into a file this function did not create.
    match fs::remove_file(&tmp) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("{}: {e}", tmp.display())),
    }
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    file.write_all(bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    drop(file);
    fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// [`write`] with the bound on files given.
pub fn write_with(run_dir: &Path, workspace: &Path, at_secs: u64, max_files: usize) -> Result<(), String> {
    write_limited(run_dir, workspace, at_secs, max_files, MAX_ENTRIES)
}

/// [`write`] with both bounds given: files, and entries of any kind.
pub fn write_limited(run_dir: &Path, workspace: &Path, at_secs: u64, max_files: usize, max_entries: usize) -> Result<(), String> {
    let listing = list(workspace, max_files, max_entries);
    let mut lines = String::new();
    for (path, sha) in &listing.files {
        lines.push_str(&json!({"path": path, "sha256": sha}).to_string());
        lines.push('\n');
    }
    write_atomic(&run_dir.join(LIST), lines.as_bytes())?;
    let record = json!({"format": FORMAT, "at": at_secs, "complete": listing.complete, "files": listing.files.len(), "entries": listing.entries});
    let text = serde_json::to_string_pretty(&record).map_err(|e| e.to_string())? + "\n";
    write_atomic(&run_dir.join(RECORD), text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch(tag: &str) -> PathBuf {
        let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/tmp").join(format!("seeded-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("work")).unwrap();
        d
    }

    fn record(run: &Path) -> Value {
        serde_json::from_str(&fs::read_to_string(run.join(RECORD)).unwrap()).unwrap()
    }

    fn listed(run: &Path) -> Vec<(String, String)> {
        fs::read_to_string(run.join(LIST)).unwrap().lines().map(|l| serde_json::from_str::<Value>(l).unwrap()).map(|v| (v["path"].as_str().unwrap().to_owned(), v["sha256"].as_str().unwrap().to_owned())).collect()
    }

    #[test]
    fn a_listing_past_the_bound_says_it_is_not_complete() {
        let run = scratch("bound");
        for i in 0..7 {
            fs::write(run.join("work").join(format!("f{i}.txt")), format!("{i}")).unwrap();
        }
        write_with(&run, &run.join("work"), 5, 4).unwrap();
        let r = record(&run);
        assert_eq!(r["complete"], false, "{r}");
        assert_eq!(listed(&run).len(), 4);
        assert_eq!(r["files"], 4);
    }

    #[test]
    fn a_listing_of_two_thousand_files_is_quick() {
        let run = scratch("two-thousand");
        for d in 0..20 {
            let dir = run.join("work/tree").join(format!("d{d}"));
            fs::create_dir_all(&dir).unwrap();
            for i in 0..100 {
                fs::write(dir.join(format!("f{i}.rs")), format!("fn f{d}_{i}() {{}}\n").repeat(40)).unwrap();
            }
        }
        let started = std::time::Instant::now();
        write(&run, &run.join("work"), 1).unwrap();
        let took = started.elapsed();
        eprintln!("seeded record of 2000 files: {took:?}");
        assert!(took.as_secs() < 5, "slow: {took:?}");
        let r = record(&run);
        assert_eq!((r["files"].as_u64(), r["complete"].as_bool()), (Some(2000), Some(true)), "{r}");
        assert_eq!(r["entries"][0]["files"], 2000);
    }

    #[test]
    fn links_are_not_followed_and_dot_entries_are_skipped_and_no_absolute_path_is_written() {
        let run = scratch("links");
        let work = run.join("work");
        fs::create_dir_all(work.join("a/.git")).unwrap();
        fs::write(work.join("a/.git/config"), "x").unwrap();
        fs::write(work.join("a/.hidden"), "x").unwrap();
        fs::write(work.join("a/x.txt"), "x").unwrap();
        fs::create_dir_all(run.join("outside")).unwrap();
        fs::write(run.join("outside/secret.txt"), "OUTSIDE").unwrap();
        std::os::unix::fs::symlink(run.join("outside"), work.join("out-link")).unwrap();
        std::os::unix::fs::symlink(run.join("outside/secret.txt"), work.join("a/file-link")).unwrap();
        write(&run, &work, 1).unwrap();
        assert_eq!(listed(&run).iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(), ["a/x.txt"]);
        let all = format!("{}{}", fs::read_to_string(run.join(RECORD)).unwrap(), fs::read_to_string(run.join(LIST)).unwrap());
        assert!(!all.contains(run.to_str().unwrap()) && !all.contains("OUTSIDE"), "{all}");
        let r = record(&run);
        let kinds: Vec<(&str, &str)> = r["entries"].as_array().unwrap().iter().map(|e| (e["path"].as_str().unwrap(), e["kind"].as_str().unwrap())).collect();
        assert_eq!(kinds, [("a", "dir"), ("out-link", "link")]);
    }

    /// `write` on its own thread: a call that does not return in 10 s is a hang, reported, not waited for.
    fn write_in_time(run: &Path) -> Result<(), String> {
        let (tx, rx) = std::sync::mpsc::channel();
        let (run, work) = (run.to_path_buf(), run.join("work"));
        std::thread::spawn(move || {
            let _ = tx.send(write(&run, &work, 1));
        });
        rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap_or_else(|_| panic!("write did not return in 10 s: it opened something that blocks"))
    }

    fn mkfifo(path: &Path) {
        assert!(std::process::Command::new("mkfifo").arg(path).status().unwrap().success());
    }

    fn paths(run: &Path) -> Vec<String> {
        listed(run).into_iter().map(|(p, _)| p).collect()
    }

    /// A mode 000 file or directory is read by the superuser: the cases that need one are not run there.
    fn permissions_bind(probe: &Path) -> bool {
        use std::os::unix::fs::PermissionsExt;
        fs::write(probe, "p").unwrap();
        fs::set_permissions(probe, fs::Permissions::from_mode(0o000)).unwrap();
        let binds = fs::read(probe).is_err();
        fs::set_permissions(probe, fs::Permissions::from_mode(0o644)).unwrap();
        binds
    }

    #[test]
    fn the_record_is_not_written_through_a_link_at_its_temporary_name() {
        let run = scratch("tmp-link");
        let victim = run.join("victim.txt");
        fs::write(&victim, "VICTIM-ORIGINAL").unwrap();
        std::os::unix::fs::symlink(&victim, run.join("seeded.tmp")).unwrap();
        std::os::unix::fs::symlink(&victim, run.join("seeded-files.tmp")).unwrap();
        fs::write(run.join("work/a.txt"), "a").unwrap();
        write_in_time(&run).unwrap();
        assert_eq!(fs::read_to_string(&victim).unwrap(), "VICTIM-ORIGINAL", "the seed record was written through a link into a file it does not own");
        assert_eq!(paths(&run), ["a.txt"]);
        assert_eq!(record(&run)["files"], 1);
        for name in [RECORD, LIST] {
            assert!(fs::symlink_metadata(run.join(name)).unwrap().is_file(), "{name} is a regular file of its own");
        }
    }

    #[test]
    fn a_named_pipe_in_the_seed_is_never_opened_and_the_record_says_so() {
        let run = scratch("fifo");
        let work = run.join("work");
        fs::create_dir_all(work.join("tree")).unwrap();
        fs::write(work.join("tree/a.txt"), "a").unwrap();
        mkfifo(&work.join("pipe-top"));
        mkfifo(&work.join("tree/pipe-nested"));
        write_in_time(&run).unwrap();
        assert_eq!(paths(&run), ["tree/a.txt"]);
        let kinds: Vec<(String, String)> = record(&run)["entries"].as_array().unwrap().iter().map(|e| (e["path"].as_str().unwrap().to_owned(), e["kind"].as_str().unwrap().to_owned())).collect();
        assert_eq!(kinds, [("pipe-top".to_owned(), "other".to_owned()), ("tree".to_owned(), "dir".to_owned())]);
    }

    #[test]
    fn a_name_that_is_not_text_makes_the_record_incomplete_not_wrong() {
        use std::os::unix::ffi::OsStrExt;
        let run = scratch("non-utf8");
        let work = run.join("work");
        fs::create_dir_all(work.join("tree")).unwrap();
        fs::write(work.join("tree/ok.txt"), "a").unwrap();
        fs::write(work.join("tree").join(std::ffi::OsStr::from_bytes(b"bad-\xff.txt")), "b").unwrap();
        write_in_time(&run).unwrap();
        assert_eq!(record(&run)["complete"], false, "{}", record(&run));
    }

    #[test]
    fn the_list_is_sorted_by_path_as_the_docs_say() {
        let run = scratch("sorted");
        let work = run.join("work");
        fs::create_dir_all(work.join("a")).unwrap();
        fs::write(work.join("a/x.txt"), "x").unwrap();
        fs::write(work.join("a.txt"), "a").unwrap();
        fs::write(work.join("b.txt"), "b").unwrap();
        write_in_time(&run).unwrap();
        assert_eq!(paths(&run), ["a.txt", "a/x.txt", "b.txt"]);
    }

    #[test]
    fn top_level_dot_entries_are_skipped() {
        let run = scratch("dot-top");
        let work = run.join("work");
        fs::create_dir_all(work.join(".cache")).unwrap();
        fs::write(work.join(".cache/token.txt"), "t").unwrap();
        fs::write(work.join(".hidden"), "h").unwrap();
        fs::write(work.join("ok.txt"), "o").unwrap();
        write_in_time(&run).unwrap();
        assert_eq!(paths(&run), ["ok.txt"]);
        let names: Vec<String> = record(&run)["entries"].as_array().unwrap().iter().map(|e| e["path"].as_str().unwrap().to_owned()).collect();
        assert_eq!(names, ["ok.txt"]);
    }

    #[test]
    fn a_nested_listing_past_the_bound_says_it_is_not_complete() {
        let run = scratch("bound-nested");
        let work = run.join("work");
        fs::create_dir_all(work.join("tree")).unwrap();
        for i in 0..7 {
            fs::write(work.join("tree").join(format!("f{i}.txt")), format!("{i}")).unwrap();
        }
        write_with(&run, &work, 1, 4).unwrap();
        assert_eq!(record(&run)["complete"], false);
        assert_eq!(listed(&run).len(), 4);
    }

    #[test]
    fn a_directory_past_the_entry_bound_says_it_is_not_complete() {
        let run = scratch("entries");
        let work = run.join("work");
        fs::create_dir_all(work.join("many")).unwrap();
        for i in 0..6 {
            std::os::unix::fs::symlink("x", work.join("many").join(format!("l{i}"))).unwrap();
        }
        write_limited(&run, &work, 1, MAX_FILES, 5).unwrap();
        assert_eq!(record(&run)["complete"], false);
        write_limited(&run, &work, 1, MAX_FILES, 6).unwrap();
        assert_eq!(record(&run)["complete"], true, "a directory of exactly the bound is complete");
    }

    #[test]
    fn a_workspace_that_cannot_be_read_is_said_not_listed_as_empty() {
        let run = scratch("no-workspace");
        write(&run, &run.join("no-such-work"), 1).unwrap();
        assert_eq!(record(&run)["complete"], false, "{}", record(&run));
    }

    #[test]
    fn an_unreadable_directory_and_an_unreadable_file_make_the_record_incomplete() {
        use std::os::unix::fs::PermissionsExt;
        let run = scratch("unreadable");
        if !permissions_bind(&run.join("probe")) {
            eprintln!("skipped: this user reads a file of mode 000");
            return;
        }
        let work = run.join("work");
        fs::create_dir_all(work.join("closed")).unwrap();
        fs::write(work.join("closed/in.txt"), "i").unwrap();
        fs::write(work.join("ok.txt"), "o").unwrap();
        fs::set_permissions(work.join("closed"), fs::Permissions::from_mode(0o000)).unwrap();
        write_in_time(&run).unwrap();
        fs::set_permissions(work.join("closed"), fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(record(&run)["complete"], false, "an unreadable directory: {}", record(&run));
        // an unreadable nested file and an unreadable top-level file, each alone: neither is listed and each says so
        for (tag, rel) in [("unreadable-nested-file", "tree/secret.txt"), ("unreadable-top-file", "top.txt")] {
            let run = scratch(tag);
            fs::create_dir_all(run.join("work/tree")).unwrap();
            fs::write(run.join("work").join(rel), "s").unwrap();
            fs::set_permissions(run.join("work").join(rel), fs::Permissions::from_mode(0o000)).unwrap();
            write_in_time(&run).unwrap();
            assert_eq!(record(&run)["complete"], false, "{tag}: {}", record(&run));
            assert!(listed(&run).is_empty(), "{tag}: {:?}", listed(&run));
        }
    }

    /// Every file under `dir` with its content, a link with its target: what a function that must leave a tree alone is compared on.
    fn tree_state(dir: &Path) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in fs::read_dir(&d).unwrap() {
                let p = e.unwrap().path();
                let md = fs::symlink_metadata(&p).unwrap();
                let rel = p.strip_prefix(dir).unwrap().display().to_string();
                if md.is_dir() {
                    stack.push(p);
                } else if md.is_file() {
                    out.push((rel, fs::read_to_string(&p).unwrap()));
                } else {
                    out.push((rel, format!("{:?}", fs::read_link(&p))));
                }
            }
        }
        out.sort();
        out
    }

    /// A directory outside the run with a file in it, and the state of both: the run's own record must never touch it.
    fn outside_of(run: &Path) -> (PathBuf, Vec<(String, String)>) {
        let outside = run.join("outside-dir");
        fs::create_dir_all(outside.join("inner")).unwrap();
        fs::write(outside.join("inner/keep.txt"), "KEEP").unwrap();
        let before = tree_state(&outside);
        (outside, before)
    }

    #[test]
    fn a_link_at_the_final_name_is_replaced_not_followed() {
        let run = scratch("final-link");
        let victim = run.join("victim.txt");
        fs::write(&victim, "VICTIM-ORIGINAL").unwrap();
        fs::write(run.join("work/a.txt"), "a").unwrap();
        std::os::unix::fs::symlink(&victim, run.join(RECORD)).unwrap();
        std::os::unix::fs::symlink(&victim, run.join(LIST)).unwrap();
        write_in_time(&run).unwrap();
        assert_eq!(fs::read_to_string(&victim).unwrap(), "VICTIM-ORIGINAL", "the record was written through a link at its final name");
        for name in [RECORD, LIST] {
            assert!(fs::symlink_metadata(run.join(name)).unwrap().is_file(), "{name} is a regular file of its own");
        }
        assert_eq!(paths(&run), ["a.txt"]);
    }

    #[test]
    fn a_hard_link_is_listed_by_its_own_name_only_and_the_record_holds_no_outside_path() {
        let run = scratch("hard-link");
        let outside = run.join("outside.txt");
        fs::write(&outside, "OUTSIDE-CONTENT").unwrap();
        fs::hard_link(&outside, run.join("work/inside-name.txt")).unwrap();
        write_in_time(&run).unwrap();
        let list = fs::read_to_string(run.join(LIST)).unwrap();
        let all = format!("{}{}", fs::read_to_string(run.join(RECORD)).unwrap(), list);
        assert_eq!(paths(&run), ["inside-name.txt"]);
        assert!(!all.contains("outside.txt") && !all.contains("OUTSIDE-CONTENT") && !all.contains(run.to_str().unwrap()), "{all}");
    }

    /// `write` reaches the walk's bound of entries through `write_with` and `write_limited`; the case above runs `write_limited` with
    /// a bound of 5, so only a walk of more than the default (200 000, docs/loom-run-directory.md) holds the wiring of the constant.
    /// The links are made by eight threads, each in a directory of its own.
    #[test]
    fn the_default_walk_stops_at_the_documented_number_of_entries_and_says_it_is_not_complete() {
        let run = scratch("entries-default");
        let work = run.join("work");
        let per = MAX_ENTRIES / 8 + 1;
        std::thread::scope(|s| {
            for t in 0..8 {
                let dir = work.join(format!("d{t}"));
                s.spawn(move || {
                    fs::create_dir_all(&dir).unwrap();
                    for i in 0..per {
                        std::os::unix::fs::symlink("x", dir.join(format!("l{i}"))).unwrap();
                    }
                });
            }
        });
        assert_eq!((MAX_ENTRIES, MAX_FILES), (200_000, 20_000), "the numbers docs/loom-run-directory.md names");
        write(&run, &work, 1).unwrap();
        assert_eq!(record(&run)["complete"], false, "{}", record(&run));
    }

    #[test]
    fn a_link_to_a_directory_at_the_temporary_name_is_removed_and_the_directory_is_left_alone() {
        let run = scratch("tmp-dir-link");
        let (outside, before) = outside_of(&run);
        std::os::unix::fs::symlink(&outside, run.join("seeded.tmp")).unwrap();
        std::os::unix::fs::symlink(&outside, run.join("seeded-files.tmp")).unwrap();
        write_in_time(&run).unwrap();
        assert_eq!(tree_state(&outside), before, "the directory behind the link at the temporary name was written into or removed from");
        assert!(fs::symlink_metadata(run.join(RECORD)).unwrap().is_file());
    }

    #[test]
    fn a_directory_at_the_temporary_name_or_at_the_final_name_ends_in_an_error_and_nothing_outside_the_run_changes() {
        for (tag, name) in [("tmp-dir", "seeded.tmp"), ("final-dir", RECORD)] {
            let run = scratch(tag);
            let (_outside, before) = outside_of(&run);
            fs::create_dir_all(run.join(name)).unwrap();
            fs::write(run.join(name).join("inside.txt"), "INSIDE").unwrap();
            let r = write_in_time(&run);
            assert!(r.is_err(), "{tag}: a directory at the name: {r:?}");
            assert_eq!(fs::read_to_string(run.join(name).join("inside.txt")).unwrap(), "INSIDE", "{tag}: the directory was emptied");
            assert_eq!(tree_state(&run.join("outside-dir")), before, "{tag}");
        }
    }

    /// A program of the run that plants a link at the temporary name between the removal and the create: the file is created
    /// new, so the plant makes the create fail and is never written through. 60 writes against a planter that never stops.
    #[test]
    fn a_link_planted_at_the_temporary_name_while_the_record_is_written_is_never_written_through() {
        let run = scratch("race-link");
        fs::write(run.join("work/a.txt"), "a").unwrap();
        let victim = run.join("victim.txt");
        fs::write(&victim, "VICTIM-ORIGINAL").unwrap();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let planter = {
            let (stop, run, victim) = (stop.clone(), run.clone(), victim.clone());
            std::thread::spawn(move || {
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    for name in ["seeded.tmp", "seeded-files.tmp"] {
                        let _ = fs::remove_file(run.join(name));
                        let _ = std::os::unix::fs::symlink(&victim, run.join(name));
                    }
                }
            })
        };
        for _ in 0..60 {
            let _ = write_in_time(&run);
        }
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        planter.join().unwrap();
        assert_eq!(fs::read_to_string(&victim).unwrap(), "VICTIM-ORIGINAL", "a link planted at the temporary name was written through");
    }

    /// Two writers of one run directory: each record is whole after both ended, never a mix of the two writes.
    #[test]
    fn two_writers_never_leave_a_record_that_is_not_the_whole_record() {
        let run = scratch("two-writers");
        for i in 0..40 {
            fs::write(run.join("work").join(format!("f{i}.txt")), format!("{i}\n").repeat(200)).unwrap();
        }
        for round in 0..30 {
            let handles: Vec<_> = (0..2)
                .map(|_| {
                    let run = run.clone();
                    std::thread::spawn(move || {
                        let _ = write(&run, &run.join("work"), 1);
                    })
                })
                .collect();
            for h in handles {
                h.join().unwrap();
            }
            let lines = fs::read_to_string(run.join(LIST)).unwrap();
            let valid = lines.lines().filter(|l| serde_json::from_str::<Value>(l).is_ok()).count();
            assert_eq!((record(&run)["files"].clone(), lines.lines().count(), valid), (Value::from(40), 40, 40), "round {round}");
        }
    }
}
