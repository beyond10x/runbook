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

fn walk_largest(v: &Value, best: &mut Option<u64>) {
    match v {
        Value::Sequence(items) => {
            for (i, item) in items.iter().enumerate() {
                if item.as_str() == Some(ARG) {
                    if let Some(m) = items.get(i + 1).and_then(scalar_usd) {
                        *best = Some(best.map_or(m, |b| b.max(m)));
                    }
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
                if items[i].as_str() == Some(ARG) {
                    if let Some(m) = items.get(i + 1).and_then(scalar_usd) {
                        if m > limit {
                            items[i + 1] = Value::String(micro_to_usd_text(limit));
                            changed = true;
                        }
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

    const BINDINGS: &str = "format: loom.bindings/1\nactions:\n  - action: a\n    executor:\n      kind: program\n      argv: [tools/llm-node.sh, a, --max-turns, \"15\", --max-budget-usd, \"2\", --stub, x]\n    budget: {max_runs: 3}\n  - action: b\n    executor:\n      kind: program\n      argv: [tools/llm-node.sh, b, --max-budget-usd, \"0.5\"]\n";

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
    fn nothing_is_written_when_no_budget_is_above_the_limit() {
        let d = dir("nochange");
        std::fs::write(d.join("sdc.yaml"), BINDINGS).unwrap();
        limit_call_budgets(&d, 5_000_000).unwrap();
        assert_eq!(std::fs::read_to_string(d.join("sdc.yaml")).unwrap(), BINDINGS);
    }
}
