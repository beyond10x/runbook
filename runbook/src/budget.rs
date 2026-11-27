//! The budget of one call: before each step the bindings copy is limited so that no call may
//! spend more than the ledger granted. Only the `--max-budget-usd` argv value of a model tool is
//! touched: in a tree only `max_runs` acts, and Loom refuses `max_budget_micro_usd` (and
//! `max_turns`, `max_tokens`) at load on a non-agent binding, so this module never adds or keeps
//! such a member.
//!
//! A binding states the per-call budget of a program without its argv too: the member
//! `max_budget_usd: <number>` of a program action (an action with `executor.argv` or
//! `arguments_from`), and the same member at the top of a bindings file for every program action
//! of that file that names none itself (not for a child file an `open` action names). It is never
//! passed to the program: Loom refuses a member it does not know, so a run's copy of the bindings
//! holds none ([`seal`]) and the file [`SEALED_FILE`] beside the copy keeps what was taken out.
//! The order: `--max-budget-usd` in the argv, then the action's member, then the file's member,
//! then the largest budget the runbook names.

use crate::record_write::Held;
use serde_json::json;
use serde_yaml::Value;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

const ARG: &str = "--max-budget-usd";
/// The member of a program action, and of a bindings file, that names the per-call budget of its
/// programs in USD (`0`: the program calls no model).
pub const MEMBER: &str = "max_budget_usd";
/// The file in a run's copy of the bindings directory that keeps the members [`seal`] took out of
/// the copy (micro USD), read back by every reader of this module.
pub const SEALED_FILE: &str = ".call-budgets.json";
const SEALED_FORMAT: &str = "runbook.call-budgets/1";

/// The bindings directory `dir`, held ([`Held::open`]: a link at its name is an error), and the names of its `*.yaml` and
/// `*.yml` files, listed through the descriptor.
fn yaml_files(dir: &Path) -> Result<(Held, Vec<OsString>), String> {
    let held = Held::open(dir)?;
    let files = held.names()?.into_iter().filter(|n| Path::new(n).extension().is_some_and(|x| x == "yaml" || x == "yml") && held.is_file(n)).collect();
    Ok((held, files))
}

fn usd_text_to_micro(s: &str) -> Option<u64> {
    let x: f64 = s.trim().parse().ok()?;
    (x.is_finite() && x >= 0.0).then(|| (x * 1_000_000.0).round() as u64)
}

fn micro_to_usd_text(micro: u64) -> String {
    let s = format!("{}.{:06}", micro / 1_000_000, micro % 1_000_000);
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn scalar_usd(v: &Value) -> Option<u64> {
    match v {
        Value::String(s) => usd_text_to_micro(s),
        Value::Number(_) => member_usd(v),
        _ => None,
    }
}

/// The budget a member `max_budget_usd` names: a YAML number, finite and 0 or more.
fn member_usd(v: &Value) -> Option<u64> {
    match v {
        Value::Number(n) => n.as_f64().and_then(|x| (x.is_finite() && x >= 0.0).then(|| (x * 1_000_000.0).round() as u64)),
        _ => None,
    }
}

fn shown(v: &Value) -> String {
    serde_yaml::to_string(v).map_or_else(|_| "a value".to_owned(), |t| t.trim().to_owned())
}

/// The value `--max-budget-usd=<v>` names, when `text` is written so.
fn joined_value(text: &str) -> Option<&str> {
    text.strip_prefix(ARG)?.strip_prefix('=')
}

/// The budget the item at `i` of a sequence names: `--max-budget-usd <v>` (the next item) or
/// `--max-budget-usd=<v>`. `None` when the item names none; `Some(None)` when it names one that is
/// no number of USD of 0 or more.
fn budget_at(items: &[Value], i: usize) -> Option<Option<u64>> {
    let text = items[i].as_str()?;
    if text == ARG {
        return Some(items.get(i + 1).and_then(scalar_usd));
    }
    joined_value(text).map(usd_text_to_micro)
}

fn walk_largest(v: &Value, best: &mut Option<u64>) {
    match v {
        Value::Sequence(items) => {
            for (i, item) in items.iter().enumerate() {
                if let Some(Some(m)) = budget_at(items, i) {
                    *best = Some(best.map_or(m, |b| b.max(m)));
                }
                walk_largest(item, best);
            }
        }
        Value::Mapping(map) => {
            for item in map.values() {
                walk_largest(item, best);
            }
        }
        _ => {}
    }
}

/// The largest budget a bindings document names: in an argv, as the file's member or as an
/// action's member.
fn document_largest(doc: &Value, best: &mut Option<u64>) {
    walk_largest(doc, best);
    let members = doc.get(MEMBER).into_iter().chain(actions(doc).iter().filter_map(|a| a.get(MEMBER)));
    for m in members.filter_map(member_usd) {
        *best = Some(best.map_or(m, |b| b.max(m)));
    }
}

fn walk_limit(v: &mut Value, limit: u64) -> bool {
    let mut changed = false;
    match v {
        Value::Sequence(items) => {
            for i in 0..items.len() {
                if let Some(Some(m)) = budget_at(items, i) {
                    if m > limit {
                        if items[i].as_str() == Some(ARG) {
                            items[i + 1] = Value::String(micro_to_usd_text(limit));
                        } else {
                            items[i] = Value::String(format!("{ARG}={}", micro_to_usd_text(limit)));
                        }
                        changed = true;
                    }
                }
                changed |= walk_limit(&mut items[i], limit);
            }
        }
        Value::Mapping(map) => {
            for item in map.values_mut() {
                changed |= walk_limit(item, limit);
            }
        }
        _ => {}
    }
    changed
}

/// The per-call budget a program's arguments name (`--max-budget-usd <v>` or
/// `--max-budget-usd=<v>`), micro USD: the largest when they name it more than once; `None` when they
/// name none (a value that is no number of USD of 0 or more names none; the bindings of the daemon's
/// runs are refused at load for one, `runbook_budgets`).
pub fn call_budget_of_args(args: &[String]) -> Option<u64> {
    let mut best = None;
    for (i, a) in args.iter().enumerate() {
        let named = if a == ARG { args.get(i + 1).and_then(|v| usd_text_to_micro(v)) } else { joined_value(a).and_then(usd_text_to_micro) };
        if let Some(m) = named {
            best = Some(best.map_or(m, |b: u64| b.max(m)));
        }
    }
    best
}

/// One program of a binding: its argv (`executor.argv` or `arguments_from`) and the budget its
/// binding names beside the argv, the action's member or else its file's (micro USD).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramBudget {
    pub argv: Vec<String>,
    pub micro_usd: Option<u64>,
}

/// What the bindings a daemon run reads say about per-call budgets: the root's file, each child
/// file an `open` action names (`executor.bindings`, beside the file that names it) and theirs.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RunbookBudgets {
    /// The largest per-call budget any of them names (an argv value or a member), micro USD.
    pub largest: Option<u64>,
    /// The first action with a program (`executor.argv`, or `arguments_from`) whose budget no source
    /// names: no argv value, no member of the action, no member of its file.
    pub unnamed: Option<String>,
    /// Every program of them, in the order read.
    pub programs: Vec<ProgramBudget>,
}

fn actions(doc: &Value) -> &[Value] {
    doc.get("actions").and_then(Value::as_sequence).map_or(&[], Vec::as_slice)
}

fn action_name(a: &Value) -> &str {
    a.get("action").and_then(Value::as_str).unwrap_or("(unnamed)")
}

/// The argvs of the programs an action runs: `executor.argv` and `arguments_from`.
fn programs(a: &Value) -> Vec<&[Value]> {
    [a.get("executor").and_then(|e| e.get("argv")), a.get("arguments_from")].into_iter().flatten().filter_map(Value::as_sequence).map(Vec::as_slice).collect()
}

fn names_budget(items: &[Value]) -> bool {
    (0..items.len()).any(|i| budget_at(items, i).is_some())
}

/// The largest budget an argv names, micro USD.
fn argv_budget(items: &[Value]) -> Option<u64> {
    (0..items.len()).filter_map(|i| budget_at(items, i).flatten()).max()
}

/// Every budget an action names is a number of USD of 0 or more, or the action is refused by name.
fn check_budgets(v: &Value, action: &str, file: &str) -> Result<(), String> {
    match v {
        Value::Sequence(items) => {
            for (i, item) in items.iter().enumerate() {
                if let Some(None) = budget_at(items, i) {
                    let given = match item.as_str().and_then(joined_value) {
                        Some(text) => format!("{text:?}"),
                        None => items.get(i + 1).map_or_else(|| "nothing".to_owned(), shown),
                    };
                    return Err(format!("the action {action} of {file} names {ARG} {given}: a per-call budget is a number of USD of 0 or more"));
                }
                check_budgets(item, action, file)?;
            }
            Ok(())
        }
        Value::Mapping(map) => map.values().try_for_each(|item| check_budgets(item, action, file)),
        _ => Ok(()),
    }
}

/// The members `max_budget_usd` of a bindings document named `file`: each is a number of USD of 0 or
/// more (the file's is refused naming the file, an action's naming the action), an action's stands
/// on an action that runs a program, and it names the budget an argv of that action names, if one
/// does.
pub fn check_members(doc: &Value, file: &str) -> Result<(), String> {
    if let Some(v) = doc.get(MEMBER) {
        if member_usd(v).is_none() {
            return Err(format!("the bindings file {file} names {MEMBER} {}: a per-call budget is a number of USD of 0 or more", shown(v)));
        }
    }
    for a in actions(doc) {
        let Some(v) = a.get(MEMBER) else { continue };
        let action = action_name(a);
        let Some(member) = member_usd(v) else {
            return Err(format!("the action {action} of {file} names {MEMBER} {}: a per-call budget is a number of USD of 0 or more", shown(v)));
        };
        let programs = programs(a);
        if programs.is_empty() {
            return Err(format!("the action {action} of {file} names {MEMBER} and runs no program (no executor.argv, no arguments_from): the member is the per-call budget of a program"));
        }
        for named in programs.into_iter().filter_map(argv_budget) {
            if named != member {
                return Err(format!(
                    "the action {action} of {file} names {ARG} {} in its arguments and {MEMBER} {}: the two disagree; name one budget",
                    micro_to_usd_text(named),
                    micro_to_usd_text(member)
                ));
            }
        }
    }
    Ok(())
}

/// The members [`seal`] took out of the files of a copy, by the file's name relative to the copy's
/// directory: the file's own and each action's, by its index (micro USD).
#[derive(Debug, Default)]
struct Sealed {
    files: BTreeMap<String, SealedFile>,
}

#[derive(Debug, Default)]
struct SealedFile {
    file: Option<u64>,
    actions: BTreeMap<usize, u64>,
}

fn usd_number(micro: u64) -> Value {
    Value::Number(serde_yaml::Number::from(micro as f64 / 1_000_000.0))
}

impl Sealed {
    /// The members kept beside the copy in `dir`; none when the file is not there. A file that
    /// cannot be read is an error: the budgets of the run are not known.
    fn load(dir: &Path) -> Result<Sealed, String> {
        let path = dir.join(SEALED_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Sealed::default()),
            Err(e) => return Err(format!("{SEALED_FILE}: {e}")),
        };
        Sealed::parse(&text)
    }

    /// [`Sealed::load`] through the held directory of a run's copy: the file is there when its name is
    /// listed, and it is read as every name of the run is ([`Held::read_text`]): a link, a pipe or a
    /// file with a second name at its name is an error that names [`SEALED_FILE`] and no path.
    fn load_held(held: &Held) -> Result<Sealed, String> {
        if !held.names()?.iter().any(|n| n == SEALED_FILE) {
            return Ok(Sealed::default());
        }
        let text = held.read_text(SEALED_FILE).map_err(|e| inside(e, held.path()))?;
        Sealed::parse(&text)
    }

    fn parse(text: &str) -> Result<Sealed, String> {
        let bad = || format!("{SEALED_FILE}: not a {SEALED_FORMAT} document");
        let doc: serde_json::Value = serde_json::from_str(text).map_err(|_| bad())?;
        if doc.get("format").and_then(serde_json::Value::as_str) != Some(SEALED_FORMAT) {
            return Err(bad());
        }
        let mut out = Sealed::default();
        for (name, entry) in doc.get("files").and_then(serde_json::Value::as_object).ok_or_else(bad)? {
            let file = match entry.get("file") {
                None | Some(serde_json::Value::Null) => None,
                Some(v) => Some(v.as_u64().ok_or_else(bad)?),
            };
            let mut actions = BTreeMap::new();
            for (index, v) in entry.get("actions").and_then(serde_json::Value::as_object).ok_or_else(bad)? {
                actions.insert(index.parse::<usize>().map_err(|_| bad())?, v.as_u64().ok_or_else(bad)?);
            }
            out.files.insert(name.clone(), SealedFile { file, actions });
        }
        Ok(out)
    }

    /// `doc`, the file `name` of the copy, with the members taken out of it put back.
    fn restore(&self, name: &str, mut doc: Value) -> Value {
        let Some(entry) = self.files.get(name) else { return doc };
        if let (Some(m), Some(map)) = (entry.file, doc.as_mapping_mut()) {
            map.insert(Value::String(MEMBER.to_owned()), usd_number(m));
        }
        if let Some(actions) = doc.get_mut("actions").and_then(Value::as_sequence_mut) {
            for (i, m) in &entry.actions {
                if let Some(map) = actions.get_mut(*i).and_then(Value::as_mapping_mut) {
                    map.insert(Value::String(MEMBER.to_owned()), usd_number(*m));
                }
            }
        }
        doc
    }
}

/// How a walk reads the bindings files and the kept file.
#[derive(Clone, Copy)]
enum Reading {
    /// By path: the bindings a daemon run reads at its start (the runbook home, or the run's pinned
    /// copy, which lies outside the workspace and is checked against its digest).
    Path,
    /// Through the held directory of each file ([`Held::open`] of its directory, [`Held::read_text`]):
    /// a run's copy, which [`seal`] writes.
    Held,
}

/// An error of a held read or write with the path of `base` left out: it names the file below it.
fn inside(e: String, base: &Path) -> String {
    e.replace(&format!("{}/", base.display()), "")
}

fn read_yaml(file: &Path, how: Reading) -> Option<Value> {
    let text = match how {
        Reading::Path => std::fs::read_to_string(file).ok()?,
        Reading::Held => Held::open(file.parent()?).ok()?.read_text(file.file_name()?).ok()?,
    };
    serde_yaml::from_str::<Value>(&text).ok()
}

fn rel_name(file: &Path, base: &Path) -> String {
    file.strip_prefix(base).unwrap_or(file).display().to_string()
}

/// Visits the bindings files a run reads from `root`, each once: the root's and each child file an
/// `open` action names, beside the file that names it, with the members of the copy put back. A
/// file that cannot be read or parsed is skipped: Loom refuses it when the run opens it. `how` says
/// how the files and the kept file are read; a kept file that cannot be read is an error.
fn walk(root: &Path, how: Reading, mut visit: impl FnMut(&Path, &str, &Value) -> Result<(), String>) -> Result<(), String> {
    let base = root.parent().unwrap_or(Path::new("."));
    let sealed = match how {
        Reading::Path => Sealed::load(base)?,
        Reading::Held => Sealed::load_held(&Held::open(base)?)?,
    };
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut queue = vec![root.to_path_buf()];
    while let Some(file) = queue.pop() {
        if seen.contains(&file) {
            continue;
        }
        seen.push(file.clone());
        let Some(doc) = read_yaml(&file, how) else { continue };
        let name = rel_name(&file, base);
        let doc = sealed.restore(&name, doc);
        visit(&file, &name, &doc)?;
        for a in actions(&doc) {
            let executor = a.get("executor");
            if executor.and_then(|e| e.get("kind")).and_then(Value::as_str) == Some("open") {
                if let Some(child) = executor.and_then(|e| e.get("bindings")).and_then(Value::as_str) {
                    let rel = Path::new(child);
                    if !rel.is_absolute() && !rel.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                        queue.push(file.parent().unwrap_or(base).join(rel));
                    }
                }
            }
        }
    }
    Ok(())
}

/// Reads the budgets of the bindings `root` names, as `RunbookBudgets` says. A budget that is no
/// number of USD of 0 or more is refused naming its action and file (`bindings/<file>`, no path), and
/// so is a member that [`check_members`] refuses. A file that cannot be read or parsed adds nothing:
/// Loom refuses it when the run opens it.
pub fn runbook_budgets(root: &Path) -> Result<RunbookBudgets, String> {
    let mut out = RunbookBudgets::default();
    walk(root, Reading::Path, |_, rel, doc| {
        let name = format!("bindings/{rel}");
        check_members(doc, &name)?;
        document_largest(doc, &mut out.largest);
        let file_member = doc.get(MEMBER).and_then(member_usd);
        for a in actions(doc) {
            let action = action_name(a);
            check_budgets(a, action, &name)?;
            let member = a.get(MEMBER).and_then(member_usd).or(file_member);
            for items in programs(a) {
                if !names_budget(items) && member.is_none() && out.unnamed.is_none() {
                    out.unnamed = Some(action.to_owned());
                }
                out.programs.push(ProgramBudget { argv: items.iter().filter_map(|w| w.as_str().map(str::to_owned)).collect(), micro_usd: member });
            }
        }
        Ok(())
    })?;
    Ok(out)
}

/// Whether a program of the bindings `root` names (the root's and each child file's) calls a model, which gives a daemon
/// run a model step (`docs/model-routes.md`, section 2): an agent node (`executor.argv` runs `sdc-node`,
/// [`crate::delegation::AGENT_NODE`]), or a program whose per-call budget, read as [`Reserves::of`] reads it, is above 0:
/// the largest its arguments name, else the member of its action, else of its file, else the largest of the runbook. A
/// budget of 0 is a program that calls no model ([`MEMBER`]). Bindings a run is refused for at load (a refused budget, a
/// program no budget covers, [`reserves`]) count as calling one.
pub fn calls_a_model(root: &Path) -> bool {
    let Ok(Reserves { unnamed_micro_usd, .. }) = reserves(root) else { return true };
    let mut calls = false;
    let walked = walk(root, Reading::Path, |_, _, doc| {
        let file_member = doc.get(MEMBER).and_then(member_usd);
        for a in actions(doc) {
            let first = a.get("executor").and_then(|e| e.get("argv")).and_then(Value::as_sequence).and_then(|argv| argv.first()).and_then(Value::as_str);
            let agent_node = first.is_some_and(|f| Path::new(f).file_name().is_some_and(|name| name == crate::delegation::AGENT_NODE[0]));
            let member = a.get(MEMBER).and_then(member_usd).or(file_member);
            for items in programs(a) {
                let budget = if names_budget(items) { argv_budget(items) } else { Some(member.unwrap_or(unnamed_micro_usd)) };
                calls |= agent_node || budget.is_none_or(|m| m > 0);
            }
        }
        Ok(())
    });
    walked.is_err() || calls
}

/// The program of an action as its binding names it, for the end of a run whose step failed: whether it is an agent
/// node (`executor.argv` runs `sdc-node`, [`crate::delegation::AGENT_NODE`]) and its `timeout_seconds`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionProgram {
    pub agent_node: bool,
    pub timeout_seconds: Option<i64>,
}

/// The programs of the actions of the bindings `root` names, the root's and each child file's, by action; the first
/// binding of an action wins. A file that cannot be read adds nothing.
pub fn action_programs(root: &Path) -> BTreeMap<String, ActionProgram> {
    let mut out = BTreeMap::new();
    let _ = walk(root, Reading::Path, |_, _, doc| {
        for a in actions(doc) {
            let first = a.get("executor").and_then(|e| e.get("argv")).and_then(Value::as_sequence).and_then(|argv| argv.first()).and_then(Value::as_str);
            let Some(first) = first else { continue };
            let agent_node = Path::new(first).file_name().is_some_and(|name| name == crate::delegation::AGENT_NODE[0]);
            let timeout_seconds = a.get("timeout_seconds").and_then(Value::as_i64);
            out.entry(action_name(a).to_owned()).or_insert(ActionProgram { agent_node, timeout_seconds });
        }
        Ok(())
    });
    out
}

/// What each program of a daemon run reserves as it starts.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Reserves {
    /// What a program reserves whose budget no source names: the largest budget of the runbook.
    pub unnamed_micro_usd: u64,
    /// The programs of the bindings the run reads, with the budget each binding names beside the argv.
    pub programs: Vec<ProgramBudget>,
}

/// Whether the child `program` with `args` runs `argv`: the program is argv's first word (as
/// written, or resolved below the bindings) and the arguments are the rest.
fn runs(program: &str, args: &[String], argv: &[String]) -> bool {
    let Some((first, rest)) = argv.split_first() else { return false };
    (program == first || Path::new(program).ends_with(first)) && args == rest
}

impl Reserves {
    /// What the program `program` started with `args` reserves: the budget its arguments name; else
    /// the budget the binding of that argv names beside it (the largest when two bindings run the same
    /// argv); else the largest budget of the runbook.
    pub fn of(&self, program: &str, args: &[String]) -> u64 {
        if let Some(m) = call_budget_of_args(args) {
            return m;
        }
        self.programs.iter().filter(|p| runs(program, args, &p.argv)).map(|p| p.micro_usd.unwrap_or(self.unnamed_micro_usd)).max().unwrap_or(self.unnamed_micro_usd)
    }
}

/// What the programs of a daemon run reserve (`Reserves`), from the bindings `root` names. A
/// runbook with a program whose budget no source names and no budget in any binding is refused,
/// naming the reason: a run of it would be unbound in money. A runbook with no program reserves 0.
pub fn reserves(root: &Path) -> Result<Reserves, String> {
    let b = runbook_budgets(root)?;
    let unnamed_micro_usd = match (b.largest, b.unnamed) {
        (Some(m), _) => m,
        (None, None) => 0,
        (None, Some(action)) => {
            return Err(format!(
                "the program of the action {action} names no per-call budget ({ARG} in its arguments, {MEMBER} on its action or its file) and no binding of the runbook names one: a run of it could call a model unbound in money; name a budget in the arguments of a program or as {MEMBER} on its binding ({MEMBER}: 0 for a program that calls no model)"
            ))
        }
    };
    Ok(Reserves { unnamed_micro_usd, programs: b.programs })
}

/// What a program of a daemon run that names no per-call budget reserves: the largest budget the
/// runbook's bindings name (`runbook_budgets`), as a program that may call a model; refused as
/// [`reserves`] refuses.
pub fn reserve_for_unnamed(root: &Path) -> Result<u64, String> {
    reserves(root).map(|r| r.unnamed_micro_usd)
}

/// Takes the members `max_budget_usd` out of the bindings files a run's copy reads from `root` (the
/// root's and its children's), after [`check_members`] holds for each, and keeps them in
/// [`SEALED_FILE`] beside the root: Loom reads the copy and refuses a member it does not know. A file
/// with no member is not written, and no file is kept when no member was taken. A [`SEALED_FILE`]
/// the copy holds already counts as the members it names (every reader reads it so), so sealing a
/// sealed copy changes nothing.
///
/// The copy is read and written through the held directory of each file ([`Reading::Held`],
/// [`Held::write`]): a link at a bindings file is not read and nothing is written through it (Loom
/// refuses the member it keeps; a run ends on the link before its first step), and a link, a pipe or a
/// file with a second name at [`SEALED_FILE`] is an error that names it.
pub fn seal(root: &Path) -> Result<(), String> {
    let base = root.parent().unwrap_or(Path::new("."));
    let mut sealed: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    let mut edits: Vec<(PathBuf, Value)> = Vec::new();
    walk(root, Reading::Held, |file, rel, doc| {
        check_members(doc, &format!("bindings/{rel}"))?;
        let take = |doc: &mut Value| {
            let file_member = doc.as_mapping_mut().and_then(|m| m.remove(MEMBER)).as_ref().and_then(member_usd);
            let mut taken = serde_json::Map::new();
            if let Some(actions) = doc.get_mut("actions").and_then(Value::as_sequence_mut) {
                for (i, a) in actions.iter_mut().enumerate() {
                    if let Some(m) = a.as_mapping_mut().and_then(|m| m.remove(MEMBER)).as_ref().and_then(member_usd) {
                        taken.insert(i.to_string(), json!(m));
                    }
                }
            }
            (file_member, taken)
        };
        let (file_member, taken) = take(&mut doc.clone());
        if file_member.is_some() || !taken.is_empty() {
            sealed.insert(rel.to_owned(), json!({"file": file_member, "actions": taken}));
        }
        // The file as it stands, without what the kept file put back: written only when it holds a member.
        if let Some(mut own) = read_yaml(file, Reading::Held) {
            let (own_file, own_taken) = take(&mut own);
            if own_file.is_some() || !own_taken.is_empty() {
                edits.push((file.to_path_buf(), own));
            }
        }
        Ok(())
    })?;
    for (file, doc) in edits {
        let text = serde_yaml::to_string(&doc).map_err(|e| format!("{}: {e}", rel_name(&file, base)))?;
        let (Some(dir), Some(leaf)) = (file.parent(), file.file_name()) else { return Err(format!("{}: not a file name", rel_name(&file, base))) };
        // The file keeps its permission bits, as a write in place keeps them.
        Held::open(dir).and_then(|held| held.write_mode(leaf, text.as_bytes(), held.mode(leaf).unwrap_or(0o644))).map_err(|e| inside(e, base))?;
    }
    let held = Held::open(base).map_err(|e| inside(e, base))?;
    if sealed.is_empty() {
        if held.names()?.iter().any(|n| n == SEALED_FILE) {
            held.remove(SEALED_FILE);
        }
    } else {
        let text = json!({"format": SEALED_FORMAT, "files": sealed}).to_string() + "\n";
        held.write_mode(SEALED_FILE, text.as_bytes(), 0o600).map_err(|e| inside(e, base))?;
    }
    Ok(())
}

/// The largest per-call budget the bindings in `dir` name (`--max-budget-usd` values and members
/// `max_budget_usd`, those [`seal`] kept beside a copy included), micro USD; `None` when they name
/// none.
pub fn largest_call_budget(dir: &Path) -> Result<Option<u64>, String> {
    let mut best = None;
    let (held, files) = yaml_files(dir)?;
    let sealed = Sealed::load_held(&held)?;
    for name in files {
        let text = held.read_text(&name)?;
        let v: Value = serde_yaml::from_str(&text).map_err(|e| format!("{}: {e}", dir.join(&name).display()))?;
        document_largest(&sealed.restore(&Path::new(&name).display().to_string(), v), &mut best);
    }
    Ok(best)
}

/// Lowers every per-call budget in the argv of the bindings copy `dir` to at most `micro_usd`. A
/// member `max_budget_usd` is not lowered: it is reserved as written. The directory is held for the
/// whole call: every file is read and written through its descriptor, never through its path.
pub fn limit_call_budgets(dir: &Path, micro_usd: u64) -> Result<(), String> {
    let (held, files) = yaml_files(dir)?;
    for name in files {
        let file = dir.join(&name);
        let text = held.read_text(&name)?;
        let mut v: Value = serde_yaml::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
        if walk_limit(&mut v, micro_usd) {
            let out = serde_yaml::to_string(&v).map_err(|e| format!("{}: {e}", file.display()))?;
            held.write(&name, out.as_bytes())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::current_exe().unwrap().parent().unwrap().join(format!("loomrun-budget-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const BINDINGS: &str = "format: loom.bindings/1\nactions:\n  - action: a\n    executor:\n      kind: program\n      argv: [sdc-node, a, --max-turns, \"15\", --max-budget-usd, \"2\", --stub, x]\n    budget: {max_runs: 3}\n  - action: b\n    executor:\n      kind: program\n      argv: [sdc-node, b, --max-budget-usd, \"0.5\"]\n";

    #[test]
    fn the_largest_budget_is_read_and_limiting_lowers_only_the_argv_values() {
        let d = dir("limit");
        std::fs::write(d.join("sdc.yaml"), BINDINGS).unwrap();
        assert_eq!(largest_call_budget(&d).unwrap(), Some(2_000_000));
        limit_call_budgets(&d, 300_000).unwrap();
        let text = std::fs::read_to_string(d.join("sdc.yaml")).unwrap();
        assert!(!text.contains("max_budget_micro_usd"), "no member Loom refuses on a program binding: {text}");
        assert!(text.contains("max_runs: 3"), "{text}");
        assert!(!text.contains("'2'") && !text.contains("\"2\"") && !text.contains("'0.5'") && !text.contains("\"0.5\""), "{text}");
        assert_eq!(largest_call_budget(&d).unwrap(), Some(300_000));
    }

    #[test]
    fn limiting_refuses_a_link_at_a_bindings_file_and_writes_nothing_outside() {
        let d = dir("link");
        let outside = d.join("outside.yaml");
        std::fs::write(&outside, BINDINGS).unwrap();
        std::fs::create_dir(d.join("bindings")).unwrap();
        std::os::unix::fs::symlink(&outside, d.join("bindings/sdc.yaml")).unwrap();
        let refused = limit_call_budgets(&d.join("bindings"), 300_000).unwrap_err();
        assert!(refused.contains("sdc.yaml"), "{refused}");
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), BINDINGS, "the file the link points at was written");
        assert!(std::fs::symlink_metadata(d.join("bindings/sdc.yaml")).unwrap().file_type().is_symlink(), "the link was replaced by a file the run never read");
    }

    #[test]
    fn limiting_keeps_the_rework_members_of_a_budget_as_they_were() {
        let d = dir("rework");
        let text = BINDINGS.replace("budget: {max_runs: 3}", "budget: {max_runs: 3, runs_per: rejection, rework_limit: 2, rework_on: {code_review: [rejected], merge_approval: [rejected]}}");
        std::fs::write(d.join("sdc.yaml"), &text).unwrap();
        limit_call_budgets(&d, 300_000).unwrap();
        let v: Value = serde_yaml::from_str(&std::fs::read_to_string(d.join("sdc.yaml")).unwrap()).unwrap();
        let want: Value = serde_yaml::from_str("{max_runs: 3, runs_per: rejection, rework_limit: 2, rework_on: {code_review: [rejected], merge_approval: [rejected]}}").unwrap();
        assert_eq!(v["actions"][0]["budget"], want, "{v:?}");
    }

    /// Two program actions, `a` and `b`, whose argv name no budget; `top` stands at the top of the
    /// file and `a_member`, `b_member` on the actions (each a line, or empty).
    fn members_file(top: &str, a_member: &str, b_member: &str) -> String {
        format!("format: loom.bindings/1\n{top}actions:\n  - action: a\n{a_member}    executor:\n      kind: program\n      argv: [tools/a.sh]\n  - action: b\n{b_member}    executor:\n      kind: program\n      argv: [tools/b.sh, --flag]\n")
    }

    #[test]
    fn a_runbook_whose_programs_name_max_budget_usd_0_needs_no_budget_elsewhere() {
        let d = dir("member-zero");
        for (tag, text) in [("actions", members_file("", "    max_budget_usd: 0\n", "    max_budget_usd: 0\n")), ("file", members_file("max_budget_usd: 0\n", "", ""))] {
            let root = d.join(format!("{tag}.yaml"));
            std::fs::write(&root, text).unwrap();
            assert_eq!(reserve_for_unnamed(&root), Ok(0), "{tag}");
            assert_eq!(runbook_budgets(&root).unwrap().unnamed, None, "{tag}");
        }
        // One program with no budget from any source, and none anywhere: refused as before.
        let root = d.join("one-short.yaml");
        std::fs::write(&root, members_file("", "    max_budget_usd: 0\n", "")).unwrap();
        assert_eq!(runbook_budgets(&root).unwrap().unnamed.as_deref(), Some("b"));
    }

    #[test]
    fn a_member_that_disagrees_with_its_argv_or_is_no_number_of_usd_is_refused_naming_its_action_or_file() {
        let d = dir("member-refused");
        let disagree = BINDINGS.replace("  - action: b\n", "  - action: b\n    max_budget_usd: 0.4\n");
        let agree = BINDINGS.replace("  - action: b\n", "  - action: b\n    max_budget_usd: 0.5\n");
        std::fs::write(d.join("disagree.yaml"), disagree).unwrap();
        std::fs::write(d.join("agree.yaml"), agree).unwrap();
        let why = runbook_budgets(&d.join("disagree.yaml")).unwrap_err();
        assert!(why.contains("the action b of bindings/disagree.yaml") && why.contains("max_budget_usd") && why.contains("0.5") && why.contains("0.4"), "{why}");
        assert!(runbook_budgets(&d.join("agree.yaml")).is_ok());
        for (n, bad) in ["abc", "-1", ".nan", "-.inf", "\"0.3\"", "~", "false", "{usd: 1}"].iter().enumerate() {
            let on_action = d.join(format!("action{n}.yaml"));
            std::fs::write(&on_action, members_file("", &format!("    max_budget_usd: {bad}\n"), "")).unwrap();
            let why = runbook_budgets(&on_action).unwrap_err();
            assert!(why.contains(&format!("the action a of bindings/action{n}.yaml names max_budget_usd")), "{bad}: {why}");
            let on_file = d.join(format!("file{n}.yaml"));
            std::fs::write(&on_file, members_file(&format!("max_budget_usd: {bad}\n"), "", "")).unwrap();
            let why = runbook_budgets(&on_file).unwrap_err();
            assert!(why.contains(&format!("the bindings file bindings/file{n}.yaml names max_budget_usd")), "{bad}: {why}");
        }
        // A member on an action that runs no program is the budget of nothing.
        let open = d.join("open.yaml");
        std::fs::write(&open, "format: loom.bindings/1\nactions:\n  - action: c.open\n    max_budget_usd: 0\n    executor: {kind: open, name: c, bindings: none.yaml}\n").unwrap();
        assert!(runbook_budgets(&open).unwrap_err().contains("the action c.open of bindings/open.yaml names max_budget_usd and runs no program"));
    }

    #[test]
    fn the_largest_budget_counts_the_members_and_limiting_leaves_a_member_as_written() {
        let d = dir("member-largest");
        let text = BINDINGS.replace("  - action: b\n", "  - action: b\n    max_budget_usd: 0.5\n").replace("format: loom.bindings/1\n", "format: loom.bindings/1\nmax_budget_usd: 2.5\n");
        std::fs::write(d.join("sdc.yaml"), &text).unwrap();
        assert_eq!(largest_call_budget(&d).unwrap(), Some(2_500_000));
        limit_call_budgets(&d, 300_000).unwrap();
        let v: Value = serde_yaml::from_str(&std::fs::read_to_string(d.join("sdc.yaml")).unwrap()).unwrap();
        assert_eq!((v["max_budget_usd"].as_f64(), v["actions"][1]["max_budget_usd"].as_f64()), (Some(2.5), Some(0.5)), "{v:?}");
        assert_eq!(largest_call_budget(&d).unwrap(), Some(2_500_000), "a member is reserved as written");
    }

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| (*w).to_owned()).collect()
    }

    #[test]
    fn a_program_reserves_its_argv_budget_then_its_action_member_then_its_file_member_then_the_largest() {
        // The root names 0.05 for its file and opens a child. In the root, `a` names 0.3 in its argv, `b`
        // names 0.1 as its member, `c` names nothing itself (the file's 0.05). In the child, `d` names
        // nothing and its file names nothing: the root's member does not reach it, and it reserves the
        // largest of the runbook (0.3).
        let d = dir("member-order");
        let root = "format: loom.bindings/1\nmax_budget_usd: 0.05\nactions:\n  - action: a\n    executor: {kind: program, argv: [tools/a.sh, --max-budget-usd, \"0.3\"]}\n  - action: b\n    max_budget_usd: 0.1\n    executor: {kind: program, argv: [tools/b.sh]}\n  - action: c\n    executor: {kind: program, argv: [tools/c.sh]}\n  - action: k.open\n    executor: {kind: open, name: k, bindings: child.yaml}\n";
        let child = "format: loom.bindings/1\nactions:\n  - action: d\n    executor: {kind: program, argv: [tools/d.sh]}\n";
        std::fs::write(d.join("root.yaml"), root).unwrap();
        std::fs::write(d.join("child.yaml"), child).unwrap();
        let r = reserves(&d.join("root.yaml")).unwrap();
        let of = |p: &str, a: &[&str]| r.of(&d.join(p).display().to_string(), &args(a));
        assert_eq!(of("tools/a.sh", &["--max-budget-usd", "0.3"]), 300_000);
        assert_eq!(of("tools/b.sh", &[]), 100_000);
        assert_eq!(of("tools/c.sh", &[]), 50_000);
        assert_eq!(of("tools/d.sh", &[]), 300_000, "the root file's member reached a program of the child file");
        assert_eq!(runbook_budgets(&d.join("root.yaml")).unwrap().unnamed.as_deref(), Some("d"));
        // A program no binding runs, and one whose arguments differ from its binding's, reserve the largest.
        assert_eq!(of("tools/b.sh", &["--other"]), 300_000);
        // The child names its own member: it covers `d`.
        std::fs::write(d.join("child.yaml"), child.replace("actions:\n", "max_budget_usd: 0\nactions:\n")).unwrap();
        let r = reserves(&d.join("root.yaml")).unwrap();
        assert_eq!(r.of(&d.join("tools/d.sh").display().to_string(), &[]), 0);
        assert_eq!(runbook_budgets(&d.join("root.yaml")).unwrap().unnamed, None);
    }

    /// The model step of a runbook's programs (`docs/model-routes.md`, section 2): an agent node, or a program whose
    /// per-call budget, read as a run reserves it, is above 0. A budget of 0 is a program that calls no model.
    #[test]
    fn a_program_calls_a_model_when_it_is_an_agent_node_or_its_budget_is_above_0() {
        let d = dir("calls");
        let calls = |root: &str, child: Option<&str>| {
            std::fs::write(d.join("root.yaml"), root).unwrap();
            let _ = std::fs::remove_file(d.join("child.yaml"));
            if let Some(child) = child {
                std::fs::write(d.join("child.yaml"), child).unwrap();
            }
            calls_a_model(&d.join("root.yaml"))
        };
        let file = |actions: &str| format!("format: loom.bindings/1\nactions:\n{actions}");
        let program = |name: &str, argv: &str| format!("  - action: {name}\n    executor: {{kind: program, argv: [{argv}]}}\n");
        let zero = program("a", "tools/a.sh, --max-budget-usd, \"0\"");
        let open = "  - action: k.open\n    executor: {kind: open, name: k, bindings: child.yaml}\n";

        assert!(!calls(&file("  - action: r\n    executor: {kind: person, principal: reviewer}\n"), None), "no program");
        assert!(!calls(&file(&zero), None), "a program whose argv names 0");
        assert!(!calls(&file(&zero.replace("0\"", "0.000\"")), None), "0 written as 0.000");
        assert!(calls(&file(&program("a", "tools/a.sh, --max-budget-usd, \"0.01\"")), None), "a program whose argv names 0.01");
        assert!(calls(&file(&program("a", "tools/a.sh, --max-budget-usd=0.5")), None), "the joined form");
        assert!(!calls(&file("  - action: a\n    max_budget_usd: 0\n    executor: {kind: program, argv: [tools/a.sh]}\n"), None), "the action's member 0");
        assert!(!calls(&format!("format: loom.bindings/1\nmax_budget_usd: 0\nactions:\n{}", program("a", "tools/a.sh")), None), "the file's member 0");
        assert!(calls(&format!("format: loom.bindings/1\nmax_budget_usd: 0.2\nactions:\n{}", program("a", "tools/a.sh")), None), "the file's member 0.2");
        // the argv's budget comes before the member, as a run reserves it
        assert!(calls(&format!("format: loom.bindings/1\nmax_budget_usd: 0\nactions:\n{}", program("a", "tools/a.sh, --max-budget-usd, \"0.3\"")), None), "an argv 0.3 beside a file member 0");
        // a program that names no budget reserves the largest of the runbook
        assert!(!calls(&file(&format!("{zero}{}", program("b", "tools/b.sh"))), None), "an unnamed program where the largest is 0");
        assert!(calls(&file(&format!("{zero}{}{open}", program("b", "tools/b.sh"))), Some(&file(&program("c", "tools/c.sh, --max-budget-usd, \"0.3\"")))), "an unnamed program where a child names 0.3");
        // an agent node calls a model whatever its budget
        assert!(calls(&file(&program("n", "tools/sdc-node, n, --max-budget-usd, \"0\"")), None), "an agent node with budget 0");
        // a child's program counts; the root's member does not reach it
        assert!(calls(&format!("format: loom.bindings/1\nmax_budget_usd: 0\nactions:\n{open}"), Some(&file(&program("c", "tools/c.sh, --max-budget-usd, \"0.3\"")))), "a child's program with 0.3");
        assert!(!calls(&file(&format!("{zero}{open}")), Some(&format!("format: loom.bindings/1\nmax_budget_usd: 0\nactions:\n{}", program("c", "tools/c.sh")))), "a child whose file member is 0");
        // bindings a run is refused for are taken to call a model (the run is refused at load all the same)
        assert!(calls(&file(&program("a", "tools/a.sh")), None), "a program no budget covers");
        assert!(calls(&file("  - action: a\n    max_budget_usd: -1\n    executor: {kind: program, argv: [tools/a.sh]}\n"), None), "a refused member");
    }

    #[test]
    fn a_sealed_copy_holds_no_member_and_every_reader_reads_the_members_it_held() {
        let d = dir("seal");
        let root = "format: loom.bindings/1\nmax_budget_usd: 0.05\nactions:\n  - action: b\n    max_budget_usd: 0.1\n    executor: {kind: program, argv: [tools/b.sh]}\n  - action: c\n    executor: {kind: program, argv: [tools/c.sh]}\n  - action: k.open\n    executor: {kind: open, name: k, bindings: child.yaml}\n";
        let child = "format: loom.bindings/1\nactions:\n  - action: d\n    executor: {kind: program, argv: [tools/d.sh, --max-budget-usd, \"0.02\"]}\n";
        std::fs::write(d.join("root.yaml"), root).unwrap();
        std::fs::write(d.join("child.yaml"), child).unwrap();
        let before = reserves(&d.join("root.yaml")).unwrap();
        seal(&d.join("root.yaml")).unwrap();
        let text = std::fs::read_to_string(d.join("root.yaml")).unwrap();
        assert!(!text.contains(MEMBER), "{text}");
        assert_eq!(std::fs::read_to_string(d.join("child.yaml")).unwrap(), child, "a file with no member is not written");
        assert!(d.join(SEALED_FILE).is_file());
        assert_eq!(reserves(&d.join("root.yaml")).unwrap(), before);
        assert_eq!(largest_call_budget(&d).unwrap(), Some(100_000));
        // Sealing again changes nothing; a copy with no member keeps no file.
        seal(&d.join("root.yaml")).unwrap();
        assert_eq!(reserves(&d.join("root.yaml")).unwrap(), before);
        let plain = dir("seal-plain");
        std::fs::write(plain.join("sdc.yaml"), BINDINGS).unwrap();
        seal(&plain.join("sdc.yaml")).unwrap();
        assert_eq!(std::fs::read_to_string(plain.join("sdc.yaml")).unwrap(), BINDINGS);
        assert!(!plain.join(SEALED_FILE).exists());
        // A copy whose member a run refuses is not sealed.
        let bad = dir("seal-bad");
        std::fs::write(bad.join("sdc.yaml"), BINDINGS.replace("  - action: b\n", "  - action: b\n    max_budget_usd: -1\n")).unwrap();
        assert!(seal(&bad.join("sdc.yaml")).unwrap_err().contains("the action b of bindings/sdc.yaml"));
    }

    /// A kept file that names `budget` for the file `sdc.yaml`, in micro USD.
    fn kept(budget: u64) -> String {
        format!("{{\"format\":\"{SEALED_FORMAT}\",\"files\":{{\"sdc.yaml\":{{\"file\":{budget},\"actions\":{{}}}}}}}}\n")
    }

    #[test]
    fn a_link_at_the_kept_file_of_a_copy_is_not_read_for_the_largest_call_budget() {
        let d = dir("kept-link");
        let outside = d.join("outside.json");
        std::fs::write(&outside, kept(9_000_000)).unwrap();
        std::fs::create_dir(d.join("bindings")).unwrap();
        std::fs::write(d.join("bindings/sdc.yaml"), BINDINGS).unwrap();
        std::os::unix::fs::symlink(&outside, d.join("bindings").join(SEALED_FILE)).unwrap();
        let got = largest_call_budget(&d.join("bindings"));
        let refused = got.clone().expect_err(&format!("the kept file was read through a link: {got:?}"));
        assert!(refused.starts_with(SEALED_FILE), "the refusal names the kept file and no path: {refused}");
    }

    #[test]
    fn sealing_writes_through_no_link_at_a_bindings_file_or_at_the_kept_file() {
        let d = dir("seal-links");
        let child = "format: loom.bindings/1\nmax_budget_usd: 0.2\nactions:\n  - action: d\n    executor: {kind: program, argv: [tools/d.sh]}\n";
        let root = "format: loom.bindings/1\nactions:\n  - action: b\n    max_budget_usd: 0.1\n    executor: {kind: program, argv: [tools/b.sh]}\n  - action: k.open\n    executor: {kind: open, name: k, bindings: child.yaml}\n";
        std::fs::create_dir(d.join("bindings")).unwrap();
        std::fs::write(d.join("outside-child.yaml"), child).unwrap();
        std::fs::write(d.join("bindings/root.yaml"), root).unwrap();
        std::os::unix::fs::symlink(d.join("outside-child.yaml"), d.join("bindings/child.yaml")).unwrap();
        let _ = seal(&d.join("bindings/root.yaml"));
        assert_eq!(std::fs::read_to_string(d.join("outside-child.yaml")).unwrap(), child, "a file outside the copy was rewritten through a link at a bindings file");

        let e = dir("seal-kept-link");
        std::fs::create_dir(e.join("bindings")).unwrap();
        std::fs::write(e.join("outside.json"), kept(9_000_000)).unwrap();
        std::fs::write(e.join("bindings/sdc.yaml"), BINDINGS).unwrap();
        std::os::unix::fs::symlink(e.join("outside.json"), e.join("bindings").join(SEALED_FILE)).unwrap();
        let refused = seal(&e.join("bindings/sdc.yaml")).expect_err("the members of a file outside the copy were taken as the copy's");
        assert!(refused.starts_with(SEALED_FILE), "{refused}");
        assert_eq!(std::fs::read_to_string(e.join("outside.json")).unwrap(), kept(9_000_000));
    }

    #[test]
    fn nothing_is_written_when_no_budget_is_above_the_limit() {
        let d = dir("nochange");
        std::fs::write(d.join("sdc.yaml"), BINDINGS).unwrap();
        limit_call_budgets(&d, 5_000_000).unwrap();
        assert_eq!(std::fs::read_to_string(d.join("sdc.yaml")).unwrap(), BINDINGS);
    }
}
