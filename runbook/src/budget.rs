//! The budget of one call: before each step the bindings copy is limited so that no call may
//! spend more than the ledger granted. Only the `--max-budget-usd` argv value of a model tool is
//! touched: in a tree only `max_runs` acts, and Loom refuses `max_budget_micro_usd` (and
//! `max_turns`, `max_tokens`) at load on a non-agent binding, so this module never adds or keeps
//! such a member.

use serde_yaml::Value;
use std::path::{Path, PathBuf};

const ARG: &str = "--max-budget-usd";

fn yaml_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let p = entry.map_err(|e| format!("{}: {e}", dir.display()))?.path();
        if p.is_file() && p.extension().is_some_and(|x| x == "yaml" || x == "yml") {
            files.push(p);
        }
    }
    files.sort();
    Ok(files)
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
        Value::Number(n) => n.as_f64().and_then(|x| (x.is_finite() && x >= 0.0).then(|| (x * 1_000_000.0).round() as u64)),
        _ => None,
    }
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

/// What the bindings a daemon run reads say about per-call budgets: the root's file, each child
/// file an `open` action names (`executor.bindings`, beside the file that names it) and theirs.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RunbookBudgets {
    /// The largest per-call budget any of them names, micro USD.
    pub largest: Option<u64>,
    /// The first action whose program (`executor.argv`, or `arguments_from`) names no budget.
    pub unnamed: Option<String>,
}

fn names_budget(items: &[Value]) -> bool {
    (0..items.len()).any(|i| budget_at(items, i).is_some())
}

/// Every budget an action names is a number of USD of 0 or more, or the action is refused by name.
fn check_budgets(v: &Value, action: &str, file: &str) -> Result<(), String> {
    match v {
        Value::Sequence(items) => {
            for (i, item) in items.iter().enumerate() {
                if let Some(None) = budget_at(items, i) {
                    let given = match item.as_str().and_then(joined_value) {
                        Some(text) => format!("{text:?}"),
                        None => items.get(i + 1).map_or_else(|| "nothing".to_owned(), |n| serde_yaml::to_string(n).map_or_else(|_| "a value".to_owned(), |t| t.trim().to_owned())),
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

/// Reads the budgets of the bindings `root` names, as `RunbookBudgets` says. A budget that is no
/// number of USD of 0 or more is refused naming its action and file (`bindings/<file>`, no path). A
/// file that cannot be read or parsed adds nothing: Loom refuses it when the run opens it.
pub fn runbook_budgets(root: &Path) -> Result<RunbookBudgets, String> {
    let base = root.parent().unwrap_or(Path::new("."));
    let mut out = RunbookBudgets::default();
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut queue = vec![root.to_path_buf()];
    while let Some(file) = queue.pop() {
        if seen.contains(&file) {
            continue;
        }
        seen.push(file.clone());
        let Some(doc) = std::fs::read_to_string(&file).ok().and_then(|t| serde_yaml::from_str::<Value>(&t).ok()) else { continue };
        let name = format!("bindings/{}", file.strip_prefix(base).unwrap_or(&file).display());
        walk_largest(&doc, &mut out.largest);
        let Some(actions) = doc.get("actions").and_then(Value::as_sequence) else { continue };
        for a in actions {
            let action = a.get("action").and_then(Value::as_str).unwrap_or("(unnamed)");
            check_budgets(a, action, &name)?;
            for program in [a.get("executor").and_then(|e| e.get("argv")), a.get("arguments_from")].into_iter().flatten() {
                if program.as_sequence().is_some_and(|items| !names_budget(items)) && out.unnamed.is_none() {
                    out.unnamed = Some(action.to_owned());
                }
            }
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
    Ok(out)
}

/// What a program of a daemon run that names no per-call budget reserves: the largest budget the
/// runbook's bindings name (`runbook_budgets`), as a program that may call a model. A runbook with
/// such a program and no budget in any binding is refused, naming the reason: a run of it would be
/// unbound in money. A runbook with no program reserves 0.
pub fn reserve_for_unnamed(root: &Path) -> Result<u64, String> {
    let b = runbook_budgets(root)?;
    match (b.largest, b.unnamed) {
        (Some(m), _) => Ok(m),
        (None, None) => Ok(0),
        (None, Some(action)) => Err(format!(
            "the program of the action {action} names no per-call budget ({ARG}) and no binding of the runbook names one: a run of it could call a model unbound in money; name a budget in the arguments of a program"
        )),
    }
}

/// The largest per-call budget the bindings in `dir` name (`--max-budget-usd` values), micro USD; `None` when they name none.
pub fn largest_call_budget(dir: &Path) -> Result<Option<u64>, String> {
    let mut best = None;
    for file in yaml_files(dir)? {
        let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let v: Value = serde_yaml::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
        walk_largest(&v, &mut best);
    }
    Ok(best)
}

/// Lowers every per-call budget in the bindings copy `dir` to at most `micro_usd`.
pub fn limit_call_budgets(dir: &Path, micro_usd: u64) -> Result<(), String> {
    for file in yaml_files(dir)? {
        let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let mut v: Value = serde_yaml::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
        if walk_limit(&mut v, micro_usd) {
            let out = serde_yaml::to_string(&v).map_err(|e| format!("{}: {e}", file.display()))?;
            std::fs::write(&file, out).map_err(|e| format!("{}: {e}", file.display()))?;
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
    fn limiting_keeps_the_rework_members_of_a_budget_as_they_were() {
        let d = dir("rework");
        let text = BINDINGS.replace("budget: {max_runs: 3}", "budget: {max_runs: 3, runs_per: rejection, rework_limit: 2, rework_on: {code_review: [rejected], merge_approval: [rejected]}}");
        std::fs::write(d.join("sdc.yaml"), &text).unwrap();
        limit_call_budgets(&d, 300_000).unwrap();
        let v: Value = serde_yaml::from_str(&std::fs::read_to_string(d.join("sdc.yaml")).unwrap()).unwrap();
        let want: Value = serde_yaml::from_str("{max_runs: 3, runs_per: rejection, rework_limit: 2, rework_on: {code_review: [rejected], merge_approval: [rejected]}}").unwrap();
        assert_eq!(v["actions"][0]["budget"], want, "{v:?}");
    }

    #[test]
    fn nothing_is_written_when_no_budget_is_above_the_limit() {
        let d = dir("nochange");
        std::fs::write(d.join("sdc.yaml"), BINDINGS).unwrap();
        limit_call_budgets(&d, 5_000_000).unwrap();
        assert_eq!(std::fs::read_to_string(d.join("sdc.yaml")).unwrap(), BINDINGS);
    }
}
