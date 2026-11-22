//! The one document the review reads (`SDC_REVIEW_MODE=facts`): the diff, the touched files, the
//! listing of the others and the test summary.
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

/// The files a unified diff touches (its `+++ impl/<path>` lines), in order, once each.
pub fn changed_files(diff: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in diff.lines().filter_map(|l| l.strip_prefix("+++ impl/")).map(|l| l.split('\t').next().unwrap_or(l)) {
        if !out.iter().any(|o| o == p) {
            out.push(p.to_string());
        }
    }
    out
}

/// The diff, but a file the change creates (`@@ -0,0 +1,N @@`) keeps only its header and one line:
/// its full text follows in the document, so its lines would be there twice.
pub fn compact_diff(diff: &str) -> String {
    fn flush(section: &mut Vec<&str>, out: &mut String) {
        let created = section.iter().any(|l| l.starts_with("@@ -0,0 +1"));
        // The `diff -ruN ...` command line and the files' timestamps are not facts about the change.
        for l in section.iter().take_while(|l| !(created && l.starts_with("@@"))).filter(|l| !l.starts_with("diff ")) {
            out.push_str(if l.starts_with("--- ") || l.starts_with("+++ ") { l.split('\t').next().unwrap_or(l) } else { l });
            out.push('\n');
        }
        if created {
            out.push_str("(new file: every line is added; its full text is below)\n");
        }
        section.clear();
    }
    let mut out = String::new();
    let mut section: Vec<&str> = Vec::new();
    for l in diff.lines() {
        if l.starts_with("diff ") {
            flush(&mut section, &mut out);
        }
        section.push(l);
    }
    flush(&mut section, &mut out);
    out
}

/// A file's text with its line numbers, the numbers a finding's `at` cites.
pub fn number_lines(text: &str) -> String {
    let w = text.lines().count().to_string().len().max(3);
    text.lines().enumerate().map(|(i, l)| format!("{:>w$}  {l}\n", i + 1)).collect()
}

/// One product file the diff does not touch: its path under `impl/`, its size, and the names of the
/// items it declares at top level when a rule of its language finds them.
pub struct Listed {
    /// The path under `impl/`.
    pub path: String,
    /// The size in bytes.
    pub bytes: usize,
    /// The top-level items a rule of the file's language finds, or none.
    pub items: Option<Vec<String>>,
}

/// What the listing is allowed to cost: item-bearing lines up to this many bytes, then paths only.
pub const LISTING_CAP: usize = 4096;
/// Items named per file; the rest are counted.
pub const LISTING_ITEMS: usize = 24;
/// A source file larger than this is listed by path and size only.
pub const LISTING_READ_MAX: usize = 512 * 1024;

/// The names a source file declares at top level (column 0), by a simple rule per language, plus the
/// routes and event listeners it registers (`GET /tasks`, `/ping`, `on fetch`), in file order, once
/// each. None when no rule applies to the file's extension (a page, a manifest, a document).
pub fn top_level_items(path: &str, text: &str) -> Option<Vec<String>> {
    let ext = path.rsplit_once('.')?.1.to_ascii_lowercase();
    let decl = match ext.as_str() {
        "rs" => r"^(?:pub(?:\([a-z: ]+\))?\s+)?(?:(?:async|unsafe|const)\s+)*(?:fn|struct|enum|trait|mod|type|static|const)\s+([A-Za-z_]\w*)",
        "js" | "mjs" | "cjs" | "jsx" | "ts" | "tsx" | "mts" | "cts" =>
            r"^(?:export\s+)?(?:default\s+)?(?:async\s+)?(?:function\*?|class|const|let|var|interface|type|enum)\s+([A-Za-z_$][\w$]*)",
        "py" => r"^(?:async\s+)?(?:def|class)\s+([A-Za-z_]\w*)",
        "go" => r"^(?:func\s+(?:\([^)]*\)\s*)?|type\s+|var\s+|const\s+)([A-Za-z_]\w*)",
        "sh" | "bash" => r"^(?:function\s+([A-Za-z_][\w-]*)|([A-Za-z_][\w-]*)\s*\(\))",
        _ => return None,
    };
    let decl = regex::Regex::new(decl).ok()?;
    let method = regex::Regex::new(r#"\.(get|post|put|patch|delete|head|options)\(\s*["'`](/[^"'`]*)["'`]"#).ok()?;
    let route = regex::Regex::new(r#"(?:\.route|HandleFunc|Handle)\(\s*["'`](/[^"'`]*)["'`]"#).ok()?;
    let listener = regex::Regex::new(r#"addEventListener\(\s*["'](\w+)["']"#).ok()?;
    let mut items: Vec<String> = Vec::new();
    let mut add = |s: String| {
        if !items.contains(&s) {
            items.push(s)
        }
    };
    for l in text.lines() {
        if let Some(c) = decl.captures(l) {
            if let Some(n) = c.iter().skip(1).flatten().next() {
                add(n.as_str().to_string());
            }
        }
        for c in method.captures_iter(l) {
            add(format!("{} {}", c[1].to_ascii_uppercase(), &c[2]));
        }
        for c in route.captures_iter(l) {
            add(c[1].to_string());
        }
        for c in listener.captures_iter(l) {
            add(format!("on {}", &c[1]));
        }
    }
    Some(items)
}

/// The product files the diff does not touch under `dir`, sorted by path: not build output, not the
/// gate's state, not version control or installed packages.
pub fn product_files(dir: &Path, changed: &[String]) -> Vec<Listed> {
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let Ok(rd) = fs::read_dir(dir.join(&rel)) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if ["target", ".sdc", ".git", ".derived-from", "node_modules"].contains(&name.as_str()) {
                continue;
            }
            let Ok(t) = e.file_type() else { continue };
            let r = rel.join(&name);
            if t.is_dir() {
                stack.push(r);
            } else if t.is_file() {
                let p = r.to_string_lossy().into_owned();
                if changed.contains(&p) {
                    continue;
                }
                let bytes = e.metadata().map(|m| m.len() as usize).unwrap_or(0);
                let items = if bytes <= LISTING_READ_MAX { fs::read(e.path()).ok().and_then(|b| top_level_items(&p, &String::from_utf8_lossy(&b))) } else { None };
                out.push(Listed { path: p, bytes, items });
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// The listing as the document's section: one line per file, path and size and, for a source file,
/// its items. The lines with sizes and items stop at `cap` bytes; the files after that are paths only.
pub fn listing_section(files: &[Listed], cap: usize) -> String {
    let mut d = String::from("\n## Product files not in the diff (path, size, and for source files the top-level items it declares; a file neither listed here nor in the diff does not exist in the tree; `at` may cite a listed file by path alone)\n");
    if files.is_empty() {
        d += "none\n";
    }
    let (mut used, mut paths_only) = (0, false);
    for f in files {
        let mut line = format!("- {} ({} bytes)", f.path, f.bytes);
        if let Some(items) = f.items.as_ref().filter(|i| !i.is_empty()) {
            let more = items.len().saturating_sub(LISTING_ITEMS);
            line += &format!(": {}{}", items.iter().take(LISTING_ITEMS).cloned().collect::<Vec<_>>().join(", "), if more > 0 { format!(", +{more} more") } else { String::new() });
        }
        line += "\n";
        if !paths_only && used + line.len() > cap {
            paths_only = true;
            d += &format!("(the listing is capped at {cap} bytes: the files below are paths only)\n");
        }
        if paths_only {
            d += &format!("- {}\n", f.path);
        } else {
            used += line.len();
            d += &line;
        }
    }
    d
}

/// The counts and the failing names of a conformance and test log, not the log: the run's header
/// line, the scenario counts, the unit tests' totals, and every scenario or test that failed.
pub fn test_summary(log: &str) -> String {
    let scenarios = regex::Regex::new(r"^\d+ scenarios?: ").unwrap();
    let unit = regex::Regex::new(r"^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored").unwrap();
    let scenario_bad = regex::Regex::new(r"^(?:failed|error)\s+(\S+)").unwrap();
    let test_bad = regex::Regex::new(r"^test (\S+) \.\.\. (?:FAILED|failed)").unwrap();
    let (mut head, mut counts) = (None, None);
    let mut totals: Option<[u64; 3]> = None;
    let mut failing: Vec<String> = Vec::new();
    for l in log.lines() {
        let t = l.trim();
        if head.is_none() && t.contains(" against ") && (t.ends_with("— passed") || t.ends_with("— failed")) {
            head = Some(t.to_string());
        } else if counts.is_none() && scenarios.is_match(t) {
            counts = Some(t.to_string());
        } else if let Some(c) = unit.captures(t) {
            let n = |i: usize| c[i].parse::<u64>().unwrap_or(0);
            let s = totals.get_or_insert([0; 3]);
            (s[0], s[1], s[2]) = (s[0] + n(1), s[1] + n(2), s[2] + n(3));
        } else if let Some(c) = scenario_bad.captures(t).or_else(|| test_bad.captures(t)) {
            if !failing.contains(&c[1].to_string()) {
                failing.push(c[1].to_string());
            }
        }
    }
    let mut out = String::new();
    if let Some(h) = head {
        out += &format!("conformance: {h}\n");
    }
    if let Some(c) = counts {
        out += &format!("scenarios: {c}\n");
    }
    if let Some([p, f, i]) = totals {
        out += &format!("unit tests: {p} passed, {f} failed, {i} ignored\n");
    }
    if out.is_empty() {
        out += "no counts found in the run's log\n";
    }
    out += &format!("failing: {}\n", if failing.is_empty() { "none".to_string() } else { failing.join(", ") });
    out
}

/// What the review is given, as facts: nothing of the tree beyond `files`, the files the diff touches.
pub struct ReviewFacts<'a> {
    /// The story's id.
    pub story: &'a str,
    /// `mainline` when the diff is against the mainline tree, else `empty` (the first story).
    pub base: &'a str,
    /// `{cite, text}`: the summary line and the lines of the `## Acceptance` section.
    pub acceptance: &'a [Value],
    /// `{id, text, cites}`: the requirements the story delivers.
    pub requirements: &'a [Value],
    /// The requirements no story of the plan covers.
    pub uncovered: &'a [Value],
    /// The compiled specification's entries of the commands the story covers.
    pub commands: &'a Map<String, Value>,
    /// The text of the tree's build constraints, when it has them.
    pub constraints: Option<&'a str>,
    /// `test_summary` of the conformance run on the revision under review.
    pub tests: &'a str,
    /// The unified diff against the mainline tree.
    pub diff: &'a str,
    /// The touched files: path under `impl/` and the text, None for a file the change deletes.
    pub files: &'a [(String, Option<String>)],
    /// Every other product file of the tree, by path, size and top-level items.
    pub listing: &'a [Listed],
}

/// The one document the review reads: the story's requirements and commands, the test summary, the
/// diff and the full text of every file the diff touches.
pub fn review_document(f: &ReviewFacts) -> String {
    let line = |v: &Value, k: &str| format!("- {}: {}\n", v[k].as_str().unwrap_or(""), v["text"].as_str().unwrap_or("").trim());
    let mut d = format!("# Review input: story `{}`\n\nThis is everything you are given. Judge the change from it alone.\n", f.story);
    d += "\n## Requirements this story delivers (a violation names the id)\n";
    d += &f.requirements.iter().map(|r| line(r, "id")).collect::<String>();
    d += "\n## Acceptance lines (a violation names the cite)\n";
    d += &f.acceptance.iter().map(|a| line(a, "cite")).collect::<String>();
    d += "\n## Commands it must serve (a violation names the command; the compiled specification)\n";
    for (name, v) in f.commands {
        d += &format!("- {name}: {v}\n");
    }
    d += "\n## Requirements no story of the plan covers (kind `outside`; never a reason to reject)\n";
    d += &if f.uncovered.is_empty() { "none\n".to_string() } else { f.uncovered.iter().map(|r| line(r, "id")).collect() };
    if let Some(c) = f.constraints {
        d += &format!("\n## Build constraints\n{}\n", c.trim_end());
    }
    d += &format!("\n## Test summary (the conformance run that passed on this revision)\n{}", f.tests);
    d += &format!("\n## Change against the {} tree (unified diff)\n~~~diff\n{}~~~\n", f.base, compact_diff(f.diff));
    d += "\n## Changed files, full text (the line numbers are the ones `at` cites; a file the diff does not touch is only listed at the end)\n";
    for (path, text) in f.files {
        match text {
            Some(t) => d += &format!("\n### impl/{path} ({} lines)\n~~~\n{}~~~\n", t.lines().count(), number_lines(t)),
            None => d += &format!("\n### impl/{path}\nDeleted by this change.\n"),
        }
    }
    d + &listing_section(f.listing, LISTING_CAP)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_level_items_are_found_by_a_rule_per_language() {
        let items = |p: &str, t: &str| top_level_items(p, t).map(|v| v.join(", "));
        assert_eq!(items("a.rs", "use x;\npub struct S;\npub(crate) async fn go() {}\n    fn nested() {}\nenum E {}\nimpl S {}\nmod m;\n"),
            Some("S, go, E, m".into()));
        assert_eq!(items("a.js", "export function f() {}\nexport default class K {}\nconst x = 1;\n  function inner() {}\napp.post('/tasks', h);\nself.addEventListener('fetch', e => {});\n"),
            Some("f, K, x, POST /tasks, on fetch".into()));
        assert_eq!(items("a.ts", "export const t: number = 1;\nexport interface I {}\nexport type T = 1;\n"), Some("t, I, T".into()));
        assert_eq!(items("a.py", "import os\n@app.route('/x')\ndef view():\n    def inner(): pass\nclass C:\n    pass\nasync def go(): pass\n"),
            Some("/x, view, C, go".into()));
        assert_eq!(items("a.go", "package main\nfunc Main() {}\nfunc (s *S) Do() {}\ntype S struct{}\nmux.HandleFunc(\"/ping\", h)\n"),
            Some("Main, Do, S, /ping".into()));
        assert_eq!(items("run.sh", "#!/bin/sh\nrun() {\n  :\n}\nfunction stop { :; }\n"), Some("run, stop".into()));
        // No rule for a page, a manifest or a document: path and size only. A source file without items has an empty list.
        for p in ["index.html", "package.json", "README.md", "style.css", "logo.png"] {
            assert_eq!(items(p, "anything"), None, "{p}");
        }
        assert_eq!(items("empty.rs", "// nothing\n"), Some(String::new()));
    }

    #[test]
    fn the_listing_is_capped_and_beyond_the_cap_it_is_paths_only() {
        let f = |p: &str, items: Option<&[&str]>| Listed { path: p.into(), bytes: 100, items: items.map(|i| i.iter().map(|s| s.to_string()).collect()) };
        let files = vec![f("a.rs", Some(&["alpha_item_name", "beta_item_name"])), f("b.rs", Some(&["gamma_item_name"])), f("c.html", None), f("d.rs", Some(&["delta_item_name"]))];
        let all = listing_section(&files, 4096);
        assert!(all.contains("- a.rs (100 bytes): alpha_item_name, beta_item_name\n") && all.contains("- d.rs (100 bytes): delta_item_name\n"), "{all}");
        assert!(all.contains("- c.html (100 bytes)\n"), "{all}");
        let capped = listing_section(&files, 80);
        assert!(capped.contains("- a.rs (100 bytes): alpha_item_name, beta_item_name\n"), "{capped}");
        assert!(!capped.contains("delta_item_name") && !capped.contains("gamma_item_name"), "{capped}");
        assert!(capped.contains("- b.rs\n") && capped.contains("- c.html\n") && capped.contains("- d.rs\n"), "{capped}");
        assert!(capped.contains("paths only"), "{capped}");
        assert!(listing_section(&[], 4096).contains("none"));
    }

    #[test]
    fn changed_files_are_the_diffs_plus_files_once_each() {
        let diff = "diff -ruN a/x b/x\n--- mainline/src/a.rs\t1\n+++ impl/src/a.rs\t2\n@@ -1 +1 @@\n-x\n+y\ndiff -ruN a/y b/y\n--- mainline/b.md\t1\n+++ impl/b.md\t2\n";
        assert_eq!(changed_files(diff), vec!["src/a.rs", "b.md"]);
        assert!(changed_files("").is_empty());
    }

    #[test]
    fn a_created_file_keeps_its_header_and_not_its_lines_in_the_diff() {
        let diff = "diff -ruN m/a i/a\n--- mainline/a.rs\t1\n+++ impl/a.rs\t2\n@@ -1 +1,2 @@\n x\n+kept\ndiff -ruN m/n i/n\n--- mainline/n.rs\t1\n+++ impl/n.rs\t2\n@@ -0,0 +1,2 @@\n+gone1\n+gone2\n";
        let c = compact_diff(diff);
        assert!(c.contains("+kept") && c.contains("+++ impl/n.rs") && c.contains("(new file"), "{c}");
        assert!(!c.contains("gone1") && !c.contains("gone2"), "{c}");
    }

    #[test]
    fn the_test_summary_has_counts_and_failing_names_only() {
        let log = "Requires an empty namespace.\ntodo v1 against interpreted 0.96.0 — failed\n  passed a/outcome/x\n  failed todo.task.AddTask/outcome/added\n  error todo.sync.Send/outcome/e\n\
            3 scenarios: 1 passed, 1 failed, 1 error, 0 unsupported\nrunning 2 tests\ntest tests::ok ... ok\ntest tests::bad ... FAILED\n\
            test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured\nrunning 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured\n";
        let s = test_summary(log);
        assert_eq!(s, "conformance: todo v1 against interpreted 0.96.0 — failed\nscenarios: 3 scenarios: 1 passed, 1 failed, 1 error, 0 unsupported\n\
            unit tests: 1 passed, 1 failed, 0 ignored\nfailing: todo.task.AddTask/outcome/added, todo.sync.Send/outcome/e, tests::bad\n");
        assert!(test_summary("nothing\n").starts_with("no counts found"));
    }
}
