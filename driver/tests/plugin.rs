//! The `runbook` plugin (`plugin/`, `.claude-plugin/marketplace.json`): its manifests carry the
//! repository version, its marketplace entry points at `./plugin`, every skill has a description
//! that says when to use it and when not, and every repository path a `SKILL.md` names exists.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn json(path: &Path) -> Value {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The first `version = "…"` line of a Cargo manifest.
fn cargo_version(manifest: &str) -> Option<String> {
    manifest.lines().find_map(|l| l.strip_prefix("version = \"")?.strip_suffix('"').map(String::from))
}

/// The manifests whose `version` differs from `want`, as `path: version`.
fn version_drift(want: &str, manifests: &[(String, Value)]) -> Vec<String> {
    manifests
        .iter()
        .filter(|(_, v)| v["version"].as_str() != Some(want))
        .map(|(p, v)| format!("{p}: {}", v["version"].as_str().unwrap_or("no version")))
        .collect()
}

/// Repository paths a text names: a token that starts with a top-level entry of the repository
/// followed by `/`, or is a top-level file. A `:line` suffix, trailing punctuation, tokens
/// holding a placeholder (`<`, `*`, `{`, `$`) and a format id such as `protocol/1` are not paths
/// to check.
fn repo_paths(text: &str, top: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for raw in text.split(|c: char| c.is_whitespace() || "`()\"',;[]".contains(c)) {
        let mut t = raw.trim_end_matches(['.', ':']);
        if t.is_empty() || t.contains(['<', '>', '*', '{', '$', '=']) {
            continue;
        }
        let version_of_a_format = t.rsplit_once('/').is_some_and(|(_, v)| !v.is_empty() && v.chars().all(|c| c.is_ascii_digit()));
        if version_of_a_format {
            continue;
        }
        if let Some((head, tail)) = t.rsplit_once(':') {
            if tail.chars().all(|c| c.is_ascii_digit() || c == '-') && !tail.is_empty() {
                t = head;
            }
        }
        let first = t.split('/').next().unwrap_or("");
        let named = top.iter().any(|e| e == first);
        if named && (t.contains('/') || Path::new(first).extension().is_some()) {
            out.push(t.to_string());
        }
    }
    out.sort();
    out.dedup();
    out
}

fn top_entries() -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(root())
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| !n.starts_with('.'))
        .collect();
    v.extend(["runs".to_string()]);
    v
}

fn skills() -> Vec<PathBuf> {
    let dir = root().join("plugin/skills");
    let mut v: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(|e| Some(e.ok()?.path()))
        .filter(|p| p.is_dir())
        .collect();
    v.sort();
    v
}

fn frontmatter(text: &str) -> serde_yaml::Value {
    let body = text.strip_prefix("---\n").expect("SKILL.md starts with ---");
    let (fm, _) = body.split_once("\n---\n").expect("frontmatter closes with ---");
    serde_yaml::from_str(fm).expect("frontmatter is YAML")
}

#[test]
fn manifests_carry_the_repository_version() {
    let version = cargo_version(&fs::read_to_string(root().join("driver/Cargo.toml")).unwrap()).unwrap();
    let manifests: Vec<(String, Value)> = [
        "plugin/.claude-plugin/plugin.json",
        "plugin/.codex-plugin/plugin.json",
    ]
    .iter()
    .map(|p| (p.to_string(), json(&root().join(p))))
    .collect();
    let drift = version_drift(&version, &manifests);
    assert!(drift.is_empty(), "plugin manifests not at {version}: {drift:?}");
}

#[test]
fn version_drift_is_found() {
    let ms = vec![
        ("a".to_string(), serde_json::json!({"version": "0.1.4"})),
        ("b".to_string(), serde_json::json!({"version": "0.1.3"})),
        ("c".to_string(), serde_json::json!({})),
    ];
    assert_eq!(version_drift("0.1.4", &ms), vec!["b: 0.1.3", "c: no version"]);
}

#[test]
fn marketplace_lists_the_plugin_at_plugin_dir() {
    let m = json(&root().join(".claude-plugin/marketplace.json"));
    assert_eq!(m["name"], "runbook");
    let plugins = m["plugins"].as_array().expect("plugins");
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0]["name"], "runbook");
    assert_eq!(plugins[0]["source"], "./plugin");
    for p in ["plugin/.claude-plugin/plugin.json", "plugin/.codex-plugin/plugin.json"] {
        assert_eq!(json(&root().join(p))["name"], "runbook", "{p}");
    }
    let codex = json(&root().join("plugin/.codex-plugin/plugin.json"));
    assert_eq!(codex["skills"], "./skills/");
}

#[test]
fn manifests_carry_the_required_fields() {
    for p in ["plugin/.claude-plugin/plugin.json", "plugin/.codex-plugin/plugin.json"] {
        let m = json(&root().join(p));
        for k in ["name", "version", "description", "license", "repository"] {
            assert!(m[k].as_str().is_some_and(|s| !s.is_empty()), "{p}: `{k}` is missing or empty");
        }
        assert!(m["author"]["name"].as_str().is_some_and(|s| !s.is_empty()), "{p}: author.name");
    }
    let m = json(&root().join(".claude-plugin/marketplace.json"));
    assert!(m["owner"]["name"].as_str().is_some_and(|s| !s.is_empty()), "marketplace owner.name");
    assert!(m["plugins"][0]["description"].as_str().is_some_and(|s| !s.is_empty()));
}

#[test]
fn the_plugin_directory_is_the_one_home_of_the_skills() {
    assert!(!root().join("skills").exists(), "a second copy of the skills under skills/");
}

#[test]
fn every_skill_has_a_name_and_a_description_with_use_and_not() {
    let dirs = skills();
    let names: Vec<String> = dirs.iter().map(|d| d.file_name().unwrap().to_string_lossy().into()).collect();
    assert_eq!(names, ["harden", "record", "workflow-builder"]);
    for d in dirs {
        let text = fs::read_to_string(d.join("SKILL.md")).unwrap();
        let fm = frontmatter(&text);
        assert_eq!(fm["name"].as_str(), d.file_name().unwrap().to_str(), "{}", d.display());
        let desc = fm["description"].as_str().unwrap_or("");
        assert!(desc.contains("Use when"), "{}: description has no 'Use when'", d.display());
        assert!(desc.to_lowercase().contains("not for"), "{}: description has no 'Not for'", d.display());
    }
}

#[test]
fn every_repository_path_a_skill_names_exists() {
    let top = top_entries();
    let mut missing = Vec::new();
    for d in skills() {
        let text = fs::read_to_string(d.join("SKILL.md")).unwrap();
        for p in repo_paths(&text, &top) {
            if !root().join(&p).exists() {
                missing.push(format!("{}: {p}", d.file_name().unwrap().to_string_lossy()));
            }
        }
        for line in text.lines() {
            for tok in line.split(|c: char| c.is_whitespace() || "`()\"',;".contains(c)) {
                if let Some(r) = tok.strip_prefix("references/") {
                    let r = r.trim_end_matches(['.', ':']);
                    if !r.is_empty() && !d.join("references").join(r).exists() {
                        missing.push(format!("{}: references/{r}", d.file_name().unwrap().to_string_lossy()));
                    }
                }
            }
        }
    }
    assert!(missing.is_empty(), "paths named in a SKILL.md that do not exist: {missing:#?}");
}

#[test]
fn a_missing_path_is_found() {
    let top = vec!["docs".to_string(), "scripts".to_string(), "protocol".to_string(), "Taskfile.yml".to_string()];
    let text = "Run `scripts/gone.sh --x` and read docs/there.md:12-30, `docs/<id>.md`, a protocol/1 file, Taskfile.yml.";
    assert_eq!(repo_paths(text, &top), ["Taskfile.yml", "docs/there.md", "scripts/gone.sh"]);
}

#[test]
fn plugin_files_hold_no_home_path_and_no_email_address() {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out);
            } else {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(&root().join("plugin"), &mut files);
    walk(&root().join(".claude-plugin"), &mut files);
    let mut bad = Vec::new();
    for f in files {
        let text = fs::read_to_string(&f).unwrap();
        for what in leaks(&text) {
            bad.push(format!("{}: {what}", f.display()));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// A home-directory path (`/home/` and a path segment) or an email address in a text.
fn leaks(text: &str) -> Vec<&'static str> {
    let mut out = Vec::new();
    let home = text.match_indices("/home/").any(|(i, m)| {
        text[i + m.len()..].chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.')
    });
    if home {
        out.push("home path");
    }
    let email = text.match_indices('@').any(|(i, _)| {
        let before = text[..i].chars().next_back().is_some_and(|c| c.is_alphanumeric() || "._+-".contains(c));
        let domain: String = text[i + 1..].chars().take_while(|c| c.is_alphanumeric() || ".-".contains(*c)).collect();
        before && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
    });
    if email {
        out.push("email address");
    }
    out
}

#[test]
fn leaks_are_found() {
    let home = format!("see {}x/y", ["/ho", "me/"].concat());
    let mail = format!("mail a.b{}example.org now", '@');
    let neither = format!("`{}` alone, {}scope and a{}b are neither", ["/ho", "me/"].concat(), '@', '@');
    assert_eq!(leaks(&home), ["home path"]);
    assert_eq!(leaks(&mail), ["email address"]);
    assert!(leaks(&neither).is_empty());
}
