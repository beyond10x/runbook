//! The one routine that copies a directory of a runbook: `runbook run` copies its bindings with it
//! (links stay links, `Links::Keep`) and the daemon copies the runbook of a run with it (only
//! regular files whose real path lies inside the runbook home, `Links::Within`). The digest of a
//! directory is here too: the daemon names it in the run's record and checks the copy against it.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// How deep a copied directory may nest, links followed. A link that points back up is refused by it.
const MAX_DEPTH: usize = 24;

/// What a link in the source is.
#[derive(Clone, Copy)]
pub enum Links<'a> {
    /// A link is made again as a link, whatever it points at.
    Keep,
    /// A link is followed, and only a file or directory whose real path lies below this canonical
    /// directory is copied, as what it is. Anything that is not a regular file or a directory is refused.
    Within(&'a Path),
}

enum Entry {
    Dir(PathBuf),
    File { from: PathBuf, to: PathBuf },
    Link { target: PathBuf, to: PathBuf },
}

/// Copies each `(source, destination)` directory. Every refusal comes before the first write: when
/// this returns an error, no destination was created.
pub fn copy_dirs(jobs: &[(&Path, &Path)], links: Links<'_>) -> Result<(), String> {
    let mut entries = Vec::new();
    for (src, dst) in jobs {
        match links {
            Links::Keep => plan(src, dst, links, 0, &mut entries)?,
            Links::Within(root) => {
                let real = std::fs::canonicalize(src).map_err(|e| format!("{}: {e}", src.display()))?;
                if !real.starts_with(root) {
                    return Err(format!("{}: a link that leaves the runbook home", src.display()));
                }
                plan(&real, dst, links, 0, &mut entries)?
            }
        }
    }
    for entry in entries {
        match entry {
            Entry::Dir(dir) => std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?,
            Entry::Link { target, to } => std::os::unix::fs::symlink(&target, &to).map_err(|e| format!("{}: {e}", to.display()))?,
            Entry::File { from, to } => {
                std::fs::copy(&from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
                // A read-only source is copied writable: the copy is the run's.
                let mut permissions = std::fs::metadata(&to).map_err(|e| format!("{}: {e}", to.display()))?.permissions();
                use std::os::unix::fs::PermissionsExt;
                permissions.set_mode(permissions.mode() | 0o200);
                std::fs::set_permissions(&to, permissions).map_err(|e| format!("{}: {e}", to.display()))?;
            }
        }
    }
    Ok(())
}

fn plan(src: &Path, dst: &Path, links: Links<'_>, depth: usize, out: &mut Vec<Entry>) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!("{}: nested deeper than {MAX_DEPTH} levels, or a link that loops", src.display()));
    }
    out.push(Entry::Dir(dst.to_path_buf()));
    let entries = std::fs::read_dir(src).map_err(|e| format!("{}: {e}", src.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", src.display()))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let kind = entry.file_type().map_err(|e| format!("{}: {e}", from.display()))?;
        match links {
            Links::Keep if kind.is_symlink() => {
                // A link stays a link: the fixtures hold links to directories and to programs.
                let target = std::fs::read_link(&from).map_err(|e| format!("{}: {e}", from.display()))?;
                out.push(Entry::Link { target, to });
            }
            Links::Keep if kind.is_dir() => plan(&from, &to, links, depth + 1, out)?,
            Links::Keep => out.push(Entry::File { from, to }),
            Links::Within(root) => {
                let real = if kind.is_symlink() {
                    let real = std::fs::canonicalize(&from).map_err(|e| format!("{}: {e}", from.display()))?;
                    if !real.starts_with(root) {
                        return Err(format!("{}: a link that leaves the runbook home", from.display()));
                    }
                    real
                } else {
                    from.clone()
                };
                let meta = std::fs::metadata(&real).map_err(|e| format!("{}: {e}", from.display()))?;
                if meta.is_dir() {
                    plan(&real, &to, links, depth + 1, out)?;
                } else if meta.is_file() {
                    out.push(Entry::File { from: real, to });
                } else {
                    return Err(format!("{}: not a regular file or a directory", from.display()));
                }
            }
        }
    }
    Ok(())
}

/// The sha256 (hex) over a directory: for each entry in path order its kind, its relative path
/// and, for a file, its length, its executable bit and its bytes; for a link, its target. A file
/// edited, added, removed, made executable or replaced by a link changes it.
pub fn digest(dir: &Path) -> Result<String, String> {
    let mut hasher = Sha256::new();
    walk(dir, dir, &mut hasher)?;
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn walk(base: &Path, dir: &Path, hasher: &mut Sha256) -> Result<(), String> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        names.push(entry.map_err(|e| format!("{}: {e}", dir.display()))?.file_name());
    }
    names.sort();
    for name in names {
        let path = dir.join(&name);
        let rel = path.strip_prefix(base).unwrap_or(&path).to_string_lossy().into_owned();
        let meta = std::fs::symlink_metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let kind: &[u8] = if meta.file_type().is_symlink() {
            b"L"
        } else if meta.is_dir() {
            b"D"
        } else if meta.is_file() {
            b"F"
        } else {
            b"O"
        };
        hasher.update(kind);
        hasher.update([0]);
        hasher.update(rel.as_bytes());
        hasher.update([0]);
        if meta.file_type().is_symlink() {
            let target = std::fs::read_link(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            hasher.update(target.to_string_lossy().as_bytes());
        } else if meta.is_dir() {
            walk(base, &path, hasher)?;
        } else if meta.is_file() {
            use std::os::unix::fs::PermissionsExt;
            let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            hasher.update(bytes.len().to_string().as_bytes());
            hasher.update([0, u8::from(meta.permissions().mode() & 0o111 != 0), 0]);
            hasher.update(&bytes);
        }
        hasher.update([0]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::current_exe().unwrap().parent().unwrap().join(format!("engine-copy-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::canonicalize(d).unwrap()
    }

    fn home(tag: &str) -> PathBuf {
        let home = dir(tag);
        std::fs::create_dir_all(home.join("bindings/tools")).unwrap();
        std::fs::write(home.join("bindings/a.yaml"), "a: 1\n").unwrap();
        std::fs::write(home.join("bindings/tools/run.sh"), "#!/bin/sh\n").unwrap();
        home
    }

    #[test]
    fn the_digest_changes_with_an_edit_an_addition_a_removal_and_a_link_and_not_otherwise() {
        let home = home("digest");
        let at = |home: &Path| digest(&home.join("bindings")).unwrap();
        let first = at(&home);
        assert_eq!(first, at(&home));
        std::fs::write(home.join("bindings/a.yaml"), "a: 2\n").unwrap();
        let edited = at(&home);
        assert_ne!(first, edited);
        std::fs::write(home.join("bindings/b.yaml"), "").unwrap();
        let added = at(&home);
        assert_ne!(edited, added);
        std::fs::remove_file(home.join("bindings/b.yaml")).unwrap();
        assert_eq!(edited, at(&home));
        std::fs::remove_file(home.join("bindings/tools/run.sh")).unwrap();
        let removed = at(&home);
        assert_ne!(edited, removed);
        std::os::unix::fs::symlink("a.yaml", home.join("bindings/tools/run.sh")).unwrap();
        assert_ne!(removed, at(&home));
    }

    #[test]
    fn a_link_that_leaves_the_home_is_refused_before_anything_is_written() {
        let home = home("leaves");
        let outside = dir("leaves-outside");
        std::fs::write(outside.join("secret"), "x").unwrap();
        std::os::unix::fs::symlink(outside.join("secret"), home.join("bindings/tools/leak")).unwrap();
        let target = dir("leaves-target");
        let (a, b) = (target.join("bindings"), target.join("again"));
        let refused = copy_dirs(&[(&home.join("bindings"), &a), (&home.join("bindings"), &b)], Links::Within(&home)).unwrap_err();
        assert!(refused.contains("leaves the runbook home"), "{refused}");
        assert!(!a.exists() && !b.exists(), "a destination was made");
        // `runbook run` keeps a link as it is.
        copy_dirs(&[(&home.join("bindings"), &a)], Links::Keep).unwrap();
        assert!(std::fs::symlink_metadata(a.join("tools/leak")).unwrap().file_type().is_symlink());
    }

    #[test]
    fn a_link_inside_the_home_is_copied_as_the_file_and_a_loop_is_refused() {
        let home = home("inside");
        std::os::unix::fs::symlink("run.sh", home.join("bindings/tools/again.sh")).unwrap();
        let target = dir("inside-target");
        let dst = target.join("bindings");
        copy_dirs(&[(&home.join("bindings"), &dst)], Links::Within(&home)).unwrap();
        let meta = std::fs::symlink_metadata(dst.join("tools/again.sh")).unwrap();
        assert!(meta.is_file() && !meta.file_type().is_symlink());
        std::os::unix::fs::symlink("..", home.join("bindings/tools/up")).unwrap();
        let refused = copy_dirs(&[(&home.join("bindings"), &target.join("loop"))], Links::Within(&home)).unwrap_err();
        assert!(refused.contains("loops"), "{refused}");
        assert!(!target.join("loop").exists());
    }

    #[test]
    fn a_source_that_is_a_link_out_of_the_home_is_refused() {
        let home = home("source");
        let outside = dir("source-outside");
        std::fs::remove_dir_all(home.join("bindings")).unwrap();
        std::os::unix::fs::symlink(&outside, home.join("bindings")).unwrap();
        let target = dir("source-target");
        let refused = copy_dirs(&[(&home.join("bindings"), &target.join("bindings"))], Links::Within(&home)).unwrap_err();
        assert!(refused.contains("leaves the runbook home"), "{refused}");
        assert!(!target.join("bindings").exists());
    }
}
