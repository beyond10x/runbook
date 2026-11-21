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
        let mut c = Command::new(tools().join(tool));
        c.args(argv).env_clear().env("PATH", path()).env("LOOM_CASE", case);
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
}
