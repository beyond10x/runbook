//! Lists the files of `ui/web/` for `src/ui_embed.rs`: the list is made from the directory at every
//! build, so a file that is added there is embedded and the build is redone when one changes.

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
        if path.is_dir() {
            walk(&path, base, out);
        } else {
            let rel = path.strip_prefix(base).unwrap().components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
            out.push((rel, path));
        }
    }
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let web = manifest.join("../ui/web");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", web.display());
    if !web.is_dir() {
        panic!("{} is not a directory: the UI is compiled into the binary from ui/web/", web.display());
    }
    let web = web.canonicalize().unwrap();
    let mut files = Vec::new();
    walk(&web, &web, &mut files);
    files.sort();
    let mut src = String::from("pub static FILES: &[(&str, &[u8])] = &[\n");
    for (rel, path) in &files {
        writeln!(src, "    ({rel:?}, include_bytes!({:?})),", path.to_str().unwrap()).unwrap();
    }
    src.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("ui_files.rs");
    std::fs::write(out, src).unwrap();
}
