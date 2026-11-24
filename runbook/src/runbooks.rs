//! The runbooks a host can start: the directories `runbook.toml` names (`runbooks = [...]`), then the
//! built-in set (the asset root), always last. One function, `load`, reads them for `runbook list`,
//! `run`, `check` and `doctor --for`; `serve` and its engine reach the same directories through
//! `layout::resolve_in` and `locate`, after `load` has accepted the set at start.
//!
//! A runbook directory holds `protocol/` and `bindings/` and may hold `views/` and `instructions/`.
//! A configured directory is read through the checks the built-in set has: its files are regular files
//! and no link leads out of the directory; a directory that is missing, or holds no `protocol/` or no
//! `bindings/`, is an error that names it. Two directories with one runbook name are an error that
//! names both. Reading only reads: nothing of a directory is executed or run through a program.
//!
//! Whether a protocol can be started is what the files say: a protocol that no protocol of the set opens
//! with `case_open` is a runbook; the others are the children of one.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const TITLE_MAX: usize = 100;
/// The directories of a runbook directory whose files are checked.
const CHECKED: [&str; 4] = ["protocol", "bindings", "views", "instructions"];
const WALK_DEPTH: usize = 8;
const WALK_ENTRIES: usize = 20_000;

/// One protocol of a runbook directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The file stem: `sdc`, `sdc-bugfix`, `sdc-story`.
    pub id: String,
    /// The id the document carries (`protocol.id`, for example `sdc.bugfix`).
    pub protocol_id: String,
    pub revision: Option<u64>,
    /// No protocol of the set opens this one: a person can start it.
    pub startable: bool,
    pub title: String,
    /// The directory it comes from: a configured one, or the asset root.
    pub dir: PathBuf,
    pub built_in: bool,
}

/// The directories in resolution order: the configured ones, then the built-in root unless a
/// configured directory is that same directory.
fn order(configured: &[PathBuf], built_in: Option<&Path>) -> Result<Vec<(PathBuf, bool)>, String> {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let mut dirs: Vec<(PathBuf, bool)> = Vec::new();
    for dir in configured {
        if let Some((first, _)) = dirs.iter().find(|(d, _)| canon(d) == canon(dir)) {
            return Err(format!("runbooks: {} is listed twice ({})", dir.display(), first.display()));
        }
        dirs.push((dir.clone(), false));
    }
    if let Some(root) = built_in {
        if !dirs.iter().any(|(d, _)| canon(d) == canon(root)) {
            dirs.push((root.to_path_buf(), true));
        }
    }
    Ok(dirs)
}

/// Every protocol of the configured directories and of the built-in root, in directory order, each
/// directory's runbooks before its children and by name. An error names the directory or the two
/// directories it is about.
pub fn load(configured: &[PathBuf], built_in: Option<&Path>) -> Result<Vec<Entry>, String> {
    let mut all: Vec<(Entry, Vec<String>)> = Vec::new();
    for (dir, is_built_in) in order(configured, built_in)? {
        if !is_built_in {
            check_dir(&dir)?;
        }
        all.extend(read_dir(&dir, is_built_in)?);
    }
    let mut seen: BTreeMap<&str, &Entry> = BTreeMap::new();
    for (e, _) in &all {
        if let Some(first) = seen.insert(e.id.as_str(), e) {
            return Err(format!("the runbook {} is in two directories: {} and {}", e.id, first.dir.display(), e.dir.display()));
        }
    }
    // What a protocol opens is read from the set the protocol belongs to: a built-in protocol is a child only
    // when a built-in protocol opens it, so a configured runbook cannot turn a built-in one into a child.
    let opened_by = |only_built_in: bool| -> BTreeSet<String> { all.iter().filter(|(e, _)| e.built_in || !only_built_in).flat_map(|(_, opens)| opens.iter().cloned()).collect() };
    let (opened_built_in, opened_all) = (opened_by(true), opened_by(false));
    let mut entries: Vec<Entry> = all
        .into_iter()
        .map(|(mut e, _)| {
            e.startable = !(if e.built_in { &opened_built_in } else { &opened_all }).contains(&e.protocol_id);
            e
        })
        .collect();
    let rank = |d: &Path| configured.iter().position(|c| c == d).unwrap_or(configured.len());
    entries.sort_by(|a, b| rank(&a.dir).cmp(&rank(&b.dir)).then_with(|| b.startable.cmp(&a.startable)).then_with(|| a.id.cmp(&b.id)));
    Ok(entries)
}

/// The runbooks a person can start, by name, in the order of `load`.
pub fn startable(entries: &[Entry]) -> Vec<&str> {
    entries.iter().filter(|e| e.startable).map(|e| e.id.as_str()).collect()
}

/// The entry named `name`.
pub fn find<'a>(entries: &'a [Entry], name: &str) -> Option<&'a Entry> {
    entries.iter().find(|e| e.id == name)
}

/// The directories searched, for a message: the configured ones and the built-in root.
pub fn searched(configured: &[PathBuf], built_in: Option<&Path>) -> String {
    let mut all: Vec<String> = configured.iter().map(|d| d.display().to_string()).collect();
    all.extend(built_in.map(|d| format!("{} (built-in)", d.display())));
    if all.is_empty() {
        "no directory".to_owned()
    } else {
        all.join(", ")
    }
}

/// The directory of `homes` that has `protocol/<name>.protocol.yaml`: none, one, or an error when two
/// do. The rule of `load` (a name is in one directory, and a name of the built-in set, the last home, is
/// never taken from a configured one), applied again where the daemon's engine resolves a name at each
/// submit: the directories can change after `load` accepted the set at the start. The error names the
/// runbook and no directory (it reaches a client).
pub fn locate(homes: &[PathBuf], name: &str) -> Result<Option<PathBuf>, String> {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let mut has: Vec<&PathBuf> = Vec::new();
    for home in homes.iter().filter(|h| h.join("protocol").join(format!("{name}.protocol.yaml")).is_file()) {
        if !has.iter().any(|seen| canon(seen) == canon(home)) {
            has.push(home);
        }
    }
    match has.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some((*one).clone())),
        _ => Err(format!("the runbook {name} is in two runbook directories: a name is in one directory only, and a name of the built-in set is never taken from a configured directory")),
    }
}

/// The protocols of one `protocol/` directory the caller named (`--protocols`), with `startable` read
/// from that directory's own files.
pub fn read_protocols(protocols: &Path) -> Result<Vec<Entry>, String> {
    let dir = protocols.parent().unwrap_or(protocols);
    let read = read_files(protocols, dir, false)?;
    let opened: BTreeSet<String> = read.iter().flat_map(|(_, opens)| opens.iter().cloned()).collect();
    Ok(read
        .into_iter()
        .map(|(mut e, _)| {
            e.startable = !opened.contains(&e.protocol_id);
            e
        })
        .collect())
}

fn read_dir(dir: &Path, built_in: bool) -> Result<Vec<(Entry, Vec<String>)>, String> {
    read_files(&dir.join("protocol"), dir, built_in)
}

fn read_files(protocols: &Path, dir: &Path, built_in: bool) -> Result<Vec<(Entry, Vec<String>)>, String> {
    let rd = std::fs::read_dir(protocols).map_err(|e| format!("{}: {e}", protocols.display()))?;
    let mut out = Vec::new();
    for ent in rd {
        let ent = ent.map_err(|e| e.to_string())?;
        let name = ent.file_name().to_string_lossy().into_owned();
        let Some(id) = name.strip_suffix(".protocol.yaml") else { continue };
        if !ent.path().is_file() {
            continue;
        }
        let text = std::fs::read_to_string(ent.path()).map_err(|e| format!("{}: {e}", ent.path().display()))?;
        let doc: serde_yaml::Value = serde_yaml::from_str(&text).map_err(|e| format!("{}: {e}", ent.path().display()))?;
        let p = &doc["protocol"];
        let opens: Vec<String> = doc["actions"]
            .as_mapping()
            .into_iter()
            .flat_map(|m| m.values())
            .filter_map(|a| a["case_open"]["protocol"].as_str().map(str::to_owned))
            .collect();
        out.push((
            Entry {
                id: id.to_string(),
                protocol_id: p["id"].as_str().unwrap_or(id).to_string(),
                revision: p["revision"].as_u64(),
                startable: true,
                title: title_of(&doc),
                dir: dir.to_path_buf(),
                built_in,
            },
            opens,
        ));
    }
    Ok(out)
}

/// `protocol.title` or `title` when the file carries one, else the first sentence of
/// `protocol.description` (the files carry only that), cut at a word near `TITLE_MAX`.
pub fn title_of(doc: &serde_yaml::Value) -> String {
    let p = &doc["protocol"];
    if let Some(t) = p["title"].as_str().or_else(|| doc["title"].as_str()) {
        return t.trim().to_string();
    }
    let d = p["description"].as_str().unwrap_or("");
    let line = d.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let first = match line.find(". ") {
        Some(i) => &line[..i],
        None => line.trim_end_matches('.'),
    };
    if first.chars().count() <= TITLE_MAX {
        return first.to_string();
    }
    let cut: String = first.chars().take(TITLE_MAX).collect();
    let cut = cut.rsplit_once(' ').map(|(a, _)| a).unwrap_or(&cut);
    format!("{}...", cut.trim_end_matches([',', ':', ';']))
}

/// The checks of a configured directory: it is a directory, it holds `protocol/` and `bindings/`, and
/// below `protocol/`, `bindings/`, `views/` and `instructions/` every entry is a regular file or a
/// directory, and a link stays inside the directory and is not dangling. Nothing is opened for
/// reading but directories, and nothing is executed.
pub fn check_dir(dir: &Path) -> Result<(), String> {
    let shown = dir.display();
    let meta = std::fs::metadata(dir).map_err(|e| format!("runbooks: {shown}: {}", cause(&e)))?;
    if !meta.is_dir() {
        return Err(format!("runbooks: {shown} is not a directory"));
    }
    let root = std::fs::canonicalize(dir).map_err(|e| format!("runbooks: {shown}: {}", cause(&e)))?;
    for needed in ["protocol", "bindings"] {
        let inside = std::fs::canonicalize(dir.join(needed)).ok().filter(|c| c.starts_with(&root) && c.is_dir());
        if inside.is_none() {
            return Err(format!("runbooks: {shown} holds no {needed}/ directory inside it (a runbook directory holds protocol/ and bindings/)"));
        }
    }
    let mut budget = WALK_ENTRIES;
    for sub in CHECKED {
        let path = dir.join(sub);
        if std::fs::symlink_metadata(&path).is_ok() {
            walk(&root, &path, 0, &mut budget)?;
        }
    }
    Ok(())
}

fn cause(e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::NotFound => "no such directory".to_owned(),
        std::io::ErrorKind::PermissionDenied => "permission denied".to_owned(),
        _ => e.to_string(),
    }
}

fn walk(root: &Path, dir: &Path, depth: usize, budget: &mut usize) -> Result<(), String> {
    let rd = std::fs::read_dir(dir).map_err(|e| format!("runbooks: {}: {}", dir.display(), cause(&e)))?;
    for ent in rd {
        let ent = ent.map_err(|e| format!("runbooks: {}: {}", dir.display(), cause(&e)))?;
        *budget = budget.checked_sub(1).ok_or_else(|| format!("runbooks: {} holds more than {WALK_ENTRIES} entries", root.display()))?;
        let path = ent.path();
        let meta = std::fs::symlink_metadata(&path).map_err(|e| format!("runbooks: {}: {}", path.display(), cause(&e)))?;
        let kind = meta.file_type();
        if kind.is_symlink() {
            let target = std::fs::canonicalize(&path).map_err(|_| format!("runbooks: {} is a dangling link", path.display()))?;
            if !target.starts_with(root) {
                return Err(format!("runbooks: {} is a link out of the directory {} (a runbook directory holds its files itself)", path.display(), root.display()));
            }
        } else if kind.is_dir() {
            if depth >= WALK_DEPTH {
                return Err(format!("runbooks: {} is nested deeper than {WALK_DEPTH} directories", path.display()));
            }
            walk(root, &path, depth + 1, budget)?;
        } else if !kind.is_file() {
            return Err(format!("runbooks: {} is not a regular file", path.display()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch directory under the build's target directory.
    fn scratch(tag: &str) -> PathBuf {
        let exe = std::env::current_exe().unwrap();
        let d = exe.parent().unwrap().parent().unwrap().join("runbook-test-tmp").join(format!("runbooks-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn put(dir: &Path, rel: &str, text: &str) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    #[test]
    fn title_is_the_first_sentence_and_short() {
        let doc: serde_yaml::Value = serde_yaml::from_str("protocol:\n  id: a\n  description: >-\n    Fix the thing (docs/x.md steps 4 to 6). More text.\n").unwrap();
        assert_eq!(title_of(&doc), "Fix the thing (docs/x.md steps 4 to 6)");
        let long = format!("protocol:\n  description: {}\n", "word ".repeat(60));
        let doc: serde_yaml::Value = serde_yaml::from_str(&long).unwrap();
        let t = title_of(&doc);
        assert!(t.ends_with("...") && t.chars().count() <= TITLE_MAX + 3, "{t}");
    }

    #[test]
    fn a_title_key_wins_other_files_are_ignored_and_a_protocol_nobody_opens_is_a_runbook() {
        let d = scratch("title");
        put(&d, "protocol/sdc.protocol.yaml", "protocol:\n  id: sdc\n  revision: 3\n  title: The cycle\n  description: Long.\nactions:\n  a.open:\n    case_open: {protocol: sdc.story, subject: idea}\n");
        put(&d, "protocol/sdc-story.protocol.yaml", "protocol:\n  id: sdc.story\n  description: Deliver one story. More.\n");
        put(&d, "protocol/README.md", "x");
        put(&d, "protocol/draft/orchestrate.protocol.yaml", "protocol:\n  id: o\n");
        put(&d, "bindings/sdc.yaml", "x: 1\n");
        let e = load(&[], Some(&d)).unwrap();
        assert_eq!(e.len(), 2);
        assert_eq!((e[0].id.as_str(), e[0].startable, e[0].title.as_str(), e[0].revision, e[0].built_in), ("sdc", true, "The cycle", Some(3), true));
        assert_eq!((e[1].id.as_str(), e[1].startable, e[1].title.as_str()), ("sdc-story", false, "Deliver one story"));
        assert_eq!(startable(&e), ["sdc"]);
    }

    #[test]
    fn the_configured_directories_come_first_and_a_name_in_two_is_an_error_naming_both() {
        let root = scratch("order");
        let (a, b, built_in) = (root.join("a"), root.join("b"), root.join("built-in"));
        for (d, id) in [(&a, "alpha"), (&b, "beta"), (&built_in, "gamma")] {
            put(d, &format!("protocol/{id}.protocol.yaml"), &format!("protocol:\n  id: {id}\n  revision: 1\n"));
            put(d, &format!("bindings/{id}.yaml"), "x: 1\n");
        }
        let e = load(&[b.clone(), a.clone()], Some(&built_in)).unwrap();
        assert_eq!(e.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["beta", "alpha", "gamma"]);
        put(&b, "protocol/alpha.protocol.yaml", "protocol:\n  id: alpha\n  revision: 2\n");
        let said = load(&[a.clone(), b.clone()], Some(&built_in)).unwrap_err();
        assert!(said.contains("alpha") && said.contains(a.to_str().unwrap()) && said.contains(b.to_str().unwrap()), "{said}");
        let twice = load(&[a.clone(), a.clone()], None).unwrap_err();
        assert!(twice.contains("listed twice"), "{twice}");
        // the built-in root named in the list is one directory, not a collision
        assert!(load(std::slice::from_ref(&built_in), Some(&built_in)).is_ok());
    }

    #[test]
    fn a_directory_that_is_missing_has_no_part_or_leads_out_is_refused_by_name() {
        let root = scratch("refuse");
        let d = root.join("d");
        assert!(load(std::slice::from_ref(&d), None).unwrap_err().contains(d.to_str().unwrap()));
        put(&d, "protocol/a.protocol.yaml", "protocol:\n  id: a\n  revision: 1\n");
        let said = load(std::slice::from_ref(&d), None).unwrap_err();
        assert!(said.contains(d.to_str().unwrap()) && said.contains("bindings/"), "{said}");
        put(&d, "bindings/a.yaml", "x: 1\n");
        assert!(load(std::slice::from_ref(&d), None).is_ok());
        put(&root, "outside.yaml", "x: 1\n");
        std::os::unix::fs::symlink(root.join("outside.yaml"), d.join("bindings/out.yaml")).unwrap();
        let said = load(std::slice::from_ref(&d), None).unwrap_err();
        assert!(said.contains("out.yaml") && said.contains("link"), "{said}");
    }
}
