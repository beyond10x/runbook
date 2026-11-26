//! sdc-check-names: no line this repository adds names a person, an organisation or a session. The
//! refused words are kept as SHA-256 digests (`driver/refused-names.txt`), never as words, and a finding
//! prints the file, the line, the class and a digest prefix, never the word. The rules are in `HELP`.
use clap::Parser;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const HELP: &str = "Refuses a line that names a person, an organisation or a session.

WORDS
  driver/refused-names.txt (built into the program): one line per refused word, the SHA-256 (hex) of
  the word lower-cased with its parts joined by `-`, then its class (person, organisation, session).
  Lines starting with `#` are comments. The file never holds a word.

WHEN A LINE IS REFUSED
  A line is cut into compound words: runs of letters, digits, `-` and `_`. A compound word is cut at
  `-` and `_` into parts. Every run of 1 to 4 consecutive parts, lower-cased and joined by `-`, is
  hashed; a line is refused when one of those digests is listed. So a session name is found inside a
  longer hyphenated word (`<name>-2026-11-23`), and a word written in capitals is found too.

WHICH LINES (default: a change)
  The lines `git diff <base>` adds (the working tree, committed or not, against <base>) and every line
  of a file git does not track yet and does not ignore (`git ls-files --others --exclude-standard`;
  a file that is gone or binary is skipped). <base> is the
  merge base of HEAD with --base (default origin/main); without --base, on a checkout that has no
  origin/main (the forge's pull-request checkout, a merge), it is the first parent of HEAD.
  --full checks every line of every tracked file (`git ls-files`, or the paths of --files-from);
  a file with a NUL byte in its first 8000 bytes is binary and skipped.

OUTPUT
  One line per refused line on stdout: `<path>:<line>: a refused <class> name (sha256 <12 hex>)`;
  a summary on stderr. Exit 0 none, 1 some, 2 an error.";

#[derive(Parser)]
#[command(name = "sdc-check-names", about = "Refuses a line that names a person, an organisation or a session.", long_about = HELP)]
struct Cli {
    /// The repository root.
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// Check every line of every tracked file instead of the lines a change adds.
    #[arg(long)]
    full: bool,
    /// The ref whose merge base with HEAD is the base of the change (default origin/main).
    #[arg(long, conflicts_with = "full")]
    base: Option<String>,
    /// With --full: a file with one path per line (relative to --root), instead of `git ls-files`.
    #[arg(long, requires = "full")]
    files_from: Option<PathBuf>,
}

const LIST: &str = include_str!("../../refused-names.txt");

/// The most parts of a compound word that one refused word may span.
const MAX_PARTS: usize = 4;

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("sdc-check-names: {e}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<u8, String> {
    let cli = Cli::parse();
    let list = parse_list(LIST)?;
    let mut lines = 0usize;
    let mut files = 0usize;
    let mut found = 0usize;
    let mut report = |path: &str, n: usize, text: &str| {
        lines += 1;
        if let Some((digest, class)) = refused(text, &list) {
            found += 1;
            println!("{path}:{n}: a refused {class} name (sha256 {})", &hex(digest)[..12]);
        }
    };
    let what = if cli.full {
        let paths = match &cli.files_from {
            Some(f) => std::fs::read_to_string(f)
                .map_err(|e| format!("{}: {e}", f.display()))?
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect(),
            None => {
                let out = git(&cli.root, &["ls-files", "-z"])?;
                String::from_utf8_lossy(&out).split('\0').filter(|l| !l.is_empty()).map(String::from).collect::<Vec<_>>()
            }
        };
        for p in &paths {
            files += usize::from(scan_file(&cli.root, p, &mut report));
        }
        "every tracked file".to_string()
    } else {
        let (base, how) = base(&cli.root, cli.base.as_deref())?;
        let out = git(
            &cli.root,
            &["-c", "core.quotePath=false", "diff", "--no-color", "--no-ext-diff", "--no-textconv", "-U0", "--src-prefix=a/", "--dst-prefix=b/", &base, "--"],
        )?;
        let mut path: Option<String> = None;
        let mut in_hunk = false;
        let mut at = 0usize;
        for raw in out.split(|b| *b == b'\n') {
            let l = String::from_utf8_lossy(raw);
            if l.starts_with("diff --git ") {
                in_hunk = false;
                path = None;
            } else if !in_hunk {
                if let Some(p) = l.strip_prefix("+++ ") {
                    path = p.strip_prefix("b/").map(|p| p.trim_matches('"').to_string());
                    files += usize::from(path.is_some());
                } else if let Some(h) = l.strip_prefix("@@ ") {
                    in_hunk = true;
                    at = hunk_start(h).ok_or_else(|| format!("git diff: a hunk header it cannot read: {l}"))?;
                }
            } else if let Some(h) = l.strip_prefix("@@ ") {
                at = hunk_start(h).ok_or_else(|| format!("git diff: a hunk header it cannot read: {l}"))?;
            } else if let Some(text) = l.strip_prefix('+') {
                if let Some(p) = &path {
                    report(p, at, text);
                }
                at += 1;
            } else if l.starts_with(' ') {
                at += 1;
            }
        }
        let out = git(&cli.root, &["ls-files", "-z", "--others", "--exclude-standard"])?;
        let mut untracked: Vec<String> = String::from_utf8_lossy(&out).split('\0').filter(|l| !l.is_empty()).map(String::from).collect();
        untracked.sort();
        for p in &untracked {
            files += usize::from(scan_file(&cli.root, p, &mut report));
        }
        format!("the lines added since {base} ({how}) and the files git does not track yet")
    };
    let line_word = if lines == 1 { "line" } else { "lines" };
    eprintln!("sdc-check-names: {what}: {lines} {line_word} of {files} files checked, {found} refused");
    Ok(u8::from(found > 0))
}

/// Every line of one file; false for a file that is gone or binary (a NUL in its first 8000 bytes).
fn scan_file(root: &Path, path: &str, report: &mut impl FnMut(&str, usize, &str)) -> bool {
    let Ok(bytes) = std::fs::read(root.join(path)) else { return false };
    if bytes[..bytes.len().min(8000)].contains(&0) {
        return false;
    }
    for (i, l) in String::from_utf8_lossy(&bytes).lines().enumerate() {
        report(path, i + 1, l);
    }
    true
}

/// The new-side start line of a hunk header's rest, `-a,b +c,d @@ ...`.
fn hunk_start(rest: &str) -> Option<usize> {
    let plus = rest.split(' ').find(|w| w.starts_with('+'))?;
    plus[1..].split(',').next()?.parse().ok()
}

fn base(root: &Path, named: Option<&str>) -> Result<(String, String), String> {
    let upstream = named.unwrap_or("origin/main");
    let spec = format!("{upstream}^{{commit}}");
    if git(root, &["rev-parse", "--verify", "--quiet", &spec]).is_ok() {
        let mb = git(root, &["merge-base", "HEAD", upstream])?;
        return Ok((String::from_utf8_lossy(&mb).trim().to_string(), format!("the merge base of HEAD with {upstream}")));
    }
    if named.is_some() {
        return Err(format!("--base {upstream}: no such commit"));
    }
    let parent = git(root, &["rev-parse", "--verify", "--quiet", "HEAD^1^{commit}"])
        .map_err(|_| "no origin/main and HEAD has no parent: name the base with --base".to_string())?;
    Ok((String::from_utf8_lossy(&parent).trim().to_string(), "no origin/main: the first parent of HEAD".into()))
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new("git").arg("-C").arg(root).args(args).output().map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(out.stdout)
}

fn parse_list(text: &str) -> Result<HashMap<[u8; 32], String>, String> {
    let mut list = HashMap::new();
    for (i, l) in text.lines().enumerate() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let bad = || format!("refused-names.txt:{}: not `<sha256 hex> <person|organisation|session>`", i + 1);
        let (h, class) = l.split_once(' ').ok_or_else(bad)?;
        if !matches!(class, "person" | "organisation" | "session") || h.len() != 64 {
            return Err(bad());
        }
        let mut d = [0u8; 32];
        for (k, b) in d.iter_mut().enumerate() {
            *b = u8::from_str_radix(&h[2 * k..2 * k + 2], 16).map_err(|_| bad())?;
        }
        list.insert(d, class.to_string());
    }
    Ok(list)
}

/// The first refused word of a line: its digest and class.
fn refused<'a>(line: &str, list: &'a HashMap<[u8; 32], String>) -> Option<([u8; 32], &'a str)> {
    let mut parts: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in line.chars().chain(std::iter::once(' ')) {
        if c.is_alphanumeric() {
            cur.extend(c.to_lowercase());
            continue;
        }
        if !cur.is_empty() {
            parts.push(std::mem::take(&mut cur));
        }
        if c == '-' || c == '_' {
            continue;
        }
        if let Some(hit) = compound(&parts, list) {
            return Some(hit);
        }
        parts.clear();
    }
    None
}

fn compound<'a>(parts: &[String], list: &'a HashMap<[u8; 32], String>) -> Option<([u8; 32], &'a str)> {
    for i in 0..parts.len() {
        let mut word = String::new();
        for (k, p) in parts[i..parts.len().min(i + MAX_PARTS)].iter().enumerate() {
            if k > 0 {
                word.push('-');
            }
            word.push_str(p);
            let d: [u8; 32] = Sha256::digest(word.as_bytes()).into();
            if let Some(class) = list.get(&d) {
                return Some((d, class));
            }
        }
    }
    None
}

fn hex(d: [u8; 32]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}
