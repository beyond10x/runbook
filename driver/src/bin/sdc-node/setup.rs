//! The start of a node under Loom: the arguments on standard
//! input, the options after the action name, the `.stub` directory of the workspace, the retry
//! feedback document.

use crate::ordjson::{NumLit, Ov};
use crate::util::{die, env_set, sha16, which, Exit, R};
use crate::yaml;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// The binding's `llm` object as the script builds it (SDC_LLM_NODE), in jq's key order.
#[derive(Serialize, Default)]
pub struct Cfg {
    /// The model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The turn cap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_turns: Option<NumLit>,
    /// The money budget of one call; none named means the request carries none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_budget_usd: Option<NumLit>,
    /// The operations the model may use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operations: Option<Vec<String>>,
    /// The instruction files by part.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<Ov>,
    /// The argv of the stub.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stub: Option<Vec<String>>,
}

/// What the start leaves for the rest of the run.
pub struct Setup {
    /// The directory of the tools (adapters, `sdc-tool.sh`, `node-classes.json`).
    pub here: PathBuf,
    /// Two levels above it: the repository, where the tools sit in `bindings/tools`.
    pub root: PathBuf,
    /// The arguments Loom gave on standard input.
    pub args: Ov,
    /// The `llm` object.
    pub cfg: Cfg,
    /// The case id.
    pub case: String,
    /// The feedback records file, when the run has feedback.
    pub feedback: Option<PathBuf>,
    /// The outermost directory the start made in the workspace (the copy of a retry's feedback
    /// document); a not-called exit removes it, since that exit writes nothing.
    pub made: Option<PathBuf>,
    /// The reasoning effort the caller names (`--effort`); the class default applies without it.
    pub effort: Option<String>,
}

const VALUED: [&str; 12] = [
    "--model", "--max-turns", "--max-budget-usd", "--operations", "--instructions", "--adapter", "--principal",
    "--stub-fixtures", "--stub-counter", "--claude-credentials", "--effort", "--replay-plan",
];

/// Reads standard input and `rest` (the arguments after the action name) as the Loom branch of the
/// script does, and exports what it exports.
pub fn setup(node: &str, rest: &[String]) -> R<Setup> {
    let Some(case) = env_set("LOOM_CASE") else {
        return Err(die(format!("llm-node: {node} could not run: LOOM_CASE is not set"), 64));
    };
    let mut raw = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut raw);
    let text = String::from_utf8_lossy(&raw).into_owned();
    let text = if text.trim().is_empty() { "{}".to_string() } else { text };
    let args = match Ov::parse(&text) {
        Ok(a @ Ov::Obj(_)) => a,
        _ => return Err(die(format!("llm-node: {node} could not run: its standard input is not one JSON object"), 64)),
    };

    let mut opt: BTreeMap<&str, String> = BTreeMap::new();
    let mut tools_dir: Option<String> = None;
    let mut stub: Option<Vec<String>> = None;
    let mut i = 0;
    while i < rest.len() {
        let a = rest[i].as_str();
        if a == "--stub" {
            stub = Some(rest[i + 1..].to_vec());
            break;
        } else if VALUED.contains(&a) || a == "--tools-dir" {
            if i + 1 >= rest.len() {
                return Err(die(format!("llm-node: {a} takes a value"), 64));
            }
            if a == "--tools-dir" {
                tools_dir = Some(rest[i + 1].clone());
            } else {
                opt.insert(VALUED.iter().find(|v| **v == a).copied().unwrap_or(""), rest[i + 1].clone());
            }
            i += 2;
        } else {
            return Err(die(format!("llm-node: unknown argument {a}"), 64));
        }
    }
    let given = |k: &str| opt.get(k).filter(|v| !v.is_empty()).cloned();

    let cfg = build_cfg(node, &opt, stub)?;
    let here = tools_directory(node, tools_dir)?;
    check_effort(node, &here, given("--effort").as_deref())?;
    let root = here.parent().and_then(Path::parent).map(Path::to_path_buf).unwrap_or_else(|| here.clone());

    let sibling = |name: &str| which(name).unwrap_or_else(|| root.join("driver/target/debug").join(name));
    std::env::set_var("SDC_LLM_NODE", serde_json::to_string(&cfg).unwrap_or_default());
    std::env::set_var("SDC_REPLY", sibling("sdc-reply"));
    std::env::set_var("SDC_CASE", &case);
    std::env::set_var("SDC_ACTION", node);
    for (flag, var) in [
        ("--adapter", "SDC_LLM_ADAPTER"),
        ("--principal", "SDC_PRINCIPAL"),
        ("--replay-plan", "SDC_REPLAY"),
        ("--stub-fixtures", "SDC_STUB_FIXTURES"),
        ("--stub-counter", "SDC_STUB_COUNTER"),
    ] {
        if let Some(v) = given(flag) {
            std::env::set_var(var, v);
        }
    }
    let work = std::env::current_dir().unwrap_or_default();
    if work.join(".stub").is_dir() {
        stub_directory(node, &here, &work)?;
    }
    if let Some(v) = given("--claude-credentials") {
        std::env::set_var("SDC_CLAUDE_CREDENTIALS", v);
    }

    let mut feedback = env_set("SDC_FEEDBACK").map(PathBuf::from);
    let mut made = None;
    if let Some(doc) = env_set("LOOM_RETRY_FEEDBACK") {
        let (copy, dir) = retry_feedback(node, &work, Path::new(&doc))?;
        feedback = copy.or(feedback);
        made = dir;
    }
    Ok(Setup { here, root, args, cfg, case, feedback, made, effort: given("--effort") })
}

/// `--effort` is one of the levels `node-classes.json` lists (`efforts`): both model CLIs ignore an
/// unknown level with a warning and run at their own default, so the program refuses it.
fn check_effort(node: &str, here: &Path, effort: Option<&str>) -> R<()> {
    let Some(effort) = effort else { return Ok(()) };
    let classes = fs::read_to_string(here.join("node-classes.json")).ok().and_then(|t| Ov::parse(&t).ok());
    let levels: Vec<String> = classes.as_ref().and_then(|c| c.get("efforts")).map(|l| l.items().iter().map(|v| v.raw()).collect()).unwrap_or_default();
    if levels.iter().any(|l| l == effort) {
        return Ok(());
    }
    Err(die(format!("llm-node: {node} could not run: --effort takes one of {}, got {effort}", levels.join(", ")), 64))
}

/// The tools directory: the option, else the `bindings/tools` of the repository this binary was
/// built in.
fn tools_directory(node: &str, given: Option<String>) -> R<PathBuf> {
    if let Some(g) = given {
        let p = PathBuf::from(g);
        return Ok(if p.is_absolute() { p } else { std::env::current_dir().unwrap_or_default().join(p) });
    }
    let exe = std::env::current_exe().ok();
    exe.iter()
        .flat_map(|e| e.ancestors().skip(1))
        .map(|d| d.join("bindings/tools"))
        .find(|d| d.join("node-classes.json").is_file())
        .ok_or_else(|| die(format!("llm-node: {node} could not run: no --tools-dir and no bindings/tools above this program"), 64))
}

/// SDC_LLM_NODE: the jq expression of the script, with its refusals.
fn build_cfg(node: &str, opt: &BTreeMap<&str, String>, stub: Option<Vec<String>>) -> R<Cfg> {
    let given = |k: &str| opt.get(k).filter(|v| !v.is_empty());
    let mut bad = String::new();
    let mut numbers = Vec::new();
    for k in ["--max-turns", "--max-budget-usd"] {
        // A bound that is named with an empty value is a non-number, never "none": an unset option is the only none.
        // A budget is a finite number of at least 0; the turn bound is a whole number of at least 1.
        let (range, ok): (&str, fn(&str) -> bool) = if k == "--max-budget-usd" {
            ("a number of at least 0", |n| sdc_tools::named::money("", n).is_ok())
        } else {
            ("a whole number of at least 1", |n| n.parse::<f64>().is_ok_and(|t| t.is_finite() && t.fract() == 0.0 && t >= 1.0))
        };
        match opt.get(k).map(|v| (v, NumLit::parse(v))) {
            None => numbers.push(None),
            Some((_, Some(n))) if ok(&n.0) => numbers.push(Some(n)),
            Some((v, parsed)) => {
                if bad.is_empty() {
                    let shown = sdc_tools::named::cut(v);
                    bad = if parsed.is_none() { format!("{k} takes a number, got {shown}") } else { format!("{k} takes {range}, got {shown}") };
                }
                numbers.push(None);
            }
        }
    }
    let mut instructions = None;
    if let Some(v) = given("--instructions") {
        match Ov::parse(v) {
            Ok(o @ Ov::Obj(_)) => instructions = Some(o),
            _ => bad.push_str(&format!("--instructions takes a JSON object, got {v}")),
        }
    }
    if !bad.is_empty() {
        return Err(die(format!("llm-node: {node} could not run: {bad}"), 64));
    }
    let mut n = numbers.into_iter();
    let (max_turns, max_budget_usd) = (n.next().flatten(), n.next().flatten());
    Ok(Cfg {
        model: given("--model").cloned(),
        max_turns,
        max_budget_usd,
        operations: given("--operations").map(|o| o.split(',').map(str::to_string).collect()),
        instructions,
        stub,
    })
}

/// `export NAME=value` unless the variable already has a value.
fn stub_set(name: &str, value: String) {
    if env_set(name).is_none() {
        std::env::set_var(name, value);
    }
}

fn flat(v: &serde_yaml::Value, out: &mut Vec<String>) {
    match v {
        serde_yaml::Value::Sequence(s) => s.iter().for_each(|i| flat(i, out)),
        other => out.push(yaml::text(Some(other))),
    }
}

/// The workspace directory `.stub` of a suite cycle: the fixture directory, the findings the stub
/// raises, the scripted model's settings and replies.
fn stub_directory(node: &str, here: &Path, work: &Path) -> R<()> {
    let stub = work.join(".stub");
    stub_set("SDC_STUB_FIXTURES", stub.display().to_string());
    stub_set("SDC_STUB_FINDINGS", format!("{}:{}", stub.join("finding.yaml").display(), stub.join("finding-2.yaml").display()));
    let fixture_adapter = |v: &str| if v == "fixture" { here.join("llm-adapter-fixture.sh").display().to_string() } else { v.to_string() };
    let settings = stub.join("settings.yaml");
    if settings.is_file() {
        let Some(doc) = yaml::load(&settings) else {
            return Err(die(format!("llm-node: {node} could not run: .stub/settings.yaml is not YAML"), 64));
        };
        if let Some(m) = doc.as_mapping() {
            for (k, v) in m {
                let mut parts = Vec::new();
                flat(v, &mut parts);
                let v = parts.join(" ");
                match yaml::text(Some(k)).as_str() {
                    "adapter" => stub_set("SDC_LLM_ADAPTER", fixture_adapter(&v)),
                    "review_adapter" => stub_set("SDC_REVIEW_ADAPTER", fixture_adapter(&v)),
                    "nodes" => stub_set("SDC_LLM_NODES", v),
                    "review_mode" => stub_set("SDC_REVIEW_MODE", v),
                    "implement_mode" => stub_set("SDC_IMPLEMENT_MODE", v),
                    "implement_coverage" => stub_set("SDC_IMPLEMENT_COVERAGE", v),
                    other => return Err(die(format!("llm-node: {node} could not run: .stub/settings.yaml has no setting {other}"), 64)),
                }
            }
        }
    }
    let mut replies: Vec<PathBuf> = fs::read_dir(stub.join("replies"))
        .map(|d| d.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "yaml") && p.is_file()).collect())
        .unwrap_or_default();
    replies.sort();
    for f in replies {
        let name = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let Some(doc) = yaml::load(&f) else {
            return Err(die(format!("llm-node: {node} could not run: .stub/replies/{name}.yaml is not YAML"), 64));
        };
        let json = serde_json::to_string(&doc).unwrap_or_default();
        match name.as_str() {
            "review" => stub_set("SDC_REVIEW_REPLIES", json),
            "implement" => stub_set("SDC_IMPLEMENT_REPLIES", json),
            "coverage" => stub_set("SDC_COVERAGE_REPLIES", json),
            "fixture-stub" => stub_set("SDC_FIXTURE_STUB", json),
            _ => return Err(die(format!("llm-node: {node} could not run: .stub/replies/{name}.yaml is no reply list"), 64)),
        }
    }
    Ok(())
}

fn is_blob(v: &Ov) -> Option<&str> {
    match v {
        Ov::Str(s) if s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) => Some(s),
        _ => None,
    }
}

/// The retry records of a `loom.retry-feedback/1` document as the driver's feedback file: kind,
/// result, subject and the log, here the tail Loom keeps of the cited blob, written beside the
/// document's copy under `.sdc/loom-feedback/`. The second value is the outermost directory this
/// call made (`Setup::made`), or none when they all existed.
fn retry_feedback(node: &str, work: &Path, doc: &Path) -> R<(Option<PathBuf>, Option<PathBuf>)> {
    let parsed = fs::read_to_string(doc).ok().and_then(|t| Ov::parse(&t).ok());
    let Some(parsed) = parsed.filter(|d| d.str_of("format") == Some("loom.retry-feedback/1")) else {
        return Err(die(format!("llm-node: {node} could not run: LOOM_RETRY_FEEDBACK names no loom.retry-feedback/1 document"), 126));
    };
    let dir = work.join(".sdc/loom-feedback").join(sha16(doc));
    // "Was there before" is asked of the path itself, not of what a link points at: a dangling link is there.
    let made = [&dir, &work.join(".sdc/loom-feedback"), &work.join(".sdc")].into_iter().rfind(|d| d.symlink_metadata().is_err()).cloned();
    let _ = fs::create_dir_all(&dir);
    let logs: Vec<&Ov> = parsed.get("logs").map(Ov::items).unwrap_or_default();
    let blobs: Vec<&str> = logs.iter().filter_map(|l| l.get("blob_sha256").and_then(is_blob)).collect();
    for b in &blobs {
        let tail = logs.iter().find(|l| l.get("blob_sha256").and_then(is_blob) == Some(b)).and_then(|l| l.get("tail"));
        let _ = fs::write(dir.join(b), tail.map(Ov::raw).unwrap_or_else(|| "null".to_string()));
    }
    let target = dir.join("feedback.json");
    let Some(records) = parsed.get("records") else {
        let _ = fs::write(&target, "");
        return Ok((None, made));
    };
    let one = |r: &Ov| {
        let get = |k: &str| r.get(k).cloned().unwrap_or(Ov::Null);
        let alt = |k: &str| r.field(k).cloned().unwrap_or(Ov::Null);
        let log = match r.get("blob_sha256") {
            Some(Ov::Str(b)) if blobs.contains(&b.as_str()) => Ov::Str(format!("{}/{b}", dir.display())),
            _ => Ov::Null,
        };
        Ov::Obj(vec![
            ("kind".into(), get("kind")),
            ("result".into(), get("result")),
            ("subject".into(), get("subject")),
            ("action".into(), alt("action")),
            ("exit".into(), alt("exit")),
            ("log".into(), log),
        ])
    };
    let out = Ov::Arr(records.items().into_iter().map(one).collect());
    let mut text = serde_json::to_string_pretty(&out).map_err(|_| Exit(126))?;
    text.push('\n');
    let _ = fs::write(&target, text);
    Ok((Some(target), made))
}
