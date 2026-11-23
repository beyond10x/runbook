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
use std::io::Read;
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

fn list(workspace: &Path, max_files: usize) -> Listing {
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
                    if seen > MAX_ENTRIES {
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
    fs::write(&tmp, bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// [`write`] with the bound on files given.
pub fn write_with(run_dir: &Path, workspace: &Path, at_secs: u64, max_files: usize) -> Result<(), String> {
    let listing = list(workspace, max_files);
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
}
