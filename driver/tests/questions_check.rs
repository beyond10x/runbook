//! `questions.check` (`check-questions` in `bindings/tools/sdc-tool.sh`) on a scope-changing
//! question's context keys (`story:interview-question-context`): `excerpt`, `default_reason` and a
//! `consequences` entry `{does, cost}` for every option. The shape is the one
//! `sdc-decision-request` parses. Module `loom_mode`: the tools started as Loom starts a bound
//! program behave as the driver-mode run of the same input (`story:loom-tools-program-contract`).

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

const DISSECTION: &str = "items:
  - {id: d1, type: ambiguity, span: for me and my partner, note: whose list}
  - {id: d2, type: wish, span: to-do list, note: a list}
";

const OPTIONS: &str = "  options:
  - One shared list
  - Separate lists
  default: One shared list
";
const EXCERPT: &str = "  excerpt: for me and my partner\n";
const REASON: &str = "  default_reason: The idea says one partner, so one list.\n";
const CONSEQUENCES: &str = "  consequences:
  - does: Builds one list both read and edit.
    cost: The smallest build.
  - does: Builds a list per user with sharing.
    cost: Roughly twice the build.
";
const ONE_CONSEQUENCE: &str = "  consequences:
  - does: Builds one list both read and edit.
    cost: The smallest build.
";
const EMPTY_COST: &str = "  consequences:
  - does: Builds one list both read and edit.
    cost: The smallest build.
  - does: Builds a list per user with sharing.
    cost: ''
";

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn tool() -> PathBuf {
    manifest().join("../bindings/tools/sdc-tool.sh")
}

/// A case's work/ with one dissection and the given questions.yaml; runs `check-questions` in it.
fn check_in(name: &str, questions: &str) -> Output {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("questions-check")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("dissection.yaml"), DISSECTION).unwrap();
    fs::write(dir.join("questions.yaml"), questions).unwrap();
    Command::new("bash")
        .arg(tool())
        .arg("check-questions")
        .current_dir(&dir)
        .output()
        .unwrap()
}

/// q1 about d1, scope-changing and open, with the given context blocks between its keys.
fn scope_question(blocks: &[&str], tail: &str) -> String {
    format!(
        "questions:\n- id: q1\n  item: d1\n  question: One list or two?\n{}{}  scope_changing: true\n{tail}",
        OPTIONS,
        blocks.concat()
    )
}

const OPEN: &str = "  status: open\n";
const SETTLED: &str = "  status: accepted\n  answer: One shared list\n  answered_by: operator\n";

fn code(o: &Output) -> i32 {
    o.status.code().unwrap()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn refused_naming(case: &str, blocks: &[&str], key: &str) {
    let o = check_in(case, &scope_question(blocks, OPEN));
    assert_eq!(code(&o), 2, "{case}: stderr: {}", stderr(&o));
    let e = stderr(&o);
    assert!(
        e.contains("q1") && e.contains(key),
        "{case}: stderr names q1 and {key}: {e}"
    );
}

#[test]
fn scope_changing_without_excerpt_is_refused() {
    refused_naming("no-excerpt", &[REASON, CONSEQUENCES], "excerpt");
}

#[test]
fn scope_changing_without_default_reason_is_refused() {
    refused_naming("no-reason", &[EXCERPT, CONSEQUENCES], "default_reason");
}

#[test]
fn scope_changing_without_consequences_is_refused() {
    refused_naming("no-consequences", &[EXCERPT, REASON], "consequences");
}

#[test]
fn scope_changing_with_a_consequence_missing_for_an_option_is_refused() {
    refused_naming(
        "short-consequences",
        &[EXCERPT, REASON, ONE_CONSEQUENCE],
        "consequences",
    );
}

#[test]
fn scope_changing_with_an_empty_cost_is_refused() {
    refused_naming("empty-cost", &[EXCERPT, REASON, EMPTY_COST], "consequences");
}

#[test]
fn scope_changing_with_all_three_exits_as_before() {
    let blocks = [EXCERPT, REASON, CONSEQUENCES];
    let open = check_in("all-open", &scope_question(&blocks, OPEN));
    assert_eq!(code(&open), 1, "open: stderr: {}", stderr(&open));
    let settled = check_in("all-settled", &scope_question(&blocks, SETTLED));
    assert_eq!(code(&settled), 0, "settled: stderr: {}", stderr(&settled));
}

#[test]
fn a_question_that_is_not_scope_changing_needs_none_of_them() {
    let q = "questions:\n- id: q1\n  item: d1\n  question: One list or two?\n  options:\n  - a\n  - b\n  default: a\n  scope_changing: false\n";
    let open = check_in("plain-open", &format!("{q}  status: open\n"));
    assert_eq!(code(&open), 1, "stderr: {}", stderr(&open));
    let settled = check_in(
        "plain-settled",
        &format!("{q}  status: accepted\n  answer: a\n"),
    );
    assert_eq!(code(&settled), 0, "stderr: {}", stderr(&settled));
}

#[test]
fn every_missing_key_of_a_question_is_named() {
    let o = check_in("all-missing", &scope_question(&[], OPEN));
    assert_eq!(code(&o), 2);
    let e = stderr(&o);
    for key in ["excerpt", "default_reason", "consequences"] {
        assert!(e.contains(key), "{key} named: {e}");
    }
}

/// The questions fixtures keep the exit they had before the keys (never 2: their shape is valid).
#[test]
fn the_questions_fixtures_pass_the_check() {
    let fx = manifest().join("examples/sdc/fixtures");
    let dissection = fs::read_to_string(fx.join("dissection.yaml")).unwrap();
    for (name, want) in [
        ("questions-open.yaml", 1),
        ("questions-answered.yaml", 0),
        ("scope-question/questions-open.yaml", 1),
        ("scope-question/questions-answered.yaml", 1),
    ] {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("questions-check")
            .join(name.replace('/', "-"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("dissection.yaml"), &dissection).unwrap();
        fs::copy(fx.join(name), dir.join("questions.yaml")).unwrap();
        let o = Command::new("bash")
            .arg(tool())
            .arg("check-questions")
            .current_dir(&dir)
            .output()
            .unwrap();
        assert_eq!(code(&o), want, "{name}: stderr: {}", stderr(&o));
    }
}

/// Every scope-changing question in a fixture carries the three keys, one consequence per option.
#[test]
fn scope_changing_questions_in_fixtures_carry_the_keys() {
    let fx = manifest().join("examples/sdc/fixtures");
    for name in [
        "questions-open.yaml",
        "questions-answered.yaml",
        "scope-question/questions-open.yaml",
        "scope-question/questions-answered.yaml",
        "offline-decision/questions-answered.yaml",
        "questions-trial-2.yaml",
    ] {
        let doc: serde_yaml::Value =
            serde_yaml::from_str(&fs::read_to_string(fx.join(name)).unwrap()).unwrap();
        for q in doc["questions"].as_sequence().unwrap() {
            if q["scope_changing"].as_bool() != Some(true) {
                continue;
            }
            let id = q["id"].as_str().unwrap();
            let options = q["options"].as_sequence().map_or(0, Vec::len);
            let consequences = q["consequences"].as_sequence().map_or(0, Vec::len);
            assert!(options >= 2, "{name} {id}: options");
            assert_eq!(consequences, options, "{name} {id}: consequences");
            assert!(q["excerpt"].as_str().is_some_and(|s| !s.is_empty()));
            assert!(q["default_reason"].as_str().is_some_and(|s| !s.is_empty()));
        }
    }
}

/// Loom mode of the tools (`LOOM_CASE` set; docs/loom-bindings.md, "Tool changes for Loom's
/// program contract"). Each tool is started as Loom starts a bound program: an empty environment
/// but `PATH` and `LOOM_CASE` (and `LOOM_RETRY_FEEDBACK` on a retry), the arguments as one JSON
/// object on standard input, the workspace as its start directory, the binding's argv. Each run
/// is compared with the driver-mode run of the same input (the driver's variables, standard input
/// empty): the same exit, output and files in the workspace. The driver-mode run is given what
/// Loom gives: no case directory, and feedback records that name no action and no exit.
mod loom_mode {
    use regex::Regex;
    use std::collections::BTreeMap;
    use std::fs;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};

    fn tools() -> PathBuf {
        super::manifest().join("../bindings/tools")
    }

    fn fx() -> PathBuf {
        super::manifest().join("examples/sdc/fixtures")
    }

    /// `<tmp>/loom-mode/<name>/{driver,loom}`, each empty with a `work/`.
    fn roots(name: &str) -> (PathBuf, PathBuf) {
        let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("loom-mode").join(name);
        let _ = fs::remove_dir_all(&base);
        let (d, l) = (base.join("driver"), base.join("loom"));
        fs::create_dir_all(d.join("work")).unwrap();
        fs::create_dir_all(l.join("work")).unwrap();
        (d, l)
    }

    /// Both workspaces get the same file.
    fn put(roots: &(PathBuf, PathBuf), rel: &str, text: &str) {
        for r in [&roots.0, &roots.1] {
            let p = r.join("work").join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text).unwrap();
        }
    }

    /// This process's PATH with the driver crate's binaries first: under Loom `sdc-findings` and
    /// `sdc-bugfix` come from PATH.
    fn path() -> String {
        let bins = Path::new(env!("CARGO_BIN_EXE_sdc-findings")).parent().unwrap();
        format!("{}:{}", bins.display(), std::env::var("PATH").unwrap_or_default())
    }

    #[derive(Debug, PartialEq)]
    struct Run {
        code: Option<i32>,
        stdout: String,
        stderr: String,
        files: BTreeMap<String, String>,
    }

    /// Text with the run's root and the instants and process ids it wrote made comparable.
    fn norm(root: &Path, text: &str) -> String {
        let t = text.replace(&root.display().to_string(), "<root>");
        let t = Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z").unwrap().replace_all(&t, "<instant>");
        Regex::new(r#""pid":\s*\d+"#).unwrap().replace_all(&t, r#""pid":0"#).into_owned()
    }

    /// Every file under `dir`, by path relative to `work`, normalized; Loom mode's own copy of the
    /// feedback (`.sdc/loom-feedback/`) is not an output.
    fn files(root: &Path, work: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let rel = p.strip_prefix(work).unwrap().display().to_string();
            if rel == ".sdc/loom-feedback" {
                continue;
            }
            if p.is_dir() {
                files(root, work, &p, out);
            } else {
                out.insert(rel, norm(root, &String::from_utf8_lossy(&fs::read(&p).unwrap())));
            }
        }
    }

    fn run(root: &Path, mut c: Command, stdin: &str) -> Run {
        let work = root.join("work");
        c.current_dir(&work).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = c.spawn().unwrap();
        child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
        let o = child.wait_with_output().unwrap();
        let mut f = BTreeMap::new();
        files(root, &work, &work, &mut f);
        Run {
            code: o.status.code(),
            stdout: norm(root, &String::from_utf8_lossy(&o.stdout)),
            stderr: norm(root, &String::from_utf8_lossy(&o.stderr)),
            files: f,
        }
    }

    /// The driver's start: its variables, standard input empty, `sdc-findings` named by variable.
    fn driver(root: &Path, tool: &str, argv: &[&str], env: &[(&str, &str)]) -> Run {
        let mut c = Command::new(tools().join(tool));
        c.args(argv).env_clear().env("PATH", path()).env("SDC_FINDINGS", env!("CARGO_BIN_EXE_sdc-findings"));
        for (k, v) in env {
            c.env(k, v);
        }
        run(root, c, "")
    }

    /// Loom's start: `PATH`, `LOOM_CASE`, `LOOM_RETRY_FEEDBACK` when given, the arguments on stdin.
    fn loom(root: &Path, tool: &str, argv: &[&str], case: &str, arguments: &str, feedback: Option<&Path>) -> Run {
        loom_named(root, tool, argv, case, arguments, feedback, &[])
    }

    /// Loom's start with the names it gave the case: `LOOM_CASE_NAME`, `LOOM_PARENT_CASE_NAME`.
    fn loom_named(root: &Path, tool: &str, argv: &[&str], case: &str, arguments: &str, feedback: Option<&Path>, names: &[(&str, &str)]) -> Run {
        let mut c = Command::new(tools().join(tool));
        c.args(argv).env_clear().env("PATH", path()).env("LOOM_CASE", case);
        for (k, v) in names {
            c.env(k, v);
        }
        if let Some(f) = feedback {
            c.env("LOOM_RETRY_FEEDBACK", f);
        }
        run(root, c, arguments)
    }

    fn same(what: &str, d: &Run, l: &Run) {
        assert_eq!(d.code, l.code, "{what}: exit; driver {d:#?}\nloom {l:#?}");
        assert_eq!(d.stdout, l.stdout, "{what}: stdout");
        assert_eq!(d.stderr, l.stderr, "{what}: stderr");
        assert_eq!(d.files.keys().collect::<Vec<_>>(), l.files.keys().collect::<Vec<_>>(), "{what}: the files written");
        for (k, v) in &d.files {
            assert_eq!(v, &l.files[k], "{what}: {k}");
        }
    }

    /// The binding's argv of an agent node (bindings/sdc-intake.yaml and the like): the model, the
    /// turn and budget caps, the stub's argv last.
    fn node_argv<'a>(node: &'a str, stub: &[&'a str], extra: &[&'a str]) -> Vec<&'a str> {
        let mut a = vec![node, "--model", "claude-haiku-5-5", "--max-turns", "15", "--max-budget-usd", "0.5"];
        a.extend_from_slice(extra);
        a.push("--stub");
        a.extend_from_slice(stub);
        a
    }

    const WRITE_INSTRUCTIONS: &str = r#"{"task":"instructions/questions.write.md"}"#;
    const NODE: &str = r#"{"model":"claude-haiku-5-5","max_turns":15,"max_budget_usd":0.5,"instructions":{"task":"instructions/questions.write.md"},"stub":["stub-file","questions-open.yaml","questions.yaml"]}"#;

    #[test]
    fn a_check_subcommand_runs_the_same_under_loom() {
        let r = roots("check");
        let dissection = fs::read_to_string(fx().join("dissection.yaml")).unwrap();
        put(&r, "dissection.yaml", &dissection);
        for (i, q) in ["questions-open.yaml", "questions-answered.yaml"].iter().enumerate() {
            put(&r, "questions.yaml", &fs::read_to_string(fx().join(q)).unwrap());
            let d = driver(&r.0, "sdc-tool.sh", &["check-questions"], &[("SDC_ACTION", "questions.check"), ("SDC_CASE", "IDEA-1.intake")]);
            let l = loom(&r.1, "sdc-tool.sh", &["check-questions", "--action", "questions.check"], "IDEA-1.intake", "{}", None);
            assert_eq!(d.code, Some([1, 0][i]), "{q}: {d:#?}");
            same(q, &d, &l);
        }
        // A check that runs sdc-findings: from PATH under Loom, never from HOME.
        for f in ["idea.yaml", "requirements.yaml"] {
            put(&r, f, &fs::read_to_string(fx().join(f)).unwrap());
        }
        let d = driver(&r.0, "sdc-tool.sh", &["check-requirements"], &[("SDC_ACTION", "requirements.check"), ("SDC_CASE", "IDEA-1.intake")]);
        let l = loom(&r.1, "sdc-tool.sh", &["check-requirements", "--action", "requirements.check"], "IDEA-1.intake", "{}", None);
        assert_eq!(d.code, Some(0), "{d:#?}");
        same("check-requirements", &d, &l);
    }

    #[test]
    fn list_stories_reads_the_epic_from_loom_case() {
        let r = roots("list-stories");
        for root in [&r.0, &r.1] {
            let aep = |args: &[&str]| {
                let o = Command::new("aep").args(args).args(["--store", "aep", "--root", "."]).current_dir(root.join("work")).output().unwrap();
                assert!(o.status.success(), "aep {args:?}: {}", String::from_utf8_lossy(&o.stderr));
            };
            aep(&["plan", "artifact", "new", "epic", "billing", "--title", "Billing", "--summary", "invoices"]);
            aep(&["plan", "artifact", "new", "story", "invoices", "--title", "Invoices", "--summary", "covers billing.invoice.Create", "--relate", "delivers:epic:billing"]);
            aep(&["plan", "artifact", "new", "story", "other", "--title", "Other", "--summary", "covers nothing"]);
        }
        let d = driver(&r.0, "sdc-tool.sh", &["list-stories"], &[("SDC_ACTION", "stories.open"), ("SDC_CASE", "IDEA-1.billing")]);
        let l = loom(&r.1, "sdc-tool.sh", &["list-stories", "--action", "stories.open"], "IDEA-1.billing", "{}", None);
        assert_eq!((d.code, d.stdout.as_str()), (Some(0), "{\"stories\":[\"invoices\"]}\n"), "{d:#?}");
        same("list-stories", &d, &l);
    }

    /// An epic case: the epic is `LOOM_CASE_NAME`, whatever the case id looks like.
    #[test]
    fn list_stories_in_an_epic_case_reads_the_epic_from_loom_case_name() {
        let r = roots("list-stories-named");
        let aep = |args: &[&str]| {
            let o = Command::new("aep").args(args).args(["--store", "aep", "--root", "."]).current_dir(r.1.join("work")).output().unwrap();
            assert!(o.status.success(), "aep {args:?}: {}", String::from_utf8_lossy(&o.stderr));
        };
        aep(&["plan", "artifact", "new", "epic", "billing", "--title", "Billing", "--summary", "invoices"]);
        aep(&["plan", "artifact", "new", "story", "invoices", "--title", "Invoices", "--summary", "covers billing.invoice.Create", "--relate", "delivers:epic:billing"]);
        let l = loom_named(&r.1, "sdc-tool.sh", &["list-stories", "--action", "stories.open"], "IDEA-1.unrelated", "{}", None, &[("LOOM_CASE_NAME", "billing")]);
        assert_eq!((l.code, l.stdout.as_str()), (Some(0), "{\"stories\":[\"invoices\"]}\n"), "{l:#?}");
        let l = loom_named(&r.1, "sdc-tool.sh", &["list-stories", "--action", "stories.open"], "IDEA-1.billing.2", "{}", None, &[("LOOM_CASE_NAME", "billing")]);
        assert_eq!((l.code, l.stdout.as_str()), (Some(0), "{\"stories\":[\"invoices\"]}\n"), "{l:#?}");
    }

    /// The prompt llm-node wrote on the dry adapter for `node`, under Loom with `names`.
    fn named_prompt(name: &str, node: &str, case: &str, names: &[(&str, &str)]) -> String {
        let r = roots(name);
        let cfg = if node == "plan.decompose" { "plan.decompose" } else { "change.implement" };
        let instructions = if node == "plan.decompose" {
            r#"{"task":"instructions/plan.decompose.md","inputs":"instructions/plan.decompose.inputs.md"}"#
        } else {
            r#"{"task":"instructions/change.implement.md","coverage":"instructions/change.implement.coverage.md","coverage_inputs":"instructions/change.implement.coverage.inputs.md"}"#
        };
        let argv = [cfg, "--model", "claude-haiku-5-5", "--max-turns", "15", "--max-budget-usd", "0.5", "--instructions", instructions, "--adapter", "dry", "--stub", "stub-tree", "impl", "impl"];
        let l = loom_named(&r.1, "llm-node.sh", &argv, case, "{}", None, names);
        assert_eq!(l.code, Some(0), "{l:#?}");
        l.files[&format!(".sdc/llm/001-{node}/prompt.md")].clone()
    }

    /// A story case takes its story from `LOOM_CASE_NAME` and its epic from `LOOM_PARENT_CASE_NAME`,
    /// not from the case id: under a repeated epic the id is `IDEA-1.billing.2.story-a`.
    #[test]
    fn a_story_case_takes_story_and_epic_from_the_names_loom_gave() {
        let p = named_prompt("named-story", "change.implement", "IDEA-1.billing.2.story-a", &[("LOOM_CASE_NAME", "story-a"), ("LOOM_PARENT_CASE_NAME", "billing")]);
        assert!(p.contains("aep/story/story-a.md") && !p.contains("aep/story/2.md"), "{p}");
        let p = named_prompt("named-story-ids", "change.implement", "IDEA-1.x.y", &[("LOOM_CASE_NAME", "story-a"), ("LOOM_PARENT_CASE_NAME", "billing")]);
        assert!(p.contains("aep/story/story-a.md"), "{p}");
        let p = named_prompt("named-plan", "plan.decompose", "IDEA-1.billing.2.plan", &[("LOOM_CASE_NAME", "plan"), ("LOOM_PARENT_CASE_NAME", "billing")]);
        assert!(p.contains("Decompose epic 'billing'"), "{p}");
        let p = named_prompt("named-plan-ids", "plan.decompose", "IDEA-1.x.plan", &[("LOOM_CASE_NAME", "plan"), ("LOOM_PARENT_CASE_NAME", "billing")]);
        assert!(p.contains("Decompose epic 'billing'"), "{p}");
    }

    /// Without the names (a Loom before its wave 2) the tools cut the case id, as in driver mode.
    #[test]
    fn without_the_names_loom_mode_cuts_the_case_id() {
        let p = named_prompt("unnamed-story", "change.implement", "IDEA-1.billing.story-a", &[]);
        assert!(p.contains("aep/story/story-a.md"), "{p}");
        let p = named_prompt("unnamed-plan", "plan.decompose", "IDEA-1.billing.plan", &[]);
        assert!(p.contains("Decompose epic 'billing'"), "{p}");
    }

    /// A feedback record whose blob digest is not 64 hex digits is not a log path: the file it
    /// would name (`<loom-feedback>/x` for `../x`) is not read into `in/feedback.md`.
    #[test]
    fn a_retry_feedback_record_with_a_path_for_a_digest_reads_no_file() {
        let r = roots("retry-traversal");
        for f in ["idea.yaml", "dissection.yaml"] {
            put(&r, f, &fs::read_to_string(fx().join(f)).unwrap());
        }
        put(&r, "questions.yaml", "questions: []\n");
        put(&r, ".sdc/loom-feedback/x", "SECRET-OUTSIDE-THE-LOG-DIRECTORY\n");
        let doc = serde_json::json!({"format": "loom.retry-feedback/1", "case": "IDEA-1.intake", "action": "questions.write",
            "records": [{"evidence_id": "e1", "kind": "questions_shape", "subject": "questions", "subject_revision": "r1",
                "result": "invalid", "producer": "sdc-check", "observed_at": "2026-11-21T00:00:00Z", "blob_sha256": "../x"}],
            "logs": [{"blob_sha256": "../x", "tail": "the tail"}]});
        let doc_path = r.1.join("store-feedback.json");
        fs::write(&doc_path, doc.to_string()).unwrap();
        let stub = ["stub-file", "questions-open.yaml", "questions.yaml"];
        let l = loom(&r.1, "llm-node.sh", &node_argv("questions.write", &stub, &["--adapter", "dry", "--instructions", WRITE_INSTRUCTIONS]), "IDEA-1.intake", "{}", Some(&doc_path));
        assert_eq!(l.code, Some(0), "{l:#?}");
        let feedback = &l.files[".sdc/llm/001-questions.write/cwd/in/feedback.md"];
        assert!(feedback.contains("## questions_shape = invalid") && !feedback.contains("SECRET-OUTSIDE"), "{feedback}");
    }

    /// The named option, not a guess: each bad option value says which option it was.
    #[test]
    fn a_bad_option_value_names_the_option() {
        let r = roots("bad-options");
        let stub = ["stub-file", "dissection.yaml", "dissection.yaml"];
        for (extra, want) in [
            (vec!["--max-turns", "many"], "--max-turns takes a number"),
            (vec!["--max-budget-usd", "lots"], "--max-budget-usd takes a number"),
            (vec!["--instructions", "{not json"], "--instructions takes a JSON object"),
        ] {
            let mut argv = vec!["idea.dissect"];
            argv.extend_from_slice(&extra);
            argv.extend_from_slice(&["--stub"]);
            argv.extend_from_slice(&stub);
            let l = loom(&r.1, "llm-node.sh", &argv, "IDEA-1.intake", "{}", None);
            assert_eq!(l.code, Some(64), "{extra:?}: {l:#?}");
            assert!(l.stderr.contains(want), "{extra:?}: {}", l.stderr);
        }
    }

    #[test]
    fn an_agent_node_on_the_stub_and_on_dry_runs_the_same_under_loom() {
        // idea.dissect on the stub: the binding's stub argv writes dissection.yaml.
        let r = roots("stub");
        put(&r, "idea.yaml", &fs::read_to_string(fx().join("idea.yaml")).unwrap());
        let stub = ["stub-file", "dissection.yaml", "dissection.yaml"];
        let cfg = r#"{"model":"claude-haiku-5-5","max_turns":15,"max_budget_usd":0.5,"instructions":{"task":"instructions/idea.dissect.md"},"stub":["stub-file","dissection.yaml","dissection.yaml"]}"#;
        let d = driver(&r.0, "llm-node.sh", &["idea.dissect"], &[("SDC_ACTION", "idea.dissect"), ("SDC_CASE", "IDEA-1.intake"), ("SDC_ARGS", "{}"), ("SDC_LLM_NODE", cfg)]);
        let l = loom(&r.1, "llm-node.sh", &node_argv("idea.dissect", &stub, &["--instructions", r#"{"task":"instructions/idea.dissect.md"}"#]), "IDEA-1.intake", "{}", None);
        assert_eq!(d.code, Some(0), "{d:#?}");
        assert!(d.files.contains_key("dissection.yaml"), "{d:#?}");
        same("idea.dissect on the stub", &d, &l);
        // spec.author on dry: the prompt names the domain the arguments give (standard input under
        // Loom, SDC_ARGS for the driver), the request names the binding's model and caps.
        let r = roots("dry");
        for f in ["idea.yaml", "requirements.yaml", "context.yaml"] {
            put(&r, f, &fs::read_to_string(fx().join(f)).unwrap());
        }
        let cfg = r#"{"model":"claude-haiku-5-5","max_turns":40,"max_budget_usd":2,"instructions":{"task":"instructions/spec.author.md","inputs":"instructions/spec.author.inputs.md"},"stub":["stub-tree","billing-spec","spec"]}"#;
        let args = r#"{"domain":"billing"}"#;
        let d = driver(&r.0, "llm-node.sh", &["spec.author"], &[("SDC_ACTION", "spec.author"), ("SDC_CASE", "IDEA-1.billing.spec"), ("SDC_ARGS", args), ("SDC_LLM_NODE", cfg), ("SDC_LLM_ADAPTER", "dry")]);
        let argv = ["spec.author", "--model", "claude-haiku-5-5", "--max-turns", "40", "--max-budget-usd", "2", "--instructions", r#"{"task":"instructions/spec.author.md","inputs":"instructions/spec.author.inputs.md"}"#, "--adapter", "dry", "--stub", "stub-tree", "billing-spec", "spec"];
        let l = loom(&r.1, "llm-node.sh", &argv, "IDEA-1.billing.spec", args, None);
        assert_eq!(d.code, Some(0), "{d:#?}");
        let prompt = &d.files[".sdc/llm/001-spec.author/prompt.md"];
        assert!(prompt.contains("format: ess/1, system: billing, version: v1"), "{prompt}");
        assert!(d.files[".sdc/llm/001-spec.author/request.json"].contains("\"max_turns\": 40"), "{d:#?}");
        assert!(d.files.contains_key("spec/system.yaml"), "{d:#?}");
        same("spec.author on dry", &d, &l);
    }

    /// A person's rejection reaches the writer as the reasons text (Loom's `--result rejected
    /// --reasons-file`): the stub records them in the rejected artifact, which makes the rework a
    /// new revision, and the model adapter's `in/feedback.md` carries them.
    #[test]
    fn a_persons_rejection_reaches_the_stub_writer_as_reasons_text() {
        let r = roots("person-rejection");
        put(&r, "requirements.yaml", &fs::read_to_string(fx().join("requirements.yaml")).unwrap());
        put(&r, "context.yaml", &fs::read_to_string(fx().join("context.yaml")).unwrap());
        let reasons = "The context lacks the named risks.\nName the failure modes of a missing connection.\n";
        let blob = "5e".repeat(32);
        let doc = serde_json::json!({"format": "loom.retry-feedback/1", "case": "IDEA-1.direction", "action": "context.default",
            "records": [{"evidence_id": "e1", "kind": "context_approval", "subject": "context", "subject_revision": "r2",
                "result": "rejected", "producer": "person:owner", "observed_at": "2026-11-22T00:00:00Z", "blob_sha256": blob,
                "action": "context.approve", "exit": null}],
            "logs": [{"blob_sha256": blob, "tail": reasons}]});
        let doc_path = r.1.join("feedback-doc.json");
        fs::write(&doc_path, doc.to_string()).unwrap();
        let stub = ["stub-file", "context.yaml", "context.yaml"];
        let l = loom(&r.1, "llm-node.sh", &node_argv("context.default", &stub, &[]), "IDEA-1.direction", "{}", Some(&doc_path));
        assert_eq!(l.code, Some(0), "{l:#?}");
        let context = &l.files["context.yaml"];
        assert!(
            context.ends_with("# rework of the rejection of context.approve:\n# - The context lacks the named risks.\n# - Name the failure modes of a missing connection.\n"),
            "{context}"
        );
        // The same document through the model adapter's dry run reaches the writer's input.
        let dry = loom(&r.1, "llm-node.sh", &node_argv("context.default", &stub, &["--adapter", "dry", "--instructions", r#"{"task":"instructions/context.default.md"}"#]), "IDEA-1.direction", "{}", Some(&doc_path));
        let feedback = dry.files.iter().find(|(k, _)| k.ends_with("/cwd/in/feedback.md")).map(|(_, v)| v.clone()).unwrap_or_default();
        assert!(feedback.contains("context_approval = rejected") && feedback.contains("Name the failure modes of a missing connection."), "{dry:#?}");
    }

    #[test]
    fn a_retried_writer_reads_its_feedback_from_loom_retry_feedback() {
        let r = roots("retry");
        for f in ["idea.yaml", "dissection.yaml"] {
            put(&r, f, &fs::read_to_string(fx().join(f)).unwrap());
        }
        put(&r, "questions.yaml", "questions: []\n");
        let stub = ["stub-file", "questions-open.yaml", "questions.yaml"];
        let env = [("SDC_ACTION", "questions.write"), ("SDC_CASE", "IDEA-1.intake"), ("SDC_ARGS", "{}"), ("SDC_LLM_NODE", NODE), ("SDC_LLM_ADAPTER", "dry")];
        // No feedback: questions.yaml exists, so questions.write is not called.
        let d = driver(&r.0, "llm-node.sh", &["questions.write"], &env);
        let l = loom(&r.1, "llm-node.sh", &node_argv("questions.write", &stub, &["--adapter", "dry", "--instructions", WRITE_INSTRUCTIONS]), "IDEA-1.intake", "{}", None);
        assert!(d.code == Some(0) && d.stdout.contains("questions.write not called"), "{d:#?}");
        same("questions.write, no feedback", &d, &l);
        // The check's record that sent it back, with its log: the driver's SDC_FEEDBACK, and Loom's
        // loom.retry-feedback/1 document holding the record and the log's tail.
        let log = "check-questions: scope-changing question lacks a key\nq1: lacks excerpt\n";
        fs::write(r.0.join("check.log"), log).unwrap();
        let fb = serde_json::json!([{"kind": "questions_shape", "result": "invalid", "subject": "questions",
            "action": null, "exit": null, "log": r.0.join("check.log")}]);
        fs::write(r.0.join("feedback.json"), fb.to_string()).unwrap();
        let blob = "4f".repeat(32);
        let doc = serde_json::json!({"format": "loom.retry-feedback/1", "case": "IDEA-1.intake", "action": "questions.write",
            "records": [{"evidence_id": "e1", "kind": "questions_shape", "subject": "questions", "subject_revision": "r1",
                "result": "invalid", "producer": "sdc-check", "observed_at": "2026-11-21T00:00:00Z", "blob_sha256": blob}],
            "logs": [{"blob_sha256": blob, "tail": log}]});
        fs::create_dir_all(r.1.join("store/blobs")).unwrap();
        let doc_path = r.1.join("store/blobs/feedback");
        fs::write(&doc_path, doc.to_string()).unwrap();
        let fb_path = r.0.join("feedback.json").display().to_string();
        let mut env = env.to_vec();
        env.push(("SDC_FEEDBACK", &fb_path));
        let d = driver(&r.0, "llm-node.sh", &["questions.write"], &env);
        let l = loom(&r.1, "llm-node.sh", &node_argv("questions.write", &stub, &["--adapter", "dry", "--instructions", WRITE_INSTRUCTIONS]), "IDEA-1.intake", "{}", Some(&doc_path));
        assert_eq!(d.code, Some(0), "{d:#?}");
        let feedback = &d.files[".sdc/llm/001-questions.write/cwd/in/feedback.md"];
        assert!(feedback.contains("## questions_shape = invalid (from ?, exit ?)") && feedback.contains("q1: lacks excerpt"), "{feedback}");
        same("questions.write, retried", &d, &l);
    }

    /// A `claude` that calls no model: it checks that the adapter put the login into its config
    /// home, writes the dissection a writer would, and marks that it ran.
    fn stub_claude(name: &str) -> (PathBuf, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("loom-mode").join(name);
        let _ = fs::remove_dir_all(&base);
        let bin = base.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let mark = base.join("claude-ran");
        let script = format!(
            "#!/usr/bin/env bash\n[[ -f \"$CLAUDE_CONFIG_DIR/.credentials.json\" ]] || exit 9\nmkdir -p out && cp '{}' out/dissection.yaml\necho ran > '{}'\n\
             echo '{{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"num_turns\":1,\"total_cost_usd\":0,\"duration_ms\":1,\"usage\":{{}}}}'\n",
            fx().join("dissection.yaml").display(),
            mark.display()
        );
        let claude = bin.join("claude");
        fs::write(&claude, script).unwrap();
        fs::set_permissions(&claude, fs::Permissions::from_mode(0o755)).unwrap();
        (bin, mark)
    }

    /// Loom starts the real adapter with `PATH` and `LOOM_*` alone: no `HOME`, no
    /// `SDC_CLAUDE_CREDENTIALS`, no `ANTHROPIC_API_KEY`. It stops before any call, with a message
    /// that names every variable it could take the login from, not with an unbound variable.
    #[test]
    fn the_claude_cli_adapter_without_home_names_the_variables_it_needs() {
        let r = roots("claude-cli-no-home");
        let (bin, mark) = stub_claude("claude-cli-no-home-bin");
        let work = r.1.join("work");
        fs::write(work.join("prompt.md"), "prompt\n").unwrap();
        fs::create_dir_all(work.join("cwd")).unwrap();
        let request = serde_json::json!({"record_dir": work.join("rec"), "prompt_file": work.join("prompt.md"), "cwd": work.join("cwd"),
            "model": "claude-haiku-5-5", "max_turns": 1, "max_budget_usd": 0.1, "operations": ["file.read", "file.write"]});
        fs::write(work.join("request.json"), request.to_string()).unwrap();
        let mut c = Command::new(tools().join("llm-adapter-claude-cli.sh"));
        c.arg(work.join("request.json")).env_clear().env("PATH", format!("{}:{}", bin.display(), path())).env("LOOM_CASE", "IDEA-1.intake");
        let l = run(&r.1, c, "");
        assert_eq!(l.code, Some(127), "{l:#?}");
        for name in ["HOME", "CLAUDE_CONFIG_DIR", "SDC_CLAUDE_CREDENTIALS", "ANTHROPIC_API_KEY"] {
            assert!(l.stderr.contains(name), "the message does not name {name}: {}", l.stderr);
        }
        assert!(!l.stderr.contains("unbound variable") && !mark.exists(), "{l:#?}");
    }

    /// The binding passes the login as an argv entry (`--claude-credentials <file>`): llm-node
    /// exports it as SDC_CLAUDE_CREDENTIALS and a node runs on the claude-cli adapter under an
    /// environment of `PATH` and `LOOM_CASE`.
    #[test]
    fn a_node_on_the_claude_cli_adapter_takes_its_login_from_the_binding_under_loom() {
        let r = roots("claude-cli-login");
        let (bin, mark) = stub_claude("claude-cli-login-bin");
        put(&r, "idea.yaml", &fs::read_to_string(fx().join("idea.yaml")).unwrap());
        let login = r.1.join("login.json");
        fs::write(&login, "{}").unwrap();
        let stub = ["stub-file", "dissection.yaml", "dissection.yaml"];
        let argv = node_argv("idea.dissect", &stub, &["--adapter", "claude-cli", "--claude-credentials", login.to_str().unwrap(), "--instructions", r#"{"task":"instructions/idea.dissect.md"}"#]);
        let mut c = Command::new(tools().join("llm-node.sh"));
        c.args(&argv).env_clear().env("PATH", format!("{}:{}", bin.display(), path())).env("LOOM_CASE", "IDEA-1.intake");
        let l = run(&r.1, c, "{}");
        assert_eq!(l.code, Some(0), "{l:#?}");
        assert!(mark.exists() && l.files.contains_key("dissection.yaml"), "{l:#?}");
    }

    // ------------------------------------------------------------------------------------------
    // The login of the metaharness adapter, and of the other two model paths, under Loom's
    // environment (`PATH` and `LOOM_*` alone). Nothing here calls a model: the programs the
    // adapters start are stand-ins in POSIX sh. A stand-in finds its log directory through its own
    // resolved path (`<base>/bin/<name>` and `<base>/log`), because the environment is empty.

    const DUMMY_LOGIN: &str = "dummy-login";

    /// A `claude` that makes no request. It exits 9 when the login is not in its config home.
    /// Otherwise it records the login's text and mode and the HOME it got, copies the files a
    /// test put beside it (`write-out`: a writer's dissection, `write-decision`: a decider's
    /// reply) and prints one `result` line as `claude -p` ends a session, with a structured output.
    const CLAUDE_STANDIN: &str = r#"#!/bin/sh
me=$(readlink -f "$0"); b=$(dirname "$me")/..; d=$b/log
if [ ! -f "$CLAUDE_CONFIG_DIR/.credentials.json" ]; then echo login-absent > "$d/claude.$$"; exit 9; fi
{ echo login-present; echo "login=$(cat "$CLAUDE_CONFIG_DIR/.credentials.json")"
  echo "mode=$(stat -c %a "$CLAUDE_CONFIG_DIR/.credentials.json")"; echo "home=${HOME:-}"
  echo "config=$CLAUDE_CONFIG_DIR"; echo "tmpdir=${TMPDIR:-}"; echo "ppid=$PPID"; } > "$d/claude.$$"
[ -f "$d/hang" ] && { echo $$ > "$d/claude-pid"; echo $PPID > "$d/claude-ppid"; exec sleep 60; }
[ -f "$b/write-out" ] && { mkdir -p out && cp "$b/write-out" out/dissection.yaml; }
[ -f "$b/write-decision" ] && { mkdir -p out && cp "$b/write-decision" out/decision.json; }
printf '%s\n' '{"type":"result","subtype":"success","is_error":false,"num_turns":1,"total_cost_usd":0,"duration_ms":1,"duration_api_ms":0,"session_id":"s","usage":{"input_tokens":0,"output_tokens":0},"result":"ok","terminal_reason":"completed","structured_output":{"decision":"approve","reasons":["ok"],"cites":[]}}'
"#;

    /// A `metaharness` that makes no request and refuses as the real one does when its credential
    /// source is the operator's login and no file is where it looks: `$HOME/.claude/.credentials.json`
    /// (metaharness `credentials_file`, which reads the process environment's HOME and nothing
    /// else). It records its argv and what it found there. `fail` in its log directory makes it
    /// end with exit 1 after it looked.
    const METAHARNESS_STANDIN: &str = r#"#!/bin/sh
me=$(readlink -f "$0"); b=$(dirname "$me")/..; d=$b/log
printf '%s\n' "$@" > "$d/argv.$$"
cwd=; creds=operator-login
while [ $# -gt 0 ]; do case $1 in --cwd) cwd=$2; shift 2 ;; --credentials) creds=$2; shift 2 ;; *) shift ;; esac; done
f=${HOME:-/nonexistent}/.claude/.credentials.json
echo "home=${HOME:-}" > "$d/seen.$$"
echo "tmpdir=${TMPDIR:-}" >> "$d/seen.$$"
echo "credentials=$creds" >> "$d/seen.$$"
if [ "$creds" = operator-login ]; then
  if [ -z "${HOME:-}" ] || [ ! -f "$f" ]; then
    echo "metaharness: the adapter refused to plan the launch: the run's credential source needs the operator's credential file and none was named" >&2
    echo refused >> "$d/seen.$$"; exit 2
  fi
  { echo "login=$(cat "$f")"; echo "file-mode=$(stat -c %a "$f")"; echo "home-mode=$(stat -c %a "$HOME")"
    echo "dir-mode=$(stat -c %a "$HOME/.claude")"; } >> "$d/seen.$$"
fi
[ -f "$d/fail" ] && { echo "metaharness: the stand-in ends the session with an error" >&2; exit 1; }
[ -f "$d/hang" ] && { echo $$ > "$d/mh-pid"; exec sleep 60; }
[ -n "$cwd" ] && [ -f "$b/write-out" ] && { mkdir -p "$cwd/out" && cp "$b/write-out" "$cwd/out/dissection.yaml"; }
[ -n "$cwd" ] && [ -f "$b/write-decision" ] && { mkdir -p "$cwd/out" && cp "$b/write-decision" "$cwd/out/decision.json"; }
printf '%s\n' '{"format":"metaharness.event/1","seq":1,"run":"standin","event":"session.ended","is_error":false,"subtype":"success","terminal_reason":"completed","num_turns":1,"duration_ms":1,"total_cost_usd":0.0,"usage":{}}'
"#;

    /// A `sdc-reply` for the model paths that need the case's store: it answers what the tools ask
    /// before and after the call with canned documents and hands `adapters` to the real binary.
    fn reply_standin() -> String {
        format!(
            r#"#!/bin/sh
[ "$1" = adapters ] && exec '{real}' "$@"
sub=$1; shift
cwd=
while [ $# -gt 0 ]; do case $1 in --cwd) cwd=$2; shift 2 ;; *) shift ;; esac; done
case $sub in
  review-inputs) mkdir -p "$cwd/in"; printf 'review input\n' > "$cwd/in/review.md"; echo '{{"base":"mainline","document_bytes":13,"coverage":false}}' ;;
  schema) echo '{{"type":"object"}}' ;;
  check-review) echo '{{"notes":[]}}' ;;
  citable) echo '{{"artifacts":[],"examples":{{}}}}' ;;
  rejections) echo '[]' ;;
  *) exit 0 ;;
esac
"#,
            real = env!("CARGO_BIN_EXE_sdc-reply")
        )
    }

    struct Standin {
        base: PathBuf,
        bin: PathBuf,
        log: PathBuf,
    }

    impl Standin {
        /// `<tmp>/loom-mode/<name>-standin/{bin,log}` with the given programs in `bin/`.
        fn new(name: &str, programs: &[(&str, &str)]) -> Standin {
            use std::os::unix::fs::PermissionsExt;
            let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("loom-mode").join(format!("{name}-standin"));
            let _ = fs::remove_dir_all(&base);
            let (bin, log) = (base.join("bin"), base.join("log"));
            fs::create_dir_all(&bin).unwrap();
            fs::create_dir_all(&log).unwrap();
            for (n, text) in programs {
                fs::write(bin.join(n), text).unwrap();
                fs::set_permissions(bin.join(n), fs::Permissions::from_mode(0o755)).unwrap();
            }
            Standin { base, bin, log }
        }

        /// The PATH of a bound program: the stand-ins first.
        fn path(&self) -> String {
            format!("{}:{}", self.bin.display(), path())
        }

        /// What the stand-ins wrote under `prefix` in their log directory, one text per file.
        fn logged(&self, prefix: &str) -> Vec<String> {
            let mut out = Vec::new();
            for e in fs::read_dir(&self.log).unwrap() {
                let e = e.unwrap();
                if e.file_name().to_string_lossy().starts_with(prefix) {
                    out.push(fs::read_to_string(e.path()).unwrap());
                }
            }
            out
        }
    }

    /// The value of `key=` in a stand-in's log text.
    fn logged_value(text: &str, key: &str) -> String {
        text.lines().find_map(|l| l.strip_prefix(&format!("{key}="))).unwrap_or_else(|| panic!("no {key}= in {text}")).to_owned()
    }

    /// A file whose text is `dummy-login`, beside the roots of a test and never inside them.
    fn dummy_login(r: &(PathBuf, PathBuf)) -> PathBuf {
        let p = r.1.parent().unwrap().join("dummy-login-file");
        fs::write(&p, DUMMY_LOGIN).unwrap();
        p
    }

    /// The login as the runner hands it over: the dummy file, and a link to it called `login` in a
    /// directory of the user's with mode 700 (`runbook/src/loom_run/private.rs`). Returns the
    /// link. Both lie beside the roots of a test, never inside them.
    fn private_login(r: &(PathBuf, PathBuf)) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let file = dummy_login(r);
        let dir = r.1.parent().unwrap().join("private");
        fs::create_dir_all(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        let link = dir.join("login");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        link
    }

    /// What lies in the private directory of `private_login`, by name.
    fn private_listing(link: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(link.parent().unwrap()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    /// Starts `c` as `run` does but does not wait: for a call that is to be killed.
    fn start(root: &Path, mut c: Command, stdin: &str) -> std::process::Child {
        c.current_dir(root.join("work")).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());
        let mut child = c.spawn().unwrap();
        child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
        child
    }

    /// The pid a stand-in wrote to `name` in its log directory, once it is there.
    fn wait_for_pid(st: &Standin, name: &str) -> u32 {
        let t = std::time::Instant::now();
        loop {
            if let Ok(text) = fs::read_to_string(st.log.join(name)) {
                if let Ok(pid) = text.trim().parse() {
                    return pid;
                }
            }
            assert!(t.elapsed().as_secs() < 20, "{name} did not appear");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// Ends a process by its pid.
    fn kill(signal: &str, pid: u32) {
        assert!(Command::new("kill").args([signal, &pid.to_string()]).status().unwrap().success());
    }

    /// Every file below `dir` whose path or text holds one of `needles`.
    fn hits(dir: &Path, needles: &[String]) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let meta = fs::symlink_metadata(&p).unwrap();
            let name = p.display().to_string();
            if needles.iter().any(|n| name.contains(n.as_str())) {
                out.push(p.clone());
            }
            if meta.is_dir() {
                out.extend(hits(&p, needles));
            } else if meta.is_file() && needles.iter().any(|n| String::from_utf8_lossy(&fs::read(&p).unwrap()).contains(n.as_str())) {
                out.push(p);
            }
        }
        out
    }

    /// No private directory of an adapter is left in a call's record directory.
    fn assert_no_private_copy(rec: &Path) {
        for e in fs::read_dir(rec).unwrap() {
            let name = e.unwrap().file_name().to_string_lossy().into_owned();
            assert!(!name.contains("home"), "{name} is left in {}", rec.display());
        }
    }

    fn rec_of(r: &(PathBuf, PathBuf), node: &str) -> PathBuf {
        r.1.join("work/.sdc/llm").join(format!("001-{node}"))
    }

    /// `llm-node.sh idea.dissect` started as Loom starts it, on `adapter`, with the dummy login.
    fn dissect_under_loom(r: &(PathBuf, PathBuf), st: &Standin, adapter: &str, login: Option<&Path>) -> Run {
        put(r, "idea.yaml", &fs::read_to_string(fx().join("idea.yaml")).unwrap());
        let stub = ["stub-file", "dissection.yaml", "dissection.yaml"];
        let mut extra = vec!["--adapter", adapter];
        let login_text = login.map(|l| l.display().to_string());
        if let Some(l) = &login_text {
            extra.extend(["--claude-credentials", l.as_str()]);
        }
        extra.extend(["--instructions", r#"{"task":"instructions/idea.dissect.md"}"#]);
        let argv = node_argv("idea.dissect", &stub, &extra);
        let mut c = Command::new(tools().join("llm-node.sh"));
        c.args(&argv).env_clear().env("PATH", st.path()).env("LOOM_CASE", "IDEA-1.intake");
        run(&r.1, c, "{}")
    }

    /// A request for an adapter, as llm-node.sh writes it.
    fn adapter_request(r: &(PathBuf, PathBuf)) -> PathBuf {
        let work = r.1.join("work");
        fs::write(work.join("prompt.md"), "prompt\n").unwrap();
        fs::create_dir_all(work.join("cwd")).unwrap();
        let request = serde_json::json!({"record_dir": work.join("rec"), "prompt_file": work.join("prompt.md"), "cwd": work.join("cwd"),
            "model": "claude-haiku-5-5", "max_turns": 1, "max_budget_usd": 0.1, "operations": ["file.read"], "node": "idea.dissect", "attempt": 1});
        fs::write(work.join("request.json"), request.to_string()).unwrap();
        work.join("request.json")
    }

    #[test]
    fn a_node_on_the_metaharness_adapter_takes_its_login_from_the_binding_under_loom() {
        let r = roots("mh-login");
        let st = Standin::new("mh-login", &[("metaharness", METAHARNESS_STANDIN)]);
        fs::copy(fx().join("dissection.yaml"), st.base.join("write-out")).unwrap();
        let login = private_login(&r);
        let l = dissect_under_loom(&r, &st, "metaharness", Some(&login));
        assert_eq!(l.code, Some(0), "{l:#?}");
        assert!(l.files.contains_key("dissection.yaml"), "{l:#?}");
        // The place the real binary looks (`$HOME/.claude/.credentials.json`) held the login, in a
        // private directory: 700 for the home and its `.claude`, 600 for the file.
        let seen = st.logged("seen.");
        assert_eq!(seen.len(), 1, "{seen:?}");
        assert_eq!(logged_value(&seen[0], "login"), DUMMY_LOGIN, "{seen:?}");
        assert_eq!((logged_value(&seen[0], "file-mode"), logged_value(&seen[0], "home-mode"), logged_value(&seen[0], "dir-mode")), ("600".into(), "700".into(), "700".into()), "{seen:?}");
        // The private home lay in the runner's private directory, not in the run directory, it is
        // gone, and the login's path and text are in no file of the tree.
        let home = logged_value(&seen[0], "home");
        assert!(Path::new(&home).starts_with(login.parent().unwrap()), "the private home is not in the private directory: {seen:?}");
        assert_eq!(logged_value(&seen[0], "tmpdir"), home, "the scratch root of metaharness is the private home");
        assert!(!Path::new(&home).exists(), "the private home is left: {seen:?}");
        assert_eq!(private_listing(&login), ["login"]);
        assert_no_private_copy(&rec_of(&r, "idea.dissect"));
        let found = hits(&r.1, &[login.display().to_string(), DUMMY_LOGIN.to_owned()]);
        assert!(found.is_empty(), "the login is in {found:?}");
    }

    #[test]
    fn a_failed_metaharness_call_leaves_no_private_copy_of_the_login() {
        let r = roots("mh-login-fail");
        let st = Standin::new("mh-login-fail", &[("metaharness", METAHARNESS_STANDIN)]);
        fs::write(st.log.join("fail"), "").unwrap();
        let login = private_login(&r);
        let l = dissect_under_loom(&r, &st, "metaharness", Some(&login));
        assert_eq!(l.code, Some(1), "{l:#?}");
        let seen = st.logged("seen.");
        assert_eq!(seen.len(), 1, "{seen:?}");
        assert_eq!(logged_value(&seen[0], "login"), DUMMY_LOGIN, "the stand-in did not get the login: {seen:?}");
        assert!(!Path::new(&logged_value(&seen[0], "home")).exists(), "the private home is left after a failure: {seen:?}");
        assert_eq!(private_listing(&login), ["login"]);
        assert_no_private_copy(&rec_of(&r, "idea.dissect"));
        let found = hits(&r.1, &[login.display().to_string(), DUMMY_LOGIN.to_owned()]);
        assert!(found.is_empty(), "the login is in {found:?}");
    }

    #[test]
    fn the_metaharness_adapter_without_a_login_named_keeps_the_home_it_has() {
        let r = roots("mh-login-home");
        let st = Standin::new("mh-login-home", &[("metaharness", METAHARNESS_STANDIN)]);
        let home = r.1.parent().unwrap().join("driver-home");
        fs::create_dir_all(home.join(".claude")).unwrap();
        fs::write(home.join(".claude/.credentials.json"), DUMMY_LOGIN).unwrap();
        let request = adapter_request(&r);
        let mut c = Command::new(tools().join("llm-adapter-metaharness.sh"));
        c.arg(&request).env_clear().env("PATH", st.path()).env("HOME", &home);
        let l = run(&r.1, c, "");
        assert_eq!(l.code, Some(0), "{l:#?}");
        let seen = st.logged("seen.");
        assert_eq!(logged_value(&seen[0], "home"), home.display().to_string(), "the driver's HOME is kept: {seen:?}");
        assert_eq!(logged_value(&seen[0], "login"), DUMMY_LOGIN);
        assert_no_private_copy(&r.1.join("work/rec"));
    }

    #[test]
    fn with_a_model_endpoint_the_metaharness_adapter_keeps_credentials_none_and_copies_no_login() {
        let r = roots("mh-login-endpoint");
        let st = Standin::new("mh-login-endpoint", &[("metaharness", METAHARNESS_STANDIN)]);
        let login = private_login(&r);
        let request = adapter_request(&r);
        let mut c = Command::new(tools().join("llm-adapter-metaharness.sh"));
        c.arg(&request).env_clear().env("PATH", st.path()).env("SDC_LLM_ENDPOINT", "http://127.0.0.1:9").env("SDC_CLAUDE_CREDENTIALS", &login);
        let l = run(&r.1, c, "");
        assert_eq!(l.code, Some(0), "{l:#?}");
        let seen = st.logged("seen.");
        assert_eq!(logged_value(&seen[0], "credentials"), "none", "{seen:?}");
        assert_eq!(logged_value(&seen[0], "home"), "", "no home is made for a run that needs no login: {seen:?}");
        let argv = st.logged("argv.").concat();
        assert!(argv.contains("--model-endpoint\nhttp://127.0.0.1:9\n") && argv.contains("--credentials\nnone\n"), "{argv}");
        assert_no_private_copy(&r.1.join("work/rec"));
    }

    /// Whether a program of that name is on this process's PATH.
    fn on_path(name: &str) -> bool {
        use std::os::unix::fs::PermissionsExt;
        std::env::var("PATH").unwrap_or_default().split(':').any(|d| fs::metadata(Path::new(d).join(name)).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0))
    }

    /// The real `metaharness` plans the launch from what the adapter gives it, and starts a
    /// `claude` that is a stand-in (no request). Skipped where there is no `metaharness` on PATH.
    #[test]
    fn the_real_metaharness_finds_the_login_the_adapter_gives_it_skipped_without_metaharness_on_path() {
        if !on_path("metaharness") {
            eprintln!("skipped: no metaharness on PATH (the forge has none)");
            return;
        }
        let r = roots("mh-login-real");
        let st = Standin::new("mh-login-real", &[("claude", CLAUDE_STANDIN)]);
        let login = private_login(&r);
        let request = adapter_request(&r);
        let mut c = Command::new(tools().join("llm-adapter-metaharness.sh"));
        c.arg(&request).env_clear().env("PATH", st.path()).env("LOOM_CASE", "IDEA-1.intake").env("SDC_CLAUDE_CREDENTIALS", &login);
        let l = run(&r.1, c, "");
        assert_eq!(l.code, Some(0), "{l:#?}");
        let response: serde_json::Value = serde_json::from_str(&l.stdout).unwrap();
        assert_eq!((response["exit"].as_i64(), response["terminal_reason"].as_str()), (Some(0), Some("completed")), "{l:#?}");
        let claude = st.logged("claude.");
        assert_eq!(claude.len(), 1, "the stand-in claude did not run once: {claude:?}\n{l:#?}");
        assert_eq!((logged_value(&claude[0], "login"), logged_value(&claude[0], "mode")), (DUMMY_LOGIN.into(), "600".into()), "{claude:?}");
        let found = hits(&r.1, &[login.display().to_string(), DUMMY_LOGIN.to_owned()]);
        assert!(found.is_empty(), "the login is in {found:?}");
        assert_no_private_copy(&r.1.join("work/rec"));
        assert!(!Path::new(&logged_value(&claude[0], "home")).exists(), "the private home is left: {claude:?}");
        // Where metaharness put the config home it gave `claude` and the scratch root of its own
        // (its `TMPDIR` is the scratch root's `tmp`): below the private home, so below the runner's
        // private directory, and nowhere in the system's temporary directory.
        let (config, tmpdir) = (logged_value(&claude[0], "config"), logged_value(&claude[0], "tmpdir"));
        let private = login.parent().unwrap();
        assert!(Path::new(&config).starts_with(private) && Path::new(&tmpdir).starts_with(private), "config {config} or tmp {tmpdir} lies outside {}", private.display());
        assert_eq!(private_listing(&login), ["login"], "something of the call is left in the private directory");
    }

    /// Kills the adapter with SIGKILL while the program it started runs, ends that program by its
    /// pid, and says where the copy of the login was: not in the run directory.
    fn killed_while_running(r: &(PathBuf, PathBuf), st: &Standin, adapter: &str, login: &Path, pid_file: &str) -> Vec<String> {
        let request = adapter_request(r);
        fs::write(st.log.join("hang"), "").unwrap();
        let mut c = Command::new(tools().join(adapter));
        c.arg(&request).env_clear().env("PATH", st.path()).env("LOOM_CASE", "IDEA-1.intake").env("SDC_CLAUDE_CREDENTIALS", login);
        let mut child = start(&r.1, c, "");
        let running = wait_for_pid(st, pid_file);
        kill("-KILL", child.id());
        child.wait().unwrap();
        let found = hits(&r.1, &[login.display().to_string(), DUMMY_LOGIN.to_owned()]);
        let rec = r.1.join("work/rec");
        let left_in_rec: Vec<String> = fs::read_dir(&rec).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).filter(|n| n.contains("home")).collect();
        let in_private = private_listing(login);
        kill("-KILL", running);
        assert!(found.is_empty(), "after a SIGKILL the login is in {found:?}");
        assert!(left_in_rec.is_empty(), "after a SIGKILL {left_in_rec:?} is left in the record directory");
        in_private
    }

    #[test]
    fn a_sigkill_of_the_metaharness_adapter_leaves_no_copy_of_the_login_in_the_run_directory() {
        let r = roots("mh-kill");
        let st = Standin::new("mh-kill", &[("metaharness", METAHARNESS_STANDIN)]);
        let login = private_login(&r);
        let in_private = killed_while_running(&r, &st, "llm-adapter-metaharness.sh", &login, "mh-pid");
        // The copy exists, so the test looked at the right moment, and lies beside the link.
        assert_eq!(in_private.len(), 2, "{in_private:?}");
        assert!(in_private.iter().any(|n| n.starts_with("h.")), "{in_private:?}");
    }

    #[test]
    fn a_sigkill_of_the_claude_cli_adapter_leaves_no_copy_of_the_login_in_the_run_directory() {
        let r = roots("cli-kill");
        let st = Standin::new("cli-kill", &[("claude", CLAUDE_STANDIN)]);
        let login = private_login(&r);
        let in_private = killed_while_running(&r, &st, "llm-adapter-claude-cli.sh", &login, "claude-pid");
        assert_eq!(in_private.len(), 2, "{in_private:?}");
        assert!(in_private.iter().any(|n| n.starts_with("config-home.")), "{in_private:?}");
    }

    /// The claude-cli adapter when the login is not in a private directory (the driver's runs):
    /// the config home is made in the record directory, as it always was.
    #[test]
    fn the_claude_cli_adapter_keeps_its_config_home_in_the_record_directory_when_the_login_is_not_in_a_private_directory() {
        let r = roots("cli-plain");
        let st = Standin::new("cli-plain", &[("claude", CLAUDE_STANDIN)]);
        let login = dummy_login(&r);
        let request = adapter_request(&r);
        let mut c = Command::new(tools().join("llm-adapter-claude-cli.sh"));
        c.arg(&request).env_clear().env("PATH", st.path()).env("SDC_CLAUDE_CREDENTIALS", &login);
        let l = run(&r.1, c, "");
        assert_eq!(l.code, Some(0), "{l:#?}");
        let claude = st.logged("claude.");
        assert_eq!(logged_value(&claude[0], "login"), DUMMY_LOGIN);
        assert!(Path::new(&logged_value(&claude[0], "config")).starts_with(r.1.join("work/rec")), "{claude:?}");
        assert_no_private_copy(&r.1.join("work/rec"));
    }

    /// The claude-cli adapter with the login in the runner's private directory: the whole config
    /// home lies there (it is the directory `claude` is given as its config), the record directory
    /// holds none.
    #[test]
    fn the_claude_cli_adapter_makes_its_config_home_in_the_private_directory_of_the_login() {
        let r = roots("cli-private");
        let st = Standin::new("cli-private", &[("claude", CLAUDE_STANDIN)]);
        let login = private_login(&r);
        let request = adapter_request(&r);
        let mut c = Command::new(tools().join("llm-adapter-claude-cli.sh"));
        c.arg(&request).env_clear().env("PATH", st.path()).env("SDC_CLAUDE_CREDENTIALS", &login);
        let l = run(&r.1, c, "");
        assert_eq!(l.code, Some(0), "{l:#?}");
        let claude = st.logged("claude.");
        assert_eq!(logged_value(&claude[0], "login"), DUMMY_LOGIN);
        assert!(Path::new(&logged_value(&claude[0], "config")).starts_with(login.parent().unwrap()), "{claude:?}");
        assert_eq!(private_listing(&login), ["login"]);
        assert_no_private_copy(&r.1.join("work/rec"));
    }

    /// What the metaharness adapter does with `SDC_CLAUDE_CREDENTIALS` set and a `HOME` of the
    /// caller: `home_login` says whether `$HOME/.claude/.credentials.json` exists. Returns the log
    /// of the stand-in and the login link.
    fn mh_with_home(name: &str, home_login: bool, private: bool) -> (Vec<String>, PathBuf, PathBuf) {
        let r = roots(name);
        let st = Standin::new(name, &[("metaharness", METAHARNESS_STANDIN)]);
        let login = if private { private_login(&r) } else { dummy_login(&r) };
        let home = r.1.parent().unwrap().join("callers-home");
        fs::create_dir_all(home.join(".claude")).unwrap();
        if home_login {
            fs::write(home.join(".claude/.credentials.json"), "callers-login").unwrap();
        }
        let request = adapter_request(&r);
        let mut c = Command::new(tools().join("llm-adapter-metaharness.sh"));
        c.arg(&request).env_clear().env("PATH", st.path()).env("HOME", &home).env("SDC_CLAUDE_CREDENTIALS", &login);
        let l = run(&r.1, c, "");
        assert_eq!(l.code, Some(if home_login || private { 0 } else { 2 }), "{l:#?}");
        (st.logged("seen."), login, home)
    }

    #[test]
    fn the_metaharness_adapter_keeps_a_home_that_has_a_login_also_when_a_login_file_is_named() {
        let (seen, login, home) = mh_with_home("mh-home-has-login", true, true);
        assert_eq!(logged_value(&seen[0], "home"), home.display().to_string(), "{seen:?}");
        assert_eq!(logged_value(&seen[0], "login"), "callers-login", "{seen:?}");
        assert_eq!(logged_value(&seen[0], "tmpdir"), "", "TMPDIR is not touched: {seen:?}");
        assert_eq!(private_listing(&login), ["login"], "a private home was made");
    }

    #[test]
    fn the_metaharness_adapter_makes_a_private_home_when_the_home_it_has_holds_no_login() {
        let (seen, login, home) = mh_with_home("mh-home-no-login", false, true);
        let used = logged_value(&seen[0], "home");
        assert_ne!(used, home.display().to_string(), "{seen:?}");
        assert!(Path::new(&used).starts_with(login.parent().unwrap()), "{seen:?}");
        assert_eq!(logged_value(&seen[0], "login"), DUMMY_LOGIN, "{seen:?}");
        assert_eq!(private_listing(&login), ["login"]);
    }

    #[test]
    fn the_metaharness_adapter_does_nothing_new_when_the_login_file_is_not_in_a_private_directory() {
        let (seen, _login, home) = mh_with_home("mh-home-plain", false, false);
        assert_eq!(logged_value(&seen[0], "home"), home.display().to_string(), "{seen:?}");
        assert!(seen[0].contains("refused"), "metaharness found a login it was not given: {seen:?}");
    }

    /// Every `.credentials.json` below `dir`, by path.
    fn credential_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let meta = fs::symlink_metadata(&p).unwrap();
            if meta.is_dir() {
                credential_files(&p, out);
            } else if p.file_name().is_some_and(|n| n == ".credentials.json") {
                out.push(p);
            }
        }
    }

    /// The real `metaharness` and the adapter are both killed with SIGKILL while the stand-in `claude`
    /// runs: the copy of the login that metaharness made in its own scratch root is left, and it
    /// lies in the private directory (the adapter set `TMPDIR` to its private home), not in the run
    /// directory and not in the system's temporary directory.
    #[test]
    fn a_sigkill_of_the_real_metaharness_leaves_its_scratch_copy_of_the_login_in_the_private_directory_only_skipped_without_metaharness_on_path() {
        if !on_path("metaharness") {
            eprintln!("skipped: no metaharness on PATH (the forge has none)");
            return;
        }
        let r = roots("mh-real-kill");
        let st = Standin::new("mh-real-kill", &[("claude", CLAUDE_STANDIN)]);
        let login = private_login(&r);
        fs::write(st.log.join("hang"), "").unwrap();
        let request = adapter_request(&r);
        let mut c = Command::new(tools().join("llm-adapter-metaharness.sh"));
        c.arg(&request).env_clear().env("PATH", st.path()).env("LOOM_CASE", "IDEA-1.intake").env("SDC_CLAUDE_CREDENTIALS", &login);
        let mut child = start(&r.1, c, "");
        let claude = wait_for_pid(&st, "claude-pid");
        let metaharness = wait_for_pid(&st, "claude-ppid");
        assert_eq!(fs::read_to_string(format!("/proc/{metaharness}/comm")).unwrap().trim(), "metaharness", "the parent of claude is not metaharness");
        kill("-KILL", metaharness);
        kill("-KILL", child.id());
        child.wait().unwrap();
        let mut copies = Vec::new();
        credential_files(login.parent().unwrap(), &mut copies);
        let (found, in_private) = (hits(&r.1, &[login.display().to_string(), DUMMY_LOGIN.to_owned()]), private_listing(&login));
        kill("-KILL", claude);
        assert!(found.is_empty(), "after a SIGKILL the login is in {found:?}");
        assert_eq!(copies.len(), 2, "the adapter's copy and metaharness's scratch copy lie in the private directory: {copies:?}");
        assert!(in_private.iter().any(|n| n.starts_with("h.")), "{in_private:?}");
        let tmpdir = logged_value(&st.logged("claude.")[0], "tmpdir");
        assert!(Path::new(&tmpdir).starts_with(login.parent().unwrap()), "the scratch root lies outside the private directory: {tmpdir}");
    }

    /// The environment `runbook loom-run`'s model decider gives `decider.sh` (`runbook/src/loom_run/decider.rs`,
    /// `env`): `PATH`, an empty `HOME` inside the run directory, and the `SDC_*` it sets.
    fn decider_under_loom_run(name: &str, adapter: Option<&str>, st: &Standin) -> (Run, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let r = roots(name);
        let login = private_login(&r);
        let reply = st.base.join("sdc-reply");
        fs::write(&reply, reply_standin()).unwrap();
        fs::set_permissions(&reply, fs::Permissions::from_mode(0o755)).unwrap();
        let home = r.1.join("home");
        fs::create_dir_all(&home).unwrap();
        let rec = r.1.join("rec");
        let request = serde_json::json!({"type": "approval", "case_dir": r.1.join("case"), "action": "scope.approve", "capability": "ApproveAction",
            "subject": "scope", "path": null, "revision": "r1", "position": {"claims": {}, "open_obligations": [], "legitimate_outcomes": []},
            "record_dir": rec});
        fs::create_dir_all(r.1.join("case")).unwrap();
        fs::write(r.1.join("request.json"), request.to_string()).unwrap();
        let mut c = Command::new(tools().join("decider.sh"));
        c.args(["model"]).arg(r.1.join("request.json")).env_clear().env("PATH", st.path()).env("HOME", &home)
            .env("SDC_DECIDER_ESCALATE", "stop").env("SDC_LLM_MODEL_DECIDER", "claude-haiku-5-5").env("SDC_DECIDER_MAX_BUDGET_USD", "0.25")
            .env("SDC_REPLY", &reply).env("SDC_DECIDER_MAX_TURNS", "8").env("SDC_CLAUDE_CREDENTIALS", &login);
        if let Some(a) = adapter {
            c.env("SDC_DECIDER_ADAPTER", a);
        }
        (run(&r.1, c, ""), rec)
    }

    #[test]
    fn the_model_decider_on_the_claude_cli_adapter_finds_its_login_under_loom_run() {
        let st = Standin::new("decider-claude", &[("claude", CLAUDE_STANDIN)]);
        let (l, rec) = decider_under_loom_run("decider-claude", Some("claude-cli"), &st);
        assert_eq!(l.code, Some(0), "{l:#?}");
        let reply: serde_json::Value = serde_json::from_str(l.stdout.trim()).unwrap();
        assert_eq!((reply["valid"].as_bool(), reply["decision"].as_str(), reply["adapter"].as_str()), (Some(true), Some("approve"), Some("claude-cli")), "{l:#?}");
        let claude = st.logged("claude.");
        assert_eq!((claude.len(), logged_value(&claude[0], "login")), (1, DUMMY_LOGIN.to_owned()), "{claude:?}");
        assert_no_private_copy(&rec);
    }

    #[test]
    fn the_model_decider_on_the_default_adapter_finds_its_login_under_loom_run() {
        let st = Standin::new("decider-default", &[("metaharness", METAHARNESS_STANDIN)]);
        fs::write(st.base.join("write-decision"), r#"{"decision":"approve","reasons":["ok"],"cites":[]}"#).unwrap();
        let (l, rec) = decider_under_loom_run("decider-default", None, &st);
        assert_eq!(l.code, Some(0), "{l:#?}");
        let reply: serde_json::Value = serde_json::from_str(l.stdout.trim()).unwrap();
        assert_eq!((reply["valid"].as_bool(), reply["decision"].as_str(), reply["adapter"].as_str()), (Some(true), Some("approve"), Some("metaharness")), "{l:#?}");
        let seen = st.logged("seen.");
        assert_eq!(logged_value(&seen[0], "login"), DUMMY_LOGIN, "{seen:?}");
        assert_no_private_copy(&rec);
    }

    /// `change.review` under Loom, the writers on `--adapter metaharness`: the review class takes
    /// the claude-cli adapter (`sdc-reply adapters`) and reads the login the binding passes.
    #[test]
    fn the_review_step_finds_its_login_under_loom_when_the_writers_run_on_metaharness() {
        use std::os::unix::fs::PermissionsExt;
        let r = roots("review-login");
        let st = Standin::new("review-login", &[("claude", CLAUDE_STANDIN), ("metaharness", METAHARNESS_STANDIN), ("sdc-reply", &reply_standin())]);
        fs::set_permissions(st.bin.join("sdc-reply"), fs::Permissions::from_mode(0o755)).unwrap();
        let login = private_login(&r);
        let instructions = r#"{"task":"instructions/change.review.md","inputs":"instructions/change.review.inputs.md","coverage_inputs":"instructions/change.review.coverage.inputs.md","coverage_note":"instructions/change.review.coverage-note.md","facts":"instructions/change.review.facts.md","facts_inputs":"instructions/change.review.facts.inputs.md"}"#;
        let mut c = Command::new(tools().join("llm-node.sh"));
        c.args(["change.review", "--model", "claude-haiku-5-5", "--max-turns", "30", "--max-budget-usd", "1", "--operations", "dir.list,file.read,search",
            "--instructions", instructions, "--adapter", "metaharness", "--claude-credentials"]).arg(&login).args(["--stub", "review-change"])
            .env_clear().env("PATH", st.path()).env("LOOM_CASE", "IDEA-1.todo.list");
        let l = run(&r.1, c, "{}");
        assert_eq!(l.code, Some(0), "{l:#?}");
        let response = &l.files[".sdc/llm/001-change.review/response.json"];
        assert!(response.contains("\"adapter\": \"claude-cli\""), "{response}");
        let claude = st.logged("claude.");
        assert_eq!((claude.len(), logged_value(&claude[0], "login")), (1, DUMMY_LOGIN.to_owned()), "{claude:?}");
        assert!(st.logged("seen.").is_empty(), "the review step started metaharness");
        assert_no_private_copy(&rec_of(&r, "change.review"));
        let found = hits(&r.1, &[login.display().to_string(), DUMMY_LOGIN.to_owned()]);
        assert!(found.is_empty(), "the login is in {found:?}");
    }
}
