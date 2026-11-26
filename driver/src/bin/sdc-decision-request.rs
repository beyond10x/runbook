//! A scope-changing interview question as a decision request (`docs/decisions.md`, "A question as a
//! decision request"). Deterministic: no clock, no network, no LLM; the same case gives the same bytes.
//!
//! ```text
//! sdc-decision-request <case dir> <question id> [--level A0..A5] [--holder <holder>]
//!                      [--eligible <principal>[,<principal>…]] [--opened-at <timestamp>]
//! ```
//!
//! Reads `<case dir>/case.json`, `work/idea.yaml`, `work/questions.yaml` and `work/dissection.yaml`
//! and prints one JSON object: `request` (the `sdc.decision` `DecisionRequest` fields), `about`
//! (the dissection item and words that raised the question), `context` ("<run>, <idea>, <stage>")
//! and `what_happens_next`. `about`, `context` and `what_happens_next` are not contract fields
//! (`.engineering/planning/epic/decisions-a-person-can-take-cold.md`, "Ambiguities"). The keys
//! print in sorted order, so `what_happens_next` is last.
//!
//! The question must be scope-changing and carry `excerpt`, `default_reason` and a `consequences`
//! entry (`does`, `cost`) for every option (`story:interview-question-context`). The recommendation
//! is the question's default, cited to its dissection item as `path:line` and checked by
//! `tools/decision-grounds.sh`; with no such item, or a refused check, the request carries no
//! recommendation and says why.
//!
//! Exit: 0 printed, 2 the question or case cannot be rendered (what is missing goes to stderr),
//! 3 the grounding tool could not run, 64 usage.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde_json::{json, Map, Value};

const ACTION: &str = "scope.answer";
const LEVELS: [&str; 6] = ["A0", "A1", "A2", "A3", "A4", "A5"];
const HOLDERS: [&str; 8] = [
    "person",
    "advised",
    "decider",
    "decider_final",
    "rule",
    "rule_final",
    "rule_and_decider",
    "rule_and_decider_final",
];
const IDEA_CHARS: usize = 160;

type R<T> = Result<T, Refusal>;

enum Refusal {
    /// The question or the case cannot be rendered (exit 2).
    Input(String),
    /// The grounding tool could not run (exit 3).
    Tool(String),
}

fn input<T>(msg: impl Into<String>) -> R<T> {
    Err(Refusal::Input(msg.into()))
}

struct Args {
    case_dir: PathBuf,
    question: String,
    level: String,
    holder: String,
    eligible: Vec<String>,
    opened_at: String,
}

fn parse_args() -> Result<Args, String> {
    let mut positional = Vec::new();
    let mut level = "A0".to_string();
    let mut holder = "person".to_string();
    let mut eligible = vec!["operator".to_string()];
    let mut opened_at = "1970-01-01T00:00:00Z".to_string();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut value = |flag: &str| it.next().ok_or(format!("{flag} needs a value"));
        match a.as_str() {
            "--level" => level = value("--level")?,
            "--holder" => holder = value("--holder")?,
            "--eligible" => {
                eligible = value("--eligible")?
                    .split(',')
                    .map(str::to_string)
                    .collect()
            }
            "--opened-at" => opened_at = value("--opened-at")?,
            f if f.starts_with("--") => return Err(format!("unknown option {f}")),
            _ => positional.push(a),
        }
    }
    let [case_dir, question] = <[String; 2]>::try_from(positional)
        .map_err(|_| "expected <case dir> <question id>".to_string())?;
    if !LEVELS.contains(&level.as_str()) {
        return Err(format!("--level is one of {}", LEVELS.join(" ")));
    }
    if !HOLDERS.contains(&holder.as_str()) {
        return Err(format!("--holder is one of {}", HOLDERS.join(" ")));
    }
    if eligible.iter().any(String::is_empty) {
        return Err("--eligible names at least one principal".to_string());
    }
    Ok(Args {
        case_dir: PathBuf::from(case_dir),
        question,
        level,
        holder,
        eligible,
        opened_at,
    })
}

fn read(path: &Path) -> R<String> {
    fs::read_to_string(path).or_else(|e| input(format!("cannot read {}: {e}", path.display())))
}

fn yaml(path: &Path) -> R<serde_yaml::Value> {
    serde_yaml::from_str(&read(path)?)
        .or_else(|e| input(format!("{} does not parse: {e}", path.display())))
}

fn text(v: &serde_yaml::Value) -> Option<&str> {
    v.as_str().map(str::trim).filter(|s| !s.is_empty())
}

/// The run's name: the directory above the `store` the case lives in.
fn run_name(case_dir: &Path) -> R<String> {
    let abs = fs::canonicalize(case_dir)
        .or_else(|e| input(format!("cannot open {}: {e}", case_dir.display())))?;
    let parts: Vec<_> = abs.components().collect();
    let store = parts
        .iter()
        .rposition(|c| c.as_os_str() == "store")
        .filter(|i| *i > 0);
    match store {
        Some(i) => Ok(parts[i - 1].as_os_str().to_string_lossy().into_owned()),
        None => input(format!(
            "{} is not inside <run>/store: the context line names the run",
            case_dir.display()
        )),
    }
}

fn idea_line(raw: &str) -> String {
    let flat = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= IDEA_CHARS {
        flat
    } else {
        let cut: String = flat.chars().take(IDEA_CHARS).collect();
        format!("{}...", cut.trim_end())
    }
}

/// The 1-based lines of `- id: <id>` in a list file and the line before the next item.
fn item_lines(file: &str, id: &str) -> Option<(usize, usize)> {
    let lines: Vec<&str> = file.lines().collect();
    let is_item = |l: &str| l.trim_start().starts_with("- id:");
    let start = lines
        .iter()
        .position(|l| is_item(l) && l.trim_start()["- id:".len()..].trim() == id)?;
    let mut end = lines[start + 1..]
        .iter()
        .position(|l| is_item(l))
        .map_or(lines.len(), |p| start + 1 + p);
    while end > start + 1 && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    Some((start + 1, end))
}

struct Question {
    text: String,
    options: Vec<String>,
    default: String,
    default_reason: String,
    excerpt: String,
    item: String,
    does_cost: Vec<(String, String)>,
}

fn question(doc: &serde_yaml::Value, id: &str) -> R<Question> {
    let list = doc["questions"].as_sequence();
    let Some(q) = list.and_then(|l| l.iter().find(|q| text(&q["id"]) == Some(id))) else {
        return input(format!("no question {id} in work/questions.yaml"));
    };
    if q["scope_changing"].as_bool() != Some(true) {
        return input(format!(
            "{id} is not scope-changing: it takes its default without a decision request"
        ));
    }
    let mut missing: Vec<String> = Vec::new();
    let mut get = |key: &str| match text(&q[key]) {
        Some(s) => s.to_string(),
        None => {
            missing.push(key.to_string());
            String::new()
        }
    };
    let (qtext, default, default_reason, excerpt, item) = (
        get("question"),
        get("default"),
        get("default_reason"),
        get("excerpt"),
        get("item"),
    );
    let options: Vec<String> = q["options"]
        .as_sequence()
        .map(|o| {
            o.iter()
                .filter_map(|v| text(v).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if options.len() < 2 {
        missing.push("options (at least two)".to_string());
    }
    if options.len() > 26 {
        missing.push(format!(
            "options (at most 26 options: a letter names one option and the letters are A to Z, got {})",
            options.len()
        ));
    }
    if !options.is_empty() && !options.contains(&default) {
        missing.push("default (one of the options)".to_string());
    }
    let entries = q["consequences"].as_sequence();
    let mut does_cost = Vec::new();
    if entries.is_none_or(|e| e.len() != options.len()) {
        missing.push("consequences (one entry per option)".to_string());
    } else {
        for (i, e) in entries.into_iter().flatten().enumerate() {
            match (text(&e["does"]), text(&e["cost"])) {
                (Some(d), Some(c)) => does_cost.push((d.to_string(), c.to_string())),
                _ => missing.push(format!("consequences[{i}] does and cost")),
            }
        }
    }
    if !missing.is_empty() {
        return input(format!("{id} lacks {}", missing.join(", ")));
    }
    Ok(Question {
        text: qtext,
        options,
        default,
        default_reason,
        excerpt,
        item,
        does_cost,
    })
}

fn letter(i: usize) -> String {
    ((b'A' + i as u8) as char).to_string()
}

fn grounds_tool() -> PathBuf {
    std::env::var_os("SDC_DECISION_GROUNDS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../tools/decision-grounds.sh")
        })
}

/// Runs `tools/decision-grounds.sh` on the request and the proposed recommendation; returns its
/// `recommend` object.
fn check_grounds(request: &Value, proposal: &Value, case_dir: &Path) -> R<Value> {
    let tool = grounds_tool();
    let root = tool
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let file =
        std::env::temp_dir().join(format!("sdc-decision-request-{}.json", std::process::id()));
    let body = serde_json::to_vec(&json!({"request": request, "proposal": proposal})).unwrap();
    fs::write(&file, body)
        .map_err(|e| Refusal::Tool(format!("cannot write {}: {e}", file.display())))?;
    let out = Command::new("bash")
        .arg(&tool)
        .arg(&file)
        .arg("--case")
        .arg(case_dir)
        .arg("--root")
        .arg(&root)
        .output();
    let _ = fs::remove_file(&file);
    let out = out.map_err(|e| Refusal::Tool(format!("cannot run {}: {e}", tool.display())))?;
    let parsed: Option<Value> = serde_json::from_slice(&out.stdout).ok();
    match parsed.and_then(|v| v.get("recommend").cloned()) {
        Some(r) => Ok(r),
        None => Err(Refusal::Tool(format!(
            "{} gave no verdict (exit {:?}): {}",
            tool.display(),
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).trim()
        ))),
    }
}

fn build(args: &Args) -> R<Value> {
    let case_dir = &args.case_dir;
    let case: Value = serde_json::from_str(&read(&case_dir.join("case.json"))?)
        .or_else(|e| input(format!("case.json does not parse: {e}")))?;
    let case_id = case["id"].as_str().map(str::to_string);
    let revision = case["artifacts"]["questions"]["revision"]
        .as_str()
        .map(str::to_string);
    let (Some(case_id), Some(revision)) = (case_id, revision) else {
        return input("case.json has no id or no revision of the artifact questions");
    };
    let protocol = case["protocol"].as_str().unwrap_or("");
    let stage = protocol.strip_prefix("sdc.").unwrap_or(protocol);
    if stage.is_empty() {
        return input("case.json names no protocol: the context line names the stage");
    }
    let run = run_name(case_dir)?;
    let idea_doc = yaml(&case_dir.join("work/idea.yaml"))?;
    let Some(idea) = text(&idea_doc["text"]) else {
        return input("work/idea.yaml has no text");
    };
    let q = question(
        &yaml(&case_dir.join("work/questions.yaml"))?,
        &args.question,
    )?;

    let dissection_raw = read(&case_dir.join("work/dissection.yaml"))?;
    let dissection: serde_yaml::Value = serde_yaml::from_str(&dissection_raw)
        .or_else(|e| input(format!("work/dissection.yaml does not parse: {e}")))?;
    let item = dissection["items"]
        .as_sequence()
        .and_then(|l| l.iter().find(|i| text(&i["id"]) == Some(q.item.as_str())));
    let lines = item_lines(&dissection_raw, &q.item).filter(|_| item.is_some());

    let options: Vec<Value> = q
        .options
        .iter()
        .zip(&q.does_cost)
        .enumerate()
        .map(|(i, (answer, (does, cost)))| {
            json!({
                "letter": letter(i), "does": does, "cost": cost,
                "consequence": {"admits": [ACTION], "blocks": [], "answer": answer,
                                "produces": "artifact_revision"},
            })
        })
        .collect();
    let default_letter = letter(q.options.iter().position(|o| *o == q.default).unwrap());

    let mut request = json!({
        "request_id": format!("DR-{case_id}-{ACTION}-{}-{revision}", args.question),
        "state": "Open",
        "case_id": case_id,
        "question_key": format!("{case_id}|{ACTION}|{}", args.question),
        "unblocks": {"kind": "question", "action": ACTION, "capability": ACTION,
                     "question_id": args.question},
        "subject_artifact": "questions",
        "subject_revision": revision,
        "subject_path": "work/questions.yaml",
        "question": q.text,
        "options": options,
        "autonomy_level": args.level,
        "decision_class": "scope_changing_answer",
        "holder": args.holder,
        "eligible": args.eligible,
        "addressed_to": args.eligible[0],
        "opened_at": args.opened_at,
        "recommendation_strength": "none",
        "default_on_silence": false,
        "grounds": [],
        "decision_reasons": [],
    });
    let obj = request.as_object_mut().unwrap();

    let reason = match lines {
        None => format!(
            "the default has no ground to cite: item {} is not an item of work/dissection.yaml",
            q.item
        ),
        Some((line, line_end)) => {
            let ground = json!({
                "source": "idea",
                "citation": {"form": "path_line", "value": {
                    "path": "work/dissection.yaml", "line": line, "line_end": line_end}},
                "supports": default_letter,
                "how": q.default_reason,
            });
            let proposal = json!({"recommended_option": default_letter, "strength": "weak",
                "default_on_silence": false, "proposed_by": "rule", "grounds": [ground]});
            let verdict = check_grounds(&Value::Object(obj.clone()), &proposal, case_dir)?;
            if verdict["check_verdict"] == "verified" {
                obj.insert("recommended_option".into(), json!(default_letter));
                obj.insert("recommendation_strength".into(), json!("weak"));
                obj.insert("recommended_by".into(), json!("rule"));
                obj.insert("grounds".into(), verdict["grounds"].clone());
                obj.insert("grounding_report".into(), verdict["check_report"].clone());
                String::new()
            } else {
                verdict["check_report"]
                    .as_str()
                    .unwrap_or("the grounding check refused")
                    .to_string()
            }
        }
    };
    if !reason.is_empty() {
        obj.insert("no_recommendation_reason".into(), json!(reason));
    }

    let about = match item {
        Some(i) => format!(
            "Raised by dissection item {} ({}) on the words \"{}\" of the idea: {}",
            q.item,
            text(&i["type"]).unwrap_or("item"),
            q.excerpt,
            text(&i["note"]).unwrap_or("")
        ),
        None => format!(
            "Raised by {} on the words \"{}\" of the idea.",
            q.item, q.excerpt
        ),
    };
    let next = format!(
        "After your answer to {id} the run writes it to work/questions.yaml and takes {ACTION}, \
         which records who answered; questions.check then runs on that revision and \
         requirements.normalise follows once every scope-changing question is answered.",
        id = args.question
    );
    let mut out = Map::new();
    out.insert("about".into(), json!(about));
    out.insert(
        "context".into(),
        json!(format!("{run}, {}, {stage}", idea_line(idea))),
    );
    out.insert("request".into(), request);
    out.insert("what_happens_next".into(), json!(next));
    Ok(Value::Object(out))
}

fn main() -> ExitCode {
    sdc_tools::answer_version("sdc-decision-request");
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("sdc-decision-request: {e}");
            eprintln!("usage: sdc-decision-request <case dir> <question id> [--level A0..A5] [--holder H] [--eligible P,…] [--opened-at T]");
            return ExitCode::from(64);
        }
    };
    match build(&args) {
        Ok(v) => {
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
            ExitCode::SUCCESS
        }
        Err(Refusal::Input(m)) => {
            eprintln!("sdc-decision-request: {m}");
            ExitCode::from(2)
        }
        Err(Refusal::Tool(m)) => {
            eprintln!("sdc-decision-request: {m}");
            ExitCode::from(3)
        }
    }
}
