//! Lists the files of `ui/web/` and `ui/daemon/` for `src/ui_embed.rs`: the list is made from the two
//! directories at every build, so a file that is added there is embedded and the build is redone when one
//! changes. `ui/web/` is what the inspector (`ui/server.mjs`) serves too; `ui/daemon/` is the code only the
//! daemon serves (the answer to a decision). A path in both fails the build. A symbolic link in either
//! directory, to a file or to a directory, fails the build and names the link.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, PathBuf)>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        // an editor's swap file or a dot file is not part of the UI
        if name.starts_with('.') {
            continue;
        }
        if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            panic!("{} is a symbolic link: the UI is compiled into the binary from files and directories of ui/web/ only, so a link could pull a file from outside it", path.display());
        }
        if path.is_dir() {
            walk(&path, base, out);
        } else {
            let rel = path.strip_prefix(base).unwrap().components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
            out.push((rel, path));
        }
    }
}

/// The commit of the Loom this build links: the `#<commit>` of the `source` of `b10x-loom-tree` in
/// `Cargo.lock` (the manifest's `rev` is what that was resolved from). Read here, never typed.
fn loom_commit(lock: &Path) -> String {
    let text = std::fs::read_to_string(lock).unwrap_or_else(|e| panic!("cannot read {}: {e}", lock.display()));
    let mut in_package = false;
    for line in text.lines() {
        if line == "[[package]]" {
            in_package = false;
        } else if line == "name = \"b10x-loom-tree\"" {
            in_package = true;
        } else if in_package {
            if let Some(source) = line.strip_prefix("source = \"") {
                let commit = source.rsplit_once('#').map(|(_, c)| c.trim_end_matches('"')).unwrap_or("");
                if commit.len() == 40 && commit.chars().all(|c| c.is_ascii_hexdigit()) {
                    return commit.to_string();
                }
                panic!("the source of b10x-loom-tree in {} names no commit: {source}", lock.display());
            }
        }
    }
    panic!("{} holds no package b10x-loom-tree with a source", lock.display());
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let lock = manifest.join("Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock.display());
    println!("cargo:rustc-env=RUNBOOK_LOOM_COMMIT={}", loom_commit(&lock));
    println!("cargo:rerun-if-changed=build.rs");
    let mut files = Vec::new();
    for dir in ["../ui/web", "../ui/daemon"] {
        let dir = manifest.join(dir);
        println!("cargo:rerun-if-changed={}", dir.display());
        if !dir.is_dir() {
            panic!("{} is not a directory: the UI is compiled into the binary from ui/web/ and ui/daemon/", dir.display());
        }
        let dir = dir.canonicalize().unwrap();
        walk(&dir, &dir, &mut files);
    }
    files.sort();
    if let Some(w) = files.windows(2).find(|w| w[0].0 == w[1].0) {
        panic!("{} is in both ui/web/ and ui/daemon/: one path, one file", w[0].0);
    }
    let mut src = String::from("pub static FILES: &[(&str, &[u8])] = &[\n");
    for (rel, path) in &files {
        writeln!(src, "    ({rel:?}, include_bytes!({:?})),", path.to_str().unwrap()).unwrap();
    }
    src.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("ui_files.rs");
    std::fs::write(out, src).unwrap();
}
