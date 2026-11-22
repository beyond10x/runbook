//! sdc-check-unreferenced: files in the checked-in tree that no other file refers to, and the files
//! that would become so when some files are deleted. The rules are in `HELP` below, which is what
//! `--help` prints, and in docs/releasing.md ("The unreferenced-files check").
use clap::Parser;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const SCOPE: &[&str] = &[
    "scripts/",
    "bindings/",
    "instructions/",
    "cycles/",
    "driver/tests/fixtures/",
    "cases/",
    "tools/",
    "watch/",
    "views/",
    "decision/",
    "goals/",
    "eval/",
];

const NOT_A_REFERRER: &[&str] = &[".engineering/", "CHANGELOG.md", "docs/releases/", "experiments/"];

const CONVENTION: &[&str] = &["Cargo.toml", "Cargo.lock", "package.json", "package-lock.json", ".gitignore"];

const HELP: &str = "Lists the files nothing refers to.

INPUT
  The tracked files (`git ls-files` in --root, or the lines of --files-from, paths relative to
  --root). A listed file that is not on disk does not count.

SCOPE (the files that can be reported)
  scripts/ bindings/ instructions/ cycles/ driver/tests/fixtures/ cases/ tools/ watch/ views/
  decision/ goals/ eval/
  Left out, with the reason:
    docs/ website/ plugin/ README.md AGENTS.md  prose and published pages: read by people
    ui/                                          a Node project with its own entry points and tests
    runbook/ driver/src driver/examples          crates and bindings whose entry points are Cargo
                                                 targets and `mod` declarations of their own
    driver/tests/*.rs                            test modules, listed by tests/main.rs
    ess/ protocol/                               specification and protocol sources read by their tools
                                                 as whole trees
    experiments/ .engineering/ .github/          records, planning store and forge files
  Cargo.toml, Cargo.lock, package.json, package-lock.json and .gitignore are read by their tool by
  convention and are never reported.

REFERRERS (the files whose text counts)
  Every tracked text file except: .engineering/, experiments/, CHANGELOG.md, docs/releases/ (a
  history of what was once true), the two data files below, binary files and files over 4 MB. A
  file does not refer to itself.

WHEN A FILE IS REFERRED TO
  Text is cut into path tokens: runs of letters, digits and `_ . - / @ + ~`. A target file F is
  referred to by a referrer R when one of these holds.
  1. path        a token of R is the whole path of F from the repository root, with an optional
                 leading `./` or `$VAR/`.
  2. relative    a token of R, resolved against the directory of R or against the root of the
                 nearest Cargo.toml above R (the `..` and `.` segments applied), is the path of F.
                 For a .ts, .mjs or .js file R, a token without extension also matches F with
                 .ts, .mjs, .js or /index.ts added.
  3. dir + name  R names an ancestor directory A of F (a token of R with a `/` has A as a run of
                 its segments, or a token resolved as in 2 starts with A; the directory of R and the
                 root of its crate count as named) and, anywhere in R, F's path below A (a bare
                 file name when F lies directly in A). This covers a script that sets a directory
                 once and builds the file names later.
  4. mod         R is a .rs file with `mod x;`: x.rs or x/mod.rs beside R (and below R's own name
                 for a file that is not main, lib or mod).
  5. cargo       R is a Cargo.toml: src/main.rs, src/lib.rs, src/bin/*.rs, src/bin/*/main.rs,
                 tests/*.rs, tests/*/main.rs, examples/*.rs and benches/*.rs of its directory,
                 unless it sets the matching `autobins`, `autotests`, `autoexamples` or
                 `autobenches` to false.
  6. whole       a directory read as a whole (a glob, a directory walk): a line of the --whole file,
                 `<dir> <referrer> <reason>`; every file below <dir> is referred to by <referrer>.
                 The check refuses a line whose directory has no tracked file, whose referrer is
                 not a referrer, or whose referrer has no token with the directory's own name (its
                 last segment) as a path segment.

OUTPUT
  One path per line, sorted. --format json: an array of {\"path\", \"named_by\"}.

MODES
  (none)       every unreferenced file in scope that the --keep file does not list. Exit 0 with an
               empty list, 1 with files, 2 on an error or a refused data line.
               --keep file: `<path> <reason>` per line, no comments. The check refuses a line whose
               path is not a tracked file in scope, is referred to now, repeats, or has no reason.
  --without P  the files as if the paths P (one or more) were deleted: their references do not
               count. Prints only the files that become unreferenced because of it (they had a
               referrer; every referrer is in P), each with the files of P that name it: `path<TAB>a,b`.
               The --keep file is not read. Exit 0.
  The default data files are driver/unreferenced-keep.txt and driver/unreferenced-whole.txt under
  --root, read when present.";

#[derive(Parser)]
#[command(name = "sdc-check-unreferenced", about = "Files that nothing in the tree refers to", long_about = HELP, after_help = "")]
struct Cli {
    /// The repository root.
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// A file with one tracked path per line, instead of `git ls-files`.
    #[arg(long)]
    files_from: Option<PathBuf>,
    /// Files whose deletion is assumed (paths relative to --root).
    #[arg(long, num_args = 1..)]
    without: Vec<String>,
    /// text or json.
    #[arg(long, default_value = "text")]
    format: String,
    /// The keep file, relative to --root.
    #[arg(long)]
    keep: Option<String>,
    /// The whole-directory file, relative to --root.
    #[arg(long)]
    whole: Option<String>,
}

const DEFAULT_KEEP: &str = "driver/unreferenced-keep.txt";
const DEFAULT_WHOLE: &str = "driver/unreferenced-whole.txt";

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("sdc-check-unreferenced: {e}");
            ExitCode::from(2)
        }
    }
}

fn is_path_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-' | b'/' | b'@' | b'+' | b'~')
}

fn tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii() && is_path_byte(c as u8))).filter(|t| t.len() > 1)
}

fn dir_of(p: &str) -> &str {
    p.rfind('/').map_or("", |i| &p[..i])
}

fn base_of(p: &str) -> &str {
    p.rfind('/').map_or(p, |i| &p[i + 1..])
}

fn resolve(base: &str, segs: &[&str]) -> Option<String> {
    let mut stack: Vec<&str> = if base.is_empty() { vec![] } else { base.split('/').collect() };
    for s in segs {
        match *s {
            "" | "." => {}
            ".." => {
                stack.pop()?;
            }
            x => stack.push(x),
        }
    }
    if stack.is_empty() {
        None
    } else {
        Some(stack.join("/"))
    }
}

fn read_text(root: &Path, p: &str) -> Option<String> {
    let bytes = std::fs::read(root.join(p)).ok()?;
    if bytes.len() > 4_000_000 || bytes.iter().take(8000).any(|b| *b == 0) {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

struct Ctx {
    targets: Vec<String>,
    target_ix: HashMap<String, usize>,
    basenames: HashSet<String>,
    dirs: HashSet<String>,
    rels: HashSet<String>,
    under: HashMap<String, Vec<usize>>,
    cargo_dirs: HashSet<String>,
}

fn rel<'a>(t: &'a str, a: &str) -> &'a str {
    &t[a.len() + 1..]
}

fn build_ctx(files: &[String]) -> Ctx {
    let targets: Vec<String> = files.iter().filter(|f| SCOPE.iter().any(|s| f.starts_with(s))).cloned().collect();
    let mut target_ix = HashMap::new();
    let mut basenames = HashSet::new();
    let mut dirs = HashSet::new();
    let mut rels = HashSet::new();
    let mut under: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, t) in targets.iter().enumerate() {
        target_ix.insert(t.clone(), i);
        basenames.insert(base_of(t).to_string());
        let mut a = dir_of(t);
        while !a.is_empty() {
            dirs.insert(a.to_string());
            rels.insert(rel(t, a).to_string());
            under.entry(a.to_string()).or_default().push(i);
            a = dir_of(a);
        }
    }
    let cargo_dirs = files.iter().filter(|f| base_of(f) == "Cargo.toml").map(|f| dir_of(f).to_string()).collect();
    Ctx { targets, target_ix, basenames, dirs, rels, under, cargo_dirs }
}

fn crate_root<'a>(p: &'a str, ctx: &Ctx) -> Option<&'a str> {
    let mut d = dir_of(p);
    loop {
        if ctx.cargo_dirs.contains(d) {
            return Some(d);
        }
        if d.is_empty() {
            return None;
        }
        d = dir_of(d);
    }
}

/// The indexes of the targets that the referrer `path` with `text` refers to.
fn scan(path: &str, text: &str, ctx: &Ctx) -> Vec<usize> {
    let dr = dir_of(path);
    let cr = crate_root(path, ctx);
    let script_like = path.ends_with(".ts") || path.ends_with(".mjs") || path.ends_with(".js");
    let mut hits: HashSet<usize> = HashSet::new();
    let mut dirs_named: HashSet<String> = HashSet::new();
    let mut rels_named: HashSet<String> = HashSet::new();
    let mut bases: Vec<&str> = vec![dr];
    if let Some(c) = cr {
        if c != dr {
            bases.push(c);
        }
    }
    let try_exact = |cand: &str, hits: &mut HashSet<usize>| {
        if let Some(&i) = ctx.target_ix.get(cand) {
            hits.insert(i);
        }
    };
    for tok in tokens(text) {
        let tok = tok.trim_end_matches('.');
        if tok.len() < 2 {
            continue;
        }
        if !tok.contains('/') {
            if ctx.rels.contains(tok) {
                rels_named.insert(tok.to_string());
            }
            let base = tok;
            if ctx.basenames.contains(base) {
                for b in &bases {
                    try_exact(&format!("{b}/{base}"), &mut hits);
                }
            } else if script_like {
                for ext in [".ts", ".mjs", ".js"] {
                    let name = format!("{tok}{ext}");
                    if ctx.basenames.contains(&name) {
                        for b in &bases {
                            try_exact(&format!("{b}/{name}"), &mut hits);
                        }
                    }
                }
            }
            continue;
        }
        let segs: Vec<&str> = tok.split('/').collect();
        let own: Vec<&str> = segs.iter().copied().filter(|s| !s.is_empty() && *s != ".").collect();
        if own.first() != Some(&"..") {
            try_exact(&own.join("/"), &mut hits);
        }
        let mut cands: Vec<String> = Vec::new();
        if own.first() != Some(&"..") {
            cands.push(own.join("/"));
        }
        for b in &bases {
            if let Some(r) = resolve(b, &segs) {
                cands.push(r);
            }
        }
        for c in &cands {
            try_exact(c, &mut hits);
            if script_like {
                for ext in [".ts", ".mjs", ".js", "/index.ts"] {
                    try_exact(&format!("{c}{ext}"), &mut hits);
                }
            }
            let mut a = dir_of(c);
            while !a.is_empty() {
                if ctx.dirs.contains(a) {
                    dirs_named.insert(a.to_string());
                }
                a = dir_of(a);
            }
        }
        for run in segs.split(|s| *s == "..") {
            let run: Vec<&str> = run.iter().copied().filter(|s| !s.is_empty() && *s != ".").collect();
            let n = run.len().min(24);
            for i in 0..n {
                for j in i + 1..=n {
                    let key = run[i..j].join("/");
                    if ctx.dirs.contains(&key) {
                        dirs_named.insert(key.clone());
                    }
                    if ctx.rels.contains(&key) {
                        rels_named.insert(key);
                    }
                }
            }
        }
    }
    dirs_named.insert(dr.to_string());
    if let Some(c) = cr {
        dirs_named.insert(c.to_string());
    }
    for a in &dirs_named {
        if let Some(list) = ctx.under.get(a) {
            for &t in list {
                if rels_named.contains(rel(&ctx.targets[t], a)) {
                    hits.insert(t);
                }
            }
        }
    }
    if path.ends_with(".rs") {
        let stem = base_of(path).trim_end_matches(".rs");
        for line in text.lines() {
            let mut t = line.trim_start();
            for p in ["pub(crate) ", "pub(super) ", "pub "] {
                t = t.strip_prefix(p).unwrap_or(t);
            }
            let Some(rest) = t.strip_prefix("mod ") else { continue };
            let Some(name) = rest.split(';').next().filter(|_| rest.contains(';')) else { continue };
            let name = name.trim();
            if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                continue;
            }
            let mut homes = vec![dr.to_string()];
            if !matches!(stem, "main" | "lib" | "mod") {
                homes.push(format!("{dr}/{stem}"));
            }
            for h in homes {
                let h = h.trim_start_matches('/');
                try_exact(&format!("{h}/{name}.rs"), &mut hits);
                try_exact(&format!("{h}/{name}/mod.rs"), &mut hits);
            }
        }
    }
    if base_of(path) == "Cargo.toml" {
        let flat: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let auto = |k: &str| !flat.contains(&format!("auto{k}=false"));
        if let Some(list) = ctx.under.get(dr) {
            for &t in list {
                let r = rel(&ctx.targets[t], dr);
                let parts: Vec<&str> = r.split('/').collect();
                let found = match parts.as_slice() {
                    ["src", "main.rs"] | ["src", "lib.rs"] => true,
                    ["src", "bin", f] => auto("bins") && f.ends_with(".rs"),
                    ["src", "bin", _, "main.rs"] => auto("bins"),
                    ["tests", f] => auto("tests") && f.ends_with(".rs"),
                    ["tests", _, "main.rs"] => auto("tests"),
                    ["examples", f] => auto("examples") && f.ends_with(".rs"),
                    ["benches", f] => auto("benches") && f.ends_with(".rs"),
                    _ => false,
                };
                if found {
                    hits.insert(t);
                }
            }
        }
    }
    hits.into_iter().collect()
}

fn names_dir(text: &str, dir: &str) -> bool {
    let last = base_of(dir);
    tokens(text).any(|tok| tok.split('/').any(|s| s == last))
}

fn read_lines(root: &Path, rel_path: &str, required: bool) -> Result<Vec<String>, String> {
    match std::fs::read_to_string(root.join(rel_path)) {
        Ok(s) => Ok(s.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect()),
        Err(e) if required => Err(format!("{rel_path}: {e}")),
        Err(_) => Ok(vec![]),
    }
}

struct Whole {
    dir: String,
    referrer: String,
}

fn run() -> Result<u8, String> {
    let cli = Cli::parse();
    if cli.format != "text" && cli.format != "json" {
        return Err(format!("--format is text or json, not {}", cli.format));
    }
    let root = cli.root.clone();
    let mut files: Vec<String> = match &cli.files_from {
        Some(f) => std::fs::read_to_string(f)
            .map_err(|e| format!("{}: {e}", f.display()))?
            .lines()
            .map(|l| l.trim().trim_start_matches("./").to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        None => {
            let out = Command::new("git").arg("-C").arg(&root).args(["ls-files", "-z"]).output().map_err(|e| format!("git ls-files: {e}"))?;
            if !out.status.success() {
                return Err(format!("git ls-files failed in {}", root.display()));
            }
            String::from_utf8_lossy(&out.stdout).split('\0').filter(|l| !l.is_empty()).map(String::from).collect()
        }
    };
    files.sort();
    files.dedup();
    files.retain(|f| root.join(f).is_file());
    let keep_path = cli.keep.clone().unwrap_or_else(|| DEFAULT_KEEP.into());
    let whole_path = cli.whole.clone().unwrap_or_else(|| DEFAULT_WHOLE.into());
    for p in &cli.without {
        if !files.iter().any(|f| f == p) {
            return Err(format!("--without {p}: not a tracked file"));
        }
    }
    let ctx = build_ctx(&files);
    let referrers: Vec<&String> = files
        .iter()
        .filter(|f| !NOT_A_REFERRER.iter().any(|n| f.starts_with(n)) && **f != keep_path && **f != whole_path)
        .collect();
    let whole_lines = read_lines(&root, &whole_path, cli.whole.is_some())?;
    let wanted_evidence: HashSet<&str> = whole_lines.iter().filter_map(|l| l.split_whitespace().nth(1)).collect();
    let threads = std::thread::available_parallelism().map_or(2, |n| n.get()).min(8);
    let texts: Vec<(usize, Vec<usize>, Option<String>)> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|k| {
                let (referrers, ctx, root, wanted) = (&referrers, &ctx, &root, &wanted_evidence);
                s.spawn(move || {
                    let mut out = Vec::new();
                    for (i, r) in referrers.iter().enumerate().filter(|(i, _)| i % threads == k) {
                        match read_text(root, r) {
                            Some(t) => {
                                let h = scan(r, &t, ctx);
                                out.push((i, h, wanted.contains(r.as_str()).then_some(t)));
                            }
                            None => out.push((i, vec![], None)),
                        }
                    }
                    out
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });
    let mut refs: Vec<BTreeSet<String>> = vec![BTreeSet::new(); ctx.targets.len()];
    let mut text_of: HashMap<&str, String> = HashMap::new();
    for (i, hits, text) in texts {
        let r = referrers[i];
        for t in hits {
            if ctx.targets[t] != *r {
                refs[t].insert(r.clone());
            }
        }
        if let Some(t) = text {
            text_of.insert(r.as_str(), t);
        }
    }
    let mut errors: Vec<String> = Vec::new();
    let mut wholes: Vec<Whole> = Vec::new();
    for (n, l) in whole_lines.iter().enumerate() {
        let mut it = l.splitn(3, char::is_whitespace);
        let (dir, referrer, reason) = (it.next().unwrap_or(""), it.next().unwrap_or(""), it.next().unwrap_or("").trim());
        let at = format!("{whole_path}:{}", n + 1);
        if reason.is_empty() {
            errors.push(format!("{at}: `<dir> <referrer> <reason>` needs all three"));
            continue;
        }
        if !files.iter().any(|f| f.starts_with(&format!("{dir}/"))) {
            errors.push(format!("{at}: {dir} has no tracked file"));
            continue;
        }
        if !referrers.iter().any(|r| r.as_str() == referrer) {
            errors.push(format!("{at}: {referrer} is not a referrer (missing, or a file that does not count)"));
            continue;
        }
        if !text_of.get(referrer).is_some_and(|t| names_dir(t, dir)) {
            errors.push(format!("{at}: {referrer} has no token with the directory's own name ({}) as a path segment", base_of(dir)));
            continue;
        }
        wholes.push(Whole { dir: dir.to_string(), referrer: referrer.to_string() });
    }
    for w in &wholes {
        let prefix = format!("{}/", w.dir);
        for (i, t) in ctx.targets.iter().enumerate() {
            if t.starts_with(&prefix) && *t != w.referrer {
                refs[i].insert(w.referrer.clone());
            }
        }
    }
    let convention = |p: &str| CONVENTION.contains(&base_of(p));
    let without: BTreeSet<&str> = cli.without.iter().map(String::as_str).collect();
    // (path, named_by)
    let mut found: Vec<(String, Vec<String>)> = Vec::new();
    if !without.is_empty() {
        for (i, t) in ctx.targets.iter().enumerate() {
            if without.contains(t.as_str()) || convention(t) || refs[i].is_empty() {
                continue;
            }
            if refs[i].iter().all(|r| without.contains(r.as_str())) {
                found.push((t.clone(), refs[i].iter().cloned().collect()));
            }
        }
    } else {
        let keep_lines = read_lines(&root, &keep_path, cli.keep.is_some())?;
        let mut kept: BTreeMap<String, usize> = BTreeMap::new();
        for (n, l) in keep_lines.iter().enumerate() {
            let (path, reason) = l.split_once(char::is_whitespace).map_or((l.as_str(), ""), |(a, b)| (a, b.trim()));
            let at = format!("{keep_path}:{}", n + 1);
            if reason.is_empty() {
                errors.push(format!("{at}: `<path> <reason>` needs a reason"));
            }
            if kept.insert(path.to_string(), n + 1).is_some() {
                errors.push(format!("{at}: {path} is listed twice"));
            }
            match ctx.target_ix.get(path) {
                None => errors.push(format!("{at}: {path} is not a tracked file in scope (deleted, or never was)")),
                Some(&t) if !refs[t].is_empty() => {
                    let by: Vec<&str> = refs[t].iter().map(String::as_str).take(3).collect();
                    errors.push(format!("{at}: {path} is referred to now (by {}); remove the line", by.join(", ")));
                }
                _ => {}
            }
        }
        for (i, t) in ctx.targets.iter().enumerate() {
            if refs[i].is_empty() && !convention(t) && !kept.contains_key(t) {
                found.push((t.clone(), vec![]));
            }
        }
    }
    if !errors.is_empty() {
        for e in &errors {
            eprintln!("{e}");
        }
        return Ok(2);
    }
    found.sort();
    if cli.format == "json" {
        let arr: Vec<serde_json::Value> = found.iter().map(|(p, by)| serde_json::json!({"path": p, "named_by": by})).collect();
        println!("{}", serde_json::to_string_pretty(&arr).map_err(|e| e.to_string())?);
    } else {
        for (p, by) in &found {
            if without.is_empty() {
                println!("{p}");
            } else {
                println!("{p}\t{}", by.join(","));
            }
        }
    }
    Ok(if without.is_empty() && !found.is_empty() { 1 } else { 0 })
}
