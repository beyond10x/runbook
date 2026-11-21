//! sdc-drive: one Canon case on disk, driven one deterministic step at a time.
//!
//! Canon decides every status; this binary only reads `canon-decision/1`, runs the tool bound to
//! one admissible action, mints the `canon-evidence/1` record the binding maps the tool's exit
//! code to, and asks Canon again. Every instant Canon, evidence or the event log sees is passed
//! with `--at`; the wall clock reaches only the observation records (`observed.jsonl`, `driver.json`).
//!
//! TEMPORARY (docs/boundary.md row 27): the child-case runtime below (`open_children`, `close`
//! writing `case_outcome` into the parent, `drive` descending into open children, and the
//! `copy` binding behind `roadmap.read`) is a prototype copy of what Loom must provide. It moves
//! to Loom's governed runtime and is deleted here once Loom has it.

use clap::{Parser, Subcommand};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

#[derive(Parser)]
#[command(
    about = "Drive one Canon case: evaluate, run an admissible action's bound tool, record evidence, re-evaluate"
)]
struct Cli {
    /// The case directory (see docs/driver.md, "Case store").
    #[arg(long)]
    case_dir: PathBuf,
    /// The `canon` binary.
    #[arg(long, default_value = "canon")]
    canon: String,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Open a case: compile the protocol, write the snapshot, empty evidence and authority.
    /// `--bindings` is the action-to-tool JSON; `--artifact name=revision` overrides an artifact's
    /// initial revision; `--independent producer=dim,dim` once per producer listed as independent.
    Init {
        #[arg(long)]
        protocol: PathBuf,
        #[arg(long)]
        id: String,
        #[arg(long)]
        bindings: PathBuf,
        #[arg(long = "artifact")]
        artifacts: Vec<String>,
        #[arg(long = "independent")]
        independent: Vec<String>,
    },
    /// Evaluate the case and print where it is.
    Eval {
        #[arg(long)]
        at: String,
    },
    /// Print the admissible actions as tool definitions (name, description, input schema).
    Tools {
        #[arg(long)]
        at: String,
    },
    /// Take one step: evaluate, select an action (`--action` must be admissible; without it the
    /// fixed policy selects), run its tool, record evidence, re-evaluate.
    Step {
        #[arg(long)]
        at: String,
        #[arg(long)]
        action: Option<String>,
        #[arg(long, default_value = "{}")]
        args: String,
    },
    /// Take steps with the fixed policy, first in every open child case, until the case is
    /// terminable, awaits authority or a person, or is stuck. A child that becomes terminable with
    /// exactly one legitimate outcome is closed through it.
    /// `--max-llm-calls` caps the agent-node calls (bindings with `llm`) over the whole case tree,
    /// counted from the logs, so it holds across runs; `--max-wall-seconds` stops this run before
    /// its next step once that much time has passed. A wait on a person stops with `message` and
    /// `next`: the exact `sdc-drive` command for each decision.
    Run {
        #[arg(long)]
        at: String,
        #[arg(long, default_value_t = 200)]
        max_steps: u32,
        #[arg(long)]
        max_llm_calls: Option<usize>,
        #[arg(long)]
        max_wall_seconds: Option<u64>,
    },
    /// Record an authority decision on a capability, taken by a named principal; with `--action`,
    /// then take that action as the principal (a person's approval record). `--result rejected
    /// --reasons <file>` (with `--action`) records the approval with that result instead of the
    /// one its binding maps the exit to; the result must be one of the binding's `results`, and
    /// the reasons file (sdc.rejection/1) is the step's log blob, which a writer whose binding
    /// names the kind in `retry_when` gets back in SDC_FEEDBACK.
    Approve {
        #[arg(long)]
        at: String,
        #[arg(long)]
        capability: String,
        #[arg(long)]
        principal: String,
        #[arg(long)]
        deny: bool,
        #[arg(long)]
        action: Option<String>,
        #[arg(long, requires = "action", requires = "reasons")]
        result: Option<String>,
        #[arg(long, requires = "result")]
        reasons: Option<PathBuf>,
    },
    /// Record a decision of the protocol (`canon-decisions/1`: decision, outcome, principal, case
    /// revision) for an outcome that requires that decision alone. Canon admits such a decision
    /// from any principal in any state, so the driver admits it only directly after the case's
    /// last step denied `--after-denial` (`approve --deny`): a refused release is withheld
    /// (`explicitly_withheld` for `withheld` after `release.approve` was denied). The reasons file
    /// is the decision's log blob. Refused before anything is written otherwise.
    Decide {
        #[arg(long)]
        at: String,
        #[arg(long)]
        decision: String,
        #[arg(long)]
        outcome: String,
        #[arg(long)]
        principal: String,
        #[arg(long)]
        after_denial: String,
        #[arg(long)]
        reasons: PathBuf,
    },
    /// Terminate the case through an outcome; Canon refuses one that is not legitimate.
    /// `--principal` names who closed it on the `terminated` event (`rule:autopilot` for a close
    /// the outcome's claims settle alone); without it the event names none.
    Close {
        #[arg(long)]
        at: String,
        #[arg(long)]
        outcome: String,
        #[arg(long)]
        principal: Option<String>,
    },
    /// One more allowance of `max_runs` for the writer `--action` (keep-going, D142): records
    /// `allowance_granted` {action, principal}; the writer's runs count from it, and a
    /// `not_called` run before it no longer keeps the writer back (`useful`). Refused for an
    /// action that is not a bound writer of the case.
    Allow {
        #[arg(long)]
        at: String,
        #[arg(long)]
        action: String,
        #[arg(long)]
        principal: String,
    },
    /// Re-run `canon evaluate` on every case of the tree as last evaluated and compare the state
    /// and decision digests with the ones the log recorded.
    Replay,
}

type R<T> = Result<T, String>;

fn read_json(p: &Path) -> R<Value> {
    let s = fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    serde_json::from_str(&s).map_err(|e| format!("{}: {e}", p.display()))
}

/// The bytes `write_json` writes, so a digest of a document can be taken before it is written.
fn json_bytes(v: &Value) -> Vec<u8> {
    let mut s = serde_json::to_string_pretty(v).unwrap();
    s.push('\n');
    s.into_bytes()
}

/// Write through a temporary file and a rename, so a crash leaves the old or the new document.
fn write_json(p: &Path, v: &Value) -> R<()> {
    let tmp = p.with_extension("json.tmp");
    fs::write(&tmp, json_bytes(v)).map_err(|e| format!("{}: {e}", tmp.display()))?;
    fs::rename(&tmp, p).map_err(|e| format!("{}: {e}", p.display()))
}

/// Wall clock as RFC 3339 UTC with milliseconds. Observation only (`observed.jsonl`,
/// `driver.json`): it never reaches Canon, evidence or the event log.
fn wall_clock() -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let (days, rem) = (ms.div_euclid(86_400_000), ms.rem_euclid(86_400_000));
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let (d, m) = (
        doy - (153 * mp + 2) / 5 + 1,
        if mp < 10 { mp + 3 } else { mp - 9 },
    );
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3_600_000,
        rem / 60_000 % 60,
        rem / 1000 % 60,
        rem % 1000
    )
}

/// One writer per case tree: `<root>/.sdc-drive.lock` holds the writer's pid. A lock whose pid no
/// longer runs is stale and taken over. Beside it, `driver.json` (`sdc.driver/1`) says which
/// driver holds it. Both are removed when dropped, `driver.json` first.
struct Lock(PathBuf);

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.0.with_file_name("driver.json"));
        let _ = fs::remove_file(&self.0);
    }
}

fn lock(dir: &Path) -> R<Lock> {
    let p = tree_root(dir).join(".sdc-drive.lock");
    for _ in 0..2 {
        match fs::OpenOptions::new().write(true).create_new(true).open(&p) {
            Ok(mut f) => {
                use std::io::Write;
                write!(f, "{}", std::process::id()).map_err(|e| e.to_string())?;
                let l = Lock(p);
                // A stale `driver.json` (its pid gone with its lock) is overwritten here.
                write_json(
                    &l.0.with_file_name("driver.json"),
                    &json!({"format": "sdc.driver/1",
                    "pid": std::process::id(), "started_at": wall_clock(),
                    "argv": std::env::args().collect::<Vec<_>>()}),
                )?;
                return Ok(l);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let pid = fs::read_to_string(&p).unwrap_or_default();
                if !pid.trim().is_empty() && Path::new("/proc").join(pid.trim()).exists() {
                    return Err(format!(
                        "{}: the case tree is in use by pid {}",
                        p.display(),
                        pid.trim()
                    ));
                }
                fs::remove_file(&p).map_err(|e| e.to_string())?; // stale: its writer is gone
            }
            Err(e) => return Err(format!("{}: {e}", p.display())),
        }
    }
    Err(format!("{}: could not take the lock", p.display()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// Content digest of a file tree: relative paths and bytes, in path order.
fn tree_digest(root: &Path, h: &mut Sha256) -> R<()> {
    let mut stack = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(p) = stack.pop() {
        for e in fs::read_dir(&p).map_err(|e| format!("{}: {e}", p.display()))? {
            let path = e.map_err(|e| e.to_string())?.path();
            if path.is_dir() {
                stack.push(path)
            } else {
                files.push(path)
            }
        }
    }
    files.sort();
    for f in files {
        h.update(f.strip_prefix(root).unwrap().as_os_str().as_encoded_bytes());
        h.update(fs::read(&f).map_err(|e| e.to_string())?);
    }
    Ok(())
}

/// An artifact's revision: `t` + the first 16 hex digits of its content digest, `none` if absent.
fn revision_of(p: &Path) -> R<String> {
    if !p.exists() {
        return Ok("none".into());
    }
    let mut h = Sha256::new();
    if p.is_dir() {
        tree_digest(p, &mut h)?
    } else {
        h.update(fs::read(p).map_err(|e| e.to_string())?)
    }
    Ok(format!("t{}", &hex(&h.finalize())[..16]))
}

/// Digest of everything Canon reads: the same digest is the same decision.
fn state_digest(dir: &Path) -> R<String> {
    let mut h = Sha256::new();
    for f in ["ir.json", "case.json", "authority.json", "decisions.json"] {
        h.update(fs::read(dir.join(f)).map_err(|e| format!("{f}: {e}"))?);
    }
    tree_digest(&dir.join("evidence"), &mut h)?;
    Ok(hex(&h.finalize()))
}

fn events(dir: &Path) -> Vec<Value> {
    fs::read_to_string(dir.join("events.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// Append one event to the hash-chained log: `prev` is the digest of the line before it. Then one
/// line to the sidecar `observed.jsonl` (`sdc.observed/1`): the event's line index and the wall
/// time it was written, for a live view. Replay never reads the sidecar.
fn event(dir: &Path, at: &str, kind: &str, body: Value) -> R<()> {
    let text = fs::read_to_string(dir.join("events.jsonl")).unwrap_or_default();
    let last = text.lines().last();
    let index = text.lines().count();
    let mut e =
        json!({"seq": index, "prev": last.map(|l| sha(l.as_bytes())), "at": at, "event": kind});
    e.as_object_mut()
        .unwrap()
        .extend(body.as_object().cloned().unwrap_or_default());
    // Appended, never rewritten: a crash can cut the last line, not the lines before it.
    use std::io::Write;
    let append = |name: &str, v: &Value| -> R<()> {
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(name))
            .map_err(|e| e.to_string())?;
        f.write_all(format!("{}\n", serde_json::to_string(v).unwrap()).as_bytes())
            .map_err(|e| e.to_string())
    };
    append("events.jsonl", &e)?;
    // An observation record that cannot be written does not stop the step.
    let _ = append(
        "observed.jsonl",
        &json!({"format": "sdc.observed/1", "index": index, "event": kind,
        "action": body.get("action").cloned().unwrap_or(Value::Null), "observed_at": wall_clock(),
        "pid": std::process::id()}),
    );
    Ok(())
}

/// The snapshot `sync` would write: every artifact the bindings place in `work/` (`$artifacts`:
/// artifact → path) at the content digest of what is there now, and the next case revision if any
/// moved. Nothing is written.
fn pending(dir: &Path) -> R<(Value, Map<String, Value>)> {
    let b = read_json(&dir.join("bindings.json"))?;
    let mut case = read_json(&dir.join("case.json"))?;
    let mut moved = Map::new();
    for (art, path) in b["$artifacts"].as_object().into_iter().flatten() {
        let rev = revision_of(&dir.join("work").join(path.as_str().unwrap_or("")))?;
        if case["artifacts"][art]["revision"] != rev.as_str() {
            case["artifacts"][art]["revision"] = json!(rev);
            moved.insert(art.clone(), json!(rev));
        }
    }
    if !moved.is_empty() {
        let n = events(dir)
            .iter()
            .filter(|e| e["event"] == "revised")
            .count()
            + 1;
        case["revision"] = json!(format!("c{n}"));
    }
    Ok((case, moved))
}

/// Write a pending snapshot: `case.json`, then one `revised` event.
fn commit(dir: &Path, at: &str, case: &Value, moved: &Map<String, Value>) -> R<()> {
    if moved.is_empty() {
        return Ok(());
    }
    write_json(&dir.join("case.json"), case)?;
    event(
        dir,
        at,
        "revised",
        json!({"artifacts": moved, "case_revision": case["revision"]}),
    )
}

/// Move every artifact to the content digest of what is in `work/` now; one `revised` event and
/// one case revision per change. All cases of a tree share the root's `work/`, so a child's writes
/// (or a person's edit) reach its parent here.
fn sync(dir: &Path, at: &str) -> R<()> {
    let (case, moved) = pending(dir)?;
    commit(dir, at, &case, &moved)
}

fn canon_evaluate(canon: &str, d: &Path, at: &str) -> R<Vec<u8>> {
    let out = Command::new(canon)
        .args(["evaluate", "--ir"])
        .arg(d.join("ir.json"))
        .arg("--case")
        .arg(d.join("case.json"))
        .arg("--evidence")
        .arg(d.join("evidence"))
        .arg("--authority")
        .arg(d.join("authority.json"))
        .arg("--decisions")
        .arg(d.join("decisions.json"))
        .args(["--at", at])
        .output()
        .map_err(|e| format!("{canon}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "canon refused: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

fn evaluate(canon: &str, d: &Path, at: &str) -> R<Value> {
    sync(d, at)?;
    let stdout = canon_evaluate(canon, d, at)?;
    let decision: Value = serde_json::from_slice(&stdout).map_err(|e| e.to_string())?;
    let n = events(d).len();
    fs::write(d.join("evaluations").join(format!("{n:04}.json")), &stdout)
        .map_err(|e| e.to_string())?;
    event(
        d,
        at,
        "evaluated",
        json!({"state": state_digest(d)?, "decision": sha(&stdout),
        "file": format!("evaluations/{n:04}.json")}),
    )?;
    Ok(decision)
}

fn with_status<'a>(sec: &'a Value, status: &str) -> Vec<&'a String> {
    sec.as_object()
        .map(|m| {
            m.iter()
                .filter(|(_, v)| v["status"] == status)
                .map(|(k, _)| k)
                .collect()
        })
        .unwrap_or_default()
}

fn evidence_records(dir: &Path) -> R<Vec<Value>> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir.join("evidence"))
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    files.sort();
    files.iter().map(|f| read_json(f)).collect()
}

/// The root of the case tree: children live at `<parent>/children/<id>/`.
fn tree_root(dir: &Path) -> PathBuf {
    let mut d = dir.to_path_buf();
    while d
        .parent()
        .and_then(|p| p.file_name())
        .is_some_and(|n| n == "children")
    {
        d = d.parent().unwrap().parent().unwrap().to_path_buf();
    }
    d
}

fn children(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir.join("children"))
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("case.json").exists())
        .collect();
    v.sort();
    v
}

fn all_cases(dir: &Path) -> Vec<PathBuf> {
    let mut v = vec![dir.to_path_buf()];
    for c in children(dir) {
        v.extend(all_cases(&c));
    }
    v
}

/// TEMPORARY (Loom, boundary.md row 28): the record a `copy` binding copies, the latest record of
/// its kind at the current subject revision of a case in the same tree under the named protocol.
fn copy_source(dir: &Path, b: &Value) -> R<Option<Value>> {
    let mut found = None;
    for c in all_cases(&tree_root(dir)) {
        let case = read_json(&c.join("case.json"))?;
        if case["protocol"] != b["copy"]["from"] {
            continue;
        }
        for r in evidence_records(&c)? {
            let current = case["artifacts"][r["subject"].as_str().unwrap_or("")]["revision"]
                == r["subject_revision"];
            if r["kind"] == b["copy"]["kind"] && current {
                found = Some(r);
            }
        }
    }
    Ok(found)
}

/// The records about the current revision of their subject that Canon did not set aside.
fn current_records<'a>(dir: &Path, d: &Value, records: &'a [Value]) -> R<Vec<&'a Value>> {
    let case = read_json(&dir.join("case.json"))?;
    let excluded: Vec<&Value> = d["claims"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(_, c)| c)
        .flat_map(|c| {
            c["excluded_evidence"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|x| &x["evidence"])
        })
        .collect();
    Ok(records
        .iter()
        .filter(|r| {
            case["artifacts"][r["subject"].as_str().unwrap_or("")]["revision"]
                == r["subject_revision"]
                && !excluded.contains(&&r["id"])
        })
        .collect())
}

/// The records at the current revision that send a writer back to work: the binding's
/// `retry_when` {kind → [results]} (a check of what the writer wrote failed).
fn retry_records<'a>(b: &Value, current: &[&'a Value]) -> Vec<&'a Value> {
    let Some(when) = b["retry_when"].as_object() else {
        return vec![];
    };
    // A check of an artifact not written yet (`none`) sends nobody back: there is nothing to fix.
    current
        .iter()
        .copied()
        .filter(|r| {
            r["subject_revision"] != "none"
                && when
                    .get(r["kind"].as_str().unwrap_or(""))
                    .and_then(|v| v.as_array())
                    .is_some_and(|rs| rs.contains(&r["result"]))
        })
        .collect()
}

/// The admissible actions that can still change something, in the order the fixed policy takes
/// them: observations (`effect` other than `write`) before writes, then identifier order. An action
/// that may produce evidence is useful while a kind it produces has no applicable record at the
/// subject's current revision; one that produces none, until it succeeded against this case
/// snapshot (when to stop rewriting is the protocol's precondition, not the binding's). A `copy`
/// binding is useful while its source holds a record this case lacks. A writer is also useful
/// while a `retry_when` record about the current revision exists. An action that has run its
/// binding's `max_runs` (default 8) is not useful. Returns the driver's actions and, apart, the
/// useful `manual` ones: a person's actions, taken only through `approve --action`.
/// Exit 126 or 127 (a program the tool runs is missing or not executable), or 75 (a temporary
/// failure: an LLM node's model API refused or broke off the session, such as a 429): the tool
/// could not run, which says nothing about its subject.
fn program_error(exit: &Value) -> bool {
    exit == "126" || exit == "127" || exit == "75"
}

/// The index in `evs` of the last arrival: a `revised` event that no agent action of this case
/// produced (the case opening, a person's edit in `work/`, a person's `approve --action`, or a
/// write that reached this case from another case of the tree). A revision committed after an
/// observation is an outside edit the observation's step found, not the observation's. 0 when
/// there is none.
fn last_arrival(evs: &[Value], ir: &Value) -> usize {
    evs.iter()
        .enumerate()
        .filter(|(i, e)| {
            e["event"] == "revised"
                && !(*i > 0
                    && evs[i - 1]["event"] == "action_finished"
                    && evs[i - 1]["principal"].is_null()
                    && ir["actions"][evs[i - 1]["action"].as_str().unwrap_or("")]["effect"]
                        == "write")
        })
        .map(|(i, _)| i)
        .next_back()
        .unwrap_or(0)
}

/// The last non-empty line of an event's log blob.
fn log_line(dir: &Path, e: &Value) -> String {
    e["log"].as_str().and_then(|l| fs::read(dir.join("blobs").join(l)).ok())
        .map(|b| String::from_utf8_lossy(&b).lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").to_string())
        .unwrap_or_default()
}

/// A writer that would be useful but has run its `max_runs` since the last arrival. `sent_back`:
/// for each counted run, the first other action after it (before the writer's next run) that
/// exited non-zero, with its last log line: what made the writer useful again (runs/trial-3-haiku-2
/// stopped with "last exit 0:" and nothing else, though each run was sent back by a new finding).
fn spent_entry(dir: &Path, a: &str, runs: u64, max: u64, since: usize, evs: &[Value]) -> Value {
    let finished: Vec<&Value> = evs.iter().filter(|e| e["event"] == "action_finished").collect();
    let last = finished.iter().rev().find(|e| e["action"] == a && !program_error(&e["exit"]));
    let line = last.map(|e| log_line(dir, e)).unwrap_or_default();
    let mut sent_back = Vec::new();
    let after: Vec<&Value> = evs.iter().skip(since).filter(|e| e["event"] == "action_finished" && !program_error(&e["exit"])).collect();
    for (i, run) in after.iter().enumerate().filter(|(_, e)| e["action"] == a && !e["not_called"].is_string()) {
        let by = after[i + 1..].iter().take_while(|e| e["action"] != a).find(|e| e["exit"] != "0");
        sent_back.push(json!({"run_seq": run["seq"], "by": by.map(|e| e["action"].clone()),
            "exit": by.map(|e| e["exit"].clone()), "line": by.map(|e| log_line(dir, e))}));
    }
    json!({"action": a, "runs": runs, "max_runs": max, "since_seq": since,
        "last_exit": last.map(|e| e["exit"].clone()), "last_failure": line, "sent_back": sent_back})
}

/// `sent_back` as one line for the stop message.
fn sent_back_text(s: &Value) -> String {
    let parts: Vec<String> = s["sent_back"].as_array().into_iter().flatten().map(|b| match b["by"].as_str() {
        Some(by) => {
            let line: String = b["line"].as_str().unwrap_or("").chars().take(160).collect();
            format!("run {} then {by} exit {}: {line}", b["run_seq"], b["exit"].as_str().unwrap_or("?"))
        }
        None => format!("run {} then nothing failed", b["run_seq"]),
    }).collect();
    if parts.is_empty() { String::new() } else { format!("; sent back: {}", parts.join("; ")) }
}

/// The useful actions (`auto`, `manual`, see above) and the spent writers. Runs are counted so:
/// - an observation (not `effect: write`) has no run cap: it runs at most once per case snapshot
///   (the revisions of the artifacts it reads are part of it), and only while it is due (a kind
///   it may produce has no applicable record at its subject's revision; with no kind, until it
///   ran once at this snapshot);
/// - a writer has `max_runs` (default 8) per arrival: its runs since the last arrival
///   (`last_arrival`) count, so a person's decision or an outside revision gives the writers a
///   fresh allowance. A writer that would be useful but has none left is returned as spent.
///
/// A run that ended in a program error (exit 126/127) never counts, nor one that ended
/// `not_called` (the tool did nothing: `llm-node.sh`'s "not called" exits, runs/trial-3-haiku-4).
/// A writer whose last run was `not_called` at this very snapshot is returned apart: running it
/// again would do nothing again.
type Useful = (Vec<String>, Vec<String>, Vec<Value>, Vec<Value>);
fn useful(dir: &Path, d: &Value) -> R<Useful> {
    let ir = read_json(&dir.join("ir.json"))?;
    let bindings = read_json(&dir.join("bindings.json"))?;
    let case_bytes = fs::read(dir.join("case.json")).map_err(|e| e.to_string())?;
    let snapshot = sha(&case_bytes);
    let records = evidence_records(dir)?;
    let current = current_records(dir, d, &records)?;
    let present: Vec<&Value> = current.iter().map(|r| &r["kind"]).collect();
    let evs = events(dir);
    let since = last_arrival(&evs, &ir);
    let finished: Vec<&Value> = evs
        .iter()
        .filter(|e| e["event"] == "action_finished")
        .collect();
    let mut auto = Vec::new();
    let mut manual = Vec::new();
    let mut spent = Vec::new();
    let mut not_called = Vec::new();
    for a in with_status(&d["actions"], "admissible") {
        let b = &bindings[a.as_str()];
        if b.is_null() {
            continue; // no tool bound
        }
        let writer = ir["actions"][a.as_str()]["effect"] == "write";
        let ran = |e: &&&Value| e["action"] == a.as_str() && !program_error(&e["exit"]);
        // An observation whose step also committed an outside edit had its record discarded: it
        // did not observe this snapshot.
        let ran_here = finished.iter().any(|e| {
            ran(&e)
                && e["case"] == snapshot.as_str()
                && e["revises"].as_object().is_none_or(|m| m.is_empty())
        });
        let is_useful = if b.get("copy").is_some() {
            copy_source(dir, b)?.is_some_and(|r| !records.iter().any(|x| x["id"] == r["id"]))
        } else {
            match ir["actions"][a.as_str()]["may_produce"]
                .as_array()
                .filter(|m| !m.is_empty())
            {
                Some(kinds) => {
                    kinds.iter().any(|k| !present.contains(&&k["evidence"]))
                        && (writer || !ran_here)
                }
                None if writer => !finished.iter().any(|e| {
                    e["action"] == a.as_str() && e["exit"] == "0" && e["case"] == snapshot.as_str()
                }),
                None => !ran_here,
            }
        } || (writer && !retry_records(b, &current).is_empty());
        if !is_useful {
            continue;
        }
        // keep-going's one more allowance (`allow`, D142): the writer's runs count from it.
        let granted = evs
            .iter()
            .rposition(|e| e["event"] == "allowance_granted" && e["action"] == a.as_str());
        let since_a = granted.map_or(since, |g| g.max(since));
        // A writer whose run ended without doing anything (`not_called`, see the step) as the
        // case's last step, at this snapshot, would see the same inputs and end the same way
        // again: not useful. After any other step (a check that failed since), or an allowance
        // granted since, it is.
        if let Some(e) = finished.last().filter(|e| {
            ran(e)
                && e["not_called"].is_string()
                && e["case"] == snapshot.as_str()
                && granted.is_none_or(|g| e["seq"].as_u64() > evs[g]["seq"].as_u64())
        }) {
            not_called.push(json!({"action": a, "run_seq": e["seq"], "reason": e["not_called"]}));
            continue;
        }
        if writer {
            let runs = evs[since_a..]
                .iter()
                .filter(|e| {
                    e["event"] == "action_finished"
                        && e["action"] == a.as_str()
                        && !program_error(&e["exit"])
                        && !e["not_called"].is_string()
                })
                .count() as u64;
            let max = b["max_runs"].as_u64().unwrap_or(8);
            if runs >= max {
                spent.push(spent_entry(dir, a, runs, max, since_a, &evs));
                continue;
            }
        }
        let key = (writer, a.clone());
        if b["manual"] == true {
            manual.push(key)
        } else {
            auto.push(key)
        }
    }
    auto.sort();
    manual.sort();
    Ok((
        auto.into_iter().map(|(_, a)| a).collect(),
        manual.into_iter().map(|(_, a)| a).collect(),
        spent,
        not_called,
    ))
}

/// Where the case is: what Canon decided, and which admissible actions can still change it.
fn position(dir: &Path, d: &Value) -> R<Value> {
    let open: Vec<&Value> = d["obligations"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|o| o["status"] == "open")
                .map(|o| &o["id"])
                .collect()
        })
        .unwrap_or_default();
    let (auto, manual, spent, not_called) = useful(dir, d)?;
    let case = read_json(&dir.join("case.json"))?;
    Ok(
        json!({"case": case["id"], "state": state_digest(dir)?, "claims": d["claims"], "open_obligations": open,
        "admissible": with_status(&d["actions"], "admissible"), "useful": auto, "awaiting_person": manual,
        "spent": spent, "not_called": not_called,
        "approval_required": with_status(&d["actions"], "approval-required"),
        "legitimate_outcomes": with_status(&d["outcomes"], "legitimate")}),
    )
}

fn subst(v: &Value, from: &str, to: &str) -> Value {
    serde_json::from_str(&serde_json::to_string(v).unwrap().replace(from, to)).unwrap()
}

/// Write a case store: compiled IR, snapshot with every declared artifact (`none` until it
/// exists), empty authority and decisions, the bindings with `{dir}` resolved to the directory
/// of the bindings file they came from. A child's `work` is a link to its parent's.
#[allow(clippy::too_many_arguments)]
fn init_case(
    canon: &str,
    dir: &Path,
    protocol: &Path,
    id: &str,
    bindings: &Path,
    revisions: &Map<String, Value>,
    independence: &Map<String, Value>,
    child: bool,
) -> R<Value> {
    for sub in ["evidence", "evaluations", "blobs"] {
        fs::create_dir_all(dir.join(sub)).map_err(|e| e.to_string())?;
    }
    if child {
        std::os::unix::fs::symlink("../../work", dir.join("work")).map_err(|e| e.to_string())?;
    } else {
        fs::create_dir_all(dir.join("work")).map_err(|e| e.to_string())?;
    }
    let out = Command::new(canon)
        .args(["compile", "--path"])
        .arg(protocol)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    fs::write(dir.join("ir.json"), &out.stdout).map_err(|e| e.to_string())?;
    let ir: Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let mut arts = Map::new();
    for a in ir["artifacts"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, _)| k)
    {
        arts.insert(
            a.clone(),
            json!({"revision": revisions.get(a).cloned().unwrap_or(json!("none"))}),
        );
    }
    let src = fs::canonicalize(bindings).map_err(|e| format!("{}: {e}", bindings.display()))?;
    let b = subst(
        &read_json(&src)?,
        "{dir}",
        &src.parent().unwrap().to_string_lossy(),
    );
    let mut ind = b["$independence"].as_object().cloned().unwrap_or_default();
    ind.extend(independence.clone());
    let mut case = json!({"format": "canon-case/1", "id": id, "protocol": ir["protocol"]["id"],
        "revision": "c0", "artifacts": arts});
    if !ind.is_empty() {
        case["independence"] = json!(ind);
    }
    write_json(&dir.join("case.json"), &case)?;
    write_json(&dir.join("authority.json"), &json!([]))?;
    write_json(&dir.join("decisions.json"), &json!([]))?;
    write_json(&dir.join("bindings.json"), &b)?;
    event(
        dir,
        "-",
        "opened",
        json!({"protocol": ir["protocol"], "ir": sha(&out.stdout), "bindings": src}),
    )?;
    Ok(json!({"opened": id}))
}

/// TEMPORARY (Loom, boundary.md row 27): open the child cases a `case_open` action names, one per
/// entry of the argument the binding's `child.each` names (else one, `child.name`), each under
/// `children/<parent id>.<name>/` with the child protocol and bindings, and write one
/// `case_outcome` record per child into this case, `open` until the child terminates.
fn open_children(
    canon: &str,
    dir: &Path,
    at: &str,
    action: &str,
    decl: &Value,
    b: &Value,
    args: &Value,
) -> R<Vec<String>> {
    let c = &b["child"];
    let names: Vec<String> = match c["each"].as_str() {
        Some(k) => {
            serde_json::from_value(args[k].clone()).map_err(|e| format!("child.each `{k}`: {e}"))?
        }
        None => vec![c["name"]
            .as_str()
            .ok_or("binding names no child.name")?
            .to_string()],
    };
    let subject = decl["case_open"]["subject"]
        .as_str()
        .ok_or("action opens no case")?;
    let mut opened = Vec::new();
    for name in names {
        let case = read_json(&dir.join("case.json"))?;
        // A child opened again (its subject was revised) is a new case with a new id.
        let base = format!("{}.{name}", case["id"].as_str().unwrap());
        let id = (1..)
            .map(|n| {
                if n == 1 {
                    base.clone()
                } else {
                    format!("{base}.{n}")
                }
            })
            .find(|id| !dir.join("children").join(id).exists())
            .unwrap();
        let cdir = dir.join("children").join(&id);
        let revs = subst(&c["revisions"], "{name}", &name)
            .as_object()
            .cloned()
            .unwrap_or_default();
        init_case(
            canon,
            &cdir,
            Path::new(c["protocol"].as_str().ok_or("child.protocol")?),
            &id,
            Path::new(c["bindings"].as_str().ok_or("child.bindings")?),
            &revs,
            &Map::new(),
            true,
        )?;
        let eid = format!("e{:04}-{action}.{name}", events(dir).len());
        let rec = json!({"format": "canon-evidence/1", "id": eid, "kind": "case_outcome", "subject": subject,
            "subject_revision": case["artifacts"][subject]["revision"], "observed_at": at,
            "producer": "sdc-drive", "child_case": id, "result": "open"});
        write_json(&dir.join("evidence").join(format!("{eid}.json")), &rec)?;
        write_json(
            &cdir.join("link.json"),
            &json!({"parent": "../..", "evidence": format!("{eid}.json")}),
        )?;
        event(
            dir,
            at,
            "child_opened",
            json!({"child_case": id, "protocol": decl["case_open"]["protocol"], "evidence": eid}),
        )?;
        opened.push(id);
    }
    Ok(opened)
}

/// Terminate through an outcome. TEMPORARY (Loom, boundary.md row 27): a child then sets the
/// `result` of its record in the parent to the outcome, observed at `--at`. The record keeps the
/// `subject_revision` it was opened at (the parent's revision of the child's subject then): a
/// child is about that revision only, and a revised subject gets a new child with a new id, never
/// this record. Canon reads one `case_outcome` record per child, so `open` → outcome is the one
/// rewrite; the parent's log keeps both states.
fn close(canon: &str, dir: &Path, at: &str, outcome: &str, principal: Option<&str>) -> R<Value> {
    let p = dir.join("case.json");
    let before = read_json(&p)?;
    let mut case = before.clone();
    case["termination"] = json!(outcome);
    write_json(&p, &case)?;
    let d = evaluate(canon, dir, at).inspect_err(|_| {
        let _ = write_json(&p, &before);
    })?;
    let mut body = json!({"outcome": outcome});
    if let Some(p) = principal {
        body["principal"] = json!(p);
    }
    event(dir, at, "terminated", body)?;
    if let Ok(link) = read_json(&dir.join("link.json")) {
        let parent = dir.join(link["parent"].as_str().unwrap());
        let f = parent
            .join("evidence")
            .join(link["evidence"].as_str().unwrap());
        let mut rec = read_json(&f)?;
        if rec["result"] != "open" {
            return Err(format!(
                "{}: the child's outcome is already recorded",
                f.display()
            ));
        }
        rec["result"] = json!(outcome);
        rec["observed_at"] = json!(at);
        write_json(&f, &rec)?;
        event(
            &parent,
            at,
            "child_closed",
            json!({"child_case": case["id"], "outcome": outcome, "evidence": rec["id"]}),
        )?;
        // Every state write is followed by an evaluation: the rewritten record is part of the
        // parent's state digest, and a run that stops in a sibling before the parent's next step
        // would leave its last `evaluated` event behind the store (replay failed on
        // runs/trial-3-haiku-4's IDEA-1.todo).
        evaluate(canon, &parent, at)?;
    }
    Ok(json!({"case": case["id"], "terminated": outcome, "outcomes": d["outcomes"]}))
}

/// The person's next steps where the run waits on one: each with what is decided and the exact
/// `sdc-drive` command. `manual` actions are approvals of the subject's current revision;
/// approval-required actions need a capability granted; an admissible action whose binding has
/// a `handoff` (a writer that has done what it can) needs an edit in `work/`.
fn next_steps(canon: &str, dir: &Path, pos: &Value, bindings: &Value, ir: &Value) -> R<Vec<Value>> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // The printed command names the person who will run it: SDC_PRINCIPAL, else `person:$USER`;
    // with neither, a placeholder to fill in, never a made-up person.
    let principal = std::env::var("SDC_PRINCIPAL")
        .ok()
        .filter(|p| !p.is_empty())
        .or_else(|| {
            std::env::var("USER")
                .ok()
                .filter(|u| !u.is_empty())
                .map(|u| format!("person:{u}"))
        })
        .unwrap_or_else(|| "person:<your-id>".into());
    let case = read_json(&dir.join("case.json"))?;
    let approve = |cap: &str, action: Option<&str>| {
        format!(
        "{} --canon {canon} --case-dir {} approve --at \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\" --capability {cap} --principal {principal}{}",
        exe.display(), dir.display(), action.map(|a| format!(" --action {a}")).unwrap_or_default())
    };
    let caps = |a: &str| -> Vec<String> {
        ir["actions"][a]["requires"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| r["capability"].as_str().map(String::from))
            .collect()
    };
    let mut next = Vec::new();
    let list = |k: &str| -> Vec<String> {
        pos[k]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|a| a.as_str().map(String::from))
            .collect()
    };
    let manual: Vec<String> = list("awaiting_person")
        .into_iter()
        .chain(
            list("approval_required")
                .into_iter()
                .filter(|a| bindings[a]["manual"] == true),
        )
        .collect();
    for a in &manual {
        let subject = bindings[a]["subject"].as_str().unwrap_or("");
        let path = bindings["$artifacts"][subject].as_str().unwrap_or("?");
        for cap in caps(a) {
            next.push(json!({"action": a, "decides": format!("approve {subject} revision {} (read {}/work/{path})",
                case["artifacts"][subject]["revision"].as_str().unwrap_or("?"), dir.display()),
                "command": approve(&cap, Some(a))}));
        }
    }
    for a in list("approval_required") {
        let a = a.as_str();
        if bindings[a].is_null() || bindings[a]["manual"] == true {
            continue; // unbound: nothing would run; manual: listed above
        }
        for cap in caps(a) {
            next.push(json!({"action": a, "decides": format!("grant `{cap}` so the driver runs `{a}`: {}",
                ir["actions"][a]["description"].as_str().unwrap_or("")), "command": approve(&cap, None)}));
        }
    }
    for a in pos["admissible"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| a.as_str())
    {
        if let Some(h) = bindings[a]["handoff"].as_str() {
            next.push(json!({"action": a, "decides": subst(&json!(h), "{work}", &dir.join("work").to_string_lossy()),
                "command": format!("{} --canon {canon} --case-dir {} run --at \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"",
                    exe.display(), tree_root(dir).display())}));
        }
    }
    Ok(next)
}

/// LLM calls started so far in the whole case tree (`action_started` of a binding with `llm`),
/// less the runs that ended `not_called` (llm-node.sh exited before any model call): the count
/// of llm-calls.jsonl, which is what `--max-llm-calls` caps. The decider's calls are not driver
/// actions and are counted apart (decisions.jsonl, decider/).
fn llm_calls(dir: &Path) -> usize {
    all_cases(&tree_root(dir))
        .iter()
        .map(|c| {
            let evs = events(c);
            let started = evs
                .iter()
                .filter(|e| e["event"] == "action_started" && e["llm"] == true)
                .count();
            let not_called = evs
                .iter()
                .filter(|e| e["event"] == "action_finished" && e["not_called"].is_string())
                .count();
            started.saturating_sub(not_called)
        })
        .sum()
}

/// Run an argv without a shell in `work/`, in its own process group, killed after `timeout`
/// seconds (exit `timeout`). Returns the exit and stdout followed by stderr.
fn run_tool(
    dir: &Path,
    argv: &[String],
    env: &[(&str, String)],
    timeout: u64,
) -> R<(String, Vec<u8>)> {
    use std::os::unix::process::CommandExt;
    let (out_p, err_p) = (
        dir.join("blobs").join(".stdout"),
        dir.join("blobs").join(".stderr"),
    );
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .current_dir(dir.join("work"))
        .stdin(std::process::Stdio::null())
        .stdout(fs::File::create(&out_p).map_err(|e| e.to_string())?)
        .stderr(fs::File::create(&err_p).map_err(|e| e.to_string())?)
        .process_group(0);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().map_err(|e| format!("{}: {e}", argv[0]))?;
    let start = std::time::Instant::now();
    let code = loop {
        if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
            break st.code().map_or("signal".to_string(), |c| c.to_string());
        }
        if start.elapsed().as_secs() >= timeout {
            let _ = Command::new("kill")
                .args(["-KILL", "--", &format!("-{}", child.id())])
                .status();
            let _ = child.wait();
            break "timeout".to_string();
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    let mut log = fs::read(&out_p).unwrap_or_default();
    log.extend(fs::read(&err_p).unwrap_or_default());
    let _ = (fs::remove_file(&out_p), fs::remove_file(&err_p));
    if code == "timeout" {
        log.extend(format!("\nsdc-drive: killed after {timeout} s\n").into_bytes());
    }
    Ok((code, log))
}

/// Limits of one `run`: LLM calls over the whole case tree, and wall time of this invocation.
struct Limits {
    max_llm_calls: Option<usize>,
    deadline: Option<std::time::Instant>,
}

/// `forced`: a person's approval recorded with a set result (`approve --result`) and the log the
/// step keeps instead of the tool's output (the reasons).
#[allow(clippy::too_many_arguments)]
fn step(
    canon: &str,
    dir: &Path,
    at: &str,
    action: Option<&str>,
    args: &str,
    principal: Option<&str>,
    forced: Option<(&str, &[u8])>,
    limits: &Limits,
) -> R<(Value, u8)> {
    let decision = evaluate(canon, dir, at)?;
    let pos = position(dir, &decision)?;
    let admissible: Vec<String> = serde_json::from_value(pos["admissible"].clone()).unwrap();
    let useful: Vec<String> = serde_json::from_value(pos["useful"].clone()).unwrap();
    let bindings = read_json(&dir.join("bindings.json"))?;
    let ir = read_json(&dir.join("ir.json"))?;
    let chosen = match action {
        Some(a) if !admissible.iter().any(|x| x == a) => {
            return Err(format!(
                "action `{a}` is not admissible: {}",
                decision["actions"][a]
            ))
        }
        Some(a) if bindings[a]["manual"] == true && principal.is_none() => {
            return Err(format!(
                "`{a}` is a person's action: take it with `approve --action {a}`"
            ))
        }
        Some(a) => a.to_string(),
        // Fixed policy: the first useful action in the order `useful` gives.
        None => match useful.first() {
            Some(a) => a.clone(),
            None => {
                let mut next = next_steps(canon, dir, &pos, &bindings, &ir)?;
                // A handoff stands only while its binding's `handoff_when` (an argv run in `work/`)
                // exits 0: `amend.upstream` hands off only while a finding waits for a person.
                let mut kept = Vec::new();
                for n in next {
                    let b = &bindings[n["action"].as_str().unwrap()];
                    let due = match b["handoff_when"]
                        .as_array()
                        .filter(|_| b["handoff"].is_string())
                    {
                        Some(argv) => {
                            let argv: Vec<String> = argv
                                .iter()
                                .filter_map(|a| a.as_str().map(String::from))
                                .collect();
                            run_tool(dir, &argv, &[], 60)?.0 == "0"
                        }
                        None => true,
                    };
                    if due {
                        kept.push(n);
                    }
                }
                next = kept;
                // A spent writer that waits for a person's decision (an arrival gives it a fresh
                // allowance): a due handoff stands even while its action is not admissible.
                if !pos["spent"].as_array().unwrap().is_empty()
                    || !pos["not_called"].as_array().unwrap().is_empty()
                {
                    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                    for (a, b) in bindings.as_object().into_iter().flatten() {
                        let (Some(h), Some(argv)) =
                            (b["handoff"].as_str(), b["handoff_when"].as_array())
                        else {
                            continue;
                        };
                        if next.iter().any(|n| n["action"] == a.as_str()) {
                            continue;
                        }
                        let argv: Vec<String> = argv
                            .iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect();
                        if run_tool(dir, &argv, &[], 60)?.0 == "0" {
                            next.push(json!({"action": a, "decides": subst(&json!(h), "{work}", &dir.join("work").to_string_lossy()),
                                "command": format!("{} --canon {canon} --case-dir {} run --at \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"",
                                    exe.display(), tree_root(dir).display())}));
                        }
                    }
                }
                let nonempty = |k: &str| !pos[k].as_array().unwrap().is_empty();
                let handoff = next
                    .iter()
                    .any(|n| bindings[n["action"].as_str().unwrap()]["handoff"].is_string());
                let (verdict, code) = if nonempty("legitimate_outcomes") {
                    ("terminable", 0)
                } else if nonempty("awaiting_person") || handoff {
                    ("awaiting_person", 3)
                } else if (nonempty("spent") || nonempty("not_called")) && nonempty("approval_required") {
                    // A person's pending approval may be what the spent writer waits for
                    // (`scope.answer` after `questions.answer` refused a changed answer).
                    ("awaiting_authority", 3)
                } else if nonempty("not_called") {
                    // The writer would be useful but declines to do anything at this snapshot,
                    // and nothing else can move the case: its own stop, not three empty runs
                    // into writer_spent (runs/trial-3-haiku-4).
                    let s = &pos["not_called"][0];
                    event(dir, at, "writer_not_called", json!({"state": pos["state"], "not_called": pos["not_called"]}))?;
                    let message = format!("writer `{}` is not called at {} and nothing else is useful: run {} ended: {}",
                        s["action"].as_str().unwrap_or(""), dir.display(), s["run_seq"], s["reason"].as_str().unwrap_or(""));
                    return Ok((json!({"verdict": "writer_not_called", "case_dir": dir, "not_called": pos["not_called"],
                        "message": message, "next": next, "position": pos, "explanation": decision["explanation"]}), 4));
                } else if nonempty("spent") {
                    // A writer with nothing left is the stop (a handoff with nothing to decide was
                    // dropped above).
                    let s = &pos["spent"][0];
                    event(dir, at, "writer_spent", json!({"state": pos["state"], "spent": pos["spent"]}))?;
                    let message = format!("writer `{}` is spent at {}: {} runs since event {} (max_runs {}), last exit {}: {}{}",
                        s["action"].as_str().unwrap_or(""), dir.display(), s["runs"], s["since_seq"], s["max_runs"],
                        s["last_exit"].as_str().unwrap_or("?"), s["last_failure"].as_str().unwrap_or(""), sent_back_text(s));
                    return Ok((json!({"verdict": "writer_spent", "case_dir": dir, "spent": pos["spent"],
                        "message": message, "next": next, "position": pos, "explanation": decision["explanation"]}), 4));
                } else if nonempty("approval_required") {
                    ("awaiting_authority", 3)
                } else {
                    ("stuck", 4)
                };
                let next = if code == 3 { next } else { vec![] };
                event(
                    dir,
                    at,
                    verdict,
                    json!({"state": pos["state"], "next": next}),
                )?;
                return Ok((
                    json!({"verdict": verdict, "case_dir": dir, "next": next, "position": pos,
                    "explanation": decision["explanation"]}),
                    code,
                ));
            }
        },
    };
    let decl = &ir["actions"][&chosen];
    let b = bindings
        .get(&chosen)
        .ok_or(format!("no tool is bound to action `{chosen}`"))?;
    let b = &subst(b, "{principal}", principal.unwrap_or("{principal}"));
    // Arguments: `--args`, else the binding's fixed `args`, else the JSON object its `args_from`
    // argv prints (run like a tool, in `work/`). Checked here only for the top-level `required`
    // keys and `additionalProperties: false`; the full argument dialect is a gap.
    let mut args: Value = serde_json::from_str(args).map_err(|e| format!("--args: {e}"))?;
    if args == json!({}) && b.get("args").is_some() {
        args = b["args"].clone();
    } else if args == json!({}) && b.get("args_from").is_some() {
        let argv: Vec<String> = serde_json::from_value(b["args_from"].clone())
            .map_err(|e| format!("args_from: {e}"))?;
        let case_id = read_json(&dir.join("case.json"))?["id"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let (code, out) = run_tool(
            dir,
            &argv,
            &[("SDC_ACTION", chosen.clone()), ("SDC_CASE", case_id)],
            120,
        )?;
        if code != "0" {
            return Err(format!(
                "args_from of `{chosen}` exited {code}: {}",
                String::from_utf8_lossy(&out).trim()
            ));
        }
        args = serde_json::from_slice(&out).map_err(|e| format!("args_from of `{chosen}`: {e}"))?;
    }
    let schema = decl
        .get("arguments")
        .cloned()
        .unwrap_or(json!({"additionalProperties": false}));
    let keys = args.as_object().ok_or("arguments must be an object")?;
    let missing = schema["required"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|k| k.as_str())
        .find(|k| !keys.contains_key(*k));
    let extra = keys.keys().map(|k| k.as_str()).find(|k| {
        schema["additionalProperties"] == false && schema["properties"].get(*k).is_none()
    });
    if let Some(k) = missing.or(extra) {
        return Err(format!(
            "arguments {args} of `{chosen}` do not fit its schema at `{k}`"
        ));
    }
    let kinds: Vec<String> = match &b["produces"] {
        Value::String(k) => vec![k.clone()],
        Value::Array(a) => a
            .iter()
            .filter_map(|k| k.as_str().map(String::from))
            .collect(),
        _ => vec![],
    };
    for k in &kinds {
        if !decl["may_produce"]
            .as_array()
            .is_some_and(|a| a.iter().any(|p| p["evidence"] == k.as_str()))
        {
            return Err(format!(
                "binding of `{chosen}` produces `{k}`, which the action may not produce"
            ));
        }
    }
    // An agent node (a binding with `llm`) counts against the tree's LLM call cap.
    let is_llm = b.get("llm").is_some();
    if let Some(cap) = limits.max_llm_calls.filter(|_| is_llm) {
        if llm_calls(dir) >= cap {
            event(
                dir,
                at,
                "llm_budget_spent",
                json!({"action": chosen, "max_llm_calls": cap}),
            )?;
            return Ok((
                json!({"verdict": "llm_budget_spent", "case_dir": dir, "action": chosen,
                "position": pos}),
                5,
            ));
        }
    }
    let before = pos["state"].clone();
    let case_before = read_json(&dir.join("case.json"))?;
    let mut recorded: Vec<Value> = Vec::new();
    let code;
    let mut log = Vec::new();
    // What the step leaves for the end: the new snapshot, written after evidence and
    // `action_finished`, so a crash never leaves a moved revision without its step's record.
    let mut snapshot: Option<(Value, Map<String, Value>)> = None;
    let mut not_called: Option<String> = None;
    if b.get("copy").is_some() {
        // TEMPORARY (Loom, boundary.md row 28): copy a record from another case of the tree.
        // Kept as given, except `upstream_revisions` entries for artifacts this protocol does not
        // declare, which Canon would refuse (`undeclared-artifact`).
        event(
            dir,
            at,
            "action_started",
            json!({"action": chosen, "copy": b["copy"], "state_before": before}),
        )?;
        code = match copy_source(dir, b)? {
            Some(mut r) => {
                let mut dropped = Vec::new();
                if let Some(up) = r["upstream_revisions"].as_object_mut() {
                    up.retain(|k, _| {
                        let keep = ir["artifacts"].get(k).is_some();
                        if !keep {
                            dropped.push(k.clone())
                        }
                        keep
                    });
                    if up.is_empty() {
                        r.as_object_mut().unwrap().remove("upstream_revisions");
                    }
                }
                write_json(
                    &dir.join("evidence")
                        .join(format!("{}.json", r["id"].as_str().unwrap())),
                    &r,
                )?;
                log = format!("copied {} (dropped upstream {dropped:?})", r["id"]).into_bytes();
                recorded.push(r);
                "0".to_string()
            }
            None => "1".to_string(),
        };
    } else {
        let argv: Vec<String> = serde_json::from_value(b.get("argv").cloned().unwrap_or(json!([])))
            .map_err(|e| format!("argv: {e}"))?;
        // A writer sent back by `retry_when` gets the failing records and their logs.
        let records = evidence_records(dir)?;
        let current = current_records(dir, &decision, &records)?;
        let finished: Vec<Value> = events(dir)
            .into_iter()
            .filter(|e| e["event"] == "action_finished")
            .collect();
        let feedback: Vec<Value> = retry_records(b, &current).into_iter().map(|r| {
            let by = finished.iter().find(|e| e["evidence"].as_array().is_some_and(|ids| ids.contains(&r["id"])));
            json!({"kind": r["kind"], "result": r["result"], "subject": r["subject"], "action": by.map(|e| e["action"].clone()),
                "log": by.and_then(|e| e["log"].as_str()).map(|l| dir.join("blobs").join(l))})
        }).collect();
        let mut env = vec![
            ("SDC_ACTION", chosen.clone()),
            ("SDC_ARGS", args.to_string()),
            (
                "SDC_CASE",
                case_before["id"].as_str().unwrap_or("").to_string(),
            ),
            (
                "SDC_CASE_DIR",
                fs::canonicalize(dir)
                    .unwrap_or(dir.to_path_buf())
                    .to_string_lossy()
                    .into_owned(),
            ),
        ];
        if is_llm {
            env.push(("SDC_LLM_NODE", b["llm"].to_string()));
        }
        // A tool that exits 0 having done nothing (llm-node.sh: "<node> not called: ...") writes
        // why to this file; the run is then recorded `not_called` and not counted (`useful`).
        let not_called_file = fs::canonicalize(dir).unwrap_or(dir.to_path_buf()).join(".not-called");
        let _ = fs::remove_file(&not_called_file);
        env.push(("SDC_NOT_CALLED", not_called_file.to_string_lossy().into_owned()));
        if !feedback.is_empty() {
            let bytes = json_bytes(&json!(feedback));
            let p = dir.join("blobs").join(sha(&bytes));
            fs::write(&p, &bytes).map_err(|e| e.to_string())?;
            env.push(("SDC_FEEDBACK", p.to_string_lossy().into_owned()));
        }
        let timeout = b["timeout_s"].as_u64().unwrap_or(1800);
        event(
            dir,
            at,
            "action_started",
            json!({"action": chosen, "argv": argv, "args": args, "llm": is_llm,
            "timeout_s": timeout, "retry_of": feedback.iter().map(|f| f["kind"].clone()).collect::<Vec<_>>(),
            "state_before": before}),
        )?;
        code = if argv.is_empty() {
            "0".to_string() // the driver's own action, such as opening a child case
        } else {
            let (code, out) = run_tool(dir, &argv, &env, timeout)?;
            log = out;
            code
        };
        if let Ok(why) = fs::read_to_string(&not_called_file) {
            let _ = fs::remove_file(&not_called_file);
            not_called = (code == "0").then_some(why.trim().to_string());
        }
        // A tool cannot name a revision: what it left in `work/` is digested here.
        let (case, moved) = pending(dir)?;
        let forced_result = forced.filter(|_| code == "0").map(|(r, l)| {
            log = l.to_vec();
            json!(r)
        });
        let result = forced_result
            .as_ref()
            .or_else(|| b["results"].get(&code).or_else(|| b["results"].get("*")));
        if program_error(&json!(code)) {
            // A check that could not run is not an `invalid` result: no record (stop below).
        } else if !moved.is_empty() && decl["effect"] != "write" {
            // An observation that changed an artifact is not evidence about it.
            event(
                dir,
                at,
                "discarded",
                json!({"action": chosen, "reason": "an observing action changed an artifact"}),
            )?;
        } else if let Some(result) = result.filter(|_| !kinds.is_empty()) {
            let subject = b["subject"].as_str().ok_or("binding names no subject")?;
            let seq = events(dir).len();
            for k in &kinds {
                let r = if kinds.len() == 1 && !result.is_object() {
                    result.clone()
                } else {
                    result[k.as_str()].clone()
                };
                let id = if kinds.len() == 1 {
                    format!("e{seq:04}-{chosen}")
                } else {
                    format!("e{seq:04}-{chosen}.{k}")
                };
                let mut rec = json!({"format": "canon-evidence/1", "id": id, "kind": k, "subject": subject,
                    "subject_revision": case["artifacts"][subject]["revision"], "observed_at": at,
                    "producer": b["producer"]});
                if !r.is_null() {
                    rec["result"] = r;
                }
                if let Some(up) = b["upstream"].as_array() {
                    rec["upstream_revisions"] = up
                        .iter()
                        .filter_map(|a| a.as_str())
                        .map(|a| (a.to_string(), case["artifacts"][a]["revision"].clone()))
                        .collect::<Map<_, _>>()
                        .into();
                }
                write_json(&dir.join("evidence").join(format!("{id}.json")), &rec)?;
                recorded.push(rec);
            }
        }
        if code == "0" && decl.get("case_open").is_some() {
            let opened = open_children(canon, dir, at, &chosen, decl, b, &args)?;
            log.extend(format!("opened {opened:?}").into_bytes());
        }
        snapshot = Some((case, moved));
    }
    let blob = sha(&log);
    fs::write(dir.join("blobs").join(&blob), &log).map_err(|e| e.to_string())?;
    let case_after = snapshot
        .as_ref()
        .map_or_else(|| read_json(&dir.join("case.json")), |(c, _)| Ok(c.clone()))?;
    let moved = snapshot
        .as_ref()
        .map(|(_, m)| m.clone())
        .unwrap_or_default();
    let ids: Vec<&Value> = recorded.iter().map(|r| &r["id"]).collect();
    let mut finished = json!({"action": chosen, "exit": code, "log": blob,
        "evidence": ids, "state_before": before, "case": sha(&json_bytes(&case_after)), "revises": moved,
        "principal": principal});
    if let Some((r, _)) = forced {
        finished["result"] = json!(r);
    }
    if let Some(why) = not_called {
        finished["not_called"] = json!(why);
    }
    event(dir, at, "action_finished", finished)?;
    commit(dir, at, &case_after, &moved)?;
    if program_error(&json!(code)) {
        let tail: String = String::from_utf8_lossy(&log)
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .to_string();
        return Err(format!(
            "`{chosen}` could not run (exit {code}, a program error, no evidence recorded): {tail}"
        ));
    }
    let after = evaluate(canon, dir, at)?;
    Ok((
        json!({"took": chosen, "exit": code, "evidence": recorded, "position": position(dir, &after)?}),
        0,
    ))
}

/// The fixed policy over a case tree: drive every open child first; a child that is terminable
/// with exactly one legitimate outcome is closed through it, any other verdict of a child stops
/// the run with that child's case directory; then take one step in this case, and repeat.
fn drive(
    canon: &str,
    dir: &Path,
    at: &str,
    budget: &mut u32,
    took: &mut Vec<Value>,
    limits: &Limits,
) -> R<(Value, u8)> {
    'outer: loop {
        for c in children(dir) {
            if read_json(&c.join("case.json"))?
                .get("termination")
                .is_some()
            {
                continue;
            }
            let (v, code) = drive(canon, &c, at, budget, took, limits)?;
            let legit = &v["position"]["legitimate_outcomes"];
            if v["verdict"] == "terminable" && legit.as_array().is_some_and(|l| l.len() == 1) {
                let closed = close(canon, &c, at, legit[0].as_str().unwrap(), None)?;
                took.push(json!({"closed": closed["case"], "outcome": closed["terminated"]}));
                continue 'outer;
            }
            return Ok((v, code));
        }
        if *budget == 0 {
            event(dir, at, "step_budget_spent", json!({}))?;
            return Ok((json!({"verdict": "step_budget_spent", "case_dir": dir}), 5));
        }
        // Wall time is read only to stop; it never reaches Canon or a record.
        if limits
            .deadline
            .is_some_and(|d| std::time::Instant::now() >= d)
        {
            event(dir, at, "wall_time_spent", json!({}))?;
            return Ok((json!({"verdict": "wall_time_spent", "case_dir": dir}), 5));
        }
        *budget -= 1;
        let (v, code) = step(canon, dir, at, None, "{}", None, None, limits)?;
        if v.get("verdict").is_some() {
            return Ok((v, code));
        }
        took.push(json!({"case": v["position"]["case"], "took": v["took"], "exit": v["exit"],
            "evidence": v["evidence"].as_array().map(|a| a.iter().map(|r| json!([r["kind"], r["result"]])).collect::<Vec<_>>())}));
    }
}

fn run(cli: &Cli) -> R<(Value, u8)> {
    let dir = &cli.case_dir;
    let canon = cli.canon.as_str();
    match &cli.cmd {
        Cmd::Init {
            protocol,
            id,
            bindings,
            artifacts,
            independent,
        } => {
            let mut revs = Map::new();
            for a in artifacts {
                let (k, v) = a
                    .split_once('=')
                    .ok_or(format!("--artifact {a}: expected name=revision"))?;
                revs.insert(k.into(), json!(v));
            }
            let mut ind = Map::new();
            for i in independent {
                let (p, dims) = i
                    .split_once('=')
                    .ok_or(format!("--independent {i}: expected producer=dim,dim"))?;
                ind.insert(p.into(), json!(dims.split(',').collect::<Vec<_>>()));
            }
            Ok((
                init_case(canon, dir, protocol, id, bindings, &revs, &ind, false)?,
                0,
            ))
        }
        Cmd::Eval { at } => Ok((position(dir, &evaluate(canon, dir, at)?)?, 0)),
        Cmd::Tools { at } => {
            let d = evaluate(canon, dir, at)?;
            let ir = read_json(&dir.join("ir.json"))?;
            let tools: Vec<Value> = with_status(&d["actions"], "admissible")
                .into_iter()
                .map(|a| {
                    json!({
                "name": a, "description": ir["actions"][a]["description"],
                "input_schema": ir["actions"][a].get("arguments").cloned()
                    .unwrap_or(json!({"type": "object", "additionalProperties": false}))})
                })
                .collect();
            Ok((json!(tools), 0))
        }
        Cmd::Step { at, action, args } => step(
            canon,
            dir,
            at,
            action.as_deref(),
            args,
            None,
            None,
            &NO_LIMITS,
        ),
        Cmd::Run {
            at,
            max_steps,
            max_llm_calls,
            max_wall_seconds,
        } => {
            let mut took = Vec::new();
            let mut budget = *max_steps;
            let limits = Limits {
                max_llm_calls: *max_llm_calls,
                deadline: max_wall_seconds
                    .map(|s| std::time::Instant::now() + std::time::Duration::from_secs(s)),
            };
            let (v, code) = drive(canon, dir, at, &mut budget, &mut took, &limits)?;
            let message = match v["verdict"].as_str() {
                Some("awaiting_person" | "awaiting_authority") => {
                    let mut m = format!(
                        "waiting on a person at {}:",
                        v["case_dir"].as_str().unwrap_or("?")
                    );
                    for n in v["next"].as_array().into_iter().flatten() {
                        m += &format!(
                            "\n  {} ({}):\n    {}",
                            n["action"].as_str().unwrap_or(""),
                            n["decides"].as_str().unwrap_or(""),
                            n["command"].as_str().unwrap_or("")
                        );
                    }
                    m
                }
                Some(w @ ("writer_spent" | "writer_not_called")) => v["message"].as_str().unwrap_or(w).to_string(),
                Some(v) => v.to_string(),
                None => String::new(),
            };
            Ok((
                json!({"took": took, "verdict": v["verdict"], "case_dir": v["case_dir"], "message": message,
                "next": v.get("next").cloned().unwrap_or(json!([])), "llm_calls": llm_calls(dir),
                "position": v["position"]}),
                code,
            ))
        }
        Cmd::Approve {
            at,
            capability,
            principal,
            deny,
            action,
            result,
            reasons,
        } => {
            // A set result is checked before anything is written: it must be one the action's
            // binding can record, and the reasons must be readable.
            let forced = match (result, reasons, action) {
                (Some(r), Some(f), Some(a)) => {
                    let b = &read_json(&dir.join("bindings.json"))?[a.as_str()];
                    let known = b["results"]
                        .as_object()
                        .is_some_and(|m| m.values().any(|v| v == r.as_str()));
                    if b["manual"] != true || !known {
                        return Err(format!(
                            "`{a}` cannot record result `{r}`: its binding is not a manual approval whose `results` name it"
                        ));
                    }
                    Some((
                        r.clone(),
                        fs::read(f).map_err(|e| format!("{}: {e}", f.display()))?,
                    ))
                }
                _ => None,
            };
            let p = dir.join("authority.json");
            let mut list = read_json(&p)?;
            let decision = if *deny { "denied" } else { "granted" };
            let prior = list
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["capability"] == capability.as_str())
                .cloned();
            match prior {
                // Canon refuses a capability decided twice; a grant stays until the case ends.
                Some(e) if e["decision"] != decision => {
                    return Err(format!(
                        "capability `{capability}` is already {}",
                        e["decision"]
                    ))
                }
                Some(_) => {}
                None => {
                    list.as_array_mut()
                        .unwrap()
                        .push(json!({"capability": capability, "decision": decision}));
                    write_json(&p, &list)?;
                    // canon-authority/1 has no principal; the event log keeps who decided.
                    event(
                        dir,
                        at,
                        "authority_recorded",
                        json!({"capability": capability, "decision": decision,
                        "principal": principal}),
                    )?;
                }
            }
            match action {
                Some(a) => step(
                    canon,
                    dir,
                    at,
                    Some(a),
                    "{}",
                    Some(principal),
                    forced.as_ref().map(|(r, l)| (r.as_str(), l.as_slice())),
                    &NO_LIMITS,
                ),
                None => Ok((position(dir, &evaluate(canon, dir, at)?)?, 0)),
            }
        }
        Cmd::Decide {
            at,
            decision,
            outcome,
            principal,
            after_denial,
            reasons,
        } => {
            let ir = read_json(&dir.join("ir.json"))?;
            if ir["outcomes"][outcome.as_str()]["requires"] != json!({ "decision": decision }) {
                return Err(format!(
                    "outcome `{outcome}` does not require the decision `{decision}` alone"
                ));
            }
            let p = dir.join("decisions.json");
            let mut list = read_json(&p)?;
            if list
                .as_array()
                .into_iter()
                .flatten()
                .any(|e| e["decision"] == decision.as_str())
            {
                return Err(format!("`{decision}` is already recorded in this case"));
            }
            let last = events(dir).into_iter().rfind(|e| e["event"] != "evaluated");
            let denied = last.as_ref().is_some_and(|e| {
                e["event"] == "authority_recorded"
                    && e["capability"] == after_denial.as_str()
                    && e["decision"] == "denied"
            });
            if !denied {
                return Err(format!(
                    "`{decision}` is taken only directly after `{after_denial}` was denied in this case; its last step is {}",
                    last.map(|e| match e["action"].as_str() {
                        Some(a) => format!("{} {a}", e["event"].as_str().unwrap_or("?")),
                        None => e["event"].as_str().unwrap_or("?").to_string(),
                    })
                    .unwrap_or_else(|| "none".into())
                ));
            }
            let log = fs::read(reasons).map_err(|e| format!("{}: {e}", reasons.display()))?;
            let blob = sha(&log);
            fs::write(dir.join("blobs").join(&blob), &log).map_err(|e| e.to_string())?;
            let revision = read_json(&dir.join("case.json"))?["revision"].clone();
            let entry = json!({"decision": decision, "outcome": outcome, "principal": principal,
                "case_revision": revision});
            list.as_array_mut()
                .ok_or("decisions.json is not a list")?
                .push(entry.clone());
            write_json(&p, &list)?;
            let mut body = entry;
            body["log"] = json!(blob);
            event(dir, at, "decision_recorded", body)?;
            Ok((position(dir, &evaluate(canon, dir, at)?)?, 0))
        }
        Cmd::Close { at, outcome, principal } => Ok((close(canon, dir, at, outcome, principal.as_deref())?, 0)),
        Cmd::Allow { at, action, principal } => {
            let ir = read_json(&dir.join("ir.json"))?;
            let bindings = read_json(&dir.join("bindings.json"))?;
            if bindings[action.as_str()].is_null() || ir["actions"][action.as_str()]["effect"] != "write" {
                return Err(format!("`{action}` is no bound writer of this case"));
            }
            event(dir, at, "allowance_granted", json!({"action": action, "principal": principal}))?;
            Ok((position(dir, &evaluate(canon, dir, at)?)?, 0))
        }
        Cmd::Replay => {
            let mut report = Vec::new();
            let mut ok = true;
            for c in all_cases(dir) {
                // A child opened but not yet driven has no decision to compare: listed, not failed.
                let Some(last) = events(&c).into_iter().rfind(|e| e["event"] == "evaluated") else {
                    report.push(
                        json!({"case": read_json(&c.join("case.json"))?["id"], "evaluated": false}),
                    );
                    continue;
                };
                let out = canon_evaluate(canon, &c, last["at"].as_str().unwrap())?;
                let state = state_digest(&c)?;
                let (sm, dm) = (
                    last["state"] == state.as_str(),
                    last["decision"] == sha(&out).as_str(),
                );
                ok &= sm && dm;
                report.push(json!({"case": read_json(&c.join("case.json"))?["id"], "at": last["at"],
                    "state": state, "state_matches_log": sm, "decision": sha(&out), "decision_matches_log": dm}));
            }
            Ok((json!(report), if ok { 0 } else { 1 }))
        }
    }
}

const NO_LIMITS: Limits = Limits {
    max_llm_calls: None,
    deadline: None,
};

fn main() -> ExitCode {
    let cli = Cli::parse();
    // One writer per case tree; `init` creates the tree, `replay` only reads it.
    let _lock = match &cli.cmd {
        Cmd::Init { .. } | Cmd::Replay => None,
        _ => match lock(&cli.case_dir) {
            Ok(l) => Some(l),
            Err(e) => {
                eprintln!("sdc-drive: {e}");
                return ExitCode::from(2);
            }
        },
    };
    match run(&cli) {
        Ok((v, code)) => {
            use std::io::IsTerminal;
            if let Some(m) = v["message"]
                .as_str()
                .filter(|m| !m.is_empty() && std::io::stderr().is_terminal())
            {
                eprintln!("{m}");
            }
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
            ExitCode::from(code)
        }
        Err(e) => {
            eprintln!("sdc-drive: {e}");
            ExitCode::from(2)
        }
    }
}
