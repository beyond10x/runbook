//! sdc-unstick: the legal moves of a stopped autopilot run, the input and reply schema of the
//! `run.unstick` node that chooses one, and the check of its reply (D147, docs/driver.md
//! "Unstick"). The model never invents state: a program lists the moves, the model picks one id,
//! and scripts/run-trial.sh carries it out through existing `sdc-drive` commands.
//!
//!   sdc-unstick moves    --stop <stop.json>
//!   sdc-unstick prompt   --stop <stop.json> --moves <moves.json> --decision <canon decision.json>
//!                        --record <dir> [--attempt N --invalid "<why>"]
//!   sdc-unstick validate --moves <moves.json> --reply <out/unstick.json>
//!
//! stop.json (written by run-trial.sh): `{cause, reason, case, case_dir, action, kind, request,
//! rejection_recorded, position}`: kind is approval | question | finding | close (a decider
//! request in `request`), writer (writer_spent, writer_not_called), handoff or stuck.
//!
//! Moves (each `{id, kind, target, says}` plus what carrying it out needs):
//! - `rerun:<W>` one more allowance of writer W's `max_runs` (`sdc-drive allow`), its failing
//!   check records fed back as on any retry: each spent or not-called writer of the position;
//! - `reask:<D>` the decider asked D again with the diagnosis as a note: an approval, question or
//!   finding request;
//! - `default:<F>` the default of finding or question F, taken as `agent:unstick`;
//! - `approve:<A>` approval A (or the close) taken as `agent:unstick`, where it is still open
//!   (no rejection recorded for it);
//! - `end:<O>` the case ends through outcome O, which requires one decision alone (withheld,
//!   dropped, abandoned): the approval's capability denied (when it was never decided) or the
//!   decider's rejection recorded, the decision recorded directly after it, the case closed
//!   (`sdc-drive approve --deny` or `approve --result rejected`, `decide --after-denial`, `close`);
//! - `stop` the run stops as it would have.
//!
//! The reply (`out/unstick.json`, schema in `<record>/reply-schema.json`): `{move, diagnosis,
//! defect_class, reasons}`; `validate` prints `{valid, why, move: <the move object>, diagnosis,
//! defect_class, reasons}` and exits 0 either way (1 on an unreadable input).
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use serde_json::{json, Value};

const CLASSES: [&str; 7] = [
    "missing_route",
    "cap_too_low",
    "writer_error",
    "decider_error",
    "contradiction_in_inputs",
    "tool_defect",
    "other",
];

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Moves {
        #[arg(long)]
        stop: PathBuf,
    },
    Prompt {
        #[arg(long)]
        stop: PathBuf,
        #[arg(long)]
        moves: PathBuf,
        #[arg(long)]
        decision: PathBuf,
        #[arg(long)]
        record: PathBuf,
        #[arg(long, default_value_t = 1)]
        attempt: u32,
        #[arg(long, default_value = "")]
        invalid: String,
    },
    Validate {
        #[arg(long)]
        moves: PathBuf,
        #[arg(long)]
        reply: PathBuf,
    },
}

type R<T> = Result<T, String>;

fn read(p: &Path) -> R<Value> {
    let t = fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    serde_json::from_str(&t).map_err(|e| format!("{}: {e}", p.display()))
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

/// The legal moves of the stopped state, `stop` last.
fn moves(stop: &Value, ir: &Value) -> Vec<Value> {
    let mut out = vec![];
    let req = &stop["request"];
    let kind = s(&stop["kind"]);
    let mut writers: Vec<String> = vec![];
    for k in ["spent", "not_called"] {
        for e in stop["position"][k].as_array().into_iter().flatten() {
            let a = s(&e["action"]).to_string();
            if !a.is_empty() && !writers.contains(&a) {
                writers.push(a);
            }
        }
    }
    if kind == "writer" && !s(&stop["action"]).is_empty() && !writers.contains(&s(&stop["action"]).to_string()) {
        writers.insert(0, s(&stop["action"]).to_string());
    }
    for w in &writers {
        out.push(json!({"id": format!("rerun:{w}"), "kind": "rerun_writer", "target": w,
            "says": format!("run the writer {w} once more (one more allowance of its max_runs); its failing check records are fed back as on any retry")}));
    }
    let item = match kind {
        "finding" => s(&req["finding"]["id"]).to_string(),
        "question" => s(&req["question"]["id"]).to_string(),
        _ => s(&req["action"]).to_string(),
    };
    // A recorded rejection settles the approval: asking again or approving it is no longer legal.
    let open = stop["rejection_recorded"] != json!(true);
    if (matches!(kind, "question" | "finding") || (matches!(kind, "approval" | "close") && open)) && !item.is_empty() {
        out.push(json!({"id": format!("reask:{item}"), "kind": "reask", "target": item,
            "says": format!("ask the decider again about {item}, with your diagnosis as a note")}));
    }
    if matches!(kind, "finding" | "question") {
        let d = s(&req[kind]["default"]);
        if !d.is_empty() {
            out.push(json!({"id": format!("default:{item}"), "kind": "default", "target": item, "answer": d,
                "says": format!("take the default of {item} as agent:unstick: {d}")}));
        }
    }
    if matches!(kind, "approval" | "close") && open {
        out.push(json!({"id": format!("approve:{item}"), "kind": "approve", "target": item,
            "capability": req["capability"],
            "says": format!("take {item} as agent:unstick (approve; for the close: close through {})", s(&req["outcome"]))}));
    }
    if kind == "approval" && req["capability"].is_string() {
        for (o, v) in ir["outcomes"].as_object().into_iter().flatten() {
            if let Some(d) = v["requires"].as_object().filter(|m| m.len() == 1).and_then(|m| m.get("decision")).and_then(Value::as_str) {
                out.push(json!({"id": format!("end:{o}"), "kind": "end", "target": o, "decision": d,
                    "capability": req["capability"],
                    "says": format!("end the case {} through `{o}`: the refusal of {} recorded, `{d}` recorded as agent:unstick, the case closed; its parents end through the outcome their evaluation then offers",
                        s(&stop["case"]), s(&req["capability"]))}));
            }
        }
    }
    out.push(json!({"id": "stop", "kind": "stop", "target": null, "says": "stop the run as it would have stopped"}));
    out
}

/// The node's input: the stop, the blocked actions with their unmet preconditions, the open
/// obligations, the last 20 events, the last error text, the moves.
fn input(stop: &Value, moves: &[Value], decision: &Value) -> Value {
    let dir = Path::new(s(&stop["case_dir"]));
    let blocked: Vec<Value> = decision["actions"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, a)| a["status"] != "admissible")
        .map(|(id, a)| json!({"action": id, "status": a["status"], "unmet": a["reasons"]}))
        .collect();
    let open: Vec<Value> = decision["obligations"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|o| o["status"] == "open")
        .cloned()
        .collect();
    let evs: Vec<Value> = fs::read_to_string(dir.join("events.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect();
    let last: Vec<Value> = evs
        .iter()
        .rev()
        .take(20)
        .rev()
        .map(|e| {
            let mut e = e.clone();
            if let Some(m) = e.as_object_mut() {
                for k in ["prev", "state", "state_before", "argv"] {
                    m.remove(k);
                }
            }
            e
        })
        .collect();
    let error = evs
        .iter()
        .rev()
        .find(|e| e["event"] == "action_finished" && e["exit"] != "0" && e["log"].is_string())
        .map(|e| {
            let text = fs::read(dir.join("blobs").join(s(&e["log"])))
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .unwrap_or_default();
            let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
            json!({"action": e["action"], "exit": e["exit"], "seq": e["seq"],
                "log_tail": lines[lines.len().saturating_sub(20)..].join("\n")})
        });
    json!({"cause": stop["cause"], "reason": stop["reason"], "case": stop["case"], "action": stop["action"],
        "kind": stop["kind"], "request": stop["request"], "claims": decision["claims"], "blocked": blocked,
        "open_obligations": open, "last_events": last, "last_error": error, "moves": moves})
}

fn schema(moves: &[Value]) -> Value {
    json!({"type": "object", "additionalProperties": false,
        "required": ["move", "diagnosis", "defect_class", "reasons"],
        "properties": {
            "move": {"type": "string", "enum": moves.iter().map(|m| m["id"].clone()).collect::<Vec<_>>()},
            "diagnosis": {"type": "string", "minLength": 20},
            "defect_class": {"type": "string", "enum": CLASSES},
            "reasons": {"type": "array", "minItems": 1, "items": {"type": "string", "minLength": 1}}}})
}

fn prompt(attempt: u32, invalid: &str) -> String {
    let mut p = String::from(
        "An autopilot run of a governed software delivery cycle has stopped. You choose what happens next.\n\
Work only in the current directory. Read in/stop.json; write only out/unstick.json. You have no shell.\n\
\n\
in/stop.json holds: the stop's cause and reason, the stopped case and action, the decision request\n\
that led to it (if any), the claims, the blocked actions with their unmet preconditions, the open\n\
obligations, the case's last 20 events, the last failing action's log, and `moves`: the only moves\n\
a program can carry out from this state, each with an id and what it does.\n\
\n\
Choose the one move that lets the run go on soundly, or `stop` when none does. You do not invent\n\
state: you pick an id from `moves`. No person is available in this run.\n\
Then say what is missing, for the people who fix the system: which route, check or input is\n\
missing or wrong, in two or three sentences (`diagnosis`), and its class (`defect_class`):\n\
missing_route (the protocol or the tools have no path from this state), cap_too_low,\n\
writer_error (a writer produced wrong output), decider_error (a decision was wrong),\n\
contradiction_in_inputs (the inputs contradict each other), tool_defect, or other.\n\
\n\
Write out/unstick.json, exactly these keys:\n\
{\"move\": \"<an id from moves>\", \"diagnosis\": \"<two or three sentences>\",\n\
 \"defect_class\": \"<one of the classes>\", \"reasons\": [\"<why this move, one sentence each>\"]}\n",
    );
    if attempt > 1 {
        p.push_str(&format!(
            "\nThis is attempt {attempt}. Your previous reply was refused by the program check:\n  {invalid}\nWrite out/unstick.json again with that fixed.\n"
        ));
    }
    p.push_str("\nWhen the file is written, reply with the single word: done.\n");
    p
}

/// The reply checked against the moves: `(why, move)`, `why` empty when valid.
fn check(reply: &Value, moves: &[Value]) -> (String, Value) {
    let known = ["defect_class", "diagnosis", "move", "reasons"];
    let Some(m) = reply.as_object() else {
        return ("not a JSON object".into(), Value::Null);
    };
    let extra: Vec<&String> = m.keys().filter(|k| !known.contains(&k.as_str())).collect();
    if !extra.is_empty() {
        return (format!("unknown keys {extra:?}"), Value::Null);
    }
    let chosen = moves.iter().find(|x| x["id"] == reply["move"]).cloned();
    let why = if chosen.is_none() {
        format!("move {} is not one of the ids in moves", reply["move"])
    } else if s(&reply["diagnosis"]).trim().len() < 20 {
        "diagnosis is missing or shorter than a sentence".into()
    } else if !CLASSES.contains(&s(&reply["defect_class"])) {
        format!("defect_class {} is not one of {}", reply["defect_class"], CLASSES.join(", "))
    } else if !reply["reasons"]
        .as_array()
        .is_some_and(|r| !r.is_empty() && r.iter().all(|x| !s(x).is_empty()))
    {
        "reasons is not a non-empty list of sentences".into()
    } else {
        String::new()
    };
    (why, chosen.unwrap_or(Value::Null))
}

fn run(cli: Cli) -> R<()> {
    match cli.cmd {
        Cmd::Moves { stop } => {
            let stop = read(&stop)?;
            let ir = read(&Path::new(s(&stop["case_dir"])).join("ir.json")).unwrap_or(Value::Null);
            println!("{}", Value::Array(moves(&stop, &ir)));
        }
        Cmd::Prompt { stop, moves, decision, record, attempt, invalid } => {
            let stop = read(&stop)?;
            let moves = read(&moves)?;
            let moves = moves.as_array().ok_or("moves is not a list")?;
            let decision = read(&decision).unwrap_or(Value::Null);
            let cwd = record.join("cwd");
            fs::create_dir_all(cwd.join("in")).map_err(|e| e.to_string())?;
            fs::create_dir_all(cwd.join("out")).map_err(|e| e.to_string())?;
            let w = |p: PathBuf, v: String| fs::write(&p, v).map_err(|e| format!("{}: {e}", p.display()));
            w(cwd.join("in/stop.json"), serde_json::to_string_pretty(&input(&stop, moves, &decision)).unwrap_or_default())?;
            w(record.join("reply-schema.json"), schema(moves).to_string())?;
            w(record.join("prompt.md"), prompt(attempt, &invalid))?;
        }
        Cmd::Validate { moves, reply } => {
            let moves = read(&moves)?;
            let moves = moves.as_array().ok_or("moves is not a list")?;
            let out = match fs::read_to_string(&reply) {
                Err(_) => json!({"valid": false, "why": "out/unstick.json is missing"}),
                Ok(t) => match serde_json::from_str::<Value>(&t) {
                    Err(_) => json!({"valid": false, "why": "out/unstick.json is not JSON"}),
                    Ok(r) => {
                        let (why, m) = check(&r, moves);
                        json!({"valid": why.is_empty(), "why": if why.is_empty() { Value::Null } else { json!(why) },
                            "move": m, "diagnosis": r["diagnosis"], "defect_class": r["defect_class"], "reasons": r["reasons"]})
                    }
                },
            };
            println!("{out}");
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sdc-unstick: {e}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stop(kind: &str, request: Value, recorded: bool) -> Value {
        json!({"cause": "rejected_without_rework_path", "case": "IDEA-1.b.s", "case_dir": "/nonexistent",
            "action": "merge.approve", "kind": kind, "request": request, "rejection_recorded": recorded,
            "position": {"spent": [{"action": "change.implement"}], "not_called": []}})
    }

    #[test]
    fn a_rejected_approval_offers_rerun_end_and_stop_and_reask_or_approve_only_while_open() {
        let ir = json!({"outcomes": {"passed": {"requires": {"all": []}}, "dropped": {"requires": {"decision": "explicitly_dropped"}}}});
        let req = json!({"type": "approval", "action": "merge.approve", "capability": "change.merge"});
        let ids: Vec<Value> = moves(&stop("approval", req.clone(), true), &ir).iter().map(|m| m["id"].clone()).collect();
        assert_eq!(ids, vec![json!("rerun:change.implement"), json!("end:dropped"), json!("stop")]);
        let ids: Vec<Value> = moves(&stop("approval", req, false), &ir).iter().map(|m| m["id"].clone()).collect();
        assert_eq!(ids, vec![json!("rerun:change.implement"), json!("reask:merge.approve"), json!("approve:merge.approve"),
            json!("end:dropped"), json!("stop")]);
    }

    #[test]
    fn a_finding_offers_reask_and_its_default() {
        let req = json!({"type": "finding", "action": "spec.approve", "finding": {"id": "f1", "default": "opt1"}});
        let ids: Vec<Value> = moves(&stop("finding", req, false), &Value::Null).iter().map(|m| m["id"].clone()).collect();
        assert_eq!(ids, vec![json!("rerun:change.implement"), json!("reask:f1"), json!("default:f1"), json!("stop")]);
    }

    #[test]
    fn a_reply_must_pick_a_listed_move_and_say_what_is_missing() {
        let ms = vec![json!({"id": "stop"}), json!({"id": "rerun:w"})];
        let ok = json!({"move": "rerun:w", "diagnosis": "The writer was spent by a checker fault.", "defect_class": "writer_error", "reasons": ["it can fix it"]});
        assert_eq!(check(&ok, &ms).0, "");
        let mut bad = ok.clone();
        bad["move"] = json!("approve:x");
        assert!(check(&bad, &ms).0.contains("not one of the ids"));
        let mut bad = ok.clone();
        bad["defect_class"] = json!("bug");
        assert!(check(&bad, &ms).0.contains("defect_class"));
        let mut bad = ok;
        bad["extra"] = json!(1);
        assert!(check(&bad, &ms).0.contains("unknown keys"));
    }
}
